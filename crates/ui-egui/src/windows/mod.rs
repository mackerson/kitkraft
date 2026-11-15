//! The CK-style window model (M7.M0, `docs/workbench.md` "CK-style windows"): floating
//! `egui::Window`s inside the main viewport, each toggled from the View menu, each
//! remembering whether it is open, its position and its size in `ui.json`
//! (`ck_windows`). Every workbench tab can float as such a window too, so the tabs stay
//! reachable while the CK layout grows (a dock layout comes only if a CK sitting shows
//! docks; director assumption pending the owner).
//!
//! - [`cell_view`]: the Cell View (M7.W2), `cell`'s worldspaces, cells and references;
//!   a tab since the owner's 2026-11-01 walkthrough (finding 1), so it floats like any
//!   tab.
//! - [`warnings`]: the Warnings window (M7.M4), `validate` findings and `load` problems.
//! - [`use_info`]: the Use Info window (M7.W5), `refs --incoming` of the selected record.
//! - [`object_window`], [`data_dialog`], [`preferences`]: the Object Window (M7.W1), the
//!   Data dialog (M7.M2) and Preferences (M7.M3); stubs declared at step 0 of sprint
//!   2026-11-02 for pipeline W.
//! - `shell.rs` is the glue the window shell (`app.rs`) declares as its own child module,
//!   so it can reach the tabs; everything else here is toolkit-light state.
//!
//! The edit dialog's framework lives in `crate::dialogs`.

pub mod cell_view;
pub mod data_dialog;
pub mod object_window;
pub mod preferences;
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
    // Step 0 of sprint 2026-11-02: pipeline W's windows (M7.W1, M7.M2, M7.M3) and
    // pipeline E's build spec (M7.E5); drawn by their owners from `shell.rs`.
    ObjectWindow,
    DataDialog,
    Preferences,
    BuildSpec,
}

impl WindowKind {
    /// Every window, in View-menu order.
    pub const ALL: [WindowKind; 7] = [
        WindowKind::Warnings,
        WindowKind::UseInfo,
        WindowKind::EditDialog,
        WindowKind::ObjectWindow,
        WindowKind::DataDialog,
        WindowKind::Preferences,
        WindowKind::BuildSpec,
    ];

    /// Title and `ui.json` key.
    pub fn title(self) -> &'static str {
        match self {
            WindowKind::Warnings => "Warnings",
            WindowKind::UseInfo => "Use Info",
            WindowKind::EditDialog => "Edit Dialog",
            WindowKind::ObjectWindow => "Object Window",
            WindowKind::DataDialog => "Data Dialog",
            WindowKind::Preferences => "Preferences",
            WindowKind::BuildSpec => "Build Spec",
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
    /// A `cell` request (worlds, interiors, a world's cells, a cell's references,
    /// or their next page) for the Cell View window.
    CellView,
    /// `schema` of a record type, for a default edit-dialog layout.
    Schema(String),
    /// A request of the Object Window (M7.W1; step 0 of sprint 2026-11-02).
    Object(object_window::ObjectTicket),
    /// A request of the Data dialog (M7.M2; step 0 of sprint 2026-11-02).
    Data(data_dialog::DataTicket),
    /// A request of the editor (M7.E2 to M7.E6; step 0 of sprint 2026-11-02).
    Editor(crate::dialogs::editor::EditorTicket),
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
    /// A window frame's outer-minus-content delta (its title bar and margins),
    /// measured from the last window drawn (zero until one is); kept in memory only,
    /// never in `ui.json`.
    pub chrome: egui::Vec2,
    /// Bring this window to the front on the next frame.
    pub focus: Option<String>,
    /// Each top-level menu's button id, from the last frame (the tour opens them).
    pub menu_ids: Vec<(String, egui::Id)>,
    /// The tour opens this menu (by name) each frame while set.
    pub tour_menu: Option<String>,
    pub tickets: BTreeMap<u64, Pending>,
    pub warnings: warnings::WarningsState,
    pub use_info: use_info::UseInfoState,
    pub cell_view: cell_view::CellViewState,
    /// The Cell View was shown on the last frame (its error clears when shown again).
    pub cell_view_open: bool,
    /// The Cell View was shown on this frame (docked or floating).
    pub cell_view_shown: bool,
    pub dialog: crate::dialogs::DialogState,
    // Step 0 of sprint 2026-11-02: pipeline W's windows (M7.W1, M7.M2, M7.M3).
    pub object_window: object_window::ObjectWindowState,
    pub data_dialog: data_dialog::DataDialogState,
    pub preferences: preferences::PreferencesState,
    /// Pipeline E's editor (M7.E2 to M7.E6; step 0 of sprint 2026-11-02).
    pub editor: crate::dialogs::editor::EditorState,
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

    /// The open windows' remembered rects (outer min, content size), for the
    /// first-open placement's overlap check.
    pub fn open_rects(&self) -> Vec<[f32; 4]> {
        self.layout
            .iter()
            .filter(|(_, w)| w.open)
            .filter_map(|(_, w)| w.rect)
            .collect()
    }
}

/// What [`floating`] needs to know about one window besides its state.
#[derive(Debug, Clone, Copy)]
pub struct WinSpec<'a> {
    /// `ui.json` key (also the egui id).
    pub key: &'a str,
    pub title: &'a str,
    /// Bring it to the front this frame.
    pub focus: bool,
}

