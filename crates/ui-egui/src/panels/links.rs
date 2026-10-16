//! Links panel (M4.D1 `refs`): a form's outgoing typed FormID links, breadth first to a
//! depth, or its incoming referrers (the CK's "Use Info"), paged by `next_cursor`. Rows
//! are read by their marker (`kind: "link"` for an outgoing edge, `kind: "referrer"` for
//! an incoming one; `docs/mcp.md`, "Data shapes and markers"), never by their shape.

use serde_json::{json, Map, Value};

use crate::envelope::{list, num, text, text_or_empty};
use crate::gate::{marker, Request, KIND};

/// Deepest walk the form offers (`refs` allows 6).
pub const MAX_DEPTH: u32 = 6;

/// The panel's inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinksForm {
    /// Formref, EditorID, `runtime:<hex>`, or hex with `plugin`.
    pub form: String,
    /// Incoming referrers ("Use Info") instead of outgoing links.
    pub incoming: bool,
    /// Hops (1..=6).
    pub depth: u32,
    /// Every version of a referrer, not only winners (incoming).
    pub all_versions: bool,
    /// Record types to keep, comma or space separated (empty: all).
    pub types: String,
    /// Page size.
    pub limit: u32,
}

impl Default for LinksForm {
    fn default() -> Self {
        Self {
            form: String::new(),
            incoming: false,
            depth: 1,
            all_versions: false,
            types: String::new(),
            limit: 100,
        }
    }
}

impl LinksForm {
    /// The form's outgoing links at depth 1 ("Outgoing links" on a record row).
    pub fn outgoing(form: &str) -> Self {
        Self {
            form: form.to_string(),
            ..Self::default()
        }
    }

    /// The form's referrers ("Use info" on a record row).
    pub fn use_info(form: &str) -> Self {
        Self {
            form: form.to_string(),
            incoming: true,
            ..Self::default()
        }
    }

    /// The `refs` request for a page (`cursor` from the previous page).
    pub fn request(&self, cursor: Option<&str>) -> Result<Request, String> {
        let form = self.form.trim();
        if form.is_empty() {
            return Err("give a formref, EditorID or runtime:<hex>".to_string());
        }
        let mut args = Map::new();
        args.insert("form".into(), json!(form));
        if self.incoming {
            args.insert("incoming".into(), json!(true));
            if self.all_versions {
                args.insert("scope".into(), json!("all"));
            }
        }
        args.insert("depth".into(), json!(self.depth.clamp(1, MAX_DEPTH)));
        let types: Vec<String> = self
            .types
            .split([',', ' '])
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_ascii_uppercase)
            .collect();
        if !types.is_empty() {
            args.insert("types".into(), json!(types));
        }
        args.insert("limit".into(), json!(self.limit.clamp(1, 1000)));
        if let Some(c) = cursor {
            args.insert("cursor".into(), json!(c));
        }
        Ok(Request::new("refs", Value::Object(args)))
    }
}

/// One edge of a `refs` page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeRow {
    /// Hop (1 = the form's own links or referrers).
    pub depth: u64,
    /// The record holding the link (outgoing: the form or one reached; incoming: the
    /// referrer), as a formref when it resolves.
    pub from: Option<String>,
    /// `SIG EditorID` of `from` (or the referrer's raw FormID when it does not resolve).
    pub from_label: String,
    /// Member path of the link (`KWDA.Forms[3]`); empty when the referrer's schema walk
    /// places no member.
    pub path: String,
    /// FormID as stored.
    pub raw: String,
    /// `resolved`, `null`, `none`, `missing`, `engine_reserved`, `unresolvable`.
    pub status: String,
    /// The named form (outgoing), as a formref when it resolves.
    pub to: Option<String>,
    /// `SIG EditorID` of `to`, or the error.
    pub to_label: String,
    /// Plugin holding the version read.
    pub plugin: String,
    /// Incoming: the placed referrer's cell and world.
    pub placed: Option<String>,
    /// Incoming: the referrer version is the winner.
    pub winner: bool,
    /// Outgoing: the target's type is not one the definition lists.
    pub target_mismatch: bool,
}

