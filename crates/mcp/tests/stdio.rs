//! In-process stdio client: the server runs on a thread over OS pipes, the test speaks
//! newline-delimited JSON-RPC to it the way Claude Code would (M2.F6).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::thread::JoinHandle;

use kitcraft_engine::{Engine, EngineError, Outcome, Session};
use kitcraft_mcp::Server;
use serde_json::{json, Value};

/// A minimal valid plugin: `TES4` (HEDR), then `GRUP GMST` holding one `GMST` record.
fn synthetic_plugin() -> Vec<u8> {
    fn sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&u16::try_from(data.len()).unwrap().to_le_bytes());
        out.extend_from_slice(data);
        out
    }
    fn record(sig: &[u8; 4], flags: u32, form_id: u32, body: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&form_id.to_le_bytes());
        out.extend_from_slice(&[0; 8]);
        out.extend_from_slice(body);
        out
    }
    let mut hedr = 1.0f32.to_le_bytes().to_vec();
    hedr.extend_from_slice(&2u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    let mut out = record(b"TES4", 1, 0, &sub(b"HEDR", &hedr));
    let mut gmst = sub(b"EDID", b"fTestValue\0");
    gmst.extend(sub(b"DATA", &2.5f32.to_le_bytes()));
    let rec = record(b"GMST", 0, 0x0000_0100, &gmst);
    out.extend_from_slice(b"GRUP");
    out.extend_from_slice(&u32::try_from(rec.len() + 24).unwrap().to_le_bytes());
    out.extend_from_slice(b"GMST");
    out.extend_from_slice(&[0; 12]);
    out.extend(rec);
    out
}

fn temp_plugin(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("kitcraft-mcp-{name}-{}.esm", std::process::id()));
    std::fs::write(&path, synthetic_plugin()).unwrap();
    path
}

/// A client connected to a server thread through two OS pipes.
struct Client {
    to_server: Option<std::io::PipeWriter>,
    from_server: BufReader<std::io::PipeReader>,
    thread: Option<JoinHandle<std::io::Result<()>>>,
    next_id: i64,
}

impl Client {
    fn start(server: Server) -> Self {
        let (server_in, to_server) = std::io::pipe().unwrap();
        let (from_server, server_out) = std::io::pipe().unwrap();
        let thread = std::thread::spawn(move || {
            let mut server = server;
            server.serve(BufReader::new(server_in), server_out)
        });
        Self {
            to_server: Some(to_server),
            from_server: BufReader::new(from_server),
            thread: Some(thread),
            next_id: 0,
        }
    }

    fn send(&mut self, message: &Value) {
        let pipe = self.to_server.as_mut().unwrap();
        writeln!(pipe, "{message}").unwrap();
        pipe.flush().unwrap();
    }

    fn recv(&mut self) -> Value {
        let mut line = String::new();
        self.from_server.read_line(&mut line).unwrap();
        assert!(line.ends_with('\n'), "server closed: {line:?}");
        serde_json::from_str(&line).unwrap()
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let reply = self.recv();
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }

    fn notify(&mut self, method: &str) {
        self.send(&json!({"jsonrpc": "2.0", "method": method}));
    }

    /// Closes stdin and waits for the server to exit cleanly.
    fn finish(mut self) {
        drop(self.to_server.take());
        let mut rest = String::new();
        self.from_server.read_line(&mut rest).unwrap();
        assert_eq!(rest, "", "no output after EOF");
        self.thread.take().unwrap().join().unwrap().unwrap();
    }
}

fn modern_meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": kitcraft_mcp::PROTOCOL_VERSION,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name": "stdio-test", "version": "0"}
    })
}

/// The envelope a successful tool call carries, checked in both representations.
fn envelope(reply: &Value) -> Value {
    let result = &reply["result"];
    assert_eq!(result["isError"], false, "{reply}");
    let text = result["content"][0]["text"].as_str().unwrap();
    let from_text: Value = serde_json::from_str(text).unwrap();
    assert_eq!(from_text, result["structuredContent"]);
    for member in ["command", "data", "coverage", "limitations", "evidence"] {
        assert!(from_text.get(member).is_some(), "envelope lacks {member}");
    }
    from_text
}

fn without_timing(mut v: Value) -> Value {
    v["data"]["elapsed_ms"] = Value::Null;
    v["data"]["summary"] = Value::Null;
    v
}

