//! The viewport's OpenGL calls: the one file of the workspace that may use `unsafe`
//! (ADR 0013; `cargo xtask layers` holds the rest of the workspace to none).
//!
//! `glow` exposes every GL call as an `unsafe fn`, with no safe layer in the tree and
//! none allowed beside it (M7.R1: eframe's glow backend, no second GL crate). Each
//! `unsafe` block below calls `glow` with:
//!
//! - the context egui_glow paints with, current on this thread for the whole callback
//!   (egui_glow invokes [`paint`] from its paint pass);
//! - only objects this file created on that same context, never deleted while in use;
//! - buffer data from owned Rust slices whose length GL is told, and draw and instance
//!   counts taken from those lengths, so GL never reads past what was uploaded;
//! - no pointers, no mapped buffers, no raw handles from outside.
//!
//! Lifetime of the GL objects. Every object a [`GlScene`] makes is recorded as it is
//! made ([`GlObject`]), and deleted with that same context, never by `Drop` (which has
//! no context), at one of three points: everything made so far, when a later step of
//! [`GlScene::new`] fails; a scene whose tab was dropped (its [`GlShared`] moves it to a
//! retired list), at the next paint of any viewport; and every scene still alive or
//! retired at shutdown, in [`release`], which the app calls from `eframe::App::on_exit`
//! while the context is still current. So none outlives the window.
//!
//! Every state this file changes that egui_glow does not reset after a callback (depth
//! mask, polygon offset, the bound vertex array and program) is restored before
//! returning. Nothing here reads plugin data: markers arrive as finite instance data
//! (`super::scene::Marker::instance`), uploaded only when they change (M7.R2), and drawn
//! with one instanced call per shape for the faces and one for the outlines.
#![allow(unsafe_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use eframe::egui_glow::{self, ShaderVersion};
use eframe::glow::{self, HasContext};

use super::scene::{self, SceneColors, Shape, INSTANCE_FLOATS, LINE_LAYOUT};
use super::shaders::{self, ProgramSrc};
use super::Frame;

/// What the last draw cost, for the stats overlay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrawStats {
    /// GL draw calls in the last frame.
    pub draw_calls: u32,
    /// Marker instances drawn in the last frame.
    pub instances: u32,
    /// CPU time spent issuing them (not GPU time).
    pub submit: Duration,
    /// GPU time of the last measured frame (a timer query read one frame later; `None`
    /// on OpenGL ES or before a result is in).
    pub gpu: Option<Duration>,
    /// Frames drawn since the window opened.
    pub frames: u64,
    /// The viewport in physical pixels.
    pub size_px: [i32; 2],
}

/// Shared between the tab (which reads the stats and the error) and the paint callback
/// (which owns the GL objects).
#[derive(Default)]
pub struct GlShared {
    scene: Option<GlScene>,
    /// Why the scene could not be built (shown in the viewport); set once, not retried.
    pub error: Option<String>,
    pub stats: DrawStats,
    /// CPU time of the last [`HISTORY`] frames (preparation plus GL submission), oldest
    /// first, for the overlay's percentiles.
    pub history: VecDeque<Duration>,
}

/// Frames kept in [`GlShared::history`].
pub const HISTORY: usize = 240;

/// Scenes whose tab was dropped, waiting for a context to be deleted with (module docs,
/// "Lifetime").
static RETIRED: Mutex<Vec<GlScene>> = Mutex::new(Vec::new());
/// Every viewport that has painted, so [`release`] reaches the scenes still alive.
static LIVE: Mutex<Vec<Weak<Mutex<GlShared>>>> = Mutex::new(Vec::new());

impl Drop for GlShared {
    /// Hands the scene to [`RETIRED`]: deleting it needs the GL context, which `Drop`
    /// does not have.
    fn drop(&mut self) {
        if let Some(scene) = self.scene.take() {
            if let Ok(mut retired) = RETIRED.lock() {
                retired.push(scene);
            }
        }
    }
}

/// Deletes the scenes on [`RETIRED`] with `gl`.
fn delete_retired(gl: &glow::Context) {
    let retired = match RETIRED.lock() {
        Ok(mut r) => std::mem::take(&mut *r),
        Err(_) => return,
    };
    for scene in &retired {
        scene.destroy(gl);
    }
}

