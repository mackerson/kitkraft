//! `strings.dump`, `strings.merge` (M2.G9) over `kitkraft-strings`.
//!
//! A table is a loose `.strings`/`.dlstrings`/`.ilstrings` file, or, with `archive`, a path
//! inside a `.ba2` (read through `kitkraft-ba2`, e.g. `strings/starfield_en.strings` in
//! `Starfield - Localization.ba2`).
//!
//! Tables are parsed strictly unless `tolerant` is set (M3.G2): then a table padded past
//! its header's data size is read too, and every padding run (and any unreferenced bytes
//! at the end of the declared data) is named in `limitations` and `data.padding`.

use std::fs;
use std::path::Path;

use kitkraft_ba2::Archive;
use kitkraft_strings::{merge as merge_tables, Entry, Kind, Padding, ParseMode, Table};
use serde_json::{json, Value};

use super::ba2::refuse_game_data;
use super::papyrus::read_capped;
use super::{failed, file_name, str_arg};
use crate::args::Violation;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

/// Most conflicts named in a merge result (all are counted).
const CONFLICTS_SHOWN: usize = 100;
/// Largest table read (256 MiB; the largest official table is far smaller). Checked against
/// the loose file's length or the archive record's declared size before anything is read.
const MAX_TABLE_BYTES: u64 = 256 << 20;

/// A loaded table and where it came from.
struct Loaded {
    table: Table,
    /// File name used in evidence (the table's own name).
    file: String,
    /// The archive it was read from, if any.
    archive: Option<String>,
    /// What a tolerant parse ignored (always empty in strict mode).
    padding: Vec<Padding>,
}

impl Loaded {
    /// One limitation per padding run.
    fn padding_limitations(&self) -> impl Iterator<Item = String> + '_ {
        self.padding
            .iter()
            .map(|p| format!("{}: {p}, ignored (tolerant parse)", self.file))
    }

    fn padding_json(&self) -> Vec<Value> {
        self.padding
            .iter()
            .map(|p| {
                json!({
                    "table": self.file,
                    "where": match p.kind {
                        kitkraft_strings::PaddingKind::DataTail => "data_tail",
                        kitkraft_strings::PaddingKind::AfterData => "after_data",
                    },
                    "offset": p.offset,
                    "length": p.len,
                    "zeros": p.zeros,
                })
            })
            .collect()
    }
}

/// The parse mode the `tolerant` argument asks for.
fn mode(args: &Value) -> ParseMode {
    if args
        .get("tolerant")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        ParseMode::Tolerant
    } else {
        ParseMode::Strict
    }
}

fn load(
    command: &str,
    table: &str,
    archive: Option<&str>,
    mode: ParseMode,
) -> Result<Loaded, EngineError> {
    let kind = Kind::from_path(Path::new(table)).ok_or_else(|| {
        failed(
            command,
            format!("`{table}`: extension is not .strings, .dlstrings or .ilstrings"),
        )
    })?;
    let bytes = match archive {
        None => read_capped(Path::new(table), MAX_TABLE_BYTES)
            .map_err(|e| failed(command, format!("reading `{table}`: {e}")))?,
        Some(a) => {
            let (archive_data, mut file) =
                Archive::open(Path::new(a)).map_err(|e| failed(command, format!("{a}: {e}")))?;
            let index = archive_data
                .find(table)
                .ok_or_else(|| failed(command, format!("`{table}` is not in {a}")))?;
            archive_data
                .extract_vec(&mut file, index, MAX_TABLE_BYTES)
                .map_err(|e| failed(command, format!("{a}: {e}")))?
        }
    };
    let parsed = Table::parse_with(kind, &bytes, mode).map_err(|e| {
        let hint =
            if mode == ParseMode::Strict && matches!(e, kitkraft_strings::Error::DataSize { .. }) {
                " (`tolerant: true` reads a table padded past its data size)"
            } else {
                ""
            };
        failed(command, format!("{table}: {e}{hint}"))
    })?;
    Ok(Loaded {
        table: parsed.table,
        file: file_name(Path::new(&table.replace('\\', "/"))),
        archive: archive.map(str::to_string),
        padding: parsed.padding,
    })
}

