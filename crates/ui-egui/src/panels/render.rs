//! The Render Window tab (M7.R1 to M7.R3): a toolbar with a cell picker, the 3D viewport
//! (`crate::viewport`) in the rest of the tab, and overlays: the stats (frame time, draw
//! calls, frames drawn), the legend, and the selected reference.
//!
//! The picker loads one cell through the engine's `cell` command with `with_bounds:
//! true` (M7.S3, extended for M7.R3), a page of 1,000 references at a time up to the draw
//! cap [`DRAW_CAP`]. Each reference becomes a marker ([`crate::viewport::cell`]): its
//! base's `OBND` box placed by the reference's position, rotation and scale, or, when the
//! base has no usable bounds, a small shape that says its type; colored by base type from
//! the theme's palette tokens, with a legend. A click selects a reference: the overlay
//! shows its EditorID, FormID and base, and the Records inspector shows it too (the
//! shared selection, [`Action::Select`]); a record inspected there that is a reference of
//! the cell is selected here. The camera frames the cell when it loads. The Cell View
//! window (M7.W2) will drive the same picker later.
//!
//! The controls ([`crate::viewport::BINDINGS`]) are not yet observed against the CK's
//! Render Window (`docs/ck-parity.md`, "Interface parity").

use eframe::egui::{self, Color32, Rect, TextEdit};
use serde_json::{json, Map, Value};

use std::ops::{Add, Sub};

use crate::gate::Request;
use crate::panels::common::{action_bar, link, panel_head, Action, Slot, SlotKey, TabCtx};
use crate::theme::{self, Field, Palette, Status};
use crate::viewport::camera::OrbitCamera;
use crate::viewport::cell::{self, Category, CellPage, CellRef};
use crate::viewport::scene::{self, Marker, Shape};
use crate::viewport::{FaceMode, Stats, Viewport, BINDINGS};

/// The most references drawn (and fetched) for one cell. Measured on the release build
/// (`docs/design/workbench-performance.md`, "Viewport", V2): see that row for the frame
/// times at and beyond the cap.
pub const DRAW_CAP: usize = 20_000;
/// References per `cell` request (the command's largest page).
pub const PAGE: u64 = 1_000;
/// Measurement only: draw this many markers by tiling the loaded cell's (beyond the cap),
/// to find what the viewport draws at 60 fps (`docs/design/workbench-performance.md`).
pub const STRESS_ENV: &str = "KITKRAFT_VIEWPORT_STRESS";
/// The screenshot tour only (`crate::shots`): the cell the Render Window loads, the
/// reference it selects, and the face mode (`solid`, `see-through`, `wire`).
pub const TOUR_CELL_ENV: &str = "KITKRAFT_TOUR_RENDER_CELL";
/// See [`TOUR_CELL_ENV`].
pub const TOUR_SELECT_ENV: &str = "KITKRAFT_TOUR_RENDER_SELECT";
/// See [`TOUR_CELL_ENV`].
pub const TOUR_FACES_ENV: &str = "KITKRAFT_TOUR_RENDER_FACES";

/// Which cell to load: one by formref or EditorID (interior or exterior), or an
/// exterior cell by its world and grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellPick {
    Cell(String),
    Grid { world: String, x: i64, y: i64 },
}

/// A grid as typed: `2,1`, `2 1`, `(2, 1)`, `-1,-1`.
pub fn parse_grid(text: &str) -> Result<(i64, i64), String> {
    let inner = text.trim().trim_start_matches('(').trim_end_matches(')');
    let parts: Vec<&str> = inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    match parts.as_slice() {
        [x, y] => match (x.parse::<i64>(), y.parse::<i64>()) {
            (Ok(x), Ok(y)) => Ok((x, y)),
            _ => Err(format!(
                "`{}`: the grid is two whole numbers, x and y",
                text.trim()
            )),
        },
        _ => Err(format!(
            "`{}`: the grid is two whole numbers, x and y",
            text.trim()
        )),
    }
}

/// The picker's text: a cell (`CityNewAtlantisApartmentTEMPLATE`, `Starfield.esm:21966F`)
/// or a world and a grid (`NewAtlantis 2,1`, `NewAtlantis 2 1`, `NewAtlantis (-1, 0)`).
pub fn parse_pick(text: &str) -> Result<CellPick, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err(
            "give a cell (EditorID or Plugin.esm:XXXXXX) or a world and a grid (NewAtlantis 2,1)"
                .to_string(),
        );
    }
    match t.split_once(char::is_whitespace) {
        None => Ok(CellPick::Cell(t.to_string())),
        Some((world, grid)) => {
            let (x, y) = parse_grid(grid)?;
            Ok(CellPick::Grid {
                world: world.to_string(),
                x,
                y,
            })
        }
    }
}

