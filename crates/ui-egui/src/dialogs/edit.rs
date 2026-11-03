//! The generic edit dialog (M7.W3, M7.E2): the selected record's winning version, its
//! typed fields grouped per the type's layout (`super::Layout`), one tab at a time.
//! Labels are the schema's member names (with the signature beside them), never CK
//! text; values are the envelope's, formatted as the Records inspector formats them.
//!
//! M7.E2 (ADR 0007 Amendment 4, model A): a field whose value is a number or plain text
//! can be edited in place. OK asks the editor (`super::editor`) to preview every changed
//! field of the record as one `set` into the active file; its confirm box writes them,
//! one confirmed write per OK click. Hex payloads, localized strings, links, lists and
//! structs stay read-only here (`kit set` and MCP edit them). The edits are UI state:
//! kept in egui's memory under the record ([`changed_fields`], [`clear_edits`]), never
//! saved.

use std::collections::BTreeMap;

use eframe::egui::{self, ScrollArea};
use serde_json::{Map, Value};

use super::{place, DialogState, Layout};
use crate::panels::common::Action;
use crate::panels::records::{RecordView, SubRow, VersionView};
use crate::panels::MAX_CELL;
use crate::theme::{self, Status, View};

/// The version the dialog shows: the winner, else the last one listed.
pub fn shown_version(record: &RecordView) -> Option<&VersionView> {
    record
        .versions
        .iter()
        .find(|v| v.winner)
        .or_else(|| record.versions.last())
}

/// One edited field: the value the dialog showed when editing began, the text typed,
/// and whether the value is a number (sent to `set` as a JSON number).
#[derive(Debug, Clone, PartialEq)]
struct FieldEdit {
    shown: String,
    text: String,
    number: bool,
}

impl FieldEdit {
    fn changed(&self) -> bool {
        self.text != self.shown
    }

    /// The value `set` takes: a number when the field is numeric (an integer kept
    /// exact), else the text; `Err` for a numeric field whose text is not a number.
    fn value(&self) -> Result<Value, String> {
        let t = self.text.trim();
        if !self.number {
            return Ok(Value::String(self.text.clone()));
        }
        if let Ok(i) = t.parse::<i64>() {
            return Ok(Value::from(i));
        }
        t.parse::<f64>()
            .ok()
            .filter(|f| f.is_finite())
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| format!("`{t}` is not a number"))
    }
}

/// The edits of one record, by `set` field path (`FLTV`, `FULL[1]`).
#[derive(Debug, Clone, Default, PartialEq)]
struct Edits(BTreeMap<String, FieldEdit>);

fn edits_id(form: &str) -> egui::Id {
    egui::Id::new(("kitkraft-edit-dialog-edits", form))
}

/// The changed fields of `form` as `set` takes them (field path -> value), or why one is
/// not a value `set` can take.
pub(crate) fn changed_fields(
    ctx: &egui::Context,
    form: &str,
) -> Result<Map<String, Value>, String> {
    let edits: Edits = ctx.data(|d| d.get_temp(edits_id(form))).unwrap_or_default();
    let mut out = Map::new();
    for (path, e) in edits.0.iter().filter(|(_, e)| e.changed()) {
        out.insert(path.clone(), e.value().map_err(|m| format!("{path}: {m}"))?);
    }
    Ok(out)
}

/// Drops the edits of `form` (after its write, or on Reset).
pub(crate) fn clear_edits(ctx: &egui::Context, form: &str) {
    ctx.data_mut(|d| d.remove::<Edits>(edits_id(form)));
}

/// Whether the dialog edits `s` in place, and as a number: a value that is a number, or
/// plain text the dialog shows whole. A hex payload, a localized string, a link, a list,
/// a struct, an absent or truncated value is read-only here.
fn editable(s: &SubRow) -> Option<bool> {
    let v = s.value.as_str();
    if s.signature.len() != 4 || v.contains('\n') {
        return None;
    }
    if v.trim().parse::<f64>().is_ok_and(f64::is_finite) {
        return Some(true);
    }
    let structured = ["hex ", "opaque bytes", "(string ", "(absent)", "[", "{"]
        .iter()
        .any(|p| v.starts_with(p));
    let link = [".esm:", ".esp:", ".esl:"]
        .iter()
        .any(|p| v.to_ascii_lowercase().contains(p));
    let truncated = v.chars().count() >= MAX_CELL || v.ends_with("...");
    (!s.member.is_empty() && !structured && !link && !truncated).then_some(false)
}

