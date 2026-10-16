//! Records panel (M4.W2): record types (from `schema`, or counted in one plugin by
//! `coverage`), records of a type (`query` with `where`), and one record across the load
//! order (`inspect`) with each subrecord's member, value, offset and the schema's
//! confidence and source for it (`schema --deep` for the same type).

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::fmt_value;
use super::schema::provenance;
use crate::envelope::{list, num, text, text_or_empty};
use crate::gate::Request;

/// One record type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRow {
    /// Four-character signature.
    pub signature: String,
    /// Schema definition name (`None`: no definition).
    pub definition: Option<String>,
    /// Records of the type in the plugin (`None`: the list came from the schema).
    pub records: Option<u64>,
    /// Definition confidence (schema list only).
    pub confidence: Option<String>,
    /// `typed / opaque / unknown` subrecord occurrences (coverage only).
    pub occurrences: Option<String>,
}

/// The `schema` (no type) envelope as type rows: every defined record type.
pub fn types_from_schema(env: &Value) -> Vec<TypeRow> {
    let data = env.get("data").unwrap_or(&Value::Null);
    list(data, "records")
        .iter()
        .map(|r| TypeRow {
            signature: text_or_empty(r, "type"),
            definition: text(r, "name"),
            records: None,
            confidence: text(r, "confidence"),
            occurrences: None,
        })
        .collect()
}

/// The `coverage` envelope as type rows: every type the plugin holds, defined or not.
pub fn types_from_coverage(env: &Value) -> Vec<TypeRow> {
    let data = env.get("data").unwrap_or(&Value::Null);
    list(data, "record_types")
        .iter()
        .map(|r| {
            let occ = r.get("occurrences").unwrap_or(&Value::Null);
            let n = |k: &str| num(occ, k).unwrap_or(0);
            TypeRow {
                signature: text_or_empty(r, "signature"),
                definition: text(r, "definition"),
                records: num(r, "records"),
                confidence: None,
                occurrences: Some(format!(
                    "{} typed / {} failed / {} opaque / {} unknown",
                    n("typed"),
                    n("failed"),
                    n("opaque"),
                    n("unknown")
                )),
            }
        })
        .collect()
}

/// The `coverage` request for the type list of `plugin` (no per-signature lists).
pub fn coverage_request(plugin: &str) -> Request {
    Request::new("coverage", json!({"plugin": plugin, "signatures": false}))
}

/// The record list's inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryForm {
    /// Record type.
    pub record_type: String,
    /// `where` predicate (empty: every record).
    pub where_text: String,
    /// Every version instead of the winners.
    pub all_versions: bool,
    /// Page size.
    pub limit: u32,
}

impl Default for QueryForm {
    fn default() -> Self {
        Self {
            record_type: String::new(),
            where_text: String::new(),
            all_versions: false,
            limit: 50,
        }
    }
}

impl QueryForm {
    /// The `query` request for a page (`cursor` from the previous page).
    pub fn request(&self, cursor: Option<&str>) -> Result<Request, String> {
        let ty = self.record_type.trim();
        if ty.len() != 4 {
            return Err("pick a record type (four characters)".to_string());
        }
        let mut args = Map::new();
        args.insert("type".into(), json!(ty));
        if !self.where_text.trim().is_empty() {
            args.insert("where".into(), json!(self.where_text.trim()));
        }
        if self.all_versions {
            args.insert("scope".into(), json!("all"));
        }
        args.insert("limit".into(), json!(self.limit.clamp(1, 1000)));
        if let Some(c) = cursor {
            args.insert("cursor".into(), json!(c));
        }
        Ok(Request::new("query", Value::Object(args)))
    }
}

/// One record of a `query` page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRow {
    /// Formref (`Plugin.esm:XXXXXX`).
    pub form: String,
    /// EditorID.
    pub editor_id: String,
    /// FULL text (or its string ID when unresolved).
    pub full: String,
    /// Plugin holding this version.
    pub plugin: String,
    /// FormID as stored in that plugin.
    pub raw_form_id: String,
    /// Runtime ID (assumed).
    pub runtime_id: String,
    /// Winning version.
    pub winner: bool,
    /// Deleted (record flag 0x20).
    pub deleted: bool,
}