/// The `cell` request for a page of `pick`'s references, with their bases' bounds.
pub fn cell_request(pick: &CellPick, cursor: Option<&str>) -> Request {
    let mut args = Map::new();
    match pick {
        CellPick::Cell(c) => {
            args.insert("cell".into(), json!(c));
        }
        CellPick::Grid { world, x, y } => {
            args.insert("world".into(), json!(world));
            args.insert("x".into(), json!(x));
            args.insert("y".into(), json!(y));
        }
    }
    args.insert("with_bounds".into(), json!(true));
    args.insert("limit".into(), json!(PAGE));
    if let Some(c) = cursor {
        args.insert("cursor".into(), json!(c));
    }
    Request::new("cell", Value::Object(args))
}

/// The overlay's lines: frame time (CPU: preparing the frame and issuing GL calls; GPU
/// from a timer query when the context has one) and its median, 95th percentile and
/// maximum over the recent frames, draw calls and marker instances, frames drawn since
/// the window opened (it stays still while nothing moves: the viewport never repaints on
/// its own), the viewport size and the camera.
pub fn stats_lines(stats: &Stats, cam: &OrbitCamera) -> Vec<String> {
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1_000.0;
    let ui = ms(stats.ui);
    let gl = ms(stats.draw.submit);
    let [w, h] = stats.draw.size_px;
    let t = cam.target;
    let gpu = stats
        .draw
        .gpu
        .map_or_else(|| "n/a".to_string(), |g| format!("{:.2} ms", ms(g)));
    vec![
        format!(
            "frame   {:.2} ms cpu (ui {ui:.2}, gl {gl:.2}), gpu {gpu}",
            ui + gl
        ),
        format!(
            "recent  p50 {:.2} p95 {:.2} max {:.2} ms ({} frames)",
            ms(stats.times.p50),
            ms(stats.times.p95),
            ms(stats.times.max),
            stats.times.samples
        ),
        format!(
            "draws   {} ({} markers)",
            stats.draw.draw_calls, stats.draw.instances
        ),
        format!("frames  {}", stats.draw.frames),
        format!("size    {w} x {h} px"),
        format!(
            "camera  yaw {:.0} pitch {:.0} dist {:.1}",
            cam.yaw.to_degrees(),
            cam.pitch.to_degrees(),
            cam.distance
        ),
        format!("target  {:.1}, {:.1}, {:.1}", t.x, t.y, t.z),
    ]
}

/// The selected reference's lines: EditorID, FormID and record type, base (EditorID,
/// formref, type), placement, and where its box comes from.
pub fn selection_lines(r: &CellRef) -> Vec<String> {
    let none = || "(none)".to_string();
    let v = |v: glam::Vec3| format!("{:.2}, {:.2}, {:.2}", v.x, v.y, v.z);
    let deg = |v: glam::Vec3| {
        format!(
            "{:.1}, {:.1}, {:.1}",
            v.x.to_degrees(),
            v.y.to_degrees(),
            v.z.to_degrees()
        )
    };
    vec![
        format!("EditorID  {}", r.editor_id.clone().unwrap_or_else(none)),
        format!(
            "FormID    {} {}{}",
            r.form,
            r.signature.as_deref().unwrap_or("?"),
            if r.persistent { " persistent" } else { "" }
        ),
        format!(
            "base      {} {} {}",
            r.base_editor_id.clone().unwrap_or_else(none),
            r.base.clone().unwrap_or_else(none),
            r.base_signature.as_deref().unwrap_or("?")
        ),
        format!(
            "at        {}   rot {} deg   scale {}",
            r.position.map_or_else(none, v),
            deg(r.rotation),
            r.scale
                .map_or_else(|| "1 (none stored)".to_string(), |s| format!("{s}"))
        ),
        match r.bounds {
            Some((lo, hi)) => format!("box       OBND {} .. {}", v(lo), v(hi)),
            None => format!(
                "box       none: the base has no OBND the schema reads ({} shape)",
                r.category().label()
            ),
        },
        format!("plugin    {}", r.plugin),
    ]
}

