//! The Object Window (M7.W1, pipeline W): the category tree from `schema` (M7.S2), a
//! virtual list of one record type's records from `query`, a text filter, and a
//! double-click that opens the record's edit dialog; read-only. Pipeline E's
//! `new`/`clone` hooks (M7.E3) sit in it.
//!
//! What a sitting observed (`docs/ck-parity.md` row "Object Window (category tree, list,
//! filter)", `ck-20261101-03`) and what is built to it: a category tree with "*All" at the
//! top, a "Text Filter" box, and a list of the EditorID, the FormID and the type. Built but
//! not observed (KitKraft's own until sitting 1's S1-09 records the tree): the categories
//! below "*All" (every type is in "Other" until then, as `schema` says), each type as a
//! node of its category, the Filter and page buttons, and the "Plugin" column. Observed
//! but not built: "*All" listing every form at once (`query` takes one type, so "*All"
//! asks for a type), the use count column (a `refs --incoming` per row), and dragging a
//! row into the Render Window (placement is M7.E4).
//!
//! The filter is the engine's: a non-empty Text Filter asks `query` for the type's records
//! whose EditorID contains it (`EDID ~ "<text>"`, ASCII case-insensitive), so it covers
//! every record of the type, not only the page shown.

use eframe::egui::{self, ScrollArea};
use serde_json::{json, Value};

use crate::envelope::{list, text, text_or_empty};
use crate::gate::Request;
use crate::panels::common::{Action, Slot};
use crate::panels::records::{QueryForm, QueryView};
use crate::theme::{self, Status, View};
use crate::windows::WindowKind;

/// The tree's top node, as the CK names it (observed).
pub const ALL: &str = "*All";

/// The category `schema` gives every type no sitting observed (KitKraft's name).
const OTHER: &str = "Other";

/// Records per `query` page.
const PAGE: u32 = 1000;

/// What an Object Window request sent through the shell's tickets was for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectTicket {
    /// The category tree (`schema`).
    Categories,
    /// One type's records (`query`).
    List,
}

/// One category of the tree, with its types `(signature, name)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub name: String,
    pub types: Vec<(String, String)>,
}

/// The Object Window's state (kept in `crate::windows::Shell`).
#[derive(Debug, Default)]
pub struct ObjectWindowState {
    /// The tree, in `schema`'s order (the observed categories first, "Other" last).
    pub categories: Vec<Category>,
    /// What `schema` says of the tree (that it is unobserved).
    pub limitations: Vec<String>,
    pub categories_pending: bool,
    pub categories_error: Option<String>,
    /// The selected node: [`ALL`] or a record type.
    pub node: Option<String>,
    /// The Text Filter.
    pub filter: String,
    /// The type of the shown (or pending) list.
    pub listed: Option<String>,
    pub list: Option<QueryView>,
    pub list_error: Option<String>,
    pub list_pending: bool,
    /// The cursor of the page shown (`None`: the first).
    pub cursor: Option<String>,
    /// The selected row's formref.
    pub selected: Option<String>,
    /// A request for the shell to send ([`ObjectWindowState::take_request`]).
    pub outbox: Option<(ObjectTicket, Request)>,
}

/// What a click on a row asks for.
enum RowAsk {
    Select(String),
    Open(String),
}

/// `schema`'s categories, in the order their first type gives, "Other" last.
fn categories_of(env: &Value) -> Vec<Category> {
    let data = env.get("data").unwrap_or(&Value::Null);
    let mut out: Vec<Category> = Vec::new();
    for r in list(data, "records").iter().take(4096) {
        let sig = text_or_empty(r, "type");
        if sig.is_empty() {
            continue;
        }
        let cat = text(r, "category").unwrap_or_else(|| OTHER.to_string());
        let entry = (sig, text_or_empty(r, "name"));
        match out.iter_mut().find(|c| c.name == cat) {
            Some(c) => c.types.push(entry),
            None => out.push(Category {
                name: cat,
                types: vec![entry],
            }),
        }
    }
    if let Some(i) = out.iter().position(|c| c.name == OTHER) {
        let other = out.remove(i);
        out.push(other);
    }
    out
}

