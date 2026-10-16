//! The edit-dialog framework (M7.W3): a layout per record type as data
//! (`crates/ui-egui/layouts/<TYPE>.toml`: tabs, groups, the subrecord signatures in each
//! group, and the `docs/ck-parity.md` row it reproduces), and a generic **read-only**
//! dialog that shows any record's typed fields from its `inspect` envelope grouped per
//! the layout. A type without a layout file gets a default one built from its `schema`
//! envelope (one group per schema group, the plain subrecords in schema order), marked
//! "default layout". Every layout is marked unobserved until CK sitting 2 (M7.O3)
//! records the CK's tabs and labels (M7.W4). Nothing here edits (M7.E2 waits on M7.O4).
//!
//! A subrecord the layout does not place still shows, on an "Other" tab: the dialog
//! never hides what the record holds. A signature named twice in a layout shows under
//! the first field that names it (the schema can place one signature in two groups;
//! placement by position is M7.W4's).

pub mod edit;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::panels::records::SubRow;

/// One group of fields on a tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutGroup {
    pub name: String,
    /// Subrecord signatures, in display order.
    pub fields: Vec<String>,
}

/// One tab of the dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutTab {
    pub name: String,
    pub groups: Vec<LayoutGroup>,
}

/// A record type's dialog layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Record signature (`WEAP`).
    pub record_type: String,
    /// The "CK element" of the `docs/ck-parity.md` row it reproduces (empty: none).
    pub parity: String,
    /// Whether the tabs and labels were observed in a CK sitting.
    pub observed: bool,
    /// `layouts/WEAP.toml`, or `default layout from schema`.
    pub source: String,
    pub tabs: Vec<LayoutTab>,
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

/// Most tabs, groups and fields a layout may have (a bound on hostile files).
const MAX_ITEMS: usize = 256;

