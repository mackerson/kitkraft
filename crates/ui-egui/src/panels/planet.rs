//! Planet panel (M4.D3 `planet`): the Planet Content Manager chain of one record. A tree,
//! branch or content node shows its subtree indented by `depth`; a planet, star, biome,
//! location, worldspace or pack-in shows the content nodes that place it, each with its
//! branch chain. Every typed link anywhere in the result is listed by its marker
//! (`kind: "link"`), so a view the panel has no special rows for still shows its links.

use serde_json::{json, Map, Value};

use super::fmt_value;
use crate::envelope::{list, num, text, text_or_empty};
use crate::gate::{marker, Request, KIND};

/// Record types `planet` explains (the others are refused by the engine).
pub const PLANET_TYPES: &[&str] = &[
    "PCMT", "PCBN", "PCCN", "PNDT", "STDT", "BIOM", "LCTN", "WRLD", "PKIN", "LVLP",
];

/// The panel's inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetForm {
    /// Formref, EditorID, `runtime:<hex>`, or hex with `plugin`.
    pub form: String,
    /// Tree nodes per page.
    pub limit: u32,
}

impl Default for PlanetForm {
    fn default() -> Self {
        Self {
            form: String::new(),
            limit: 200,
        }
    }
}

impl PlanetForm {
    /// The `planet` request for a page of tree nodes.
    pub fn request(&self, cursor: Option<&str>) -> Result<Request, String> {
        let form = self.form.trim();
        if form.is_empty() {
            return Err("give a formref or EditorID of a PCM tree, branch, content node, planet, star, biome, location, worldspace or pack-in".to_string());
        }
        let mut args = Map::new();
        args.insert("form".into(), json!(form));
        args.insert("limit".into(), json!(self.limit.clamp(1, 5000)));
        if let Some(c) = cursor {
            args.insert("cursor".into(), json!(c));
        }
        Ok(Request::new("planet", Value::Object(args)))
    }
}

/// A link marker as one line: `PATH -> target SIG EditorID` or the raw value and status.
pub fn link_text(l: &Value) -> String {
    let target = match text(l, "to") {
        Some(to) => format!(
            "{to} {} {}",
            text_or_empty(l, "to_signature"),
            text_or_empty(l, "to_editor_id")
        )
        .trim_end()
        .to_string(),
        None => format!(
            "{} ({})",
            text_or_empty(l, "raw"),
            text_or_empty(l, "status")
        ),
    };
    format!("{} -> {target}", text_or_empty(l, "path"))
}

/// The marked link at `v`, if `v` is one.
fn as_link(v: Option<&Value>) -> Option<&Value> {
    v.filter(|l| l.get(KIND).and_then(Value::as_str) == Some(marker::LINK))
}

/// One tree node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRow {
    /// Depth below the tree's root (an ancestor's own depth in its chain).
    pub depth: u64,
    /// The node, as a formref.
    pub form: String,
    /// `SIG EditorID`.
    pub label: String,
    /// How the `PRVN` chain placed it (`root`, `first`, `chained`, ...).
    pub order: String,
    /// Its child selection, as stored.
    pub selection: String,
    /// What a content node places (the `PCCC` link), and the link's target formref.
    pub content: Option<(String, Option<String>)>,
    /// Conditions as stored.
    pub conditions: usize,
    /// The node could not be read.
    pub unreadable: bool,
}

/// One content node that places the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementRow {
    /// Where in the result (`placements`, `planets[0].placements`).
    pub at: String,
    /// The content node.
    pub node: String,
    /// Its EditorID.
    pub editor_id: String,
    /// The tree it belongs to.
    pub tree: String,
    /// The branches above it, root last: `EditorID (selection n, k conditions)`.
    pub chain: Vec<String>,
}

/// One typed link anywhere in the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRow {
    /// JSON path of the link in `data` (`planet.biomes[0]`).
    pub at: String,
    /// `PATH -> target`.
    pub text: String,
    /// The target formref, when it resolves.
    pub to: Option<String>,
    /// A status other than `resolved` or `null`.
    pub problem: bool,
}

