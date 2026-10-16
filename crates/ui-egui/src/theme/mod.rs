//! The KitKraft visual system (M6.V1, M6.V3): [`tokens`] holds the values as data, this
//! module applies them to egui in one place ([`apply`]) and gives the panels what they
//! draw with: the active palette's colors, the status component ([`status`]), the table
//! style ([`table`]), the frames, and the named sizes ([`width`], [`view_height`],
//! [`gap`]). No panel holds a color, a font size, a spacing edit or a size of its own (a
//! test scans the crate). `docs/design/workbench-style.md` documents the tokens
//! and the rules; `--theme` on `kit ui`, `theme` in `ui.json`, or the Start tab's picker
//! chooses a preset. `Workbench` is the default (owner decision M6.O1, 2026-10-16).

pub mod tokens;

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Frame, Grid, Margin, Stroke, TextStyle,
};

pub use tokens::{Mode, Palette, Preset, Rgb, Status, Theme};

/// A preset and a mode, as `--theme` and `ui.json` spell it: `workbench`,
/// `studio-dark`, `classic-light`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeChoice {
    pub preset: Preset,
    pub mode: Mode,
}

impl Default for ThemeChoice {
    /// Workbench (M6.O1), following the system's light or dark setting.
    fn default() -> Self {
        Self {
            preset: Preset::Workbench,
            mode: Mode::System,
        }
    }
}

impl ThemeChoice {
    /// Every accepted spelling, for help texts.
    pub const HELP: &'static str =
        "workbench (default), studio or classic (legacy); add -light or -dark to fix the mode";

    /// Parses `preset` or `preset-light` / `preset-dark` (case-insensitive).
    pub fn parse(text: &str) -> Result<Self, String> {
        let t = text.trim().to_ascii_lowercase();
        let (name, mode) = if let Some(n) = t.strip_suffix("-light") {
            (n, Mode::Light)
        } else if let Some(n) = t.strip_suffix("-dark") {
            (n, Mode::Dark)
        } else {
            (t.as_str(), Mode::System)
        };
        let preset = Preset::ALL
            .into_iter()
            .find(|p| p.id() == name)
            .ok_or_else(|| format!("unknown theme {text:?}: {}", Self::HELP))?;
        Ok(Self { preset, mode })
    }

    /// The spelling `parse` reads back.
    pub fn id(&self) -> String {
        match self.mode {
            Mode::System => self.preset.id().to_string(),
            Mode::Light => format!("{}-light", self.preset.id()),
            Mode::Dark => format!("{}-dark", self.preset.id()),
        }
    }
}

/// Where the active theme is kept in egui's memory, so free drawing functions find it.
fn key() -> egui::Id {
    egui::Id::new("kitkraft-theme")
}

/// A text field's width, by role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// A record signature.
    Tiny,
    /// A filter, a member name.
    Short,
    /// A form, a plugin name.
    Medium,
    /// A path, a `where` clause.
    Long,
    /// The Load and Ledger forms.
    Wide,
}

/// A scrolling table's height, by role (a number of table rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Short,
    Medium,
    Tall,
}

/// The width of a text field of role `field`.
pub fn width(ui: &egui::Ui, field: Field) -> f32 {
    let z = active(ui.ctx()).sizes;
    match field {
        Field::Tiny => z.field_tiny,
        Field::Short => z.field_short,
        Field::Medium => z.field_medium,
        Field::Long => z.field_long,
        Field::Wide => z.field_wide,
    }
}

/// The width of the "loaded..." plugin picker.
pub fn combo_width(ui: &egui::Ui) -> f32 {
    active(ui.ctx()).sizes.combo
}

/// The height of a scrolling table of role `view`: its rows at the density's row pitch.
pub fn view_height(ui: &egui::Ui, view: View) -> f32 {
    let t = active(ui.ctx());
    let rows = match view {
        View::Short => t.sizes.view_short_rows,
        View::Medium => t.sizes.view_medium_rows,
        View::Tall => t.sizes.view_tall_rows,
    };
    rows * (t.density.row_height + t.density.cell_y)
}

/// The space between two blocks of a panel.
pub fn gap(ui: &mut egui::Ui) {
    let block = active(ui.ctx()).spacing.block;
    ui.add_space(block);
}

/// The space before a heading in a document (Help).
pub fn heading_gap(ui: &mut egui::Ui) {
    let s = active(ui.ctx()).spacing.item_y;
    ui.add_space(s);
}

