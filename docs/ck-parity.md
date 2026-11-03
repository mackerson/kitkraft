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
The menu order in the first row was a recollection until `ck-20261101-01` observed the
CK's twelve menus (filed in `ck-20261102-01`, M7.K3); the bar follows the observed order
since the menu-sync commit of 2026-11-02.

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
`docs/gaps.md` row when the gap is KitKraft's). A `menu item` row is one item of a CK
menu, named "Menu > Item" with the CK's label, directly below its `menu` row and in the
CK's order. Its "Maps to" is a catalog id or `gap:` with the reason, and only catalog ids
and `gap:` are in backticks there. The test of M7.K5 checks that every `menu item` row
maps to a catalog id that exists or to `gap:`.

Seeded 2026-10-16 by the roadmap lane with every row `unresearched`; the rows M7 lane M
built before any sitting are `built, unobserved`. Sitting 1
(M7.O2) fills the menus and windows, sitting 2 (M7.O3) the edit dialogs; either may be
an automated sitting. The 2026-11-01 probe and `ck-20261101-01` ran before the owner
accepted Amendment 2 the same day; their observations, with those of F30 and F29
(`ck-20261101-02`, `-03`), moved rows to `observed` in a change of their own. Today:
154 `observed` (133 of them `menu item` rows), 12 `built, unobserved` and 10
`unresearched` rows (counted 2026-11-02).

