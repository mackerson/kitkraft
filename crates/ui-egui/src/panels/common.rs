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
use crate::envelope::Prepared;
use crate::gate::{Request, RunError};
use crate::panels::ledger::LedgerForm;
use crate::panels::start::StartProfile;
use crate::panels::validate::FindingRow;
use crate::theme::{self, Status, ThemeChoice, View};

/// Rows of evidence shown per result; the raw JSON has the rest.
const EVIDENCE_ROWS: usize = 300;
/// Characters of a limitation shown on its line (the full text is on hover).
const LIMITATION_CHARS: usize = 200;

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

/// A schema confidence: the unconfirmed levels as a warning status, the rest as text.
pub(crate) fn conf_label(ui: &mut egui::Ui, confidence: &str) {
    if confidence == "reference" {
        theme::status(ui, Status::Warn, "reference (unconfirmed)").on_hover_text(
            "documented in the pinned xEdit file; not yet confirmed by the corpus or the CK",
        );
    } else if confidence == "container-reference" {
        theme::status(ui, Status::Warn, "container-reference (unconfirmed)")
            .on_hover_text("a container fact from UESP's Skyrim pages that no official file shows");
    } else {
        ui.label(confidence);
    }
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
/// offset and message (the Validate and Diagnostics tabs).
pub(crate) fn findings_grid<'a>(
    ui: &mut egui::Ui,
    id: &str,
    findings: impl Iterator<Item = &'a FindingRow>,
) {
    theme::table(ui, ("findings", id)).show(ui, |ui| {
        for h in [
            "severity",
            "rule",
            "record",
            "subrecord",
            "offset",
            "message",
        ] {
            ui.strong(h);
        }
        ui.end_row();
        for f in findings.take(1000) {
            if f.severity == "error" {
                theme::status(ui, Status::Fail, &f.severity);
            } else {
                theme::status(ui, Status::Warn, &f.severity);
            }
            ui.monospace(&f.rule);
            ui.monospace(&f.record);
            ui.monospace(&f.subrecord);
            ui.monospace(
                f.offset
                    .map_or_else(|| "?".to_string(), |o| format!("{o} (0x{o:X})")),
            );
            ui.label(&f.message);
            ui.end_row();
        }
    });
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
                ui.colored_label(
                    theme::warn(ui),
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
        if let Some(s) = &view.summary {
            // A multi-line summary is a text table (`schema`): keep its columns.
            if s.contains('\n') {
                ui.monospace(s);
            } else {
                ui.label(s);
            }
        }
    });
    ui.label(format!("coverage {}", view.coverage_line()));
    if view.limitations.is_empty() {
        ui.weak("limitations: none claimed");
    } else {
        ui.colored_label(
            theme::warn(ui),
            format!("limitations ({})", view.limitations.len()),
        );
        // Every limitation stays on screen; a long one shows its first line's worth and
        // the rest on hover.
        for l in &view.limitations {
            if l.chars().count() > LIMITATION_CHARS {
                let head: String = l.chars().take(LIMITATION_CHARS).collect();
                ui.colored_label(theme::warn(ui), format!("  - {head}... (hover for all)"))
                    .on_hover_text(l);
            } else {
                ui.colored_label(theme::warn(ui), format!("  - {l}"));
            }
        }
    }
    content(ui);
    CollapsingHeader::new(format!("evidence ({} ranges)", view.evidence.len()))
        .id_salt(("evidence", id))
        .show(ui, |ui| {
            ScrollArea::vertical()
                .id_salt(("evidence-rows", id))
                .max_height(theme::view_height(ui, View::Medium))
                .show(ui, |ui| {
                    theme::table(ui, ("evidence-grid", id)).show(ui, |ui| {
                        for h in ["file", "offset", "length", "record", "subrecord"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for e in view.evidence.iter().take(EVIDENCE_ROWS) {
                            ui.label(&e.file);
                            ui.monospace(format!("{} (0x{:X})", e.offset, e.offset));
                            ui.monospace(e.length.to_string());
                            ui.monospace(&e.record);
                            ui.monospace(&e.subrecord);
                            ui.end_row();
                        }
                    });
                    if view.evidence.len() > EVIDENCE_ROWS {
                        ui.weak(format!(
                            "first {EVIDENCE_ROWS} shown; the raw JSON (MCP or `kit --json`) has all"
                        ));
                    }
                });
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
