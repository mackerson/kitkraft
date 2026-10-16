//! The 3D viewport (M7 lane R; ADR 0002 Amendment 1: L5 view code). It draws through
//! eframe's glow backend: an egui `PaintCallback` holding an `egui_glow::CallbackFn`,
//! inside the workbench's own window and GL context.
//!
//! - [`camera`]: the orbit camera, pure math (unit-tested without a window).
//! - [`scene`]: what is drawn, as data: grid, axes, the marker shapes, [`scene::Marker`]s
//!   (placement and picking), colors.
//! - [`cell`]: a `cell` envelope's references as markers, by base record type (M7.R2,
//!   M7.R3).
//! - [`shaders`]: the GLSL, with a test that parses its declarations.
//! - `gl`: the GL calls (the one file of the workspace allowed `unsafe`, ADR 0013).
//! - [`Viewport`]: the widget: mouse and keys to camera, a click to pick, one paint
//!   callback per frame.
//!
//! Game data reaches it only as markers the Render Window tab builds from the engine's
//! `cell` command (as every tab gets its data, from an envelope), never by reading a
//! plugin here.
//!
//! Frame budget: the viewport draws only when egui repaints, which it does on input. It
//! asks for more frames only while a fly key is held, or once after
//! [`Viewport::request_repaint`]; it never animates on its own. Markers are uploaded to
//! the GPU once per change ([`Viewport::set_markers`]), not per frame.

pub mod camera;
pub mod cell;
mod gl;
pub mod scene;
pub mod shaders;

use std::ops::{Mul, Sub};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, PointerButton, Sense};
use glam::{Mat4, Vec2, Vec3};

use camera::OrbitCamera;
use scene::{Marker, SceneColors, Shape, INSTANCE_FLOATS};

pub use gl::DrawStats;

/// How marker faces are drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FaceMode {
    /// Opaque faces with dark outlines.
    Solid,
    /// Faint faces over outlines in each marker's color: what is inside a box shows.
    #[default]
    SeeThrough,
    /// Outlines only.
    Wire,
}

impl FaceMode {
    pub const ALL: [FaceMode; 3] = [FaceMode::Solid, FaceMode::SeeThrough, FaceMode::Wire];

    pub fn label(self) -> &'static str {
        match self {
            FaceMode::Solid => "solid",
            FaceMode::SeeThrough => "see-through",
            FaceMode::Wire => "wire",
        }
    }

    /// Face opacity and how far outlines take the outline color over the marker's.
    fn alpha_and_mix(self) -> (f32, f32) {
        match self {
            FaceMode::Solid => (1.0, 1.0),
            FaceMode::SeeThrough => (0.22, 0.0),
            FaceMode::Wire => (0.0, 0.0),
        }
    }
}

/// The markers as uploaded: instance floats per shape ([`Shape::index`]), with the
/// generation that tells the GL side when to upload again.
#[derive(Debug, Default)]
pub(crate) struct Instances {
    pub generation: u64,
    pub per_shape: [Vec<f32>; 3],
}

impl Instances {
    fn of(markers: &[Marker], generation: u64) -> Self {
        let mut per_shape: [Vec<f32>; 3] = Default::default();
        for m in markers {
            if let Some(v) = per_shape.get_mut(m.shape.index()) {
                v.extend_from_slice(&m.instance());
            }
        }
        Self {
            generation,
            per_shape,
        }
    }
}

/// The controls, as shown in the tab and in `docs/ck-parity.md`. None of them has been
/// observed against the CK's Render Window yet (M7.O2, sitting 1).
pub const BINDINGS: &[(&str, &str)] = &[
    ("orbit", "right-drag"),
    ("pan", "middle-drag"),
    ("zoom", "mouse wheel"),
    (
        "fly",
        "W A S D while the viewport has focus (click it); Shift for faster",
    ),
];

