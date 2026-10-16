//! What every tab's drawing shares: where a reply goes ([`Slot`]), one finished request
//! as the frame draws it ([`Shown`]), what a click asks for ([`Action`]), the per-frame
//! context each tab's `ui` gets ([`TabCtx`]), and the pieces several tabs draw (links,
//! the record actions, the plugin picker, the confidence label, and the envelope block
//! around every result). Colors, sizes and spacing come from `crate::theme` only.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{self, CollapsingHeader, ComboBox, RichText, ScrollArea};

use crate::cmdline::ResultLine;
use crate::envelope::{BuildInfo, Prepared};
use crate::gate::{Request, RunError};
use crate::panels::ledger::LedgerForm;
use crate::panels::start::StartProfile;
use crate::panels::validate::FindingRow;
use crate::theme::{self, Status, ThemeChoice, View};

/// Characters of a limitation shown on its line (the full text is on hover).
const LIMITATION_CHARS: usize = 200;
/// A multi-line summary's text table is shown open up to this many lines, folded beyond.
const SUMMARY_OPEN_LINES: usize = 12;

/// Where a reply goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Profiles,
    Load,
    Types,
    Query,
    Inspect,
    InspectSchema,
    Links,
    Planet,
    Diff,
    DiagInspect,
    DiagValidate,
    Validate,
    Roundtrip,
    Schema,
    Ledger,
    // Lane G (M6.G2, M6.G3, M6.G7, M6.G8): the Cockpit and Coverage tabs.
    CockpitStatus,
    CockpitValidate,
    CockpitRoundtrip,
    CockpitMasters,
    CockpitDeploy,
    CockpitRollback,
    Coverage,
    // Lane R (M7.R2): the Render Window's cell, a page at a time.
    RenderCell,
}

/// `Slot` as a map key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SlotKey(u8);

impl From<Slot> for SlotKey {
    fn from(s: Slot) -> Self {
        SlotKey(s as u8)
    }
}

/// One finished request, with everything the frame draws built once on arrival.
pub(crate) struct Shown {
    pub request: Request,
    /// Why there is no envelope.
    pub error: Option<RunError>,
    pub elapsed: Duration,
    /// The envelope view and raw JSON lines.
    pub prepared: Prepared,
    /// The `kit` line, from the request as run and the session it ran against.
    pub line: ResultLine,
    /// The MCP `execute` call.
    pub mcp: String,
}

/// What a click asks for, applied after the frame is drawn.
pub(crate) enum Action {
    Submit(Slot, Request),
    Inspect(String),
    Diagnose(String),
    /// `refs` on a form: outgoing links, or its referrers ("Use info") when `true`.
    Links(String, bool),
    LinksPage(Option<String>),
    Planet(String),
    PlanetPage(Option<String>),
    PickType(String),
    QueryPage(Option<String>),
    UseProfile(PathBuf),
    /// A start-screen button: load this profile and remember the choice.
    LoadProfile(StartProfile),
    /// The Ledger panel's confirm click: append this row (the workbench's one write).
    LedgerAppend(LedgerForm),
    /// The Start tab's theme picker: apply this preset and mode and keep it in `ui.json`.
    SetTheme(ThemeChoice),
    /// The Help tab's "Use this checkout": remember the folder (when it is a checkout)
    /// and reread the docs.
    UseCheckout(String),
    /// The Help tab's "Reload".
    ReloadHelp,
    /// Lane R (M7.R2): a reference picked in the Render Window becomes the Records
    /// inspector's record (inspected there) without leaving the tab.
    Select(String),
}

