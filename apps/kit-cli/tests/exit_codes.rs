//! Runs the built `kit` binary and checks exit codes and output channels.

// Test helpers may fail loudly; clippy's in-test allowance does not reach helper fns.
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;
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
fn failing_command_exits_2_with_clear_message() {
    // No catalog command is Planned any more (M2.F8): a command that fails (a missing
    // spec, a missing plugin) is the exit-2 case.
    for (args, expected) in [
        (
            &["build", "no-such-spec.toml", "out.esm"][..],
            "no-such-spec.toml",
        ),
        (
            &["validate", "--plugin", "no-such-plugin.esm"],
            "no-such-plugin.esm",
        ),
    ] {
        let out = kit(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(stderr(&out).contains(expected), "{}", stderr(&out));
    }
}

/// A minimal valid plugin: `TES4` (HEDR), then `GRUP GMST` holding one `GMST` record.
fn synthetic_plugin() -> Vec<u8> {
    fn sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&u16::try_from(data.len()).expect("small").to_le_bytes());
        out.extend_from_slice(data);
        out
    }
    fn record(sig: &[u8; 4], flags: u32, form_id: u32, body: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&u32::try_from(body.len()).expect("small").to_le_bytes());
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
    out.extend_from_slice(&u32::try_from(rec.len() + 24).expect("small").to_le_bytes());
    out.extend_from_slice(b"GMST");
    out.extend_from_slice(&[0; 12]);
    out.extend(rec);
    out
}

fn temp_plugin(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("kit-cli-{name}-{}.esm", std::process::id()));
    std::fs::write(&path, bytes).expect("write temp plugin");
    path
}

/// `elapsed_ms` (and the summary that quotes it) vary run to run.
fn without_timing(mut result: Value) -> Value {
    result["data"]["elapsed_ms"] = Value::Null;
    result["data"]["summary"] = Value::Null;
    result
}