impl EdgeRow {
    /// A link that names nothing usable (missing, unresolvable, engine-reserved, none).
    pub fn problem(&self) -> bool {
        !matches!(self.status.as_str(), "resolved" | "null" | "")
    }

    /// The form a click on the row opens: the target for an outgoing link, the referrer
    /// for an incoming one.
    pub fn open(&self, incoming: bool) -> Option<&str> {
        if incoming {
            self.from.as_deref()
        } else {
            self.to.as_deref()
        }
    }
}

/// A `refs` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinksView {
    /// `outgoing` or `incoming`.
    pub direction: String,
    /// The form, as a formref.
    pub global: String,
    /// `SIG EditorID` of the form.
    pub label: String,
    /// Edges (outgoing) or referrers (incoming) in total.
    pub total: u64,
    /// Counts by status (outgoing) or by referrer type and placement (incoming).
    pub counts: String,
    /// Edges on this page.
    pub edges: Vec<EdgeRow>,
    /// Edges whose marker this view does not know (shown as a count, never guessed).
    pub unrecognized: usize,
    /// The walk stopped at its cap.
    pub truncated: bool,
    /// Cursor of the next page.
    pub next_cursor: Option<String>,
    /// Incoming: the largest cells holding placed referrers, `world / cell: n`.
    pub placed_by_cell: Vec<String>,
    /// Incoming: what the reverse index could not search.
    pub not_searched: Option<String>,
}

impl LinksView {
    /// Whether the rows are referrers.
    pub fn incoming(&self) -> bool {
        self.direction == "incoming"
    }

    /// Reads a `refs` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let label = |sig: Option<String>, edid: Option<String>| {
            format!(
                "{} {}",
                sig.unwrap_or_else(|| "?".into()),
                edid.unwrap_or_default()
            )
            .trim_end()
            .to_string()
        };
        let mut unrecognized: usize = 0;
        let edges = list(data, "edges")
            .iter()
            .filter_map(|e| match e.get(KIND).and_then(Value::as_str) {
                Some(k) if k == marker::LINK => Some(EdgeRow {
                    depth: num(e, "depth").unwrap_or(1),
                    from: text(e, "from"),
                    from_label: label(text(e, "from_signature"), text(e, "from_editor_id")),
                    path: text_or_empty(e, "path"),
                    raw: text_or_empty(e, "raw"),
                    status: text_or_empty(e, "status"),
                    to: text(e, "to"),
                    to_label: match (text(e, "error"), e.get("to")) {
                        (Some(err), _) => err,
                        (None, Some(Value::String(_))) => {
                            label(text(e, "to_signature"), text(e, "to_editor_id"))
                        }
                        // The null (`00000000`) and none (`FFFFFFFF`) values name no form.
                        (None, _) => text_or_empty(e, "status").to_ascii_uppercase(),
                    },
                    plugin: text_or_empty(e, "plugin"),
                    placed: None,
                    winner: true,
                    target_mismatch: e.get("target_allowed") == Some(&Value::Bool(false)),
                }),
                Some(k) if k == marker::REFERRER => {
                    let from = text(e, "from");
                    let from_label = match &from {
                        Some(_) => label(text(e, "from_signature"), text(e, "from_editor_id")),
                        None => format!(
                            "{} raw {} (does not resolve)",
                            text_or_empty(e, "from_signature"),
                            text_or_empty(e, "from_raw")
                        ),
                    };
                    Some(EdgeRow {
                        depth: num(e, "depth").unwrap_or(1),
                        from,
                        from_label,
                        path: text_or_empty(e, "path"),
                        raw: text_or_empty(e, "raw"),
                        status: text_or_empty(e, "status"),
                        to: text(e, "to"),
                        to_label: String::new(),
                        plugin: text_or_empty(e, "plugin"),
                        placed: e.get("placed").and_then(placed_text),
                        winner: e.get("winner").and_then(Value::as_bool).unwrap_or(false),
                        target_mismatch: false,
                    })
                }
                _ => {
                    unrecognized = unrecognized.saturating_add(1);
                    None
                }
            })
            .collect();
        let counts = data.get("counts").unwrap_or(&Value::Null);
        let counts = if let Some(by) = counts.get("by_status").and_then(Value::as_object) {
            by.iter()
                .map(|(k, v)| format!("{} {k}", crate::envelope::short(v)))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            let by_sig = counts
                .get("by_signature")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| format!("{k} {}", crate::envelope::short(v)))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            format!(
                "{} referrer(s), {} placed; {by_sig}",
                num(counts, "referrers").unwrap_or(0),
                num(counts, "placed").unwrap_or(0)
            )
        };
        let placed_by_cell = list(data, "placed_by_cell")
            .iter()
            .map(|c| {
                format!(
                    "{} / {}: {}",
                    text(c, "world").unwrap_or_else(|| "(interior)".into()),
                    text_or_empty(c, "cell"),
                    num(c, "count").unwrap_or(0)
                )
            })
            .collect();
        let not_searched = data.get("not_searched").map(|n| {
            let types = list(n, "no_definition")
                .iter()
                .map(|t| {
                    format!(
                        "{} ({})",
                        text_or_empty(t, "type"),
                        num(t, "records").unwrap_or(0)
                    )
                })
                .collect::<Vec<_>>();
            format!(
                "not searched: {} record type(s) without a definition{}{}; {} record(s) with raw subrecords",
                types.len(),
                if types.is_empty() { "" } else { ": " },
                types.join(", "),
                num(n, "records_with_raw_subrecords").unwrap_or(0)
            )
        });
        Self {
            direction: text_or_empty(data, "direction"),
            global: text_or_empty(data, "global"),
            label: label(text(data, "signature"), text(data, "editor_id")),
            total: num(data, "total").unwrap_or(0),
            counts,
            edges,
            unrecognized,
            truncated: data.get("truncated") == Some(&Value::Bool(true)),
            next_cursor: text(data, "next_cursor"),
            placed_by_cell,
            not_searched,
        }
    }
}

