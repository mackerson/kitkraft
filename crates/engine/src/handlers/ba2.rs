//! `ba2.list`, `ba2.extract`, `ba2.pack` (M2.G4) over `kitkraft-ba2`.
//!
//! Patterns are regular expressions matched case-insensitively against archive paths (as
//! `ba2x.py` matches them). Output is never written under a game `Data` directory: a
//! directory holding `Starfield.esm`, any directory above one, or `KITKRAFT_DATA_DIR`
//! ([`refuse_game_data`], shared with the other lane G commands).

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use kitkraft_ba2::{Archive, Body, Entry, Kind, PackOptions};
use regex::{Regex, RegexBuilder};
use serde_json::{json, Value};

use super::{failed, file_name, str_arg};
use crate::args::Violation;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;

/// Most entries listed in one result unless `limit` says otherwise.
const DEFAULT_LIMIT: u64 = 1000;
/// Most extracted or packed files named in a result (the counts cover all of them).
const FILES_SHOWN: usize = 1000;
/// Deepest directory nesting `ba2.pack` walks.
const MAX_DEPTH: usize = 64;
/// Default `max_entry_bytes` of `ba2.extract`: 1 GiB, above the largest official entry
/// (134,083,688 bytes, `docs/corpus.md` "Archives"), far below what a hostile record can
/// declare (a DX10 texture up to 255 chunks of 4 GiB).
const DEFAULT_MAX_ENTRY: u64 = 1 << 30;

/// What the archive-level evidence covers.
const RECORDS_EVIDENCE: &str = "evidence covers the header, the record table and the name \
     table; each entry's `record_offset` locates its own record";

/// `ba2.list`.
pub(super) fn list(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "ba2.list";
    let archive_arg = str_arg(ID, args, "archive")?;
    let filter = pattern(ID, args, "filter")?;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_LIMIT);
    let start = args.get("start").and_then(Value::as_u64).unwrap_or(0);
    let path = Path::new(archive_arg);
    let (archive, _file) =
        Archive::open(path).map_err(|e| failed(ID, format!("{archive_arg}: {e}")))?;

    let matched: Vec<(usize, &Entry)> = archive
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| filter.as_ref().is_none_or(|re| re.is_match(&e.name)))
        .collect();
    let shown: Vec<Value> = matched
        .iter()
        .skip(usize::try_from(start).unwrap_or(usize::MAX))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .map(|(i, e)| entry_json(*i, e))
        .collect();
    let file = file_name(path);
    let mut limitations = vec![RECORDS_EVIDENCE.to_string()];
    let total_matched = matched.len() as u64;
    let shown_count = shown.len() as u64;
    if shown_count < total_matched {
        limitations.push(format!(
            "showing {shown_count} of {total_matched} matching entries from position {start}; \
             use `start` and `limit` for the rest"
        ));
    }
    if !archive.has_names {
        limitations
            .push("the archive has no name table: names are `<dir hash>/<name hash>.<ext>`".into());
    }
    let codec = archive.header.codec().map(|c| c.name()).ok();
    let summary = format!(
        "{file}: v{} {} ({}), {} entries, {total_matched} matching, {shown_count} shown",
        archive.header.version,
        archive.header.kind.name(),
        codec.unwrap_or("unknown codec"),
        archive.entries.len()
    );
    let data = json!({
        "archive": archive_arg,
        "version": archive.header.version,
        "kind": archive.header.kind.name(),
        "codec": codec,
        "compression_field": archive.header.compression,
        "file_count": archive.header.file_count,
        "bytes": archive.file_len,
        "has_names": archive.has_names,
        "filter": args.get("filter"),
        "matched": total_matched,
        "start": start,
        "shown": shown_count,
        "entries": shown,
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
        evidence: archive_evidence(&archive, &file),
    })
}

