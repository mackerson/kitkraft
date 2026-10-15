//! `papyrus.events` (M4.G2): what the base game's Papyrus sources declare, with
//! `file:line` evidence.
//!
//! The `Scripts/Source` files of `Tools\ContentResources.zip` are read in place, in memory
//! (nothing extracted), and parsed by [`kitkraft_pex::psc`]. The resulting index (script
//! names, parents, flags, and every function, event, property, struct and custom event
//! with its signature and position) is cached as JSON in the KitKraft cache under
//! `papyrus-index\<key>-v<format>.json`, where `<key>` hashes every entry's name, size and
//! CRC-32 from the zip's central directory: an unchanged zip is answered from the cache
//! without reading any file data. The cache holds declarations only, never comments
//! (ADR 0006 Amendment 1); `with_comments` reads the matching files from the user's zip
//! at query time.
//!
//! A query names a script (`script`; with `inherited`, its parents' members too, each
//! tagged with the script that declares it), a member (`name`, or the `event` / `function`
//! shorthands that also set `kind`), a `kind`, and/or a case-insensitive `search` over
//! script and member names. `overrides` names the nearest ancestor that declares a member
//! of the same kind and name (a handler or override, not a declaration);
//! `declarations_only` drops those and remote event handlers.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use kitkraft_pex::psc::{self, Callable, Flag, Script};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::sources::{default_cache_root, ZipSources};
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::handlers::failed;

const ID: &str = "papyrus.events";
/// Bump when [`Index`] or what goes into it changes.
const FORMAT: u32 = 1;
const CACHE_SUBDIR: &str = "papyrus-index";
/// Longest parent chain followed (the corpus is at most a handful deep).
const MAX_CHAIN: usize = 64;
/// The prefix shown in evidence and `file`: the zip's folder for the sources.
const ZIP_FOLDER: &str = "Scripts/Source/";

