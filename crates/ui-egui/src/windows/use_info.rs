//! The Use Info window (M7.W5): the records that use the selected record, from
//! `refs --incoming` (the same request and view-model as the Links tab's "use info";
//! `crate::panels::links`), as a CK-style list: referrer, its type and EditorID, the
//! member that names the form, the plugin, and the cell of a placed referrer.
//! Double-click a row to inspect the referrer. It follows the Records tab's inspected
//! record while open, one request per new selection.

use serde_json::Value;

use crate::gate::Request;
use crate::panels::links::{LinksForm, LinksView};

/// The window's state.
#[derive(Default)]
pub struct UseInfoState {
    /// The form the shown (or pending) result is for.
    pub form: String,
    pub view: Option<LinksView>,
    /// Why there is no view (the engine's error), or its limitations.
    pub error: Option<String>,
    pub limitations: Vec<String>,
    /// Cursor of the page shown (`None`: the first).
    pub cursor: Option<String>,
    pub pending: bool,
}

impl UseInfoState {
    /// The `refs --incoming` request for `form`'s first page, when `form` is not the one
    /// already shown or pending (the window follows the selection once per change).
    pub fn follow(&mut self, form: &str) -> Option<Request> {
        let form = form.trim();
        if form.is_empty() || form == self.form {
            return None;
        }
        let req = LinksForm::use_info(form).request(None).ok()?;
        self.form = form.to_string();
        self.cursor = None;
        self.pending = true;
        Some(req)
    }

    /// The request for another page of the shown form.
    pub fn page(&mut self, cursor: Option<String>) -> Option<Request> {
        let req = LinksForm::use_info(&self.form)
            .request(cursor.as_deref())
            .ok()?;
        self.cursor = cursor;
        self.pending = true;
        Some(req)
    }

    /// Takes the engine's reply.
    pub fn receive(&mut self, result: Result<&Value, String>) {
        self.pending = false;
        match result {
            Ok(env) => {
                self.view = Some(LinksView::from_envelope(env));
                self.error = None;
                self.limitations = env
                    .get("limitations")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
            }
            Err(e) => {
                self.view = None;
                self.error = Some(e);
                self.limitations.clear();
            }
        }
    }
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, ScrollArea};

use crate::theme::{self, Status, View};

/// What the Use Info window asks for.
pub enum UseInfoAsk {
    Inspect(String),
    Page(Option<String>),
}