/// `ba2.extract`.
pub(super) fn extract(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "ba2.extract";
    let archive_arg = str_arg(ID, args, "archive")?;
    let out_dir = PathBuf::from(str_arg(ID, args, "out_dir")?);
    let pat = pattern(ID, args, "pattern")?;
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_entry = args
        .get("max_entry_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_MAX_ENTRY);
    refuse_game_data(ID, &out_dir)?;
    let path = Path::new(archive_arg);
    let (archive, mut file) =
        Archive::open(path).map_err(|e| failed(ID, format!("{archive_arg}: {e}")))?;

    let selected = archive
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| pat.as_ref().is_none_or(|re| re.is_match(&e.name)));
    let plan = plan_extraction(selected, &out_dir, overwrite, max_entry)
        .map_err(|why| failed(ID, format!("{archive_arg}: {why}; nothing was written")))?;

    let mut files = Vec::new();
    let mut bytes = 0u64;
    for (i, target) in &plan {
        let entry = archive
            .entries
            .get(*i)
            .ok_or_else(|| failed(ID, "entry index"))?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| failed(ID, format!("creating `{}`: {e}", parent.display())))?;
        }
        let out = File::create(target)
            .map_err(|e| failed(ID, format!("creating `{}`: {e}", target.display())))?;
        let mut writer = BufWriter::new(out);
        let result = archive
            .extract_to(&mut file, *i, &mut writer)
            .and_then(|n| {
                writer
                    .flush()
                    .map_err(|source| kitkraft_ba2::Error::Io { offset: n, source })?;
                Ok(n)
            });
        let n = match result {
            Ok(n) => n,
            Err(e) => {
                drop(writer);
                let _ = fs::remove_file(target);
                return Err(failed(ID, format!("{archive_arg}: {e}")));
            }
        };
        bytes = bytes.saturating_add(n);
        if files.len() < FILES_SHOWN {
            files.push(json!({
                "index": i,
                "name": entry.name,
                "size": n,
                "path": target.display().to_string(),
            }));
        }
    }
    let file_label = file_name(path);
    let mut limitations = vec![RECORDS_EVIDENCE.to_string()];
    if plan.len() > FILES_SHOWN {
        limitations.push(format!(
            "`files` names the first {FILES_SHOWN} of {} extracted files",
            plan.len()
        ));
    }
    if archive.header.kind == Kind::Textures {
        limitations.push(
            "DX10 entries are written with a synthesized DDS header (byte-identical to \
             Archive2.exe 1.2.0.1's extraction of the official archives)"
                .into(),
        );
    }
    let summary = format!(
        "{file_label}: extracted {} files ({bytes} bytes) to {}",
        plan.len(),
        out_dir.display()
    );
    Ok(Outcome {
        data: json!({
            "archive": archive_arg,
            "out_dir": out_dir.display().to_string(),
            "pattern": args.get("pattern"),
            "extracted": plan.len(),
            "bytes": bytes,
            "files": files,
            "summary": summary,
        }),
        coverage: Coverage {
            status: CoverageStatus::Complete,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence: archive_evidence(&archive, &file_label),
    })
}

/// Validates every target before the first write, so a refused extraction writes nothing:
/// sizes against `max_entry`, names through [`kitkraft_ba2::relative_path`] (which also
/// refuses names Windows would alter), two entries that land on one file of a
/// case-insensitive file system (Windows, the game's), an entry that is both a file and
/// another entry's directory, an existing directory or file in the way, and existing
/// files unless `overwrite`. Returns `(entry index, target)` in archive order.
fn plan_extraction<'a>(
    selected: impl Iterator<Item = (usize, &'a Entry)>,
    out_dir: &Path,
    overwrite: bool,
    max_entry: u64,
) -> Result<Vec<(usize, PathBuf)>, String> {
    let mut plan = Vec::new();
    // Folded target path -> the entry name that claimed it.
    let mut files: BTreeMap<String, &str> = BTreeMap::new();
    let mut dirs: BTreeMap<String, &str> = BTreeMap::new();
    for (i, entry) in selected {
        let size = entry.extracted_len();
        if size > max_entry {
            return Err(format!(
                "`{}` extracts to {size} bytes, above max_entry_bytes ({max_entry})",
                entry.name
            ));
        }
        let rel = kitkraft_ba2::relative_path(&entry.name).map_err(|e| e.to_string())?;
        let parts: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect();
        let key = parts.join("/");
        if let Some(other) = files.insert(key.clone(), &entry.name) {
            return Err(format!(
                "`{other}` and `{}` extract to the same file on a case-insensitive file system",
                entry.name
            ));
        }
        for n in 1..parts.len() {
            let dir = parts.get(..n).unwrap_or_default().join("/");
            dirs.entry(dir).or_insert(&entry.name);
        }
        let target = out_dir.join(&rel);
        if target.is_dir() {
            return Err(format!("`{}` is an existing directory", target.display()));
        }
        if !overwrite && target.exists() {
            return Err(format!(
                "`{}` exists; pass overwrite to replace it",
                target.display()
            ));
        }
        for dir in target.ancestors().skip(1) {
            if !dir.starts_with(out_dir) {
                break;
            }
            if dir.exists() && !dir.is_dir() {
                return Err(format!(
                    "`{}` is a file where `{}` needs a directory",
                    dir.display(),
                    entry.name
                ));
            }
        }
        plan.push((i, target));
    }
    for (key, name) in &files {
        if let Some(other) = dirs.get(key) {
            return Err(format!(
                "`{name}` is a file but `{other}` needs it as a directory"
            ));
        }
    }
    Ok(plan)
}

