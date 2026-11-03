//! Preferences (M7.M3, pipeline W): the workbench's settings, kept in `ui.json` only
//! (`crate::prefs`): the theme (preset and light, dark or system), the zoom, the paths it
//! remembers (the MO2 instance and profile, the KitKraft checkout the Help tab reads), and
//! the catalog's shortcuts.
//!
//! What a sitting observed of the CK's Preferences dialog: nothing yet. The row
//! "Preferences" in `docs/ck-parity.md` is `unresearched`, and sitting 1's step S1-08
//! (File > Preferences..., every tab) is still open. So the layout, the group names and the
//! labels here are KitKraft's own until that sitting, and the window says so. It will be laid
//! out as the CK's dialog where the settings correspond. File > Preferences... (an observed
//! item whose row is a `gap:`) stays disabled until its row maps to this window. The shortcuts
//! are the catalog's own, bound by the shell (M7.M5, mechanism first); the CK's keys are
//! not observed (sitting 1's S1-06: the accelerators were not exposed to UI Automation).

use eframe::egui::{self, ScrollArea};

use crate::gate::CommandSpec;
use crate::prefs::ZOOM_RANGE;
use crate::theme::{self, Mode, Preset, Status, ThemeChoice, View};

/// The zoom step of the "-" and "+" buttons.
pub const ZOOM_STEP: f32 = 0.1;

/// What the Preferences window shows besides its own state.
pub struct PrefsView<'a> {
    pub theme: ThemeChoice,
    /// Where the theme came from (`--theme`, `ui.json`, the default).
    pub theme_source: &'a str,
    /// The window's zoom factor now.
    pub zoom: f32,
    pub instance: Option<&'a str>,
    pub profile: Option<&'a str>,
    pub checkout: Option<&'a str>,
    /// Where `ui.json` is (`None`: nothing is remembered).
    pub prefs_path: Option<&'a std::path::Path>,
    /// The catalog, for its shortcuts.
    pub catalog: &'a [CommandSpec],
}

/// What a change in the window asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum PrefsAsk {
    Theme(ThemeChoice),
    /// A zoom factor, inside [`ZOOM_RANGE`].
    Zoom(f32),
    /// Use this folder as the KitKraft checkout.
    Checkout(String),
}

/// The Preferences window's state (kept in `crate::windows::Shell`).
#[derive(Debug, Default)]
pub struct PreferencesState {
    /// The checkout field.
    pub checkout: String,
    /// The field was filled from the prefs on this opening.
    pub initialized: bool,
}

/// `zoom` moved by `steps` of [`ZOOM_STEP`], rounded to a tenth and kept in [`ZOOM_RANGE`].
pub fn stepped(zoom: f32, steps: f32) -> f32 {
    let z = ((zoom + steps * ZOOM_STEP) * 10.0).round() / 10.0;
    if z.is_finite() {
        z.clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end())
    } else {
        1.0
    }
}

impl PreferencesState {
    /// The window closed: the next opening reads the prefs again.
    pub fn closed(&mut self) {
        self.initialized = false;
    }

    /// Draws the window; returns the changes asked for.
    pub fn ui(&mut self, ui: &mut egui::Ui, view: &PrefsView<'_>) -> Vec<PrefsAsk> {
        if !self.initialized {
            self.initialized = true;
            self.checkout = view.checkout.unwrap_or_default().to_string();
        }
        let mut asks = Vec::new();
        theme::status_line(
            ui,
            Status::NotRun,
            "KitKraft's own layout: the CK's Preferences dialog is not observed yet (sitting 1, step S1-08)",
        );
        match view.prefs_path {
            Some(p) => ui.weak(format!("kept in {}", p.display())),
            None => ui.weak("not kept: no %LOCALAPPDATA%, so nothing is remembered"),
        };
        theme::gap(ui);

        ui.strong("Theme");
        let mut pick = view.theme;
        ui.horizontal_wrapped(|ui| {
            for p in Preset::ALL {
                ui.selectable_value(&mut pick.preset, p, p.theme().name);
            }
            ui.separator();
            for (mode, label) in [
                (Mode::System, "system"),
                (Mode::Light, "light"),
                (Mode::Dark, "dark"),
            ] {
                ui.radio_value(&mut pick.mode, mode, label);
            }
        });
        ui.weak(format!("from {}", view.theme_source));
        if pick != view.theme {
            asks.push(PrefsAsk::Theme(pick));
        }
        theme::gap(ui);

        ui.strong("Zoom");
        ui.horizontal(|ui| {
            let z = view.zoom;
            if ui
                .add_enabled(z > *ZOOM_RANGE.start(), egui::Button::new("Zoom out"))
                .clicked()
            {
                asks.push(PrefsAsk::Zoom(stepped(z, -1.0)));
            }
            ui.monospace(format!("{:.0} %", z * 100.0));
            if ui
                .add_enabled(z < *ZOOM_RANGE.end(), egui::Button::new("Zoom in"))
                .clicked()
            {
                asks.push(PrefsAsk::Zoom(stepped(z, 1.0)));
            }
            if ui
                .add_enabled(
                    (z - 1.0).abs() > f32::EPSILON,
                    egui::Button::new("Reset zoom"),
                )
                .clicked()
            {
                asks.push(PrefsAsk::Zoom(1.0));
            }
        });
        theme::gap(ui);

        ui.strong("Paths");
        let none = "not set";
        ui.label(format!("MO2 instance: {}", view.instance.unwrap_or(none)));
        ui.label(format!(
            "Profile last loaded: {}",
            view.profile.unwrap_or(none)
        ));
        ui.weak("both set from the Start tab");
        ui.horizontal(|ui| {
            let label = ui.label("KitKraft checkout:");
            ui.add(
                egui::TextEdit::singleline(&mut self.checkout)
                    .hint_text("the folder holding AGENTS.md and docs")
                    .desired_width(theme::combo_width(ui)),
            )
            .labelled_by(label.id);
            let changed = self.checkout.trim() != view.checkout.unwrap_or_default();
            if ui
                .add_enabled(
                    changed && !self.checkout.trim().is_empty(),
                    egui::Button::new("Use This Checkout"),
                )
                .clicked()
            {
                asks.push(PrefsAsk::Checkout(self.checkout.trim().to_string()));
            }
        });
        theme::gap(ui);

        ui.strong("Shortcuts");
        ui.weak(
            "the catalog's own, shown in the menus and bound in the window (M7.M5, mechanism first); the CK's keys are not observed yet, so none of these claims to be the CK's",
        );
        ScrollArea::vertical()
            .id_salt("preferences-shortcuts")
            .max_height(theme::view_height(ui, View::Short))
            .show(ui, |ui| {
                theme::table(ui, "preferences-shortcuts-grid").show(ui, |ui| {
                    for s in view.catalog.iter().filter(|s| s.shortcut.is_some()) {
                        ui.monospace(s.shortcut.unwrap_or_default());
                        ui.label(s.label);
                        ui.end_row();
                    }
                });
            });
        asks
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn the_zoom_steps_by_a_tenth_inside_its_range() {
        assert_eq!(stepped(1.0, 1.0), 1.1);
        assert_eq!(stepped(1.0, -1.0), 0.9);
        assert_eq!(stepped(2.0, 1.0), 2.0);
        assert_eq!(stepped(0.5, -1.0), 0.5);
        assert_eq!(stepped(1.04, 0.0), 1.0);
        assert_eq!(stepped(f32::NAN, 1.0), 1.0);
    }
}
