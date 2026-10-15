//! Spawns the real `kit mcp` and speaks MCP to it over its stdin/stdout (M2.F6).
//!
//! The synthetic-plugin test runs everywhere. With `KITKRAFT_DATA_DIR` set (see
//! `docs/corpus.md`) a variant also calls `inspect_record` and `query_records` on an
//! official file; without it that variant prints a skip line and passes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread::JoinHandle;

use serde_json::{json, Value};

struct Kit {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    /// Drains the child's stderr as it is written, so a chatty child never blocks on a
    /// full pipe, and the text is there to report when the child dies.
    stderr: Option<JoinHandle<String>>,
    next_id: i64,
}

/// A directory of this test binary's own under the target dir, for the index cache: the
/// spawned servers never share `%LOCALAPPDATA%\kitkraft\cache` with other processes
/// (other worktrees' gates, the owner's `kit`), and it stays warm between runs.
fn private_cache_dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("kit-cli-mcp-index-cache")
}

impl Kit {
    fn spawn(extra: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_kit"))
            .arg("mcp")
            .args(extra)
            .env("KITKRAFT_CACHE_DIR", private_cache_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn kit mcp");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut err = child.stderr.take().unwrap();
        let stderr = std::thread::spawn(move || {
            let mut text = Vec::new();
            let _ = err.read_to_end(&mut text);
            String::from_utf8_lossy(&text).into_owned()
        });
        Self {
            child,
            stdin,
            stdout,
            stderr: Some(stderr),
            next_id: 0,
        }
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        if line.is_empty() {
            self.died(method);
        }
        let reply: Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("not a JSON-RPC line ({e}): {line:?}"));
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }

    /// The server closed stdout mid-session: report how it ended and what it said.
    fn died(&mut self, method: &str) -> ! {
        drop(self.stdin.take());
        let status = self.child.wait();
        let stderr = self
            .stderr
            .take()
            .map(|t| t.join().unwrap_or_default())
            .unwrap_or_default();
        panic!("kit mcp closed stdout during `{method}` ({status:?}); stderr:\n{stderr}");
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": tool, "arguments": arguments}))
    }

    fn initialize(&mut self) -> Value {
        let init = self.request(
            "initialize",
            json!({"protocolVersion": "2025-11-25", "capabilities": {},
                   "clientInfo": {"name": "kit-cli-test", "version": "0"}}),
        );
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        init
    }

    /// Closes stdin; the server must exit 0 with nothing more on stdout. Returns stderr.
    fn finish(mut self) -> String {
        drop(self.stdin.take());
        let mut rest = String::new();
        self.stdout.read_to_string(&mut rest).unwrap();
        assert_eq!(rest, "", "stdout after EOF");
        let status = self.child.wait().unwrap();
        let stderr = self.stderr.take().unwrap().join().unwrap();
        assert_eq!(status.code(), Some(0), "{stderr}");
        stderr
    }
}

/// The envelope of a successful tool call (structured and text forms agree).
fn envelope(reply: &Value) -> Value {
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed, reply["result"]["structuredContent"]);
    parsed
}

/// `TES4` (HEDR) then `GRUP GMST` with one `GMST` (EDID `fTestValue`).
fn synthetic_plugin() -> Vec<u8> {
    fn sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&u16::try_from(data.len()).unwrap().to_le_bytes());
        out.extend_from_slice(data);
        out
    }
    fn record(sig: &[u8; 4], form_id: u32, body: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&form_id.to_le_bytes());
        out.extend_from_slice(&[0; 8]);
        out.extend_from_slice(body);
        out
    }
    let mut hedr = 1.0f32.to_le_bytes().to_vec();
    hedr.extend_from_slice(&2u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    let mut out = record(b"TES4", 0, &sub(b"HEDR", &hedr));
    let mut gmst = sub(b"EDID", b"fTestValue\0");
    gmst.extend(sub(b"DATA", &2.5f32.to_le_bytes()));
    let rec = record(b"GMST", 0x0000_0100, &gmst);
    out.extend_from_slice(b"GRUP");
    out.extend_from_slice(&u32::try_from(rec.len() + 24).unwrap().to_le_bytes());
    out.extend_from_slice(b"GMST");
    out.extend_from_slice(&[0; 12]);
    out.extend(rec);
    out
}

fn temp_plugin(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("kit-cli-mcp-{name}-{}.esm", std::process::id()));
    std::fs::write(&path, synthetic_plugin()).unwrap();
    path
}