/// A `planet` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanetView {
    /// `tree`, `planet`, `star`, `biome`, `location`, `worldspace`, `pack-in`.
    pub kind: String,
    /// The record, as a formref.
    pub global: String,
    /// `SIG EditorID`.
    pub label: String,
    /// Tree view: the root.
    pub tree: Option<String>,
    /// Tree view: the chain above a branch or content node, root first.
    pub ancestors: Vec<NodeRow>,
    /// Tree view: the subtree page.
    pub nodes: Vec<NodeRow>,
    /// Tree view: `n nodes, b branches, c content nodes, depth d`.
    pub counts: Option<String>,
    /// Tree view: links that name nothing usable, as text.
    pub unresolved: Vec<String>,
    /// Content nodes placing the record (every view but the tree).
    pub placements: Vec<PlacementRow>,
    /// Every link marker in the result.
    pub links: Vec<LinkRow>,
    /// Cursor of the next page of nodes.
    pub next_cursor: Option<String>,
}

impl PlanetView {
    /// Reads a `planet` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let mut links = Vec::new();
        collect_links(data, "", &mut links, 0);
        let mut placements = Vec::new();
        collect_placements(data, "", &mut placements, 0);
        let counts = data.get("counts").filter(|c| c.is_object()).map(|c| {
            let by = c
                .get("by_content")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| format!("{k} {}", crate::envelope::short(v)))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            format!(
                "{} node(s): {} branch(es), {} content node(s), max depth {}; content: {by}",
                num(c, "nodes").unwrap_or(0),
                num(c, "branches").unwrap_or(0),
                num(c, "content_nodes").unwrap_or(0),
                num(c, "max_depth").unwrap_or(0)
            )
        });
        let unresolved = list(data, "unresolved")
            .iter()
            .map(|u| {
                format!(
                    "{} {} {} = {} ({}){}",
                    text_or_empty(u, "node"),
                    text_or_empty(u, "editor_id"),
                    text_or_empty(u, "path"),
                    text_or_empty(u, "raw"),
                    text_or_empty(u, "status"),
                    text(u, "error").map_or_else(String::new, |e| format!(": {e}"))
                )
            })
            .collect();
        Self {
            kind: text_or_empty(data, "kind"),
            global: text_or_empty(data, "global"),
            label: format!(
                "{} {}",
                text_or_empty(data, "signature"),
                text_or_empty(data, "editor_id")
            )
            .trim_end()
            .to_string(),
            tree: text(data, "tree"),
            ancestors: list(data, "ancestors").iter().map(node_row).collect(),
            nodes: list(data, "nodes").iter().map(node_row).collect(),
            counts,
            unresolved,
            placements,
            links,
            next_cursor: text(data, "next_cursor"),
        }
    }
}

fn node_row(n: &Value) -> NodeRow {
    let content = as_link(n.get("content")).map(|l| (link_text(l), text(l, "to")));
    NodeRow {
        depth: num(n, "depth").unwrap_or(0),
        form: text_or_empty(n, "form"),
        label: format!(
            "{} {}",
            text_or_empty(n, "signature"),
            text_or_empty(n, "editor_id")
        )
        .trim_end()
        .to_string(),
        order: text_or_empty(n, "order"),
        selection: n
            .get("child_selection")
            .filter(|v| !v.is_null())
            .map(fmt_value)
            .unwrap_or_default(),
        content,
        conditions: list(n, "conditions").len(),
        unreadable: n.get("readable") == Some(&Value::Bool(false)),
    }
}

