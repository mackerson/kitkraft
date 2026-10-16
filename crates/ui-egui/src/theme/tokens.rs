//! The KitKraft visual tokens as data (M6.V1, M6.V3): type scale, spacing scale, sizes,
//! radii, table density and a light and a dark palette per preset. `Workbench` is the
//! owner's chosen direction (M6.O1, 2026-10-16) and the default. Nothing here names the toolkit;
//! [`super::apply`] turns a [`Theme`] into egui's `Style`. The rules that bind the values
//! (status never color-only, AA contrast, density) are in
//! `docs/design/workbench-visual-system.md`.

/// An sRGB color, 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// `#rrggbb`, for the docs and the Start tab.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// WCAG 2.x relative luminance of an sRGB color (0 black, 1 white).
pub fn relative_luminance(c: Rgb) -> f64 {
    let channel = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.040_45 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.0) + 0.7152 * channel(c.1) + 0.0722 * channel(c.2)
}

/// WCAG 2.x contrast ratio of two colors, 1 to 21, whichever is lighter.
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// WCAG AA for normal-size text.
pub const AA_TEXT: f64 = 4.5;

/// WCAG AA for non-text contrast (1.4.11): control outlines and the focus ring.
pub const AA_NON_TEXT: f64 = 3.0;

/// The window's inner size at start, in points (1400 x 900; the screenshot tour uses
/// the same size).
pub const WINDOW: [f32; 2] = [1400.0, 900.0];

/// The spacing scale in points: a 4 pt grid with one 2 pt half step. Every spacing token
/// below is one of these values (a test checks it).
pub const SPACE: [f32; 8] = [0.0, 2.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0];

/// Text sizes in points (100 percent scaling: 1 pt is 1 px). Fonts stay the Windows
/// system fonts, Segoe UI and Consolas, until the owner rules on M6.O2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeScale {
    /// Notes, hints, the "ms" line.
    pub small: f32,
    /// Labels and table cells.
    pub body: f32,
    /// Button text.
    pub button: f32,
    /// FormIDs, offsets, `kit` lines (Consolas).
    pub mono: f32,
    /// A tab's title.
    pub heading: f32,
    /// A section title inside a tab (level-3 headings in Help).
    pub subheading: f32,
}

/// Fixed sizes of form fields, pickers and scroll views, in points or table rows. A
/// panel names one of these and never a number of its own (a test scans the panels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizes {
    /// A record signature (`WEAP`).
    pub field_tiny: f32,
    /// A filter, a member name, a short list.
    pub field_short: f32,
    /// A form (`Plugin.esm:XXXXXX`), a plugin name.
    pub field_medium: f32,
    /// A path, a `where` clause.
    pub field_long: f32,
    /// The Load and Ledger forms' text fields.
    pub field_wide: f32,
    /// The "loaded..." plugin picker.
    pub combo: f32,
    /// A short scrolling table (record types), in rows.
    pub view_short_rows: f32,
    /// A medium scrolling table (query results, references), in rows.
    pub view_medium_rows: f32,
    /// A tall scrolling table (load order, links, planet nodes, raw JSON), in rows.
    pub view_tall_rows: f32,
    /// A Help table cell with one or two columns.
    pub doc_cell_wide: f32,
    /// A Help table cell with three or four columns.
    pub doc_cell_mid: f32,
    /// A Help table cell with five or more columns.
    pub doc_cell_narrow: f32,
    /// The Records panes' narrowest width.
    pub pane_min: f32,
    /// The Records inspector's share of the tab's width until it is dragged (then
    /// `records_split` in `ui.json`).
    pub split_default: f32,
}

/// Spacing tokens in points, each a [`SPACE`] value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Gap between widgets on a line.
    pub item_x: f32,
    /// Gap between lines.
    pub item_y: f32,
    /// Button padding, horizontal.
    pub button_x: f32,
    /// Button padding, vertical.
    pub button_y: f32,
    /// Inner margin of the central panel.
    pub panel_margin: f32,
    /// Indent of a collapsing section.
    pub indent: f32,
    /// Space between two blocks of a panel (a form and its result list).
    pub block: f32,
}

