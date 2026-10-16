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
| View     | Render window                             | -                 | P3       | deferred | -        |

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

The only observation source is the owner-assisted CK sittings (`plan/STATUS.md` rows 12
and 15, M7.O2, M7.O3). A computer-use agent may drive the CK while the owner watches.
Screenshots stay in `observe\` (gitignored). The public CK wiki is a level-4 source for
data facts (AGENTS.md "Clean room"), not interface observation: no row here moves to
`observed` on its word. The menu order in the first row stays `unresearched` until
sitting 1 (director decision 2026-10-16, `plan/execution-plan.md` "M6").

Status, one of:

- `unresearched`: not yet observed. What the row says comes from the owner's or a
  planner's recollection and is not evidence.
- `observed`: seen in a sitting, with the sitting id in "Source".
- `reproduced`: KitKraft has it as observed, with the tour image or test in "Evidence".
- `improved`: KitKraft has it and does more; "Evidence" says what more.

"Maps to" is a catalog command id, a UI element, or `gap:` with the reason (and a
`docs/gaps.md` row when the gap is KitKraft's). The test of M7.K5 checks that every
menu-item row maps to a catalog id that exists or to `gap:`.

Seeded 2026-10-16 by the roadmap lane. Every row is `unresearched`. Sitting 1 (M7.O2)
fills the menus and windows, sitting 2 (M7.O3) the edit dialogs.

| Kind | CK element (as named in the CK) | Maps to | Status | Source | Evidence |
| ---- | ------------------------------- | ------- | ------ | ------ | -------- |
| menu | Main menu bar, order (the owner recalls File, Edit, View, World, NavMesh, Character, Gameplay, Help) | menu bar rendered from `menu_path` (M7.M1) | unresearched | owner, 2026-10-16 (recollection) | - |
| menu | File menu, every item | per item | unresearched | - | - |
| menu | Edit menu, every item (incl. undo, as far as offered) | per item; undo M7.E6 | unresearched | - | - |
| menu | View menu, every item | per item | unresearched | - | - |
| menu | World menu, every item | per item | unresearched | - | - |
| menu | NavMesh menu, every item | `gap:` navmesh generation is out of scope (ROADMAP M7); items drawn disabled | unresearched | - | - |
| menu | Character menu, every item | per item | unresearched | - | - |
| menu | Gameplay menu, every item | per item | unresearched | - | - |
| menu | Help menu, every item | per item; no CK help text is reproduced | unresearched | - | - |
| dialog | Data dialog (choose masters and the active file; details) | `load` (M7.M2) | unresearched | - | - |
| window | Object Window (category tree, list, filter) | M7.W1 over `query`, categories M7.S2 | unresearched | - | - |
| window | Cell View (worldspace, cells, references) | M7.W2 over `cell` (M7.S3) | unresearched | - | - |
| window | Render Window | 3D viewport (M7.R1..R4), read-only | unresearched | - | - |
| control | Render Window camera: orbit, pan, zoom, fly (mouse and keys) | M7.R1 | unresearched | - | - |
| dialog | Preferences | `ui.json` settings (M7.M3) | unresearched | - | - |
| window | Warnings window | `validate` findings and `load` limitations (M7.M4) | unresearched | - | - |
| window | Use Info (references to a form) | `refs --incoming` (M7.W5) | unresearched | - | - |
| dialog | Edit dialog: WEAP (tabs, groups, labels) | M7.W4, member paths per field | unresearched | - | - |
| dialog | Edit dialog: AMMO | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: COBJ | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: LCTN | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: TERM | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: CONT | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: STAT | M7.W4 | unresearched | - | - |
| dialog | Edit dialog: KYWD | M7.W4 | unresearched | - | - |
| shortcut | Keyboard shortcuts, every one observed | M7.M5 | unresearched | - | - |
| workflow | New form, duplicate form | `new`, `clone` (M7.E3) | unresearched | - | - |
| workflow | Place an object into a cell | `place` (M7.E4) | unresearched | - | - |
| workflow | Save the active file | `set`/`build` into the active file (M7.E1) | unresearched | - | - |
