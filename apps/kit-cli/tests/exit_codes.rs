//! Runs the built `kit` binary and checks exit codes and output channels.

// Test helpers may fail loudly; clippy's in-test allowance does not reach helper fns.
#![allow(clippy::expect_used)]

use std::process::{Command, Output};

use serde_json::{json, Value};

fn kit(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kit"))
        .args(args)
        .output()
        .expect("run kit")
}

fn code(args: &[&str]) -> i32 {
    kit(args).status.code().expect("exit code")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn help_and_version_exit_0() {
    for args in [&[][..], &["help"], &["--help"], &["-h"]] {
        let out = kit(args);
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        let text = stdout(&out);
        assert!(text.contains("COMMANDS:"), "{args:?}: {text}");
        assert!(text.contains("roundtrip"), "{args:?}");
        assert!(text.contains("ba2 extract"), "{args:?}");
        assert!(text.contains("mcp"), "{args:?}");
    }
    let out = kit(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).starts_with("kit "));
}

#[test]
fn per_command_help_exits_0() {
    for args in [
        &["help", "roundtrip"][..],
        &["roundtrip", "--help"],
        &["help", "ba2", "list"],
    ] {
        let out = kit(args);
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        assert!(stdout(&out).starts_with("usage: kit "), "{args:?}");
    }
}

#[test]
fn planned_command_exits_2_with_clear_message() {
    let out = kit(&["roundtrip", "X.esm"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not implemented"), "{}", stderr(&out));

    let out = kit(&["ba2", "list", "A.ba2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not implemented"), "{}", stderr(&out));
}

#[test]
fn json_output_equals_the_execute_result() {
    let expected =
        kitcraft_engine::execute("roundtrip", json!({"plugin": "X.esm", "verify": true}))
            .expect("execute");
    for args in [
        &["roundtrip", "X.esm", "--verify", "--json"][..],
        &["--json", "roundtrip", "X.esm", "--verify"],
    ] {
        let out = kit(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
        assert_eq!(got, expected, "{args:?}");
    }
}

#[test]
fn errors_exit_2() {
    assert_eq!(code(&["frobnicate"]), 2);
    assert_eq!(code(&["help", "frobnicate"]), 2);
    assert_eq!(code(&["ba2"]), 2);
    assert_eq!(code(&["roundtrip", "X.esm", "--bogus"]), 2);
    assert_eq!(code(&["roundtrip", "X.esm", "Y.esm"]), 2);

    let out = kit(&["roundtrip"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("/plugin"), "{}", stderr(&out));

    let out = kit(&["roundtrip", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
    assert_eq!(got["error"]["kind"], "invalid_args");
    assert_eq!(got["error"]["violations"][0]["pointer"], "/plugin");
    assert_eq!(got["error"]["violations"][0]["rule"], "required");
}

#[test]
fn mcp_exits_2_until_m2() {
    let out = kit(&["mcp"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not implemented until M2"));
}