/// What a tab's `ui` gets each frame besides its own state: whether the engine is busy,
/// the shown results, the loaded plugins, the notice line, the actions to apply after
/// the frame, and what the Start tab shows of the theme and the prefs.
pub(crate) struct TabCtx<'a> {
    pub busy: bool,
    /// The screenshot tour is running (`crate::shots`).
    pub tour: bool,
    pub results: &'a BTreeMap<SlotKey, Shown>,
    /// The loaded plugins as (name, path).
    pub plugins: &'a [(String, String)],
    pub notice: &'a mut Option<String>,
    pub actions: Vec<Action>,
    pub theme: ThemeChoice,
    pub theme_source: &'a str,
    /// The profile last loaded from the start screen (`ui.json`).
    pub last_profile: Option<&'a str>,
    pub prefs_path: Option<&'a Path>,
}

/// The window's title (the OS title bar).
pub(crate) const WINDOW_TITLE: &str = "KitKraft Workbench (read-only)";

/// The build the results name (M6.P6b's `build` member; the window and the engine are
/// one process, so any envelope names the window's build).
pub(crate) fn build_of(results: &BTreeMap<SlotKey, Shown>) -> Option<&BuildInfo> {
    results
        .values()
        .find_map(|s| s.prepared.view.as_ref()?.build.as_ref())
}

/// The title bar of a debug build (M6.P6a): the title with a "debug build" note; `None`
/// for a release build, whose title stays [`WINDOW_TITLE`].
pub(crate) fn debug_title(build: &BuildInfo) -> Option<String> {
    build
        .is_debug()
        .then(|| format!("{WINDOW_TITLE} - debug build (slow; `cargo xtask ui` builds release)"))
}

/// One row of a [`virtual_table`]: its cells, added left to right with [`TableRow::cell`].
pub(crate) struct TableRow<'a> {
    ui: &'a mut egui::Ui,
    widths: &'a mut Vec<f32>,
    grew: &'a mut bool,
    x: f32,
    top: f32,
    height: f32,
    gap: f32,
    col: usize,
}

impl TableRow<'_> {
    /// Adds one cell. The cell is as wide as the widest content its column has shown so
    /// far (columns only grow, so they do not jump while scrolling); content is never
    /// wrapped, and what does not fit is clipped until the column grows on the next frame.
    pub(crate) fn cell<R>(&mut self, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        self.cell_aligned(false, add)
    }

    /// Adds a number cell (M6.V5 table style): monospace, right-aligned in its column.
    pub(crate) fn num(&mut self, text: impl Into<String>) -> egui::Response {
        let text = text.into();
        self.cell_aligned(true, |ui| ui.monospace(text))
    }

    fn cell_aligned<R>(&mut self, right: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let col = self.col;
        self.col = self.col.saturating_add(1);
        if self.widths.len() <= col {
            self.widths.resize(col.saturating_add(1), 0.0);
        }
        let width = self.widths.get(col).copied().unwrap_or(0.0);
        let cell =
            egui::Rect::from_min_size(egui::pos2(self.x, self.top), egui::vec2(width, self.height));
        // Unbounded to the right, so nothing wraps; clipped to the cell. A right-aligned
        // cell is laid out from the column's right edge once the column has a width.
        let (room, layout) = if right && width > 0.0 {
            (cell, egui::Layout::right_to_left(egui::Align::Center))
        } else {
            (
                egui::Rect::from_min_max(cell.min, egui::pos2(f32::INFINITY, cell.max.y)),
                egui::Layout::left_to_right(egui::Align::Center),
            )
        };
        let mut child = self
            .ui
            .new_child(egui::UiBuilder::new().max_rect(room).layout(layout));
        child.set_clip_rect(cell.intersect(self.ui.clip_rect()));
        let out = add(&mut child);
        let used = child.min_rect().width();
        if used > width {
            if let Some(w) = self.widths.get_mut(col) {
                *w = used;
            }
            *self.grew = true;
        }
        self.x += width + self.gap;
        out
    }
}