#[test]
fn legacy_handshake_then_tools() {
    let path = temp_plugin("legacy");
    let plugin = path.to_str().unwrap();
    let mut client = Client::start(Server::new(Engine::with_default_handlers()));

    let init = client.request(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {},
               "clientInfo": {"name": "stdio-test", "version": "0"}}),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    client.notify("notifications/initialized");
    assert_eq!(client.request("ping", json!({}))["result"], json!({}));

    let list = client.request("tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for name in [
        "execute",
        "command_line",
        "inspect_record",
        "query_records",
        "diff_plugins",
        "coverage",
        "roundtrip",
    ] {
        assert!(names.contains(&name), "{name} missing from {names:?}");
    }

    // roundtrip through its shortcut equals the engine's own result.
    let reply = client.request(
        "tools/call",
        json!({"name": "roundtrip", "arguments": {"plugin": plugin, "verify": true}}),
    );
    let got = envelope(&reply);
    assert_eq!(got["data"]["identical"], true);
    assert_eq!(got["data"]["passed"], true);
    let expected =
        kitcraft_engine::execute("roundtrip", json!({"plugin": plugin, "verify": true})).unwrap();
    assert_eq!(without_timing(got), without_timing(expected));

    // coverage through `execute`.
    let reply = client.request(
        "tools/call",
        json!({"name": "execute", "arguments": {"command": "coverage", "args": {"plugin": plugin}}}),
    );
    let got = envelope(&reply);
    assert_eq!(got["command"], "coverage");
    assert_eq!(got["data"]["records"], 2);

    // inspect through `command_line`.
    let reply = client.request(
        "tools/call",
        json!({"name": "command_line", "arguments": {"text": format!("inspect fTestValue --plugin \"{plugin}\"")}}),
    );
    let got = envelope(&reply);
    assert_eq!(got["data"]["record"]["form_id"], "00000100");

    // A failing call is a tool error the model can read.
    let reply = client.request(
        "tools/call",
        json!({"name": "inspect_record", "arguments": {"form": "NoSuchThing", "plugin": plugin}}),
    );
    assert_eq!(reply["result"]["isError"], true);
    assert_eq!(
        reply["result"]["structuredContent"]["error"]["kind"],
        "failed"
    );

    client.finish();
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn modern_requests_without_handshake() {
    let path = temp_plugin("modern");
    let plugin = path.to_str().unwrap();
    let mut client = Client::start(Server::new(Engine::with_default_handlers()));

    let discover = client.request("server/discover", json!({"_meta": modern_meta()}));
    assert_eq!(
        discover["result"]["supportedVersions"][0],
        kitcraft_mcp::PROTOCOL_VERSION
    );
    assert_eq!(discover["result"]["resultType"], "complete");

    let list = client.request("tools/list", json!({"_meta": modern_meta()}));
    assert_eq!(list["result"]["resultType"], "complete");
    assert!(!list["result"]["tools"].as_array().unwrap().is_empty());

    let reply = client.request(
        "tools/call",
        json!({"_meta": modern_meta(), "name": "coverage", "arguments": {"plugin": plugin, "decode": false}}),
    );
    assert_eq!(reply["result"]["resultType"], "complete");
    let got = envelope(&reply);
    assert_eq!(got["data"]["decode"], false);

    // A legacy-only client's initialize still works on the same process.
    let init = client.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    client.finish();
    std::fs::remove_file(&path).unwrap();
}

/// A fake `load` that stores the plugin list in the session, and an `inspect` that reads
/// it back: the server keeps one session across calls (M2.F4: `load` then `inspect`).
#[test]
fn one_session_across_tool_calls() {
    fn load(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
        let listing = json!({"loaded": true, "plugins": args["plugins"].clone()});
        session.set_load_order(Some(listing.clone()));
        Ok(Outcome {
            data: listing,
            ..Outcome::not_implemented()
        })
    }
    fn inspect(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
        let plugins = session
            .load_order()
            .map(|l| l["plugins"].clone())
            .ok_or_else(|| EngineError::Failed {
                command: "inspect".into(),
                message: "nothing loaded".into(),
            })?;
        Ok(Outcome {
            data: json!({"form": args["form"], "searched": plugins}),
            ..Outcome::not_implemented()
        })
    }
    let mut engine = Engine::empty();
    engine.register_session("load", Box::new(load)).unwrap();
    engine
        .register_session("inspect", Box::new(inspect))
        .unwrap();
    let mut client = Client::start(Server::new(engine));

    let before = client.request(
        "tools/call",
        json!({"name": "inspect_record", "arguments": {"form": "0001A2B3", "plugin": "A.esm"}}),
    );
    assert_eq!(before["result"]["isError"], true);

    let plugins = json!(["Starfield.esm", "Patch.esp"]);
    client.request(
        "tools/call",
        json!({"name": "execute", "arguments": {"command": "load", "args": {"plugins": plugins}}}),
    );
    let after = client.request(
        "tools/call",
        json!({"name": "inspect_record", "arguments": {"form": "0001A2B3", "plugin": "A.esm"}}),
    );
    assert_eq!(envelope(&after)["data"]["searched"], plugins);

    let read = client.request("resources/read", json!({"uri": "kitcraft://load-order"}));
    let text = read["result"]["contents"][0]["text"].as_str().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(text).unwrap()["plugins"],
        plugins
    );
    client.finish();
}