#[test]
fn kit_mcp_serves_tools_on_a_synthetic_plugin() {
    let path = temp_plugin("synthetic");
    let plugin = path.to_str().unwrap();
    let mut kit = Kit::spawn(&[]);

    let init = kit.initialize();
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(init["result"]["serverInfo"]["name"], "kitkraft");

    let list = kit.request("tools/list", json!({}));
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"roundtrip") && names.contains(&"coverage"),
        "{names:?}"
    );

    let got = envelope(&kit.call("roundtrip", json!({"plugin": plugin, "verify": true})));
    assert_eq!(got["command"], "roundtrip");
    assert_eq!(got["data"]["passed"], true);

    let got = envelope(&kit.call("coverage", json!({"plugin": plugin})));
    assert_eq!(got["command"], "coverage");
    assert_eq!(got["data"]["records"], 2);

    let got = envelope(&kit.call(
        "inspect_record",
        json!({"form": "fTestValue", "plugin": plugin}),
    ));
    assert_eq!(got["data"]["record"]["form_id"], "00000100");

    // M2.F9: the schema of the record type just inspected, with sources and confidence.
    let got = envelope(&kit.call("record_schema", json!({"type": "GMST"})));
    assert_eq!(got["command"], "schema");
    let value = got["data"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["signature"] == "DATA")
        .unwrap()
        .clone();
    assert_eq!(value["name"], "Value");
    assert!(value["confidence"].is_string() && value["source"].is_string());

    let stderr = kit.finish();
    assert_eq!(stderr, "", "no --profile, nothing to report");
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn kit_mcp_profile_preload_failure_is_reported_not_fatal() {
    let missing = std::env::temp_dir().join("kit-cli-mcp-no-such-profile");
    let mut kit = Kit::spawn(&["--profile", missing.to_str().unwrap()]);
    let init = kit.initialize();
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert_eq!(kit.request("ping", json!({}))["result"], json!({}));
    let stderr = kit.finish();
    assert!(stderr.contains("preloading --profile failed"), "{stderr}");
    // The model is told too, so it knows the session is empty.
    assert!(
        instructions.contains("Preloading the profile"),
        "{instructions} / {stderr}"
    );
}

/// With the official files: `coverage` names a real record, `inspect_record` finds it,
/// and `query_records` answers once a load order is given.
#[test]
fn kit_mcp_on_an_official_file() {
    let Some(dir) =
        std::env::var_os("KITKRAFT_DATA_DIR").or_else(|| std::env::var_os("KITCRAFT_DATA_DIR"))
    else {
        println!(
            "skip kit_mcp_on_an_official_file: KITKRAFT_DATA_DIR is not set (see docs/corpus.md)"
        );
        return;
    };
    let path = PathBuf::from(dir).join("Constellation.esm");
    if !path.is_file() {
        println!(
            "skip kit_mcp_on_an_official_file: {} not found",
            path.display()
        );
        return;
    }
    let plugin = path.to_str().unwrap();
    let mut kit = Kit::spawn(&[]);
    kit.initialize();

    let coverage = envelope(&kit.call(
        "coverage",
        json!({"plugin": plugin, "decode": false, "signatures": true}),
    ));
    let first = coverage["data"]["record_types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["signature"] != "TES4")
        .and_then(|t| t["signatures"][0]["first_record"].as_str())
        .expect("a record type with a first record")
        .to_string();
    let (signature, form) = first.split_once(':').unwrap();

    let got = envelope(&kit.call("inspect_record", json!({"form": form, "plugin": plugin})));
    assert_eq!(got["data"]["record"]["signature"], signature);
    assert!(!got["evidence"].as_array().unwrap().is_empty());

    // `query` needs a load order: without one it is a `failed` tool error that says so;
    // with the plugin loaded it answers.
    let reply = kit.call("query_records", json!({"type": signature, "limit": 5}));
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    assert_eq!(
        reply["result"]["structuredContent"]["error"]["kind"], "failed",
        "{reply}"
    );
    // Constellation.esm's master must be loaded for its FormIDs to resolve (the index cache
    // makes this cheap after the first run).
    let data = path.parent().unwrap().to_str().unwrap();
    let reply = kit.call(
        "query_records",
        json!({"type": signature, "limit": 5, "data_dir": data,
               "plugins": ["Starfield.esm", "Constellation.esm"]}),
    );
    let got = envelope(&reply);
    assert_eq!(got["command"], "query");
    assert!(got["data"]["matched"].as_u64().unwrap() >= 1, "{got}");
    kit.finish();
}