/// The row height and column gap of every [`virtual_table`]: the theme's density
/// (`row_height`, `cell_x`), never less than a control's height.
fn table_metrics(ui: &egui::Ui) -> (f32, f32) {
    let theme = theme::active(ui.ctx());
    let control = ui.spacing().interact_size.y;
    if theme.restyle {
        (theme.density.row_height.max(control), theme.density.cell_x)
    } else {
        (control, ui.spacing().item_spacing.x)
    }
}

/// Draws one row of a [`virtual_table`] at the next place in `ui`.
fn table_row(
    ui: &mut egui::Ui,
    widths: &mut Vec<f32>,
    grew: &mut bool,
    zebra: bool,
    add: impl FnOnce(&mut TableRow<'_>),
) {
    let (height, gap) = table_metrics(ui);
    let total: f32 = widths.iter().map(|w| w + gap).sum();
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(total.max(ui.available_width()), height),
        egui::Sense::hover(),
    );
    if zebra {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().faint_bg_color);
    }
    let mut row = TableRow {
        ui,
        widths,
        grew,
        x: rect.min.x,
        top: rect.min.y,
        height,
        gap,
        col: 0,
    };
    add(&mut row);
}

/// A table that lays out only the rows scrolled into view (M6.P4), so a page of 1000
/// query rows or 30,000 evidence ranges costs a screenful per frame. The header stays at
/// the top; every row has the theme's row height; zebra rows when the density asks for
/// them; the scroll position is kept per `id`, and selection is the caller's state, so
/// both survive a repaint. `row(r, i)` adds row `i`'s cells, one per header column.
/// Returns the rectangle of the rows' scroll area.
pub(crate) fn virtual_table(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    header: &[&str],
    rows: usize,
    view: View,
    mut row: impl FnMut(&mut TableRow<'_>, usize),
) -> egui::Rect {
    let id = ui.make_persistent_id(("virtual-table", id));
    let mut widths: Vec<f32> = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let mut grew = false;
    let zebra = theme::active(ui.ctx()).density.zebra;
    let (height, _) = table_metrics(ui);
    let max_height = theme::view_height(ui, view);
    let rect = ScrollArea::horizontal()
        .id_salt((id, "h"))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            table_row(ui, &mut widths, &mut grew, false, |r| {
                for h in header {
                    r.cell(|ui| ui.strong(*h));
                }
            });
            ScrollArea::vertical()
                .id_salt((id, "v"))
                .max_height(max_height)
                .auto_shrink([false, true])
                .show_rows(ui, height, rows, |ui, range| {
                    for i in range {
                        let stripe = zebra && i & 1 == 1;
                        table_row(ui, &mut widths, &mut grew, stripe, |r| row(r, i));
                    }
                })
                .inner_rect
        })
        .inner;
    ui.data_mut(|d| d.insert_temp(id, widths));
    if grew {
        ui.ctx().request_repaint();
    }
    rect
}

// ---- the panel frame (M6.V5) -------------------------------------------------------------
//
// Every tab is drawn in the same four parts, top to bottom:
//
// 1. [`panel_head`]: the tab's title and one paragraph on what it shows;
// 2. [`action_bar`]: its form and buttons, in one outlined strip on the raised surface;
// 3. per result, [`envelope_block`]'s header (the `kit` line and the MCP call, both
//    copyable; PASSED or FAILED; coverage) and the envelope's limitations strip
//    ([`limitations_strip`], never hidden);
// 4. the body: the panel's own tables (a [`virtual_table`] for anything that can grow,
//    `theme::table` for a few fixed rows), then the evidence ranges and the raw JSON.
//
// A test (`every_tab_is_drawn_in_the_panel_frame`) holds that each tab file calls the
// first two and draws no heading of its own.

/// The panel frame's head: the tab's title (the heading style) and what it shows.
pub(crate) fn panel_head(ui: &mut egui::Ui, title: &str, about: &str) {
    ui.heading(title);
    if !about.is_empty() {
        ui.label(about);
    }
    theme::gap(ui);
}