/// A `query` page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryView {
    /// Records on the page.
    pub rows: Vec<QueryRow>,
    /// Matched on this page.
    pub matched: u64,
    /// Undecided records on this page (not matched).
    pub unknown: u64,
    /// `(form, reasons)` for the first undecided records.
    pub unknown_records: Vec<(String, String)>,
    /// Cursor of the next page.
    pub next_cursor: Option<String>,
}

impl QueryView {
    /// Reads a `query` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let rows = list(data, "results")
            .iter()
            .map(|r| {
                let full = r.get("full").map_or_else(String::new, |f| {
                    text(f, "text")
                        .or_else(|| text(f, "string_id").map(|id| format!("(string {id})")))
                        .unwrap_or_default()
                });
                QueryRow {
                    form: text_or_empty(r, "form"),
                    editor_id: text_or_empty(r, "editor_id"),
                    full,
                    plugin: text_or_empty(r, "plugin"),
                    raw_form_id: text_or_empty(r, "raw_form_id"),
                    runtime_id: text_or_empty(r, "runtime_id"),
                    winner: r.get("winner").and_then(Value::as_bool).unwrap_or(false),
                    deleted: r.get("deleted").and_then(Value::as_bool).unwrap_or(false),
                }
            })
            .collect();
        let unknown_records = list(data, "unknown_records")
            .iter()
            .map(|u| {
                let reasons = list(u, "reasons")
                    .iter()
                    .map(crate::envelope::short)
                    .collect::<Vec<_>>()
                    .join("; ");
                (text_or_empty(u, "form"), reasons)
            })
            .collect();
        Self {
            rows,
            matched: num(data, "matched").unwrap_or(0),
            unknown: num(data, "unknown").unwrap_or(0),
            unknown_records,
            next_cursor: text(data, "next_cursor"),
        }
    }
}

/// The `inspect` request for `form` (a formref, EditorID, `runtime:<hex>`, or hex with
/// `plugin`).
pub fn inspect_request(form: &str, plugin: Option<&str>) -> Result<Request, String> {
    let form = form.trim();
    if form.is_empty() {
        return Err("give a formref, EditorID or FormID".to_string());
    }
    let mut args = Map::new();
    args.insert("form".into(), json!(form));
    if let Some(p) = plugin.map(str::trim).filter(|p| !p.is_empty()) {
        args.insert("plugin".into(), json!(p));
    }
    Ok(Request::new("inspect", Value::Object(args)))
}

/// The `schema --deep` request whose members give confidence and source per signature.
pub fn schema_request(signature: &str) -> Request {
    Request::new("schema", json!({"type": signature, "deep": true}))
}

/// One subrecord of one version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubRow {
    /// Signature.
    pub signature: String,
    /// Schema member name (empty: no layout).
    pub member: String,
    /// Decoded value, text, or hex.
    pub value: String,
    /// Absolute offset of the subrecord (or in the decompressed body).
    pub offset: Option<u64>,
    /// Payload size.
    pub size: Option<u64>,
    /// Confidence of the member that names the signature (`None`: not joined yet, or the
    /// schema does not name it).
    pub confidence: Option<String>,
    /// Source citation of that member.
    pub source: Option<String>,
}

impl SubRow {
    /// A `reference`-confidence member (or `container-reference`, which ranks with it):
    /// documented upstream, not yet confirmed.
    pub fn unconfirmed(&self) -> bool {
        matches!(
            self.confidence.as_deref(),
            Some("reference" | "container-reference")
        )
    }
}

/// One version of the record in the load order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionView {
    /// Plugin holding it.
    pub plugin: String,
    /// Load index (`None` in single-plugin mode).
    pub load_index: Option<u64>,
    /// The winner.
    pub winner: bool,
    /// Deleted.
    pub deleted: bool,
    /// `direct` or the assumption used.
    pub resolved_by: String,
    /// FormID as stored in this plugin.
    pub raw_form_id: String,
    /// `first version`, `identical-to-previous`, `differs`, `deleted`.
    pub class: String,
    /// `SIG#n change` for each subrecord that differs from the previous version.
    pub changes: Vec<String>,
    /// Record header flags.
    pub flags: String,
    /// Subrecords.
    pub subrecords: Vec<SubRow>,
}

