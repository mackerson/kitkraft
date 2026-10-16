//! `cargo xtask oracle ck` (M5.A2): our plugin against the Creation Kit's export of the
//! same content, every difference classed (lane A).
//!
//! `cargo xtask oracle ck <fixture-dir>... [--out <dir>] [--check]`
//!
//! A fixture folder holds `kitkraft.esm` (ours), `ck.esm` (the CK's export of the same
//! content) and `fixture.toml` ([`FixtureToml`]; the file names can be overridden there).
//! Per fixture:
//!
//! 1. `kit roundtrip --verify --decode` on both files;
//! 2. a positional byte diff, the `TES4` diff and `kit diff <ours> <ck>` (all entries);
//! 3. every difference becomes one item: a `TES4` header field or subrecord, a record only
//!    in the CK's file, a record only in ours, or a record's flags, subrecord order or one
//!    subrecord; when the bytes differ and neither diff sees anything, one `@bytes` item
//!    (record header stamps, group headers);
//! 4. each item is classed **explained** (a `[[normalization]]` entry of `fixture.toml`
//!    matches it: an expected CK normalization, citing the ledger finding that shows it),
//!    **ours-missing** (a record the CK wrote and we did not), or **unexplained**;
//! 5. every record `match` names is looked up in both files (`kit diff --form`).
//!
//! The report goes to `<out>/<fixture>/report.md` and `report.json` (default `out`:
//! `observe/ck-oracle/`, gitignored), with a one-line verdict in `verdict.txt` that is
//! also printed. `kit`'s index cache lives in `<out>/<fixture>/cache` while the run lasts.
//! Nothing is written into the fixture folder. `--check` exits non-zero when a fixture has
//! an unexplained difference, a failed round trip, or a `match` record missing from
//! either file. Records are matched by FormID (`kit diff`): a record the CK saved under
//! another FormID than ours shows as one ours-missing and one unexplained item.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::evidence::{arg, byte_diff, sha256_hex, table, tes4, tes4_diff, KitBinary, Runner};

const FIXTURE_TOML: &str = "fixture.toml";
const OURS: &str = "kitkraft.esm";
const CK: &str = "ck.esm";
/// Differing byte runs kept per fixture.
const RANGE_CAP: usize = 32;
/// `kit diff --limit`: list every entry.
const DIFF_LIMIT: &str = "1000000";

/// Keys `fixture.toml` may hold.
const TOP_KEYS: [&str; 8] = [
    "fixture",
    "title",
    "ck_version",
    "ours",
    "ck",
    "match",
    "normalization",
    "notes",
];
/// Keys of one `[[normalization]]` entry.
const RULE_KEYS: [&str; 4] = ["record", "subrecord", "reason", "ledger"];

/// `fixture.toml` of a CK oracle fixture.
#[derive(Debug, Default)]
pub(crate) struct FixtureToml {
    /// Matrix id (`F29`); the folder name when absent.
    pub fixture: Option<String>,
    pub title: Option<String>,
    /// CK title bar version.
    pub ck_version: Option<String>,
    /// Our file, default `kitkraft.esm`.
    pub ours: Option<String>,
    /// The CK's file, default `ck.esm`.
    pub ck: Option<String>,
    /// `match`: records that must exist in both files, as `kit diff` labels them
    /// (`Plugin.esm:XXXXXX`; our own forms carry our file's name).
    pub matches: Vec<String>,
    /// Expected CK normalizations.
    pub normalization: Vec<Normalization>,
}

/// One allow-list entry: which differences it explains, and the ledger finding it cites.
#[derive(Debug, Clone)]
pub(crate) struct Normalization {
    /// A record signature (`TES4`, `REFR`), a form label (`Plugin.esm:XXXXXX`), or `*`.
    pub record: String,
    /// A subrecord signature, a `TES4` header field (`@flags`, `@form-version`,
    /// `@timestamp`, `@version-control`, `@unknown`), `@flags`, `@order`, `@record` (a
    /// record in one file only), `@bytes`, or `*` (the default).
    pub subrecord: Option<String>,
    /// Why the CK writes it so.
    pub reason: String,
    /// The ledger finding that shows it (`docs/ck-observations.md` entry id).
    pub ledger: String,
}

impl Normalization {
    fn matches(&self, item: &Item) -> bool {
        let record = self.record.trim();
        let record_ok = record == "*"
            || record == item.signature
            || item
                .form
                .as_deref()
                .is_some_and(|f| f.eq_ignore_ascii_case(record));
        let sub = self.subrecord.as_deref().map_or("*", str::trim);
        record_ok && (sub == "*" || sub == item.subrecord)
    }
}

/// The class of one difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    Explained,
    OursMissing,
    Unexplained,
}

impl Class {
    fn as_str(self) -> &'static str {
        match self {
            Class::Explained => "explained",
            Class::OursMissing => "ours-missing",
            Class::Unexplained => "unexplained",
        }
    }
}