/// The panel frame's action bar: `add` draws the tab's form and buttons inside one
/// outlined strip as wide as the panel.
pub(crate) fn action_bar<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let out = theme::bar_frame(ui)
        .show(ui, |ui| {
            theme::fill_visible_width(ui);
            add(ui)
        })
        .inner;
    theme::gap(ui);
    out
}

/// The envelope's limitations, in a strip of their own: a warning status with their
/// count, then each limitation as body text (a long one shows its first
/// [`LIMITATION_CHARS`] characters and the rest on hover). None claimed is said so.
pub(crate) fn limitations_strip(ui: &mut egui::Ui, limitations: &[String]) {
    if limitations.is_empty() {
        ui.weak("limitations: none claimed");
        return;
    }
    theme::strip_frame(ui).show(ui, |ui| {
        theme::fill_visible_width(ui);
        theme::status(
            ui,
            Status::Warn,
            &format!("limitations ({})", limitations.len()),
        );
        for l in limitations {
            if l.chars().count() > LIMITATION_CHARS {
                let head: String = l.chars().take(LIMITATION_CHARS).collect();
                ui.label(format!("- {head}... (hover for all)"))
                    .on_hover_text(l);
            } else {
                ui.label(format!("- {l}"));
            }
        }
    });
}

/// A FormID's resolution as a status (M6.V4): `resolved` passes; `null` (the null
/// FormID, a value and not a failure) is plain text; anything else (`unresolved`,
/// `unresolvable`, a missing master) fails, with the engine's word.
pub(crate) fn formid_status(ui: &mut egui::Ui, word: &str) -> egui::Response {
    match word {
        "resolved" => theme::status(ui, Status::Pass, word),
        "null" | "NULL" | "" => ui.label(word),
        _ => theme::status(ui, Status::Fail, word),
    }
}

/// A link that submits a request: disabled while the engine is busy, so clicks do not
/// pile up a queue behind a long command (the status line can drop one).
pub(crate) fn link(
    ui: &mut egui::Ui,
    enabled: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add_enabled(enabled, egui::Link::new(text))
}

/// The two `refs` actions every record row offers: "links" (its outgoing typed links) and
/// "use info" (the records that name it).
pub(crate) fn record_actions(ui: &mut egui::Ui, busy: bool, form: &str, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        if link(ui, !busy, "links")
            .on_hover_text("Outgoing links: the typed FormIDs this record stores (refs)")
            .clicked()
        {
            actions.push(Action::Links(form.to_string(), false));
        }
        if link(ui, !busy, "use info")
            .on_hover_text("Use info: the records that name this form (refs --incoming)")
            .clicked()
        {
            actions.push(Action::Links(form.to_string(), true));
        }
    });
}

/// A form cell: a link that inspects `form` (labelled `form label`), or the label alone
/// when the form does not resolve.
pub(crate) fn form_cell(
    ui: &mut egui::Ui,
    busy: bool,
    form: Option<&str>,
    label: &str,
    actions: &mut Vec<Action>,
) {
    match form {
        Some(f) => {
            if link(ui, !busy, format!("{f} {label}"))
                .on_hover_text("inspect")
                .clicked()
            {
                actions.push(Action::Inspect(f.to_string()));
            }
        }
        None => {
            ui.label(label);
        }
    }
}

/// The status a schema confidence level is drawn with (M6.V4): the confirmed levels
/// (`corpus`, `ck-diff`, `verified`) pass, the unconfirmed ones (`reference`,
/// `container-reference`) warn, and a level this window does not know is unknown.
pub(crate) fn conf_status(confidence: &str) -> Status {
    match confidence {
        "corpus" | "ck-diff" | "verified" => Status::Pass,
        "reference" | "container-reference" => Status::Warn,
        _ => Status::Unknown,
    }
}