/// The controls as one hover text.
pub fn controls_text() -> String {
    let mut out: Vec<String> = BINDINGS.iter().map(|(a, b)| format!("{a}: {b}")).collect();
    out.push("select: click a marker (the smallest box under the cursor)".into());
    out.push("Not yet observed against the CK's Render Window (docs/ck-parity.md).".into());
    out.join("\n")
}

/// Copies of `markers` laid out side by side until there are `n` (the stress
/// measurement, [`STRESS_ENV`]).
pub fn tile(markers: &[Marker], n: usize) -> Vec<Marker> {
    let Some((lo, hi)) = scene::bounds_of(markers) else {
        return markers.to_vec();
    };
    let size = hi.sub(lo);
    let (w, h) = (size.x.max(1.0) * 1.1, size.y.max(1.0) * 1.1);
    let per_copy = markers.len().max(1);
    let copies = n.div_ceil(per_copy).max(1);
    let cols = (copies as f64).sqrt().ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity(n);
    for k in 0..copies {
        let (col, row) = (
            k.checked_rem(cols).unwrap_or(0),
            k.checked_div(cols).unwrap_or(0),
        );
        let shift = glam::Vec3::new(col as f32 * w, row as f32 * h, 0.0);
        for m in markers {
            if out.len() >= n {
                return out;
            }
            out.push(Marker {
                position: m.position.add(shift),
                ..*m
            });
        }
    }
    out
}

// ---- drawing

/// The tab's state.
pub struct RenderTab {
    pub viewport: Viewport,
    /// Show the stats overlay, the legend.
    pub stats: bool,
    pub legend: bool,
    /// The picker's text.
    pub pick_text: String,
    /// The cell loading or shown, and the references fetched so far.
    pick: Option<CellPick>,
    page: Option<CellPage>,
    loading: bool,
    /// The reference each marker stands for.
    index: Vec<usize>,
    /// The palette the markers were colored with.
    palette: Option<Palette>,
    /// The Records inspector's record (set by the shell each frame) and the one last
    /// acted on.
    pub inspected: Option<String>,
    seen_inspected: Option<String>,
    /// The picker's last parse error.
    error: Option<String>,
    /// [`STRESS_ENV`], when set.
    stress: usize,
    /// The draw cap in use.
    cap: usize,
}

impl Default for RenderTab {
    fn default() -> Self {
        let stress = std::env::var(STRESS_ENV)
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0);
        Self {
            viewport: Viewport::default(),
            stats: true,
            legend: true,
            pick_text: String::new(),
            pick: None,
            page: None,
            loading: false,
            index: Vec::new(),
            palette: None,
            inspected: None,
            seen_inspected: None,
            error: None,
            stress,
            cap: DRAW_CAP,
        }
    }
}

impl RenderTab {
    /// Starts loading `pick` (the first page); the shell submits the request.
    fn start(&mut self, pick: CellPick) -> Request {
        let req = cell_request(&pick, None);
        self.pick = Some(pick);
        self.page = None;
        self.loading = true;
        self.error = None;
        req
    }

    /// A page arrived: keep its references; the next page's request while the cell has
    /// more and the cap is not reached, else the markers are built and framed.
    /// The page is read on the engine thread (`crate::prepare`).
    pub(crate) fn receive_page(&mut self, page: CellPage) -> Option<Request> {
        let next = page.next_cursor.clone();
        match &mut self.page {
            Some(p) if p.cell == page.cell => {
                p.refs.extend(page.refs);
                p.next_cursor = page.next_cursor;
                p.limitations = page.limitations;
            }
            _ => self.page = Some(page),
        }
        let fetched = self.page.as_ref().map_or(0, |p| p.refs.len());
        if let (Some(cursor), true) = (next, fetched < self.cap) {
            // Later pages name the cell by its form (the grid found it once).
            let cell = self
                .page
                .as_ref()
                .map(|p| p.cell.clone())
                .unwrap_or_default();
            return Some(cell_request(&CellPick::Cell(cell), Some(&cursor)));
        }
        self.loading = false;
        self.palette = None;
        self.seen_inspected = None;
        None
    }

