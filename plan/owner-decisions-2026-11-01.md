# Owner decision sheet, 2026-11-01

One sheet for every decision that is still yours, swept from `plan/STATUS.md`,
`plan/adr/*` and `plan/execution-plan.md` at `b1b9a9f`. You should not need to open
another file. Each item names the file to read only if you want more.

## How to answer

- Reply **`accept all`** to take every recommendation in groups A and B (items 1 to 23).
- Or **`accept all except 4, 19: <your answer>`**. Short answers are fine, for example
  `19: no` or `21: wait`.
- Group C is a to-do list, not a decision. Nothing to answer. If you like, reply
  `done 24, 27` when you finish one.
- Group D stays yours by design and comes later. Do not answer it now.

**Count:** 16 confirmations (A), 7 real choices (B), 11 owner actions (C), 2 later
sign-offs (D). Three items were decided in chat today (below) and need nothing.

## Already decided (2026-11-01, in chat; recorded, nothing to answer)

The owner, in chat, 2026-11-01, verbatim: **"Draft the amendment, and go ahead with 2 and 3"**.

- **The amendment**: ADR 0012 Amendment 2 (automated CK sittings through UI Automation)
  is drafted, commit `b1b9a9f`. Drafting is done; *accepting* it is item 17 below.
- **2**: an agent may run CK probe phase 2 without you: open the CK's menus through UI
  Automation (the interface Windows gives screen readers) without invoking any menu item.
  In the same go, the **F30 sitting**: an agent opens our `out\poi\KitKraftFirstPOI.esm`
  in the CK and saves it unchanged. F29 is not included (item 21).
- **3**: an agent may launch the game on the `Kitkraft Dev` profile for the first rung of
  the ladder: `GetSFSEVersion` over HTTP (M4.T1, check V0).
- **Row 10 (console mods, M4.O7) is done**, 2026-11-01. The console endpoint is on port
  45555, not 55555. Fixing the hardcoded port in `crates/game` is agent work, not yours.

---

## A. Confirmations of things already built or followed

Each of these is already in effect. "Accept" keeps it. "Overturn" means rework, named in
the item.

**1. Keep the CK-parity status `built, unobserved`?**
- What: some CK-style parts (menu bar, Warnings and Use Info windows, edit dialogs) were
  built before anyone looked at the real CK. They are marked `built, unobserved`, which
  ranks below `observed` and never counts as parity. ADR 0012 Amendment 1.
- Recommend: **accept.** It is the honest label for "we built it, the CK may differ".
- Unblocks: nothing new. Overturning sends 14 rows back to `unresearched`.
- Note: the ADR and `docs/foothold/next-sprint.md` say 13 rows; `docs/ck-parity.md` has 14
  today (and 22 `unresearched`, not 23 as the M3 exit table says).

**2. Keep `unsafe` Rust allowed in exactly one file, for the 3D view?**
- What: every OpenGL call the 3D Render Window makes is `unsafe` in the library egui uses.
  ADR 0013 allows `unsafe` only in `crates/ui-egui/src/viewport/gl.rs`; the gate fails on
  it anywhere else.
- Recommend: **accept.** There is no safe way to draw the viewport, and the scope is one file.
- Unblocks: keeps the Render Window. Overturning reverts M7.R1's GL path (no 3D view).

**3. Keep the NIF reader behind the engine, and drop the reserved "render" layer?**
- What: the window gets mesh bounds and geometry only through engine commands; it never
  reads `.nif` files itself. The unused L2 `render` slot is retired. ADR 0002 Amendment 1.
- Recommend: **accept.** Same rule as everything else: the window has no logic of its own.
- Unblocks: nothing new; confirms the layering.

**4. Keep the geometry format: base64 buffers inside the JSON result?**
- What: when `mesh.geometry` exists (M7.S5), it returns vertex and index arrays as base64
  text (binary written as text, about 1.33 times the size) inside the normal JSON result,
  so CLI and MCP see the same thing. `plan/execution-plan.md` "M6", director decision 2.
- Recommend: **accept.** Simple; a faster channel gets an ADR only if measuring shows it is too slow.
- Unblocks: nothing yet. M7.S5 also waits on item 20.

**5. Keep CK-style floating windows inside the main window (no docking library)?**
- What: M7.M0 shows CK-style windows as floating panels; docking only if a sitting shows
  the CK docks. `docs/workbench.md` "CK-style windows".
- Recommend: **accept for now.** Note: today's UI Automation probe saw a **"Docks"** menu
  in the CK, so the CK likely does dock. Expect this to be revisited once a sitting
  observes it. A judgment call.
- Unblocks: nothing new. Overturning means redoing M7.M0 with a docking crate.

