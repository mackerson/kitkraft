//! The Cell View window (M7.W2): the engine's read-only `cell` command as a
//! CK-style window. The left list shows the worldspaces (with "Interior" as a
//! first entry), the interior cells, or one world's exterior cells with their
//! grid; the right list shows the references of the selected cell. Both lists
//! are `virtual_table`s, both paged. No engine logic here: every value comes
//! from the envelope (the engine's `CELL` data shape).

use serde_json::{json, Map, Value};

use crate::gate::Request;

/// A page's size (the `cell` command's cap).
const PAGE_LIMIT: u64 = 1000;

/// The mode of the left list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListMode {
    /// The worldspaces, with "Interior" as a first entry.
    #[default]
    Worlds,
    /// The interior cells.
    Interior,
    /// One world's exterior cells (with their grid).
    World,
}

/// One row of the left list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The row's formref (`None` for the first "Interior" entry).
    pub form: Option<String>,
    /// What the second column shows: the row's EditorID (or "Interior").
    pub editor_id: String,
    /// The cell's grid, for a world's exterior cells.
    pub grid: Option<String>,
    /// The row's kind, in the first column.
    pub kind: &'static str,
    /// The records inside, from the envelope's `child_records`.
    pub records: Option<u64>,
}

/// One reference of the selected cell.
#[derive(Debug, Clone, PartialEq)]
pub struct RefRow {
    pub form: String,
    pub signature: Option<String>,
    pub editor_id: Option<String>,
    /// The base's EditorID (or formref), as the envelope named it.
    pub base: Option<String>,
    /// "x, y, z", when the reference has a position.
    pub position: Option<String>,
    pub persistent: bool,
}

/// The window's state.
#[derive(Default)]
pub struct CellViewState {
    /// The mode of the list shown or pending, and the world a `World` list is of.
    pub list_mode: Option<ListMode>,
    pub list_world: Option<String>,
    /// The rows of the list shown.
    pub list: Vec<Entry>,
    /// The rows the list holds in total.
    pub list_total: u64,
    /// The cursor the next list page is asked with (from the last reply).
    pub list_next: Option<String>,
    /// A list request is in flight.
    pub list_pending: bool,
    /// The cursor the in-flight list request was asked with (`None`: a first page).
    list_page_cursor: Option<String>,
    /// The cell whose references are shown or pending, as a formref.
    pub cell: Option<String>,
    /// Its EditorID and world, as the reply named them.
    pub cell_editor_id: Option<String>,
    pub cell_world: Option<String>,
    /// The references shown.
    pub refs: Vec<RefRow>,
    /// The references the cell holds in total.
    pub refs_total: u64,
    /// The cursor the next references page is asked with (from the last reply).
    pub refs_next: Option<String>,
    /// A references request is in flight.
    pub refs_pending: bool,
    /// The cursor the in-flight references request was asked with.
    refs_page_cursor: Option<String>,
    /// Why there is nothing shown (the engine's error).
    pub error: Option<String>,
    /// The limitations of the last filed reply.
    pub limitations: Vec<String>,
}

impl CellViewState {
    /// The `cell` request for the worldspaces (the left list's first page),
    /// asking nothing while a list request is pending.
    pub fn worlds(&mut self) -> Option<Request> {
        if self.list_pending {
            return None;
        }
        self.begin_list(ListMode::Worlds, None);
        let req = self.list_request(None)?;
        self.list_pending = true;
        Some(req)
    }

    /// The `cell` request for the interior cells (the left list's first page).
    pub fn interiors(&mut self) -> Option<Request> {
        if self.list_pending {
            return None;
        }
        self.begin_list(ListMode::Interior, None);
        let req = self.list_request(None)?;
        self.list_pending = true;
        Some(req)
    }

    /// The `cell` request for `form`'s exterior cells (the left list's first
    /// page), asking nothing for the world already shown or pending.
    pub fn world(&mut self, form: &str) -> Option<Request> {
        if self.list_pending {
            return None;
        }
        if self.list_mode == Some(ListMode::World) && self.list_world.as_deref() == Some(form) {
            return None;
        }
        self.begin_list(ListMode::World, Some(form));
        let req = self.list_request(None)?;
        self.list_pending = true;
        Some(req)
    }