/// An `inspect` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecordView {
    /// `load_order` or `single_plugin`.
    pub mode: String,
    /// Formref.
    pub global: String,
    /// Signature.
    pub signature: String,
    /// The winner's EditorID.
    pub editor_id: String,
    /// How `form` matched.
    pub matched_by: String,
    /// Winning plugin.
    pub winner: String,
    /// Runtime ID and its basis.
    pub runtime_id: String,
    /// Every version, in load order.
    pub versions: Vec<VersionView>,
}

impl RecordView {
    /// Reads an `inspect` envelope (either mode).
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        let mode = text(data, "mode").unwrap_or_else(|| "single_plugin".to_string());
        if mode == "load_order" {
            let versions = list(data, "versions").iter().map(version).collect();
            let runtime_id = match (text(data, "runtime_id"), text(data, "runtime_id_basis")) {
                (Some(id), Some(basis)) => format!("{id} ({basis})"),
                (Some(id), None) => id,
                _ => "unknown".to_string(),
            };
            return Self {
                mode,
                global: text_or_empty(data, "global"),
                signature: text_or_empty(data, "signature"),
                editor_id: text_or_empty(data, "editor_id"),
                matched_by: text_or_empty(data, "matched_by"),
                winner: text_or_empty(data, "winner"),
                runtime_id,
                versions,
            };
        }
        let record = data.get("record").unwrap_or(&Value::Null);
        let plugin = text_or_empty(data, "plugin");
        Self {
            mode,
            global: format!("{}:{}", plugin, text_or_empty(record, "form_id")),
            signature: text_or_empty(record, "signature"),
            editor_id: text_or_empty(record, "editor_id"),
            matched_by: text_or_empty(data, "matched_by"),
            winner: String::new(),
            runtime_id: "unknown (single-plugin mode)".to_string(),
            versions: vec![VersionView {
                plugin,
                load_index: None,
                winner: false,
                deleted: false,
                resolved_by: String::new(),
                raw_form_id: text_or_empty(record, "form_id"),
                class: "single plugin: overrides not considered".to_string(),
                changes: Vec::new(),
                flags: text_or_empty(record, "flags"),
                subrecords: list(data, "subrecords").iter().map(subrecord).collect(),
            }],
        }
    }

    /// Fills each subrecord's confidence and source from a `schema --deep` envelope of
    /// the same type.
    pub fn join_schema(&mut self, schema_env: &Value) {
        let by_sig = provenance(schema_env);
        for v in &mut self.versions {
            for s in &mut v.subrecords {
                if let Some(p) = by_sig.get(&s.signature) {
                    s.confidence = Some(p.confidence.clone());
                    s.source = Some(p.source.clone());
                    if s.member.is_empty() {
                        s.member.clone_from(&p.name);
                    }
                }
            }
        }
    }

    /// Unconfirmed (`reference`) signatures shown, for the warning line.
    pub fn unconfirmed(&self) -> Vec<String> {
        let mut out: BTreeMap<String, ()> = BTreeMap::new();
        for v in &self.versions {
            for s in v.subrecords.iter().filter(|s| s.unconfirmed()) {
                out.insert(s.signature.clone(), ());
            }
        }
        out.into_keys().collect()
    }
}

fn version(v: &Value) -> VersionView {
    let vs = v.get("vs_previous").unwrap_or(&Value::Null);
    let class = if vs.is_null() {
        "first version".to_string()
    } else {
        text_or_empty(vs, "class")
    };
    let changes = list(vs, "subrecords")
        .iter()
        .map(|c| {
            format!(
                "{}#{} {}",
                text_or_empty(c, "signature"),
                num(c, "occurrence").unwrap_or(0),
                text_or_empty(c, "change")
            )
        })
        .collect();
    let record = v.get("record").unwrap_or(&Value::Null);
    VersionView {
        plugin: text_or_empty(v, "plugin"),
        load_index: num(v, "load_index"),
        winner: v.get("winner").and_then(Value::as_bool).unwrap_or(false),
        deleted: v.get("deleted").and_then(Value::as_bool).unwrap_or(false),
        resolved_by: text_or_empty(v, "resolved_by"),
        raw_form_id: text_or_empty(v, "raw_form_id"),
        class,
        changes,
        flags: text_or_empty(record, "flags"),
        subrecords: list(v, "subrecords").iter().map(subrecord).collect(),
    }
}