/// The indent of a list item at `depth` in a document (Help).
pub fn list_indent(ui: &mut egui::Ui, depth: u8) {
    let s = active(ui.ctx()).spacing;
    ui.add_space(f32::from(depth) * s.indent + s.item_y);
}

/// A row of inline runs with no gap between them (Markdown text in Help).
pub fn inline_row<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        add(ui)
    })
    .inner
}

/// The width of a Help table cell in a table of `columns` columns.
pub fn doc_cell(ui: &egui::Ui, columns: usize) -> f32 {
    let z = active(ui.ctx()).sizes;
    match columns {
        0..=2 => z.doc_cell_wide,
        3..=4 => z.doc_cell_mid,
        _ => z.doc_cell_narrow,
    }
}

/// A section title inside a tab (a level-3 heading in Help).
pub fn subheading(ui: &egui::Ui, text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text)
        .strong()
        .size(active(ui.ctx()).type_scale.subheading)
}

/// Applies `theme` in `mode` to the context: the style of both egui themes (so a system
/// switch between light and dark keeps the tokens), the mode, and the tokens the panels
/// read back through [`active`]. `Classic` restores egui's default style.
pub fn apply(ctx: &egui::Context, theme: &Theme, mode: Mode) {
    for (which, palette) in [
        (egui::Theme::Dark, &theme.dark),
        (egui::Theme::Light, &theme.light),
    ] {
        let mut style = which.default_style();
        if theme.restyle {
            restyle(&mut style, theme, palette);
        }
        ctx.set_style_of(which, style);
    }
    ctx.set_theme(match mode {
        Mode::System => egui::ThemePreference::System,
        Mode::Light => egui::ThemePreference::Light,
        Mode::Dark => egui::ThemePreference::Dark,
    });
    ctx.data_mut(|d| d.insert_temp(key(), Arc::new(*theme)));
}

/// The theme [`apply`] set last (`Classic` before the first call).
pub fn active(ctx: &egui::Context) -> Arc<Theme> {
    ctx.data(|d| d.get_temp::<Arc<Theme>>(key()))
        .unwrap_or_else(|| Arc::new(tokens::classic()))
}

/// The active palette for the mode the context is drawing in.
pub fn palette(ui: &egui::Ui) -> Palette {
    *active(ui.ctx()).palette(ui.visuals().dark_mode)
}

/// A token color as egui's.
pub fn color(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// A status's color in the active palette. Private (M6.V4): a status color reaches the
/// screen only with its glyph and word, through [`status`], [`status_line`],
/// [`status_header`] or [`status_painted`]; a test scans the crate for status colors
/// used anywhere else.
fn status_color(ui: &egui::Ui, status: Status) -> Color32 {
    color(palette(ui).status.of(status))
}

/// Code in the Help tab.
pub fn code(ui: &egui::Ui) -> Color32 {
    color(palette(ui).code)
}

/// The status component: glyph, word and color together (the word is `label`, or the
/// status's own word when `label` is empty). `Classic` draws the colored word only, as
/// the window did before the tokens.
pub fn status(ui: &mut egui::Ui, status: Status, label: &str) -> egui::Response {
    let theme = active(ui.ctx());
    let c = color(theme.palette(ui.visuals().dark_mode).status.of(status));
    let word = if label.is_empty() {
        status.word()
    } else {
        label
    };
    if !theme.status_glyphs {
        return ui.colored_label(c, word);
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        glyph(ui, status.glyph(), c);
        ui.colored_label(c, word);
    })
    .response
    .on_hover_text(format!("{} {}", status.glyph_char(), status.word()))
}

/// The status component for a long line outside a table (an error, a confirm question):
/// the same glyph, word and color, with the text wrapping at the panel's edge.
pub fn status_line(ui: &mut egui::Ui, status: Status, text: &str) -> egui::Response {
    let theme = active(ui.ctx());
    let c = color(theme.palette(ui.visuals().dark_mode).status.of(status));
    if !theme.status_glyphs {
        return ui.colored_label(c, text);
    }
    ui.horizontal_wrapped(|ui| {
        glyph(ui, status.glyph(), c);
        ui.colored_label(c, text);
    })
    .response
    .on_hover_text(format!("{} {}", status.glyph_char(), status.word()))
}

