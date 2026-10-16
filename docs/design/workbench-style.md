# Workbench style guide (M6.V6)

How the KitKraft workbench looks and how a panel is drawn: the tokens, the components,
the spacing and density rules, the status rule, the panel frame, and how a new panel is
added. The direction (Workbench, owner decision M6.O1, 2026-10-16) and its history are in
`docs/design/workbench-visual-system.md`; everything else is here.

A test keeps this page and the code in step (`the_style_guide_names_every_token_and_only_tokens`
in `crates/ui-egui/src/theme/mod.rs`): every token `crates/ui-egui/src/theme/tokens.rs`
defines is named here in backticks (`group.name`), and every `group.name` this page names
exists there. Add a token to both, or the gate fails.

## Where it lives

| Path | What |
| ---- | ---- |
| `crates/ui-egui/src/theme/tokens.rs` | The tokens as plain data (`Theme`, `Palette`, `TypeScale`, `Spacing`, `Sizes`, `Radii`, `Density`, `Status`), the three presets, WCAG luminance and contrast, and their tests. No egui type in it. |
| `crates/ui-egui/src/theme/mod.rs` | `apply(ctx, &Theme, Mode)`, the only function that writes tokens into egui's style; the status component; the table and form grids; the frames; the named sizes; the source scans. |
| `crates/ui-egui/src/panels/common.rs` | The panel frame (`panel_head`, `action_bar`, `envelope_block` with `limitations_strip`), the virtual table (`virtual_table`, `TableRow::cell`, `TableRow::num`), and the shared cells (`form_cell`, `formid_status`, `conf_label`, `record_actions`, `plugin_combo`, `findings_grid`). |
| `crates/ui-egui/src/panels/<tab>.rs` | One tab: its view-model above `// ---- drawing`, its `fn ui` below. |
| `%LOCALAPPDATA%\kitkraft\ui.json` | `"theme"`, read at start (`--theme` wins); the Start tab's picker writes it. |

Presets: `workbench` (the default), `studio`, and `classic` (legacy: egui's look before the
tokens, kept for comparison), each with an optional `-light` or `-dark` suffix; without
one the window follows Windows. An unknown name on the command line exits 2; in `ui.json`
it opens Workbench with a notice.

## Tokens

All sizes are points (a pixel at 100 percent scaling). Fonts are the Windows system fonts,
Segoe UI and Consolas (M6.O2 is open; nothing is compiled in).

### Type scale

| Token | Workbench | Studio | Classic | Used for |
| ----- | --------: | -----: | ------: | -------- |
| `type_scale.small` | 11 | 12 | 9 | small notes |
| `type_scale.body` | 13 | 14 | 13 | labels, table cells |
| `type_scale.button` | 13 | 14 | 13 | buttons, tabs |
| `type_scale.mono` | 12.5 | 13.5 | 13 | FormIDs, offsets, `kit` lines, numbers |
| `type_scale.heading` | 16 | 22 | 18 | a tab's title (`panel_head`) |
| `type_scale.subheading` | 14 | 17 | 16 | a section inside a tab (`theme::subheading`) |

### Spacing

`SPACE` = 0, 2, 4, 8, 12, 16, 24, 32: a 4-point grid with one 2-point half step. Every
spacing and density value of the token presets is on it (a test checks it).

| Token | Workbench | Studio | egui field or use |
| ----- | --------: | -----: | ----------------- |
| `spacing.item_x` | 8 | 12 | `item_spacing.x`; the bar and strip side margin |
| `spacing.item_y` | 4 | 8 | `item_spacing.y`; the strip's top and bottom margin |
| `spacing.button_x` | 8 | 12 | `button_padding.x` |
| `spacing.button_y` | 2 | 4 | `button_padding.y` |
| `spacing.panel_margin` | 8 | 16 | the central panel's margin, window margins |
| `spacing.indent` | 16 | 24 | collapsing sections, Help list depth |
| `spacing.block` | 8 | 12 | `theme::gap`: between two blocks of a panel |

### Sizes

Field widths are points; scroll views are table rows, so their height follows the density
(`theme::view_height` = rows x (`density.row_height` + `density.cell_y`)).