/// Where a window without a remembered rect opens, and at what size (UX audit A6,
/// `docs/design/workbench-ux.md`): cascaded from the right half of the central panel,
/// each window below or beside the last, at the theme's size no larger than the space
/// it gets. One rule for every window kind. `open` holds the other open windows'
/// remembered rects (outer min, content size); `chrome` is the frame delta measured
/// from a drawn window (zero until the first frame draws one, where the theme's frame
/// bounds stand in).
fn first_open(
    ctx: &egui::Context,
    open: &[[f32; 4]],
    chrome: egui::Vec2,
) -> (egui::Pos2, egui::Vec2) {
    let theme = crate::theme::active(ctx);
    let delta = if chrome == egui::Vec2::ZERO {
        // No window drawn yet: bound the frame (two window margins plus a title row
        // of at most two rows) from the theme, so a remembered window's full reach
        // is known.
        egui::vec2(
            theme.spacing.panel_margin * 2.0,
            theme.density.row_height * 2.0 + theme.spacing.panel_margin * 2.0,
        )
    } else {
        chrome
    };
    let occupied: Vec<egui::Rect> = open
        .iter()
        .map(|[x, y, w, h]| {
            egui::Rect::from_min_size(egui::pos2(*x, *y), egui::vec2(w + delta.x, h + delta.y))
        })
        .collect();
    let viewport = ctx.viewport_rect();
    // The tab bar and the status line size to their contents, and the context does
    // not report their rects, so the cascade bounds are the theme's row and frame
    // bounds: above, the menu row, the bar's frame and up to two rows of tabs;
    // below, the status line's frame and row.
    let top = viewport.min.y
        + theme.density.row_height
        + theme.spacing.item_y
        + theme.spacing.panel_margin
        + theme.density.row_height * 2.0
        + theme.spacing.item_y
        + theme.density.cell_y;
    let bottom = viewport.max.y
        - theme.spacing.panel_margin
        - theme.density.row_height
        - theme.density.cell_y;
    let margin = theme.spacing.panel_margin;
    let area = egui::Rect::from_min_max(
        egui::pos2(viewport.min.x + margin, top + margin),
        egui::pos2(viewport.max.x - margin, bottom - margin),
    );
    // A tall view plus the rows a window's content keeps around it (the Object
    // Window's action, filter and note rows are five of them) and the window
    // frame, so a window with a tall view opens with it at full height.
    let pitch = theme.density.row_height + theme.density.cell_y;
    let desired = egui::vec2(
        theme.sizes.field_wide,
        (theme.sizes.view_tall_rows + 5.0) * pitch
            + theme.density.row_height * 2.0
            + theme.spacing.panel_margin * 2.0,
    );
    let gap = theme.spacing.block;
    let mid_x = (area.min.x + area.max.x) / 2.0;
    let right = egui::Rect::from_min_max(egui::pos2(mid_x, area.min.y), area.max);
    let left = egui::Rect::from_min_max(area.min, egui::pos2(mid_x, area.max.y));
    for half in [right, left] {
        let mut y = half.min.y;
        for r in &occupied {
            if r.intersects(half) {
                y = y.max(r.max.y);
            }
        }
        if y > half.min.y {
            y += gap;
        }
        let w = desired.x.min(half.max.x - half.min.x);
        let h = desired.y.min(half.max.y - y);
        if w >= 80.0 && h >= 40.0 {
            return (egui::pos2(half.min.x, y), egui::vec2(w, h));
        }
    }
    // No room without overlap: the whole area, clamped to it.
    let w = desired.x.min(area.max.x - area.min.x).max(80.0);
    let h = desired.y.min(area.max.y - area.min.y).max(40.0);
    (area.min, egui::vec2(w, h))
}

/// Shows one floating window: open state, position and size from `state` (the first
/// time, where [`first_open`] places it clear of the other open windows), the close
/// button, and any move or resize written back to `state` once the pointer is
/// released (`dirty` then asks for `ui.json` to be written).
pub fn floating(
    ctx: &egui::Context,
    state: &mut WinState,
    dirty: &mut bool,
    chrome: &mut egui::Vec2,
    open: &[[f32; 4]],
    spec: WinSpec<'_>,
    add: impl FnOnce(&mut egui::Ui),
) {
    let WinSpec { key, title, focus } = spec;
    if !state.open {
        return;
    }
    let id = egui::Id::new(("kitkraft-ck-window", key));
    let mut window_open = true;
    let mut window = egui::Window::new(title)
        .id(id)
        .open(&mut window_open)
        .resizable(true)
        .collapsible(true)
        .constrain(true);
    window = match state.rect {
        Some([x, y, w, h]) => window
            .default_pos(egui::pos2(x, y))
            .default_size(egui::vec2(w, h)),
        None => {
            let (pos, size) = first_open(ctx, open, *chrome);
            window.default_pos(pos).default_size(size)
        }
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
    if !window_open {
        state.open = false;
        *dirty = true;
        return;
    }
    let released = !ctx.input(|i| i.pointer.any_down());
    if let (Some(r), Some(size)) = (shown, inner_size) {
        if size.x >= 80.0 && size.y >= 40.0 {
            // The frame delta (outer minus content): the first-open placement keeps
            // the next window clear of this one's title bar and margins.
            *chrome = egui::vec2(
                r.response.rect.max.x - r.response.rect.min.x - size.x,
                r.response.rect.max.y - r.response.rect.min.y - size.y,
            );
            if released {
                let now = [r.response.rect.min.x, r.response.rect.min.y, size.x, size.y];
                let moved = match state.rect {
                    Some(old) => old.iter().zip(now).any(|(a, b)| (a - b).abs() > 1.0),
                    None => true,
                };
                if moved {
                    state.rect = Some(now);
                    *dirty = true;
                }
            }
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