/// Every `link` marker under `v`, with its JSON path.
fn collect_links(v: &Value, path: &str, out: &mut Vec<LinkRow>, depth: u8) {
    if depth > 24 {
        return;
    }
    let next = depth.saturating_add(1);
    match v {
        Value::Object(m) => {
            if as_link(Some(v)).is_some() {
                let status = text_or_empty(v, "status");
                out.push(LinkRow {
                    at: path.to_string(),
                    text: link_text(v),
                    to: text(v, "to"),
                    problem: !matches!(status.as_str(), "resolved" | "null"),
                });
                return;
            }
            for (k, item) in m {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                collect_links(item, &p, out, next);
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                collect_links(item, &format!("{path}[{i}]"), out, next);
            }
        }
        _ => {}
    }
}

/// Every `placements` list under `v` (top level, or per planet of a star).
fn collect_placements(v: &Value, path: &str, out: &mut Vec<PlacementRow>, depth: u8) {
    if depth > 8 {
        return;
    }
    let next = depth.saturating_add(1);
    match v {
        Value::Object(m) => {
            for (k, item) in m {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                if k == "placements" {
                    for pl in item.as_array().map_or(&[][..], Vec::as_slice) {
                        out.push(PlacementRow {
                            at: p.clone(),
                            node: text_or_empty(pl, "node"),
                            editor_id: text_or_empty(pl, "editor_id"),
                            tree: text_or_empty(pl, "tree"),
                            chain: list(pl, "chain")
                                .iter()
                                .map(|c| {
                                    format!(
                                        "{} (selection {}, {} condition(s))",
                                        text(c, "editor_id")
                                            .unwrap_or_else(|| text_or_empty(c, "form")),
                                        c.get("child_selection")
                                            .filter(|v| !v.is_null())
                                            .map_or_else(|| "?".to_string(), fmt_value),
                                        list(c, "conditions").len()
                                    )
                                })
                                .collect(),
                        });
                    }
                } else {
                    collect_placements(item, &p, out, next);
                }
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                collect_placements(item, &format!("{path}[{i}]"), out, next);
            }
        }
        _ => {}
    }
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, CollapsingHeader, TextEdit};

use crate::panels::common::{
    envelope_block, link, record_actions, virtual_table, Action, Slot, TabCtx,
};
use crate::theme::{self, Field, Status, View};

/// The Planet tab's state: the form, the last `planet` page and its cursor.
#[derive(Default)]
pub(crate) struct PlanetTab {
    pub form: PlanetForm,
    pub view: Option<PlanetView>,
    pub cursor: Option<String>,
}