/// Deletes every GL object the viewports made, live or retired; the app calls it from
/// `eframe::App::on_exit` with the context they were made on. A viewport that painted
/// afterwards would build its scene again.
pub fn release(gl: &glow::Context) {
    delete_retired(gl);
    let live = match LIVE.lock() {
        Ok(mut l) => std::mem::take(&mut *l),
        Err(_) => return,
    };
    for shared in live.iter().filter_map(Weak::upgrade) {
        if let Ok(mut state) = shared.lock() {
            if let Some(scene) = state.scene.take() {
                scene.destroy(gl);
            }
        }
    }
}

/// One GL object a scene made, recorded so it can be deleted.
#[derive(Debug, Clone, Copy)]
enum GlObject {
    Program(glow::Program),
    Array(glow::VertexArray),
    Buffer(glow::Buffer),
    Query(glow::Query),
}

/// Deletes `objects` (made on `gl`, no longer drawn with).
fn delete_all(gl: &glow::Context, objects: &[GlObject]) {
    for o in objects {
        // SAFETY: see the module docs; each object was made on `gl` and is not used
        // after this.
        unsafe {
            match *o {
                GlObject::Program(p) => gl.delete_program(p),
                GlObject::Array(a) => gl.delete_vertex_array(a),
                GlObject::Buffer(b) => gl.delete_buffer(b),
                GlObject::Query(q) => gl.delete_query(q),
            }
        }
    }
}

/// The paint callback for one frame: builds the GL objects on first use, then draws
/// `frame`. It first deletes any retired scene.
pub fn callback(shared: Arc<Mutex<GlShared>>, frame: Frame) -> egui_glow::CallbackFn {
    if let Ok(mut live) = LIVE.lock() {
        live.retain(|w| w.strong_count() > 0);
        if !live.iter().any(|w| w.as_ptr() == Arc::as_ptr(&shared)) {
            live.push(Arc::downgrade(&shared));
        }
    }
    egui_glow::CallbackFn::new(move |info, painter| {
        delete_retired(painter.gl());
        let Ok(mut state) = shared.lock() else {
            return;
        };
        paint(&mut state, painter.gl(), &frame, info.viewport_in_pixels());
    })
}

fn paint(
    state: &mut GlShared,
    gl: &glow::Context,
    frame: &Frame,
    viewport: eframe::egui::epaint::ViewportInPixels,
) {
    if state.error.is_some() {
        return;
    }
    if state.scene.is_none() {
        match GlScene::new(gl, &frame.colors) {
            Ok(s) => state.scene = Some(s),
            Err(e) => {
                state.error = Some(e);
                return;
            }
        }
    }
    let Some(scene) = state.scene.as_mut() else {
        return;
    };
    let start = Instant::now();
    let (draw_calls, instances) = match scene.draw(gl, frame) {
        Ok(n) => n,
        Err(e) => {
            state.error = Some(e);
            return;
        }
    };
    let submit = start.elapsed();
    if state.history.len() >= HISTORY {
        state.history.pop_front();
    }
    state.history.push_back(frame.prep.saturating_add(submit));
    state.stats = DrawStats {
        draw_calls,
        instances,
        submit,
        gpu: scene.timer.as_ref().and_then(|t| t.last),
        frames: state.stats.frames.saturating_add(1),
        size_px: [viewport.width_px, viewport.height_px],
    };
}

/// A linked program and the locations of its uniforms.
struct Program {
    program: glow::Program,
    uniforms: Vec<(&'static str, Option<glow::UniformLocation>)>,
}

impl Program {
    /// Compiles and links `src`, recording the program in `made`; on a failure it deletes
    /// what it made itself.
    fn new(
        gl: &glow::Context,
        made: &mut Vec<GlObject>,
        src: &ProgramSrc,
        header: &str,
    ) -> Result<Self, String> {
        let p = Self::link(gl, src, header)?;
        made.push(GlObject::Program(p.program));
        Ok(p)
    }