/// The status component as plain text, for a widget that takes one styled text (a
/// collapsing header): the glyph's character and the status word lead `text`, all in
/// the status color (`PASS ...`, `FAIL ...`). `Classic` leaves the word out, as before.
pub fn status_header(ui: &egui::Ui, status: Status, text: &str) -> egui::RichText {
    let lead = if active(ui.ctx()).status_glyphs {
        format!("{} {}  ", status.glyph_char(), status.word().to_uppercase())
    } else {
        String::new()
    };
    egui::RichText::new(format!("{lead}{text}")).color(status_color(ui, status))
}

/// The status component for text a panel paints itself (an overlay over the 3D
/// viewport): the line with the glyph's character and the status word in front, and the
/// color to paint it in.
pub fn status_painted(ui: &egui::Ui, status: Status, text: &str) -> (String, Color32) {
    (
        format!(
            "{} {}: {text}",
            status.glyph_char(),
            status.word().to_uppercase()
        ),
        status_color(ui, status),
    )
}

/// The action bar's frame (M6.V5): the tab's form and buttons on the raised surface,
/// outlined, with the item gaps as its margin.
pub fn bar_frame(ui: &egui::Ui) -> Frame {
    let theme = active(ui.ctx());
    if !theme.restyle {
        return Frame::group(ui.style());
    }
    let pal = theme.palette(ui.visuals().dark_mode);
    Frame::new()
        .fill(color(pal.surface.raised))
        .stroke(Stroke::new(1.0, color(pal.border)))
        .corner_radius(radius(theme.radii.control))
        .inner_margin(Margin::symmetric(
            to_i8(theme.spacing.item_x),
            to_i8(theme.spacing.item_y.max(theme.spacing.button_y)),
        ))
}

/// Inside a bar or strip frame: as wide as the panel, but no wider than the part of it on
/// screen, so a frame inside a pane that scrolls sideways (Records) wraps its text at
/// the pane's visible edge instead of past it.
pub fn fill_visible_width(ui: &mut egui::Ui) {
    let theme = active(ui.ctx());
    let margin = if theme.restyle {
        theme.spacing.item_x
    } else {
        ui.spacing().item_spacing.x
    };
    let visible = ui.clip_rect().max.x - ui.max_rect().min.x - margin;
    ui.set_width(ui.available_width().min(visible).max(0.0));
}

/// The limitations strip's frame (M6.V5): the sunken surface, outlined, under the
/// result header and above the panel's body.
pub fn strip_frame(ui: &egui::Ui) -> Frame {
    let theme = active(ui.ctx());
    if !theme.restyle {
        return Frame::group(ui.style());
    }
    let pal = theme.palette(ui.visuals().dark_mode);
    Frame::new()
        .fill(color(pal.surface.sunken))
        .stroke(Stroke::new(1.0, color(pal.border)))
        .corner_radius(radius(theme.radii.control))
        .inner_margin(Margin::symmetric(
            to_i8(theme.spacing.item_x),
            to_i8(theme.spacing.item_y),
        ))
}

/// A status glyph, painted (no icon font), as tall as the body text.
fn glyph(ui: &mut egui::Ui, glyph: tokens::Glyph, c: Color32) {
    use tokens::Glyph;
    let h = ui.text_style_height(&TextStyle::Body);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(h, h), egui::Sense::hover());
    let p = ui.painter();
    let center = rect.center();
    let r = h * 0.32;
    let stroke = Stroke::new((h * 0.12).max(1.5), c);
    match glyph {
        Glyph::Disc => {
            p.circle_filled(center, r, c);
        }
        Glyph::Ring => {
            p.circle_stroke(center, r - stroke.width * 0.5, stroke);
        }
        Glyph::Cross => {
            let d = r * 0.9;
            p.line_segment([at(center, -d, -d), at(center, d, d)], stroke);
            p.line_segment([at(center, -d, d), at(center, d, -d)], stroke);
        }
        Glyph::Triangle => {
            let pts = vec![
                at(center, 0.0, -r * 1.1),
                at(center, r * 1.1, r * 0.85),
                at(center, -r * 1.1, r * 0.85),
            ];
            p.add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
        }
        Glyph::Diamond => {
            let pts = vec![
                at(center, 0.0, -r * 1.1),
                at(center, r * 1.1, 0.0),
                at(center, 0.0, r * 1.1),
                at(center, -r * 1.1, 0.0),
            ];
            p.add(egui::Shape::closed_line(pts, stroke));
            p.circle_filled(center, stroke.width * 0.6, c);
        }
        Glyph::Square => {
            let half = r * 0.9;
            let sq = egui::Rect::from_center_size(center, egui::vec2(half * 2.0, half * 2.0));
            p.rect_stroke(sq, 0.0, stroke, egui::StrokeKind::Middle);
        }
    }
}