/// `ba2.pack`.
pub(super) fn pack(args: &Value) -> Result<Outcome, EngineError> {
    const ID: &str = "ba2.pack";
    let dir = PathBuf::from(str_arg(ID, args, "dir")?);
    let out = PathBuf::from(str_arg(ID, args, "out")?);
    let kind = match args.get("format").and_then(Value::as_str) {
        Some("dx10") => Kind::Textures,
        _ => Kind::General,
    };
    let version = args
        .get("version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(2);
    let compress = args
        .get("compress")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    refuse_game_data(ID, &out)?;
    if out.is_dir() {
        return Err(failed(ID, format!("`{}` is a directory", out.display())));
    }
    if !overwrite && out.exists() {
        return Err(failed(
            ID,
            format!("`{}` exists; pass overwrite to replace it", out.display()),
        ));
    }
    let dir_abs =
        fs::canonicalize(&dir).map_err(|e| failed(ID, format!("`{}`: {e}", dir.display())))?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        if let Ok(parent) = fs::canonicalize(parent) {
            if parent.starts_with(&dir_abs) {
                return Err(failed(
                    ID,
                    format!(
                        "`{}` is inside the directory being packed; write it elsewhere",
                        out.display()
                    ),
                ));
            }
        }
    }

    let mut files = Vec::new();
    let mut skipped = Vec::new();
    let mut empty = Vec::new();
    walk(&dir_abs, &dir_abs, 0, &mut files, &mut skipped, &mut empty).map_err(|e| failed(ID, e))?;
    if kind == Kind::Textures {
        if let Some((name, _)) = files
            .iter()
            .find(|(name, _)| !name.to_ascii_lowercase().ends_with(".dds"))
        {
            return Err(failed(
                ID,
                format!("`{name}` is not a .dds file; a dx10 archive holds only textures"),
            ));
        }
    }
    let options = PackOptions {
        kind,
        version,
        compress,
    };
    let tmp = out.with_extension("ba2.partial");
    let file =
        File::create(&tmp).map_err(|e| failed(ID, format!("creating `{}`: {e}", tmp.display())))?;
    let mut writer = BufWriter::new(file);
    let report = kitkraft_ba2::pack(&files, |p| fs::read(p), &options, &mut writer);
    let flushed = writer.flush();
    drop(writer);
    let report = match (report, flushed) {
        (Ok(r), Ok(())) => r,
        (Err(e), _) => {
            let _ = fs::remove_file(&tmp);
            return Err(failed(ID, e.to_string()));
        }
        (_, Err(e)) => {
            let _ = fs::remove_file(&tmp);
            return Err(failed(ID, format!("writing `{}`: {e}", tmp.display())));
        }
    };
    finish(&tmp, &out, overwrite).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        failed(ID, e)
    })?;

    let shown: Vec<Value> = report
        .files
        .iter()
        .take(FILES_SHOWN)
        .map(|f| json!({"name": f.name, "size": f.size, "stored": f.stored}))
        .collect();
    let mut limitations = Vec::new();
    if report.files.len() > FILES_SHOWN {
        limitations.push(format!(
            "`files` names the first {FILES_SHOWN} of {} packed files",
            report.files.len()
        ));
    }
    if !skipped.is_empty() {
        limitations.push(format!(
            "skipped {} entries that are neither files nor directories (symbolic links): {}",
            skipped.len(),
            skipped.join(", ")
        ));
    }
    if !empty.is_empty() {
        limitations.push(format!(
            "skipped {} empty files, as Archive2 does (no official archive holds one): {}",
            empty.len(),
            empty.join(", ")
        ));
    }
    if kind == Kind::Textures {
        limitations.push(
            "DDS headers are not stored: extraction synthesizes them, so a texture whose \
             header differs from the synthesized one extracts with the same pixels and a \
             different header"
                .into(),
        );
    }
    let out_label = file_name(&out);
    let summary = format!(
        "{out_label}: packed {} files ({} bytes) as {} v{version}",
        report.files.len(),
        report.bytes,
        kind.name()
    );
    Ok(Outcome {
        data: json!({
            "dir": dir.display().to_string(),
            "out": out.display().to_string(),
            "format": kind.name(),
            "version": version,
            "compress": compress,
            "packed": report.files.len(),
            "bytes": report.bytes,
            "files": shown,
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
            length: report.bytes,
            record: None,
            subrecord: None,
        }],
    })
}

