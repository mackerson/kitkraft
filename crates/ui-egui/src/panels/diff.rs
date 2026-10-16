//! Diff panel (M4.W3): two plugins through `diff`, or a plugin's overrides against the
//! versions before them through `conflicts`; per-field rows (`fields[]` paths with before
//! and after), opaque members as byte ranges, untyped subrecords as byte spans.

use serde_json::{json, Map, Value};

use super::fmt_value;
use crate::envelope::{list, num, short, text, text_or_empty};
use crate::gate::Request;

/// What to compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffMode {
    /// `diff a b`: two files, FormIDs matched through their masters.
    #[default]
    TwoPlugins,
    /// `conflicts --plugin`: a plugin's overrides in the loaded order, each against the
    /// version before it (its masters, or an earlier plugin).
    VsMasters,
}

/// The panel's inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffForm {
    /// Mode.
    pub mode: DiffMode,
    /// First plugin (`diff`), or the overriding plugin (`conflicts`).
    pub a: String,
    /// Second plugin (`diff` only).
    pub b: String,
    /// Only this form (`diff`) / only this record type (`conflicts`).
    pub filter: String,
    /// Only forms two or more plugins override (`conflicts`).
    pub only_conflicts: bool,
}

impl DiffForm {
    /// The request for the current mode.
    pub fn request(&self, cursor: Option<&str>) -> Result<Request, String> {
        let mut args = Map::new();
        let filter = self.filter.trim();
        match self.mode {
            DiffMode::TwoPlugins => {
                if self.a.trim().is_empty() || self.b.trim().is_empty() {
                    return Err("give two plugins".to_string());
                }
                args.insert("a".into(), json!(self.a.trim()));
                args.insert("b".into(), json!(self.b.trim()));
                if !filter.is_empty() {
                    args.insert("form".into(), json!(filter));
                }
                args.insert("limit".into(), json!(200));
                Ok(Request::new("diff", Value::Object(args)))
            }
            DiffMode::VsMasters => {
                if self.a.trim().is_empty() {
                    return Err("give the plugin whose overrides to compare".to_string());
                }
                args.insert("plugin".into(), json!(self.a.trim()));
                if !filter.is_empty() {
                    args.insert("type".into(), json!(filter));
                }
                if self.only_conflicts {
                    args.insert("only_conflicts".into(), json!(true));
                }
                args.insert("limit".into(), json!(50));
                if let Some(c) = cursor {
                    args.insert("cursor".into(), json!(c));
                }
                Ok(Request::new("conflicts", Value::Object(args)))
            }
        }
    }
}

/// One compared value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRow {
    /// `SIG#occurrence`.
    pub subrecord: String,
    /// `added`, `removed`, `changed`.
    pub change: String,
    /// Field path (`KWDA.Forms[1]`), or the signature when compared as bytes.
    pub path: String,
    /// Value before (`(absent)` when the member is missing on that side).
    pub before: String,
    /// Value after.
    pub after: String,
}

/// One record that differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordDiff {
    /// Formref.
    pub form: String,
    /// Signature.
    pub signature: String,
    /// `differs`, `identical-to-previous`, `deleted`, ... (conflicts), or the diff class.
    pub class: String,
    /// `A.esm -> B.esm` (diff) or `previous -> plugin` (conflicts).
    pub sides: String,
    /// Header flags differ.
    pub flags_differ: bool,
    /// Per-field rows.
    pub rows: Vec<FieldRow>,
}

/// A diff or conflicts result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffView {
    /// `added 1, removed 0, changed 2, unchanged 50` or the per-plugin override counts.
    pub counts: Vec<String>,
    /// Records added in b (`diff`).
    pub added: Vec<String>,
    /// Records removed in b (`diff`).
    pub removed: Vec<String>,
    /// Records that differ.
    pub records: Vec<RecordDiff>,
    /// Next page (`conflicts`).
    pub next_cursor: Option<String>,
}