/// A schema confidence as the status component: its level as the word (the unconfirmed
/// levels say so), its meaning on hover.
pub(crate) fn conf_label(ui: &mut egui::Ui, confidence: &str) {
    if confidence.is_empty() {
        ui.label("");
        return;
    }
    let (word, hover) = match confidence {
        "reference" => (
            "reference (unconfirmed)".to_string(),
            "documented in the pinned xEdit file; not yet confirmed by the corpus or the CK",
        ),
        "container-reference" => (
            "container-reference (unconfirmed)".to_string(),
            "a container fact from UESP's Skyrim pages that no official file shows",
        ),
        "corpus" => (
            confidence.to_string(),
            "confirmed by statistics over the official files",
        ),
        "ck-diff" => (
            confidence.to_string(),
            "confirmed by a Creation Kit differential",
        ),
        "verified" => (confidence.to_string(), "verified end to end"),
        _ => (
            confidence.to_string(),
            "a confidence level this window does not know",
        ),
    };
    theme::status(ui, conf_status(confidence), &word).on_hover_text(hover);
}

/// A picker over the loaded plugins: sets `value` to the plugin's path, or its name when
/// `by_name` (arguments that filter by plugin name, such as `conflicts --plugin`).
pub(crate) fn plugin_combo(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    plugins: &[(String, String)],
    by_name: bool,
) {
    if plugins.is_empty() {
        return;
    }
    ComboBox::from_id_salt(id)
        .selected_text("loaded...")
        .width(theme::combo_width(ui))
        .show_ui(ui, |ui| {
            for (name, path) in plugins {
                let pick = if by_name { name } else { path };
                ui.selectable_value(value, pick.clone(), name);
            }
        });
}

/// Validate findings as a table: severity as a status, then rule, record, subrecord,
/// offset and message (the Validate and Diagnostics tabs); every finding, laid out only
/// where scrolled into view.
pub(crate) fn findings_grid<'a>(
    ui: &mut egui::Ui,
    id: &str,
    findings: impl IntoIterator<Item = &'a FindingRow>,
) {
    let findings: Vec<&FindingRow> = findings.into_iter().collect();
    let header = [
        "severity",
        "rule",
        "record",
        "subrecord",
        "offset",
        "message",
    ];
    virtual_table(
        ui,
        ("findings", id),
        &header,
        findings.len(),
        View::Medium,
        |r, i| {
            let Some(f) = findings.get(i) else { return };
            r.cell(|ui| {
                if f.severity == "error" {
                    theme::status(ui, Status::Fail, &f.severity);
                } else {
                    theme::status(ui, Status::Warn, &f.severity);
                }
            });
            r.cell(|ui| ui.monospace(&f.rule));
            r.cell(|ui| ui.monospace(&f.record));
            r.cell(|ui| ui.monospace(&f.subrecord));
            r.num(
                f.offset
                    .map_or_else(|| "?".to_string(), |o| format!("{o} (0x{o:X})")),
            );
            r.cell(|ui| ui.label(&f.message));
        },
    );
}

