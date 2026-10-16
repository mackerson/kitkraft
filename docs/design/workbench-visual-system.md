# Workbench visual system: tokens and rules (M6 lane V)

**Status: tokens landed (M6.V1, as the director scoped it 2026-10-16), direction open.**
The owner picks a direction from the two mockups in
[`mockups/2026-10-16/README.md`](mockups/2026-10-16/README.md) (M6.O1). Until then the
window's default stays today's look (`classic`). After M6.O1, M6.V3 to M6.V6 finish the
system on the chosen preset; the full style guide is `docs/design/workbench-style.md`
(M6.V6).

Why: the owner's pains on 2026-10-16 were "look, feel, performance". The director's brief
named plain egui styling, cramped tables and no theming. The tokens below are the answer
for look and feel. They are data, in one place, so a direction is chosen by values and
not by editing panels.

## Where they live

| Path | What |
| ---- | ---- |
| `crates/ui-egui/src/theme/tokens.rs` | The tokens as plain data (`Theme`, `Palette`, `TypeScale`, `Spacing`, `Radii`, `Density`, `Status`), the three presets, WCAG luminance and contrast, and the tests. No egui type in it. |
| `crates/ui-egui/src/theme/mod.rs` | `apply(ctx, &Theme, Mode)`: the only function that writes tokens into egui's `Style` and `Visuals`. Also what panels draw with: `palette`, `warn`/`bad`/`good`/`code`, the status component `status`, the table style `table`, and the panel frames. `ThemeChoice` parses `--theme`. |
| `apps/kit/src/main.rs` | `kit-ui --theme <preset>` (so `kit ui --theme <preset>`). |
| `%LOCALAPPDATA%\kitkraft\ui.json` | `"theme": "<preset>"`, read at start. `--theme` wins over it. The window never writes it (M6.V3 adds a picker after M6.O1). |

A preset is spelled `classic`, `workbench` or `studio`, with an optional `-light` or
`-dark` suffix (for example `studio-dark`). Without a suffix the window follows Windows'
light or dark setting, as egui does today. An unknown name on the command line is a usage
error (exit 2). An unknown name in `ui.json` opens the window in `classic` with a notice.
The Start tab names the active preset, its mode, and where the choice came from, and shows
the five statuses as the preset draws them.

## The tokens

All sizes are in points; at 100 percent Windows scaling a point is a pixel. Fonts stay the
Windows system fonts, Segoe UI (proportional) and Consolas (monospace), until the owner
rules on M6.O2. Nothing is compiled in.

### Type scale

| Token | Classic (egui default) | Workbench | Studio | Used for |
| ----- | ---------------------: | --------: | -----: | -------- |
| `type_scale.small` | 9 | 11 | 12 | small notes |
| `type_scale.body` | 13 | 13 | 14 | labels, table cells |
| `type_scale.button` | 13 | 13 | 14 | buttons, tabs |
| `type_scale.mono` | 13 | 12.5 | 13.5 | FormIDs, offsets, `kit` lines |
| `type_scale.heading` | 18 | 16 | 22 | a tab's title |

### Spacing scale

`SPACE` = 0, 2, 4, 8, 12, 16, 24, 32: a 4-point grid with one 2-point half step. Every
spacing and density token is on it (a test checks it).

| Token | Classic | Workbench | Studio | egui field |
| ----- | ------: | --------: | -----: | ---------- |
| `spacing.item_x` | 8 | 8 | 12 | `spacing.item_spacing.x` |
| `spacing.item_y` | 3 | 4 | 8 | `spacing.item_spacing.y` |
| `spacing.button_x` | 4 | 8 | 12 | `spacing.button_padding.x` |
| `spacing.button_y` | 1 | 2 | 4 | `spacing.button_padding.y` |
| `spacing.panel_margin` | 8 | 8 | 16 | central panel inner margin, window margin |
| `spacing.indent` | 18 | 16 | 24 | collapsing section indent |

(Classic's values are egui 0.36's and are recorded only for comparison; they are not on the
scale.)