    fn link(gl: &glow::Context, src: &ProgramSrc, header: &str) -> Result<Self, String> {
        // SAFETY: see the module docs; every object is created and used on `gl`.
        unsafe {
            let program = gl.create_program()?;
            let mut stages = Vec::new();
            for (kind, body, stage) in [
                (glow::VERTEX_SHADER, src.vertex, "vertex"),
                (glow::FRAGMENT_SHADER, src.fragment, "fragment"),
            ] {
                let shader = match gl.create_shader(kind) {
                    Ok(shader) => shader,
                    Err(e) => {
                        for s in stages {
                            gl.delete_shader(s);
                        }
                        gl.delete_program(program);
                        return Err(e);
                    }
                };
                gl.shader_source(shader, &format!("{header}{body}"));
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let log = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    for s in stages {
                        gl.delete_shader(s);
                    }
                    gl.delete_program(program);
                    return Err(format!("{} {stage} shader: {log}", src.name));
                }
                gl.attach_shader(program, shader);
                stages.push(shader);
            }
            for (location, name) in (0u32..).zip(src.attributes) {
                gl.bind_attrib_location(program, location, name);
            }
            gl.link_program(program);
            let linked = gl.get_program_link_status(program);
            for s in stages {
                gl.detach_shader(program, s);
                gl.delete_shader(s);
            }
            if !linked {
                let log = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(format!("{} program: {log}", src.name));
            }
            let uniforms = src
                .uniforms
                .iter()
                .map(|u| (*u, gl.get_uniform_location(program, u)))
                .collect();
            Ok(Self { program, uniforms })
        }
    }

    fn at(&self, name: &str) -> Option<&glow::UniformLocation> {
        self.uniforms
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, l)| l.as_ref())
    }
}

const TOO_LARGE: &str = "vertex layout too large";

/// Bytes per float.
const F32: i32 = 4;

/// Points the vertex attributes from `first` on at the buffer bound to `ARRAY_BUFFER`:
/// one attribute per entry of `layout` (its component count), interleaved; per instance
/// when `instanced`.
///
/// SAFETY (for the caller): a vertex array and the buffer are bound on `gl`.
unsafe fn point_attributes(
    gl: &glow::Context,
    first: u32,
    layout: &[i32],
    instanced: bool,
) -> Result<(), String> {
    let stride_floats = layout
        .iter()
        .try_fold(0i32, |acc, n| acc.checked_add(*n))
        .ok_or(TOO_LARGE)?;
    let stride = stride_floats.checked_mul(F32).ok_or(TOO_LARGE)?;
    let mut offset: i32 = 0;
    for (location, size) in (first..).zip(layout) {
        // SAFETY: see the function docs; the offsets stay inside `stride`.
        unsafe {
            gl.enable_vertex_attrib_array(location);
            gl.vertex_attrib_pointer_f32(location, *size, glow::FLOAT, false, stride, offset);
            if instanced {
                gl.vertex_attrib_divisor(location, 1);
            }
        }
        offset = offset
            .checked_add(size.checked_mul(F32).ok_or(TOO_LARGE)?)
            .ok_or(TOO_LARGE)?;
    }
    Ok(())
}

/// Floats as the bytes GL copies.
fn bytes(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

/// A vertex array with one interleaved float buffer.
struct Mesh {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    /// Floats per vertex.
    stride_floats: usize,
    /// Vertices uploaded.
    count: i32,
}

impl Mesh {
    /// `layout`: the component count of each attribute, in location order. Each object
    /// is recorded in `made` as it is made.
    fn new(
        gl: &glow::Context,
        made: &mut Vec<GlObject>,
        layout: &[i32],
        floats: &[f32],
    ) -> Result<Self, String> {
        let stride_floats = layout
            .iter()
            .try_fold(0usize, |acc, n| acc.checked_add(usize::try_from(*n).ok()?))
            .ok_or(TOO_LARGE)?;
        // SAFETY: see the module docs; the array and buffer are bound before pointing.
        let (vao, vbo) = unsafe {
            let vao = gl.create_vertex_array()?;
            made.push(GlObject::Array(vao));
            let vbo = gl.create_buffer()?;
            made.push(GlObject::Buffer(vbo));
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            let pointed = point_attributes(gl, 0, layout, false);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            pointed?;
            (vao, vbo)
        };
        let mut mesh = Self {
            vao,
            vbo,
            stride_floats,
            count: 0,
        };
        mesh.upload(gl, floats)?;
        Ok(mesh)
    }

    /// Replaces the buffer's contents; the vertex count follows the data.
    fn upload(&mut self, gl: &glow::Context, floats: &[f32]) -> Result<(), String> {
        let vertices = floats
            .len()
            .checked_div(self.stride_floats)
            .ok_or("empty vertex layout")?;
        self.count = i32::try_from(vertices).map_err(|_| "too many vertices".to_string())?;
        // SAFETY: see the module docs; the bytes are an owned slice and GL copies them.
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes(floats), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        Ok(())
    }

    /// One draw call over every vertex uploaded.
    fn draw(&self, gl: &glow::Context, mode: u32) {
        // SAFETY: see the module docs; `count` is the number of vertices uploaded.
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays(mode, 0, self.count);
        }
    }
}