impl ObjectWindowState {
    /// The request the shell should send now, if any.
    pub fn take_request(&mut self) -> Option<(ObjectTicket, Request)> {
        self.outbox.take()
    }

    /// The window is shown: asks for the tree once.
    pub fn shown(&mut self) {
        if self.categories.is_empty() && !self.categories_pending && self.categories_error.is_none()
        {
            self.categories_pending = true;
            self.outbox = Some((ObjectTicket::Categories, Request::new("schema", json!({}))));
        }
    }

    /// Selects tree node `node` ([`ALL`] or a record type) and asks for its first page.
    pub fn select_node(&mut self, node: &str) {
        self.node = Some(node.to_string());
        self.selected = None;
        if node == ALL {
            self.list = None;
            self.list_error = None;
            self.listed = None;
            return;
        }
        self.request_page(None);
    }

    /// The `query` of the selected type with the filter, from `cursor`.
    fn request_page(&mut self, cursor: Option<String>) {
        let Some(ty) = self.node.clone().filter(|n| n != ALL) else {
            return;
        };
        let filter = self.filter.trim().to_string();
        if filter.contains('"') || filter.contains('\\') {
            self.list_error = Some("the Text Filter takes no `\"` or `\\`".to_string());
            return;
        }
        let form = QueryForm {
            record_type: ty.clone(),
            where_text: if filter.is_empty() {
                String::new()
            } else {
                format!("EDID ~ \"{filter}\"")
            },
            all_versions: false,
            limit: PAGE,
        };
        match form.request(cursor.as_deref()) {
            Ok(req) => {
                self.listed = Some(ty);
                self.cursor = cursor;
                self.list_pending = true;
                self.list_error = None;
                self.outbox = Some((ObjectTicket::List, req));
            }
            Err(e) => self.list_error = Some(e),
        }
    }

    /// The Text Filter is applied (Enter, or the Filter button): the list again.
    pub fn apply_filter(&mut self) {
        self.selected = None;
        self.request_page(None);
    }

    /// Opens row `form` in the edit dialog (the CK's double-click).
    pub(crate) fn open(&mut self, form: &str) -> Vec<Action> {
        self.selected = Some(form.to_string());
        vec![
            Action::Inspect(form.to_string()),
            Action::OpenWindow(WindowKind::EditDialog),
        ]
    }

    /// Takes the reply to a request sent with `Pending::Object(ticket)`.
    pub fn receive(&mut self, ticket: &ObjectTicket, result: Result<&Value, String>) {
        match ticket {
            ObjectTicket::Categories => {
                self.categories_pending = false;
                match result {
                    Ok(env) => {
                        self.categories = categories_of(env);
                        self.limitations = crate::envelope::strings(env, "limitations")
                            .into_iter()
                            .filter(|l| l.contains("Object Window categories"))
                            .collect();
                        self.categories_error = None;
                    }
                    Err(e) => self.categories_error = Some(e),
                }
            }
            ObjectTicket::List => {
                self.list_pending = false;
                match result {
                    Ok(env) => {
                        self.list = Some(QueryView::from_envelope(env));
                        self.list_error = None;
                    }
                    Err(e) => {
                        self.list = None;
                        self.list_error = Some(e);
                    }
                }
            }
        }
    }

    /// Takes a successful reply that came back on `Slot::ObjectCategories` or
    /// `Slot::ObjectList` (the window sends neither; it uses tickets).
    pub(crate) fn receive_slot(&mut self, slot: Slot, _request: &Request, env: &Value) {
        match slot {
            Slot::ObjectCategories => self.receive(&ObjectTicket::Categories, Ok(env)),
            Slot::ObjectList => self.receive(&ObjectTicket::List, Ok(env)),
            _ => {}
        }
    }

