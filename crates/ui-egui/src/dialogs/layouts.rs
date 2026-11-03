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
//! screenshots only). M7.K4 maps each label to a schema member; until then every label
//! is a gap the members test reports.

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
}

/// CK sitting 2's record of the edit dialogs (M7.W4).
pub const SITTING2_FILE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/layouts/sitting2.toml"
));

/// The sitting observation of `signature`'s edit dialog, when sitting 2 recorded one.
pub fn observation(signature: &str) -> Option<Observation> {
    let table: toml::Table = SITTING2_FILE.parse().ok()?;
    let o = table
        .get("dialog")?
        .as_array()?
        .iter()
        .filter_map(toml::Value::as_table)
        .find(|d| d.get("type").and_then(toml::Value::as_str) == Some(signature))?;
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
    let map = o
        .get("map")
        .and_then(toml::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(toml::Value::as_table)
                .filter_map(|m| {
                    let label = m.get("label")?.as_str()?.to_string();
                    let member = m.get("member")?.as_str()?.to_string();
                    Some((label, member))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Observation {
        sitting: s("sitting"),
        step: s("step"),
        form: s("form"),
        status: s("status"),
        tab_control: s("tab_control"),
        labels,
        map,
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
];

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
        assert_eq!(SHIPPED.len(), 8);
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
            // The tabs and groups are ours: a shipped layout says unobserved; the other
            // types take the default layout from the schema.
            if let Some(layout) = shipped(t) {
                assert!(!layout.unwrap().observed, "{t}: the tabs are ours");
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
    /// field path). Empty tonight: no label is mapped yet. A mapping lands in two places,
    /// the `[[dialog]]`'s `map` in `layouts/sitting2.toml` and here, so a mapping changed in
    /// one and not the other fails.
    const MAPPED: &[(&str, &[(&str, &str)])] = &[
        ("AMMO", &[]),
        ("COBJ", &[]),
        ("LCTN", &[]),
        ("TERM", &[]),
        ("CONT", &[]),
        ("STAT", &[]),
        ("KYWD", &[]),
    ];

    /// Whether the deep `schema` envelope of a type has the field path `path` (`SIG` or
    /// `SIG.Member`): the subrecord (top level or in a group), and the member named inside.
    fn schema_has(env: &serde_json::Value, path: &str) -> bool {
        fn names(v: &serde_json::Value, out: &mut Vec<String>) {
            for k in ["members", "fields"] {
                for m in v
                    .get(k)
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(n) = m.get("name").and_then(serde_json::Value::as_str) {
                        out.push(n.to_string());
                    }
                    names(m, out);
                }
            }
        }
        let (sig, member) = path.split_once('.').unwrap_or((path, ""));
        let is_sig = |m: &&serde_json::Value| {
            m.get("signature").and_then(serde_json::Value::as_str) == Some(sig)
        };
        let top: Vec<&serde_json::Value> = env["data"]["members"]
            .as_array()
            .into_iter()
            .flatten()
            .collect();
        let entries: Vec<&serde_json::Value> = top
            .iter()
            .copied()
            .filter(is_sig)
            .chain(
                top.iter()
                    .flat_map(|g| g["members"].as_array().into_iter().flatten())
                    .filter(is_sig),
            )
            .collect();
        !entries.is_empty()
            && (member.is_empty()
                || entries.iter().any(|e| {
                    let mut out = Vec::new();
                    names(e, &mut out);
                    out.iter().any(|n| n == member)
                }))
    }

    /// M7.W4's members check (review finding M1 of M7.W4): a label is mapped only by an
    /// explicit pair (label, field path) in its `[[dialog]]`'s `map`, which must name an
    /// observed label once and a field path the type's schema has; the pairs must equal
    /// the [`MAPPED`] pin. Every other label is a gap, named; a type whose dialog was not
    /// observed is a gap as a whole. The report is pinned in full.
    #[test]
    fn every_observed_label_is_mapped_or_reported_as_a_gap() {
        let engine = ReadOnlyEngine::new();
        let mut gaps = Vec::new();
        for (t, _) in SITTING2 {
            let o = observation(t).unwrap();
            if o.status != "observed" {
                assert!(o.map.is_empty(), "{t}: a mapping for an unobserved dialog");
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
            let env = engine
                .run(&Request::new("schema", json!({"type": t, "deep": true})))
                .unwrap();
            let mut seen: Vec<&String> = Vec::new();
            for (label, member) in &o.map {
                assert!(
                    o.labels.contains(label),
                    "{t}: `{label}` is no observed label"
                );
                assert!(!seen.contains(&label), "{t}: `{label}` mapped twice");
                seen.push(label);
                assert!(
                    schema_has(&env, member),
                    "{t}: `{label}` -> `{member}` is not a field of the schema"
                );
            }
            let unmapped: Vec<&str> = o
                .labels
                .iter()
                .filter(|l| !seen.contains(l))
                .map(String::as_str)
                .collect();
            if !unmapped.is_empty() {
                gaps.push(format!("{t}: not mapped (M7.K4): {}", unmapped.join(" | ")));
            }
        }
        eprintln!("M7.W4 gaps:\n  {}", gaps.join("\n  "));
        // The full report: WEAP unobserved, and every label of the seven named, since
        // M7.K4 has mapped none (the record's labels, held equal to it by the test above).
        let mut expected = vec!["WEAP: unobserved (ck-20261102-03 inconclusive)".to_string()];
        for (t, _) in SITTING2.iter().skip(1) {
            expected.push(format!(
                "{t}: not mapped (M7.K4): {}",
                observation(t).unwrap().labels.join(" | ")
            ));
        }
        assert_eq!(gaps, expected);
        assert!(gaps[1].starts_with("AMMO: not mapped (M7.K4): ID | Name | Short name | Value"));
        // The path check refuses what the review's probe used: a name that is no field
        // path, and a member the subrecord lacks.
        let kywd = engine
            .run(&Request::new(
                "schema",
                json!({"type": "KYWD", "deep": true}),
            ))
            .unwrap();
        assert!(schema_has(&kywd, "EDID"));
        assert!(schema_has(&kywd, "TNAM"));
        assert!(schema_has(&kywd, "CNAM.Red"));
        assert!(!schema_has(&kywd, "Keywords"));
        assert!(!schema_has(&kywd, "EditorID"));
        assert!(!schema_has(&kywd, "CNAM.Nothing"));
    }
}