/// A string ID: `0x`-prefixed hex (`0x0002E469`) or decimal.
fn parse_id(command: &str, text: &str) -> Result<u32, EngineError> {
    let t = text.trim();
    let parsed = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => t.parse::<u32>(),
    };
    parsed.map_err(|_| EngineError::InvalidArgs {
        command: command.to_string(),
        violations: vec![Violation {
            pointer: "/id".into(),
            rule: "format",
            message: format!("`{text}` is not a string ID (0x-prefixed hex or decimal)"),
        }],
    })
}

fn entry_json(e: &Entry, kind: Kind) -> Value {
    let mut v = json!({
        "id": format!("0x{:08X}", e.id),
        "text": e.text(),
        "utf8": e.is_utf8(),
        "offset": e.offset,
        "length": e.stored_len(kind),
    });
    if !e.is_utf8() {
        if let Some(obj) = v.as_object_mut() {
            let hex: String = e.bytes.iter().map(|b| format!("{b:02X}")).collect();
            obj.insert("bytes_hex".into(), Value::String(hex));
        }
    }
    v
}

fn source_limitation(loaded: &Loaded) -> Option<String> {
    loaded.archive.as_ref().map(|a| {
        format!(
            "the table was read from {}; evidence offsets are within the table file `{}`, \
             not the archive",
            file_name(Path::new(a)),
            loaded.file
        )
    })
}