impl DiffView {
    /// Reads a `diff` or `conflicts` envelope.
    pub fn from_envelope(env: &Value) -> Self {
        let data = env.get("data").unwrap_or(&Value::Null);
        if text(env, "command").as_deref() == Some("conflicts") {
            return Self::from_conflicts(data);
        }
        let counts = data.get("counts").map_or_else(Vec::new, |c| {
            c.as_object()
                .map(|m| m.iter().map(|(k, v)| format!("{k} {}", short(v))).collect())
                .unwrap_or_default()
        });
        let entry = |r: &Value| {
            format!(
                "{} {}",
                text_or_empty(r, "signature"),
                text_or_empty(r, "form")
            )
        };
        let a = text_or_empty(data, "a");
        let b = text_or_empty(data, "b");
        let records = list(data, "changed")
            .iter()
            .map(|r| RecordDiff {
                form: text_or_empty(r, "form"),
                signature: text_or_empty(r, "signature"),
                class: text_or_empty(r, "class"),
                sides: format!("{a} -> {b}"),
                flags_differ: r
                    .get("flags_differ")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                rows: field_rows(r),
            })
            .collect();
        Self {
            counts,
            added: list(data, "added").iter().map(entry).collect(),
            removed: list(data, "removed").iter().map(entry).collect(),
            records,
            next_cursor: None,
        }
    }

    fn from_conflicts(data: &Value) -> Self {
        let counts = list(data, "counts")
            .iter()
            .map(|c| {
                format!(
                    "{} overrides {}: {}",
                    text_or_empty(c, "plugin"),
                    text_or_empty(c, "master"),
                    num(c, "total").unwrap_or(0)
                )
            })
            .collect();
        let mut records = Vec::new();
        for item in list(data, "items") {
            for o in list(item, "overrides") {
                records.push(RecordDiff {
                    form: text_or_empty(item, "form"),
                    signature: text_or_empty(item, "signature"),
                    class: text_or_empty(o, "class"),
                    sides: format!(
                        "{} -> {}",
                        text(o, "previous").unwrap_or_else(|| "(none)".to_string()),
                        text_or_empty(o, "plugin")
                    ),
                    flags_differ: o
                        .get("flags_differ")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    rows: field_rows(o),
                });
            }
        }
        Self {
            counts,
            added: Vec::new(),
            removed: Vec::new(),
            records,
            next_cursor: text(data, "next_cursor"),
        }
    }
}

/// Identical byte-only rows of one signature folded into the first: its row index, the
/// occurrences seen, and the signature.
struct Folded {
    row: usize,
    occurrences: Vec<u64>,
    sig: String,
}

/// The rows of one record's `subrecords[]`: one per `fields[]` entry, or one byte-span
/// row when the subrecord is compared as bytes. Byte-only rows of one signature that
/// say exactly the same thing (same change, same spans: `conflicts` reports none, so a
/// navmesh's dozens of `NVMI` occurrences all read "range not reported") become one row
/// naming how many occurrences it stands for.
fn field_rows(record: &Value) -> Vec<FieldRow> {
    let mut rows = Vec::new();
    let mut folded: Vec<Folded> = Vec::new();
    for s in list(record, "subrecords") {
        let sig = text_or_empty(s, "signature");
        let occurrence = num(s, "occurrence").unwrap_or(0);
        let subrecord = format!("{sig}#{occurrence}");
        let change = text_or_empty(s, "change");
        let fields = list(s, "fields");
        if fields.is_empty() {
            let reason = text(s, "as_bytes").map_or_else(String::new, |r| format!(" ({r})"));
            let row = FieldRow {
                subrecord,
                change,
                path: format!("{sig} as bytes{reason}"),
                before: span(s.get("a")),
                after: span(s.get("b")),
            };
            let same = folded.iter_mut().find(|f| {
                f.sig == sig
                    && rows.get(f.row).is_some_and(|r: &FieldRow| {
                        r.change == row.change
                            && r.path == row.path
                            && r.before == row.before
                            && r.after == row.after
                    })
            });
            match same {
                Some(f) => f.occurrences.push(occurrence),
                None => {
                    folded.push(Folded {
                        row: rows.len(),
                        occurrences: vec![occurrence],
                        sig,
                    });
                    rows.push(row);
                }
            }
            continue;
        }
        for f in fields {
            rows.push(FieldRow {
                subrecord: subrecord.clone(),
                change: change.clone(),
                path: text_or_empty(f, "path"),
                before: fmt_value(f.get("before").unwrap_or(&Value::Null)),
                after: fmt_value(f.get("after").unwrap_or(&Value::Null)),
            });
        }
    }
    for f in folded.iter().filter(|f| f.occurrences.len() > 1) {
        if let Some(row) = rows.get_mut(f.row) {
            let first = f.occurrences.iter().min().copied().unwrap_or(0);
            let last = f.occurrences.iter().max().copied().unwrap_or(0);
            row.subrecord = format!("{} x{} (#{first}..#{last})", f.sig, f.occurrences.len());
        }
    }
    rows
}