/// One difference between ours and the CK's file.
#[derive(Debug, Clone)]
pub(crate) struct Item {
    /// Record signature, `TES4`, or `*` for `@bytes`.
    signature: String,
    /// The form label, when the item is about one record.
    form: Option<String>,
    /// See [`Normalization::subrecord`].
    subrecord: String,
    /// `ck-only`, `ours-only`, `added`, `removed`, `changed`, `differs`.
    change: String,
    detail: String,
    class: Class,
    because: Option<String>,
}

impl Item {
    fn new(signature: &str, form: Option<&str>, subrecord: &str, change: &str) -> Self {
        Self {
            signature: signature.to_string(),
            form: form.map(str::to_string),
            subrecord: subrecord.to_string(),
            change: change.to_string(),
            detail: String::new(),
            class: Class::Unexplained,
            because: None,
        }
    }

    fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = d.into();
        self
    }

    fn to_json(&self) -> Value {
        json!({
            "record": self.signature,
            "form": self.form,
            "subrecord": self.subrecord,
            "change": self.change,
            "detail": self.detail,
            "class": self.class.as_str(),
            "because": self.because,
        })
    }
}

/// `cargo xtask oracle ck ...`.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    let mut check = false;
    let mut out = root.join("observe").join("ck-oracle");
    let mut dirs = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--check" => check = true,
            "--out" => out = PathBuf::from(it.next().context("--out needs a folder")?),
            "-h" | "--help" => {
                println!("cargo xtask oracle ck <fixture-dir>... [--out <dir>] [--check]");
                println!();
                println!("  Each folder: kitkraft.esm (ours), ck.esm (the CK's), fixture.toml.");
                println!("  Reports: <out>/<fixture>/report.md (default observe/ck-oracle).");
                println!(
                    "  The kit binary is KITKRAFT_KIT, else `cargo run --release -p kit-cli`."
                );
                return Ok(());
            }
            flag if flag.starts_with("--") => bail!("oracle ck: unknown flag `{flag}`"),
            dir => dirs.push(PathBuf::from(dir)),
        }
    }
    if dirs.is_empty() {
        bail!("oracle ck: name at least one fixture folder");
    }
    let kit = KitBinary::from_env(root);
    let mut failing = Vec::new();
    for dir in &dirs {
        let dir =
            std::path::absolute(dir).with_context(|| format!("bad path {}", dir.display()))?;
        let report = oracle(&dir, &out, &kit)?;
        let verdict = report
            .get("verdict_line")
            .and_then(Value::as_str)
            .unwrap_or("?");
        println!("{verdict}");
        if report.get("fails_check").and_then(Value::as_bool) == Some(true) {
            failing.push(verdict.to_string());
        }
    }
    if check && !failing.is_empty() {
        bail!(
            "oracle ck --check: {} fixture(s) fail: {}",
            failing.len(),
            failing.join("; ")
        );
    }
    Ok(())
}