### Radii

| Token | Classic | Workbench | Studio |
| ----- | ------: | --------: | -----: |
| `radii.control` (buttons, inputs, tabs) | 2 | 2 | 6 |
| `radii.panel` (windows, menus) | 6 | 4 | 8 |

### Table density

| Token | Classic | Workbench | Studio |
| ----- | ------- | --------- | ------ |
| `density.name` | compact | compact | comfortable |
| `density.row_height` (also the minimum control height) | 18 | 20 | 28 |
| `density.cell_x` (gap between cells) | 8 | 12 | 16 |
| `density.cell_y` (gap between rows) | 3 | 2 | 4 |
| `density.zebra` | on | on | off |

### Palette

Each preset has a light and a dark palette with the same token names.

| Token | Meaning |
| ----- | ------- |
| `surface.base` | panel background |
| `surface.raised` | buttons, popups, windows, the top bar |
| `surface.sunken` | text fields, code wells |
| `surface.zebra` | every other table row (when `density.zebra`) |
| `surface.hover` | a hovered control |
| `surface.selection` | a selected tab, row or text |
| `text.strong` | headings, hovered and selected controls |
| `text.body` | labels and cells |
| `text.muted` | notes (`weak` text) |
| `accent` | links, the outline of a hovered or pressed control |
| `border` | separators, control and window outlines |
| `code` | code in the Help tab |
| `status.pass`, `status.fail`, `status.warn`, `status.not_run`, `status.inconclusive` | the five statuses |

| Token | Workbench light | Workbench dark | Studio light | Studio dark |
| ----- | --------------- | -------------- | ------------ | ----------- |
| `surface.base` | `#f3f4f6` | `#15171a` | `#faf6f0` | `#1f1b17` |
| `surface.raised` | `#ffffff` | `#22262b` | `#fffdf9` | `#2a251f` |
| `surface.sunken` | `#e9ebee` | `#0f1113` | `#f1eadf` | `#181512` |
| `surface.zebra` | `#eaecef` | `#1b1e22` | `#f5f0e8` | `#24201b` |
| `surface.hover` | `#e1e5ea` | `#2b3036` | `#ebe3d6` | `#352f28` |
| `surface.selection` | `#d3e3f3` | `#1f3a52` | `#f3dccb` | `#4a3424` |
| `text.strong` | `#111417` | `#f0f2f4` | `#231e1a` | `#f7f1e9` |
| `text.body` | `#30363d` | `#c9ced4` | `#3f3731` | `#dcd3c7` |
| `text.muted` | `#4f5862` | `#9aa3ad` | `#5c5249` | `#b0a598` |
| `accent` | `#1d5c94` | `#6cb4ee` | `#963f1f` | `#eba571` |
| `border` | `#c3c9d0` | `#343a41` | `#dccfbf` | `#40382f` |
| `code` | `#1d5c94` | `#8fc6f2` | `#7a3a1c` | `#f0b68a` |
| `status.pass` | `#1a6b37` | `#4fc98a` | `#2a6b33` | `#72d192` |
| `status.fail` | `#b42318` | `#ff7a70` | `#a8261a` | `#ff8f80` |
| `status.warn` | `#7a5000` | `#f2b33d` | `#7a4d00` | `#f0bf5a` |
| `status.not_run` | `#4f5862` | `#a3acb6` | `#5c5249` | `#b3a99b` |
| `status.inconclusive` | `#6a3d9e` | `#c49be8` | `#6d3c91` | `#d3aae9` |

Lowest contrast of each status color against the six surfaces (measured by the test's
own WCAG 2.x luminance code):

| Preset, mode | pass | fail | warn | not-run | inconclusive | muted text | accent |
| ------------ | ---: | ---: | ---: | ------: | -----------: | ---------: | -----: |
| Workbench light | 5.0 | 5.0 | 5.4 | 5.5 | 5.8 | 5.5 | 5.3 |
| Workbench dark | 5.6 | 4.6 | 6.3 | 5.1 | 5.2 | 4.6 | 5.3 |
| Studio light | 4.9 | 5.4 | 5.5 | 5.8 | 5.9 | 5.8 | 5.3 |
| Studio dark | 6.2 | 5.3 | 6.8 | 5.0 | 5.9 | 4.8 | 5.6 |

