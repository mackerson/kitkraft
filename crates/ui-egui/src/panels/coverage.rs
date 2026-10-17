//! Coverage panel (M6.G7): the support-depth matrix, `coverage --matrix` (read-only), one
//! row per record type with the six statuses kept apart (AGENTS.md "Reporting":
//! parsed, typed-readable, typed-writable, CK-differential-tested, game-runtime-tested,
//! save/reload-verified), each drawn with the status component, and the evidence each
//! cell points at (a test, a doc section, a ledger entry) with whether it resolves. A
//! filter by type narrows the rows; `types` narrows the engine's walk.

use serde_json::{json, Map, Value};

use crate::envelope::{list, num, strings, text, text_or_empty};
use crate::gate::Request;
use crate::theme::Status;

/// The six statuses in matrix order: (the row's member, the column title).
pub const COLUMNS: [(&str, &str); 6] = [
    ("parsed", "parsed"),
    ("typed_readable", "typed-readable"),
    ("typed_writable", "typed-writable"),
    ("ck_differential_tested", "CK-differential-tested"),
    ("game_runtime_tested", "game-runtime-tested"),
    ("save_reload_verified", "save/reload-verified"),
];

/// The panel's inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoverageForm {
    /// Record types for the engine's walk (`WEAP AMMO`; empty: every type).
    pub types: String,
    /// One plugin to examine (empty: the official files in the Data dir).
    pub plugin: String,
    /// The evidence file (empty: the engine finds `docs/schema/evidence.toml`).
    pub evidence: String,
}

impl CoverageForm {
    /// The record types, upper-cased, four characters each.
    pub fn types_list(&self) -> Result<Vec<String>, String> {
        let types: Vec<String> = self
            .types
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
            .map(str::to_ascii_uppercase)
            .collect();
        if let Some(bad) = types.iter().find(|t| t.chars().count() != 4) {
            return Err(format!("{bad}: a record type has four characters (WEAP)"));
        }
        Ok(types)
    }

    /// `coverage --matrix` with the form's limits.
    pub fn request(&self) -> Result<Request, String> {
        let mut a = Map::new();
        a.insert("matrix".into(), json!(true));
        let types = self.types_list()?;
        if !types.is_empty() {
            a.insert("types".into(), json!(types));
        }
        for (k, v) in [("plugin", &self.plugin), ("evidence", &self.evidence)] {
            if !v.trim().is_empty() {
                a.insert(k.into(), json!(v.trim()));
            }
        }
        Ok(Request::new("coverage", Value::Object(a)))
    }
}

/// One pointer to evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pointer {
    /// `test: <file>::<fn>`, `doc: <file>#<heading>`, `ledger: <entry>`.
    pub pointer: String,
    /// The engine found what it names.
    pub resolves: bool,
    /// Why not.
    pub error: Option<String>,
}

/// One status cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// `pass`, `fail`, `not-run`, `inconclusive`, `unknown`.
    pub status: String,
    /// The typed share, the reason, or the evidence entry's note.
    pub detail: String,
    /// What proves it.
    pub evidence: Vec<Pointer>,
}

/// One record type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixRow {
    /// `WEAP`.
    pub ty: String,
    /// The definition's name, if any.
    pub definition: String,
    /// Records examined.
    pub records: u64,
    /// The six cells, in [`COLUMNS`] order.
    pub cells: Vec<Cell>,
}

/// A `coverage --matrix` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatrixView {
    /// `official`, `plugin`, `plugins`.
    pub source: String,
    /// The files examined (names).
    pub files: Vec<String>,
    /// The evidence file, or `None` (the four evidence statuses are then unknown).
    pub evidence_file: Option<String>,
    /// Per status, `pass 3, not-run 40`.
    pub counts: Vec<(String, String)>,
    /// The rows.
    pub rows: Vec<MatrixRow>,
}

/// The status component's status for a matrix word; an unexpected word is unknown,
/// never pass.
pub fn status_of(word: &str) -> Status {
    match word {
        "pass" => Status::Pass,
        "fail" => Status::Fail,
        "not-run" => Status::NotRun,
        "inconclusive" => Status::Inconclusive,
        _ => Status::Unknown,
    }
}