/// `strings.dump`.
pub(super) fn dump(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "strings.dump";
    let table_arg = str_arg(ID, args, "table")?;
    let archive = args.get("archive").and_then(Value::as_str);
    let id = args
        .get("id")
        .and_then(Value::as_str)
        .map(|t| parse_id(ID, t))
        .transpose()?;
    let contains = args
        .get("contains")
        .and_then(Value::as_str)
        .map(str::to_lowercase);
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(100);
    let start = args.get("start").and_then(Value::as_u64).unwrap_or(0);
    let loaded = load(ID, table_arg, archive, mode(args))?;
    let kind = loaded.table.kind;

    let matched: Vec<&Entry> = loaded
        .table
        .entries
        .iter()
        .filter(|e| id.is_none_or(|want| e.id == want))
        .filter(|e| {
            contains
                .as_ref()
                .is_none_or(|c| e.text().to_lowercase().contains(c.as_str()))
        })
        .collect();
    let shown: Vec<&Entry> = matched
        .iter()
        .copied()
        .skip(usize::try_from(start).unwrap_or(usize::MAX))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .collect();
    let evidence: Vec<Evidence> = shown
        .iter()
        .filter_map(|e| {
            Some(Evidence {
                file: loaded.file.clone(),
                offset: e.offset?,
                length: e.stored_len(kind),
                record: None,
                subrecord: None,
            })
        })
        .collect();
    let mut limitations: Vec<String> = source_limitation(&loaded).into_iter().collect();
    limitations.extend(loaded.padding_limitations());
    let (total, n) = (matched.len() as u64, shown.len() as u64);
    if n < total {
        limitations.push(format!(
            "showing {n} of {total} matching strings from position {start}; use `start` and \
             `limit` for the rest"
        ));
    }
    let invalid = shown.iter().filter(|e| !e.is_utf8()).count();
    if invalid > 0 {
        limitations.push(format!(
            "{invalid} shown strings are not valid UTF-8: `text` shows them decoded as \
             Windows-1252 and `bytes_hex` has the raw bytes"
        ));
    }
    let summary = match (id, shown.first()) {
        (Some(want), Some(e)) => format!("{}: 0x{want:08X} = {:?}", loaded.file, e.text()),
        (Some(want), None) => format!("{}: no string 0x{want:08X}", loaded.file),
        _ => format!(
            "{}: {} strings ({}), {total} matching, {n} shown",
            loaded.file,
            loaded.table.len(),
            kind.extension()
        ),
    };
    let data = json!({
        "table": table_arg,
        "archive": archive,
        "kind": kind.extension(),
        "count": loaded.table.len(),
        "matched": total,
        "start": start,
        "shown": n,
        "tolerant": mode(args) == ParseMode::Tolerant,
        "padding": loaded.padding_json(),
        "entries": shown.iter().map(|e| entry_json(e, kind)).collect::<Vec<_>>(),
        "summary": summary,
    });
    Ok(Outcome {
        data,
        coverage: Coverage {
            status: CoverageStatus::Complete,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    })
}

/// `strings.merge`.
pub(super) fn merge(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "strings.merge";
    let out = str_arg(ID, args, "out")?;
    let tables: Vec<&str> = args
        .get("tables")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let out_path = Path::new(out);
    refuse_game_data(ID, out_path)?;
    let mut loaded = Vec::with_capacity(tables.len());
    for t in &tables {
        loaded.push(load(ID, t, None, mode(args))?);
    }
    let kind = loaded
        .first()
        .map(|l| l.table.kind)
        .ok_or_else(|| failed(ID, "no tables given"))?;
    if Kind::from_path(out_path) != Some(kind) {
        return Err(failed(
            ID,
            format!(
                "`{out}` must have the .{} extension of the tables it merges",
                kind.extension()
            ),
        ));
    }
    for t in &tables {
        if fs::canonicalize(t).ok() == fs::canonicalize(out_path).ok() && out_path.exists() {
            return Err(failed(ID, format!("`{out}` is one of the inputs")));
        }
    }
    let inputs: Vec<Table> = loaded.iter().map(|l| l.table.clone()).collect();
    let merged = merge_tables(&inputs).map_err(|e| failed(ID, e.to_string()))?;
    let bytes = merged
        .table
        .to_bytes()
        .map_err(|e| failed(ID, e.to_string()))?;
    // Write beside the target and rename, so a failed write never leaves a truncated table.
    let mut tmp_name = out_path.as_os_str().to_os_string();
    tmp_name.push(".partial");
    let tmp = std::path::PathBuf::from(tmp_name);
    fs::write(&tmp, &bytes)
        .and_then(|()| fs::rename(&tmp, out_path))
        .map_err(|e| {
            let _ = fs::remove_file(&tmp);
            failed(ID, format!("writing `{out}`: {e}"))
        })?;

    let text = |table: usize, id: u32| -> Value {
        inputs
            .get(table)
            .and_then(|t| t.get(id))
            .map_or(Value::Null, |e| Value::String(e.text().into_owned()))
    };
    let conflicts: Vec<Value> = merged
        .conflicts
        .iter()
        .take(CONFLICTS_SHOWN)
        .map(|c| {
            json!({
                "id": format!("0x{:08X}", c.id),
                "replaced": tables.get(c.replaced),
                "winner": tables.get(c.winner),
                "old": text(c.replaced, c.id),
                "new": text(c.winner, c.id),
            })
        })
        .collect();
    let duplicates: Vec<Value> = merged
        .duplicates
        .iter()
        .take(CONFLICTS_SHOWN)
        .map(|d| {
            json!({
                "id": format!("0x{:08X}", d.id),
                "table": tables.get(d.table),
                "same_bytes": d.same_bytes,
                "kept": text(d.table, d.id),
            })
        })
        .collect();
    let mut limitations: Vec<String> = loaded
        .iter()
        .flat_map(Loaded::padding_limitations)
        .collect();
    if loaded.iter().any(|l| !l.padding.is_empty()) {
        limitations.push(
            "the merged table is written in the official layout, without padding".to_string(),
        );
    }
    if !merged.duplicates.is_empty() {
        limitations.push(format!(
            "{} IDs repeat inside one input table (see `duplicates`); the first occurrence \
             in that table is used, as lookups do, and the repeats are dropped{}",
            merged.duplicates.len(),
            if merged.duplicates.len() > CONFLICTS_SHOWN {
                format!("; `duplicates` lists the first {CONFLICTS_SHOWN}")
            } else {
                String::new()
            }
        ));
    }
    if merged.conflicts.len() > CONFLICTS_SHOWN {
        limitations.push(format!(
            "`conflicts` lists the first {CONFLICTS_SHOWN} of {}",
            merged.conflicts.len()
        ));
    }
    let out_label = file_name(out_path);
    let summary = format!(
        "{out_label}: merged {} tables into {} strings, {} conflicts (later table wins)",
        tables.len(),
        merged.table.len(),
        merged.conflicts.len()
    );
    Ok(Outcome {
        data: json!({
            "tables": tables,
            "out": out,
            "kind": kind.extension(),
            "entries": merged.table.len(),
            "bytes": bytes.len(),
            "conflict_count": merged.conflicts.len(),
            "conflicts": conflicts,
            "duplicate_count": merged.duplicates.len(),
            "duplicates": duplicates,
            "identical_repeats": merged.identical_repeats,
            "tolerant": mode(args) == ParseMode::Tolerant,
            "padding": loaded.iter().flat_map(Loaded::padding_json).collect::<Vec<_>>(),
            "summary": summary,
        }),
        coverage: Coverage {
            status: CoverageStatus::Complete,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence: vec![Evidence {
            file: out_label,
            offset: 0,
            length: bytes.len() as u64,
            record: None,
            subrecord: None,
        }],
    })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use crate::execute;
    use kitkraft_strings::{Entry, Kind, Table};
    use serde_json::json;
    use std::fs;

    /// A padded table fails strictly (with a hint) and reads with `tolerant`, the padding
    /// named in `limitations` and `data.padding`; merge writes it unpadded (M3.G2).
    #[test]
    fn padded_tables_need_tolerant_and_name_the_padding() {
        let dir = std::env::temp_dir().join(format!("kitkraft-strings-pad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let good = Table::new(Kind::Strings, vec![Entry::new(7, "Rifle")])
            .to_bytes()
            .unwrap();
        let mut padded = good.clone();
        padded.extend_from_slice(&[0, 0, 0, 0]);
        let path = dir.join("padded_en.strings");
        fs::write(&path, &padded).unwrap();
        let table = path.to_str().unwrap();

        let err = execute("strings.dump", json!({"table": table})).unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("tolerant"), "{err}");

        let r = execute("strings.dump", json!({"table": table, "tolerant": true})).unwrap();
        assert_eq!(r["data"]["entries"][0]["text"], "Rifle");
        assert_eq!(r["data"]["tolerant"], true);
        assert_eq!(
            r["data"]["padding"],
            json!([{"table": "padded_en.strings", "where": "after_data",
                    "offset": good.len(), "length": 4, "zeros": true}])
        );
        let limitation = format!(
            "padded_en.strings: 4 bytes of zero padding at offset {} (after the header's data \
             size), ignored (tolerant parse)",
            good.len()
        );
        assert_eq!(r["limitations"], json!([limitation]));

        let other = dir.join("other_en.strings");
        fs::write(&other, &good).unwrap();
        let out = dir.join("merged_en.strings");
        let merged = execute(
            "strings.merge",
            json!({"tables": [table, other.to_str().unwrap()],
                   "out": out.to_str().unwrap(), "tolerant": true}),
        )
        .unwrap();
        assert_eq!(merged["data"]["padding"].as_array().unwrap().len(), 1);
        assert_eq!(fs::read(&out).unwrap(), good);
        let limitations = merged["limitations"].as_array().unwrap();
        assert!(limitations[1].as_str().unwrap().contains("without padding"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dump_and_merge_through_execute() {
        let dir = std::env::temp_dir().join(format!("kitkraft-strings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let a = dir.join("mod_en.strings");
        let b = dir.join("patch_en.strings");
        let t = |entries: Vec<Entry>| Table::new(Kind::Strings, entries).to_bytes().unwrap();
        fs::write(
            &a,
            t(vec![
                Entry::new(1, "Rifle"),
                Entry::new(0x2E469, "Old Earth Hunting Rifle"),
            ]),
        )
        .unwrap();
        fs::write(
            &b,
            t(vec![
                Entry::new(0x2E469, "Old Soviet Marksman Rifle"),
                Entry::new(3, "Pistol"),
            ]),
        )
        .unwrap();

        let dumped = execute(
            "strings.dump",
            json!({"table": a.to_str().unwrap(), "id": "0x2E469"}),
        )
        .unwrap();
        assert_eq!(dumped["data"]["matched"], 1);
        assert_eq!(
            dumped["data"]["entries"][0]["text"],
            "Old Earth Hunting Rifle"
        );
        assert_eq!(dumped["data"]["entries"][0]["id"], "0x0002E469");
        // 8-byte header + 2 directory entries + "Rifle\0".
        assert_eq!(dumped["evidence"][0]["offset"], 8 + 16 + 6);
        assert_eq!(dumped["evidence"][0]["length"], 24);
        let by_text = execute(
            "strings.dump",
            json!({"table": a.to_str().unwrap(), "contains": "HUNTING"}),
        )
        .unwrap();
        assert_eq!(by_text["data"]["matched"], 1);
        let bad = execute(
            "strings.dump",
            json!({"table": a.to_str().unwrap(), "id": "zz"}),
        )
        .unwrap_err();
        assert_eq!(bad.kind(), "invalid_args");

        let out = dir.join("merged_en.strings");
        let merged = execute(
            "strings.merge",
            json!({"tables": [a.to_str().unwrap(), b.to_str().unwrap()],
                   "out": out.to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(merged["data"]["entries"], 3);
        assert_eq!(merged["data"]["conflict_count"], 1);
        assert_eq!(
            merged["data"]["conflicts"][0]["new"],
            "Old Soviet Marksman Rifle"
        );
        assert_eq!(
            merged["data"]["conflicts"][0]["old"],
            "Old Earth Hunting Rifle"
        );
        let table = Table::read_file(&out).unwrap();
        let ids: Vec<u32> = table.entries.iter().map(|e| e.id).collect();
        assert_eq!(ids, [1, 3, 0x2E469]);
        let first = fs::read(&out).unwrap();
        execute(
            "strings.merge",
            json!({"tables": [a.to_str().unwrap(), b.to_str().unwrap()],
                   "out": out.to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(fs::read(&out).unwrap(), first, "deterministic");
        // Regression (review round 2): written through a temporary file and a rename; a
        // failed rename leaves neither a partial table nor the temporary file.
        assert!(!dir.join("merged_en.strings.partial").exists());
        let blocked = dir.join("blocked.strings");
        fs::create_dir_all(&blocked).unwrap();
        let err = execute(
            "strings.merge",
            json!({"tables": [a.to_str().unwrap(), b.to_str().unwrap()],
                   "out": blocked.to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("writing"), "{err}");
        assert!(!dir.join("blocked.strings.partial").exists());

        // Regression (review round 2): an ID repeated inside one input is reported as a
        // duplicate, apart from the cross-table conflicts.
        let c = dir.join("dup_en.strings");
        fs::write(&c, t(vec![Entry::new(7, "kept"), Entry::new(7, "dropped")])).unwrap();
        let merged = execute(
            "strings.merge",
            json!({"tables": [a.to_str().unwrap(), c.to_str().unwrap()],
                   "out": dir.join("m2_en.strings").to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(merged["data"]["conflict_count"], 0);
        assert_eq!(merged["data"]["duplicate_count"], 1);
        assert_eq!(merged["data"]["duplicates"][0]["id"], "0x00000007");
        assert_eq!(merged["data"]["duplicates"][0]["kept"], "kept");
        assert_eq!(merged["data"]["duplicates"][0]["same_bytes"], false);
        assert!(merged["limitations"][0]
            .as_str()
            .unwrap()
            .contains("repeat inside one input"));

        let wrong_ext = execute(
            "strings.merge",
            json!({"tables": [a.to_str().unwrap(), b.to_str().unwrap()],
                   "out": dir.join("x.dlstrings").to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(wrong_ext.to_string().contains(".strings"), "{wrong_ext}");
        let self_out = execute(
            "strings.merge",
            json!({"tables": [a.to_str().unwrap(), b.to_str().unwrap()],
                   "out": a.to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(
            self_out.to_string().contains("one of the inputs"),
            "{self_out}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The known string from the official English table, through `execute` (skips without
    /// `KITKRAFT_DATA_DIR`): `Starfield.esm` WEAP 0021BBCD stores FULL = 0x0002E469.
    #[test]
    fn official_known_string() {
        let Some(dir) =
            crate::handlers::env_var_os("KITKRAFT_DATA_DIR").map(std::path::PathBuf::from)
        else {
            println!("skipped: KITKRAFT_DATA_DIR is not set");
            return;
        };
        let archive = dir.join("Starfield - Localization.ba2");
        if !archive.is_file() {
            println!("skipped: {} is missing", archive.display());
            return;
        }
        let r = execute(
            "strings.dump",
            json!({"archive": archive.to_str().unwrap(),
                   "table": "strings/starfield_en.strings", "id": "0x0002E469"}),
        )
        .unwrap();
        assert_eq!(r["data"]["entries"][0]["text"], "Old Earth Hunting Rifle");
        assert_eq!(r["evidence"][0]["file"], "starfield_en.strings");
        assert!(r["limitations"][0]
            .as_str()
            .unwrap()
            .contains("Localization.ba2"));
    }
}