/// One shape's geometry: its faces (position, normal) and its edges (position).
struct ShapeGeometry {
    faces: glow::Buffer,
    faces_count: i32,
    edges: glow::Buffer,
    edges_count: i32,
}

impl ShapeGeometry {
    fn new(gl: &glow::Context, made: &mut Vec<GlObject>, shape: Shape) -> Result<Self, String> {
        let faces = scene::solid_floats(&shape.faces());
        let edges: Vec<f32> = shape.edges().into_iter().flatten().collect();
        let count = |n: usize, per: usize| {
            n.checked_div(per)
                .and_then(|v| i32::try_from(v).ok())
                .ok_or_else(|| "shape too large".to_string())
        };
        // SAFETY: see the module docs; owned slices, copied by GL.
        unsafe {
            let fb = gl.create_buffer()?;
            made.push(GlObject::Buffer(fb));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(fb));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes(&faces), glow::STATIC_DRAW);
            let eb = gl.create_buffer()?;
            made.push(GlObject::Buffer(eb));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(eb));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes(&edges), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            Ok(Self {
                faces: fb,
                faces_count: count(faces.len(), scene::SOLID_FLOATS)?,
                edges: eb,
                edges_count: count(edges.len(), 3)?,
            })
        }
    }
}

/// The instance attributes after the vertex ones: the model matrix's four columns, then
/// the color (`scene::INSTANCE_FLOATS`).
const INSTANCE_LAYOUT: [i32; 5] = [4, 4, 4, 4, 4];

/// A set of marker instances of one shape: their buffer and a vertex array each for the
/// faces and the edges programs.
struct Instances {
    buffer: glow::Buffer,
    count: i32,
    faces: glow::VertexArray,
    edges: glow::VertexArray,
}

impl Instances {
    fn new(
        gl: &glow::Context,
        made: &mut Vec<GlObject>,
        geometry: &ShapeGeometry,
    ) -> Result<Self, String> {
        // SAFETY: see the module docs; each array is bound while its attributes are
        // pointed, and unbound after.
        unsafe {
            let buffer = gl.create_buffer()?;
            made.push(GlObject::Buffer(buffer));
            let faces = gl.create_vertex_array()?;
            made.push(GlObject::Array(faces));
            gl.bind_vertex_array(Some(faces));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(geometry.faces));
            let mut pointed = point_attributes(gl, 0, &scene::SOLID_LAYOUT, false);
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
            pointed = pointed.and_then(|()| point_attributes(gl, 2, &INSTANCE_LAYOUT, true));
            let edges = gl.create_vertex_array()?;
            made.push(GlObject::Array(edges));
            gl.bind_vertex_array(Some(edges));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(geometry.edges));
            pointed = pointed.and_then(|()| point_attributes(gl, 0, &[3], false));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
            pointed = pointed.and_then(|()| point_attributes(gl, 1, &INSTANCE_LAYOUT, true));
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            pointed?;
            Ok(Self {
                buffer,
                count: 0,
                faces,
                edges,
            })
        }
    }

    fn upload(&mut self, gl: &glow::Context, floats: &[f32]) -> Result<(), String> {
        let n = floats.len().checked_div(INSTANCE_FLOATS).unwrap_or(0);
        self.count = i32::try_from(n).map_err(|_| "too many markers".to_string())?;
        // SAFETY: see the module docs; the bytes are an owned slice and GL copies them.
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.buffer));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes(floats), glow::DYNAMIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        Ok(())
    }
}

/// One shape: its geometry, every marker of it, and the selected one (when of it).
struct ShapeGl {
    geometry: ShapeGeometry,
    all: Instances,
    selected: Instances,
}

/// GPU time of a frame through a `TIME_ELAPSED` query (desktop OpenGL 3.3 and later):
/// begun when no result is pending, read when available, never waited on.
struct GpuTimer {
    query: glow::Query,
    pending: bool,
    last: Option<Duration>,
}