/// `cell <formref> EditorID in world <formref> (persistent)` of a placed referrer; the
/// cell and world are `formref` markers.
fn placed_text(p: &Value) -> Option<String> {
    let place = |v: &Value| {
        if v.get(KIND).and_then(Value::as_str) != Some(marker::FORMREF) {
            return None;
        }
        let name = match text(v, "form") {
            Some(f) => format!("{f} {}", text_or_empty(v, "editor_id"))
                .trim_end()
                .to_string(),
            None => format!("raw {} (unresolved)", text_or_empty(v, "raw")),
        };
        Some(name)
    };
    let cell = place(p.get("cell")?)?;
    let world = p
        .get("world")
        .and_then(place)
        .map_or_else(String::new, |w| format!(" in {w}"));
    let persistent = if p.get("persistent") == Some(&Value::Bool(true)) {
        " (persistent)"
    } else {
        ""
    };
    Some(format!("cell {cell}{world}{persistent}"))
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, CollapsingHeader, TextEdit};

use crate::panels::common::{
    envelope_block, form_cell, record_actions, virtual_table, Action, Slot, TabCtx,
};
use crate::theme::{self, Field, Status, View};

/// The Links tab's state: the form, the last `refs` page and its cursor.
#[derive(Default)]
pub(crate) struct LinksTab {
    pub form: LinksForm,
    pub view: Option<LinksView>,
    pub cursor: Option<String>,
}