/// Moves the finished temporary archive to `out`. Without `overwrite` the move never
/// replaces a file (create-new semantics): a hard link fails if `out` appeared meanwhile;
/// where hard links are unsupported, an existence check and a rename stand in for it.
fn finish(tmp: &Path, out: &Path, overwrite: bool) -> Result<(), String> {
    let rename =
        || fs::rename(tmp, out).map_err(|e| format!("renaming to `{}`: {e}", out.display()));
    let exists = || format!("`{}` exists; pass overwrite to replace it", out.display());
    if overwrite {
        return rename();
    }
    match fs::hard_link(tmp, out) {
        Ok(()) => {
            let _ = fs::remove_file(tmp);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(exists()),
        Err(_) if out.exists() => Err(exists()),
        Err(_) => rename(),
    }
}

/// Collects `(archive path, file path)` for every regular file under `dir`, sorted; empty
/// files go to `empty` instead (Archive2 skips them too).
fn walk(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<(String, String)>,
    skipped: &mut Vec<String>,
    empty: &mut Vec<String>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "`{}` is nested more than {MAX_DEPTH} directories deep",
            dir.display()
        ));
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("reading `{}`: {e}", dir.display()))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("reading `{}`: {e}", dir.display()))?;
    entries.sort();
    for path in entries {
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| format!("reading `{}`: {e}", path.display()))?;
        if meta.is_dir() {
            walk(root, &path, depth.saturating_add(1), out, skipped, empty)?;
        } else if meta.is_file() && meta.len() == 0 {
            empty.push(path.display().to_string());
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| format!("`{}` is outside `{}`", path.display(), root.display()))?;
            let name: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push((name.join("/"), path.display().to_string()));
        } else {
            skipped.push(path.display().to_string());
        }
    }
    Ok(())
}

/// One entry for `ba2.list`.
fn entry_json(index: usize, entry: &Entry) -> Value {
    let mut v = json!({
        "index": index,
        "name": entry.name,
        "size": entry.extracted_len(),
        "stored": entry.stored_len(),
        "compressed": entry.is_compressed(),
        "record_offset": entry.record_offset,
        "data_offset": entry.data_offset(),
    });
    if let (Body::Texture(t), Some(obj)) = (&entry.body, v.as_object_mut()) {
        obj.insert(
            "texture".into(),
            json!({
                "width": t.width,
                "height": t.height,
                "mips": t.mip_count,
                "dxgi_format": t.format,
                "cubemap": t.cubemap != 0,
                "chunks": t.chunks.len(),
            }),
        );
    }
    v
}

