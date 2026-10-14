//! `roundtrip` (M1.12): parse a plugin and write it back through the `kitcraft-esm`
//! streaming driver, comparing the output with the input byte for byte.
//!
//! The driver always compares. `verify` decides whether a difference is a failed check:
//! with it, `data.passed` is `identical` (and `kit` exits 1 on `false`); without it,
//! `data.passed` is `null` and a difference is only reported. `decode` (M3.F1) also
//! inflates every compressed body strictly and re-encodes it
//! (`RoundtripOptions::with_decode`, M3.A2); it is a check too, so with it `data.passed`
//! is `identical` and a body that fails is a mismatch.
//!
//! `out` never replaces an existing file unless `overwrite` is `true`: the file is opened
//! with `create_new`, so an existing one is a `failed` error and stays untouched.

use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

use kitcraft_esm::driver::{roundtrip_file_to, RoundtripOptions, RoundtripReport};
use serde_json::{json, Value};

use super::{failed, file_name, str_arg};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

const ID: &str = "roundtrip";

/// What the comparison covers, stated on every result.
pub(crate) const CONTAINER_ONLY: &str = "container level: every group, record and subrecord \
     header is parsed and rewritten; subrecord payloads are compared as opaque bytes and \
     compressed record bodies are re-emitted as read, not decompressed";

/// What the comparison covers with `decode`, stated instead of [`CONTAINER_ONLY`].
pub(crate) const DECODED: &str = "container level plus strict decode: every group, record \
     and subrecord header is parsed and rewritten and subrecord payloads are compared as \
     opaque bytes; every compressed body is inflated strictly (declared size, end marker and \
     checksum, nothing after the stream) and its subrecords parsed and re-encoded, and the \
     original compressed bytes are what is written";

/// Stated when the driver stopped at a mismatch.
pub(crate) const STOPPED_AT_MISMATCH: &str = "stopped at the first mismatch: counts cover only \
     the top-level entries read up to and including the one that differs";

/// Stated when `out` was given and the rewrite differed.
pub(crate) const OUT_INCOMPLETE: &str = "the --out file is incomplete: writing stopped after \
     the top-level entry containing the first mismatch";

/// The handler.
pub(super) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let plugin = str_arg(ID, args, "plugin")?;
    let verify = args.get("verify").and_then(Value::as_bool).unwrap_or(false);
    let out = args.get("out").and_then(Value::as_str);
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let decode = args.get("decode").and_then(Value::as_bool).unwrap_or(false);
    let path = Path::new(plugin);
    let options = if decode {
        RoundtripOptions::with_decode()
    } else {
        RoundtripOptions::default()
    };

    let start = Instant::now();
    let report = match out {
        None => roundtrip_file_to(path, &options, None),
        Some(out) => {
            let out_path = Path::new(out);
            refuse_same_file(path, out_path)?;
            let file = open_out(out_path, overwrite)
                .map_err(|e| failed(ID, format!("cannot create `{out}`: {e}")))?;
            let mut writer = BufWriter::new(file);
            let report = roundtrip_file_to(path, &options, Some(&mut writer));
            let flushed = writer.flush();
            if report.is_err() {
                // Do not leave a half-written file that looks like a plugin.
                drop(writer);
                let _ = fs::remove_file(out_path);
            } else if let Err(e) = flushed {
                return Err(failed(ID, format!("writing `{out}`: {e}")));
            }
            report
        }
    }
    .map_err(|e| failed(ID, format!("{plugin}: {e}")))?;
    let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

    Ok(outcome(
        plugin,
        &file_name(path),
        Checks { verify, decode },
        out,
        &report,
        elapsed_ms,
    ))
}

/// Opens `out` for writing: a new file, or (with `overwrite`) an existing one truncated.
fn open_out(out: &Path, overwrite: bool) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.write(true);
    if overwrite {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    options.open(out).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            std::io::Error::new(
                e.kind(),
                "the file exists; pass `overwrite: true` (`--overwrite`) to replace it",
            )
        } else {
            e
        }
    })
}