fn subrecord(s: &Value) -> SubRow {
    let value = if let Some(v) = s.get("value") {
        fmt_value(v)
    } else if let Some(t) = text(s, "text") {
        t
    } else if let Some(id) = text(s, "string_id") {
        format!("(string {id}, not resolved)")
    } else {
        let hex = text_or_empty(s, "payload_hex");
        if s.get("payload_truncated") == Some(&Value::Bool(true)) {
            format!("hex {hex}...")
        } else {
            format!("hex {hex}")
        }
    };
    SubRow {
        signature: text_or_empty(s, "signature"),
        member: text_or_empty(s, "member"),
        value,
        offset: num(s, "offset"),
        size: num(s, "size"),
        confidence: None,
        source: None,
    }
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, CollapsingHeader, ScrollArea, TextEdit};

use crate::panels::common::{
    action_bar, conf_label, envelope_block, link, panel_head, plugin_combo, record_actions,
    virtual_table, Action, Slot, TabCtx,
};
use crate::panels::planet::PLANET_TYPES;
use crate::theme::{self, Field, Status, View};

/// The Records tab's state: the type list, the query form and page, the inspected
/// record, and the share of the tab's width the inspector pane takes (`records_split`
/// in `ui.json`).
#[derive(Default)]
pub(crate) struct RecordsTab {
    pub type_rows: Vec<TypeRow>,
    pub type_filter: String,
    pub coverage_plugin: String,
    pub query_form: QueryForm,
    pub query_view: Option<QueryView>,
    pub query_cursor: Option<String>,
    pub record_form: String,
    pub record_plugin: String,
    pub record_view: Option<RecordView>,
    /// The inspector's share of the width, from `ui.json` or the theme's default.
    pub split: Option<f32>,
    /// A new share after a drag of the line between the panes ends, for the shell to
    /// save once.
    pub split_changed: Option<f32>,
}

