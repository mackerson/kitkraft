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
//! - buffer data from owned Rust slices whose length GL is told, and draw counts taken
//!   from those lengths, so GL never reads past what was uploaded;
//! - no pointers, no mapped buffers, no raw handles from outside.
//!
//! Lifetime of the GL objects. A [`GlScene`] is built on the first paint of its tab and
//! deleted with that same context, never by `Drop` (which has no context), at one of
//! three points: a partly built scene, when a later step of [`GlScene::new`] fails; a
//! scene whose tab was dropped (its [`GlShared`] moves it to a retired list), at the
//! next paint of any viewport; and every scene still alive or retired at shutdown, in
//! [`release`], which the app calls from `eframe::App::on_exit` while the context is
//! still current. So none outlives the window.
//!
//! Every state this file changes that egui_glow does not reset after a callback (depth
//! mask, polygon offset, the bound vertex array and program) is restored before
//! returning. Nothing here reads plugin data: in R1 the scene is the grid, the axes and
//! one placeholder box; later markers arrive already as finite matrices
//! (`super::scene::Marker::model`).
#![allow(unsafe_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use eframe::egui_glow::{self, ShaderVersion};
use eframe::glow::{self, HasContext};

use super::scene::{self, LineVertex, SceneColors, LINE_LAYOUT, SOLID_LAYOUT};
use super::shaders::{self, ProgramSrc};
use super::Frame;