/// Writing the output over the input would truncate it before it is read.
fn refuse_same_file(plugin: &Path, out: &Path) -> Result<(), EngineError> {
    if let (Ok(a), Ok(b)) = (fs::canonicalize(plugin), fs::canonicalize(out)) {
        if a == b {
            return Err(failed(
                ID,
                format!(
                    "--out `{}` is the input plugin; refusing to overwrite it",
                    out.display()
                ),
            ));
        }
    }
    Ok(())
}

/// The checks a run asked for: either makes a difference a failed check (`passed`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Checks {
    /// `verify`: byte-identical output is required.
    pub verify: bool,
    /// `decode`: every compressed body is inflated strictly and re-encoded.
    pub decode: bool,
}

/// Builds the envelope parts from a driver report.
pub(crate) fn outcome(
    plugin: &str,
    file: &str,
    checks: Checks,
    out: Option<&str>,
    report: &RoundtripReport,
    elapsed_ms: u64,
) -> Outcome {
    let Checks { verify, decode } = checks;
    let identical = report.identical();
    let stats = &report.stats;
    let mut limitations = vec![if decode { DECODED } else { CONTAINER_ONLY }.to_string()];
    let mut not_examined = Vec::new();

    let (mismatch, evidence, summary) = if identical {
        let evidence = Evidence {
            file: file.to_string(),
            offset: 0,
            length: report.input_len,
            record: None,
            subrecord: None,
        };
        let summary = format!(
            "{file}: identical ({} bytes, {} records, {} groups, {} compressed records) in {elapsed_ms} ms",
            report.input_len, stats.records, stats.groups, stats.compressed_records
        );
        (Value::Null, evidence, summary)
    } else {
        limitations.push(STOPPED_AT_MISMATCH.to_string());
        if out.is_some() {
            limitations.push(OUT_INCOMPLETE.to_string());
        }
        // `identical()` is false either with a mismatch or with a length difference; the
        // driver reports the latter as a mismatch too, but do not rely on it.
        let (offset, expected, actual, path) = match &report.mismatch {
            Some(m) => (m.offset, m.expected, m.actual, m.path.as_str()),
            None => (
                report.input_len.min(report.output_len),
                None,
                None,
                "end of file",
            ),
        };
        if report.output_len < report.input_len {
            not_examined.push(format!(
                "bytes {}..{} (after the top-level entry with the first mismatch)",
                report.output_len, report.input_len
            ));
        }
        let (record, subrecord) = record_in_path(path);
        let evidence = Evidence {
            file: file.to_string(),
            offset,
            length: 1,
            record,
            subrecord,
        };
        let summary = match (expected, actual) {
            // A strict-decode fault (`decode`): no byte pair, the path names the fault.
            (None, None) if decode => {
                format!("{file}: MISMATCH at offset {offset} (0x{offset:X}): {path}")
            }
            _ => format!(
                "{file}: MISMATCH at offset {offset} (0x{offset:X}): expected {}, wrote {} in {path}",
                byte_text(expected),
                byte_text(actual)
            ),
        };
        let mismatch = json!({
            "offset": offset,
            "expected": expected,
            "actual": actual,
            "path": path,
        });
        (mismatch, evidence, summary)
    };

    let data = json!({
        "plugin": plugin,
        "identical": identical,
        "verify": verify,
        "decode": decode,
        "passed": if verify || decode { Value::Bool(identical) } else { Value::Null },
        "bytes": report.input_len,
        "bytes_compared": report.output_len,
        "records": stats.records,
        "groups": stats.groups,
        "compressed_records": stats.compressed_records,
        "elapsed_ms": elapsed_ms,
        "out": out,
        "mismatch": mismatch,
        "summary": summary,
    });
    Outcome {
        data,
        coverage: Coverage {
            status: if identical {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined,
        },
        limitations,
        evidence: vec![evidence],
    }
}

fn byte_text(byte: Option<u8>) -> String {
    byte.map_or_else(|| "end of data".to_string(), |b| format!("0x{b:02X}"))
}

/// Pulls `SIGN:FORMID` and the subrecord signature out of a driver path such as
/// `GRUP WEAP (type 0) @123 > WEAP 0001A2B3 @147 > FULL @171`.
fn record_in_path(path: &str) -> (Option<String>, Option<String>) {
    let mut record = None;
    let mut subrecord = None;
    for segment in path.split(" > ") {
        let Some((head, _at)) = segment.rsplit_once(" @") else {
            continue;
        };
        if head.starts_with("GRUP ") && head.ends_with(')') {
            continue;
        }
        if record.is_none() {
            record = head
                .rsplit_once(' ')
                .map(|(signature, form)| format!("{signature}:{form}"));
        } else if subrecord.is_none() {
            subrecord = Some(head.to_string());
        }
    }
    (record, subrecord)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{
        outcome, record_in_path, Checks, CONTAINER_ONLY, DECODED, OUT_INCOMPLETE,
        STOPPED_AT_MISMATCH,
    };
    use crate::handlers::testdata;
    use crate::{execute, EngineError};
    use kitcraft_esm::driver::{Mismatch, RoundtripReport, Stats};
    use serde_json::{json, Value};

    /// `elapsed_ms` varies run to run; everything else must match exactly.
    fn without_elapsed(mut result: Value) -> Value {
        let ms = result["data"]["elapsed_ms"].take();
        assert!(ms.is_u64(), "elapsed_ms is a count: {ms}");
        let summary = result["data"]["summary"].take();
        let summary = summary.as_str().unwrap().to_string();
        let cut = summary.rfind(" in ").unwrap();
        result["data"]["summary"] = Value::String(summary[..cut].to_string());
        result
    }

    #[test]
    fn synthetic_plugin_is_identical_through_execute() {
        let bytes = testdata::plugin();
        let path = testdata::write_temp("roundtrip-identical", &bytes);
        let plugin = path.to_str().unwrap();
        let result = execute("roundtrip", json!({"plugin": plugin, "verify": true})).unwrap();
        let file = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(
            without_elapsed(result),
            json!({
                "command": "roundtrip",
                "data": {
                    "plugin": plugin,
                    "identical": true,
                    "verify": true,
                    "decode": false,
                    "passed": true,
                    "bytes": bytes.len(),
                    "bytes_compared": bytes.len(),
                    "records": testdata::RECORDS,
                    "groups": testdata::GROUPS,
                    "compressed_records": 1,
                    "elapsed_ms": null,
                    "out": null,
                    "mismatch": null,
                    "summary": format!(
                        "{file}: identical ({} bytes, {} records, {} groups, 1 compressed records)",
                        bytes.len(), testdata::RECORDS, testdata::GROUPS
                    ),
                },
                "coverage": {
                    "status": "complete",
                    "decoded_subrecords": null,
                    "opaque_subrecords": null,
                    "not_examined": []
                },
                "limitations": [CONTAINER_ONLY],
                "evidence": [{"file": file, "offset": 0, "length": bytes.len(),
                              "record": null, "subrecord": null}]
            })
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn without_verify_passed_is_null() {
        let path = testdata::write_temp("roundtrip-noverify", &testdata::plugin());
        let result = execute("roundtrip", json!({"plugin": path.to_str().unwrap()})).unwrap();
        assert_eq!(result["data"]["identical"], true);
        assert_eq!(result["data"]["verify"], false);
        assert_eq!(result["data"]["passed"], Value::Null);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn out_writes_the_rebuilt_plugin() {
        let bytes = testdata::plugin();
        let path = testdata::write_temp("roundtrip-in", &bytes);
        let out = path.with_extension("out.esm");
        let result = execute(
            "roundtrip",
            json!({"plugin": path.to_str().unwrap(), "out": out.to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(result["data"]["identical"], true);
        assert_eq!(result["data"]["out"], out.to_str().unwrap());
        assert_eq!(std::fs::read(&out).unwrap(), bytes);
        std::fs::remove_file(&out).unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn out_may_not_be_the_input() {
        let bytes = testdata::plugin();
        let path = testdata::write_temp("roundtrip-self", &bytes);
        let p = path.to_str().unwrap();
        for overwrite in [false, true] {
            let err = execute(
                "roundtrip",
                json!({"plugin": p, "out": p, "overwrite": overwrite}),
            )
            .unwrap_err();
            assert_eq!(err.kind(), "failed");
            assert!(err.to_string().contains("refusing"), "{err}");
            assert_eq!(std::fs::read(&path).unwrap(), bytes, "input untouched");
        }
        std::fs::remove_file(&path).unwrap();
    }

    /// An existing `out` is never truncated unless `overwrite` is set.
    #[test]
    fn existing_out_needs_overwrite() {
        let bytes = testdata::plugin();
        let path = testdata::write_temp("roundtrip-exists-in", &bytes);
        let out = testdata::write_temp("roundtrip-exists-out", b"keep me");
        let args = json!({"plugin": path.to_str().unwrap(), "out": out.to_str().unwrap()});
        let err = execute("roundtrip", args.clone()).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("overwrite"), "{err}");
        assert_eq!(
            std::fs::read(&out).unwrap(),
            b"keep me",
            "existing file untouched"
        );

        let mut args = args;
        args["overwrite"] = json!(true);
        let result = execute("roundtrip", args).unwrap();
        assert_eq!(result["data"]["identical"], true);
        assert_eq!(std::fs::read(&out).unwrap(), bytes);
        std::fs::remove_file(&out).unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn unreadable_and_malformed_plugins_fail_with_offsets() {
        let missing = std::env::temp_dir().join("kitcraft-engine-no-such-plugin.esm");
        let err = execute("roundtrip", json!({"plugin": missing.to_str().unwrap()})).unwrap_err();
        assert!(matches!(err, EngineError::Failed { .. }), "{err:?}");

        let mut bytes = testdata::plugin();
        bytes.truncate(bytes.len() - 3);
        let path = testdata::write_temp("roundtrip-truncated", &bytes);
        let out = path.with_extension("partial.esm");
        let err = execute(
            "roundtrip",
            json!({"plugin": path.to_str().unwrap(), "out": out.to_str().unwrap()}),
        )
        .unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("offset"), "{err}");
        assert!(
            !out.exists(),
            "a failed roundtrip removes its partial output"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn mismatch_carries_offset_path_and_evidence() {
        let report = RoundtripReport {
            input_len: 1000,
            output_len: 400,
            mismatch: Some(Mismatch {
                offset: 171,
                expected: Some(0x41),
                actual: Some(0x42),
                path: "GRUP WEAP (type 0) @123 > WEAP 0001A2B3 @147 > FULL @171".into(),
            }),
            stats: Stats {
                records: 3,
                groups: 1,
                ..Stats::default()
            },
        };
        let checks = |verify, decode| Checks { verify, decode };
        let o = outcome(
            "D:\\X.esm",
            "X.esm",
            checks(true, false),
            Some("o.esm"),
            &report,
            5,
        );
        assert_eq!(o.data["identical"], false);
        assert_eq!(o.data["passed"], false);
        assert_eq!(
            o.data["mismatch"],
            json!({"offset": 171, "expected": 0x41, "actual": 0x42,
                   "path": "GRUP WEAP (type 0) @123 > WEAP 0001A2B3 @147 > FULL @171"})
        );
        assert_eq!(
            o.data["summary"],
            "X.esm: MISMATCH at offset 171 (0xAB): expected 0x41, wrote 0x42 in \
             GRUP WEAP (type 0) @123 > WEAP 0001A2B3 @147 > FULL @171"
        );
        let ev = &o.evidence[0];
        assert_eq!((ev.offset, ev.length), (171, 1));
        assert_eq!(ev.record.as_deref(), Some("WEAP:0001A2B3"));
        assert_eq!(ev.subrecord.as_deref(), Some("FULL"));
        assert_eq!(o.coverage.status, crate::CoverageStatus::Partial);
        assert_eq!(
            o.coverage.not_examined,
            ["bytes 400..1000 (after the top-level entry with the first mismatch)"]
        );
        assert_eq!(
            o.limitations,
            [CONTAINER_ONLY, STOPPED_AT_MISMATCH, OUT_INCOMPLETE]
        );

        // Without --verify the difference is reported but is not a failed check.
        let o = outcome("X.esm", "X.esm", checks(false, false), None, &report, 5);
        assert_eq!(o.data["passed"], Value::Null);
        assert_eq!(o.limitations, [CONTAINER_ONLY, STOPPED_AT_MISMATCH]);
        // `decode` is a check of its own.
        let o = outcome("X.esm", "X.esm", checks(false, true), None, &report, 5);
        assert_eq!(o.data["passed"], false);
        assert_eq!(o.limitations, [DECODED, STOPPED_AT_MISMATCH]);
    }

    /// The compressed weapon of the synthetic plugin with the last byte of its zlib stream
    /// (the Adler-32 checksum) flipped: same size, so only `decode` sees it.
    fn corrupt_plugin() -> Vec<u8> {
        let mut bytes = testdata::plugin();
        let packed = kitcraft_esm::compress::deflate(&testdata::packed_fields(), 9, 0).unwrap();
        let at = bytes
            .windows(packed.len())
            .position(|w| w == packed.as_slice())
            .unwrap();
        *bytes.get_mut(at + packed.len() - 1).unwrap() ^= 0xFF;
        bytes
    }

    #[test]
    fn decode_reports_a_corrupt_compressed_stream() {
        let path = testdata::write_temp("roundtrip-corrupt", &corrupt_plugin());
        let plugin = path.to_str().unwrap();
        // Container level: the body is re-emitted as read, so the file is identical.
        let plain = execute("roundtrip", json!({"plugin": plugin, "verify": true})).unwrap();
        assert_eq!(plain["data"]["passed"], true);
        let r = execute("roundtrip", json!({"plugin": plugin, "decode": true})).unwrap();
        assert_eq!(r["data"]["decode"], true);
        assert_eq!(r["data"]["identical"], false);
        assert_eq!(r["data"]["passed"], false);
        let path_text = r["data"]["mismatch"]["path"].as_str().unwrap();
        assert!(path_text.contains("compressed body"), "{r}");
        assert_eq!(r["data"]["mismatch"]["expected"], Value::Null);
        let summary = r["data"]["summary"].as_str().unwrap();
        assert!(
            summary.contains("MISMATCH") && summary.contains("compressed body"),
            "{summary}"
        );
        assert_eq!(
            r["evidence"][0]["record"],
            format!("WEAP:{:08X}", testdata::PACKED)
        );
        assert_eq!(r["limitations"][0], DECODED);
        // A sound plugin passes with decode.
        let good = testdata::write_temp("roundtrip-decode-good", &testdata::plugin());
        let ok = execute(
            "roundtrip",
            json!({"plugin": good.to_str().unwrap(), "decode": true}),
        )
        .unwrap();
        assert_eq!(ok["data"]["passed"], true);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(&good).unwrap();
    }

    #[test]
    fn paths_split_into_record_and_subrecord() {
        assert_eq!(
            record_in_path("GRUP CELL (type 0) @10 > GRUP 00000200 (type 6) @50"),
            (None, None)
        );
        assert_eq!(
            record_in_path("TES4 00000000 @0 > HEDR @24"),
            (Some("TES4:00000000".into()), Some("HEDR".into()))
        );
        assert_eq!(
            record_in_path("GRUP LAND (type 0) @9 > LAND 00000009 @33"),
            (Some("LAND:00000009".into()), None)
        );
        assert_eq!(record_in_path(""), (None, None));
    }
}