/// `center` moved by `dx`, `dy` points.
fn at(center: egui::Pos2, dx: f32, dy: f32) -> egui::Pos2 {
    egui::pos2(center.x + dx, center.y + dy)
}

/// The table style: a grid with the density's row height, cell gaps and zebra rows.
pub fn table(ui: &egui::Ui, id: impl egui::AsIdSalt) -> Grid {
    let d = active(ui.ctx()).density;
    let grid = Grid::new(id).striped(d.zebra);
    if active(ui.ctx()).restyle {
        grid.min_row_height(d.row_height)
            .spacing(egui::vec2(d.cell_x, d.cell_y))
    } else {
        grid
    }
}

/// A form's label and field columns: no zebra, the item gap between rows.
pub fn form_grid(ui: &egui::Ui, id: impl egui::AsIdSalt) -> Grid {
    let t = active(ui.ctx());
    let grid = Grid::new(id).num_columns(2).striped(false);
    if t.restyle {
        grid.spacing(egui::vec2(t.spacing.item_x, t.spacing.item_y))
    } else {
        grid
    }
}

/// The central panel's frame: the panel margin token on the base surface.
pub fn central_frame(ui: &egui::Ui) -> Frame {
    let theme = active(ui.ctx());
    let frame = Frame::central_panel(ui.style());
    if theme.restyle {
        frame.inner_margin(margin(theme.spacing.panel_margin))
    } else {
        frame
    }
}

/// The top bar's frame (title and tabs) and the status line's (engine state, session).
pub fn top_frame(ui: &egui::Ui) -> Frame {
    let theme = active(ui.ctx());
    let frame = Frame::side_top_panel(ui.style());
    if theme.restyle {
        let pal = theme.palette(ui.visuals().dark_mode);
        frame
            .inner_margin(Margin::symmetric(
                to_i8(theme.spacing.panel_margin),
                to_i8(theme.spacing.button_y.max(4.0)),
            ))
            .fill(color(pal.surface.raised))
            .stroke(Stroke::new(1.0, color(pal.border)))
    } else {
        frame
    }
}

/// The Records split: the narrowest pane, and the inspector's share of the tab before
/// the owner drags the line between them.
pub fn split(ui: &egui::Ui) -> (f32, f32) {
    let z = active(ui.ctx()).sizes;
    (z.pane_min, z.split_default)
}

/// The width of the grab area around the line between two panes.
pub fn split_handle(ui: &egui::Ui) -> f32 {
    active(ui.ctx()).spacing.item_x
}

/// The line between two panes: the border color, the accent while hovered or dragged.
pub fn split_stroke(ui: &egui::Ui, hot: bool) -> Stroke {
    let pal = palette(ui);
    Stroke::new(1.0, color(if hot { pal.accent } else { pal.border }))
}

/// The window's size at start.
pub const WINDOW: [f32; 2] = tokens::WINDOW;

fn to_i8(v: f32) -> i8 {
    // Token values are small whole points (0 to 32); anything else is clamped.
    let clamped = v.clamp(0.0, 64.0).round();
    i8::try_from(clamped as i32).unwrap_or(8)
}

fn margin(v: f32) -> Margin {
    Margin::same(to_i8(v))
}

fn radius(v: u8) -> CornerRadius {
    CornerRadius::same(v)
}

