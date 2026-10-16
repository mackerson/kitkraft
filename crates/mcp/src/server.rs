//! JSON-RPC 2.0 over stdio, one message per line, speaking the Model Context Protocol.
//!
//! The server is dual-era (MCP 2026-07-28, "Versioning and Compatibility"):
//!
//! - **Modern** (`2026-07-28`): no handshake. Every request carries
//!   `params._meta["io.modelcontextprotocol/protocolVersion"]` and
//!   `["io.modelcontextprotocol/clientCapabilities"]`; a request missing either is
//!   `-32602`, an unsupported version `-32022` with the supported list. `server/discover`
//!   returns versions, capabilities and identity. Results carry `resultType: "complete"`
//!   and `_meta["io.modelcontextprotocol/serverInfo"]`; list and read results carry
//!   `ttlMs`/`cacheScope`. `initialize` and `ping` do not exist in this era (`-32601`).
//! - **Legacy** (`2025-11-25`, `2025-06-18`): `initialize` negotiates the version (the
//!   client's if we know it, else `2025-11-25`), then `notifications/initialized`,
//!   `ping`, and the same `tools/*` and `resources/*` methods. Requests before
//!   `initialize` are served too (lenient; nothing depends on the handshake). A request
//!   whose `_meta` names one of these versions is served as legacy. `2025-03-26` and
//!   `2024-11-05` are not offered: they require JSON-RPC batches, which this server
//!   refuses (`-32600`); a client asking for them is offered `2025-11-25`.
//!
//! Every version served is listed in `server/discover`'s `supportedVersions` and in a
//! `-32022` error's `supported`, newest first.
//!
//! Methods: `server/discover`, `initialize`, `ping`, `tools/list`, `tools/call`,
//! `resources/list`, `resources/read`, `resources/templates/list`; notifications are
//! accepted and never answered. Malformed JSON is `-32700`, a non-request `-32600`, an
//! unknown method (or one the request's era lacks) `-32601`, bad params `-32602`. A line longer than
//! [`MAX_MESSAGE_BYTES`] is refused (`-32600`) without buffering it. Nothing here
//! panics on input.
//!
//! Requests are handled one at a time, in order; `notifications/cancelled` cannot
//! interrupt a running command and is ignored.

use std::io::{self, BufRead, Write};

use serde_json::{json, Map, Value};

use crate::backend::{Backend, CallError};
use crate::tools;
use kitkraft_engine::Engine;

/// The protocol revision this server implements natively (per-request `_meta`).
pub const PROTOCOL_VERSION: &str = "2026-07-28";
/// Modern revisions accepted in `_meta`.
pub const MODERN_VERSIONS: &[&str] = &[PROTOCOL_VERSION];
/// Handshake-era revisions accepted by `initialize`, newest first. Older revisions
/// (`2025-03-26`, `2024-11-05`) require JSON-RPC batch support, which this server does
/// not have, so a client asking for them is offered the first entry instead.
pub const LEGACY_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18"];
/// Every revision this process serves, newest first: [`MODERN_VERSIONS`] then
/// [`LEGACY_VERSIONS`] (`server/discover` and `-32022` name these).
pub const SUPPORTED_VERSIONS: &[&str] = &[PROTOCOL_VERSION, "2025-11-25", "2025-06-18"];
/// Longest accepted message line in bytes (without the newline).
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// JSON-RPC error codes used here.
pub mod code {
    /// Invalid JSON.
    pub const PARSE_ERROR: i64 = -32700;
    /// Not a valid request object.
    pub const INVALID_REQUEST: i64 = -32600;
    /// No such method.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// Bad parameters (also: unknown tool, unknown resource on 2026-07-28).
    pub const INVALID_PARAMS: i64 = -32602;
    /// A bug on our side.
    pub const INTERNAL_ERROR: i64 = -32603;
    /// Resource not found, handshake-era revisions only.
    pub const LEGACY_RESOURCE_NOT_FOUND: i64 = -32002;
    /// MCP 2026-07-28 `UnsupportedProtocolVersionError`.
    pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;
}

/// Caching hint for results that only change with the binary (tools, catalog).
const STATIC_TTL_MS: u64 = 3_600_000;

/// A JSON-RPC error object.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    /// Error code.
    pub code: i64,
    /// Human-readable message.
    pub message: String,
    /// Optional structured detail.
    pub data: Option<Value>,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("code".into(), Value::from(self.code));
        obj.insert("message".into(), Value::String(self.message.clone()));
        if let Some(data) = &self.data {
            obj.insert("data".into(), data.clone());
        }
        Value::Object(obj)
    }
}

/// Which era a request belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Era {
    Modern,
    Legacy,
}

/// The MCP server: one backend (one engine, one session) per process.
#[derive(Debug)]
pub struct Server {
    backend: Backend,
    legacy_version: Option<String>,
    notes: Vec<String>,
}

