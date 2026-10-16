# Workbench visual system: tokens and rules (M6 lane V)

**Status: direction chosen (M6.O1, 2026-10-16): Workbench. Token system landed (M6.V3).**
`workbench` is the window's default. M6.V4 to M6.V6 finish the system on it; the full style
guide is `docs/design/workbench-style.md` (M6.V6).

## Chosen direction

The owner, in chat, 2026-10-16 (M6.O1), verbatim: "Workbench, themes look great only keep the mocks if there's a good reason."

- **Workbench** is the default preset: neutral graphite and cool grey, dense, 13 pt body,
  20 pt zebra rows, square corners, one steel-blue accent, designed dark first. Without
  `--theme` or a `theme` in `ui.json` the window opens in Workbench and follows Windows'
  light or dark setting.
- **Studio** stays selectable (`--theme studio`, or the Start tab's picker): the other
  mockup direction, kept because it costs nothing (it is data in `tokens.rs`) and keeps
  the token system honest (every panel must look right under two token sets).
- **Classic** stays selectable as **legacy** (the picker says so): egui's default style
  from before the tokens, below AA in light mode, kept only for comparison.
- **The mockups were removed** (`docs/design/mockups/2026-10-16/`, 12 PNGs, 3.6 MB). No
  good reason to keep them was found: the choice is recorded here and in
  `plan/execution-plan.md` (M6.O1), the images stay in the history at commit `48beef1`, and
  any of them can be made again with the screenshot tour and `--theme`
  (`docs/workbench.md`, "Screenshot tour"). Tour images stay outside the repo.

Why: the owner's pains on 2026-10-16 were "look, feel, performance". The director's brief
named plain egui styling, cramped tables and no theming. The tokens below are the answer
for look and feel. They are data, in one place, so a direction is chosen by values and
not by editing panels.

## Where they live

| Path | What |
| ---- | ---- |
| `crates/ui-egui/src/theme/tokens.rs` | The tokens as plain data (`Theme`, `Palette`, `TypeScale`, `Spacing`, `Sizes`, `Radii`, `Density`, `Status`), the three presets, WCAG luminance and contrast, and the tests. No egui type in it. |
| `crates/ui-egui/src/theme/mod.rs` | `apply(ctx, &Theme, Mode)`: the only function that writes tokens into egui's `Style` and `Visuals`. Also what panels draw with: `palette`, `warn`/`bad`/`good`/`code`, the status component `status` (and `status_line` for a long line that wraps), the table style `table`, `form_grid`, the frames, and the named sizes (`width`, `combo_width`, `view_height`, `gap`, `doc_cell`, `subheading`, `split`). `ThemeChoice` parses `--theme`. The source scan (`no_color_size_or_spacing_literal_outside_the_theme`) lives here. |
| `crates/ui-egui/src/panels/<tab>.rs` | Each tab's drawing (`fn ui(&mut self, ui, ctx)`), which takes every color, size and gap from the module above. |
| `apps/kit/src/main.rs` | `kit-ui --theme <preset>` (so `kit ui --theme <preset>`). |
| `%LOCALAPPDATA%\kitkraft\ui.json` | `"theme": "<preset>"`, read at start; `--theme` wins over it. The Start tab's picker (preset; system, light or dark) applies a choice at once and writes it here. `"records_split"`: the Records inspector's share of the width. |

A preset is spelled `workbench` (the default), `studio` or `classic` (legacy), with an
optional `-light` or `-dark` suffix (for example `studio-dark`). Without a suffix the
window follows Windows' light or dark setting. An unknown name on the command line is a
usage error (exit 2). An unknown name in `ui.json` opens the window in `workbench` with a
notice. The Start tab's theme line has the picker, says where the choice came from, and
shows the six statuses as the preset draws them.

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
| `type_scale.subheading` | 16 | 14 | 17 | a section title inside a tab (Help's level-3 headings) |

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
| `spacing.indent` | 18 | 16 | 24 | collapsing section indent, Help list depth |
| `spacing.block` | 6 | 8 | 12 | `theme::gap`: space between two blocks of a panel |

(Classic's values are egui 0.36's and are recorded only for comparison; they are not on the
scale.)

### Sizes

`Sizes` names every fixed size a panel uses, so no panel holds a number of its own. Field
widths and Help cell widths are in points; scroll views are in table rows, so their height
follows the density (`theme::view_height` = rows x (`row_height` + `cell_y`)).

| Token | Workbench and Classic | Studio | Used for |
| ----- | --------------------: | -----: | -------- |
| `sizes.field_tiny` | 60 | 72 | a record signature |
| `sizes.field_short` | 140 | 160 | a filter, a member, a plugin for hex FormIDs |
| `sizes.field_medium` | 260 | 300 | a form, the rules list, a date |
| `sizes.field_long` | 420 | 460 | a path, a `where` clause |
| `sizes.field_wide` | 560 | 600 | the Load and Ledger forms |
| `sizes.combo` | 112 | 128 | the "loaded..." plugin picker |
| `sizes.view_short_rows` | 10 | 10 | the record types list |
| `sizes.view_medium_rows` | 14 | 14 | query results, references, evidence |
| `sizes.view_tall_rows` | 19 | 19 | load order, links, planet nodes, raw JSON |
| `sizes.doc_cell_wide`, `_mid`, `_narrow` | 520, 360, 232 | same | Help table cells for 1 to 2, 3 to 4, 5+ columns |
| `sizes.pane_min` | 320 | 320 | the narrowest Records pane |
| `sizes.split_default` | 0.45 | 0.45 | the inspector's share of the Records tab until dragged |

The window opens at `WINDOW` = 1400 x 900 points (the screenshot tour uses the same size).

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
| `accent` | links, the outline of a hovered control |
| `border` | separators, grid lines, window outlines (decorative: no contrast rule) |
| `control_border` | the outline of a button, field or picker: 3:1 on every surface |
| `focus` | the focus ring (a focused field's outline, a pressed or focused control): 3:1 on every surface |
| `code` | code in the Help tab |
| `status.pass`, `status.fail`, `status.warn`, `status.not_run`, `status.inconclusive`, `status.unknown` | the six statuses |

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
| `control_border` | `#6f7985` | `#7d8793` | `#806f5c` | `#948572` |
| `focus` | `#1d5c94` | `#6cb4ee` | `#963f1f` | `#eba571` |
| `code` | `#1d5c94` | `#8fc6f2` | `#7a3a1c` | `#f0b68a` |
| `status.pass` | `#1a6b37` | `#4fc98a` | `#2a6b33` | `#72d192` |
| `status.fail` | `#b42318` | `#ff7a70` | `#a8261a` | `#ff8f80` |
| `status.warn` | `#7a5000` | `#f2b33d` | `#7a4d00` | `#f0bf5a` |
| `status.not_run` | `#4f5862` | `#a3acb6` | `#5c5249` | `#b3a99b` |
| `status.inconclusive` | `#6a3d9e` | `#c49be8` | `#6d3c91` | `#d3aae9` |
| `status.unknown` | `#0b6470` | `#5fcfd8` | `#1f6470` | `#7fd6d2` |

Lowest contrast of each status color against the six surfaces (measured by the test's
own WCAG 2.x luminance code):

| Preset, mode | pass | fail | warn | not-run | inconclusive | muted text | accent |
| ------------ | ---: | ---: | ---: | ------: | -----------: | ---------: | -----: |
| Workbench light | 5.0 | 5.0 | 5.4 | 5.5 | 5.8 | 5.5 | 5.3 |
| Workbench dark | 5.6 | 4.6 | 6.3 | 5.1 | 5.2 | 4.6 | 5.3 |
| Studio light | 4.9 | 5.4 | 5.5 | 5.8 | 5.9 | 5.8 | 5.3 |
| Studio dark | 6.2 | 5.3 | 6.8 | 5.0 | 5.9 | 4.8 | 5.6 |

`control_border` and `focus` measure at least 3.2:1 against every surface in all four
palettes (test `control_border_and_focus_meet_non_text_contrast_in_both_modes`), and
`status.unknown` at least 5.1:1.

`classic` keeps the pre-token four constants (`WARN #e6aa3c`, `BAD #e65a50`, `GOOD #6ebe6e`,
`CODE #8cbeeb`, the same in both modes) on egui's default style. On egui's light panel
(`#f8f8f8`) they measure warning 1.9, pass 2.1 and fail 3.3: below AA. That is the
problem the presets fix; `classic` stays as it is, selectable and marked legacy, and the
contrast rules below cover the two token presets.

## Rules

1. **Status is never color only.** Pass, fail, warning, not-run, inconclusive and
   unknown are drawn by `theme::status(ui, Status, label)`: a painted glyph (filled disc,
   cross, triangle, empty ring, diamond, empty square; drawn with egui's painter, so no
   icon font is needed), then the word, both in the status color. A collapsing header, which takes plain text,
   leads with the status word instead (`PASS masters.missing: ...`). Two statuses never
   share a glyph or a word (test `statuses_differ_by_glyph_and_word_not_only_color`).
   `classic` shows the colored word only, as the window does today.
2. **AA contrast.** Every status color, text level, `accent` and `code` is at least 4.5:1
   against every surface (`base`, `raised`, `sunken`, `zebra`, `hover`, `selection`) in
   both modes of both token presets (tests `every_status_color_meets_aa_on_every_surface_in_both_modes`
   and `text_accent_and_code_meet_aa_on_every_surface_in_both_modes`; luminance is computed
   in the test, no dependency). A new color token is added to the palette and to these
   loops, never as a literal in a panel. Control outlines and the focus ring are 3:1
   (WCAG 1.4.11; `AA_NON_TEXT`).
3. **Density.** A table is `theme::table(ui, id)`, never a bare striped `Grid`: it takes
   the row height, cell gaps and zebra setting from `density`. Compact (Workbench) puts a
   row every 22 points; comfortable (Studio) every 32, about 30 percent fewer rows on
   screen (compare the two Records mockups). Numbers
   right-aligned, sticky headers and column resizing come with M6.V5 and M6.P4.
4. **Scaling.** Sizes are points; at 100 percent scaling the mockups are pixel-exact.
   Nothing in the tokens assumes a scale; 125 and 150 percent are lane X (deferred).
5. **One place.** Only `theme::apply` writes egui's style. A panel reads tokens through
   the helpers below and never sets a size or a color of its own. A test scans every
   `.rs` file under `crates/ui-egui/src` outside `theme/` and fails on a `Color32`
   constant or constructor, `from_rgb`/`from_gray`, `spacing_mut()`, `style_mut()`,
   `FontId::`, `Margin::`, `Stroke::new(`, or a sizing call (`.size(`, `add_space(`,
   `desired_width(`, `max_height(`, `.width(`, `vec2(` and the like) whose argument is a
   number (`no_color_size_or_spacing_literal_outside_the_theme`).

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

// A field, a scroll view, a gap: named sizes, never numbers.
ui.add(TextEdit::singleline(&mut form.plugin).desired_width(theme::width(ui, Field::Long)));
ScrollArea::vertical().max_height(theme::view_height(ui, View::Tall));
theme::gap(ui);

// Anything else: the whole palette.
let pal = theme::palette(ui);
let rule_color = theme::color(pal.status.of(Status::Warn));
```

`theme::apply` runs when the window opens (`app.rs`, `run_window`), with the preset and
mode from `--theme`, else `ui.json`, else `workbench`, and again when the Start tab's picker
changes it. It sets egui's style for both its light and dark themes, so switching Windows
between light and dark keeps the preset.

## Layout (M6.V3 round 2)

- **Top bar**: the title and the 11 tabs (they wrap instead of pushing anything off the
  window), and the notice line.
- **Status line** at the bottom: the engine's state (running, queued, drop pending, or
  idle), the session (`session: <profile> (<n> plugins)`), and the read-only reminder.
  It used to sit at the end of the top bar, where wide tabs pushed it off the right edge.
- **Records**: two panes, each scrolling on its own. Types and the query on the left, the
  record inspector on the right (the form field, then the versions); drag the line between
  them to resize. The inspector's share of the width is kept in `ui.json`
  (`records_split`, 0.2 to 0.8) when a drag ends.

## Not yet

- The status component on every status in every panel (M6.V4): the panels now draw
  statuses with it (PASSED/FAILED, findings, rule groups, unresolved FormIDs, unknown
  scales and definitions, problems, the ledger's appended and preview lines, profile
  badges, unconfirmed confidence); M6.V4 adds the confidence levels and the test that no
  status text is colored outside it.
- One panel frame and table style for all 11 tabs (M6.V5), with sticky headers, numbers
  right-aligned and column resizing (with M6.P4).
