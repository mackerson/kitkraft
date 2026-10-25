# Status

Updated: 2026-10-16 (paused; see "Pause" below; `main` at `81272e2`, after fix round W3; planning docs reconciled against it, "Pause" section 2); owner next actions and M5 planning added at `fcb443a` (`docs/foothold/gap-matrix.md`, `plan/execution-plan.md` "M5"); rows 12 to 15 revised in the M5 review fix round; row 16 updated 2026-10-16 (owner rulings M6.O3 and M7.O1, lane V started; M6.O1 decided)

## Pivot (2026-10-23): read this first

The owner resumed the project on a cheaper footing (ADR 0014): development moves to the local
agent, one ticket per unattended run, graded by the gate, with a read-only Opus check-in. How to
run and merge a ticket: `docs/local-agent.md`. The queue: `plan/tickets/README.md`; the first
ticket is KK-001, "The gate passes on Linux" (20 tests assume Windows). The "Pause" note below
still lists the owner-gated rows and the director decisions pending confirmation. The six lanes
launched after the pause never ran; their work is the ticket queue's candidate list.

Landed 2026-10-23: the other session's branches `deploy-enable-dev-profile`, `deploy-meta-ini`
(ADR 0009 Amendments 2 and 3, MEL DLC specs) and `lane-d/scale-aware-remap` (M5.D12), merged
at `a2fd508` with the Windows gate green.

## Pause (2026-10-16, evening): read this first when resuming