impl Server {
    /// A server over `engine`.
    pub fn new(engine: Engine) -> Self {
        Self {
            backend: Backend::new(engine),
            legacy_version: None,
            notes: Vec::new(),
        }
    }

    /// The backend (for preloading the session before serving).
    pub fn backend(&self) -> &Backend {
        &self.backend
    }

    /// Adds a sentence to the `instructions` clients show the model (for example the
    /// result of a `--profile` preload).
    pub fn add_note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// The legacy revision `initialize` negotiated, if any.
    pub fn negotiated_version(&self) -> Option<&str> {
        self.legacy_version.as_deref()
    }

    /// Reads messages from `input` until end of file, writing one response line per
    /// request to `output`. Returns on EOF; I/O errors are returned.
    pub fn serve<R: BufRead, W: Write>(&mut self, mut input: R, mut output: W) -> io::Result<()> {
        loop {
            let reply = match read_frame(&mut input, MAX_MESSAGE_BYTES)? {
                Frame::Eof => return Ok(()),
                Frame::Oversize => Some(
                    error_response(
                        Value::Null,
                        &RpcError::new(
                            code::INVALID_REQUEST,
                            format!("message longer than {MAX_MESSAGE_BYTES} bytes"),
                        ),
                    )
                    .to_string(),
                ),
                Frame::Line(bytes) => match std::str::from_utf8(&bytes) {
                    Ok(text) => self.handle_line(text),
                    Err(e) => Some(
                        error_response(
                            Value::Null,
                            &RpcError::new(code::PARSE_ERROR, format!("not UTF-8: {e}")),
                        )
                        .to_string(),
                    ),
                },
            };
            if let Some(reply) = reply {
                output.write_all(reply.as_bytes())?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
    }

    /// Handles one line; returns the response line, or `None` for a notification, a
    /// client response, or a blank line.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        if line.trim().is_empty() {
            return None;
        }
        let response = match serde_json::from_str::<Value>(line) {
            Ok(message) => self.handle_message(message)?,
            Err(e) => error_response(
                Value::Null,
                &RpcError::new(code::PARSE_ERROR, format!("parse error: {e}")),
            ),
        };
        Some(response.to_string())
    }