/// Reads and checks a fixture's `fixture.toml` (absent: the defaults).
pub(crate) fn read_fixture(dir: &Path) -> Result<FixtureToml> {
    let path = dir.join(FIXTURE_TOML);
    if !path.is_file() {
        return Ok(FixtureToml::default());
    }
    let shown = path.display();
    let text = fs::read_to_string(&path).with_context(|| format!("reading {shown}"))?;
    let t: toml::Table = toml::from_str(&text).with_context(|| format!("parsing {shown}"))?;
    if let Some(k) = t.keys().find(|k| !TOP_KEYS.contains(&k.as_str())) {
        bail!(
            "{shown}: unknown key `{k}` (expected one of {})",
            TOP_KEYS.join(", ")
        );
    }
    let top = |k: &str| opt_str(&t, k).with_context(|| shown.to_string());
    let matches = match t.get("match") {
        None => Vec::new(),
        Some(toml::Value::Array(a)) => a
            .iter()
            .map(|v| v.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .with_context(|| format!("{shown}: `match` must list strings"))?,
        Some(_) => bail!("{shown}: `match` must be an array of form labels"),
    };
    let rules = match t.get("normalization") {
        None => Vec::new(),
        Some(toml::Value::Array(a)) => a.clone(),
        Some(_) => bail!("{shown}: `normalization` must be [[normalization]] tables"),
    };
    let mut normalization = Vec::new();
    for (i, v) in rules.iter().enumerate() {
        let n = i.saturating_add(1);
        let r = v
            .as_table()
            .with_context(|| format!("{shown}: normalization #{n} is not a table"))?;
        if let Some(k) = r.keys().find(|k| !RULE_KEYS.contains(&k.as_str())) {
            bail!("{shown}: normalization #{n}: unknown key `{k}`");
        }
        let get = |k: &str| -> Result<String> {
            Ok(opt_str(r, k)
                .with_context(|| format!("{shown}: normalization #{n}"))?
                .unwrap_or_default()
                .trim()
                .to_string())
        };
        let (record, reason, ledger) = (get("record")?, get("reason")?, get("ledger")?);
        if ledger.is_empty() || reason.is_empty() {
            bail!("{shown}: normalization #{n} needs a reason and the ledger finding it cites");
        }
        if record.is_empty() {
            bail!("{shown}: normalization #{n} needs a record (signature, form or *)");
        }
        let subrecord =
            opt_str(r, "subrecord").with_context(|| format!("{shown}: normalization #{n}"))?;
        normalization.push(Normalization {
            record,
            subrecord,
            reason,
            ledger,
        });
    }
    Ok(FixtureToml {
        fixture: top("fixture")?,
        title: top("title")?,
        ck_version: top("ck_version")?,
        ours: top("ours")?,
        ck: top("ck")?,
        matches,
        normalization,
    })
}

/// The string at `key`, if any; another type is an error.
fn opt_str(t: &toml::Table, key: &str) -> Result<Option<String>> {
    match t.get(key) {
        None => Ok(None),
        Some(toml::Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => bail!("`{key}` must be a string"),
    }
}

/// The `kit` result's `data`, or an error string.
fn data_or_error(out: &crate::evidence::KitOut) -> std::result::Result<Value, String> {
    match out.error() {
        Some(e) => Err(e),
        None => Ok(out.data().clone()),
    }
}

/// The text before `:` of a TES4 diff line, as an item subrecord.
fn tes4_key(line: &str) -> (String, String) {
    let key = line.split(':').next().unwrap_or_default().trim();
    let sub = match key {
        "flags" => "@flags".to_string(),
        "form version" => "@form-version".to_string(),
        "timestamp" => "@timestamp".to_string(),
        "version control" => "@version-control".to_string(),
        "unknown" => "@unknown".to_string(),
        "subrecord order" => "@order".to_string(),
        other => other.split('[').next().unwrap_or(other).to_string(),
    };
    let change = if line.contains(": added (") {
        "added"
    } else if line.contains(": removed (") {
        "removed"
    } else {
        "changed"
    };
    (sub, change.to_string())
}

fn s<'v>(v: &'v Value, key: &str) -> &'v str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The items of one `kit diff <ours> <ck>` result.
pub(crate) fn diff_items(diff: &Value) -> Vec<Item> {
    let mut items = Vec::new();
    let list = |key: &str| diff.get(key).and_then(Value::as_array).cloned();
    for r in list("added").unwrap_or_default() {
        items.push(
            Item::new(
                s(&r, "signature"),
                Some(s(&r, "form")),
                "@record",
                "ck-only",
            )
            .detail(format!(
                "only in the CK's file, at offset {}",
                r.get("offset").unwrap_or(&Value::Null)
            )),
        );
    }
    for r in list("removed").unwrap_or_default() {
        items.push(
            Item::new(
                s(&r, "signature"),
                Some(s(&r, "form")),
                "@record",
                "ours-only",
            )
            .detail(format!(
                "only in ours, at offset {}",
                r.get("offset").unwrap_or(&Value::Null)
            )),
        );
    }
    for c in list("changed").unwrap_or_default() {
        let (sig, form) = (s(&c, "signature"), s(&c, "form"));
        let before = items.len();
        if c.get("flags_differ").and_then(Value::as_bool) == Some(true) {
            items.push(
                Item::new(sig, Some(form), "@flags", "changed").detail(format!(
                    "record flags {} -> {}",
                    c.pointer("/a/flags").and_then(Value::as_str).unwrap_or("?"),
                    c.pointer("/b/flags").and_then(Value::as_str).unwrap_or("?")
                )),
            );
        }
        if c.get("order_differs").and_then(Value::as_bool) == Some(true) {
            items.push(Item::new(sig, Some(form), "@order", "changed").detail("subrecord order"));
        }
        for sub in c
            .get("subrecords")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let paths: Vec<String> = sub
                .get("fields")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|f| {
                    format!(
                        "{}: {} -> {}",
                        s(f, "path"),
                        f.get("before").unwrap_or(&Value::Null),
                        f.get("after").unwrap_or(&Value::Null)
                    )
                })
                .collect();
            let detail = if paths.is_empty() {
                format!(
                    "occurrence {}, first difference at payload byte {}",
                    sub.get("occurrence").unwrap_or(&Value::Null),
                    sub.get("first_difference").unwrap_or(&Value::Null)
                )
            } else {
                paths.join("; ")
            };
            items.push(
                Item::new(sig, Some(form), s(sub, "signature"), s(sub, "change")).detail(detail),
            );
        }
        if items.len() == before {
            items.push(
                Item::new(sig, Some(form), "@record", s(&c, "class")).detail("record differs"),
            );
        }
    }
    items
}

/// Classes every item against the allow-list.
pub(crate) fn classify(items: &mut [Item], rules: &[Normalization]) {
    for item in items {
        if let Some(rule) = rules.iter().find(|r| r.matches(item)) {
            item.class = Class::Explained;
            item.because = Some(format!("{} ({})", rule.reason, rule.ledger));
        } else if item.change == "ck-only" {
            item.class = Class::OursMissing;
        } else {
            item.class = Class::Unexplained;
        }
    }
}