/// Corner radii in points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Radii {
    /// Buttons, inputs, selectable tabs.
    pub control: u8,
    /// Windows, menus, popups.
    pub panel: u8,
}

/// How tables are laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Density {
    /// `compact` or `comfortable`.
    pub name: &'static str,
    /// Minimum row height in points (also the minimum height of a control).
    pub row_height: f32,
    /// Gap between cells, horizontal.
    pub cell_x: f32,
    /// Gap between rows.
    pub cell_y: f32,
    /// Alternate rows on the `zebra` surface.
    pub zebra: bool,
}

/// Background levels. Every text and status color is checked against each of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surfaces {
    /// The panel background.
    pub base: Rgb,
    /// Buttons, popups, windows.
    pub raised: Rgb,
    /// Text fields, scroll wells, code.
    pub sunken: Rgb,
    /// Every other table row when density says zebra.
    pub zebra: Rgb,
    /// A hovered control.
    pub hover: Rgb,
    /// A selected tab, row or text.
    pub selection: Rgb,
}

/// Text levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextLevels {
    /// Headings, selected and hovered controls.
    pub strong: Rgb,
    /// Labels and cells.
    pub body: Rgb,
    /// Notes (`weak` text): still AA on every surface.
    pub muted: Rgb,
}

/// The five result statuses. A status is drawn as glyph, word and color together
/// ([`Status::glyph`], [`Status::word`]); color is never the only cue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The check ran and passed.
    Pass,
    /// The check ran and failed (an error finding, a mismatch, an unresolved FormID).
    Fail,
    /// A warning finding, an unknown, an unconfirmed fact.
    Warn,
    /// The check was not run.
    NotRun,
    /// It ran, but the evidence is missing or ambiguous.
    Inconclusive,
    /// A value the reader could not establish (a scale, a definition): reported as
    /// unknown, never as absent.
    Unknown,
}

impl Status {
    /// All six, in legend order.
    pub const ALL: [Status; 6] = [
        Status::Pass,
        Status::Fail,
        Status::Warn,
        Status::NotRun,
        Status::Inconclusive,
        Status::Unknown,
    ];

    /// The word shown beside the glyph (the ledger's spelling).
    pub fn word(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Fail => "fail",
            Status::Warn => "warning",
            Status::NotRun => "not-run",
            Status::Inconclusive => "inconclusive",
            Status::Unknown => "unknown",
        }
    }

    /// The glyph's shape, painted with egui's painter (no icon font): a filled disc, a
    /// cross, a triangle, an empty ring, a diamond and an empty square. A Unicode stand-in is in
    /// [`Status::glyph_char`] for plain text.
    pub fn glyph(self) -> Glyph {
        match self {
            Status::Pass => Glyph::Disc,
            Status::Fail => Glyph::Cross,
            Status::Warn => Glyph::Triangle,
            Status::NotRun => Glyph::Ring,
            Status::Inconclusive => Glyph::Diamond,
            Status::Unknown => Glyph::Square,
        }
    }

    /// The glyph as a character (copied text, docs).
    pub fn glyph_char(self) -> char {
        match self.glyph() {
            Glyph::Disc => '\u{25CF}',
            Glyph::Cross => '\u{00D7}',
            Glyph::Triangle => '\u{25B2}',
            Glyph::Ring => '\u{25CB}',
            Glyph::Diamond => '\u{25CA}',
            Glyph::Square => '\u{25A1}',
        }
    }
}

/// A status glyph's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// Filled circle.
    Disc,
    /// Diagonal cross.
    Cross,
    /// Upward triangle.
    Triangle,
    /// Empty circle.
    Ring,
    /// Diamond outline with a dot.
    Diamond,
    /// Empty square.
    Square,
}

/// Status colors, used for the glyph and the word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusColors {
    pub pass: Rgb,
    pub fail: Rgb,
    pub warn: Rgb,
    pub not_run: Rgb,
    pub inconclusive: Rgb,
    pub unknown: Rgb,
}

impl StatusColors {
    /// The color of `status`.
    pub fn of(&self, status: Status) -> Rgb {
        match status {
            Status::Pass => self.pass,
            Status::Fail => self.fail,
            Status::Warn => self.warn,
            Status::NotRun => self.not_run,
            Status::Inconclusive => self.inconclusive,
            Status::Unknown => self.unknown,
        }
    }
}