    /// Handles one parsed message; returns the response, or `None` when none is due.
    pub fn handle_message(&mut self, message: Value) -> Option<Value> {
        let Value::Object(message) = message else {
            let what = if message.is_array() {
                "batches are not supported"
            } else {
                "a message must be a JSON object"
            };
            return Some(error_response(
                Value::Null,
                &RpcError::new(code::INVALID_REQUEST, what),
            ));
        };
        let id = message.get("id");
        if let Some(id) = id {
            if !valid_id(id) {
                return Some(error_response(
                    Value::Null,
                    &RpcError::new(code::INVALID_REQUEST, "id must be a string or an integer"),
                ));
            }
        }
        let reply_id = id.cloned().unwrap_or(Value::Null);
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return id.map(|_| {
                error_response(
                    reply_id.clone(),
                    &RpcError::new(code::INVALID_REQUEST, "jsonrpc must be \"2.0\""),
                )
            });
        }
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            if message.contains_key("result") || message.contains_key("error") {
                // A response to a request we never send; nothing to answer.
                return None;
            }
            return Some(error_response(
                reply_id,
                &RpcError::new(code::INVALID_REQUEST, "method must be a string"),
            ));
        };
        let empty = Value::Object(Map::new());
        let params = match message.get("params") {
            None | Some(Value::Null) => &empty,
            Some(p @ Value::Object(_)) => p,
            Some(_) => {
                return id.map(|_| {
                    error_response(
                        reply_id.clone(),
                        &RpcError::new(code::INVALID_PARAMS, "params must be an object"),
                    )
                });
            }
        };
        // Notifications (no id: `notifications/initialized`, `notifications/cancelled`,
        // ...) need no action and must not be answered.
        id?;
        let response = match self.dispatch(method, params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": reply_id, "result": result }),
            Err(err) => error_response(reply_id, &err),
        };
        Some(response)
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, RpcError> {
        let era = era_of(params)?;
        let result = match (method, era) {
            ("initialize", Era::Legacy) => self.initialize(params),
            ("server/discover", Era::Modern) => Ok(self.discover()),
            ("server/discover", Era::Legacy) => Err(missing_meta(META_VERSION)),
            ("ping", Era::Legacy) => Ok(json!({})),
            ("tools/list", _) => self.tools_list(params, era),
            ("tools/call", _) => self.tools_call(params),
            ("resources/list", _) => self.resources_list(params, era),
            ("resources/templates/list", _) => Ok(json!({ "resourceTemplates": [] })),
            ("resources/read", _) => self.resources_read(params, era),
            ("initialize" | "ping", Era::Modern) => Err(RpcError::new(
                code::METHOD_NOT_FOUND,
                format!("method not found: {method} (not part of MCP {PROTOCOL_VERSION})"),
            )),
            _ => Err(RpcError::new(
                code::METHOD_NOT_FOUND,
                format!("method not found: {method}"),
            )),
        }?;
        Ok(match era {
            Era::Modern => modern_result(result),
            Era::Legacy => result,
        })
    }

    fn initialize(&mut self, params: &Value) -> Result<Value, RpcError> {
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RpcError::new(
                    code::INVALID_PARAMS,
                    "initialize: protocolVersion (string) is required",
                )
            })?;
        let version = LEGACY_VERSIONS
            .iter()
            .find(|v| **v == requested)
            .or(LEGACY_VERSIONS.first())
            .copied()
            .unwrap_or_default();
        self.legacy_version = Some(version.to_string());
        Ok(json!({
            "protocolVersion": version,
            "capabilities": capabilities(),
            "serverInfo": server_info(),
            "instructions": self.instructions(),
        }))
    }

    fn discover(&self) -> Value {
        json!({
            "supportedVersions": SUPPORTED_VERSIONS,
            "capabilities": capabilities(),
            "instructions": self.instructions(),
            "ttlMs": STATIC_TTL_MS,
            "cacheScope": "public",
        })
    }

    fn instructions(&self) -> String {
        let shortcuts: Vec<&str> = tools::TOOLS
            .iter()
            .filter(|t| matches!(t.target, tools::Target::Command(_)))
            .map(|t| t.name)
            .collect();
        let mut text = format!(
            "KitKraft: clean-room Starfield plugin tooling ({} catalog commands). Call `execute` with {{command, args}} (each command and its arguments is listed in its description and in the kitkraft://commands resource) or `command_line` with a kit command line; {} are shortcuts. Every result is the evidence envelope {{command, data, coverage, limitations, evidence}}: report unknown as unknown and cite evidence offsets. A load order loaded with `execute` `load` stays loaded for this server process (resource kitkraft://load-order). record_schema shows which fields a record type has, with sources and confidence; kitkraft://validate-rules lists what `validate` checks.",
            self.backend.command_count(),
            shortcuts.join(", ")
        );
        for note in &self.notes {
            text.push(' ');
            text.push_str(note);
        }
        text
    }

    fn tools_list(&self, params: &Value, era: Era) -> Result<Value, RpcError> {
        reject_cursor(params)?;
        let tools = tools::definitions().map_err(|e| internal(&e.to_string()))?;
        let mut result = json!({ "tools": tools });
        if era == Era::Modern {
            add_cache(&mut result, STATIC_TTL_MS, "public");
        }
        Ok(result)
    }

    fn tools_call(&self, params: &Value) -> Result<Value, RpcError> {
        let name = params.get("name").and_then(Value::as_str).ok_or_else(|| {
            RpcError::new(
                code::INVALID_PARAMS,
                "tools/call: name (string) is required",
            )
        })?;
        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => Value::Null,
            Some(a @ Value::Object(_)) => a.clone(),
            Some(_) => {
                return Err(RpcError::new(
                    code::INVALID_PARAMS,
                    "tools/call: arguments must be an object",
                ))
            }
        };
        match self.backend.call_tool(name, arguments) {
            Ok(out) => Ok(json!({
                "content": [{ "type": "text", "text": out.text }],
                "structuredContent": out.structured,
                "isError": out.is_error,
            })),
            Err(CallError::UnknownTool(name)) => Err(RpcError::new(
                code::INVALID_PARAMS,
                format!("unknown tool: {name}"),
            )),
            Err(CallError::Internal(message)) => Err(internal(&message)),
        }
    }

    fn resources_list(&self, params: &Value, era: Era) -> Result<Value, RpcError> {
        reject_cursor(params)?;
        let mut result = json!({ "resources": self.backend.resources() });
        if era == Era::Modern {
            add_cache(&mut result, STATIC_TTL_MS, "public");
        }
        Ok(result)
    }

    fn resources_read(&self, params: &Value, era: Era) -> Result<Value, RpcError> {
        let uri = params.get("uri").and_then(Value::as_str).ok_or_else(|| {
            RpcError::new(
                code::INVALID_PARAMS,
                "resources/read: uri (string) is required",
            )
        })?;
        let text = match self.backend.read_resource(uri) {
            Some(Ok(text)) => text,
            Some(Err(CallError::Internal(message) | CallError::UnknownTool(message))) => {
                return Err(internal(&message))
            }
            None => {
                let code = match era {
                    Era::Modern => code::INVALID_PARAMS,
                    Era::Legacy => code::LEGACY_RESOURCE_NOT_FOUND,
                };
                return Err(RpcError {
                    code,
                    message: format!("resource not found: {uri}"),
                    data: Some(json!({ "uri": uri })),
                });
            }
        };
        let mut result = json!({
            "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }]
        });
        if era == Era::Modern {
            // The load order changes with every `load`; the catalog only with the binary.
            let (ttl, scope) = if crate::backend::STATIC_URIS.contains(&uri) {
                (STATIC_TTL_MS, "public")
            } else {
                (0, "private")
            };
            add_cache(&mut result, ttl, scope);
        }
        Ok(result)
    }
}