impl RecordsTab {
    /// Types and the query on the left, the inspector on the right, each scrolling on its
    /// own, so a picked record shows beside the table. The line between them is dragged
    /// to resize; the inspector's share of the width is kept in `ui.json` when the drag
    /// ends.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        panel_head(
            ui,
            "Records",
            "Record types (`schema`, or `coverage` of a plugin), a query over the session (`query`), and the record inspector (`inspect`) beside it; drag the line between the panes to resize.",
        );
        let rect = ui.available_rect_before_wrap();
        let (pane_min, default) = theme::split(ui);
        let handle = theme::split_handle(ui);
        let total = rect.width();
        let share = self.split.unwrap_or(default).clamp(0.2, 0.8);
        let right_max = (total - pane_min - handle).max(pane_min);
        let right_w = (total * share).max(pane_min).min(right_max);
        let sep_x = rect.max.x - right_w - handle;
        let handle_rect = egui::Rect::from_min_max(
            egui::pos2(sep_x, rect.min.y),
            egui::pos2(sep_x + handle, rect.max.y),
        );
        let drag = ui
            .interact(
                handle_rect,
                ui.id().with("records-split"),
                egui::Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if drag.dragged() && total > 0.0 {
            if let Some(p) = drag.interact_pointer_pos() {
                let w = (rect.max.x - p.x - handle * 0.5)
                    .max(pane_min)
                    .min(right_max);
                self.split = Some((w / total).clamp(0.2, 0.8));
            }
        }
        if drag.drag_stopped() {
            self.split_changed = self.split;
        }
        let line_x = sep_x + handle * 0.5;
        ui.painter().vline(
            line_x,
            rect.y_range(),
            theme::split_stroke(ui, drag.hovered() || drag.dragged()),
        );
        let left_rect = egui::Rect::from_min_max(rect.min, egui::pos2(sep_x, rect.max.y));
        let right_rect = egui::Rect::from_min_max(egui::pos2(sep_x + handle, rect.min.y), rect.max);
        let tour = ctx.tour;
        let mut left = pane(ui, left_rect);
        ScrollArea::both()
            .id_salt("records-left-scroll")
            .animated(!tour)
            .auto_shrink([false, false])
            .show(&mut left, |ui| {
                self.types(ui, ctx);
                self.query(ui, ctx);
            });
        let mut right = pane(ui, right_rect);
        ScrollArea::both()
            .id_salt("records-inspector-scroll")
            .animated(!tour)
            .auto_shrink([false, false])
            .show(&mut right, |ui| self.inspector(ui, ctx));
        ui.advance_cursor_after_rect(rect);
    }

    fn types(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let plugins = ctx.plugins;
        let actions = &mut ctx.actions;
        // The screenshot tour folds the type list so the query table shows beside the
        // inspector in one image.
        CollapsingHeader::new("Record types")
            .id_salt("types")
            .default_open(true)
            .open(ctx.tour.then_some(false))
            .show(ui, |ui| {
                action_bar(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(!busy, egui::Button::new("Defined types (schema)"))
                            .clicked()
                        {
                            actions.push(Action::Submit(
                                Slot::Types,
                                Request::new("schema", Value::Object(Map::new())),
                            ));
                        }
                        ui.label("or types in");
                        plugin_combo(ui, "cov-plugin", &mut self.coverage_plugin, plugins, false);
                        if ui
                            .add_enabled(
                                !busy && !self.coverage_plugin.is_empty(),
                                egui::Button::new("Count (coverage)"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Submit(
                                Slot::Types,
                                coverage_request(&self.coverage_plugin),
                            ));
                        }
                        ui.label("filter");
                        ui.add(
                            TextEdit::singleline(&mut self.type_filter)
                                .desired_width(theme::width(ui, Field::Tiny)),
                        );
                    })
                });
                envelope_block(ui, ctx.results.get(&Slot::Types.into()), |ui| {
                    let filter = self.type_filter.trim().to_ascii_uppercase();
                    let shown: Vec<_> = self
                        .type_rows
                        .iter()
                        .filter(|t| filter.is_empty() || t.signature.contains(&filter))
                        .collect();
                    virtual_table(
                        ui,
                        "types-grid",
                        &["type", "definition", "records", "confidence / occurrences"],
                        shown.len(),
                        View::Short,
                        |row, i| {
                            let Some(t) = shown.get(i) else { return };
                            row.cell(|ui| {
                                if link(ui, !busy, &t.signature)
                                    .on_hover_text("query this type")
                                    .clicked()
                                {
                                    actions.push(Action::PickType(t.signature.clone()));
                                }
                            });
                            row.cell(|ui| match &t.definition {
                                Some(d) => ui.label(d),
                                None => theme::status(
                                    ui,
                                    Status::Unknown,
                                    "no definition (subrecords unknown)",
                                ),
                            });
                            row.num(t.records.map_or_else(String::new, |n| n.to_string()));
                            row.cell(|ui| match (&t.occurrences, &t.confidence) {
                                (Some(o), _) => {
                                    ui.label(o);
                                }
                                (None, Some(c)) => conf_label(ui, c),
                                (None, None) => {
                                    ui.label("");
                                }
                            });
                        },
                    );
                });
            });
    }

    fn query(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let actions = &mut ctx.actions;
        CollapsingHeader::new("Query")
            .id_salt("query")
            .default_open(true)
            .show(ui, |ui| {
                action_bar(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("type");
                        ui.add(
                            TextEdit::singleline(&mut self.query_form.record_type)
                                .desired_width(theme::width(ui, Field::Tiny)),
                        );
                        ui.label("where");
                        ui.add(
                            TextEdit::singleline(&mut self.query_form.where_text)
                                .desired_width(theme::width(ui, Field::Long))
                                .hint_text("FULL ~ \"Rifle\" and not has KWDA"),
                        );
                        ui.checkbox(&mut self.query_form.all_versions, "all versions");
                        if ui.add_enabled(!busy, egui::Button::new("Run")).clicked() {
                            actions.push(Action::QueryPage(None));
                        }
                    })
                });
                envelope_block(ui, ctx.results.get(&Slot::Query.into()), |ui| {
                    let Some(view) = &self.query_view else { return };
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} matched, {} undecided on this page",
                            view.matched, view.unknown
                        ));
                        if self.query_cursor.is_some()
                            && ui
                                .add_enabled(!busy, egui::Button::new("First page"))
                                .clicked()
                        {
                            actions.push(Action::QueryPage(None));
                        }
                        if let Some(next) = &view.next_cursor {
                            if ui
                                .add_enabled(!busy, egui::Button::new("Next page"))
                                .clicked()
                            {
                                actions.push(Action::QueryPage(Some(next.clone())));
                            }
                        }
                    });
                    for (form, reasons) in &view.unknown_records {
                        theme::status(ui, Status::Unknown, &format!("undecided {form}: {reasons}"));
                    }
                    let selected = self.record_view.as_ref().map(|r| r.global.as_str());
                    let rows_rect = virtual_table(
                        ui,
                        "query-grid",
                        &[
                            "form",
                            "",
                            "editor id",
                            "name",
                            "plugin",
                            "raw FormID",
                            "runtime ID",
                            "",
                        ],
                        view.rows.len(),
                        View::Medium,
                        |row, i| {
                            let Some(r) = view.rows.get(i) else { return };
                            let picked = selected == Some(r.form.as_str());
                            let text = if picked {
                                egui::RichText::new(&r.form).strong()
                            } else {
                                egui::RichText::new(&r.form)
                            };
                            row.cell(|ui| {
                                if link(ui, !busy, text).on_hover_text("inspect").clicked() {
                                    actions.push(Action::Inspect(r.form.clone()));
                                }
                            });
                            row.cell(|ui| record_actions(ui, busy, &r.form, actions));
                            row.cell(|ui| ui.label(&r.editor_id));
                            row.cell(|ui| ui.label(&r.full));
                            row.cell(|ui| ui.label(&r.plugin));
                            row.cell(|ui| ui.monospace(&r.raw_form_id));
                            row.cell(|ui| ui.monospace(&r.runtime_id));
                            let mut tags = Vec::new();
                            if r.winner {
                                tags.push("winner");
                            }
                            if r.deleted {
                                tags.push("deleted");
                            }
                            row.cell(|ui| ui.label(tags.join(", ")));
                        },
                    );
                    crate::app::note_perf_target(
                        ui.ctx(),
                        crate::app::PERF_TARGET_RECORDS,
                        rows_rect,
                    );
                });
            });
    }

    /// The inspector pane: the form to inspect and the record's versions.
    fn inspector(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let actions = &mut ctx.actions;
        ui.strong("Record");
        action_bar(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("form");
                ui.add(
                    TextEdit::singleline(&mut self.record_form)
                        .desired_width(theme::width(ui, Field::Medium))
                        .hint_text("Plugin.esm:XXXXXX, EditorID, runtime:XXXXXXXX"),
                );
                ui.label("plugin (hex FormID numbering)");
                ui.add(
                    TextEdit::singleline(&mut self.record_plugin)
                        .desired_width(theme::width(ui, Field::Short)),
                );
                if ui
                    .add_enabled(!busy, egui::Button::new("Inspect"))
                    .clicked()
                {
                    match inspect_request(&self.record_form, Some(&self.record_plugin)) {
                        Ok(req) => actions.push(Action::Submit(Slot::Inspect, req)),
                        Err(e) => *ctx.notice = Some(e),
                    }
                }
            })
        });
        if ctx.results.get(&Slot::Inspect.into()).is_none() {
            ui.weak("pick a form in the query table, or type one above");
        }
        envelope_block(ui, ctx.results.get(&Slot::Inspect.into()), |ui| {
            let Some(view) = &self.record_view else {
                return;
            };
            record_view(ui, view, busy, actions);
        });
        if let Some(shown) = ctx.results.get(&Slot::InspectSchema.into()) {
            CollapsingHeader::new("provenance source: schema --deep")
                .id_salt("record-schema")
                .show(ui, |ui| envelope_block(ui, Some(shown), |_| {}));
        }
    }
}