| Kind | CK element (as named in the CK) | Maps to | Status | Source | Evidence |
| ---- | ------------------------------- | ------- | ------ | ------ | -------- |
| menu | Main menu bar, order (observed: File, Edit, View, Docks, Galaxy, World, Terrain, NavMesh, Character, Gameplay, Audio, Help) | menu bar rendered from `menu_path` (M7.M1): these menus in this order (`crates/ui-egui/src/menu.rs` `CK_MENU_ORDER`), KitKraft's own menus (Record, Tools, Archive, Papyrus, Strings, Game) before Help; a CK menu without a command shows a disabled line | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-*.png`), filed in `ck-20261102-01`; the probe of 2026-11-01 (`observe\2026-11-01-ck-uia-probe\`) saw the same order with nothing loaded; the owner's recollection (2026-10-16) named eight of the twelve in the same relative order | tests `menu::tests::ck_menus_come_first_in_the_recorded_order_and_kitkraft_menus_before_help`, `menu::tests::every_ck_menu_row_in_the_parity_doc_maps_to_the_bar` |
| menu | File menu, every item | per item: 13 items in 5 groups, one menu item row each below | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-File.png`), filed in `ck-20261102-01` | - |
| menu item | File > Data... | `load` (the Data dialog, M7.M2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Save Plugin | `set`, `build` into the active file (M7.E1, M7.E5): each confirmed edit is written when it is made (ADR 0007 Amendment 4) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Import/Export | `gap:` not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Preferences... | `gap:` no command: the Preferences window (M7.M3) keeps workbench settings in ui.json | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Theme (experimental) | `gap:` no command: the workbench theme is a UI setting (M6) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Validate Loaded Data | `validate`. Observed disabled (data loaded, no active file) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Compact Active File Form IDs for ... | `gap:` no command compacts FormIDs | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Convert Active File to ... | `gap:` no command converts an existing file's header (see the row "Convert the active file to a master") | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Create Archive | `ba2.pack` | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Upload Plugin and Archive to Bethesda.net | `gap:` not offered: KitKraft has no Bethesda.net upload or login. Observed disabled (data loaded, no active file) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Login to Bethesda.net | `gap:` not offered: KitKraft has no Bethesda.net upload or login | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Logout | `gap:` not offered: KitKraft has no Bethesda.net upload or login. Observed disabled (data loaded, no active file) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | File > Exit | `gap:` no command: closing the window | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | Edit menu, every item (incl. undo, as far as offered) | per item: 12 items in 4 groups, one menu item row each below; undo is `revert` (M7.E6) | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-Edit.png`), filed in `ck-20261102-01` | - |
| menu item | Edit > Undo | `revert` (undo of the last edit, M7.E6) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Redo | `gap:` no redo (M7.E6 undoes the last edit only) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Cut Render | `gap:` no command: moving and copying references in the Render Window is M8 (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Copy Render | `gap:` no command: moving and copying references in the Render Window is M8 (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Paste Render | `gap:` no command: moving and copying references in the Render Window is M8 (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Paste in Place | `gap:` no command: moving and copying references in the Render Window is M8 (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Duplicate | `clone` (M7.E3) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Search & Replace... | `gap:` no search-and-replace command | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Find Text | `query` (a text match over records) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Find by Condition | `gap:` no search by condition | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Render Window Hotkeys | `gap:` no command: the Render Window tab lists its own bindings (M7.R1) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Edit > Picking Preferences | `gap:` not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | View menu, every item | per item: 31 items in 7 groups, one menu item row each below | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-View.png`), filed in `ck-20261102-01` | - |
| menu item | View > Toggle Render Features | `gap:` no command: the Render Window tab is read-only (M7.R) and has no such view option yet | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Refresh Render Window | `gap:` no command: the Render Window tab is read-only (M7.R) and has no such view option yet | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Object Windows | `query` (the Object Window, M7.W1) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Render Windows | `cell` (the Render Window tab draws a cell's references, M7.R2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Packin | `gap:` not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Toolbar | `gap:` no command: KitKraft has no toolbar | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Cell View | `cell` (the Cell View tab, M7.W2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Statusbar | `gap:` no command: window arrangement is workbench UI state, kept in ui.json (M7.M0) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Scene Info Window | `gap:` not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Open Windows... | `gap:` no command: window arrangement is workbench UI state, kept in ui.json (M7.M0) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Preview Window | `gap:` no command: no preview window yet | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Preview Animations | `gap:` no command: animation is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Show/Hide | `gap:` not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Reference Batch Action Window | `gap:` no batch action over references | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Layers | `gap:` no command for editor layers | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Default Layer Manager | `gap:` no command for editor layers. Observed disabled (data loaded, no active file) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Object Window Layouts | `gap:` no command: window arrangement is workbench UI state, kept in ui.json (M7.M0) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Material Editor | `gap:` no command: materials are M8 (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Material Palette | `gap:` no command: materials are M8 (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Material Swap Editor | `gap:` no command: materials are M8 (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > AVMS | `gap:` not known yet: grouped with the material items, its window not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Current Cell Only | `gap:` no command: the Render Window tab is read-only (M7.R) and has no such view option yet. Observed disabled (data loaded, no active file) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Isometric | `gap:` no command: the Render Window camera (M7.R1) has no fixed views yet | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Top | `gap:` no command: the Render Window camera (M7.R1) has no fixed views yet | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Depth Biasing | `gap:` no command: the Render Window tab is read-only (M7.R) and has no such view option yet | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Warnings | `validate` (the Warnings window, M7.M4) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Inspector | `gap:` not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Properties | `gap:` not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Particle Editor | `gap:` no command: particle effects are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Volumes | `gap:` not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | View > Reflection Probe | `gap:` not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | Docks menu, every item | per item: 4 items in 1 group, one menu item row each below. The screenshot shows more entries below a separator that the dump does not list under Docks: not counted, observed in sitting 1's rest | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-Docks.png`), filed in `ck-20261102-01` | - |
| menu item | Docks > Save Current Layout | `gap:` no command: window arrangement is workbench UI state, kept in ui.json (M7.M0) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Docks > Save Layout as... | `gap:` no command: window arrangement is workbench UI state, kept in ui.json (M7.M0) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Docks > Recent Layouts | `gap:` no command: window arrangement is workbench UI state, kept in ui.json (M7.M0) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Docks > Manage Layouts... | `gap:` no command: window arrangement is workbench UI state, kept in ui.json (M7.M0) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | Galaxy menu, every item | per item: 5 items in 2 groups, one menu item row each below | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-Galaxy.png`), filed in `ck-20261102-01` | - |
| menu item | Galaxy > Galaxy View | `gap:` no command: no galaxy view (`planet` reads one planet's content) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Galaxy > Block Pattern Editor | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Galaxy > Generate Locations | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Galaxy > Gen Overlay Placement Data | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Galaxy > Generate Biome Placements | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | World menu, every item | per item: 16 items in 7 groups, one menu item row each below | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-World.png`), filed in `ck-20261102-01` | - |
| menu item | World > World Spaces... | `cell` (the Cell View lists the worldspaces, M7.W2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Cells... | `cell` (the Cell View, M7.W2) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Go to Location | `gap:` no command: the Render Window shows the cell chosen in the Cell View | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > World Testing And Reports | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > View Render Test Failures... | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > View BetaComment Data | `gap:` no command | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Run Havok Sim | `gap:` no command: physics simulation is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Animate Lighting and Effects | `gap:` no command: lighting is M8 (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Create Local Maps | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Validate Room/Portal Alignment | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Align Tanget Space at NIF Intersection | `gap:` no command: world generation, tests and exports are not in M7. Observed disabled (data loaded, no active file) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Generate LOS | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Generate Max Height Data | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Force Weather | `gap:` no command: the Render Window draws no weather | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Toggle Wind | `gap:` no command: the Render Window draws no weather | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | World > Export MapMarker Data | `gap:` no command: world generation, tests and exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | Terrain menu, every item | per item: 8 items in 2 groups, one menu item row each below | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-Terrain.png`), filed in `ck-20261102-01` | - |
| menu item | Terrain > Landscape Editing | `gap:` no command: terrain editing is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Terrain > Landscape Cutting | `gap:` no command: terrain editing is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Terrain > Object Palette Editing (Qt) | `gap:` no command: object palettes are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Terrain > Heightmap Autopaint | `gap:` no command: terrain editing is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Terrain > Heightmap Import | `gap:` no command: terrain editing is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Terrain > Export Landscape... | `gap:` no command: terrain editing is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Terrain > Reimport All Blocks | `gap:` no command: terrain editing is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Terrain > Cell Terrain Data Viewer | `gap:` no command: terrain editing is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | NavMesh menu, every item | `gap:` navmesh generation is out of scope (ROADMAP M7); KitKraft draws the menu's items disabled. 8 items in 4 groups, one menu item row each below | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-NavMesh.png`), filed in `ck-20261102-01` | - |
| menu item | NavMesh > Generate Current Cell | `gap:` navmesh generation is out of scope (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | NavMesh > Generate Loaded Area | `gap:` navmesh generation is out of scope (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | NavMesh > Generate WorldSpace | `gap:` navmesh generation is out of scope (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | NavMesh > Cycle View Mode | `gap:` navmesh generation is out of scope (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | NavMesh > Draw Cover | `gap:` navmesh generation is out of scope (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | NavMesh > Draw Traversal | `gap:` navmesh generation is out of scope (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | NavMesh > Other Options | `gap:` navmesh generation is out of scope (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | NavMesh > Clean NavMesh Splines | `gap:` navmesh generation is out of scope (ROADMAP M7) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | Character menu, every item | per item: 15 items in 4 groups, one menu item row each below | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-Character.png`), filed in `ck-20261102-01` | - |
| menu item | Character > Filtered Dialogue... | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Export Relationship Data | `gap:` no command: character data exports are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Export Dialogue... | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Export Speech Challenge Scenes... | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Export Dialogue For Specific Voices | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Export Companion Affinity Event | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > List Neutral Emotion Dialogue | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > List Dialogue Needing Review | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Quest Voice Asset Totals | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Update NPC Body Tint Colors | `gap:` no command: character appearance tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Update FaceBones Model Availability | `gap:` no command: character appearance tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Export FaceGen Customization Textures | `gap:` no command: character appearance tools are not in M7. Observed disabled (data loaded, no active file) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Edit Player Dialogue | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Quest Aliases | `gap:` no command for quest aliases | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Character > Generate Player Address Lip Files | `gap:` no command: dialogue and voice tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | Gameplay menu, every item | per item: 12 items in 3 groups, one menu item row each below | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-Gameplay.png`), filed in `ck-20261102-01` | - |
| menu item | Gameplay > Settings... | `query` (game settings are GMST records; `set` edits one) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Papyrus Script Manager | `gap:` no script list; `papyrus.disasm` reads one compiled script | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Compile Papyrus Scripts... | `papyrus.compile` | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Migrate Papyrus Property Data... | `gap:` no command | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Export Papyrus Attach Data... | `gap:` no command: no export of script attachments | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Animations... | `gap:` no command: animation is not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Creature Creator | `gap:` no command: creature tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Camera Paths... | `gap:` no command | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Default Objects... | `gap:` no command for the default objects list | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Validate Forms | `validate` | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Recipe Manager | `query` (recipes are COBJ records) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Gameplay > Workshop Editor | `gap:` no command: workshop tools are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | Audio menu, every item | per item: 5 items in 1 group, one menu item row each below | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-Audio.png`), filed in `ck-20261102-01` | - |
| menu item | Audio > Animation Sound Tag Overrides... | `gap:` no command: audio tools (Wwise, FaceFX, voice files) are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Audio > Process Local Voice WAVs | `gap:` no command: audio tools (Wwise, FaceFX, voice files) are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Audio > Build Soundbank for Active File | `gap:` no command: audio tools (Wwise, FaceFX, voice files) are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Audio > Reload Wwise Data | `gap:` no command: audio tools (Wwise, FaceFX, voice files) are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Audio > Run FaceFX Compiler | `gap:` no command: audio tools (Wwise, FaceFX, voice files) are not in M7 | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu | Help menu, every item | per item: 4 items in 3 groups, one menu item row each below; no CK help text is reproduced | observed | `ck-20261101-01` (`12-menus.txt`, `shots\12-menu-Help.png`), filed in `ck-20261102-01` | - |
| menu item | Help > Contents | `gap:` no CK help text is reproduced; KitKraft's Help menu has its own (`schema`, `doctor`) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Help > Snap Windows | `gap:` no command: window arrangement is workbench UI state, kept in ui.json (M7.M0) | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Help > Third-Party Software Licenses | `gap:` no command: KitKraft lists its own in ATTRIBUTION.md | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| menu item | Help > About | `gap:` no command yet | observed | `ck-20261101-01`, filed in `ck-20261102-01` | - |
| dialog | Data dialog (choose masters and the active file; details) | `load` (M7.M2). Observed: a Win32 dialog (its rows are not exposed to UI Automation); each plugin listed with a status ("Plugin File", "Active File") and its parent masters; typing a name selects its row, a double-click ticks it, "Set as Active File" makes the selected plugin the active file; OK loads (about 2 minutes for `Starfield.esm`). A master-flagged plugin cannot be the active file. KitKraft addition, not a CK element (director ruling R17, 2026-11-02): an "Active file path:" field with "Use This File" takes a new or existing .esp or .esm outside the load order, the Data folders and MO2 as the active file. KitKraft's own, not observed in the CK: Cancel, the "Active file:" line, "No Active File" and the details pane | observed | `ck-20261101-01`, `-02`, `-03` (agent-run sittings, ADR 0012 Amendment 2) | - |
| window | Object Window (category tree, list, filter) | M7.W1 over `query`, categories M7.S2. Observed: a floating, dockable window; a category tree with "*All" at the top (about 15 s not responding while it fills), a Text Filter box, and a list whose rows show the EditorID, the FormID, the type and a use count (`Crate_MilitarySupply_01` `001E4FBD` STAT, 459, 460 once placed); a row drags into the Render Window to place it | observed | `ck-20261101-03` | - |
| window | Cell View (worldspace, cells, references) | `cell` (M7.W2, M7.S3): the Cell View tab (View > Cell View opens it; it floats like any tab; a tab since owner finding 1, 2026-11-01); the worldspaces (with "Interior" as a first entry) or one world's exterior cells with their grid on the left, the selected cell's references on the right, each paged; a click on a reference selects it, a double-click inspects it; the row of the inspected record (the shared selection with the Render Window) is drawn selected and the list scrolls to it once when the selection changes; "Show in Render Window" loads the cell there and the Render Window tab comes to the front (KK-037). Observed in the CK: View > Cell View is a toggle that shows or hides the window; a World Space list ("Interior" for interior cells), a Cell Filter box and a list of cells (a tooltip shows the name and FormID, e.g. `'CityNewAtlantisApartmentTEMPLATE' (0021966F)`); a double-click loads the cell into the Render Window | observed | `ck-20261101-03` | tests `windows::cell_view::tests::cell_view_lists_worlds_interiors_and_a_worlds_cells`, `windows::cell_view::tests::cell_view_reads_a_cells_references_and_pages`, `windows::cell_view::tests::the_cell_view_marks_the_selected_reference`, harness `app::harness::tests::finding_1_cell_view_is_a_tab`, `finding_2_the_cell_view_list_shows_its_rows` |
| window | Render Window | 3D viewport (M7.R1..R4), read-only. M7.R1 (2026-10-16): the workbench's "Render Window" tab with a grid, world axes, a horizon tint, one placeholder box and a stats overlay; M7.R2/R3 (2026-10-16): a cell's references as `OBND` boxes or type shapes from `cell --with-bounds`, colored by base type, click to select (shared with Records); the rotation convention is pinned from corpus evidence (`viewport/scene.rs` `ref_rotation`), not from the CK; M7.W2 (2026-10-27, KK-037): one shared selection with the Cell View — its "Show in Render Window" loads the cell through the tab's own picker (first page, then the pages), and the Cell View offers "Show this cell" when this window shows a different cell | unresearched | - | - |
| control | Render Window camera: orbit, pan, zoom, fly (mouse and keys) | M7.R1 bindings, **unobserved against the CK** (to be checked in sitting 1, M7.O2): orbit on right-drag, pan on middle-drag, zoom on the mouse wheel, fly with W A S D while the viewport has focus (Shift faster); select with a left click (M7.R2); `crates/ui-egui/src/viewport/mod.rs` `BINDINGS` | unresearched | - | - |
| dialog | Preferences | `ui.json` settings (M7.M3) | unresearched | - | - |
| window | Warnings window | `validate` findings and `load` problems and limitations (M7.M4): View > Warnings, a floating window; severity as glyph and word, filter, copy, double-click inspects | built, unobserved | - | test `windows::warnings::tests::rows_come_from_load_problems_limitations_and_findings`; tour `ck-warnings.png` (kept local) |
| window | Use Info (references to a form) | `refs --incoming` (M7.W5): View > Use Info, a floating window that follows the inspected record; double-click inspects the referrer | built, unobserved | - | test `windows::use_info::tests::it_follows_the_selection_once_and_reads_referrers`; tour `ck-use-info.png` (kept local) |
| dialog | Edit dialog: WEAP (tabs, groups, labels) | M7.W4, member paths per field; M7.W3 read-only dialog with KitKraft's own tab and group names, `crates/ui-egui/layouts/WEAP.toml` | built, unobserved | - | test `dialogs::layouts::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row`; tour `ck-edit-weap.png` (kept local) |
| dialog | Edit dialog: AMMO | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: COBJ | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: LCTN | M7.W4; M7.W3 read-only dialog, `crates/ui-egui/layouts/LCTN.toml` (names ours) | built, unobserved | - | test `dialogs::layouts::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: TERM | M7.W4; M7.W3 read-only dialog, `crates/ui-egui/layouts/TERM.toml` (names ours) | built, unobserved | - | test `dialogs::layouts::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: CONT | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: STAT | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: KYWD | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: ACTI | M7.W3 read-only dialog, `crates/ui-egui/layouts/ACTI.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::layouts::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: FACT | M7.W3 read-only dialog, `crates/ui-egui/layouts/FACT.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::layouts::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: PKIN | M7.W3 read-only dialog, `crates/ui-egui/layouts/PKIN.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::layouts::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: CELL | M7.W3 read-only dialog, `crates/ui-egui/layouts/CELL.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::layouts::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog: REFR | M7.W3 read-only dialog, `crates/ui-egui/layouts/REFR.toml` (names ours; not in sitting 2's first eight types) | built, unobserved | - | test `dialogs::layouts::tests::every_layout_field_exists_in_the_schema_and_every_layout_cites_a_parity_row` |
| dialog | Edit dialog of any other type | M7.W3 default layout built from `schema` (one group per schema group) | built, unobserved | - | test `dialogs::tests::a_type_without_a_file_gets_a_default_layout_from_its_schema`; tour `ck-edit-default.png` (kept local) |
| window | Window arrangement (floating or docked windows) | M7.M0: floating `egui::Window`s in the main window, each toggled from View and remembered in `ui.json` (`ck_windows`); every workbench tab can float too (View > Workbench tabs as windows); a dock layout only if a sitting shows the CK's docks | built, unobserved | director assumption 2026-10-16, confirmed by the owner for now 2026-11-01 (to revisit: the 2026-11-01 probe saw a "Docks" menu) | test `windows::tests::the_layout_round_trips_and_ignores_damage`; tour `ck-layout.png` (kept local) |
| shortcut | Keyboard shortcuts, every one observed | M7.M5 | unresearched | - | - |
| workflow | New form, duplicate form | `new`, `clone` (M7.E3) | unresearched | - | - |
| workflow | Place an object into a cell | `place` (M7.E4). Observed: load the cell from the Cell View, drag the base object's row from the Object Window into the Render Window (the status bar names the new reference, `Crate_MilitarySupply_01 (01000807)`), double-click it for the Reference dialog, type the fields, OK. The CK numbers the reference itself (`000807` in a small-master-capable `.esp`) and makes it temporary | observed | `ck-20261101-03`, `ck-20261101-04` | - |
| dialog | Reference dialog (a placed reference) | `place` fields (M7.E4); the REFR edit dialog (M7.W3). Observed: fields Reference Editor ID, Base Object, Layer, Location Override; tabs 3D Data, Extra, Reflected by, Linked Ref, Linked From, Enable Parent, Emittance, MultiBound, Keywords, Location Ref Type, NavMesh Import Option, Attach Ref, Comments, Scripts; 3D Data holds Position X/Y/Z and Rotation X/Y/Z (in degrees), each with a step box, then Scale and Test Radius; 22 flag checkboxes below the tabs (Respawns and Allow Permanent Projected Decals ticked by default), none of them Persistent. A new EditorID raises "Rename Confirmation" ("in use by 0 objects and 0 references"; Yes) | observed | `ck-20261101-03` | - |
| workflow | Save the active file | `set`/`build` into the active file (M7.E1). Observed: File > Save Plugin saves an existing active file in place with no prompt (under MO2, into its mod folder); with no active file it opens "Select Target Plugin" (below). Ctrl+S did not save once, with the window not in front (not retried). File > Exit closed the CK with no prompt after a save | observed | `ck-20261101-02`, `ck-20261101-03` | - |
| dialog | Select Target Plugin (File > Save Plugin with no active file) | `build` to a new file (M7.E1). Observed: a file dialog in the game's `Data` folder, file name `*.esp`, type "TES Plugin Files (*.esp)"; under MO2 a new file lands in `overwrite\` | observed | `ck-20261101-03` | - |
| workflow | Convert the active file to a master | `deploy`/`build` header flags (none yet for an existing file). Observed: File > Convert Active File to ... offers Small Master, Medium Master and Full Master; Full Master asked "Convert the active file to a Master file?" and wrote a separate `.esm` next to the `.esp` (the `.esp` re-saved, its next object ID changed) | observed | `ck-20261101-03` | - |