/// The common frame around one result: the command line and MCP call that produce it
/// (copyable), errors, summary, pass/fail, coverage, limitations (always shown), the
/// panel's own `content`, the evidence table and the raw `data`. Everything it shows was
/// built once when the reply arrived ([`Shown`]); per frame it only lays out what is on
/// screen.
pub(crate) fn envelope_block(
    ui: &mut egui::Ui,
    shown: Option<&Shown>,
    content: impl FnOnce(&mut egui::Ui),
) {
    let Some(shown) = shown else {
        return;
    };
    ui.separator();
    let id = shown.request.id.as_str();
    match &shown.line.line {
        Some(line) => {
            ui.horizontal(|ui| {
                ui.weak(format!("{} ms", shown.elapsed.as_millis()));
                if ui.small_button("copy").clicked() {
                    ui.ctx().copy_text(line.text.clone());
                }
                ui.monospace(&line.text);
            });
            if !line.exact {
                theme::status_line(
                    ui,
                    Status::Warn,
                    "a value holds both quote characters: the line above is not exact; use the MCP form",
                );
            }
            if shown.line.session_added {
                ui.weak("load-order arguments added from the session's load: each `kit` process starts with an empty session (this window keeps one, like `kit mcp`)");
            }
        }
        None => {
            ui.horizontal(|ui| {
                ui.weak(format!("{} ms", shown.elapsed.as_millis()));
                ui.weak("no `kit` line: no load had succeeded in this session, so a fresh `kit` process would not see what this ran against; the MCP call below is exact");
            });
        }
    }
    ui.horizontal(|ui| {
        if ui.small_button("copy").clicked() {
            ui.ctx().copy_text(shown.mcp.clone());
        }
        ui.weak("MCP execute:");
        ui.monospace(&shown.mcp);
    });
    if let Some(err) = &shown.error {
        theme::status_line(ui, Status::Fail, &err.to_string());
        raw_lines(ui, id, "error JSON", &shown.prepared, true);
        return;
    }
    let Some(view) = &shown.prepared.view else {
        return;
    };
    // A multi-line summary is a text table (`schema`, `planet`, `coverage --matrix`): its
    // first line heads the result, the rest (columns kept) follows the limitations strip,
    // folded when long, so the strip and the panel's own tables stay near the top.
    let (first, rest) = match view.summary.as_deref() {
        Some(s) => match s.split_once('\n') {
            Some((head, tail)) => (Some(head), Some(tail)),
            None => (Some(s), None),
        },
        None => (None, None),
    };
    ui.horizontal_wrapped(|ui| {
        match view.passed {
            Some(true) => {
                theme::status(ui, Status::Pass, "PASSED");
            }
            Some(false) => {
                theme::status(ui, Status::Fail, "FAILED");
            }
            None => {}
        }
        if let Some(s) = first {
            ui.label(s);
        }
    });
    let coverage = format!("coverage {}", view.coverage_line());
    if coverage.chars().count() > LIMITATION_CHARS {
        let head: String = coverage.chars().take(LIMITATION_CHARS).collect();
        ui.label(format!("{head}... (hover for all)"))
            .on_hover_text(coverage);
    } else {
        ui.label(coverage);
    }
    limitations_strip(ui, &view.limitations);
    if let Some(rest) = rest {
        let lines = rest.lines().count();
        CollapsingHeader::new(format!("summary ({lines} more lines)"))
            .id_salt(("summary", id))
            .default_open(lines <= SUMMARY_OPEN_LINES)
            .show(ui, |ui| {
                ui.monospace(rest);
            });
    }
    content(ui);
    CollapsingHeader::new(format!("evidence ({} ranges)", view.evidence.len()))
        .id_salt(("evidence", id))
        .show(ui, |ui| {
            virtual_table(
                ui,
                ("evidence-grid", id),
                &["file", "offset", "length", "record", "subrecord"],
                view.evidence.len(),
                View::Medium,
                |r, i| {
                    let Some(e) = view.evidence.get(i) else {
                        return;
                    };
                    r.cell(|ui| ui.label(&e.file));
                    r.num(format!("{} (0x{:X})", e.offset, e.offset));
                    r.num(e.length.to_string());
                    r.cell(|ui| ui.monospace(&e.record));
                    r.cell(|ui| ui.monospace(&e.subrecord));
                },
            );
        });
    raw_lines(ui, id, "raw data (JSON)", &shown.prepared, false);
}