impl PlanetTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let actions = &mut ctx.actions;
        ui.heading("Planet");
        ui.label("The Planet Content Manager chain of one record (`planet`): a tree, branch or content node shows its subtree; a planet, star, biome, location, worldspace or pack-in the content nodes that place it. Conditions are shown as stored and never evaluated.");
        ui.horizontal_wrapped(|ui| {
            ui.label("form");
            ui.add(
                TextEdit::singleline(&mut self.form.form)
                    .desired_width(theme::width(ui, Field::Medium))
                    .hint_text("PCM_QuestRequest, Plugin.esm:XXXXXX"),
            );
            ui.label("nodes per page");
            ui.add(egui::DragValue::new(&mut self.form.limit).range(1..=5000));
            if ui.add_enabled(!busy, egui::Button::new("Show")).clicked() {
                actions.push(Action::PlanetPage(None));
            }
        });
        envelope_block(ui, ctx.results.get(&Slot::Planet.into()), |ui| {
            let Some(view) = &self.view else {
                return;
            };
            ui.horizontal(|ui| {
                ui.strong(format!("{} {}  [{}]", view.global, view.label, view.kind));
                record_actions(ui, busy, &view.global, actions);
            });
            if let Some(tree) = &view.tree {
                ui.horizontal(|ui| {
                    ui.label("tree");
                    if tree != &view.global && link(ui, !busy, tree).clicked() {
                        actions.push(Action::Planet(tree.clone()));
                    } else if tree == &view.global {
                        ui.monospace(tree);
                    }
                });
            }
            if let Some(c) = &view.counts {
                ui.label(c);
            }
            for u in &view.unresolved {
                theme::status(ui, Status::Fail, &format!("unresolved: {u}"));
            }
            ui.horizontal(|ui| {
                if self.cursor.is_some()
                    && ui
                        .add_enabled(!busy, egui::Button::new("First page"))
                        .clicked()
                {
                    actions.push(Action::PlanetPage(None));
                }
                if let Some(next) = &view.next_cursor {
                    if ui
                        .add_enabled(!busy, egui::Button::new("Next page"))
                        .clicked()
                    {
                        actions.push(Action::PlanetPage(Some(next.clone())));
                    }
                }
            });
            for (title, rows) in [("ancestors", &view.ancestors), ("nodes", &view.nodes)] {
                if rows.is_empty() {
                    continue;
                }
                ui.strong(format!("{title} ({})", rows.len()));
                virtual_table(
                    ui,
                    ("planet-grid", title),
                    &["node", "", "order", "selection", "conditions", "content"],
                    rows.len(),
                    View::Tall,
                    |row, i| {
                        let Some(n) = rows.get(i) else { return };
                        let indent = "    ".repeat(usize::try_from(n.depth.min(12)).unwrap_or(0));
                        row.cell(|ui| {
                            if link(ui, !busy, format!("{indent}{} {}", n.form, n.label))
                                .on_hover_text("planet chain of this node")
                                .clicked()
                            {
                                actions.push(Action::Planet(n.form.clone()));
                            }
                        });
                        row.cell(|ui| record_actions(ui, busy, &n.form, actions));
                        row.cell(|ui| {
                            if n.unreadable {
                                theme::status(ui, Status::Fail, "not readable")
                            } else {
                                ui.label(&n.order)
                            }
                        });
                        row.cell(|ui| ui.label(&n.selection));
                        row.cell(|ui| ui.label(n.conditions.to_string()));
                        row.cell(|ui| match &n.content {
                            Some((text, Some(to))) => {
                                if link(ui, !busy, text)
                                    .on_hover_text("inspect the content")
                                    .clicked()
                                {
                                    actions.push(Action::Inspect(to.clone()));
                                }
                            }
                            Some((text, None)) => {
                                theme::status(ui, Status::Fail, text);
                            }
                            None => {
                                ui.label("");
                            }
                        });
                    },
                );
            }
            if !view.placements.is_empty() {
                ui.strong(format!(
                    "placed by {} content node(s)",
                    view.placements.len()
                ));
                theme::table(ui, "planet-placements").show(ui, |ui| {
                    for h in ["at", "content node", "tree", "chain (nearest first)"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for p in &view.placements {
                        ui.weak(&p.at);
                        if link(ui, !busy, format!("{} {}", p.node, p.editor_id))
                            .on_hover_text("planet chain of this node")
                            .clicked()
                        {
                            actions.push(Action::Planet(p.node.clone()));
                        }
                        if p.tree.is_empty() {
                            ui.label("");
                        } else if link(ui, !busy, &p.tree).clicked() {
                            actions.push(Action::Planet(p.tree.clone()));
                        }
                        ui.label(p.chain.join(" <- "));
                        ui.end_row();
                    }
                });
            }
            CollapsingHeader::new(format!("every typed link ({})", view.links.len()))
                .id_salt("planet-links")
                .default_open(view.nodes.is_empty() && view.placements.is_empty())
                .show(ui, |ui| {
                    theme::table(ui, "planet-links-grid").show(ui, |ui| {
                        for h in ["where", "link", ""] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for l in &view.links {
                            ui.weak(&l.at);
                            match (&l.to, l.problem) {
                                (Some(to), false) => {
                                    if link(ui, !busy, &l.text).clicked() {
                                        actions.push(Action::Inspect(to.clone()));
                                    }
                                    record_actions(ui, busy, to, actions);
                                }
                                _ => {
                                    if l.problem {
                                        theme::status(ui, Status::Fail, &l.text);
                                    } else {
                                        ui.weak(&l.text);
                                    }
                                    ui.label("");
                                }
                            }
                            ui.end_row();
                        }
                    });
                });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{check_shape, data_shape};

    fn link(path: &str, raw: &str, status: &str, to: Option<(&str, &str, &str)>) -> Value {
        json!({"kind": "link", "path": path, "raw": raw, "status": status,
            "to": to.map(|t| t.0), "to_signature": to.map(|t| t.1), "to_editor_id": to.map(|t| t.2),
            "targets": ["WRLD", "PKIN", "LVLP"], "target_allowed": to.map(|_| true),
            "resolved_by": to.map(|_| "direct"),
            "error": if to.is_none() { json!("no master at index 05") } else { Value::Null },
            "offset": 300, "body_offset": 12})
    }

    fn node(
        form: &str,
        sig: &str,
        edid: &str,
        depth: u64,
        parent: Option<&str>,
        order: &str,
        content: Value,
    ) -> Value {
        json!({"form": form, "signature": sig, "editor_id": edid, "depth": depth, "parent": parent,
            "order": order, "node_type": 0, "child_selection": 1, "consume_request_on_failure": null,
            "count_curve": null, "distribution_curve": null,
            "parent_link": parent.map(|p| link("PRTN", "00000808", "resolved", Some((p, "PCMT", "TreeA")))),
            "previous": null, "content": content, "override_content_properties": false,
            "keywords": [], "conditions": [{"Function": 560, "Parameter1": {"kind": "opaque", "opaque": "01080000"}, "member": "Conditions.CTDA"}]})
    }

    fn tree() -> Value {
        json!({"command": "planet", "data": {
            "form": "TreeA", "matched_by": "editor_id", "global": "Base.esm:000808", "signature": "PCMT",
            "editor_id": "TreeA", "kind": "tree", "tree": "Base.esm:000808", "ancestors": [],
            "default_objects": [],
            "nodes": [
                node("Base.esm:000808", "PCMT", "TreeA", 0, None, "root", Value::Null),
                node("Base.esm:00080A", "PCCN", "ContentWorld", 1, Some("Base.esm:000808"), "first",
                     link("PCCC", "00000803", "resolved", Some(("Base.esm:000803", "WRLD", "WorldA")))),
                node("Base.esm:00080C", "PCCN", "ContentBad", 1, Some("Base.esm:000808"), "chained",
                     link("PCCC", "05000001", "unresolvable", None)),
                {"form": "Base.esm:0008FF", "depth": 1, "parent": "Base.esm:000808", "readable": false}
            ],
            "counts": {"nodes": 4, "branches": 0, "content_nodes": 2, "max_depth": 1, "by_content": {"WRLD": 1, "unresolvable": 1}},
            "total": 4,
            "unresolved": [{"node": "Base.esm:00080C", "editor_id": "ContentBad", "path": "PCCC", "raw": "05000001",
                            "status": "unresolvable", "to": null, "error": "no master at index 05"}],
            "target_mismatches": [],
            "next_cursor": null, "records_read": 4, "tree_nodes_indexed": 3, "elapsed_ms": 2, "summary": "TreeA"},
            "coverage": {"status": "partial", "decoded_subrecords": null, "opaque_subrecords": 0, "not_examined": []},
            "limitations": [], "evidence": []})
    }

    fn star() -> Value {
        let placement = json!({"node": "Base.esm:00080A", "editor_id": "ContentWorld",
            "content": link("PCCC", "00000803", "resolved", Some(("Base.esm:000803", "WRLD", "WorldA"))),
            "tree": "Base.esm:000808",
            "chain": [{"form": "Base.esm:000809", "signature": "PCBN", "editor_id": "BranchA", "child_selection": 0, "conditions": [{"Function": 560}]},
                      {"form": "Base.esm:000808", "signature": "PCMT", "editor_id": "TreeA", "child_selection": 1, "conditions": []}]});
        json!({"command": "planet", "data": {
            "form": "StarA", "matched_by": "editor_id", "global": "Base.esm:000805", "signature": "STDT",
            "editor_id": "StarA", "kind": "star",
            "star": {"system_id": 7, "links": [link("PNAM", "00000806", "resolved", Some(("Base.esm:000806", "PNDT", "PlanetA")))]},
            "planets": [{"planet": {"form": "Base.esm:000806", "signature": "PNDT", "editor_id": "PlanetA"},
                         "worldspaces": ["Base.esm:000803"], "placements": [placement]}],
            "next_cursor": null, "records_read": 6, "tree_nodes_indexed": 3, "elapsed_ms": 2, "summary": "StarA"},
            "coverage": {"status": "partial", "decoded_subrecords": null, "opaque_subrecords": 0, "not_examined": []},
            "limitations": [], "evidence": []})
    }

    #[test]
    fn canned_envelopes_follow_the_engines_documented_shape() {
        for env in [tree(), star()] {
            let errs = check_shape(data_shape("planet").unwrap(), &env["data"], "data");
            assert!(errs.is_empty(), "{errs:#?}");
        }
    }

    #[test]
    fn a_tree_is_nodes_by_depth_with_their_content_links() {
        let v = PlanetView::from_envelope(&tree());
        assert_eq!(v.kind, "tree");
        assert_eq!(v.label, "PCMT TreeA");
        assert_eq!(v.nodes.len(), 4);
        assert_eq!(v.nodes[0].order, "root");
        assert_eq!(v.nodes[0].selection, "1");
        assert_eq!(v.nodes[0].conditions, 1);
        let world = &v.nodes[1];
        assert_eq!(world.depth, 1);
        assert_eq!(
            world.content,
            Some((
                "PCCC -> Base.esm:000803 WRLD WorldA".to_string(),
                Some("Base.esm:000803".to_string())
            ))
        );
        assert_eq!(
            v.nodes[2].content,
            Some(("PCCC -> 05000001 (unresolvable)".to_string(), None))
        );
        assert!(v.nodes[3].unreadable);
        assert_eq!(
            v.counts.as_deref(),
            Some("4 node(s): 0 branch(es), 2 content node(s), max depth 1; content: WRLD 1, unresolvable 1")
        );
        assert_eq!(v.unresolved.len(), 1);
        // Every link marker, wherever it sits: two parent links and two contents.
        assert_eq!(v.links.len(), 4);
        assert!(v
            .links
            .iter()
            .any(|l| l.at == "nodes[2].content" && l.problem));
        assert!(v.placements.is_empty());
    }

    #[test]
    fn a_star_lists_its_planets_placements_and_links() {
        let v = PlanetView::from_envelope(&star());
        assert_eq!(v.kind, "star");
        assert!(v.nodes.is_empty());
        assert_eq!(v.placements.len(), 1);
        let p = &v.placements[0];
        assert_eq!(p.at, "planets[0].placements");
        assert_eq!(p.editor_id, "ContentWorld");
        assert_eq!(
            p.chain,
            vec![
                "BranchA (selection 0, 1 condition(s))",
                "TreeA (selection 1, 0 condition(s))"
            ]
        );
        let at: Vec<&str> = v.links.iter().map(|l| l.at.as_str()).collect();
        assert_eq!(
            at,
            vec!["planets[0].placements[0].content", "star.links[0]"]
        );
        assert_eq!(v.links[1].to.as_deref(), Some("Base.esm:000806"));
    }

    #[test]
    fn requests() {
        let f = PlanetForm {
            form: " TreeA ".into(),
            limit: 9000,
        };
        assert_eq!(
            f.request(Some("p2")).unwrap().args,
            json!({"form": "TreeA", "limit": 5000, "cursor": "p2"})
        );
        assert!(PlanetForm::default().request(None).is_err());
        assert!(PLANET_TYPES.contains(&"PNDT"));
    }
}