/// Every GL object the viewport draws with.
struct GlScene {
    sky: Program,
    lines: Program,
    faces: Program,
    edges: Program,
    sky_mesh: Mesh,
    grid: Mesh,
    axes: Mesh,
    shapes: Vec<ShapeGl>,
    /// The generation of the markers uploaded, and of the selection.
    uploaded: Option<u64>,
    selected: Option<(Shape, [f32; INSTANCE_FLOATS])>,
    /// The colors the line meshes were built with (rebuilt when the theme changes).
    colors: SceneColors,
    timer: Option<GpuTimer>,
    /// Every object made, for [`GlScene::destroy`].
    objects: Vec<GlObject>,
}

impl GlScene {
    fn new(gl: &glow::Context, colors: &SceneColors) -> Result<Self, String> {
        let version = ShaderVersion::get(gl);
        if !version.is_new_shader_interface() {
            return Err(format!(
                "the viewport needs OpenGL 3 or OpenGL ES 3 shaders; this context offers {version:?}"
            ));
        }
        let v = gl.version();
        // Instanced attributes (`glVertexAttribDivisor`) are core in OpenGL 3.3 and ES 3.0.
        if !v.is_embedded && (v.major, v.minor) < (3, 3) {
            return Err(format!(
                "the viewport needs OpenGL 3.3 (instanced drawing); this context is {}.{}",
                v.major, v.minor
            ));
        }
        let header = shaders::header(version.version_declaration(), version.is_embedded());
        // Every object is recorded as it is made; on a failure all of them are deleted.
        let mut made = Vec::new();
        let built = Self::build(gl, colors, &header, !v.is_embedded, &mut made).and_then(|s| {
            // SAFETY: see the module docs.
            let error = unsafe { gl.get_error() };
            if error == glow::NO_ERROR {
                Ok(s)
            } else {
                Err(format!(
                    "OpenGL error 0x{error:04x} while building the scene"
                ))
            }
        });
        match built {
            Ok(mut scene) => {
                scene.objects = made;
                Ok(scene)
            }
            Err(e) => {
                delete_all(gl, &made);
                Err(e)
            }
        }
    }

    /// Makes the shapes, the timer, the programs and the meshes, recording each object
    /// in `made`.
    fn build(
        gl: &glow::Context,
        colors: &SceneColors,
        header: &str,
        timed: bool,
        made: &mut Vec<GlObject>,
    ) -> Result<Self, String> {
        let mut shapes = Vec::new();
        for shape in Shape::ALL {
            let geometry = ShapeGeometry::new(gl, made, shape)?;
            let all = Instances::new(gl, made, &geometry)?;
            let selected = Instances::new(gl, made, &geometry)?;
            shapes.push(ShapeGl {
                geometry,
                all,
                selected,
            });
        }
        let timer = if timed {
            // SAFETY: see the module docs.
            unsafe { gl.create_query() }.ok().map(|query| {
                made.push(GlObject::Query(query));
                GpuTimer {
                    query,
                    pending: false,
                    last: None,
                }
            })
        } else {
            None
        };
        Ok(Self {
            sky: Program::new(gl, made, &shaders::SKY, header)?,
            lines: Program::new(gl, made, &shaders::LINES, header)?,
            faces: Program::new(gl, made, &shaders::MARKER_FACES, header)?,
            edges: Program::new(gl, made, &shaders::MARKER_EDGES, header)?,
            // One triangle that covers the viewport.
            sky_mesh: Mesh::new(gl, made, &[2], &[-1.0, -1.0, 3.0, -1.0, -1.0, 3.0])?,
            grid: Mesh::new(
                gl,
                made,
                &LINE_LAYOUT,
                &scene::line_floats(&scene::grid_lines(colors)),
            )?,
            axes: Mesh::new(
                gl,
                made,
                &LINE_LAYOUT,
                &scene::line_floats(&scene::axis_lines(colors)),
            )?,
            shapes,
            uploaded: None,
            selected: None,
            colors: *colors,
            timer,
            objects: Vec::new(),
        })
    }

    /// Deletes every object the scene made; `self` is not drawn with afterwards.
    fn destroy(&self, gl: &glow::Context) {
        delete_all(gl, &self.objects);
    }