    /// The screenshot tour's seed (`crate::shots`): the cell named by [`TOUR_CELL_ENV`],
    /// the face mode by [`TOUR_FACES_ENV`], and the record by [`TOUR_SELECT_ENV`] selected
    /// through the shared selection. Nothing without the variables.
    pub(crate) fn tour_seed(&mut self) -> Vec<Action> {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let mut out = Vec::new();
        if let Some(f) = var(TOUR_FACES_ENV) {
            if let Some(m) = FaceMode::ALL.into_iter().find(|m| m.label() == f.trim()) {
                self.viewport.faces = m;
            }
        }
        if let Some(cell) = var(TOUR_CELL_ENV) {
            self.pick_text = cell;
            if let Ok(p) = parse_pick(&self.pick_text) {
                out.push(Action::Submit(Slot::RenderCell, self.start(p)));
            }
        }
        if let Some(form) = var(TOUR_SELECT_ENV) {
            out.push(Action::Select(form));
        }
        out
    }

    /// The request failed (the shell keeps its error in the results).
    pub(crate) fn failed(&mut self) {
        self.loading = false;
    }

    /// The reference behind marker `i`.
    fn ref_of(&self, marker: usize) -> Option<&CellRef> {
        let r = *self.index.get(marker)?;
        self.page.as_ref()?.refs.get(r)
    }

    /// Rebuilds the markers in `palette`'s colors, keeping the selection.
    fn rebuild(&mut self, palette: &Palette, frame: bool) {
        let keep = self
            .viewport
            .selected()
            .and_then(|i| self.index.get(i).copied());
        let refs = self.page.as_ref().map_or(&[][..], |p| p.refs.as_slice());
        let (mut markers, index) = cell::markers(refs, palette, self.cap);
        if self.stress > 0 && !markers.is_empty() {
            markers = tile(&markers, self.stress);
        }
        self.index = index;
        self.viewport.set_markers(markers);
        if frame {
            self.viewport.frame_all();
        }
        self.viewport
            .select(keep.and_then(|r| self.index.iter().position(|i| *i == r)));
        self.palette = Some(*palette);
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx) {
        let palette = theme::palette(ui);
        if !self.loading && self.page.is_some() && self.palette != Some(palette) {
            // New data frames the camera; a theme change only recolors.
            let fresh = self.palette.is_none();
            self.rebuild(&palette, fresh);
        }
        self.follow_inspector();
        self.toolbar(ui, ctx);
        theme::gap(ui);
        let response = self.viewport.show(ui);
        if let Some(picked) = self.viewport.take_pick() {
            if let Some(r) = picked.and_then(|i| self.ref_of(i)) {
                let form = r.form.clone();
                self.seen_inspected = Some(form.clone());
                ctx.actions.push(Action::Select(form));
            }
        }
        let stats = self.viewport.stats();
        if let Some(err) = &stats.error {
            // The status component for painted text (M6.V4): glyph and word lead it.
            let (line, color) =
                theme::status_painted(ui, Status::Fail, &format!("viewport: {err}"));
            overlay(ui, response.rect, egui::Align2::LEFT_TOP, &[line], color);
            return;
        }
        if self.stats {
            let mut lines = stats_lines(&stats, &self.viewport.camera);
            if self.stress > 0 {
                lines.push(format!("stress  {} markers ({STRESS_ENV})", self.stress));
            }
            overlay(
                ui,
                response.rect,
                egui::Align2::LEFT_TOP,
                &lines,
                ui.visuals().strong_text_color(),
            );
        }
        let selected = self
            .viewport
            .selected()
            .and_then(|i| self.ref_of(i))
            .cloned();
        if let Some(r) = &selected {
            overlay(
                ui,
                response.rect,
                egui::Align2::RIGHT_TOP,
                &selection_lines(r),
                ui.visuals().strong_text_color(),
            );
        }
        if self.legend {
            if let Some(page) = &self.page {
                legend(ui, response.rect, &palette, &cell::counts(&page.refs));
            }
        }
    }