/// One round trip, digested.
fn roundtrip(kit: &dyn Runner, plugin: &Path, cache: &Path) -> Result<Value> {
    let out = kit.kit(
        &[
            "roundtrip".to_string(),
            arg(plugin)?,
            "--verify".into(),
            "--decode".into(),
        ],
        cache,
    )?;
    Ok(match data_or_error(&out) {
        Ok(d) => json!({
            "passed": d.get("passed").and_then(Value::as_bool).unwrap_or(false),
            "records": d.get("records"),
            "groups": d.get("groups"),
            "compressed_records": d.get("compressed_records"),
            "mismatch": d.get("mismatch"),
        }),
        Err(e) => json!({"passed": false, "error": e}),
    })
}

/// Where a `match` record is: `both`, `ck-only`, `ours-only`, `neither`, or an error.
fn locate(kit: &dyn Runner, ours: &Path, ck: &Path, form: &str, cache: &Path) -> Result<String> {
    let out = kit.kit(
        &[
            "diff".to_string(),
            arg(ours)?,
            arg(ck)?,
            "--form".into(),
            form.to_string(),
            "--limit".into(),
            "1".into(),
        ],
        cache,
    )?;
    Ok(match data_or_error(&out) {
        Err(e) => format!("error: {e}"),
        Ok(d) => {
            let n = |k: &str| {
                d.pointer(&format!("/counts/{k}"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
            };
            if n("changed") > 0 || n("unchanged") > 0 {
                "both".to_string()
            } else if n("added") > 0 {
                "ck-only".to_string()
            } else if n("removed") > 0 {
                "ours-only".to_string()
            } else {
                "neither".to_string()
            }
        }
    })
}

/// Runs the oracle on one fixture folder; writes `<out_root>/<fixture>/`.
pub(crate) fn oracle(dir: &Path, out_root: &Path, kit: &dyn Runner) -> Result<Value> {
    if !dir.is_dir() {
        bail!("oracle ck: {} is not a folder", dir.display());
    }
    let fixture = read_fixture(dir)?;
    let folder = dir.file_name().map_or_else(
        || "fixture".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let name = fixture.fixture.clone().unwrap_or_else(|| folder.clone());
    let ours_name = fixture.ours.clone().unwrap_or_else(|| OURS.to_string());
    let ck_name = fixture.ck.clone().unwrap_or_else(|| CK.to_string());
    let (ours, ck) = (dir.join(&ours_name), dir.join(&ck_name));
    for p in [&ours, &ck] {
        if !p.is_file() {
            bail!("oracle ck: {} is missing", p.display());
        }
    }
    let out = out_root.join(&folder);
    fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    let cache = out.join("cache");
    fs::create_dir_all(&cache)?;
    let mut limitations: Vec<String> = Vec::new();
    if !dir.join(FIXTURE_TOML).is_file() {
        limitations.push(format!(
            "no {FIXTURE_TOML}: no normalization is allowed and no record is required"
        ));
    }

    let (ba, bb) = (fs::read(&ours)?, fs::read(&ck)?);
    let bytes = byte_diff(&ba, &bb, RANGE_CAP);
    let rt_ours = roundtrip(kit, &ours, &cache)?;
    let rt_ck = roundtrip(kit, &ck, &cache)?;

    let mut items = Vec::new();
    let mut kit_diff = json!({"skipped": "identical bytes"});
    if !bytes.identical {
        match (tes4(&ba), tes4(&bb)) {
            (Ok(x), Ok(y)) => {
                for line in tes4_diff(&x, &y) {
                    let (sub, change) = tes4_key(&line);
                    items.push(Item::new("TES4", None, &sub, &change).detail(line));
                }
            }
            (x, y) => limitations.push(format!(
                "TES4 not comparable: {} / {}",
                x.err().unwrap_or_default(),
                y.err().unwrap_or_default()
            )),
        }
        let d = kit.kit(
            &[
                "diff".to_string(),
                arg(&ours)?,
                arg(&ck)?,
                "--limit".into(),
                DIFF_LIMIT.into(),
            ],
            &cache,
        )?;
        match data_or_error(&d) {
            Ok(data) => {
                items.extend(diff_items(&data));
                kit_diff = json!({
                    "keyed_by": data.get("keyed_by"),
                    "counts": data.get("counts"),
                    "summary": data.get("summary"),
                    "limitations": d.json.get("limitations"),
                });
            }
            Err(e) => {
                limitations.push(format!("kit diff failed: {e}"));
                kit_diff = json!({"error": e});
            }
        }
        if items.is_empty() {
            items.push(Item::new("*", None, "@bytes", "differs").detail(format!(
                "{} differing byte(s) in {} run(s), first at offset {}; neither the TES4 diff \
                 nor kit diff sees a difference (record header stamps or group headers)",
                bytes.differing_bytes,
                bytes.ranges_total,
                bytes.first_difference.map_or_else(|| "?".to_string(), |o| o.to_string())
            )));
        }
    }
    classify(&mut items, &fixture.normalization);

    let mut matches = Vec::new();
    for form in &fixture.matches {
        let at = locate(kit, &ours, &ck, form, &cache)?;
        matches.push(json!({"form": form, "found": at}));
    }
    let _ = fs::remove_dir_all(&cache);

    let n = |c: Class| items.iter().filter(|i| i.class == c).count();
    let (explained, missing, unexplained) = (
        n(Class::Explained),
        n(Class::OursMissing),
        n(Class::Unexplained),
    );
    let rt_ok = |r: &Value| r.get("passed").and_then(Value::as_bool) == Some(true);
    let match_found = matches.iter().filter(|m| s(m, "found") == "both").count();
    let roundtrips_ok = rt_ok(&rt_ours) && rt_ok(&rt_ck);
    let fails_check = unexplained > 0 || !roundtrips_ok || match_found < matches.len();
    let verdict = if bytes.identical && roundtrips_ok && match_found == matches.len() {
        "identical"
    } else if fails_check {
        "fail"
    } else if missing > 0 {
        "ours-missing"
    } else {
        "clean"
    };
    let pass = |r: &Value| if rt_ok(r) { "pass" } else { "FAIL" };
    let verdict_line = format!(
        "{name}: {verdict} ({unexplained} unexplained, {missing} ours-missing, {explained} \
         explained; roundtrip ours {}, ck {}; match {match_found}/{})",
        pass(&rt_ours),
        pass(&rt_ck),
        matches.len()
    );

    let report = json!({
        "generated_by": "cargo xtask oracle ck (M5.A2)",
        "fixture": name,
        "folder": folder,
        "title": fixture.title,
        "ck_version": fixture.ck_version,
        "ours": {"file": ours_name, "bytes": ba.len(), "sha256": sha256_hex(&ba), "roundtrip": rt_ours},
        "ck": {"file": ck_name, "bytes": bb.len(), "sha256": sha256_hex(&bb), "roundtrip": rt_ck},
        "bytes": bytes.to_json(),
        "kit_diff": kit_diff,
        "items": items.iter().map(Item::to_json).collect::<Vec<_>>(),
        "counts": {"explained": explained, "ours_missing": missing, "unexplained": unexplained},
        "match": matches,
        "normalization": fixture.normalization.iter().map(|r| json!({
            "record": r.record, "subrecord": r.subrecord, "reason": r.reason, "ledger": r.ledger,
        })).collect::<Vec<_>>(),
        "verdict": verdict,
        "verdict_line": verdict_line,
        "fails_check": fails_check,
        "limitations": limitations,
    });
    let mut text = serde_json::to_string_pretty(&report)?;
    text.push('\n');
    fs::write(out.join("report.json"), text)?;
    fs::write(out.join("report.md"), render(&report))?;
    fs::write(out.join("verdict.txt"), format!("{verdict_line}\n"))?;
    Ok(report)
}

fn render(r: &Value) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# CK oracle: {}\n", s(r, "fixture"));
    let _ = writeln!(md, "**Verdict:** {}\n", s(r, "verdict_line"));
    let _ = writeln!(
        md,
        "Generated by `cargo xtask oracle ck` (M5.A2) from the folder `{}`. CK version: {}.\n",
        s(r, "folder"),
        r.get("ck_version")
            .and_then(Value::as_str)
            .unwrap_or("not recorded")
    );
    let file_row = |k: &str| {
        let f = r.get(k).cloned().unwrap_or(Value::Null);
        let rt = f.get("roundtrip").cloned().unwrap_or(Value::Null);
        vec![
            k.to_string(),
            s(&f, "file").to_string(),
            f.get("bytes").map(Value::to_string).unwrap_or_default(),
            s(&f, "sha256").to_string(),
            match rt.get("error").and_then(Value::as_str) {
                Some(e) => format!("error: {e}"),
                None => format!(
                    "{} ({} records)",
                    if rt.get("passed").and_then(Value::as_bool) == Some(true) {
                        "pass"
                    } else {
                        "FAIL"
                    },
                    rt.get("records").map(Value::to_string).unwrap_or_default()
                ),
            },
        ]
    };
    md.push_str("## Files\n\n");
    md.push_str(&table(
        &[
            "Side",
            "File",
            "Bytes",
            "SHA-256",
            "roundtrip --verify --decode",
        ],
        &[file_row("ours"), file_row("ck")],
    ));
    let b = r.get("bytes").cloned().unwrap_or(Value::Null);
    let _ = writeln!(
        md,
        "\nBytes: {}; {} differing over the common length in {} run(s), first at {}.\n",
        if b.get("identical").and_then(Value::as_bool) == Some(true) {
            "identical"
        } else {
            "different"
        },
        b.get("differing_bytes").unwrap_or(&Value::Null),
        b.get("ranges_total").unwrap_or(&Value::Null),
        b.get("first_difference").unwrap_or(&Value::Null)
    );
    if let Some(sum) = r.pointer("/kit_diff/summary").and_then(Value::as_str) {
        let _ = writeln!(md, "`kit diff`: {sum}\n");
    }
    md.push_str("## Differences\n\n");
    let rows: Vec<Vec<String>> = r
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|i| {
            vec![
                s(i, "class").to_string(),
                s(i, "record").to_string(),
                s(i, "form").to_string(),
                s(i, "subrecord").to_string(),
                s(i, "change").to_string(),
                s(i, "detail").to_string(),
                s(i, "because").to_string(),
            ]
        })
        .collect();
    if rows.is_empty() {
        md.push_str("None.\n");
    } else {
        md.push_str(&table(
            &[
                "Class",
                "Record",
                "Form",
                "Subrecord",
                "Change",
                "Detail",
                "Explained by",
            ],
            &rows,
        ));
    }
    md.push_str("\n## Required records (`match`)\n\n");
    let rows: Vec<Vec<String>> = r
        .get("match")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|m| vec![s(m, "form").to_string(), s(m, "found").to_string()])
        .collect();
    if rows.is_empty() {
        md.push_str("None named.\n");
    } else {
        md.push_str(&table(&["Form", "Found in"], &rows));
    }
    let lims: Vec<&str> = r
        .get("limitations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !lims.is_empty() {
        md.push_str("\n## Limitations\n\n");
        for l in lims {
            let _ = writeln!(md, "- {l}");
        }
    }
    md
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::evidence::tests::{sub, tes4_record};
    use crate::evidence::KitOut;
    use crate::observe::tests::{built_kit, Temp};
    use std::cell::RefCell;

    /// A plugin with a HEDR of `count` records and one GLOB `KC_One` = `value`.
    fn plugin(count: u32, value: f32) -> Vec<u8> {
        let mut h = 0.96f32.to_le_bytes().to_vec();
        h.extend_from_slice(&count.to_le_bytes());
        h.extend_from_slice(&0x801u32.to_le_bytes());
        let mut out = tes4_record(0, &sub(b"HEDR", &h));
        let mut body = sub(b"EDID", b"KC_One\0");
        body.extend(sub(b"FLTV", &value.to_le_bytes()));
        let mut rec = b"GLOB".to_vec();
        rec.extend_from_slice(&(body.len() as u32).to_le_bytes());
        rec.extend_from_slice(&[0; 4]);
        rec.extend_from_slice(&0x800u32.to_le_bytes());
        rec.extend_from_slice(&[0; 8]);
        rec.extend(body);
        out.extend_from_slice(b"GRUP");
        out.extend_from_slice(&((rec.len() + 24) as u32).to_le_bytes());
        out.extend_from_slice(b"GLOB");
        out.extend_from_slice(&[0; 12]);
        out.extend(rec);
        out
    }

    /// A fake kit: round trips pass, `diff` answers with `diff`, `diff --form` with
    /// `form_counts`.
    struct Fake {
        diff: Value,
        form_counts: Value,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl Runner for Fake {
        fn kit(&self, args: &[String], _cache: &Path) -> Result<KitOut> {
            self.calls.borrow_mut().push(args.to_vec());
            let data = match args[0].as_str() {
                "roundtrip" => json!({"passed": true, "records": 2}),
                "diff" if args.iter().any(|a| a == "--form") => {
                    json!({"counts": self.form_counts})
                }
                "diff" => self.diff.clone(),
                other => panic!("unexpected kit {other}"),
            };
            Ok(KitOut {
                code: Some(0),
                json: json!({"data": data}),
            })
        }
    }

    fn fake(diff: Value) -> Fake {
        Fake {
            diff,
            form_counts: json!({"added": 0, "removed": 0, "changed": 0, "unchanged": 1}),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn changed_fltv() -> Value {
        json!({
            "keyed_by": "name",
            "counts": {"added": 1, "removed": 0, "changed": 1, "unchanged": 0},
            "summary": "x",
            "added": [{"form": "kitkraft.esm:000801", "signature": "GLOB", "offset": 99}],
            "removed": [],
            "changed": [{
                "form": "kitkraft.esm:000800", "signature": "GLOB", "class": "differs",
                "flags_differ": false, "order_differs": false,
                "subrecords": [{"signature": "FLTV", "occurrence": 0, "change": "changed",
                    "fields": [{"path": "FLTV", "before": 1.0, "after": 5.0}]}],
            }],
        })
    }

    /// Every file under `dir` with its bytes.
    fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut v: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                let b = fs::read(&p).unwrap();
                (p, b)
            })
            .collect();
        v.sort();
        v
    }

    fn fixture(tmp: &Temp, ours: &[u8], ck: &[u8], toml: &str) -> PathBuf {
        let dir = tmp.0.join("F99");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(OURS), ours).unwrap();
        fs::write(dir.join(CK), ck).unwrap();
        fs::write(dir.join(FIXTURE_TOML), toml).unwrap();
        dir
    }

    #[test]
    fn identical_files_need_no_diff() {
        let tmp = Temp::new("ckoracle-identical");
        let p = plugin(1, 1.0);
        let dir = fixture(
            &tmp,
            &p,
            &p,
            "fixture = \"F99\"\nmatch = [\"kitkraft.esm:000800\"]\n",
        );
        let before = snapshot(&dir);
        let k = fake(Value::Null);
        let out = tmp.0.join("out");
        let r = oracle(&dir, &out, &k).unwrap();
        assert_eq!(r["verdict"], "identical", "{r}");
        assert_eq!(r["fails_check"], false);
        assert!(k
            .calls
            .borrow()
            .iter()
            .all(|c| c[0] != "diff" || c.contains(&"--form".to_string())));
        assert_eq!(
            snapshot(&dir),
            before,
            "nothing is written into the fixture folder"
        );
        assert!(out.join("F99").join("report.md").is_file());
        assert!(!out.join("F99").join("cache").exists());
        let line = fs::read_to_string(out.join("F99").join("verdict.txt")).unwrap();
        assert!(line.starts_with("F99: identical (0 unexplained"), "{line}");
    }

    #[test]
    fn allow_listed_differences_are_explained() {
        let tmp = Temp::new("ckoracle-explained");
        let toml = "fixture = \"F99\"\n\
            [[normalization]]\nrecord = \"TES4\"\nsubrecord = \"HEDR\"\n\
            reason = \"the CK counts records itself\"\nledger = \"ck-20990101-01\"\n\
            [[normalization]]\nrecord = \"kitkraft.esm:000800\"\nsubrecord = \"FLTV\"\n\
            reason = \"rounding\"\nledger = \"ck-20990101-02\"\n\
            [[normalization]]\nrecord = \"GLOB\"\nsubrecord = \"@record\"\n\
            reason = \"the CK adds a default\"\nledger = \"ck-20990101-03\"\n";
        let dir = fixture(&tmp, &plugin(1, 1.0), &plugin(2, 5.0), toml);
        let r = oracle(&dir, &tmp.0.join("out"), &fake(changed_fltv())).unwrap();
        assert_eq!(
            r["counts"],
            json!({"explained": 3, "ours_missing": 0, "unexplained": 0}),
            "{r}"
        );
        assert_eq!(r["verdict"], "clean");
        assert_eq!(r["fails_check"], false);
        let hedr = r["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["subrecord"] == "HEDR")
            .unwrap();
        assert_eq!(
            hedr["because"],
            "the CK counts records itself (ck-20990101-01)"
        );
    }

    #[test]
    fn unlisted_differences_are_unexplained_or_ours_missing() {
        let tmp = Temp::new("ckoracle-unexplained");
        let dir = fixture(
            &tmp,
            &plugin(1, 1.0),
            &plugin(2, 5.0),
            "fixture = \"F99\"\n",
        );
        let mut k = fake(changed_fltv());
        k.form_counts = json!({"added": 1, "removed": 0, "changed": 0, "unchanged": 0});
        fs::write(
            dir.join(FIXTURE_TOML),
            "fixture = \"F99\"\nmatch = [\"kitkraft.esm:000801\"]\n",
        )
        .unwrap();
        let r = oracle(&dir, &tmp.0.join("out"), &k).unwrap();
        // TES4 HEDR and GLOB FLTV unexplained, the CK-only GLOB ours-missing.
        assert_eq!(
            r["counts"],
            json!({"explained": 0, "ours_missing": 1, "unexplained": 2}),
            "{r}"
        );
        assert_eq!(r["verdict"], "fail");
        assert_eq!(r["fails_check"], true);
        assert_eq!(r["match"][0]["found"], "ck-only");
        let md = fs::read_to_string(tmp.0.join("out").join("F99").join("report.md")).unwrap();
        assert!(
            md.contains(
                "| unexplained | GLOB | kitkraft.esm:000800 | FLTV | changed | FLTV: 1.0 -> 5.0 |"
            ),
            "{md}"
        );
    }

    #[test]
    fn bytes_only_differences_are_one_item() {
        let tmp = Temp::new("ckoracle-bytes");
        let ours = plugin(1, 1.0);
        let mut ck = ours.clone();
        // A record header stamp (the GLOB's timestamp field) that kit diff does not see.
        let at = ours.len() - 47 + 16;
        ck[at] = 7;
        let toml = "[[normalization]]\nrecord = \"*\"\nsubrecord = \"@bytes\"\n\
            reason = \"record header stamps\"\nledger = \"ck-20990101-04\"\n";
        let dir = fixture(&tmp, &ours, &ck, toml);
        let empty = json!({"counts": {}, "added": [], "removed": [], "changed": []});
        let r = oracle(&dir, &tmp.0.join("out"), &fake(empty)).unwrap();
        assert_eq!(r["items"].as_array().unwrap().len(), 1, "{r}");
        assert_eq!(r["items"][0]["subrecord"], "@bytes");
        assert_eq!(r["verdict"], "clean");
        // The fixture name falls back to the folder.
        assert!(r["verdict_line"]
            .as_str()
            .unwrap()
            .starts_with("F99: clean"));
    }

    #[test]
    fn a_normalization_without_its_ledger_finding_is_refused() {
        let tmp = Temp::new("ckoracle-noledger");
        let toml = "[[normalization]]\nrecord = \"TES4\"\nreason = \"why\"\nledger = \"\"\n";
        let p = plugin(1, 1.0);
        let dir = fixture(&tmp, &p, &p, toml);
        let e = oracle(&dir, &tmp.0.join("out"), &fake(Value::Null)).unwrap_err();
        assert!(format!("{e:#}").contains("ledger"), "{e:#}");
        fs::write(dir.join(FIXTURE_TOML), "unknown_key = 1\n").unwrap();
        assert!(oracle(&dir, &tmp.0.join("out"), &fake(Value::Null)).is_err());
    }

    #[test]
    fn tes4_lines_become_items() {
        assert_eq!(
            tes4_key("HEDR[0]: 12 bytes a -> 12 bytes b"),
            ("HEDR".into(), "changed".into())
        );
        assert_eq!(
            tes4_key("SNAM[0]: added (7 bytes \"x\")"),
            ("SNAM".into(), "added".into())
        );
        assert_eq!(
            tes4_key("timestamp: 0 -> 5"),
            ("@timestamp".into(), "changed".into())
        );
        assert_eq!(
            tes4_key("subrecord order: A B -> B A"),
            ("@order".into(), "changed".into())
        );
    }

    /// Synthetic fixtures built by the real `kit new` (two plugins that differ in known
    /// ways): skipped when no `kit` binary was built next to this test.
    #[test]
    fn synthetic_fixtures_through_the_real_kit() {
        let Some(kit) = built_kit() else {
            eprintln!("skipped: no kit binary next to the test executable (build kit-cli)");
            return;
        };
        let runner = KitBinary::at(kit);
        let tmp = Temp::new("ckoracle-real");
        let dir = tmp.0.join("F98");
        fs::create_dir_all(&dir).unwrap();
        let mut h = 0.96f32.to_le_bytes().to_vec();
        h.extend_from_slice(&0u32.to_le_bytes());
        h.extend_from_slice(&0x800u32.to_le_bytes());
        let base = dir.join("KCBase.esm");
        fs::write(&base, tes4_record(1, &sub(b"HEDR", &h))).unwrap();
        let cache = tmp.0.join("cache");
        let new = |edid: &str, value: &str, into: &Path| {
            let r = runner
                .kit(
                    &[
                        "new".to_string(),
                        "GLOB".into(),
                        edid.into(),
                        arg(into).unwrap(),
                        "--fields".into(),
                        format!("{{\"FLTV\": {value}}}"),
                        "--plugins".into(),
                        arg(&base).unwrap(),
                        "--no-cache".into(),
                    ],
                    &cache,
                )
                .unwrap();
            assert!(r.error().is_none(), "{:?}", r.json);
        };
        // Ours: KC_One 1, KC_Two 2. The "CK's": KC_One 5, KC_Two 2, KC_Three 3.
        new("KC_One", "1.0", &dir.join(OURS));
        new("KC_Two", "2.0", &dir.join(OURS));
        new("KC_One", "5.0", &dir.join(CK));
        new("KC_Two", "2.0", &dir.join(CK));
        new("KC_Three", "3.0", &dir.join(CK));
        let ours_bytes = fs::read(dir.join(OURS)).unwrap();
        let first = tes4(&ours_bytes).unwrap();
        let own = |n: u32| format!("{OURS}:{:06X}", (first.hedr.unwrap().2 & 0xFF_FFFF) - 2 + n);
        fs::write(
            dir.join(FIXTURE_TOML),
            format!(
                "fixture = \"F98\"\nmatch = [\"{}\", \"{}\"]\n\
                 [[normalization]]\nrecord = \"TES4\"\nsubrecord = \"HEDR\"\n\
                 reason = \"record count and next object ID\"\nledger = \"synthetic\"\n",
                own(0),
                own(1)
            ),
        )
        .unwrap();
        let out = tmp.0.join("out");
        let before = snapshot(&dir);
        let r = oracle(&dir, &out, &runner).unwrap();
        assert_eq!(snapshot(&dir), before);
        assert_eq!(r["ours"]["roundtrip"]["passed"], true, "{r}");
        assert_eq!(r["ck"]["roundtrip"]["passed"], true, "{r}");
        assert_eq!(
            r["counts"],
            json!({"explained": 1, "ours_missing": 1, "unexplained": 1}),
            "{r}"
        );
        let fltv = r["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["subrecord"] == "FLTV")
            .unwrap();
        assert_eq!(fltv["form"], own(0), "{r}");
        assert_eq!(
            r["match"],
            json!([{"form": own(0), "found": "both"}, {"form": own(1), "found": "both"}])
        );
        assert_eq!(r["fails_check"], true);
        // Explaining the value change leaves only the record the CK has and we lack.
        let mut toml = fs::read_to_string(dir.join(FIXTURE_TOML)).unwrap();
        toml.push_str(
            "[[normalization]]\nrecord = \"GLOB\"\nsubrecord = \"FLTV\"\n\
             reason = \"value set in the CK\"\nledger = \"synthetic\"\n",
        );
        fs::write(dir.join(FIXTURE_TOML), toml).unwrap();
        let r = oracle(&dir, &out, &runner).unwrap();
        assert_eq!(r["verdict"], "ours-missing", "{r}");
        assert_eq!(r["fails_check"], false);
        // Ours against itself: identical.
        let same = tmp.0.join("F97");
        fs::create_dir_all(&same).unwrap();
        fs::copy(dir.join(OURS), same.join(OURS)).unwrap();
        fs::copy(dir.join(OURS), same.join(CK)).unwrap();
        fs::copy(&base, same.join("KCBase.esm")).unwrap();
        let r = oracle(&same, &out, &runner).unwrap();
        assert_eq!(r["verdict"], "identical", "{r}");
    }
}