/// The cached index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Index {
    format: u32,
    key: String,
    /// `.psc` files read.
    pub files: usize,
    pub scripts: Vec<ScriptEntry>,
    /// `path: error` per file that did not parse.
    pub failures: Vec<String>,
    /// Statements the reader did not understand, over all files.
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ScriptEntry {
    pub name: String,
    pub parent: Option<String>,
    pub flags: Vec<String>,
    /// Path below `Scripts/Source`, `/`-separated.
    pub file: String,
    pub line: u32,
    pub offset: u64,
    pub len: u64,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Member {
    /// `event`, `function`, `property`, `struct`, `custom_event`.
    pub kind: String,
    pub name: String,
    pub remote: Option<String>,
    pub state: Option<String>,
    /// Return type (function) or type (property).
    pub ty: Option<String>,
    /// Parameters (function, event) or members (struct).
    pub params: Vec<ParamEntry>,
    pub flags: Vec<String>,
    pub signature: String,
    pub line: u32,
    pub offset: u64,
    pub len: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ParamEntry {
    #[serde(rename = "type")]
    pub ty: String,
    pub name: String,
    pub default: Option<String>,
}

/// `(file, line) -> (doc, comment)`.
type Comments = HashMap<(String, u32), (Option<String>, Option<String>)>;

const KINDS: [&str; 5] = ["event", "function", "property", "struct", "custom_event"];

fn flag_texts(flags: &[Flag]) -> Vec<String> {
    flags.iter().map(ToString::to_string).collect()
}

fn param_entries(params: &[psc::Param]) -> Vec<ParamEntry> {
    params
        .iter()
        .map(|p| ParamEntry {
            ty: p.ty.to_string(),
            name: p.name.clone(),
            default: p.default.clone(),
        })
        .collect()
}

/// `Type name = default, ...`.
fn param_list(params: &[ParamEntry]) -> String {
    params
        .iter()
        .map(|p| match &p.default {
            Some(d) => format!("{} {} = {d}", p.ty, p.name),
            None => format!("{} {}", p.ty, p.name),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn with_flags(head: String, flags: &[String]) -> String {
    if flags.is_empty() {
        head
    } else {
        format!("{head} {}", flags.join(" "))
    }
}

fn callable_member(c: &Callable) -> Member {
    let params = param_entries(&c.params);
    let flags = flag_texts(&c.flags);
    let name = match &c.remote {
        Some(r) => format!("{r}.{}", c.name),
        None => c.name.clone(),
    };
    let head = match (c.kind, &c.return_type) {
        (psc::CallableKind::Event, _) => format!("Event {name}({})", param_list(&params)),
        (psc::CallableKind::Function, Some(t)) => {
            format!("{t} Function {name}({})", param_list(&params))
        }
        (psc::CallableKind::Function, None) => {
            format!("Function {name}({})", param_list(&params))
        }
    };
    Member {
        kind: c.kind.as_str().to_string(),
        name: c.name.clone(),
        remote: c.remote.clone(),
        state: c.state.clone(),
        ty: c.return_type.as_ref().map(ToString::to_string),
        signature: with_flags(head, &flags),
        params,
        flags,
        line: c.span.line,
        offset: c.span.offset,
        len: c.span.len,
    }
}

fn script_entry(file: &str, s: &Script) -> ScriptEntry {
    let mut members: Vec<Member> = Vec::new();
    members.extend(s.events.iter().map(callable_member));
    members.extend(s.functions.iter().map(callable_member));
    for p in &s.properties {
        let flags = flag_texts(&p.flags);
        let head = match &p.default {
            Some(d) => format!("{} Property {} = {d}", p.ty, p.name),
            None => format!("{} Property {}", p.ty, p.name),
        };
        members.push(Member {
            kind: "property".into(),
            name: p.name.clone(),
            remote: None,
            state: None,
            ty: Some(p.ty.to_string()),
            params: Vec::new(),
            signature: with_flags(head, &flags),
            flags,
            line: p.span.line,
            offset: p.span.offset,
            len: p.span.len,
        });
    }
    for st in &s.structs {
        members.push(Member {
            kind: "struct".into(),
            name: st.name.clone(),
            remote: None,
            state: None,
            ty: None,
            params: st
                .members
                .iter()
                .map(|m| ParamEntry {
                    ty: m.ty.to_string(),
                    name: m.name.clone(),
                    default: m.default.clone(),
                })
                .collect(),
            flags: Vec::new(),
            signature: format!("Struct {}", st.name),
            line: st.span.line,
            offset: st.span.offset,
            len: st.span.len,
        });
    }
    for c in &s.custom_events {
        members.push(Member {
            kind: "custom_event".into(),
            name: c.name.clone(),
            remote: None,
            state: None,
            ty: None,
            params: Vec::new(),
            flags: flag_texts(&c.flags),
            signature: format!("CustomEvent {}", c.name),
            line: c.span.line,
            offset: c.span.offset,
            len: c.span.len,
        });
    }
    members.sort_by_key(|m| m.line);
    ScriptEntry {
        name: s.name.clone(),
        parent: s.parent.clone(),
        flags: flag_texts(&s.flags),
        file: file.to_string(),
        line: s.span.line,
        offset: s.span.offset,
        len: s.span.len,
        members,
    }
}

/// Parses every source of an open zip into an index.
fn build(sources: &mut ZipSources) -> Result<Index, String> {
    let files = sources.read_all_psc()?;
    let mut scripts = Vec::with_capacity(files.len());
    let mut failures = Vec::new();
    let mut diagnostics = 0usize;
    for (path, bytes) in &files {
        match psc::parse(bytes) {
            Ok(s) => {
                diagnostics = diagnostics.saturating_add(s.diagnostics.len());
                scripts.push(script_entry(path, &s));
            }
            Err(e) => failures.push(format!("{ZIP_FOLDER}{path}: {e}")),
        }
    }
    scripts.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    Ok(Index {
        format: FORMAT,
        key: format!("{:016x}", sources.key),
        files: files.len(),
        scripts,
        failures,
        diagnostics,
    })
}

/// The index for `zip`: from `cache_root` when present and current, else built (and
/// written there, unless `cache_root` is `None`). Returns whether it came from the cache.
pub(crate) fn index_for(
    zip: &Path,
    cache_root: Option<&Path>,
) -> Result<(Index, bool, ZipSources), String> {
    let mut sources = ZipSources::open(zip)?;
    let key = format!("{:016x}", sources.key);
    let path = cache_root.map(|root| {
        root.join(CACHE_SUBDIR)
            .join(format!("{key}-v{FORMAT}.json"))
    });
    if let Some(p) = &path {
        if let Some(index) = fs::read(p)
            .ok()
            .and_then(|b| serde_json::from_slice::<Index>(&b).ok())
            .filter(|i| i.format == FORMAT && i.key == key)
        {
            return Ok((index, true, sources));
        }
    }
    let index = build(&mut sources)?;
    if let Some(p) = &path {
        // A cache write failure only costs time on the next call.
        let _ = write_atomic(p, &index);
    }
    Ok((index, false, sources))
}

fn write_atomic(path: &Path, index: &Index) -> Result<(), String> {
    let dir = path.parent().ok_or("no cache directory")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(index).map_err(|e| e.to_string())?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

/// The zip to read: the argument, else next to `KITKRAFT_DATA_DIR`, else the Steam
/// install's `Tools` folder.
fn default_zip(args: &Value) -> Result<PathBuf, EngineError> {
    if let Some(z) = args.get("sources_zip").and_then(Value::as_str) {
        let p = PathBuf::from(z);
        return if p.is_file() {
            Ok(p)
        } else {
            Err(failed(ID, format!("sources zip `{z}` does not exist")))
        };
    }
    let mut candidates = Vec::new();
    if let Some(data) = super::super::data_dir(args) {
        if let Some(game) = data.parent() {
            candidates.push(game.join("Tools").join("ContentResources.zip"));
        }
    }
    candidates.push(
        PathBuf::from(super::compile::STEAM_GAME)
            .join("Tools")
            .join("ContentResources.zip"),
    );
    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .ok_or_else(|| {
            failed(
                ID,
                format!(
                    "Tools\\ContentResources.zip not found (looked at {}); pass sources_zip",
                    candidates
                        .iter()
                        .map(|p| format!("`{}`", p.display()))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        })
}

/// The query arguments.
struct Query {
    script: Option<String>,
    name: Option<String>,
    kind: String,
    search: Option<String>,
    inherited: bool,
    declarations_only: bool,
    with_comments: bool,
    start: usize,
    limit: usize,
}

fn query(args: &Value) -> Result<Query, EngineError> {
    let s = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
    let b = |k: &str, d: bool| args.get(k).and_then(Value::as_bool).unwrap_or(d);
    let n = |k: &str, d: usize| {
        args.get(k)
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(d)
    };
    let mut name = s("name");
    let mut kind = s("kind").unwrap_or_else(|| "all".into());
    for shorthand in ["event", "function"] {
        if let Some(v) = s(shorthand) {
            if name.as_ref().is_some_and(|x| !x.eq_ignore_ascii_case(&v)) {
                return Err(failed(
                    ID,
                    format!("`{shorthand}` and `name` name different members"),
                ));
            }
            if kind != "all" && kind != shorthand {
                return Err(failed(
                    ID,
                    format!("`{shorthand}` contradicts kind `{kind}`"),
                ));
            }
            name = Some(v);
            kind = shorthand.to_string();
        }
    }
    Ok(Query {
        script: s("script"),
        name,
        kind,
        search: s("search").map(|x| x.to_ascii_lowercase()),
        inherited: b("inherited", true),
        declarations_only: b("declarations_only", false),
        with_comments: b("with_comments", false),
        start: n("start", 0),
        limit: n("limit", 100).max(1),
    })
}

/// One result row: a member and the script that declares it.
struct Hit<'i> {
    script: &'i ScriptEntry,
    member: &'i Member,
    overrides: Option<&'i str>,
    inherited: bool,
}

/// `papyrus.events`.
pub(crate) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let q = query(args)?;
    let zip = default_zip(args)?;
    let no_cache = args.get("no_cache").and_then(Value::as_bool) == Some(true);
    let cache_root = (!no_cache).then(default_cache_root);
    let (index, cached, mut sources) =
        index_for(&zip, cache_root.as_deref()).map_err(|e| failed(ID, e))?;
    answer(&index, cached, &zip, &mut sources, &q)
}

fn answer(
    index: &Index,
    cached: bool,
    zip: &Path,
    sources: &mut ZipSources,
    q: &Query,
) -> Result<Outcome, EngineError> {
    let by_name: HashMap<String, &ScriptEntry> = index
        .scripts
        .iter()
        .map(|s| (s.name.to_ascii_lowercase(), s))
        .collect();
    let mut limitations =
        vec!["declarations only: function and event bodies are not read".to_string()];

    // The scripts to look in: one script and (inherited) its parents, or all.
    let mut chain: Vec<&ScriptEntry> = Vec::new();
    let mut root: Option<Value> = None;
    if let Some(name) = &q.script {
        let Some(&first) = by_name.get(&name.to_ascii_lowercase()) else {
            let needle = name.to_ascii_lowercase();
            let near: Vec<&str> = index
                .scripts
                .iter()
                .filter(|s| s.name.to_ascii_lowercase().contains(&needle))
                .take(10)
                .map(|s| s.name.as_str())
                .collect();
            return Err(failed(
                ID,
                if near.is_empty() {
                    format!("no script named `{name}` in the sources")
                } else {
                    format!(
                        "no script named `{name}`; names containing it: {}",
                        near.join(", ")
                    )
                },
            ));
        };
        let (c, missing) = parent_chain(first, &by_name);
        if let Some(m) = missing {
            limitations.push(m);
        }
        chain = if q.inherited { c } else { vec![first] };
        root = Some(json!({
            "name": first.name,
            "parent": first.parent,
            "chain": chain_names(first, &by_name),
            "flags": first.flags,
            "file": format!("{ZIP_FOLDER}{}", first.file),
            "line": first.line,
        }));
    }
    let scope: Vec<&ScriptEntry> = if q.script.is_some() {
        chain.clone()
    } else {
        index.scripts.iter().collect()
    };

    let mut hits: Vec<Hit<'_>> = Vec::new();
    for (depth, script) in scope.iter().enumerate() {
        let script_match = q
            .search
            .as_ref()
            .is_some_and(|s| script.name.to_ascii_lowercase().contains(s));
        for m in &script.members {
            if q.kind != "all" && m.kind != q.kind {
                continue;
            }
            if q.name
                .as_ref()
                .is_some_and(|n| !m.name.eq_ignore_ascii_case(n))
            {
                continue;
            }
            if let Some(s) = &q.search {
                if !script_match && !m.name.to_ascii_lowercase().contains(s) {
                    continue;
                }
            }
            let overrides = overridden_in(script, m, &by_name);
            if q.declarations_only && (overrides.is_some() || m.remote.is_some()) {
                continue;
            }
            hits.push(Hit {
                script,
                member: m,
                overrides,
                inherited: q.script.is_some() && depth > 0,
            });
        }
    }

    let total = hits.len();
    let page: Vec<&Hit<'_>> = hits.iter().skip(q.start).take(q.limit).collect();
    let next_start = q.start.checked_add(page.len()).filter(|&n| n < total);

    let mut comments: Comments = HashMap::new();
    if q.with_comments && !page.is_empty() {
        limitations.push(
            "doc and comment are Bethesda's text, read now from your ContentResources.zip: \
             for your own reference, not for redistribution"
                .into(),
        );
        let mut files: Vec<&str> = page.iter().map(|h| h.script.file.as_str()).collect();
        files.sort_unstable();
        files.dedup();
        for file in files {
            let bytes = sources
                .read(file)
                .map_err(|e| failed(ID, e))?
                .ok_or_else(|| failed(ID, format!("`{file}` is no longer in the zip")))?;
            if let Ok(s) = psc::parse(&bytes) {
                collect_comments(file, &s, &mut comments);
            }
        }
    }

    let matches: Vec<Value> = page
        .iter()
        .map(|h| {
            let m = h.member;
            let mut v = json!({
                "kind": m.kind,
                "name": m.name,
                "script": h.script.name,
                "file": format!("{ZIP_FOLDER}{}", h.script.file),
                "line": m.line,
                "signature": m.signature,
                "flags": m.flags,
                "native": m.flags.iter().any(|f| f.eq_ignore_ascii_case("native")),
                "inherited": h.inherited,
                "overrides": h.overrides,
            });
            if let Some(obj) = v.as_object_mut() {
                match m.kind.as_str() {
                    "function" | "event" => {
                        obj.insert("return_type".into(), json!(m.ty));
                        obj.insert("params".into(), json!(m.params));
                        obj.insert("state".into(), json!(m.state));
                        obj.insert("remote".into(), json!(m.remote));
                    }
                    "property" => {
                        obj.insert("type".into(), json!(m.ty));
                    }
                    "struct" => {
                        obj.insert("members".into(), json!(m.params));
                    }
                    _ => {}
                }
                if q.with_comments {
                    let (doc, comment) = comments
                        .get(&(h.script.file.clone(), m.line))
                        .cloned()
                        .unwrap_or((None, None));
                    obj.insert("doc".into(), json!(doc));
                    obj.insert("comment".into(), json!(comment));
                }
            }
            v
        })
        .collect();

    let evidence: Vec<Evidence> = page
        .iter()
        .map(|h| Evidence {
            file: format!("{ZIP_FOLDER}{}", h.script.file),
            offset: h.member.offset,
            length: h.member.len,
            record: None,
            subrecord: None,
        })
        .collect();

    let mut counts = serde_json::Map::new();
    for k in KINDS {
        let n: usize = index
            .scripts
            .iter()
            .map(|s| s.members.iter().filter(|m| m.kind == k).count())
            .sum();
        counts.insert(k.to_string(), json!(n));
    }
    if !index.failures.is_empty() {
        limitations.push(format!(
            "{} of {} sources did not parse and are not in the index; first: {}",
            index.failures.len(),
            index.files,
            index.failures.first().map_or("", String::as_str)
        ));
    }
    if index.diagnostics > 0 {
        limitations.push(format!(
            "{} statements outside bodies were not understood and are missing",
            index.diagnostics
        ));
    }
    if next_start.is_some() {
        limitations.push(format!(
            "listed {} of {total} matches from {}; pass start to page",
            page.len(),
            q.start
        ));
    }
    let lines: Vec<String> = page
        .iter()
        .map(|h| {
            format!(
                "{}{}:{}  [{}] {}",
                ZIP_FOLDER, h.script.file, h.member.line, h.script.name, h.member.signature
            )
        })
        .collect();
    let summary = format!(
        "{total} match(es){}\n{}",
        if total > page.len() {
            format!(", showing {}", page.len())
        } else {
            String::new()
        },
        lines.join("\n")
    );
    Ok(Outcome {
        data: json!({
            "sources_zip": zip.display().to_string(),
            "cached": cached,
            "files": index.files,
            "scripts": index.scripts.len(),
            "counts": counts,
            "failures": index.failures.len(),
            "script": root,
            "query": {
                "script": q.script,
                "name": q.name,
                "kind": q.kind,
                "search": q.search,
                "inherited": q.inherited,
                "declarations_only": q.declarations_only,
            },
            "total": total,
            "start": q.start,
            "limit": q.limit,
            "next_start": next_start,
            "matches": matches,
            "summary": summary.trim_end(),
        }),
        coverage: Coverage {
            status: if index.failures.is_empty() && index.diagnostics == 0 {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: index.failures.clone(),
        },
        limitations,
        evidence,
    })
}

/// `first` and its parents, nearest first; a parent missing from the sources ends the
/// chain with a limitation.
fn parent_chain<'i>(
    first: &'i ScriptEntry,
    by_name: &HashMap<String, &'i ScriptEntry>,
) -> (Vec<&'i ScriptEntry>, Option<String>) {
    let mut chain = vec![first];
    let mut current = first;
    while let Some(parent) = &current.parent {
        if chain.len() >= MAX_CHAIN {
            return (
                chain,
                Some(format!(
                    "parent chain of `{}` cut at {MAX_CHAIN}",
                    first.name
                )),
            );
        }
        let Some(&next) = by_name.get(&parent.to_ascii_lowercase()) else {
            return (
                chain,
                Some(format!(
                    "`{}` extends `{parent}`, which is not in the sources; its members are not listed",
                    current.name
                )),
            );
        };
        if chain.iter().any(|s| std::ptr::eq(*s, next)) {
            return (
                chain,
                Some(format!(
                    "parent chain of `{}` loops at `{}`",
                    first.name, next.name
                )),
            );
        }
        chain.push(next);
        current = next;
    }
    (chain, None)
}

fn chain_names(first: &ScriptEntry, by_name: &HashMap<String, &ScriptEntry>) -> Vec<String> {
    parent_chain(first, by_name)
        .0
        .iter()
        .skip(1)
        .map(|s| s.name.clone())
        .collect()
}

/// The nearest ancestor of `script` declaring a member of `m`'s kind and name (for a
/// remote handler: the remote script and its ancestors).
fn overridden_in<'i>(
    script: &'i ScriptEntry,
    m: &Member,
    by_name: &HashMap<String, &'i ScriptEntry>,
) -> Option<&'i str> {
    if !matches!(m.kind.as_str(), "event" | "function") {
        return None;
    }
    let chain: Vec<&ScriptEntry> = match &m.remote {
        Some(r) => {
            let start = by_name.get(&r.to_ascii_lowercase())?;
            parent_chain(start, by_name).0
        }
        None => parent_chain(script, by_name)
            .0
            .into_iter()
            .skip(1)
            .collect(),
    };
    chain
        .into_iter()
        .find(|s| {
            s.members.iter().any(|x| {
                x.kind == m.kind && x.remote.is_none() && x.name.eq_ignore_ascii_case(&m.name)
            })
        })
        .map(|s| s.name.as_str())
}

/// `(file, line) -> (doc, comment)` for every declaration of `s`.
fn collect_comments(file: &str, s: &Script, out: &mut Comments) {
    let mut put = |line: u32, doc: Option<String>, comment: Option<String>| {
        if doc.is_some() || comment.is_some() {
            out.insert((file.to_string(), line), (doc, comment));
        }
    };
    for c in s.functions.iter().chain(s.events.iter()) {
        put(c.span.line, c.doc.clone(), c.comment.clone());
    }
    for p in &s.properties {
        put(p.span.line, p.doc.clone(), None);
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
mod tests;