#[test]
fn roundtrip_exit_codes() {
    let bytes = synthetic_plugin();
    let path = temp_plugin("roundtrip", &bytes);
    let p = path.to_str().expect("utf-8 temp path");

    // Identical: exit 0, with and without --verify; text output is the summary line.
    for args in [&["roundtrip", p, "--verify"][..], &["roundtrip", p]] {
        let out = kit(args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        let text = stdout(&out);
        assert!(text.contains(": identical ("), "{text}");
        assert!(text.contains("limitation: container level"), "{text}");
    }

    // --out writes the rebuilt bytes.
    let rebuilt = path.with_extension("out.esm");
    let r = rebuilt.to_str().expect("utf-8 temp path");
    assert_eq!(code(&["roundtrip", p, "--verify", "--out", r]), 0);
    assert_eq!(std::fs::read(&rebuilt).expect("rebuilt"), bytes);
    std::fs::remove_file(&rebuilt).expect("cleanup");

    // Errors: missing file, truncated plugin, not a plugin -> exit 2.
    let missing = std::env::temp_dir().join("kit-cli-no-such-plugin.esm");
    assert_eq!(code(&["roundtrip", missing.to_str().expect("utf-8")]), 2);
    let truncated = temp_plugin("truncated", &bytes[..bytes.len() - 1]);
    let out = kit(&["roundtrip", truncated.to_str().expect("utf-8"), "--verify"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("offset"), "{}", stderr(&out));
    let out = kit(&["roundtrip", truncated.to_str().expect("utf-8"), "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
    assert_eq!(got["error"]["kind"], "failed");
    let not_plugin = temp_plugin("notplugin", &bytes[bytes.len() - 70..]);
    assert_eq!(code(&["roundtrip", not_plugin.to_str().expect("utf-8")]), 2);

    // A --verify mismatch cannot be produced from a parseable file, since the writer
    // re-emits every byte as read; exit 1 comes from --decode (`roundtrip_decode_exit_1`).
    for p in [&path, &truncated, &not_plugin] {
        std::fs::remove_file(p).expect("cleanup");
    }
}

/// A zlib stream holding `data` in one stored (uncompressed) block, with its Adler-32.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let len = u16::try_from(data.len()).expect("small");
    let mut out = vec![0x78, 0x01, 0x01];
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65_521;
        b = (b + a) % 65_521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

/// `TES4` + `GRUP GMST` holding one compressed `GMST`; `corrupt` flips the last byte of
/// its stream (the Adler-32), which keeps every size right.
fn compressed_plugin(corrupt: bool) -> Vec<u8> {
    fn sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&u16::try_from(data.len()).expect("small").to_le_bytes());
        out.extend_from_slice(data);
        out
    }
    fn record(sig: &[u8; 4], flags: u32, form_id: u32, body: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&u32::try_from(body.len()).expect("small").to_le_bytes());
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
    let mut body = u32::try_from(gmst.len())
        .expect("small")
        .to_le_bytes()
        .to_vec();
    body.extend(zlib_stored(&gmst));
    if corrupt {
        *body.last_mut().expect("stream") ^= 0xFF;
    }
    let rec = record(b"GMST", 0x0004_0000, 0x0000_0100, &body);
    out.extend_from_slice(b"GRUP");
    out.extend_from_slice(&u32::try_from(rec.len() + 24).expect("small").to_le_bytes());
    out.extend_from_slice(b"GMST");
    out.extend_from_slice(&[0; 12]);
    out.extend(rec);
    out
}

/// M3.F1: `--decode` inflates compressed bodies strictly, so a corrupt but well-sized
/// stream is a failed check (exit 1, `data.passed: false`); without it the file is
/// identical at container level.
#[test]
fn roundtrip_decode_exit_1() {
    let good = temp_plugin("decode-good", &compressed_plugin(false));
    let g = good.to_str().expect("utf-8 temp path");
    let out = kit(&["roundtrip", g, "--decode"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).contains(": identical ("), "{}", stdout(&out));

    let bad = temp_plugin("decode-corrupt", &compressed_plugin(true));
    let b = bad.to_str().expect("utf-8 temp path");
    assert_eq!(code(&["roundtrip", b, "--verify"]), 0);
    let out = kit(&["roundtrip", b, "--decode", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
    assert_eq!(got["data"]["passed"], false);
    assert_eq!(got["data"]["decode"], true);
    let path = got["data"]["mismatch"]["path"].as_str().expect("path");
    assert!(path.contains("compressed body"), "{path}");
    assert_eq!(got["evidence"][0]["record"], "GMST:00000100");
    for p in [&good, &bad] {
        std::fs::remove_file(p).expect("cleanup");
    }
}

/// M4.F1: a bare name found nowhere is `not_found`, naming each place searched.
#[test]
fn bare_name_not_found_exits_2_naming_the_places() {
    let empty = std::env::temp_dir().join(format!("kit-cli-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty).expect("temp dir");
    let e = empty.to_str().expect("utf-8 temp path");
    let out = kit(&["coverage", "NoSuchPlugin.esm", "--data-dir", e, "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
    assert_eq!(got["error"]["kind"], "not_found");
    let searched = got["error"]["searched"].as_array().expect("searched");
    assert_eq!(searched.len(), 3, "{got}");
    assert!(searched[2].as_str().expect("text").contains(e), "{got}");
    std::fs::remove_dir_all(&empty).expect("cleanup");
}

#[test]
fn json_output_equals_the_execute_result() {
    let path = temp_plugin("json", &synthetic_plugin());
    let p = path.to_str().expect("utf-8 temp path");
    let expected = kitkraft_engine::execute("roundtrip", json!({"plugin": p, "verify": true}))
        .expect("execute");
    for args in [
        &["roundtrip", p, "--verify", "--json"][..],
        &["--json", "roundtrip", p, "--verify"],
    ] {
        let out = kit(args);
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
        assert_eq!(
            without_timing(got),
            without_timing(expected.clone()),
            "{args:?}"
        );
    }

    let expected = kitkraft_engine::execute("inspect", json!({"form": "fTestValue", "plugin": p}))
        .expect("execute");
    let out = kit(&["inspect", "fTestValue", "--plugin", p, "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
    assert_eq!(got, expected);
    assert_eq!(got["data"]["record"]["form_id"], "00000100");
    std::fs::remove_file(&path).expect("cleanup");
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
fn mcp_exits_0_when_stdin_closes() {
    // `output()` gives the child an empty stdin: the server sees EOF at once.
    let out = kit(&["mcp"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out), "", "stdout carries only MCP messages");
}

/// A header-only plugin (`TES4` with `HEDR` and one `MAST`/`DATA` pair per master).
fn header_plugin(flags: u32, masters: &[&str]) -> Vec<u8> {
    fn sub(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = sig.to_vec();
        out.extend_from_slice(&u16::try_from(data.len()).expect("small").to_le_bytes());
        out.extend_from_slice(data);
        out
    }
    let mut hedr = 0.96f32.to_le_bytes().to_vec();
    hedr.extend_from_slice(&0u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());
    let mut body = sub(b"HEDR", &hedr);
    for m in masters {
        body.extend(sub(b"MAST", format!("{m}\0").as_bytes()));
        body.extend(sub(b"DATA", &[0; 8]));
    }
    let mut out = b"TES4".to_vec();
    out.extend_from_slice(&u32::try_from(body.len()).expect("small").to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    out.extend(body);
    out
}

/// `validate` finds a standalone plugin's masters in `KITKRAFT_DATA_DIR` when no
/// `--data-dir` is given, and a bare plugin name resolves there too.
#[test]
fn validate_defaults_data_dir_to_kitkraft_data_dir() {
    let root = std::env::temp_dir().join(format!("kit-cli-data-dir-{}", std::process::id()));
    let (data, out) = (root.join("Data"), root.join("out"));
    std::fs::create_dir_all(&data).expect("data dir");
    std::fs::create_dir_all(&out).expect("out dir");
    std::fs::write(data.join("KitMaster.esm"), header_plugin(1, &[])).expect("master");
    let patch = out.join("KitPatch.esp");
    std::fs::write(&patch, header_plugin(0, &["KitMaster.esm"])).expect("patch");
    let p = patch.to_str().expect("utf-8 temp path");
    let run = |args: &[&str], env: Option<&PathBuf>| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_kit"));
        cmd.args(args).arg("--json");
        match env {
            Some(dir) => cmd.env("KITKRAFT_DATA_DIR", dir),
            None => cmd
                .env_remove("KITKRAFT_DATA_DIR")
                .env_remove("KITCRAFT_DATA_DIR")
                .env("KITKRAFT_DETECT", "0"),
        };
        let out = cmd.output().expect("run kit");
        let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
        (out.status.code(), got)
    };

    let (status, got) = run(&["validate", "--plugin", p], Some(&data));
    assert_eq!(status, Some(0), "{got:#}");
    assert_eq!(got["data"]["errors"], 0, "{got:#}");
    let found = got["data"]["masters"][0]["path"].as_str().expect("found");
    assert!(found.ends_with("KitMaster.esm"), "{found}");

    let (status, got) = run(&["validate", "--plugin", p], None);
    assert_eq!(status, Some(1), "{got:#}");
    assert_eq!(got["data"]["findings"][0]["rule"], "masters.missing");
    let message = got["data"]["findings"][0]["message"]
        .as_str()
        .expect("text");
    assert!(message.contains("KITKRAFT_DATA_DIR"), "{message}");

    // An explicit --data-dir still wins over the environment.
    let elsewhere = root.join("elsewhere");
    let (status, _) = run(
        &[
            "validate",
            "--plugin",
            p,
            "--data-dir",
            elsewhere.to_str().expect("utf-8"),
        ],
        Some(&data),
    );
    assert_eq!(status, Some(1));

    // A bare name resolves in the Data dir.
    let (status, got) = run(&["validate", "--plugin", "KitMaster.esm"], Some(&data));
    assert_eq!(status, Some(0), "{got:#}");

    // The pre-rename `KITCRAFT_DATA_DIR` is still read when the new name is unset
    // (deprecated, kept for one release).
    let out = Command::new(env!("CARGO_BIN_EXE_kit"))
        .args(["validate", "--plugin", p, "--json"])
        .env_remove("KITKRAFT_DATA_DIR")
        .env("KITCRAFT_DATA_DIR", &data)
        .output()
        .expect("run kit");
    let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
    assert_eq!(out.status.code(), Some(0), "{got:#}");
    assert_eq!(got["data"]["errors"], 0, "{got:#}");
    std::fs::remove_dir_all(&root).expect("cleanup");
}

/// A profile whose MO2 instance names no gamePath loads from `KITKRAFT_DATA_DIR`, and
/// says so; without the variable (and with detection off, `KITKRAFT_DETECT=0`, so the
/// machine's own game install is not the fallback) it fails naming the missing gamePath.
#[test]
fn load_profile_without_game_path_falls_back_to_kitkraft_data_dir() {
    let root = std::env::temp_dir().join(format!("kit-cli-mo2-{}", std::process::id()));
    let (data, profile) = (root.join("Data"), root.join("profiles").join("P"));
    std::fs::create_dir_all(&data).expect("data dir");
    std::fs::create_dir_all(&profile).expect("profile dir");
    std::fs::write(root.join("ModOrganizer.ini"), b"[General]\n").expect("ini");
    std::fs::write(profile.join("modlist.txt"), b"").expect("modlist");
    std::fs::write(profile.join("plugins.txt"), b"*KitMaster.esm\r\n").expect("plugins");
    std::fs::write(profile.join("loadorder.txt"), b"KitMaster.esm\r\n").expect("order");
    std::fs::write(data.join("KitMaster.esm"), header_plugin(1, &[])).expect("master");
    let p = profile.to_str().expect("utf-8 temp path");
    let run = |env: Option<&PathBuf>| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_kit"));
        cmd.args(["load", "--profile", p, "--no-cache", "--json"]);
        match env {
            Some(dir) => cmd.env("KITKRAFT_DATA_DIR", dir),
            None => cmd
                .env_remove("KITKRAFT_DATA_DIR")
                .env_remove("KITCRAFT_DATA_DIR")
                .env("KITKRAFT_DETECT", "0"),
        };
        let out = cmd.output().expect("run kit");
        let got: Value = serde_json::from_str(&stdout(&out)).expect("stdout is JSON");
        (out.status.code(), got)
    };
    let (status, got) = run(Some(&data));
    assert_eq!(status, Some(0), "{got:#}");
    assert_eq!(
        got["data"]["plugins"][0]["name"], "KitMaster.esm",
        "{got:#}"
    );
    let limitations = got["limitations"].to_string();
    assert!(
        limitations.contains("used KITKRAFT_DATA_DIR"),
        "{limitations}"
    );

    let (status, got) = run(None);
    assert_eq!(status, Some(2), "{got:#}");
    assert!(
        got["error"]["message"].to_string().contains("gamePath"),
        "{got:#}"
    );
    std::fs::remove_dir_all(&root).expect("cleanup");
}
