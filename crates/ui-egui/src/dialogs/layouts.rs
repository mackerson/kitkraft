//! The layout files the binary ships (M7.W3 read-only dialogs; M7.W4, pipeline E, adds
//! the observed ones), and the test that holds each against the schema and its
//! `docs/ck-parity.md` row.
//!
//! Moved here from `dialogs/mod.rs` at step 0 of sprint 2026-11-02 (seam hole H5), so
//! M7.W4 can add a layout or mark one observed in E's own file.

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
mod tests {
    use super::super::{schema_signatures, shipped};
    use super::SHIPPED;
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
}
