# CK sitting: fixture saves F28, F23, F20, F27 and what UI Automation cannot see

Owner decisions of 2026-11-05, items 11 and 12. One sitting, owner at the keyboard, an agent
beside him driving the frame, taking screenshots and writing the records. Prepared
2026-11-06 from the sources listed at the end. Nothing here has been run.

How to read the steps:

- **Owner** = only you can do it (the box or menu is not visible to the agent's tools, or it
  needs your eyes on a list the CK does not publish).
- **Agent** = the agent does it from its PowerShell window.
- **Either** = the agent can do it, but you may do it faster by hand.
- *(never observed: find it and tell the agent what you clicked)* = no sitting has ever
  seen this menu path or field. Do not guess; find it, say out loud exactly what you
  clicked (menu, item, tab, label), and the agent writes it down.
- "Observed" next to a step means an earlier sitting saw it work that way.

Rules that hold all the time:

- Only the MO2 profile `Kitkraft Dev` (folder `C:\Modding\MO2-Starfield\profiles\Kitkraft Dev`).
  Never `Default`. The CK only ever starts through MO2, never from Steam or the game
  folder: a save made outside MO2 would go into the real `Data` folder.
- Saves: only `KC_F28.esp`, `KC_F23.esp`, `KC_F20.esp`, `KC_F27.esp`, each in its own launch.
  No other save. In launch 1 (Part B) nothing is saved at all.