`classic` keeps today's four constants (`WARN #e6aa3c`, `BAD #e65a50`, `GOOD #6ebe6e`,
`CODE #8cbeeb`, the same in both modes) on egui's default style. On egui's light panel
(`#f8f8f8`) they measure warning 1.9, pass 2.1 and fail 3.3: below AA. That is the
problem the presets fix; `classic` stays as it is only until M6.O1, and the contrast rule
below covers the two token presets.

## Rules

1. **Status is never color only.** Pass, fail, warning, not-run and inconclusive are
   drawn by `theme::status(ui, Status, label)`: a painted glyph (filled disc, cross,
   triangle, empty ring, diamond; drawn with egui's painter, so no icon font is needed),
   then the word, both in the status color. A collapsing header, which takes plain text,
   leads with the status word instead (`PASS masters.missing: ...`). Two statuses never
   share a glyph or a word (test `statuses_differ_by_glyph_and_word_not_only_color`).
   `classic` shows the colored word only, as the window does today.
2. **AA contrast.** Every status color, text level, `accent` and `code` is at least 4.5:1
   against every surface (`base`, `raised`, `sunken`, `zebra`, `hover`, `selection`) in
   both modes of both token presets (tests `every_status_color_meets_aa_on_every_surface_in_both_modes`
   and `text_accent_and_code_meet_aa_on_every_surface_in_both_modes`; luminance is computed
   in the test, no dependency). A new color token is added to the palette and to these
   loops, never as a literal in a panel. (M6.V3 adds the scan for `Color32` literals
   outside the theme module, and the 3:1 rule for borders and the focus ring.)
3. **Density.** A table is `theme::table(ui, id)`, never a bare striped `Grid`: it takes
   the row height, cell gaps and zebra setting from `density`. Compact (Workbench) puts a
   row every 22 points; comfortable (Studio) every 32, about 30 percent fewer rows on
   screen (compare the two Records mockups). Numbers
   right-aligned, sticky headers and column resizing come with M6.V5 and M6.P4.
4. **Scaling.** Sizes are points; at 100 percent scaling the mockups are pixel-exact.
   Nothing in the tokens assumes a scale; 125 and 150 percent are lane X (deferred).
5. **One place.** Only `theme::apply` writes egui's style. A panel reads tokens through
   the helpers below and never sets a size or a color of its own.

## How a panel uses them

```rust
use crate::theme::{self, Status};

// Caution and error text: the palette's status colors for the current mode.
ui.colored_label(theme::warn(ui), "unknown");
ui.colored_label(theme::bad(ui), err.to_string());

// A status: glyph, word and color (the word may be a domain word such as "error").
theme::status(ui, Status::Pass, "PASSED");
theme::status(ui, Status::Fail, &finding.severity);

// A table: density's row height, cell gaps and zebra rows.
theme::table(ui, ("findings", id)).show(ui, |ui| {
    // rows ...
});

// Anything else: the whole palette.
let pal = theme::palette(ui);
let rule_color = theme::color(pal.status.of(Status::Warn));
```

`theme::apply` runs once when the window opens (`app.rs`, `run_window`), with the preset
and mode from `--theme`, else `ui.json`, else `classic`. It sets egui's style for both
its light and dark themes, so switching Windows between light and dark keeps the preset.

## Not yet

- A theme picker in the window and saving the choice to `ui.json` (M6.V3, after M6.O1).
- The status component on every status in every panel (M6.V4); today it covers
  `PASSED`/`FAILED`, validate findings and rule groups, unresolved FormIDs, and the
  diagnostics "no finding" line.
- One panel frame and table style for all 11 tabs (M6.V5).
- A focus-ring token and the 3:1 rule for non-text contrast (M6.V3).
