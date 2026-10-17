//! The generic read-only edit dialog (M7.W3): the selected record's winning version,
//! its typed fields grouped per the type's layout (`super::Layout`), one tab at a time.
//! Labels are the schema's member names (with the signature beside them), never CK
//! text; values are the envelope's, formatted as the Records inspector formats them.

use eframe::egui::{self, ScrollArea};

use super::{place, DialogState, Layout};
use crate::gate::Request;
use crate::panels::records::{RecordView, VersionView};
use crate::theme::{self, Status, View};

/// The version the dialog shows: the winner, else the last one listed.
pub fn shown_version(record: &RecordView) -> Option<&VersionView> {
    record
        .versions
        .iter()
        .find(|v| v.winner)
        .or_else(|| record.versions.last())
}

impl DialogState {
    /// Draws the dialog for `record`; returns a `schema` request when the type needs a
    /// default layout and has none yet.
    pub fn ui(&mut self, ui: &mut egui::Ui, record: Option<&RecordView>) -> Option<Request> {
        let Some(record) = record.filter(|r| !r.signature.is_empty()) else {
            theme::status_line(
                ui,
                Status::NotRun,
                "no record selected: inspect one in the Records tab (or double-click a Use Info row)",
            );
            return None;
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
                return None;
            }
            None => {
                let req = self.schema_request(sig);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!(
                        "building a default layout from `schema --type {sig}`"
                    ));
                });
                return req;
            }
        };
        header(ui, &layout, record);
        let Some(version) = shown_version(record) else {
            theme::status_line(ui, Status::Unknown, "the envelope lists no version");
            return None;
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
        let footer = ui.spacing().interact_size.y * 2.5;
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
                    // the value wrapping in the rest of the width.
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
                                let value = ui.add(
                                    egui::Label::new(egui::RichText::new(&s.value).monospace())
                                        .wrap_mode(egui::TextWrapMode::Wrap),
                                );
                                if let Some(o) = s.offset {
                                    value.on_hover_text(format!(
                                        "at {o} (0x{o:X}), {} bytes; read-only",
                                        s.size.map_or_else(|| "?".to_string(), |n| n.to_string())
                                    ));
                                }
                            });
                        });
                    }
                    theme::gap(ui);
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            ui.add_enabled(false, egui::Button::new("OK")).on_disabled_hover_text(
                "read-only: field editing comes with M7.E2, after the owner rules on ADR 0007 Amendment 4 (M7.O4)",
            );
            ui.weak(format!(
                "version: {}{}",
                version.plugin,
                if version.winner { " (winner)" } else { "" }
            ));
        });
        None
    }
}

/// The line that says where the layout comes from and that it is unobserved.
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
        theme::status(ui, Status::NotRun, "read-only");
        if record.versions.len() > 1 {
            ui.weak(format!(
                "{} versions in the load order",
                record.versions.len()
            ));
        }
    });
}