**6. Keep drawing reference boxes from OBND first?**
- What: OBND is the bounding box stored in each base record of a plugin. The Render Window
  draws those boxes now; boxes computed from the actual mesh wait on item 20.
- Recommend: **accept.** It needs no new source and already works.
- Unblocks: nothing new. Overturning makes M7.R3 wait entirely on item 20.

**7. Keep the reference rotation order Rx(-x) Ry(-y) Rz(-z)?**
- What: how a placed object's rotation is applied in the 3D view. It was pinned by lining
  up boxes in three game cells; never checked against the CK.
  `crates/ui-egui/src/viewport/scene.rs` (`ref_rotation`).
- Recommend: **accept.** It matches the game data; a CK sitting can re-check it later.
- Unblocks: nothing new.

**8. Keep the CK menu order "unresearched" until a sitting observes it?**
- What: the menu bar was built in a remembered order, so it counts as no parity. Today's
  probe already saw a different order (File, Edit, View, Docks, Galaxy, World, Terrain,
  NavMesh, Character, Gameplay, Audio, Help). `plan/execution-plan.md` "M6", decision 4.
- Recommend: **accept.** If you accept item 17, an automated sitting can record the
  real order and the menu gets redone from it.
- Unblocks: M7.M1 after the sitting.

**9. Keep the clipboard crates under the Boost licence (BSL-1.0)?**
- What: egui's copy buttons pull in `clipboard-win` and `error-code`, licensed BSL-1.0.
  It is permissive and compatible with MIT/Apache, but not named in our licence rule. The
  workbench merged with them on the director's ruling (STATUS row 7, first half; ADR 0007
  "Dependency list").
- Recommend: **accept.** Dropping them means no copy buttons.
- Unblocks: closes row 7's first half.

**10. Confirm `crates/game` added no third-party dependencies?**
- What: the game-session crate (M4.T3) merged 2026-10-16 using only crates already in the
  workspace, all MIT or Apache-2.0; no HTTP or process crate (STATUS row 13, M5.O2).
- Recommend: **accept.** There is nothing to review.
- Unblocks: closes row 13.

**11. Confirm `glam` 0.34.1 for the 3D camera maths?**
- What: added by M7.R1, licence MIT OR Apache-2.0, no dependencies of its own; listed in
  `ATTRIBUTION.md`. M6.O5 asks you to confirm additions at their merge.
- Recommend: **accept.** Inside the licence rule.
- Unblocks: closes M6.O5 for now (`egui_extras` is not added yet).

**12. Confirm the project licence: MIT OR Apache-2.0?**
- What: the repo is scaffolded this way. ADR 0001 still says "license pending owner
  confirmation" (AGENTS.md owner decision 4).
- Recommend: **accept.** Standard for Rust tools; matches every dependency rule.
- Unblocks: closes ADR 0001's "Open" section.

**13. Confirm the git remote: private GitHub, only the director pushes `main`, nothing public?**
- What: the repo pushes to `github.com/mackerson/kitkraft` (private). But STATUS "Open
  owner decisions" row 6 still says "local only", STATUS "Divergence from the brief" says
  "no remote", and AGENTS.md line 204 says both "no remote yet" and names the remote.
- Recommend: **accept.** It matches what is happening; the docs then get one line fixed.
- Unblocks: removes the contradiction. "Publishing anything" stays yours.

**14. Keep third-party mods out as evidence (M2.O5: default "no")?**
- What: the mods in your MO2 folder are loaded as your load order, but no fact about the
  file format is sourced from them. `plan/execution-plan.md` M2.O5.
- Recommend: **accept "no".** The official files and the CK are cleaner sources.
- Unblocks: closes M2.O5.

