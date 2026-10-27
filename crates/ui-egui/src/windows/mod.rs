//! The CK-style window model (M7.M0, `docs/workbench.md` "CK-style windows"): floating
//! `egui::Window`s inside the main viewport, each toggled from the View menu, each
//! remembering whether it is open, its position and its size in `ui.json`
//! (`ck_windows`). Every workbench tab can float as such a window too, so the tabs stay
//! reachable while the CK layout grows (a dock layout comes only if a CK sitting shows
//! docks; director assumption pending the owner).
//!
//! - [`cell_view`]: the Cell View window (M7.W2), `cell`'s worldspaces, cells and
//!   references.
//! - [`warnings`]: the Warnings window (M7.M4), `validate` findings and `load` problems.
//! - [`use_info`]: the Use Info window (M7.W5), `refs --incoming` of the selected record.
//! - `shell.rs` is the glue the window shell (`app.rs`) declares as its own child module,
//!   so it can reach the tabs; everything else here is toolkit-light state.
//!
//! The edit dialog's framework lives in `crate::dialogs`.

pub mod cell_view;
pub mod use_info;
pub mod warnings;

use std::collections::BTreeMap;

use eframe::egui;
use serde_json::{json, Map, Value};

/// The CK-style windows the View menu toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WindowKind {
    Warnings,
    UseInfo,
    EditDialog,
}

impl WindowKind {
    /// Every window, in View-menu order.
    pub const ALL: [WindowKind; 3] = [
        WindowKind::Warnings,
        WindowKind::UseInfo,
        WindowKind::EditDialog,
    ];

    /// Title and `ui.json` key.
    pub fn title(self) -> &'static str {
        match self {
            WindowKind::Warnings => "Warnings",
            WindowKind::UseInfo => "Use Info",
            WindowKind::EditDialog => "Edit Dialog",
        }
    }

    /// Where a window opens the first time (below the menu bar and tabs), in points.
    pub fn default_pos(self) -> [f32; 2] {
        match self {
            WindowKind::Warnings => [24.0, 420.0],
            WindowKind::UseInfo => [520.0, 110.0],
            WindowKind::EditDialog => [700.0, 90.0],
        }
    }

    /// The size a window opens at the first time, in points.
    pub fn default_size(self) -> [f32; 2] {
        match self {
            WindowKind::Warnings => [760.0, 320.0],
            WindowKind::UseInfo => [640.0, 360.0],
            WindowKind::EditDialog => [620.0, 520.0],
        }
    }
}

/// The `ui.json` key of a floating tab.
pub fn tab_key(label: &str) -> String {
    format!("tab:{label}")
}

/// One window's remembered state.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WinState {
    pub open: bool,
    /// Left, top (the outer frame), width and height (the content), in points.
    pub rect: Option<[f32; 4]>,
}

/// Every window's state, as kept in `ui.json` under `ck_windows`:
/// `{"Warnings": {"open": true, "rect": [x, y, w, h]}, "tab:Records": {...}}`.
pub fn layout_from_json(v: Option<&Value>) -> BTreeMap<String, WinState> {
    let mut out = BTreeMap::new();
    let Some(Value::Object(map)) = v else {
        return out;
    };
    for (k, w) in map.iter().take(64) {
        let open = w.get("open").and_then(Value::as_bool).unwrap_or(false);
        let rect = w.get("rect").and_then(Value::as_array).and_then(|a| {
            let n: Vec<f32> = a
                .iter()
                .filter_map(Value::as_f64)
                .map(|f| f as f32)
                .filter(|f| f.is_finite() && f.abs() < 100_000.0)
                .collect();
            match n.as_slice() {
                [x, y, w, h] if *w >= 80.0 && *h >= 40.0 => Some([*x, *y, *w, *h]),
                _ => None,
            }
        });
        out.insert(k.clone(), WinState { open, rect });
    }
    out
}

/// The inverse of [`layout_from_json`].
pub fn layout_to_json(layout: &BTreeMap<String, WinState>) -> Value {
    let mut map = Map::new();
    for (k, w) in layout {
        let mut o = Map::new();
        o.insert("open".into(), json!(w.open));
        if let Some([x, y, wd, h]) = w.rect {
            o.insert(
                "rect".into(),
                json!([x.round(), y.round(), wd.round(), h.round()]),
            );
        }
        map.insert(k.clone(), Value::Object(o));
    }
    Value::Object(map)
}

/// What the windows wait for from the engine (requests they sent themselves).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    /// `refs --incoming` for the Use Info window.
    UseInfo,
    /// `schema` of a record type, for a default edit-dialog layout.
    Schema(String),
}

/// The state of the CK-style shell: the windows, the floating tabs, and the requests
/// the windows sent.
#[derive(Default)]
pub struct Shell {
    /// `ui.json` was read (on the first frame).
    pub loaded: bool,
    pub layout: BTreeMap<String, WinState>,
    /// The layout changed: write `ui.json`.
    pub dirty: bool,
    /// Bring this window to the front on the next frame.
    pub focus: Option<String>,
    /// Each top-level menu's button id, from the last frame (the tour opens them).
    pub menu_ids: Vec<(String, egui::Id)>,
    /// The tour opens this menu (by name) each frame while set.
    pub tour_menu: Option<String>,
    pub tickets: BTreeMap<u64, Pending>,
    pub warnings: warnings::WarningsState,
    pub use_info: use_info::UseInfoState,
    pub dialog: crate::dialogs::DialogState,
    /// The screenshot tour's own steps (after the tabs).
    pub tour_step: usize,
    pub tour_frames: u8,
    /// When the tour set its current step up (windows fade in over real time, so the
    /// image waits for time, not frames).
    pub tour_since: Option<std::time::Instant>,
    /// The tab the central panel showed last that does not float (where it returns when
    /// the shown tab is floated).
    pub last_docked: Option<&'static str>,
}