    /// Uploads what changed since the last frame: the line colors, the markers (by
    /// generation) and the selection.
    fn update(&mut self, gl: &glow::Context, frame: &Frame) -> Result<(), String> {
        if frame.colors != self.colors {
            self.grid
                .upload(gl, &scene::line_floats(&scene::grid_lines(&frame.colors)))?;
            self.axes
                .upload(gl, &scene::line_floats(&scene::axis_lines(&frame.colors)))?;
            self.colors = frame.colors;
        }
        if self.uploaded != Some(frame.markers.generation) {
            for (shape, gl_shape) in Shape::ALL.iter().zip(&mut self.shapes) {
                let floats = frame
                    .markers
                    .per_shape
                    .get(shape.index())
                    .map_or(&[][..], Vec::as_slice);
                gl_shape.all.upload(gl, floats)?;
            }
            self.uploaded = Some(frame.markers.generation);
        }
        if frame.selected != self.selected {
            for (shape, gl_shape) in Shape::ALL.iter().zip(&mut self.shapes) {
                let one = match &frame.selected {
                    Some((s, inst)) if s == shape => &inst[..],
                    _ => &[][..],
                };
                gl_shape.selected.upload(gl, one)?;
            }
            self.selected = frame.selected;
        }
        Ok(())
    }

    /// Draws one frame; returns the number of draw calls and of marker instances.
    fn draw(&mut self, gl: &glow::Context, frame: &Frame) -> Result<(u32, u32), String> {
        self.update(gl, frame)?;
        let c = &frame.colors;
        let mut calls: u32 = 0;
        let mut instances: u32 = 0;
        // SAFETY: see the module docs; uniforms are set on the program in use, with
        // slices of the length their GLSL type needs; instance counts are the uploaded
        // ones.
        unsafe {
            let timing = match &mut self.timer {
                Some(t) if t.pending => {
                    if gl.get_query_parameter_u32(t.query, glow::QUERY_RESULT_AVAILABLE) != 0 {
                        let ns = gl.get_query_parameter_u64(t.query, glow::QUERY_RESULT);
                        t.last = Some(Duration::from_nanos(ns));
                        t.pending = false;
                    }
                    false
                }
                Some(t) => {
                    gl.begin_query(glow::TIME_ELAPSED, t.query);
                    t.pending = true;
                    true
                }
                None => false,
            };

            // The depth buffer is shared with egui's whole window: clear it inside this
            // viewport only (egui_glow left the scissor at the clip rectangle).
            gl.depth_mask(true);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);

            // 1. The horizon tint, behind everything.
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::BLEND);
            gl.depth_mask(false);
            gl.use_program(Some(self.sky.program));
            gl.uniform_matrix_4_f32_slice(
                self.sky.at(shaders::U_INV_VIEW_PROJ),
                false,
                &frame.inv_view_proj,
            );
            gl.uniform_4_f32_slice(self.sky.at(shaders::U_ZENITH), &c.zenith);
            gl.uniform_4_f32_slice(self.sky.at(shaders::U_HORIZON), &c.horizon);
            gl.uniform_4_f32_slice(self.sky.at(shaders::U_GROUND), &c.ground);
            self.sky_mesh.draw(gl, glow::TRIANGLES);
            calls = calls.saturating_add(1);

            // 2. Opaque marker faces, depth-tested, back faces culled, pushed back a
            //    little so their outlines win the depth test.
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
            gl.front_face(glow::CCW);
            gl.enable(glow::POLYGON_OFFSET_FILL);
            gl.polygon_offset(1.0, 1.0);
            let solid = frame.face_alpha >= 1.0;
            if solid {
                gl.depth_mask(true);
                calls = calls.saturating_add(self.draw_faces(gl, frame, &mut instances));
            }
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.disable(glow::CULL_FACE);