/// What one frame of the viewport needs on the GL side, computed before painting.
pub(crate) struct Frame {
    pub view_proj: [f32; 16],
    pub inv_view_proj: [f32; 16],
    pub eye: [f32; 3],
    /// Unit direction toward the light (from over the viewer's shoulder).
    pub light: [f32; 3],
    /// Distances from the eye where the grid starts and finishes fading out.
    pub grid_fade: [f32; 2],
    pub colors: SceneColors,
    /// The grid's model matrix (scaled to the camera, under its target) and the axes'.
    pub grid_model: [f32; 16],
    pub axes_model: [f32; 16],
    /// Every marker, as instance data (shared, uploaded when its generation changes).
    pub markers: Arc<Instances>,
    /// The selected marker's shape and instance data.
    pub selected: Option<(Shape, [f32; INSTANCE_FLOATS])>,
    /// Face opacity and outline color mix ([`FaceMode`]).
    pub face_alpha: f32,
    pub edge_mix: f32,
    /// CPU time spent preparing this frame before painting (input, camera, matrices).
    pub prep: Duration,
}

impl Frame {
    fn new(
        cam: &OrbitCamera,
        aspect: f32,
        colors: SceneColors,
        markers: Arc<Instances>,
        floor: f32,
    ) -> Self {
        let vp = cam.view_proj(aspect);
        let light = cam
            .up()
            .mul(0.8)
            .sub(cam.forward().mul(0.5))
            .sub(cam.right().mul(0.3))
            .normalize_or(Vec3::Z);
        // Fade the grid out from a few orbit distances away, so its far lines do not
        // alias toward the horizon.
        let start = (cam.distance * 3.0).max(1.0);
        let step = scene::grid_step_for(cam.distance);
        let (face_alpha, edge_mix) = FaceMode::default().alpha_and_mix();
        Self {
            view_proj: vp.to_cols_array(),
            inv_view_proj: vp.inverse().to_cols_array(),
            eye: cam.eye().to_array(),
            light: light.to_array(),
            grid_fade: [start, start * 2.5],
            colors,
            grid_model: scene::grid_model(step, cam.target, floor).to_cols_array(),
            axes_model: Mat4::from_scale(Vec3::splat(step)).to_cols_array(),
            markers,
            selected: None,
            face_alpha,
            edge_mix,
            prep: Duration::ZERO,
        }
    }
}

/// A point of the viewport `rect` in normalized device coordinates (x right, y up).
pub fn to_ndc(rect: egui::Rect, p: egui::Pos2) -> Vec2 {
    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);
    Vec2::new(
        (p.x - rect.min.x) / w * 2.0 - 1.0,
        1.0 - (p.y - rect.min.y) / h * 2.0,
    )
}

/// The input of one frame that moves the camera, gathered from egui (toolkit-free, so
/// the mapping is tested).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ViewInput {
    /// Right-drag this frame, in points.
    pub orbit: Vec2,
    /// Middle-drag this frame, in points.
    pub pan: Vec2,
    /// Wheel, in points (positive: wheel up, zoom in).
    pub scroll: f32,
    /// W minus S, and D minus A (each -1, 0 or 1); zero unless the viewport has focus.
    pub ahead: f32,
    pub side: f32,
    pub fast: bool,
    /// Seconds since the last frame.
    pub dt: f32,
    /// The viewport's height in points.
    pub height: f32,
}

impl ViewInput {
    /// A fly key is held: the next frame must come without waiting for an event.
    pub fn keeps_moving(&self) -> bool {
        self.ahead != 0.0 || self.side != 0.0
    }
}

/// Applies one frame's input to the camera; `true` when it moved.
pub fn apply(cam: &mut OrbitCamera, input: &ViewInput) -> bool {
    let before = *cam;
    if input.orbit != Vec2::ZERO {
        cam.orbit(input.orbit);
    }
    if input.pan != Vec2::ZERO {
        cam.pan(input.pan, input.height);
    }
    if input.scroll != 0.0 {
        cam.zoom(input.scroll);
    }
    if input.keeps_moving() {
        cam.fly(input.ahead, input.side, input.dt, input.fast);
    }
    *cam != before
}