/// One mode's colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub surface: Surfaces,
    pub text: TextLevels,
    /// Links and the focus of a hovered or pressed control.
    pub accent: Rgb,
    /// Separators, grid lines and window outlines (decorative: no contrast rule).
    pub border: Rgb,
    /// The outline of an interactive control (button, field, picker): 3:1 on every
    /// surface, so a control is found by its edge and not only by its fill.
    pub control_border: Rgb,
    /// The focus ring of the control that has keyboard focus: 3:1 on every surface.
    pub focus: Rgb,
    /// Code in the Help tab.
    pub code: Rgb,
    pub status: StatusColors,
}

impl Palette {
    /// Every background text can sit on, with its token name.
    pub fn surfaces(&self) -> [(&'static str, Rgb); 6] {
        let s = self.surface;
        [
            ("surface.base", s.base),
            ("surface.raised", s.raised),
            ("surface.sunken", s.sunken),
            ("surface.zebra", s.zebra),
            ("surface.hover", s.hover),
            ("surface.selection", s.selection),
        ]
    }
}

/// Light, dark, or whatever Windows is set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Follow the system (egui's default, today's behaviour).
    System,
    Light,
    Dark,
}

/// The presets. `Workbench` is the owner's choice (M6.O1, 2026-10-16) and the default;
/// `Studio` is the other mockup direction (M6.V2), kept selectable; `Classic` is the
/// pre-token look (egui's default style), kept selectable as legacy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Classic,
    Workbench,
    Studio,
}

impl Preset {
    /// All presets, the default first and the legacy one last.
    pub const ALL: [Preset; 3] = [Preset::Workbench, Preset::Studio, Preset::Classic];

    /// The name `--theme` and `ui.json` use.
    pub fn id(self) -> &'static str {
        match self {
            Preset::Classic => "classic",
            Preset::Workbench => "workbench",
            Preset::Studio => "studio",
        }
    }

    /// Kept only for comparison: the pre-token look (`Classic`).
    pub fn legacy(self) -> bool {
        self == Preset::Classic
    }

    /// The preset's tokens.
    pub fn theme(self) -> Theme {
        match self {
            Preset::Classic => classic(),
            Preset::Workbench => workbench(),
            Preset::Studio => studio(),
        }
    }
}

/// A complete token set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub preset: Preset,
    /// Display name ("Workbench").
    pub name: &'static str,
    /// One line on the direction.
    pub summary: &'static str,
    /// `false` for `Classic` only: egui's default style is kept as it is, and only the
    /// status colors below are used (today's `WARN`, `BAD`, `GOOD` and `CODE`).
    pub restyle: bool,
    /// Paint the status glyph before the word. `Classic` shows the word only, as today.
    pub status_glyphs: bool,
    pub type_scale: TypeScale,
    pub spacing: Spacing,
    pub sizes: Sizes,
    pub radii: Radii,
    pub density: Density,
    pub light: Palette,
    pub dark: Palette,
}

impl Theme {
    /// The palette of a mode (`dark` true for dark).
    pub fn palette(&self, dark: bool) -> &Palette {
        if dark {
            &self.dark
        } else {
            &self.light
        }
    }
}

/// The sizes every preset starts from (Studio widens the fields).
const SIZES: Sizes = Sizes {
    field_tiny: 60.0,
    field_short: 140.0,
    field_medium: 260.0,
    field_long: 420.0,
    field_wide: 560.0,
    combo: 112.0,
    view_short_rows: 10.0,
    view_medium_rows: 14.0,
    view_tall_rows: 19.0,
    doc_cell_wide: 520.0,
    doc_cell_mid: 360.0,
    doc_cell_narrow: 232.0,
    pane_min: 320.0,
    split_default: 0.45,
};