            // 3. Lines, blended: the grid (fading out with distance, not writing depth),
            //    the axes.
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(self.lines.program));
            gl.uniform_matrix_4_f32_slice(
                self.lines.at(shaders::U_VIEW_PROJ),
                false,
                &frame.view_proj,
            );
            gl.uniform_matrix_4_f32_slice(
                self.lines.at(shaders::U_MODEL),
                false,
                &frame.grid_model,
            );
            gl.uniform_3_f32_slice(self.lines.at(shaders::U_EYE), &frame.eye);
            gl.uniform_2_f32_slice(self.lines.at(shaders::U_FADE), &frame.grid_fade);
            gl.depth_mask(false);
            self.grid.draw(gl, glow::LINES);
            gl.uniform_matrix_4_f32_slice(
                self.lines.at(shaders::U_MODEL),
                false,
                &frame.axes_model,
            );
            gl.uniform_2_f32_slice(self.lines.at(shaders::U_FADE), &NO_FADE);
            gl.depth_mask(true);
            self.axes.draw(gl, glow::LINES);
            calls = calls.saturating_add(2);

            // 4. Marker outlines (dark in the solid mode, each marker's color otherwise),
            //    then the see-through faces over them without writing depth.
            gl.use_program(Some(self.edges.program));
            gl.uniform_matrix_4_f32_slice(
                self.edges.at(shaders::U_VIEW_PROJ),
                false,
                &frame.view_proj,
            );
            gl.uniform_4_f32_slice(self.edges.at(shaders::U_EDGE), &c.edge);
            gl.uniform_1_f32(self.edges.at(shaders::U_MIX), frame.edge_mix);
            for s in &self.shapes {
                if s.all.count > 0 {
                    gl.bind_vertex_array(Some(s.all.edges));
                    gl.draw_arrays_instanced(glow::LINES, 0, s.geometry.edges_count, s.all.count);
                    calls = calls.saturating_add(1);
                }
            }
            if !solid && frame.face_alpha > 0.0 {
                gl.depth_mask(false);
                calls = calls.saturating_add(self.draw_faces(gl, frame, &mut instances));
                gl.depth_mask(true);
            } else if !solid {
                instances = self.shapes.iter().fold(0u32, |n, s| {
                    n.saturating_add(u32::try_from(s.all.count).unwrap_or(0))
                });
            }

            // 5. The selected marker's outline, over everything (seen through walls).
            gl.disable(glow::DEPTH_TEST);
            gl.use_program(Some(self.edges.program));
            gl.uniform_4_f32_slice(self.edges.at(shaders::U_EDGE), &c.select);
            gl.uniform_1_f32(self.edges.at(shaders::U_MIX), 1.0);
            for s in &self.shapes {
                if s.selected.count > 0 {
                    gl.bind_vertex_array(Some(s.selected.edges));
                    gl.draw_arrays_instanced(
                        glow::LINES,
                        0,
                        s.geometry.edges_count,
                        s.selected.count,
                    );
                    calls = calls.saturating_add(1);
                }
            }

            // Leave what egui_glow does not reset as GL's defaults.
            gl.enable(glow::DEPTH_TEST);
            gl.depth_mask(true);
            gl.bind_vertex_array(None);
            gl.use_program(None);
            if timing {
                gl.end_query(glow::TIME_ELAPSED);
            }
        }
        Ok((calls, instances))
    }

    /// The marker faces, one instanced call per shape; returns the draw calls and adds
    /// the instances drawn to `instances`.
    fn draw_faces(&self, gl: &glow::Context, frame: &Frame, instances: &mut u32) -> u32 {
        let mut calls = 0u32;
        // SAFETY: see the module docs; instance counts are the uploaded ones.
        unsafe {
            gl.use_program(Some(self.faces.program));
            gl.uniform_matrix_4_f32_slice(
                self.faces.at(shaders::U_VIEW_PROJ),
                false,
                &frame.view_proj,
            );
            gl.uniform_3_f32_slice(self.faces.at(shaders::U_LIGHT), &frame.light);
            gl.uniform_1_f32(self.faces.at(shaders::U_ALPHA), frame.face_alpha);
            for s in &self.shapes {
                if s.all.count > 0 {
                    gl.bind_vertex_array(Some(s.all.faces));
                    gl.draw_arrays_instanced(
                        glow::TRIANGLES,
                        0,
                        s.geometry.faces_count,
                        s.all.count,
                    );
                    calls = calls.saturating_add(1);
                    *instances = instances.saturating_add(u32::try_from(s.all.count).unwrap_or(0));
                }
            }
        }
        calls
    }
}

/// A fade range no line reaches.
const NO_FADE: [f32; 2] = [1.0e9, 2.0e9];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_layout_covers_the_instance_floats() {
        let n: i32 = INSTANCE_LAYOUT.iter().sum();
        assert_eq!(usize::try_from(n).ok(), Some(INSTANCE_FLOATS));
        assert_eq!(bytes(&[1.0, 2.0]).len(), 8);
    }
}