/// -1, 0 or 1 from a pair of keys.
fn axis(plus: bool, minus: bool) -> f32 {
    match (plus, minus) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}

/// The viewport widget's state: camera, markers and the selection, the repaint request
/// and the GL side.
pub struct Viewport {
    pub camera: OrbitCamera,
    /// What is drawn besides the grid and the axes ([`Viewport::set_markers`]).
    markers: Vec<Marker>,
    instances: Arc<Instances>,
    /// The selected marker (an index into the markers).
    selected: Option<usize>,
    /// What the last click picked: `Some(None)` for empty space; taken by the tab.
    picked: Option<Option<usize>>,
    pub faces: FaceMode,
    /// The grid's height: the lowest marker's bottom (0 for the placeholder).
    floor: f32,
    /// The viewport's last width over height, and where it was laid out.
    aspect: f32,
    rect: Option<egui::Rect>,
    repaint: bool,
    gl: Arc<Mutex<gl::GlShared>>,
    /// CPU time of the last [`Viewport::show`] (input, camera, frame preparation).
    last_ui: Duration,
}

impl Default for Viewport {
    fn default() -> Self {
        let markers = vec![Marker::placeholder()];
        Self {
            camera: OrbitCamera::default(),
            instances: Arc::new(Instances::of(&markers, 0)),
            markers,
            selected: None,
            picked: None,
            faces: FaceMode::default(),
            floor: 0.0,
            aspect: 16.0 / 9.0,
            rect: None,
            repaint: false,
            gl: Arc::default(),
            last_ui: Duration::ZERO,
        }
    }
}

/// What the stats overlay shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    pub draw: DrawStats,
    /// CPU time of the last frame's viewport preparation.
    pub ui: Duration,
    /// The viewport's CPU time per frame over the recent frames.
    pub times: FrameTimes,
    /// Why the GL scene could not be built.
    pub error: Option<String>,
}

/// Median, 95th percentile and maximum of a set of frame times (nearest rank).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameTimes {
    pub samples: usize,
    pub p50: Duration,
    pub p95: Duration,
    pub max: Duration,
}

/// [`FrameTimes`] of `samples` (any order); all zero when there are none.
pub fn frame_times(samples: impl IntoIterator<Item = Duration>) -> FrameTimes {
    let mut sorted: Vec<Duration> = samples.into_iter().collect();
    sorted.sort_unstable();
    let n = sorted.len();
    // Nearest rank: the smallest value with at least p of the samples at or below it.
    let rank = |p: f64| {
        let r = (p * n as f64).ceil() as usize;
        sorted
            .get(r.saturating_sub(1).min(n.saturating_sub(1)))
            .copied()
            .unwrap_or_default()
    };
    FrameTimes {
        samples: n,
        p50: rank(0.50),
        p95: rank(0.95),
        max: sorted.last().copied().unwrap_or_default(),
    }
}

impl Viewport {
    /// Asks for one more frame (for data that changed outside an input event, such as a
    /// reply from the engine). Cleared when that frame is drawn.
    pub fn request_repaint(&mut self) {
        self.repaint = true;
    }

    /// Back to the default view of the origin.
    pub fn reset_camera(&mut self) {
        self.camera = OrbitCamera::default();
    }

    /// Replaces the markers (uploaded once, on the next frame), clears the selection and
    /// puts the grid under the lowest one. An empty list shows the placeholder again.
    pub fn set_markers(&mut self, markers: Vec<Marker>) {
        self.markers = if markers.is_empty() {
            vec![Marker::placeholder()]
        } else {
            markers
        };
        let generation = self.instances.generation.wrapping_add(1);
        self.instances = Arc::new(Instances::of(&self.markers, generation));
        self.selected = None;
        self.floor = scene::bounds_of(&self.markers).map_or(0.0, |(lo, _)| lo.z);
        self.request_repaint();
    }