/// What the last draw cost, for the stats overlay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrawStats {
    /// GL draw calls in the last frame.
    pub draw_calls: u32,
    /// CPU time spent issuing them (not GPU time).
    pub submit: Duration,
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
    let draw_calls = scene.draw(gl, frame);
    let submit = start.elapsed();
    if state.history.len() >= HISTORY {
        state.history.pop_front();
    }
    state.history.push_back(frame.prep.saturating_add(submit));
    state.stats = DrawStats {
        draw_calls,
        submit,
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
    fn new(gl: &glow::Context, src: &ProgramSrc, header: &str) -> Result<Self, String> {
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

    /// Deletes the program; `self` is not used afterwards.
    fn destroy(&self, gl: &glow::Context) {
        // SAFETY: see the module docs; the program was made on `gl` and is no longer
        // drawn with.
        unsafe { gl.delete_program(self.program) }
    }

    fn at(&self, name: &str) -> Option<&glow::UniformLocation> {
        self.uniforms
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, l)| l.as_ref())
    }
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
    /// `layout`: the component count of each attribute, in location order.
    fn new(gl: &glow::Context, layout: &[i32], floats: &[f32]) -> Result<Self, String> {
        const TOO_LARGE: &str = "vertex layout too large";
        let stride_floats = layout
            .iter()
            .try_fold(0usize, |acc, n| acc.checked_add(usize::try_from(*n).ok()?))
            .ok_or(TOO_LARGE)?;
        let stride = stride_floats
            .checked_mul(4)
            .and_then(|b| i32::try_from(b).ok())
            .ok_or(TOO_LARGE)?;
        // Every attribute's offset, checked before any GL object exists.
        let mut attributes = Vec::with_capacity(layout.len());
        let mut offset: i32 = 0;
        for (location, size) in (0u32..).zip(layout) {
            attributes.push((location, *size, offset));
            offset = offset
                .checked_add(size.checked_mul(4).ok_or(TOO_LARGE)?)
                .ok_or(TOO_LARGE)?;
        }
        // SAFETY: see the module docs.
        let vao = unsafe { gl.create_vertex_array()? };
        // SAFETY: see the module docs.
        let vbo = match unsafe { gl.create_buffer() } {
            Ok(vbo) => vbo,
            Err(e) => {
                // SAFETY: see the module docs; the vertex array made just above, never
                // bound, is deleted.
                unsafe { gl.delete_vertex_array(vao) };
                return Err(e);
            }
        };
        // SAFETY: see the module docs. The attribute offsets stay inside `stride`.
        unsafe {
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            for (location, size, offset) in attributes {
                gl.enable_vertex_attrib_array(location);
                gl.vertex_attrib_pointer_f32(location, size, glow::FLOAT, false, stride, offset);
            }
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        let mut mesh = Self {
            vao,
            vbo,
            stride_floats,
            count: 0,
        };
        if let Err(e) = mesh.upload(gl, floats) {
            mesh.destroy(gl);
            return Err(e);
        }
        Ok(mesh)
    }

    /// Deletes the vertex array and its buffer; `self` is not used afterwards.
    fn destroy(&self, gl: &glow::Context) {
        // SAFETY: see the module docs; both objects were made on `gl` and are no longer
        // drawn with.
        unsafe {
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
        }
    }

    /// Replaces the buffer's contents; the vertex count follows the data.
    fn upload(&mut self, gl: &glow::Context, floats: &[f32]) -> Result<(), String> {
        let vertices = floats
            .len()
            .checked_div(self.stride_floats)
            .ok_or("empty vertex layout")?;
        self.count = i32::try_from(vertices).map_err(|_| "too many vertices".to_string())?;
        let bytes: Vec<u8> = floats.iter().flat_map(|f| f.to_ne_bytes()).collect();
        // SAFETY: see the module docs; `bytes` is an owned slice and GL copies it.
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes, glow::STATIC_DRAW);
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

/// Every GL object the viewport draws with.
struct GlScene {
    sky: Program,
    lines: Program,
    solid: Program,
    sky_mesh: Mesh,
    grid: Mesh,
    axes: Mesh,
    box_faces: Mesh,
    box_edges: Mesh,
    /// The colors the line meshes were built with (rebuilt when the theme changes).
    colors: SceneColors,
}

/// Line vertices for the marker outlines, in `colors.edge`.
fn edge_lines(colors: &SceneColors) -> Vec<LineVertex> {
    scene::unit_box_edges()
        .into_iter()
        .map(|pos| LineVertex {
            pos,
            color: colors.edge,
        })
        .collect()
}

impl GlScene {
    fn new(gl: &glow::Context, colors: &SceneColors) -> Result<Self, String> {
        let version = ShaderVersion::get(gl);
        if !version.is_new_shader_interface() {
            return Err(format!(
                "the viewport needs OpenGL 3 or OpenGL ES 3 shaders; this context offers {version:?}"
            ));
        }
        let header = shaders::header(version.version_declaration(), version.is_embedded());
        // Built one by one; on a failure everything made so far is deleted.
        let mut programs = Vec::with_capacity(3);
        let mut meshes = Vec::with_capacity(5);
        let built = Self::build(gl, colors, &header, &mut programs, &mut meshes).and_then(|()| {
            // SAFETY: see the module docs.
            let error = unsafe { gl.get_error() };
            if error == glow::NO_ERROR {
                Ok(())
            } else {
                Err(format!(
                    "OpenGL error 0x{error:04x} while building the scene"
                ))
            }
        });
        let made = (
            <[Program; 3]>::try_from(programs),
            <[Mesh; 5]>::try_from(meshes),
        );
        match (built, made) {
            (
                Ok(()),
                (Ok([sky, lines, solid]), Ok([sky_mesh, grid, axes, box_faces, box_edges])),
            ) => Ok(Self {
                sky,
                lines,
                solid,
                sky_mesh,
                grid,
                axes,
                box_faces,
                box_edges,
                colors: *colors,
            }),
            (built, (programs, meshes)) => {
                for p in programs.map_or_else(|v| v, Vec::from) {
                    p.destroy(gl);
                }
                for m in meshes.map_or_else(|v| v, Vec::from) {
                    m.destroy(gl);
                }
                Err(built
                    .err()
                    .unwrap_or_else(|| "the scene was built incomplete".to_string()))
            }
        }
    }

    /// Makes the programs (sky, lines, solid), then the meshes (sky, grid, axes, box
    /// faces, box edges), pushing each as it is made.
    fn build(
        gl: &glow::Context,
        colors: &SceneColors,
        header: &str,
        programs: &mut Vec<Program>,
        meshes: &mut Vec<Mesh>,
    ) -> Result<(), String> {
        for src in [&shaders::SKY, &shaders::LINES, &shaders::SOLID] {
            programs.push(Program::new(gl, src, header)?);
        }
        let line_layout = LINE_LAYOUT;
        let solid_layout = SOLID_LAYOUT;
        let parts: [(&[i32], Vec<f32>); 5] = [
            // One triangle that covers the viewport.
            (&[2], vec![-1.0, -1.0, 3.0, -1.0, -1.0, 3.0]),
            (&line_layout, scene::line_floats(&scene::grid_lines(colors))),
            (&line_layout, scene::line_floats(&scene::axis_lines(colors))),
            (&solid_layout, scene::solid_floats(&scene::unit_box_faces())),
            (&line_layout, scene::line_floats(&edge_lines(colors))),
        ];
        for (layout, floats) in parts {
            meshes.push(Mesh::new(gl, layout, &floats)?);
        }
        Ok(())
    }

    /// Deletes every GL object of the scene; `self` is not used afterwards.
    fn destroy(&self, gl: &glow::Context) {
        for p in [&self.sky, &self.lines, &self.solid] {
            p.destroy(gl);
        }
        for m in [
            &self.sky_mesh,
            &self.grid,
            &self.axes,
            &self.box_faces,
            &self.box_edges,
        ] {
            m.destroy(gl);
        }
    }

    /// Draws one frame; returns the number of draw calls.
    fn draw(&mut self, gl: &glow::Context, frame: &Frame) -> u32 {
        if frame.colors != self.colors {
            let rebuilt = self
                .grid
                .upload(gl, &scene::line_floats(&scene::grid_lines(&frame.colors)))
                .and_then(|()| {
                    self.axes
                        .upload(gl, &scene::line_floats(&scene::axis_lines(&frame.colors)))
                })
                .and_then(|()| {
                    self.box_edges
                        .upload(gl, &scene::line_floats(&edge_lines(&frame.colors)))
                });
            if rebuilt.is_ok() {
                self.colors = frame.colors;
            }
        }
        let c = &frame.colors;
        let mut calls: u32 = 0;
        // SAFETY: see the module docs; uniforms are set on the program in use, with
        // slices of the length their GLSL type needs.
        unsafe {
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

            // 2. Marker faces, depth-tested, back faces culled, pushed back a little so
            //    their outlines win the depth test.
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.depth_mask(true);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
            gl.front_face(glow::CCW);
            gl.enable(glow::POLYGON_OFFSET_FILL);
            gl.polygon_offset(1.0, 1.0);
            gl.use_program(Some(self.solid.program));
            gl.uniform_matrix_4_f32_slice(
                self.solid.at(shaders::U_VIEW_PROJ),
                false,
                &frame.view_proj,
            );
            gl.uniform_3_f32_slice(self.solid.at(shaders::U_LIGHT), &frame.light);
            for (model, color) in &frame.markers {
                gl.uniform_matrix_4_f32_slice(self.solid.at(shaders::U_MODEL), false, model);
                gl.uniform_4_f32_slice(self.solid.at(shaders::U_COLOR), color);
                self.box_faces.draw(gl, glow::TRIANGLES);
                calls = calls.saturating_add(1);
            }
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.disable(glow::CULL_FACE);

            // 3. Lines, blended: the grid (fading out with distance, not writing depth),
            //    the axes, the marker outlines.
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(self.lines.program));
            gl.uniform_matrix_4_f32_slice(
                self.lines.at(shaders::U_VIEW_PROJ),
                false,
                &frame.view_proj,
            );
            gl.uniform_matrix_4_f32_slice(self.lines.at(shaders::U_MODEL), false, &IDENTITY);
            gl.uniform_3_f32_slice(self.lines.at(shaders::U_EYE), &frame.eye);
            gl.uniform_2_f32_slice(self.lines.at(shaders::U_FADE), &frame.grid_fade);
            gl.depth_mask(false);
            self.grid.draw(gl, glow::LINES);
            gl.depth_mask(true);
            gl.uniform_2_f32_slice(self.lines.at(shaders::U_FADE), &NO_FADE);
            self.axes.draw(gl, glow::LINES);
            calls = calls.saturating_add(2);
            for (model, _) in &frame.markers {
                gl.uniform_matrix_4_f32_slice(self.lines.at(shaders::U_MODEL), false, model);
                self.box_edges.draw(gl, glow::LINES);
                calls = calls.saturating_add(1);
            }

            // Leave what egui_glow does not reset as GL's defaults.
            gl.depth_mask(true);
            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
        calls
    }
}

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];
/// A fade range no line reaches.
const NO_FADE: [f32; 2] = [1.0e9, 2.0e9];