impl Shell {
    /// Whether window `kind` is open.
    pub fn is_open(&self, kind: WindowKind) -> bool {
        self.layout.get(kind.title()).is_some_and(|w| w.open)
    }

    /// Opens or closes window `kind` (remembered).
    pub fn set_open(&mut self, kind: WindowKind, open: bool) {
        self.set(kind.title(), open);
    }

    /// Whether tab `label` floats as a window.
    pub fn is_floating(&self, label: &str) -> bool {
        self.layout.get(&tab_key(label)).is_some_and(|w| w.open)
    }

    /// Floats tab `label` as a window, or docks it back into the central panel.
    pub fn set_floating(&mut self, label: &str, floating: bool) {
        self.set(&tab_key(label), floating);
    }

    fn set(&mut self, key: &str, open: bool) {
        let w = self.layout.entry(key.to_string()).or_default();
        if w.open != open {
            w.open = open;
            self.dirty = true;
        }
        if open {
            self.focus = Some(key.to_string());
        }
    }
}

/// What [`floating`] needs to know about one window besides its state.
#[derive(Debug, Clone, Copy)]
pub struct WinSpec<'a> {
    /// `ui.json` key (also the egui id).
    pub key: &'a str,
    pub title: &'a str,
    /// The position and size it opens at when `ui.json` has none.
    pub default_pos: [f32; 2],
    pub default_size: [f32; 2],
    /// Bring it to the front this frame.
    pub focus: bool,
}

/// Shows one floating window: open state, position and size from `state` (the first
/// time), the close button, and any move or resize written back to `state` once the
/// pointer is released (`dirty` then asks for `ui.json` to be written).
pub fn floating(
    ctx: &egui::Context,
    state: &mut WinState,
    dirty: &mut bool,
    spec: WinSpec<'_>,
    add: impl FnOnce(&mut egui::Ui),
) {
    let WinSpec {
        key,
        title,
        default_pos,
        default_size,
        focus,
    } = spec;
    if !state.open {
        return;
    }
    let id = egui::Id::new(("kitkraft-ck-window", key));
    let mut open = true;
    let mut window = egui::Window::new(title)
        .id(id)
        .open(&mut open)
        .resizable(true)
        .collapsible(true)
        .constrain(true);
    window = match state.rect {
        Some([x, y, w, h]) => window
            .default_pos(egui::pos2(x, y))
            .default_size(egui::vec2(w, h)),
        None => window
            .default_pos(egui::pos2(default_pos[0], default_pos[1]))
            .default_size(egui::vec2(default_size[0], default_size[1])),
    };
    let mut inner_size = None;
    let shown = window.show(ctx, |ui| {
        inner_size = Some(ui.available_rect_before_wrap().size());
        add(ui);
    });
    if focus {
        if let Some(r) = &shown {
            ctx.move_to_top(r.response.layer_id);
        }
    }
    if !open {
        state.open = false;
        *dirty = true;
        return;
    }
    let released = !ctx.input(|i| i.pointer.any_down());
    if let (Some(r), Some(size), true) = (shown, inner_size, released) {
        let now = [r.response.rect.min.x, r.response.rect.min.y, size.x, size.y];
        let moved = match state.rect {
            Some(old) => old.iter().zip(now).any(|(a, b)| (a - b).abs() > 1.0),
            None => true,
        };
        if moved && size.x >= 80.0 && size.y >= 40.0 {
            state.rect = Some(now);
            *dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_round_trips_and_ignores_damage() {
        let mut layout = BTreeMap::new();
        layout.insert(
            "Warnings".to_string(),
            WinState {
                open: true,
                rect: Some([10.0, 20.0, 700.0, 300.0]),
            },
        );
        layout.insert(
            tab_key("Records"),
            WinState {
                open: false,
                rect: None,
            },
        );
        let v = layout_to_json(&layout);
        assert_eq!(layout_from_json(Some(&v)), layout);
        let damaged = json!({"Warnings": {"open": "yes", "rect": [1, 2, 3]},
                             "Use Info": {"open": true, "rect": [0, 0, 1e9, 50]},
                             "Edit Dialog": 5});
        let back = layout_from_json(Some(&damaged));
        assert_eq!(back.get("Warnings"), Some(&WinState::default()));
        assert_eq!(
            back.get("Use Info"),
            Some(&WinState {
                open: true,
                rect: None
            })
        );
        assert_eq!(back.get("Edit Dialog"), Some(&WinState::default()));
        assert!(layout_from_json(None).is_empty());
    }

    #[test]
    fn toggles_mark_the_layout_dirty_once() {
        let mut shell = Shell::default();
        shell.set_open(WindowKind::Warnings, true);
        assert!(shell.is_open(WindowKind::Warnings) && shell.dirty);
        assert_eq!(shell.focus.as_deref(), Some("Warnings"));
        shell.dirty = false;
        shell.set_open(WindowKind::Warnings, true);
        assert!(!shell.dirty);
        shell.set_floating("Records", true);
        assert!(shell.is_floating("Records") && !shell.is_floating("Load"));
    }
}