/// `bytes @file_offset len n` for a diff span; `(absent)` when the side has no such
/// subrecord (`null`); `bytes (range not reported)` when the result carries no spans
/// (`conflicts` does not): unknown is never shown as absent.
fn span(v: Option<&Value>) -> String {
    match v {
        Some(s) if s.is_object() => format!(
            "bytes @{} len {}",
            num(s, "file_offset").map_or_else(|| "?".to_string(), |n| n.to_string()),
            num(s, "length").map_or_else(|| "?".to_string(), |n| n.to_string())
        ),
        Some(_) => "(absent)".to_string(),
        None => "bytes (range not reported)".to_string(),
    }
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, CollapsingHeader, TextEdit};

use crate::panels::common::{
    envelope_block, link, plugin_combo, record_actions, Action, Slot, TabCtx,
};
use crate::theme::{self, Field};

/// The Diff tab's state: the form and the last `diff` or `conflicts` page.
#[derive(Default)]
pub(crate) struct DiffTab {
    pub form: DiffForm,
    pub view: Option<DiffView>,
}

impl DiffTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        let busy = ctx.busy;
        let plugins = ctx.plugins;
        let actions = &mut ctx.actions;
        ui.heading("Diff");
        ui.horizontal_wrapped(|ui| {
            ui.radio_value(
                &mut self.form.mode,
                DiffMode::TwoPlugins,
                "two plugins (diff)",
            );
            ui.radio_value(
                &mut self.form.mode,
                DiffMode::VsMasters,
                "a plugin vs the versions before it (conflicts; needs a load)",
            );
        });
        ui.horizontal_wrapped(|ui| {
            let path = theme::width(ui, Field::Long);
            match self.form.mode {
                DiffMode::TwoPlugins => {
                    ui.label("a");
                    ui.add(TextEdit::singleline(&mut self.form.a).desired_width(path));
                    plugin_combo(ui, "diff-a", &mut self.form.a, plugins, false);
                    ui.label("b");
                    ui.add(TextEdit::singleline(&mut self.form.b).desired_width(path));
                    plugin_combo(ui, "diff-b", &mut self.form.b, plugins, false);
                    ui.label("form");
                }
                DiffMode::VsMasters => {
                    ui.label("plugin");
                    ui.add(TextEdit::singleline(&mut self.form.a).desired_width(path));
                    plugin_combo(ui, "diff-p", &mut self.form.a, plugins, true);
                    ui.checkbox(
                        &mut self.form.only_conflicts,
                        "only forms 2+ plugins override",
                    );
                    ui.label("type");
                }
            }
            ui.add(
                TextEdit::singleline(&mut self.form.filter)
                    .desired_width(theme::width(ui, Field::Short)),
            );
            if ui
                .add_enabled(!busy, egui::Button::new("Compare"))
                .clicked()
            {
                match self.form.request(None) {
                    Ok(req) => actions.push(Action::Submit(Slot::Diff, req)),
                    Err(e) => *ctx.notice = Some(e),
                }
            }
        });
        envelope_block(ui, ctx.results.get(&Slot::Diff.into()), |ui| {
            let Some(view) = &self.view else { return };
            ui.label(view.counts.join("; "));
            if !view.added.is_empty() {
                ui.label(format!("added: {}", view.added.join(", ")));
            }
            if !view.removed.is_empty() {
                ui.label(format!("removed: {}", view.removed.join(", ")));
            }
            if let Some(next) = &view.next_cursor {
                if ui
                    .add_enabled(!busy, egui::Button::new("Next page"))
                    .clicked()
                {
                    if let Ok(req) = self.form.request(Some(next)) {
                        actions.push(Action::Submit(Slot::Diff, req));
                    }
                }
            }
            for (i, r) in view.records.iter().enumerate() {
                let title = format!(
                    "{} {}  {}  [{}]{}",
                    r.signature,
                    r.form,
                    r.sides,
                    r.class,
                    if r.flags_differ { "  flags differ" } else { "" }
                );
                CollapsingHeader::new(title)
                    .id_salt(("diff-rec", i))
                    .default_open(i < 5)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if link(ui, !busy, "inspect across the load order").clicked() {
                                actions.push(Action::Inspect(r.form.clone()));
                            }
                            record_actions(ui, busy, &r.form, actions);
                        });
                        theme::table(ui, ("diff-grid", i)).show(ui, |ui| {
                            for h in ["subrecord", "change", "path", "before", "after"] {
                                ui.strong(h);
                            }
                            ui.end_row();
                            for f in &r.rows {
                                ui.monospace(&f.subrecord);
                                ui.label(&f.change);
                                ui.monospace(&f.path);
                                ui.label(&f.before);
                                ui.label(&f.after);
                                ui.end_row();
                            }
                        });
                    });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_per_mode() {
        let mut form = DiffForm {
            a: "A.esm".into(),
            b: "B.esm".into(),
            ..DiffForm::default()
        };
        assert_eq!(
            form.request(None).unwrap().args,
            json!({"a": "A.esm", "b": "B.esm", "limit": 200})
        );
        form.mode = DiffMode::VsMasters;
        form.filter = "WEAP".into();
        form.only_conflicts = true;
        let req = form.request(Some("c1")).unwrap();
        assert_eq!(req.id, "conflicts");
        assert_eq!(
            req.args,
            json!({"plugin": "A.esm", "type": "WEAP", "only_conflicts": true, "limit": 50, "cursor": "c1"})
        );
        assert!(DiffForm::default().request(None).is_err());
    }

    #[test]
    fn diff_rows_show_fields_opaque_ranges_and_byte_spans() {
        let env = json!({"command": "diff", "data": {"a": "A.esm", "b": "B.esm",
        "counts": {"added": 1, "removed": 0, "changed": 1, "unchanged": 3},
        "added": [{"form": "B.esm:000900", "signature": "GLOB", "raw_form_id": "00000900", "offset": 10}],
        "removed": [],
        "changed": [{"form": "A.esm:000800", "signature": "WEAP", "class": "differs", "flags_differ": false,
            "subrecords": [
                {"signature": "DNAM", "occurrence": 0, "change": "changed", "member": "Data",
                 "fields": [
                    {"path": "DNAM.Speed", "before": 1.0, "after": 1.5},
                    {"path": "DNAM.Rest", "before": {"kind": "opaque", "opaque": {"payload_offset": 8, "length": 4, "file_offset": 300}}, "after": null}
                 ]},
                {"signature": "KWDA", "occurrence": 0, "change": "changed",
                 "fields": [{"path": "KWDA.Forms[1]", "before": {"kind": "formref", "form": "A.esm:000801"}, "after": {"kind": "formref", "form": null, "null": true}}]},
                {"signature": "XXXX", "occurrence": 1, "change": "added", "as_bytes": "no layout",
                 "a": null, "b": {"file_offset": 500, "body_offset": 20, "length": 6}}
            ]}]}});
        let view = DiffView::from_envelope(&env);
        assert_eq!(
            view.counts,
            vec!["added 1", "changed 1", "removed 0", "unchanged 3"]
        );
        assert_eq!(view.added, vec!["GLOB B.esm:000900"]);
        let rows = &view.records[0].rows;
        assert_eq!(rows.len(), 4);
        assert_eq!(
            (rows[0].before.as_str(), rows[0].after.as_str()),
            ("1.0", "1.5")
        );
        assert_eq!(rows[1].before, "opaque bytes @300 len 4");
        assert_eq!(rows[1].after, "(absent)");
        assert_eq!(rows[2].after, "NULL");
        assert_eq!(rows[3].path, "XXXX as bytes (no layout)");
        assert_eq!(
            (rows[3].before.as_str(), rows[3].after.as_str()),
            ("(absent)", "bytes @500 len 6")
        );
        assert_eq!(view.records[0].sides, "A.esm -> B.esm");
    }

    #[test]
    fn conflicts_rows_are_one_record_per_override() {
        let env = json!({"command": "conflicts", "data": {
            "counts": [{"master": "Base.esm", "plugin": "Mod.esp", "total": 2, "types": {"GLOB": 2}}],
            "next_cursor": "c2",
            "items": [{"form": "Base.esm:000800", "signature": "GLOB", "editor_id": "TestGlobal",
                "overrides": [{"plugin": "Mod.esp", "previous": "Base.esm", "class": "differs", "flags_differ": false,
                    "subrecords": [{"signature": "FLTV", "occurrence": 0, "change": "changed",
                        "fields": [{"path": "FLTV", "before": 1.0, "after": 2.0}]},
                        {"signature": "NVMI", "occurrence": 0, "change": "changed"}]}]}]}});
        let view = DiffView::from_envelope(&env);
        assert_eq!(view.counts, vec!["Mod.esp overrides Base.esm: 2"]);
        assert_eq!(view.records[0].sides, "Base.esm -> Mod.esp");
        assert_eq!(view.records[0].rows[0].after, "2.0");
        // `conflicts` carries no byte spans: unknown, not absent.
        assert_eq!(view.records[0].rows[1].before, "bytes (range not reported)");
        assert_eq!(view.next_cursor.as_deref(), Some("c2"));
    }

    #[test]
    fn identical_byte_only_rows_collapse_per_signature() {
        let byte_row = |sig: &str, n: u64, change: &str| json!({"signature": sig, "occurrence": n, "change": change});
        let mut subs: Vec<Value> = (0..12).map(|n| byte_row("NVMI", n, "changed")).collect();
        subs.push(byte_row("NVMI", 12, "added"));
        subs.push(byte_row("ONAM", 0, "changed"));
        subs.push(
            json!({"signature": "FLTV", "occurrence": 0, "change": "changed",
            "fields": [{"path": "FLTV", "before": 1.0, "after": 2.0}]}),
        );
        let env = json!({"command": "conflicts", "data": {"counts": [], "items": [
            {"form": "Base.esm:000800", "signature": "NAVM",
             "overrides": [{"plugin": "Mod.esp", "previous": "Base.esm", "class": "differs", "subrecords": subs}]}]}});
        let rows = &DiffView::from_envelope(&env).records[0].rows;
        let names: Vec<&str> = rows.iter().map(|r| r.subrecord.as_str()).collect();
        assert_eq!(
            names,
            vec!["NVMI x12 (#0..#11)", "NVMI#12", "ONAM#0", "FLTV#0"]
        );
        assert_eq!(rows[0].before, "bytes (range not reported)");

        // `diff` spans differ per occurrence: nothing is folded.
        let env = json!({"command": "diff", "data": {"a": "A.esm", "b": "B.esm", "changed": [
            {"form": "A.esm:000800", "signature": "NAVM", "class": "differs", "subrecords": [
                {"signature": "NVMI", "occurrence": 0, "change": "changed",
                 "a": {"file_offset": 10, "length": 4}, "b": {"file_offset": 90, "length": 4}},
                {"signature": "NVMI", "occurrence": 1, "change": "changed",
                 "a": {"file_offset": 20, "length": 4}, "b": {"file_offset": 99, "length": 4}}]}]}});
        let rows = &DiffView::from_envelope(&env).records[0].rows;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].subrecord, "NVMI#1");
    }
}