/// The tokens written into one of egui's styles.
fn restyle(style: &mut egui::Style, theme: &Theme, pal: &Palette) {
    let t = theme.type_scale;
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(t.small, FontFamily::Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(t.body, FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(t.button, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(t.heading, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(t.mono, FontFamily::Monospace),
        ),
    ]
    .into();

    let s = theme.spacing;
    let sp = &mut style.spacing;
    sp.item_spacing = egui::vec2(s.item_x, s.item_y);
    sp.button_padding = egui::vec2(s.button_x, s.button_y);
    sp.indent = s.indent;
    sp.interact_size.y = theme.density.row_height;
    sp.window_margin = margin(s.panel_margin);
    sp.menu_margin = margin(s.item_x);

    let v = &mut style.visuals;
    let su = pal.surface;
    v.panel_fill = color(su.base);
    v.window_fill = color(su.raised);
    v.extreme_bg_color = color(su.sunken);
    v.text_edit_bg_color = Some(color(su.sunken));
    v.code_bg_color = color(su.sunken);
    v.faint_bg_color = color(if theme.density.zebra {
        su.zebra
    } else {
        su.base
    });
    v.striped = theme.density.zebra;
    v.hyperlink_color = color(pal.accent);
    v.warn_fg_color = color(pal.status.warn);
    v.error_fg_color = color(pal.status.fail);
    v.weak_text_color = Some(color(pal.text.muted));
    v.override_text_color = None;
    v.selection.bg_fill = color(su.selection);
    // Also the focused text field's outline: the focus token (3:1 on every surface).
    v.selection.stroke = Stroke::new(1.0, color(pal.focus));
    v.window_stroke = Stroke::new(1.0, color(pal.border));
    v.window_corner_radius = radius(theme.radii.panel);
    v.menu_corner_radius = radius(theme.radii.panel);

    let control = radius(theme.radii.control);
    let border = Stroke::new(1.0, color(pal.border));
    let outline = Stroke::new(1.0, color(pal.control_border));
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = color(su.base);
    w.noninteractive.weak_bg_fill = color(su.base);
    w.noninteractive.bg_stroke = border;
    w.noninteractive.fg_stroke = Stroke::new(1.0, color(pal.text.body));
    w.noninteractive.corner_radius = control;
    w.inactive.bg_fill = color(su.raised);
    w.inactive.weak_bg_fill = color(su.raised);
    w.inactive.bg_stroke = outline;
    w.inactive.fg_stroke = Stroke::new(1.0, color(pal.text.body));
    w.inactive.corner_radius = control;
    w.hovered.bg_fill = color(su.hover);
    w.hovered.weak_bg_fill = color(su.hover);
    w.hovered.bg_stroke = Stroke::new(1.0, color(pal.accent));
    w.hovered.fg_stroke = Stroke::new(1.5, color(pal.text.strong));
    w.hovered.corner_radius = control;
    w.active.bg_fill = color(su.selection);
    w.active.weak_bg_fill = color(su.selection);
    // Pressed, or holding keyboard focus: the focus ring.
    w.active.bg_stroke = Stroke::new(1.0, color(pal.focus));
    w.active.fg_stroke = Stroke::new(2.0, color(pal.text.strong));
    w.active.corner_radius = control;
    w.open.bg_fill = color(su.raised);
    w.open.weak_bg_fill = color(su.hover);
    w.open.bg_stroke = outline;
    w.open.fg_stroke = Stroke::new(1.0, color(pal.text.strong));
    w.open.corner_radius = control;
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn choices_parse_and_print_back() {
        for (text, preset, mode) in [
            ("classic", Preset::Classic, Mode::System),
            ("Workbench", Preset::Workbench, Mode::System),
            ("workbench-dark", Preset::Workbench, Mode::Dark),
            (" studio-light ", Preset::Studio, Mode::Light),
        ] {
            let c = ThemeChoice::parse(text).unwrap();
            assert_eq!((c.preset, c.mode), (preset, mode), "{text}");
            assert_eq!(ThemeChoice::parse(&c.id()).unwrap(), c);
        }
        for bad in ["", "dark", "studio-dim", "neon"] {
            let e = ThemeChoice::parse(bad).unwrap_err();
            assert!(e.contains("workbench") && e.contains("studio"), "{e}");
        }
        assert_eq!(ThemeChoice::default().preset, Preset::Workbench);
        assert_eq!(ThemeChoice::default().mode, Mode::System);
    }

    /// What a line of panel code may not hold: a color literal, a font size, a spacing
    /// edit, or a size given as a number (M6.V3). Returns why, or `None`.
    fn hard_coded(line: &str) -> Option<&'static str> {
        let code = line.split("//").next().unwrap_or("");
        for (needle, why) in [
            ("Color32::", "a Color32 constant or constructor"),
            ("from_rgb", "a color literal"),
            ("from_gray", "a color literal"),
            ("spacing_mut()", "a spacing edit"),
            ("style_mut()", "a style edit"),
            ("FontId::", "a font size"),
            ("Margin::", "a margin"),
            ("Stroke::new(", "a stroke width"),
        ] {
            if code.contains(needle) {
                return Some(why);
            }
        }
        // A sizing call whose argument starts with a number.
        for call in [
            ".size(",
            "add_space(",
            "desired_width(",
            "max_height(",
            "min_height(",
            "max_width(",
            "set_max_width(",
            ".width(",
            "default_size(",
            "inner_margin(",
            "vec2(",
        ] {
            let mut rest = code;
            while let Some(i) = rest.find(call) {
                rest = rest.get(i + call.len()..).unwrap_or("");
                let arg = rest.trim_start();
                if arg.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
                    return Some("a size given as a number");
                }
            }
        }
        None
    }

    #[test]
    fn the_scan_catches_what_it_should() {
        for bad in [
            "ui.colored_label(Color32::GRAY, x);",
            "let c = egui::Color32::from_rgb(1, 2, 3);",
            "ui.spacing_mut().item_spacing.x = 0.0;",
            "RichText::new(t).strong().size(16.0)",
            "TextEdit::singleline(&mut s).desired_width(420.0)",
            "ScrollArea::vertical().max_height( 300.0)",
            "ui.add_space(6.0);",
            "ComboBox::from_id_salt(id).width(110.0)",
        ] {
            assert!(hard_coded(bad).is_some(), "{bad}");
        }
        for good in [
            "TextEdit::singleline(&mut s).desired_width(theme::width(ui, Field::Long))",
            "ui.add_space(f32::from(d) * s.indent);",
            "fn color(c: Rgb) -> egui::Color32 {",
            "// Color32::GRAY was here",
            "theme::gap(ui);",
        ] {
            assert!(hard_coded(good).is_none(), "{good}");
        }
    }

    /// M6.V3: no hard-coded color, font size, spacing edit or size outside `theme/` (every
    /// `.rs` file under `src/`, tests included).
    #[test]
    fn no_color_size_or_spacing_literal_outside_the_theme() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let theme_dir = root.join("theme");
        let mut stack = vec![root];
        let mut problems = Vec::new();
        let mut scanned = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    if path != theme_dir {
                        stack.push(path);
                    }
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                scanned += 1;
                let text = std::fs::read_to_string(&path).unwrap();
                for (n, line) in text.lines().enumerate() {
                    if let Some(why) = hard_coded(line) {
                        problems.push(format!(
                            "{}:{}: {why}: {}",
                            path.display(),
                            n + 1,
                            line.trim()
                        ));
                    }
                }
            }
        }
        assert!(scanned >= 20, "scanned {scanned} files");
        assert!(problems.is_empty(), "{problems:#?}");
    }

    #[test]
    fn apply_sets_both_styles_and_the_active_tokens() {
        let ctx = egui::Context::default();
        apply(&ctx, &tokens::studio(), Mode::Light);
        assert_eq!(active(&ctx).preset, Preset::Studio);
        let light = ctx.style_of(egui::Theme::Light);
        let dark = ctx.style_of(egui::Theme::Dark);
        let s = tokens::studio();
        assert_eq!(light.visuals.panel_fill, color(s.light.surface.base));
        assert_eq!(dark.visuals.panel_fill, color(s.dark.surface.base));
        assert_eq!(
            light.text_styles.get(&TextStyle::Heading).map(|f| f.size),
            Some(s.type_scale.heading)
        );
        assert!(!light.visuals.striped);

        // Classic puts egui's defaults back.
        apply(&ctx, &tokens::classic(), Mode::System);
        assert_eq!(active(&ctx).preset, Preset::Classic);
        let light = ctx.style_of(egui::Theme::Light);
        assert_eq!(
            light.visuals.panel_fill,
            egui::Theme::Light.default_style().visuals.panel_fill
        );
        let (dark, default) = (
            ctx.style_of(egui::Theme::Dark),
            egui::Theme::Dark.default_style(),
        );
        assert_eq!(dark.visuals.widgets, default.visuals.widgets);
        assert_eq!(dark.visuals.faint_bg_color, default.visuals.faint_bg_color);
        assert_eq!(dark.spacing.item_spacing, default.spacing.item_spacing);
        assert_eq!(dark.text_styles, default.text_styles);
    }

    /// What a line of code outside `theme/` may not hold (M6.V4): text colored by hand
    /// (`colored_label`), a status color taken from the palette, egui's own warning and
    /// error colors, or the status component's parts (its glyph character, the glyph
    /// switch) used to build a status of its own. Returns why, or `None`.
    fn status_outside_the_component(line: &str) -> Option<&'static str> {
        let code = line.split("//").next().unwrap_or("");
        for (needle, why) in [
            (
                "colored_label(",
                "colored text outside the status component",
            ),
            ("theme::warn(", "the warning color without the component"),
            ("theme::bad(", "the fail color without the component"),
            ("theme::good(", "the pass color without the component"),
            (".status.of(", "a status color from the palette"),
            (".status.pass", "a status color from the palette"),
            (".status.fail", "a status color from the palette"),
            (".status.warn", "a status color from the palette"),
            (".status.not_run", "a status color from the palette"),
            (".status.inconclusive", "a status color from the palette"),
            (".status.unknown", "a status color from the palette"),
            ("warn_fg_color", "egui's warning color"),
            ("error_fg_color", "egui's error color"),
            ("glyph_char(", "a hand-made status (use status_header)"),
            ("status_glyphs", "a hand-made status (use status_header)"),
        ] {
            if code.contains(needle) {
                return Some(why);
            }
        }
        None
    }

    /// Every `.rs` file under `src/` outside `theme/`, with its text.
    fn sources_outside_the_theme() -> Vec<(std::path::PathBuf, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let theme_dir = root.join("theme");
        let mut stack = vec![root];
        let mut out = Vec::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    if path != theme_dir {
                        stack.push(path);
                    }
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    out.push((path, text));
                }
            }
        }
        out
    }

    #[test]
    fn the_status_scan_catches_what_it_should() {
        for bad in [
            "ui.colored_label(c, \"unknown\");",
            "let c = theme::warn(ui);",
            "theme::bad(ui),",
            "let c = theme::color(theme::palette(ui).status.of(status));",
            "let c = color(pal.status.fail);",
            "ui.visuals().warn_fg_color",
            "format!(\"{} {}\", s.glyph_char(), s.word())",
            "if theme::active(ui.ctx()).status_glyphs {",
        ] {
            assert!(status_outside_the_component(bad).is_some(), "{bad}");
        }
        for good in [
            "theme::status(ui, Status::Fail, &e.status);",
            "theme::status_line(ui, Status::Warn, w);",
            "let title = theme::status_header(ui, status, &text);",
            "let first = tab.status.as_ref().unwrap();",
            "// ui.colored_label(c, x) was here",
            "RichText::new(code).monospace().color(theme::code(ui))",
        ] {
            assert!(status_outside_the_component(good).is_none(), "{good}");
        }
    }

    /// M6.V4: status is never color-only. Outside `theme/`, no text is drawn in a status
    /// color except through the status component (`status`, `status_line`,
    /// `status_header`, `status_painted`), which adds the glyph and the word.
    #[test]
    fn no_status_text_is_colored_outside_the_status_component() {
        let sources = sources_outside_the_theme();
        assert!(sources.len() >= 20, "scanned {} files", sources.len());
        let mut problems = Vec::new();
        for (path, text) in &sources {
            for (n, line) in text.lines().enumerate() {
                if let Some(why) = status_outside_the_component(line) {
                    problems.push(format!(
                        "{}:{}: {why}: {}",
                        path.display(),
                        n + 1,
                        line.trim()
                    ));
                }
            }
        }
        assert!(problems.is_empty(), "{problems:#?}");
    }

    /// Every shape painted by one frame of `ctx`, flattened.
    fn shapes_of(out: &egui::FullOutput) -> Vec<egui::Shape> {
        fn flat(s: &egui::Shape, into: &mut Vec<egui::Shape>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, into)),
                other => into.push(other.clone()),
            }
        }
        let mut v = Vec::new();
        for c in &out.shapes {
            flat(&c.shape, &mut v);
        }
        v
    }

    /// M6.V4: the component draws the glyph (a painted shape, no font) and the word, both
    /// in the status color, for every status in both modes of both token presets; the
    /// plain-text forms lead with the glyph's character and the word.
    #[test]
    fn the_status_component_draws_glyph_word_and_color() {
        for preset in [Preset::Workbench, Preset::Studio] {
            for mode in [Mode::Light, Mode::Dark] {
                let ctx = egui::Context::default();
                apply(&ctx, &preset.theme(), mode);
                for s in Status::ALL {
                    let mut header = None;
                    let mut painted = None;
                    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                        status(ui, s, "");
                        header = Some(status_header(ui, s, "rule").text().to_string());
                        painted = Some(status_painted(ui, s, "viewport"));
                    });
                    let want = color(preset.theme().palette(mode == Mode::Dark).status.of(s));
                    let shapes = shapes_of(&out);
                    let word = shapes.iter().any(|sh| match sh {
                        egui::Shape::Text(t) => t.galley.text() == s.word(),
                        _ => false,
                    });
                    let glyph = shapes.iter().any(|sh| match sh {
                        egui::Shape::Circle(c) => c.fill == want || c.stroke.color == want,
                        egui::Shape::LineSegment { stroke, .. } => stroke.color == want,
                        egui::Shape::Path(p) => {
                            p.fill == want || p.stroke.color == egui::epaint::ColorMode::Solid(want)
                        }
                        egui::Shape::Rect(r) => r.stroke.color == want,
                        _ => false,
                    });
                    assert!(word, "{preset:?} {mode:?} {s:?}: no word");
                    assert!(
                        glyph,
                        "{preset:?} {mode:?} {s:?}: no glyph in the status color"
                    );
                    let lead = format!("{} {}", s.glyph_char(), s.word().to_uppercase());
                    assert!(header.unwrap().starts_with(&lead));
                    let (line, c) = painted.unwrap();
                    assert!(line.starts_with(&lead), "{line}");
                    assert_eq!(c, want);
                    out.textures_delta.clear();
                }
            }
        }
    }

    /// The tokens `tokens.rs` defines, as the style guide names them: each field of the
    /// token structs as `group.field`, and the palette's own colors bare (`accent`).
    fn defined_tokens() -> std::collections::BTreeSet<String> {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join("theme")
                .join("tokens.rs"),
        )
        .unwrap();
        let groups = [
            ("TypeScale", "type_scale."),
            ("Spacing", "spacing."),
            ("Sizes", "sizes."),
            ("Radii", "radii."),
            ("Density", "density."),
            ("Surfaces", "surface."),
            ("TextLevels", "text."),
            ("StatusColors", "status."),
            ("Palette", ""),
        ];
        let mut out = std::collections::BTreeSet::new();
        for (name, prefix) in groups {
            let start = src
                .find(&format!("pub struct {name} {{"))
                .unwrap_or_else(|| panic!("no struct {name}"));
            let body = &src[start..];
            let body = &body[..body.find("\n}").unwrap()];
            for line in body.lines().skip(1) {
                let Some(field) = line.trim().strip_prefix("pub ") else {
                    continue;
                };
                let Some((field, ty)) = field.split_once(':') else {
                    continue;
                };
                // The palette's own colors only; its groups are listed above.
                if prefix.is_empty() && ty.trim().trim_end_matches(',') != "Rgb" {
                    continue;
                }
                out.insert(format!("{prefix}{}", field.trim()));
            }
        }
        out
    }

    /// M6.V6: `docs/design/workbench-style.md` names every token `tokens.rs` defines (in
    /// backticks), and every `group.name` it names in a token group exists.
    #[test]
    fn the_style_guide_names_every_token_and_only_tokens() {
        let doc = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("docs")
                .join("design")
                .join("workbench-style.md"),
        )
        .unwrap();
        let defined = defined_tokens();
        assert!(defined.len() >= 50, "{defined:?}");
        for t in [
            "type_scale.mono",
            "status.not_run",
            "accent",
            "control_border",
        ] {
            assert!(defined.contains(t), "{t} not read from tokens.rs");
        }
        let named: Vec<&str> = doc.split('`').skip(1).step_by(2).map(str::trim).collect();
        let missing: Vec<&String> = defined
            .iter()
            .filter(|t| !named.contains(&t.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "tokens the style guide does not name: {missing:?}"
        );
        let groups = [
            "type_scale.",
            "spacing.",
            "sizes.",
            "radii.",
            "density.",
            "surface.",
            "text.",
            "status.",
        ];
        let unknown: Vec<&&str> = named
            .iter()
            .filter(|n| {
                groups.iter().any(|g| n.starts_with(g))
                    && n.chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_' || c == '.')
                    && !defined.contains(**n)
            })
            .collect();
        assert!(
            unknown.is_empty(),
            "the style guide names tokens tokens.rs does not define: {unknown:?}"
        );
    }
}