    /// Draws the window's content; returns the actions it asks for. Pipeline E's
    /// `new`/`clone` controls (M7.E3) come from its one hook here.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, busy: bool) -> Vec<Action> {
        let mut actions = crate::dialogs::new::object_window_actions(ui, self.selected.as_deref());
        ui.horizontal(|ui| {
            let label = ui.label("Text Filter");
            let edit = ui
                .add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .hint_text("part of an EditorID")
                        .desired_width(theme::combo_width(ui)),
                )
                .labelled_by(label.id);
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.add_enabled(!busy, egui::Button::new("Filter")).clicked() || enter {
                self.apply_filter();
            }
        });
        for l in &self.limitations {
            theme::status_line(ui, Status::Unknown, l);
        }
        let height = theme::view_height(ui, View::Tall);
        ui.columns(2, |cols| {
            let [left, right] = cols else {
                return;
            };
            ScrollArea::vertical()
                .id_salt("object-window-tree")
                .auto_shrink([false, false])
                .max_height(height)
                .show(left, |ui| self.tree_ui(ui));
            actions.extend(self.list_ui(right, height, busy));
        });
        actions
    }

    fn tree_ui(&mut self, ui: &mut egui::Ui) {
        if self.categories_pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("schema");
            });
        }
        if let Some(e) = &self.categories_error {
            theme::status_line(ui, Status::Fail, e);
        }
        let mut pick = None;
        if ui
            .selectable_label(self.node.as_deref() == Some(ALL), ALL)
            .clicked()
        {
            pick = Some(ALL.to_string());
        }
        for c in &self.categories {
            let title = if c.name == OTHER {
                format!("{OTHER} (unobserved)")
            } else {
                c.name.clone()
            };
            egui::CollapsingHeader::new(title)
                .id_salt(("object-window-category", c.name.as_str()))
                .default_open(self.categories.len() == 1)
                .show(ui, |ui| {
                    for (sig, name) in &c.types {
                        let on = self.node.as_deref() == Some(sig.as_str());
                        if ui.selectable_label(on, format!("{sig} {name}")).clicked() {
                            pick = Some(sig.clone());
                        }
                    }
                });
        }
        if let Some(p) = pick {
            self.select_node(&p);
        }
    }

    fn list_ui(&mut self, ui: &mut egui::Ui, height: f32, busy: bool) -> Vec<Action> {
        match self.node.as_deref() {
            None => {
                ui.weak("pick a type in the tree");
                return Vec::new();
            }
            Some(ALL) => {
                theme::status_line(
                    ui,
                    Status::NotRun,
                    "*All: the CK lists every form here; KitKraft's `query` takes one type, so pick one below *All",
                );
                return Vec::new();
            }
            Some(_) => {}
        }
        if self.list_pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("query");
            });
        }
        if let Some(e) = &self.list_error {
            theme::status_line(ui, Status::Fail, e);
            return Vec::new();
        }
        let Some(v) = self.list.as_ref() else {
            return Vec::new();
        };
        let mut ask = None;
        let mut page = None;
        ui.horizontal(|ui| {
            ui.weak(format!("{} row(s) on this page", v.rows.len()));
            if ui
                .add_enabled(
                    !busy && self.cursor.is_some(),
                    egui::Button::new("first page"),
                )
                .clicked()
            {
                page = Some(None);
            }
            if let Some(next) = &v.next_cursor {
                if ui
                    .add_enabled(!busy, egui::Button::new("next page"))
                    .clicked()
                {
                    page = Some(Some(next.clone()));
                }
            }
        });
        let ty = self.listed.clone().unwrap_or_default();
        let row_h = ui.text_style_height(&egui::TextStyle::Body).max(1.0);
        let selected = self.selected.as_deref();
        ui.strong("EditorID, FormID, Type, Plugin");
        ScrollArea::vertical()
            // A new type or page starts at the top (the id follows the list shown).
            .id_salt(("object-window-rows", ty.as_str(), self.cursor.as_deref()))
            .auto_shrink([false, false])
            .max_height(height)
            .show_rows(ui, row_h, v.rows.len(), |ui, range| {
                for r in v.rows.iter().skip(range.start).take(range.len()) {
                    ui.horizontal(|ui| {
                        let on = selected == Some(r.form.as_str());
                        let label = if r.editor_id.is_empty() {
                            "(no EditorID)"
                        } else {
                            r.editor_id.as_str()
                        };
                        let name = ui.selectable_label(on, label);
                        ui.monospace(&r.form);
                        ui.label(&ty);
                        ui.weak(&r.plugin);
                        if name.double_clicked() {
                            ask = Some(RowAsk::Open(r.form.clone()));
                        } else if name.clicked() && ask.is_none() {
                            ask = Some(RowAsk::Select(r.form.clone()));
                        }
                    });
                }
            });
        if let Some(p) = page {
            self.request_page(p);
        }
        match ask {
            Some(RowAsk::Open(form)) => self.open(&form),
            Some(RowAsk::Select(form)) => {
                self.selected = Some(form);
                Vec::new()
            }
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn schema_env() -> Value {
        json!({"data": {"records": [
            {"type": "WEAP", "name": "Weapon", "category": "Other"},
            {"type": "GLOB", "name": "Global", "category": "Other"},
            {"type": "AMMO", "name": "Ammo", "category": "Items"},
        ]}, "limitations": ["definitions only", "Object Window categories: only *All"]})
    }

    #[test]
    fn the_tree_comes_from_schema_with_other_last() {
        let mut s = ObjectWindowState::default();
        s.shown();
        let (t, req) = s.take_request().unwrap();
        assert_eq!((t, req.id.as_str()), (ObjectTicket::Categories, "schema"));
        crate::gate::check(&req).unwrap();
        s.shown();
        assert!(s.take_request().is_none(), "asked once");
        s.receive(&ObjectTicket::Categories, Ok(&schema_env()));
        let names: Vec<&str> = s.categories.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Items", "Other"]);
        assert_eq!(s.categories[1].types.len(), 2);
        assert_eq!(s.limitations.len(), 1, "the unobserved tree is said");
    }

    #[test]
    fn a_type_lists_its_records_and_the_filter_asks_the_engine() {
        let mut s = ObjectWindowState::default();
        s.select_node("GLOB");
        let (t, req) = s.take_request().unwrap();
        assert_eq!(t, ObjectTicket::List);
        assert_eq!(req.args["type"], "GLOB");
        assert!(req.args.get("where").is_none());
        crate::gate::check(&req).unwrap();
        s.filter = "Glob8".into();
        s.apply_filter();
        let (_, req) = s.take_request().unwrap();
        assert_eq!(req.args["where"], "EDID ~ \"Glob8\"");
        s.filter = "a\"b".into();
        s.apply_filter();
        assert!(s.take_request().is_none());
        assert!(s.list_error.is_some());
        // *All asks for nothing: query takes one type.
        s.select_node(ALL);
        assert!(s.take_request().is_none() && s.list.is_none());
        s.filter.clear();
        let env = json!({"data": {"results": [{"form": "A.esm:000800", "editor_id": "Glob800"}]}});
        s.select_node("GLOB");
        s.take_request();
        s.receive(&ObjectTicket::List, Ok(&env));
        assert_eq!(s.list.as_ref().unwrap().rows.len(), 1);
        s.receive(&ObjectTicket::List, Err("no load order".into()));
        assert!(s.list.is_none() && s.list_error.is_some());
    }

    /// The CK's double-click: the record is inspected and the edit dialog opens.
    #[test]
    fn opening_a_row_inspects_it_in_the_edit_dialog() {
        let mut s = ObjectWindowState::default();
        let actions = s.open("A.esm:000800");
        assert!(matches!(&actions[0], Action::Inspect(f) if f == "A.esm:000800"));
        assert!(matches!(
            actions[1],
            Action::OpenWindow(WindowKind::EditDialog)
        ));
        assert_eq!(s.selected.as_deref(), Some("A.esm:000800"));
    }
}