const fn rgb(hex: u32) -> Rgb {
    Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// The legacy look: egui 0.36's default style, and the four colors `app.rs` had as
/// constants (`WARN`, `BAD`, `GOOD`, `CODE`), the same in both modes. Its light mode does
/// not meet AA for the warning and pass colors; it is kept unchanged on purpose for
/// comparison (selectable, marked legacy), and the contrast tests cover the two token
/// presets only.
pub fn classic() -> Theme {
    // egui 0.36 defaults (`Style::default()`, `Visuals::dark()` / `light()`), recorded so
    // the docs can show what the presets change.
    let status = StatusColors {
        pass: rgb(0x6ebe6e),
        fail: rgb(0xe65a50),
        warn: rgb(0xe6aa3c),
        not_run: rgb(0x8c8c8c),
        inconclusive: rgb(0x8c8c8c),
        unknown: rgb(0xe6aa3c),
    };
    let dark = Palette {
        surface: Surfaces {
            base: rgb(0x1b1b1b),
            raised: rgb(0x3c3c3c),
            sunken: rgb(0x0a0a0a),
            zebra: rgb(0x202020),
            hover: rgb(0x464646),
            selection: rgb(0x00436f),
        },
        text: TextLevels {
            strong: rgb(0xffffff),
            body: rgb(0x8c8c8c),
            muted: rgb(0x6e6e6e),
        },
        accent: rgb(0x5aaaff),
        border: rgb(0x3c3c3c),
        control_border: rgb(0x3c3c3c),
        focus: rgb(0xc0deff),
        code: rgb(0x8cbeeb),
        status,
    };
    let light = Palette {
        surface: Surfaces {
            base: rgb(0xf8f8f8),
            raised: rgb(0xe6e6e6),
            sunken: rgb(0xffffff),
            zebra: rgb(0xf3f3f3),
            hover: rgb(0xdcdcdc),
            selection: rgb(0x90d1ff),
        },
        text: TextLevels {
            strong: rgb(0x000000),
            body: rgb(0x505050),
            muted: rgb(0x8c8c8c),
        },
        accent: rgb(0x009bff),
        border: rgb(0xbebebe),
        control_border: rgb(0xbebebe),
        focus: rgb(0x005299),
        code: rgb(0x8cbeeb),
        status,
    };
    Theme {
        preset: Preset::Classic,
        name: "Classic",
        summary: "legacy: egui's default style from before the tokens, kept for comparison; below AA contrast in light mode",
        restyle: false,
        status_glyphs: false,
        type_scale: TypeScale {
            small: 9.0,
            body: 13.0,
            button: 13.0,
            mono: 13.0,
            heading: 18.0,
            subheading: 16.0,
        },
        spacing: Spacing {
            item_x: 8.0,
            item_y: 3.0,
            button_x: 4.0,
            button_y: 1.0,
            panel_margin: 8.0,
            indent: 18.0,
            block: 6.0,
        },
        sizes: SIZES,
        radii: Radii {
            control: 2,
            panel: 6,
        },
        density: Density {
            name: "compact",
            row_height: 18.0,
            cell_x: 8.0,
            cell_y: 3.0,
            zebra: true,
        },
        light,
        dark,
    }
}

/// "Workbench", the owner's choice (M6.O1) and the default: neutral graphite and cool
/// grey, dense, an instrument panel. 13 pt body, 20 pt zebra rows, square corners, one
/// steel-blue accent; designed dark first.
pub fn workbench() -> Theme {
    Theme {
        preset: Preset::Workbench,
        name: "Workbench",
        summary: "the default: neutral and dense, an instrument panel, compact zebra tables, square corners, steel-blue accent",
        restyle: true,
        status_glyphs: true,
        type_scale: TypeScale {
            small: 11.0,
            body: 13.0,
            button: 13.0,
            mono: 12.5,
            heading: 16.0,
            subheading: 14.0,
        },
        spacing: Spacing {
            item_x: 8.0,
            item_y: 4.0,
            button_x: 8.0,
            button_y: 2.0,
            panel_margin: 8.0,
            indent: 16.0,
            block: 8.0,
        },
        sizes: SIZES,
        radii: Radii {
            control: 2,
            panel: 4,
        },
        density: Density {
            name: "compact",
            row_height: 20.0,
            cell_x: 12.0,
            cell_y: 2.0,
            zebra: true,
        },
        dark: Palette {
            surface: Surfaces {
                base: rgb(0x15171a),
                raised: rgb(0x22262b),
                sunken: rgb(0x0f1113),
                zebra: rgb(0x1b1e22),
                hover: rgb(0x2b3036),
                selection: rgb(0x1f3a52),
            },
            text: TextLevels {
                strong: rgb(0xf0f2f4),
                body: rgb(0xc9ced4),
                muted: rgb(0x9aa3ad),
            },
            accent: rgb(0x6cb4ee),
            border: rgb(0x343a41),
            control_border: rgb(0x7d8793),
            focus: rgb(0x6cb4ee),
            code: rgb(0x8fc6f2),
            status: StatusColors {
                pass: rgb(0x4fc98a),
                fail: rgb(0xff7a70),
                warn: rgb(0xf2b33d),
                not_run: rgb(0xa3acb6),
                inconclusive: rgb(0xc49be8),
                unknown: rgb(0x5fcfd8),
            },
        },
        light: Palette {
            surface: Surfaces {
                base: rgb(0xf3f4f6),
                raised: rgb(0xffffff),
                sunken: rgb(0xe9ebee),
                zebra: rgb(0xeaecef),
                hover: rgb(0xe1e5ea),
                selection: rgb(0xd3e3f3),
            },
            text: TextLevels {
                strong: rgb(0x111417),
                body: rgb(0x30363d),
                muted: rgb(0x4f5862),
            },
            accent: rgb(0x1d5c94),
            border: rgb(0xc3c9d0),
            control_border: rgb(0x6f7985),
            focus: rgb(0x1d5c94),
            code: rgb(0x1d5c94),
            status: StatusColors {
                pass: rgb(0x1a6b37),
                fail: rgb(0xb42318),
                warn: rgb(0x7a5000),
                not_run: rgb(0x4f5862),
                inconclusive: rgb(0x6a3d9e),
                unknown: rgb(0x0b6470),
            },
        },
    }
}

/// "Studio": warm paper and umber, more air, stronger hierarchy. Larger type and
/// headings, 28 pt rows without zebra, rounded corners, a terracotta accent; designed
/// light first.
pub fn studio() -> Theme {
    Theme {
        preset: Preset::Studio,
        name: "Studio",
        summary: "warm and airy: larger type and headings, comfortable rows without zebra, rounded corners, terracotta accent",
        restyle: true,
        status_glyphs: true,
        type_scale: TypeScale {
            small: 12.0,
            body: 14.0,
            button: 14.0,
            mono: 13.5,
            heading: 22.0,
            subheading: 17.0,
        },
        spacing: Spacing {
            item_x: 12.0,
            item_y: 8.0,
            button_x: 12.0,
            button_y: 4.0,
            panel_margin: 16.0,
            indent: 24.0,
            block: 12.0,
        },
        sizes: Sizes {
            field_tiny: 72.0,
            field_short: 160.0,
            field_medium: 300.0,
            field_long: 460.0,
            field_wide: 600.0,
            combo: 128.0,
            ..SIZES
        },
        radii: Radii {
            control: 6,
            panel: 8,
        },
        density: Density {
            name: "comfortable",
            row_height: 28.0,
            cell_x: 16.0,
            cell_y: 4.0,
            zebra: false,
        },
        light: Palette {
            surface: Surfaces {
                base: rgb(0xfaf6f0),
                raised: rgb(0xfffdf9),
                sunken: rgb(0xf1eadf),
                zebra: rgb(0xf5f0e8),
                hover: rgb(0xebe3d6),
                selection: rgb(0xf3dccb),
            },
            text: TextLevels {
                strong: rgb(0x231e1a),
                body: rgb(0x3f3731),
                muted: rgb(0x5c5249),
            },
            accent: rgb(0x963f1f),
            border: rgb(0xdccfbf),
            control_border: rgb(0x806f5c),
            focus: rgb(0x963f1f),
            code: rgb(0x7a3a1c),
            status: StatusColors {
                pass: rgb(0x2a6b33),
                fail: rgb(0xa8261a),
                warn: rgb(0x7a4d00),
                not_run: rgb(0x5c5249),
                inconclusive: rgb(0x6d3c91),
                unknown: rgb(0x1f6470),
            },
        },
        dark: Palette {
            surface: Surfaces {
                base: rgb(0x1f1b17),
                raised: rgb(0x2a251f),
                sunken: rgb(0x181512),
                zebra: rgb(0x24201b),
                hover: rgb(0x352f28),
                selection: rgb(0x4a3424),
            },
            text: TextLevels {
                strong: rgb(0xf7f1e9),
                body: rgb(0xdcd3c7),
                muted: rgb(0xb0a598),
            },
            accent: rgb(0xeba571),
            border: rgb(0x40382f),
            control_border: rgb(0x948572),
            focus: rgb(0xeba571),
            code: rgb(0xf0b68a),
            status: StatusColors {
                pass: rgb(0x72d192),
                fail: rgb(0xff8f80),
                warn: rgb(0xf0bf5a),
                not_run: rgb(0xb3a99b),
                inconclusive: rgb(0xd3aae9),
                unknown: rgb(0x7fd6d2),
            },
        },
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn token_presets() -> [Theme; 2] {
        [workbench(), studio()]
    }

    #[test]
    fn luminance_and_contrast_match_the_wcag_reference_points() {
        assert!((relative_luminance(Rgb(0, 0, 0)) - 0.0).abs() < 1e-9);
        assert!((relative_luminance(Rgb(255, 255, 255)) - 1.0).abs() < 1e-9);
        assert!((contrast_ratio(Rgb(0, 0, 0), Rgb(255, 255, 255)) - 21.0).abs() < 1e-9);
        // #767676 on white is the classic 4.54:1 AA boundary grey.
        let r = contrast_ratio(Rgb(0x76, 0x76, 0x76), Rgb(255, 255, 255));
        assert!((r - 4.54).abs() < 0.01, "{r}");
        assert_eq!(
            contrast_ratio(Rgb(1, 2, 3), Rgb(200, 100, 50)),
            contrast_ratio(Rgb(200, 100, 50), Rgb(1, 2, 3))
        );
    }

    /// M6.V1: every status color is AA text on every surface, light and dark.
    #[test]
    fn every_status_color_meets_aa_on_every_surface_in_both_modes() {
        let mut failures = Vec::new();
        for theme in token_presets() {
            for (mode, pal) in [("light", &theme.light), ("dark", &theme.dark)] {
                for status in Status::ALL {
                    for (surface, bg) in pal.surfaces() {
                        let r = contrast_ratio(pal.status.of(status), bg);
                        if r < AA_TEXT {
                            failures.push(format!(
                                "{} {mode}: status {} {} on {surface} {}: {r:.2}",
                                theme.name,
                                status.word(),
                                pal.status.of(status).hex(),
                                bg.hex()
                            ));
                        }
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn text_accent_and_code_meet_aa_on_every_surface_in_both_modes() {
        let mut failures = Vec::new();
        for theme in token_presets() {
            for (mode, pal) in [("light", &theme.light), ("dark", &theme.dark)] {
                let fgs = [
                    ("text.strong", pal.text.strong),
                    ("text.body", pal.text.body),
                    ("text.muted", pal.text.muted),
                    ("accent", pal.accent),
                    ("code", pal.code),
                ];
                for (name, fg) in fgs {
                    for (surface, bg) in pal.surfaces() {
                        let r = contrast_ratio(fg, bg);
                        if r < AA_TEXT {
                            failures.push(format!(
                                "{} {mode}: {name} {} on {surface} {}: {r:.2}",
                                theme.name,
                                fg.hex(),
                                bg.hex()
                            ));
                        }
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// M6.V3: control outlines and the focus ring are 3:1 on every surface (WCAG 1.4.11),
    /// light and dark.
    #[test]
    fn control_border_and_focus_meet_non_text_contrast_in_both_modes() {
        let mut failures = Vec::new();
        for theme in token_presets() {
            for (mode, pal) in [("light", &theme.light), ("dark", &theme.dark)] {
                for (name, fg) in [("control_border", pal.control_border), ("focus", pal.focus)] {
                    for (surface, bg) in pal.surfaces() {
                        let r = contrast_ratio(fg, bg);
                        if r < AA_NON_TEXT {
                            failures.push(format!(
                                "{} {mode}: {name} {} on {surface} {}: {r:.2}",
                                theme.name,
                                fg.hex(),
                                bg.hex()
                            ));
                        }
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        // The rule has teeth: the decorative border is far below it.
        let w = workbench();
        assert!(contrast_ratio(w.dark.border, w.dark.surface.base) < AA_NON_TEXT);
    }

    /// Status is never color-only: each status has its own glyph and its own word.
    #[test]
    fn statuses_differ_by_glyph_and_word_not_only_color() {
        for (i, a) in Status::ALL.iter().enumerate() {
            for b in Status::ALL.iter().skip(i + 1) {
                assert_ne!(a.glyph(), b.glyph(), "{a:?} {b:?}");
                assert_ne!(a.glyph_char(), b.glyph_char(), "{a:?} {b:?}");
                assert_ne!(a.word(), b.word(), "{a:?} {b:?}");
            }
        }
        for t in token_presets() {
            assert!(t.status_glyphs, "{}", t.name);
        }
    }

    #[test]
    fn spacing_tokens_sit_on_the_scale_and_presets_are_distinct() {
        for theme in token_presets() {
            let s = theme.spacing;
            let d = theme.density;
            for v in [
                s.item_x,
                s.item_y,
                s.button_x,
                s.button_y,
                s.panel_margin,
                s.indent,
                s.block,
                d.cell_x,
                d.cell_y,
            ] {
                assert!(
                    SPACE.contains(&v) || (v > 0.0 && v % 4.0 == 0.0),
                    "{}: {v}",
                    theme.name
                );
            }
            assert_eq!(d.row_height % 2.0, 0.0, "{}", theme.name);
            assert!(theme.type_scale.small < theme.type_scale.body);
            assert!(theme.type_scale.body < theme.type_scale.subheading);
            assert!(theme.type_scale.subheading <= theme.type_scale.heading);
            let z = theme.sizes;
            for v in [
                z.field_tiny,
                z.field_short,
                z.field_medium,
                z.field_long,
                z.field_wide,
                z.combo,
                z.doc_cell_wide,
                z.doc_cell_mid,
                z.doc_cell_narrow,
                z.pane_min,
            ] {
                assert!(v > 0.0 && v % 4.0 == 0.0, "{}: size {v}", theme.name);
            }
            assert!(z.field_tiny < z.field_short && z.field_short < z.field_medium);
            assert!(z.field_medium < z.field_long && z.field_long < z.field_wide);
            assert!(z.view_short_rows < z.view_medium_rows);
            assert!(z.view_medium_rows < z.view_tall_rows);
            assert!(z.split_default > 0.2 && z.split_default < 0.8);
        }
        let ids: Vec<&str> = Preset::ALL.iter().map(|p| p.id()).collect();
        assert_eq!(ids, ["workbench", "studio", "classic"]);
        assert!(Preset::Classic.legacy());
        assert!(!Preset::Workbench.legacy() && !Preset::Studio.legacy());
        for p in Preset::ALL {
            assert_eq!(p.theme().preset, p);
        }
        assert_ne!(workbench().density, studio().density);
        assert_ne!(workbench().light, studio().light);
    }

    /// The check has teeth: today's warning color on egui's light panel is well under AA,
    /// which is why `Classic` is outside the token presets' contrast rule.
    #[test]
    fn classic_light_fails_aa_as_measured() {
        let c = classic();
        let r = contrast_ratio(c.light.status.warn, c.light.surface.base);
        assert!(r < 2.5, "{r}");
        assert!(contrast_ratio(c.light.status.pass, c.light.surface.base) < AA_TEXT);
    }

    #[test]
    fn classic_keeps_todays_constants() {
        let c = classic();
        assert!(!c.restyle && !c.status_glyphs);
        for pal in [&c.light, &c.dark] {
            assert_eq!(pal.status.warn, Rgb(230, 170, 60));
            assert_eq!(pal.status.fail, Rgb(230, 90, 80));
            assert_eq!(pal.status.pass, Rgb(110, 190, 110));
            assert_eq!(pal.code, Rgb(140, 190, 235));
        }
    }
}
