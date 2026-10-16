//! The Render Window tab (M7.R1, the owner's "3D placeholder"): a toolbar, the 3D
//! viewport (`crate::viewport`) in the rest of the tab, and a stats overlay (frame time,
//! draw calls, frames drawn). Read-only and without game data: the grid, the world axes,
//! a horizon tint and one placeholder box at the origin. M7.R2 adds the selected cell's
//! references as markers from the engine's `cell` command.
//!
//! The controls ([`crate::viewport::BINDINGS`]) are not yet observed against the CK's
//! Render Window (`docs/ck-parity.md`, "Interface parity").

use eframe::egui::{self, Color32, Rect};

use crate::panels::common::{action_bar, panel_head, TabCtx};
use crate::theme::{self, Status};
use crate::viewport::camera::OrbitCamera;
use crate::viewport::{Stats, Viewport, BINDINGS};

/// The overlay's lines: frame time (CPU: preparing the frame and issuing GL calls; not
/// GPU time) and its median, 95th percentile and maximum over the recent frames, draw
/// calls, frames drawn since the window opened (it stays still while nothing moves: the
/// viewport never repaints on its own), the viewport size and the camera.
pub fn stats_lines(stats: &Stats, cam: &OrbitCamera) -> Vec<String> {
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1_000.0;
    let ui = ms(stats.ui);
    let gl = ms(stats.draw.submit);
    let [w, h] = stats.draw.size_px;
    let t = cam.target;
    vec![
        format!("frame   {:.2} ms cpu (ui {ui:.2}, gl {gl:.2})", ui + gl),
        format!(
            "recent  p50 {:.2} p95 {:.2} max {:.2} ms ({} frames)",
            ms(stats.times.p50),
            ms(stats.times.p95),
            ms(stats.times.max),
            stats.times.samples
        ),
        format!("draws   {}", stats.draw.draw_calls),
        format!("frames  {}", stats.draw.frames),
        format!("size    {w} x {h} px"),
        format!(
            "camera  yaw {:.0} pitch {:.0} dist {:.0}",
            cam.yaw.to_degrees(),
            cam.pitch.to_degrees(),
            cam.distance
        ),
        format!("target  {:.0}, {:.0}, {:.0}", t.x, t.y, t.z),
    ]
}

/// The controls as one hover text.
pub fn controls_text() -> String {
    let mut out: Vec<String> = BINDINGS.iter().map(|(a, b)| format!("{a}: {b}")).collect();
    out.push("Not yet observed against the CK's Render Window (docs/ck-parity.md).".into());
    out.join("\n")
}

// ---- drawing

/// The tab's state.
pub struct RenderTab {
    pub viewport: Viewport,
    /// Show the stats overlay.
    pub stats: bool,
}

impl Default for RenderTab {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            stats: true,
        }
    }
}

impl RenderTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut TabCtx) {
        // The panel frame (M6.V5): title, then the toolbar as the action bar.
        panel_head(ui, "Render Window", "");
        action_bar(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Reset view")
                    .on_hover_text("Back to the default view of the origin")
                    .clicked()
                {
                    self.viewport.reset_camera();
                }
                ui.checkbox(&mut self.stats, "Stats");
                ui.separator();
                ui.weak("controls").on_hover_text(controls_text());
                ui.separator();
                ui.weak("placeholder: grid, axes and one box at the origin; no game data yet");
            })
        });
        let response = self.viewport.show(ui);
        let stats = self.viewport.stats();
        if let Some(err) = &stats.error {
            // The status component for painted text (M6.V4): glyph and word lead it.
            let (line, color) =
                theme::status_painted(ui, Status::Fail, &format!("viewport: {err}"));
            overlay(ui, response.rect, &[line], color);
        } else if self.stats {
            let lines = stats_lines(&stats, &self.viewport.camera);
            overlay(ui, response.rect, &lines, ui.visuals().strong_text_color());
        }
    }
}

/// Text in a box at the viewport's top-left corner, drawn after (over) the 3D scene.
fn overlay(ui: &egui::Ui, rect: Rect, lines: &[String], text: Color32) {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let pad = ui.spacing().item_spacing.x;
    let painter = ui.painter_at(rect);
    let galley = painter.layout_no_wrap(lines.join("\n"), font, text);
    let at = rect.shrink(pad).min;
    let bg = Rect::from_min_size(at, galley.size()).expand(pad * 0.5);
    let fill = ui.visuals().extreme_bg_color.gamma_multiply(0.8);
    painter.rect_filled(bg, ui.visuals().widgets.noninteractive.corner_radius, fill);
    painter.galley(at, galley, text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::DrawStats;
    use std::time::Duration;

    #[test]
    fn stats_show_frame_time_draw_calls_and_frames() {
        let stats = Stats {
            draw: DrawStats {
                draw_calls: 5,
                submit: Duration::from_micros(120),
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
        assert_eq!(lines[0], "frame   0.42 ms cpu (ui 0.30, gl 0.12)");
        assert_eq!(lines[1], "recent  p50 0.40 p95 0.50 max 0.50 ms (2 frames)");
        assert_eq!(lines[2], "draws   5");
        assert_eq!(lines[3], "frames  37");
        assert_eq!(lines[4], "size    1280 x 720 px");
        assert_eq!(lines[5], "camera  yaw 60 pitch 30 dist 1500");
        assert_eq!(lines[6], "target  0, 0, 0");
    }

    #[test]
    fn the_controls_say_they_are_unobserved() {
        let t = controls_text();
        for (action, binding) in BINDINGS {
            assert!(t.contains(action) && t.contains(binding));
        }
        assert!(t.contains("Not yet observed against the CK"));
    }
}