The owner paused the project after the 2026-10-16 push ("bring everything down gracefully,
get things into a good position and then push and pause"). Every lane that was running has
been merged with a green `cargo xtask ci`, pushed to `origin/main`, and its worktree removed.
No agent is running. **Update, later the same evening:** sections 1 and 2 below were completed
unattended after the pause (fix round W3 applied and reviewed; follow-ups a to d applied in W3b;
planning docs reconciled at 81272e2), so the director goal ("finish every agent-executable ticket
in M5, M6 and M7") is met as far as an agent can take it: only owner-gated work (section 4) and
the next-sprint list remain. A resuming session starts at section 5.

### 1. Apply the wave-3 review fix list (one Opus fix lane, then re-review)

The Fable review of lanes P (window), T10, D4, V3 and M passed all five on code and
returned these fixes. Fix round W3 applied all seven (`f758223` to `a8469e7`, marked in
`34299f6`, merged at `81272e2`):

| # | Finding | Where | State |
| - | ------- | ----- | ----- |
| 1 | **MEDIUM** `place --fields` (and `new`) can write a group member without its group lead (e.g. `XPDO` without `XPDD` on a REFR); `validate` stays silent because `layout.undecoded` skips subrecords the walk gave no layout. Fix: `scaffold::add_slots` refuses (or adds the lead); `validate` flags out-of-place subrecords | `crates/engine/src/edit/scaffold.rs` ~72-92, `crates/engine/src/validate/mod.rs` ~981 | applied (`f758223`) |
| 2 | **MEDIUM, governance** ADR 0012 §4 says an `unresearched` parity row cannot become `reproduced` without a sitting, yet `docs/ck-parity.md` ~79-93 uses a status `reproduced (layout unobserved)` the ADR does not define, and the menu-bar order row moved to it with Source "recollection". Director decision (pending owner): amend ADR 0012 to define **`built, unobserved`** as a status below `observed` (the thing exists in KitKraft but matches no observation), move every such row to it, keep the no-skipping rule | `plan/adr/0012-ck-interface-clean-room.md`, `docs/ck-parity.md`, `crates/ui-egui/src/menu.rs` wording | applied (`f96686e`) |
| 3 | LOW Playbook step 3: on the owner's box MO2 has `Default` selected, so the first `kit mod run` ends `fail` at deploy; say "select KitKraft Dev in MO2 before the run" and name the refusal | `docs/playbooks/mod-from-brief.md` ~83, ~113 | applied (`7e1f742`) |
| 4 | LOW `virtual_table` keeps scroll offset and only-growing column widths per id across data changes; reset both on a new query or type pick | `crates/ui-egui/src/panels/common.rs` ~253-297, `app.rs` ~779-792 | applied (`45e44ad`) |
| 5 | LOW No test exercises `run_with_progress` refusing a write; add one line to `writing_commands_and_their_aliases_are_refused_before_the_engine` | `crates/ui-egui/src/gate.rs` tests | applied (`8dd90e1`) |
| 6 | LOW (lane R request) `OBND` is opaque for DOOR and LIGH, so those references draw as type shapes; type it from the pinned xEdit lines (ObjectBounds, `wbDefinitionsCommon.pas@9fb01688#L9348-L9365`) | `crates/schema/defs/{DOOR,LIGH}.toml`, then `cargo xtask codegen` | applied (`a8469e7`) |
| 7 | NOTE `scripts/launch-dev.ps1` always refuses today (`launch.evidence` is empty in `crates/game/profiles/starfield.toml`); `Select-String -SimpleMatch` on a short token could false-match; it runs whatever `kit` is on PATH | `scripts/launch-dev.ps1` ~43-51 | applied (`021c065`) |

Lane D round 5 (merged after the review) also touched `scaffold.rs`; fix 1 was applied on top of it.

### 2. Reconcile the planning docs against `main`

After the merges of 2026-10-16 (lanes H, F, P engine, A+B, V2, T, D3, G, R1, D7/H2, E, T10,
P window, D4, V3, M, R2, D5 and three fix rounds) these are stale and need one docs lane:
`docs/foothold/gap-matrix.md` (M5.F2: run `kit coverage --matrix` and refresh every row),
`docs/foothold/next-sprint.md` (what landed, what is next), the "M5 exit criteria" and
"M3 and M4 exit criteria" tables in this file, and ROADMAP.md "Where we are". Then a Fable
review of the reconciliation.

**Done 2026-10-16** (reconciliation lane, `2e3c5f5`, against `main` at `81272e2`): the
gap matrix refreshed from a release `kit coverage --matrix` and `kit schema` of that
commit, `next-sprint.md` rewritten, the M3/M4 tables reconciled and an M5 exit table
added below, `plan/execution-plan.md` ticket marks made consistent, ROADMAP.md "Where we
are" rewritten. The Fable review is item 1 of `docs/foothold/next-sprint.md` "Next, in
order".

### 3. Decisions the director took in the owner's absence (confirm or overturn)

| Decision | Where recorded |
| -------- | -------------- |
| ADR 0013: `unsafe` allowed in exactly one file (`crates/ui-egui/src/viewport/gl.rs`) for the GL viewport; the workspace lint is `deny`, and `cargo xtask layers` fails on `unsafe` anywhere else | `plan/adr/0013-scoped-unsafe-for-the-gl-viewport.md` |
| M7.M0 window model: CK-style floating `egui::Window`s inside the main viewport; docks only if a sitting shows them | `docs/workbench.md` "CK-style windows" |
| M7.R3 uses `OBND` for bounding boxes; the NIF-derived variant waits on M7.O5 | `plan/execution-plan.md` M7.R3 |
| REFR rotation convention Rx(-x)·Ry(-y)·Rz(-z), pinned from corpus box-corner alignment in three cells, unobserved against the CK | `crates/ui-egui/src/viewport/cell.rs`, `docs/ck-parity.md` Render Window row |
| CK static-strings method (M5.A4) retained but not cited or repeated until ruled | "Director assumptions pending owner" table below |
| M6.O2 fonts: system fonts only; M7.O5: no NIF source adopted | same table |

### 4. Owner-gated work (unchanged by the pause)

Rows 10 (M4.O7 console mods; note the dev profile has `LocalSettings=false`, ledger
`game-20261016-T1`), 12 (M5.O1 CK sitting; script in `docs/ck-sessions/f29-f30.md`), 14
(M4.O4 session 2), 15 (M4.O1 sittings), 17 (static-strings ruling), and M7.O2/O3 (interface
sittings) in "Owner next actions". Everything runtime-facing (M4.T4..T7, M5.C2, every
FTH-EXP row, M5.B3, M5.H2's second half) stays `not-run` until row 10 or a sitting.

### 5. How to resume

1. Open a session on the repo (`C:\Modding\kitcraft`; the rename to `kitkraft` is still
   optional, `C:\Modding\scripts\finish_kitkraft_move.ps1`, all sessions closed).
2. `kit doctor`; `cargo xtask dist --sync-bin` if `kit --version` does not name `main`'s commit.
3. Sections 1 and 2 are done. Start at `docs/foothold/next-sprint.md` "Next, in order",
   item 1 (the Fable review of the section 2 reconciliation), then items 2 to 6 there
   (item 2: lane D's M5.D1 generator and the M5.D2/D3 recipes).

## Director goal (2026-10-16)

The goal is to drive every agent-executable ticket in M5, M6 and M7 to done under the
director protocol. Opus writes in isolated worktrees, Fable reviews, and the director
merges on a green gate and pushes `main`. Where the owner would be asked, the director
takes a reasonable default decision and records it as an assumption pending the owner (the
table below). Only owner-gated work stays in the owner table: CK sittings, Nexus installs,
in-game sessions and license rulings.

### Director assumptions pending owner

| Item | Assumption taken | What it unblocks or holds | Owner may |
| ---- | ---------------- | ------------------------- | --------- |
| M6.O2 (fonts) | Keep the Windows system fonts; no font is compiled in | M6.V3's type scale and M6.V4 proceed on system fonts | choose a compiled-in OFL font or an MIT/Apache icon font instead (row 16) |
| M7.O5 (NIF and geometry sources) | No NIF source is adopted until the owner rules | M7.R3 and later (the 3D viewport chain) wait; M7.S4 waits | name the permitted sources |
| M5.O2 (`crates/game` dependency licenses) | The director confirms the dependency list at the M4.T3 merge if every license is MIT or Apache-2.0. Anything else stops the merge for the owner | M4.T3 + M5.C1 merge (path b) | review the list after the merge |
| CK binary static strings (M5.A4) | Director decision 2026-10-16: no further static analysis of `CreationKit.exe` (a strings scan sits between AGENTS.md's "run and observed as a black box only" and ADR 0005 §3's ban on disassembly, decompilation, debugging and memory inspection). The existing entry (`docs/ck-observations.md` "M5.A4") is retained but not cited | M5.A4 stays inconclusive; its switch names are not evidence for any ticket; the unattended-CK experiment waits for the ruling | rule on it (row 17): an ADR 0005 amendment or an AGENTS.md clarification, either allowing static strings (then the entry may be cited) or forbidding them (then it is removed) |

## Owner next actions

In-game checks use only the MO2 profile **KitKraft Dev**, each on a disposable save. Never
use `Default` (AGENTS.md, "Owner's live game is off limits").

| # | Action | Unblocks | State |
| - | ------ | -------- | ----- |
| 1 | In-game check of the rename plugin `OldSovietMarksmanRifle.esm` (M2.O6) | M2 exit | **done 2026-10-14: PASS** |
| 2 | In-game check of the first POI (M4.O2). Install `out\poi\package` through MO2 and check the hashes against its manifest. Then in the console: `coc CityNewAtlantisApartmentTEMPLATE`, `player.moveto KitKraftFirstPOICrate`, click the crate (the console should show `04001000`), then `prid 04001000` (Kitkraft Dev's slot, confirmed by its newest save: `kit testplan ... --from-save newest`, `docs/poi/first-poi.md`; `02001000` and `28001000` were wrong IDs, `docs/poi/first-poi-investigation.md`). Take a screenshot and run the persistence steps in `docs/poi/first-poi.md`. In the same session, install the rename package (W12, `docs/packaging.md`) | M4 exit (first POI, W2, W5, W12) | pending (owner) |
| 3 | In-game check of overhaul build 2 (M3.O5, steps and checklist in `docs/overhaul/build-2.md`, "In game"; package `out\overhaul\package\KitKraftGunOverhaul-0.2.0.zip`), on `KitKraft Dev` with a disposable save | M3 exit (overhaul) | pending (owner); build 2 landed 2026-10-15 |
| 4 | ~~Answer the brief rows B8, B11 and B12~~ | overhaul build 2 (M3.E9 rerun) | **answered 2026-10-15**: B8 dropped, B11 base 18 (drum untouched, 48), B12 as built; applied in build 2 |
| 5 | Close every Claude Code session that has the repo open. Then double-click `C:\Modding\kitcraft\dist\KitKraft\Finish Move.cmd` (it warns, asks for `YES`, and runs `C:\Modding\scripts\finish_kitkraft_move.ps1` from a copy in TEMP; docs/QUICKSTART.md section 8). The script renames `C:\Modding\kitcraft` to `C:\Modding\kitkraft`, re-points the MCP registrations, and runs `git worktree prune`, so run it only when no lane worktree is in use. Open new sessions on `C:\Modding\kitkraft` | the folder name every doc already uses | pending (owner) |
| 6 | Decide when to start the workbench (lane W, M4.W0..W6, ADR 0007 Amendment 1). The M4.W0 gate needs M4.D1 (`refs`) and M4.D2 (placement context), and neither is on `main`. The deferred CK sessions (M3.O1, M4.O1) wait for M4.W2..W5 | M3.O1 and M4.O1 (CK), with them every `ck-diff` upgrade | **decided 2026-10-15: started**. Lane W built the read-only workbench (`kit ui`, `docs/workbench.md`) |
| 7 | Rule on two licenses in the workbench's dependency tree (ADR 0007, "Dependency list"). First, **BSL-1.0** (`clipboard-win`, `error-code`): eframe always pulls these in for the copy buttons; the license is permissive but is not named in the MIT/Apache/Zlib/BSD rule. Second, whether the optional `default_fonts` feature may ever be enabled (OFL-1.1 and Ubuntu Font Licence). It is off: the window uses the Windows system fonts | the first workbench merge to `main` | open (owner decision) |
| 9 | ~~Rule on **ADR 0010** (agent-driven game sessions on `KitKraft Dev`, M4.O6)~~ | lane C (M4.T1..T10 under their ids, M5.C1, M5.C2): unattended closing of in-game rows, then brief-to-mod end to end | **decided 2026-10-15: approved in principle** (`docs/foothold/owner-decisions-2026-10-15.md` item 6, addendum item 5). Conditions and the sub-question answers are ADR 0010 Amendment 1: isolated dev profile, disposable saves, explicit authorized paths, backups and restore-only rollback, timeouts, screenshots in scope (input injection deferred), statuses not-run/pass/fail/inconclusive with missing evidence = inconclusive, manual acceptance retained |
| 10 | M4.O7 (path (b) of M5's shortest validated path): install Console API (Nexus 4280), Console Command Runner Updated (Nexus 7318) and optionally Console Output To File (Nexus 3142) into the `KitKraft Dev` profile through MO2; add `[General] bAlwaysActive=1` and the `[Papyrus]` logging keys to the profile's `StarfieldCustom.ini`; windowed in the profile's `StarfieldPrefs.ini`; create a disposable base save `KitKraftBase` in a quiet interior and note its SHA-256; note the versions in `docs/source-registry.md`. Agents do not download or install these (Nexus needs the owner's login) | M4.T1 (V0: `GetSFSEVersion` over HTTP on 1.16.244), then M4.T3 + M5.C1 and M5.C2, the first unattended save/reload | **pending (owner)**; row 9 is decided |
| 8 | Run the workbench walkthrough against `KitKraft Dev` when the workbench changes (it is `#[ignore]`d, so `cargo xtask ci` skips it). PowerShell: `$env:KITKRAFT_WORKBENCH_PROFILE = 'C:\Modding\MO2-Starfield\profiles\KitKraft Dev'`, then `cargo test --release -p kitkraft-ui-egui --test dev_profile -- --ignored --nocapture`; it reads the profile and writes nothing but the engine's index cache (`docs/workbench.md`, "Panels") | confidence that `docs/workbench.md` matches the real profile | pending (owner), after each workbench round |
| 11 | Install KitKraft for daily use (lane F, docs/QUICKSTART.md): close every Claude Code session, double-click `C:\Modding\kitcraft\dist\KitKraft\Install.cmd`, read the dry run (the same as "Install dry run" below), type `y`. Then start Claude Code: `/mcp` should list `kitkraft`, and `kit doctor` should end with no FIX on the MCP lines. Do it after row 5, or let `Finish Move.cmd` re-point it afterwards | `kit` on PATH in every terminal; the MCP server registered as `kitkraft` at the dist build | pending (owner) |
| 12 | M5.O1 (path (a) of M5's shortest validated path): an owner-assisted CK sitting from the steps in this row, which are enough on their own (M5.A1's session script, when written, adds the ledger detail and the later sittings): **F29** (in the CK, place `Crate_MilitarySupply_01` `Starfield.esm:1E4FBD` in `CityNewAtlantisApartmentTEMPLATE` at the first POI's position, persistent, EditorID `KitKraftFirstPOICrate`, save with `Starfield.esm` as the only master) and **F30** (open `out\poi\KitKraftFirstPOI.esm` in the CK and save an unchanged copy). Screenshots and files into `observe\<date>-F29\` and `observe\<date>-F30\`. A computer-use agent may drive the CK while you watch; CK work is not assumed to run unattended (addendum item 5) | M5.A3: the first CK differential (Gate K0 "CK can load/re-export"), run with `cargo xtask observe`, `kit roundtrip --verify --decode` and `kit diff`; every `ck-diff` cell of `docs/foothold/gap-matrix.md` | **pending (owner); can start now** (it no longer waits on M5.A1) |
| 13 | M5.O2: confirm the dependency list `crates/game` adds (an HTTP client and a process crate) at the M4.T3 merge (ADR 0010 Amendment 1 (c)). Blocks the merge only if a license is not MIT or Apache-2.0 | M4.T3 merge, so path (b) | **M4.T3 merged 2026-10-16** with no third-party crate (ADR 0010 "Dependency list": workspace crates only, all MIT OR Apache-2.0), so it went in under the director assumption above; pending (owner): review the list |
| 14 | M4.O4: in-game session 2 on `KitKraft Dev` with a disposable save, template O4 in `docs/game-observations.md`: the ownership and actors recipes (O4-01..O4-04), the outpost object and fabricator recipes (O4-05, O4-06; `docs/recipes/workshop.md`, `fabricator.md`), and the **scripts recipe in game** (O4-07; `docs/recipes/scripts.md`, "In game", plugin `KitKraftScripts.esm`) | M5.B3 (the scripted activator in game, Gate K0) unless lane C's M4.T6 closes O4-07 first; M5.O3; M4 exit | pending (owner) |
| 15 | M4.O1: the CK sittings for fixtures F20..F28 (`plan/execution-plan.md` M4, "Fixture matrix (M4.H1)"), owner-assisted; a computer-use agent may drive the CK while you watch. Deferred on 2026-10-14 until the workbench existed (row 6); the workbench is built, so the deferral's condition is met. Best after M5.O1 (row 12), whose F29/F30 sitting sets up the CK | M5.A5 (fixtures through the CK differential), and through it FTH-EXP-01, -04, -08 and -10; M4.A1..A6's rules raised to `ck-diff` | pending (owner), after row 12 |
| 16 | M6 "Cockpit" and M7 "CK parity" owner decisions (`plan/execution-plan.md` "M6" and "M7", lane O). **Decided 2026-10-16** (owner, in chat, verbatim: "Approve Amendment 3 and ADR 0012, start lane V mockups"): **M6.O3**, ADR 0007 Amendment 3 accepted as written. That is the cockpit write scope: `ledger`, `deploy --dev`, `deploy.rollback`, `game.session`, `game.probe` and `game.stop`, each previewed and confirmed per click, and nothing else; the window never launches the game itself, and `testplan` stays out. It also confirms the director's decision on rollback in and `testplan` out. **M7.O1**, ADR 0012 (`plan/adr/0012-ck-interface-clean-room.md`, the CK interface clean-room rule) accepted. Lane V started: M6.V1 and M6.V2 landed. **M6.O1 decided 2026-10-16: Workbench** (owner, in chat, verbatim: "Workbench, themes look great only keep the mocks if there's a good reason."); `workbench` is the default, `studio` selectable, `classic` legacy, the mockups removed. **Still open**: **M6.O2**, fonts, either Windows system fonts (the default) or a compiled-in OFL font or MIT/Apache icon font (row 7's BSL-1.0 ruling is still open beside it); **M7.O5**, the sources for Starfield NIF and geometry layout. Later: M6.O4 (the cockpit sign-off at the end of M6; a check at 125 and 150 percent is optional, lane X is deferred), M7.O2 and M7.O3 (CK interface sittings, combinable with rows 12 and 15), M7.O4 (Amendment 4, the editor write scope) and M7.O6 (A/B sign-off). **Director decisions still pending your confirmation** (`plan/execution-plan.md` "M6", "Decisions taken by the director 2026-10-16, pending owner"; ADR 0002 Amendment 1): `crates/nif` stays behind the engine and the L2 `render` slot is retired; M7.S5 geometry as base64 buffers in the JSON envelope; CK menu order `unresearched` until sitting 1 | Open: any M6.V4 icon font and a compiled-in font for the type scale (O2), M7.S4 and so M7.R3 to M7.R5 (M7.O5), M6 exit (O4). Unblocked by the 2026-10-16 rulings: M6.G3 to M6.G6 (each still on its other dependencies), M7.K2 onward, M6.V3 onward (O1) | **O3, M7.O1 and O1 decided 2026-10-16**; open: O2 and M7.O5, both can be decided now |
| 17 | Rule whether a static strings scan of `CreationKit.exe` (printable text read from the binary's bytes, no disassembly, debugging or memory) is allowed: AGENTS.md "Clean room" allows the CK only "run and observed as a black box", ADR 0005 §3 forbids disassembly, decompilation, debugging and memory inspection, and the scan is neither. Answer as an ADR 0005 amendment or an AGENTS.md clarification. Until then: no further static analysis, and `docs/ck-observations.md` "M5.A4" is retained but not cited ("Director assumptions pending owner") | M5.A4 (unattended CK route) | pending (owner) |

**Director ruling pending owner confirmation (2026-10-15):** eframe's clipboard support brings `clipboard-win` and `error-code` under BSL-1.0 (Boost). BSL-1.0 is permissive with no copyleft and is compatible with MIT/Apache-2.0; applying owner clarification 2 (no pause unless the license category differs), the workbench was merged with this noted. Owner: confirm, or ask for the clipboard feature to be dropped.

**M3.O5 result (2026-10-15):** PASS. Owner ran every `player.additem` row of docs/overhaul/build-2.md on the vanilla `KitKraft Dev` profile with build 2 loaded: C1-C7 and C9-C14 pass (C8 dropped). Rifle shows "Old Earth Marksman Rifle". Gun overhaul closed; M3 exit criteria met except the deferred CK session.

**M4.O2 result (2026-10-15):** PASS. On the vanilla `KitKraft Dev` profile the owner reached the crate with `coc CityNewAtlantisApartmentTEMPLATE` + `player.moveto 04001000` (runtime slot from the save, not MO2's index). Save/quit/reload persistence row: pending owner confirmation.

## Lane F: human accessibility (2026-10-15)

Start at **docs/QUICKSTART.md**. No environment variable or `--data-dir` is needed any
more in the owner's steps:

- `kit doctor` (catalog command, read-only) finds the game (Steam registry and
  libraries), the Data dir, the game version, the MO2 instance and its profiles (the
  KitKraft Dev folder is `Kitkraft Dev`; matched ignoring case), xEdit/xDump64, the
  Papyrus compiler, ContentResources.zip, SFSE, the cache and the MCP registrations, one
  line each with the path, how it was found, and `OK` or a fix. Its detection is every
  command's default: argument > environment variable > detected; a bare `--profile
  "KitKraft Dev"` is that profile of the detected instance.
- `cargo xtask dist` writes `C:\Modding\kitcraft\dist\KitKraft\` (built 2026-10-15):
  `kit.exe`, `kit-ui.exe`, Workbench, Shell, Validate Plugin, Load Dev Profile, Deploy
  to Dev, Finish Move, Install, Uninstall `.cmd` files and `README.txt`. Checked here:
  `kit doctor` (output in QUICKSTART section 3), `Validate Plugin.cmd` on
  `out\overhaul\KitKraftGunOverhaul.esm` (passed, 0 errors, 0 warnings), Load Dev
  Profile, Shell, and Deploy to Dev against a synthetic MO2 instance (dry run, cancel,
  and a real deploy there). Not run: `Install.cmd`, `kit install --apply`, the workbench
  window, Finish Move, and any deploy against `C:\Modding\MO2-Starfield`.
- `kit install` / `kit uninstall` (CLI only, dry run unless `--apply`) do the PATH and
  MCP edits; tested against a temp home and a JSON stand-in for the registry.

### Install dry run

`C:\Modding\kitcraft\dist\KitKraft\kit.exe install --dry-run` on the dev box, 2026-10-15
(read-only; this is what `Install.cmd` will show and do):

```text
kit install would change:
- User PATH: HKCU\Environment Path (the user PATH)
    add C:\Modding\kitcraft\dist\KitKraft
    old value (REG_EXPAND_SZ): C:\Users\macke\.cargo\bin;C:\Users\macke\AppData\Local\Programs\Python\Python310\Scripts\;C:\Users\macke\AppData\Local\Programs\Python\Python310\;C:\Users\macke\AppData\Local\Microsoft\WindowsApps;C:\Users\macke\AppData\Local\JetBrains\Toolbox\scripts;C:\Users\macke\AppData\Local\GitHubDesktop\bin;C:\Program Files\LOVE;C:\Users\macke\.lmstudio\bin;%USERPROFILE%\AppData\Local\Microsoft\WindowsApps;C:\Users\macke\AppData\Roaming\npm;%USERPROFILE%\.dotnet\tools
    old value saved to C:\Modding\kitcraft\dist\KitKraft\backups\user-path-1792105194.txt
- Claude Code user config: C:\Users\macke\.claude.json
    remove the pre-rename entry `kitcraft` (command C:/Modding/kitcraft/bin/kit.exe)
    env KITCRAFT_DATA_DIR dropped: it names the Data dir `kit doctor` detects anyway
    set mcpServers.kitkraft: command C:/Modding/kitcraft/dist/KitKraft/kit.exe, args ["mcp","--profile","C:/Modding/MO2-Starfield/profiles/Default"]
    backup copy first: C:\Users\macke\.claude.json.kitkraft-backup-1792105194
- Project MCP file: C:\Modding\kitcraft\.mcp.json
    remove the pre-rename entry `kitcraft` (command C:/Modding/kitcraft/bin/kit.exe)
    env KITCRAFT_DATA_DIR dropped: it names the Data dir `kit doctor` detects anyway
    set mcpServers.kitkraft: command C:/Modding/kitcraft/dist/KitKraft/kit.exe, args ["mcp","--profile","C:/Modding/MO2-Starfield/profiles/Default"]
    backup copy first: C:\Modding\kitcraft\.mcp.json.kitkraft-backup-1792105194
- Install record: C:\Modding\kitcraft\dist\KitKraft\install-state.json
    what was there before, for kit uninstall
Dry run: nothing was changed. Run `kit install --apply` to make these changes.
```

Notes: the backup names carry the time of the run. Rewriting `.claude.json` keeps every
key but sorts them (serde_json without key-order preservation); the backup copy holds the
original. The MCP server keeps preloading `Default` read-only, as the `kitcraft` entry
did. `C:/Modding/kitcraft/bin/kit.exe` stays on disk; nothing points at it afterwards.

- Landed (lane P window side, M6.P1 partly, M6.P2a, M6.P3a, M6.P4, M6.P5a, M6.P7, 2026-10-16): `KITKRAFT_WORKBENCH_PERF=1` trace and a scripted bench; before/after on release builds, three runs cold and warm (`docs/design/workbench-performance.md` "Window measurements"): 1000-row scroll p95 19.7 -> 0.9 ms, Records arrival frame 44 -> 1.6 ms, Diff arrival frame (13.5 MB `conflicts`) 351 -> 4.6 ms, warm load drawn 0.96 -> 0.75 s; view-models built and envelopes freed on the one engine thread; virtualized tables; progress bar on the status line; no second load of the loaded order; window restore, focus and place in `ui.json`; the Diff panel asks `conflicts` for `detail: fields`.
- Landed (lane G, M6.G1, M6.G2, M6.G3, M6.G7, M6.G8, M7.E1 draft, 2026-10-16): the gate holds `COCKPIT_SCOPE` (Amendment 3's six ids) and `PERMITTED_WRITES` = `ledger`, `deploy`, `deploy.rollback` (preview unconfirmed, writing form only from its panel's confirm click, `dev: true`, never `Default`; per-entry source scan; scope test); `deploy.status` and `doctor` allowed; a Cockpit tab (plugin from `out\` or a path; review = `validate`, `roundtrip --verify`, against its masters; the six Reporting rows; dev deploy and roll back with dry-run preview and a confirm box closed unless the loaded dev profile matches; ledger entries naming the plugin; game buttons disabled until M6.G4/G5 permit them) and a Coverage tab (`coverage --matrix`, status per cell, filter, evidence); the confirm path tested on a synthetic MO2 instance only; ADR 0007 Amendment 4 drafted (proposed, M7.O4).
- Landed (lanes A and B, 2026-10-16): M5.A1 session script `docs/ck-sessions/f29-f30.md` for the owner's M5.O1 sitting (F29 `KitKraftFirstPOI-CK.esm`, F30 `KitKraftFirstPOI-CKRoundtrip.esm`, outputs in `observe\ck-<date>\`, every CK UI claim unobserved); M5.A2 `cargo xtask oracle ck <fixture-dir>... [--check]` (explained / ours-missing / unexplained, reports in `observe\ck-oracle\`, synthetic fixtures only, no real CK output yet); M5.A4 static strings of `CreationKit.exe` (method pending owner ruling, row 17: retained, not cited), so CK fixtures stay owner-assisted; M5.B1 folder-vs-zip `papyrus.events` index test (same index); M5.B2 50 base-game scripts 50/50 equivalent to the hand-run compiler (`docs/research/pex-differential.md`); M7.S4 prep memo `docs/research/nif-sources-2026-10-16.md` for M7.O5.
- Landed (docs lane, 2026-10-16): the owner's rulings of 2026-10-16 ("Approve Amendment 3 and ADR 0012, start lane V mockups") are recorded. ADR 0007 Amendment 3 is accepted (M6.O3), with an "Owner ruling 2026-10-16" block whose consequences were checked against M6.G1 to M6.G6. M6.G3's first "done when" is now the gate test extension for `deploy` and `deploy.rollback`. ADR 0012 was written as `plan/adr/0012-ck-interface-clean-room.md` with status Accepted (M7.O1; this closes M7.K1). Lane V started, with M6.V1 and M6.V2 in progress. Row 16 above, `plan/execution-plan.md`, `docs/foothold/next-sprint.md`, `ROADMAP.md`, `docs/workbench.md` and `docs/ck-parity.md` follow, and AGENTS.md gets its pointers in a separate commit. No code changed.
- Landed (lane E, FTH-EXP-01 and FTH-EXP-03 agent halves, 2026-10-16): `prototypes/` created (`README.md`, `foothold/README.md` with the C2 lifecycle state model and the export rule); `exp01-authored-site/` (a `LocTypeOutpost` `LCTN` with the vanilla template's values, the beacon `ACTI` cloned, beacon, centre marker and map marker placed in `SettleCrucibleWorld`; arm B places vanilla's `OutpostBeacon_Packin`) and `exp03-claim-terminal/` (`TERM`/`TMLM`, `KitKraft:SiteClaim` on a switch, idempotent `KitKraftSiteClaimed`, owner `FACT`); built to `out\prototypes\`, validate 0/0, roundtrip identical, refs clean, testplan probes plus hand-written claim and workshop probes; game rows not-run. Request (lane D): `[[place]]` `fields` with `new:<place>` values (linked refs, map marker data, location ref lists) and `set`/`clone` adding an absent subrecord (cell `XLCN`, `LCTN` `PNAM`); typed `KWDA` in `new LCTN` (written as hex today).
- Landed (lane D round 3, 2026-10-16): M5.D6 `refs`/`validate` follow VMAD script property objects (43,186 newly visited FormIDs in the official files; `inspect --full` decodes a VMAD), M6.P5b `conflicts --detail` (SFBGS00D page 28.7 MB -> 51.7 KB), M6.P3b measured (warm 1.0 s CLI, 1 ms in session), M7.S6 `dry_run` on set/new/clone, M7.S7 `revert`, M7.S3 `cell`; `crates/ui-egui/src/gate.rs` got two list entries (`revert` writing, `cell` unused read-only); the Diff panel should pass `detail: "fields"` to `conflicts` (lane P/G).
- Landed (lane D round 5, M5.D8 to M5.D11, 2026-10-16): `set`/`[[set]]`/`clone` add a missing typed subrecord at its schema slot (CELL `XLCN`, ACTI `KWDA`+`KSIZ`, LCTN `PNAM`); arrays take typed JSON values (FormIDs, numbers, strings, struct entries) with `count_from` members and `KSIZ` kept equal to the count; `new:` names a `[[place]]` (FormIDs allocated first) and `[[place]]` takes `fields`; `place --dry-run`. Mini-site rebuilt with its `LCPR`, cell `XLCN` and switch `KWDA` (`05aac031...`), FTH-EXP-01 arm A with beacon/map-marker links, marker data, location `MNAM`/`LCPR` and cell `XLCN` (`8aa846f1...`); validate 0/0, roundtrip identical, refs clean (`crates/engine/tests/lane_d5_corpus.rs`).
- Landed (lane D round 4, M5.D4/D5 follow-ups, 2026-10-16): typed-writable `pass` for COBJ, STAT, REFR, ACHR and CELL (`authoring_tests.rs`, new `authoring_world_tests.rs`, rows in `docs/schema/evidence.toml`): `place` takes `fields` (subrecords added at their schema slots, then written as `set` writes) and `new`/`[[new]]` write a new interior CELL (block and sub-block from its FormID, INCC counted; exterior refused `new.cell-exterior`); typed-readable `pass` for WEAP and CONT from four `BfcbBlock` layouts (`DisplayCase`, `QualityUpgrade`, `SoundTag`; `ParticleSystem` opaque, its helper is in an unapproved unit); `docs/foothold/gap-matrix.md` rows refreshed (MX4), `docs/record-parity.md` regenerated. M6.P5b needs no lane D follow-up.
- Landed (lane V, M6.V1 and M6.V2 as the director scoped them, 2026-10-16): visual tokens in `crates/ui-egui/src/theme/` (type, 4 pt spacing scale, radii, table density, light and dark palettes, status glyph + word + color, AA contrast test), presets `classic` (default, today's look), `workbench` and `studio` via `kit ui --theme <preset>[-light|-dark]` or `ui.json` `theme`; 12 mockup PNGs and the comparison page `docs/design/mockups/2026-10-16/README.md` for the owner's **M6.O1**; the `app.rs` split of the plan's V1 row is still open (execution-plan "Lane V notes").
- Landed (lane P engine side, M6.P2b, M6.P6a, M6.P6b, 2026-10-16): `Engine::execute_with_progress` (typed `Progress` events: `index` per plugin from every load, `reverse-index`, `records`, `roundtrip`, `scan`, `validate`, `classes`/`rules`/`write`; `kit --progress` prints them to stderr; MCP ignores them; envelopes identical except timings, tested); `kit --version` and a new envelope member `build` name version, commit (`-dirty`), date and profile, stamped by `cargo xtask dist`/`ui` through compile-time env vars (no `build.rs`; `unknown` otherwise); `cargo xtask ui` builds release and opens `kit ui --dev`; a debug `kit ui` starts a release window or says where to get one (`--allow-debug` to run the debug one). Not done: MCP progress notifications, the P6a opt-level measurement, the About line (lane V).
- Landed (lane V round 2, M6.V3 and the rest of M6.V1, 2026-10-16): M6.O1 recorded (Workbench); `workbench` is the default preset (system light or dark; `--theme`, `ui.json` and a Start-tab picker override; `classic` marked legacy; mockups removed); every tab's drawing moved to `panels/<tab>.rs` (`app.rs` 2,791 -> 851 lines) and takes all colors, sizes and gaps from `theme` (scan test); new tokens `control_border`, `focus` (3:1 tested), `status.unknown`, `Sizes`; the session text moved to a bottom status line; Records shows the inspector beside the table (split remembered in `ui.json`); tour re-run at 1400 x 900 on Kitkraft Dev.
- Landed (lane V round 3, M6.V4, M6.V5 partial, M6.V6, 2026-10-16): status never color-only (status colors private to `theme/`; `status_header`, `status_painted`; confidence levels and FormID resolution as statuses; scan and draw tests); one panel frame on all 14 tabs incl. Cockpit, Coverage and Render Window (`panel_head`, `action_bar`, `envelope_block` with a limitations strip; test); coverage matrix, cockpit master and ledger tables and diagnostics references on `virtual_table`, numbers right-aligned; the Start tab's About line from the envelopes' `build` and a "debug build" title bar (M6.P6a window side); `docs/design/workbench-style.md` with a token doc-sync test; tour of every tab in light and dark on Kitkraft Dev. Open: column resize by drag.
- Landed (fix round W1 after the Fable review of wave 1, 2026-10-16): MCP instructions and `execute`'s description name all six envelope members (`build` added), tested against the envelope shape (`shapes::envelope_members`); the progress-sink claim reads "identical except timings" (engine docs, `docs/mcp.md`, `kit --progress` help); M5.A4's strings scan of `CreationKit.exe` marked method pending owner ruling, retained but not cited (assumptions table, owner row 17); F29/F30 script UI claims marked unobserved one by one; Free Lanes Q-ALL command given `--limit 1000` (every count rechecked, none changed); EL-02's GBFM count cited; hostile-encounters note: repeated `--plugins`, `SFBGS00D.esm:05F4DC`, `--function CheckForAttack` (each rerun), plan rows point at the dated file.
- Landed (fix lane X, 2026-10-16): `cargo xtask dist` no longer refuses while the MCP server or workbench runs `dist\KitKraft\kit.exe`/`kit-ui.exe`: a program in use is renamed aside to `<name>.stale-<unix-seconds>`, the new build copied in, and every `*.stale-*` the folder still has is deleted when unlocked (one line per swap plus the count left; no process is stopped); opt-in `--sync-bin` refreshes the main checkout's gitignored `bin\` the same way; README.txt and docs/QUICKSTART.md say to restart Claude Code or the workbench after rebuilding.
- Landed (fix lane P, 2026-10-16): `kit install`'s MCP `--profile` defaults to the KitKraft Dev profile `kit doctor` detects (else MO2's active one, printed as a `limitation:`; `kit install --profile <name-or-path>` overrides) and re-points a registered `Default` with a `- args`/`+ args` diff of the entry, so re-running `Install.cmd` on the dev box moves both registrations to `Kitkraft Dev`; `kit doctor`'s MCP lines name each registration's profile and FIX `Default`; `.mcp.json.example` and docs follow (the live `.mcp.json` is gitignored: re-run `Install.cmd` to update it).
- Landed (lane F/W fix round after the Fable review, 2026-10-15): `kit install`/`uninstall --apply` refuse while `claude.exe` runs (desktop app and native CLI; `--force-while-running`, `--check-running` for `Install.cmd`); `--env-file` refuses anything but `{}`/`{"Path":...}`; the gate's `Request.confirmed` is private; `profiles`, `kit doctor`, profile names and `kit install` share one MO2 order (`KITKRAFT_MO2_DIR`, then `kitkraft_io::host`), and `kit install` honors `KITKRAFT_DETECT=0`; QUICKSTART notes the `.claude.json` normalization; `dist\KitKraft` rebuilt.
- Landed (lane M, M7.M0, M7.M1, M7.M4, M7.W3, M7.W5, 2026-10-16): the CK-style shell in `crates/ui-egui`: a menu bar from the catalog (CK menus in the recorded, unresearched order, KitKraft's menus before Help, disabled items with their reason), floating windows toggled from View and kept in `ui.json` (Warnings over `load` and `validate`, Use Info over `refs --incoming`, every tab as a window; no dock crate, director assumption pending the owner), and a read-only edit dialog from `crates/ui-egui/layouts/<TYPE>.toml` (WEAP, ACTI, LCTN, TERM, FACT, PKIN, CELL, REFR; names ours, unobserved) with a default layout from `schema`; `toml` added to ui-egui (already in the tree); parity rows `reproduced (layout unobserved)` (renamed `built, unobserved` by ADR 0012 Amendment 1 in fix round W3, `f96686e`); tour images `ck-*.png` on Kitkraft Dev.
- Landed (fix round W3, the wave-3 review fix list of "Pause" section 1, rows 1 to 7, 2026-10-16): `place`/`set`/`clone`/`new` refuse a group member whose group lead the record lacks and the fields do not give (with the lead given it is added first), backed by a placement check, and `validate` has a new error rule `layout.unplaced` (0 findings on the 14 official files, release `validate_corpus_official_files`); ADR 0012 Amendment 1 (director decision, pending owner) defines `built, unobserved` and the 13 `reproduced (layout unobserved)` parity rows (the menu-bar order row included) moved to it, `menu.rs`, the edit dialog and `docs/workbench.md` worded to match; the playbook says to select KitKraft Dev in MO2 before the first `kit mod run` and quotes the `refused [live-profile]` text; `virtual_table_for` resets scroll and column widths when a new query or type list arrives; the gate test also refuses through `run_with_progress`; DOOR and LIGH type `OBND` as `ObjectBounds` (codegen, `docs/record-parity.md` regenerated, release `corpus_full_roundtrip` byte-identical, their references now get boxes); `scripts/launch-dev.ps1` matches the launch evidence as an anchored ledger entry heading and runs this checkout's `bin\kit.exe` or `dist\KitKraft\kit.exe`. Re-review pending.
- Landed (fix round W3b, the Fable review follow-ups (a) to (d) of fix round W3, 2026-10-16): the Links, Diagnostics, Load, Coverage, Diff and Cockpit masters tables pass a data key to `virtual_table_for`, bumped when the result arrives, so a new request starts them at the top with fresh widths; `set`/`clone`/`place`/`new` add the members a started group requires (zero-filled, named in `limitations`; refused naming one with no fixed size), and `validate` has a new error rule `layout.group-incomplete` (`kitkraft_doc::typed::group_gaps`; 0 findings on the 14 official files, release `validate_corpus_official_files`; `docs/validate.md`); the playbook names `--dev-profile` (not `--profile`) for `kit mod run`; the lane M note says `built, unobserved`.
- Landed (lane R, M7.R1, 2026-10-16): the workbench's "Render Window" tab, a GL viewport through eframe's glow backend (`egui_glow::CallbackFn`; glow 0.17.0 as eframe pins it) with a ground grid, world axes, a horizon tint, one placeholder box (`viewport::scene::Marker`, the path M7.R2 reuses), an orbit/pan/zoom/WASD camera (bindings unobserved against the CK, `docs/ck-parity.md`), repaint only on input or request, and a stats overlay; `glam` 0.34.1 (MIT OR Apache-2.0) added; the L2 `render` slot removed from xtask, AGENTS.md and `docs/architecture.md`. **Director**: ADR 0013 (proposed) relaxes `unsafe_code` from forbid to deny with one allowed file, `viewport/gl.rs`, enforced by `cargo xtask layers`, because every `glow` call is `unsafe`.
- Landed (lane R round 2, M7.R2 and M7.R3 partial, 2026-10-16): the Render Window loads a cell (`cell --with-bounds`, new engine argument: each base's `OBND` with evidence) and draws its references as OBND boxes or type shapes, colored by base type with a legend, instanced (V2: 20,000 markers at 0.23 ms GPU; draw cap 20,000), click to select shared with the Records inspector, camera framed on the cell; metric grid (Starfield positions are meters); REFR rotation pinned as `Rx(-x)Ry(-y)Rz(-z)` from corpus corner alignment. Requests: lane BX to type `OBND` for DOOR and LIGH (opaque today, so their references draw as shapes); M7.W2 to drive the picker.
- Landed (lane C, runtime, 2026-10-16): **M4.T2** `kit testplan` also writes `probes.toml` (one probe per checklist step, rows `T-<nn>`; every O2/O4 template row mapped in `docs/game-sessions.md`); **M4.T3 + M5.C1** new L3 crate `crates/game` (no third-party crate: std TCP console, std process; ADR 0010 "Dependency list", for **M5.O2**) and catalog commands `game.session` (`--start`, `--dry-run`), `game.status`, `game.probe`, `game.stop`, with preflight refusals, backups and restore-only rollback, timeouts, `not-run`/`pass`/`fail`/`inconclusive`, and the ledger entry, all tested against a fake `/console` endpoint on a fixture MO2 instance; **M4.T9** stub profiles `skyrimse`/`fallout4` (refused, naming what is missing); **M4.T1 (a)** not-run (the owner's MO2 was running; `game-20261016-T1`, candidate grade D). No game was launched: a live session is refused until M4.T1 records the launch/console/save facts and M4.O7 is done; the dev profile has `LocalSettings=false` (finding in `game-20261016-T1`). `crates/ui-egui/src/gate.rs` only classifies the four new ids (no new permission).
- Landed (lane T round 2, 2026-10-16): **M4.T10 partial**: `kit mod run <spec>` (catalog `mod.run`, writing) chains build, validate, roundtrip --verify, package, deploy --dev, testplan and game.session (preflight by `--dry-run` first), stops at the first failing stage, and writes `out\<name>\report.md`, `report.json` and `stages\<nn>-<stage>.json` (a verdict per probe, the ledger entry id; `--until`, `--dry-run`); `docs/playbooks/mod-from-brief.md` (brief template, spec kinds, reading the report, fix loop), AGENTS.md "Start every session" step 4, `scripts\launch-dev.ps1` (prints the M4.T1 candidate MO2 command and refuses until the ledger has the launch fact), `scripts\tail-console.ps1`, `scripts\open-observe.ps1`. Transcript check `crates/engine/tests/mod_from_brief.rs`: the rename brief and the first POI spec pass stages 1 to 6 against a fixture MO2 and stop at the session, refused for `unverified-profile` (every probe not-run, `partial`, exit 0); the "done when" waits on M4.T1 and M4.O7, and on the owner enabling `KitKraft Dev - <plugin>` once in MO2 (deploy never edits a profile). **Request (director, at merge):** add `"mod.run"` to `WRITING` in `crates/ui-egui/src/gate.rs`; this lane may not touch ui-egui, and its classification test fails until then.
- Landed (owner session, 2026-10-18): **ADR 0009 Amendment 3** (owner ruling): `deploy` and `deploy.rollback` write the dev mod's `meta.ini` (`version`, `comments=KitKraft dev build <id>`, `category` from MO2's `categories.dat`; new file, or only those keys replaced in MO2's); `deploy` takes `category` and `version`, `mod.run` passes `version` to package and `category` to deploy; the deployment manifest records the version. Code `crates/engine/src/deploy/meta.rs`; tests in `meta.rs` and `deploy/tests.rs`. The owner checked MO2 shows the version and category: pass, ledger `game-20261018-01`.
- Landed (owner session, 2026-10-18): **ADR 0009 Amendment 2** (owner ruling): catalog commands `deploy.enable` and `deploy.disable` (writing, `destructiveHint`; refused in the window), the one profile write path: `KitKraft Dev` only (Default `live-profile`, others `not-dev-profile`), refused while MO2, the game, SFSE, the CK or MO2's helpers run, only the named mods' lines in `modlist.txt`/`plugins.txt`/`loadorder.txt`, every other byte kept, backup with SHA-256 in `%LOCALAPPDATA%\kitkraft\profile-backups\`, restore on failure; third-party mods by exact folder name. Code `crates/engine/src/deploy/enable.rs`, tests `enable_tests.rs`; AGENTS.md, `docs/deploy.md`, the playbook, README and QUICKSTART name the exception. The owner checked that MO2 2.5.2 keeps the edit when it opens the profile: pass, ledger `game-20261018-01` (MEL and both MEL-DLC mods enabled, plugins last).
- Landed (reconciliation lane, "Pause" section 2, 2026-10-16, docs only): the planning documents checked against `main` at `81272e2`. `docs/foothold/gap-matrix.md` (M5.F2) refreshed from a release `kit coverage --matrix` (185 types: parsed pass 185; typed-readable pass 54, fail 131; typed-writable pass 13; CK-diff not-run 185; game pass 5; save/reload not-run 185) and `kit schema` (96 definitions, 4,038 subrecord definitions, `ck-diff` 0), with new capability rows for subrecord adds, previews and revert, `cell`, the session controller, `mod.run`, the cockpit, the viewport and the CK shell, and Gate K0's mini-plugin item met on data; `docs/foothold/next-sprint.md` rewritten (landed by ticket, next in order, owner-gated rows, director decisions pending); the M3/M4 tables above reconciled and an M5 exit table added; `plan/execution-plan.md` ticket marks made consistent; ROADMAP.md "Where we are". No code changed.

## Current milestone: M5 partial (owner-gated), M6 and M7 in progress; M3 and M4 closing

Paused 2026-10-16, evening; `main` at `81272e2` (after fix round W3). M0, M1 and M2 are
done. M3 and M4 have no agent-executable criterion left: what stays open is the owner's
CK sittings (rows 12 and 15), the in-game session 2 (row 14), the save/reload rows and the
closing gates. M5 is partial and owner-gated: its agent tickets are on `main` except M5.D1's
generator and M5.D2/D3's recipes (`docs/foothold/next-sprint.md`, item 2), and each of
its two paths waits on one owner action (path (a) on row 12, path (b) on row 10). M6
(cockpit) has its visual system, performance work and the cockpit's deploy and rollback;
its game loop (M6.G4 to M6.G6) and sign-off wait on a live M4.T3 session. M7 (CK parity)
has a read-only shell and Render Window built before any interface sitting, so every
parity row they touch is `built, unobserved` or `unresearched` (ADR 0012 Amendment 1).
The tables below check every "done when" bullet of `plan/execution-plan.md` against
`main` at `81272e2`; status per capability is `docs/foothold/gap-matrix.md` (checked at
the same commit).

## M3 and M4 exit criteria (reconciled 2026-10-16 at `81272e2`)

States: **done** (on `main`), **pending-owner** (needs an owner action above),
**carried** (open, with the task that carries it and the reason). Not rerun for this
reconciliation except `kit coverage --matrix` and `kit schema` (gap matrix, keys MX and
SC). Each "done" cites the evidence that lane recorded.

### M3 is done when

| Criterion (`plan/execution-plan.md`) | State | Tasks and evidence |
| ------------------------------------ | ----- | ------------------ |
| Round 3 types (WEAP AMMO PROJ AMDL OMOD COBJ ZOOM LVLI FLST INNR) typed, sourced, with a confidence; 0 conflicts on all 14 files; 105 lstring signatures typed | **done** | M3.B1..B5, B7, B8; merged in M4.BX1. Parity: failed 0, 0 typed conflicts (regenerated at `a8469e7`). Round 4 typed the OMOD property values (sibling-field unions) |
| KYWD and GMST remainders typed | **done** | M3.B6, lane B round 5 (KYWD from the pinned xEdit file L13297-L13312; GMST XALG L12357, DATA decider Common L5887-L5911). Parity: KYWD 22,948 and GMST 4,858 decoded occurrences, 0 failed, 0 re-encode differences |
| Staged xEdit files deleted and recorded | **carried** | To the end of M4: the family lanes, lane D round 4 and fix round W3 (DOOR/LIGH `OBND`, `a8469e7`) still transcribe from `C:\Modding\tools\xedit-ref\` |
| `docs/ck-observations.md` entries for every M3.O1 fixture; M3.H3 targets answered; TES4 `SNAM`/`BNAM`/`CHGL` settled; CK fixtures roundtrip (M3.A4) | **pending-owner** | No CK sitting yet (gap matrix CK0). M3.O1's fixtures come through rows 12 (M5.O1, F29/F30, script `docs/ck-sessions/f29-f30.md`) and 15 (M4.O1, F20..F28); the deferral's condition (the workbench) is met. `cargo xtask observe` and `cargo xtask oracle ck` (M5.A2, synthetic fixtures) are ready |
| The 160 recompression misses settled or restated with evidence | **done** (restated) | M3.A1: classified E/N/U with the study in `docs/gaps.md`. F07 still checks it in the CK (with the sittings above) |
| `style: "ck"` in `set`/`build`, matching the CK's bytes or explained (M3.E5) | **carried** | M3.A3, M3.E5, after the CK sittings (rows 12, 15) |
| `docs/ck-parity.md` ranks the CK features (M3.H5) | **carried** | M3.H5, after the sittings. `docs/ck-parity.md` now also holds the M7 interface rows (13 `built, unobserved`, 23 `unresearched`) |
| Spec v2 and `kit overhaul` `Implemented` | **done** | M3.E6, M3.E7 |
| First overhaul build from the owner's brief validated (M3.E9) | **done** (build 2) | Build 2 (2026-10-15, `docs/overhaul/build-2.md`): 13 rows implemented, B8 dropped; validate 0/0, roundtrip identical, packaged 0.2.0 |
| Owner's in-game overhaul result recorded (M3.O5) | **done** | PASS 2026-10-15 (C1-C7, C9-C14), ledger `game-20261015-02` (M5.F4) with the plugin's hash. Its save/reload row O5-15 was not run |
| `docs/oracles.md`: the Spriggit run, and the SF1Edit run with M3.B11's tables | **done** | M3.H4 (both harnesses). M3.B11 rerun 2026-10-15 on every record of the 12 typed types: 149,454 agree, 2,825 agree with fewer xEdit fields, 190 disagree (X9, version skew), 0 ours. Also the Papyrus compiler oracle and M5.B2 (50 of 50). A build at the pin is the owner's (`docs/gaps.md`) |
| Every hardening task done or carried; `docs/gaps.md` says what M3 closed | **carried** | Done: M3.A1, A2, A5, B10, D1, D2, D3, D6, E1, E2, E3, E4 (lstrings from the schema, lane E round 5), F1..F4, G1..G3, and M3.D4 as M4.D1 (`refs`). Carried, each with its `docs/gaps.md` row: M3.B9 (other deciders; round 4 did the sibling field), M3.D5 (its in-game half is M4.T1 (f), after row 10; its CK half M3.O1), M3.E10 (patch remapping) |
| ADR 0007 committed; M3.O6 recorded | **done** | M3.F4; M3.O6 decided 2026-10-14 (Amendment 1). ADR 0007 Amendment 3 accepted 2026-10-16 (M6.O3); Amendment 4 proposed (M7.O4) |
| `cargo xtask ci` green; corpus roundtrip identical; parity regenerated; STATUS says M4 | **done** at `01c77d8` | Gates as recorded then. Since: every 2026-10-16 lane merged on a green `cargo xtask ci` ("Pause" above); parity regenerated at `a8469e7` |

### M4 is done when

| Criterion | State | Tasks and evidence |
| --------- | ----- | ------------------ |
| QUEST, ACTOR, ITEMS/WORKSHOP typed, with WORLD, PLANET and round 3 merged; M4.BX1 after every merge with 0 typed conflicts | **done** | Six families: round 3, BW, BP, BQ1, BC1, BI1, each merged by M4.BX1. `unordered` record mode for CELL/REFR/ACHR. Since: lane D's component layouts and condition unions (`24ac42d`, round 4) and fix round W3's DOOR/LIGH `OBND`, 0 re-encode conflicts (`docs/record-parity.md` at `a8469e7`) |
| `docs/record-parity.md` shows the per-family counts | **done** | Lane B round 5: the "Families" table (`xtask/src/parity.rs` `FAMILIES`); regenerated at `a8469e7` |
| M4.BX2 inventories (`LAYR RFGP WKMF IRES MESG`) | **done** | Lane B, 2026-10-15 ("Done", "M4.BX2 inventory definitions"): 14 types, every official record placed, decoded and re-encoded identically (`crates/esm/tests/inventory_types.rs`). No owner is assigned to the inventory types yet |
| Builder: computed cell/world paths, rebuild gate (M4.A2); allocator and reference composition tested | **done** | M4.A1, A2, A3, A5, with the Fable review fix round; new interior cells since lane D round 4 |
| NAVM passthrough on a synthetic plugin, and on F22 if it exists | **done** (synthetic) | M4.A4. The F22 half is **pending-owner** with M4.O1 (row 15) |
| `new`, `clone`, `place`, `package`, `testplan`, `papyrus.events` `Implemented` and on MCP | **done** | M4.E1, E2, E6, G2, and `package`; `deploy --dev` live and reviewed (M4.E11). Since 2026-10-16 also `cell`, `revert`, `game.session`/`status`/`probe`/`stop`, `mod.run`, and `dry_run` on `set`, `new`, `clone`, `place` |
| `planet` `Implemented` | **done** | M4.D3, lane D (`kit planet`, alias `pcm`) |
| Spec v3 builds deterministically | **done** | M4.E3 (two builds identical); since M5.D10 `new:` also names a `[[place]]` |
| Cell rules run with 0 errors on the official files | **done** | M4.E4: one known error (`ref.orphan` on 0x1FB). `layout.undecoded` and `layout.unplaced` 0 findings (`validate_corpus_official_files`) |
| First POI built, validated and packaged | **done** | `out\poi\KitKraftFirstPOI.esm` (`docs/poi/first-poi.md`), reviewed |
| First POI visited in game, persistence steps recorded in `docs/game-observations.md` | **partly** | Visit **done**: M4.O2 PASS 2026-10-15, ledger `game-20261015-03` (O2-05, O2-06 pass). Persistence **pending-owner**: O2-08 not run (row 2), or lane C's M5.C2 after row 10 |
| P0 first deliverables W1..W6, W12 | **partly** | W1 **done** (M4.E1, `KitKraftW1.esm`). W2 **done** in game (the visit above). W3 recipe built (M4.E7, `docs/recipes/workshop.md`), **pending-owner** (M4.O4, O4-05). W4 recipe built (M4.E8), **pending-owner** (M4.O4). W5 **pending-owner** (persistence: O2-08 and the O4 persistence rows; `docs/design/persistence.md` done). W6 built (M4.E9 script attachment, `docs/recipes/scripts.md`), in game **pending-owner** (O4-07, row 14) or M4.T6. W12 **pending-owner**: the rename package install (O2-02) was not run in `game-20261015-03` |
| P1 deliverables W7..W10 done or carried; W11 scoping document | **partly** | W7 **done** (M4.D3: `kit planet`, `docs/design/planet-content.md`). W8 recipe built (M4.E7, `docs/recipes/fabricator.md`), **pending-owner** (O4-06). W9 recipe built (M4.E8), **pending-owner** (M4.O4). W10 **done** (M4.D1 `refs`; `VMAD` objects since M5.D6). W11 **done** (M4.G3) |
| M4.O4 in-game results recorded | **pending-owner** | Row 14. The recipes for O4-01 to O4-07 are built; lane C's M4.T6 may close rows first after row 10 |
| `docs/design/persistence.md`, `docs/design/planet-content.md`; `docs/research/asset-formats.md` | **done** | M4.H3 (39 graded claims), M4.D3, M4.G3 |
| M4.O3 and M4.O5 recorded | **done** | ADR 0008, ADR 0009 (2026-10-14) |
| Lane W | **done** (read-only part) | `crates/ui-egui` + `apps/kit` (`docs/workbench.md`); built on since by M6 (lanes V, P, G) and M7 (lanes M, R). Licence ruling: row 7 |
| CK fixtures F20..F28 (M4.O1, M4.H2) | **pending-owner** | Row 15; no longer deferred (the workbench exists). The first POI does not need them |
| `cargo xtask ci` green, corpus roundtrip identical, parity regenerated, STATUS says M5 | **carried** | To M4 close, after the owner rows above. Parity regenerated at `a8469e7`; this file names M5 as running beside M4 |

## M5 exit criteria (reconciled 2026-10-16 at `81272e2`)

Against `plan/execution-plan.md` "M5 is done when", with the states above.

| Criterion | State | Tasks and evidence |
| --------- | ----- | ------------------ |
| Paths (a) and (b) are reached, and M5.A5 has run every fixture the owner recorded | **pending-owner** | (a): M5.A1 (session script) and M5.A2 (`cargo xtask oracle ck`, synthetic fixtures) done; M5.A3 waits on row 12 (M5.O1). (b): M4.T2 done, M4.T3 + M5.C1 built and tested against fakes (`crates/game/tests/session.rs`); M4.T1 and M5.C2 wait on row 10 (M4.O7; ledger `game-20261016-T1`). M5.A5 waits on row 15 |
| `LCTN`, `TERM`, `PKIN` and `FACT` typed-writable with reference integrity (M5.D2..D6); the K0 mini-plugin (M5.D7) has game and save/reload rows | **partly** | Typed-writable and dangling-link tests for the four (`24ac42d`; `kit coverage --matrix` pass), `VMAD` object values followed (M5.D6), missing subrecords, typed arrays and `new:` placements (M5.D8 to M5.D10). **Carried** (lane D, `docs/foothold/next-sprint.md` item 2): M5.D2's corpus clone-sample test and the recipes `docs/recipes/location.md`, `terminal.md`. The mini-site is built (validate 0/0, every link resolves, `KitKraftMiniSite.esm` `05aac031...`); its game and save/reload rows are **pending-owner** (row 10, then M5.C2, or an owner sitting M5.O4) |
| Gate K0 is met: the scripted activator fires in game (M5.B3) and the CK re-export differential is clean (M5.A3) | **pending-owner** | M5.B3 waits on row 14 (or M4.T6 after row 10); M5.A3 on row 12. K0's other three items are met for KitKraft-built fixtures (`docs/foothold/gap-matrix.md` "Gate K0") |
| The PEX differential (M5.B2) is recorded; the CK sources' provenance is complete (M5.B1) | **done** | M5.B1 (`extracted_folder_gives_the_same_index_as_the_zip`, registry row CK-CONTENT-RESOURCES); M5.B2 (`docs/research/pex-differential.md`: 50 of 50 equivalent). The CK's own compile route stays unobserved |
| Every FTH-EXP ticket has a status with evidence (`not-run` with its reason counts) | **done** | `plan/execution-plan.md` M5 lane E: FTH-EXP-01 and -03 agent halves built (prototypes; game and CK rows not-run), FTH-EXP-10 partial (mixed-master decode), the other nine `not-run`, each naming the tickets and owner rows it waits on |
| `docs/engine-limitations.md`, `docs/compatibility.md` and a generated gap matrix exist; `docs/research/hostile-encounters-2026-10-16.md` exists | **partly** | M5.F1, M5.F3 and M5.H1 done. The gap matrix is checked by hand at `81272e2` (M5.F2); its generation is **carried** with M5.D1 (lane D, next-sprint item 2) |
| `cargo xtask ci` is green; the full corpus roundtrip is still identical | **carried** | To M5 close. Every 2026-10-16 merge passed `cargo xtask ci` ("Pause" above); `corpus_full_roundtrip` runs in it when `KITKRAFT_DATA_DIR` is set |

## Toolchain (verified on the dev box)

- rustc 1.99.0 (b940084d7 2026-09-28)
- cargo 1.99.0 (5f94df478 2026-08-27)
- clippy 0.1.99 (b940084d7e 2026-09-28)
- rustfmt 1.10.0-stable (b940084d7e 2026-09-28)
- MSVC 14.37 linker

## Done

- `PLAN.md` brief written (owner).
- Repository skeleton: workspace manifest, lints, 10 library crates
  (`schema esm ba2 pex strings nif doc io engine mcp`), `apps/kit-cli`, `xtask` with a real
  layer check, agent docs, ADRs 0001-0004, docs templates.
- Engine `CommandSpec` type and the first-cut catalog (all entries `Planned`).
- `corpus/manifest.toml`: the 14 official ESMs on this machine with sizes and SHA-256.
- M0.1 toolchain installed (versions above).
- M0.2 first build: `cargo build --workspace` succeeds; `cargo run -p kit-cli -- help` prints
  the catalog.
- M0.3 `cargo fmt --all` applied; `cargo xtask ci` green (layers, fmt, clippy `-D warnings`,
  tests). Fixes needed: three `clippy::manual_contains` hits (engine `find_command`, xtask
  banned-dependency checks). `clippy.toml` keys are all recognised by clippy 0.1.99; kept.
- Layer check verified to fail on a deliberate upward edge (`ba2 -> doc`) and on an unlisted
  same-layer edge (`esm -> strings`); both reverted. (An edge that forms a cycle, such as
  `schema -> engine`, is rejected by cargo itself before xtask runs.)
- Corpus manifest re-verified 2026-10-14: size and SHA-256 of all 14 ESMs match.
- M0.4 first commit on `main`.
- M1.8 (lane B) definition format: TOML loader + validator in `kitkraft-schema`, format in
  `crates/schema/defs/README.md`; `source`/`evidence` must use the ADR 0005 forms.
- M1.9 (lane B, rescoped) seed definitions: TES4 and the GRUP header typed from
  `uesp-skyrim` pages and confirmed on the corpus; GMST, GLOB, KYWD, WEAP as corpus
  inventories (order, cardinality, BFCB..BFCE and OBTE..STOP blocks), every payload opaque;
  WEAP layout pending ADR 0005 (ck-diff). Corpus findings: TES4 MAST has no DATA in any
  official ESM; GRUP type 10 occurs (unnamed, preserved).
- M1.10 (lane B) codegen: `kitkraft_schema::generate(&Schema) -> String`; golden tests for the
  seed set and an all-features fixture; the goldens are rustfmt-clean and compile under the
  workspace lints in `crates/schema/tests/generated.rs`, with `encode(decode(x)) == x` on
  well-formed, malformed and random record bodies.
- Lane A (merged on `main` at 6fcfeaf): M0.5 corpus test helper, M1.1 bounded reader,
  M1.2 parse model, M1.2b TES4 header inventory (`docs/corpus.md`), M1.3 writer, M1.4
  compressed records, M1.5 streaming driver, M1.6 corpus roundtrip, M1.7 hostile input.
  All 14 corpus files roundtrip byte-identical; `Starfield.esm` in 8.77 s with a peak
  working set of 1,238 MiB; zlib-rs at level 9 recompresses 99.88% of compressed records
  byte-identically (`docs/gaps.md`).
- M1.11 (lane C) `execute(id, args)`, the result envelope, and the shared command-line
  parser.
- M1.12 (lane C) `roundtrip` wired to the esm driver, `Status::Implemented`; result carries
  `identical`, counts, `elapsed_ms`, and on mismatch `{offset, expected, actual, path}` with
  evidence at the differing byte; `--out` writes the rebuilt plugin.
- M1.13 (lane C) `kit roundtrip <plugin> [--verify] [--out path] [--json]`: exit 0
  identical, 1 `--verify` mismatch (`data.passed: false`), 2 error. 2026-10-14, release
  build: `kit roundtrip --verify` reports identical and exits 0 on all 14 corpus files
  (M1 exit criterion 2), e.g. `Constellation.esm` 39,210 bytes / 57 records, `Starfield.esm`
  3,829,247 records in 3.1 s with a warm file cache.
- Lane C: minimal `inspect <form> <plugin>` (FormID or EditorID; header fields, subrecords
  with offsets and hex payloads capped at 256 bytes; `typed decode pending schema wiring`).
- Lane B round 2 (review findings 1-13): generated records carry `XXXX` per subrecord
  (`RawSub/OwnedSub.oversized`, typed values as `Sub<T>`); `Ctx.localized` and
  `LString::{Id, Text}`; `size_by_version` probed at every gate boundary, overlaps
  rejected; format adds `if_remaining`, `count_from`, `wstring`, `bstring`, shared
  definitions in `defs/common/` (`group_ref`, `type = "ref:Name"`; BfcbBlock and ObteBlock
  deduplicated) and headers without a signature (`RECORD.toml`, the 24-byte record header,
  confirmed by a header walk of 8 official ESMs); codegen emits `enum Record` +
  `RECORDS`, `DecodeError: Display + Error`, `[T; N]` fixed arrays, reserved prelude names;
  `builtin()` cached; `leaf_subrecords`, `known_signatures`, `Signature::from_bytes`;
  `examples/generated.rs` compiles the goldens outside `cfg(test)`; ADR 0005 Amendment 1
  (`uesp-starfield:`, `/CTDA` forbidden, `/Raw_Data` as inventory cross-check).
- M1.15 (lane C) `cargo xtask codegen [--check]`: `builtin()` -> `generate()` -> rustfmt
  (workspace `rustfmt.toml`) -> `crates/esm/src/generated/{mod.rs,records.rs}`, DO NOT EDIT
  header with the defs hash (FNV-1a 64 over `BUILTIN_DEFINITIONS`); `--check` fails on a
  stale, missing or foreign file and runs in `cargo xtask ci` after `layers`. esm exposes it
  as `pub mod generated` (not wired into the reader). `BUILTIN_DEFINITIONS` has no generated
  region, so `crates/schema/tests/defs.rs` stays its guard (see Requests).
- M1.14 (lane C) `coverage` command (`Implemented`): per record type, record and compressed
  counts and every subrecord signature with its count, classed `typed` / `opaque` /
  `unknown` against the schema; compressed records decompressed by default (Starfield.esm
  8.3 s, 2.4 s with `--no-decode`, release). `cargo xtask parity` runs `kit coverage` and
  `kit roundtrip --verify` over the corpus and writes `docs/record-parity.md` (idempotent,
  checked): 186 record types, 7,490,102 records, 136,741/136,741 compressed records
  examined, 50,419,561 subrecords of which 77 typed, 59,660 opaque, 50,359,824 unknown.
- AGENTS.md clean-room section points to ADR 0005 Amendment 1 (starfieldwiki.net);
  `docs/gaps.md` stale scaffold row removed.
- Lane A round 2 (review findings 1-8, `crates/esm`): the writer rejects a compressed flag
  that disagrees with the body (`FlagBodyMismatch`); decoded compressed records keep their
  inflated bytes and re-emit the original stream when touched without a change;
  `DecodeLimits::max_subrecords` (1,000,000) with a count-then-parse decode into one shared
  buffer (an 8 KB bomb of 1.4 M empty subrecords fails before any is built); two-pass
  writer (`Entry::encoded_len`) that streams modified groups, and
  `driver::rewrite_file` (edit `Starfield.esm`'s `WRLD` in 2.6 s at a 1,293 MiB peak, no
  group buffered); `HEDR` count documented from the corpus, `Tes4::set_record_count`, kept
  verbatim unless records/groups are added or removed (then the 12-file meaning);
  `esm::bridge` (`RawSub`/`OwnedSub`/`Ctx` from the generated module, `Record::raw_subs`,
  `Subrecord::from_owned`, `Plugin::ctx`, `Record::typed`/`set_typed`); fast corpus tests add
  `SFBGS004.esm`/`SFBGS047.esm` (real compressed records and `XXXX` in every gate) and check
  manifest SHA-256; absolute subrecord offsets, reserved `XXXX` signature, depth cap 64, one
  hostile harness, `rust-version = "1.88"`. Full corpus run at 087ab97: 14/14 identical,
  15.4 s, peak 1,379 MiB. Corpus `XXXX` inventory: 0 of 12,849 have a payload <= 0xFFFF.
- Source policy (owner decision 2026-10-14): ADR 0004 **decided**: upstream xEdit
  `Core/wbDefinitionsSF1.pas` pinned at `9fb016884bec138ea6c7b872cec831537d464c3e` is the
  record-definition reference, source material only. `docs/xedit-reference.md` records the
  verification (default branch `dev-4.1.6`, why no tag is pinned, MPL-2.0, all 18 lane B
  record types defined, line spans, Pascal-to-TOML map). ADR 0005 Amendment 2 (four-level
  hierarchy, SF1Edit differential comparison, Skyrim container-only), ADR 0001 Amendment 1
  (clean room), AGENTS.md, PLAN.md, README, NOTICE, ATTRIBUTION updated. Schema loader:
  `xedit:` sources (pinned SHA + line range) and `confidence`
  (`reference`/`corpus`/`ck-diff`/`verified`; required for `xedit:` citations, optional
  elsewhere, always backed by matching citations). `*.pas` gitignored.
- **Owner approval 2026-10-14** (M2.O1, M2.O2, open decision 2b): the owner approved the
  ADR 0004 decision, `docs/xedit-reference.md`, and 2b: `Core/wbDefinitionsCommon.pas` may
  be cited at the same SHA under the same rules. Six clarifications recorded in ADR 0004
  ("Owner approval 2026-10-14"): the common file is authorized (no blind import of
  Skyrim-specific readings); further helper files need only a documented dependency;
  confidence semantics (`reference` allowed source, `corpus` local fixture, `ck-diff` CK
  output, `verified` independent validation); lossless first; raw vs master-aware resolved
  FormIDs, never guess an unavailable master's type; scope discipline. ADR 0004 rule text:
  the loader accepts both files, same SHA. OMOD is in scope (M2.O2). ADR 0005 Amendment 3
  adopts the owner's dossier (`docs/research/starfield-dossier-2026-10-14.md`) as an
  evidence backlog, not a scope change: `docs/source-registry.md` (every source with our
  status), evidence grades A-D alongside `confidence`, the general source rule, and the
  conflict-register entries that matter to us. `docs/xedit-reference.md` gains a
  `wbDefinitionsCommon.pas` section (blob `cfa80ba0...`, 338,167 bytes, 10,348 lines;
  conditions, script data and base form components turn out to be defined in the SF1 file
  itself; the common file supplies object bounds, keywords and callbacks).

- Lane F (M2.F1..F6, F8): `Engine` owns a `Session` (`SessionHandler`s registered in
  `handlers::session_defaults()`; typed slots; `Session::load_order` listing); stdio MCP
  server in `crates/mcp`, hand-written over `serde_json`, MCP 2026-07-28 plus the
  handshake revisions 2025-11-25 and 2025-06-18 (the batch-era 2025-03-26 and 2024-11-05
  negotiate to 2025-11-25; every served version is in `server/discover` and `-32022`;
  `ping`/`initialize` are legacy-only); tools `execute`, `command_line`,
  `inspect_record`, `query_records`, `diff_plugins`, `coverage`, `roundtrip` generated from
  the catalog; resources `kitkraft://commands`, `kitkraft://load-order`; argument and
  command errors as `isError` results, engine bugs `-32603` (`docs/mcp.md`); `kit mcp
  [--profile DIR]`; in-process pipe client test and a spawned-binary test (with
  `KITKRAFT_DATA_DIR`: `inspect_record` on `Constellation.esm` passes, `query_records`
  answers `not_implemented` until M2.D11); `Status::Deferred { reason, milestone }` and
  the `catalog_closure` test (every `Planned` command must be listed in `PLANNED_IN_M2`
  with its owning M2 task; nothing is deferred yet because every command has an M2 owner).
  Review round 2 (Fable, no blockers) fixed: `roundtrip --out` opens with `create_new`
  unless `overwrite: true`, and `destructiveHint` is `true` on the tools that can write;
  `coverage` gained `signatures` and `top`, and MCP defaults them to `false`/`20`
  (`backend::MCP_DEFAULTS`; `Starfield.esm`: 18 KB instead of 695 KB, text leads with
  `data.summary`); `execute` reports `unknown_command` before other violations and takes
  `args: null` as `{}`; a `not_implemented` `--profile` preload stays out of the
  instructions; `docs/mcp.md` states that paths are not confined to `KITKRAFT_DATA_DIR`.
  **Round 3 (2026-10-14):** M2.F8 closed: every catalog command is `Implemented`,
  `PLANNED_IN_M2` is gone and `catalog_closure` fails on any `Planned` command; partial
  commands say so in `limitations` (inspect single-plugin mode, diff/conflicts compare
  stored bytes). `data_dir` defaults to `KITKRAFT_DATA_DIR` on every command that takes it
  (a profile prefers MO2's `gamePath`); `validate` searches a standalone plugin's masters in
  the load order, next to the plugin, then the Data dir, and resolves a bare plugin name
  there (`kit validate --plugin out\X.esm` now passes with only the variable set). M2.F9:
  `schema` command (`kit schema --type WEAP [--member KWDA] [--deep]`, alias `defs`, MCP
  tool `record_schema`): members with type, source citation and confidence, top level by
  default; `cargo xtask parity` adds a per-confidence table from it. Resources:
  `kitkraft://validate-rules` (rules, severities, denylist, docs); `kitkraft://commands`
  gains `cli` and `tools`. The flaky `kit_mcp_on_an_official_file`: spawned servers use a
  private `KITKRAFT_CACHE_DIR` under the test target dir (no more sharing the user's index
  cache with other worktrees' gates), stderr is drained on a thread, and an EOF reports the
  child's exit status and stderr; 20 consecutive runs pass (the original crash did not
  reproduce under 4-way parallel cold-cache stress either). AGENTS.md, ADR 0001 Amendment
  1 and the plan name both xEdit files at one SHA; README has the M2 status and a quick
  start.

- Lane A, M2 (`crates/esm`): M2.A0 review round 3 follow-ups 1-10 (each with a
  regression test); M2.A1 record index (`Starfield.esm` in 3.3 s release, 288 MiB peak;
  counts equal the streaming parse on all 14 files); M2.A2 random access (1,000 seeded
  `Starfield.esm` entries and every small-file record equal the streaming parse); M2.A5
  plugin builder and streaming upsert (replacing one record of each small file changes
  only that record and its ancestors' size fields; top-level group order checked against
  all 14 files); M2.A6 record edit API (set, insert, remove; edited compressed corpus
  records decompress to the edits, every other record keeps its bytes); M2.A3 typed decode
  check in the driver (all 14 files: 13,168 typed records, 0 conflicts); M2.A4 `XXXX`
  writer policy in `docs/corpus.md`. TES4 flag constants named from the pinned xEdit
  definitions. Full release roundtrip at b27d0dc: 14/14 identical, 16.7 s, 1,394 MiB.

- Lane B, M2 (`crates/schema`): **M2.B1** provenance on every node: `Provenance { source,
  evidence, confidence }` on records, headers, shared definitions, members and fields
  (members and fields may set their own keys or inherit); `confidence` required on every
  file (so on every `uesp-skyrim:` citation too), ADR 0004 values; TES4/GRUP/RECORD say
  `corpus`; loader accepts `wbDefinitionsCommon.pas` at the pin; `XEDIT_REVISION` and
  `XEDIT_FILES` public. **M2.B2** universal subrecords typed with corpus evidence (scan of
  all 14 ESMs: 265,346 EDID, 44,077 FULL and 16,434 DESC all 4 bytes, 110,383 KSIZ/KWDA
  pairs consistent, 180,802 of 180,804 KWDA FormIDs resolve to KYWD): `EditorId`, `Name`,
  `Description`, shared group `Keywords { Count, Forms }`; new inventory definitions for
  AMMO, AMDL, COBJ, OMOD, ZOOM. **M2.B3** generated `form_ids()`/`form_ids_mut()`/
  `lstrings()`/`lstrings_mut()` with member paths on every record and on `Record`, plus
  `Record::extra()`. **M2.B9** GMST `DATA` as a union by EditorID prefix (new format
  construct `type = "union"`, `decider = "edid-prefix"`: b/f/i/s/u), GLOB `FLTV` as `f32`;
  KYWD bodies stay opaque (no readable source; `docs/gaps.md`). Corpus check through the
  generated records: every AMDL (126), AMMO (154), COBJ (3,518), GLOB (3,319), GMST
  (2,418), KYWD (6,965), OMOD (3,330), TES4 (14), WEAP (452) and ZOOM (141) record decodes
  with an empty `extra` and re-encodes identical. `BUILTIN_DEFINITIONS` has a marked
  generated region (answers lane C's request).

- Lane G, M2.G0..G9 (`crates/ba2`, `crates/pex`, `crates/strings`; engine
  `handlers/{ba2,papyrus,strings}.rs`), all checked 2026-10-14:
  - G0: ADR 0006 (sources for BA2, `.pex`, string tables); `docs/source-registry.md` rows.
  - G1-G3 `ba2`: GNRL/DX10 v2/v3 reader, zlib and LZ4-block codecs, DDS header synthesis,
    deterministic packer. All 90 official archives list (1,763,906 entries; every
    name/dir hash recomputes; 65 GNRL v2, 10 DX10 v2, 15 DX10 v3 LZ4); 100 seeded entries
    per archive extract to their **declared sizes** (7,360 entries, 7.3 GB, 16 s release;
    this sample checks sizes, not bytes); every texture's chunks equal its mip chain
    (`docs/corpus.md` "Archives"). Byte identity with Archive2 rests on the manual
    differential run below (seven archives), not on the sample.
    **Archive2.exe 1.2.0.1 differential (manual run):** our extraction is byte-identical
    to Archive2's for every entry of `Starfield - Textures01` (5,574) and `Textures05`
    (7,084, all 16 DXGI formats, 25 cube maps), `SFBGS004 - Textures` (18), `OldMars -
    Textures` (42), `Starfield - Localization` (27), `Starfield - Misc` (4,753),
    `SFBGS004 - Main` (739).
    Sample hash: `strings/starfield_en.strings` (1,542,583 bytes) SHA-256
    `E18CADD47270CC7C0F37A33BDE9FE3B4049468CF94558379DA36356BB144951E` from ours, from
    `ba2x.py`, and from Archive2. Archive2 opens our packed archives (Localization strings as
    GNRL v2 and v3, OldMars textures as DX10 v2 and v3) and extracts every file
    byte-identical to the input.
  - G4: `ba2.list`, `ba2.extract`, `ba2.pack` `Implemented` (refuse writes under a game
    Data dir).
  - G5 `pex`: all 5,708 official scripts (Misc, SFBGS00D/047/050 Main, ShatteredSpace
    Main01) parse to their last byte; **text equals `pexdis.py`'s on all 5,575 scripts
    `pexdis.py` can read** (0 differ). `pexdis.py` fails on the other 133: it reads an
    extra `u16` after each struct name; the corpus wins (ADR 0006), so `crates/pex` reads
    structs without it.
  - G6 `papyrus.disasm`, G7 `papyrus.compile` `Implemented`. PapyrusCompiler 4.7.0.5 run
    on Scripts/Source from `Tools\ContentResources.zip`: DefaultAliasOnActivate,
    DefaultCounterQuestA and the owner's noc patch scripts nocOutfitChangerQuest,
    nocOutfitChangerContainer, SQ_Airlock_QuestScript, CS7_SpaceshipLandingQuestScript
    compile through `kit papyrus compile` and `kit papyrus disasm` equals `pexdis.py` on
    each (6 scripts). Never `-release`.
  - G8 `strings`: all 378 official tables (13 archives, 9 languages) parse and rewrite
    byte-identically. G9 `strings.dump`, `strings.merge` `Implemented`. Known string:
    `strings/starfield_en.strings` ID `0x0002E469` = "Old Earth Hunting Rifle", the FULL
    (`69E40200` at offset 21,546,047) of `Starfield.esm` WEAP `0021BBCD`
    (`RussianHuntingRifle`).
  - Review round 2 (2026-10-14), each with a regression test: DX10 chunks split by mip
    **area** (`w * h >= 512 * 512`; recomputes all 136,401 official chunk plans, the
    edge reading gets 421 wrong); `extract_vec(.., cap)` refuses oversized entries before
    decoding (64 MiB `.pex`, 256 MiB tables, `ba2.extract` `max_entry_bytes` default
    1 GiB); one LZ4 blob at most 256 MiB; the user Data folder (`My Games\Starfield\Data`)
    is refused like the game's; extraction refuses Windows-hostile names (trailing `.` or
    space, forbidden characters, CON/NUL/COM1/...) and case-folding or file/directory
    collisions while planning, before the first write; `pack` refuses non-ASCII names
    (`docs/gaps.md`), hash-triple collisions and empty files (Archive2 skips them;
    `ba2.pack` skips and reports them), DDS headers with zero dimensions or too many mips;
    `ba2.pack` `overwrite` (create-new by default); `strings.merge` writes temp + rename
    and reports in-table duplicate IDs (`duplicates`, first wins) apart from cross-table
    conflicts; `.pex` object size fields are checked (all 5,708 official scripts agree)
    and versions other than 3.12 are flagged. Not done (optional): a tolerant parse mode
    for padded string tables (done in M3.G2, below).

- Lane D, M2.D1..D13 (`crates/doc`, `crates/io`, `docs/load-order.md`; engine
  `handlers/{load,inspect,query,diff,conflicts}.rs`), all checked 2026-10-14:
  - D1 `docs/load-order.md`: every MO2 profile rule with its evidence (file and line, or
    nolvus.net / STEP / Ortham / LOOT by URL). modlist.txt's first line is the highest
    priority (docs plus a file check: both Free Lanes compatibility patches ship the same
    files as their bases and are listed first). **Still open (M2.O4):** MO2's own UI view of
    one such file.
  - D2 `io`: read-only profile reader, overlay (overwrite > mods by priority > Data,
    case-insensitive), plugin plan naming why each plugin is active (`Starfield.ccc`,
    hardcoded official masters, `plugins.txt`, MO2 unmanaged entries, BlueprintShips
    companions). Default profile: 84 active plugins, every one an existing file (70 from
    mods, 14 from Data), order is `loadorder.txt`'s entries under `hardcoded-official-slots`
    (review round 2 below), profile files unchanged.
  - D3 (a, b) corpus: raw FormIDs address master *classes* by scale (`FE` small, `FD`
    medium, else full; the index counts that class only; the file itself is the next index
    of its own class), confirmed on SFBGS047 (masters full, small, full: its overrides of
    the second full master are `01xxxxxx`). All 14 files resolve every header FormID; 15
    SFBGS050 overrides address SFBGS00D records that do not exist; largest own object ID
    per scale fits 12/16/24 bits. (c) flag names from the pinned xEdit file via the owner's
    dossier; runtime slots are the assumption `runtime-slots-by-scale` (stated in every
    result that shows a runtime ID).
  - D4 `doc::LoadOrder` (raw vs `GlobalForm`, master maps, missing masters are typed
    errors, late masters reported); D5 index cache (`%LOCALAPPDATA%\kitkraft\cache`, path +
    size + mtime + both format versions + checksum) and override table; index and random
    access now through lane A's `esm::index` (M2.A1/A2). Default profile: 7,739,193
    records, cold 6.0 s, warm 0.68 s (`kit load`, release). 27 third-party small/medium
    plugins store own records full-style: resolved by the named assumption
    `full-style-self` (Ortham's documented example; no official file does it); 4 records
    of `HG_ShipCargoHoldAtHome.esm` stay unresolved.
  - D6 conflict classes; Default profile: 15,586 overridden forms, 15,893 overrides
    (8,206 deleted, 6,031 differs, 1,641 identical to previous, 15 without a base), 2.8 s.
  - D7 string tables through lane G's readers (loose via the overlay, else
    `<stem> - *.ba2`): `Starfield.esm` WEAP `21BBCD` FULL resolves to "Old Earth Hunting
    Rifle" with the table offset as evidence (release corpus test).
  - D8 `load`, D9 `inspect` (every version, winner, classes, lstrings, schema-typed
    values), D10 `where` language (typed member paths through lane B's schema, 10,000
    random inputs), D11 `query` (`kit query --type WEAP --where "FULL ~ 'Old Earth Hunting
    Rifle'"`: the rifle, 0 unknown, 0.2 s warm in-process), D12 `diff` (corpus file vs a
    copy with one flipped byte: exactly that subrecord, offsets on both sides), D13
    `conflicts`: **esmscan.py run read-only on the Default profile (master Starfield.esm,
    83 plugins) and `kit conflicts --master Starfield.esm` agree on all 59 plugins and
    every per-type count (8,234 override records).** All `Implemented`, tested through
    `execute`, reachable over MCP (`execute`, `inspect_record`, `query_records`,
    `diff_plugins`; `load` fills `kitkraft://load-order`).
  - **Review round 2 (2026-10-14), all 13 findings fixed with regression tests:**
    (1) order by the named assumption `hardcoded-official-slots`: `Starfield.ccc` plugins,
    then official plugins without a `plugins.txt` entry (`ShatteredSpace`, `SFBGS004`,
    `SFBGS00D`, `SFBGS050`, `BlueprintShips-SFBGS050`, `SFBGS047`, which MO2's
    `loadorder.txt` lists after managed plugins naming them as masters), then the rest.
    Default profile: late master links 2 -> 0; 15 forms change winner, all from a slotted
    official plugin to a later managed mod (the reviewer's 15). Remaining late masters are
    an ordering error in every result's limitations. M2.O4 stays open for the game's own
    order. (2) `full-style-self` IDs masked to the plugin's scale (`01012345` in a medium
    file = its `2345`), `resolved_by` per version, examples in `load`, same-plugin
    collisions reported; Default profile: 63,798 records of 27 plugins, none masked, no
    collisions. (3) raw `00000000` is null (`ResolveError::Null`, `{"null": true}`, `where`
    `null`); formrefs refuse object 0. (4)/(5) the load order's limitations (failed or
    missing plugins, missing masters, ordering errors, assumptions, profile warnings) are
    shared by `load`, `inspect`, `query` and `conflicts`, implicit loads included. (6)
    `ResolveError::MasterNotLoaded` / `PluginNotLoaded`. (7) `deleted` on versions, results
    and the inspect summary; `where` never evaluates a deleted payload (unknown with the
    reason). (8) kind mismatches are unknown except union variants; `&` bit test
    (`@flags & 0x20`). (9) `inspect` bare hex needs `plugin`; runtime IDs take `runtime:`.
    (10) cache staleness rule documented; mtime 0 never cached; a content sample (size +
    first/last 64 KiB) catches same-size-and-mtime edits; cache format 3. (11) `diff` keeps
    the files' plugin spelling; raw fallback names the own-scale condition. (12) `@flags`
    without an index entry is unknown. (13) runtime slots follow the scale bits only (update
    and blueprint flags do not change the class), doc and code agree; no profile plugin sets
    0x200. New Default-profile numbers (release): `kit load` cold 4.7 s / peak working set
    482 MB, warm 0.73 s / 376 MB (reviewer: 478 / 375 MB); conflicts 15,586 forms, 15,893
    overrides (8,206 deleted, 6,028 differs, 1,644 identical, 15 first versions), 2.7 s;
    esmscan counts unchanged (8,234 records from Starfield.esm by 59 plugins).
- Lane E, M2.E1..E10 (engine `src/edit/`, `src/validate/`, `handlers/{set,validate,build}.rs`,
  the `FIELD=value` shorthand; `docs/validate.md`, `docs/smoke-rename.md`), all checked
  2026-10-14:
  - E1 shorthand: `kit set <form> FULL="..." --into X.esp`, `SIG.member=1.5`,
    `SIG=hex:...` give exactly the `--fields` JSON (values always strings; the command
    parses them by type). E2 field paths `SIG`, `SIG[n]`, `SIG.Member[n]` against lane B's
    schema (unions by EditorID prefix), typed values range-checked, `hex:` for opaque,
    `{mul}/{add}`, FormID 0 is `null`; errors name the path and a rule id.
  - E3 writing rules in `docs/corpus.md` ("Writing rules"), asserted by
    `crates/engine/tests/writing_rules.rs`: master flag for `.esm`, `HEDR` 0.96 / next ID
    `0xFF001000` / form version 582, never localized, string ID 0 has no text, **no ONAM for
    top-level overrides** (ONAM lists only group-type 7/9/10 children; 853 top-level
    overrides in 10 official files are never listed). The `ck-diff` half (M2.O4) is a
    limitation of `set` and `build`.
  - E4 patch writer: lossless copies (only edited subrecords change), every lstring of a
    localized source inlined from lane D's string tables (ID 0 -> empty; unresolved or
    non-UTF-8 refuses), masters = source masters + source (prefix-compatible targets
    extended; otherwise remapped through the M2.B3 visitors, refused when opaque), the
    cell/world/topic parent rule, never a load-order plugin / game Data / user Data / MO2.
  - E5 `set` across the load order (session or `profile`/`plugins`): winning version,
    before/after per field, evidence in source and patch. Until lane D's round 2 it refuses
    an unverified load order (failed plugin, late or missing master, active plugin without
    a file) and a deleted winner; scaled sources are refused (round 2).
  - E6-E8 `validate` (session-aware; `{plugin?, scripts?, rules?}`): `masters.missing`,
    `masters.order`, `formid.dangling` (header + typed members, scale-aware full slots),
    `lstring.copied-id`, `override.stripped-name`, `record.flags-body`,
    `papyrus.denylisted-call` (`Debug.SetGodMode`, from our noc patch). On the 14 official
    files (`validate_corpus_official_files`, release, 76 s): **0 errors**; warnings: 92
    `override.stripped-name` on CELL/LCTN/NPC_/QUST/WRLD (the DLCs drop those names
    themselves, so those types warn instead of erroring) and 15 `formid.dangling` in
    SFBGS050 (overrides of SFBGS00D forms SFBGS00D lacks: injected records, the same 15
    lane D found).
  - E9 `build` from TOML (`[[rule]]` over `query` matches, `[[set]]`, conflicts named by
    key, fresh plugin validated before it replaces `out`).
  - E10 rename: `C:\Modding\kitkraft\out\OldSovietMarksmanRifle.esm` (2,197 bytes; `TES4`
    master flag, `HEDR` 2 records, master `Starfield.esm`; one `WEAP 0021BBCD`
    `RussianHuntingRifle` with FULL "Old Soviet Marksman Rifle" and its 8 other lstrings
    inline), identical from `kit set` and `kit build docs\smoke-rename.toml`; `validate`
    0 errors / 0 warnings; `roundtrip --verify` identical; with the plugin appended
    `inspect` shows the new name winning. Automated in `crates/engine/tests/smoke_rename.rs`
    (64 s debug). Owner steps: M2.O6 below and `docs/smoke-rename.md`.
- Lane E round 2 (Fable review, 13 findings, each with a regression test), 2026-10-14:
**M2.O6 result (2026-10-14):** PASS. Owner loaded `OldSovietMarksmanRifle.esm` (SHA-256 9c0b28a6...) through MO2 on a disposable save, `player.additem 0021BBCD 1`, and the item displayed as "Old Soviet Marksman Rifle". First in-game confirmation of the set/validate/build pipeline and of inline UTF-8 lstrings in a non-localized override of a localized master.

  `build` guards `out` like any patch target (a load-order plugin named as `out` was
  overwritten) and needs `overwrite` for an existing file; `set` stages, runs the plugin
  rules and writes only when its records pass; both stage under the OS temp dir (stale
  dirs removed). Corpus lstring table (`edit::lstrings`, `docs/validate.md`: the xEdit
  file is not on this machine, so the corpus method; 185 types, 105 lstring signatures,
  checked by `lstring_table_matches_the_corpus`): untyped lstrings (ATTX, ITXT, SHRT, ...)
  are inlined, a non-zero 4-byte subrecord of unknown layout from a localized source
  refuses the copy unless `allow_untyped_lstrings`, `validate` warns on them in overrides
  of localized masters. Small/medium sources (or masters) refused; own-form dangling
  FormIDs are errors. New `TES4`: `CNAM` "DEFAULT" + `INCC` (interior cells, true in all
  14 files); `SNAM`/`BNAM`/`CHGL` a stated deviation. Non-ASCII inline text, struct-tail
  lstring edits, whole-vs-member build conflicts, exact 64-bit `add` / 2^53 `mul`,
  `inf`/`nan`/`1e400` range errors, hex strings NUL-terminated, evidence before rename,
  `callmethod` class matching. `validate_corpus_official_files` unchanged (0 errors, 92 +
  15 warnings). The rename plugin was regenerated (TES4 only changed):
  **2,221 bytes, SHA-256 `9C0B28A6E9C4CDF4C7878CF7DD745A022CF6E0AE943D2AD3DA6C8F044078EA27`**.
- M3 lane A (container), 2026-10-14:
  - M3.A1 recompression study: the 160 compressed records zlib-rs level 9 does not
    reproduce are listed in `docs/corpus.md` ("Recompression misses";
    `corpus_recompression_misses`) and classified in `docs/gaps.md` ("Recompression
    parity"): 147 NPC_/CELL (class E) differ only in the distance of their last match,
    which runs to the end of the data, so the original compressor's choice depends on
    bytes past the end of the record; 12 NAVM of one cell (class N) are level 8; 1 NAVM is
    unexplained. No zlib-rs window/memory-level/strategy/level/tuning/flush variant, nor C
    zlib 1.3.2 or miniz_oxide, reproduces class E (out-of-tree study, procedure in
    `docs/gaps.md`). Default writer unchanged (level 9) pending M3.H3. F07 candidates filed
    to lane H (Requests).
  - M3.A2 `roundtrip --decode` in the driver: with `RoundtripOptions::decode` every
    compressed body is inflated strictly (declared size, end marker and checksum, nothing
    after the stream), its subrecords parsed and re-encoded to the inflated bytes; a body
    that fails is a `Mismatch` (offset where inflating stopped, path ending in
    `> compressed body: <fault>`), decode limits stay errors. Synthetic fixtures for each
    fault; the M1.7 harness accepts only such decode mismatches. Full corpus 14/14
    identical with it (18.3 s, 1,397 MiB, `docs/corpus.md`). The command side is M3.F1
    (Request to lane F).
- M4 lane A (container), 2026-10-14:
  - M4.A1 cell and world group structure (`docs/corpus.md` "Cell and world groups";
    `kitkraft_esm::cells`; `crates/esm/tests/cells.rs`, every rule asserted on every group
    and record of the 14 files, 27 s release): group types 1..10 with parents, labels and
    contents; type 10 = quest children (`DLBR`, `DIAL` + type 7, `SCEN`); persistence =
    flag 0x400 and type 8 together (115,985 / 6,714,413, no exception); a world's
    persistent references in its persistent cell (the flag-0x400 `CELL` first in type 1);
    interior block/sub-block = last two decimal digits of the object ID (17,020 cells; `FE`
    object IDs are 12 bits, decided by `SFBGS047` `FE001016`); exterior sub-block = grid/8,
    block = sub-block/4, labels y low / x high (22,682 cells); grid = `XCLC` i32 pair, x
    first, and cell width 100 units (99.983 % of 2,038,073 exterior references), both found
    by corpus search, not from a definition; order rules; the official-pipeline vs
    `BlueprintShips` differences (`PlacementStyle`). Not waiting on BW: the two layout
    facts used (`XCLC` grid, `DATA` floats 0-1) are corpus-derived and named as such.
  - M4.A2 computed placement paths and the rebuild gate: `cells::placement(&RecordHeader,
    &Parent, &PlacementStyle)` (cell children persistent by flag 0x400; refusals where the
    corpus never shows a combination), `cells::cell_place_in` / `persistent_cell` from an
    index; `Plugin::upsert_styled` and the streaming upsert (`RewriteOptions::placement`)
    create blocks, sub-blocks, world, cell, quest and topic children and records in corpus
    order, with style stamps and the placeholder top-level groups. **Rebuild gate**
    (`crates/esm/tests/rebuild.rs`): every top-level group of all 14 files rebuilt from
    an empty plugin through computed paths, in file order and shuffled, is byte-identical
    (12 pipeline files including the whole of `Starfield.esm`, not a sample; the two
    `BlueprintShips` files after 69 listed differences: 17 empty cell children groups, 52
    off-majority group stamps). 116 s release; the five small files whole in every gate.
    `Plugin::upsert` keeps its signature (official style); INCC stays the caller's (lane E
    already adds inserted interior cells).
  - M4.A3 new-form allocator `kitkraft_esm::forms::NewForms` (`for_index`, `for_plugin`,
    `allocate`, `peek`, `write_hedr`): own FormIDs `<master count><object ID>` above both
    the `HEDR` next object ID and the highest own object ID, `HEDR` advanced keeping its
    high byte; small/medium plugins, both scale bits, 253+ masters and object IDs past
    `0xFFFFFF` are `Error::Refused`. Corpus defaults named in `forms::LIMITATIONS` until F06
    (and F11/F26). `tests/forms.rs`: every small/medium official file refused; the 6 full
    ones start one past their largest own object ID (Starfield.esm `003FFD5A`).
    `Tes4::set_next_object_id` added.
  - M4.A4 NAVM passthrough `kitkraft_esm::navmesh`: `import_navmeshes(&source, &mut
    target, &style)` (in memory) and `navmesh_placements(&source, &target_index,
    read_cell, &style) -> Vec<Placed>` (for the streaming upsert) copy every `NAVM` of a
    CK-saved plugin unchanged (compressed bodies as written) into the temporary children
    of the same cell in the target, `NAVM` first; refused (`Error::Refused`) when the
    master lists differ in any way (payload FormIDs are opaque), when the target lacks the
    cell, or for a `NAVM` outside a cell's temporary children. Synthetic test: a "CK"
    plugin minus its navmesh plus the import equals the CK plugin byte for byte, in memory
    and streamed. `tests/navmesh.rs` runs on `observe\<date>-F22\` and skips with one line
    without it (F22 not observed yet). `NAVI` is not imported (F22 will show whether the
    CK changes it).
  - M4.A5 reference composition `kitkraft_esm::refs`: `compose_reference(&RefSpec,
    allow_reference) -> ComposedRef { record, members, limitations }` and
    `ComposedRef::placed(CellPlace, cell, &style) -> Placed` (persistent -> the cell's
    type 8, which for a world is its persistent cell). Raw subrecords in corpus order
    (`EDID`, leading, `NAME`, extra, `DATA`, trailing); flags 0x400/0x800 from the spec.
    Each member's confidence comes from the schema's `REFR`/`ACHR` definition when one is
    built in (lane BW's, after BX), else from the corpus: `NAME`, `DATA` X/Y, persistence
    `corpus`; `DATA` Z, rotation, flag 0x800 and raw flags below corpus, refused unless in
    `allow_reference`. So until BW+BX land, a POI placement must allow
    `DATA.Position.Z` and `DATA.Rotation`; afterwards nothing changes in code.
    `tests/refs.rs`: reference shape on all 6,734,456 references; 9,144 sampled
    references composed from their decoded values are byte-identical.
  - M4.A6 (corpus half; F23 still open for the CK's own choices): `ONAM` membership rule
    found and asserted on every record of the 14 files (`tests/onam.rs`): listed exactly
    when the record overrides a record its master has and sits in group type 7, 9 or 10
    (`cells::ONAM_GROUP_TYPES`); never type 8, cells or top-level records. The official
    list order is not reproducible (neither file nor FormID order). Writer:
    `cells::onam_forms(&plugin, overrides_master_record) -> Vec<u32>` (file order, a
    stated choice until F23) and `Tes4::set_overridden_forms(&mut tes4, &forms)` (after
    the master list, as in every file). Request to lane E below.
  - Fix round after the Fable review of the placement core (2026-10-14): (1) blocker:
    the own FormID prefix counts only full masters (`NewForms` takes the masters'
    scales, refuses unknown ones); (2) a children group without its record is refused,
    `cells::carry_parents` carries the `WRLD`/`CELL` overrides, `upsert_before_owner` is
    the rebuild-only opt-in; (3) existing interior cells keep their observed blocks;
    (4) a persistence toggle moves the record (`Upsert::Moved`); (5) off-cell positions
    are named, bad reference specs refused; (6) `onam_forms_in`, `NavmeshImport`
    master overrides; (7) `PlacementStyle::detect`, flag-0x400 path checks, one
    persistent cell per world, iterative walks, `DecodeLimits` through the navmesh
    import, the blueprint `HEDR` count limitation. Rebuild gate and full roundtrip
    unchanged (see the lane's report). Lane E: new signatures in Requests below.
- Lanes F and G, M3 (2026-10-14):
  - M3.F2 `coverage` decodes every record of a defined type through its generated
    definition (plugin context: form version, TES4 localized flag) and encodes it again:
    typed occurrences whose payload does not decode are `failed` (counted apart, first 20
    in `typed_failures` with offsets), re-encode differences are `reencode_conflicts`,
    decoded occurrences are split by member confidence (`decoded_by_confidence`).
    `cargo xtask parity [--check]` adds a "Decoded through the definitions" table; corpus:
    20,437 records decoded, 40,776 typed occurrences, 0 failed, 0 re-encode conflicts, all
    `corpus` confidence; `--check` passes on the regenerated file.
  - M3.F3 hygiene: `docs/gaps.md` rows name their closing M3 tasks (the `PLANNED_IN_M2`
    and "MCP server not implemented" rows removed); answered Requests struck through with
    where they were done; `docs/mcp.md` names the coming `refs` (M3.D4).
  - M3.F4: `plan/adr/0007-egui-shell-over-the-catalog.md`, status **Proposed**: options
    (A: minimal egui shell in M3 = M3.F5; B: defer to M4; C: iced, Slint, web), costs, and
    the recommendation B. Awaiting the owner's M3.O6; M3.F5 is gated on it. No UI code.
  - M3.G1 `papyrus.compile` takes `sources_zip` (`kit papyrus compile ... --sources-zip
    <Tools\ContentResources.zip>`): the zip's 5,034 `Scripts/Source` `.psc`/`.flg` files
    (12.3 MB) are extracted once into the KitKraft cache, keyed by name, size and CRC-32,
    hostile names refused before any write, and imported after `imports` (zip crate 9,
    deflate only). PapyrusCompiler 4.7.0.5 with only the zip: DefaultAliasOnActivate,
    DefaultCounterQuestA and the noc patch's nocOutfitChangerQuest,
    nocOutfitChangerContainer, SQ_Airlock_QuestScript, CS7_SpaceshipLandingQuestScript all
    compile (first call 14.5 s with the extraction, then 1-2 s each). Gaps row closed.
  - M3.G2 tolerant string-table parse: `kitkraft_strings::ParseMode::{Strict, Tolerant}`,
    `Table::parse_with` -> `Parsed { table, padding }` (`Padding { kind: DataTail |
    AfterData, offset, len, zeros }`); `Table::parse` stays strict and unchanged.
    `strings.dump` / `strings.merge` take `tolerant` and name each padding run in
    `limitations` and `data.padding`; a strict `DataSize` failure hints at `tolerant`.
    Lane D's string loading (`crates/doc`) still parses strictly.
  - M3.G3 (research) Archive2 1.2.0.1 and non-ASCII names, black-box runs on synthetic
    files (`docs/corpus.md` "Non-ASCII names in Archive2"): names lower-cased and stored
    as Windows-1252 (characters outside it fail the run); name/dir hashes stop at the
    first non-ASCII byte (extension field: 0 there), so such names collide and Archive2
    silently kept 1 of 4; bytes 0xF0..F5/F8..FD hash to run-dependent values (16 of 256
    hashes changed between two runs). `crates/ba2`: names decoded as Windows-1252 (was
    Latin-1; extraction now matches Archive2's names), `hash::archive2_record_hashes`
    with tests from the observed values, the reader's hash check accepts it for non-ASCII
    names. `ba2.pack` keeps refusing them (collisions, unstable bytes, game lookup not
    observed); gaps row restated.
- Lane F, M4 (2026-10-14):
  - M4.F1 bare input names (the F7 Request): `handlers/resolve.rs` resolves an input
    plugin, archive or table given as a bare file name the same way in every command:
    the session's load order (by plugin name), then the working directory, then the Data
    dir (`data_dir`, else `KITKRAFT_DATA_DIR`); a path with a directory is used as given.
    Used by `roundtrip`, `coverage`, `diff`, `ba2.list`/`extract`, `strings.dump`/`merge`,
    `papyrus.disasm` (now session handlers through `resolve::Resolving`, with a
    `data_dir` argument each), `validate` (`plugin`, `scripts`), `inspect`'s single-plugin
    `plugin`, and the `plugins` list of every load-order command (`load()`: `set`,
    `build`, `conflicts`, `query`, ...; a bare name in the working directory now wins over
    the Data dir there, as `validate` already did). A bare name found nowhere is the new
    `EngineError::NotFound` (kind `not_found`, `error.searched` in `kit --json` and MCP).
    Outputs (`out`, `out_dir`, `into`) never resolve, so the write guards are unchanged.
    Tests through `execute` per command (`handlers/resolve_tests.rs`): `Constellation.esm`
    (or its archives, `Starfield - Misc.ba2` for `papyrus.disasm`) resolves from
    `KITKRAFT_DATA_DIR`, and an empty `data_dir` gives `not_found` naming it.
  - M3.F1 `roundtrip --decode` (lane A's M3.A2 Request): `decode` in the catalog, CLI
    (`--decode`) and MCP passes `RoundtripOptions::with_decode()`; with it `data.passed`
    is `identical` (a check, like `verify`), `data.decode` is reported, the first
    limitation states the strict decode, and a decode fault's summary names its path.
    End to end: `kit roundtrip <corrupt> --decode --json` exits 1 with `data.passed:
    false` on a synthetic plugin whose compressed GMST has its Adler-32 flipped
    (`apps/kit-cli/tests/exit_codes.rs`, `roundtrip_decode_exit_1`); both `docs/gaps.md`
    roundtrip rows closed.
- Lane E, M3 (engine `src/edit/`, `src/validate/`, `src/build/`,
  `handlers/{set,validate,build,overhaul}.rs`, `docs/overhaul/`):
  - M3.E3 `validate` hardening: `FE`/`FD` FormIDs resolve through lane D's class encoding
    (`kitkraft_doc::RawRef`; a master of unknown scale before the index and
    `full-style-self` stay unresolved, never guessed); `scripts` takes `.ba2` archives and
    `plugin_archives` adds the plugin's own `<stem> - *.ba2`. `validate_corpus_official_files`
    (release, 83 s): 0 errors, unresolved FormIDs 0 on all 14 files, warnings unchanged
    (92 `override.stripped-name` on CELL/LCTN/NPC_/QUST/WRLD, the DLCs' own; 15
    `formid.dangling` in SFBGS050, overrides of SFBGS00D forms SFBGS00D lacks).
  - M3.E6 `build` spec v2 (`crates/engine/src/build/`): named classes, `[[rule]]` over a
    class with `via` (typed FormID members followed hop by hop; lane D's `follow`, M3.D4,
    replaces the local one when it lands), operators `mul`/`add`/`min`/`max`/`clamp`,
    one write per record across rules and classes (different writes to one field name both
    keys), `unknown member` checked against the schema, `strict`, `plan` (change list with
    before/after, per-class and per-rule counts, nothing written), and the
    `reference`-confidence gate (`allow_reference`). Built and tested on the members typed
    today (GLOB `FLTV`, GMST `DATA`, `EDID`, `KSIZ`/`KWDA`, `FULL`); the weapon members
    (WEAP/AMMO/PROJ/AMDL, lane B round 3: M3.B1..B3) plug in through the schema with no
    code change. The `reference` gate is tested on a synthetic schema (the builtin has no
    `reference` member yet). Version 1 specs build as before (the rename keeps its hash).
  - M3.E7 `overhaul.scaffold` (CLI `kit overhaul`, `handlers/overhaul.rs`,
    `build/scaffold.rs`): a starter spec v2 per record type, one class per `group_by` value
    with match counts and min/median/max of each typed numeric member as comments, every
    rule commented out; deterministic; `out` guarded (`.toml`, never game/user Data or
    MO2, `overwrite`). On the Default profile `kit overhaul WEAP --group-by KWDA`: 541
    winning WEAP, 40 classes, 0.8 s warm (release); `kit build <scaffold> --plan`: 0
    changes, every class's count equal to the scaffold's (4.5 s). Today the only typed
    numeric WEAP member is `KSIZ`; the weapon stats appear when lane B types WEAP (M3.B1).
  - M3.E8 design brief template `docs/overhaul/brief.md` (source, critique points with
    owner's-words claim, timestamp, classes, desired change, measurable target; out of
    scope; in-game checklist; the traceability rule `brief = "B<n>"`). Next for the owner:
    **M3.O4** (pick the critique, fill the brief), which gates M3.E9.
  - Not started, gated: M3.E1/E2 on lane D's M3.D1 (`missing_active_plugins`,
    `Resolved.bytes`, not on main); M3.E4 on M3.B8; M3.E5 on M3.A3 (and M3.H3/M3.O1);
    M3.E9 on M3.O4 (owner); M3.E10 on M3.B1 (typed WEAP).
- Lane D, M3 (`crates/doc`, `crates/io`, `docs/load-order.md`; engine
  `handlers/{load,inspect,query,diff,conflicts}.rs`), 2026-10-14:
  - M3.D1 lane E's round 2 requests: `Loaded::missing_active_plugins` (a field; also
    `missing_active_plugins` in `load`'s listing) and `kitkraft_doc::source`:
    `LoadOrder::source_check(form, missing_active)` / `Loaded::source_check(form)` returns
    the winner with `blocking` and `unrelated` problems (`is_safe()`); `inspect` shows it as
    `winner_source`. Blocking: no or deleted winner, a version's plugin loading before its
    master, a plugin after the winner that failed to load or has unresolved records that
    could be this form (it names the defining plugin and a stored object ID matches). Default
    profile: the rifle `Starfield.esm:0021BBCD` is safe (unrelated: 4 unresolved records of
    `HG_ShipCargoHoldAtHome.esm`, none can be it). `strings::Resolved` has `bytes` (the
    table's exact bytes) and `utf8`; every non-UTF-8 `starfield_en` entry resolves with its
    bytes (`crates/doc/tests/strings_corpus.rs`).
  - M3.D2 (generic part; proven on today's typed members): `kitkraft_doc::fields` compares
    a subrecord with a schema layout member by member (`typed::decode`), FormIDs resolved
    by plugin name on each side, opaque members as payload byte ranges
    (`Decoded::Opaque { at, len }`); `compare::classify_with` / `diff_subrecords_with`,
    `conflicts::classify_versions`. `diff`, `conflicts` and `inspect`'s `vs_previous` report
    `member` and `fields[] {path, before, after}` (paths `KWDA.Forms[1]`, `FULL.Name`) and
    count `equal_by_value` (bytes differ, values equal: another master order). Corpus:
    SFBGS004.esm vs a copy with one keyword changed reports `KWDA.Forms[0]`
    (`crates/engine/tests/diff_corpus.rs`); differently ordered master lists compare equal.
    WEAP/AMMO/... fields beyond the universal subrecords follow automatically once lane B
    types them (M3.B1+). Default profile `conflicts --count-classes`: 6,029 differs, 1,644
    identical (unchanged by the typed comparison), 2.7 s warm.
  - M3.D3: `kitkraft_doc::confidence::reference_members` finds the `reference`-confidence
    members behind a value (a whole decoded subrecord, or a `where` path); `inspect` and
    `query` name them in `limitations` (`... reference confidence ...: GLOB FLTV.Value`).
    No builtin member is `reference` yet, so the test (`reference_members_surface_as_
    limitations`, through `execute`) installs a schema whose GLOB `FLTV` cites xEdit at
    `reference` (a test-only session slot, `SchemaOverride`). `query` decodes through
    `search::run_page_with(.., schema)` (`run_page` keeps the builtin schema).
  - M3.D6: `crates/doc/tests/typed_corpus.rs` (five small files fast, all 14 `#[ignore]`,
    release 24 s): the interpreter `inspect`/`where`/`diff` decode with agrees with the
    generated decoder behind `Plugin::typed` on all 20,423 records of defined types
    (FormIDs and string IDs), and every lstring ID resolves except 181 pinned corpus facts
    (85 ARMO FULL IDs of Starfield.esm; BlueprintShips-SFBGS050.esm ships no string
    tables). Counts in `docs/corpus.md`; the stale schema/inspect row of `docs/gaps.md`
    closed, the lstring row restated.
  - Not done, gated: M3.D4 (`follow`, `refs`) needs M3.B1 and M3.B2 (typed WEAP, AMMO,
    PROJ) on main; M3.D5 needs the owner sessions M3.O1 (F00) and M3.O2.
- Planet schema family (parallel schema lane, `crates/schema/defs`, 2026-10-14): PCMT,
  PCBN, PCCN, LVLP, PKIN, GBFM, BIOM, PNDT, STDT, SUNP, SFBK, SFPT, RSGD, RSPJ, TRAV
  transcribed from the pinned xEdit files (one commit per type, every member cited with
  its line range); new shared `common/` definitions Condition, ObjectBounds,
  ObjectPaletteDefaults, Transforms, ColorBgra, BiomeNoiseLayer, ResourceBiomeTier,
  ResourceCellTier. Corpus check (14 files, out-of-tree checker over the generated
  decoder): all 26,290 records of the 15 types decode with every subrecord placed, nothing
  in `extra`, and re-encode byte-identically (PNDT after decompression); confidence
  `corpus`, members never seen in an official file `reference`. Opaque by design:
  component blocks, CTDA unions, VMAD, OBTS, NVNM, SUNP REFL/RDIF, unnamed slots (gaps
  rows). PCCN occurs (2,148 records); no RSPG/RSSD exist. Mod-author guide:
  `docs/schema/planet-family.md` (tree composition, pack-ins, the GBFM component
  histogram, opaque members, open questions). ~~Derived files are left to the integrator
  (M4 family lane rule, BX): `crates/esm/src/generated/`, the `BUILTIN_DEFINITIONS` region
  of `crates/schema/src/lib.rs` (23 new entries), `crates/schema/tests/golden/seed.rs`
  (`KITKRAFT_UPDATE_GOLDEN=1`) and the builtin-set lists in `crates/schema/tests/defs.rs`
  (`builtin_set_loads_and_matches_the_directory`).~~ Done in M4.BX1 (PLANET merge,
  2026-10-14): all four regenerated on main (`codegen --check` clean); `cargo xtask ci`
  passed with `KITKRAFT_DATA_DIR`; ignored release corpus tests for esm, doc and engine
  pass (M2.A3 typed check 0 conflicts; doc `typed_corpus` 46,713 typed records, 181
  pinned unresolved lstrings as before); `docs/record-parity.md` regenerated: 25 record
  definitions, 50,419,561 subrecord occurrences, 379,633 typed (0 failed), 218,127
  opaque, 49,821,801 unknown (Starfield.esm parity pass 43.6 s).
- Quest schema family (parallel schema lane M4.BQ1, `crates/schema/defs`, 2026-10-14):
  FACT, QUST, PACK, DIAL, INFO, SCEN, SMBN, SMQN, SMEN, CHAL transcribed from the pinned
  xEdit files (one commit per type, every member cited with its line range); new shared
  `common/` definitions ScriptAttachment (VMAD header typed, scripts and fragments raw; a
  type), PackageLocation, QuestReferenceAlias, SoundReference; PLANET's Condition reused.
  Corpus check (14 files, out-of-tree checker over the regenerated decoder): all 267,549
  records of the ten types decode with every subrecord placed, nothing in `extra`, and
  re-encode byte-identically (FACT 953, QUST 2,610, PACK 4,578, DIAL 87,031, INFO
  162,028, SCEN 9,673, SMBN 173, SMQN 471, SMEN 22, CHAL 10; none compressed; about 6 s in
  release for all ten). INFO and DIAL are not order-unstable: every official record
  follows the upstream member order. DIAL and SCEN live only in quest children groups
  (type 10), INFO in topic children groups (type 7). QUST stages, objectives and the
  three alias kinds are typed at `corpus` (W10 acceptance input for M4.D1); aliases
  interleave, so QUST stores runs (`Aliases`: reference, then location, then collection
  aliases per entry; lossless). Opaque by design: VMAD after its header, CTDA unions,
  component blocks (all of CHAL), SCEN action type-specific subrecords (union by action
  type, `any_until` the end marker), PACK input values/targets, INFO ENAM flag sets, QUST
  ALFG/ALAM/create-matching ALCA, unnamed slots (gaps rows). Mod-author guide:
  `docs/schema/quest-family.md`. `cargo xtask ci` passed locally with
  `KITKRAFT_DATA_DIR` and the derived files regenerated (the ignored official-files
  validate test needs BX's known-error update, see Requests). Derived files are left to the
  integrator (M4 family lane rule, BX): `crates/esm/src/generated/`, the
  `BUILTIN_DEFINITIONS` region of `crates/schema/src/lib.rs` (14 new entries: 10 records,
  4 common), `crates/schema/tests/golden/seed.rs` (`KITKRAFT_UPDATE_GOLDEN=1`) and the
  builtin-set lists in `crates/schema/tests/defs.rs`.
- M4.BX1 QUEST merge (integrator, BX3, 2026-10-14): the quest family's full CHAL, DIAL,
  FACT, INFO and QUST win over round 3's lstring-only stubs; every lstring the corpus
  table lists for them is a member except CHAL FULL/DESC, which official challenges carry
  only inside their one base form component (added to `COMPONENT_LSTRINGS`). Shared
  blocks folded: the family's `common/SoundReference` had the same 40-byte layout as round
  3's (one file, both citations). `common/Condition`: CIS1/CIS2 raised to `corpus` (17 and
  210 occurrences in QUST, PACK, SCEN, INFO) and the family's 102,280 CTDA added to the
  evidence. Derived files regenerated (generated code, `BUILTIN_DEFINITIONS`, golden,
  `tests/defs.rs` lists: 80 record definitions, 30 shared). `validate_corpus_official_files`:
  the 22 new Starfield.esm errors are `KNOWN_ERRORS` (SMEN `PNAM` = 0x5B x20, a reserved
  story manager root; `QuestReferenceAlias.ALFR` = 0x14 x2, the player reference class).
  Gates: `cargo xtask ci` with `KITKRAFT_DATA_DIR`; ignored release tests for esm, doc and
  engine all pass (17 min: 14/14 identical, typed check 0 conflicts, `world_family_full`,
  `corpus_rebuild_full`, doc `typed_corpus` 7,254,548 typed records and the 181 pinned
  unresolved lstrings). Parity: 80 record definitions,
  44,743,862 typed (1,712,200 failed, all the
  unordered CELL/REFR/ACHR placements, unchanged), 2,249,507 opaque, 3,426,192 unknown.
  `papyrus.compile`'s source extraction now retries its folder rename briefly (a fresh
  private cache under `.targets\` failed with "Access is denied" while a scanner held the
  new files).
- M4.BX1 position-dependent layouts (integrator, 2026-10-14): `kitkraft_doc::typed` now
  resolves a subrecord's layout by its position in the definition (`placements`,
  `layout_of`, `layout_at`, `path_layout`: the generated matcher's rules over the schema
  model, member paths from the definition), never by signature alone; inside an
  `any_until` body or where no member accepts it, the result is a typed `LayoutError`
  (`Raw`, `Unplaced`; `Ambiguous` for a `where` path over occurrences with different
  layouts, `CNAM[1]` picks one) and the payload is opaque. `diff`/`conflicts`
  (`fields::compare`), `where` (`subject`), `inspect`, and lane E's `edit::fields`
  (`layout_at`, `layouts`, replacing the signature-only `layout_for`) with its callers
  (`set`, FormID remap, `inline_lstrings`, `validate`) use it. Regression tests: PKIN VNAM
  inside a component (`pkin_vnam_inside_a_component_is_raw`), PNDT's duplicated
  CNAM/FNAM/GNAM (`pndt_duplicated_signatures_resolve_by_position`,
  `layouts_resolve_by_position`). `validate_corpus_official_files` keeps the zero-error
  rule but lists the 22 real dangling FormIDs as `KNOWN_ERRORS` with evidence and exact
  counts (Starfield.esm PKIN `PRPS.ActorValue` 0x33D x7 and 0x33E x14; SFBGS050.esm
  PNDT `EOVR.Worldspace` 0202B2F4 x1).
- M4.BX1 lane B round 3 merge (integrator, 2026-10-14): round 3's ten typed types and 49
  lstring-only stubs on top of the planet family. The planet family's full BIOM, GBFM,
  PKIN, PNDT, RSPJ and STDT win over round 3's stubs; every lstring round 3 listed for them
  is a member except the three official records carry only inside a base form component
  (GBFM HULL, PNDT FULL, STDT FULL: raw with the component data, typed by the engine's
  corpus-table fallback; `COMPONENT_LSTRINGS` in `crates/schema/tests/defs.rs`), and GBFM
  gets back its combination FULL (`reference`, xEdit L6961-L6974) now that layouts resolve
  by position. `common/Condition.toml`: one definition (the planet family's typed CTDA
  struct, round 3's COBJ evidence added), used by COBJ, FLST, LVLI, LVLP, PCBN, PCMT, RSPJ.
  `edit::fields` keeps the positional lookup with round 3's no-fallback expectation.
  `validate_corpus_official_files`: 23 new FLST `LNAM` errors in Starfield.esm, all object
  IDs 0x2D8..0x359 in five actor-value form lists (ENV_imGui_* and
  SpaceshipPartSummedValues), the PKIN PRPS class (engine-reserved actor values, grade D),
  listed as `KNOWN_ERRORS` with exact counts. Gates: `cargo xtask ci` with
  `KITKRAFT_DATA_DIR`, the ignored release corpus tests for esm, doc and engine (14/14
  identical, M2.A3 typed check 0 conflicts, doc `typed_corpus` 7,127,228 typed records and
  181 pinned unresolved lstrings); parity: 72 record definitions, 1,028,119 typed (0
  failed), 26,866 opaque, 49,364,576 unknown subrecord occurrences.
- **World schema family (parallel schema lane, 2026-10-14).** WRLD, CELL, REFR, ACHR,
  NAVM, LCTN and LCRT typed from the pinned xEdit files (`crates/schema/defs/`, nine new
  shared definitions in `defs/common/`), each member cited with its lines and `confidence`
  (`corpus` where the corpus has it, else `reference`). `crates/esm/tests/world_family.rs`
  checks every record of the seven types in the 14 files (6,880,278 records; REFR sampled
  1 in 20 in `cargo test`, all in `--ignored` release, 78 s): nothing placed fails to
  decode and every record re-encodes identically. CELL, REFR and ACHR are unordered in the
  files: 784, 1,814,375 and 11,288 subrecords fall outside the best linear member order and
  stay raw in `extra` (gaps row). The full run also asserts the cell/world group layout
  (types 1..9, block and subblock label rules); `docs/schema/world-family.md` has the
  counts. Other lanes' tests touched to stay green (each a few lines): `esm` bridge and
  driver unit tests (CELL/REFR/NAVM now defined), engine `schema` test (REFR -> LAND as the
  undefined type), schema `defs.rs` lists, the golden `seed.rs` (regenerate with
  `KITKRAFT_UPDATE_GOLDEN=1` after merging), and `doc` `typed_corpus.rs` now resolves a
  subrecord's layout by the generated matcher's placement (REFR has two `INAM` layouts,
  and BFCB bodies hold `PNAM`/`NAM*` of other sizes). `crates/esm/src/generated/` is not
  committed: run `cargo xtask codegen` after merging.
- M4.BX1 WORLD merge (integrator, 2026-10-14): the world family's full CELL, LCTN, REFR and
  WRLD win over round 3's lstring-only stubs (every lstring round 3 listed is a member);
  `typed_corpus.rs` keeps main's positional resolution (it supersedes the lane's
  `match_members` placement); `bridge`/`driver`/`schema` tests take "the type is defined"
  (LAND is the undefined type). Shared blocks folded: PositionRotation's position and
  rotation and ObjectBounds' corners are `ref:Vec3`, OwnerData's NoCrime is `ref:Bool8`;
  `common_blocks_are_not_duplicated` (`crates/schema/tests/defs.rs`) fails on two
  `common/` blocks with the same layout or one spelling out another shared type inline.
  `validate_corpus_official_files` explains the 86 new errors as `KNOWN_ERRORS`: object ID
  0x14 in PLYR-capable reference slots (EnableParent x6, XATR x4, XLKR x1, LCTN LCEP x2),
  REFR NAME 0x1FB x1 (Starfield.esm); REFR XPLK stored one file index above the plugin's
  own in SFBGS00D (4), SFBGS050 (4) and ShatteredSpace (30), every target present at the
  plugin's own index; SFBGS050 CELL XBPS snap links to 47 references the file lacks.
  Gates: `cargo xtask ci`, ignored release corpus tests (14/14 identical, typed check 0
  conflicts, `world_family_full` passes, doc `typed_corpus` 7,239,631 typed records);
  parity: 75 record definitions, 41,197,795 typed, 2,035,002 opaque, 7,186,764 unknown;
  **1,712,200 failed**, all subrecords the ordered matcher cannot place in unordered CELL
  (784), REFR (1,700,128) and ACHR (11,288) records (kept raw in `extra`, lossless; the
  gaps row), none a payload that failed to decode. They go to 0 with lane B's `unordered`
  record mode (Requests).
- M4.BX1 lane A M4 merge (integrator, 2026-10-14): `esm::cells`, the rebuild gate, the
  forms allocator, NAVM import, `refs::compose_reference`, `upsert_styled`; `bridge` keeps
  one implementation (lane A's: `check_subrecords` reads `Record::extra()`, no `extra_of`)
  with its all-types test. With the world family on the branch, `compose_reference` takes
  REFR/ACHR `DATA` Z and rotation at `corpus` (gaps row updated; flag 0x800 still needs
  `allow_reference`). Ignored release tests: `corpus_rebuild_full` 0 differences on all 14
  files in both orders (the 12 official-pipeline files whole and byte-identical;
  BlueprintShips-Starfield 65 and BlueprintShips-SFBGS050 4 listed normalizations),
  roundtrip 14/14, typed check 0 conflicts; parity unchanged.
- M4.BX1 later merges (integrator, 2026-10-14): the policy docs (ADR 0007 amendment, 0008,
  0009), lane F (bare-name resolver, `roundtrip --decode`), main (owner's brief, live-game
  rules), lane E M4.E11 (`deploy`), lane G M4 (`.psc` reader, `papyrus.events`,
  `package`). `docs/source-registry.md`: the policy branch's COMMONLIB/SFSE rows, lane G's
  PAPYRUS/OPEN-PAPYRUS rows. End-to-end on a synthetic MO2 instance in a temp dir (`set`
  rename -> `package` -> `deploy --dev` -> redeploy -> `deploy.rollback` ->
  `deploy.status`): the package manifest verified, rollback restored the first plugin byte
  for byte, profiles and `ModOrganizer.ini` untouched; one mismatch fixed (`package`
  writes `plugins` as `{file, ...}` objects, which `deploy` now reads). ADR 0009
  Amendment 1 accepts the five departures `docs/deploy.md` lists.
- `deploy --dev` / `package` fix round after a Fable review (2026-10-14): the `Default`
  gate fails closed (UTF-8 BOM stripped in `Ini::parse`; no `selected_profile` refuses
  `live-profile-unknown` unless `--profile` names an existing non-Default profile;
  diagnostics read only an accepted `profile`); `patch::absolute` resolves through the
  deepest existing ancestor and `guard_location` refuses any link or junction (`set`,
  `build`, `package`); `.previous`/`.retired` are dropped only when they match their
  manifest (`previous-modified`); `mods\` is checked against the user Data folder and may
  not be overwrite, downloads or profiles; `deploy.status` reports one odd folder as
  `state: "error"`; a `<target>.kitkraft-lock` (`create_new`) serializes writers and
  guards recovery; path length is measured under `.previous` and the longest temp name;
  rollback copies `meta.ini` after the swap and recovery removes our `.tmp` names;
  `package` writes archive entries' `source_sha256`, replaces by renames only, and shares
  the 20,000-file limit with `deploy`. `docs/deploy.md` gains operating notes.
- **M4.E11 `deploy --dev` (lane E, ADR 0009, 2026-10-14).** Commands `deploy` (CLI
  `kit deploy --dev <source> <mo2>`), `deploy.rollback`, `deploy.status` in
  `crates/engine/src/deploy/` and `handlers/deploy.rs`; `docs/deploy.md` has the shapes,
  layout, algorithm, refusal rules and a read-only `--dry-run` against the owner's
  instance (refused: MO2's active profile is `Default`; nothing written). A plugin or
  package folder goes into `<mods>\KitKraft Dev - <stem>\` only: `kitkraft-deploy.json`
  (marker, SHA-256 and source of every file, time, previous id), staged in
  `<target>.staging`, swapped in by folder renames, the replaced one kept as
  `<target>.previous` for rollback, a journal beside them recovered on the next call;
  refuses `Default` (active or `--profile`), a same-named folder without the marker, hand
  edits, links/junctions; never enables the mod (the report lists the MO2 steps). Tests on
  a synthetic MO2 instance (14): first deploy, redeploy, rollback and its undo, Default
  refusal, collisions, interrupted deploy at four points and interrupted rollback
  recovered, hand edits, a junction, package-manifest verification, profile files,
  `ModOrganizer.ini`, an unrelated mod, `overwrite\` and `downloads\` byte-identical.
  Differences from ADR 0009's text (per the owner's brief) are listed in
  `docs/deploy.md`; the ADR is on an unmerged branch and needs an amendment.
- Lane E, M4 (set + validate + build, 2026-10-14):
  - M3.E1 (carried from M3): `set` and `build` no longer refuse a whole load order. Each
    copied form goes through lane D's `Loaded::source_check` (`handlers/set.rs`
    `safe_source`): a blocking problem (a holder of a version loading before its master, a
    later plugin that failed or has unresolved records that could be the form, a missing or
    deleted winner) refuses that form; every other problem (an active plugin with no file,
    an unrelated ordering error) is a limitation. Tests through `execute`: a synthetic MO2
    profile with an active plugin that has no file writes an unrelated form with a
    limitation and refuses a form a plugin needing the missing one may override.
  - M3.E2: lstrings inlined from string tables are the table's bytes (`Resolved::bytes`),
    so the few non-UTF-8 official entries are copied exactly instead of refused;
    `record.flags-body` accepts any one NUL-terminated string. The `docs/gaps.md` row is
    closed.
  - M4.E2 `place` (`Implemented`; `handlers/place.rs`, writer `edit::patch::write_patch_with`;
    CLI `kit place <base> --position "[x,y,z]" <into> --cell <CELL>` or `--world <WRLD>`):
    a new `REFR` (`ACHR` for an `NPC_` base) composed by lane A's `compose_reference`, a
    FormID from `NewForms` once the master list is final, `HEDR` next object ID advanced;
    the winning `CELL` (and `WRLD`) carried as an unchanged copy before the children groups
    (a cell the target already has is kept where it is, and the reference goes under its
    observed path); `world` + position picks the exterior cell by `XCLC` grid, and a
    persistent reference of a world goes into the world's persistent cell. Typed refusals
    (`place.<code>:`): base not placeable (references, cells, worlds, navmeshes; `LVLN`
    until checked), no cell or world, cell and world both, no exterior cell at the grid,
    a temporary reference in a persistent cell, EditorID invalid or clashing (load order,
    target, or twice), a small or medium master (`place.scaled-master`, per lane A's review
    round), a parent not carried, a FormID already in the target; a position outside the
    named exterior cell is a limitation. Evidence: base and cell winners, every written
    record; `placed[]` lists base, cell, world, grid, group path and each member with its
    confidence; `carried[]` says whether each carried record equals its winner byte for
    byte. Tests through `execute` (synthetic master: interior, persistent and exterior
    cells; ACHR; observed path; determinism and the CLI form; every refusal) and
    `tests/place_corpus.rs` (ignored, release, 18.6 s): 8 interior and 8 exterior
    `Starfield.esm` references with only `NAME`+`DATA` placed again from their base, cell
    or world, position and rotation reproduce their record bodies byte for byte
    (header: new FormID, composition-default stamps).
  - M4.E3 `build` spec version 3 (`build/spec.rs`, `handlers/build.rs`): `[[place]]`
    entries (as `kit place`, with an optional spec-local `name`; positions and rotations as
    three finite numbers), `[[import]]` (every `NAVM` of a Creation Kit plugin copied
    unchanged by lane A's `import_navmeshes`; refused unless the master lists match; `ONAM`
    redone and every written record located again), class rules over typed world records
    (a rule over `REFR` writes overrides with their cells carried). The writer now writes
    `TES4` `ONAM` by lane A's M4.A6 rule (overrides of master records in groups 7, 9, 10, in
    file order; F23 open) for every `set`, `build` and `place`. Each cell a place needs is
    carried once, and not when a rule or set writes it. `plan` lists the new forms with
    their FormIDs; versions 1 and 2 build as before. `[[new]]` and `[[clone]]` landed with
    M4.E1 (below); a `new:` name of a placed reference is refused, since a placed
    reference is never a base, cell or world. Tests through
    `execute` (`handlers/build_v3_tests.rs`): two builds byte-identical, the plan, rules over
    `REFR` with `ONAM`, an import (and its master refusal), and an error per case (unknown
    name, a cycle, a placed reference as a base, duplicate names and EditorIDs, positions,
    a `reference`-only member without `allow_reference`, two writes to one field,
    `[[new]]`, version 2 keys).
  - M4.E4 validate cell rules (`validate/cells.rs`; rows in `docs/validate.md`):
    `cell.missing-parent` (a cell child outside cell children, or a WRLD/CELL children
    group whose record nobody has: error; a master's record not carried: warning),
    `ref.orphan` (a REFR/ACHR base that resolves nowhere, or no NAME: error),
    `ref.outside-block` (a cell in the wrong block/sub-block for its object ID or XCLC grid:
    error, warning in small/medium plugins; an exterior reference outside its cell's
    square: warning), `ref.persistent-flag` (flag 0x400 against group type 8/9, on a NAVM,
    on a CELL that is not a world's persistent cell: error), `ref.persistence` (a temporary
    reference a quest, scene, package, dialogue or story-manager record or a door link
    names: warning, grade D), `cell.actor-without-navmesh` (warning; masters scanned by
    group headers only), `cell.incc` (error; missing INCC with interior cells: warning),
    `navmesh.unchanged` (a NAVM override equal to its master's: warning). Positive and
    negative synthetic fixtures per rule (`validate/cells_tests.rs`).
    `validate_corpus_official_files` (release, 279 s): 0 unexplained errors; one new known
    error (`ref.orphan` on `Starfield.esm` `REFR:00014893`, base 0x1FB, the same record
    `formid.dangling` lists). New warnings by kind, with their reasons:
    `ref.outside-block` REFR (Starfield.esm 4, SFBGS00D 57, SFBGS050 217, ShatteredSpace
    76: exterior references outside their cell's square, the corpus's 0.02 %);
    `navmesh.unchanged` NAVM (SFBGS003 301: navmesh overrides byte-equal to Starfield.esm's,
    an official-pipeline habit, so a warning); `ref.persistence` REFR (SFBGS00D 5: door
    links, `XTEL`, to temporary references). Corpus finding that shaped `ref.persistence`:
    official files name temporary references from `WRLD` `RNAM` and `LCTN` `LCSR`/`LCPR`
    (48,538 in Starfield.esm), so those referrers are not reported.
  - M4.E6 `testplan` (`Implemented`; `handlers/testplan.rs`; CLI `kit testplan <plugin>
    <out> --profile <dir>`): writes only `out/kitkraft-test.txt` (CRLF console commands:
    `coc <cell EditorID>`, `player.moveto`/`prid <runtime ID>` per new reference,
    `player.additem <runtime ID> 1` per new item, `coc`+`prid` per reference override) and
    `out/checklist.md` (setup on the MO2 profile `KitKraft Dev` from a disposable save, the
    plugin's SHA-256, a step per form with the expected observation, persistence steps,
    the `docs/game-observations.md` ledger entry, the removal rule for plugins that own
    forms). Runtime IDs from lane D's `LoadOrder::runtime_id` with the plugin appended (a
    master-flagged plugin after the last master-flagged one): assumptions
    `runtime-slots-by-scale` and `masters-load-first` and the batch folder (grade D) are
    limitations. Deterministic; `out` guarded (never game/user Data or MO2). Tests through
    `execute` (`handlers/testplan_tests.rs`).
  - M4.E1 `new` and `clone` (`Implemented`; `handlers/newform.rs`; CLI `kit new <type>
    <edid> <into> [--fields JSON] [--masters ...]`, `kit clone <form> <edid> <into>
    [--fields JSON]`): a new base form of a typed top-level type (`EDID`, the schema's
    required top-level members as zeros or an empty string, each field's subrecord created
    in schema order then written as `set` writes it), or a copy of a top-level winner with
    a new FormID and EditorID (lstrings inlined, FormIDs as stored, remapped or refused as
    the patch writer does). FormIDs from lane A's `NewForms` after the master list is final
    (`write_patch` `Copy::new_form`); full-scale targets and masters only; EditorID clashes,
    nested types (references: `place`; cells, worlds, quest children), untyped types,
    `EDID` in fields and repeated occurrences are typed refusals (`new.<code>`,
    `clone.<code>`). Spec v3 `[[new]]` and `[[clone]]` (names; a `[[place]]` base may be
    `new:<name>`; a clone of a form of the same build and a new cell or world are refused).
    Tests through `execute` (`handlers/newform_tests.rs`, `build_v3_tests.rs`). **W1
    deliverable** on the official file: `kit new GLOB KitKraftTestGlobal` (FLTV 1.0) and
    `kit clone Starfield.esm:1E4FBD KitKraftCrateClone` into `out\w1\KitKraftW1.esm`
    (430 bytes, SHA-256 `5d122dd3c93a0fecdbe8cde6da116f5f9c710d45d49d07a559ae51e68c2c33ae`):
    FormIDs `01001000`, `01001001`, `HEDR` next object ID `FF001002`; validate 0 findings,
    roundtrip identical, `inspect` finds the clone. Whether they equal the CK's choice is
    fixture F06's question (not observed; named in `limitations`).
  - M4.E8 recipes `docs/recipes/ownership.md` + `ownership.toml` (W4: `XOWN.Owner` of
    `CityNewAtlantisFreestarEmbassy` set to `PlayerFaction`; 15,053 bytes, SHA-256
    `cc77ed38...cc259`) and `docs/recipes/actors.md` + `actors.toml` (W9: an `ACHR` of
    `Vendor_ChunksEmployee_M` in the embassy, which has a `NAVM`; 15,190 bytes, SHA-256
    `837b6371...8f27a`): each builds, validates with 0 findings and roundtrips identical;
    every fact cites its source. `cell.actor-without-navmesh` caught the first choice of
    cell (the POI's apartment has no `NAVM`). Finding: `query` by owner cannot decide 65
    `CELL`s (`XOWN` out of the ordered definition's place; lane B's `unordered` mode).
    The ownership cell is a stand-in until M4.H1 (F27). In-game steps for M4.O4 in each
    recipe (KitKraft Dev, disposable save).
  - Not started, gated: M4.E9 (script attachment) waits for BQ's typed script-attachment
    block (`VMAD` is opaque in every definition); M4.E10 (procedural placement) waits for
    M4.D3 (`planet`, not on main).
  - **First POI (rung 0) built** (`docs/poi/first-poi.md`, spec `docs/poi/first-poi.toml`):
    `out\poi\KitKraftFirstPOI.esm` (1,821 bytes, SHA-256
    `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1`, master
    `Starfield.esm` only): one persistent `REFR` `KitKraftFirstPOICrate` (`01001000`) of
    `Crate_MilitarySupply_01` (`Starfield.esm:1E4FBD`) in the interior cell
    `CityNewAtlantisApartmentTEMPLATE` (`Starfield.esm:21966F`, one version on Default).
    Two builds identical; `validate` 0 errors 0 warnings (14 rules); `roundtrip --verify`
    identical; `inspect` with the plugin appended finds it; `testplan` against the Default
    profile (read only) gives runtime ID `28001000`; `package` validated it into
    `out\poi\package`. **Owner (M4.O2):** on **KitKraft Dev** with a disposable save,
    `coc CityNewAtlantisApartmentTEMPLATE`, then `player.moveto 28001000` (or
    `player.moveto KitKraftFirstPOICrate`), `prid` it, screenshot, and run the persistence
    steps in `docs/poi/first-poi.md`.
- Lane G, M4 (archives, scripts, strings; 2026-10-14):
  - M4.G0: ADR 0006 Amendment 1: `.psc` declaration facts come from the official sources
    in `Tools\ContentResources.zip` read in place (level 1), the compiler as a black box
    (level 2) and the CK wiki for semantics (level 4); the Papyrus Index only cross-checks;
    no other Papyrus parser, compiler or grammar is read; source comments are shown only on
    request and never cached or quoted. Registry rows PAPYRUS, CORPUS-ARCHIVES and
    OPEN-PAPYRUS updated.
  - M4.G1: `kitkraft_pex::psc::parse` reads a `.psc`'s declarations (header with parent
    and flags; functions and events with typed parameters, defaults, return types, flags
    and state; remote event handlers; properties with groups, `RequiresGuard(...)` and
    full Get/Set forms; structs and members; variables; imports, custom events, guards,
    states, groups); bodies skipped; a statement it does not understand is a diagnostic
    with its line, a structural fault a typed `PscError` with its line. Synthetic fixtures
    per construct, truncation and noise never panic. Corpus (`every_official_source_parses`,
    the zip read in memory, 1.5 s debug): all 5,033 `.psc` files parse with 0 errors and
    0 unread statements: 21,175 functions (1,223 native), 3,567 events (515 remote
    handlers), 27,000 properties, 167 structs, 58 custom events.
  - M4.G2: `papyrus.events` (`Implemented`; CLI `kit papyrus events --script
    ObjectReference`, `--event OnLoad --declarations-only`, `--search outpost`; MCP via
    `execute`/`command_line`): the zip read in place, index cached by its central
    directory (no comments in the cache), parent chains resolved, overrides and remote
    handlers marked, `with_comments` reads doc/comment from the user's zip at query time,
    `file:line` plus byte-range evidence. Counts per kind on the official zip (5,033
    scripts): event 3,567, function 21,175, property 27,000, struct 167, custom_event 58;
    `OnLoad` is declared (not overridden) in ActiveMagicEffect, ObjectReference,
    RefCollectionAlias and ReferenceAlias; ObjectReference's chain is Form, ScriptObject;
    `--search outpost` 491 matches. Tests: synthetic zip (chain, overrides, remote
    handlers, search, paging, comments only on request, cache hit/miss/corrupt, errors,
    command-line forms) and `official_sources`. **Open:** the acceptance check that the
    events `docs/design/persistence.md` cites exist waits for that file (M4.H3, lane H; not
    on main): `kit papyrus events --event <name> --declarations-only` is the check.
  - M4.G3 (scoping): `docs/research/asset-formats.md`: `ba2.list` over all 90 official
    archives (1,763,906 entries, 138.5 GiB; 65 GNRL v2, 10 DX10 v2, 15 DX10 v3 LZ4) by
    extension (38 extensions) and by archive group, with what KitKraft does per format,
    the permitted source if any (only DDS headers, string tables and `.pex` have one) and
    the workstream that would author it. Finding: no `.mat` file exists in any official
    archive; materials ship as a `.cdb` component database. No implementation.
  - M4.G4 (research): `docs/corpus.md` "Archive names": all 90 archives are `<stem> -
    <suffix>.ba2`; the 50 base-game archives (and `BlueprintShips-Starfield -
    Localization`) are named in `Starfield.ini` lists; the other 38 belong to 11 official
    plugins by name only (suffixes `Main`, `Main01/02`, `Textures`, `Localization`,
    `Voices_<lang>`). Rule for `package`: `<plugin stem> - Main.ba2` (GNRL) and `<plugin
    stem> - Textures.ba2` (DX10). Grade C until M4.O2 sees a mod's `- Main.ba2` load.
  - `package` (the plan's M4.E5 shape, assigned to lane G for M4; `Implemented`; CLI `kit
    package <plugin> <out>`; MCP via `execute`/`command_line`): `validate` first (a failing
    plugin is refused, nothing written); `<out>\<name>\` with the plugin, loose files and
    scripts (`Scripts\<namespace>\<name>.pex` from the name inside each `.pex`) at their
    Data paths or packed per M4.G4; `kitkraft-manifest.json` (SHA-256 and size of every
    file, masters, record counts per type, KitKraft version, spec SHA-256, validate
    result, archive contents); a deterministic zip; only `out` is written, and an `out` in
    Data, the user Data dir or MO2 (its `mods\` included) is refused. Deviation from the
    plan's arguments: one `plugin` (not `plugins`), since `plugins` names the load order
    in every other command. Tests: two runs byte-identical (folder and zip), the manifest
    verifies after a copy, ba2 names and contents, validate refusal, every guard,
    command-line form. Real runs 2026-10-14: the rename plugin packaged (manifest SHA-256
    equals `docs/smoke-rename.md`); `black-box: Archive2.exe 1.2.0.1` extracts a `--ba2`
    package's two archives with the same file list (general files identical, texture
    pixels identical under Archive2's own DDS header). Install steps and the manifest check
    in `docs/packaging.md`. **Owner (M4.O2):** install the rename package through MO2 on
    `KitKraft Dev` and check the hashes on disk against the manifest.
- M4.BC1 ACTOR schema family (family lane BC, `crates/schema/defs`, 2026-10-14): NPC_,
  LVLN, OTFT, CLAS, AFFE, RACE, PERK, SPEL, MGEF transcribed from the pinned xEdit files
  (one commit per type, every member cited with its line range); new shared `common/`
  definitions `SoundReference`, `AttackData`, `Effect`. Corpus check (14 files, generated
  decoder regenerated in the worktree): all 13,350 records of the 9 types (NPC_ 8,447, all
  compressed; LVLN 298; OTFT 698; CLAS 8; AFFE 1,593; RACE 47; PERK 608; SPEL 662; MGEF
  989) decode with every subrecord placed, nothing in `extra`, and re-encode
  byte-identically; `corpus_full_roundtrip` (release) 14/14 identical, 0 typed conflicts;
  doc `typed_corpus` all files passes (60,063 typed records, the 181 pinned unresolved
  lstrings as before). Confidence `corpus`, members never seen in an official file
  `reference`. Opaque by design: VMAD, component blocks, NPC_ ACBS level union, NAM5, OBTS,
  ONA2 tail, RACE DAT2 unnamed words, PERK effect DATA/EPFD unions, ATAV, MGEF archetype
  slots (gaps rows in `docs/schema/actor-family.md`, the mod-author guide). `cargo xtask
  ci` green in the worktree with the generated files regenerated; generated output, the
  `BUILTIN_DEFINITIONS` region, the golden `seed.rs` and the `tests/defs.rs` lists are left
  to the integrator (9 records, 3 common files to add).
- M4.BX1 ACTOR merge (integrator, BX3, 2026-10-14): the actor family's full CLAS, MGEF,
  NPC_, PERK, RACE and SPEL win over round 3's lstring-only stubs (every corpus-table
  lstring is a member). Shared blocks folded: the family's `SoundReference` (an event-set
  struct plus the start form, the same 40 bytes) into the existing flat one, LVLN's and
  LVLP's inline model groups into `common/Model` (same members and cardinalities), NPC_'s
  opaque destructible block into `common/Destructible` (absent from the official NPC_);
  `common_blocks_are_not_duplicated` now also checks that those records reference the
  shared blocks. Derived files regenerated (83 record definitions, 32 shared).
  **Engine-reserved object IDs** (ADR 0005 Amendment 4, `docs/validate.md`): instead of
  listing the 318 new `PRPS.ActorValue` errors, `formid.dangling` no longer reports a typed
  member naming an object ID below 0x800 in `Starfield.esm`'s index space that it does not
  define; it counts them (`reserved_form_ids`, `reserved_by_member`, a limitation). Every
  earlier `< 0x800` known error (PKIN/FLST actor values, the player reference 0x14, REFR
  NAME 0x1FB, SMEN PNAM 0x5B, quest alias ALFR 0x14) left `KNOWN_ERRORS`; the corpus test
  pins the counts per plugin and member path instead (`RESERVED`: 738 in all, 397 in
  Starfield.esm: NPC_ PRPS 139, RACE PRPS 178, FLST LNAM 23, PKIN PRPS 21, SMEN PNAM 20,
  ...); the DLC warnings of the same class are gone. The one other new error,
  `NPC_:0018B5F4` `RDSA.SpecificReference = 00577823` (a stale value its flags do not
  enable), is a `KNOWN_ERRORS` entry. Synthetic test `formid_dangling_skips_engine_reserved_ids`.
  Gates: `cargo xtask ci`; ignored release tests for esm, doc and engine (31 min; doc
  `typed_corpus` 7,257,137 typed records, 181 pinned unresolved lstrings). Parity: 83 record
  definitions, 45,923,589 typed (1,712,200 failed, the unordered CELL/REFR/ACHR placements),
  2,262,151 opaque, 2,233,821 unknown.
- M4.BI1 ITEMS/WORKSHOP schema family (parallel schema lane, `crates/schema/defs`,
  2026-10-14): STAT, MISC, CONT, ACTI, FURN, MSTT, TERM, TMLM, ARMO, IDLE, ANIO, CPTH
  transcribed from the pinned xEdit files (staged copies verified first: sizes and blob
  hashes match `docs/xedit-reference.md`; one commit per type, every member cited with its
  line range). New shared `common/` definitions: Model (first owner, as planned),
  Destructible, ContainerItems, ObjectTemplate, SoundReference, ObjectProperty, ColorRgb,
  RadioReceiver, FurnitureMarkerFlags, FurnitureMarkerParameter, FurnitureEntryPoint
  (ObjectBounds already existed from the planet family and is reused). Corpus check (14
  files, out-of-tree checker over the generated decoder): all 33,939 records of the 12
  types (STAT 22,296, MSTT 2,134, ACTI 1,940, MISC 1,495, IDLE 1,302, ARMO 1,168, FURN
  1,045, TMLM 974, CONT 866, TERM 501, CPTH 152, ANIO 66; none compressed) decode with
  every subrecord placed, nothing in `extra`, and re-encode byte-identically; confidence
  `corpus`, members never seen `reference`. Opaque by design: VMAD, component blocks,
  NVNM, CTDA unions, COED's middle slot, OBTS, FURN NNAM, TERM WBDT, TMLM padding (gaps
  rows in the guide). Lstring members equal the corpus lstring table for these types.
  Mod-author guide with the outpost build menu read from the corpus (recipe categories,
  beacon keywords, created-object traits, what a new buildable needs):
  `docs/schema/items-workshop-family.md`. One small change outside the lane, in its own
  commit: two lane E unit tests used ACTI ATTX as their example of an lstring the schema
  does not type; now typed, the examples move to FLOR ATTX (Requests). Derived files left
  to the integrator (family lane rule): `crates/esm/src/generated/`, the
  `BUILTIN_DEFINITIONS` region (12 records, 11 shared files), `crates/schema/tests/golden/
  seed.rs` and the builtin lists in `crates/schema/tests/defs.rs`; with them regenerated
  locally `cargo xtask ci` passed (with `KITKRAFT_DATA_DIR`).
- M4.BX1 ITEMS merge (integrator, BX3, 2026-10-14): the items/workshop family's full ACTI,
  CONT, FURN, IDLE, MISC, MSTT, STAT, TERM and TMLM win over round 3's lstring-only stubs
  (every corpus-table lstring is a member); lane E's two tests keep main's ACTI ATTX example
  in `edit::fields` (typed, no fallback) and take the lane's FLOR ATTX one in `patch_tests`.
  Shared blocks folded (one definition each, every citation kept, every reference
  updated): the family's `Model` into main's (MOLM becomes `repeated`: one STAT has two),
  `Destructible`, `SoundReference` and `ColorRgb` into main's; its `ObjectProperty` into the
  world family's identical `PropertyValue`, which PKIN, NPC_, RACE and CLAS PRPS now use too
  (they spelled the element out inline); its `ObjectTemplate` and the inline object
  templates of NPC_ and GBFM into round 3's `ObteBlock` (FULL made optional: no official
  NPC_ or GBFM combination has one; ARMO, CONT and TERM drop their own STOP for the block's;
  GBFM keeps its second STOP), so OBTS is typed in ARMO, NPC_ and GBFM with 0 new decode
  failures; NPC_'s inline container items and the quest alias inventory into the family's
  `ContainerItems` (the base-object target list from the actor family);
  `RadioReceiver` uses `ref:Bool8`. 86 record definitions, 37 shared;
  `common_blocks_are_not_duplicated` checks every fold. Engine-reserved IDs: the 646 new
  Starfield.esm PRPS actor values (ACTI 128, CONT 443, FURN 13, STAT 61, TERM 1) are counted,
  not reported; `reserved_by_member` is keyed by record type and member path (shared types
  share a path), and `RESERVED` pins 1,410 occurrences in 7 files (Starfield.esm 1,043).
  Gates: `cargo xtask ci`; ignored release tests for esm, doc and engine (17 min; doc `typed_corpus` 7,258,523 typed records, 181 pinned unresolved lstrings); parity:
  86 record definitions, 46,249,514 typed (1,712,200 failed, the unordered CELL/REFR/ACHR
  placements only), 2,261,438 opaque, 1,908,609 unknown.
- Lane E, M3.E9 (first real overhaul build), 2026-10-14: `docs/overhaul/overhaul.toml`
  translates the owner's brief (B1..B14) into 16 classes and 16 rules, each naming its
  row; targets resolved by EditorID/FULL on the `KitKraft Dev` profile (14 WEAP, 4 AMDL,
  3 AMMO, 2 ammo LVLI, 10 OMOD; FormIDs in the spec header). `kit build --plan` now prints
  every change with its brief row, rule, before and after (and each JSON change has
  `brief`); `docs/overhaul/build-1-plan.md` is that output: 42 field changes on 17
  overrides (13 WEAP, 4 AMDL). `out\overhaul\KitKraftGunOverhaul.esm` (43,955 bytes,
  SHA-256 `a44d49bb...4a95342aae`; masters `Starfield.esm`, `OldSovietMarksmanRifle.esm`,
  the rifle's winner in `KitKraft Dev`): validate 0 errors 0 warnings, `roundtrip
  --verify` identical, packaged (`out\overhaul\package\KitKraftGunOverhaul-0.1.0.zip`),
  `deploy --dev --dry-run` refused only by the live-profile guard (MO2's active profile is
  `Default`). Rows: B1, B2, B4, B5, B7, B9, B13, B14 implemented; B3, B6, B11, B12 partial;
  B8 open (the automatic receivers carry no damage property: owner to restate), B10 open
  (OMOD value). The open parts are OMOD property values (`WSPS` on default receivers,
  `ACPT` on magazines), opaque until lane B types `ObjectModProperty.Value1` (Requests).
  `crates/engine/tests/overhaul_build.rs` builds the spec on a fixture of the 33 real
  records (copied from `Starfield.esm` into a temp localized master with the English
  tables; skipped without `KITKRAFT_DATA_DIR`) and checks all 42 changes. Report, owner
  steps and checklist: `docs/overhaul/build-1.md`. **Owner (M3.O5):** the in-game check
  there, on `KitKraft Dev` with a disposable save.
- **KitKraft identifier rename done** (M4.BX1 integration, 2026-10-14; owner decision 1):
  packages `kitkraft-*`, crate paths `kitkraft_*` (every `use`, Cargo dependency, xtask
  `LAYER_TABLE` and layer rule), env vars `KITKRAFT_*`, MCP server name `kitkraft` and
  resource URIs `kitkraft://`, the index cache `%LOCALAPPDATA%\kitkraft\cache` (not
  migrated: rebuilt on first use), the package manifest `kitkraft-manifest.json`,
  `.mcp.json.example`, `.githooks`, docs and plan prose, path mentions
  `C:\Modding\kitkraft` (the director moves the folder after merge), generated code and
  goldens regenerated. Deprecated for one release: the old `KITCRAFT_*` env vars are still
  read when the new one is unset (engine and xtask `env_var_os`, the doc cache, the corpus
  tests), and `deploy` still reads a package's `kitcraft-manifest.json` (`package`
  `overwrite` still recognizes a folder holding one); see `docs/corpus.md`, "Environment".
  The binary stays `kit`; commit history and the remote are unchanged.
- Lane B round 4 (2026-10-15): struct-field unions decided by a sibling field (and an
  optional secondary sibling), applied to `common/ObjectModProperty.toml`: OMOD and OBTS
  property values are typed (8,669 entries, 0 unknown value types, byte-identical; member
  paths `DATA.Properties[n].Value1.Float` etc. for lane E). Unordered-record ambiguity lint;
  COBJ FULL `reference` plus a parity check that `corpus` members occur; coverage counts
  confidence by the placed member (reference decoded occurrences 45,265 -> 1,527); PropertyValue
  `since = 152`, REFR XLOC 9-byte form, INFO TRDA Emotion as a FormID with 0xFFFFFFFF as none;
  reserved object IDs pinned per member, with a warning outside the named set (one: SFBGS00D
  OMOD 0x3B7). Open: TES4 DATA/INTV (docs/gaps.md), GBFM STOP Marker2 is live (not dropped).
- Lane B round 5 (2026-10-15): ADR 0005 Amendment 5 (`container-reference` on TES4 DATA and
  INTV, no parity exemption list); M3.B6 (KYWD and GMST XALG typed from the pinned xEdit
  file, KYWD FNAM and VMAD opaque); a per-family table at the top of
  `docs/record-parity.md`; codegen leaves line layout to rustfmt (goldens and generated code
  byte-identical); X8, non-UTF-8 strings shown as Windows-1252. Gates: `cargo xtask ci`,
  `codegen --check`, `parity --check`, and the ignored release corpus tests of esm, doc and
  engine (roundtrip identical, typed corpus, `validate_corpus_official_files`) all pass.
- **M3.A5 structure-aware mutation (lane A, 2026-10-15).** `crates/esm/tests/common/mutate.rs`
  walks the parsed tree and mutates record/group/subrecord sizes, `XXXX` fields,
  decompressed-size prefixes, flags (compressed, deleted), form versions, group labels and
  types and the `HEDR` count, plus size-consistent splices (duplicated or dropped subrecords
  and entries, 1-70 extra group levels, recompressed bodies, a subrecord bomb), seeded
  xorshift64*. Every input goes through parse, decode, plain, strict-decode and typed
  roundtrips and the index; errors must be typed, in the input, not before the mutated
  entry, never `Internal`; parseable input must roundtrip byte-identically. `cargo test`
  runs 400 per fixture and 100 per small file; the `#[ignore]`d full run does 2,000 per
  input (the five `SMALL` files and two fixtures, as the lane brief set; the plan row's
  100,000 per file ran once with `KITKRAFT_MUTATIONS=100000`: 700,000 inputs, 24.7 min,
  clean, recorded in `docs/corpus.md`). No panic or
  allocation blow-up found, so no parser change; results in `docs/corpus.md`, "Hostile
  input"; `docs/gaps.md` fuzzing row restated (cargo-fuzz still needs nightly).
- **Lane W, human QA wave (2026-10-15).** The workbench now carries the owner's in-game
  QA: a **Start** tab (shown when `kit ui` has no arguments) lists the MO2 instance, its
  profiles (`Default` marked "owner's live game, read-only") and the Data dir, from a new
  read-only engine command `profiles` (stopgap detection until lane F's `doctor`), with one
  load button per profile and the last choice kept in `%LOCALAPPDATA%\kitkraft\ui.json`
  (UI prefs, the only file the window writes on its own); a **Help** tab renders the
  owner next actions, overhaul build 2's in-game steps and checklist C1..C14 (copyable
  `player.additem` lines), the first POI's steps and the persistence protocol from the
  repo docs at runtime; a **Ledger** tab records a result through the new engine command
  `ledger` (`writes: true`; appends a dated row to `docs/game-observations.md` only,
  refuses any other path and `Default`), the one permitted write, confirmed per click
  (ADR 0007 Amendment 2; `gate::PERMITTED_WRITE`, `Request::confirmed`). Screenshot tour
  re-run on `KitKraft Dev` with the new tabs (images outside the repo);
  `docs/workbench.md` and `docs/mcp.md` updated.
- **Lane W, M4.W1..W6 (read-only workbench, 2026-10-15; the owner started it the same
  day).**
  - What exists:
    - `crates/ui-egui` (L5). Its only workspace dependency is `kitkraft-engine`, which the
      new xtask `INTERNAL_ONLY` rule enforces (ADR 0002).
    - `apps/kit`, which builds the `kit-ui` binary.
    - `kit ui` in kit-cli, a launcher like `kit mcp` with no `CommandSpec`.
  - `gate.rs` is the only holder of the engine. Its allowlist is `load`, `inspect`,
    `query`, `diff`, `conflicts`, `validate`, `coverage` and `schema`, plus `roundtrip`
    with `verify` and without `out`. It refuses every other command and alias. Tests check
    the catalog classification, try every refusal, and scan the crate's sources.
  - The engine runs on a worker thread. The panel view-models have no egui types and are
    tested against canned envelopes. Every result shows its `kit` line and MCP call, with
    copy buttons; the line is checked by parsing it back with the engine's parser. Every
    result also shows its limitations, coverage and evidence.
  - Checked read-only on KitKraft Dev with the `dev_profile` test and the screenshot tour
    (`docs/workbench.md`).
  - The dependency list is in ADR 0007. eframe 0.36 is built with `glow` only. No fonts
    are compiled in; the window uses the system fonts.
  - Carried:
    - `build --plan`.
    - `refs` and placement context (M4.D1, M4.D2).
    - The M4.W0 shape tests (lane F).
    - The owner's licence ruling (row 7).
- **Lanes F and W follow-ups, 2026-10-15.**
  - **M4.W0 data shapes (lane F).** `kitkraft_engine::shapes`: a documented shape for the
    `data` of `load`, `inspect`, `query`, `diff`, `conflicts`, `refs`, `planet`,
    `coverage`, `schema` and `validate` (every mode; strict: an undocumented member fails),
    printed to `docs/shapes.md` by `write_shape_docs` (a test fails when the file and the
    code differ). Explicit markers (`kind`): `formref` on every stored FormID value
    (`inspect` values, `diff`/`conflicts`/`vs_previous` fields, `planet` values, `refs`
    `placed.cell`/`world`), `lstring`, `opaque` (`inspect`'s opaque members were the
    string `"opaque"`, now a byte range with the file offset), `link` (`refs` outgoing
    edges, `planet` links), `referrer` (incoming edges), `evidence` (every envelope
    evidence entry). Limitations stay strings (prose, not values; every front end reads
    them so). `handlers/shapes_tests.rs` runs all ten commands in 31 modes on the refs and
    planet fixtures through `execute`, checks every envelope, the markers at their
    positions, and that the checker catches an unmarked formref and an undocumented
    member. Rules: `docs/mcp.md`, "Data shapes and markers". xtask's SF1Edit comparator
    reads the new opaque marker (`sf1edit::scalar_object`). Not covered: `build --plan`
    (M4.W6 adds its shape with the guard).
  - **Links and Planet panels (lane W; lane D's Request).** `refs` and `planet` are on
    `gate::ALLOWLIST` (read only) and in `cmdline::SESSION_COMMANDS`; `gate` re-exports
    the engine's marker names, which the view-models key on (`panels::fmt_value`,
    `formids`; the shape heuristic and its `docs/gaps.md` row are gone). Links tab:
    outgoing links to a depth or "use info" (incoming, winner or every version, type
    filter), rows from `link`/`referrer` markers (an unknown marker is counted, not
    guessed), placed referrers with cell and world, `next_cursor` paging. Planet tab: a
    tree's nodes indented by depth with their content links, the placements (with the
    branch chain) of a planet, star, biome, location, worldspace or pack-in, and every
    `link` marker in the result. "links" and "use info" on every record row (query rows,
    the record view, diff records, diagnostics, links and planet rows); "planet chain"
    on a record of a planet-family type. View-model tests on canned envelopes, which are
    checked against the engine's documented shapes; `dev_profile` walks both panels.
- Lane E, M3.E9 round 2 (overhaul build 2), 2026-10-15: `has_opaque` (engine
  `edit/patch.rs`) now flags only unknown layout (whole opaque payload, rest-of-payload run,
  edid-prefix union); fixed-size untyped struct fields and sibling-decided unions are known
  layout, and `untyped_id_runs` refuses a FormID remap (and validate counts a subrecord as
  unchecked) only when such bytes are 4+ non-zero (every official OMOD's are zero), so OMOD
  overrides copy from localized sources and remap (test
  `omod_overrides_of_localized_sources_copy_and_remap`). Spec: the default receivers'
  `WSPS` scaled with the WEAP rate (B3 `077F13` 7.5 -> 5.625, B6 `057555` 1.0 -> 1.5625,
  B11 `04A750` 16 -> 12), B10 `12D03F` ACPT ADD 5 -> 2 (total 7); owner answers applied
  (B8 dropped in brief.md, C8; B11 base 18 with the drum's +30 untouched, 48; B12 as built,
  Large 14, Extended 10, XL 22). `docs/overhaul/build-2-plan.md`: 46 changes on 21
  overrides (13 WEAP, 4 AMDL, 4 OMOD), master `Starfield.esm` only (the smoke test's mod
  folder is gone from `mods\`). `out\overhaul\KitKraftGunOverhaul.esm` 45,271 bytes,
  SHA-256 `5e2f84e7...148f982d1`; validate 0/0, roundtrip identical, package 0.2.0, deploy
  dry run refused by the live-profile guard (MO2 on `Default`). Rows: 13 implemented, B8
  dropped. Report, owner steps, checklist: `docs/overhaul/build-2.md`. **Owner (M3.O5):**
  the in-game check there, on `KitKraft Dev` with a disposable save.
- Lane D, M4 (`crates/doc`; engine `handlers/{refs,planet,load,inspect,build}.rs`), 2026-10-15:
  - **M4.D1 `refs`** (CLI `kit refs <form> [--depth n] [--incoming]`, alias `useinfo`; MCP
    through `execute`/`command_line`, no shortcut; `writes: false`). `kitkraft_doc::links`:
    every value of a member the schema types as `formid`, with its member path in the
    `set`/`where`/`diff` grammar (`WAM2.Ammunition.AmmoType`, `KWDA.Forms[3]`), the raw
    value, the definition's target types (`target_allowed`), file and body offsets, and a
    status that keeps `resolved`, `null`, `none` (`FFFFFFFF`), `missing`, `engine_reserved`
    (the validate rule, a test keeps the constants equal) and `unresolvable` (with the
    reason) apart; subrecords not scanned (no layout, component blocks, undecodable) and
    `opaque` members are counted in `limitations`. Outgoing: the winner's links, breadth
    first to `depth`. Incoming: `kitkraft_doc::backrefs`, a reverse index per plugin from
    the generated decoder's `form_ids()` (raw values; the target turned into each plugin's
    spellings, `full-style-self` included) with cell-group ranges, cached as `<hash>.kclnk`
    beside the record index (schema fingerprint and record count in the staleness rule);
    referrers in cells come after the others with cell, world and persistence; `scope`,
    `types`, paging by cursor. Times (`docs/load-order.md`, "Reverse links"): on the 84-plugin
    `KitKraft Dev` profile 18.9 s cold, 0.9 s warm (index 0.17 s), 220 MB cache. Corpus
    (`crates/engine/tests/refs_corpus.rs`, release, ignored): the rifle `21BBCD` has 111
    links (76 resolved, 35 null, 16 keywords), `WAM2.Ammunition.AmmoType` ->
    `Ammo9x39mm` `2B559B` -> a PROJ at depth 2; `NA_Hard_RetWallASupport_03` lists its 2
    placed REFRs in world `Starfield.esm:01251B` on the 14 official files.
  - **`follow` replaces lane E's local `via`** (`kitkraft_doc::links::follow(lo, reader,
    schema, form, where-path)`): `build` calls it; a hop now reports `missing` and
    engine-reserved values as unfollowed instead of failing on them. Build tests unchanged.
  - `inspect::find_form` is shared (`refs`, `planet`); `Loaded` keeps the cache directory
    and the reverse index.
  - **M4.D3 `planet`** (CLI `kit planet <form>`, alias `pcm`; MCP through `execute`/
    `command_line`; `writes: false`): for a `PCMT`/`PCBN`/`PCCN`, the tree from the nodes'
    upward links (`PRTN`, siblings ordered by `PRVN`, `order` says how), each node with
    selection fields, keywords, conditions as stored (union fields as hex), content and one
    level of content detail, ancestors, the `DFOB` holding a tree (stored-bytes match), and
    every unresolved or out-of-target link of the tree; for a `PNDT` its worldspaces,
    biomes, star (system ID match) and the content nodes placing its worldspaces with their
    branch chains; `STDT`, `BIOM`, `LCTN`, `WRLD`, `PKIN`, `LVLP` views likewise, with
    keyword condition candidates (raw-byte matches) for biomes and locations. Nothing
    evaluates a condition. On the 14 official files all five trees walk with 0 unresolved
    links (2,518 nodes; content counts of `Starfield.esm` equal planet-family.md's), about
    0.9 s each warm; `crates/engine/tests/planet_corpus.rs` (release, ignored) pins the
    trees, a planet chain (Mars -> Sol, 12 placing nodes) and prints the link table.
    `docs/design/planet-content.md`: link statistics by member, tree depths and sibling
    chains, condition function counts, the two typed paths to placed content (tree and
    biome), every claim graded, open questions for M4.O1/M4.O4.
- M3.B10 `count_from` consistency (lane B, 2026-10-15): generated encode writes a count
  field from its array's length (`count_value`) and the array up to what the count's type
  holds (`write_counted`; a `u8` count saturates at 255 and the array is cut to it), so an
  edited array never carries a stale count; a version-gated array that is absent leaves its
  count as stored. The loader rejects one count field shared by two arrays
  (`SharedCountFrom`). Tests: `yyyy_count_from_is_written_from_the_array_length`,
  `vvvv_gated_count_from_array` (new fixture member `VVVV.CNTG`), `count_from_rules`, and
  `set_typed_writes_count_from_counts_from_the_array` (esm: a STAT `MOLM` edited through
  `Record::set_typed`). Corpus re-encode unchanged (decode reads exactly the stored count).
  `docs/gaps.md` row restated to the residual: the engine's byte-level `kit set` on a
  `count_from` array or its count (lane E).
- M4.BX2 inventory definitions (lane B, 2026-10-15) for the 14 record types no family
  owns, from a scan of every official record (`crates/esm/tests/inventory_types.rs`;
  `inventory_types_full` is ignored, release): corpus order and cardinality, EDID, the
  corpus table's lstrings, `Keywords` and `BfcbBlock` typed, every other member opaque and
  named after its signature, each citing the scan at `corpus` and the pinned xEdit line
  (`crates/schema/defs/README.md`, "Inventory definitions"). PGRE and PHZD are `unordered`
  (two orders in the corpus; xEdit L7113). DOOR, IRES, LSCR and MESG replace their M3.B8
  lstring-only stubs (same lstrings; paths are now top-level, `MESG.MenuButton.ITXT`).
  Records per type, every one placed, decoded and re-encoded identically: RFGP 169,006,
  LAYR 10,424, MESG 2,671, PGRE 2,599, LIGH 1,723, SOUN 1,133, WWED 681, PHZD 450, LSCR
  393, DOOR 376, IRES 333, ARTO 212, WKMF 81, TXST 23. Parity: 96 definitions;
  container/other 77,174 typed, 467,889 opaque, 534,776 unknown (was 56,063 / 6,998 /
  1,016,778); `INVENTORY` in `xtask/src/parity.rs` lists them. **Owners**: none assigned
  yet; the matrix names the consumers (W2: LAYR, RFGP; W3: WKMF; W7/W8: IRES; W10: MESG).
  Record an owner here when a family lane takes a type. Gaps row: `docs/gaps.md` (M4.BX2
  inventories).
- Lane E round 5 (engine `edit/`, `validate/`, `build/`, `handlers/`; `crates/io`
  instance; `docs/recipes/`), 2026-10-15:
  - **M3.E4 lstrings from the schema.** `set`, `build` and `validate` find every schema
    `lstring` wherever the layout puts it (whole subrecord, any struct member, array
    element, union variant) through `kitkraft_doc::typed::lstring_spans` (the decoder's
    walk, with member paths), replacing the "whole subrecord or last struct field" rule in
    `edit::patch` and `validate`. `edit::lstrings` is a cross-check and fallback:
    `compare` finds 100 of the table's 105 lstrings typed as whole-subrecord schema
    members, 0 conflicts, and 5 (CHAL `FULL`/`DESC`, GBFM `HULL`, PNDT/STDT `FULL`) inside
    component blocks the schema keeps raw, where the table is used with a limitation
    (unit test `the_schema_types_every_table_lstring`; the corpus test
    `lstring_table_matches_the_corpus` now also checks the schema against the fresh scan).
    No current definition has an lstring inside a struct (all are whole subrecords). The
    `docs/gaps.md` lstring row is closed; `docs/validate.md` "lstring signature table"
    updated.
  - Engine-reserved constants defined once (`kitkraft_doc::links`, re-exported by
    `validate`); `kitkraft_io::Instance` (read-only MO2 instance) used by `deploy` and the
    profile reader (Requests, struck).
  - **M4.E7 recipes W3 and W8** (`docs/recipes/workshop.{md,toml}`,
    `docs/recipes/fabricator.{md,toml}`), each fact with its `query`/`inspect`/`refs`
    evidence: an outpost recipe is a `COBJ` filed by its `BNAM` category keyword
    (`OutpostCategoryDecorations` `293ADF`), and nothing lists it (0 incoming links); a
    fabricator recipe is a `COBJ` filed by `WorkbenchIndustrialKeyword` (`0AA209`), the
    ammunition ones behind one condition. Workshop: a clone of `CoffeeBrewingMachine01`
    and of its recipe `co_Outpost_Decor_CoffeeMaker01` with `CNAM = "new:coffee"` (726
    bytes, `f9dc6d24...`); fabricator: a clone of the unconditioned
    `co_mfg_Tier01_AdaptiveFrame` making `AmmoShotgun` x6 (373 bytes, `1d5cce72...`).
    Both build, validate 0/0, roundtrip identical; testplans written (no console command:
    nothing placed), in-game steps in the docs. Engine: a spec field value `new:<name>`
    names a `[[new]]`/`[[clone]]` of the same build (`{"local_form": EDID}`, applied by
    the patch writer once FormIDs are allocated; test
    `field_values_name_new_forms_of_the_build`). **Owner (M4.O4):** the in-game steps of
    both docs on `KitKraft Dev`, disposable save (both plugins own forms).
  - **M4.E9 script attachment.** `edit::vmad`: the `VMAD` script list (lane BQ's
    `ScriptAttachment` header plus scripts, properties of every type, fragment bytes kept),
    from `wbDefinitionsSF1.pas@9fb01688#L3232-L3411`; all 12,857 official `VMAD`s
    re-encode identically (`tests/vmad_corpus.rs`), and attaching to a corpus copy keeps
    the vanilla scripts and fragments byte for byte. `place`, `set` and `new` take
    `scripts = [{name, properties}]` (spec v3 `[[place]]`, `[[set]]`, `[[new]]`,
    `[[clone]]` too; values typed by the declaration, objects as formrefs, EditorIDs, null
    or `new:<name>`); the patch writer attaches them in its late phase (VMAD created in the
    schema's position or updated). `validate::attached`: a script is found as loose
    `Scripts\NS\Name.pex` (next to the target or in Data) or in the archives of the plugin
    and the load order; declarations are the `.pex` property tables up the `extends`
    chain, with `kit papyrus events`'s source index as fallback. Rules
    `papyrus.missing-script` and `papyrus.property-unresolved` (**error**; **warning** when
    an override carries the master's attachment unchanged); the same cases refuse a write.
    The official files carry stale attachments in their own records, pinned as known
    errors in `validate_corpus_official_files` (Starfield.esm 14 + 16, ShatteredSpace 3 +
    13, SFBGS00D 2 + 25, SFBGS050 2 + 7, BlueprintShips-Starfield 1). Recipe
    `docs/recipes/scripts.{md,toml}` with `KitKraft:HelloOnLoad` (ours, compiled by
    `papyrus.compile`, OnLoad from `papyrus.events`) on a placed switch in the embassy:
    15,297 bytes, validate 0/0, roundtrip identical. Tests through `execute`:
    `handlers/scripts_tests.rs` (a fixture per rule, refusals, inherited warning).
    **Owner (M4.O4):** the in-game steps in `docs/recipes/scripts.md`.- M5.D12 scale-aware remap (lane D, 2026-10-18, branch `lane-d/scale-aware-remap`):
  `write_patch` copies records whose winner is a small or medium plugin (or has such
  masters) as stored when the patch's masters keep their scale classes and order, and
  remaps by class otherwise (`kitkraft_doc::MasterMap`); the old plain-list remap is gone
  (it mis-indexed full masters behind a small or medium one). Unblocks MEL DLC row B2: the
  `SFBGS003.esm` bounty targets build (`kit build --plan` validate 0/0). `docs/gaps.md`
  rows restated.

## In progress

- M4.BX1 lane H merge (integrator, BX3, 2026-10-14): `cargo xtask oracle sf1edit` (xDump64
  harness, M3.H4) and its `docs/oracles.md`, `source-registry`, `xedit-reference` and
  gaps rows; conflicts in `docs/gaps.md` and STATUS kept both sides (the stale "not
  installed" and "not transcribed yet" gaps rows dropped). Rerun on the five small official
  files after the three family merges (14.5 min; the dumper's exception log deleted):
  agreements 191 -> 4,963, gaps 9,672 -> 2,225 occurrences; the 2,675 new disagreements are
  enum, flag, GUID and opaque-field display conversions the comparator does not make, not
  value differences (`docs/oracles.md`, "Rerun after the schema integration"; a comparator
  request to lane H). Gates: `cargo xtask ci`, the ignored release tests for esm, doc and
  engine, parity (unchanged from the ITEMS merge).
- M4.BX1 `unordered` record mode (integrator, BX3, 2026-10-14; closes the world lane's
  request and the gaps row): `unordered = true` on a record definition (loader: records
  only, no top-level `any_until`, `DiskOrder` reserved; `RecordDef::unordered`). Codegen:
  `match_unordered` matches a subrecord from the current member on and, when none accepts
  it, from the first member again (groups still match one run in their own order); the
  record keeps `disk_order` (the member of each typed value in on-disk order) and
  `encode` writes the values back in that order (edits: remaining values keep their order,
  added ones follow member by member; a fresh record is written in definition order), so
  decoding stays lossless and every member keeps its path. Generated `UNORDERED` and
  `match_body` pick the matcher for `bridge`, `coverage` and the world family test;
  `kitkraft_doc::typed::placements` mirrors the rule. Applied to CELL, REFR and ACHR.
  Tests: fixture `UNOR` (any order, `XXXX` kept, edits, a second optional value raw in
  place, random bodies), loader rules, doc placement. Corpus: `world_family_full` (release,
  all 6,719,725 REFR, 40,251 CELL, 14,731 ACHR) places every subrecord the definitions name
  (0 unplaced, 0 failed, 0 re-encode differences, 89 s); parity **failed 1,712,200 -> 0**
  (46,249,514 typed, 2,261,438 opaque, 1,908,609 unknown; no subrecord is left undecodable).
  The newly typed out-of-order CELL XBPS and REFR XPLK expose more of the two known
  dangling classes (`validate_corpus_official_files`, fixed at the main merge): SFBGS050
  XBPS 24/23 -> 119/107, BlueprintShips-SFBGS050 XBPS 10/10 (five cells, 20 of its own
  references it lacks), ShatteredSpace XPLK 30 -> 41 (all at file index 02). Main merged
  afterwards (lane E's M4 commands and cell rules, lane A's placement fixes): `ref.orphan`
  keeps its REFR NAME 0x1FB known error; all gates green on the merged head.
- **M2.F7 (owner check pending).** `.mcp.json.example` and "Use from Claude Code" in
  `docs/mcp.md` are in; an agent cannot restart Claude Code, so the owner runs:
**M2.F7 result (2026-10-14):** PASS. Owner opened a fresh Claude Code session: `kitkraft` connected with exactly 8 tools (execute, command_line, coverage, diff_plugins, inspect_record, query_records, record_schema, roundtrip), the Default profile preloaded, `record_schema WEAP` returned 50 subrecord definitions (6 typed, 44 opaque), `coverage Constellation.esm` returned 57 records / 8 types. Caveat filed as a lane F request below.

  1. `cargo build --release -p kit-cli`
  2. `claude mcp add --scope local kitkraft -- C:\Modding\kitkraft\target\release\kit.exe mcp`
     (or copy `.mcp.json.example` to `.mcp.json`, which is gitignored, and approve it).
  3. New Claude Code session: `/mcp` lists `kitkraft` connected, 8 tools.
  4. Ask it to call `mcp__kitkraft__coverage` on `<Data>\Constellation.esm`, then
     `mcp__kitkraft__execute` with `{command: "roundtrip", args: {plugin, verify: true}}`,
     then `mcp__kitkraft__inspect_record` on a form `coverage` names (`first_record`), then
     `mcp__kitkraft__query_records {type: "WEAP"}` and `mcp__kitkraft__record_schema
     {type: "WEAP"}`; each returns an envelope (`query_records` needs a load order: pass
     `--profile` to `kit mcp`, which now preloads it).
  5. Paste a transcript excerpt here. Headless alternative: `claude -p "..." --mcp-config
     .mcp.json --allowedTools "mcp__kitkraft__*"`.
- **Lane B round 3 (M3, `crates/schema`).** M3.O0 done: the owner staged the pinned files
  outside the repo at `C:\Modding\tools\xedit-ref\` (`wbDefinitionsSF1.pas`,
  `wbDefinitionsCommon.pas`, `PIN.txt`). M3.B0 done 2026-10-14: sizes 789,023 and 338,167
  bytes, blob hashes `ace733b52e259e559aed0f4c71edf9e3432fc62b` and
  `cfa80ba05c176d7571e19895e835f8df735a06cf`, SHA-256 `9f01921a...952346d5` and
  `e616b654...682b7d26`, all as in `docs/xedit-reference.md`; the loader already accepts
  both files (M2.B1). Typed so far (each checked on every corpus record of its type: typed
  members decoded, nothing left in `extra`, zero re-encode differences, and the full-corpus
  typed check of `corpus_full_roundtrip` clean):
  - **M3.B1 WEAP**: all 452 records; every subrecord xEdit lists is typed except VMAD (script
    data, unions) and MO4C (unknown upstream); the BFCB component data stays raw (a union
    decided by the component type). New shared definitions: `Model`, `Destructible`,
    `SoundReference`, `ObjectModProperty`, `Vec3`, `Bool8`; `BfcbBlock` and `ObteBlock`
    typed (component type, end markers, object template items).
  - **M3.B2 AMMO, PROJ**: all 154 AMMO and 299 PROJ records. AMMO: every member typed
    except VMAD; DATA's weight is `if_remaining` (xEdit makes it optional). PROJ (no
    inventory before; the corpus check is its inventory): every member typed except VMAD and
    the BFCB component data; PROD's two particle define strings have a u32 length prefix,
    typed as a count field plus raw bytes (the format has no u32-prefixed string); the
    unused 0-byte DATA is an `empty` marker.
  - **M3.B3 AMDL**: all 126 records (EDID + 65-byte ANAM, 17 fields typed; the last one
    `if_remaining` as xEdit makes it optional).
  - **M3.B4 OMOD**: all 3,330 records (DATA 34..511 bytes): DATA typed with its include
    list and its property entries (shared `ObjectModProperty`, 24 bytes each: value type,
    function type and property as integers, Value1/Value2 opaque because they are xEdit
    unions and Value1 can be a FormID; M3.B9 can type them once a sibling-field decider
    exists). Name and the attach parent slot list carry u32 prefixes, typed as explicit
    count fields. VMAD, the unknown XFLG after the model block (never reached in the corpus:
    the model block's XFLG takes it) and the BFCB component data stay opaque.
  - **M3.B5 COBJ, ZOOM**: all 3,518 COBJ and 141 ZOOM records. COBJ: every member typed
    except VMAD, the BFCB component data and the condition payloads (new shared group
    `Condition`: CTDA opaque, a struct of xEdit unions; CIS1/CIS2 typed); components use the
    new shared type `Component`. ZOOM: EDID + the 35-byte ZNAM, fully typed.
  - **M3.B7 LVLI, FLST, INNR** (no inventories before; the corpus check is their
    inventory): all 6,543 LVLI (22,701 entries), 1,415 FLST and 8 INNR records. LVLI: every
    member typed except VMAD (one official record has one), the BFCB component data,
    condition payloads and the COED owner data (an xEdit union); the entry count LLCT is a
    separate subrecord, stored as read (M3.B10 covers `count_from` arrays only, so an edit
    that adds an entry must update LLCT itself; noted in the definition). FLST: LNAM list,
    conditional entries (INAM, CITC, conditions), ANAM. INNR: rulesets with their names,
    keywords, XNAM properties (the target byte is an xEdit union by UNAM, kept as an
    integer) and indexes.
  - **M3.B8 lstrings**: all 105 lstring signatures of lane E's corpus table are schema
    `lstring` members (`corpus`; the xEdit line cited for 103, CHAL DESC corpus only).
    The 49 record types without a definition got lstring-only definitions: EDID plus a
    repeated `LocalizedStrings` group of optional lstring members, so every occurrence is
    typed wherever it stands and every other subrecord stays raw in place. That is less
    than the plan's "inventory definition" (corpus order and cardinality of every member):
    a full inventory for types such as NPC_, QUST, REFR, INFO is left to later rounds.
    Corpus check: every lstring occurrence of those 49 types is visited (counts equal), 0
    conflicts, 0 re-encode differences, including 6.7 million REFR.

### M3 lane H (evidence: CK observation and oracles)

| Task | State |
| ---- | ----- |
| M3.H1 fixture matrix and session scripts | **done** except F07's recompression candidates (wait for M3.A1's Request): `docs/ck-observations.md` has the ledger format, the matrix F00..F14 with every form resolved (FormID, EditorID, why, the command that found it), the M3.O1 script per fixture, the M3.O2 lists with KitKraft's assumed runtime IDs, and the `fixture.toml` template |
| M3.H2 `cargo xtask observe` | **done**: `cargo xtask observe <fixture-dir>` writes `report.md`/`report.json` in the fixture folder only (SHA-256 of every file; per plugin TES4, `roundtrip --verify`, `coverage`, `validate`; base, `[[compare]]` and `kit_equivalent` comparisons as byte runs, TES4 diff and `kit diff`, our equivalent in both styles with `ck` reported unavailable until M3.E5); `cargo xtask observe new <F-id>` writes the fixture's `fixture.toml`. Tests: synthetic fixture dirs with a fake `kit` (nothing written outside, rerun byte-identical) and the real `kit` when built |
| M3.H4 oracle harness | **Spriggit part done**: `cargo xtask oracle spriggit [--official] [--synthetic] [<plugin>...]` (YAML deleted unread; compared with `kit diff`, `coverage`, `roundtrip`, byte and TES4 diffs); run 2026-10-14 on the 5 smallest official files, 3 hand-built synthetic plugins, 3 `kit set` overrides and the rename plugin; `docs/oracles.md` has the build, fixture set and 11 disagreements (S1: 885 lstring-sized values zeroed in localized files; S3: our TES4 `DATA` after `MAST`, see Requests). M3.O1 fixtures and the overhaul build: rerun when they exist. **SF1Edit part done** (2026-10-14, after M3.O3): `cargo xtask oracle sf1edit [--official] [--synthetic] [<plugin>...] [--types ..] [--sample N]` runs xEdit 4.1.5f's console dumper (`xDump64.exe`; `KITKRAFT_SF1EDIT` = `xSFEdit64.exe` or the dumper) on a scratch Data copy and compares with `kit query`/`inspect`; the editor's script mode is **blocked** headless (Module Selection window; without `-script`, the developer message), documented in `docs/oracles.md`. Results: all records, FormIDs, signatures and EditorIDs agree where xEdit can read the file; 711 decoded members agree on `Starfield.esm` WEAP/AMMO/PROJ/COBJ/OMOD (50 each) and 191 on the four readable small official files (every record), 1 disagreement (X8, ours: lossy cp1252 lstring text); version skew: GPOG/GPOF unknown, SFBGS047 (light master) not loadable, 51 WEAPs with an undecodable component; the coverage-gap list per round 3 type is in `docs/oracles.md` for M3.B11 |
| M3.H3 findings, M3.H5 CK feature ranking | **blocked on M3.O1** (the owner's CK session; `docs/ck-observations.md` has the resolved script, `cargo xtask observe new <id>` the templates) |

### M4 lane H (carried tasks, 2026-10-15)

| Task | State |
| ---- | ----- |
| M4.H3 `docs/design/persistence.md` | **done** (research skeleton): persistent references on disk, what keeps references alive, quest aliases, load/attach/reset events, cell and location resets, procedural locations, outposts, ownership; 39 claims `PS-01..PS-39`, each graded (ADR 0005 Am. 3) with its source (official `.psc` declarations and Bethesda's comments paraphrased, corpus counts and values, our schema); design rules for a claim/restore and outpost mod; gotchas; every grade D claim is a question with its ledger row. Corpus facts found: `iHoursToRespawnCell` 168, `iHoursToRespawnCellCleared` 480, `iDaysToRespawnVendor` 2; 17 of 6,017 `LCTN` flagged never-resets, none workshop; no `ECZN` in `Starfield.esm`; 517 `LCTN` `LCPR` lists; alias fill-type counts. Appendix A: `kit papyrus events --name <n> --declarations-only` for all 59 names (56 found; `IsCleared`, `SetCleared`, `IsPersistent` absent). CK wiki unreachable (maintenance redirect) and UESP's CK mirrors refuse automated reading: no claim rests on them. Papyrus Index cross-check (registry: cite only here or in a test): `papyrus-index:` <https://papyrus.bellcube.dev/starfield/script/objectreference/> lists the same eight load, cell, reset and workshop events on ObjectReference as Appendix A |
| M3.B11 SF1Edit differential on typed types | **done** (lane H, 2026-10-15): `cargo xtask oracle sf1edit Starfield.esm --types WEAP,AMMO,PROJ,AMDL,OMOD,COBJ,ZOOM,LVLI,FLST,INNR,KYWD,GMST --sample 0 --no-check`, all 21,686 records, 239 s (inspects through one `kit mcp` session). Comparator: typed walk along the schema, order-preserving alignment, enum/flags/integer-code names as name pairs checked for consistency over the run, GUID and byte-text display, nested lstrings resolved through the string tables, float-max "Default", NUL and multi-line text, "fewer fields" class. Result: 149,454 agree, 2,825 agree with fewer xEdit fields (AMDL `ANAM` X10, length prefixes), 190 disagree (`PROJ` `PROD`, X9: 4.1.5f splits the u32 Flags), 0 of ours, so no gaps row; 13,077 -> 190 disagreements against the old comparator; X8 now agrees. Remaining opaque-vs-typed gaps are all inside component blocks (`ANAM`/`BNAM`/`STCP`, `FLTR`, OMOD `KSIZ`/`KWDA`) or `VMAD`: `docs/oracles.md` "M3.B11" |
| M4.H4 game ledger | **done**: `docs/game-observations.md` (the name `kit testplan`, the recipes and ADR 0008 already use; `docs/ck-observations.md` points to it): entry format `game-<yyyymmdd>-<nn>`, persistence protocol (leave, wait, save/reload, return, long absence > 168 h), templates O2 (rename done, packages, POI `coc CityNewAtlantisApartmentTEMPLATE` then `player.moveto KitKraftFirstPOICrate`, persistence, console `placeatme`), O4 (ownership, actors, recipe rows pending M4.E7/E9, PS-question rows), O5 (overhaul C1..C14 with FormIDs, persistence and drop-the-plugin rows); first entry `game-20261014-01` (M2.O6 PASS, from STATUS) |

## Next

M2 lanes (from `PLAN.md` "M2 - agent surface" and the step 2 ladder). Owners are assigned
when M2 is broken down in `plan/execution-plan.md`.

| Lane       | Scope                                                                                                          | Where                                        |
| ---------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| doc        | Load order, FormID resolution across masters, overrides and conflicts ("which plugin wins")                    | `crates/doc`                                 |
| io         | MO2 profile reader: `plugins.txt`, mod list, Creations manifests                                               | `crates/io`                                  |
| mcp        | stdio MCP server over `execute()` (`execute`, `command_line`, ...); `kit mcp` launcher                         | `crates/mcp`, `apps/kit-cli`                 |
| ports      | `ba2` (`ba2x.py`), `pex` (`pexdis.py`), `strings` ported from `C:\Modding\scripts`                             | `crates/ba2`, `crates/pex`, `crates/strings` |
| validate   | Rules: missing masters, bad links, localized FULL copies, stripped FULL/NTRM, god-mode pex calls               | `crates/engine`                              |
| edit       | `set`, `query`, `diff` over load order and typed records (needs generated records wired in esm)                | `crates/engine`                              |
| smoke test | Rename "Old Earth Hunting Rifle" to "Old Soviet Marksman Rifle": `kit set` -> `kit validate` -> launch via MO2 | all                                          |
| container  | `roundtrip --decode`; typed edits through `Record::set_typed` (decode side wired: `Record::typed`)             | `crates/esm` (lane A)                        |
| schema     | Round 3: M3.B0-B5, B7, B8 done (see In progress); B6, B9-B12 open                                               | `crates/schema` (lane B)                     |

Lane B round 3 transcribes definitions from the pinned xEdit files (`xedit:` provenance,
`confidence = "reference"` until the corpus confirms) in this priority: **WEAP, AMMO, PROJ,
AMDL, OMOD, COBJ, ZOOM**; then **GMST, GLOB, KYWD** typed; then **LVLI, FLST, INNR**. Each
definition is checked against every corpus occurrence before it moves to
`confidence = "corpus"`; unions stay opaque.

## Blockers

- None for lane B (the pinned xEdit files are staged, M3.O0).

## Decision records (owner, 2026-10-14)

- **M2 fully closed.** M2.F7 (fresh-session MCP check) and M2.O6 (in-game rename) both
  passed (results under "In progress" and "Done" above); no M2 task remains open.
- **M3.O3 done** (was M2.O3): xEdit is installed at `C:\Modding\tools\xEdit\`; Starfield
  mode is `xSFEdit64.exe` (file version 4.1.5.0, older than the `dev-4.1.6` pin
  `9fb01688...`, so M3.B11 records the skew). Unblocks M3.H4's SF1Edit part
  (`KITKRAFT_SF1EDIT`) and M3.B11 (see Requests).
- **Owner decision 1 closed: the name is KitKraft.** Identifiers are renamed too:
  crates `kitkraft-*`, env vars `KITKRAFT_*` (`DATA_DIR`, `CACHE_DIR`, `SF1EDIT`), MCP
  server name `kitkraft`, resource URIs `kitkraft://`, the cache dir, and the repo folder
  `C:\Modding\kitkraft`; the binary stays `kit`. **Rename done** in the M4.BX1
  integration (see "Done"); the old `KITCRAFT_*` env vars and `kitcraft-manifest.json` are
  read as a deprecated fallback for one release. Prose uses KitKraft.
- **M4.O3 decided, default overturned** (ADR 0008): SFSE, native runtime extensions and
  analogous frameworks are supported where technically justified; core crates never
  depend on them (`runtime/` or an `RT` crate); CommonLibSF not read or linked until its
  license exceptions are quoted from its `LICENSE` and reviewed; SFSE binaries never
  bundled; game versions tracked (owner's install: `Starfield.exe` 1.16.244.0 Steam, SFSE
  0.2.21); PC-first; gameplay is not compromised only to avoid SFSE. Policy, not an
  implementation order: no runtime component in M4.
- **M4.O5 decided, default approved with an exception** (ADR 0009): `package` stays
  target-independent and never writes into MO2; a separate, explicitly invoked
  `kit deploy --dev` (M4.E11, lane E, later) writes one isolated
  `KitKraft Dev - <plugin>` mod under MO2's `mods\` with path validation, a hashed
  manifest, temp + rename replacement, diagnostics and rollback; never profiles,
  `plugins.txt`, `loadorder.txt` or other mods.
- **M3.O6 decided: ADR 0007 accepted with scope** (Amendment 1): the egui M5 deferral is
  partially overturned. A minimal read-only developer workbench (L5 `crates/ui-egui`,
  `apps/kit`, over `Engine::execute` only: record browser, structural diff, schema and
  provenance, FormID and reference diagnostics, build validation) is planned as M4 lane W
  (M4.W0..W6) after the inspection APIs stabilize; the full authoring UI stays M5. M3.F5
  is not built; M4.F3 is superseded.
- **CK sessions deferred** (M3.O1 and M4.O1, F00..F28): until the developer workbench
  exists (M4.W2..W5), so the owner can A/B the CK against KitKraft's inspection surface;
  computer use is available for an agent-driven CK session then. The first POI path does
  not wait; `ck-diff` confidence, the `ck` write style and the CK fixture questions do
  (`plan/execution-plan.md`, M3 and M4 "Ordering" updates).

## Open owner decisions

| #   | Decision                                | Default until decided             |
| --- | --------------------------------------- | --------------------------------- |
| 1   | Project name                            | **decided 2026-10-14**: KitKraft; identifiers renamed to `kitkraft-*` / `KITKRAFT_*` / `kitkraft://` (**done**, M4.BX1 integration; old env names read for one release); binary `kit` |
| 2   | xEdit MPL source policy (ADR 0004)      | **decided 2026-10-14** (pinned xEdit SF1 definitions as reference) |
| 2a  | Review `docs/xedit-reference.md` and ADRs 0004/0005/0001 before lane B round 3 | **approved 2026-10-14** (M2.O1; lane B unpaused) |
| 2b  | Whether `wbDefinitionsCommon.pas` (helpers the SF1 file uses) may be cited too | **approved 2026-10-14**: yes, same SHA, same rules (ADR 0004 owner approval) |
| 3   | Video critique used as overhaul brief   | none; overhaul work is after M2   |
| 4   | License                                 | MIT OR Apache-2.0 (scaffolded)    |
| 5   | Contact kinggath about SS2 now or later | later                             |
| 6   | Git remote / publishing                 | local only                        |
| M3.O6 | UI (ADR 0007)                         | **decided 2026-10-14**: accepted with scope (read-only workbench, M4 lane W; authoring UI M5) |
| M4.O3 | SFSE / runtime extensions             | **decided 2026-10-14**: supported where technically justified (ADR 0008) |
| M4.O6 | Agent-driven game sessions on `KitKraft Dev` (ADR 0010) | **decided 2026-10-15: approved in principle** (ADR 0010 Amendment 1): screenshots in scope, input injection deferred, `crates/game` dependencies MIT/Apache-2.0 confirmed by the owner at the merge |
| M4.O5 | Installing into MO2                   | **decided 2026-10-14**: `package` never; `kit deploy --dev` only (ADR 0009) |
| UI track | Audience, pains, stack, editor goal (2026-10-16) | **decided 2026-10-16**: "Both, cockpit first" (M6 Cockpit, then M7 CK parity, M8 Beyond the CK); top pains (1) look, feel and performance, (2) game-loop integration, while navigation and the read-only wall are not top pains; keep egui (license rule unchanged, OFL fonts off); M7 reproduces the CK's interface from black-box observation ("users know one, know the other"), and its Render Window is a 3D viewport from its first ticket. `ROADMAP.md` "M6 onward" |
| M6.O3 | Cockpit write scope (ADR 0007 Amendment 3) | **decided 2026-10-16: approved** as written ("Approve Amendment 3 and ADR 0012, start lane V mockups"): `ledger`, `deploy --dev`, `deploy.rollback`, `game.session`, `game.probe`, `game.stop`, each previewed and confirmed per click; each enters the gate with its ticket (M6.G3 to M6.G5) |
| M7.O1 | CK interface clean room (ADR 0012) | **decided 2026-10-16: approved** (same words); `plan/adr/0012-ck-interface-clean-room.md` |

### M2 owner tasks (lane O, `plan/execution-plan.md`)

| #     | Task                                                        | State                                  |
| ----- | ----------------------------------------------------------- | -------------------------------------- |
| M2.O1 | Approve ADR 0004, ADR 0005 Amendment 2, `docs/xedit-reference.md` | **done 2026-10-14** (with six clarifications) |
| M2.O2 | Whether OMOD is in scope                                    | **done 2026-10-14**: in scope (M2.B7)  |
| M2.O3 | Install SF1Edit (agents do not download executables)        | **done 2026-10-14 as M3.O3**: xEdit 4.1.5f at `C:\Modding\tools\xEdit\` (older than the pin; `docs/oracles.md`) |
| M2.O4 | CK and MO2 observation sessions                             | open; blocks M2.D1 check, M2.D3 (c), the `hardcoded-official-slots` check, M2.E3 `ck-diff` part |
| M2.O5 | Third-party mod plugins as inventory evidence               | open; default **no**                   |
| M2.O6 | In-game check of the rename plugin                          | **done 2026-10-14: PASS** (M2 fully closed) |

#### M2.O6: in-game check of the rename plugin (owner)

The plugin is `C:\Modding\kitkraft\out\OldSovietMarksmanRifle.esm` (2,221 bytes,
SHA-256 `9C0B28A6...4078EA27`, regenerated 2026-10-14 after lane E round 2 added `CNAM`
and `INCC` to its `TES4`; full hash and details in `docs/smoke-rename.md`). It
renames `Starfield.esm:0021BBCD` (WEAP `RussianHuntingRifle`, "Old Earth Hunting Rifle")
to "Old Soviet Marksman Rifle". In the Default profile the rifle's winner is
`Starfield.esm` (no mod overrides it, checked 2026-10-14 with `kit inspect --profile`), so
the patch's only master is `Starfield.esm`. Use a disposable save.

1. MO2: right-click the mod list > *Create empty mod* > `Old Soviet Marksman Rifle
   (kitkraft smoke test)`.
2. Copy the plugin into `C:\Modding\MO2-Starfield\mods\Old Soviet Marksman Rifle
   (kitkraft smoke test)\`.
3. Enable the mod and its plugin; put the plugin last in the load order (it must load
   after the rifle's winner; if a mod that overrides `0021BBCD` is added later, rebuild
   the patch from that load order and keep the patch after it).
4. Launch Starfield through MO2, load the disposable save, console:
   `player.additem 0021BBCD 1`.
5. Read the name in the inventory and the weapon's inspect view; record it here (pass or
   fail, with a screenshot if possible).

Expected: "Old Soviet Marksman Rifle". The shown name is the base FULL plus the
instance naming rules of INNR `dn_CommonGun` (the rifle's `INRD`), which prefix names by
keyword ("Titanium", "Elemental", ...): a modded instance may read "Elemental Old Soviet
Marksman Rifle", which is still a pass. The old name means the plugin is inactive or not
last, or an OMOD/INNR string supplies the name (record which); garbage such as `%@` means a
string ID was read as text (a `lstring.copied-id` miss). Disable the mod afterwards; it adds
no forms and no scripts.

Result: **PASS 2026-10-14** (recorded under "Done", "M2.O6 result").

### Divergence from the brief (for the owner)

The agents' docs diverge from `PLAN.md` in two places, now reconciled in `PLAN.md` ("AGENTS.md
wins") but still the owner's to confirm:

- **xEdit stance.** Resolved 2026-10-14: the owner decided ADR 0004 (pinned
  `wbDefinitionsSF1.pas` as the record-definition reference); `PLAN.md` and AGENTS.md agree.
- **No push.** The brief said "push to main"; AGENTS.md says commit to local `main`, no
  remote (owner decision 6).

## Requests (cross-lane)

- ~~**Lane D (M4.D1, M4.D3) -> lane W.**~~ Done 2026-10-15 (lanes F and W follow-ups: Links and Planet panels, `refs`/`planet` on the gate's allowlist and in `cmdline::SESSION_COMMANDS`, "links"/"use info" on every record row). Was: `refs` and `planet` are in the catalog (`writes:
  false`, read only); `planet` needs the same treatment (a Planet view over `data.nodes`
  with `depth` indentation, or `data.placements`). `refs` in detail: it is in the catalog (`writes: false`, read only). The
  workbench has no generic envelope view a new command can use without code, so lane D
  only listed `refs` and `planet` in the gate test's `UNUSED_READ_ONLY` (its classification test
  requires every catalog command be classified). Please: move `refs` to `gate::ALLOWLIST` and
  `cmdline::SESSION_COMMANDS`; give the Records panel's inspect view "Outgoing links" and
  "Use info" actions (`refs {form}` and `refs {form, incoming: true}`) that list
  `data.edges` (path, status, `to`/`from` as links that run `inspect`, `placed.cell`/
  `placed.world` for placed referrers) with `next_cursor` paging, and show
  `limitations`. Result shape: `docs/mcp.md`, "`refs`".
- ~~**Lane D (M4.D1) -> lane E (FYI).** `kitkraft_doc::links::{RESERVED_MASTER,
  RESERVED_OBJECT_IDS, NONE_FORM_ID}` restate validate's rule (a test in
  `handlers/refs.rs` keeps them equal); `validate` could use them, so the rule lives once.~~
  Done (lane E round 5): the constants are defined only in `kitkraft_doc::links`;
  `kitkraft_engine::validate` re-exports them, and the equality test is gone.

- ~~**To lane F (from the F7 check):** `coverage`, `diff` and `roundtrip` reject a bare plugin filename and need the full Data path, while `load`, `inspect` and `query` fall back to KITKRAFT_DATA_DIR. Make bare names resolve the same way in every command (M4.F1 or earlier).~~
  Done in M4.F1 (Done, "Lane F, M4"): one resolver (`handlers/resolve.rs`) for every
  input name; `not_found` names the places searched.

Agents append here when they need a change in a crate they do not own.

- **Lane E (M4.E9) -> lane G (`papyrus.compile`).** (1) A relative `imports` path is
  handed to the compiler as given, and the compiler resolves it against `out_dir` (its
  working directory): `--imports docs\recipes\scripts` failed with "Cannot use import
  folder ...\out\recipes\Scripts\docs\recipes\scripts"; make imports (and `out_dir`)
  absolute before the call. (2) A `.psc` path of a namespaced script
  (`ScriptName KitKraft:HelloOnLoad` in `KitKraft\HelloOnLoad.psc`) compiles as
  `HelloOnLoad` and the compiler refuses the name mismatch; deriving the namespace from
  the folders below an import (or reading the `ScriptName` line) would let the path form
  work. Workaround used in `docs/recipes/scripts.md`: the name plus an absolute import.
  **Landed 2026-10-15 (lane B, M5.B2; papyrus oracle P1..P3, `docs/oracles.md`):** both
  done (paths absolute, `;` lists and `--import`; the `.psc` path compiles as its declared
  name), plus `-ignorecwd`; `cargo xtask oracle papyrus` finds all eight of our scripts
  semantically equivalent to the official compiler run by hand.
- **Lane E (M4.E9) -> lane B / lane D (FYI).** Object values inside `VMAD` (43,246 in the
  official files) are now readable through `kitkraft_engine::edit::vmad::objects`;
  `refs` and `formid.dangling` still skip them (the schema keeps `ScriptAttachment.Data`
  opaque). Feeding them to `formid.dangling` names reserved actor values below 0x2D0 from
  VMAD properties (Aggression 0x2BC, Confidence 0x2BD, Assistance 0x2C1, Suspicious 0x2EC)
  and changes the pinned reserved counts, so it was left out of M4.E9.

- ~~**Lane E (M3.E9) -> lane B (M3.B9).**~~ Done (lane B round 4; used in overhaul build 2,
  `docs/overhaul/build-2.md`). The overhaul's open parts were OMOD property
  values: `ObjectModProperty.Value1` is opaque because xEdit decides it by the sibling
  `ValueType` (`wbDefinitionsSF1.pas@9fb0168...#L1981-L2031`, list `#L6894-L6938`), and
  the format only has `edid-prefix` unions, so this is format work (loader, codegen,
  `kitkraft_doc::typed`, `edit::fields`), not a TOML-only definition. Narrowest useful
  step: Value1 as `i32` when `ValueType` = Int (0) and `f32` when Float (1), every other
  variant raw (FormIDs stay opaque as now); corpus re-encode identical. It closes, with
  rules only: `WSPS` on receivers `077F13` (7.5 -> 5.625), `057555` (1.0 -> 1.5625),
  `04A750` (16 -> 12); `ACPT` on magazines `12D03F` (+5 -> +2), `087D4B` (+30, owner to
  confirm), `132C4A`/`132C49`/`132C48` (+8/+4/+16 -> +4/+2/+8). Details:
  `docs/overhaul/build-1.md`, "Why partial or open".
- ~~**Lane E (M3.E9) -> owner.**~~ Answered 2026-10-15 (B8 dropped; B11 base 18; B12 as
  built; `docs/overhaul/build-2.md`). Brief row B8 has no counterpart in the records (no
  automatic receiver OMOD carries a damage property); please restate it
  (`docs/overhaul/build-1.md`, B8). B11 and B12 also need a reading: which magazine
  "18 rounds" means for the Drum Beat, and whether B12's "14" is the Large (already 14
  after this build) or the Extra Large magazine.

- ~~**Plan maintainer -> lane B (round 5): implement ADR 0005 Amendment 5.**~~ Done (lane B
  round 5): `Confidence::ContainerReference` (`container-reference`, after `reference` in
  `Confidence::ALL`; `Confidence::below_corpus` ranks the two together), the loader rule
  (`uesp-skyrim:` citation, TES4/header files only), TES4 DATA/INTV set to it, parity's
  occurrence check both ways with no name list, `reference_members`, `build`'s gate, the
  `schema` limitations and the UI's unconfirmed marker; coverage and `kit schema` counts
  gain a `container-reference` bucket (a column in `docs/record-parity.md`). Was: new confidence
  `container-reference`. It needs a `uesp-skyrim:` citation and is accepted only in the
  container definitions (`TES4`, group header, record header). Steps:
  - Set it on `TES4.Master.FileSize` (`DATA`) and `TES4.TagifyCount` (`INTV`).
  - Drop `UNOBSERVED_ALLOWED` from `xtask/src/parity.rs`. The occurrence check skips
    `container-reference` as it skips `reference`, and fails when a `container-reference`
    member occurs in the corpus.
  - Rank it with `reference` in the engine's coverage counts, in
    `kitkraft_doc::confidence::reference_members` and in `build`'s `allow_reference` gate.
  - Update `crates/schema/defs/README.md`, rerun `cargo xtask codegen` and regenerate
    `docs/record-parity.md`.
  - Close the `docs/gaps.md` TES4 DATA/INTV row in the same commit.
- ~~**Plan maintainer -> lane E (overhaul build 2, M3.E9 rerun).**~~ Done 2026-10-15
  (`docs/overhaul/build-2.md`; owner row 3 now waits on the in-game check). Lane B round 4 typed
  `ObjectModProperty.Value1` (paths `DATA.Properties[n].Value1.Float` etc.). Rebuild
  `docs/overhaul/overhaul.toml` with the `WSPS`/`ACPT` rules named in "Lane E (M3.E9) ->
  lane B (M3.B9)" above, and fold in the owner's B8/B11/B12 answers when they come. Then hand the
  owner build 2 for M3.O5 (STATUS "Owner next actions", row 3).
- **Owner -> lanes H and B (M3.O3 done 2026-10-14).** SF1Edit is installed:
  `C:\Modding\tools\xEdit\xSFEdit64.exe` (file version 4.1.5.0). Lane H: run M3.H4's
  SF1Edit part with `KITKRAFT_SF1EDIT` pointing there, and update the SF1EDIT row of
  `docs/source-registry.md` ("not installed yet"). Lane B: M3.B11 is unblocked on the
  SF1Edit side; record the build's skew from the pin.
- ~~**Policy -> lane D (later, for M4.E11, ADR 0009).** A read-only MO2 instance check in
  `crates/io` (`ModOrganizer.ini`, `mods\`, `profiles\`; reparse points reported) for
  `kit deploy --dev`.~~ Done (lane E round 5, at the director's request):
  `kitkraft_io::Instance` (`crates/io/src/instance.rs`): `read`/`from_ini`, the root, base,
  mods, profiles, overwrite and downloads folders, `gamePath`, `gameName`,
  `selected_profile` (`@ByteArray` decoded), `profiles()`, `folders()` with each folder's
  state and `reparse_points()`; nothing written or followed.
- **Policy -> lane F (later, M4.W0, ADR 0007 Amendment 1).** `data` shape tests for the
  inspection commands and a catalog marking of writing commands, as the gate for lane W.
- **Policy -> all lanes (naming).** Write KitKraft in prose and use only the `kitkraft`
  identifiers: the rename is done (M4.BX1 integration, "Done" and Decision records).
- ~~**Lane E (M4.E11) -> lane D.** `deploy` resolves the MO2 instance itself
  (`crates/engine/src/deploy/instance.rs`: `ModOrganizer.ini` `mod_directory`,
  `profiles_directory`, `overwrite_directory`, `download_directory`, `base_directory`,
  `selected_profile`; links and junctions refused) with `kitkraft_io::ini::Ini` and
  `Profile`. ADR 0009 wants a read-only instance type in `crates/io`; when it exists,
  `Instance::resolve` should call it (the settings logic duplicates part of
  `Profile::read_with`).~~ Done (lane E round 5): `deploy`'s `Instance::resolve` and
  `Profile::read_with` both read the settings through `kitkraft_io::Instance`; the private
  copy is gone (the profile reader now also ignores empty values and takes relative
  folders against the instance root, as `deploy` did).
- **Lane E (M4.E11) -> plan maintainer / integrator.** ADR 0009 (unmerged branch) needs an
  amendment for the brief's layout: `.previous` folder in `mods\` instead of external
  backups (one kept), journal beside the folders, `dry_run` instead of `plan`,
  `kitkraft-deploy.json` (`docs/deploy.md`, "Where this differs"). `docs/mcp.md` could
  link `docs/deploy.md` under "Command results". When M4.E5 `package` lands, check its
  `kitkraft-manifest.json` against what `deploy` verifies (`files[]` of
  `{path, sha256, size}`, optional `plugins[]`).
- **Lane G -> lanes E, F, H (M4 lane G).** (Part 3 answered 2026-10-15 by lane H: `docs/design/persistence.md` Appendix A lists `kit papyrus events` for every name it cites; M4.G2's acceptance test can read that list.) (1) Lane E: `package` (M4.E5's shape) landed in
  lane G (`handlers/package.rs`, `docs/packaging.md`), validating with `validate::run`
  and guarding `out` with `edit::patch::guard_location`; `testplan` (M4.E6) can read the
  folder's `kitkraft-manifest.json` for plugin hashes. (2) Lane F (M4.F1): `package`
  writes files (`destructiveHint`), `papyrus.events` is read-only; both are in the
  `docs/mcp.md` command sections. (3) Lane H (M4.H3): take event names for
  `docs/design/persistence.md` from `kit papyrus events --event <name>
  --declarations-only` (or `--script ObjectReference --kind event`); M4.G2's acceptance
  test that the cited events exist is added once that file is on main.
- ~~**Integrator (M4.BX1) -> lane H (oracle comparator).**~~ Done 2026-10-15 (M3.B11, lane H: enum, flag, code, GUID and byte-text conversions, consistency-checked name pairs; `docs/oracles.md`). Was: After the family merges the
  SF1Edit rerun shows 2,675 disagreements on the small files that are display conversions:
  enum and flag fields (number vs name or flag list), Wwise GUIDs (raw vs `{...}`), structs
  holding opaque or union fields, signature-valued fields. Converting those by kind (the
  schema has the names) would make the counts evidence (`docs/oracles.md`).
- ~~**Lane H -> lane D (strings), from the SF1Edit run (X8).**~~ Done (lane B round 5, at
  the director's request): `kitkraft_strings::Entry::text` shows an entry that is not
  valid UTF-8 as Windows-1252, one char per byte (the table `kitkraft-ba2` uses for
  archive names); bytes, `is_utf8` and `Resolved.bytes` are unchanged, and `strings`
  says the shown text is Windows-1252. Not rerun against SF1Edit. Was: some official string entries
  are Windows-1252, not UTF-8 (`Starfield - Localization.ba2` dlstrings `0x000301B6` holds
  byte `0x92` for an apostrophe); our `text` shows U+FFFD there while xEdit shows `’`.
  A Windows-1252 fallback for non-UTF-8 entries (bytes stay in `Resolved.bytes`) would
  make the text agree; until then the oracle counts it as one disagreement of ours.

- **Lane A -> lane B (answer to the RawSub/XXXX request; re-checked after lane B round 2).**
  `esm::bridge` re-exports the generated `RawSub`/`OwnedSub`/`Ctx` (checked against `main`
  at dcfe253, no local copies): `From<&Subrecord>` sets `oversized` from the `XXXX` prefix,
  `Subrecord::from_owned` writes `XXXX` when `oversized` is set or the payload exceeds
  `u16::MAX`, `Plugin::ctx(record)` passes the record's form version and TES4 `0x80`. The
  `XXXX` inventory is done: 0 of 12,849 have a payload <= 0xFFFF (smallest 65,536), so the
  `docs/gaps.md` row is closed. If the generated shapes change again, `bridge.rs` is the
  one file to adjust.
- **Lane A -> lane C.** The decode side of the typed wiring is in esm: `Plugin::typed(record)`
  / `Record::typed(ctx)` (`None` for a type without a definition or an undecoded compressed
  record: call `Record::decompress` first) and `Record::set_typed`. `coverage` and `inspect`
  can now decode payloads instead of classifying by signature (**`coverage` done in
  M3.F2**; `inspect` is M3.D6, lane D). API changes from round 2
  that engine code may meet: `Body::Compressed` has a third field `inflated` (patterns need
  `..`), `DecodeLimits` has `max_subrecords` (build it with `..DecodeLimits::default()`),
  new `Error` variants (`FlagBodyMismatch`, `ReservedSignature`, `TooManySubrecords`), and
  `driver::rewrite_file` for streaming edits (`RewriteReport::hedr_fix` when records or
  groups are added or removed).
- ~~Lane B -> owner / AGENTS.md maintainer: point AGENTS.md at ADR 0005 Amendment 1.~~
  Done 2026-10-14: AGENTS.md "Clean room" now states the Amendment 2 hierarchy, with UESP's
  Starfield pages at level 4.
- ~~**Policy -> lane B.** Store `confidence` in the model, `corpus` on TES4/GRUP/RECORD,
  required on `uesp-skyrim:`, re-export the pin.~~ Done in M2.B1 (lane B branch):
  `Provenance` on every node, `confidence` required on every file, `XEDIT_REVISION`.
- ~~**Policy -> lane C.** `coverage` / `cargo xtask parity` should show `confidence`;
  `reference` values should surface in `inspect`/`query` limitations.~~ Coverage and parity
  half done: M2.F9 ("Definitions by confidence") and M3.F2 (`decoded_by_confidence` per
  record type, "Decoded through the definitions"); the `inspect`/`query` half is M3.D3
  (lane D), see the next-but-one entry.
- ~~**Policy -> lane B (owner approval 2026-10-14).** Accept `wbDefinitionsCommon.pas` in
  `xedit:` citations.~~ Done in M2.B1 (loader, tests, README).
- ~~**Policy -> AGENTS.md / plan maintainer.**~~ Done (lane F round 3): AGENTS.md, ADR 0001
  Amendment 1 and `plan/execution-plan.md` (M2.O1, M2.O2, M2.B1 confidence names) updated.
  Was: AGENTS.md "Clean room" ("Other xEdit files are
  not covered yet") and owner decision 2 ("extending the reference beyond
  `wbDefinitionsSF1.pas`"), ADR 0001 Amendment 1 (only the SF1 file), and
  `plan/execution-plan.md` M2.O1/M2.O2 should reflect the approval (the common file at the
  same SHA; O1 and O2 done). Also: M2.B1 names confidence values
  `confirmed`/`corpus-consistent`/`hypothesis`, while ADR 0004 and the loader use
  `reference`/`corpus`/`ck-diff`/`verified`; the ADR wins.
- **Policy -> lane C (partly done, lane F round 3).** `cargo xtask parity` now has a
  "Definitions by confidence" table (from `kit schema --json`), and `kit schema` /
  `record_schema` states `reference`-confidence definitions as a limitation; `inspect` and
  `query` results do not yet. **Done (lane D, M3.D3):** `inspect` and `query` name the
  `reference`-confidence members behind the values they show or test.
- **Policy -> M2.** Install SF1Edit (matching the pin if possible, else record the build) for
  `query` results do not yet (M3.D3).
- **Policy -> M2 (now M3.O3).** Install SF1Edit (matching the pin if possible, else record the build) for
  the differential comparison in ADR 0005 Amendment 2 §2a; never bundle it.
- **Lane B -> owner (default chosen).** Inventory definitions name members after their
  signature (`Wfir`, `Flld2`); renaming them once meanings are sourced (`ck-wiki`)
  is a breaking change for `kit query`. Default: accept the rename when it happens. Done
  for the universal subrecords in M2.B2: `Edid` is now `EditorId`, `Full` `Name`, `Desc`
  `Description`, `Ksiz`/`Kwda` the group `Keywords { Count, Forms }`; GMST `Data` is
  `Value`, GLOB `Fltv` is `Value`.
- **Lane B -> lane A (FYI, done in lane B's branch).** The M2.B2 rename broke one line of
  the bridge test (`gmst.edid` was opaque bytes); lane B changed it to
  `gmst.editor_id ... == b"fFoo"` in `crates/esm/src/bridge.rs` so the gate stays green.
  Generated API additions you may use: `Record::extra()`, the four visitors, and union
  enums (`GmstValue`) decoded through `put_by_prefix` (a missing EDID keeps DATA raw).
- ~~**Lane B -> lane F / coverage owner (FYI, done in lane B's branch).** The engine coverage
  test now counts EDID, FULL and GMST DATA as `typed` (expectations updated in
  `crates/engine/src/handlers/coverage.rs`). For M2.F9: per-member confidence is
  `SubrecordDef::provenance.confidence` (also on groups, fields, records); a union member
  is `FieldType::Union`, not opaque.~~ Used in M2.F9 (`schema`) and M3.F2 (`coverage`).
- ~~**Lane D -> lane F.**~~ Answered in lane F round 3 (below). (1) `inspect`'s `plugin` is no longer required, so it is no longer
  positional: `kit inspect <form> --plugin <file>`; the tests that used the positional form
  (engine `cmdline.rs`, kit-cli `exit_codes.rs`, mcp `tests/stdio.rs`) were updated. (2)
  `query` is implemented: tests that used it as the `Planned` example now use `build`
  (engine `lib.rs`, kit-cli `main.rs` and `exit_codes.rs`, mcp `server.rs`); mcp
  `backend.rs`'s null-args test now expects `load`'s `failed`; kit-cli `tests/mcp.rs`
  loads `Starfield.esm` + `Constellation.esm` for `query_records`. (3) `PLANNED_IN_M2`
  entries `load`, `query`, `diff` are stale. (4) M2.F7 step 4: `query_records` answers
  now, and `kit mcp --profile` preloads through `load`. (5) New catalog entry `conflicts`
  (alias `esmscan`); the MCP tool set may want a `conflicts` shortcut. **Lane F round 3:**
  (3) pruned (`PLANNED_IN_M2` is gone); (4) in the M2.F7 steps; (5) not added: `execute`
  reaches it, and the tool list stays small (8 tools with `record_schema`).
- **Lane D -> owner (M2.O4).** In one session: (a) MO2's left pane for `StarUI Inventory`
  vs its Free Lanes patch (which mod MO2 shows providing `Interface\inventorymenu.swf`);
  (b) in game, `help` or a console click on a Constellation.esm object should show an
  `FE000xxx` ID and a full plugin's object `<slot>xxxxxx` (`kit inspect <form> --profile
  ...` prints the runtime ID it assumes; `inspect` now needs `runtime:<hex>` for a runtime
  ID); (c) whether `HG_ShipCargoHoldAtHome.esm`'s records (stored `02xxxxxx` with one
  master) exist in game; (d) where the game loads `ShatteredSpace`, `SFBGS004`, `SFBGS00D`,
  `SFBGS050`, `BlueprintShips-SFBGS050` and `SFBGS047` (KitKraft now assumes right after
  the `Starfield.ccc` plugins, `hardcoded-official-slots`): the runtime IDs of their objects
  in game would show their slots. Lead: the CK's own `EditorDataFilesLoaded.txt` in the game
  dir lists each loaded file with its index and ID form (`001 - (FE000XXX) -
  'KZ_Mantis.esm'`); a CK session with the full profile would record the CK's order.
- **Lane D -> lane H / owner (M3.O2 script, from M3.D6).** BlueprintShips-SFBGS050.esm is
  localized but ships no string tables; its 66 LCTN and 30 GBFM names (FULL) resolve
  nowhere in KitKraft. In game, a location from it (for example the first unresolved LCTN
  `kit inspect` lists) would show whether the game reads its names from SFBGS050's tables
  (30 of the 96 IDs exist there) or shows none. **Lane H:** in the M3.O2 script
  (`docs/ck-observations.md`, step 3) with `BlueprintInteriorSFTER_TerranShip_Generic_TESTLocation`
  (`FULL` 0x0002E9C4; none of the 66 LCTN name IDs is in `sfbgs050_en.strings`).
- **Lane D -> lanes E, F (review round 2 API changes).** A deleted version is
  `RecordLoc.flags & kitkraft_doc::flags::RECORD_FLAG_DELETED != 0` (`inspect` and `query`
  show `deleted`), for lane E's refusal of deleted sources. `Version` has `basis`
  (`kitkraft_doc::Basis`: `Direct`, `FullStyleSelf`, `FullStyleSelfMasked`);
  `MasterMap::resolve_detailed` returns it instead of a bool. `ResolveError` gained
  `MasterNotLoaded`, `PluginNotLoaded` and `Null` (raw `00000000` never resolves);
  `FormRef::parse` refuses object ID 0. `inspect` reads bare hex only with `plugin`; a
  runtime ID is `runtime:<hex>` (front ends and examples that pass bare hex runtime IDs
  need the prefix). Index cache format 3: old entries rebuild once. For lane E's
  `verify_load_order`: the Default profile now has no late master, and the "active plugins
  with no file" limitation is now in `Loaded::limitations` (it was only in `load`'s own
  result before, so the `NO_FILE` check could not see it); whether `set`/`build` can drop
  the "unverified" refusal is lane E's call.
- **Lane D -> lane B.** 14 official GMST string settings hold string ID 0, which is in no
  table; `query` leaves them unknown. A source for what ID 0 means would close it.
  `where` member names are the schema's PascalCase names, matched case-insensitively.
- ~~**Lane E -> lane F (tests touched, catalog closure).**~~ Done in lane F round 3 (see
  the end of this entry). `set`, `validate` and `build`
  are `Implemented`, so no catalog command is `Planned` any more. Tests that used `build`
  as the Planned example were changed in lane E's commit 714bfe6: engine
  `command_without_handler_is_not_implemented` (an engine without the handler),
  `apps/kit-cli` `exit_code_follows_status_and_passed` (a Planned spec built in the test),
  `apps/kit-cli/tests/exit_codes.rs` `failing_command_exits_2_with_clear_message` (missing
  spec / plugin), `crates/mcp/src/server.rs` (build with a missing spec is `failed`).
  `PLANNED_IN_M2` still lists `set`, `build`, `validate` (stale; prune at will).
  `validate`'s `plugin` is optional, so it is a flag (`kit validate --plugin X.esp`), as
  lane D did for `inspect`. **Done (lane F round 3):** `PLANNED_IN_M2` removed; the
  closure test now fails on any `Planned` command.
- **Lane E -> lane D (round 2).** (1) `set`/`build` refuse a load order with failed
  plugins, late or missing masters, or active plugins without a file ("load order
  unverified"); the last is found by the text of `load`'s limitation ("active plugins with
  no file"). Please expose the missing active plugins as a field of `Loaded` and say when
  the winning override is a safe source again, so lane E can drop the blanket refusal. The
  owner's Default profile is refused today: `stroudpremiumedition.esm` loads before
  `sfbgs00d.esm` and `SFBGS047.esm`. (2) `strings::Resolved` has only lossy `text`; lane G
  notes some official strings are not UTF-8. `set` refuses such a string (a U+FFFD in the
  text) rather than change it; a `bytes` field would let it copy them exactly.
  **Done (lane D, M3.D1):** (1) `Loaded::missing_active_plugins` (the limitation text stays
  for readers, but nothing needs to match it) and `Loaded::source_check(form)`
  (`kitkraft_doc::SourceCheck { winner, blocking, unrelated }`, `is_safe()`): refuse on
  `blocking`, report `unrelated` as limitations (M3.E1). (2) `Resolved::bytes` and
  `Resolved::utf8` (M3.E2).
- **Lane E -> owner (M2.O4).** The `ck-diff` half of M2.E3 is open: what the CK writes for
  a new non-localized plugin (master flag for `.esm`, `HEDR` next object ID, inline text
  encoding: we write UTF-8). Until observed it is a limitation on every `set`/`build`.
- **Lane F -> lanes D, E, G (session and catalog closure).** A handler that needs the
  load order is a `fn(&mut Session, &Value) -> Result<Outcome, EngineError>` added to
  `handlers::session_defaults()` (stateless handlers stay in `defaults()`). Keep your
  typed state in a slot (`Session::insert` / `get` / `get_mut`); `load` also sets
  `Session::set_load_order(Some(listing))`, which `kitkraft://load-order` serves as is.
  When you flip a command to `Implemented`, removing its `PLANNED_IN_M2` line in
  `crates/engine/src/lib.rs` is optional (a stale entry only prints); a command you will
  not finish in M2 becomes `Status::Deferred { reason, milestone: "M3" }`.
- **Lane G -> lane D (M2.G8 landed; for M2.D7).** `kitkraft-strings` API:
  - `Kind::{Strings, DlStrings, IlStrings}`, `Kind::from_path(&Path)`,
    `Kind::from_extension(&str)`, `kind.extension()`;
    `table_name("Starfield.esm", "en", Kind::Strings) == "starfield_en.strings"`.
  - `Table::parse(kind, &bytes) -> Result<Table, Error>`, `Table::read_file(&Path)`;
    `table.get(id: u32) -> Option<&Entry>`; `Entry { id, bytes, offset: Option<u64> }`
    with `entry.text() -> Cow<str>` (lossy UTF-8), `entry.is_utf8()`,
    `entry.stored_len(kind)`; `offset` is the absolute offset in the table file (evidence).
    `Error` variants carry byte offsets.
  - Tables in archives: `kitkraft_ba2::Archive::open(&path) -> (Archive, File)`,
    `archive.find("strings/starfield_en.strings") -> Option<usize>` (case-insensitive,
    either separator), `archive.extract_vec(&mut file, index, cap)` (round 2: `cap` in
    bytes, e.g. `256 << 20`; above it `Error::TooLarge`). Corpus: base-game tables are
    in `Starfield - Localization.ba2`; the DLC and Creation tables are in
    `<Plugin> - Localization.ba2` or `<Plugin> - Main.ba2`; names are lower-case
    `strings/<plugin>_<lang>.<ext>`, languages `de en es fr it ja pl ptbr zhhans`. In
    `starfield_en` the ID sets of the three kinds are disjoint (FULL is in `.strings`).
- **Lane G -> lanes D, E (string tables, review round 2).** Compare and copy strings by
  `Entry::bytes`, not by the lossy `text()`: a few entries of the English tables are not
  UTF-8 (cp1252-style bytes), and `text()` replaces those bytes, so a round trip through
  it changes them. A table may repeat an ID: `Table::get` and `strings.merge` both use the
  **first** occurrence (merge reports the others in `duplicates`, apart from cross-table
  `conflicts`); use `get`, not a map built by later-wins insertion.
- ~~**Lane G -> lane F (FYI, review round 2).**~~ Answered in lane F round 3 (end of this
  entry). `ba2.extract` gained `max_entry_bytes` and
  `ba2.pack` gained `overwrite` in the catalog; the expected parameter list in
  `crates/engine/src/params.rs` `required_first_in_required_order` was updated to match.
  Also seen once in a full `cargo xtask ci` run after merging lane D (passed on rerun and
  in isolation): `apps/kit-cli/tests/mcp.rs` `kit_mcp_on_an_official_file` read EOF from
  the `kit mcp` child (it exited mid-session); worth a look if it recurs. **Lane F round
  3:** not reproduced (20 runs, and 4-way parallel cold-cache stress); the spawned servers
  now use a private index cache under the test target dir instead of the user's (shared
  with every other worktree's gate across cache format changes), stderr is drained, and an
  EOF fails with the child's exit status and stderr.
- **Lane G -> lane D (FYI, done in the lane G merge).** `Archive::extract_vec` takes a
  byte cap now; `crates/doc/src/strings.rs` passes 256 MiB (the `strings.dump` cap). An
  entry above it is `kitkraft_ba2::Error::TooLarge`, reported like any other table error.
- **Lane G -> lane E (for M2.E8).** `kitkraft_pex::Pex::parse(&bytes)`; calls are
  `Instruction`s with `opcode_name(op)` `callmethod` / `callstatic` / `callparent`, the
  function name in `args[0]` (`Value::Identifier`, resolve with `pex.string(i)`), the base
  object in `args[1]`; `.pex` inside archives via `kitkraft_ba2` as above, or
  `execute("papyrus.disasm", {pex, archive?, format: "json"})`.
- ~~**Lane G -> lane F.**~~ Answered in lane F round 3 (end of this entry; (1) is moot: no
  catalog command is `Planned`, and those tests build their own spec). (1) Tests that used `ba2.list` as their example of a `Planned`
  command now use `query` (engine `lib.rs`, `apps/kit-cli` `main.rs` and
  `tests/exit_codes.rs`, `crates/mcp/src/server.rs`); they break again when `query`
  lands, so a spec built in the test may be better. (2) `papyrus.disasm` puts the
  multi-line disassembly in `data.summary` so `kit` prints `pexdis.py`'s output; if `kit`
  should prefer a `data.text` member instead, say so. (3) `PLANNED_IN_M2` still lists the
  seven lane G commands (now `Implemented`; prune at will). (4) The status paragraph at the
  top of `docs/mcp.md` still lists only `roundtrip`, `coverage` and `inspect`. **Lane F
  round 3:** (2) `data.summary` stays (`kit schema` does the same); (3) and (4) done.
- ~~**Lane C -> lane B.** Mark a generated region for `BUILTIN_DEFINITIONS`.~~ Done
  (lane B, M2.B2 commit): the entries sit between
  `    // BEGIN generated by cargo xtask codegen: BUILTIN_DEFINITIONS` and
  `    // END generated by cargo xtask codegen: BUILTIN_DEFINITIONS` in
  `crates/schema/src/lib.rs`, one rustfmt-formatted entry per file in file-name order
  (top level, then `common/`). xtask (lane F) may rewrite exactly those lines;
  `tests/defs.rs` checks the markers and stays the guard until it does.
- **Lane A -> lane D (M2.A1, M2.A2 landed).** `kitkraft_esm::index`:
  `index_file(&Path, &IndexOptions) -> Result<PluginIndex>` (also `index_bytes`,
  `index_reader`). `PluginIndex { file_len, header: Record<'static> /* TES4, parsed */,
  records: Vec<RecordLoc> /* file order, TES4 first */, groups: Vec<GroupLoc> }`;
  `RecordLoc { signature, form_id /* as stored */, flags, form_version, offset, data_size,
  group: Option<u32> /* innermost group, index into groups */, edid: Option<Box<[u8]>> }`;
  `GroupLoc { header: GroupHeader, offset, size, parent: Option<u32>, depth }`;
  `PluginIndex::group_path(Option<u32>)`, `group_headers(&RecordLoc)`. Masters and flags:
  `tes4::Tes4::new(&index.header)`. Random access: `read_record_at(&mut R: Read+Seek,
  offset, Option<&DecodeLimits>) -> Result<Record<'static>>` and `read_indexed(&mut R,
  &RecordLoc, ...)` (header checked against the entry: `Error::IndexMismatch`; a group
  offset is `Error::NotARecord`). `INDEX_FORMAT_VERSION` (1) for the M2.D5 cache key. The
  index is not serializable yet (no serde in esm); say if D5 wants a byte format from
  esm instead of its own.
- **Lane A -> lane E (M2.A5 landed; review follow-ups 1, 3, 8, 9 in).**
  `kitkraft_esm::build`: `Plugin::create(&HeaderSpec)`; `HeaderSpec::new(masters)` fills
  corpus-observed defaults (flags 0, form version 582, `HEDR` 0.96, next object ID
  0xFF001000) that M2.E3 should confirm or change; masters are written exactly as given
  (`MAST` + 8-byte zero `DATA`), nothing inferred. `Plugin::upsert(&[GroupHeader],
  Record) -> Result<Upsert /* Replaced | Inserted */>`: the path is
  `PluginIndex::group_headers(&loc)` of the source record; a record under cell or world
  children needs its parent CELL/WRLD record upserted first (a missing children group goes
  right after the record whose FormID is its label, else at the end). Streaming:
  `upsert_file_to_path(in, out, &RewriteOptions, Vec<Placed>, |tes4| ...)` (temp file +
  rename, `HEDR` patched, `set_masters` for the header edit), `upsert_reader` for a
  `Read + Seek` input. `driver::rewrite_file_to_path` applies the `HEDR` fix itself.
  `Record::typed(ctx) -> Result<TypedRecord, NotTyped>` (`NoDefinition` | `NotDecoded`);
  `Plugin::typed(&record, &DecodeLimits) -> Result<Option<TypedRecord>>` decodes
  compressed records on the fly. `Subrecord::from_owned` / `set_typed` choose `XXXX` by
  size; unchanged subrecords keep their bytes. FormIDs are copied as stored; remapping is
  E4's (via B3). M2.A6 edit API (decodes a compressed record on first edit, recompresses
  only if changed): `Record::find_subrecord(sig, nth) -> Option<usize>`,
  `set_subrecord(index, data, &DecodeLimits) -> Result<bool /* changed */>`,
  `insert_subrecord(index, Subrecord, &limits)`, `remove_subrecord(index, &limits)`,
  `edit_subrecords(&limits) -> Result<&mut Vec<Subrecord>>`; out-of-range is
  `Error::NoSuchSubrecord`.
- **Lane A -> lanes D, F (M2.A3 landed).** Typed decode check: `Record::check_typed(ctx)` /
  `bridge::check_subrecords(sig, &subs, ctx) -> Result<TypedCheck, NotTyped>`
  (`subrecords`, `placed`, `conflicts: Vec<TypedConflict { index, signature, offset,
  kind: NotDecoded | Reencoded }>`); `RoundtripOptions { typed: true, .. }` fills
  `Stats::{typed_records, typed_subrecords, typed_unchecked, typed_failed_records,
  typed_conflicts, typed_conflict_list}` for `coverage` and `cargo xtask parity` (M2.F9).
  On all 14 files today: 13,168 typed records, 0 conflicts (`docs/corpus.md`).
  **Lane F (M3.F2):** answered for `coverage`, which runs the same decode + re-encode check
  per record (with `Record::extra()`, see below) and feeds `cargo xtask parity`.
- ~~**Lane A -> lane B.** Please generate an accessor for the raw subrecords a typed record
  kept: `generated::records::Record::extra(&self)`.~~ Done in M2.B3 (`Record::extra()` is
  generated for every defined type).
- ~~**Lane F -> lane A (M3.F2).** `esm::bridge::extra_of` still matches five variants (GLOB,
  GMST, KYWD, TES4, WEAP), so `check_subrecords` misses `NotDecoded` conflicts in AMDL,
  AMMO, COBJ, OMOD and ZOOM (and in every round 3 type); `typed.extra()` covers them all.~~
  Done in lane B's M3.B1 commit (FYI to lane A): `extra_of` returns `typed.extra()` for
  every record type, a one-function change, so `corpus_full_roundtrip`'s typed check sees
  undecoded payloads in every round 3 type. Lane A (M4) then removed `extra_of`:
  `check_subrecords` reads the generated `Record::extra()` directly, and a test checks every
  defined type (merged in M4.BX1).
- **Lane B -> lanes D, E, F (FYI, done in lane B's M3.B1 commit).** Typing WEAP changed five
  engine test expectations, updated in the same commit: `edit::fields` (WEAP WABB is a
  schema lstring now, no table fallback), `edit::patch_tests` (the opaque subrecord in the
  remap refusal test is VMAD, inserted after EDID, instead of WDMG), `handlers::coverage`
  (MODL is typed; counts), `handlers::schema` (EDID cites `xedit:`; VMAD is the opaque
  member checked), `handlers::set_tests` (the `hex:` error now comes from the WDMG struct).
  Member names changed in the shared blocks: `BfcbBlock.Bfcb`/`BfcbBody`/`Bfce` are
  `ComponentType`/`ComponentData`/`EndMarker`; `ObteBlock.Obte`/`ObtsEntry`/`Obts`/`Stop`
  are `CombinationCount`/`Combination`/`TemplateItem`/`EndMarker`; WEAP's placeholder names
  (`Obnd`, `Wfir`, `Flld2`, ...) are replaced by the xEdit-backed layout (`kit schema
  --type WEAP --deep`).
  After merging main (lane B's merge commit), three more expectations followed the typed
  schema: `handlers::coverage` (the M3.F2 `failed` counts with MODL typed; 8 decoded by
  confidence), `handlers::overhaul_tests` (KSIZ is looked up by name: WEAP's numeric
  members now come first) and `tests/diff_corpus.rs` (picks a record type whose definition
  places KWDA, since lstring-only definitions keep it raw).
- **Lane A -> lane E (M4.A1..A6 landed; for M4.E1, E2, E3, E4).** `kitkraft_esm`:
  - `forms::NewForms::for_index(&index, &master_scales)` / `for_plugin(&plugin,
    &master_scales)` / `new(&tes4, &master_scales, own_max)` -> `allocate() ->
    Result<u32>`, `write_hedr(&mut tes4)`; `master_scales: &[Option<tes4::Scale>]` is
    each master's scale in master-list order (`Scale::of_flags(master_tes4.flags())`, or
    from lane D's load order); the own prefix is the **full**-master count, and an
    unknown (`None`) scale, a wrong-length list or a small/medium target is
    `Error::Refused` (fix round, review blocker: masters [Starfield full, Constellation
    small, SFBGS003 medium] give 01xxxxxx). One allocator per plugin per edit session
    (not `Copy`). Name `forms.limitations()` in results (`LIMITATIONS` plus a low `HEDR`
    next object ID and the blueprint flag's `HEDR` count meaning).
    `forms::names_master(form_id, &master_scales) -> Option<bool>` classifies a stored
    FormID.
  - `refs::compose_reference(&RefSpec, allow_reference: &[&str]) -> Result<ComposedRef>`
    (`RefSpec::new(sig, form_id, base, position, rotation)`, then `persistent`,
    `initially_disabled`, `editor_id`, `extra`...); `ComposedRef { record, position,
    members, limitations }`; `composed.placed(place, cell, &style) -> Result<Placed>`
    (`&mut self` since the fix round: a position outside its exterior cell adds a
    limitation; non-finite numbers, a NUL in the EditorID and `EDID`/`NAME`/`DATA`/`XXXX`
    in `leading`/`extra`/`trailing` are refused). Until BW's
    `REFR`/`ACHR` definitions reach `main` through BX, `DATA.Position.Z` and
    `DATA.Rotation` are below `corpus` and must be in `allow_reference` (nothing to change
    when they land). They have landed (M4.BX1): both come from the REFR/ACHR `DATA`
    definition at `corpus`.
  - `cells::cell_place_in(&index, &loc, &cell_record, &limits) -> Result<CellPlace>`
    (interior / world persistent cell / exterior with its `XCLC` grid),
    `cells::persistent_cell(&index, world) -> Option<&RecordLoc>` (where a persistent
    reference in a world goes), `cells::Grid::of_position(x, y)` (cell of a position,
    100 units), `cells::placement(&header, &Parent, &style)` for any record under a cell,
    quest or topic; `Plugin::upsert_styled` / `RewriteOptions::placement` place in corpus
    order. A placed reference needs its parent `CELL` (and `WRLD`) carried in the plugin
    first: since the M4 placement-core fix round, `upsert_styled` and the streaming upsert
    refuse (`Error::Placement`, "carry the WRLD/CELL override first") a children group
    (types 1, 6, 7, 10) whose record is not in its parent group; `cells::carry_parents(
    &master_index, &place, cell, &style, &limits, read)` gives the lossless `WRLD`/`CELL`
    overrides to upsert first (skip those the target has). Before the fix the group was
    created without its record, a shape no official file has. **Lane E:** the engine's
    `edit::source::PARENT_LABELLED` is `[1, 6, 7]`; a copied `DIAL`/`SCEN`/`DLBR` (or an
    `INFO`, whose `DIAL` sits in type 10) also needs its `QUST`, so add 10 there, or such
    a patch is now refused instead of getting an ownerless quest children group.
  - `navmesh::import_navmeshes(&ck_plugin, &mut target, &NavmeshOptions { style, limits,
    master_scales })` and `navmesh::navmesh_placements(&ck_plugin, &target_index,
    read_cell, &options)` for `[[import]]`; refused unless both plugins have the same
    masters. `NavmeshImport::master_forms` lists imported NAVMs that override a master's
    (ONAM candidates), `unknown_forms` those a missing scale leaves undecided.
  - `CellPlace::Interior { labels }` (fix round): an existing interior cell keeps the
    block/sub-block it was read in (`cell_place_in`); `CellPlace::INTERIOR` computes them
    for a new cell. `PlacementStyle::detect(&index)` reads a target's style back.
    `Upsert::Moved`: a reference whose flag 0x400 changed leaves its cell's other
    children group (engine's patch report says "moved"). Paths are checked against flag
    0x400 and a second persistent `CELL` per world is refused.
  - ONAM (M4.A6): when a patch overrides records in groups 7, 9 or 10, set
    `Tes4::set_overridden_forms(&mut tes4, &cells::onam_forms(&plugin, |id| <id is a
    master's and the master has it>))` (`cells::onam_forms_in(&index, pick)` for a
    streamed patch); patches that only override top-level records or persistent
    references still need none. INCC stays yours (as M2.E3).
- **Lane E -> lanes D, F (M3 round 1).** (1) Lane F: new catalog command
  `overhaul.scaffold` (alias `overhaul`, session handler registered in
  `handlers::session_defaults`), `build` gained `plan`, `validate` gained
  `plugin_archives` and `.ba2` inputs in `scripts`; `docs/mcp.md` has their sections, the
  command list (M3.F3) may want them. **Lane F:** (1) answered: the sections are the
  command list; no shortcut tool (`execute` and `command_line` reach it, and the tool
  descriptions and `kitkraft://commands` list it from the catalog). (2) Lane D: `build`'s `via` follows typed FormID
  members through `VersionSubject::lookup` locally (`handlers/build.rs` `follow`); M3.D4's
  `follow(form, member path)` should replace it when it lands. M3.E1/E2 wait for M3.D1.
  **Done (lane D, M4.D1):** `build` calls `kitkraft_doc::links::follow`; the local one is
  gone.
- ~~**Lane E -> lane B (round 2).** `crates/engine/src/edit/lstrings.rs` is a corpus-built
  stand-in for lstring typing the schema lacks (105 signatures, e.g. ACTI/FLOR/FURN/NPC_
  `ATTX`, NPC_ `SHRT`, INFO `NAM1`, MESG `ITXT`, QUST `CNAM`, WEAP `WABB`). As round 3
  transcribes definitions from the pinned xEdit file, please type these as `lstring`.~~
  Done in lane B's M3.B8 commit: all 105 are schema `lstring` members (`corpus`, citing
  `lstring_table_matches_the_corpus`, plus the xEdit line for 103 of them; CHAL `DESC` is
  not in xEdit's CHAL definition, the corpus wins). `crates/schema/tests/defs.rs`
  `lstring_members_cover_the_corpus_table` keeps a copy of the 105 pairs. **Lane B -> lane E
  (M3.E4):** `layout_for` no longer reaches the table fallback for any of them; tests
  updated in the same commit (`edit::fields` layouts, `edit::patch_tests` untyped lstrings:
  ACTI `ATTX` now comes from the schema, no table limitation; `handlers::schema` uses ARMO
  as the undefined type). **Lane B -> lane A (FYI):** CELL and REFR now have lstring-only
  definitions, so `bridge` and `driver` tests use NAVM as the undefined type and count CELL
  and the three REFR as typed records.
- **World schema lane -> lane A (child-group placement).** The world family is typed
  (`docs/schema/world-family.md`). To add cells and placed references, the builder needs
  to derive the group path itself: interior CELL -> top `CELL` group, type 2 block and
  type 3 subblock (labels from the FormID's object part, mod 10 and div 10 mod 10: 17,019
  of 17,020 official cells; the exception is a small-master FormID), then the CELL, its
  type 6 children right after it, then type 8 (persistent) or 9 (temporary); exterior
  CELL -> the world's type 1 group, type 4 block (Y div 32, X div 32 of `XCLC`) and type 5
  subblock (div 8), both labels two i16 with Y first (22,682 of 22,682), then as above;
  NAVM goes in type 9. Type 8 and 9 occur in either order inside type 6 (most cells have
  type 9 only). The full corpus counts and the asserted rules are in the doc and in
  `crates/esm/tests/world_family.rs` (`world_family_full`).
- **World schema lane -> lane B (schema owner).** CELL, REFR and ACHR need an `unordered`
  record mode (match members in any order, keep the input order for encoding): with the
  ordered matcher 889,512 REFR, 6,434 ACHR and 502 CELL records keep some subrecords in
  `extra` (counts per signature in the member notes). Until then the definitions use the
  best linear order. Also, the world family adds shared types `PositionRotation`,
  `OwnerData`, `EnableParent`, `ColorRgb`, `ColorRgba`, `PropertyValue`, `RagdollBone` and
  groups `LinkedReferences`, `GroupedPackIn`; ~~if round 3 adds equivalents (`Vec3`, `Bool8`
  exist in the round 3 worktree), fold them together at integration.~~ Done in M4.BX1 (WORLD
  merge): PositionRotation's position and rotation and ObjectBounds' corners are `Vec3`,
  OwnerData's NoCrime is `Bool8`. The unordered mode is done too (M4.BX1, integrator,
  2026-10-14; see Done).
- **World schema lane -> lane D.** `kitkraft_doc::typed::field_type` picks a layout by
  signature, which is ambiguous for REFR `INAM` (patrol idle FormID vs pack-in u16) and
  wrong for subrecords inside BFCB bodies that share a member's signature (WRLD `PNAM`,
  `NAM2`..`NAM9`). `crates/doc/tests/typed_corpus.rs` now maps each subrecord to the member
  the generated matcher placed it in (`match_members` hits); `inspect`/`where`/`diff`
  should do the same. Done in M4.BX1 (positional layouts, `typed::placements`/`layout_at`);
  `typed_corpus.rs` uses them.
- **World schema lane -> lane A (bridge).** `esm::bridge::extra_of` still lists five
  variants; the seven world types also need `typed.extra()` for `check_subrecords` to report
  `NotDecoded` (lane F's request above). `world_family.rs` uses `Record::extra()` directly.
  Done: `extra_of` returns `typed.extra()` for every type (lane B round 3, merged in M4.BX1).
- **Integrator (M4.BX1) -> lane E.** `formid.dangling` flags object ID 0x14 in reference
  slots whose targets include PLYR (13 errors in Starfield.esm, now `KNOWN_ERRORS`); if a
  source confirms 0x14 is the engine's player reference, resolve it there. **-> owner/lane H
  (CK check):** REFR `XPLK` references in SFBGS00D, SFBGS050 and ShatteredSpace are stored
  one file index above the plugin's own (38 occurrences); does the CK write or read them
  that way?
- **Lane H -> lane E (and the `docs/corpus.md` "TES4 header" text).** Our patches write
  `TES4` as `HEDR CNAM MAST DATA INCC`, but no official file has a `DATA` after `MAST`: a
  walk of all 14 `TES4` records (13 with masters) finds none, as `crates/schema/defs/TES4.toml`
  says ("MAST is never followed by DATA"), while `docs/corpus.md` says "Every MAST is
  followed by an 8-byte DATA". Spriggit 0.41.0 drops our `DATA` on a round trip
  (`docs/oracles.md` S3). Suggest: correct the `docs/corpus.md` sentence, and either stop
  writing `DATA` or list it as a stated deviation in "Writing rules" until M3.O1's F01/F02
  show what the CK writes (`docs/ck-observations.md`). Changing it changes the rename
  plugin's SHA-256.
- **Lane H -> lane A (M3.A1).** File the F07 recompression candidates (up to 5, CK-openable
  in a dialog) as a Request here; lane H adds them to F07 in `docs/ck-observations.md` and
  the `observe new F07` template.
- **Lane A -> lane H (M3.A1, F07 candidates for M3.H1).** Five compressed records from
  `Starfield.esm` that zlib-rs level 9 does not recompress identically and that the CK
  opens in a dialog, for F07's "one trivial edit each" (all class E in `docs/gaps.md`:
  the last match differs). Each override then shows whether the CK writes the original
  stream, ours (level 9), or a third; compare the CK's body with the original and with
  `deflate(level 9)` of the same decompressed bytes (and say whether the CK recompressed
  the edited record at all, or stored it uncompressed).
  1. NPC_ `0000697A` `UC_NA_CL_TolimanScene_Friend` (3,858 bytes inflated, 1,468 stream;
     the typical NPC_ case: last match 35 at distance 38, ours 37).
  2. NPC_ `002BF312` `Traits_OctopedeA_HornedCrab_Small` (645 / 343; the smallest NPC_
     miss).
  3. NPC_ `00196BB2` `PCM_Porrima_Porrima-III_Predator01` (1,379 / 667; the changed
     distance also changes the Huffman tables, so the streams differ from byte 4).
  4. CELL `0027459B` `LC125SonasHome` (interior cell; 330 / 270; last match length 4).
  5. CELL `000090AF` `SM015DeimosStaryardExterior` (36,764 / 12,614; the largest miss
     outside NAVM).
  The 12 level-8 NAVM and the unexplained NAVM (`001BA29F`) are navmeshes of cell
  `00000025`, not dialog-editable, so F07 does not cover them.
- ~~**Lane A -> lane F (M3.A2 landed; for M3.F1).**~~ Done in M3.F1 (Done, "Lane F,
  M4"): `roundtrip --decode`, exit 1 end to end. `RoundtripOptions::decode:
  Some(DecodeLimits)` (or `RoundtripOptions::with_decode()`) is the driver half of
  `roundtrip --decode`: a compressed body that does not inflate to exactly its declared
  size, whose stream lacks its end marker or checksum or is followed by other bytes, or
  whose subrecords do not parse or re-encode, comes back as `report.mismatch` (so
  `identical()` is false and the CLI's exit 1 applies) with `expected` and `actual` both
  `None`, `offset` where inflating stopped (the body start for size and content faults),
  and `path` ending in ` > compressed body: <fault>` (e.g. `GRUP LAND (type 0) @330 >
  LAND 00000501 @439 > compressed body: invalid zlib stream: ...`). A decode limit
  (`DecompressedTooLarge`, `TooManySubrecords`) is still an `Err`. No public type changed
  (`Mismatch` and `RoundtripReport` have the same fields), so the engine compiles as is;
  for an end-to-end exit-1 fixture, a compressed record whose last stream byte is flipped
  (Adler-32) is the simplest (`driver::tests::decode_reports_a_corrupt_well_sized_stream`).
- ~~**Planet schema lane -> lane D.** `kitkraft_doc::typed::field_type` looks a layout up by
  signature, so it cannot tell a top-level member from the same signature at another
  position: PNDT has CNAM, FNAM and GNAM twice with different layouts (it returns the
  "ambiguous" error), and the component blocks of PCBN, PKIN and GBFM contain NAM1..NAM5,
  VNAM, CNAM and ANAM that mean something else at the top level (it decodes them with the
  top-level layout). `typed_reader_agrees_with_generated_and_lstrings_resolve_all_files`
  (`#[ignore]`) now panics at `Starfield.esm PKIN:0000B065 VNAM` (a 1-byte VNAM inside the
  placement-properties component). Please resolve positions with
  `generated::records::match_members` (the decoder's own placement) and skip `any_until`
  bodies. Until then the GBFM object-template FULL is left out of the schema (it made the
  small-file test fail on SFBGS004 GBFM FE000046).~~ Done in M4.BX1 (positional layouts in
  `kitkraft_doc::typed`, see Done); the GBFM object-template FULL can be typed again
  (family lane BP).
- ~~**Planet schema lane -> lane E.** (1) `edit::fields::layout_for` has the same
  signature-only limit (PNDT CNAM/FNAM/GNAM return the "different layouts" error, so
  `inline_lstrings` refuses to copy a PNDT record from a localized source). (2)
  `validate_corpus_official_files` (`#[ignore]`) now reports errors on official files,
  because more FormIDs are typed: 21 `formid.dangling` in Starfield.esm (PKIN `PRPS`
  actor values 0x33D/0x33E, which no file defines; ShatteredSpace has 2 such warnings) and
  1 in SFBGS050 (PNDT `EOVR` entry naming WRLD 0202B2F4, which SFBGS050 lacks). They are
  real corpus facts; decide whether object IDs below 0x800 are engine-reserved (a grade D
  hypothesis worth a CK check) and how the acceptance test treats official danglings.~~ Done in M4.BX1: (1) `edit::fields::layout_at`/`layouts` resolve by
  position; (2) the acceptance test lists the 22 as `KNOWN_ERRORS` with evidence and exact
  counts. Still open as a CK question: whether object IDs below 0x800 are engine-reserved.
- **Planet schema lane -> lane A.** Same as lane F's entry above: `bridge::extra_of` lists
  five types, so `check_subrecords` reports no `NotDecoded` for the 15 planet types either.
- **Planet schema lane -> lane H (CK observation).** Questions only the CK can answer
  (details in `docs/schema/planet-family.md`, "Open questions"): (1) which CK fields map to
  the placement-properties component (ZNAM is unnamed upstream) and how the CK writes it on
  a new PCCN/PCBN/PKIN/LVLP; (2) whether the CK ever writes PCCB child lists or only
  PRTN/PRVN when nodes are added or reordered; (3) how the CK shows the 4 PCCN without PCCC
  and the 5 whose PCCC names a REFR; (4) which of PKIN's two OPDS slots the CK edits;
  (5) which CK checkboxes set the record flags named only upstream (PKIN, GBFM, STDT,
  LVLP); (6) the unnamed values (BIOM BNAM/GNAM slots, PNDT PPBD/HNAM slots, TRAV DNAM
  `Value4`, SFBK NAM1, PKIN FNAM bit 0); (7) RSPJ `NumberCreated`/`Tier`, 0 everywhere.
- **Quest schema lane -> BX (integrator).** Done in M4.BX1 (QUEST merge, see Done): (1) Regenerate the four derived files on merge
  (see Done, quest family). (2) `common/Condition.toml` (PLANET's, which a family lane may
  not edit) keeps CIS1/CIS2 at `reference`, but they occur in this family (CIS1: QUST 14,
  PACK 2, SCEN 1; CIS2: QUST 43, PACK 18, INFO 115, SCEN 34) and decode identically: raise
  them to `corpus` with that evidence, and add the family's 102,280 CTDA (all decode)
  to the Condition evidence. (3) `validate_corpus_official_files` (ignored, release) with
  this family regenerated reports 22 new errors in Starfield.esm, all object IDs below
  0x800 that no official file defines, the class of the PKIN `KNOWN_ERRORS`: SMEN `PNAM`
  = 0x5B on 20 event nodes and QUST `QuestReferenceAlias.ALFR` = 0x14 on 2 aliases (plus
  warnings: SMEN in SFBGS003 and ShatteredSpace, QUST in SFBGS00D and ShatteredSpace, 1
  each). Please list them as known with that evidence. INFO TRDA Emotion was demoted to
  raw in this lane (0xFFFFFFFF sentinel, 53,844 lines). (4) No duplicate shared blocks
  expected from this lane; BC/BI may
  define their own script-attachment block: this lane's is a `type` (VMAD payload), not a
  group.
- ~~**Quest schema lane -> lane B (codegen library, `crates/schema/src/codegen.rs`).**~~
  Done (lane B round 5): the generator no longer mimics rustfmt's widths (one-line
  calls, arms and struct literals, no single-literal special case); `cargo xtask codegen`
  formats every file it writes and the golden tests format `generate()` the same way
  before comparing, so long names and one-member shared groups cannot break the gate.
  Regenerated output and goldens were byte-identical. Was: the
  generator's rustfmt mimicry has two gaps the golden test (`golden_files_are_rustfmt_clean`)
  hits: (1) a shared group with exactly one member emits a one-element `MEMBERS` array
  rustfmt would join onto one line; (2) a call whose arguments exceed rustfmt's call width
  (long member paths in `read`, long field names in `emit`) is not broken. This lane
  worked around both (ScriptAttachment is a type; shorter member names), but other lanes
  will meet them.
- **Quest schema lane -> lane A.** As the planet lane's entry: `bridge::extra_of` lists five
  types, so `check_subrecords` reports no `NotDecoded` for the ten quest family types; the
  out-of-tree check covered it.
- **Quest schema lane -> lane H (CK observation).** Questions only the CK can answer (details
  in `docs/schema/quest-family.md`, "Open questions"): (1) QUST alias ALFG: float or
  integer (official values 0 and the bit pattern 2)? (2) QUST create-matching ALCA: 4 bytes
  in the corpus, 6 upstream; what they hold. (3) QUST collection ALAM: which CK field (0,
  twice 0x40000000). (4) INFO ENAM: which flag set the CK shows for an info group. (5) SCEN
  VNAM (20 bytes, every scene) and INFO INTV (4 bytes after TSCE): which CK fields.
  (6) SMEN PNAM = 0x5B on every event node and QUST ALFR = 0x14 on two aliases: what
  those low object IDs are to the CK (engine-reserved forms?). (7) Whether the CK
  writes a CITC before quest, scene, package-level and response conditions on a new record
  (no official one has it).
- **Actor schema lane (BC) -> integrator (BX).** Done in M4.BX1 (ACTOR merge, see Done; (2) by the engine-reserved ID rule, (3) CIS1 was raised at the QUEST merge, (4) model and destructible folded, PRPS at the ITEMS merge). At the ACTOR merge: (1) add the 9 records
  and `common/{AttackData,Effect,SoundReference}.toml` to `BUILTIN_DEFINITIONS`, the
  `tests/defs.rs` lists and the golden; (2) `validate_corpus_official_files` then reports
  319 new `formid.dangling` errors in Starfield.esm, all explained, none a layout error:
  318 are `PRPS.ActorValue` values below 0x800 that no file defines, the same class as the
  known PKIN ones (NPC_ 139: 0x385 x102, 0x32D x17, 0x330 x7, 0x3A2 x7, 0x39E x5, 0x351
  x1; RACE 178: 0x2D8, 0x2D9, 0x2E0 x31 each, 0x39E, 0x39F, 0x3A0, 0x3A1 x21 each, 0x385
  x1), plus `NPC_:0018B5F4` `RDSA.SpecificReference = 00577823`, a stale value in an
  entry whose flags do not enable it; the DLC files report the same actor values as
  warnings. Please list them in `KNOWN_ERRORS`. (3) `common/Condition.toml`'s CIS1 is
  `reference`, but one official PERK condition carries a 40-byte CIS1 that decodes; it can
  be raised to `corpus`. (4) Duplicate candidates: the inline PRPS property layout (NPC_,
  RACE, CLAS, as in PKIN), the model group (LVLN, as in LVLP) and the destructible block
  (NPC_, kept opaque) may meet other families' blocks. (5) Codegen note: the generated
  code follows rustfmt only up to its call-width heuristic; long member paths make the
  golden `seed.rs` not rustfmt-clean (worked around with shorter NPC_/RACE names). (5) done in lane B round 5: codegen output and goldens are formatted by rustfmt, no width mimicry.
- **Actor schema lane (BC) -> lane H (CK observation).** Questions only the CK can answer
  (details in `docs/schema/actor-family.md`, "Open questions"): (1) actor values below
  0x800 in `PRPS` (NPC_, RACE, PKIN): does the CK name them and keep them on save? (2) does
  the CK write `STOP` on an NPC_ without an object template, and keep PRKZ, COCT, CS3H,
  EDCT, OBTE and LVLN LLCT in sync with their lists? (3) how `ACBS.Level` reads and is
  written with `PcLevelMult`; (4) which template applies to a section whose flag is set and
  whose `TPTA` slot is 0; (5) which controls set each `ONA2` override flag and the block
  order; (6) whether `RDSA.SpecificReference` is cleared when `UseSpecificReference` is
  off; (7) EPFT against the EPFD sizes (4, 8) and whether the CK writes a 3-byte PERK
  `DATA`; (8) any CK field that changes NPC_ `NAM5` (always `FF 00`), RACE `DAT2` offsets
  92-127 or the chargen NNAM.
- **Items/workshop schema lane (M4.BI1) -> BX (integrator).** Done in M4.BX1 (ITEMS merge, see Done; (2) by the engine-reserved ID rule of ADR 0005 Amendment 4). (1) Derived files to
  regenerate at merge: `crates/esm/src/generated/` (`cargo xtask codegen`), the
  `BUILTIN_DEFINITIONS` region (12 records, 11 `common/` files), `crates/schema/tests/
  golden/seed.rs` (`KITKRAFT_UPDATE_GOLDEN=1`) and the record and common lists in
  `crates/schema/tests/defs.rs`. (2) `validate_corpus_official_files` (`#[ignore]`) then
  reports 646 new `formid.dangling` errors in Starfield.esm, all `ObjectProperty.ActorValue`
  (the PRPS element) naming object IDs below 0x800 that no official file defines, the same
  fact as PKIN's known 0x33D/0x33E: ACTI 128, CONT 443, FURN 13, STAT 61, TERM 1; by value
  0x330 207, 0x3A2 230, 0x33E 70, 0x2D0 62, 0x33D 31, 0x32E 19, 0x32F 19, 0x387 6, 0x338 1,
  0x3AC 1 (the DLC files report the same values as warnings). Extend `KNOWN_ERRORS` or,
  better, decide once how actor values below 0x800 are treated (the open CK question from
  the planet lane). (3) Shared blocks to fold: `common/Model` vs LVLP's inline `Model`
  group and the weapon family's inline MODL/FLLD members; `common/ObjectProperty` vs PKIN's
  inline PRPS element; `common/ObjectTemplate` vs GBFM's inline OBTE/Combination and WEAP's
  `ObteBlock`; `common/SoundReference` vs any inline sound slot another family types
  (WEAP, ASPC, KSSM use the same upstream helper); `ColorRgb` is RGB plus a pad byte, not a
  duplicate of `ColorBgra`. (4) Ignored release corpus tests with these definitions: esm
  `corpus_full_roundtrip` passes (all 14 identical; M2.A3 typed check 0 conflicts, 0
  unchecked), doc `typed_corpus` passes (80,652 typed records, the same 181 pinned
  unresolved lstrings); engine `validate_corpus_official_files` fails only on (2).
- **Items/workshop schema lane -> lane E (FYI, done in its own commit).** Two unit tests
  used ACTI ATTX as their example of an lstring the schema does not type
  (`edit::fields::tests::layouts_and_fallback`,
  `patch_tests::untyped_lstrings_of_localized_sources`); ACTI is typed now (ATTX as an
  lstring), so both examples moved to FLOR ATTX (no definition, an lstring in the corpus
  table). Test-only change.
- **Items/workshop schema lane -> lane E and the plan (W3 row).** `WKMF` is "Wwise Keyword
  Mapping" (sound selection for ship engines, cockpits, grav drives, creature animation),
  not a workshop record; W3 does not need it. What registers an outpost object is the COBJ
  BNAM category keyword (`docs/schema/items-workshop-family.md`, input for M4.E7).
- **Items/workshop schema lane -> lane H (CK observation).** Questions only the CK or the
  game can answer (details in `docs/schema/items-workshop-family.md`, "Open questions"):
  (1) which COBJ field the CK writes as BNAM and as FNAM for an outpost recipe, and which
  the build menu reads; (2) whether a PTT2 with a workshop-icon transform is required for
  an object to show in the outpost menu; (3) which CK checkbox sets record flag 0x10000000
  on outpost furniture; (4) the CK fields behind FURN FNAM 0x1, CONT DATA 0x8, MNAM bit 29,
  CPTH DATA 0x8, IDLE FNAM 0x10 and TERM's one-byte WBDT; (5) the CK names of TMLM INAM,
  TMLM XLOC's last u32 and STAT DNAM's second float; (6) whether the CK writes ARMO's
  FLLD/XFLG inside the world models; (7) what references the `OutpostFoundation*List`
  form lists; (8) for M4.O4 on `KitKraft Dev`: does a new recipe with a vanilla
  `OutpostCategory*` BNAM show at every beacon, and does a new category keyword need the
  beacon and `OutpostRecipeCategories`.

- **POI investigation (2026-10-15) -> lane D (`kitkraft_io` order, `kitkraft_doc::form::runtime_slots`) and lane E (`testplan`).**
  The owner's `player.moveto 02001000` failed because the ID was wrong. The plugin itself
  is fine. Two saves of Kitkraft Dev and one of Default show the game's own plugin lists
  (evidence, method and tables: `docs/poi/first-poi-investigation.md`).
  (1) Full slots: the game puts `Starfield`, `ShatteredSpace`, `SFBGS00D` and `SFBGS050`
  first, then user full plugins in `loadorder.txt` order, then the blueprint (0x800)
  plugins after every other full plugin. `load` puts `BlueprintShips-Starfield` at `01`, so
  every user full plugin comes out 2 slots too high (Kitkraft Dev: POI `06`, game `04`;
  MO2 showed `02`).
  (2) Small and medium official order: the game has `SFBGS004` before `SFBGS007`, and
  `SFBGS003` before `SFBGS006`. KitKraft follows the ccc order and gets both wrong. User
  small and medium slots already match.
  Please: change `hardcoded-official-slots` and `runtime-slots-by-scale` to the save's
  order, with blueprint plugins last among full plugins, and add a regression fixture with
  these saves' lists. Consider a read-only save-header reader in `kitkraft_io` (BCPS, zlib
  chunks; plugin lists at about 0xA0 of the decompressed data), so `testplan` can take
  runtime IDs from the profile's newest save instead of an assumption. `testplan` should
  lead with `player.moveto <EditorID>` and with clicking the object in the console, and
  should label numeric IDs as predicted. The writer needs no change: the new ref has the
  same shape as `TIG_Shipyards.esm`'s working persistent ref in a vanilla interior cell.
  **Answered (lanes D and E fix round, 2026-10-15).** (1) `hardcoded-official-slots`: the
  official plugins first in `kitkraft_io::plan::OFFICIAL_ORDER` (each scale's order as the
  saves record it; `ShatteredSpace` and `SFBGS004` join `hardcoded-official`), then
  `loadorder.txt`; `runtime-slots-by-scale`: blueprint full plugins take the last full
  slots (`kitkraft_doc::form::runtime_slots`), but stay with the official plugins in the
  load order (ten Default plugins name `BlueprintShips-Starfield` as a master). Regression
  fixture with the saves' lists: `crates/io/src/lib.rs` `kitkraft_dev_order_matches_its_saves`.
  `kit load --profile` reproduces every slot of both profiles' newest saves and of `Save56`.
  (2) `kitkraft_io::save` reads a save's plugin lists (read-only; format in
  `docs/load-order.md`, "Save headers"); `load`/`testplan --from-save <path|newest>` adopt
  them as `confirmed-by-save` when they name exactly the load order's plugins. (3)
  `testplan` names references by EditorID, has the owner click the object to read its ID,
  and labels every numeric ID `confirmed-by-save` or predicted with its assumptions;
  `docs/poi/first-poi.md` regenerated (`04001000`, confirmed by `Exitsave0` `000052`).
- **Lane D landed (2026-10-15, authoring; owner priority D):** the support-depth matrix
  (`kit coverage --matrix`, MCP `coverage` with `matrix`; six statuses per record type;
  `docs/schema/support-matrix.md`, evidence in `docs/schema/evidence.toml`, every pointer
  checked by `every_evidence_pointer_resolves`). LCTN, TERM, PKIN, FACT: typed-readable on
  every official record (`crates/doc/tests/authoring_types.rs`), typed-writable and
  dangling-link tests (`crates/engine/src/handlers/authoring_tests.rs`); four base form
  component layouts typed in `common/BfcbBlock.toml` (`crates/doc/tests/components.rs`),
  the condition and package-location value unions typed (`validate_corpus` reserved pins
  updated: CTDA Reference 0x14, actor values 0x2BC/0x2BD/0x3B6). `docs/record-parity.md`
  not regenerated: run `cargo xtask parity` at the integration. Also fixed the
  `profiles` CommandSpec the lane W/F merge broke (engine did not compile on `main`).
- **Fix round B/D landed (2026-10-15, Fable review of lanes B and D):** `set`/`new`/`clone`/`build` refuse a payload that no longer decodes as its layout (hex: arrays of partial elements, counts that disagree; ACPR/LCPR entries are 16 bytes, PRIS 20) and the new `validate` rule `layout.undecoded` (error; 0 findings on the 14 official files, pinned in `validate_corpus`); `papyrus.compile` prefers the official ContentResources flags file and names the one used with its SHA-256, and refuses an `out_dir` under MO2 (`guard_location`); float values in `papyrus disasm --format json` carry `bits` and the oracle compares by them; a TAB in `docs/source-registry.md` fixed.
- **Roadmap lane landed (2026-10-16, docs only):** the UI track after M5 was planned from
  the owner's 2026-10-16 decisions (owner row 16, "Open owner decisions" row "UI track").
  - `ROADMAP.md`: "M6+: editor" is replaced by M6 "Cockpit", M7 "CK parity", an M8 "Beyond
    the CK" sketch and "Later".
  - `plan/execution-plan.md`: M6 has 36 tickets in lanes V, P, G, X and O. M7 has 38
    tickets in lanes K, M, W, E, R (the 3D viewport chain), S and O. M8 has 8 example
    tickets.
  - ADR 0007 Amendment 3 (**proposed**): the cockpit write scope.
  - `docs/design/workbench-performance.md`: the measurement protocol, with a first CLI
    baseline. On the dev profile (16 plugins, 7.49M records) a warm load takes 0.65 s and
    a cold one 3.5 s. `conflicts --plugin SFBGS00D.esm` takes 26.7 s and returns 29.8 MB
    for one page, which is engine request M6.P5b.
  - `docs/ck-parity.md` "Interface parity": seeded, every row `unresearched`.
  - `docs/workbench.md` "Roadmap".
  - Requests filed as tickets: M6.P2b and M6.P6b go to lane F, M6.P3b and M6.P5b to lane
    D. Finding: `C:\Modding\kitcraft\bin` lacks `kit-ui.exe`, so `kit ui` there exits 2
    (M6.P6a).
- **Lane H landed (2026-10-16, M5.H1 research, docs only; result hypothesis):** `docs/research/hostile-encounters-2026-10-16.md`. Vanilla outpost attacks are story-manager script events (`SQ_OutpostAttackBranch` `349C9F`, keywords `04643E`/`349C9E`/`346518`) sent by `SQ_ParentScript.CheckForAttack` only while the beacon is loaded, with 7-day per-planet actor-value cooldowns; the Starborn attack is gated on the Armillary (`MQ_ArmillaryResourceAV`, MQ204 stage 620); Terran Armada adds a node instead of overriding; vanilla's only unattended branch destroys unloaded outpost objects (`SQ_OutpostAttackHumansScript`), against addendum item 4. All six mechanics: no native hook needed (one weak candidate, not opened). M5.H2 fixture proposed (stage-triggered Starborn spawn at the first POI, 12 probes); it needs QUST stage writes and object script properties. Request to lane D: `inspect` shows `VMAD` only to 256 bytes, so `SQ_Parent`'s special-case array is unread; a full-payload view. Registry row `PRIOR-ART-ATTACKS`.
- **Lane F (M5 documentation) landed (2026-10-16, docs only):** M5.F4 ledger entries `game-20261015-02` (M3.O5) and `-03` (M4.O2) in `docs/game-observations.md`, graded owner report, with hashes recovered from the installed files and game 1.16.244.0 from the session saves. M5.F1 `docs/engine-limitations.md` (11 observed rows, 5 unobserved, 12 hypotheses). M5.F3 `docs/compatibility.md`: versions, all 14 masters, and the DLC mapping (Terran Armada confirmed from Steam depot 3763700; Free Lanes and Moon Jumper ship in the base depots and their records name them). ADR 0011 §3 now points to it. M5.F5 `docs/research/free-lanes-outposts-2026-10-16.md`: neither DLC changes a vanilla outpost record in substance; both add records (shared container, X-Tech extractors, faction workbenches and outposts). M5.F2 gap matrix checked at `5d03d01`. Request to lane D: point `docs/schema/evidence.toml`'s game entries at the two ledger ids and add typed-writable tests for `COBJ`, `STAT`, `REFR`, `ACHR`, `CELL` (the gap matrix's MX key lists where the two disagree).
- **Lane D7/H2 landed (2026-10-16, M5.D7 and M5.H2 first half; result partial):** `docs/recipes/mini-site.md` + `.toml` (Gate K0 mini-plugin: KYWD/TMLM/TERM/PKIN/ACTI clones, new FACT and LCTN with `LocTypeOutpost` and its own keyword, the scripts recipe's script on the ACTI, three persistent REFRs in the first POI's cell; `KitKraftMiniSite.esm` SHA-256 `10f22dbc...`, validate 0/0, roundtrip identical, every link resolves, 21 probes; `crates/engine/tests/mini_site_corpus.rs`, release `--ignored`). `prototypes/` created (ADR 0011 export rule) with `hostile-fixture/`: a two-stage `QUST` written by `new` (first QUST write; unproven in game), three GLOBs, a scripted controller at the first POI spawning two `LvlStarborn_Aggro` (template `LCharStarborn`) with a cooldown, stage 20 or the trigger GLOB cleans up (`KitKraftHostileFixture.esm` `614beeaa...`, validate 0/0, roundtrip identical, 10 probes). Gaps: a reference cannot be put in a location (`[[set]]` cannot add `XLCN`, `new:` cannot name a `[[place]]`), `KWDA` of a new form only as hex, `testplan` emits no probes for scripted behaviour (README table). Game and save/reload rows not-run.
- **Fix round W2 landed (2026-10-16, Fable review of lanes G, R, D7/H2 and E):** `gate.rs` binds a confirmation to the id and arguments it was made for (`check` refuses a confirmed request changed afterwards, with a test); the viewport deletes its GL objects (`gl.rs`: partly built scenes on the error path, a dropped tab's scene at the next paint, all at `App::on_exit`); the cockpit masters test asserts `detail: fields` off the catalog; ADR 0007 Amendment 4's "Exists" column corrected (`new`/`clone`/`revert` have `dry_run`, `place` has none: new ticket M7.S6b); hostile-fixture README ends with its Reporting block; `docs/gaps.md` says `deny`, not `forbid`.
- **KK-001 landed (2026-10-23, first local-agent ticket, ADR 0014):** `cargo xtask ci` passes on Linux as well as Windows. The 20 Linux-only test failures were fixed with platform-neutral test paths, one `cfg_attr(not(windows), ignore)` (the Papyrus compile test), and non-Windows handling of Windows-spelled paths in `io` and `engine`. Windows behaviour is unchanged. Authored by the local Qwen agent in 90 minutes, graded 9/9, reviewed and gated on Windows by Claude.
- **Apparel pipeline (owner, 2026-10-23) -> KitKraft lanes B, D and T.** The owner plans a
  clothing mod built by an AI pipeline in its own repository (not this one), with KitKraft
  as its record, package, deploy and test stage. Mesh export there runs a GPL Starfield
  geometry bridge as a tool, run only and never read, so the clean-room `.mesh` work here
  stays open. Scope is whole outfits (biped slot 3), customized by templates, workbench
  mods or tailoring recipes, across several body types. The owner is researching prior
  art and body-type targets first, so none of these is scheduled yet. Evidence: the
  Default profile, 2026-10-23, read only:
  every sampled vanilla and modded "jacket", "pants", "shirt", "skirt" and "dress" ARMO
  fills slot 3 (the KZ_AOS and KZ_TCR items fill slots 3 and 5); 209 of the 420 vanilla
  `Clothes_*` ARMOs have `APPR` `ap_armor_Lining` (`Starfield.esm:022821`), filled with
  stat OMODs through `OBTS` (`Clothes_Medical_Doctor_01_Medic`); 407 have `OBTE`, and
  `Clothes_Matteo_Hat` (`030B4B`) uses one with a property 14 swap to `1991B1`.
  1. Lane B: type `ARMA` (1,238 records, parsed, 0 typed; `docs/record-parity.md`): race,
     biped slots, the male and female models and whatever body-type data the corpus shows.
  2. Lane B: name the `BO64` biped slot bits, sourced and corpus-checked (they are a plain
     `u64` today).
  3. Lane B: type the record type of `1991B1` (the swap `OBTS` property 14 names) if it is
     not typed, and confirm which `ObjectModProperty` values an ARMO OMOD can carry
     (stats only, or a model or material swap).
  4. Lane D: a read-only `.mesh`/`.nif` reader in `crates/nif` (bounds, vertex and
     triangle counts, bone names, referenced `.mesh` paths) from the corpus study only
     (`docs/research/nif-sources-2026-10-16.md`), so `validate`/`package` can check the
     pipeline's assets instead of trusting them.
  5. Lane T: apparel probes for `testplan`: equip on the player and on an NPC, a
     screenshot per body type, the workbench and fabricator menus listing the item.
  6. Owner experiments (`KitKraft Dev`, disposable save): does the armor workbench list
     clothing, and does an OMOD on a new attach point change an outfit's look?
  7. Lane B (added 2026-10-23, from `C:\Modding\everyday-outfits\docs\material-swaps.md`):
     type `LMSW` (17,410 records, parsed, 0 typed): its one `REFL` subrecord is Bethesda's
     reflection format (`BETH`, a `STRT` string table, `TYPE`/`CLAS`/`OBJT`/`LIST`
     blocks) holding entries that map a key material path to an override material path
     (`LMS_Clothes_Matteo_Hat`, `1991B1`). Clothing colourways reach it through `OBTS`
     property 14 (`Clothes_Mei_Devine_NavyTan` `1F1DC9` against `_CreamBlue` `1F1DCE`).
     `clone` copies an `LMSW` today; `set` cannot edit the paths.
     Queued for the local agent 2026-10-24: KK-008 (inventory definition, after KK-006) and KK-007 (a
     lossless `REFL` decoder, corpus-derived only, and swap entries in `inspect --full`);
     the write half follows KK-007.
  8. Lane E (added 2026-10-23): validate packaged `.mat` files (JSON parses, object IDs
     unique, references defined, textures present, no object ID defined by two plugins),
     and resolve ARMA models to `.nif`, `.mat`, `.mesh` and textures in `inspect`.
     Evidence and format notes: `docs/research/materials-lmsw-2026-10-23.md` (its `.mat`
     facts come from installed mods' files; the owner has not ruled on that source
     category).
- **KK-003 landed (2026-10-24, second local-agent ticket):** the MCP server sends
  progress. A `tools/call` that carries a string or integer `progressToken` in
  `params._meta` is executed through `Engine::execute_with_progress` on a worker
  thread; each progress event goes out as a `notifications/progress` line (the token
  echoed, `progress` a server-side counter up by one per event, `message` the event's
  line, no `total`) while the command runs, all before the response, whose envelope is
  identical to one without a token apart from timings. A token of any other type is
  ignored, not an error; unparseable lines still answer `-32700`. `handle_line` and
  `handle_message` are unchanged; three tests in `server.rs`; `docs/mcp.md` updated.
- **KK-004 landed (2026-10-24, local-agent ticket):** `crates/engine/tests/clone_sample_corpus.rs`
  clones 100 official LCTN and 100 official TERM records (spread evenly by FormID over
  `Starfield.esm`) into one patch per type and compares each with its source subrecord by
  subrecord: 100 of 100 identical for both (only the FormID, the `EDID` and the inlined
  lstrings differ), the patch validating with 0 errors. Both tests are `#[ignore]`: 24 and
  32 minutes, because each `clone` re-reads `Starfield.esm`. Evidence row in
  `docs/schema/support-matrix.md`. Authored by the local Qwen agent; three clippy lints fixed
  and Windows-gated by Claude.
- **KK-005 landed (2026-10-25, local-agent ticket):** `location.added-ref-temporary`
  (warning, grade D, M5.D2): an `LCTN` `ACPR` (added persistent refs) entry that names a
  reference the plugin holds in temporary children (group type 9) — the finding names
  the location, the entry index, and the reference; only references the plugin itself
  holds are checked (reusing `ref.persistence`'s collection), and entries naming a
  master's reference are counted as a limitation, the masters not read. `LCPR`
  (preferred refs) and `LCSR` (custom refs) are deliberately not checked: the official
  files name temporary references there thousands of times, which is why
  `ref.persistence` leaves `LCTN` out of its naming types. Three synthetic tests in
  `cells_tests.rs`; the release corpus run pins 6 official findings in
  `validate_corpus.rs` (SFBGS003 1: LCTN:00010DFB entry 0 names ACHR:FD005C01, a
  temporary child of the medium-scale plugin itself; SFBGS050 5); `docs/validate.md`
  rule row and corpus note. Authored by the local Qwen agent (7/7, Opus QA ACCEPT); a
  compressed record's finding offset fixed and Windows-gated by Claude.
- **KK-007 landed (2026-10-25, local-agent ticket):** `crates/doc/src/refl.rs` decodes
  Bethesda's `REFL` serialization losslessly: the chunk run (`BETH`, `STRT`, `TYPE`,
  `CLAS`, `OBJT`, `LIST`), the NUL-terminated string table, and the class declarations;
  `encode` reproduces the input byte for byte. `lmsw_entries` reads an `LMSW` payload's
  entries in payload order: the swaps (key and override material paths, with the key
  material's form reference), the material form references, and the named property values
  where the payload shows them. `crates/doc/tests/refl_corpus.rs` walks all 29,897 official
  `REFL`s (17,410 `LMSW`) in the 14 ESMs: every one decodes and re-encodes byte-identically,
  and the `LMSW` ones yield 41,938 entries (2,586 payloads hold no swaps: 2,567 hold only
  material form references, 19 only empty lists). `inspect --full` adds an `lmsw` object to
  an `LMSW` record's `REFL` subrecord with the swap entries (`key`, `override`); a decode
  error answers `{"error": ...}`. `crates/engine/tests/refl_inspect.rs` checks the
  `Starfield.esm:1991B1` swap through `execute`. Both corpus tests are `#[ignore]` (they
  read the 2.5 GB of plugins). Authored by the local Qwen agent; its `.scratch/` survey files
  dropped, the corpus test made to fail on any `REFL` and pin the `LMSW` counts, and
  Windows-gated by Claude.
- **KK-006 landed (2026-10-25, local-agent ticket):** `ARMA` (Armor Addon) is typed from the
  pinned xEdit file (`crates/schema/defs/ARMA.toml`), with the corpus's six `FLLD` light
  layers per record, `XFLG` after a model's `FLLD`, and repeated `MODL`/`BNAM`. The shared
  `defs/common/BipedSlots.toml` names 53 of the 64 `BO64` biped slot bits (bit 0 `HideHair`,
  3 `Body`, 4 `Hat`, 35 `SsBody`; the upstream `Unused` bits and bit 63, which does not fit
  the signed bit values, stay raw: `docs/gaps.md`); `ARMO` and `ARMA` both use it.
  `crates/esm/tests/apparel_types.rs` places, decodes and re-encodes every `ARMA` and `ARMO`
  (the ignored full run: 1,238 and 1,168 records, all identical; 756 `ARMO` set `Body`);
  `biped_slot_bits_are_named` in `crates/schema/tests/defs.rs`. Apparel requests 1 and 2.
  Authored by the local Qwen agent over two runs (6/6, Opus QA ACCEPT); squashed, and
  Windows-gated by Claude.