/// The prepared raw JSON in a scroll area that lays out only the visible lines.
fn raw_lines(ui: &mut egui::Ui, id: &str, title: &str, prepared: &Prepared, open: bool) {
    CollapsingHeader::new(format!("{title} ({} lines)", prepared.raw.len()))
        .id_salt(("raw", id, title))
        .default_open(open)
        .show(ui, |ui| {
            if prepared.raw_truncated {
                ui.weak(format!(
                    "cut to {} lines of {} characters; the MCP call or `kit --json` has all",
                    crate::envelope::RAW_LINES,
                    crate::envelope::RAW_LINE_CHARS
                ));
            }
            let row = ui.text_style_height(&egui::TextStyle::Monospace);
            ScrollArea::both()
                .id_salt(("raw-rows", id, title))
                .max_height(theme::view_height(ui, View::Tall))
                .auto_shrink([false, true])
                .show_rows(ui, row, prepared.raw.len(), |ui, range| {
                    for line in prepared.raw.get(range).unwrap_or_default() {
                        ui.add(
                            egui::Label::new(RichText::new(line).monospace())
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    }
                });
        });
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::theme::{Mode, Preset};

    /// M6.V5: every tab (one file per tab under `panels/`, lane G's Cockpit and Coverage
    /// and the Render Window included) is drawn in the panel frame: its drawing calls
    /// `panel_head` and `action_bar` and draws no heading of its own; the frame's parts
    /// are this file's.
    #[test]
    fn every_tab_is_drawn_in_the_panel_frame() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("panels");
        let mut tabs = Vec::new();
        let mut problems = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if !name.ends_with(".rs") || name == "common.rs" || name == "mod.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let drawing = text
                .split("// ---- drawing")
                .nth(1)
                .unwrap_or_else(|| panic!("{name}: no `// ---- drawing` part"));
            let drawing = drawing.split("#[cfg(test)]").next().unwrap();
            for call in ["panel_head(", "action_bar("] {
                if !drawing.contains(call) {
                    problems.push(format!("{name}: no {call}..)"));
                }
            }
            if drawing.matches("panel_head(").count() > 1 {
                problems.push(format!("{name}: more than one panel_head"));
            }
            if drawing.contains("ui.heading(") {
                problems.push(format!("{name}: a heading of its own"));
            }
            tabs.push(name);
        }
        assert!(tabs.len() >= 14, "{tabs:?}");
        for t in ["cockpit.rs", "coverage.rs", "render.rs", "start.rs"] {
            assert!(tabs.iter().any(|n| n == t), "{t} missing from {tabs:?}");
        }
        assert!(problems.is_empty(), "{problems:#?}");
    }

    /// The About line's and the title bar's source (M6.P6a, M6.P6b): a debug build's
    /// title says so; a release build keeps the plain title.
    #[test]
    fn a_debug_build_says_so_in_the_title() {
        let b = |profile: &str| BuildInfo {
            version: "0.0.0".into(),
            commit: "abc1234".into(),
            date: "2026-10-16".into(),
            profile: profile.into(),
        };
        let t = debug_title(&b("debug")).unwrap();
        assert!(
            t.starts_with(WINDOW_TITLE) && t.contains("debug build"),
            "{t}"
        );
        assert_eq!(debug_title(&b("release")), None);
    }

    /// M6.P4: a 100,000-row table lays out about a screenful of rows per frame, from the
    /// top, and the header's columns only grow.
    #[test]
    fn a_virtual_table_lays_out_only_the_visible_rows() {
        let ctx = egui::Context::default();
        theme::apply(&ctx, &Preset::Workbench.theme(), Mode::Light);
        let raw = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::from(theme::WINDOW),
            )),
            ..Default::default()
        };
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.clear();
            let mut out = ctx.run_ui(raw(), |ui| {
                virtual_table(ui, "t", &["a", "b"], 100_000, View::Medium, |r, i| {
                    seen.push(i);
                    r.cell(|ui| ui.label(format!("row {i}")));
                    r.cell(|ui| ui.monospace("x".repeat(i % 7)));
                });
            });
            out.textures_delta.clear();
        }
        assert!(!seen.is_empty(), "no row drawn");
        assert!(seen.len() < 200, "{} rows laid out", seen.len());
        assert_eq!(seen.first(), Some(&0));
    }
}
