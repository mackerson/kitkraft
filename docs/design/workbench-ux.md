# Workbench UX spec and audit (2026-11-01)

What every click in the workbench (`crates/ui-egui`, `kit ui`) must do for the person at
the window, how a test checks it without anyone clicking, and what the first audit found.
The owner walked the workbench by hand on 2026-11-01 and found ten problems in twenty
minutes; this page and its harness make that walk an agent's job. The owner keeps only
the final sign-offs (M6.O4 cockpit, M7.O6 CK A/B).

Looks (tokens, components, the panel frame) are `docs/design/workbench-style.md`'s; this
page is about behaviour.

## The harness

`crates/ui-egui/src/app/harness.rs` (with its tests in `app/harness/tests.rs`) drives the
window's own frame, `App::frame`, through `egui_kittest` at the window's usual 1400 x 900
points, and reads what it drew from egui's own AccessKit tree: every widget's role, text,
enabled state and rectangle. No window opens, no GPU is used, and the AccessKit platform
adapter stays off (the reason Windows UI Automation cannot see the workbench). It runs in
`cargo xtask ci` like any unit test, without a game install:

- The engine runs on its worker as in the window: a plain engine with nothing loaded
  (`workbench()`), or the engine's game fakes (`workbench_over(ReadOnlyEngine::with_game_fakes(..))`:
  a fixture MO2 instance, a fake console and a fake process host).
- A reply only a real profile gives is filed with `inject`, the path a worker reply takes.
- Nothing reads or writes `ui.json`; the harness never clicks a confirm button.
- The fonts are Windows' (`segoeui.ttf`), as in the window; on Linux (the local agent's
  gate) the first TrueType font under `/usr/share/fonts` (DejaVu Sans when present). With
  no font at all each harness test prints `harness: skipped: no system font` and passes,
  as corpus tests skip without the game; a ticket's acceptance greps for that line.

Run it alone: `cargo test -p kitkraft-ui-egui --lib app::harness`. The audit report below
comes from `cargo test -p kitkraft-ui-egui --lib audit_every_tab -- --ignored --nocapture`.