    /// The markers drawn.
    /// Where the viewport was last laid out.
    pub fn rect(&self) -> Option<egui::Rect> {
        self.rect
    }

    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    /// Frames every marker.
    pub fn frame_all(&mut self) {
        if let Some((lo, hi)) = scene::bounds_of(&self.markers) {
            self.camera.frame_box(lo, hi, self.aspect);
            self.request_repaint();
        }
    }

    /// Frames the selected marker, if any.
    pub fn frame_selected(&mut self) {
        let one = self.selected.and_then(|i| self.markers.get(i)).copied();
        if let Some((lo, hi)) = one.and_then(|m| scene::bounds_of(&[m])) {
            self.camera.frame_box(lo, hi, self.aspect);
            self.request_repaint();
        }
    }

    /// The selected marker's index.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Selects a marker (or none) from outside the viewport (the Records inspector).
    pub fn select(&mut self, index: Option<usize>) {
        let index = index.filter(|i| *i < self.markers.len());
        if index != self.selected {
            self.selected = index;
            self.request_repaint();
        }
    }

    /// What the last click in the viewport picked, once: `Some(Some(i))` a marker,
    /// `Some(None)` empty space, `None` no click.
    pub fn take_pick(&mut self) -> Option<Option<usize>> {
        self.picked.take()
    }

    /// The last frame's numbers, for the overlay.
    pub fn stats(&self) -> Stats {
        let (draw, times, error) = match self.gl.lock() {
            Ok(g) => (
                g.stats,
                frame_times(g.history.iter().copied()),
                g.error.clone(),
            ),
            Err(_) => (
                DrawStats::default(),
                FrameTimes::default(),
                Some("viewport state poisoned".into()),
            ),
        };
        Stats {
            draw,
            ui: self.last_ui,
            times,
            error,
        }
    }