/// Every engine error kind maps per `docs/mcp.md`: caller errors are `isError` results,
/// engine bugs are `-32603`.
#[test]
fn error_kinds_map_per_the_docs() {
    fn failing(err: fn() -> EngineError) -> impl Fn(&Value) -> Result<Outcome, EngineError> {
        move |_: &Value| Err(err())
    }
    let mut engine = Engine::empty();
    engine
        .register(
            "validate",
            Box::new(failing(|| EngineError::Deferred {
                command: "validate".into(),
                reason: "later".into(),
                milestone: "M3".into(),
            })),
        )
        .unwrap();
    engine
        .register(
            "build",
            Box::new(failing(|| EngineError::BadSchema {
                command: "build".into(),
                message: "bug".into(),
            })),
        )
        .unwrap();
    engine
        .register(
            "diff",
            Box::new(failing(|| EngineError::Registration("diff".into(), "bug"))),
        )
        .unwrap();
    let mut client = Client::start(Server::new(engine));
    let call = |client: &mut Client, command: &str, args: Value| {
        client.request(
            "tools/call",
            json!({"name": "execute", "arguments": {"command": command, "args": args}}),
        )
    };
    let r = call(&mut client, "validate", json!({"plugin": "X.esm"}));
    assert_eq!(
        r["result"]["structuredContent"]["error"]["kind"],
        "deferred"
    );
    assert_eq!(r["result"]["isError"], true);
    let r = call(
        &mut client,
        "build",
        json!({"spec": "s.toml", "out": "o.esp"}),
    );
    assert_eq!(r["error"]["code"], -32603, "{r}");
    let r = call(&mut client, "diff", json!({"a": "A.esm", "b": "B.esm"}));
    assert_eq!(r["error"]["code"], -32603, "{r}");
    let r = call(
        &mut client,
        "set",
        json!({"form": "x", "fields": {}, "into": "P.esp"}),
    );
    assert_eq!(
        r["result"]["structuredContent"]["error"]["kind"],
        "not_implemented"
    );
    client.finish();
}

/// Over MCP, `coverage` is a summary unless the caller asks for the signature lists, on
/// every route; the text block leads with `data.summary` and still equals the envelope.
#[test]
fn coverage_is_a_summary_by_default_over_mcp() {
    let path = temp_plugin("coverage-summary");
    let plugin = path.to_str().unwrap();
    let mut client = Client::start(Server::new(Engine::with_default_handlers()));
    let lists_signatures = |envelope: &Value| {
        envelope["data"]["record_types"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["signatures"].is_array())
    };

    let calls = [
        json!({"name": "coverage", "arguments": {"plugin": plugin}}),
        json!({"name": "execute", "arguments": {"command": "coverage", "args": {"plugin": plugin}}}),
        json!({"name": "command_line", "arguments": {"text": format!("coverage \"{plugin}\"")}}),
    ];
    for call in &calls {
        let reply = client.request("tools/call", call.clone());
        let got = envelope(&reply);
        assert_eq!(got["data"]["signatures"], false, "{call}");
        assert_eq!(got["data"]["top"], 20, "{call}");
        assert!(got["data"]["record_types"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["signatures"].is_null()));
        assert_eq!(got["data"]["records"], 2, "totals are unchanged");
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.starts_with(r#"{"command":"coverage","data":{"summary":"#),
            "{text}"
        );
    }

    // Asked for explicitly, the lists come back; `top` limits the types.
    let reply = client.request(
        "tools/call",
        json!({"name": "coverage", "arguments": {"plugin": plugin, "signatures": true, "top": 1}}),
    );
    let got = envelope(&reply);
    assert!(lists_signatures(&got));
    assert_eq!(got["data"]["record_types"].as_array().unwrap().len(), 1);
    assert_eq!(got["data"]["record_types_seen"], 2);
    let reply = client.request(
        "tools/call",
        json!({"name": "command_line", "arguments": {"text": format!("coverage \"{plugin}\" --signatures")}}),
    );
    assert!(lists_signatures(&envelope(&reply)));

    // The CLI-side default (the engine with no MCP layer) keeps the lists.
    let direct = kitcraft_engine::execute("coverage", json!({"plugin": plugin})).unwrap();
    assert!(lists_signatures(&direct));
    client.finish();
    std::fs::remove_file(&path).unwrap();
}