/// The `set` field path of each subrecord of `version`: its signature, with `[n]` for
/// the n-th repeat (`set` counts from 0), keyed by its position in the list.
fn field_paths(version: &VersionView) -> Vec<String> {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    version
        .subrecords
        .iter()
        .map(|s| {
            let n = seen.entry(s.signature.as_str()).or_insert(0);
            let path = if *n == 0 {
                s.signature.clone()
            } else {
                format!("{}[{n}]", s.signature)
            };
            *n = n.saturating_add(1);
            path
        })
        .collect()
}

impl DialogState {
    /// Draws the dialog for `record` and returns what its buttons ask for: OK asks the
    /// editor to preview the changed fields with `set` (`Action::OpenEditor`). The shell
    /// asks for the `schema` a type without a layout needs (`DialogState::schema_request`).
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, record: Option<&RecordView>) -> Vec<Action> {
        let mut actions = Vec::new();
        let Some(record) = record.filter(|r| !r.signature.is_empty()) else {
            theme::status_line(
                ui,
                Status::NotRun,
                "no record selected: inspect one in the Records tab (or double-click a Use Info row)",
            );
            return actions;
        };
        if self.record != record.global {
            self.record.clone_from(&record.global);
            self.tab = 0;
        }
        let sig = record.signature.as_str();
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!("{sig} {}", record.editor_id));
            ui.monospace(&record.global);
        });
        let layout = match self.layout(sig) {
            Some(Ok(l)) => l,
            Some(Err(e)) => {
                theme::status_line(ui, Status::Fail, &format!("no layout for {sig}: {e}"));
                return actions;
            }
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!(
                        "building a default layout from `schema --type {sig}`"
                    ));
                });
                return actions;
            }
        };
        header(ui, &layout, record);
        let Some(version) = shown_version(record) else {
            theme::status_line(ui, Status::Unknown, "the envelope lists no version");
            return actions;
        };
        let ctx = ui.ctx().clone();
        let id = edits_id(&record.global);
        let mut edits: Edits = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
        let paths = field_paths(version);
        let path_of = |s: &SubRow| -> Option<&String> {
            version
                .subrecords
                .iter()
                .position(|x| std::ptr::eq(x, s))
                .and_then(|i| paths.get(i))
        };
        let placed = place(&layout, &version.subrecords);
        ui.horizontal_wrapped(|ui| {
            for (i, t) in placed.iter().enumerate() {
                let n: usize = t.groups.iter().map(|g| g.1.len()).sum();
                ui.selectable_value(&mut self.tab, i, format!("{} ({n})", t.name));
            }
        });
        ui.separator();
        let tab = placed.get(self.tab).or_else(|| placed.first());
        let footer = ui.spacing().interact_size.y * 3.5;
        ScrollArea::vertical()
            .id_salt(("edit-dialog", self.tab))
            .auto_shrink([false, false])
            .max_height((ui.available_height() - footer).max(theme::view_height(ui, View::Short)))
            .show(ui, |ui| {
                let Some(tab) = tab else { return };
                for (gi, (group, rows)) in tab.groups.iter().enumerate() {
                    ui.label(theme::subheading(ui, group.as_str()));
                    if rows.is_empty() {
                        ui.weak("  (none in this record)");
                        theme::gap(ui);
                        continue;
                    }
                    // One line per field: the label and signature in fixed columns,
                    // the value (or its edit box) in the rest of the width.
                    let label_w = theme::width(ui, theme::Field::Medium);
                    let sig_w = theme::width(ui, theme::Field::Tiny);
                    for (ri, s) in rows.iter().enumerate() {
                        ui.push_id(("edit-dialog-row", gi, ri), |ui| {
                            ui.horizontal_top(|ui| {
                                let label = if s.member.is_empty() {
                                    s.signature.as_str()
                                } else {
                                    s.member.as_str()
                                };
                                let h = ui.spacing().interact_size.y;
                                let left = egui::Layout::left_to_right(egui::Align::Center);
                                ui.allocate_ui_with_layout(egui::vec2(label_w, h), left, |ui| {
                                    ui.set_min_width(label_w);
                                    ui.add(egui::Label::new(label).truncate());
                                });
                                ui.allocate_ui_with_layout(egui::vec2(sig_w, h), left, |ui| {
                                    ui.set_min_width(sig_w);
                                    ui.label(egui::RichText::new(&s.signature).monospace().weak());
                                });
                                let edit = editable(s).zip(path_of(s));
                                if let Some((number, path)) = edit {
                                    field_edit(ui, &mut edits, path, s, number);
                                } else {
                                    let value = ui.add(
                                        egui::Label::new(egui::RichText::new(&s.value).monospace())
                                            .wrap_mode(egui::TextWrapMode::Wrap),
                                    );
                                    if let Some(o) = s.offset {
                                        value.on_hover_text(format!(
                                            "at {o} (0x{o:X}), {} bytes; read-only here (edit it with `kit set` or MCP)",
                                            s.size.map_or_else(|| "?".to_string(), |n| n.to_string())
                                        ));
                                    }
                                }
                            });
                        });
                    }
                    theme::gap(ui);
                }
            });
        let changed = edits.0.values().filter(|e| e.changed()).count();
        let invalid = edits
            .0
            .iter()
            .filter(|(_, e)| e.changed())
            .find_map(|(p, e)| e.value().err().map(|m| format!("{p}: {m}")));
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            let ok = ui.add_enabled(changed > 0 && invalid.is_none(), egui::Button::new("OK"));
            let ok = match (&invalid, changed) {
                (Some(why), _) => ok.on_disabled_hover_text(why.as_str()),
                (None, 0) => ok.on_disabled_hover_text("no field changed: edit a value first"),
                _ => ok.on_hover_text(
                    "previews the changed fields with `set --dry-run` into the active file; the confirm box writes them (one write per OK, ADR 0007 Amendment 4)",
                ),
            };
            if ok.clicked() {
                actions.push(Action::OpenEditor {
                    command: "set".to_string(),
                    form: record.global.clone(),
                });
            }
            if ui
                .add_enabled(changed > 0, egui::Button::new("Reset"))
                .on_hover_text("drops the edits of this record")
                .clicked()
            {
                edits.0.clear();
            }
            // M7.E6: the editor's undo of its last write of this record.
            if ui
                .button("Undo last write")
                .on_hover_text(
                    "undoes the editor's last write of this record into the active file: `revert --dry-run` first, then a confirm",
                )
                .clicked()
            {
                actions.push(Action::OpenEditor {
                    command: "revert".to_string(),
                    form: record.global.clone(),
                });
            }
            ui.weak(format!(
                "{changed} changed; version: {}{}",
                version.plugin,
                if version.winner { " (winner)" } else { "" }
            ));
        });
        if let Some(why) = invalid {
            theme::status_line(ui, Status::Fail, &why);
        }
        ctx.data_mut(|d| d.insert_temp(id, edits));
        actions
    }
}