    /// Selects the Records inspector's record here when it is a reference of the cell.
    fn follow_inspector(&mut self) {
        if self.inspected == self.seen_inspected {
            return;
        }
        self.seen_inspected.clone_from(&self.inspected);
        let Some(form) = &self.inspected else { return };
        let Some(page) = &self.page else { return };
        let marker = page
            .refs
            .iter()
            .position(|r| &r.form == form)
            .and_then(|r| self.index.iter().position(|i| *i == r));
        if marker.is_some() {
            self.viewport.select(marker);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx) {
        let busy = ctx.busy;
        // The panel frame (M6.V5): title, then the toolbar as the action bar.
        panel_head(ui, "Render Window", "");
        action_bar(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("cell");
                let edit = ui.add(
                TextEdit::singleline(&mut self.pick_text)
                    .desired_width(theme::width(ui, Field::Long))
                    .hint_text(
                        "CityNewAtlantisApartmentTEMPLATE, Starfield.esm:21966F, NewAtlantis 2,1",
                    ),
            );
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let load = ui
                    .add_enabled(!busy && !self.loading, egui::Button::new("Load"))
                    .on_hover_text(
                        "cell --with-bounds: the cell's references and their bases' OBND",
                    )
                    .clicked();
                if (load || enter) && !self.loading {
                    match parse_pick(&self.pick_text) {
                        Ok(p) => {
                            let req = self.start(p);
                            ctx.actions.push(Action::Submit(Slot::RenderCell, req));
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                ui.separator();
                for mode in FaceMode::ALL {
                    ui.selectable_value(&mut self.viewport.faces, mode, mode.label());
                }
                ui.separator();
                if ui
                    .add_enabled(self.page.is_some(), egui::Button::new("Frame cell"))
                    .clicked()
                {
                    self.viewport.frame_all();
                }
                if ui
                    .button("Reset view")
                    .on_hover_text("Back to the default view of the origin")
                    .clicked()
                {
                    self.viewport.reset_camera();
                }
                ui.checkbox(&mut self.stats, "Stats");
                ui.checkbox(&mut self.legend, "Legend");
                ui.weak("controls").on_hover_text(controls_text());
            })
        });
        self.status(ui, ctx);
    }

    /// The line under the toolbar: what is loading or shown, the cap, the limitations,
    /// and the selected reference's links.
    fn status(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx) {
        let busy = ctx.busy;
        ui.horizontal_wrapped(|ui| {
            if let Some(e) = &self.error {
                theme::status(ui, theme::Status::Fail, e);
                return;
            }
            if let Some(shown) = ctx.results.get(&SlotKey::from(Slot::RenderCell)) {
                if let (Some(err), false) = (&shown.error, self.loading) {
                    theme::status(ui, theme::Status::Fail, &err.to_string());
                    return;
                }
            }
            let Some(page) = &self.page else {
                if self.loading {
                    ui.spinner();
                    ui.label("loading the cell ...");
                } else {
                    ui.weak("no cell loaded: the placeholder box at the origin; pick a cell above");
                }
                return;
            };
            if self.loading {
                ui.spinner();
                ui.label(format!(
                    "{}: {} of {} references fetched ...",
                    page.cell_editor_id.as_deref().unwrap_or(&page.cell),
                    page.refs.len(),
                    page.total
                ));
                return;
            }
            let drawn = self.index.len();
            let placed = page.refs.iter().filter(|r| r.position.is_some()).count();
            let boxes = page.refs.iter().filter(|r| r.bounds.is_some()).count();
            ui.label(format!(
                "{} {}{}: {drawn} of {} references drawn, {boxes} as OBND boxes",
                page.cell,
                page.cell_editor_id.as_deref().unwrap_or("(no EditorID)"),
                page.world
                    .as_deref()
                    .map_or_else(String::new, |w| format!(" in {w}")),
                page.total
            ));
            if page.total > self.cap {
                theme::status(
                    ui,
                    theme::Status::Warn,
                    &format!(
                        "draw cap: {} of {} references fetched and drawn (DRAW_CAP, docs/design/workbench-performance.md)",
                        self.cap, page.total
                    ),
                );
            }
            let unplaced = page.refs.len().saturating_sub(placed);
            if unplaced > 0 {
                theme::status(
                    ui,
                    theme::Status::Unknown,
                    &format!("{unplaced} without a position: not drawn"),
                );
            }
            if !page.limitations.is_empty() {
                ui.weak(format!("{} limitations", page.limitations.len()))
                    .on_hover_text(page.limitations.join("\n\n"));
            }
            let selected = self.viewport.selected().and_then(|i| self.ref_of(i)).cloned();
            if let Some(r) = selected {
                ui.separator();
                ui.label(r.editor_id.as_deref().unwrap_or(&r.form));
                if link(ui, !busy, "inspect")
                    .on_hover_text("open it in the Records tab")
                    .clicked()
                {
                    ctx.actions.push(Action::Inspect(r.form.clone()));
                }
                if let Some(b) = &r.base {
                    if link(ui, !busy, "base")
                        .on_hover_text("open its base in the Records tab")
                        .clicked()
                    {
                        ctx.actions.push(Action::Inspect(b.clone()));
                    }
                }
                if link(ui, true, "frame").on_hover_text("frame it").clicked() {
                    self.viewport.frame_selected();
                }
            }
        });
    }
}

/// Text in a box at a corner of the viewport, drawn after (over) the 3D scene.
fn overlay(ui: &egui::Ui, rect: Rect, corner: egui::Align2, lines: &[String], text: Color32) {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let pad = ui.spacing().item_spacing.x;
    let painter = ui.painter_at(rect);
    let galley = painter.layout_no_wrap(lines.join("\n"), font, text);
    let inner = rect.shrink(pad);
    let at = corner.align_size_within_rect(galley.size(), inner).min;
    let bg = Rect::from_min_size(at, galley.size()).expand(pad * 0.5);
    let fill = ui.visuals().extreme_bg_color.gamma_multiply(0.8);
    painter.rect_filled(bg, ui.visuals().widgets.noninteractive.corner_radius, fill);
    painter.galley(at, galley, text);
}

/// The legend at the viewport's bottom-left: per group present, its glyph (the shape a
/// reference without bounds is drawn as), its color, its name and count.
fn legend(ui: &egui::Ui, rect: Rect, palette: &Palette, counts: &[(Category, usize)]) {
    if counts.is_empty() {
        return;
    }
    let font = egui::TextStyle::Body.resolve(ui.style());
    let h = ui.text_style_height(&egui::TextStyle::Body);
    let pad = ui.spacing().item_spacing.x;
    let painter = ui.painter_at(rect);
    let text_color = ui.visuals().strong_text_color();
    let galleys: Vec<_> = counts
        .iter()
        .map(|(c, n)| {
            painter.layout_no_wrap(format!("{} {n}", c.label()), font.clone(), text_color)
        })
        .collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + h + pad;
    let height = h * counts.len() as f32;
    let size = egui::vec2(width, height);
    let at = egui::Align2::LEFT_BOTTOM
        .align_size_within_rect(size, rect.shrink(pad))
        .min;
    let bg = Rect::from_min_size(at, size).expand(pad * 0.5);
    let fill = ui.visuals().extreme_bg_color.gamma_multiply(0.8);
    painter.rect_filled(bg, ui.visuals().widgets.noninteractive.corner_radius, fill);
    for (row, ((cat, _), galley)) in counts.iter().zip(galleys).enumerate() {
        let y = at.y + h * row as f32;
        let center = egui::pos2(at.x + h * 0.5, y + h * 0.5);
        let color = theme::color(cat.color(palette));
        glyph(&painter, cat.shape(), center, h * 0.36, color);
        painter.galley(egui::pos2(at.x + h + pad * 0.5, y), galley, text_color);
    }
}

/// A shape's glyph: a square, a diamond or a triangle.
fn glyph(painter: &egui::Painter, shape: Shape, c: egui::Pos2, r: f32, color: Color32) {
    let p = |x: f32, y: f32| egui::pos2(c.x + x * r, c.y + y * r);
    let points = match shape {
        Shape::Box => vec![p(-1.0, -1.0), p(1.0, -1.0), p(1.0, 1.0), p(-1.0, 1.0)],
        Shape::Diamond => vec![p(0.0, -1.2), p(1.2, 0.0), p(0.0, 1.2), p(-1.2, 0.0)],
        Shape::Pyramid => vec![p(0.0, -1.2), p(1.2, 1.0), p(-1.2, 1.0)],
    };
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::viewport::DrawStats;
    use std::time::Duration;

    #[test]
    fn stats_show_frame_time_draw_calls_and_frames() {
        let stats = Stats {
            draw: DrawStats {
                draw_calls: 5,
                instances: 432,
                submit: Duration::from_micros(120),
                gpu: Some(Duration::from_micros(250)),
                frames: 37,
                size_px: [1280, 720],
            },
            ui: Duration::from_micros(300),
            times: crate::viewport::frame_times([
                Duration::from_micros(400),
                Duration::from_micros(500),
            ]),
            error: None,
        };
        let lines = stats_lines(&stats, &OrbitCamera::default());
        assert_eq!(
            lines[0],
            "frame   0.42 ms cpu (ui 0.30, gl 0.12), gpu 0.25 ms"
        );
        assert_eq!(lines[1], "recent  p50 0.40 p95 0.50 max 0.50 ms (2 frames)");
        assert_eq!(lines[2], "draws   5 (432 markers)");
        assert_eq!(lines[3], "frames  37");
        assert_eq!(lines[4], "size    1280 x 720 px");
        assert_eq!(lines[5], "camera  yaw 60 pitch 30 dist 15.0");
        assert_eq!(lines[6], "target  0.0, 0.0, 0.0");
    }

    #[test]
    fn the_controls_say_they_are_unobserved() {
        let t = controls_text();
        for (action, binding) in BINDINGS {
            assert!(t.contains(action) && t.contains(binding));
        }
        assert!(t.contains("Not yet observed against the CK"));
    }

    #[test]
    fn the_picker_reads_a_cell_or_a_world_and_grid() {
        assert_eq!(
            parse_pick(" CityNewAtlantisApartmentTEMPLATE "),
            Ok(CellPick::Cell("CityNewAtlantisApartmentTEMPLATE".into()))
        );
        assert_eq!(
            parse_pick("Starfield.esm:21966F"),
            Ok(CellPick::Cell("Starfield.esm:21966F".into()))
        );
        for (text, x, y) in [
            ("NewAtlantis 2,1", 2, 1),
            ("NewAtlantis 2 1", 2, 1),
            ("NewAtlantis (-1, 0)", -1, 0),
            ("NewAtlantis  -3,-4 ", -3, -4),
        ] {
            assert_eq!(
                parse_pick(text),
                Ok(CellPick::Grid {
                    world: "NewAtlantis".into(),
                    x,
                    y
                }),
                "{text}"
            );
        }
        for bad in [
            "",
            "   ",
            "NewAtlantis 2",
            "NewAtlantis 2,1,3",
            "NewAtlantis a,b",
            "W 1.5,2",
        ] {
            assert!(parse_pick(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_grid("(7,-8)"), Ok((7, -8)));
    }

    #[test]
    fn requests_ask_for_bounds_a_page_at_a_time() {
        let r = cell_request(&CellPick::Cell("X".into()), None);
        assert_eq!(r.id, "cell");
        assert_eq!(
            r.args,
            json!({"cell": "X", "with_bounds": true, "limit": PAGE})
        );
        let g = cell_request(
            &CellPick::Grid {
                world: "W".into(),
                x: 2,
                y: -1,
            },
            Some("c1"),
        );
        assert_eq!(
            g.args,
            json!({"world": "W", "x": 2, "y": -1, "with_bounds": true, "limit": PAGE, "cursor": "c1"})
        );
        // The window may run it: `cell` is read-only and allowed.
        assert!(crate::gate::ALLOWLIST.iter().any(|a| a.id == "cell"));
    }

    /// A canned page of two references in a cell of three (paged).
    fn page_env(items: &[Value], next: Option<&str>) -> Value {
        json!({
            "command": "cell",
            "data": {
                "mode": "cell", "cell": "Test.esm:000900", "cell_editor_id": "TestRoom",
                "world": null, "total": 3, "next_cursor": next, "summary": "",
                "items": items,
            },
            "limitations": ["a limitation"],
        })
    }

    fn item(form: &str, at: [f32; 3], sig: &str) -> Value {
        json!({
            "form": form, "signature": "REFR", "editor_id": format!("{form}Ref"),
            "base": {"kind": "formref", "form": "Test.esm:000800"}, "base_signature": sig,
            "position": at, "rotation": [0.0, 0.0, 0.0], "scale": null,
            "bounds": {"min": [-1.0, -1.0, 0.0], "max": [1.0, 1.0, 2.0]},
            "persistent": false, "plugin": "Test.esm",
        })
    }

    /// One frame of the tab in an 1000 x 700 window (no GL: the paint callback is
    /// recorded, never run); returns the actions the tab asked for.
    fn run(ctx: &egui::Context, tab: &mut RenderTab, events: Vec<egui::Event>) -> Vec<Action> {
        let results = std::collections::BTreeMap::new();
        let mut notice = None;
        let mut actions = Vec::new();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1000.0, 700.0),
            )),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let mut tc = TabCtx {
                busy: false,
                tour: false,
                results: &results,
                plugins: &[],
                notice: &mut notice,
                actions: Vec::new(),
                theme: theme::ThemeChoice::default(),
                theme_source: "test",
                last_profile: None,
                prefs_path: None,
            };
            tab.ui(ui, &mut tc);
            actions = tc.actions;
        });
        out.textures_delta.clear();
        actions
    }

    /// The shell model of the shared selection: pages are fetched until the cell is
    /// complete, the camera frames it, a click on a marker selects its reference and asks
    /// the Records inspector for it, and a record inspected there selects its marker.
    #[test]
    fn pages_markers_and_the_shared_selection() {
        let ctx = egui::Context::default();
        let mut tab = RenderTab {
            pick_text: "TestRoom".into(),
            ..RenderTab::default()
        };
        let first = tab.start(parse_pick(&tab.pick_text).unwrap());
        assert_eq!(first.args["cell"], "TestRoom");
        let next = tab
            .receive_page(cell::parse_page(&page_env(
                &[
                    item("Test.esm:000901", [0.0, 0.0, 0.0], "STAT"),
                    item("Test.esm:000902", [40.0, 0.0, 0.0], "CONT"),
                ],
                Some("cur"),
            )))
            .expect("a second page");
        assert_eq!(next.args["cell"], "Test.esm:000900");
        assert_eq!(next.args["cursor"], "cur");
        assert!(tab.loading);
        assert!(tab
            .receive_page(cell::parse_page(&page_env(
                &[item("Test.esm:000903", [80.0, 0.0, 0.0], "LIGH")],
                None
            )))
            .is_none());
        assert!(!tab.loading);
        // The first frame builds the markers and frames the cell.
        run(&ctx, &mut tab, Vec::new());
        assert_eq!(tab.viewport.markers().len(), 3);
        assert_eq!(tab.viewport.camera.target.x, 40.0);

        // A record inspected in Records that is a reference of the cell is selected here.
        tab.inspected = Some("Test.esm:000903".into());
        run(&ctx, &mut tab, Vec::new());
        assert_eq!(tab.viewport.selected(), Some(2));
        let lines = selection_lines(tab.ref_of(2).unwrap());
        assert!(lines[0].contains("Test.esm:000903Ref"));
        assert!(lines[1].contains("Test.esm:000903 REFR"));
        assert!(lines[2].contains("Test.esm:000800 LIGH"));

        // Clicking the middle marker (the camera's target) selects it and asks Records.
        let mid = tab.viewport.rect().unwrap().center();
        run(&ctx, &mut tab, vec![egui::Event::PointerMoved(mid)]);
        let press = |pressed| egui::Event::PointerButton {
            pos: mid,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        run(&ctx, &mut tab, vec![press(true)]);
        let actions = run(&ctx, &mut tab, vec![press(false)]);
        assert_eq!(tab.viewport.selected(), Some(1));
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, Action::Select(f) if f == "Test.esm:000902")),
            "the click selects in Records too"
        );
        // Records then shows it: nothing changes here (no loop).
        tab.inspected = Some("Test.esm:000902".into());
        assert!(run(&ctx, &mut tab, Vec::new()).is_empty());
        assert_eq!(tab.viewport.selected(), Some(1));
        // A record that is not in the cell leaves the selection alone.
        tab.inspected = Some("Other.esm:000001".into());
        run(&ctx, &mut tab, Vec::new());
        assert_eq!(tab.viewport.selected(), Some(1));
    }

    #[test]
    fn the_cap_stops_paging_and_limits_markers() {
        let mut tab = RenderTab {
            cap: 1,
            ..RenderTab::default()
        };
        tab.start(CellPick::Cell("TestRoom".into()));
        let env = page_env(
            &[
                item("Test.esm:000901", [0.0, 0.0, 0.0], "STAT"),
                item("Test.esm:000902", [4.0, 0.0, 0.0], "STAT"),
            ],
            Some("cur"),
        );
        assert!(
            tab.receive_page(cell::parse_page(&env)).is_none(),
            "the cap is reached"
        );
        let pal = *crate::theme::tokens::workbench().palette(true);
        tab.rebuild(&pal, true);
        assert_eq!(tab.viewport.markers().len(), 1);
    }

    #[test]
    fn tiling_copies_markers_side_by_side() {
        let m = Marker::placeholder();
        let t = tile(&[m, m], 5);
        assert_eq!(t.len(), 5);
        assert!(t.iter().any(|x| x.position.x > m.position.x + 1.0));
        assert!(tile(&[], 3).is_empty());
    }
}