/// The era of a request from its `_meta`, checking the modern required fields. No
/// version, or a handshake-era one, is legacy.
fn era_of(params: &Value) -> Result<Era, RpcError> {
    let meta = params.get("_meta").and_then(Value::as_object);
    let Some(version) = meta.and_then(|m| m.get(META_VERSION)) else {
        return Ok(Era::Legacy);
    };
    let Some(version) = version.as_str() else {
        return Err(RpcError::new(
            code::INVALID_PARAMS,
            format!("_meta[\"{META_VERSION}\"] must be a string"),
        ));
    };
    if LEGACY_VERSIONS.contains(&version) {
        return Ok(Era::Legacy);
    }
    if !MODERN_VERSIONS.contains(&version) {
        return Err(RpcError {
            code: code::UNSUPPORTED_PROTOCOL_VERSION,
            message: "Unsupported protocol version".to_string(),
            data: Some(json!({ "supported": SUPPORTED_VERSIONS, "requested": version })),
        });
    }
    if !meta
        .and_then(|m| m.get(META_CAPABILITIES))
        .is_some_and(Value::is_object)
    {
        return Err(missing_meta(META_CAPABILITIES));
    }
    Ok(Era::Modern)
}

fn missing_meta(key: &str) -> RpcError {
    RpcError::new(
        code::INVALID_PARAMS,
        format!("missing required _meta[\"{key}\"] (MCP {PROTOCOL_VERSION} per-request metadata)"),
    )
}

/// We never hand out a cursor, so any cursor is invalid.
fn reject_cursor(params: &Value) -> Result<(), RpcError> {
    match params.get("cursor") {
        None | Some(Value::Null) => Ok(()),
        Some(_) => Err(RpcError::new(
            code::INVALID_PARAMS,
            "invalid cursor: this server returns every item in one page",
        )),
    }
}

fn internal(message: &str) -> RpcError {
    RpcError::new(code::INTERNAL_ERROR, format!("internal error: {message}"))
}

fn capabilities() -> Value {
    json!({
        "tools": { "listChanged": false },
        "resources": { "subscribe": false, "listChanged": false }
    })
}

fn server_info() -> Value {
    json!({
        "name": "kitkraft",
        "title": "KitKraft",
        "version": env!("CARGO_PKG_VERSION"),
    })
}

/// Adds the fields every modern result carries.
fn modern_result(mut result: Value) -> Value {
    if let Some(obj) = result.as_object_mut() {
        obj.insert("resultType".into(), Value::String("complete".into()));
        let meta = obj
            .entry("_meta")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(meta) = meta.as_object_mut() {
            meta.insert(META_SERVER_INFO.into(), server_info());
        }
    }
    result
}

fn add_cache(result: &mut Value, ttl_ms: u64, scope: &str) {
    if let Some(obj) = result.as_object_mut() {
        obj.insert("ttlMs".into(), Value::from(ttl_ms));
        obj.insert("cacheScope".into(), Value::String(scope.to_string()));
    }
}

/// JSON-RPC ids are strings or integers; MCP also forbids `null`.
fn valid_id(id: &Value) -> bool {
    match id {
        Value::String(_) => true,
        Value::Number(n) => n.is_i64() || n.is_u64(),
        _ => false,
    }
}

fn error_response(id: Value, err: &RpcError) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": err.to_json() })
}

/// One line read from the input.
#[derive(Debug, PartialEq, Eq)]
enum Frame {
    /// A line without its newline (a trailing `\r` is kept; JSON treats it as space).
    Line(Vec<u8>),
    /// A line longer than the cap; its bytes were discarded.
    Oversize,
    /// End of input.
    Eof,
}

