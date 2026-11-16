//! The layout files the binary ships (M7.W3 read-only dialogs; M7.W4, pipeline E, adds
//! the observed ones), and the test that holds each against the schema and its
//! `docs/ck-parity.md` row.
//!
//! Moved here from `dialogs/mod.rs` at step 0 of sprint 2026-11-02 (seam hole H5), so
//! M7.W4 can add a layout or mark one observed in E's own file.
//!
//! M7.W4 (sprint 2026-11-02): CK sitting 2 (`ck-20261102-03`) recorded the labels of seven
//! edit dialogs (AMMO, COBJ, LCTN, TERM, CONT, STAT, KYWD) and could not open WEAP's.
//! `layouts/sitting2.toml` holds one `[[dialog]]` per type ([`observation`]): the
//! sitting, its step, its status and, when observed, the dialog's labels in the record's
//! order, copied from `docs/ck-observations.md` (a test holds them equal to the record).
//! The tabs and groups stay ours (`observed = false`, or the default layout from the
//! schema for a type without a layout file): the sitting recorded
//! labels, not which field each label edits, and no tab names (LCTN's three are in its
//! screenshots only).
//!
//! M7.K4 (sprint 2026-11-09): each observed label is either mapped to the `set` field path
//! it edits (`map`) or named as a gap with its reason (`gaps`): a button, a column or
//! caption, a record header flag (`set` edits subrecords only), a member the schema lacks,
//! or a label the sitting did not tie to one field. A label is never both, and never
//! neither (the members test).
//!
//! M7.W4, six layouts (sprint 2026-11-09, D6): AMMO, COBJ, CONT, STAT, KYWD (new) and TERM
//! (rewritten) are one page each, named after the dialog's observed title: one group per
//! mapped label, in the record's order, whose field is the signature the label edits and
//! whose `members` is its `set` field path. The edit dialog places such a layout by member
//! ([`place_members`]): each label shows the subrecords of its signature outside the
//! base-form component blocks, and edits the member it names in place when the value is
//! a number or plain text. A field whose signature a component block of the record also
//! holds is read-only (`set` addresses `SIG[n]` across the whole record; D1 review L4).
//! The unmapped labels are listed read-only with their reasons. AMMO's and COBJ's pages
//! are observed (the sitting recorded no tab control); the other four say unobserved,
//! since the sitting did not record whether they have tabs.

use super::Layout;
use crate::panels::records::SubRow;

/// What a CK sitting recorded of a type's edit dialog (a `[[dialog]]` of
/// `layouts/sitting2.toml`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// The ledger entry (`ck-20261102-03`).
    pub sitting: String,
    /// The sitting's step (`S2-02`).
    pub step: String,
    /// The form opened (its EditorID).
    pub form: String,
    /// `observed` or `inconclusive`.
    pub status: String,
    /// What the record says of the tab control (`none`, `3 tabs ...`, `not recorded`).
    pub tab_control: String,
    /// The dialog's labels in the record's order (empty unless observed).
    pub labels: Vec<String>,
    /// M7.K4's mapping, label by label: (observed label, `set` field path such as `EDID`
    /// or `CNAM.Red`), from the `[[dialog]]`'s `map = [{ label, member }]`. Empty until
    /// M7.K4 maps a label; a label without a pair is a gap.
    pub map: Vec<(String, String)>,
    /// M7.K4's gaps: (observed label, why it maps to no field path), from the
    /// `[[dialog]]`'s `gaps = [{ label, reason }]`.
    pub gaps: Vec<(String, String)>,
}

/// CK sitting 2's record of the edit dialogs (M7.W4).
pub const SITTING2_FILE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/layouts/sitting2.toml"
));

/// The sitting observation of `signature`'s edit dialog, when sitting 2 recorded one. The
/// file is parsed once per process (the dialog asks every frame; M7.W4 review L4).
pub fn observation(signature: &str) -> Option<Observation> {
    static ALL: std::sync::OnceLock<Vec<(String, Observation)>> = std::sync::OnceLock::new();
    ALL.get_or_init(parse_sitting2)
        .iter()
        .find(|(t, _)| t == signature)
        .map(|(_, o)| o.clone())
}

/// Every `[[dialog]]` of [`SITTING2_FILE`], by type.
fn parse_sitting2() -> Vec<(String, Observation)> {
    let Ok(table) = SITTING2_FILE.parse::<toml::Table>() else {
        return Vec::new();
    };
    table
        .get("dialog")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
        .filter_map(|d| {
            let t = d.get("type")?.as_str()?.to_string();
            Some((t, dialog_of(d)?))
        })
        .collect()
}