fn archive_evidence(archive: &Archive, file: &str) -> Vec<Evidence> {
    let header_len = archive.header.len();
    let records_end = archive
        .entries
        .last()
        .map_or(header_len, |e| match &e.body {
            Body::General(_) => e
                .record_offset
                .saturating_add(kitkraft_ba2::GENERAL_RECORD_LEN),
            Body::Texture(t) => t.chunks.last().map_or(
                e.record_offset
                    .saturating_add(kitkraft_ba2::TEXTURE_RECORD_LEN),
                |c| {
                    c.record_offset
                        .saturating_add(kitkraft_ba2::CHUNK_RECORD_LEN)
                },
            ),
        });
    let mut evidence = vec![
        Evidence {
            file: file.to_string(),
            offset: 0,
            length: header_len,
            record: None,
            subrecord: None,
        },
        Evidence {
            file: file.to_string(),
            offset: header_len,
            length: records_end.saturating_sub(header_len),
            record: None,
            subrecord: None,
        },
    ];
    if archive.has_names {
        evidence.push(Evidence {
            file: file.to_string(),
            offset: archive.header.name_table_offset,
            length: archive
                .file_len
                .saturating_sub(archive.header.name_table_offset),
            record: None,
            subrecord: None,
        });
    }
    evidence
}

/// A case-insensitive regex from an optional argument; a bad pattern is `invalid_args`.
fn pattern(command: &str, args: &Value, name: &str) -> Result<Option<Regex>, EngineError> {
    let Some(text) = args.get(name).and_then(Value::as_str) else {
        return Ok(None);
    };
    RegexBuilder::new(text)
        .case_insensitive(true)
        .size_limit(1 << 20)
        .build()
        .map(Some)
        .map_err(|e| EngineError::InvalidArgs {
            command: command.to_string(),
            violations: vec![Violation {
                pointer: format!("/{name}"),
                rule: "pattern",
                message: format!("not a valid regular expression: {e}"),
            }],
        })
}

/// Refuses a write target inside a game `Data` directory: `KITKRAFT_DATA_DIR`, or any
/// directory at or above the target that holds `Starfield.esm`.
pub(super) fn refuse_game_data(command: &str, target: &Path) -> Result<(), EngineError> {
    let abs = absolute(target);
    let data_dir =
        crate::handlers::env_var_os("KITKRAFT_DATA_DIR").map(|d| absolute(Path::new(&d)));
    let refuse = |why: String| {
        failed(
            command,
            format!(
                "refusing to write `{}`: {why}; the game Data directory is read-only to KitKraft",
                target.display()
            ),
        )
    };
    if let Some(data) = data_dir.filter(|d| !d.as_os_str().is_empty()) {
        if abs.starts_with(&data) {
            return Err(refuse(format!(
                "it is under KITKRAFT_DATA_DIR `{}`",
                data.display()
            )));
        }
    }
    for dir in abs.ancestors() {
        if dir.join("Starfield.esm").is_file() {
            return Err(refuse(format!(
                "`{}` holds Starfield.esm (a game Data directory)",
                dir.display()
            )));
        }
        if is_user_data_dir(dir) {
            return Err(refuse(format!(
                "`{}` is the game's user Data directory (Documents\\My Games\\Starfield\\Data)",
                dir.display()
            )));
        }
    }
    Ok(())
}

/// The per-user Data folder the game also loads from: a directory named `Data` whose parent
/// is `My Games\Starfield`, or whose parent holds `StarfieldCustom.ini` or
/// `StarfieldPrefs.ini` (names compared ASCII case-insensitively, as Windows does).
fn is_user_data_dir(dir: &Path) -> bool {
    let named = |p: &Path, want: &str| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(want))
    };
    if !named(dir, "Data") {
        return false;
    }
    let Some(parent) = dir.parent() else {
        return false;
    };
    let under_my_games =
        named(parent, "Starfield") && parent.parent().is_some_and(|g| named(g, "My Games"));
    under_my_games
        || ["StarfieldCustom.ini", "StarfieldPrefs.ini"]
            .iter()
            .any(|ini| parent.join(ini).is_file())
}