    /// The `cell` request for `form`'s references (the right list's first
    /// page), asking nothing for the cell already shown or pending.
    pub fn cell(&mut self, form: &str) -> Option<Request> {
        if self.refs_pending {
            return None;
        }
        if self.cell.as_deref() == Some(form) {
            return None;
        }
        self.begin_refs(form);
        let req = self.refs_request(None)?;
        self.refs_pending = true;
        Some(req)
    }

    /// The request for the next page of the list or references that `cursor`
    /// names (each side's "next page" button passes its own `*_next`); asking
    /// nothing when no side wants that cursor.
    pub fn page(&mut self, cursor: Option<String>) -> Option<Request> {
        let Some(cursor) = cursor else {
            return None;
        };
        if self.list_mode.is_some() && self.list_next.as_deref() == Some(cursor.as_str()) {
            let req = self.list_request(Some(cursor.clone()))?;
            self.list_page_cursor = Some(cursor);
            self.list_pending = true;
            Some(req)
        } else if self.cell.is_some() && self.refs_next.as_deref() == Some(cursor.as_str()) {
            let req = self.refs_request(Some(cursor.clone()))?;
            self.refs_page_cursor = Some(cursor);
            self.refs_pending = true;
            Some(req)
        } else {
            None
        }
    }

    fn begin_list(&mut self, mode: ListMode, world: Option<&str>) {
        self.list_mode = Some(mode);
        self.list_world = world.map(str::to_string);
        self.list.clear();
        self.list_total = 0;
        self.list_next = None;
        self.list_page_cursor = None;
    }

    fn begin_refs(&mut self, form: &str) {
        self.cell = Some(form.to_string());
        self.cell_editor_id = None;
        self.cell_world = None;
        self.refs.clear();
        self.refs_total = 0;
        self.refs_next = None;
        self.refs_page_cursor = None;
    }

    /// The `cell` request for a page (first when `cursor` is `None`) of the
    /// list `list_mode` and `list_world` describe.
    fn list_request(&self, cursor: Option<String>) -> Option<Request> {
        let mut args = Map::new();
        match self.list_mode {
            Some(ListMode::Interior) => {
                args.insert("interior".to_string(), json!(true));
            }
            Some(ListMode::World) => {
                args.insert("world".to_string(), json!(self.list_world.clone()?));
            }
            _ => {}
        }
        args.insert("limit".to_string(), json!(PAGE_LIMIT));
        if let Some(c) = cursor {
            args.insert("cursor".to_string(), json!(c));
        }
        Some(Request::new("cell", Value::Object(args)))
    }

    /// The `cell` request for a page (first when `cursor` is `None`) of
    /// `cell`'s references.
    fn refs_request(&self, cursor: Option<String>) -> Option<Request> {
        let mut args = Map::new();
        args.insert("cell".to_string(), json!(self.cell.clone()?));
        args.insert("limit".to_string(), json!(PAGE_LIMIT));
        if let Some(c) = cursor {
            args.insert("cursor".to_string(), json!(c));
        }
        Some(Request::new("cell", Value::Object(args)))
    }

    /// Takes the engine's reply, filed by its `data.mode`: a list reply files the
    /// left list (a first page replaces it, a later page appends), a `cell` reply
    /// files the right list the same way; the limitations come from the envelope,
    /// and an error clears the view and shows the message.
    pub fn receive(&mut self, result: Result<&Value, String>) {
        match result {
            Ok(env) => self.file(env),
            Err(e) => {
                self.list.clear();
                self.list_total = 0;
                self.list_next = None;
                self.list_page_cursor = None;
                self.list_pending = false;
                self.refs.clear();
                self.refs_total = 0;
                self.refs_next = None;
                self.refs_page_cursor = None;
                self.refs_pending = false;
                self.cell = None;
                self.cell_editor_id = None;
                self.cell_world = None;
                self.error = Some(e);
                self.limitations.clear();
            }
        }
    }