impl LinksTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let actions = &mut ctx.actions;
        ui.heading("Links");
        ui.label("A form's typed FormID links (`refs`), or the records that name it (`refs --incoming`, the CK's Use Info). Every row comes from a `link` or `referrer` marker; a status other than resolved or null is shown as a failure.");
        ui.horizontal_wrapped(|ui| {
            ui.label("form");
            ui.add(
                TextEdit::singleline(&mut self.form.form)
                    .desired_width(theme::width(ui, Field::Medium))
                    .hint_text("Plugin.esm:XXXXXX, EditorID, runtime:XXXXXXXX"),
            );
            ui.radio_value(&mut self.form.incoming, false, "outgoing links");
            ui.radio_value(&mut self.form.incoming, true, "use info (incoming)");
            ui.label("depth");
            ui.add(egui::DragValue::new(&mut self.form.depth).range(1..=MAX_DEPTH));
            if self.form.incoming {
                ui.checkbox(&mut self.form.all_versions, "every version");
            }
            ui.label("types");
            ui.add(
                TextEdit::singleline(&mut self.form.types)
                    .desired_width(theme::width(ui, Field::Short))
                    .hint_text("WEAP, LVLI"),
            );
            if ui.add_enabled(!busy, egui::Button::new("Run")).clicked() {
                actions.push(Action::LinksPage(None));
            }
        });
        envelope_block(ui, ctx.results.get(&Slot::Links.into()), |ui| {
            let Some(view) = &self.view else { return };
            let incoming = view.incoming();
            ui.horizontal(|ui| {
                ui.strong(format!("{} {}", view.global, view.label));
                record_actions(ui, busy, &view.global, actions);
            });
            ui.label(format!(
                "{} {} in total; {}",
                view.total,
                if incoming { "referrer(s)" } else { "link(s)" },
                view.counts
            ));
            if view.truncated {
                theme::status(
                    ui,
                    Status::Warn,
                    "the walk stopped at its cap: lower the depth",
                );
            }
            if view.unrecognized > 0 {
                theme::status(
                    ui,
                    Status::Unknown,
                    &format!(
                        "{} edge(s) with a marker this view does not know (raw JSON below)",
                        view.unrecognized
                    ),
                );
            }
            if let Some(n) = &view.not_searched {
                ui.weak(n);
            }
            ui.horizontal(|ui| {
                if self.cursor.is_some()
                    && ui
                        .add_enabled(!busy, egui::Button::new("First page"))
                        .clicked()
                {
                    actions.push(Action::LinksPage(None));
                }
                if let Some(next) = &view.next_cursor {
                    if ui
                        .add_enabled(!busy, egui::Button::new("Next page"))
                        .clicked()
                    {
                        actions.push(Action::LinksPage(Some(next.clone())));
                    }
                }
            });
            if !view.placed_by_cell.is_empty() {
                CollapsingHeader::new(format!(
                    "placed referrers by cell ({})",
                    view.placed_by_cell.len()
                ))
                .id_salt("links-cells")
                .show(ui, |ui| {
                    for c in &view.placed_by_cell {
                        ui.monospace(c);
                    }
                });
            }
            let heads: &[&str] = if incoming {
                &[
                    "hop", "referrer", "", "path", "raw", "status", "plugin", "placed",
                ]
            } else {
                &["hop", "from", "path", "raw", "status", "to", "", "plugin"]
            };
            virtual_table(
                ui,
                "links-grid",
                heads,
                view.edges.len(),
                View::Tall,
                |row, i| {
                    let Some(e) = view.edges.get(i) else { return };
                    row.cell(|ui| ui.label(e.depth.to_string()));
                    row.cell(|ui| form_cell(ui, busy, e.from.as_deref(), &e.from_label, actions));
                    if incoming {
                        row.cell(|ui| match e.from.as_deref() {
                            Some(f) => record_actions(ui, busy, f, actions),
                            None => {
                                ui.label("");
                            }
                        });
                    }
                    row.cell(|ui| ui.monospace(&e.path));
                    row.cell(|ui| ui.monospace(&e.raw));
                    row.cell(|ui| {
                        if e.problem() {
                            theme::status(ui, Status::Fail, &e.status)
                        } else {
                            ui.label(&e.status)
                        }
                    });
                    if incoming {
                        row.cell(|ui| ui.label(&e.plugin));
                        row.cell(|ui| ui.label(e.placed.as_deref().unwrap_or("")));
                    } else {
                        row.cell(|ui| {
                            if e.problem() {
                                ui.colored_label(theme::bad(ui), &e.to_label);
                            } else {
                                form_cell(ui, busy, e.to.as_deref(), &e.to_label, actions);
                            }
                        });
                        row.cell(|ui| match e.to.as_deref() {
                            Some(f) => record_actions(ui, busy, f, actions),
                            None => {
                                ui.label("");
                            }
                        });
                        row.cell(|ui| {
                            if e.target_mismatch {
                                theme::status(
                                    ui,
                                    Status::Warn,
                                    &format!("{} (target type not listed)", e.plugin),
                                );
                            } else {
                                ui.label(&e.plugin);
                            }
                        });
                    }
                },
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{check_shape, data_shape};

    fn link(depth: u64, from: &str, path: &str, raw: &str, status: &str, to: Value) -> Value {
        json!({"kind": "link", "path": path, "raw": raw, "status": status, "to": to,
            "to_signature": if to.is_null() { Value::Null } else { json!("KYWD") },
            "to_editor_id": if to.is_null() { Value::Null } else { json!("Kw") },
            "targets": ["KYWD"], "target_allowed": if to.is_null() { Value::Null } else { json!(true) },
            "resolved_by": if to.is_null() { Value::Null } else { json!("direct") },
            "error": if status == "unresolvable" { json!("no master at index 03") } else { Value::Null },
            "offset": 120, "body_offset": 30,
            "depth": depth, "from": from, "from_signature": "WEAP", "from_editor_id": "Rifle", "plugin": "Base.esm"})
    }

    /// A `refs` outgoing envelope as the engine writes it (checked against its documented
    /// shape below).
    fn outgoing() -> Value {
        json!({"command": "refs", "data": {
            "form": "Rifle", "matched_by": "editor_id", "global": "Base.esm:000801",
            "signature": "WEAP", "editor_id": "Rifle", "direction": "outgoing", "depth": 1,
            "forms_read": 1, "total": 3, "counts": {"by_status": {"resolved": 1, "null": 1, "unresolvable": 1}},
            "edges": [
                link(1, "Base.esm:000801", "KWDA.Forms[0]", "00000800", "resolved", json!("Base.esm:000800")),
                link(1, "Base.esm:000801", "KWDA.Forms[1]", "00000000", "null", Value::Null),
                link(1, "Base.esm:000801", "KWDA.Forms[2]", "03000001", "unresolvable", Value::Null),
                {"kind": "future-marker", "path": "X"}
            ],
            "next_cursor": "c2", "truncated": false, "summary": "...", "elapsed_ms": 1},
            "coverage": {"status": "partial", "decoded_subrecords": null, "opaque_subrecords": 0, "not_examined": []},
            "limitations": [], "evidence": []})
    }

    fn incoming() -> Value {
        json!({"command": "refs", "data": {
            "form": "Kw", "matched_by": "editor_id", "global": "Base.esm:000800",
            "signature": "KYWD", "editor_id": "Kw", "direction": "incoming", "depth": 1, "scope": "winner",
            "total": 2, "counts": {"referrers": 2, "placed": 1, "by_signature": {"REFR": 1, "WEAP": 1}},
            "edges": [
                {"kind": "referrer", "depth": 1, "group": "referrer", "from": "Base.esm:000801",
                 "from_raw": "00000801", "from_signature": "WEAP", "from_editor_id": "Rifle",
                 "plugin": "Patch.esm", "winner": true, "to": "Base.esm:000800", "placed": null,
                 "path": "KWDA.Forms[0]", "raw": "01000800", "status": "resolved",
                 "resolved_by": "direct", "offset": 400, "body_offset": 20},
                {"kind": "referrer", "depth": 1, "group": "placed", "from": null,
                 "from_raw": "0100ABCD", "from_signature": "REFR", "from_editor_id": null,
                 "plugin": "Patch.esm", "winner": true, "to": "Base.esm:000800",
                 "placed": {"cell": {"kind": "formref", "form": "Base.esm:000C00", "raw": "00000C00", "editor_id": "TestCell"},
                            "world": null, "persistent": true, "group_type": 8}}
            ],
            "next_cursor": null, "truncated": false, "summary": "...", "elapsed_ms": 3,
            "placed_by_cell": [{"world": null, "cell": "Base.esm:000C00", "count": 1}],
            "index": {"plugins": 2, "cache_hits": 0, "built": 2, "uncached": 0, "elapsed_ms": 1},
            "not_searched": {"no_definition": [{"type": "DFOB", "records": 3}], "records_with_raw_subrecords": 0}},
            "coverage": {"status": "partial", "decoded_subrecords": null, "opaque_subrecords": 0, "not_examined": []},
            "limitations": [], "evidence": []})
    }

    #[test]
    fn canned_envelopes_follow_the_engines_documented_shape() {
        // The future marker in `outgoing` is the one deliberate departure.
        let out = check_shape(data_shape("refs").unwrap(), &outgoing()["data"], "data");
        assert!(!out.is_empty());
        assert!(
            out.iter().all(|p| p.starts_with("data.edges[3]")),
            "{out:#?}"
        );
        let inc = check_shape(data_shape("refs").unwrap(), &incoming()["data"], "data");
        assert!(inc.is_empty(), "{inc:#?}");
    }

    #[test]
    fn outgoing_rows_come_from_link_markers() {
        let v = LinksView::from_envelope(&outgoing());
        assert!(!v.incoming());
        assert_eq!(v.global, "Base.esm:000801");
        assert_eq!(v.label, "WEAP Rifle");
        assert_eq!(v.edges.len(), 3);
        assert_eq!(
            v.unrecognized, 1,
            "an unknown marker is counted, not guessed"
        );
        assert_eq!(v.counts, "1 null, 1 resolved, 1 unresolvable");
        let first = &v.edges[0];
        assert_eq!(first.open(false), Some("Base.esm:000800"));
        assert_eq!(first.to_label, "KYWD Kw");
        assert!(!first.problem());
        assert!(
            !v.edges[1].problem(),
            "the null FormID is a value, not a problem"
        );
        assert_eq!(v.edges[1].to_label, "NULL");
        let bad = &v.edges[2];
        assert!(bad.problem());
        assert_eq!(bad.open(false), None);
        assert_eq!(bad.to_label, "no master at index 03");
        assert_eq!(v.next_cursor.as_deref(), Some("c2"));
    }

    #[test]
    fn incoming_rows_are_referrers_with_their_cells() {
        let v = LinksView::from_envelope(&incoming());
        assert!(v.incoming());
        assert_eq!(v.counts, "2 referrer(s), 1 placed; REFR 1, WEAP 1");
        assert_eq!(v.edges[0].open(true), Some("Base.esm:000801"));
        assert_eq!(v.edges[0].from_label, "WEAP Rifle");
        let placed = &v.edges[1];
        assert_eq!(placed.open(true), None);
        assert_eq!(placed.from_label, "REFR raw 0100ABCD (does not resolve)");
        assert_eq!(
            placed.placed.as_deref(),
            Some("cell Base.esm:000C00 TestCell (persistent)")
        );
        assert_eq!(v.placed_by_cell, vec!["(interior) / Base.esm:000C00: 1"]);
        assert_eq!(
            v.not_searched.as_deref(),
            Some("not searched: 1 record type(s) without a definition: DFOB (3); 0 record(s) with raw subrecords")
        );
    }

    #[test]
    fn requests_for_outgoing_and_use_info() {
        assert_eq!(
            LinksForm::outgoing("Base.esm:000801")
                .request(None)
                .unwrap()
                .args,
            json!({"form": "Base.esm:000801", "depth": 1, "limit": 100})
        );
        let mut f = LinksForm::use_info("Kw");
        f.all_versions = true;
        f.depth = 9;
        f.types = "weap, refr".into();
        assert_eq!(
            f.request(Some("c1")).unwrap().args,
            json!({"form": "Kw", "incoming": true, "scope": "all", "depth": 6,
                   "types": ["WEAP", "REFR"], "limit": 100, "cursor": "c1"})
        );
        assert!(LinksForm::default().request(None).is_err());
    }
}