| Token | Workbench, Classic | Studio | Used for |
| ----- | -----------------: | -----: | -------- |
| `sizes.field_tiny` | 60 | 72 | a record signature |
| `sizes.field_short` | 140 | 160 | a filter, a member, a plugin for hex FormIDs |
| `sizes.field_medium` | 260 | 300 | a form, the rules list, a date |
| `sizes.field_long` | 420 | 460 | a path, a `where` clause |
| `sizes.field_wide` | 560 | 600 | the Load and Ledger forms |
| `sizes.combo` | 112 | 128 | the "loaded..." plugin picker |
| `sizes.view_short_rows` | 10 | 10 | the record types list |
| `sizes.view_medium_rows` | 14 | 14 | query results, references, evidence, findings |
| `sizes.view_tall_rows` | 19 | 19 | load order, links, planet nodes, the coverage matrix, raw JSON |
| `sizes.doc_cell_wide` | 520 | 520 | a Help table cell, 1 to 2 columns |
| `sizes.doc_cell_mid` | 360 | 360 | a Help table cell, 3 to 4 columns |
| `sizes.doc_cell_narrow` | 232 | 232 | a Help table cell, 5 or more columns |
| `sizes.pane_min` | 320 | 320 | the narrowest Records pane |
| `sizes.split_default` | 0.45 | 0.45 | the inspector's share of the Records tab until dragged |

The window opens at `WINDOW` = 1400 x 900 points (the screenshot tour uses the same size).

### Radii

| Token | Workbench | Studio | Classic |
| ----- | --------: | -----: | ------: |
| `radii.control` (buttons, fields, tabs, the action bar, the limitations strip) | 2 | 6 | 2 |
| `radii.panel` (windows, menus) | 4 | 8 | 6 |

### Density