**15. Close owner decision 3 (which video is the gun overhaul brief)?**
- What: you filled the brief on 2026-10-14 ("Everything WRONG With The Guns In
  Starfield", `docs/overhaul/brief.md`), and build 2 passed in game. STATUS "Open owner
  decisions" row 3 and AGENTS.md still list it as open.
- Recommend: **accept (close it).**
- Unblocks: removes a stale open item.

**16. Keep "contact kinggath about Sim Settlements 2" at "later"?**
- What: AGENTS.md owner decision 5; rung 3 of the ladder. Nothing is waiting on it.
- Recommend: **accept "later".**
- Unblocks: nothing.

---

## B. Real choices

**17. Accept ADR 0012 Amendment 2: automated CK sittings through UI Automation?**
- What: an agent launches the CK through MO2 on `Kitkraft Dev`, with you away, and reads
  what is on screen through UI Automation. Only what a user can see counts (labels, menus,
  window layout, enabled or disabled), each with a screenshot. Internal names in the tree
  (Qt object names, class names, control IDs) are used to drive controls but never cited
  or copied. Tooltip and help text is dropped. Dumps stay in `observe\`, never committed.
  One exception to "nothing writes in the game folder": the CK's two settings files
  (`QtCreationKitSavedSettings.ini`, `CreationKitPrefs.ini`) are backed up and restored
  around each run. It covers observing only; saving plugins from the CK still needs your
  per-sitting OK (item 21). `plan/adr/0012-ck-interface-clean-room.md`, Amendment 2.
- Recommend: **accept.** It removes your keyboard time from CK observation and keeps the
  clean-room line (labels only, our own words).
- Unblocks: the 14 `built, unobserved` and 22 `unresearched` parity rows; M7.O2 and M7.O3
  (items 30, 31) become agent runs; item 18 can be "no". Until accepted, today's phase-2
  probe runs, but nothing in `docs/ck-parity.md` moves on its results.

**18. Static strings scan of `CreationKit.exe`: allowed or not? (STATUS row 17)**
- What: last month an agent read printable text out of the CK binary and found
  command-line switch names. That is not disassembly, but it is not "black box" either.
  The finding is kept but not cited (`docs/ck-observations.md` "M5.A4").
- Recommend: **no, and delete the M5.A4 entry**, if you accept item 17. Amendment 2 gives
  an unattended route that stays black box, so the scan is not needed. A judgment call: a
  "no" also means the CK's own command-line switches cannot be tried from that list.
- Unblocks: closes row 17 and the M5.A4 question.

**19. Fonts: keep Windows system fonts only? (M6.O2, with row 7's second half)**
- What: the window uses Segoe UI and Consolas from Windows. The alternative is to compile
  in a font under the OFL (Open Font Licence) or an MIT/Apache icon font, listed in
  `ATTRIBUTION.md`. egui's `default_fonts` feature (OFL and Ubuntu Font Licence) stays off.
- Recommend: **keep system fonts; `default_fonts` stays off.** The visual system is
  already built and approved on them; an icon font adds a licence to track for little gain.
- Unblocks: closes M6.O2 and row 7's second half.

**20. NIF and mesh sources (M7.O5): approve the memo's three recommendations?**
- What: to draw real meshes we need the layout of Starfield's `.nif` and `.mesh` files.
  The memo (`docs/research/nif-sources-2026-10-16.md`) recommends: (1) boxes from OBND, no
  NIF source needed; (2) for geometry, only our own study of the official files, with
  community notes as pointers, and the NifSkope Starfield fork as a run-only cross-check
  once its licence is confirmed; (3) do **not** use niftools `nif.xml` (GPL-3.0), nor the
  GPL Blender bridges or nifly.
- Recommend: **approve 1 to 3.** It keeps GPL descriptions out, as with Mutagen. Cost:
  M7.S5 starts as a slow research ticket.
- Unblocks: M7.S4, then M7.R3's mesh-based boxes, M7.R4 and M7.R5.

**21. May an agent also run F29 in the CK without you?**
- What: today you allowed F30 (open our plugin, save unchanged). F29 is the next sitting:
  place `Crate_MilitarySupply_01` in `CityNewAtlantisApartmentTEMPLATE`, persistent,
  EditorID `KitKraftFirstPOICrate`, and save a new plugin with `Starfield.esm` as its only
  master (STATUS row 12). Amendment 2 decision 6 needs your OK per sitting for any CK save.
- Recommend: **yes, once F30 comes back clean** (same rules: dev profile, files into
  `observe\`, settings files restored). A judgment call: F29 is an edit, not just a save.
  The later fixture sittings F20..F28 (row 15) would still ask you one by one.
- Unblocks: M5.A3, the first CK differential (Gate K0), and the `ck-diff` cells of the gap matrix.

**22. ADR 0007 Amendment 4, the editor's write scope (M7.O4): accept, and with which save model?**
- What: lets the CK-style edit dialogs write, but only into the one "active file", and
  only through six commands (`set`, `new`, `clone`, `place`, `build`, `revert`), each
  previewed and confirmed. Model A: one confirmed write per OK click. Model B: collect edits
  until Save, one confirm for the batch. `plan/adr/0007-egui-shell-over-the-catalog.md`
  "Amendment 4".
- Recommend: **accept with A**, or **`22: wait`** if you would rather see the dialogs
  first. STATUS row 16 files it under "later". A keeps every write previewed on its own and
  the window free of unsaved state.
- Unblocks: M7.E2 to M7.E6 (editing from the window). Until then the dialogs are read-only.

**23. Ticket KK-221: may `new` write a worldspace (WRLD) with no cells?**
- What: queued ticket. It lets `new` create a bare WRLD; cells come later through `place`.
  No such world has ever been loaded in the game. The merge report called it "a product
  call". `plan/tickets/KK-221.toml`.
- Recommend: **yes, run it.** It only writes dev plugins, the ticket adds a `docs/gaps.md`
  line saying a new world is not game-tested, and an agent game session can check it later.
  A judgment call: the alternative is to hold it until a game check exists.
- Unblocks: KK-221, and WRLD's typed-write tests.

---

## C. Owner actions (not decisions; do when convenient)

**24. Finish the folder move** (STATUS row 5). Close every Claude Code session, then
double-click `C:\Modding\kitcraft\dist\KitKraft\Finish Move.cmd` and type `YES`. Only
when no agent worktree is in use.

**25. Install KitKraft for daily use** (row 11). After item 24: close sessions, run
`dist\KitKraft\Install.cmd`, type `y`; then `/mcp` should list `kitkraft`.

**26. First POI, what is left** (row 2). The visit passed 2026-10-15. Left: the
save/quit/reload row (O2-08) and installing the rename package (W12). Now that row 10 is
done, an agent game session (M5.C2) may do O2-08 instead. Note: row 2 still reads
"pending" although the visit passed.

**27. Gun overhaul** (row 3). It **passed** 2026-10-15; row 3 still reads "pending". Only
the save/reload row O5-15 was not run; an agent session can do it. Nothing else for you.

**28. In-game session 2** (row 14, M4.O4): ownership, actors, outpost, fabricator and the
scripts recipe (O4-01 to O4-07). An agent session (M4.T6) may close O4-07 first.

**29. CK fixture sittings F20..F28** (row 15, M4.O1). After F29. With item 21, these can
become agent runs one sitting at a time.

**30. CK interface sitting 1** (M7.O2). Goes to an agent if you accept item 17.

**31. CK interface sitting 2**, the edit dialogs (M7.O3). Same as item 30.

**32. Workbench walkthrough** (row 8) after each workbench change; and try the campaign's
UI tickets in a real window: launch, follow and stop (KK-032), probe table (KK-033),
confirm box (KK-034), Cell View (KK-036; should Base show the EditorID?), start-up time
(KK-040).

**33. Eyeball three schema files against the pinned xEdit file**: ENCH's `ENIT` names,
ARTO's `DNAM` bits, and ALCH's flags (the merge session could not read the xEdit file from
the host; tests prove sizes, not names).

**34. Leftovers from M2.O4** (now M3.O2): one MO2 check (which mod MO2 shows as providing a
file two mods ship) and the runtime-ID check. Note: STATUS still lists M2.O4 open, but
`plan/execution-plan.md` split it into M3.O1 and M3.O2. Small, also: delete four merged
local branches (`kk/KK-002-wip`, `kk/KK-022-wip`, `kk/KK-100-wip`, `kk/KK-109-wip`), and
the local agent's commits carry a `Co-Authored-By: Claude Code` trailer it did not earn (a
harness fix in `F:\AI\Scripts`).

---

## D. Sign-offs that stay yours (later; do not answer now)

**35. M6.O4, the cockpit sign-off.** A walkthrough of the M6 exit criteria on the release
build and `Kitkraft Dev`, including a game session run from the window. Waits on a live
M4.T3 session (decided item 3 is the first step). Optional: the same at 125 and 150
percent Windows scaling.

**36. M7.O6, the CK A/B sign-off.** The same tasks in the CK and in KitKraft (find a form,
edit a field, duplicate, place, save), then the CK opens and re-exports the result. Waits
on the editor (item 22) and the interface sittings.

---

## Conflicts found between files (for the director to fix after your answers)

- STATUS rows 2 and 3 say "pending"; the results under the table say PASS (items 26, 27).
- Owner decision 3 is open in STATUS and AGENTS.md but answered in `docs/overhaul/brief.md` (item 15).
- Owner decision 6 "local only" versus the private remote that is pushed; AGENTS.md line 204 says both (item 13).
- STATUS lists M2.O4 open; the plan split it into M3.O1 and M3.O2 (item 34).
- `built, unobserved` count: 13 in ADR 0012 Amendment 1, next-sprint and the M3 exit table; 14 in `docs/ck-parity.md`, Amendment 2 and the milestone summary (item 1).
- ADR 0013's status points to STATUS "Director assumptions pending owner", which does not list it (it is in "Pause" section 3); next-sprint says its status line reads "pending the director's confirmation", which is no longer true.
- "Pause" section 3 places the rotation in `viewport/cell.rs`; it is in `viewport/scene.rs`.
- Row 12 says CK work is "not assumed to run unattended"; today's chat lets an agent run F30.