Reference images come from the screenshot tour (`KITKRAFT_WORKBENCH_SHOTS=<dir>`,
`docs/workbench.md` "Screenshot tour"): the audit's are in
`observe\workbench-ux-2026-11-01\before\` (gitignored, never committed), named below by
file.

Limits: AccessKit carries no clip rectangles, so "in view" means "the node's centre lies in
the window"; a node clipped by a nested scroll area but still inside the window counts as
in view. The harness checks no pixels (colours, overlap of painted shapes): the tour's
images are the record of those. The Render Window's GL viewport draws nothing headless.

## Principles

Each principle names the helper or test that checks it.

1. **Every click gives visible feedback within the viewport.** A button's result, its
   refusal, or a line saying it did nothing appears where the owner is looking, without
   scrolling. Check: after the click and the engine's reply (`settle`), `assert_in_view`
   finds the feedback's text with its centre inside the 1400 x 900 window. A result that
   lands below the fold is scrolled to (the session preview does it once per new reply).
2. **Refusals and errors are shown in full, next to the control that caused them.** Every
   refusal line the engine returns (`[rule] message`) is drawn as returned, never only
   counted, and never pointed at ("above", "below") from somewhere it is not. Check: each
   refusal's full text is in view and starts below its button within a third of the
   window's height (`findings_7_and_9_...`); the pointer phrase is absent (`assert_absent`).
3. **One control per action, no duplicates.** One button runs one action, and one action
   has one button. Check: `one(label)` finds exactly one node; the audit lists every
   label that appears twice on a tab.
4. **Forms are grouped by task**, in the order the work goes, each group outlined with its
   heading and holding only its own fields. Check: the groups' headings come in the
   task's order, and each control's top lies between its group's heading and the next
   (`finding_6_the_cockpit_is_grouped_by_task`).
5. **Lists scroll smoothly and show their rows.** A list shows its first rows, one under
   the other in one column, at its full height wherever it sits on the page, and the
   wheel brings later rows in. Check: the first rows are in view, stacked, left-aligned,
   and a `scroll_down` over the list brings a later row in (`finding_2_...`).
6. **Pickers show every entry in full and can be cleared.** A drop-down lists all its
   entries without a scroll of its own (up to the window's height), each as wide as its
   text; the value it set can be put back to empty with one click. Check: every entry's
   node is in view and as wide as its laid-out text, no scroll bar beside the entries, and
   "Clear" empties the field (`finding_4_...`).
7. **Tables have row spacing and can be resized.** Rows are separated (at least 4 points
   between one row's text and the next), and every scrolling table has a grip under it
   that makes it taller or shorter. Check: the gap between consecutive row texts, and a
   drag on the grip (AccessKit `Splitter` "resize table <id>") moves the grip and shows
   more rows (`finding_5_...`).
8. **Panels are discoverable from the tab bar or a named menu.** Every panel has one
   button in the tab bar (or is a window named in the View menu), and opening it shows
   its heading in view. Check: `tab_button` finds exactly one toggled button per tab and
   `every_tab_opens_from_the_tab_bar_with_its_heading_in_view`.

## Findings

Owner = the owner's walkthrough of 2026-11-01 (release build of `main` at `a45d90a`).
Audit = the harness audit and the tour of the same day. "Fixed" names the harness test
that failed before the fix and passes after it.

| # | Tab | Control | Principle | Screenshot | Finding | State |
| - | --- | ------- | --------- | ---------- | ------- | ----- |
| O1 | (none) | View > Cell View | 8 | `ck-layout.png` | The Cell View was not in the tab bar, only under View, and opened as a window over the current tab. | Fixed: a tab (`Tab::CellView`, which floats like any tab; the `cell` menu item opens it). `finding_1_cell_view_is_a_tab` |
| O2 | Cell View | worldspaces list | 5 | `ck-layout.png` | "551 row(s)" over a blank area; scrolling showed one row's sliver; "Interior" never appeared. Two causes: the heading, header and rows were laid out side by side in one horizontal row, and egui gave a scroll area only the space left in the visible window (A8). | Fixed: two columns, each top to bottom; tables keep their full height. `finding_2_the_cell_view_list_shows_its_rows` |
| O3 | Start | start-up load | - | `start.png` | Start-up from the index cache is fast. | Kept as is. |
| O4 | Cockpit | Plugin under review, `out\` picker | 6 | `cockpit.png` | Started empty; the drop-down showed about 8 of 15 entries and cut long paths; no way back to empty. | Fixed: the picker lists every entry in full up to the window's height; "Clear" empties the plugin and its results. (It still starts empty: nothing names a default plugin.) `finding_4_the_plugin_picker_shows_every_entry_and_clears` |
| O5 | Cockpit | Ledger entries table | 7 | `cockpit.png` | Cramped, and could not be resized. The table sat at the page's end, where egui's sizing (A8) left it about three rows. | Fixed: full height, and a grip under every scrolling table. `finding_5_the_ledger_table_has_row_spacing_and_resizes` |
| O6 | Cockpit | whole tab | 4 | `cockpit.png` | Session, review, deploy and ledger fields were not grouped logically (the session came before the review). | Fixed: Plugin under review (action bar), then outlined groups Review, Dev deployment, Game session (what runs: probes, base save; then where: MO2 instance, profile, observe, ledger), Ledger entries. `finding_6_the_cockpit_is_grouped_by_task` |
| O7 | Cockpit | "Launch and run probes" (top row) | 1 | `cockpit.png` | Submitted a dry run whose result landed below, out of view, or nowhere. | Fixed: the top row is gone (O8); "Preview session" shows its result right under it and scrolls it into view. `findings_7_and_9_a_refused_preview_shows_its_refusals_by_the_button` |
| O8 | Cockpit | top game-button row | 3 | `cockpit.png` | Duplicated the session section's "Preview session" and "Launch...". | Fixed: removed; the session section's buttons read their enabled state from the catalog and the gate (`game_buttons`), and say why when off. `finding_8_no_duplicate_game_buttons` |
| O9 | Cockpit | Launch... (confirm closed) | 2 | `cockpit.png` | After a refused dry run: "the dry run would not start (the refusals, above)", but no refusal was drawn anywhere (they were `selected-profile` and `running`). | Fixed: the dry run's summary, every `[rule] message` and its plan are drawn under "Preview session"; an engine error for the preview is drawn there too. `finding_9_the_owners_two_refusals_are_shown`, `findings_7_and_9_...` |
| O10 | (CLI) | `kit game session --dry-run` | 2 | - | Text output said "session would be refused (2 reason(s))" without the reasons. | Fixed in the engine: the summary (what text output prints, and MCP and the window show) has one `[rule] message` line per refusal. `every_preflight_refusal_names_its_rule_and_writes_nothing` (engine) |
| O11 | Cockpit | probes, base save sha256 | 1 | `cockpit.png` | Both started empty. | Fixed: prefilled from the ledger's newest passing session naming the plugin (its `Status` passes, or every step row does), with a line naming the entry. `the_session_form_is_prefilled_from_the_newest_passing_ledger_session`, `the_newest_passing_session_for_a_plugin` |
| A1 | Load, Records, Links, Planet, Diff, Validate, Cockpit | Load, Inspect, Run, Show, Compare, validate, roundtrip --verify, Status, Preview deploy | 2 | `load.png`, `cockpit.png` | A form a button refuses ("give a plugin", "no MO2 instance: load the dev profile") is reported only in the notice line under the tabs (y 69), up to 600 points from the button (Cockpit's "Preview deploy" at y 670). | Ticket `KK-246`. |
| A2 | Load | List | 1 | `load.png` | "List" on a folder with no profiles showed nothing. | Fixed: a line by the button says so. `audit_a2_an_empty_profiles_listing_says_so` |
| A3 | Records | Query > Run | 1 | `records.png` | "Record types" opened expanded, which put the query form's Run at y 876 of 900, under the status line. | Fixed: "Record types" starts folded (as the tour already drew it). `audit_a3_the_records_query_opens_in_view` |
| A4 | Help | Reload | 1 | `help.png` | "Reload" gave no visible sign it ran. | Fixed: the info line names what it reread. `audit_a4_help_reload_says_what_it_reread` |
| A5 | every result block | copy, copy | 3 | `records.png` | Two buttons labelled "copy" per result (the `kit` line and the MCP call): same label, different actions. | Ticket `KK-247`. |
| A6 | any | Warnings, Use Info, Edit Dialog windows | 1, 8 | `ck-layout.png` | The CK-style windows open at fixed places over the current tab and over each other; with all open, the tab under them is unreadable. | Ticket `KK-248`. |
| A7 | Cockpit, Validate | plugin path fields | 6 | `cockpit.png`, `validate.png` | A single-line field cuts a long path (`...\KitKraftMELDL`) with no way to read the rest but scrolling the caret. | Ticket `KK-249`. |
| A8 | every tab with a table | `virtual_table` | 5, 7 | `cockpit.png` | egui sizes a nested scroll area from the space left in the visible window, so any table low on a scrolled page shrank to a few rows (the cause behind O2 and O5). | Fixed in `virtual_table_for` (a minimum height equal to its view's); `finding_5_...`, `finding_2_...` |
| A9 | Records, Validate, Diff | wide tables | 5 | `records.png`, `validate.png` | Wide columns (plugin, message) end at the pane's edge; they scroll sideways. | No action: horizontal scroll is the table's design (style guide, "virtual table"). |

The audit's inventory (`audit_every_tab`, engine with nothing loaded): no tab has two
buttons with one label except "copy" (A5); every tab's heading opens in view; the Cockpit
runs to y 1028 at 900 (its later groups scroll, by design); the Help tab to y 5967 (the
docs).