/// The edit box of one field: the text typed (starting from the value shown), and what it
/// was when it changed.
fn field_edit(ui: &mut egui::Ui, edits: &mut Edits, path: &str, s: &SubRow, number: bool) {
    let e = edits
        .0
        .entry(path.to_string())
        .or_insert_with(|| FieldEdit {
            shown: s.value.clone(),
            text: s.value.clone(),
            number,
        });
    // The record was read again with another value: an untouched box follows it.
    if e.shown != s.value && !e.changed() {
        e.shown.clone_from(&s.value);
        e.text.clone_from(&s.value);
    }
    // The box takes the row's width less room for the "was" note, so the row stays
    // inside the dialog.
    let note = theme::width(ui, theme::Field::Short);
    let r = ui.add(
        egui::TextEdit::singleline(&mut e.text)
            .font(egui::TextStyle::Monospace)
            .desired_width((ui.available_width() - note).max(note)),
    );
    let hover = format!(
        "`set` field {path}{}; OK previews it into the active file",
        if number { " (a number)" } else { "" }
    );
    r.on_hover_text(hover);
    if e.changed() {
        ui.add(egui::Label::new(egui::RichText::new(format!("was {}", e.shown)).weak()).truncate());
    }
}

/// The line that says where the layout comes from, that it is unobserved, and where the
/// edits go.
fn header(ui: &mut egui::Ui, layout: &Layout, record: &RecordView) {
    ui.horizontal_wrapped(|ui| {
        if layout.observed {
            theme::status(ui, Status::Pass, "observed layout");
        } else {
            theme::status(ui, Status::Warn, "built, unobserved").on_hover_text(
                "tab and group names are KitKraft's until CK sitting 2 (M7.O3) records the CK's (ADR 0012 Amendment 1)",
            );
        }
        ui.weak(&layout.source);
        theme::status(ui, Status::NotRun, "edits go to the active file").on_hover_text(
            "numbers and plain text edit in place; OK previews them with `set` into the active file (File > Data...) and a confirm box writes them",
        );
        if record.versions.len() > 1 {
            ui.weak(format!(
                "{} versions in the load order",
                record.versions.len()
            ));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(sig: &str, member: &str, value: &str) -> SubRow {
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

    /// Numbers and plain text edit in place; hex, lstrings, links, lists, structs, absent
    /// and truncated values do not.
    #[test]
    fn only_numbers_and_plain_text_are_edited_in_place() {
        assert_eq!(editable(&row("FLTV", "Value", "1.5")), Some(true));
        assert_eq!(editable(&row("DATA", "", "-3")), Some(true));
        assert_eq!(editable(&row("EDID", "EditorId", "Glob800")), Some(false));
        for v in [
            "hex 00AA",
            "opaque bytes 00",
            "(string 0x0002E469)",
            "(absent)",
            "[a, b]",
            "{X: 1}",
            "Starfield.esm:000123 (Thing)",
            "two\nlines",
            "cut...",
        ] {
            assert_eq!(editable(&row("FULL", "Name", v)), None, "{v}");
        }
        // Plain text needs the schema's member name (a known field).
        assert_eq!(editable(&row("XXXX", "", "text")), None);
    }

    /// `set` paths: the signature, `[n]` for the n-th repeat.
    #[test]
    fn a_repeated_signature_is_addressed_by_its_occurrence() {
        let v = VersionView {
            plugin: "Fixture.esm".into(),
            load_index: Some(0),
            winner: true,
            deleted: false,
            resolved_by: "direct".into(),
            raw_form_id: "00000800".into(),
            class: "first version".into(),
            changes: Vec::new(),
            flags: String::new(),
            subrecords: vec![
                row("EDID", "EditorId", "A"),
                row("FULL", "Name", "x"),
                row("FULL", "Name", "y"),
                row("FLTV", "Value", "1"),
            ],
        };
        assert_eq!(field_paths(&v), ["EDID", "FULL", "FULL[1]", "FLTV"]);
    }

    /// A numeric field goes to `set` as a number (an integer exactly), text as text; a
    /// numeric field that is not a number is refused before OK.
    #[test]
    fn edited_values_take_the_fields_type() {
        let e = |text: &str, number: bool| FieldEdit {
            shown: "1.5".into(),
            text: text.into(),
            number,
        };
        assert_eq!(e("7.25", true).value(), Ok(json!(7.25)));
        assert_eq!(e(" 42 ", true).value(), Ok(json!(42)));
        assert!(e("seven", true).value().is_err());
        assert!(e("inf", true).value().is_err());
        assert_eq!(e("Rifle", false).value(), Ok(json!("Rifle")));
        assert!(!e("1.5", true).changed());
    }
}