    /// Draws the viewport into all the space `ui` has left and handles its input.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let start = Instant::now();
        let size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        if response.is_pointer_button_down_on() && !response.has_focus() {
            response.request_focus();
        }
        let focused = response.has_focus();
        if focused {
            // Arrow keys and Escape stay with the viewport instead of moving focus.
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        tab: false,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: false,
                    },
                );
            });
        }
        // Read the response before `ui.input`: its methods lock the context themselves.
        let drag = Vec2::new(response.drag_delta().x, response.drag_delta().y);
        let orbiting = response.dragged_by(PointerButton::Secondary);
        let panning = response.dragged_by(PointerButton::Middle);
        let hovered = response.hovered();
        let input = ui.input(|i| ViewInput {
            orbit: if orbiting { drag } else { Vec2::ZERO },
            pan: if panning { drag } else { Vec2::ZERO },
            scroll: if hovered {
                i.smooth_scroll_delta.y
            } else {
                0.0
            },
            ahead: if focused {
                axis(i.key_down(Key::W), i.key_down(Key::S))
            } else {
                0.0
            },
            side: if focused {
                axis(i.key_down(Key::D), i.key_down(Key::A))
            } else {
                0.0
            },
            fast: i.modifiers.shift,
            dt: i.stable_dt.clamp(0.0, 0.1),
            height: rect.height(),
        });
        if input.scroll != 0.0 {
            // The wheel was the viewport's: nothing else scrolls with it.
            ui.input_mut(|i| i.smooth_scroll_delta.y = 0.0);
        }
        apply(&mut self.camera, &input);
        if input.keeps_moving() || std::mem::take(&mut self.repaint) {
            ui.ctx().request_repaint();
        }
        if orbiting || panning {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        let aspect = if rect.height() > 0.0 {
            rect.width() / rect.height()
        } else {
            1.0
        };
        self.aspect = aspect;
        self.rect = Some(rect);
        // A left click (not a drag) picks the smallest marker box under the cursor.
        if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let (origin, dir) = self.camera.ray(aspect, to_ndc(rect, p));
                let hit = scene::pick(&self.markers, origin, dir);
                self.selected = hit;
                self.picked = Some(hit);
                ui.ctx().request_repaint();
            }
        }
        let mut frame = Frame::new(
            &self.camera,
            aspect,
            SceneColors::for_mode(ui.visuals().dark_mode),
            Arc::clone(&self.instances),
            self.floor,
        );
        (frame.face_alpha, frame.edge_mix) = self.faces.alpha_and_mix();
        frame.selected = self
            .selected
            .and_then(|i| self.markers.get(i))
            .map(|m| (m.shape, m.instance()));
        frame.prep = start.elapsed();
        ui.painter().add(egui::PaintCallback {
            rect,
            callback: Arc::new(gl::callback(Arc::clone(&self.gl), frame)),
        });
        if focused {
            let stroke = ui.visuals().selection.stroke;
            ui.painter()
                .rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
        }
        self.last_ui = start.elapsed();
        response
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn no_input_moves_nothing_and_asks_for_no_frame() {
        let mut cam = OrbitCamera::default();
        let idle = ViewInput {
            dt: 1.0 / 60.0,
            height: 600.0,
            ..ViewInput::default()
        };
        assert!(!apply(&mut cam, &idle));
        assert!(!idle.keeps_moving());
        assert_eq!(cam, OrbitCamera::default());
    }

    #[test]
    fn each_binding_moves_the_camera_its_own_way() {
        let base = ViewInput {
            dt: 1.0 / 60.0,
            height: 600.0,
            ..ViewInput::default()
        };
        let cam0 = OrbitCamera::default();

        let mut cam = cam0;
        assert!(apply(
            &mut cam,
            &ViewInput {
                orbit: Vec2::new(5.0, 0.0),
                ..base
            }
        ));
        assert_eq!((cam.target, cam.distance), (cam0.target, cam0.distance));
        assert_ne!(cam.yaw, cam0.yaw);

        let mut cam = cam0;
        assert!(apply(
            &mut cam,
            &ViewInput {
                pan: Vec2::new(5.0, 5.0),
                ..base
            }
        ));
        assert_eq!(
            (cam.yaw, cam.pitch, cam.distance),
            (cam0.yaw, cam0.pitch, cam0.distance)
        );
        assert_ne!(cam.target, cam0.target);

        let mut cam = cam0;
        assert!(apply(
            &mut cam,
            &ViewInput {
                scroll: 50.0,
                ..base
            }
        ));
        assert!(cam.distance < cam0.distance);
        assert_eq!(cam.target, cam0.target);

        let mut cam = cam0;
        let fly = ViewInput {
            ahead: axis(true, false),
            side: axis(false, false),
            ..base
        };
        assert!(fly.keeps_moving());
        assert!(apply(&mut cam, &fly));
        assert!((cam.target - cam0.target).dot(cam0.forward()) > 0.0);
        assert_eq!(axis(true, true), 0.0);
        assert_eq!(axis(false, true), -1.0);
    }

    #[test]
    fn the_frame_carries_finite_matrices_for_every_marker() {
        let cam = OrbitCamera::default();
        let markers = [
            Marker::placeholder(),
            Marker {
                shape: Shape::Pyramid,
                ..Marker::placeholder()
            },
        ];
        let f = Frame::new(
            &cam,
            1.5,
            SceneColors::DARK,
            Arc::new(Instances::of(&markers, 7)),
            0.0,
        );
        assert_eq!(f.markers.generation, 7);
        assert_eq!(f.markers.per_shape[0].len(), INSTANCE_FLOATS);
        assert!(f.markers.per_shape[1].is_empty());
        assert_eq!(f.markers.per_shape[2].len(), INSTANCE_FLOATS);
        assert!(f
            .view_proj
            .iter()
            .chain(&f.inv_view_proj)
            .chain(&f.grid_model)
            .chain(&f.axes_model)
            .all(|v| v.is_finite()));
        assert!(f.grid_fade[0] < f.grid_fade[1]);
        assert!((Vec3::from(f.light).length() - 1.0).abs() < 1e-5);
        // The light comes from above, toward the viewer.
        assert!(f.light[2] > 0.0 && Vec3::from(f.light).dot(cam.forward()) < 0.0);
    }

    /// One egui frame of an 800 x 600 window holding only the viewport (no GL: the paint
    /// callback is recorded, never run). Returns how soon egui was asked to repaint.
    fn frame(ctx: &egui::Context, vp: &mut Viewport, events: Vec<egui::Event>) -> Duration {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(800.0, 600.0),
            )),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            vp.show(ui);
        });
        out.textures_delta.clear();
        out.viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(Duration::MAX, |v| v.repaint_delay)
    }

    fn at(x: f32, y: f32) -> egui::Pos2 {
        egui::Pos2::new(x, y)
    }

    fn button(pos: egui::Pos2, button: PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn key(key: Key, pressed: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Drags with `which` from the middle of the viewport, 120 points right and 40 down.
    fn drag(ctx: &egui::Context, vp: &mut Viewport, which: PointerButton) {
        frame(ctx, vp, vec![egui::Event::PointerMoved(at(400.0, 300.0))]);
        frame(ctx, vp, vec![button(at(400.0, 300.0), which, true)]);
        for i in 1..=6u8 {
            let p = at(
                400.0 + 20.0 * f32::from(i),
                300.0 + 20.0 * f32::from(i) / 3.0,
            );
            frame(ctx, vp, vec![egui::Event::PointerMoved(p)]);
        }
        frame(ctx, vp, vec![button(at(520.0, 340.0), which, false)]);
    }

    /// The bindings through egui's real input handling: right-drag orbits, middle-drag
    /// pans, the wheel zooms, W flies only once the viewport has focus, and an idle
    /// viewport asks for no frame while a held fly key does.
    #[test]
    fn mouse_and_keys_drive_the_camera_through_egui() {
        let ctx = egui::Context::default();
        let start = OrbitCamera::default();

        let mut vp = Viewport::default();
        drag(&ctx, &mut vp, PointerButton::Secondary);
        assert_ne!(vp.camera.yaw, start.yaw, "right-drag orbits");
        assert!(
            vp.camera.pitch > start.pitch,
            "dragging down raises the eye"
        );
        assert_eq!(
            (vp.camera.target, vp.camera.distance),
            (start.target, start.distance)
        );

        let mut vp = Viewport::default();
        drag(&ctx, &mut vp, PointerButton::Middle);
        assert_ne!(vp.camera.target, start.target, "middle-drag pans");
        assert_eq!((vp.camera.yaw, vp.camera.pitch), (start.yaw, start.pitch));

        let mut vp = Viewport::default();
        frame(
            &ctx,
            &mut vp,
            vec![egui::Event::PointerMoved(at(400.0, 300.0))],
        );
        frame(
            &ctx,
            &mut vp,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::Vec2::new(0.0, 120.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        for _ in 0..30 {
            frame(&ctx, &mut vp, Vec::new());
        }
        assert!(vp.camera.distance < start.distance, "wheel up zooms in");
        assert_eq!(vp.camera.target, start.target);

        // W without focus does nothing; after a click it flies, and keeps asking for
        // frames while held.
        let ctx = egui::Context::default();
        let mut vp = Viewport::default();
        frame(&ctx, &mut vp, vec![key(Key::W, true)]);
        frame(&ctx, &mut vp, vec![key(Key::W, false)]);
        assert_eq!(vp.camera, start, "no focus, no fly");
        frame(
            &ctx,
            &mut vp,
            vec![egui::Event::PointerMoved(at(400.0, 300.0))],
        );
        frame(
            &ctx,
            &mut vp,
            vec![button(at(400.0, 300.0), PointerButton::Primary, true)],
        );
        frame(
            &ctx,
            &mut vp,
            vec![button(at(400.0, 300.0), PointerButton::Primary, false)],
        );
        let before = vp.camera.target;
        frame(&ctx, &mut vp, vec![key(Key::W, true)]);
        let held = frame(&ctx, &mut vp, Vec::new());
        assert!(
            (vp.camera.target - before).dot(start.forward()) > 0.0,
            "W flies"
        );
        assert_eq!(
            held,
            Duration::ZERO,
            "a held fly key asks for the next frame"
        );
        frame(&ctx, &mut vp, vec![key(Key::W, false)]);
        let mut idle = Duration::ZERO;
        for _ in 0..5 {
            idle = frame(&ctx, &mut vp, Vec::new());
        }
        assert_eq!(idle, Duration::MAX, "an idle viewport asks for no frame");
        // The request flag asks once (egui then paints at most one extra frame of its
        // own) and the viewport is idle again.
        vp.request_repaint();
        assert_eq!(frame(&ctx, &mut vp, Vec::new()), Duration::ZERO);
        let settled = (0..3).map(|_| frame(&ctx, &mut vp, Vec::new())).last();
        assert_eq!(settled, Some(Duration::MAX));
    }

    /// A click through egui's real input handling picks the marker under the cursor;
    /// a click on empty space picks nothing; framing brings every marker into view.
    #[test]
    fn a_click_picks_the_marker_under_the_cursor() {
        let ctx = egui::Context::default();
        let mut vp = Viewport::default();
        let far = Marker {
            position: Vec3::new(250.0, 150.0, 200.0),
            ..Marker::placeholder()
        };
        vp.set_markers(vec![Marker::placeholder(), far]);
        assert_eq!(vp.markers().len(), 2);
        let click = |ctx: &egui::Context, vp: &mut Viewport, p: egui::Pos2| {
            frame(ctx, vp, vec![egui::Event::PointerMoved(p)]);
            frame(ctx, vp, vec![button(p, PointerButton::Primary, true)]);
            frame(ctx, vp, vec![button(p, PointerButton::Primary, false)]);
            vp.take_pick()
        };
        // The default camera looks at the origin, where the first marker stands.
        assert_eq!(click(&ctx, &mut vp, at(400.0, 300.0)), Some(Some(0)));
        assert_eq!(vp.selected(), Some(0));
        assert_eq!(vp.take_pick(), None, "taken once");
        assert_eq!(click(&ctx, &mut vp, at(5.0, 5.0)), Some(None));
        assert_eq!(vp.selected(), None);
        vp.select(Some(1));
        vp.frame_selected();
        assert_eq!(vp.camera.target, far.position);
        assert_eq!(click(&ctx, &mut vp, at(400.0, 300.0)), Some(Some(1)));
        vp.frame_all();
        for m in vp.markers().to_vec() {
            for c in m.corners() {
                let ndc = vp.camera.to_ndc(800.0 / 600.0, c).unwrap();
                assert!(ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0);
            }
        }
        vp.select(Some(9));
        assert_eq!(vp.selected(), None, "out of range");
        vp.set_markers(Vec::new());
        assert_eq!(vp.markers(), &[Marker::placeholder()]);
    }

    #[test]
    fn ndc_maps_the_rect_corners() {
        let r = egui::Rect::from_min_size(at(100.0, 50.0), egui::Vec2::new(200.0, 100.0));
        assert_eq!(to_ndc(r, at(100.0, 50.0)), Vec2::new(-1.0, 1.0));
        assert_eq!(to_ndc(r, at(300.0, 150.0)), Vec2::new(1.0, -1.0));
        assert_eq!(to_ndc(r, at(200.0, 100.0)), Vec2::ZERO);
    }

    #[test]
    fn frame_times_are_nearest_rank_percentiles() {
        assert_eq!(frame_times([]), FrameTimes::default());
        let ms = Duration::from_millis;
        let t = frame_times((1..=100).rev().map(ms));
        assert_eq!(
            t,
            FrameTimes {
                samples: 100,
                p50: ms(50),
                p95: ms(95),
                max: ms(100)
            }
        );
        let one = frame_times([ms(7)]);
        assert_eq!((one.p50, one.p95, one.max), (ms(7), ms(7), ms(7)));
    }

    #[test]
    fn the_bindings_name_all_four_controls() {
        let names: Vec<&str> = BINDINGS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["orbit", "pan", "zoom", "fly"]);
    }
}