- Never press OK, Apply or Save unless the step says so. Close dialogs with Cancel (or the
  window's X where the step says so).
- Any message box or prompt the step does not mention: stop, do not answer it, let the agent
  take a screenshot, then decide together.
- Nothing is read out of the CK's files. Labels only: no tooltip, status-bar or help text is
  written down.

## Overview

| # | Item | Who | Order | Unblocks |
| - | ---- | --- | ----- | -------- |
| 0 | Before you start (closed programs, preflight, backups) | Agent; Owner closes programs | before each launch | everything below |
| B1 | Menu shortcuts (S1-06) | Owner reads, Agent records | launch 1 | parity "Keyboard shortcuts, every one observed" (M7.M5) |
| B2 | Render Window Hotkeys bindings (S1-06) | Owner | launch 1 | same row; M7.R1 camera (R4) |
| B3 | Preferences, every page (S1-08) | Owner clicks tabs, Agent screenshots | launch 1 | parity "Preferences" (M7.M3 tour images) |
| B4 | Object Window category tree (S1-09) | Owner | launch 1 | parity "Object Window (category tree, list, filter)", categories of M7.S2 |
| B5 | Object Window row context menu (S1-11) | Owner | launch 1 | parity "Use Info (references to a form)"; the Duplicate step of F28 |
| B6 | WEAP edit dialog (S2-01) | Owner picks the row; Either for tabs | launch 1 | parity "Edit dialog: WEAP" (M7.W4, M7.K4) |
| B7 | LCTN dialog tab names | Owner reads, Agent screenshots | launch 1 | parity "Edit dialog: LCTN" (M7.W4) |
| B8 | Edit > Undo with a disposable edit | Owner | launch 1, after B1-B7 | parity "Edit > Undo" mapping (`revert`, M7.E6); E's ask |
| B9 | Render Window camera (S1-07) | Owner | launch 1, last | parity "Render Window camera" (M7.R1, R4) |
| B10 | Submenus (S1-04), optional | Owner reads | launch 1, only if time | the 12 submenu `menu item` rows |
| A1 | F28 save: vanilla outpost object duplicated | Owner (right-click, save box); Agent the rest | launch 2 | STATUS row 15 (M4.O1), M5.A5, M4.H2; parity "New form, duplicate form" |
| A2 | F23 save: one vanilla reference moved | Owner | launch 3 | row 15, M5.A5, M4.H2; M4.A6 (`ONAM`) |
| A3 | F20 save: new interior cell with one STAT | Owner | launch 4 | row 15, M5.A5, M4.H2; M4.A1/M4.A5; base of F22, F25, F26 |
| A4 | F27 save: owner of one vanilla interior cell changed | Owner | launch 5 | row 15, M5.A5, M4.H2; M4.E8 ownership members |
| 9 | End of each launch (restore, checks) | Agent | after each launch | - |

Why five launches: every fixture's base is F01 (`Starfield.esm` only, no active file), so
each starts from a fresh load; the frame lets exactly one fixture file be written per launch
(`-SaveAllowed F28` allows only a file whose name holds `F28`); and Part B must run in a
launch where no save is allowed at all. Each load took 115 to 134 s in the 2026-11-02/03
sittings.

The A part keeps the order the owner OK'd: F28, F23, F20, F27. Part B runs first because
it saves nothing and checks the frame before the first save. If you prefer Part B last,
see "Questions".

## 0. Before you start (every launch)

### 0.1 What must be closed (Owner)

1. Close Starfield, the SFSE loader, the Creation Kit and Mod Organizer 2. The frame refuses
   to start while any of `Starfield`, `sfse_loader`, `CreationKit`, `ModOrganizer` runs.
2. Do not start MO2 yourself: the frame starts it on `Kitkraft Dev`.
3. Before launch 2 onward: MO2's `C:\Modding\MO2-Starfield\overwrite\` must hold nothing from
   the previous fixture (see "What to do with the saved file" in each A block). It was empty
   when checked on 2026-11-06.

### 0.2 The scripts (where they live)

All in `C:\Modding\kitkraft-observe\scripts\` (outside the repo, not committed):

- `ck-run.ps1`: runs one sitting: preflight, settings backup, launch, a steps file, then CK
  and MO2 closed, settings restored, postflight.
- `ck-sitting.ps1`: the frame functions (`Invoke-Preflight`, `Backup-CkSettings`,
  `Start-CkThroughMo2`, `Close-Ck`, `Close-Mo2`, `Restore-CkSettings`, `Invoke-Postflight`).
- `ck-lib.ps1`, `ck-input.ps1`, `ck-steps-lib.ps1`: UI Automation, input, screenshots
  (`Shot`), dumps (`Save-Dump`), `Load-Data`.
- `ck-sitting.selftest.ps1`: a desk test of the frame that launches nothing.

**Missing: there is no steps file for an owner-driven launch.** The three steps files that
exist (`steps-sitting1-rest.ps1`, `steps-sitting2.ps1`, `steps-F28.ps1`) drive the CK
themselves and do not wait for the owner. `ck-run.ps1` runs only a steps file that carries a
line `# READY`, and refuses `-SaveAllowed` with a file that carries `# OBSERVE-ONLY`. Before
the sitting the agent must write two small steps files (not written here; this preparation
changed nothing):

- `steps-owner-observe.ps1`: lines `# READY` and `# OBSERVE-ONLY`, refuses any
  `-SaveAllowed`, then simply waits until the `CreationKit` process has exited (the owner
  closes the CK himself), logging the time.
- `steps-owner-fixture.ps1`: line `# READY` only, refuses to run without `-SaveAllowed`,
  then waits the same way and, after the CK exits, copies the fixture's file out of
  `overwrite\` into the sitting folder (as the last block of `steps-F28.ps1` does) and logs
  its size and SHA-256.

The agent runs the desk test once before launch 1:
`powershell -File C:\Modding\kitkraft-observe\scripts\ck-sitting.selftest.ps1`

### 0.3 Preflight and backups (Agent)

The agent runs, from `C:\Modding\kitkraft-observe\scripts\` (sitting ids are
`ck-<yyyymmdd>-<nn>` of the sitting's date, numbered 01, 02, ... ; one per launch):

```powershell
# launch 1, Part B (no save)
powershell -File .\ck-run.ps1 -Id ck-<yyyymmdd>-01 -Name partB -Steps .\steps-owner-observe.ps1
# launches 2 to 5, one fixture each
powershell -File .\ck-run.ps1 -Id ck-<yyyymmdd>-02 -Name F28 -Steps .\steps-owner-fixture.ps1 -SaveAllowed F28
powershell -File .\ck-run.ps1 -Id ck-<yyyymmdd>-03 -Name F23 -Steps .\steps-owner-fixture.ps1 -SaveAllowed F23
powershell -File .\ck-run.ps1 -Id ck-<yyyymmdd>-04 -Name F20 -Steps .\steps-owner-fixture.ps1 -SaveAllowed F20
powershell -File .\ck-run.ps1 -Id ck-<yyyymmdd>-05 -Name F27 -Steps .\steps-owner-fixture.ps1 -SaveAllowed F27
```

To check the preflight alone without launching anything, add `-PreflightOnly`.

What the preflight does (refuses and stops if any check fails):

1. No `Starfield`, `sfse_loader`, `CreationKit` or `ModOrganizer` process runs.
2. The profile is `Kitkraft Dev` (never `Default`) and its folder exists; `CreationKit.exe` is
   in `C:\Program Files (x86)\Steam\steamapps\common\Starfield`.
3. It writes `preflight.json` into the sitting folder
   `C:\Modding\kitkraft-observe\ck-<yyyy-mm-dd>\<name>\`: the SHA-256 of `CreationKit.exe`
   (a file hash only, allowed by owner decision 10), the hashes of the CK's two settings
   files, the hashes of the dev profile's `modlist.txt`, `plugins.txt`, `loadorder.txt`, a
   listing of every file in `overwrite\` and every plugin of each `KitKraft Dev - *` mod,
   and the hashes of `Default`'s three list files and `ModOrganizer.ini` (watched, never
   written).
4. It copies the dev profile's three list files into `lists-before\` (MO2 rewrites
   `plugins.txt` and `loadorder.txt` when it launches; the copy is how they are put back).

Then `Backup-CkSettings` copies `CreationKitPrefs.ini` and `QtCreationKitSavedSettings.ini`
from the game folder into `settings-before\` with a manifest of their hashes (it refuses to
overwrite an earlier backup). Without `-Defaults` the CK keeps its current window layout.

### 0.4 Launch (Agent)

`ck-run.ps1` starts
`C:\Modding\MO2-Starfield\ModOrganizer.exe -p "Kitkraft Dev" "moshortcut://:Creation Kit"`
and waits up to 300 s for the CK's main window. The session log says
`CK main window up (pid ...)`. Then the steps file waits for you.

Note: the agent's own clicks and keys stop by themselves whenever a CK window is not in
front (`ck-input.ps1`), so it will not type into another program while you work.

### 0.5 Load the data (Either; the same in every launch)

Observed (ck-20261101-01..03, ck-20261102-02, -03, ck-20261103-01):

1. File > Data...
2. In the list, find `Starfield.esm` (typing its name selects it) and **double-click** it to
   tick it. A single click or Space did not tick it.
3. Tick nothing else. Do not set an active file.
4. OK. The load takes about two minutes (115 to 134 s observed). It is done when the
   title bar shows `Starfield Creation Kit 64-bit [Branch: Main, Version: 1.16.244.0]`.
   **Agent:** write the title into the log (`Save-CkTitle`) and take a screenshot.
   If the version is not 1.16.244.0, say so before going on.

### 0.6 Screenshots and records (Agent, all through the sitting)

- Every state written down gets a screenshot at the same moment into the sitting's
  `shots\` folder, named `<item>-<nn>.png` (e.g. `b2-hotkeys-03.png`). Where UI Automation
  sees the window, also a dump (`Save-Dump`).
- Record form for every Part B item, appended to the sitting's `session.md`:

```text
Item: B2 Render Window Hotkeys bindings (S1-06)
Sitting: ck-<yyyymmdd>-01 (owner-assisted), CK 1.16.244.0 (title), <date> <time>
Opened by: <exact clicks>
Seen by: owner, read aloud; screenshots shots\b2-hotkeys-01.png .. -NN.png
Observed (exact text, in screen order; one line per entry):
  ...
Not seen or unclear: ...
Status: observed | observed in part | inconclusive (why) | not-run (why)
```

- Status words, as the ledger uses them: **observed** = seen on screen and in a screenshot
  of that moment; **observed in part** = some of it; **inconclusive** = tried and could not be
  seen or confirmed (say why); **not-run** = not attempted (say why). Values in fields
  (EditorIDs, numbers, file paths) are data, not labels: write them only where a fixture
  needs them.
- After the sitting the agent turns each record into one entry in
  `docs/ck-observations.md` (ledger format), as an owner-assisted sitting. That is a repo
  change for later, not part of the sitting.

## Part B: what UI Automation cannot see (launch 1, nothing saved)

Start with 0.1 to 0.5 (no `-SaveAllowed`). After the load: Object Window, click the
category "*All" and wait (about 15 s "not responding" while the list fills, observed).
The sources disagree on where "*All" sits in the tree: the parity row says at the top,
the sitting 2 notes say it is the last item. Use whichever you see and say which.

### B1. Menu shortcuts (S1-06)

UI Automation exposed no shortcut on any of the 133 items; the 2026-11-01 screenshots show
one beside 17 items.

1. **Owner:** open each top menu in turn: File, Edit, View, Docks, Galaxy, World, Terrain,
   NavMesh, Character, Gameplay, Audio, Help.
2. **Owner:** for each item that shows a shortcut, read out `Menu > Item` and the shortcut
   exactly as drawn (e.g. letters, `Ctrl+`, symbols, spacing as shown).
3. **Owner:** also say which items show a check mark and which show a submenu arrow.
4. **Agent:** one screenshot per open menu (`b1-<menu>.png`).
5. **Owner:** Escape to close each menu. Click nothing in them.
6. **Agent:** record one line per item that has a shortcut: `Edit > Undo = <text as shown>`;
   then a line per check mark and per arrow. Compare the count with 17 and note any
   difference.

### B2. Render Window Hotkeys bindings (S1-06)

Observed: Edit > Render Window Hotkeys opens "Render Window Hotkeys (RenderWindow[0])" with
a list (rows not visible to UI Automation), a disabled combo box, buttons Assign, Alt,
Control, Shift, Revert and a Filter box.

1. **Owner:** Edit > Render Window Hotkeys.
2. **Owner:** do not click Assign, Alt, Control, Shift or Revert, and do not type in the
   Filter box.
3. **Owner:** read the list from top to bottom: for each row, its name and its key exactly
   as shown, and the column headings if there are any. Scroll a page at a time.
4. **Agent:** one screenshot per page (`b2-hotkeys-01.png` ...), before each scroll.
5. **Owner:** close the dialog with its window's X (that is how it was closed before, with
   no prompt).
6. **Agent:** record the rows in list order, one per line, `<name> = <key as shown>`, plus
   the row count. Check the title bar is unchanged (no `*`).

### B3. Preferences, every page (S1-08)

Observed: File > Preferences... opens "Preferences" with buttons Apply and Close and a tab
control. Two pages were seen (the one shown on opening: snap, gizmo, camera and movement
settings; another with Enable Shadows, Debug Mode, Texture debug, Enlighten and Visualize).
The tab names were never readable by UI Automation.

1. **Owner:** File > Preferences...
2. **Owner:** read the tab names left to right, exactly as shown. Say which tab is selected
   on opening.
3. **Owner:** click each tab in turn. On each page read the group titles and the labels of
   every checkbox, box and list, top to bottom, left to right, and say which checkboxes are
   ticked. Change nothing.
4. **Agent:** one screenshot per page (`b3-prefs-<tab number>.png`); a dump of the dialog
   per page.
5. **Owner:** never press Apply. Close with the window's X (as before). Whether the Close
   button applies changes is not known, so do not use it.
6. **Agent:** record: tab names in order; per tab, its labels in screen order and the
   ticked ones.

### B4. Object Window category tree (S1-09)

The tree's items are not visible to UI Automation (inconclusive twice).

1. **Owner:** in the Object Window's tree, expand every node (click each `+` or arrow) from
   top to bottom until nothing is left to expand. Expanding is safe; do not drag anything.
2. **Owner:** read every node, top to bottom, saying its depth (top level, one in, two in).
3. **Agent:** screenshots as the tree scrolls (`b4-tree-01.png` ...). Also screenshot the
   list's column headings.
4. **Agent:** record the tree as an indented list in screen order, and the column headings
   in order. Note where "*All" sits.
5. **Owner:** say which node holds weapons (needed for B6).

### B5. Object Window row context menu (S1-11)

Seen once in a screenshot (2026-11-03): Edit, Duplicate, Delete, Rename, Duplicate and
Rename, and more. A right-click posted by the agent opened no menu, and the menu's items are
not visible to UI Automation.

1. **Owner:** in the Object Window's Text Filter type `Crate_MilitarySupply_01` (observed:
   one row, `001E4FBD`, STAT).
2. **Owner:** right-click the row.
3. **Owner:** read every entry top to bottom, exactly as shown, with separators, greyed-out
   entries, check marks, submenu arrows and any shortcut text.
4. **Agent:** screenshot with the menu open (`b5-context.png`).
5. **Owner:** if the menu has an entry for where the form is used (a "use info" style entry),
   click that one only, read the window's title and labels, let the agent screenshot it,
   then close it with its window's X. Otherwise click nothing.
6. **Owner:** Escape to close the menu.
7. **Agent:** record entries in order, separators as `/`, greyed as *(off)*; check the
   title bar has no `*`.

### B6. WEAP edit dialog (S2-01)

Twice the Text Filter's first row for `RussianHuntingRifle` opened a Keyword dialog
instead of the weapon. The weapon is `Starfield.esm:0021BBCD`, type WEAP.

1. **Owner:** select the weapon category in the tree (from B4; its name is not known yet).
2. **Owner:** Text Filter: `RussianHuntingRifle`.
3. **Owner:** find the row whose FormID column shows `0021BBCD` and type shows WEAP, and
   double-click it.
4. **Owner:** check the dialog's ID box shows `RussianHuntingRifle`. If not, Cancel and say
   what opened.
5. **Owner:** read the dialog's title, then its tab names left to right; click each tab and
   read its group titles and labels top to bottom. Change nothing.
6. **Agent:** a screenshot and a dump per tab (`b6-weap-<nn>`); UI Automation saw the other
   seven dialogs' labels, so the dump may carry them.
7. **Owner:** Cancel. Never OK.
8. **Agent:** record title, tabs in order, labels per tab; check the title bar has no `*`.

### B7. LCTN dialog tab names

Observed: `CityNewAtlantisLocation` (`Starfield.esm:01295A`) opens "Location" with a
three-tab control in its right pane; the tab names were in the screenshot only.

1. **Owner:** select "*All" again; Text Filter: `CityNewAtlantisLocation`; double-click the
   row whose type is LCTN and ID box shows `CityNewAtlantisLocation`.
2. **Owner:** read the three tab names left to right, exactly as shown; click each and say
   which labels belong to it.
3. **Agent:** a screenshot per tab (`b7-lctn-<n>.png`).
4. **Owner:** Cancel.
5. **Agent:** record the tab names in order and the labels under each.

### B8. Edit > Undo, with a disposable edit

Never observed: with nothing edited there was nothing to undo.

The disposable edit: place one crate in the cell used for F29 and undo it. Placing is an
observed workflow (ck-20261101-03), and the crate's use count in the Object Window shows
whether it came and went (459, 460 once placed).

Why it cannot end up in a saved file:

- This launch runs without `-SaveAllowed`; the postflight fails the sitting if any plugin
  file in `overwrite\` or a dev mod is new or changed.
- No active file is set, so any save opens "Select Target Plugin" and needs a name typed
  and Save clicked (observed). In this launch nobody types a name or clicks Save; if that
  dialog ever appears, Cancel it.
- The edit is undone at once, and anything left over is thrown away when the CK closes
  without saving (step 9 below).

Steps:

1. **Owner:** before any edit, open Edit and say whether Undo and Redo are greyed out and
   what text they show. **Agent:** screenshot `b8-01-edit-before.png`.
2. **Owner:** View > Cell View (observed; it is a toggle). World Space: "Interior". Cell
   Filter: `CityNewAtlantisApartmentTEMPLATE`. Double-click the cell to load it into the
   Render Window (observed).
3. **Owner:** Object Window: "*All", Text Filter `Crate_MilitarySupply_01`. Note the count
   column (459 observed before).
4. **Owner:** drag the row into the Render Window (observed). Say what the status bar names,
   the new count, and whether the title bar now ends with `*`.
   **Agent:** screenshot `b8-02-placed.png`.
5. **Owner:** open Edit; read Undo's and Redo's text exactly as shown now (it may name the
   action). **Agent:** screenshot `b8-03-edit-after.png`.
6. **Owner:** click Edit > Undo. Say whether the crate disappeared, the count, and whether
   the `*` went. **Agent:** screenshot `b8-04-undone.png`.
7. **Owner:** open Edit again and read Undo's and Redo's text. Do not click Redo. Escape.
   **Agent:** screenshot `b8-05-edit-final.png`.
8. If Undo is greyed out at step 5, or does nothing at step 6: record that, do not try other
   edits, go on. The edit is thrown away at the end.
9. **Agent:** record each step's text and state. The parity row maps Undo to `revert`; say
   whether the CK's Undo covers a placement.

### B9. Render Window camera (S1-07), last in this launch

The cell from B8 is still loaded. Never observed. Do not left-drag in the Render Window: it
may move a reference (not known).

For each input below:

1. **Agent:** screenshot `b9-<n>-before.png`.
2. **Owner:** with the mouse over the Render Window, do the one input, short and small.
3. **Agent:** screenshot `b9-<n>-after.png`; read the title bar (a `*` means something was
   edited: say so).
4. **Owner:** say what moved: the view turned around a point, slid sideways, zoomed in or
   out, moved forward or back, or nothing.

Inputs, in this order: right-drag left to right; middle-drag left to right; wheel one notch
forward; wheel one notch back; hold W one second; then A, S, D; then Shift+W.

5. **Agent:** record one line per input: input, what moved, `*` or not. If you know of other
   camera inputs (for example a key that frames the selection), try them only if they are
   in the B1 or B2 list, and record them the same way.

### B10. Submenus (S1-04), optional, not in decision 12

The 12 submenus opened in the screenshots but their items were not visible to UI
Automation: File > Import/Export, Theme (experimental), Compact Active File Form IDs for
..., Convert Active File to ...; View > Toggle Render Features, Object Windows, Render
Windows, Packin; Docks > Recent Layouts; World > World Testing And Reports, Generate Max
Height Data; NavMesh > Other Options.

1. **Owner:** hover each, read its items in order (with arrows, check marks, greyed ones).
   Click nothing.
2. **Agent:** a screenshot per submenu (`b10-<nn>.png`); record.

### End of launch 1

1. **Owner:** File > Exit. If the CK asks whether to save: answer the choice that does not
   save, and read its wording aloud first so the agent can screenshot it (this prompt has
   never been seen; its buttons are not known).
2. Then section 9.

## Part A: the four fixture saves (launches 2 to 5)

For each: section 0.1 to 0.5 with `-SaveAllowed <id>`, then the block, then section 9.

The save dialog (observed, ck-20261101-03 and ck-20261103-01): with no active file,
File > Save Plugin opens "Select Target Plugin", a Windows save dialog in `Starfield\Data`
with the file name box holding `*.esp` and the type "TES Plugin Files (*.esp)". Under MO2 a
new file lands in `C:\Modding\MO2-Starfield\overwrite\`, not in the real `Data` folder.
UI Automation cannot reach its file name box, so typing the name and clicking Save are
always **Owner** steps. Do not change the folder in that dialog.

### A1. F28: a vanilla outpost object duplicated in the CK

- Matrix (M4.H1): base F01; first question "the CK's clone choices (FormID, EditorID, what is
  copied) against `clone`".
- Closes: its part of STATUS row 15 (M4.O1), M5.A5, M4.H2; parity "New form, duplicate form"
  (unresearched).
- The object (chosen for the 2026-11-03 attempt): STAT `CoffeeBrewingMachine01`,
  `Starfield.esm:136FDF`, the object the outpost recipe `co_Outpost_Decor_CoffeeMaker01`
  builds.
- The automated attempt (ck-20261103-01) got as far as the duplicate; it stopped at the
  file name box.

Steps:

1. **Either:** Object Window, "*All", Text Filter `CoffeeBrewingMachine01`. Observed: two
   rows, `CoffeeBrewingMachine01` (STAT) and `Outpost_CoffeeBrewingMachine01` (TRNS).
2. **Owner:** double-click the STAT row; check the ID box shows `CoffeeBrewingMachine01`;
   read its FormID from the list (`00136FDF` expected). Cancel.
3. **Owner:** right-click the STAT row. Read the menu entries aloud (if B5 already recorded
   them, just confirm). **Agent:** screenshot `f28-01-context.png`.
4. **Owner:** click Duplicate (the plain one, not "Duplicate and Rename"). Observed in
   2026-11-03 attempts 5 to 7: no prompt; the list went from 2 rows to 3 and the title bar
   gained `*`.
5. **Owner:** read the new row's EditorID, FormID and type. Double-click it, read its ID box
   and Name box, let the agent screenshot (`f28-02-copy.png`), then Cancel.
6. **Owner:** File > Save Plugin. "Select Target Plugin" opens.
7. **Owner:** in the file name box replace `*.esp` with `KC_F28.esp`. Click Save.
8. **Owner:** if any message appears, read it aloud and let the agent screenshot it before
   you answer (none is expected; none was seen in F29's save).
9. **Owner:** read the title bar after the save.
10. **Agent:** check that `C:\Modding\MO2-Starfield\overwrite\KC_F28.esp` exists; log its
    size and SHA-256; screenshot.
11. **Owner:** File > Exit (observed: no prompt after a save).

Check afterwards (Agent): the postflight says `ok`; `KC_F28.esp` is the only plugin file new
in `overwrite\` (the CK's `Backup\AutoSaveDEFAULT.bak` is expected beside it); nothing changed
in any `KitKraft Dev - *` mod.

What to do with the saved file:

1. **Agent:** copy `overwrite\KC_F28.esp` into the sitting folder
   `C:\Modding\kitkraft-observe\ck-<yyyy-mm-dd>\F28\` (the copy keeps its name; `steps-F28.ps1`
   named its copy `ck.esp`), and check the copy's SHA-256 equals the original's.
2. **Owner:** with MO2 closed, delete `KC_F28.esp` and `Backup\AutoSaveDEFAULT.bak` from
   `C:\Modding\MO2-Starfield\overwrite\` (agents write nothing under MO2 except `kit
   deploy`; the F29 files were left to you the same way). Do this before the next launch.
3. Later, not in the sitting: the agent makes the fixture folder and runs the ingestion
   (`kit roundtrip --verify --decode`, `kit diff`, `cargo xtask observe`, `cargo xtask oracle
   ck`). See "Not verified" about the fixture folder and `fixture.toml`.

### A2. F23: one vanilla reference moved

- Matrix: base F01; first question "`ONAM` membership for an overridden temporary child".
- Closes: its part of row 15, M5.A5, M4.H2; M4.A6 (the `ONAM` rule needs F23).
- **Not defined anywhere:** which reference, and how far to move it. Proposal (confirm it):
  a temporary vanilla reference in `CityNewAtlantisApartmentTEMPLATE` (the F29 cell, whose
  load path is observed), moved by setting Position X to the float sentinel `1234.5` that the
  M3 fixtures use.

Steps:

1. **Owner:** View > Cell View, World Space "Interior", Cell Filter
   `CityNewAtlantisApartmentTEMPLATE`, double-click the cell (observed).
2. **Owner:** choose one vanilla reference in the Render Window and double-click it to open
   its Reference dialog. Double-clicking a reference opened the Reference dialog for a
   placed crate (observed); for a vanilla reference it was never tried, and choosing one
   from the Cell View instead was never observed *(find it and tell the agent what you
   clicked)*.
3. **Owner:** read Reference Editor ID, Base Object, and the 3D Data tab's Position X/Y/Z and
   Rotation X/Y/Z (observed fields). Read the reference's FormID wherever the CK shows it
   (where it shows it is not known). **Agent:** screenshot `f23-01-before.png`.
4. **Owner:** on 3D Data, set Position X to `1234.5` (or the value agreed). Change nothing
   else, not the EditorID (a new EditorID raised "Rename Confirmation" in F29).
5. **Owner:** OK. Any message: read it aloud, agent screenshots, then decide.
   **Agent:** screenshot `f23-02-after.png`; read the title (`*` expected).
6. **Owner:** File > Save Plugin; file name `KC_F23.esp`; Save.
7. **Agent:** check `overwrite\KC_F23.esp`; size and SHA-256.
8. **Owner:** File > Exit.

Check afterwards and the saved file: as A1, with `F23` in place of `F28` (copy to
`ck-<yyyy-mm-dd>\F23\`, owner deletes `KC_F23.esp` and `Backup\AutoSaveDEFAULT.bak` from
`overwrite\`). Record the reference's FormID, EditorID, base and old and new position.

### A3. F20: a new interior cell with one placed vanilla STAT

- Matrix: base F01; first question "group structure of a new interior cell; block and
  sub-block; `INCC`; persistent or temporary by default".
- Closes: its part of row 15, M5.A5, M4.H2; M4.A1 and M4.A5 evidence; F22, F25 and F26 are
  built on F20 (not OK'd, not in this sitting).
- **Not defined anywhere:** the STAT, the cell's EditorID, the position. Proposal (confirm
  it): STAT `Crate_MilitarySupply_01` (`Starfield.esm:1E4FBD`, the F29 crate; drag into the
  Render Window is observed), cell EditorID `KC_F20_Cell`, the crate left where it drops.

Steps:

1. **Owner:** create a new interior cell. How the CK does this was never observed (World >
   Cells... exists as a menu item but was never opened) *(find it and tell the agent what
   you clicked)*. Read every field of the dialog it opens; set only the EditorID
   (`KC_F20_Cell` or the agreed one) and whatever the CK requires to create it; say what
   you set. **Agent:** screenshot each dialog (`f20-01-...`).
2. **Owner:** load the new cell into the Render Window (for an existing cell: Cell View,
   "Interior", double-click; for a new one not observed *(tell the agent what you
   clicked)*).
3. **Owner:** Object Window, "*All", Text Filter `Crate_MilitarySupply_01`; drag the row
   into the Render Window (observed). Say what the status bar names.
4. **Owner:** double-click the placed crate; read the Reference dialog's fields and 3D Data
   values and which flag checkboxes are ticked (Respawns and Allow Permanent Projected
   Decals were ticked by default in F29). Change nothing; Cancel.
   **Agent:** screenshot `f20-02-ref.png`.
5. **Owner:** File > Save Plugin; file name `KC_F20.esp`; Save. Any message: read it aloud
   first.
6. **Agent:** check `overwrite\KC_F20.esp`; size and SHA-256.
7. **Owner:** File > Exit.

Check afterwards and the saved file: as A1, with `F20`.

### A4. F27: the owner of one vanilla interior cell changed

- Matrix: base F01; first question "the ownership members M4.E8 writes".
- Closes: its part of row 15, M5.A5, M4.H2; the CK side of M4.E8 (`docs/recipes/ownership.md`
  says "the CK's own choice of members is fixture F27").
- **Not defined anywhere:** the cell and the new owner (the recipe says lane H picks the
  F27 cell). Proposal (confirm it), taken from KitKraft's own ownership recipe so the two
  files can be compared: cell `CityNewAtlantisFreestarEmbassy` (`Starfield.esm:017307`,
  owned by `Starfield.esm:24F839` today), new owner `PlayerFaction` (`Starfield.esm:01C21C`).

Steps:

1. **Owner:** open the cell's own properties dialog. Never observed: the Cell View's
   double-click loads a cell, it does not open its properties; World > Cells... exists but
   was never opened *(find it and tell the agent what you clicked)*.
2. **Owner:** read the dialog's title, tabs and the ownership labels (never observed).
   **Agent:** screenshot `f27-01-before.png`.
3. **Owner:** set the owner to `PlayerFaction` (or the agreed one). Change nothing else.
   Say exactly which control you used.
4. **Owner:** OK. Any message: read it aloud first. **Agent:** screenshot; read the title.
5. **Owner:** File > Save Plugin; file name `KC_F27.esp`; Save.
6. **Agent:** check `overwrite\KC_F27.esp`; size and SHA-256.
7. **Owner:** File > Exit.

Check afterwards and the saved file: as A1, with `F27`.

## 9. End of every launch (Agent)

`ck-run.ps1` does these by itself after the steps file returns, each one even if an earlier
one fails:

1. `Close-Ck`: closes the CK through its window if still open; stops it only if it hangs 90 s
   (a finding; unsaved changes are then lost, which is the safe side).
2. `Close-Mo2`: closes MO2 (60 s, then stops it, a finding).
3. `Restore-CkSettings`: copies both settings files back from `settings-before\` and checks
   each hash.
4. `Invoke-Postflight`:
   - no `Starfield`, `sfse_loader`, `CreationKit` or `ModOrganizer` left running;
   - `Default`'s three list files unchanged (a change fails the sitting);
     `ModOrganizer.ini` changing is only recorded (MO2 rewrites it);
   - the dev profile's `plugins.txt` and `loadorder.txt`: if changed (MO2 prunes them at
     launch), restored from `lists-before\` and the hash checked against `preflight.json`;
     `modlist.txt` must be unchanged;
   - `overwrite\` and the dev mods: in launch 1 any new or changed plugin fails it; in
     launches 2 to 5 only the file holding the fixture id may be new;
   - the two settings files hash as at preflight.
5. The log ends `postflight ok`, or `FAILED` (then the launch is `inconclusive`: stop and
   look before the next launch).

Own checks the agent then makes by hand (no script does these; plain PowerShell):

```powershell
Get-Process Starfield, sfse_loader, CreationKit, ModOrganizer -ErrorAction SilentlyContinue   # prints nothing
Get-FileHash -Algorithm SHA256 'C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt','C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\loadorder.txt','C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\modlist.txt'
```

Compare with `lists` in that launch's `preflight.json`. On 2026-11-03 they were
`ece6c8e2...` (modlist), `513953f7...` (plugins), `29043dc1...` (loadorder); use the new
preflight's values, not these.

After launch 5: `overwrite\` holds nothing from the sitting once you have deleted the four
fixture files and the autosave backup.

## Not verified

- No steps file exists for an owner-driven launch; the two described in 0.2 must be written
  and desk-tested first. The frame has never run with the owner at the keyboard.
- F23, F20, F27: the matrix gives one line each and names no form, cell, owner or value.
  Every concrete choice in A2 to A4 is a proposal, not a plan decision.
- Menu paths never observed: creating a new interior cell (A3.1, A3.2), opening a cell's
  properties and its ownership field (A4.1 to A4.3), opening a vanilla reference's dialog
  (A2.2), where the CK shows a reference's FormID (A2.3), the weapon category's name (B6.1).
- What Edit > Undo covers (B8) and which camera inputs do anything (B9): that is what the
  sitting observes.
- The CK's prompt on exit with unsaved changes: never seen; its wording and buttons are
  unknown (2026-11-03 attempt 7 hung 90 s on close and was stopped).
- Whether Preferences' Close button applies changes: unknown (B3 uses the window's X).
- Where "*All" sits in the category tree: the parity row says top, the sitting 2 notes say
  last.
- Whether `KC_F2x.esp` lands in `overwrite\` for these fixtures: observed only for F29's
  `.esp`.
- Whether the copy of a saved fixture also belongs in `C:\Modding\kitkraft\observe\<yyyy-mm-dd>-<id>\`
  (the matrix's fixture folder) or only in `C:\Modding\kitkraft-observe\` (sprint ruling D9).
- `cargo xtask observe new` knows fixtures F00..F14 only; F20..F28 have no `fixture.toml`
  template, so ingestion needs one written by hand or an xtask change.
- The matrix gives F01 (`KC_F01.esp`, the empty plugin) as every fixture's base, and
  `fixture.toml`'s `base` points at that file, but F01 has never been saved.
- STATUS "Owner next actions" still asks you to remove the F29 files from `overwrite\`; the
  folder was empty when read on 2026-11-06, so that line looks stale.
- The profile folder is spelled `Kitkraft Dev`; the plan writes `KitKraft Dev`. The frame and
  `kit doctor` match it ignoring case.

## Questions for the owner

1. F23: which reference, and is Position X = `1234.5` the move you want?
2. F20: `Crate_MilitarySupply_01` as the STAT and `KC_F20_Cell` as the EditorID, or others?
3. F27: the Freestar Collective Embassy and `PlayerFaction` (matching the ownership recipe),
   or another cell and owner?
4. Part B first (as written) or after the four saves?
5. B10 (the 12 submenus) is not in decision 12: include it if time allows?
6. Should the agent also copy each saved fixture into `C:\Modding\kitkraft\observe\<yyyy-mm-dd>-<id>\`
   for ingestion, or keep everything in `C:\Modding\kitkraft-observe\`?
7. Do you delete the files in `overwrite\` after each launch yourself (as with F29), or do
   you allow the agent to remove the four `KC_F2x.esp` files and the autosave backup?

## Sources read

`plan/STATUS.md` ("Owner decisions (2026-11-05)", "Owner next actions" rows 12 and 15),
`plan/execution-plan.md` (M4 "Fixture matrix (M4.H1)", M4.A6, M4.E8, M4.O1),
`plan/adr/0012-ck-interface-clean-room.md` (Amendment 2), `AGENTS.md`,
`docs/ck-observations.md` (frame, sittings 1 and 2, ledger ck-20261101-01 to ck-20261103-01),
`docs/ck-parity.md`, `docs/recipes/ownership.md`, `xtask/src/observe.rs` (fixture ids);
sprint notes `TRACKED.md` (22:22, 22:39, 01:41, 02:15), `RULINGS.md` (D9, Q3),
`reports/COMPLETION-REPORT.md` section 3; `C:\Modding\kitkraft-observe\scripts\*.ps1`,
`ck-2026-11-03\F28-attempt7\` and `x11-f28-attempt1.out`, `-attempt7.out`.