impl MatrixView {
    /// Reads a `coverage --matrix` envelope (`data.matrix` true).
    pub fn from_envelope(env: &Value) -> Self {
        let d = env.get("data").unwrap_or(&Value::Null);
        let rows = list(d, "rows")
            .iter()
            .map(|r| {
                let cells = COLUMNS
                    .iter()
                    .map(|(key, _)| {
                        let c = r.get(*key).unwrap_or(&Value::Null);
                        let detail = match *key {
                            "parsed" => match num(c, "decode_failed") {
                                Some(n) if n > 0 => format!("{n} record(s) did not decompress"),
                                _ => String::new(),
                            },
                            "typed_readable" => {
                                let pct = c
                                    .get("typed_bytes_percent")
                                    .and_then(Value::as_f64)
                                    .map(|p| format!("{p:.1}% of payload bytes typed"));
                                [pct, text(c, "why")]
                                    .into_iter()
                                    .flatten()
                                    .collect::<Vec<_>>()
                                    .join("; ")
                            }
                            _ => text(c, "note").unwrap_or_default(),
                        };
                        Cell {
                            status: text(c, "status").unwrap_or_else(|| "unknown".into()),
                            detail,
                            evidence: list(c, "evidence")
                                .iter()
                                .map(|p| Pointer {
                                    pointer: text_or_empty(p, "pointer"),
                                    resolves: p.get("resolves") == Some(&Value::Bool(true)),
                                    error: text(p, "error"),
                                })
                                .collect(),
                        }
                    })
                    .collect();
                MatrixRow {
                    ty: text_or_empty(r, "type"),
                    definition: text_or_empty(r, "definition"),
                    records: num(r, "records").unwrap_or(0),
                    cells,
                }
            })
            .collect();
        let counts = strings(d, "statuses")
            .into_iter()
            .map(|s| {
                let line = d
                    .get("counts")
                    .and_then(|c| c.get(&s))
                    .and_then(Value::as_object)
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| format!("{k} {}", v.as_u64().unwrap_or(0)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                (s, line)
            })
            .collect();
        Self {
            source: text_or_empty(d, "source"),
            files: list(d, "files")
                .iter()
                .map(|f| text_or_empty(f, "file"))
                .collect(),
            evidence_file: text(d, "evidence_file"),
            counts,
            rows,
        }
    }

    /// The rows whose type or definition contains `filter` (case-insensitive); all when
    /// it is empty.
    pub fn filtered<'a>(&'a self, filter: &'a str) -> impl Iterator<Item = &'a MatrixRow> {
        let f = filter.trim().to_ascii_lowercase();
        self.rows.iter().filter(move |r| {
            f.is_empty()
                || r.ty.to_ascii_lowercase().contains(&f)
                || r.definition.to_ascii_lowercase().contains(&f)
        })
    }
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, TextEdit};

use crate::panels::common::{
    action_bar, envelope_block, panel_head, virtual_table_for, Action, Slot, TabCtx,
};
use crate::theme::{self, Field, View};

/// The Coverage tab's state.
#[derive(Default)]
pub(crate) struct CoverageTab {
    pub form: CoverageForm,
    pub filter: String,
    pub view: Option<MatrixView>,
    /// The row whose evidence is shown.
    pub selected: Option<String>,
    /// Bumped each time a result replaces the table's rows, so the table scrolls back to
    /// the top and measures its columns afresh ([`virtual_table_for`]).
    pub data: u64,
}

impl CoverageTab {
    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut TabCtx<'_>,
        checkout: Option<&std::path::Path>,
    ) {
        let busy = ctx.busy;
        panel_head(
            ui,
            "Coverage",
            "The support-depth matrix (coverage --matrix, read-only): one row per record type, six statuses never collapsed into one. parsed and typed-readable are measured on the examined plugins; the other four come from docs/schema/evidence.toml and are not-run where it names nothing. Schema presence is not support.",
        );
        if self.form.evidence.trim().is_empty() {
            if let Some(root) = checkout {
                self.form.evidence = root
                    .join("docs")
                    .join("schema")
                    .join("evidence.toml")
                    .display()
                    .to_string();
            }
        }
        let form = &mut self.form;
        let filter = &mut self.filter;
        action_bar(ui, |ui| {
            theme::form_grid(ui, "coverage-form").show(ui, |ui| {
                ui.label("record types");
                ui.add(
                    TextEdit::singleline(&mut form.types)
                        .desired_width(theme::width(ui, Field::Medium))
                        .hint_text("WEAP AMMO (empty: every type; slower)"),
                );
                ui.end_row();
                ui.label("plugin");
                ui.add(
                    TextEdit::singleline(&mut form.plugin)
                        .desired_width(theme::width(ui, Field::Long))
                        .hint_text("empty: the official files in the Data dir"),
                );
                ui.end_row();
                ui.label("evidence file");
                ui.add(
                    TextEdit::singleline(&mut form.evidence)
                        .desired_width(theme::width(ui, Field::Long)),
                );
                ui.end_row();
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("coverage --matrix"))
                    .clicked()
                {
                    match form.request() {
                        Ok(r) => ctx.actions.push(Action::Submit(Slot::Coverage, r)),
                        Err(e) => *ctx.notice = Some(e),
                    }
                }
                ui.label("filter by type");
                ui.add(
                    TextEdit::singleline(filter)
                        .desired_width(theme::width(ui, Field::Short))
                        .hint_text("WEAP, weapon"),
                );
            });
        });
        let view = &self.view;
        let filter = &self.filter;
        let selected = &mut self.selected;
        let data = self.data;
        envelope_block(ui, ctx.results.get(&Slot::Coverage.into()), |ui| {
            let Some(view) = view else { return };
            matrix_ui(ui, view, data, filter, selected);
        });
    }
}