/// `path` made absolute and canonical as far as it exists (the rest appended), so `..` and
/// symbolic links cannot hide a Data directory.
fn absolute(path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|d| d.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut existing = joined.as_path();
    let mut rest = Vec::new();
    loop {
        if let Ok(canon) = fs::canonicalize(existing) {
            let mut out = canon;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                existing = parent;
            }
            _ => return joined,
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::refuse_game_data;
    use crate::execute;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kitkraft-ba2-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pack_list_extract_through_execute() {
        let root = temp_dir("cmd");
        let src = root.join("src");
        fs::create_dir_all(src.join("Scripts")).unwrap();
        fs::create_dir_all(src.join("strings")).unwrap();
        fs::write(src.join("Scripts").join("Alpha.pex"), vec![7u8; 4000]).unwrap();
        fs::write(src.join("strings").join("mod_en.strings"), b"hello").unwrap();
        fs::write(src.join("empty.txt"), b"").unwrap();
        let out = root.join("Mod - Main.ba2");
        let packed = execute(
            "ba2.pack",
            json!({"dir": src.to_str().unwrap(), "out": out.to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(packed["data"]["packed"], 2);
        // Empty files are skipped, as Archive2 does, and reported.
        assert!(
            packed["limitations"][0]
                .as_str()
                .unwrap()
                .contains("skipped 1 empty files"),
            "{packed}"
        );
        assert_eq!(packed["data"]["format"], "gnrl");
        let bytes = fs::read(&out).unwrap();
        assert_eq!(packed["data"]["bytes"], bytes.len());
        assert!(!out.with_extension("ba2.partial").exists());

        // Deterministic.
        let again = root.join("again.ba2");
        execute(
            "ba2.pack",
            json!({"dir": src.to_str().unwrap(), "out": again.to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(fs::read(&again).unwrap(), bytes);
        // Regression (review round 2): an existing archive is replaced only on request.
        fs::write(&again, b"keep me").unwrap();
        let err = execute(
            "ba2.pack",
            json!({"dir": src.to_str().unwrap(), "out": again.to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("overwrite"), "{err}");
        assert_eq!(fs::read(&again).unwrap(), b"keep me");
        execute(
            "ba2.pack",
            json!({"dir": src.to_str().unwrap(), "out": again.to_str().unwrap(),
                   "overwrite": true}),
        )
        .unwrap();
        assert_eq!(fs::read(&again).unwrap(), bytes);
        assert!(!again.with_extension("ba2.partial").exists());
        // The create-new move refuses a file that appears while packing.
        let tmp = root.join("t.partial");
        fs::write(&tmp, b"new").unwrap();
        let err = super::finish(&tmp, &again, false).unwrap_err();
        assert!(err.contains("exists"), "{err}");
        assert_eq!(fs::read(&again).unwrap(), bytes);
        super::finish(&tmp, &root.join("fresh.ba2"), false).unwrap();
        assert!(!tmp.exists());
        assert_eq!(fs::read(root.join("fresh.ba2")).unwrap(), b"new");

        let listed = execute(
            "ba2.list",
            json!({"archive": out.to_str().unwrap(), "filter": "PEX$"}),
        )
        .unwrap();
        assert_eq!(listed["data"]["matched"], 1);
        assert_eq!(listed["data"]["entries"][0]["name"], "scripts/alpha.pex");
        assert_eq!(listed["data"]["entries"][0]["size"], 4000);
        assert_eq!(listed["data"]["entries"][0]["compressed"], true);
        assert_eq!(listed["coverage"]["status"], "complete");
        assert_eq!(listed["evidence"][0]["offset"], 0);

        let paged = execute(
            "ba2.list",
            json!({"archive": out.to_str().unwrap(), "limit": 1, "start": 1}),
        )
        .unwrap();
        assert_eq!(paged["data"]["shown"], 1);
        assert_eq!(paged["data"]["entries"][0]["index"], 1);
        assert!(paged["limitations"][1]
            .as_str()
            .unwrap()
            .contains("showing 1 of 2"));

        let dst = root.join("dst");
        // Above the per-entry cap: refused before anything is written.
        let err = execute(
            "ba2.extract",
            json!({"archive": out.to_str().unwrap(), "out_dir": dst.to_str().unwrap(),
                   "max_entry_bytes": 100}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("max_entry_bytes"), "{err}");
        assert!(!dst.exists());
        let extracted = execute(
            "ba2.extract",
            json!({"archive": out.to_str().unwrap(), "out_dir": dst.to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(extracted["data"]["extracted"], 2);
        assert_eq!(
            fs::read(dst.join("scripts").join("alpha.pex")).unwrap(),
            vec![7u8; 4000]
        );
        assert_eq!(
            fs::read(dst.join("strings").join("mod_en.strings")).unwrap(),
            b"hello"
        );

        // A second extraction refuses to overwrite unless asked.
        let err = execute(
            "ba2.extract",
            json!({"archive": out.to_str().unwrap(), "out_dir": dst.to_str().unwrap()}),
        )
        .unwrap_err();
        assert_eq!(err.kind(), "failed");
        let again = execute(
            "ba2.extract",
            json!({"archive": out.to_str().unwrap(), "out_dir": dst.to_str().unwrap(),
                   "pattern": "strings", "overwrite": true}),
        )
        .unwrap();
        assert_eq!(again["data"]["extracted"], 1);

        let bad = execute(
            "ba2.list",
            json!({"archive": out.to_str().unwrap(), "filter": "("}),
        )
        .unwrap_err();
        assert_eq!(bad.kind(), "invalid_args");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn dx10_pack_needs_textures_and_out_outside_dir() {
        let root = temp_dir("dx10");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("readme.txt"), b"x").unwrap();
        let err = execute(
            "ba2.pack",
            json!({"dir": src.to_str().unwrap(), "out": root.join("t.ba2").to_str().unwrap(),
                   "format": "dx10"}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("not a .dds"), "{err}");
        let err = execute(
            "ba2.pack",
            json!({"dir": src.to_str().unwrap(), "out": src.join("self.ba2").to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("inside the directory"), "{err}");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn game_data_is_refused() {
        let root = temp_dir("data");
        let data = root.join("Starfield").join("Data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("Starfield.esm"), b"TES4").unwrap();
        for target in [
            data.join("out"),
            data.join("Scripts").join("x").join("..").join("y"),
            data.clone(),
        ] {
            let err = refuse_game_data("ba2.extract", &target).unwrap_err();
            assert!(err.to_string().contains("read-only"), "{err}");
        }
        refuse_game_data("ba2.extract", &root.join("elsewhere")).unwrap();
        let archive = root.join("a.ba2");
        fs::write(&archive, b"BTDX").unwrap();
        let err = execute(
            "ba2.extract",
            json!({"archive": archive.to_str().unwrap(), "out_dir": data.join("x").to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("Starfield.esm"), "{err}");
        let err = execute("ba2.list", json!({"archive": archive.to_str().unwrap()})).unwrap_err();
        assert!(err.to_string().contains("truncated"), "{err}");
        fs::remove_dir_all(&root).unwrap();
    }

    /// A stored GNRL archive holding `names` (each file is `x`), its name table patched by
    /// `renames` (same-length byte replacements, for names `pack` refuses to write).
    fn archive_of(path: &std::path::Path, names: &[&str], renames: &[(&str, &str)]) {
        let inputs: Vec<(String, String)> = names
            .iter()
            .map(|n| (n.to_string(), n.to_string()))
            .collect();
        let mut out = std::io::Cursor::new(Vec::new());
        let options = kitkraft_ba2::PackOptions {
            compress: false,
            ..Default::default()
        };
        kitkraft_ba2::pack(&inputs, |_| Ok(b"x".to_vec()), &options, &mut out).unwrap();
        let mut bytes = out.into_inner();
        for (from, to) in renames {
            assert_eq!(from.len(), to.len());
            let at = bytes
                .windows(from.len())
                .position(|w| w == from.as_bytes())
                .unwrap();
            bytes[at..at + to.len()].copy_from_slice(to.as_bytes());
        }
        fs::write(path, bytes).unwrap();
    }

    /// Regression (review round 2): names that collide or that Windows would alter are
    /// refused while planning, before the first file is written.
    #[test]
    fn extraction_plan_is_validated_before_writing() {
        let root = temp_dir("plan");
        let a = root.join("a.ba2");
        let dst = root.join("dst");
        let extract = |dst: &std::path::Path| {
            execute(
                "ba2.extract",
                json!({"archive": a.to_str().unwrap(), "out_dir": dst.to_str().unwrap()}),
            )
        };
        // (names packed, renames in the name table, expected error text)
        type Case<'a> = (&'a [&'a str], &'a [(&'a str, &'a str)], &'a str);
        let cases: [Case; 5] = [
            (
                &["a/aa.txt", "a/ab.txt", "b.txt"],
                &[("a/ab.txt", "A/AA.TXT")],
                "same file",
            ),
            (&["x", "x/y.txt"], &[], "as a directory"),
            (&["a.txt", "bux.txt"], &[("bux.txt", "aux.txt")], "device"),
            (
                &["a.txt", "zz/b.txt"],
                &[("zz/b.txt", "z./b.txt")],
                "ends in",
            ),
            (&["a.txt", "b.txt"], &[("b.txt", "b?txt")], "forbids"),
        ];
        for (names, renames, want) in cases {
            archive_of(&a, names, renames);
            let err = extract(&dst).unwrap_err();
            assert!(err.to_string().contains(want), "{names:?}: {err}");
            assert!(err.to_string().contains("nothing was written"), "{err}");
            assert!(!dst.exists(), "{names:?}: something was written");
        }
        // A file where an entry needs a directory, and a directory where it needs a file.
        archive_of(&a, &["a.txt", "sub/x.txt"], &[]);
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("sub"), b"in the way").unwrap();
        let err = extract(&dst).unwrap_err();
        assert!(err.to_string().contains("needs a directory"), "{err}");
        assert!(!dst.join("a.txt").exists());
        fs::remove_file(dst.join("sub")).unwrap();
        fs::create_dir_all(dst.join("a.txt")).unwrap();
        let err = execute(
            "ba2.extract",
            json!({"archive": a.to_str().unwrap(), "out_dir": dst.to_str().unwrap(),
                   "overwrite": true}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("existing directory"), "{err}");
        assert!(!dst.join("sub").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    /// Regression (review round 2): the user Data folder under Documents\My Games is a
    /// game Data directory too, with or without Starfield.esm.
    #[test]
    fn user_data_dir_is_refused() {
        let root = temp_dir("userdata");
        let user = root.join("My Games").join("Starfield").join("Data");
        fs::create_dir_all(&user).unwrap();
        for target in [
            user.join("Scripts"),
            user.clone(),
            root.join("my games")
                .join("STARFIELD")
                .join("data")
                .join("x"),
        ] {
            let err = refuse_game_data("ba2.extract", &target).unwrap_err();
            assert!(err.to_string().contains("user Data"), "{err}");
        }
        // Identified by the ini files next to it, wherever it lives.
        let moved = root.join("Docs").join("SF").join("Data");
        fs::create_dir_all(&moved).unwrap();
        refuse_game_data("ba2.extract", &moved.join("x")).unwrap();
        fs::write(root.join("Docs").join("SF").join("StarfieldPrefs.ini"), b"").unwrap();
        assert!(refuse_game_data("ba2.extract", &moved.join("x")).is_err());
        // A Data folder elsewhere, and a sibling of the user Data folder, are fine.
        refuse_game_data("ba2.extract", &root.join("Mod").join("Data").join("x")).unwrap();
        refuse_game_data(
            "ba2.extract",
            &root.join("My Games").join("Starfield").join("Saves"),
        )
        .unwrap();
        // The guard runs before the tables are read (they only need to exist).
        let (a, b) = (root.join("a.strings"), root.join("b.strings"));
        fs::write(&a, b"").unwrap();
        fs::write(&b, b"").unwrap();
        let err = execute(
            "strings.merge",
            json!({"tables": [a.to_str().unwrap(), b.to_str().unwrap()],
                   "out": user.join("Strings").join("m.strings").to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("read-only"), "{err}");
        fs::remove_dir_all(&root).unwrap();
    }
}