/// One `[[dialog]]` as an [`Observation`].
fn dialog_of(o: &toml::Table) -> Option<Observation> {
    let s = |k: &str| {
        o.get(k)
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let labels = o
        .get("labels")
        .and_then(toml::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let pairs = |key: &str, second: &str| -> Vec<(String, String)> {
        o.get(key)
            .and_then(toml::Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(toml::Value::as_table)
                    .filter_map(|m| {
                        let label = m.get("label")?.as_str()?.to_string();
                        let value = m.get(second)?.as_str()?.to_string();
                        Some((label, value))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let map = pairs("map", "member");
    let gaps = pairs("gaps", "reason");
    Some(Observation {
        sitting: s("sitting"),
        step: s("step"),
        form: s("form"),
        status: s("status"),
        tab_control: s("tab_control"),
        labels,
        map,
        gaps,
    })
}

/// The layout files shipped in the binary, by record type.
pub const SHIPPED: &[(&str, &str)] = &[
    (
        "WEAP",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/WEAP.toml")),
    ),
    (
        "ACTI",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/ACTI.toml")),
    ),
    (
        "LCTN",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/LCTN.toml")),
    ),
    (
        "TERM",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/TERM.toml")),
    ),
    (
        "FACT",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/FACT.toml")),
    ),
    (
        "PKIN",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/PKIN.toml")),
    ),
    (
        "CELL",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/CELL.toml")),
    ),
    (
        "REFR",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/REFR.toml")),
    ),
    (
        "AMMO",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/AMMO.toml")),
    ),
    (
        "COBJ",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/COBJ.toml")),
    ),
    (
        "CONT",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/CONT.toml")),
    ),
    (
        "STAT",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/STAT.toml")),
    ),
    (
        "KYWD",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/layouts/KYWD.toml")),
    ),
];

/// The field paths of the seven member layouts (the six observed types and REFR) whose
/// value is a set of flags or an enum, mapped or falling to "Other" (D7 review L1),
/// component-block enums such as `Bool8` included (D7-fix review L1). Read-only goes by
/// signature, so a signature any of whose definitions is flags or an enum is listed.
/// The engine shows such a value as a plain number (the raw bits, the raw value), so the
/// edit dialog would offer it as a number to type; it stays read-only instead (D2-fix
/// review L2: typing `1` into AMMO's flags sets IgnoresNormalWeaponResistance, not
/// Playable). Pinned against the schema by the test
/// `flags_and_enums_are_pinned_from_the_schema`.
pub const NOT_NUMBERS: &[(&str, &str)] = &[
    ("AMMO", "DNAM.Flags"),
    ("AMMO", "FLLD"),
    ("AMMO", "YNAM"),
    ("AMMO", "VNAM"),
    ("AMMO", "NAM8"),
    ("AMMO", "ESSF"),
    ("AMMO", "XFLG"),
    ("COBJ", "RECF"),
    ("COBJ", "FLLD"),
    ("COBJ", "YNAM"),
    ("COBJ", "VNAM"),
    ("COBJ", "NAM8"),
    ("COBJ", "ESSF"),
    ("COBJ", "TNAM"),
    ("COBJ", "LRNM"),
    ("TERM", "NAM1"),
    ("TERM", "FNAM"),
    ("TERM", "MNAM"),
    ("TERM", "FLLD"),
    ("TERM", "YNAM"),
    ("TERM", "VNAM"),
    ("TERM", "NAM8"),
    ("TERM", "ESSF"),
    ("TERM", "XFLG"),
    ("CONT", "DATA"),
    ("CONT", "FLLD"),
    ("CONT", "YNAM"),
    ("CONT", "VNAM"),
    ("CONT", "NAM8"),
    ("CONT", "ESSF"),
    ("CONT", "XFLG"),
    ("STAT", "FLLD"),
    ("STAT", "YNAM"),
    ("STAT", "VNAM"),
    ("STAT", "NAM8"),
    ("STAT", "ESSF"),
    ("STAT", "XFLG"),
    ("KYWD", "FLLD"),
    ("KYWD", "YNAM"),
    ("KYWD", "VNAM"),
    ("KYWD", "NAM8"),
    ("KYWD", "ESSF"),
    ("REFR", "FLLD"),
    ("REFR", "YNAM"),
    ("REFR", "VNAM"),
    ("REFR", "NAM8"),
    ("REFR", "ESSF"),
    ("REFR", "XACT"),
    ("REFR", "XLSM"),
    ("REFR", "FNAM"),
    ("REFR", "VISI"),
    ("REFR", "XLCM"),
    ("REFR", "INAM"),
];

/// Numeric fields the edit dialog shows read-only although they are plain numbers: REFR's
/// rotation is stored in radians while the CK's Reference dialog edits it in degrees, so
/// a CK user typing `90` would write 90 radians (D7 review M1). No conversion is offered.
pub const READ_ONLY: &[(&str, &str)] = &[
    ("REFR", "DATA.Rotation.X"),
    ("REFR", "DATA.Rotation.Y"),
    ("REFR", "DATA.Rotation.Z"),
];

/// Whether the edit dialog may offer `path` of `record_type` as a number to type: not a
/// flags or enum member ([`NOT_NUMBERS`]) and not held read-only ([`READ_ONLY`]).
pub fn edits_as_number(record_type: &str, path: &str) -> bool {
    !NOT_NUMBERS
        .iter()
        .chain(READ_ONLY)
        .any(|(t, p)| *t == record_type && *p == path)
}

/// Which of `subrows` lie inside a base-form component block (`BFCB` to `BFCE`, both
/// included), whose subrecords may reuse a signature the record also has at the top level.
pub fn in_component(subrows: &[SubRow]) -> Vec<bool> {
    let mut inside = false;
    subrows
        .iter()
        .map(|s| {
            if s.signature == "BFCB" {
                inside = true;
            }
            let here = inside;
            if s.signature == "BFCE" {
                inside = false;
            }
            here
        })
        .collect()
}

/// The text of the member `steps` name inside a struct value as the Records inspector
/// formats it (`{A: 1, B: {C: 2}}`, `panels::fmt_value`); the value itself for no steps.
/// `None` for a value that is no such struct, a cut one (`...`), or a member it lacks.
pub fn member_text(value: &str, steps: &[&str]) -> Option<String> {
    let Some((first, rest)) = steps.split_first() else {
        return Some(value.to_string());
    };
    if value.ends_with("...") {
        return None;
    }
    let inner = value.strip_prefix('{')?.strip_suffix('}')?;
    let mut depth = 0u32;
    let mut start = 0usize;
    let mut items = Vec::new();
    for (i, c) in inner.char_indices() {
        match c {
            '{' | '[' => depth = depth.saturating_add(1),
            '}' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(inner.get(start..i)?);
                start = i.saturating_add(1);
            }
            _ => {}
        }
    }
    items.push(inner.get(start..)?);
    let v = items.iter().find_map(|item| {
        let (k, v) = item.trim().split_once(": ")?;
        (k == *first).then_some(v)
    })?;
    member_text(v, rest)
}

/// One subrecord a labelled field shows (a field of a member layout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRow<'a> {
    /// Its position in the version's subrecords (for its `set` occurrence).
    pub index: usize,
    pub sub: &'a SubRow,
    /// The member steps inside the subrecord (`["Value"]` for `DATA.Value`).
    pub steps: Vec<String>,
    /// A component block of the record holds the signature too: read-only here.
    pub blocked: bool,
}

/// One labelled field: the observed label, the `set` field path it edits, and the
/// subrecords it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberGroup<'a> {
    pub label: String,
    pub path: String,
    pub rows: Vec<MemberRow<'a>>,
}

/// One page of a member layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberPage<'a> {
    pub name: String,
    pub groups: Vec<MemberGroup<'a>>,
}

/// The subrecords no label of a member layout shows, each with its position in the
/// version's subrecords (for its `set` occurrence path).
pub type Leftover<'a> = Vec<(usize, &'a SubRow)>;

/// One line of a member page in the order sitting 2 recorded the labels (D2-fix review
/// L3): a mapped label's group (its index in the page), or an unmapped label and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Mapped(usize),
    Gap(String, String),
}

/// The lines of `page` in the order sitting 2 recorded `record_type`'s labels, each label
/// its group or its gap; without an observation, the groups in the page's order. A group
/// the observation does not list (none today) follows.
pub fn observed_lines(record_type: &str, page: &MemberPage<'_>) -> Vec<Line> {
    let mut out = Vec::new();
    if let Some(o) = observation(record_type) {
        for l in &o.labels {
            if let Some(i) = page.groups.iter().position(|g| &g.label == l) {
                out.push(Line::Mapped(i));
            } else if let Some((_, why)) = o.gaps.iter().find(|(g, _)| g == l) {
                out.push(Line::Gap(l.clone(), why.clone()));
            }
        }
    }
    for i in 0..page.groups.len() {
        if !out.contains(&Line::Mapped(i)) {
            out.push(Line::Mapped(i));
        }
    }
    out
}