    /// Files a reply by its `data.mode`.
    fn file(&mut self, env: &Value) {
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
        let data = env.get("data").unwrap_or(&Value::Null);
        match data.get("mode").and_then(Value::as_str) {
            Some("worlds") => self.file_worlds(data),
            Some("interior") => self.file_interior(data),
            Some("world") => self.file_world(data),
            Some("cell") => self.file_cell(data),
            other => {
                self.list_pending = false;
                self.refs_pending = false;
                self.error = Some(format!(
                    "a reply in a mode this window does not know: {other:?}"
                ));
            }
        }
    }

    fn file_worlds(&mut self, data: &Value) {
        let append = self.list_page_cursor.is_some();
        self.list_pending = false;
        self.list_page_cursor = None;
        self.list_mode = Some(ListMode::Worlds);
        self.list_world = None;
        let rows: Vec<Entry> = items(data).into_iter().filter_map(world_entry).collect();
        // The first entry opens the interior cells (only on the first page).
        let mut entries = if append {
            Vec::new()
        } else {
            vec![Entry {
                form: None,
                editor_id: "Interior".to_string(),
                grid: None,
                kind: "interior",
                records: None,
            }]
        };
        entries.extend(rows);
        self.file_list(entries, data, append);
    }

    fn file_interior(&mut self, data: &Value) {
        let append = self.list_page_cursor.is_some();
        self.list_pending = false;
        self.list_page_cursor = None;
        self.list_mode = Some(ListMode::Interior);
        self.list_world = None;
        let entries: Vec<Entry> = items(data).into_iter().filter_map(interior_entry).collect();
        self.file_list(entries, data, append);
    }

    fn file_world(&mut self, data: &Value) {
        let append = self.list_page_cursor.is_some();
        self.list_pending = false;
        self.list_page_cursor = None;
        self.list_mode = Some(ListMode::World);
        self.list_world = text(data, "world");
        let entries: Vec<Entry> = items(data).into_iter().filter_map(world_cell_entry).collect();
        self.file_list(entries, data, append);
    }

    fn file_list(&mut self, entries: Vec<Entry>, data: &Value, append: bool) {
        if append {
            self.list.extend(entries);
        } else {
            self.list = entries;
        }
        self.list_total =
            u64_of(data, "total").unwrap_or(u64::try_from(self.list.len()).unwrap_or(0));
        self.list_next = text(data, "next_cursor");
    }

    fn file_cell(&mut self, data: &Value) {
        let append = self.refs_page_cursor.is_some();
        self.refs_pending = false;
        self.refs_page_cursor = None;
        let rows: Vec<RefRow> = items(data).into_iter().filter_map(ref_row).collect();
        if append {
            self.refs.extend(rows);
        } else {
            self.refs = rows;
        }
        if let Some(c) = text(data, "cell") {
            self.cell = Some(c);
        }
        self.cell_editor_id = text(data, "cell_editor_id");
        self.cell_world = text(data, "world");
        self.refs_total =
            u64_of(data, "total").unwrap_or(u64::try_from(self.refs.len()).unwrap_or(0));
        self.refs_next = text(data, "next_cursor");
    }
}

fn items(data: &Value) -> Vec<&Value> {
    data.get("items")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn u64_of(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_u64)
}

/// A worldspace row: its EditorID (or formref) and how many cells and records it holds.
fn world_entry(item: &Value) -> Option<Entry> {
    Some(Entry {
        form: text(item, "form"),
        editor_id: text(item, "editor_id").or_else(|| text(item, "form"))?,
        grid: None,
        kind: "world",
        records: u64_of(item, "child_records"),
    })
}

/// An interior cell row: its EditorID (or formref) and its records.
fn interior_entry(item: &Value) -> Option<Entry> {
    Some(Entry {
        form: text(item, "form"),
        editor_id: text(item, "editor_id").or_else(|| text(item, "form"))?,
        grid: None,
        kind: "interior cell",
        records: u64_of(item, "child_records"),
    })
}

/// A world's exterior cell row: its EditorID (or formref), its grid and its records.
fn world_cell_entry(item: &Value) -> Option<Entry> {
    Some(Entry {
        form: text(item, "form"),
        editor_id: text(item, "editor_id").or_else(|| text(item, "form"))?,
        grid: grid_of(item),
        kind: "cell",
        records: u64_of(item, "child_records"),
    })
}