/// Reads one `\n`-terminated line, keeping at most `max` bytes in memory.
fn read_frame<R: BufRead>(input: &mut R, max: usize) -> io::Result<Frame> {
    let mut line = Vec::new();
    let mut oversize = false;
    let mut started = false;
    loop {
        let available = match input.fill_buf() {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            return Ok(match (started, oversize) {
                (false, _) => Frame::Eof,
                (true, true) => Frame::Oversize,
                (true, false) => Frame::Line(line),
            });
        }
        started = true;
        let newline = available.iter().position(|b| *b == b'\n');
        let chunk = match newline {
            Some(at) => available.get(..at).unwrap_or_default(),
            None => available,
        };
        if !oversize {
            if line.len().saturating_add(chunk.len()) > max {
                oversize = true;
                line = Vec::new();
            } else {
                line.extend_from_slice(chunk);
            }
        }
        let used = match newline {
            Some(at) => at.saturating_add(1),
            None => available.len(),
        };
        input.consume(used);
        if newline.is_some() {
            return Ok(if oversize {
                Frame::Oversize
            } else {
                Frame::Line(line)
            });
        }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::{
        read_frame, Frame, Server, LEGACY_VERSIONS, MODERN_VERSIONS, PROTOCOL_VERSION,
        SUPPORTED_VERSIONS,
    };
    use kitkraft_engine::Engine;
    use serde_json::{json, Value};

    fn server() -> Server {
        Server::new(Engine::with_default_handlers())
    }

    fn call(server: &mut Server, message: Value) -> Value {
        let line = server.handle_line(&message.to_string()).unwrap();
        assert!(!line.contains('\n'), "one message per line");
        serde_json::from_str(&line).unwrap()
    }

    fn meta() -> Value {
        json!({
            "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
            "io.modelcontextprotocol/clientCapabilities": {},
            "io.modelcontextprotocol/clientInfo": {"name": "test", "version": "0"}
        })
    }

    fn modern(id: i64, method: &str, mut params: Value) -> Value {
        params["_meta"] = meta();
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
    }

    fn legacy(id: i64, method: &str, params: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
    }

    #[test]
    fn initialize_negotiates_a_legacy_version() {
        let mut s = server();
        let r = call(
            &mut s,
            legacy(
                1,
                "initialize",
                json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "c", "version": "1"}}),
            ),
        );
        assert_eq!(r["id"], 1);
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "kitkraft");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        assert!(r["result"]["capabilities"]["resources"].is_object());
        assert!(r["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("execute"));
        assert!(r["result"].get("resultType").is_none());
        assert_eq!(s.negotiated_version(), Some("2025-06-18"));

        // An unknown version gets our newest legacy one.
        let r = call(
            &mut s,
            legacy(2, "initialize", json!({"protocolVersion": "2099-01-01"})),
        );
        assert_eq!(r["result"]["protocolVersion"], LEGACY_VERSIONS[0]);
        // Missing version: invalid params.
        let r = call(&mut s, legacy(3, "initialize", json!({})));
        assert_eq!(r["error"]["code"], -32602);
    }

    /// The batch-era revisions are not offered: a client asking for them is answered
    /// with our newest legacy revision, which has no batches.
    #[test]
    fn batch_era_versions_negotiate_to_the_newest_legacy_one() {
        assert!(!LEGACY_VERSIONS.contains(&"2025-03-26"));
        assert!(!LEGACY_VERSIONS.contains(&"2024-11-05"));
        let mut s = server();
        for (id, requested) in [(1, "2025-03-26"), (2, "2024-11-05")] {
            let r = call(
                &mut s,
                legacy(id, "initialize", json!({"protocolVersion": requested})),
            );
            assert_eq!(r["result"]["protocolVersion"], "2025-11-25", "{requested}");
        }
        assert_eq!(s.negotiated_version(), Some("2025-11-25"));
        // A batch is still refused.
        let line = s
            .handle_line(r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#)
            .unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["error"]["code"], -32600);
    }

    #[test]
    fn supported_versions_name_every_era() {
        let expected: Vec<&str> = MODERN_VERSIONS
            .iter()
            .chain(LEGACY_VERSIONS)
            .copied()
            .collect();
        assert_eq!(SUPPORTED_VERSIONS, expected.as_slice());
        // A request whose _meta names a legacy version is served as legacy (no
        // clientCapabilities needed, no modern result fields).
        let mut s = server();
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "ping", "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2025-06-18"}}});
        assert_eq!(call(&mut s, msg)["result"], json!({}));
    }

    /// `initialize` and `ping` belong to the handshake era: a modern request for either
    /// is method-not-found, and an unsupported version is -32022 even for `initialize`.
    #[test]
    fn handshake_methods_are_legacy_only() {
        let mut s = server();
        let r = call(
            &mut s,
            modern(1, "initialize", json!({"protocolVersion": "2025-11-25"})),
        );
        assert_eq!(r["error"]["code"], -32601);
        assert_eq!(s.negotiated_version(), None);
        let msg = json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {
            "protocolVersion": "2025-11-25",
            "_meta": {"io.modelcontextprotocol/protocolVersion": "1900-01-01",
                      "io.modelcontextprotocol/clientCapabilities": {}}}});
        assert_eq!(call(&mut s, msg)["error"]["code"], -32022);
    }

    #[test]
    fn notifications_get_no_reply() {
        let mut s = server();
        for method in [
            "notifications/initialized",
            "notifications/cancelled",
            "notifications/whatever",
        ] {
            let msg = json!({"jsonrpc": "2.0", "method": method, "params": {"requestId": 1}});
            assert_eq!(s.handle_line(&msg.to_string()), None, "{method}");
        }
        // A response from the client is not answered either.
        assert_eq!(
            s.handle_line(r#"{"jsonrpc":"2.0","id":9,"result":{}}"#),
            None
        );
        assert_eq!(s.handle_line("   "), None);
    }

    #[test]
    fn ping_is_legacy_only() {
        let mut s = server();
        let r = call(&mut s, legacy(1, "ping", json!(null)));
        assert_eq!(r["result"], json!({}));
        let r = call(&mut s, modern(2, "ping", json!({})));
        assert_eq!(r["error"]["code"], -32601);
    }

    #[test]
    fn discover_and_modern_metadata() {
        let mut s = server();
        let r = call(&mut s, modern(1, "server/discover", json!({})));
        let result = &r["result"];
        assert_eq!(result["supportedVersions"], json!(SUPPORTED_VERSIONS));
        assert_eq!(result["supportedVersions"][0], PROTOCOL_VERSION);
        assert_eq!(result["resultType"], "complete");
        assert!(result["capabilities"]["tools"].is_object());
        assert!(result["ttlMs"].is_u64());
        assert_eq!(result["cacheScope"], "public");
        assert_eq!(
            result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "kitkraft"
        );

        // Without _meta, discover is a missing-field error (the legacy-probe signal).
        let r = call(&mut s, legacy(2, "server/discover", json!({})));
        assert_eq!(r["error"]["code"], -32602);

        // Unsupported version: -32022 with the supported list.
        let msg = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "1900-01-01",
            "io.modelcontextprotocol/clientCapabilities": {}}}});
        let r = call(&mut s, msg);
        assert_eq!(r["error"]["code"], -32022);
        assert_eq!(r["error"]["data"]["supported"], json!(SUPPORTED_VERSIONS));
        assert_eq!(r["error"]["data"]["requested"], "1900-01-01");

        // Missing clientCapabilities: -32602.
        let msg = json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list", "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION}}});
        assert_eq!(call(&mut s, msg)["error"]["code"], -32602);
        // Version of the wrong type: -32602.
        let msg = json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list", "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": 2026, "io.modelcontextprotocol/clientCapabilities": {}}}});
        assert_eq!(call(&mut s, msg)["error"]["code"], -32602);
    }

    #[test]
    fn tools_list_in_both_eras() {
        let mut s = server();
        let legacy_list = call(&mut s, legacy(1, "tools/list", json!({})));
        let names: Vec<&str> = legacy_list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "execute",
                "command_line",
                "inspect_record",
                "query_records",
                "diff_plugins",
                "coverage",
                "record_schema",
                "roundtrip"
            ]
        );
        let modern_list = call(&mut s, modern(2, "tools/list", json!({})));
        assert_eq!(
            modern_list["result"]["tools"],
            legacy_list["result"]["tools"]
        );
        assert_eq!(modern_list["result"]["resultType"], "complete");
        assert!(modern_list["result"]["ttlMs"].is_u64());
        let r = call(&mut s, legacy(3, "tools/list", json!({"cursor": "x"})));
        assert_eq!(r["error"]["code"], -32602);
    }

    #[test]
    fn tools_call_maps_errors() {
        let mut s = server();
        // Unknown tool: protocol error.
        let r = call(
            &mut s,
            legacy(1, "tools/call", json!({"name": "nope", "arguments": {}})),
        );
        assert_eq!(r["error"]["code"], -32602);
        // Missing name, non-object arguments: protocol errors.
        let r = call(&mut s, legacy(2, "tools/call", json!({"arguments": {}})));
        assert_eq!(r["error"]["code"], -32602);
        let r = call(
            &mut s,
            legacy(
                3,
                "tools/call",
                json!({"name": "execute", "arguments": [1]}),
            ),
        );
        assert_eq!(r["error"]["code"], -32602);

        let tool_error = |r: &Value, kind: &str| {
            assert_eq!(r["result"]["isError"], true, "{r}");
            assert_eq!(
                r["result"]["structuredContent"]["error"]["kind"], kind,
                "{r}"
            );
            let text: Value =
                serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
            assert_eq!(text, r["result"]["structuredContent"]);
        };
        // invalid_args through a shortcut and through execute.
        let r = call(
            &mut s,
            legacy(
                4,
                "tools/call",
                json!({"name": "roundtrip", "arguments": {}}),
            ),
        );
        tool_error(&r, "invalid_args");
        assert_eq!(
            r["result"]["structuredContent"]["error"]["violations"][0]["pointer"],
            "/plugin"
        );
        let r = call(
            &mut s,
            legacy(
                5,
                "tools/call",
                json!({"name": "execute", "arguments": {"command": "roundtrip", "args": {"verify": "yes"}}}),
            ),
        );
        tool_error(&r, "invalid_args");
        let r = call(
            &mut s,
            legacy(6, "tools/call", json!({"name": "execute", "arguments": {}})),
        );
        tool_error(&r, "invalid_args");
        // unknown_command.
        let r = call(
            &mut s,
            legacy(
                7,
                "tools/call",
                json!({"name": "execute", "arguments": {"command": "frobnicate"}}),
            ),
        );
        tool_error(&r, "unknown_command");
        // usage (command_line).
        let r = call(
            &mut s,
            legacy(
                8,
                "tools/call",
                json!({"name": "command_line", "arguments": {"text": "roundtrip X.esm --bogus"}}),
            ),
        );
        tool_error(&r, "usage");
        // failed (build is implemented; the spec does not exist). No catalog command is
        // Planned any more, so not_implemented is covered by tests/stdio.rs on an empty
        // engine.
        let r = call(
            &mut s,
            modern(
                9,
                "tools/call",
                json!({"name": "execute", "arguments": {"command": "build", "args": {"spec": "a.toml", "out": "b.esm"}}}),
            ),
        );
        tool_error(&r, "failed");
        assert_eq!(r["result"]["resultType"], "complete");
        // failed (missing file).
        let r = call(
            &mut s,
            legacy(
                10,
                "tools/call",
                json!({"name": "coverage", "arguments": {"plugin": "Z:\\no\\such\\plugin.esm"}}),
            ),
        );
        tool_error(&r, "failed");
        // An alias is accepted by execute.
        let r = call(
            &mut s,
            legacy(
                11,
                "tools/call",
                json!({"name": "execute", "arguments": {"command": "show", "args": {}}}),
            ),
        );
        tool_error(&r, "invalid_args");
        assert_eq!(
            r["result"]["structuredContent"]["error"]["violations"][0]["pointer"],
            "/form"
        );
    }

    /// The support-depth matrix over MCP (lane D): the `coverage` tool with `matrix`, on a
    /// header-only plugin and the checked-in evidence file.
    #[test]
    fn coverage_matrix_over_mcp() {
        let mut s = server();
        let dir = std::env::temp_dir().join(format!("kitkraft-mcp-matrix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let plugin = dir.join("Header.esm");
        let mut bytes = b"TES4".to_vec();
        bytes.extend_from_slice(&18u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(b"HEDR");
        bytes.extend_from_slice(&12u16.to_le_bytes());
        bytes.extend_from_slice(&1.0f32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0x800u32.to_le_bytes());
        std::fs::write(&plugin, bytes).unwrap();
        let evidence = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/schema/evidence.toml"
        );
        let r = call(
            &mut s,
            legacy(
                1,
                "tools/call",
                json!({"name": "coverage", "arguments": {
                    "matrix": true, "plugin": plugin.display().to_string(),
                    "evidence": evidence, "types": ["WEAP", "LCTN"],
                }}),
            ),
        );
        assert_eq!(r["result"]["isError"], false, "{r}");
        let rows = r["result"]["structuredContent"]["data"]["rows"]
            .as_array()
            .unwrap();
        let row = |t: &str| rows.iter().find(|r| r["type"] == t).unwrap();
        // No instance in this plugin: measured statuses are not-run; the evidence file
        // still says what the game and the tests have shown.
        assert_eq!(row("WEAP")["parsed"]["status"], "not-run");
        assert_eq!(row("WEAP")["game_runtime_tested"]["status"], "pass");
        assert_eq!(row("LCTN")["typed_writable"]["status"], "pass");
        assert_eq!(row("LCTN")["save_reload_verified"]["status"], "not-run");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn command_line_help() {
        let mut s = server();
        let r = call(
            &mut s,
            legacy(
                1,
                "tools/call",
                json!({"name": "command_line", "arguments": {"text": "help roundtrip"}}),
            ),
        );
        assert_eq!(r["result"]["isError"], false);
        let help = r["result"]["structuredContent"]["help"].as_str().unwrap();
        assert!(help.starts_with("usage: roundtrip <plugin>"), "{help}");
        let r = call(
            &mut s,
            legacy(
                2,
                "tools/call",
                json!({"name": "command_line", "arguments": {"text": "help"}}),
            ),
        );
        assert!(r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("ba2 extract"));
    }

    #[test]
    fn resources() {
        let mut s = server();
        let r = call(&mut s, legacy(1, "resources/list", json!({})));
        let uris: Vec<&str> = r["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["uri"].as_str().unwrap())
            .collect();
        assert_eq!(
            uris,
            [
                "kitkraft://commands",
                "kitkraft://load-order",
                "kitkraft://validate-rules"
            ]
        );
        let r = call(
            &mut s,
            modern(
                8,
                "resources/read",
                json!({"uri": "kitkraft://validate-rules"}),
            ),
        );
        assert_eq!(r["result"]["cacheScope"], "public", "{r}");
        let text = r["result"]["contents"][0]["text"].as_str().unwrap();
        let rules: Value = serde_json::from_str(text).unwrap();
        let ids: Vec<&str> = rules["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        let expected: Vec<&str> = kitkraft_engine::validate::RULES
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, expected);
        assert_eq!(rules["rules"][0]["severity"], "error");
        assert_eq!(rules["docs"], "docs/validate.md");
        assert_eq!(rules["denylist"][0]["function"], "setgodmode");
        let r = call(
            &mut s,
            legacy(2, "resources/read", json!({"uri": "kitkraft://commands"})),
        );
        let text = r["result"]["contents"][0]["text"].as_str().unwrap();
        let catalog: Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            catalog.as_array().unwrap().len(),
            kitkraft_engine::catalog().len()
        );
        let r = call(
            &mut s,
            modern(3, "resources/read", json!({"uri": "kitkraft://load-order"})),
        );
        let text = r["result"]["contents"][0]["text"].as_str().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(text).unwrap()["loaded"],
            false
        );
        assert_eq!(r["result"]["cacheScope"], "private");
        assert_eq!(r["result"]["ttlMs"], 0);
        // Not found: -32002 on legacy, -32602 on modern.
        let r = call(
            &mut s,
            legacy(4, "resources/read", json!({"uri": "kitkraft://nope"})),
        );
        assert_eq!(r["error"]["code"], -32002);
        let r = call(
            &mut s,
            modern(5, "resources/read", json!({"uri": "kitkraft://nope"})),
        );
        assert_eq!(r["error"]["code"], -32602);
        let r = call(&mut s, legacy(6, "resources/read", json!({})));
        assert_eq!(r["error"]["code"], -32602);
        let r = call(&mut s, legacy(7, "resources/templates/list", json!({})));
        assert_eq!(r["result"]["resourceTemplates"], json!([]));
    }

    /// Malformed input of every kind is answered with an error, never a panic.
    #[test]
    fn malformed_input_table() {
        let mut s = server();
        let cases: &[(&str, i64)] = &[
            ("{", -32700),
            (r#"{"jsonrpc":"2.0","id":1,"method":"ping""#, -32700),
            ("not json", -32700),
            ("[]", -32600),
            (r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#, -32600),
            ("42", -32600),
            (r#""text""#, -32600),
            ("null", -32600),
            (r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#, -32600),
            (r#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#, -32600),
            (r#"{"jsonrpc":"2.0","id":true,"method":"ping"}"#, -32600),
            (r#"{"jsonrpc":"2.0","id":{},"method":"ping"}"#, -32600),
            (r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#, -32600),
            (r#"{"id":1,"method":"ping"}"#, -32600),
            (r#"{"jsonrpc":"2.0","id":1}"#, -32600),
            (r#"{"jsonrpc":"2.0","id":1,"method":7}"#, -32600),
            (r#"{"jsonrpc":"2.0","id":1,"method":"nope"}"#, -32601),
            (
                r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":[1]}"#,
                -32602,
            ),
            (
                r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":"x"}"#,
                -32602,
            ),
            (
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":5}}"#,
                -32602,
            ),
            (
                r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":[]}}"#,
                -32602,
            ),
            (
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}"#,
                -32602,
            ),
        ];
        for (input, code) in cases {
            let line = s
                .handle_line(input)
                .unwrap_or_else(|| panic!("no reply to {input}"));
            let reply: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(reply["error"]["code"], *code, "{input} -> {line}");
            assert_eq!(reply["jsonrpc"], "2.0");
        }
        // String and large integer ids are echoed back.
        let r = call(
            &mut s,
            json!({"jsonrpc": "2.0", "id": "abc", "method": "ping"}),
        );
        assert_eq!(r["id"], "abc");
        let r = call(
            &mut s,
            json!({"jsonrpc": "2.0", "id": u64::MAX, "method": "ping"}),
        );
        assert_eq!(r["id"], u64::MAX);
    }

    #[test]
    fn serve_reads_lines_until_eof() {
        let mut s = server();
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\r\n",
            "\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "garbage\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}"
        );
        let mut out = Vec::new();
        s.serve(input.as_bytes(), &mut out).unwrap();
        let lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["id"], 1);
        assert_eq!(lines[1]["error"]["code"], -32700);
        assert_eq!(lines[2]["id"], 2);
    }

    #[test]
    fn oversize_and_non_utf8_lines_are_refused() {
        let mut big = vec![b' '; 64];
        big.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n");
        let mut reader = &big[..];
        assert_eq!(read_frame(&mut reader, 16).unwrap(), Frame::Oversize);
        assert!(matches!(
            read_frame(&mut reader, 1024).unwrap(),
            Frame::Line(_)
        ));
        assert_eq!(read_frame(&mut reader, 1024).unwrap(), Frame::Eof);
        // Unterminated oversize tail.
        let mut reader = &[b'x'; 40][..];
        assert_eq!(read_frame(&mut reader, 16).unwrap(), Frame::Oversize);

        let mut s = server();
        let mut out = Vec::new();
        s.serve(&b"\xff\xfe\n"[..], &mut out).unwrap();
        let reply: Value = serde_json::from_slice(out.strip_suffix(b"\n").unwrap()).unwrap();
        assert_eq!(reply["error"]["code"], -32700);
    }
}