/// The matrix: what was examined, the counts per status, the rows (a virtual table: every
/// record type, laid out only where scrolled into view), and the selected row's evidence.
fn matrix_ui(
    ui: &mut egui::Ui,
    view: &MatrixView,
    data: u64,
    filter: &str,
    selected: &mut Option<String>,
) {
    ui.label(format!(
        "{} file(s) examined ({}): {}",
        view.files.len(),
        view.source,
        view.files.join(", ")
    ));
    match &view.evidence_file {
        Some(f) => {
            ui.weak(format!("evidence: {f}"));
        }
        None => {
            theme::status_line(
                ui,
                Status::Unknown,
                "no evidence file: the four evidence statuses are unknown",
            );
        }
    }
    theme::table(ui, "coverage-counts").show(ui, |ui| {
        for (s, line) in &view.counts {
            ui.label(s);
            ui.label(line);
            ui.end_row();
        }
    });
    theme::gap(ui);
    let rows: Vec<&MatrixRow> = view.filtered(filter).collect();
    ui.weak(format!(
        "{} of {} type(s); click a type for its evidence",
        rows.len(),
        view.rows.len()
    ));
    let mut header = vec!["type", "records"];
    header.extend(COLUMNS.iter().map(|(_, title)| *title));
    let mut pick = None;
    virtual_table_for(
        ui,
        "coverage-grid",
        data,
        &header,
        rows.len(),
        View::Tall,
        |row, i| {
            let Some(r) = rows.get(i) else { return };
            let sel = selected.as_deref() == Some(r.ty.as_str());
            row.cell(|ui| {
                if ui
                    .selectable_label(sel, egui::RichText::new(&r.ty).monospace())
                    .on_hover_text(&r.definition)
                    .clicked()
                {
                    pick = Some(r.ty.clone());
                }
            });
            row.num(r.records.to_string());
            for c in &r.cells {
                row.cell(|ui| {
                    let mut hover = c.detail.clone();
                    for p in &c.evidence {
                        hover.push_str(&format!(
                            "\n{} {}",
                            if p.resolves {
                                "resolves:"
                            } else {
                                "DOES NOT RESOLVE:"
                            },
                            p.pointer
                        ));
                    }
                    let resp = theme::status(ui, status_of(&c.status), &c.status);
                    if !hover.trim().is_empty() {
                        resp.on_hover_text(hover.trim());
                    }
                });
            }
        },
    );
    if let Some(p) = pick {
        *selected = if selected.as_deref() == Some(p.as_str()) {
            None
        } else {
            Some(p)
        };
    }
    if let Some(row) = selected
        .as_deref()
        .and_then(|t| view.rows.iter().find(|r| r.ty == t))
    {
        theme::gap(ui);
        ui.label(theme::subheading(
            ui,
            format!("{} ({}): evidence", row.ty, row.definition),
        ));
        // Six fixed rows with several pointers per cell: the grid table style.
        theme::table(ui, "coverage-evidence").show(ui, |ui| {
            for h in ["status", "result", "detail", "evidence"] {
                ui.strong(h);
            }
            ui.end_row();
            for ((_, title), c) in COLUMNS.iter().zip(&row.cells) {
                ui.label(*title);
                theme::status(ui, status_of(&c.status), &c.status);
                ui.label(&c.detail);
                ui.vertical(|ui| {
                    if c.evidence.is_empty() {
                        ui.weak("none");
                    }
                    for p in &c.evidence {
                        if p.resolves {
                            theme::status(ui, Status::Pass, &p.pointer);
                        } else {
                            theme::status(
                                ui,
                                Status::Inconclusive,
                                &format!(
                                    "{} (does not resolve{})",
                                    p.pointer,
                                    p.error
                                        .as_deref()
                                        .map_or_else(String::new, |e| format!(": {e}"))
                                ),
                            );
                        }
                    }
                });
                ui.end_row();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_a_matrix_with_the_forms_limits() {
        let f = CoverageForm {
            types: "weap, ammo".into(),
            ..CoverageForm::default()
        };
        let r = f.request().unwrap();
        assert_eq!(r.args, json!({"matrix": true, "types": ["WEAP", "AMMO"]}));
        crate::gate::check(&r).unwrap();
        assert!(CoverageForm {
            types: "WEAPON".into(),
            ..CoverageForm::default()
        }
        .request()
        .is_err());
    }

    /// The view-model on the engine's own matrix shape: a real `coverage --matrix` over a
    /// synthetic plugin, with an evidence file whose pointers resolve and do not.
    #[test]
    fn the_view_reads_the_engines_matrix() {
        let dir = std::env::temp_dir().join(format!("kitkraft-ui-matrix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("docs").join("schema")).unwrap();
        std::fs::write(dir.join("docs").join("x.md"), "# X\n\n## Here\n").unwrap();
        let evidence = dir.join("docs").join("schema").join("evidence.toml");
        std::fs::write(
            &evidence,
            "[[entry]]\ntype = \"GLOB\"\nstatus = \"typed-writable\"\nresult = \"pass\"\nevidence = [\"doc: docs/x.md#Here\"]\n\n[[entry]]\ntype = \"GLOB\"\nstatus = \"game-runtime-tested\"\nresult = \"pass\"\nevidence = [\"ledger: game-29990101-01\"]\n",
        )
        .unwrap();
        let plugin = dir.join("Base.esm");
        std::fs::write(
            &plugin,
            crate::panels::cockpit::fake_mo2::plugin_bytes(&[], 1, &[0x800], 1.0),
        )
        .unwrap();
        let form = CoverageForm {
            types: "GLOB".into(),
            plugin: plugin.display().to_string(),
            evidence: evidence.display().to_string(),
        };
        let env = crate::gate::ReadOnlyEngine::new()
            .run(&form.request().unwrap())
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let v = MatrixView::from_envelope(&env);
        assert_eq!(v.source, "plugin");
        assert_eq!(v.files, ["Base.esm"]);
        assert!(v.evidence_file.is_some());
        assert_eq!(v.counts.len(), 6, "{:?}", v.counts);
        let glob = v.rows.iter().find(|r| r.ty == "GLOB").unwrap();
        assert_eq!(glob.records, 1);
        let words: Vec<&str> = glob.cells.iter().map(|c| c.status.as_str()).collect();
        assert_eq!(words.len(), 6);
        assert_eq!(words[0], "pass", "{glob:?}");
        // typed-writable: its doc pointer resolves; game-runtime-tested: the ledger
        // entry does not exist, so it is inconclusive, never pass.
        assert_eq!(words[2], "pass", "{glob:?}");
        assert!(glob.cells[2].evidence.iter().all(|p| p.resolves));
        assert_eq!(words[4], "inconclusive", "{glob:?}");
        assert!(glob.cells[4].evidence.iter().any(|p| !p.resolves));
        assert_eq!(words[3], "not-run");
        assert_eq!(words[5], "not-run");
        assert_eq!(v.filtered("glo").count(), 1);
        assert_eq!(v.filtered("WEAP").count(), 0);
        assert_eq!(v.filtered("").count(), v.rows.len());
        for w in words {
            assert_ne!(status_of(w), Status::Unknown, "{w}");
        }
        assert_eq!(status_of("whatever"), Status::Unknown);
    }
}
