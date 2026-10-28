//! Inventory definitions from the corpus (M4.BX2; the KK-1xx inventory tickets).
//!
//! - `cargo xtask inventory-def <SIG> --name <TypeName> [--ticket KK-NNN] [--evidence
//!   <citation>]... [--force]` writes `crates/schema/defs/<SIG>.toml` from a scan of the 14
//!   official ESMs, in the style of the checked-in inventory definitions (`LAYR.toml`,
//!   `CSTY.toml`, `SFPC.toml`): every top-level signature the corpus shows, in corpus
//!   order and cardinality; EDID the shared zstring, `BFCB` .. `BFCE` the shared
//!   `BfcbBlock`, `KSIZ`/`KWDA` the shared `Keywords`, lane E's table lstrings typed, every
//!   other payload `opaque` and named after its signature; each member's `source` cites
//!   the scan's counts and sizes with today's date. A consecutive run of entries that
//!   repeats (AVMD's LNAM, VNAM, NNAM) becomes one repeated group when every record agrees
//!   on it; anything the corpus leaves open (an order seen both ways, a repeat the
//!   generator cannot group) is written flat with a `# TODO(review): ...` comment and a
//!   warning, never guessed. `--evidence` adds a citation to the record's `evidence` (the
//!   ticket's `xedit:` record range); nothing here reads xEdit.
//! - `cargo xtask inventory-check <SIG>...` compares every count a definition cites from
//!   the scan (records, records with each member, occurrences, per-record maxima, sizes,
//!   the header comment's record count) and each top-level member's `card` with a fresh
//!   scan, and fails on any difference.
//!
//! The scan is `kitkraft_esm::inventory`, the code `tests/inventory_types.rs` prints from,
//! run through `examples/inventory_scan.rs` in a release build (`cargo run --release`), so
//! the numbers are the inventory test's and this crate does not link the container.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

use crate::codegen::{DefSet, DEFS_DIR};

/// The token a component block stands as in the scan's orders
/// (`kitkraft_esm::inventory::BLOCK_TOKEN`).
const BLOCK: &str = "{BFCB..BFCE}";
/// Tokens the generator substitutes for a run it recognised.
const KEYWORDS: &str = "{Keywords}";
const GROUP: &str = "{Group}";

/// What every corpus `source` the generator writes starts with (the schema tests require
/// it of an inventory's members).
const SCAN_SOURCE: &str = "corpus: inventory_types_full (crates/esm/tests/inventory_types.rs)";

/// The EDID evidence of every inventory (LAYR's): the shared EditorID helper of the
/// pinned xEdit file (only when the file already cites xEdit) and the UESP string
/// conventions page.
const EDID_XEDIT: &str =
    "xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L3182-L3182";
const EDID_UESP: &str = "uesp-skyrim: https://en.uesp.net/wiki/Skyrim_Mod:File_Format_Conventions";

/// Lane E's corpus lstring table (`TABLE`: record type, lstring signatures, ...).
const LSTRING_TABLE: &str = "crates/engine/src/edit/lstrings.rs";

/// One top-level signature's counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SigCounts {
    pub(crate) records: u64,
    pub(crate) occurrences: u64,
    pub(crate) max_per_record: u64,
    pub(crate) max_runs: u64,
    pub(crate) min_size: u64,
    pub(crate) max_size: u64,
}

/// A token of a collapsed order: a signature (or a recognised block), `run` when it
/// repeated consecutively (`A+`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tok {
    key: String,
    run: bool,
}

/// One record type's inventory, as the scan wrote it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Scan {
    pub(crate) records: u64,
    pub(crate) in_blocks: Vec<String>,
    /// Each distinct collapsed order and how many records have it.
    orders: Vec<(Vec<Tok>, u64)>,
    pub(crate) sigs: BTreeMap<String, SigCounts>,
}

fn tokens(order: &str) -> Vec<Tok> {
    order
        .split(' ')
        .filter(|t| !t.is_empty())
        .map(|t| match t.strip_suffix('+') {
            Some(k) => Tok {
                key: k.to_string(),
                run: true,
            },
            None => Tok {
                key: t.to_string(),
                run: false,
            },
        })
        .collect()
}

fn get_u64(table: &toml::Table, key: &str) -> Result<u64> {
    table
        .get(key)
        .and_then(toml::Value::as_integer)
        .and_then(|n| u64::try_from(n).ok())
        .with_context(|| format!("inventory scan: no `{key}`"))
}