impl UseInfoState {
    /// Draws the window's content; `selected` is the inspected record (or none).
    pub fn ui(&self, ui: &mut egui::Ui, selected: Option<&str>, busy: bool) -> Option<UseInfoAsk> {
        let mut ask = None;
        let Some(sel) = selected else {
            theme::status_line(
                ui,
                Status::NotRun,
                "no record selected: inspect one in the Records tab (or double-click a warning)",
            );
            return None;
        };
        if self.pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("refs --incoming {sel}"));
            });
        }
        if let Some(e) = &self.error {
            theme::status_line(ui, Status::Fail, e);
            return None;
        }
        let Some(v) = &self.view else {
            return None;
        };
        ui.horizontal_wrapped(|ui| {
            ui.strong(&v.label);
            ui.monospace(&v.global);
            ui.weak(format!("used by {} ({})", v.total, v.counts));
        });
        // Always listed, folded so the referrers stay in view.
        let n = self
            .limitations
            .len()
            .saturating_add(usize::from(v.not_searched.is_some()));
        if n > 0 {
            egui::CollapsingHeader::new(format!(
                "limitations ({n}): what this result does not cover"
            ))
            .id_salt("use-info-limitations")
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("use-info-limitations-rows")
                    .max_height(theme::view_height(ui, View::Short))
                    .show(ui, |ui| {
                        for l in &self.limitations {
                            theme::status_line(ui, Status::Warn, l);
                        }
                        if let Some(n) = &v.not_searched {
                            theme::status_line(ui, Status::Unknown, n);
                        }
                    });
            });
        }
        if v.unrecognized > 0 {
            theme::status_line(
                ui,
                Status::Unknown,
                &format!(
                    "{} row(s) of a kind this window does not know",
                    v.unrecognized
                ),
            );
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !busy && self.cursor.is_some(),
                    egui::Button::new("first page"),
                )
                .clicked()
            {
                ask = Some(UseInfoAsk::Page(None));
            }
            if let Some(next) = &v.next_cursor {
                if ui
                    .add_enabled(!busy, egui::Button::new("next page"))
                    .clicked()
                {
                    ask = Some(UseInfoAsk::Page(Some(next.clone())));
                }
            }
            if v.truncated {
                theme::status(ui, Status::Warn, "the walk stopped at its cap");
            }
        });
        if v.edges.is_empty() {
            theme::status_line(ui, Status::Pass, "no record in the load order names it");
            return ask;
        }
        ui.separator();
        ScrollArea::both()
            .id_salt("use-info-rows")
            .auto_shrink([false, false])
            .max_height(theme::view_height(ui, View::Tall))
            .show(ui, |ui| {
                theme::table(ui, "use-info-grid").show(ui, |ui| {
                    for h in ["referrer", "form", "member", "plugin", "placed in"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for e in v.edges.iter().take(1000) {
                        let a = ui.add(
                            egui::Label::new(egui::RichText::new(&e.from_label).monospace())
                                .sense(egui::Sense::click()),
                        );
                        let b = ui.add(
                            egui::Label::new(
                                egui::RichText::new(e.from.as_deref().unwrap_or("unresolved"))
                                    .monospace(),
                            )
                            .sense(egui::Sense::click()),
                        );
                        ui.monospace(&e.path);
                        ui.label(if e.winner {
                            e.plugin.clone()
                        } else {
                            format!("{} (not the winner)", e.plugin)
                        });
                        ui.label(e.placed.as_deref().unwrap_or(""));
                        ui.end_row();
                        if let Some(f) = &e.from {
                            let hover = "double-click: inspect the referrer";
                            if a.on_hover_text(hover).double_clicked()
                                || b.on_hover_text(hover).double_clicked()
                            {
                                ask = Some(UseInfoAsk::Inspect(f.clone()));
                            }
                        }
                    }
                });
            });
        ask
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{check, check_shape, data_shape};
    use serde_json::json;

    fn incoming() -> Value {
        json!({"command": "refs", "data": {
            "form": "Kw", "matched_by": "editor_id", "global": "Base.esm:000800",
            "signature": "KYWD", "editor_id": "Kw", "direction": "incoming", "depth": 1, "scope": "winner",
            "total": 1, "counts": {"referrers": 1, "placed": 0, "by_signature": {"WEAP": 1}},
            "edges": [
                {"kind": "referrer", "depth": 1, "group": "referrer", "from": "Base.esm:000801",
                 "from_raw": "00000801", "from_signature": "WEAP", "from_editor_id": "Rifle",
                 "plugin": "Patch.esm", "winner": true, "to": "Base.esm:000800", "placed": null,
                 "path": "KWDA.Forms[0]", "raw": "01000800", "status": "resolved",
                 "resolved_by": "direct", "offset": 400, "body_offset": 20}],
            "next_cursor": null, "truncated": false, "summary": "...", "elapsed_ms": 3,
            "placed_by_cell": [], "not_searched": {"no_definition": [], "records_with_raw_subrecords": 0},
            "index": {"plugins": 2, "cache_hits": 0, "built": 2, "uncached": 0, "elapsed_ms": 1}},
            "coverage": {"status": "partial", "decoded_subrecords": null, "opaque_subrecords": 0, "not_examined": []},
            "limitations": ["the reverse index skips 3 records"], "evidence": []})
    }

    #[test]
    fn it_follows_the_selection_once_and_reads_referrers() {
        let problems = check_shape(data_shape("refs").unwrap(), &incoming()["data"], "data");
        assert!(problems.is_empty(), "{problems:#?}");
        let mut s = UseInfoState::default();
        let req = s.follow("Base.esm:000800").unwrap();
        assert_eq!(req.id, "refs");
        assert_eq!(req.args["incoming"], json!(true));
        assert_eq!(req.args["form"], json!("Base.esm:000800"));
        assert!(check(&req).is_ok(), "the gate runs it");
        assert!(s.pending);
        // The same selection asks nothing more; a page or a new one does.
        assert!(s.follow("Base.esm:000800").is_none());
        let page = s.page(Some("c2".into())).unwrap();
        assert_eq!(page.args["cursor"], json!("c2"));
        assert!(
            s.follow("Base.esm:000800").is_none(),
            "paging keeps the selection"
        );
        s.receive(Ok(&incoming()));
        assert!(!s.pending);
        let v = s.view.as_ref().unwrap();
        assert_eq!(v.edges.len(), 1);
        assert_eq!(v.edges[0].from.as_deref(), Some("Base.esm:000801"));
        assert_eq!(v.edges[0].from_label, "WEAP Rifle");
        assert_eq!(s.limitations, vec!["the reverse index skips 3 records"]);
        s.receive(Err("not_found: no such form".into()));
        assert!(s.view.is_none() && s.error.is_some());
        assert!(s.follow("  ").is_none());
    }
}