/// The cell's grid, "x, y".
fn grid_of(item: &Value) -> Option<String> {
    let x = item.get("x")?.as_f64()?;
    let y = item.get("y")?.as_f64()?;
    Some(format!("{x:.0}, {y:.0}"))
}

/// A reference's position as "x, y, z" (a missing or unreadable slot as "?").
fn position_of(item: &Value) -> Option<String> {
    let a = item.get("position").and_then(Value::as_array)?;
    let slot = |i: usize| {
        a.get(i)
            .and_then(Value::as_f64)
            .filter(|f| f.is_finite())
            .map(|f| format!("{f:.0}"))
            .unwrap_or_else(|| "?".to_string())
    };
    Some(format!("{}, {}, {}", slot(0), slot(1), slot(2)))
}

/// One reference row.
fn ref_row(item: &Value) -> Option<RefRow> {
    let position = position_of(item);
    let base = item.get("base").and_then(|b| text(b, "form"));
    Some(RefRow {
        form: text(item, "form")?,
        signature: text(item, "signature"),
        editor_id: text(item, "editor_id"),
        base: base.or_else(|| text(item, "base_editor_id")),
        position,
        persistent: item.get("persistent").and_then(Value::as_bool) == Some(true),
    })
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui;

use crate::panels::common::virtual_table;
use crate::theme::{self, Status, View};

/// What the Cell View window asks for.
pub enum CellViewAsk {
    /// Show the worldspaces (the left list's first page).
    Worlds,
    /// Show the interior cells.
    Interiors,
    /// Show `form`'s exterior cells (the left list).
    World(String),
    /// Show `form`'s references (the right list).
    Cell(String),
    /// The next page of the list or references that `cursor` names.
    Page(Option<String>),
    /// A reference clicked: the shared selection.
    Select(String),
    /// A reference double-clicked: inspect it.
    Inspect(String),
}

/// The error to show: the engine's "no load order" says "load a profile first".
fn message_of(e: &str) -> &str {
    if e.starts_with("no load order") {
        "load a profile first"
    } else {
        e
    }
}

impl CellViewState {
    /// Draws the window's content: the list on the left, the selected cell's
    /// references on the right.
    pub fn ui(&self, ui: &mut egui::Ui, busy: bool) -> Option<CellViewAsk> {
        let mut ask = None;
        if let Some(e) = &self.error {
            theme::status_line(ui, Status::Fail, message_of(e));
            return None;
        }
        if self.list_pending || self.refs_pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(if self.list_pending {
                    "cell (a list page)"
                } else {
                    "cell (a references page)"
                });
            });
        }
        if self.list_mode.is_none() && self.cell.is_none() && !self.list_pending && !self.refs_pending
        {
            theme::status_line(ui, Status::NotRun, "load a profile first");
            return None;
        }
        self.limitations_block(ui);
        ui.horizontal(|ui| {
            self.left_list(ui, busy, &mut ask);
            ui.separator();
            self.right_refs(ui, busy, &mut ask);
        });
        ask
    }

    /// The limitations of the last filed reply, folded so the lists stay in view.
    fn limitations_block(&self, ui: &mut egui::Ui) {
        if self.limitations.is_empty() {
            return;
        }
        let n = self.limitations.len();
        egui::CollapsingHeader::new(format!(
            "limitations ({n}): what this result does not cover"
        ))
        .id_salt("cell-view-limitations")
        .show(ui, |ui| {
            eframe::egui::ScrollArea::vertical()
                .id_salt("cell-view-limitations-rows")
                .max_height(theme::view_height(ui, View::Short))
                .show(ui, |ui| {
                    for l in &self.limitations {
                        theme::status_line(ui, Status::Warn, l);
                    }
                });
        });
    }

    /// The list on the left: worldspaces (with "Interior" first), the interior
    /// cells, or one world's exterior cells with their grid, paged.
    fn left_list(
        &self,
        ui: &mut egui::Ui,
        busy: bool,
        ask: &mut Option<CellViewAsk>,
    ) {
        let heading = match self.list_mode {
            Some(ListMode::Worlds) => "worldspaces",
            Some(ListMode::Interior) => "interior cells",
            Some(ListMode::World) => "exterior cells",
            None => "list",
        };
        ui.horizontal_wrapped(|ui| {
            ui.strong(heading);
            if let Some(w) = &self.list_world {
                ui.monospace(w);
            }
            ui.weak(format!("{} row(s)", self.list_total));
        });
        virtual_table(
            ui,
            "cell-view-list",
            &["kind", "editor id", "grid", "refs"],
            self.list.len(),
            View::Tall,
            |row, i| {
                let Some(e) = self.list.get(i) else {
                    return;
                };
                row.cell(|ui| ui.label(e.kind));
                let id = row.cell(|ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(&e.editor_id).monospace())
                            .sense(egui::Sense::click()),
                    )
                });
                row.cell(|ui| ui.label(e.grid.as_deref().unwrap_or("")));
                row.num(e.records.map(|n| n.to_string()).unwrap_or_default());
                if id.clicked() {
                    *ask = Some(match (e.form.as_deref(), e.kind) {
                        (Some(f), "world") => CellViewAsk::World(f.to_string()),
                        (Some(f), _) => CellViewAsk::Cell(f.to_string()),
                        (None, _) => CellViewAsk::Interiors,
                    });
                }
            },
        );
        if let Some(next) = &self.list_next {
            if ui
                .add_enabled(!busy && !self.list_pending, egui::Button::new("next page"))
                .clicked()
            {
                *ask = Some(CellViewAsk::Page(Some(next.clone())));
            }
        }
    }

    /// The references on the right: the selected cell's, paged.
    fn right_refs(
        &self,
        ui: &mut egui::Ui,
        busy: bool,
        ask: &mut Option<CellViewAsk>,
    ) {
        let Some(c) = &self.cell else {
            theme::status_line(ui, Status::NotRun, "click a cell in the list");
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.strong(c);
            if let Some(e) = &self.cell_editor_id {
                ui.monospace(e);
            }
            if let Some(w) = &self.cell_world {
                ui.weak(w);
            }
            ui.weak(format!("{} reference(s)", self.refs_total));
        });
        virtual_table(
            ui,
            "cell-view-refs",
            &["editor id", "base", "type", "position", "persistent"],
            self.refs.len(),
            View::Tall,
            |row, i| {
                let Some(r) = self.refs.get(i) else {
                    return;
                };
                let id = row.cell(|ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(r.editor_id.as_deref().unwrap_or(&r.form))
                                .monospace(),
                        )
                        .sense(egui::Sense::click()),
                    )
                });
                row.cell(|ui| ui.label(r.base.as_deref().unwrap_or("—")));
                row.cell(|ui| ui.label(r.signature.as_deref().unwrap_or("—")));
                row.cell(|ui| ui.label(r.position.as_deref().unwrap_or("—")));
                row.cell(|ui| ui.label(if r.persistent { "persistent" } else { "" }));
                if id.double_clicked() {
                    *ask = Some(CellViewAsk::Inspect(r.form.clone()));
                } else if id.clicked() {
                    *ask = Some(CellViewAsk::Select(r.form.clone()));
                }
            },
        );
        if let Some(next) = &self.refs_next {
            if ui
                .add_enabled(!busy && !self.refs_pending, egui::Button::new("next page"))
                .clicked()
            {
                *ask = Some(CellViewAsk::Page(Some(next.clone())));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{check, check_shape, data_shape};
    use serde_json::json;

    /// An envelope the engine would return for `data` (checked against the `CELL`
    /// shape first, like `use_info.rs`'s test).
    fn env(data: Value) -> Value {
        let env = json!({"command": "cell", "data": data,
            "coverage": {"status": "full", "decoded_subrecords": null,
                         "opaque_subrecords": 0, "not_examined": []},
            "limitations": [], "evidence": []});
        assert!(check_shape(data_shape("cell").unwrap(), &env["data"], "data").is_empty());
        env
    }

    fn worlds_env() -> Value {
        env(json!({"mode": "worlds", "total": 2,
            "items": [
                {"form": "Starfield.esm:000001", "editor_id": "NewAtlantis", "cells": 1200,
                 "child_records": 34567, "versions": 1},
                {"form": "Starfield.esm:000002", "editor_id": "Vilkas", "cells": 400,
                 "child_records": 8000, "versions": 1}],
            "interior_cells": 25, "next_cursor": null,
            "summary": "2 worlds", "elapsed_ms": 1, "index_ms": 2}))
    }

    fn interiors_env() -> Value {
        env(json!({"mode": "interior", "total": 2,
            "items": [
                {"form": "Starfield.esm:21966F",
                 "editor_id": "CityNewAtlantisApartmentTEMPLATE", "child_records": 45,
                 "versions": 1},
                {"form": "Starfield.esm:219670", "editor_id": "CityNewAtlantisOfficeTEMPLATE",
                 "child_records": 12, "versions": 1}],
            "next_cursor": null, "summary": "2 interiors", "elapsed_ms": 1, "index_ms": 1}))
    }

    fn world_env() -> Value {
        env(json!({"mode": "world", "total": 2,
            "items": [
                {"form": "Starfield.esm:219671", "editor_id": "NA_00_00", "x": 0, "y": 0,
                 "child_records": 10, "versions": 1},
                {"form": "Starfield.esm:219672", "editor_id": "NA_00_01", "x": 0, "y": 1,
                 "child_records": 8, "versions": 1}],
            "world": "Starfield.esm:000001", "world_editor_id": "NewAtlantis",
            "next_cursor": null, "summary": "2 cells", "elapsed_ms": 1, "index_ms": 1}))
    }

    fn cell_page1_env() -> Value {
        env(json!({"mode": "cell", "total": 4,
            "items": [
                {"form": "Starfield.esm:0A6F06", "signature": "REFR",
                 "editor_id": "WorkshopLight01",
                 "base": {"kind": "formref", "form": "Starfield.esm:00375C",
                          "raw": "0000375C"},
                 "base_signature": "STAT", "base_editor_id": "LP_Lights_Vol",
                 "position": [4, 0, 5], "rotation": null, "scale": 1,
                 "persistent": false, "group": "temporary", "flags": "",
                 "deleted": false, "plugin": "Starfield.esm", "versions": 1},
                {"form": "Starfield.esm:0A6F07", "signature": "ACHR",
                 "editor_id": "ResidentNPC", "base": null, "position": [1, 2, 3],
                 "scale": 2, "persistent": true, "group": "persistent", "versions": 1}],
            "cell": "Starfield.esm:21966F",
            "cell_editor_id": "CityNewAtlantisApartmentTEMPLATE", "world": null,
            "next_cursor": "c1", "summary": "4 references", "elapsed_ms": 2, "index_ms": 1}))
    }

    fn cell_page2_env() -> Value {
        env(json!({"mode": "cell", "total": 4,
            "items": [
                {"form": "Starfield.esm:0A6F08", "signature": "MISC",
                 "editor_id": "Crate01", "base": null, "position": null,
                 "persistent": false, "versions": 1},
                {"form": "Starfield.esm:0A6F09", "signature": "MISC",
                 "editor_id": "Crate02", "base": null, "position": [7, 8, 9],
                 "persistent": false, "versions": 1}],
            "cell": "Starfield.esm:21966F",
            "cell_editor_id": "CityNewAtlantisApartmentTEMPLATE", "world": null,
            "next_cursor": null, "summary": "4 references", "elapsed_ms": 1, "index_ms": 1}))
    }

    #[test]
    fn cell_view_lists_worlds_interiors_and_a_worlds_cells() {
        let mut s = CellViewState::default();
        let req = s.worlds().unwrap();
        assert_eq!(req.id, "cell");
        assert_eq!(req.args["limit"], json!(1000));
        assert!(check(&req).is_ok(), "the gate runs it");
        assert!(s.list_pending);
        // A second `worlds()` while one is pending asks nothing.
        assert!(s.worlds().is_none());
        s.receive(Ok(&worlds_env()));
        assert!(!s.list_pending);
        assert_eq!(s.list.len(), 3);
        assert_eq!(s.list[0].editor_id, "Interior");
        assert!(s.list[0].form.is_none());
        assert_eq!(s.list[1].editor_id, "NewAtlantis");
        assert_eq!(s.list[1].form.as_deref(), Some("Starfield.esm:000001"));
        assert_eq!(s.list[1].records, Some(34567));
        assert_eq!(s.list_total, 2);
        assert!(s.list_next.is_none());

        let req = s.interiors().unwrap();
        assert_eq!(req.args["interior"], json!(true));
        assert!(check(&req).is_ok());
        assert!(s.list_pending);
        s.receive(Ok(&interiors_env()));
        assert_eq!(s.list.len(), 2);
        assert_eq!(s.list[0].kind, "interior cell");
        assert_eq!(s.list[0].form.as_deref(), Some("Starfield.esm:21966F"));
        assert_eq!(s.list[0].records, Some(45));

        let req = s.world("Starfield.esm:000001").unwrap();
        assert_eq!(req.args["world"], json!("Starfield.esm:000001"));
        assert!(check(&req).is_ok());
        s.receive(Ok(&world_env()));
        assert_eq!(s.list.len(), 2);
        assert_eq!(s.list[0].grid.as_deref(), Some("0, 0"));
        assert_eq!(s.list[1].grid.as_deref(), Some("0, 1"));
        assert_eq!(s.list_world.as_deref(), Some("Starfield.esm:000001"));
        // The world already shown asks nothing again.
        assert!(s.world("Starfield.esm:000001").is_none());
    }

    #[test]
    fn cell_view_reads_a_cells_references_and_pages() {
        let mut s = CellViewState::default();
        let req = s.cell("Starfield.esm:21966F").unwrap();
        assert_eq!(req.id, "cell");
        assert_eq!(req.args["cell"], json!("Starfield.esm:21966F"));
        assert!(check(&req).is_ok(), "the gate runs it");
        assert!(s.refs_pending);
        // The cell already pending asks nothing again.
        assert!(s.cell("Starfield.esm:21966F").is_none());
        s.receive(Ok(&cell_page1_env()));
        assert!(!s.refs_pending);
        assert_eq!(s.refs.len(), 2);
        assert_eq!(s.refs[0].editor_id.as_deref(), Some("WorkshopLight01"));
        assert_eq!(s.refs[0].base.as_deref(), Some("Starfield.esm:00375C"));
        assert_eq!(s.refs[0].position.as_deref(), Some("4, 0, 5"));
        assert!(!s.refs[0].persistent);
        assert_eq!(s.refs[1].position.as_deref(), Some("1, 2, 3"));
        assert!(s.refs[1].persistent);
        assert_eq!(s.refs_total, 4);
        assert_eq!(s.refs_next.as_deref(), Some("c1"));
        assert_eq!(s.cell_editor_id.as_deref(), Some("CityNewAtlantisApartmentTEMPLATE"));
        assert!(s.cell_world.is_none());

        // No side wants a cursor it has not been given.
        assert!(s.page(None).is_none());
        assert!(s.page(Some("other".into())).is_none());
        let req = s.page(Some("c1".into())).unwrap();
        assert_eq!(req.args["cursor"], json!("c1"));
        assert_eq!(req.args["cell"], json!("Starfield.esm:21966F"));
        assert!(check(&req).is_ok());
        assert!(s.refs_pending);
        s.receive(Ok(&cell_page2_env()));
        assert_eq!(s.refs.len(), 4);
        assert_eq!(s.refs[3].editor_id.as_deref(), Some("Crate02"));
        assert_eq!(s.refs[3].position.as_deref(), Some("7, 8, 9"));
        assert!(s.refs_next.is_none());

        // An error clears the view and shows the message.
        s.receive(Err("no load order: pass --profile <MO2 profile dir> \
                       or --plugins <file>..."
            .into()));
        assert!(s.refs.is_empty() && s.list.is_empty());
        assert!(s.cell.is_none() && s.cell_editor_id.is_none());
        assert_eq!(
            s.error.as_deref(),
            Some("no load order: pass --profile <MO2 profile dir> \
                  or --plugins <file>...")
        );
    }
}
