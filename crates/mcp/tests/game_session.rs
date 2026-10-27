//! The four game-session commands over the real stdio protocol: `game.session` with
//! `start: true` returns at once and the session lives on in the server's one engine,
//! so an agent follows it with `game.status`, re-runs a row with `game.probe` and ends
//! it with `game.stop` in later calls, against the engine's fakes
//! (`kitkraft_engine::testing::GameFakes`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::io::{BufRead, BufReader, Write};
use std::thread::JoinHandle;
use std::time::Duration;

use kitkraft_engine::{testing::GameFakes, Engine};
use kitkraft_mcp::Server;
use serde_json::{json, Value};

/// A client connected to a server thread through two OS pipes (the one in `stdio.rs`).
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

/// `game.status` for `id` until `data.phase` is `idle`, every 25 ms, at most 400 polls.
fn poll_idle(client: &mut Client, id: &str) -> Value {
    let mut polls = 0;
    loop {
        let reply = client.request(
            "tools/call",
            json!({"name": "execute", "arguments": {"command": "game.status", "args": {"id": id}}}),
        );
        let got = envelope(&reply);
        polls += 1;
        if got["data"]["phase"] == json!("idle") {
            return got;
        }
        assert!(polls < 400, "no idle phase after {polls} polls: {got}");
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// `game.session` with `start: true` returns at once over MCP; the session lives on in
/// the server's one engine, so `game.status`, `game.probe` and `game.stop` reach it in
/// later calls, and a `game.status` after the stop is a tool error.
#[test]
fn game_session_started_over_mcp_is_followed_and_stopped() {
    let fakes = GameFakes::new("mcp-followed").unwrap();
    let engine = Engine::with_default_handlers();
    fakes.install(&engine);
    let mut client = Client::start(Server::new(engine));

    let init = client.request(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {},
               "clientInfo": {"name": "stdio-test", "version": "0"}}),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    client.notify("notifications/initialized");

    let reply = client.request(
        "tools/call",
        json!({"name": "execute", "arguments": {"command": "game.session",
              "args": fakes.session_args(json!({"start": true}))}}),
    );
    let started = envelope(&reply);
    assert_eq!(started["data"]["started"], json!(true), "{started}");
    let id = started["data"]["session"].as_str().unwrap().to_string();

    // Follow it until the probes are done: three rows, T-01 and T-02 pass, T-03
    // inconclusive (the manual row).
    let idle = poll_idle(&mut client, &id);
    let rows = idle["data"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "{rows:?}");
    let verdict = |row: &str| {
        rows.iter()
            .find(|x| x["row"] == json!(row))
            .unwrap()["verdict"]["status"]
            .clone()
    };
    assert_eq!(verdict("T-01"), json!("pass"), "{idle}");
    assert_eq!(verdict("T-02"), json!("pass"), "{idle}");
    assert_eq!(verdict("T-03"), json!("inconclusive"), "{idle}");

    // Re-run one row through `game.probe`.
    let reply = client.request(
        "tools/call",
        json!({"name": "execute", "arguments": {"command": "game.probe",
              "args": {"id": id, "row": "T-01"}}}),
    );
    let probe = envelope(&reply);
    assert_eq!(probe["data"]["row"]["verdict"]["status"], json!("pass"), "{probe}");

    // End it; the report names the ledger entry.
    let reply = client.request(
        "tools/call",
        json!({"name": "execute", "arguments": {"command": "game.stop",
              "args": {"id": id}}}),
    );
    let stopped = envelope(&reply);
    assert!(
        stopped["data"]["report"]["ledger_entry"].as_str().is_some(),
        "{stopped}"
    );

    // A further status is a tool error naming the missing session.
    let reply = client.request(
        "tools/call",
        json!({"name": "execute", "arguments": {"command": "game.status",
              "args": {"id": id}}}),
    );
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    let err = &reply["result"]["structuredContent"]["error"];
    assert_eq!(err["kind"], "failed", "{reply}");
    assert!(err["message"].as_str().unwrap().contains("no session"), "{reply}");

    client.finish();
}

/// A session left running when the server's stdin closes is not abandoned: the engine
/// stops it on drop (`impl Drop for Running`), the game ends, and the fixture's ledger
/// gains its entry.
#[test]
fn a_session_left_running_is_stopped_when_the_server_exits() {
    let fakes = GameFakes::new("mcp-exit").unwrap();
    let engine = Engine::with_default_handlers();
    fakes.install(&engine);
    let mut client = Client::start(Server::new(engine));

    let reply = client.request(
        "tools/call",
        json!({"name": "execute", "arguments": {"command": "game.session",
              "args": fakes.session_args(json!({"start": true}))}}),
    );
    let started = envelope(&reply);
    let id = started["data"]["session"].as_str().unwrap().to_string();
    poll_idle(&mut client, &id);

    // Close stdin without `game.stop`; the server thread's engine drop stops the
    // session still running.
    client.finish();

    let ledger = std::fs::read_to_string(fakes.ledger()).unwrap();
    assert!(ledger.contains("### game-"), "no ledger entry in {ledger}");
    assert!(ledger.contains(&id), "no entry for {id} in {ledger}");
}

/// The four game-session commands have no shortcut tool: they are reached through
/// `execute`, whose description lists them from the catalog.
#[test]
fn game_commands_are_reached_through_execute() {
    let engine = Engine::with_default_handlers();
    let mut client = Client::start(Server::new(engine));

    let list = client.request("tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        assert!(!name.starts_with("game"), "{name} is a shortcut tool");
    }

    let execute = tools
        .iter()
        .find(|t| t["name"] == "execute")
        .expect("execute tool");
    let desc = execute["description"].as_str().unwrap();
    for id in ["game.session", "game.status", "game.probe", "game.stop"] {
        assert!(desc.contains(&format!("- {id} {{")), "{id} not in {desc}");
    }

    client.finish();
}