/// Reads a layout file.
pub fn parse(text: &str, source: &str) -> Result<Layout, String> {
    let table: toml::Table = text.parse().map_err(|e| format!("{source}: {e}"))?;
    let s = |t: &toml::Table, k: &str| -> Result<String, String> {
        t.get(k)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("{source}: `{k}` must be a string"))
    };
    let arr = |t: &toml::Table, k: &str| -> Result<Vec<toml::Value>, String> {
        let a = t
            .get(k)
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{source}: `{k}` must be an array"))?;
        if a.len() > MAX_ITEMS {
            return Err(format!("{source}: more than {MAX_ITEMS} `{k}`"));
        }
        Ok(a.clone())
    };
    let record_type = s(&table, "type")?;
    if record_type.len() != 4 {
        return Err(format!("{source}: `type` must be a 4-character signature"));
    }
    let mut tabs = Vec::new();
    for tab in arr(&table, "tab")? {
        let tab = tab
            .as_table()
            .ok_or_else(|| format!("{source}: a [[tab]] must be a table"))?;
        let mut groups = Vec::new();
        for g in arr(tab, "group")? {
            let g = g
                .as_table()
                .ok_or_else(|| format!("{source}: a [[tab.group]] must be a table"))?;
            let fields = arr(g, "fields")?
                .iter()
                .map(|f| {
                    f.as_str()
                        .filter(|f| f.len() == 4)
                        .map(str::to_string)
                        .ok_or_else(|| format!("{source}: a field must be a 4-character signature"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            groups.push(LayoutGroup {
                name: s(g, "name")?,
                fields,
            });
        }
        tabs.push(LayoutTab {
            name: s(tab, "name")?,
            groups,
        });
    }
    Ok(Layout {
        record_type,
        parity: s(&table, "parity").unwrap_or_default(),
        observed: table
            .get("observed")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        source: source.to_string(),
        tabs,
    })
}

/// The shipped layout of `signature`, if there is one.
pub fn shipped(signature: &str) -> Option<Result<Layout, String>> {
    SHIPPED
        .iter()
        .find(|(t, _)| *t == signature)
        .map(|(t, text)| parse(text, &format!("layouts/{t}.toml")))
}

/// A `schema` envelope's top-level members as (name, signatures): a subrecord gives its
/// one signature, a group all of its own.
pub fn schema_members(env: &Value) -> Vec<(String, Vec<String>)> {
    let data = env.get("data").unwrap_or(&Value::Null);
    data.get("members")
        .and_then(Value::as_array)
        .map(|members| {
            members
                .iter()
                .filter_map(|m| {
                    let name = m.get("name").and_then(Value::as_str)?.to_string();
                    let sigs: Vec<String> = match m.get("signature").and_then(Value::as_str) {
                        Some(sig) => vec![sig.to_string()],
                        None => m
                            .get("signatures")
                            .and_then(Value::as_array)
                            .map(|a| {
                                a.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_string)
                                    .collect()
                            })
                            .unwrap_or_default(),
                    };
                    let group = m.get("kind").and_then(Value::as_str) == Some("group");
                    Some((if group { name } else { String::new() }, sigs))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Every signature a `schema` envelope defines at the top level or in a group.
pub fn schema_signatures(env: &Value) -> BTreeSet<String> {
    schema_members(env)
        .into_iter()
        .flat_map(|(_, sigs)| sigs)
        .collect()
}

/// The default layout of a type with no layout file: one tab, a "Record" group with the
/// plain subrecords in schema order, then one group per schema group.
pub fn default_layout(signature: &str, schema_env: &Value) -> Layout {
    let mut plain = Vec::new();
    let mut groups = Vec::new();
    for (group, sigs) in schema_members(schema_env) {
        if group.is_empty() {
            plain.extend(sigs);
        } else {
            groups.push(LayoutGroup {
                name: group,
                fields: sigs,
            });
        }
    }
    let mut all = vec![LayoutGroup {
        name: "Record".to_string(),
        fields: plain,
    }];
    all.extend(groups);
    Layout {
        record_type: signature.to_string(),
        parity: String::new(),
        observed: false,
        source: "default layout from schema".to_string(),
        tabs: vec![LayoutTab {
            name: "Fields".to_string(),
            groups: all,
        }],
    }
}

/// One tab as shown: its groups with the subrecords placed in each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedTab<'a> {
    pub name: String,
    pub groups: Vec<(String, Vec<&'a SubRow>)>,
}

/// The tab that holds what the layout does not place.
pub const OTHER_TAB: &str = "Other";

/// `subrows` grouped per `layout`. A subrecord goes to a field naming its signature: the
/// group of the subrecord before it when that group names it too (so a `FULL` after an
/// `OBTE` stays in the object-template group), else the first field naming it. Within a
/// group, fields follow the layout's order, except in a group where a signature repeats
/// (a block such as `OBTE FULL OBTF ...` per template), which keeps the record's order.
/// Empty groups and tabs are kept (the layout shows where a field would be); what no
/// field names goes to an [`OTHER_TAB`] tab at the end.
pub fn place<'a>(layout: &Layout, subrows: &'a [SubRow]) -> Vec<PlacedTab<'a>> {
    let mut claims: BTreeMap<&str, Vec<(usize, usize)>> = BTreeMap::new();
    for (ti, tab) in layout.tabs.iter().enumerate() {
        for (gi, g) in tab.groups.iter().enumerate() {
            for f in &g.fields {
                claims.entry(f.as_str()).or_default().push((ti, gi));
            }
        }
    }
    let mut out: Vec<PlacedTab<'a>> = layout
        .tabs
        .iter()
        .map(|t| PlacedTab {
            name: t.name.clone(),
            groups: t
                .groups
                .iter()
                .map(|g| (g.name.clone(), Vec::new()))
                .collect(),
        })
        .collect();
    let mut other = Vec::new();
    let mut last: Option<(usize, usize)> = None;
    for s in subrows {
        let Some(candidates) = claims.get(s.signature.as_str()) else {
            other.push(s);
            last = None;
            continue;
        };
        let at = last
            .filter(|l| candidates.contains(l))
            .or_else(|| candidates.first().copied());
        if let Some((ti, gi)) = at {
            if let Some(g) = out.get_mut(ti).and_then(|t| t.groups.get_mut(gi)) {
                g.1.push(s);
            }
        }
        last = at;
    }
    for (ti, t) in out.iter_mut().enumerate() {
        for (gi, g) in t.groups.iter_mut().enumerate() {
            let mut seen = BTreeSet::new();
            let repeats = g.1.iter().any(|s| !seen.insert(s.signature.as_str()));
            if repeats {
                continue;
            }
            let fields = layout
                .tabs
                .get(ti)
                .and_then(|t| t.groups.get(gi))
                .map(|g| g.fields.as_slice())
                .unwrap_or_default();
            g.1.sort_by_key(|s| {
                fields
                    .iter()
                    .position(|f| *f == s.signature)
                    .unwrap_or(usize::MAX)
            });
        }
    }
    if !other.is_empty() {
        out.push(PlacedTab {
            name: OTHER_TAB.to_string(),
            groups: vec![("Not placed by the layout".to_string(), other)],
        });
    }
    out
}

/// The dialog's state: the tab shown, the default layouts built so far (by type; `Err`
/// when the `schema` request failed), and which types have a `schema` request out.
#[derive(Default)]
pub struct DialogState {
    pub tab: usize,
    /// The record the tab index belongs to (a new record starts on the first tab).
    pub record: String,
    pub defaults: BTreeMap<String, Result<Layout, String>>,
    pub pending: BTreeSet<String>,
}

impl DialogState {
    /// The layout for `signature`: shipped, else the default built from its schema, else
    /// `None` (and the caller asks for the `schema` once).
    pub fn layout(&self, signature: &str) -> Option<Result<Layout, String>> {
        shipped(signature).or_else(|| self.defaults.get(signature).cloned())
    }

    /// The `schema` request a default layout for `signature` needs, once.
    pub fn schema_request(&mut self, signature: &str) -> Option<crate::gate::Request> {
        if signature.len() != 4
            || shipped(signature).is_some()
            || self.defaults.contains_key(signature)
            || self.pending.contains(signature)
        {
            return None;
        }
        self.pending.insert(signature.to_string());
        Some(crate::gate::Request::new(
            "schema",
            serde_json::json!({ "type": signature }),
        ))
    }

    /// Takes the `schema` reply for `signature`.
    pub fn receive_schema(&mut self, signature: &str, result: Result<&Value, String>) {
        self.pending.remove(signature);
        let layout = result.map(|env| default_layout(signature, env));
        self.defaults.insert(signature.to_string(), layout);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{check, ReadOnlyEngine, Request};
    use serde_json::json;
    use std::path::{Path, PathBuf};

    fn sub(sig: &str, member: &str, value: &str) -> SubRow {
        SubRow {
            signature: sig.into(),
            member: member.into(),
            value: value.into(),
            offset: Some(0),
            size: Some(4),
            confidence: None,
            source: None,
        }
    }

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf()
    }

    /// Every shipped layout parses, names its own type, cites a `dialog` row of
    /// `docs/ck-parity.md`, says it is unobserved, and names only signatures the record
    /// schema defines for that type (asked through the gate, as the window asks).
    #[test]
    fn every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row() {
        let parity = std::fs::read_to_string(repo().join("docs").join("ck-parity.md")).unwrap();
        let dialog_rows: Vec<String> = parity
            .lines()
            .filter(|l| l.starts_with("| dialog |"))
            .filter_map(|l| l.split('|').nth(2).map(|c| c.trim().to_string()))
            .collect();
        let engine = ReadOnlyEngine::new();
        assert_eq!(SHIPPED.len(), 8);
        for (t, _) in SHIPPED {
            let layout = shipped(t).unwrap().unwrap();
            assert_eq!(&layout.record_type, t);
            assert!(!layout.observed, "{t}: no sitting has observed it yet");
            assert!(
                dialog_rows.contains(&layout.parity),
                "{t}: parity `{}` is no dialog row of docs/ck-parity.md",
                layout.parity
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

    #[test]
    fn parse_refuses_what_is_not_a_layout() {
        assert!(parse("type = 5", "x").is_err());
        assert!(parse("type = \"WEAPON\"\ntab = []", "x").is_err());
        assert!(parse("type = \"WEAP\"\n[[tab]]\nname = \"A\"\n[[tab.group]]\nname = \"G\"\nfields = [\"TOOLONG\"]", "x").is_err());
        let ok = parse(
            "type = \"WEAP\"\n[[tab]]\nname = \"A\"\n[[tab.group]]\nname = \"G\"\nfields = [\"EDID\"]",
            "x",
        )
        .unwrap();
        assert_eq!(ok.tabs[0].groups[0].fields, vec!["EDID"]);
        assert!(!ok.observed && ok.parity.is_empty());
    }

    #[test]
    fn subrecords_are_placed_per_layout_and_the_rest_shows_on_other() {
        let layout = parse(
            "type = \"WEAP\"\n[[tab]]\nname = \"Object\"\n[[tab.group]]\nname = \"Identity\"\nfields = [\"EDID\", \"FULL\"]\n[[tab]]\nname = \"Keywords\"\n[[tab.group]]\nname = \"Keywords\"\nfields = [\"KSIZ\", \"KWDA\"]\n[[tab.group]]\nname = \"Empty\"\nfields = [\"APPR\"]",
            "t",
        )
        .unwrap();
        let rows = vec![
            sub("FULL", "Name", "Rifle"),
            sub("EDID", "EditorId", "Gun"),
            sub("KWDA", "Keywords", "[a, b]"),
            sub("DNAM", "", "hex 00"),
            sub("KSIZ", "KeywordCount", "2"),
        ];
        let placed = place(&layout, &rows);
        let names: Vec<&str> = placed.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["Object", "Keywords", OTHER_TAB]);
        let ident: Vec<&str> = placed[0].groups[0]
            .1
            .iter()
            .map(|s| s.signature.as_str())
            .collect();
        assert_eq!(
            ident,
            vec!["EDID", "FULL"],
            "layout order, not record order"
        );
        assert_eq!(placed[1].groups[1].1.len(), 0, "an empty group stays");
        assert_eq!(placed[2].groups[0].1[0].signature, "DNAM");

        // A signature in two groups follows the block it sits in.
        let layout = parse(
            "type = \"WEAP\"
[[tab]]
name = \"A\"
[[tab.group]]
name = \"Identity\"
fields = [\"EDID\", \"FULL\"]
[[tab.group]]
name = \"Templates\"
fields = [\"OBTE\", \"FULL\", \"OBTF\"]",
            "t",
        )
        .unwrap();
        let rows = vec![
            sub("EDID", "EditorId", "Gun"),
            sub("FULL", "Name", "Rifle"),
            sub("OBTE", "Count", "2"),
            sub("FULL", "Name", "Default"),
            sub("OBTF", "Flags", "0"),
            sub("FULL", "Name", "Scoped"),
        ];
        let placed = place(&layout, &rows);
        let sigs = |g: usize| -> Vec<&str> {
            placed[0].groups[g]
                .1
                .iter()
                .map(|s| s.value.as_str())
                .collect()
        };
        assert_eq!(sigs(0), vec!["Gun", "Rifle"]);
        assert_eq!(
            sigs(1),
            vec!["2", "Default", "0", "Scoped"],
            "record order in a block"
        );
    }

    #[test]
    fn a_type_without_a_file_gets_a_default_layout_from_its_schema() {
        let engine = ReadOnlyEngine::new();
        let env = engine
            .run(&Request::new("schema", json!({"type": "KYWD"})))
            .unwrap();
        let mut state = DialogState::default();
        assert!(state.layout("KYWD").is_none());
        let req = state.schema_request("KYWD").unwrap();
        assert!(check(&req).is_ok());
        assert!(state.schema_request("KYWD").is_none(), "asked once");
        assert!(state.schema_request("WEAP").is_none(), "WEAP has a file");
        state.receive_schema("KYWD", Ok(&env));
        let layout = state.layout("KYWD").unwrap().unwrap();
        assert_eq!(layout.source, "default layout from schema");
        assert!(!layout.observed);
        assert_eq!(layout.tabs[0].groups[0].name, "Record");
        assert_eq!(
            layout.tabs[0].groups[0].fields.first().map(String::as_str),
            Some("EDID")
        );
        let sigs = schema_signatures(&env);
        for g in &layout.tabs[0].groups {
            for f in &g.fields {
                assert!(sigs.contains(f));
            }
        }
    }
}