/// A child area of `ui` laid out top-down in `rect` and clipped to it.
fn pane(ui: &mut egui::Ui, rect: egui::Rect) -> egui::Ui {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child
}

/// One inspected record: its identity line, actions, and each version's subrecords.
fn record_view(ui: &mut egui::Ui, view: &RecordView, busy: bool, actions: &mut Vec<Action>) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!(
            "{} {} {}",
            view.signature, view.global, view.editor_id
        ));
        ui.label(format!(
            "matched by {}; winner {}; runtime ID {}",
            view.matched_by, view.winner, view.runtime_id
        ));
        if !view.global.is_empty() && view.mode == "load_order" {
            if link(ui, !busy, "diagnose").clicked() {
                actions.push(Action::Diagnose(view.global.clone()));
            }
            record_actions(ui, busy, &view.global, actions);
            if PLANET_TYPES.contains(&view.signature.as_str())
                && link(ui, !busy, "planet chain")
                    .on_hover_text("the Planet Content Manager chain of this record (planet)")
                    .clicked()
            {
                actions.push(Action::Planet(view.global.clone()));
            }
        }
    });
    let unconfirmed = view.unconfirmed();
    if !unconfirmed.is_empty() {
        theme::status(
            ui,
            Status::Warn,
            &format!(
                "reference-confidence members (unconfirmed): {}",
                unconfirmed.join(", ")
            ),
        );
    }
    for (i, v) in view.versions.iter().enumerate() {
        let mut title = format!(
            "{} {}  raw {}  {}",
            v.load_index.map_or_else(String::new, |n| format!("[{n}]")),
            v.plugin,
            v.raw_form_id,
            v.class
        );
        if v.winner {
            title.push_str("  WINNER");
        }
        if v.deleted {
            title.push_str("  DELETED");
        }
        if !v.resolved_by.is_empty() && v.resolved_by != "direct" {
            title.push_str(&format!("  ({})", v.resolved_by));
        }
        CollapsingHeader::new(title)
            .id_salt(("version", i))
            .default_open(v.winner || view.versions.len() == 1)
            .show(ui, |ui| {
                if !v.changes.is_empty() {
                    ui.label(format!("vs previous: {}", v.changes.join(", ")));
                }
                ui.weak(format!("record flags {}", v.flags));
                theme::table(ui, ("subs", i)).show(ui, |ui| {
                    for h in [
                        "sig",
                        "member",
                        "value",
                        "offset",
                        "size",
                        "confidence",
                        "source",
                    ] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for s in &v.subrecords {
                        ui.monospace(&s.signature);
                        ui.label(&s.member);
                        ui.label(&s.value);
                        ui.monospace(s.offset.map_or_else(|| "?".to_string(), |o| o.to_string()));
                        ui.monospace(s.size.map_or_else(|| "?".to_string(), |n| n.to_string()));
                        match &s.confidence {
                            Some(c) => conf_label(ui, c),
                            None => {
                                ui.weak("not named by the schema");
                            }
                        }
                        ui.weak(s.source.as_deref().unwrap_or(""));
                        ui.end_row();
                    }
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inspect_env() -> Value {
        json!({"command": "inspect", "data": {
        "mode": "load_order", "global": "Base.esm:000800", "signature": "KYWD",
        "editor_id": "TestKeyword", "matched_by": "editor_id", "winner": "Patch.esp",
        "runtime_id": "00000800", "runtime_id_basis": "assumed:runtime-slots-by-scale",
        "versions": [
            {"plugin": "Base.esm", "load_index": 0, "winner": false, "deleted": false,
             "resolved_by": "direct", "raw_form_id": "00000800", "vs_previous": null,
             "record": {"flags": "0x00000000"},
             "subrecords": [
                {"signature": "EDID", "member": "EditorId", "value": "TestKeyword", "offset": 100, "size": 12},
                {"signature": "FULL", "string_id": "0000ABCD", "offset": 118, "size": 4},
                {"signature": "XXXX", "payload_hex": "0102", "payload_truncated": false, "offset": 128, "size": 2}
             ]},
            {"plugin": "Patch.esp", "load_index": 1, "winner": true, "deleted": false,
             "resolved_by": "direct", "raw_form_id": "00000800",
             "record": {"flags": "0x00000000"},
             "vs_previous": {"class": "differs", "subrecords": [{"signature": "TNAM", "occurrence": 0, "change": "added"}]},
             "subrecords": [
                {"signature": "TNAM", "value": {"kind": "formref", "raw": "00000801", "form": "Base.esm:000801"}, "offset": 40, "size": 4}
             ]}
        ]}})
    }

    #[test]
    fn inspect_versions_and_values_come_from_the_envelope() {
        let view = RecordView::from_envelope(&inspect_env());
        assert_eq!(view.global, "Base.esm:000800");
        assert_eq!(view.runtime_id, "00000800 (assumed:runtime-slots-by-scale)");
        assert_eq!(view.versions.len(), 2);
        let base = &view.versions[0];
        assert_eq!(base.class, "first version");
        assert_eq!(base.subrecords[1].value, "(string 0000ABCD, not resolved)");
        assert_eq!(base.subrecords[2].value, "hex 0102");
        let patch = &view.versions[1];
        assert_eq!(patch.changes, vec!["TNAM#0 added"]);
        assert_eq!(patch.subrecords[0].value, "Base.esm:000801 [raw 00000801]");
    }

    #[test]
    fn schema_join_adds_confidence_and_marks_reference_members() {
        let mut view = RecordView::from_envelope(&inspect_env());
        let schema = json!({"command": "schema", "data": {"members": [
            {"kind": "subrecord", "signature": "EDID", "name": "EditorId", "confidence": "corpus", "source": "corpus: test"},
            {"kind": "group", "name": "G", "members": [
                {"kind": "subrecord", "signature": "TNAM", "name": "Target", "confidence": "reference", "source": "xedit: test"}
            ]}
        ]}});
        view.join_schema(&schema);
        let edid = &view.versions[0].subrecords[0];
        assert_eq!(edid.confidence.as_deref(), Some("corpus"));
        let tnam = &view.versions[1].subrecords[0];
        assert!(tnam.unconfirmed());
        assert_eq!(tnam.member, "Target");
        assert_eq!(view.unconfirmed(), vec!["TNAM"]);
        // A signature the schema does not name stays unknown, not "absent".
        assert_eq!(view.versions[0].subrecords[2].confidence, None);
    }

    #[test]
    fn single_plugin_mode_is_one_version_without_overrides() {
        let env = json!({"command": "inspect", "data": {"plugin": "Test.esm", "matched_by": "form_id",
            "record": {"signature": "GLOB", "form_id": "00000800", "editor_id": "TestGlobal", "flags": "0x00000000"},
            "subrecords": [{"signature": "FLTV", "payload_hex": "0000803F", "offset": 50, "size": 4}]}});
        let view = RecordView::from_envelope(&env);
        assert_eq!(view.mode, "single_plugin");
        assert_eq!(view.versions.len(), 1);
        assert!(view.versions[0].class.contains("overrides not considered"));
        assert_eq!(view.versions[0].subrecords[0].value, "hex 0000803F");
    }

    #[test]
    fn query_form_and_page() {
        let form = QueryForm {
            record_type: "WEAP".into(),
            where_text: " FULL ~ 'Rifle' ".into(),
            all_versions: true,
            limit: 5000,
        };
        assert_eq!(
            form.request(Some("q1.x")).unwrap().args,
            json!({"type": "WEAP", "where": "FULL ~ 'Rifle'", "scope": "all", "limit": 1000, "cursor": "q1.x"})
        );
        assert!(QueryForm::default().request(None).is_err());
        let env = json!({"command": "query", "data": {"matched": 1, "unknown": 1, "next_cursor": null,
            "results": [{"form": "Test.esm:000800", "editor_id": "TestRifle", "full": {"text": "Test Rifle"},
                "plugin": "Test.esm", "raw_form_id": "00000800", "runtime_id": "00000800", "winner": true, "deleted": false}],
            "unknown_records": [{"form": "Test.esm:000801", "reasons": ["FULL: string ID in no table"]}]}});
        let page = QueryView::from_envelope(&env);
        assert_eq!(page.rows[0].full, "Test Rifle");
        assert_eq!(page.next_cursor, None);
        assert_eq!(page.unknown_records[0].1, "FULL: string ID in no table");
    }

    #[test]
    fn type_lists_from_schema_and_from_coverage() {
        let schema = json!({"data": {"records": [{"type": "GLOB", "name": "Global", "confidence": "corpus"}]}});
        let rows = types_from_schema(&schema);
        assert_eq!(rows[0].signature, "GLOB");
        assert_eq!(rows[0].records, None);
        let cov = json!({"data": {"record_types": [{"signature": "AVMD", "definition": null, "records": 35,
            "occurrences": {"typed": 0, "failed": 0, "opaque": 0, "unknown": 766}}]}});
        let rows = types_from_coverage(&cov);
        assert_eq!(rows[0].definition, None);
        assert_eq!(rows[0].records, Some(35));
        assert_eq!(
            rows[0].occurrences.as_deref(),
            Some("0 typed / 0 failed / 0 opaque / 766 unknown")
        );
        assert_eq!(
            coverage_request("A.esm").args,
            json!({"plugin": "A.esm", "signatures": false})
        );
    }
}