| Token | Workbench | Studio | Classic |
| ----- | --------- | ------ | ------- |
| `density.name` | compact | comfortable | compact |
| `density.row_height` (also a control's minimum height) | 20 | 28 | 18 |
| `density.cell_x` (gap between cells) | 12 | 16 | 8 |
| `density.cell_y` (gap between rows) | 2 | 4 | 3 |
| `density.zebra` | on | off | on |

### Palette

Each preset has a light and a dark palette with the same names.

| Token | Workbench light | Workbench dark | Meaning |
| ----- | --------------- | -------------- | ------- |
| `surface.base` | `#f3f4f6` | `#15171a` | panel background |
| `surface.raised` | `#ffffff` | `#22262b` | buttons, popups, the top bar, the action bar |
| `surface.sunken` | `#e9ebee` | `#0f1113` | text fields, code wells, the limitations strip |
| `surface.zebra` | `#eaecef` | `#1b1e22` | every other table row (when `density.zebra`) |
| `surface.hover` | `#e1e5ea` | `#2b3036` | a hovered control |
| `surface.selection` | `#d3e3f3` | `#1f3a52` | a selected tab, row or text |
| `text.strong` | `#111417` | `#f0f2f4` | headings, hovered and selected controls |
| `text.body` | `#30363d` | `#c9ced4` | labels and cells |
| `text.muted` | `#4f5862` | `#9aa3ad` | notes (`ui.weak`) |
| `accent` | `#1d5c94` | `#6cb4ee` | links, a hovered control's outline |
| `border` | `#c3c9d0` | `#343a41` | separators, frames, grid lines (decorative) |
| `control_border` | `#6f7985` | `#7d8793` | a button's, field's or picker's outline (3:1) |
| `focus` | `#1d5c94` | `#6cb4ee` | the focus ring (3:1) |
| `code` | `#1d5c94` | `#8fc6f2` | code in the Help tab |
| `status.pass` | `#1a6b37` | `#4fc98a` | pass |
| `status.fail` | `#b42318` | `#ff7a70` | fail, an error, an unresolved FormID |
| `status.warn` | `#7a5000` | `#f2b33d` | a warning, an unconfirmed fact, the limitations |
| `status.not_run` | `#4f5862` | `#a3acb6` | not run (yet) |
| `status.inconclusive` | `#6a3d9e` | `#c49be8` | ran, evidence missing or ambiguous |
| `status.unknown` | `#0b6470` | `#5fcfd8` | could not be established (never "absent") |

Studio's values are in `tokens.rs` (`studio()`); its warm palette passes the same tests.
Classic keeps the four pre-token constants on egui's default style and is below AA in light
mode; that is why it is marked legacy.

## Contrast

Every status color, text level, `accent` and `code` is at least 4.5:1 against every
surface (`base`, `raised`, `sunken`, `zebra`, `hover`, `selection`) in both modes of both
token presets; `control_border` and `focus` at least 3:1 (WCAG 1.4.11). The tests compute
the WCAG 2.x luminance themselves (no dependency). A new color is a token added to the
palette and to these loops, never a literal in a panel.

## Components

| Component | Where | What it draws |
| --------- | ----- | ------------- |
| `theme::status(ui, status, word)` | theme | the status glyph, painted (no icon font), then the word, both in the status color; the status's own word when `word` is empty; hover names the status |
| `theme::status_line(ui, status, text)` | theme | the same for a long line that wraps (an error, a confirm question) |
| `theme::status_header(ui, status, text)` | theme | the plain-text form for widgets that take one styled text (a collapsing header): `● PASS text` |
| `theme::status_painted(ui, status, text)` | theme | the line and color for text a panel paints itself (the viewport overlay) |
| `formid_status(ui, word)` | common | a FormID's resolution: `resolved` passes, `null` is plain text, anything else fails |
| `conf_label(ui, level)` | common | a schema confidence: `corpus`, `ck-diff`, `verified` pass; `reference`, `container-reference` warn ("unconfirmed"); any other level is unknown |
| `virtual_table(ui, id, header, rows, view, row)` | common | the table style (below) |
| `theme::table(ui, id)` | theme | a grid with the density's row height, gaps and zebra rows, for a few fixed rows (key and value pairs, the review steps) |
| `theme::form_grid(ui, id)` | theme | a form's label and field columns |
| `theme::gap(ui)` | theme | `spacing.block` between two blocks |
| `theme::width(ui, Field)`, `theme::view_height(ui, View)`, `theme::combo_width(ui)` | theme | the named sizes |

The glyphs: pass a filled disc, fail a cross, warning a triangle, not-run an empty ring,
inconclusive a diamond, unknown an empty square. No two statuses share a glyph or a word
(test `statuses_differ_by_glyph_and_word_not_only_color`).

## The status rule (M6.V4)

Status is never color only. Pass, fail, warning, not-run, inconclusive, unknown, the
confidence levels and resolved or unresolved FormIDs are drawn with the components above:
glyph, word and color together. The status colors are private to `theme/`. A test
(`no_status_text_is_colored_outside_the_status_component`) scans every source file outside
`theme/` and fails on `colored_label`, a status color read from the palette, egui's warning
or error color, or a status built by hand from the glyph character; another
(`the_status_component_draws_glyph_word_and_color`) draws each status in both modes of both
token presets and checks the glyph shape, the word and the color. Classic shows the colored
word only, as before the tokens (legacy).

## The table style (M6.V5)

A table that can grow is a `virtual_table`: only the rows scrolled into view are laid out
(M6.P4); the header stays on top while the rows scroll; every row is `density.row_height`
high, zebra when `density.zebra`; columns are as wide as their widest content so far and
never wrap; the scroll position survives a repaint. In a row, `cell` adds a left-aligned
cell and `num` a number: monospace, right-aligned. FormIDs, signatures, offsets and raw
values are monospace. A short fixed table (key and value pairs, the review steps, the
evidence of one coverage row) is `theme::table`. Column resizing by drag is not built yet.

## The panel frame (M6.V5)

Every tab is drawn in four parts, top to bottom, with the helpers in `panels/common.rs`:

1. **Head**: `panel_head(ui, title, about)`: the title in `type_scale.heading` and one
   paragraph on what the tab shows and which commands it runs.
2. **Action bar**: `action_bar(ui, |ui| ...)`: the tab's form and buttons in one outlined
   strip on `surface.raised`, as wide as the visible panel. A tab with several forms
   (Records, Diagnostics) gives each its own bar above the result it fills.
3. **Result header and limitations**: `envelope_block(ui, shown, body)`: the time, the
   `kit` line and the MCP call (each copyable), PASSED or FAILED and the summary's first
   line, coverage, then the limitations strip (`limitations_strip`: the warning status
   with their count, then each limitation in body text on `surface.sunken`; never hidden;
   a long one is cut with the rest on hover). A multi-line summary's text table follows,
   folded beyond 12 lines. An error replaces all of it with a fail status line and the
   error JSON.
4. **Body**: the closure given to `envelope_block`: the tab's own tables and notes, then
   the evidence ranges and the raw JSON (folded).

The Render Window tab has the head and its toolbar as the action bar; the viewport is its
body. A test (`every_tab_is_drawn_in_the_panel_frame`) holds that each tab file calls
`panel_head` once and `action_bar`, and draws no heading of its own.

States: empty (no result yet: the bar alone, or a short weak hint such as the Records
inspector's), busy (the status line at the bottom shows the running command, its progress
and the queue; buttons that submit are disabled), error (the fail status line in the
result block; a form error is the notice line under the tabs, also a fail status).

## Window chrome

- **Top bar** (`theme::top_frame`): the title, the tabs (they wrap), the notice line.
- **Status line** at the bottom (same frame): engine state, session, the read-only reminder.
- **Title bar**: `KitKraft Workbench (read-only)`; a debug build adds "debug build" once the
  first envelope's `build` member says so (M6.P6a).
- **About line** (Start tab): `KitKraft <version> (<commit>, <date>, <profile>)` from the
  envelopes' `build` member (M6.P6b), with a warning status on a debug build.

## Adding a panel

1. Make `crates/ui-egui/src/panels/<tab>.rs`: the view-model first (requests built from the
   tab's inputs, rows read from envelopes; tested on canned and real envelopes without a
   window), then `// ---- drawing ----`, the tab's state and `fn ui(&mut self, ui, ctx)`.
2. Draw it in the frame: `panel_head`, then `action_bar` with the form, then
   `envelope_block(ui, ctx.results.get(&Slot::<Yours>.into()), |ui| { ... })` with the body.
3. Rows that can grow go in a `virtual_table`; numbers with `num`; statuses with the
   status components; sizes with `theme::width` and `theme::view_height`; gaps with
   `theme::gap`.
4. Add its `Slot` in `common.rs`, its `Tab` and the dispatch in `app.rs`, and a request only
   the gate allows (`gate.rs`).
5. Run `cargo test -p kitkraft-ui-egui`: the scans (no color, size or spacing literal, no
   status outside the component, the panel frame) cover the new file at once. Look at it
   in the screenshot tour (`docs/workbench.md`, "Screenshot tour") in light and dark.

## Do and don't

| Do | Don't |
| -- | ----- |
| `theme::status(ui, Status::Warn, "the file name only")` | `ui.colored_label(yellow, "the file name only")` |
| `theme::status_header(ui, status, &title)` for a collapsing header | build `● PASS` from `glyph_char` in a panel |
| `row.num(n.to_string())` for a count or an offset | a left-aligned number |
| `virtual_table` for anything that can pass a screenful | a bare `Grid` inside a `ScrollArea` |
| `theme::gap(ui)` and the named widths | `ui.add_space(6.0)`, `.desired_width(420.0)` |
| one `panel_head` and an `action_bar` per tab | `ui.heading(...)` and loose buttons |
| show unknown as the unknown status | leave a blank where a value could not be read |

## Screenshot tour

`KITKRAFT_WORKBENCH_SHOTS=<dir>` and a profile: the window runs a fixed read-only tour and
saves one PNG per tab (`docs/workbench.md`, "Screenshot tour"). Add `--theme
workbench-light` or `--theme workbench-dark` for each mode. The images stay outside the
repo; lane V round 3 (2026-10-16) checked every tab in both modes on `Kitkraft Dev`.