/// Scans the 14 official ESMs (`KITKRAFT_DATA_DIR`, sizes checked against
/// `corpus/manifest.toml`) for `types`.
pub(crate) fn scan(root: &Path, types: &[String]) -> Result<BTreeMap<String, Scan>> {
    let files = crate::parity::corpus_paths(root)?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    println!(
        "inventory: scanning {} files for {} (release build of kitkraft-esm's inventory_scan)",
        files.len(),
        types.join(", ")
    );
    let output = Command::new(&cargo)
        .args(["run", "--quiet", "--release", "-p", "kitkraft-esm"])
        .args(["--example", "inventory_scan", "--", "--types"])
        .arg(types.join(","))
        .args(&files)
        .current_dir(root)
        .output()
        .with_context(|| format!("could not start `{cargo}`"))?;
    if !output.status.success() {
        bail!(
            "the inventory scan failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let text = String::from_utf8(output.stdout).context("inventory scan output is not UTF-8")?;
    let table: toml::Table = toml::from_str(&text).context("parsing the inventory scan")?;
    let mut out = BTreeMap::new();
    let empty = toml::Table::new();
    let all = table
        .get("type")
        .and_then(toml::Value::as_table)
        .unwrap_or(&empty);
    for (ty, t) in all {
        let t = t
            .as_table()
            .context("inventory scan: a type is not a table")?;
        let mut scan = Scan {
            records: get_u64(t, "records")?,
            ..Scan::default()
        };
        for v in t
            .get("in_blocks")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            scan.in_blocks
                .push(v.as_str().unwrap_or_default().to_string());
        }
        for o in t
            .get("orders")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let o = o
                .as_table()
                .context("inventory scan: an order is not a table")?;
            let order = o.get("order").and_then(toml::Value::as_str).unwrap_or("");
            scan.orders.push((tokens(order), get_u64(o, "records")?));
        }
        for (sig, s) in t
            .get("sig")
            .and_then(toml::Value::as_table)
            .unwrap_or(&empty)
        {
            let s = s
                .as_table()
                .context("inventory scan: a signature is not a table")?;
            scan.sigs.insert(
                sig.clone(),
                SigCounts {
                    records: get_u64(s, "records")?,
                    occurrences: get_u64(s, "occurrences")?,
                    max_per_record: get_u64(s, "max_per_record")?,
                    max_runs: get_u64(s, "max_runs")?,
                    min_size: get_u64(s, "min_size")?,
                    max_size: get_u64(s, "max_size")?,
                },
            );
        }
        out.insert(ty.clone(), scan);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Structure: member order, cardinality, recognised blocks and groups.

/// How a member stands in the record.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Sub(String),
    Block,
    Keywords,
    Group {
        lead: String,
        /// (signature, card) in entry order.
        members: Vec<(String, &'static str)>,
    },
}

#[derive(Debug, Clone)]
struct Item {
    kind: Kind,
    card: &'static str,
    /// `# TODO(review): ...` lines written above the member.
    todos: Vec<String>,
    /// Earlier members it never shares a record with (order unknown from the corpus).
    unordered_with: Vec<String>,
}

/// How a token is named in notes.
fn label(key: &str, lead: Option<&str>) -> String {
    match key {
        BLOCK => "the component blocks".to_string(),
        KEYWORDS => "KSIZ/KWDA".to_string(),
        GROUP => format!("the {} entries", lead.unwrap_or("?")),
        other => other.to_string(),
    }
}

/// Positions of `key` in `seq`.
fn count(seq: &[Tok], key: &str) -> usize {
    seq.iter().filter(|t| t.key == key).count()
}

/// Replaces each `KSIZ KWDA` pair with one [`KEYWORDS`] token when every `KSIZ` is
/// directly followed by its `KWDA` (and each occurs once per record). `Ok(false)`: no
/// keyword list; `Err`: why the pair cannot be the shared group.
fn merge_keywords(orders: &mut [(Vec<Tok>, u64)]) -> Result<bool, String> {
    let any = orders
        .iter()
        .any(|(s, _)| s.iter().any(|t| t.key == "KSIZ" || t.key == "KWDA"));
    if !any {
        return Ok(false);
    }
    for (seq, _) in orders.iter() {
        for (i, t) in seq.iter().enumerate() {
            let next = seq.get(i.saturating_add(1)).map(|t| t.key.as_str());
            let prev = i
                .checked_sub(1)
                .and_then(|p| seq.get(p))
                .map(|t| t.key.as_str());
            let bad = (t.key == "KSIZ" && (t.run || next != Some("KWDA")))
                || (t.key == "KWDA" && (t.run || prev != Some("KSIZ")));
            if bad {
                return Err("KSIZ and KWDA do not always stand as one KSIZ, KWDA pair".into());
            }
        }
        if count(seq, "KSIZ") > 1 {
            return Err("a record has two keyword lists".into());
        }
    }
    for (seq, _) in orders.iter_mut() {
        let mut out = Vec::with_capacity(seq.len());
        for t in seq.drain(..) {
            match t.key.as_str() {
                "KSIZ" => out.push(Tok {
                    key: KEYWORDS.to_string(),
                    run: false,
                }),
                "KWDA" => {}
                _ => out.push(t),
            }
        }
        *seq = out;
    }
    Ok(true)
}

/// [`order_keys`]: the keys in order, the pairs seen both ways, the transitive "before".
type KeyOrder = (
    Vec<String>,
    Vec<(String, String)>,
    BTreeSet<(String, String)>,
);

/// The order of the keys of `seqs`: a topological sort of "a run of `a` comes before a run
/// of `b` in some sequence", ties broken by first appearance in the most common
/// sequences. Returns the order, the pairs seen both ways, and the transitive "before"
/// relation (for the members whose relative order the corpus never shows).
fn order_keys(seqs: &[(Vec<Tok>, u64)]) -> KeyOrder {
    let mut by_weight: Vec<&(Vec<Tok>, u64)> = seqs.iter().collect();
    by_weight.sort_by_key(|s| std::cmp::Reverse(s.1));
    let mut rank: Vec<String> = Vec::new();
    for (seq, _) in &by_weight {
        for t in seq {
            if !rank.contains(&t.key) {
                rank.push(t.key.clone());
            }
        }
    }
    let mut before: BTreeSet<(String, String)> = BTreeSet::new();
    for (seq, _) in seqs {
        for (i, a) in seq.iter().enumerate() {
            for b in seq.iter().skip(i.saturating_add(1)) {
                if a.key != b.key {
                    before.insert((a.key.clone(), b.key.clone()));
                }
            }
        }
    }
    let conflicts: Vec<(String, String)> = before
        .iter()
        .filter(|(a, b)| a < b && before.contains(&(b.clone(), a.clone())))
        .cloned()
        .collect();
    let mut remaining: Vec<String> = rank.clone();
    let mut order = Vec::new();
    while !remaining.is_empty() {
        let free = remaining.iter().position(|k| {
            !remaining.iter().any(|o| {
                o != k
                    && before.contains(&(o.clone(), k.clone()))
                    && !before.contains(&(k.clone(), o.clone()))
            })
        });
        // A cycle with no pair seen both ways: take the first in rank.
        let pick = free.unwrap_or(0);
        order.push(remaining.remove(pick));
    }
    let mut closure = before.clone();
    loop {
        let mut added = Vec::new();
        for (a, b) in &closure {
            for (c, d) in &closure {
                if b == c && a != d && !closure.contains(&(a.clone(), d.clone())) {
                    added.push((a.clone(), d.clone()));
                }
            }
        }
        if added.is_empty() {
            break;
        }
        closure.extend(added);
    }
    (order, conflicts, closure)
}

/// A repeated group the corpus shows: its lead signature, its members, and the orders
/// with each group's span replaced by one [`GROUP`] token.
struct Inferred {
    lead: String,
    members: Vec<(String, &'static str)>,
    tops: Vec<(Vec<Tok>, u64)>,
}

/// Infers one repeated group from the keys that occur in more than one run of a record
/// (`multi`): in each record the span from the first to the last such key, split into
/// entries at its first key (the lead). Fails, saying why, when the records disagree on
/// the lead, an entry holds a key twice, a group key also stands outside the span, a
/// group key's order differs between entries, or a key still repeats outside the group.
fn infer_group(orders: &[(Vec<Tok>, u64)], multi: &BTreeSet<String>) -> Result<Inferred, String> {
    let mut lead: Option<String> = None;
    let mut spans: Vec<Option<(usize, usize)>> = Vec::new();
    for (seq, _) in orders {
        let idx: Vec<usize> = seq
            .iter()
            .enumerate()
            .filter(|(_, t)| multi.contains(&t.key))
            .map(|(i, _)| i)
            .collect();
        let (Some(first), Some(last)) = (idx.first(), idx.last()) else {
            spans.push(None);
            continue;
        };
        let here = seq.get(*first).map(|t| t.key.clone()).unwrap_or_default();
        match &lead {
            None => lead = Some(here),
            Some(l) if *l != here => {
                return Err(format!(
                    "the repeated run starts with {l} in some records and {here} in others"
                ))
            }
            Some(_) => {}
        }
        spans.push(Some((*first, *last)));
    }
    let lead = lead.ok_or("no repeated run")?;
    let mut entries: Vec<(Vec<Tok>, u64)> = Vec::new();
    for ((seq, n), span) in orders.iter().zip(&spans) {
        let Some((first, last)) = span else { continue };
        let inner = seq.get(*first..=*last).unwrap_or_default();
        let mut entry: Vec<Tok> = Vec::new();
        for t in inner {
            if t.key == lead && !entry.is_empty() {
                entries.push((std::mem::take(&mut entry), *n));
            }
            entry.push(t.clone());
        }
        entries.push((entry, *n));
    }
    for (entry, _) in &entries {
        for t in entry {
            if count(entry, &t.key) > 1 {
                return Err(format!("{} occurs twice in one {lead} entry", t.key));
            }
        }
    }
    let group: BTreeSet<String> = entries
        .iter()
        .flat_map(|(e, _)| e.iter().map(|t| t.key.clone()))
        .collect();
    let mut tops = Vec::new();
    for ((seq, n), span) in orders.iter().zip(&spans) {
        let top: Vec<Tok> = match span {
            None => seq.clone(),
            Some((first, last)) => {
                let entries_here = seq
                    .get(*first..=*last)
                    .unwrap_or_default()
                    .iter()
                    .filter(|t| t.key == lead)
                    .count();
                let mut top: Vec<Tok> = seq.get(..*first).unwrap_or_default().to_vec();
                top.push(Tok {
                    key: GROUP.to_string(),
                    run: entries_here > 1,
                });
                top.extend_from_slice(seq.get(last.saturating_add(1)..).unwrap_or_default());
                top
            }
        };
        if let Some(t) = top.iter().find(|t| group.contains(&t.key)) {
            return Err(format!(
                "{} stands both inside and outside the {lead} entries",
                t.key
            ));
        }
        if let Some(t) = top.iter().find(|t| count(&top, &t.key) > 1) {
            return Err(format!("{} repeats outside the {lead} entries", t.key));
        }
        tops.push((top, *n));
    }
    let (keys, conflicts, _) = order_keys(&entries);
    if let Some((a, b)) = conflicts.first() {
        return Err(format!(
            "{a} and {b} stand in both orders inside the entries"
        ));
    }
    let total: u64 = entries.iter().map(|(_, n)| *n).sum();
    let members = keys
        .into_iter()
        .map(|k| {
            let with: u64 = entries
                .iter()
                .filter(|(e, _)| e.iter().any(|t| t.key == k))
                .map(|(_, n)| *n)
                .sum();
            let run = entries
                .iter()
                .any(|(e, _)| e.iter().any(|t| t.key == k && t.run));
            let card = if run {
                "repeated"
            } else if with == total {
                "required"
            } else {
                "optional"
            };
            (k, card)
        })
        .collect();
    Ok(Inferred {
        lead,
        members,
        tops,
    })
}

/// The cardinality the counts of one top-level signature give.
fn sub_card(c: &SigCounts, total: u64) -> &'static str {
    if c.max_per_record > 1 {
        "repeated"
    } else if c.records == total {
        "required"
    } else {
        "optional"
    }
}

/// The members of a record type, in order, from its scan.
fn plan(scan: &Scan) -> (Vec<Item>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut orders = scan.orders.clone();
    if let Err(why) = merge_keywords(&mut orders) {
        warnings.push(format!(
            "KSIZ/KWDA kept as opaque members, not the shared Keywords group: {why}"
        ));
    }
    let multi: BTreeSet<String> = orders
        .iter()
        .flat_map(|(seq, _)| {
            seq.iter()
                .filter(|t| count(seq, &t.key) > 1)
                .map(|t| t.key.clone())
                .collect::<Vec<_>>()
        })
        .collect();
    let mut group: Option<(String, Vec<(String, &'static str)>)> = None;
    let mut flat_repeats: Vec<(String, String)> = Vec::new();
    let tops = if multi.is_empty() {
        orders
    } else {
        match infer_group(&orders, &multi) {
            Ok(g) => {
                warnings.push(format!(
                    "inferred one repeated group from the corpus orders: entries of {} \
                     (review its name and members)",
                    g.members
                        .iter()
                        .map(|(k, c)| format!("{k} {c}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                group = Some((g.lead, g.members));
                g.tops
            }
            Err(why) => {
                for k in &multi {
                    flat_repeats.push((k.clone(), why.clone()));
                }
                warnings.push(format!(
                    "{} occur in separate runs of one record and could not be grouped ({why}); \
                     written flat with TODO(review)",
                    multi.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
                orders
            }
        }
    };
    let (keys, conflicts, closure) = order_keys(&tops);
    let lead = group.as_ref().map(|(l, _)| l.as_str());
    let mut items: Vec<Item> = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        let mut todos = Vec::new();
        let (kind, card) = match k.as_str() {
            BLOCK => {
                if tops.iter().any(|(s, _)| count(s, BLOCK) > 1) {
                    todos.push(
                        "component blocks stand at two places in one record; check the orders"
                            .to_string(),
                    );
                }
                (Kind::Block, "repeated")
            }
            KEYWORDS => {
                let c = scan.sigs.get("KSIZ").cloned().unwrap_or_default();
                (Kind::Keywords, sub_card(&c, scan.records))
            }
            GROUP => {
                let (lead, members) = group.clone().unwrap_or_default();
                todos.push(format!(
                    "group inferred from the corpus orders (each entry starts with {lead}); \
                     check it against the orders the inventory test prints and rename it if \
                     the ticket names it"
                ));
                (Kind::Group { lead, members }, "repeated")
            }
            sig => {
                let c = scan.sigs.get(sig).cloned().unwrap_or_default();
                if let Some((_, why)) = flat_repeats.iter().find(|(s, _)| s == sig) {
                    todos.push(format!(
                        "{sig} occurs in up to {} separate runs of one record (not consecutive), \
                         which a flat member cannot place; the generator could not group it \
                         ({why}). Make it a repeated group (see SFPC.toml) or two members, \
                         from the orders the inventory test prints",
                        c.max_runs
                    ));
                }
                (Kind::Sub(sig.to_string()), sub_card(&c, scan.records))
            }
        };
        for (a, b) in &conflicts {
            let other = if a == k {
                b
            } else if b == k {
                a
            } else {
                continue;
            };
            // A repeat that could not be grouped shows every pair around it both ways;
            // its own TODO covers them.
            let repeated = |s: &String| flat_repeats.iter().any(|(r, _)| r == s);
            if repeated(a) || repeated(b) {
                continue;
            }
            if keys.iter().position(|x| x == other) < Some(i) {
                todos.push(format!(
                    "the corpus shows {} both before and after {}; if records of this type \
                     are unordered set `unordered = true` (README, \"Unordered records\"), else \
                     settle the order",
                    label(k, lead),
                    label(other, lead)
                ));
                warnings.push(format!(
                    "{} and {} occur in both orders",
                    label(k, lead),
                    label(other, lead)
                ));
            }
        }
        let unordered_with = keys
            .iter()
            .take(i)
            .filter(|o| {
                !closure.contains(&((*o).clone(), k.clone()))
                    && !closure.contains(&(k.clone(), (*o).clone()))
            })
            .map(|o| label(o, lead))
            .collect();
        items.push(Item {
            kind,
            card,
            todos,
            unordered_with,
        });
    }
    (items, warnings)
}

// ---------------------------------------------------------------------------------------
// Writing the definition.

/// Width of the comment lines the generator writes.
const WIDTH: usize = 92;

/// Appends `text` as comment lines of at most [`WIDTH`] columns (a longer word stands
/// alone): `first` opens the first line, `rest` the others.
fn comment(out: &mut String, first: &str, rest: &str, text: &str) {
    let mut line = first.to_string();
    let mut empty = true;
    for word in text.split_whitespace() {
        if !empty && line.len().saturating_add(1).saturating_add(word.len()) > WIDTH {
            out.push_str(&line);
            out.push('\n');
            line = rest.to_string();
            empty = true;
        }
        if !empty {
            line.push(' ');
        }
        line.push_str(word);
        empty = false;
    }
    out.push_str(&line);
    out.push('\n');
}

/// A TOML basic string.
fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A signature as a member name (`FNAM` -> `Fnam`, `NAM0` -> `Nam0`).
fn member_name(sig: &str) -> String {
    let mut out = String::new();
    for (i, c) in sig.chars().filter(char::is_ascii_alphanumeric).enumerate() {
        if i == 0 {
            if c.is_ascii_digit() {
                out.push_str("Sig");
            }
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }
    if out.is_empty() {
        out.push_str("Unnamed");
    }
    out
}

fn sizes(c: &SigCounts) -> String {
    if c.min_size == c.max_size {
        format!("{} bytes", c.min_size)
    } else {
        format!("{}..{} bytes", c.min_size, c.max_size)
    }
}

/// `in A of N official SIG records (C occurrences[, up to M per record])`.
fn counts_phrase(c: &SigCounts, total: u64, sig: &str) -> String {
    let per = if c.max_per_record > 1 {
        format!(", up to {} per record", c.max_per_record)
    } else {
        String::new()
    };
    let plural = if c.occurrences == 1 { "" } else { "s" };
    format!(
        "in {} of {total} official {sig} records ({} occurrence{plural}{per})",
        c.records, c.occurrences
    )
}

struct Spec<'a> {
    sig: &'a str,
    name: &'a str,
    ticket: Option<&'a str>,
    evidence: &'a [String],
    date: &'a str,
}

/// The definition file's text and the number of TODO(review) comments in it.
fn render(
    spec: &Spec<'_>,
    scan: &Scan,
    items: &[Item],
    lstrings: &BTreeSet<String>,
) -> (String, usize) {
    let Spec {
        sig,
        name,
        ticket,
        evidence,
        date,
    } = *spec;
    let total = scan.records;
    let src = |phrase: String| q(&format!("{SCAN_SOURCE}, {date}: {phrase}"));
    let tag = ticket.map_or("inventory".to_string(), |t| format!("{t} inventory"));
    let xedit = evidence.iter().any(|e| e.starts_with("xedit:"));
    let has = |k: &Kind| items.iter().any(|i| i.kind == *k);
    let has_edid = has(&Kind::Sub("EDID".into()));
    let has_block = has(&Kind::Block);
    let has_keywords = has(&Kind::Keywords);
    let typed_lstrings: Vec<&String> = lstrings
        .iter()
        .filter(|l| scan.sigs.contains_key(l.as_str()))
        .collect();

    let mut out = String::new();
    let _ = writeln!(
        out,
        "# {sig}: {name}, inventory definition{}.",
        ticket.map_or(String::new(), |t| format!(" ({t})"))
    );
    out.push_str("#\n# Sources (ADR 0004, ADR 0005):\n");
    comment(
        &mut out,
        "#   ",
        "#     ",
        &format!(
            "corpus: the inventory scan of every official {sig} record ({total} in the 14 ESMs, \
             crates/esm/tests/inventory_types.rs, inventory_types_full, {date}): which \
             subrecords occur, in which order and how often, written out by `cargo xtask \
             inventory-def`. Every member is listed because the corpus has it, and every \
             official record decodes with nothing left unplaced and re-encodes byte for byte."
        ),
    );
    for e in evidence {
        let _ = writeln!(out, "#   {e}");
        out.push_str("#     (cited for the record as a whole).\n");
    }
    let mut shared = Vec::new();
    if has_keywords {
        shared.push("Keywords");
    }
    if has_block {
        shared.push("BfcbBlock");
    }
    if has_edid || !shared.is_empty() {
        let mut line =
            "uesp-skyrim / universal scan: the EDID string convention (M2.B2)".to_string();
        if shared.is_empty() {
            line.push('.');
        } else if shared.len() == 1 {
            let _ = write!(
                line,
                "; the shared {} keeps its own provenance.",
                shared.join("")
            );
        } else {
            let _ = write!(
                line,
                "; the shared {} keep their own provenance.",
                shared.join(" and ")
            );
        }
        comment(&mut out, "#   ", "#     ", &line);
    }
    if typed_lstrings.is_empty() {
        out.push_str(
            "#   lane E's corpus lstring table: no subrecord of this type is an lstring (M3.B8).\n",
        );
    } else {
        let list: Vec<&str> = typed_lstrings.iter().map(|s| s.as_str()).collect();
        comment(
            &mut out,
            "#   ",
            "#     ",
            &format!(
                "lane E's corpus lstring table: {} (M3.B8).",
                list.join(", ")
            ),
        );
    }
    let mut typed = Vec::new();
    if has_edid {
        typed.push("EDID");
    }
    if !typed_lstrings.is_empty() {
        typed.push("the lstrings");
    }
    if has_keywords {
        typed.push("the keyword list");
    }
    if has_block {
        typed.push("the component block frame");
    }
    let typed_text = match typed.split_last() {
        None => "every payload is opaque".to_string(),
        Some((last, [])) => format!("{last} is typed; every other payload is opaque"),
        Some((last, rest)) => format!(
            "{} and {last} are typed; every other payload is opaque",
            rest.join(", ")
        ),
    };
    out.push_str("#\n");
    comment(
        &mut out,
        "# ",
        "# ",
        &format!(
            "An inventory: {typed_text} (named after its signature, a placeholder) until a \
             family lane decodes the record (docs/gaps.md)."
        ),
    );
    if !scan.in_blocks.is_empty() {
        comment(
            &mut out,
            "# ",
            "# ",
            &format!(
                "Inside component blocks (kept raw by BfcbBlock): {}.",
                scan.in_blocks.join(" ")
            ),
        );
    }
    let _ = writeln!(out, "signature = {}", q(sig));
    let _ = writeln!(out, "name = {}", q(name));
    let _ = writeln!(
        out,
        "source = {}",
        q(&format!(
            "{SCAN_SOURCE}, {date}: every official {sig} record ({total}) decodes with every \
             subrecord placed and re-encodes identically"
        ))
    );
    if !evidence.is_empty() {
        out.push_str("evidence = [\n");
        for e in evidence {
            let _ = writeln!(out, "  {},", q(e));
        }
        out.push_str("]\n");
    }
    out.push_str("confidence = \"corpus\"\n");
    let mut short: Vec<&str> = Vec::new();
    if has_edid {
        short.push("EDID");
    }
    if !typed_lstrings.is_empty() {
        short.push("lstrings");
    }
    if has_keywords {
        short.push("keywords");
    }
    if has_block {
        short.push("component frames");
    }
    let what = match short.split_last() {
        None => "every payload opaque".to_string(),
        Some((last, [])) => format!("{last} typed, every other payload opaque"),
        Some((last, rest)) => {
            format!(
                "{} and {last} typed, every other payload opaque",
                rest.join(", ")
            )
        }
    };
    let _ = writeln!(
        out,
        "note = {}",
        q(&format!(
            "inventory definition{}: corpus order and cardinality; {what}",
            ticket.map_or(String::new(), |t| format!(" ({t})"))
        ))
    );

    let mut todos = 0usize;
    for item in items {
        out.push('\n');
        for t in &item.todos {
            comment(&mut out, "# TODO(review): ", "#   ", t);
            todos = todos.saturating_add(1);
        }
        let order_note = if item.unordered_with.is_empty() {
            String::new()
        } else {
            format!(
                "; never in one record with {}, so its order relative to {} is not known from \
                 the corpus",
                item.unordered_with.join(", "),
                if item.unordered_with.len() == 1 {
                    "it"
                } else {
                    "them"
                }
            )
        };
        match &item.kind {
            Kind::Block => {
                let c = scan.sigs.get("BFCB").cloned().unwrap_or_default();
                out.push_str("[[member]]\ngroup_ref = \"BfcbBlock\"\n");
                let _ = writeln!(out, "card = {}", q(item.card));
                let _ = writeln!(
                    out,
                    "source = {}",
                    src(format!("{}, {}", counts_phrase(&c, total, sig), sizes(&c)))
                );
                out.push_str("confidence = \"corpus\"\n");
                let _ = writeln!(
                    out,
                    "note = {}",
                    q(&format!(
                        "base form components; the component data stays raw{order_note}"
                    ))
                );
            }
            Kind::Keywords => {
                let c = scan.sigs.get("KSIZ").cloned().unwrap_or_default();
                out.push_str("[[member]]\ngroup_ref = \"Keywords\"\n");
                let _ = writeln!(out, "card = {}", q(item.card));
                let _ = writeln!(
                    out,
                    "source = {}",
                    src(format!("{}, {}", counts_phrase(&c, total, sig), sizes(&c)))
                );
                out.push_str("confidence = \"corpus\"\n");
                if !order_note.is_empty() {
                    let _ = writeln!(
                        out,
                        "note = {}",
                        q(&format!("the keyword list{order_note}"))
                    );
                }
            }
            Kind::Group { lead, members } => {
                let c = scan.sigs.get(lead).cloned().unwrap_or_default();
                let parts: Vec<String> = members
                    .iter()
                    .map(|(k, card)| {
                        let s = scan.sigs.get(k).cloned().unwrap_or_default();
                        let size = sizes(&s);
                        match *card {
                            "required" => format!("{k} ({size})"),
                            other => format!("{k} ({size}, {other})"),
                        }
                    })
                    .collect();
                let _ = writeln!(
                    out,
                    "[[member]]\ngroup = {}",
                    q(&format!("{}Entries", member_name(lead)))
                );
                let _ = writeln!(out, "card = {}", q(item.card));
                let _ = writeln!(
                    out,
                    "source = {}",
                    src(format!(
                        "{}, each entry {}",
                        counts_phrase(&c, total, sig),
                        parts.join(", ")
                    ))
                );
                out.push_str("confidence = \"corpus\"\n");
                let none = total.saturating_sub(c.records);
                let lacking = if none == 0 {
                    String::new()
                } else {
                    format!("; {none} official records have no entries")
                };
                let _ = writeln!(
                    out,
                    "note = {}",
                    q(&format!(
                        "one entry per {lead}, its members in this order, the entries \
                         consecutive{lacking}{order_note}"
                    ))
                );
                for (k, card) in members {
                    let s = scan.sigs.get(k).cloned().unwrap_or_default();
                    out.push('\n');
                    out.push_str("[[member.member]]\n");
                    let _ = writeln!(out, "sig = {}", q(k));
                    let _ = writeln!(out, "name = {}", q(&member_name(k)));
                    out.push_str("type = \"opaque\"\n");
                    let _ = writeln!(out, "card = {}", q(card));
                    let _ = writeln!(
                        out,
                        "source = {}",
                        src(format!("{}, {}", counts_phrase(&s, total, sig), sizes(&s)))
                    );
                    out.push_str("confidence = \"corpus\"\n");
                    let _ = writeln!(
                        out,
                        "note = {}",
                        q(&format!(
                            "payload not decoded ({tag}); observed {}",
                            sizes(&s)
                        ))
                    );
                }
            }
            Kind::Sub(k) => {
                let s = scan.sigs.get(k).cloned().unwrap_or_default();
                let source = src(format!("{}, {}", counts_phrase(&s, total, sig), sizes(&s)));
                out.push_str("[[member]]\n");
                let _ = writeln!(out, "sig = {}", q(k));
                if k == "EDID" {
                    out.push_str("name = \"EditorId\"\ntype = \"zstring\"\n");
                    let _ = writeln!(out, "card = {}", q(item.card));
                    let _ = writeln!(out, "source = {source}");
                    out.push_str("evidence = [\n");
                    if xedit {
                        let _ = writeln!(out, "  {},", q(EDID_XEDIT));
                    }
                    let _ = writeln!(out, "  {},", q(EDID_UESP));
                    out.push_str("]\nconfidence = \"corpus\"\n");
                    let lack = total.saturating_sub(s.records);
                    let mut notes = Vec::new();
                    if lack > 0 {
                        notes.push(format!(
                            "{lack} official record{} lack{} it",
                            if lack == 1 { "" } else { "s" },
                            if lack == 1 { "s" } else { "" }
                        ));
                    }
                    if !order_note.is_empty() {
                        notes.push(order_note.trim_start_matches("; ").to_string());
                    }
                    if !notes.is_empty() {
                        let _ = writeln!(out, "note = {}", q(&notes.join("; ")));
                    }
                } else if lstrings.contains(k) {
                    let n = match k.as_str() {
                        "FULL" => "Name".to_string(),
                        "DESC" => "Description".to_string(),
                        other => member_name(other),
                    };
                    let _ = writeln!(out, "name = {}", q(&n));
                    out.push_str("type = \"lstring\"\n");
                    let _ = writeln!(out, "card = {}", q(item.card));
                    let _ = writeln!(out, "source = {source}");
                    out.push_str("evidence = [\n");
                    let _ = writeln!(
                        out,
                        "  {},",
                        q(&format!(
                            "corpus: lstring_table_matches_the_corpus (crates/engine/tests/lstring_table.rs): lane E's corpus lstring table (edit::lstrings) classes {sig} {k} as an lstring"
                        ))
                    );
                    out.push_str("]\nconfidence = \"corpus\"\n");
                    if !order_note.is_empty() {
                        let _ = writeln!(out, "note = {}", q(order_note.trim_start_matches("; ")));
                    }
                } else {
                    let _ = writeln!(out, "name = {}", q(&member_name(k)));
                    out.push_str("type = \"opaque\"\n");
                    let _ = writeln!(out, "card = {}", q(item.card));
                    let _ = writeln!(out, "source = {source}");
                    out.push_str("confidence = \"corpus\"\n");
                    let _ = writeln!(
                        out,
                        "note = {}",
                        q(&format!(
                            "payload not decoded ({tag}); observed {}{order_note}",
                            sizes(&s)
                        ))
                    );
                }
            }
        }
    }
    (out, todos)
}

/// The lstring signatures lane E's corpus table lists for `sig` (its `TABLE` row, read as
/// text: xtask does not link the engine). `None` when the table or the row is missing.
fn table_lstrings(root: &Path, sig: &str) -> Option<BTreeSet<String>> {
    let text = fs::read_to_string(root.join(LSTRING_TABLE)).ok()?;
    let prefix = format!("(\"{sig}\", \"");
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with(&prefix))?;
    let rest = line.get(prefix.len()..)?;
    let field = rest.split('"').next()?;
    Some(field.split_whitespace().map(str::to_string).collect())
}

fn valid_sig(sig: &str) -> bool {
    sig.len() == 4
        && sig
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// `cargo xtask inventory-def <SIG> --name <TypeName> [--ticket KK-NNN] [--evidence C]...
/// [--force]`.
pub(crate) fn run_def(root: &Path, args: &[String]) -> Result<()> {
    let mut sig: Option<String> = None;
    let mut name: Option<String> = None;
    let mut ticket: Option<String> = None;
    let mut evidence: Vec<String> = Vec::new();
    let mut force = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--name" => name = it.next().cloned(),
            "--ticket" => ticket = it.next().cloned(),
            "--evidence" => evidence.push(it.next().cloned().context("--evidence needs a value")?),
            "--force" => force = true,
            other if sig.is_none() && !other.starts_with('-') => sig = Some(other.to_string()),
            other => bail!("inventory-def: unexpected argument `{other}`"),
        }
    }
    let usage = "usage: cargo xtask inventory-def <SIG> --name <TypeName> [--ticket KK-NNN] \
                 [--evidence <citation>]... [--force]";
    let sig = sig.context(usage)?;
    let name = name.context(usage)?;
    if !valid_sig(&sig) {
        bail!("inventory-def: `{sig}` is not a record signature (four of A-Z, 0-9, _)");
    }
    let pascal = name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric());
    if !pascal {
        bail!("inventory-def: --name `{name}` is not PascalCase");
    }
    let path = root.join(DEFS_DIR).join(format!("{sig}.toml"));
    if path.exists() && !force {
        bail!(
            "inventory-def: {} exists; pass --force to overwrite it",
            path.display()
        );
    }
    let date = crate::observe::today().context("no system date")?;
    let scans = scan(root, std::slice::from_ref(&sig))?;
    let scan = scans
        .get(&sig)
        .filter(|s| s.records > 0)
        .with_context(|| format!("no official {sig} record in the corpus"))?;
    let lstrings = match table_lstrings(root, &sig) {
        Some(l) => l,
        None => {
            println!("inventory-def: warning: {sig} is not in {LSTRING_TABLE}; no member typed as an lstring");
            BTreeSet::new()
        }
    };
    let (items, mut warnings) = plan(scan);
    for l in lstrings
        .iter()
        .filter(|l| scan.sigs.contains_key(l.as_str()))
    {
        warnings.push(format!(
            "{l} typed as an lstring from lane E's table; compare its member with MESG.toml"
        ));
    }
    let spec = Spec {
        sig: &sig,
        name: &name,
        ticket: ticket.as_deref(),
        evidence: &evidence,
        date: &date,
    };
    let (text, todos) = render(&spec, scan, &items, &lstrings);

    // The file must load with the rest of the set (names, provenance, shapes).
    let set = DefSet::on_disk(root)?;
    let file = format!("{sig}.toml");
    let mut files: Vec<(String, String)> =
        set.files.into_iter().filter(|(n, _)| *n != file).collect();
    files.push((file.clone(), text.clone()));
    kitkraft_schema::load_sources(files.iter().map(|(n, t)| (n.as_str(), t.as_str())))
        .map_err(|e| anyhow!("inventory-def: the generated {file} does not load: {e}"))?;

    fs::write(&path, &text).with_context(|| format!("writing {}", path.display()))?;
    println!(
        "inventory-def: wrote {DEFS_DIR}/{file} ({} records, {} members, {todos} TODO(review))",
        scan.records,
        items.len()
    );
    for w in &warnings {
        println!("inventory-def: warning: {sig}: {w}");
    }
    println!(
        "next: review every `TODO(review)` in {DEFS_DIR}/{file} (delete each once settled), then \
         `cargo xtask add-def {sig}`, `KK_INVENTORY_ONLY={sig} cargo test -p kitkraft-esm \
         --release --test inventory_types -- --include-ignored --exact inventory_types_full \
         --nocapture` and `cargo xtask inventory-check {sig}`"
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Checking a definition's cited counts.

/// The counts a `source` cites: `in A of N official SIG records (C occurrences[, up to M
/// per record])`, and the text after it.
#[derive(Debug, PartialEq, Eq)]
struct Cited {
    with: u64,
    total: u64,
    sig: String,
    occurrences: u64,
    per_record: Option<u64>,
    tail: String,
}

fn take_num(s: &str) -> Option<(u64, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let n = s.get(..end)?.parse().ok()?;
    Some((n, s.get(end..)?))
}

fn parse_cited(source: &str) -> Option<Cited> {
    source
        .match_indices("in ")
        .find_map(|(i, _)| parse_cited_at(source.get(i.saturating_add(3)..)?))
}

/// [`parse_cited`] at `rest`, the text right after an `in `.
fn parse_cited_at(rest: &str) -> Option<Cited> {
    let (with, rest) = take_num(rest)?;
    let rest = rest.strip_prefix(" of ")?;
    let (total, rest) = take_num(rest)?;
    let rest = rest.strip_prefix(" official ")?;
    let sig = rest.get(..4)?.to_string();
    let rest = rest.get(4..)?.strip_prefix(" records (")?;
    let (occurrences, rest) = take_num(rest)?;
    let rest = rest.strip_prefix(" occurrence")?;
    let rest = rest.strip_prefix('s').unwrap_or(rest);
    let close = rest.find(')')?;
    let extra = rest.get(..close)?;
    let tail = rest.get(close.saturating_add(1)..)?.to_string();
    let per_record = extra
        .find(" per record")
        .and_then(|p| extra.get(..p))
        .and_then(|before| before.rsplit([' ', '.']).next())
        .and_then(|n| n.parse().ok());
    Some(Cited {
        with,
        total,
        sig,
        occurrences,
        per_record,
        tail,
    })
}

/// `a..b` or `a` before each ` bytes` in `text`, with the signature written right before
/// it as `SIG (` when there is one.
fn cited_sizes(text: &str) -> Vec<(Option<String>, u64, u64)> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices(" bytes") {
        let before = text.get(..i).unwrap_or_default();
        let start = before.rfind([' ', '(']).map_or(0, |p| p.saturating_add(1));
        let word = before.get(start..).unwrap_or_default();
        let (lo, hi) = match word.split_once("..") {
            Some((a, b)) => (a.parse().ok(), b.parse().ok()),
            None => (word.parse().ok(), word.parse().ok()),
        };
        let (Some(lo), Some(hi)) = (lo, hi) else {
            continue;
        };
        let sig = before
            .get(..start)
            .and_then(|b| b.strip_suffix(" ("))
            .and_then(|b| b.get(b.len().saturating_sub(4)..))
            .filter(|s| valid_sig(s))
            .map(str::to_string);
        out.push((sig, lo, hi));
    }
    out
}

/// The signature whose counts a member's `source` cites: its own, the first member's of a
/// group, `BFCB` for the component block, `KSIZ` for the keyword list.
fn lead_sig(member: &toml::Table) -> Option<String> {
    if let Some(s) = member.get("sig").and_then(toml::Value::as_str) {
        return Some(s.to_string());
    }
    match member.get("group_ref").and_then(toml::Value::as_str) {
        Some("BfcbBlock") => return Some("BFCB".into()),
        Some("Keywords") => return Some("KSIZ".into()),
        Some(_) => return None,
        None => {}
    }
    member
        .get("member")
        .and_then(toml::Value::as_array)
        .and_then(|m| m.first())
        .and_then(toml::Value::as_table)
        .and_then(lead_sig)
}

/// Every difference between the counts `text` (a definition of `sig`) cites and `scan`.
fn check_def(sig: &str, text: &str, scan: &Scan) -> Result<(Vec<String>, usize)> {
    let table: toml::Table = toml::from_str(text).context("parsing the definition")?;
    let mut errors = Vec::new();
    let mut checked = 0usize;
    let total = scan.records;
    for line in text.lines().filter(|l| l.starts_with('#')) {
        if let Some(p) = line.find(" in the 14 ESMs") {
            let before = line.get(..p).unwrap_or_default();
            let word = before.rsplit([' ', '(']).next().unwrap_or_default();
            match word.parse::<u64>() {
                Ok(n) if n == total => checked = checked.saturating_add(1),
                Ok(n) => errors.push(format!(
                    "header comment: {n} records in the 14 ESMs, the corpus has {total}"
                )),
                Err(_) => {}
            }
        }
    }
    let file_source = table
        .get("source")
        .and_then(toml::Value::as_str)
        .unwrap_or("");
    if file_source.starts_with(SCAN_SOURCE) {
        let marker = format!("every official {sig} record (");
        match file_source
            .find(&marker)
            .and_then(|p| file_source.get(p.saturating_add(marker.len())..))
            .and_then(take_num)
        {
            Some((n, _)) if n == total => checked = checked.saturating_add(1),
            Some((n, _)) => errors.push(format!(
                "source: cites {n} official {sig} records, the corpus has {total}"
            )),
            None => errors.push(format!(
                "source: no `every official {sig} record (N)` to check"
            )),
        }
    }
    let empty = Vec::new();
    let members = table
        .get("member")
        .and_then(toml::Value::as_array)
        .unwrap_or(&empty);
    let mut positions: BTreeMap<String, usize> = BTreeMap::new();
    count_positions(members, &mut positions);
    let mut check = Check {
        sig,
        scan,
        shared: positions
            .into_iter()
            .filter(|(_, n)| *n > 1)
            .map(|(k, _)| k)
            .collect(),
        shared_sums: BTreeMap::new(),
        errors,
        checked,
    };
    check.members(members, true, "");
    // A signature at two positions (KSSM's RSMC alone and leading each entry): each
    // member cites its own share, so the shares add up to the corpus total.
    for (k, cited) in std::mem::take(&mut check.shared_sums) {
        let corpus = scan.sigs.get(&k).map_or(0, |c| c.occurrences);
        if cited != corpus {
            check.errors.push(format!(
                "{k} stands at more than one member: their cited occurrences add up to \
                 {cited}, the corpus has {corpus}"
            ));
        }
    }
    Ok((check.errors, check.checked))
}

/// Subrecord members per signature, at any depth (a group's lead counts once, as its
/// first member).
fn count_positions(members: &[toml::Value], out: &mut BTreeMap<String, usize>) {
    for m in members.iter().filter_map(toml::Value::as_table) {
        if let Some(s) = m.get("sig").and_then(toml::Value::as_str) {
            let n = out.entry(s.to_string()).or_insert(0);
            *n = n.saturating_add(1);
        }
        if let Some(nested) = m.get("member").and_then(toml::Value::as_array) {
            count_positions(nested, out);
        }
    }
}

/// The state of checking one definition.
struct Check<'a> {
    sig: &'a str,
    scan: &'a Scan,
    /// Signatures that stand at more than one member: their counts are per position, so
    /// only their sum ([`Check::shared_sums`]) and sizes within the corpus range are checked.
    shared: BTreeSet<String>,
    shared_sums: BTreeMap<String, u64>,
    errors: Vec<String>,
    checked: usize,
}

impl Check<'_> {
    fn members(&mut self, members: &[toml::Value], top: bool, path: &str) {
        let (sig, scan) = (self.sig, self.scan);
        let total = scan.records;
        for m in members.iter().filter_map(toml::Value::as_table) {
            let name = m
                .get("name")
                .or_else(|| m.get("group"))
                .or_else(|| m.get("group_ref"))
                .and_then(toml::Value::as_str)
                .unwrap_or("?");
            let here = format!("{path}{name}");
            let lead = lead_sig(m);
            let counts = lead.as_ref().and_then(|l| scan.sigs.get(l));
            if let (Some(l), None) = (&lead, counts) {
                self.errors
                    .push(format!("{here}: no official {sig} record has {l}"));
            }
            if let Some(nested) = m.get("member").and_then(toml::Value::as_array) {
                self.members(nested, false, &format!("{here}."));
            }
            let shared = lead.as_ref().is_some_and(|l| self.shared.contains(l));
            let own_sig = m.get("sig").and_then(toml::Value::as_str);
            if let (true, false, Some(c), Some(own)) = (top, shared, counts, own_sig) {
                let card = m
                    .get("card")
                    .and_then(toml::Value::as_str)
                    .unwrap_or("required");
                let want = sub_card(c, total);
                if card != want {
                    self.errors.push(format!(
                        "{here}: card = \"{card}\", the corpus gives \"{want}\" ({own} in {} of \
                         {total} records, up to {} per record)",
                        c.records, c.max_per_record
                    ));
                }
            }
            let source = m.get("source").and_then(toml::Value::as_str).unwrap_or("");
            if !source.starts_with(SCAN_SOURCE) {
                continue;
            }
            let (Some(lead), Some(c)) = (lead, counts) else {
                continue;
            };
            let Some(cited) = parse_cited(source) else {
                self.errors.push(format!(
                    "{here}: source does not cite `in A of N official {sig} records (C \
                     occurrences)`"
                ));
                continue;
            };
            self.checked = self.checked.saturating_add(1);
            let mut diff = Vec::new();
            if cited.sig != sig {
                diff.push(format!("names {} records, not {sig}", cited.sig));
            }
            if cited.total != total {
                diff.push(format!(
                    "{} records cited, the corpus has {total}",
                    cited.total
                ));
            }
            if shared {
                if own_sig.is_some() {
                    let sum = self.shared_sums.entry(lead.clone()).or_insert(0);
                    *sum = sum.saturating_add(cited.occurrences);
                }
                if cited.with > c.records || cited.occurrences > c.occurrences {
                    diff.push(format!(
                        "{lead} in {} records ({} occurrences) cited, more than the corpus has \
                         ({}, {})",
                        cited.with, cited.occurrences, c.records, c.occurrences
                    ));
                }
            } else {
                if cited.with != c.records {
                    diff.push(format!(
                        "{lead} in {} records cited, the corpus: {}",
                        cited.with, c.records
                    ));
                }
                if cited.occurrences != c.occurrences {
                    diff.push(format!(
                        "{} {lead} occurrences cited, the corpus: {}",
                        cited.occurrences, c.occurrences
                    ));
                }
                if let Some(p) = cited.per_record {
                    if p != c.max_per_record {
                        diff.push(format!(
                            "up to {p} per record cited, the corpus: {}",
                            c.max_per_record
                        ));
                    }
                }
            }
            let own = m.contains_key("sig") || m.contains_key("group_ref");
            for (named, lo, hi) in cited_sizes(&cited.tail) {
                let target = match named {
                    Some(s) => scan.sigs.get(&s).map(|x| (s.clone(), x)),
                    None if own => Some((lead.clone(), c)),
                    None => None,
                };
                if let Some((s, x)) = target {
                    let fits = if self.shared.contains(&s) {
                        lo >= x.min_size && hi <= x.max_size
                    } else {
                        (lo, hi) == (x.min_size, x.max_size)
                    };
                    if !fits {
                        diff.push(format!(
                            "{s} sizes {lo}..{hi} cited, the corpus: {}..{}",
                            x.min_size, x.max_size
                        ));
                    }
                }
                if own {
                    // A member cites its own size once.
                    break;
                }
            }
            if !diff.is_empty() {
                self.errors.push(format!("{here}: {}", diff.join("; ")));
            }
        }
    }
}

/// `cargo xtask inventory-check <SIG>...`.
pub(crate) fn run_check(root: &Path, args: &[String]) -> Result<()> {
    if args.is_empty() || args.iter().any(|a| !valid_sig(a)) {
        bail!("usage: cargo xtask inventory-check <SIG>...");
    }
    let scans = scan(root, args)?;
    let mut failed = 0usize;
    for sig in args {
        let path = root.join(DEFS_DIR).join(format!("{sig}.toml"));
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let empty = Scan::default();
        let scan = scans.get(sig).unwrap_or(&empty);
        let (errors, checked) =
            check_def(sig, &text, scan).with_context(|| format!("{}", path.display()))?;
        if errors.is_empty() {
            println!(
                "inventory-check: {sig} ok ({} records; {checked} cited counts match the corpus)",
                scan.records
            );
        } else {
            failed = failed.saturating_add(1);
            for e in &errors {
                println!("inventory-check: {sig}: {e}");
            }
        }
    }
    if failed > 0 {
        bail!("{failed} definition(s) cite counts the corpus does not show");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn scan_of(orders: &[(&str, u64)], sigs: &[(&str, u64, u64, u64, u64, u64, u64)]) -> Scan {
        Scan {
            records: orders.iter().map(|(_, n)| n).sum(),
            in_blocks: Vec::new(),
            orders: orders.iter().map(|(o, n)| (tokens(o), *n)).collect(),
            sigs: sigs
                .iter()
                .map(
                    |&(s, records, occurrences, max_per_record, max_runs, min, max)| {
                        (
                            s.to_string(),
                            SigCounts {
                                records,
                                occurrences,
                                max_per_record,
                                max_runs,
                                min_size: min,
                                max_size: max,
                            },
                        )
                    },
                )
                .collect(),
        }
    }

    fn kinds(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|i| match &i.kind {
                Kind::Sub(s) => format!("{s}:{}", i.card),
                Kind::Block => format!("Bfcb:{}", i.card),
                Kind::Keywords => format!("Keywords:{}", i.card),
                Kind::Group { lead, members } => format!(
                    "({lead}: {})",
                    members
                        .iter()
                        .map(|(k, c)| format!("{k}:{c}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            })
            .collect()
    }

    #[test]
    fn flat_members_follow_the_corpus_order() {
        let scan = scan_of(
            &[
                ("EDID OBND XALG DATA", 3),
                ("EDID OBND DATA", 5),
                ("EDID {BFCB..BFCE}+ KSIZ KWDA DATA+", 1),
            ],
            &[
                ("EDID", 9, 9, 1, 1, 3, 40),
                ("OBND", 8, 8, 1, 1, 24, 24),
                ("XALG", 3, 3, 1, 1, 8, 8),
                ("DATA", 9, 10, 2, 1, 4, 4),
                ("BFCB", 1, 2, 2, 1, 32, 36),
                ("BFCE", 1, 2, 2, 1, 0, 0),
                ("KSIZ", 1, 1, 1, 1, 4, 4),
                ("KWDA", 1, 1, 1, 1, 4, 8),
            ],
        );
        let (items, warnings) = plan(&scan);
        assert_eq!(
            kinds(&items),
            [
                "EDID:required",
                "OBND:optional",
                "XALG:optional",
                "Bfcb:repeated",
                "Keywords:optional",
                "DATA:repeated"
            ]
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        // OBND and the component blocks never share a record.
        assert_eq!(items[3].unordered_with, ["OBND", "XALG"]);
    }

    #[test]
    fn a_repeated_run_becomes_one_group() {
        let scan = scan_of(
            &[
                ("EDID MNAM LNAM VNAM NNAM LNAM NNAM MODT", 4),
                ("EDID MNAM LNAM NNAM MODT", 2),
                ("EDID MNAM MODT", 1),
            ],
            &[
                ("EDID", 7, 7, 1, 1, 3, 40),
                ("MNAM", 7, 7, 1, 1, 4, 4),
                ("LNAM", 6, 10, 2, 2, 4, 91),
                ("VNAM", 4, 4, 1, 1, 19, 127),
                ("NNAM", 6, 10, 2, 2, 4, 4),
                ("MODT", 7, 7, 1, 1, 4, 4),
            ],
        );
        let (items, _) = plan(&scan);
        assert_eq!(
            kinds(&items),
            [
                "EDID:required",
                "MNAM:required",
                "(LNAM: LNAM:required VNAM:optional NNAM:required)",
                "MODT:required"
            ]
        );
        let (text, todos) = render(
            &Spec {
                sig: "AVMD",
                name: "AvmsData",
                ticket: Some("KK-022"),
                evidence: &[],
                date: "2026-10-28",
            },
            &scan,
            &items,
            &BTreeSet::new(),
        );
        assert_eq!(todos, 1);
        assert!(text.contains("group = \"LnamEntries\""), "{text}");
        assert!(text.contains(
            "in 6 of 7 official AVMD records (10 occurrences, up to 2 per record), each entry \
             LNAM (4..91 bytes), VNAM (19..127 bytes, optional), NNAM (4 bytes)"
        ));
        // What the generator writes, the check reads back without a difference.
        let (errors, checked) = check_def("AVMD", &text, &scan).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(checked, 2 + 1 + 1 + 1 + 3 + 1);
    }

    #[test]
    fn an_ungroupable_repeat_is_flagged_not_guessed() {
        let scan = scan_of(
            &[("EDID ANAM BNAM CNAM ANAM CNAM BNAM", 2)],
            &[
                ("EDID", 2, 2, 1, 1, 3, 4),
                ("ANAM", 2, 4, 2, 2, 4, 4),
                ("BNAM", 2, 4, 2, 2, 4, 4),
                ("CNAM", 2, 2, 1, 1, 4, 4),
            ],
        );
        let (items, warnings) = plan(&scan);
        assert!(
            warnings.iter().any(|w| w.contains("could not be grouped")),
            "{warnings:?}"
        );
        assert!(items.iter().any(|i| !i.todos.is_empty()));
    }

    #[test]
    fn both_orders_are_flagged() {
        let scan = scan_of(
            &[("EDID ANAM BNAM", 2), ("EDID BNAM ANAM", 1)],
            &[
                ("EDID", 3, 3, 1, 1, 3, 4),
                ("ANAM", 3, 3, 1, 1, 4, 4),
                ("BNAM", 3, 3, 1, 1, 4, 4),
            ],
        );
        let (items, warnings) = plan(&scan);
        assert_eq!(
            kinds(&items),
            ["EDID:required", "ANAM:required", "BNAM:required"]
        );
        assert!(items[2]
            .todos
            .iter()
            .any(|t| t.contains("unordered = true")));
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn the_check_reads_every_cited_form() {
        let c = parse_cited(
            "corpus: inventory_types_full (x), 2026-10-28: in 60 of 62 official SFPC records \
             (335 occurrences, up to 10 per record), each a BNAM (13..32 bytes) followed by its \
             CNAM (4 bytes)",
        )
        .unwrap();
        assert_eq!(
            (c.with, c.total, c.occurrences, c.per_record),
            (60, 62, 335, Some(10))
        );
        assert_eq!(c.sig, "SFPC");
        assert_eq!(
            cited_sizes(&c.tail),
            [
                (Some("BNAM".to_string()), 13, 32),
                (Some("CNAM".to_string()), 4, 4)
            ]
        );
        let c = parse_cited(
            "corpus: x, 2026-10-27: in 1263 of 17410 official LMSW records (1915 occurrences, \
             0..2 per record), BFCB frame 25..36 bytes",
        )
        .unwrap();
        assert_eq!(c.per_record, Some(2));
        assert_eq!(cited_sizes(&c.tail), [(None, 25, 36)]);
        assert!(
            parse_cited("corpus: in 1 of 6530 official AVMD records (3 BFCB blocks)").is_none()
        );
    }

    #[test]
    fn the_check_finds_invented_counts() {
        let scan = scan_of(
            &[("EDID LNAM", 4)],
            &[("EDID", 4, 4, 1, 1, 3, 9), ("LNAM", 4, 4, 1, 1, 4, 4)],
        );
        let text = "# X: Y (4 in the 14 ESMs,\n\
            signature = \"AVMD\"\nname = \"AvmsData\"\n\
            source = \"corpus: inventory_types_full (crates/esm/tests/inventory_types.rs), d: every official AVMD record (4) decodes\"\n\
            confidence = \"corpus\"\n\
            [[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = \"zstring\"\n\
            source = \"corpus: inventory_types_full (crates/esm/tests/inventory_types.rs), d: in 4 of 4 official AVMD records (4 occurrences), 3..9 bytes\"\n\
            confidence = \"corpus\"\n\
            [[member]]\nsig = \"LNAM\"\nname = \"Lnam\"\ntype = \"opaque\"\ncard = \"optional\"\n\
            source = \"corpus: inventory_types_full (crates/esm/tests/inventory_types.rs), d: in 4 of 4 official AVMD records (46000 occurrences), 4 bytes\"\n\
            confidence = \"corpus\"\n";
        let (errors, _) = check_def("AVMD", text, &scan).unwrap();
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors[0].contains("card = \"optional\""), "{errors:?}");
        assert!(
            errors[1].contains("46000 LNAM occurrences cited"),
            "{errors:?}"
        );
    }
}