/// The placement of a layout whose groups name the member each label edits (`members`):
/// `None` for a layout without (the signature placement, `super::place`). Each label
/// shows every subrecord of its signature outside the component blocks; a signature a
/// component block also holds makes the field read-only. Returned last: the subrecords no
/// label shows (component blocks included), with their positions, for the "Other"
/// page.
pub fn place_members<'a>(
    layout: &Layout,
    subrows: &'a [SubRow],
) -> Option<(Vec<MemberPage<'a>>, Leftover<'a>)> {
    if !layout
        .tabs
        .iter()
        .any(|t| t.groups.iter().any(|g| g.members.is_some()))
    {
        return None;
    }
    let comp = in_component(subrows);
    let inside = |i: usize| comp.get(i).copied().unwrap_or(true);
    let mut shown = vec![false; subrows.len()];
    let mut pages = Vec::new();
    for t in &layout.tabs {
        let mut groups = Vec::new();
        for g in &t.groups {
            let path = g
                .members
                .as_ref()
                .and_then(|m| m.first())
                .or_else(|| g.fields.first())
                .cloned()
                .unwrap_or_default();
            let mut parts = path.split('.');
            let sig = parts.next().unwrap_or_default().to_string();
            let steps: Vec<String> = parts.map(str::to_string).collect();
            let blocked = subrows
                .iter()
                .enumerate()
                .any(|(i, s)| inside(i) && s.signature == sig);
            let mut rows = Vec::new();
            for (i, s) in subrows.iter().enumerate() {
                if s.signature == sig && !inside(i) {
                    if let Some(x) = shown.get_mut(i) {
                        *x = true;
                    }
                    rows.push(MemberRow {
                        index: i,
                        sub: s,
                        steps: steps.clone(),
                        blocked,
                    });
                }
            }
            groups.push(MemberGroup {
                label: g.name.clone(),
                path,
                rows,
            });
        }
        pages.push(MemberPage {
            name: t.name.clone(),
            groups,
        });
    }
    let rest = subrows
        .iter()
        .enumerate()
        .zip(&shown)
        .filter(|(_, s)| !**s)
        .map(|(r, _)| r)
        .collect();
    Some((pages, rest))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::super::{schema_signatures, shipped};
    use super::{observation, SHIPPED};
    use crate::gate::{check, ReadOnlyEngine, Request};
    use serde_json::json;
    use std::path::{Path, PathBuf};

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf()
    }

    /// Statuses at or above `observed` (`docs/ck-parity.md`, "Status").
    const OBSERVED_OR_BETTER: &[&str] = &["observed", "reproduced", "improved"];

    /// Every shipped layout parses, names its own type, cites a `dialog` row of
    /// `docs/ck-parity.md`, says it is observed only when that row is `observed` or
    /// better, and names only signatures the record schema defines for that type (asked
    /// through the gate, as the window asks).
    #[test]
    fn every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row() {
        let parity = std::fs::read_to_string(repo().join("docs").join("ck-parity.md")).unwrap();
        // (element, status) of every dialog row.
        let dialog_rows: Vec<(String, String)> = parity
            .lines()
            .filter(|l| l.starts_with("| dialog |"))
            .filter_map(|l| {
                let mut cells = l.split('|').map(str::trim);
                let element = cells.nth(2)?.to_string();
                let status = cells.nth(1)?.to_string();
                Some((element, status))
            })
            .collect();
        let engine = ReadOnlyEngine::new();
        assert_eq!(SHIPPED.len(), 13);
        for (t, _) in SHIPPED {
            let layout = shipped(t).unwrap().unwrap();
            assert_eq!(&layout.record_type, t);
            let status = dialog_rows
                .iter()
                .find(|(element, _)| *element == layout.parity)
                .map(|(_, status)| status.as_str());
            assert!(
                status.is_some(),
                "{t}: parity `{}` is no dialog row of docs/ck-parity.md",
                layout.parity
            );
            assert!(
                !layout.observed || status.is_some_and(|s| OBSERVED_OR_BETTER.contains(&s)),
                "{t}: `observed = true` needs its row `observed` or better, not {status:?}"
            );
            let req = Request::new("schema", json!({"type": t}));
            assert!(check(&req).is_ok());
            let env = engine.run(&req).unwrap();
            let sigs = schema_signatures(&env);
            assert!(sigs.len() > 10, "{t}: {sigs:?}");
            for tab in &layout.tabs {
                assert!(!tab.name.trim().is_empty() && !tab.groups.is_empty(), "{t}");
                for g in &tab.groups {
                    assert!(!g.fields.is_empty(), "{t}/{}/{}", tab.name, g.name);
                    for f in &g.fields {
                        assert!(sigs.contains(f), "{t}: {f} is not in the schema");
                    }
                }
            }
        }
    }
    /// Sitting 2's eight types and their steps in `ck-20261102-03`.
    const SITTING2: &[(&str, &str)] = &[
        ("WEAP", "S2-01"),
        ("AMMO", "S2-02"),
        ("COBJ", "S2-03"),
        ("LCTN", "S2-04"),
        ("TERM", "S2-05"),
        ("CONT", "S2-06"),
        ("STAT", "S2-07"),
        ("KYWD", "S2-08"),
    ];

    /// The labels of a sitting row (`Labels: a, b (c, d), e; OK, Cancel`), split at
    /// top-level commas.
    fn row_labels(row: &str) -> Option<Vec<String>> {
        let start = row.find("Labels: ")? + "Labels: ".len();
        let rest = row.get(start..)?;
        let end = rest.find("; OK, Cancel")?;
        let text = rest.get(..end)?;
        let mut out = Vec::new();
        let (mut depth, mut cur) = (0i32, String::new());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            if c == ',' && depth == 0 && chars.peek() == Some(&' ') {
                chars.next();
                out.push(std::mem::take(&mut cur));
                continue;
            }
            cur.push(c);
        }
        out.push(cur);
        Some(out)
    }

    #[test]
    fn a_rows_labels_split_at_top_level_commas() {
        let row = "| S2-08 KYWD `X` | **observed**. Labels: ID, Editor Color (R, G, B, Select Color), Restricted; OK, Cancel |";
        assert_eq!(
            row_labels(row).unwrap(),
            ["ID", "Editor Color (R, G, B, Select Color)", "Restricted"]
        );
        assert!(row_labels("| S2-01 WEAP | **inconclusive** |").is_none());
    }

    /// M7.W4: every one of sitting 2's eight types has its `[[dialog]]`; the
    /// seven observed ones carry labels and the record's form; WEAP is the marked
    /// placeholder, "inconclusive", with no labels. When `docs/ck-observations.md` holds
    /// the sitting (`ck-20261102-03`), each observed layout's labels equal the record's
    /// row exactly, in its order (ADR 0012: nothing from memory); on a branch without
    /// the record the comparison says so and is skipped.
    #[test]
    fn sitting_two_layouts_hold_the_recorded_labels() {
        let doc = std::fs::read_to_string(repo().join("docs").join("ck-observations.md"))
            .unwrap_or_default();
        let has_record = doc.contains("### ck-20261102-03");
        if !has_record {
            eprintln!(
                "sitting_two_layouts_hold_the_recorded_labels: ck-20261102-03 is not in docs/ck-observations.md on this branch; the label comparison is skipped (it runs once X's record is merged)"
            );
        }
        for (t, step) in SITTING2 {
            let o = observation(t).unwrap_or_else(|| panic!("{t}: no [[dialog]] in sitting2.toml"));
            assert_eq!(o.sitting, "ck-20261102-03", "{t}");
            assert_eq!(&o.step, step, "{t}");
            assert!(!o.form.is_empty(), "{t}");
            // A shipped layout says observed only for a dialog the sitting saw without a
            // tab control (D6: AMMO, COBJ); the others' tabs are ours.
            if let Some(layout) = shipped(t) {
                assert_eq!(
                    layout.unwrap().observed,
                    o.tab_control == "none",
                    "{t}: observed only where no tab control was recorded"
                );
            }
            if *t == "WEAP" {
                assert_eq!(o.status, "inconclusive", "WEAP");
                assert!(o.labels.is_empty(), "WEAP has no observed labels");
                continue;
            }
            assert_eq!(o.status, "observed", "{t}");
            assert!(o.labels.len() >= 8, "{t}: {:?}", o.labels);
            if !has_record {
                continue;
            }
            let prefix = format!("| {step} {t} `{}`", o.form);
            let row = doc
                .lines()
                .find(|l| l.starts_with(&prefix))
                .unwrap_or_else(|| panic!("{t}: no row `{prefix}` in the record"));
            assert!(row.contains("**observed**"), "{t}: {row}");
            assert_eq!(
                row_labels(row).unwrap(),
                o.labels,
                "{t}: the labels differ from the record"
            );
        }
        // LCTN's three tabs are recorded as a count only.
        assert!(observation("LCTN")
            .unwrap()
            .tab_control
            .starts_with("3 tabs"));
        assert_eq!(observation("AMMO").unwrap().tab_control, "none");
    }

    /// M7.K4's mapping as the members check pins it, type by type: (observed label, `set`
    /// field path), in the labels' order. A mapping lands in two places, the
    /// `[[dialog]]`'s `map` in `layouts/sitting2.toml` and here, so a mapping changed in one
    /// and not the other fails. Several labels may name one field: the flags of a flags
    /// member (LCTN's level flags, CONT's `DATA`) each edit that member.
    const MAPPED: &[(&str, &[(&str, &str)])] = &[
        (
            "AMMO",
            &[
                ("ID", "EDID"),
                ("Name", "FULL"),
                ("Short name", "ONAM"),
                ("Value", "DATA.Value"),
                ("Model", "MODL"),
                ("Pickup Sound", "PUSH"),
                ("Putdown Sound", "PDSH"),
                ("Projectile", "DNAM.Projectile"),
                ("Damage", "DNAM.Damage"),
                ("Description", "DESC"),
                ("Keywords", "KWDA"),
                ("Preview Transform", "PTT2.Preview"),
                ("Shell Casing", "NAM1"),
                ("Health", "DNAM.Health"),
                ("Weight", "DATA.Weight"),
                ("Flags (Playable, Has Count Based 3D)", "DNAM.Flags"),
            ],
        ),
        (
            "COBJ",
            &[
                ("ID", "EDID"),
                ("Created Object", "CNAM"),
                ("Created Object Count", "NNAM"),
                ("Required Item List", "FVPA"),
                ("Workbench Keyword", "BNAM"),
                ("Recipe Filters", "FNAM"),
                ("Pickup Sound:", "PUSH"),
                ("Putdown Sound:", "PDSH"),
                ("Crafting Sound: (each with Select Audio Data)", "CUSH"),
                ("Priority", "SNAM"),
                ("Menu Art Object", "ANAM"),
                ("Description", "DESC"),
                ("Learned From", "GNAM"),
                ("Base Return Scale Table", "CVT0"),
                ("Required Perks", "RQPK"),
                ("Max Build Count", "JNAM"),
                ("Value", "DATA"),
                ("Exclude From Workshop LOD", "RECF"),
            ],
        ),
        (
            "LCTN",
            &[
                ("ID", "EDID"),
                ("Name", "FULL"),
                ("Parent Location", "PNAM"),
                ("World Location Marker Ref", "MNAM"),
                ("Music Type", "NAM1"),
                ("Unreported Crime Faction", "FNAM"),
                ("Actor Fade Mult", "ANAM"),
                ("World Location Radius", "RNAM"),
                ("Random Conversation Timer", "TNAM"),
                ("Color (R, G, B, Select)", "CNAM"),
                ("Keywords", "KWDA"),
                ("Match PC Below Minimum Level", "DATA.LevelFlags"),
                ("Never Resets", "DATA.LevelFlags"),
                ("Disable Combat Boundary", "DATA.LevelFlags"),
                ("Workshop", "DATA.LevelFlags"),
                ("Min Level:", "DATA.MinLevel"),
                ("Max Level:", "DATA.MaxLevel"),
                ("Required Rank", "DATA.FactionRank"),
                ("Use parent", "DATA.LevelFlags"),
            ],
        ),
        (
            "TERM",
            &[
                ("ID", "EDID"),
                ("Name", "FULL"),
                ("Model", "MODL"),
                ("Marker Model", "XMRK"),
                ("Looping Sound (Select Audio Data)", "ALSH"),
                ("Papyrus Scripts:", "VMAD"),
                ("Keywords", "KWDA"),
                ("Actor Values", "PRPS"),
                ("Preview Transform", "PTT2.Preview"),
                ("Forced Loc Ref Types:", "FTYP"),
                ("Art Theme", "NAM1"),
                ("Furniture Template", "FTMP"),
                ("Ignored By Sandbox", "FNAM"),
                ("Active Markers", "MNAM"),
            ],
        ),
        (
            "CONT",
            &[
                ("ID", "EDID"),
                ("Name", "FULL"),
                ("Model", "MODL"),
                ("Object", "CNTO.Item"),
                ("Count", "CNTO.Count"),
                ("Item List", "CNTO"),
                ("Owner", "COED.Owner"),
                ("Health %", "COED.ItemCondition"),
                ("Papyrus Scripts:", "VMAD"),
                ("Keywords", "KWDA"),
                ("Preview Transform", "PTT2.Preview"),
                ("Native Terminal", "NTRM"),
                ("Contains Only", "ONAM"),
                ("Actor Values", "PRPS"),
                ("Forced Loc Ref Types:", "FTYP"),
                ("Attach Parent Slots", "APPR"),
                ("Snap Template", "SNTP"),
                ("Respawns", "DATA"),
                ("Show Owner", "DATA"),
            ],
        ),
        (
            "STAT",
            &[
                ("ID", "EDID"),
                ("Display Name", "FULL"),
                ("Model", "MODL"),
                ("Object Palette Defaults", "OPDS"),
                ("Actor Values", "PRPS"),
                ("Preview Transform", "PTT2.Preview"),
                ("Leaf Amplitude", "DNAM.Leaf.LeafAmplitude"),
                ("Leaf Frequency", "DNAM.Leaf.LeafFrequency"),
                ("Default Layer", "DEFL"),
                ("Snap Template", "SNTP"),
                ("Snap Behavior", "SNBH"),
                ("Forced Loc Ref Type", "FTYP"),
            ],
        ),
        (
            "KYWD",
            &[
                ("ID", "EDID"),
                ("Display Name", "FULL"),
                ("Type", "TNAM"),
                ("Rule", "DATA"),
                ("Linkage Name", "ENAM"),
                ("Notes", "DNAM"),
                ("Editor Color (R, G, B, Select Color)", "CNAM"),
            ],
        ),
    ];

    /// How many labels each type maps and how many are gaps (M7.K4, sprint 2026-11-09).
    const COUNTS: &[(&str, usize, usize)] = &[
        ("AMMO", 16, 5),
        ("COBJ", 18, 5),
        ("LCTN", 19, 17),
        ("TERM", 14, 12),
        ("CONT", 19, 28),
        ("STAT", 12, 25),
        ("KYWD", 7, 1),
    ];

    /// Whether the deep `schema` envelope of a type has the `set` field path `path`
    /// (`SIG`, `SIG.Member` or `SIG.Member.Member`): a subrecord of that signature, at the
    /// top level or in a group (not inside the base-form component block, whose payloads
    /// stay raw), whose members or fields hold each named step in turn.
    fn schema_has(env: &serde_json::Value, path: &str) -> bool {
        use serde_json::Value;
        fn entries<'a>(members: &'a Value, sig: &str, out: &mut Vec<&'a Value>) {
            for m in members.as_array().into_iter().flatten() {
                if m.get("signature").and_then(Value::as_str) == Some(sig) {
                    out.push(m);
                } else if m.get("kind").and_then(Value::as_str) == Some("group")
                    && m.get("name").and_then(Value::as_str) != Some("BfcbBlock")
                {
                    entries(&m["members"], sig, out);
                }
            }
        }
        fn child<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
            ["members", "fields"]
                .iter()
                .flat_map(|k| v.get(*k).and_then(Value::as_array).into_iter().flatten())
                .find(|m| m.get("name").and_then(Value::as_str) == Some(name))
        }
        let mut steps = path.split('.');
        let Some(sig) = steps.next() else {
            return false;
        };
        let steps: Vec<&str> = steps.collect();
        if steps.iter().any(|s| s.is_empty()) {
            return false;
        }
        let mut found = Vec::new();
        entries(&env["data"]["members"], sig, &mut found);
        found
            .iter()
            .any(|e| steps.iter().try_fold(*e, |v, s| child(v, s)).is_some())
    }

    /// M7.K4's check of one observed dialog against its type's deep schema: every pair
    /// names an observed label once and a field path the schema has; every gap names an
    /// observed label once, with a reason; no label is both, and every label is one of
    /// them. Returns the gap report line, or the first rule broken.
    fn k4_check(
        t: &str,
        o: &super::Observation,
        env: &serde_json::Value,
    ) -> Result<String, String> {
        let mut seen: Vec<&String> = Vec::new();
        for (label, member) in &o.map {
            if !o.labels.contains(label) {
                return Err(format!("{t}: `{label}` is no observed label"));
            }
            if seen.contains(&label) {
                return Err(format!("{t}: `{label}` mapped twice"));
            }
            seen.push(label);
            if !schema_has(env, member) {
                return Err(format!(
                    "{t}: `{label}` -> `{member}` is not a field of the schema"
                ));
            }
        }
        let mut gapped: Vec<&String> = Vec::new();
        for (label, reason) in &o.gaps {
            if !o.labels.contains(label) {
                return Err(format!("{t}: gap `{label}` is no observed label"));
            }
            if seen.contains(&label) {
                return Err(format!("{t}: `{label}` is both mapped and a gap"));
            }
            if gapped.contains(&label) {
                return Err(format!("{t}: gap `{label}` named twice"));
            }
            if reason.trim().is_empty() {
                return Err(format!("{t}: gap `{label}` has no reason"));
            }
            gapped.push(label);
        }
        if let Some(l) = o
            .labels
            .iter()
            .find(|l| !seen.contains(l) && !gapped.contains(l))
        {
            return Err(format!("{t}: `{l}` is neither mapped nor a gap"));
        }
        Ok(format!(
            "{t}: {} mapped, {} gaps (M7.K4): {}",
            o.map.len(),
            o.gaps.len(),
            o.gaps
                .iter()
                .map(|(l, r)| format!("{l} ({r})"))
                .collect::<Vec<_>>()
                .join(" | ")
        ))
    }

    /// M7.W4's members check, completed by M7.K4 (sprint 2026-11-09): a label is mapped
    /// only by an explicit pair (label, field path) in its `[[dialog]]`'s `map`, which must
    /// equal the [`MAPPED`] pin and pass [`k4_check`]; every other label is a gap with its
    /// reason in `gaps`. A type whose dialog was not observed is a gap as a whole. The
    /// counts are pinned, and the report printed for `docs/gaps.md`.
    #[test]
    fn every_observed_label_is_mapped_or_reported_as_a_gap() {
        let engine = ReadOnlyEngine::new();
        let mut gaps = Vec::new();
        let (mut mapped_total, mut gap_total) = (0usize, 0usize);
        for (t, _) in SITTING2 {
            let o = observation(t).unwrap();
            if o.status != "observed" {
                assert!(
                    o.map.is_empty() && o.gaps.is_empty(),
                    "{t}: a mapping for an unobserved dialog"
                );
                gaps.push(format!("{t}: unobserved ({} {})", o.sitting, o.status));
                continue;
            }
            let pinned: Vec<(String, String)> = MAPPED
                .iter()
                .find(|(ty, _)| ty == t)
                .map(|(_, pairs)| {
                    pairs
                        .iter()
                        .map(|(l, m)| ((*l).to_string(), (*m).to_string()))
                        .collect()
                })
                .unwrap_or_default();
            assert_eq!(o.map, pinned, "{t}: the mapping differs from the pin");
            // The pairs follow the labels' order, as the gaps do.
            let order = |pairs: &[(String, String)]| -> Vec<usize> {
                pairs
                    .iter()
                    .filter_map(|(l, _)| o.labels.iter().position(|x| x == l))
                    .collect()
            };
            for pairs in [&o.map, &o.gaps] {
                let idx = order(pairs);
                assert!(idx.windows(2).all(|w| w[0] < w[1]), "{t}: out of order");
            }
            let env = engine
                .run(&Request::new("schema", json!({"type": t, "deep": true})))
                .unwrap();
            gaps.push(k4_check(t, &o, &env).unwrap_or_else(|e| panic!("{e}")));
            let counts = COUNTS.iter().find(|(ty, _, _)| ty == t).unwrap();
            assert_eq!(
                (o.map.len(), o.gaps.len()),
                (counts.1, counts.2),
                "{t}: the counts differ from the pin"
            );
            mapped_total += o.map.len();
            gap_total += o.gaps.len();
        }
        eprintln!("M7.K4 gaps:\n  {}", gaps.join("\n  "));
        assert_eq!((mapped_total, gap_total), (105, 93));
        assert_eq!(gaps[0], "WEAP: unobserved (ck-20261102-03 inconclusive)");
        assert!(gaps[1].starts_with("AMMO: 16 mapped, 5 gaps (M7.K4): Edit (a button"));
        assert!(
            gaps[7].starts_with("KYWD: 7 mapped, 1 gaps (M7.K4): Restricted (a record header flag")
        );
    }

    /// The path check refuses what the review's probe used (a name that is no field path,
    /// a member the subrecord lacks), a step out of chain, and a signature only the raw
    /// component block holds; it accepts a nested member and a subrecord two groups deep.
    #[test]
    fn the_k4_path_check_follows_each_step() {
        let engine = ReadOnlyEngine::new();
        let schema = |t: &str| {
            engine
                .run(&Request::new("schema", json!({"type": t, "deep": true})))
                .unwrap()
        };
        let kywd = schema("KYWD");
        assert!(schema_has(&kywd, "EDID"));
        assert!(schema_has(&kywd, "TNAM"));
        assert!(schema_has(&kywd, "CNAM.Red"));
        assert!(!schema_has(&kywd, "Keywords"));
        assert!(!schema_has(&kywd, "EditorID"));
        assert!(!schema_has(&kywd, "CNAM.Nothing"));
        assert!(!schema_has(&kywd, "CNAM."));
        assert!(!schema_has(&kywd, "BFCB"));
        let stat = schema("STAT");
        assert!(schema_has(&stat, "DNAM.Leaf.LeafAmplitude"));
        assert!(!schema_has(&stat, "DNAM.LeafAmplitude.Leaf"));
        assert!(!schema_has(&stat, "DNAM.Nothing.LeafAmplitude"));
        assert!(!schema_has(&stat, "KWDA"));
        let cont = schema("CONT");
        assert!(schema_has(&cont, "CNTO.Item"));
        assert!(schema_has(&cont, "COED.Owner"));
        assert!(!schema_has(&cont, "CNTO.Owner"));
        // A PRPS row's member is no field path: `set` takes the array subrecord whole.
        assert!(schema_has(&cont, "PRPS"));
        assert!(!schema_has(&cont, "PRPS.ActorValue"));
    }

    /// D1 review L2: a gap that blames a record header flag names one the type's schema
    /// has (the label without spaces or dashes, case ignored; `Navmesh` before it for a
    /// navmesh import option; the named flag for an inverse), so `docs/gaps.md` never
    /// cites a flag that does not exist.
    #[test]
    fn a_header_flag_gap_names_a_flag_the_schema_has() {
        let engine = ReadOnlyEngine::new();
        let squash = |t: &str| -> String {
            t.chars()
                .filter(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_lowercase()
        };
        let mut checked = 0;
        for (t, _) in SITTING2 {
            let o = observation(t).unwrap();
            let env = engine
                .run(&Request::new("schema", json!({"type": t, "deep": true})))
                .unwrap();
            let flags: Vec<String> = env["data"]["header_flags"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|f| f["name"].as_str().map(squash))
                .collect();
            for (label, reason) in &o.gaps {
                if !reason.contains("header flag") || reason.contains("no header flag") {
                    continue;
                }
                // A reading the sitting did not confirm says "probably" first (review L2 of D2).
                let reason = reason.strip_prefix("probably ").unwrap_or(reason);
                let want = if let Some(rest) = reason.strip_prefix("the inverse of the ") {
                    squash(rest.split_whitespace().next().unwrap_or_default())
                } else if reason.starts_with("a navmesh import option") {
                    format!("navmesh{}", squash(label))
                } else {
                    squash(label)
                };
                assert!(
                    flags.contains(&want),
                    "{t}: `{label}` blames a header flag `{want}` the schema lacks: {flags:?}"
                );
                checked += 1;
            }
        }
        assert!(checked >= 25, "{checked}");
    }

    /// Every rule of [`k4_check`] refuses its planted break: a label mapped to no field,
    /// a label that was not observed, one mapped twice, one both mapped and a gap, a gap
    /// without a reason, a gap named twice, and a label left out of both.
    #[test]
    fn the_k4_check_refuses_each_planted_break() {
        let engine = ReadOnlyEngine::new();
        let env = engine
            .run(&Request::new(
                "schema",
                json!({"type": "KYWD", "deep": true}),
            ))
            .unwrap();
        let good = observation("KYWD").unwrap();
        assert!(k4_check("KYWD", &good, &env).is_ok());
        fn pair(l: &str, m: &str) -> (String, String) {
            (l.to_string(), m.to_string())
        }
        type Plant = Box<dyn Fn(&mut super::Observation)>;
        let planted: Vec<(&str, Plant)> = vec![
            (
                "is not a field",
                Box::new(|o| o.map[0] = pair("ID", "Keywords")),
            ),
            (
                "is no observed label",
                Box::new(|o| o.map.push(pair("Name", "FULL"))),
            ),
            ("mapped twice", Box::new(|o| o.map.push(pair("ID", "EDID")))),
            (
                "both mapped and a gap",
                Box::new(|o| o.gaps.push(pair("ID", "a planted reason"))),
            ),
            ("has no reason", Box::new(|o| o.gaps[0].1 = " ".to_string())),
            (
                "named twice",
                Box::new(|o| o.gaps.push(pair("Restricted", "again"))),
            ),
            ("neither mapped nor a gap", Box::new(|o| o.gaps.clear())),
            (
                "gap `Name` is no observed label",
                Box::new(|o| o.gaps.push(pair("Name", "planted"))),
            ),
        ];
        for (want, plant) in planted {
            let mut o = good.clone();
            plant(&mut o);
            let err = k4_check("KYWD", &o, &env).unwrap_err();
            assert!(err.contains(want), "{want}: {err}");
        }
    }

    /// D6 (M7.W4): the six layouts are one page each, named after the observed title, one
    /// group per mapped label in the record's order: the label, the signature it edits and
    /// its `set` field path, exactly the `[[dialog]]`'s `map`.
    #[test]
    fn the_six_layouts_follow_the_observed_labels() {
        let titles = [
            ("AMMO", "Ammo"),
            ("COBJ", "Constructible Object"),
            ("TERM", "Terminal"),
            ("CONT", "Container"),
            ("STAT", "Static"),
            ("KYWD", "Keyword"),
        ];
        for (t, title) in titles {
            let o = observation(t).unwrap();
            let l = shipped(t).unwrap().unwrap();
            assert_eq!(l.tabs.len(), 1, "{t}: one page");
            assert_eq!(l.tabs[0].name, title, "{t}");
            assert_eq!(l.observed, o.tab_control == "none", "{t}");
            let got: Vec<(String, String)> = l.tabs[0]
                .groups
                .iter()
                .map(|g| {
                    let m = g.members.clone().unwrap_or_default();
                    assert_eq!(m.len(), 1, "{t}/{}", g.name);
                    let sig = m[0].split('.').next().unwrap().to_string();
                    assert_eq!(g.fields, [sig], "{t}/{}", g.name);
                    (g.name.clone(), m[0].clone())
                })
                .collect();
            assert_eq!(got, o.map, "{t}: the groups differ from the K4 map");
        }
    }

    fn sub(sig: &str, member: &str, value: &str) -> crate::panels::records::SubRow {
        crate::panels::records::SubRow {
            signature: sig.to_string(),
            member: member.to_string(),
            value: value.to_string(),
            offset: None,
            size: None,
            confidence: None,
            source: None,
        }
    }

    /// A member's text inside a formatted struct: nested, past a list's commas, and
    /// `None` for a missing member, a cut value or a value that is no struct.
    #[test]
    fn member_text_reads_a_formatted_struct() {
        use super::member_text;
        let v = "{MaxAngle: 1.5, Value2: [1, 2], Leaf: {LeafAmplitude: 3.5, LeafFrequency: 4}}";
        assert_eq!(member_text(v, &["MaxAngle"]).as_deref(), Some("1.5"));
        assert_eq!(
            member_text(v, &["Leaf", "LeafFrequency"]).as_deref(),
            Some("4")
        );
        assert_eq!(member_text(v, &["Value2"]).as_deref(), Some("[1, 2]"));
        assert_eq!(member_text(v, &[]).as_deref(), Some(v));
        assert_eq!(member_text(v, &["Nothing"]), None);
        assert_eq!(member_text(v, &["Leaf", "Nothing"]), None);
        assert_eq!(member_text("{Value: 12, Weight: 0.5...", &["Value"]), None);
        assert_eq!(member_text("12", &["Value"]), None);
    }

    /// D6's placement, with the D1 review's L4 as a negative: each label shows its
    /// signature's subrecords outside the component blocks; a signature a component
    /// block of the record also holds (`DNAM` here) is read-only for every label naming
    /// it; what no label shows (the block, an unplaced subrecord) is returned last; a
    /// layout without members is not placed by member.
    #[test]
    fn member_placement_skips_component_blocks_and_blocks_a_shared_signature() {
        use super::{in_component, place_members};
        let rows = vec![
            sub("EDID", "EditorId", "AmmoX"),
            sub("BFCB", "ComponentType", "BGSAnimationGraph_Component"),
            sub("DNAM", "ResponseFile", "x.txt"),
            sub("BFCE", "EndMarker", ""),
            sub("FULL", "Name", "Ammo X"),
            sub("DATA", "Data", "{Value: 12, Weight: 0.5}"),
            sub(
                "DNAM",
                "AmmoData",
                "{Projectile: null, Flags: 0, Damage: 4, Health: 1}",
            ),
            sub("XXXX", "", "hex 00"),
        ];
        assert_eq!(
            in_component(&rows),
            [false, true, true, true, false, false, false, false]
        );
        let layout = shipped("AMMO").unwrap().unwrap();
        let (pages, rest) = place_members(&layout, &rows).unwrap();
        let groups = &pages[0].groups;
        let g = |label: &str| groups.iter().find(|g| g.label == label).unwrap();
        assert_eq!(g("ID").rows.len(), 1);
        assert_eq!(g("ID").rows[0].index, 0);
        assert!(!g("ID").rows[0].blocked);
        assert_eq!(g("Weight").path, "DATA.Weight");
        assert_eq!(g("Weight").rows[0].steps, ["Weight"]);
        assert!(!g("Weight").rows[0].blocked);
        // DNAM: the top-level one only, and read-only (the block holds a DNAM too).
        let damage = g("Damage");
        assert_eq!(damage.rows.len(), 1);
        assert_eq!(damage.rows[0].index, 6);
        assert!(
            damage.rows[0].blocked,
            "L4: a signature the block holds is read-only"
        );
        assert!(g("Projectile").rows[0].blocked);
        assert!(g("Short name").rows.is_empty(), "no ONAM in this record");
        let rest: Vec<(usize, &str)> = rest
            .iter()
            .map(|(i, s)| (*i, s.signature.as_str()))
            .collect();
        assert_eq!(rest, [(1, "BFCB"), (2, "DNAM"), (3, "BFCE"), (7, "XXXX")]);
        // A record without the block: DNAM edits.
        let plain: Vec<_> = rows
            .iter()
            .filter(|s| {
                !matches!(s.signature.as_str(), "BFCB" | "BFCE") && s.member != "ResponseFile"
            })
            .cloned()
            .collect();
        let (pages, _) = place_members(&layout, &plain).unwrap();
        assert!(pages[0]
            .groups
            .iter()
            .all(|g| g.rows.iter().all(|r| !r.blocked)));
        // WEAP's layout names no members: placed by signature.
        assert!(place_members(&shipped("WEAP").unwrap().unwrap(), &rows).is_none());
    }

    /// The type of the leaf `path` names in a deep `schema` envelope (the subrecord at the
    /// top level or in a group outside the component block, then each member step).
    fn leaf_type(env: &serde_json::Value, path: &str) -> Option<String> {
        use serde_json::Value;
        fn find<'a>(members: &'a Value, sig: &str) -> Option<&'a Value> {
            for m in members.as_array().into_iter().flatten() {
                if m.get("signature").and_then(Value::as_str) == Some(sig) {
                    return Some(m);
                }
                if m.get("kind").and_then(Value::as_str) == Some("group")
                    && m.get("name").and_then(Value::as_str) != Some("BfcbBlock")
                {
                    if let Some(f) = find(&m["members"], sig) {
                        return Some(f);
                    }
                }
            }
            None
        }
        let mut steps = path.split('.');
        let mut v = find(&env["data"]["members"], steps.next()?)?;
        for step in steps {
            v = ["members", "fields"]
                .iter()
                .flat_map(|k| v.get(*k).and_then(Value::as_array).into_iter().flatten())
                .find(|m| m.get("name").and_then(Value::as_str) == Some(step))?;
        }
        v.get("type").and_then(Value::as_str).map(str::to_string)
    }

    /// Every subrecord a deep `schema` envelope defines, at the top level, in a group or in
    /// the base-form component block (whose rows fall to "Other"), as (signature, type),
    /// in schema order (D7-fix review L1: the block's `Bool8` enums too).
    fn subrecords_in(env: &serde_json::Value) -> Vec<(String, String)> {
        use serde_json::Value;
        fn walk(members: &Value, out: &mut Vec<(String, String)>) {
            for m in members.as_array().into_iter().flatten() {
                if let Some(sig) = m.get("signature").and_then(Value::as_str) {
                    let ty = m.get("type").and_then(Value::as_str).unwrap_or_default();
                    out.push((sig.to_string(), ty.to_string()));
                } else if m.get("kind").and_then(Value::as_str) == Some("group") {
                    walk(&m["members"], out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&env["data"]["members"], &mut out);
        out
    }

    /// D2-fix review L2, widened by D7 review L1 and D7-fix review L1: for the seven member
    /// layouts (the six observed types and REFR), every mapped field path and every
    /// subrecord that can fall to "Other" (component blocks included) whose schema type is
    /// flags or an enum is in [`super::NOT_NUMBERS`], and
    /// nothing else is, so the edit dialog never offers raw bits or a raw enum value as a
    /// number to type.
    #[test]
    fn flags_and_enums_are_pinned_from_the_schema() {
        use super::{edits_as_number, NOT_NUMBERS};
        let engine = ReadOnlyEngine::new();
        let mut found: Vec<(String, String)> = Vec::new();
        for t in ["AMMO", "COBJ", "TERM", "CONT", "STAT", "KYWD", "REFR"] {
            let env = engine
                .run(&Request::new("schema", json!({"type": t, "deep": true})))
                .unwrap();
            let layout = shipped(t).unwrap().unwrap();
            let mapped = layout
                .tabs
                .iter()
                .flat_map(|tab| tab.groups.iter())
                .flat_map(|g| g.members.clone().unwrap_or_default());
            let mapped = mapped.map(|path| {
                let ty = leaf_type(&env, &path).unwrap_or_else(|| panic!("{t}: {path}"));
                (path, ty)
            });
            // A signature is read-only by name, so any of its definitions (top level, a
            // group, a component) that is flags or an enum puts it in.
            for (path, ty) in mapped.chain(subrecords_in(&env)) {
                let entry = (t.to_string(), path);
                if (ty.contains("flags<") || ty.contains("enum<")) && !found.contains(&entry) {
                    found.push(entry);
                }
            }
        }
        let pinned: Vec<(String, String)> = NOT_NUMBERS
            .iter()
            .map(|(t, p)| ((*t).to_string(), (*p).to_string()))
            .collect();
        assert_eq!(pinned, found);
        assert!(!edits_as_number("AMMO", "DNAM.Flags"));
        assert!(edits_as_number("AMMO", "DNAM.Damage"));
        assert!(edits_as_number("AMMO", "DATA.Value"));
        // D7 review M1: REFR's rotation is read-only, its position is not.
        assert!(!edits_as_number("REFR", "DATA.Rotation.Z"));
        assert!(edits_as_number("REFR", "DATA.Position.X"));
    }

    /// D2-fix review L3: a member page's lines follow the observed labels, mapped and
    /// unmapped interleaved (AMMO's "Edit", observed sixth, is the sixth line), each label
    /// once.
    #[test]
    fn a_member_page_follows_the_observed_order() {
        use super::{observed_lines, place_members, Line};
        for t in ["AMMO", "COBJ", "TERM", "CONT", "STAT", "KYWD"] {
            let o = observation(t).unwrap();
            let layout = shipped(t).unwrap().unwrap();
            let (pages, _) = place_members(&layout, &[]).unwrap();
            let lines = observed_lines(t, &pages[0]);
            let labels: Vec<&str> = lines
                .iter()
                .map(|l| match l {
                    Line::Mapped(i) => pages[0].groups[*i].label.as_str(),
                    Line::Gap(label, _) => label.as_str(),
                })
                .collect();
            assert_eq!(labels, o.labels, "{t}");
        }
        let o = observation("AMMO").unwrap();
        let (pages, _) = place_members(&shipped("AMMO").unwrap().unwrap(), &[]).unwrap();
        let lines = observed_lines("AMMO", &pages[0]);
        assert_eq!(o.labels[5], "Edit");
        assert!(matches!(&lines[5], Line::Gap(l, _) if l == "Edit"));
        assert!(matches!(lines[4], Line::Mapped(_)));
    }

    /// D7 (M7.W4, REFR): the layout follows the Reference dialog `ck-20261101-03` observed
    /// (`docs/ck-parity.md`, row "Reference dialog (a placed reference)", read here): its
    /// tabs are observed tab names in the observed order (those no schema member holds
    /// left out), "3D Data" first, opening with the four fields shown above the tabs in
    /// their observed order, then Position and Rotation X, Y, Z and Scale; every group
    /// names one field path REFR's schema has.
    #[test]
    fn the_refr_layout_follows_the_reference_dialog() {
        let parity = std::fs::read_to_string(repo().join("docs").join("ck-parity.md")).unwrap();
        let row = parity
            .lines()
            .find(|l| l.starts_with("| dialog | Reference dialog (a placed reference) |"))
            .expect("the Reference dialog row");
        let after = |key: &str| -> Vec<String> {
            let start = row.find(key).unwrap() + key.len();
            let rest = &row[start..];
            let end = rest.find(';').unwrap();
            rest[..end].split(", ").map(str::to_string).collect()
        };
        let observed_tabs = after("tabs ");
        let observed_fields = after("Observed: fields ");
        assert_eq!(observed_tabs.len(), 14, "{observed_tabs:?}");
        assert_eq!(
            observed_fields,
            [
                "Reference Editor ID",
                "Base Object",
                "Layer",
                "Location Override"
            ]
        );
        let l = shipped("REFR").unwrap().unwrap();
        assert_eq!(l.parity, "Reference dialog (a placed reference)");
        assert!(!l.observed, "the tabs' contents are mostly ours");
        let names: Vec<&str> = l.tabs.iter().map(|t| t.name.as_str()).collect();
        let mut at = 0;
        for n in &names {
            let i = observed_tabs
                .iter()
                .position(|o| o == n)
                .unwrap_or_else(|| panic!("`{n}` is no observed tab"));
            assert!(i >= at, "`{n}` out of the observed order");
            at = i;
        }
        assert_eq!(names[0], "3D Data");
        let first: Vec<&str> = l.tabs[0].groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(&first[..4], observed_fields.as_slice());
        assert_eq!(
            &first[4..],
            [
                "Position X",
                "Position Y",
                "Position Z",
                "Rotation X (radians)",
                "Rotation Y (radians)",
                "Rotation Z (radians)",
                "Scale"
            ]
        );
        let engine = ReadOnlyEngine::new();
        let env = engine
            .run(&Request::new(
                "schema",
                json!({"type": "REFR", "deep": true}),
            ))
            .unwrap();
        for t in &l.tabs {
            for g in &t.groups {
                let m = g.members.clone().unwrap_or_default();
                assert_eq!(m.len(), 1, "{}", g.name);
                assert!(schema_has(&env, &m[0]), "{}: {}", g.name, m[0]);
                assert_eq!(g.fields, [m[0].split('.').next().unwrap()], "{}", g.name);
            }
        }
    }
}
