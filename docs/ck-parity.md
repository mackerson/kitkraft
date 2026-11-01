# Creation Kit parity

What the real Creation Kit does, observed as a black box, and where KitKraft stands. Work on
this list is M3; the viewport and world editing are deferred to M4+.

## Observation protocol

1. Launch `CreationKit.exe` through MO2 (executable #4). Never attach a debugger,
   disassembler, or REA-style tool to it.
2. Start from a known input (a plugin we built, or an empty plugin), perform one action,
   save.
3. Diff CK's output against ours: `cargo xtask observe observe\<date>-<id>` writes
   `report.md` (byte runs, `TES4` diff, `kit diff`, our equivalent in each write style)
   into the fixture folder. The fixtures, their forms and the session script are in
   `docs/ck-observations.md`; each finding becomes a ledger entry there (`ck-<date>-<nn>`)
   and a row below with the date and the CK version shown in its title bar.
4. Screenshots and CK-saved files stay in `observe/` (gitignored). Write findings in our own
   words.

## Checklist

Priority: P1 needed for data work, P2 editor convenience, P3 deferred.

| Area     | CK feature                                | Kit command       | Priority | Status   | Evidence |
| -------- | ----------------------------------------- | ----------------- | -------- | -------- | -------- |
| Plugins  | Data dialog: choose masters + active file | `load`            | P1       | planned  | -        |
| Plugins  | Save plugin (header, ONAM, masters)       | `build`, `set`    | P1       | planned  | -        |
| Plugins  | Compression choice on save                | -                 | P1       | unknown  | -        |
| Plugins  | ESM / light / medium plugin flags         | -                 | P1       | unknown  | -        |
| Records  | Edit a field in the object window         | `set`             | P1       | planned  | -        |
| Records  | "Use info" / references to a form         | `inspect`         | P1       | planned  | -        |
| Records  | Duplicate / new form, FormID allocation   | -                 | P1       | unknown  | -        |
| Records  | Localized strings on save                 | `strings.*`       | P1       | planned  | -        |
| Scripts  | Compile Papyrus                           | `papyrus.compile` | P2       | planned  | -        |
| Archives | Pack BA2                                  | `ba2.pack`        | P2       | planned  | -        |
| Checks   | Plugin errors / warnings on load          | `validate`        | P2       | planned  | -        |
| World    | Cell / worldspace editing                 | -                 | P3       | deferred | -        |
| World    | Navmesh generation                        | -                 | P3       | deferred | -        |
| Audio    | FaceFX / LIP generation, Wwise            | -                 | P3       | deferred | -        |
| View     | Render window                             | -                 | P3       | planned  | -        |

## Findings

None yet: the first CK session (M3.O1) has not happened. M3.H5 extends the checklist above
from what it shows (menus, dialogs, save and export options, warnings on load), with a
ledger id or session note as evidence for every row.

| Date | CK version | Action | Observed | Kit difference | Follow-up |
| ---- | ---------- | ------ | -------- | -------------- | --------- |

## Interface parity (M7)

The exit-criteria source for M7 "CK parity" (`ROADMAP.md`; tickets in
`plan/execution-plan.md` "M7"). There is one row per CK window, menu, menu item, dialog,
shortcut or camera control. A Creation Kit user should find each one in KitKraft where
they expect it. Rows are reproduced from black-box observation only (ADR 0012,
`plan/adr/0012-ck-interface-clean-room.md`, accepted 2026-10-16; its section 4 says what
evidence each status needs):

- Layout, labels, workflows and shortcuts are described here in our own words.
- CK icons, images, help text, resources and binaries are never copied, and the CK's
  resource files are never read.
- All art is KitKraft's.

The observation sources are the owner-assisted CK sittings (`plan/STATUS.md` rows 12
and 15, M7.O2, M7.O3), where a computer-use agent may drive the CK while the owner
watches, and, since 2026-11-01, automated sittings an agent runs through UI Automation
(ADR 0012 Amendment 2, accepted by the owner 2026-11-01). Screenshots and dumps stay in
`observe\` (gitignored). The public CK wiki is a level-4 source for data facts (AGENTS.md
"Clean room"), not interface observation: no row here moves to `observed` on its word.
The menu order in the first row is a recollection: the bar built to it is `built,
unobserved` until a sitting observes the CK's order (director decisions 2026-10-16,
`plan/execution-plan.md` "M6", and ADR 0012 Amendment 1, both confirmed by the owner
2026-11-01).

Status, one of:

- `unresearched`: not yet observed. What the row says comes from the owner's or a
  planner's recollection and is not evidence.
- `built, unobserved`: below `observed` (ADR 0012 Amendment 1, director decision
  2026-10-16, accepted by the owner 2026-11-01). KitKraft has the element (a window, dialog or menu bar),
  but it matches no observation: its layout, order and labels are KitKraft's own (or a
  recollection's). "Evidence" names the test or tour image of what was built; it is
  never counted as parity. It moves to `observed` only through a sitting, and the element
  is redone from the observation where it differs.
- `observed`: seen in a sitting, with the sitting id in "Source".
- `reproduced`: KitKraft has it as observed, with the tour image or test in "Evidence".
- `improved`: KitKraft has it and does more; "Evidence" says what more.

No row skips a step: `unresearched` and `built, unobserved` rows reach `reproduced` only
through `observed` (ADR 0012 section 4).

"Maps to" is a catalog command id, a UI element, or `gap:` with the reason (and a
`docs/gaps.md` row when the gap is KitKraft's). The test of M7.K5 checks that every
menu-item row maps to a catalog id that exists or to `gap:`.

Seeded 2026-10-16 by the roadmap lane with every row `unresearched`; the rows M7 lane M
built before any sitting are `built, unobserved`. Sitting 1
(M7.O2) fills the menus and windows, sitting 2 (M7.O3) the edit dialogs; either may be
an automated sitting. The 2026-11-01 probe and `ck-20261101-01` ran before the owner
accepted Amendment 2 the same day; their observations, with those of F30 and F29
(`ck-20261101-02`, `-03`), moved rows to `observed` in a change of their own. Today:
8 `observed`, 13 `built, unobserved` and 18 `unresearched` rows (counted 2026-11-01).

| Kind | CK element (as named in the CK) | Maps to | Status | Source | Evidence |
| ---- | ------------------------------- | ------- | ------ | ------ | -------- |
| menu | Main menu bar, order (the owner recalls File, Edit, View, World, NavMesh, Character, Gameplay, Help) | menu bar rendered from `menu_path` (M7.M1): these menus in this order (`crates/ui-egui/src/menu.rs` `CK_MENU_ORDER`), KitKraft's own menus (Record, Tools, Archive, Papyrus, Strings, Game) before Help; a CK menu without a command shows a disabled line | built, unobserved | owner, 2026-10-16 (recollection) | tests `menu::tests::ck_menus_come_first_in_the_recorded_order_and_kitkraft_menus_before_help`, `menu::tests::every_ck_menu_row_in_the_parity_doc_maps_to_the_bar`; tour `ck-menu-file.png`, `ck-menu-navmesh.png` (kept local) |
| menu | File menu, every item | per item | unresearched | - | - |
| menu | Edit menu, every item (incl. undo, as far as offered) | per item; undo M7.E6 | unresearched | - | - |
| menu | View menu, every item | per item | unresearched | - | - |
| menu | World menu, every item | per item | unresearched | - | - |
| menu | NavMesh menu, every item | `gap:` navmesh generation is out of scope (ROADMAP M7); items drawn disabled | unresearched | - | - |
| menu | Character menu, every item | per item | unresearched | - | - |
| menu | Gameplay menu, every item | per item | unresearched | - | - |
| menu | Help menu, every item | per item; no CK help text is reproduced | unresearched | - | - |
| dialog | Data dialog (choose masters and the active file; details) | `load` (M7.M2). Observed: a Win32 dialog (its rows are not exposed to UI Automation); each plugin listed with a status ("Plugin File", "Active File") and its parent masters; typing a name selects its row, a double-click ticks it, "Set as Active File" makes the selected plugin the active file; OK loads (about 2 minutes for `Starfield.esm`). A master-flagged plugin cannot be the active file | observed | `ck-20261101-01`, `-02`, `-03` (agent-run sittings, ADR 0012 Amendment 2) | - |
| window | Object Window (category tree, list, filter) | M7.W1 over `query`, categories M7.S2. Observed: a floating, dockable window; a category tree with "*All" at the top (about 15 s not responding while it fills), a Text Filter box, and a list whose rows show the EditorID, the FormID, the type and a use count (`Crate_MilitarySupply_01` `001E4FBD` STAT, 459, 460 once placed); a row drags into the Render Window to place it | observed | `ck-20261101-03` | - |
| window | Cell View (worldspace, cells, references) | `cell` (M7.W2, M7.S3): the Cell View tab (View > Cell View opens it; it floats like any tab; a tab since owner finding 1, 2026-11-01); the worldspaces (with "Interior" as a first entry) or one world's exterior cells with their grid on the left, the selected cell's references on the right, each paged; a click on a reference selects it, a double-click inspects it; the row of the inspected record (the shared selection with the Render Window) is drawn selected and the list scrolls to it once when the selection changes; "Show in Render Window" loads the cell there and the Render Window tab comes to the front (KK-037). Observed in the CK: View > Cell View is a toggle that shows or hides the window; a World Space list ("Interior" for interior cells), a Cell Filter box and a list of cells (a tooltip shows the name and FormID, e.g. `'CityNewAtlantisApartmentTEMPLATE' (0021966F)`); a double-click loads the cell into the Render Window | observed | `ck-20261101-03` | tests `windows::cell_view::tests::cell_view_lists_worlds_interiors_and_a_worlds_cells`, `windows::cell_view::tests::cell_view_reads_a_cells_references_and_pages`, `windows::cell_view::tests::the_cell_view_marks_the_selected_reference`, harness `app::harness::tests::finding_1_cell_view_is_a_tab`, `finding_2_the_cell_view_list_shows_its_rows` |
| window | Render Window | 3D viewport (M7.R1..R4), read-only. M7.R1 (2026-10-16): the workbench's "Render Window" tab with a grid, world axes, a horizon tint, one placeholder box and a stats overlay; M7.R2/R3 (2026-10-16): a cell's references as `OBND` boxes or type shapes from `cell --with-bounds`, colored by base type, click to select (shared with Records); the rotation convention is pinned from corpus evidence (`viewport/scene.rs` `ref_rotation`), not from the CK; M7.W2 (2026-10-27, KK-037): one shared selection with the Cell View — its "Show in Render Window" loads the cell through the tab's own picker (first page, then the pages), and the Cell View offers "Show this cell" when this window shows a different cell | unresearched | - | - |
| control | Render Window camera: orbit, pan, zoom, fly (mouse and keys) | M7.R1 bindings, **unobserved against the CK** (to be checked in sitting 1, M7.O2): orbit on right-drag, pan on middle-drag, zoom on the mouse wheel, fly with W A S D while the viewport has focus (Shift faster); select with a left click (M7.R2); `crates/ui-egui/src/viewport/mod.rs` `BINDINGS` | unresearched | - | - |
| dialog | Preferences | `ui.json` settings (M7.M3) | unresearched | - | - |
| window | Warnings window | `validate` findings and `load` problems and limitations (M7.M4): View > Warnings, a floating window; severity as glyph and word, filter, copy, double-click inspects | built, unobserved | - | test `windows::warnings::tests::rows_come_from_load_problems_limitations_and_findings`; tour `ck-warnings.png` (kept local) |
| window | Use Info (references to a form) | `refs --incoming` (M7.W5): View > Use Info, a floating window that follows the inspected record; double-click inspects the referrer | built, unobserved | - | test `windows::use_info::tests::it_follows_the_selection_once_and_reads_referrers`; tour `ck-use-info.png` (kept local) |
| dialog | Edit dialog: WEAP (tabs, groups, labels) | M7.W4, member paths per field; M7.W3 read-only dialog with KitKraft's own tab and group names, `crates/ui-egui/layouts/WEAP.toml` | built, unobserved | - | test `dialogs::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row`; tour `ck-edit-weap.png` (kept local) |
| dialog | Edit dialog: AMMO | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: COBJ | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: LCTN | M7.W4; M7.W3 read-only dialog, `crates/ui-egui/layouts/LCTN.toml` (names ours) | built, unobserved | - | test `dialogs::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: TERM | M7.W4; M7.W3 read-only dialog, `crates/ui-egui/layouts/TERM.toml` (names ours) | built, unobserved | - | test `dialogs::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: CONT | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: STAT | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: KYWD | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: ACTI | M7.W3 read-only dialog, `crates/ui-egui/layouts/ACTI.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: FACT | M7.W3 read-only dialog, `crates/ui-egui/layouts/FACT.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: PKIN | M7.W3 read-only dialog, `crates/ui-egui/layouts/PKIN.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: CELL | M7.W3 read-only dialog, `crates/ui-egui/layouts/CELL.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: REFR | M7.W3 read-only dialog, `crates/ui-egui/layouts/REFR.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog of any other type | M7.W3 default layout built from `schema` (one group per schema group) | built, unobserved | - | test `dialogs::tests::a_type_without_a_file_gets_a_default_layout_from_its_schema`; tour `ck-edit-default.png` (kept local) |
| window | Window arrangement (floating or docked windows) | M7.M0: floating `egui::Window`s in the main window, each toggled from View and remembered in `ui.json` (`ck_windows`); every workbench tab can float too (View > Workbench tabs as windows); a dock layout only if a sitting shows the CK's docks | built, unobserved | director assumption 2026-10-16, confirmed by the owner for now 2026-11-01 (to revisit: the 2026-11-01 probe saw a "Docks" menu) | test `windows::tests::the_layout_round_trips_and_ignores_damage`; tour `ck-layout.png` (kept local) |
| shortcut | Keyboard shortcuts, every one observed | M7.M5 | unresearched | - | - |
| workflow | New form, duplicate form | `new`, `clone` (M7.E3) | unresearched | - | - |
| workflow | Place an object into a cell | `place` (M7.E4). Observed: load the cell from the Cell View, drag the base object's row from the Object Window into the Render Window (the status bar names the new reference, `Crate_MilitarySupply_01 (01000807)`), double-click it for the Reference dialog, type the fields, OK. The CK numbers the reference itself (`000807` in a small-master-capable `.esp`) and makes it temporary | observed | `ck-20261101-03`, `ck-20261101-04` | - |
| dialog | Reference dialog (a placed reference) | `place` fields (M7.E4); the REFR edit dialog (M7.W3). Observed: fields Reference Editor ID, Base Object, Layer, Location Override; tabs 3D Data, Extra, Reflected by, Linked Ref, Linked From, Enable Parent, Emittance, MultiBound, Keywords, Location Ref Type, NavMesh Import Option, Attach Ref, Comments, Scripts; 3D Data holds Position X/Y/Z and Rotation X/Y/Z (in degrees), each with a step box, then Scale and Test Radius; 22 flag checkboxes below the tabs (Respawns and Allow Permanent Projected Decals ticked by default), none of them Persistent. A new EditorID raises "Rename Confirmation" ("in use by 0 objects and 0 references"; Yes) | observed | `ck-20261101-03` | - |
| workflow | Save the active file | `set`/`build` into the active file (M7.E1). Observed: File > Save Plugin saves an existing active file in place with no prompt (under MO2, into its mod folder); with no active file it opens "Select Target Plugin" (below). Ctrl+S did not save once, with the window not in front (not retried). File > Exit closed the CK with no prompt after a save | observed | `ck-20261101-02`, `ck-20261101-03` | - |
| dialog | Select Target Plugin (File > Save Plugin with no active file) | `build` to a new file (M7.E1). Observed: a file dialog in the game's `Data` folder, file name `*.esp`, type "TES Plugin Files (*.esp)"; under MO2 a new file lands in `overwrite\` | observed | `ck-20261101-03` | - |
| workflow | Convert the active file to a master | `deploy`/`build` header flags (none yet for an existing file). Observed: File > Convert Active File to ... offers Small Master, Medium Master and Full Master; Full Master asked "Convert the active file to a Master file?" and wrote a separate `.esm` next to the `.esp` (the `.esp` re-saved, its next object ID changed) | observed | `ck-20261101-03` | - |
