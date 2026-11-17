# Status

Updated: 2026-11-17 (after the owner walk of that evening, `game-20261117-08`; before it, the
unattended bench of 2026-11-16/17 and the owner's ruling on saves). Cut to the live state on 2026-11-15 (KK-352).

This file holds only what is live: where the project stands, the owner's next actions, open
items and blockers. Keep it short (under about 300 lines) and edit facts in place; don't
append a dated log.

- **History** up to 2026-11-15 (the sections "Done", "Pause", "Campaign", "Requests",
  "Blockers", "Next", "Decision records", "Open owner decisions", "Owner decisions
  (2026-11-05)", "Lane F" and the rest, which code comments and docs still cite as STATUS
  "...") is frozen in `plan/archive/STATUS-to-2026-11-15.md`.
- **What landed** is the merge commit and the ticket's row in `plan/tickets/README.md`, not
  a line here: `git log --merges --format='%h %ad %s' --date=short` lists it. No "Landed"
  lines (retired 2026-11-15; see the last section).
- **Current numbers** (record types, definitions, typed and writable counts, the six
  capability statuses) are generated: `docs/record-parity.md` and the headline of
  `docs/foothold/gap-matrix.md` (`cargo xtask parity`). Don't copy them here.
- Which doc is for what: `docs/README.md`.

## Where we are

- **Milestones** (`ROADMAP.md`): M0 to M2 done. M3 and M4 are closing. Owner ruling
  2026-11-17, verbatim: "close M4 with F20 carried into M7": F20, F22, F25 and F26 are M7
  exit criteria now, not M4's. Left before M4 closes: O4-01, O4-02 in the
  recipe's form (the embassy cell owned by `PlayerFaction`: the owner walk's W-11 looked at
  a vase with its own owner and read Steal, so the cell owner's effect is unobserved;
  re-probe on an item with no owner of its own, `docs/gaps.md`; the reference-owner variant
  passed, W-01 to W-05); O4-05 (the outpost object, not run: no outpost); F27 (re-sit); F21, F24 (not run); deleting the staged xEdit files
  (`C:\Modding\tools\xedit-ref\`, still there) and recording it; the final parity
  regeneration. Since the ruling of
  2026-11-16 (ADR 0012 Amendment 3, ADR 0010 Amendment 3) an agent may save in the CK and
  in the dev profile without asking per sitting, so most of what was owner-gated is agent
  work again: the CK fixtures F20 to F27 that have not passed, the `ck` write style, the
  overhaul's save/reload row O5-15, the workshop row O4-05 (O4-06 passed in the owner walk). A
  game launch still needs the owner's go for that session. Left for the owner alone: the
  ownership UI rows O4-01 and O4-02 (a person's look) and the milestone sign-offs. The ten
  explained-difference rules were accepted on 2026-11-17 ("Accept all ten"); every accepted
  explained difference is a row of `docs/explained-nonconformities.md`. M5 is partial. M6 (the cockpit) waits on
  the window run and its sign-off (M6.O4). M7 (CK parity) is in progress.
- **Owner walk of 2026-11-17** (`kit walk`, `docs/game-observations.md` `game-20261117-08`):
  passed O4-06 (the fabricator recipe crafted), O4-07's `OnActivate` (the owner pressed the
  switch; the log line and the counter), W12 (the rename package through MO2, manifest
  `ok`) and the reference-owner form of O4-01 and O4-02. Failed: W-10, the save of W-09 was
  never written, so O5-15 is still open (`kit walk` now checks the save landed); W-11, see
  above. It also showed the first POI's crate and the verification mod's row stand where
  no one can walk (`docs/gaps.md`; M4.O2's pass is presence and persistence).
- **Bench of 2026-11-16/17** (unattended, the owner's go for that night): three CK
  sittings took `ck-differential-tested` from 2 to 37 types, and the re-comparison with the
  accepted rules (`ck-20261117-65`) added 14 more; fixtures F28 and F23 pass,
  F27 is inconclusive, F20 was not run (no way found to create a new interior cell in the
  CK); the verification mod's 25 rows and the verification quest's 5 rows pass in game;
  OW-2's command-line parts pass except `help` on the renamed item (inconclusive). Not
  run: OW-2 in the window and M6.O4 (the computer-use dialog needs a person). Records:
  `docs/bench/ck-batch/triage-20261117.md`, `resit-20261117.md`, `resit2-20261117.md`,
  `docs/game-observations.md` `game-20261117-01` to `-06`.
- **How work runs**: tickets in `plan/tickets/` (ADR 0014). The overnight local agent takes
  the headless ones, graded by the gate, merged by a morning session; Opus lanes take
  design work and anything that relaxes a guard or touches the write path. The queue and
  each ticket's state: `plan/tickets/README.md`. Runnable now: KK-371 to KK-404 (the
  local-model backlog written 2026-11-17; KK-370 merged the same morning).
- **Merged on 2026-11-16 and 2026-11-17**: KK-330 to KK-343 and KK-360 to KK-370; the
  component blocks (raw 24,430 to 3); alias scripts, struct properties and the quest
  fragment checks; the members the CK adds to a new record, for about 40 types
  (`e7718b5e`, `2c4848f9`).
- **Merged on 2026-11-14 and 2026-11-15**: KK-252 to KK-266 (Opus lanes, typed types,
  typed-write and clone-sample tests); conditions, KK-267 to KK-272 (`8d6c998a`); dialogue,
  KK-274 to KK-279 (`92773276`); component layouts, KK-280 to KK-288 (`957773f5`) and
  KK-289 (`a6f7cfaf`); typed-write tests for the fourteen untested family rows, KK-300 to
  KK-313 (`544da6f8`); housekeeping, KK-320 to KK-324 (`91a694ee`); the component-name
  decider of ADR 0015, KK-292 to KK-295 (`0e3ee544`); this docs cleanup, KK-350 to KK-353.
- **What works and what doesn't**: `docs/gaps.md` (check before claiming something works),
  `docs/engine-limitations.md`, and the gap matrix's six statuses per record type.
- **Planning**: `plan/next-phase/README.md` (2026-11-14, marked where `main` has moved):
  what is left per milestone, the ranked holes, ticket drafts.

## Owner next actions

Rows cite other sections of this file as it was before 2026-11-15 ("Pause", "Install dry
run", "Director assumptions pending owner"); they are in `plan/archive/STATUS-to-2026-11-15.md`.

**Decisions are done (2026-11-01).** The owner answered `plan/owner-decisions-2026-11-01.md`
in chat, verbatim: "accept all" (items 1 to 23), and corrected item 13: "on 13 we're
public". Each answer is recorded in the sheet's "Answers" section and in the rows below.
What is left for the owner:

- **Group C of the sheet** (items 24 to 34), his to-do list: the folder move and install
  (rows 5 and 11), the W12 rename package (row 2, done 2026-11-17), session 2 (row 14), the workbench
  walkthrough (row 8), the three schema files to eyeball, and the M2.O4 leftovers.
- **F30 and F29 are done** (row 12, `ck-20261101-02`, `ck-20261101-03`; the owner counted
  F30 as clean). Left from them: remove the CK's `KitKraftFirstPOI-CK.esp`, `.esm` and
  `Backup\AutoSaveDEFAULT.bak` from MO2's `overwrite\` folder.
- Group D (M6.O4, M7.O6) stays his for later.
- **From the bench of 2026-11-16/17**: rule on the send order from the ledger's timestamp table; sit at the desk
  once for OW-2 in the window and M6.O4 (click Allow on the computer-use dialog); say
  whether the 37 commits of `e7718b5e`'s CK lane that do not compile alone stay on public
  `main` as they are.
- **Saves (ruling of 2026-11-16)**: rows 12 and 15 below say a CK save needs the owner's OK
  per sitting. That no longer holds (ADR 0012 Amendment 3). The one limit is the owner's
  primary saves folder, which nothing writes, renames or deletes.

In-game checks use only the MO2 profile **KitKraft Dev**, each on a disposable save. Never
use `Default` (AGENTS.md, "Owner's live game is off limits").

| # | Action | Unblocks | State |
| - | ------ | -------- | ----- |
| 1 | In-game check of the rename plugin `OldSovietMarksmanRifle.esm` (M2.O6) | M2 exit | **done 2026-10-14: PASS** |
| 2 | In-game check of the first POI (M4.O2). Install `out\poi\package` through MO2 and check the hashes against its manifest. Then in the console: `coc CityNewAtlantisApartmentTEMPLATE`, `player.moveto KitKraftFirstPOICrate`, click the crate (the console should show `04001000`), then `prid 04001000` (Kitkraft Dev's slot, confirmed by its newest save: `kit testplan ... --from-save newest`, `docs/poi/first-poi.md`; `02001000` and `28001000` were wrong IDs, `docs/poi/first-poi-investigation.md`). Take a screenshot and run the persistence steps in `docs/poi/first-poi.md`. In the same session, install the rename package (W12, `docs/packaging.md`) | M4 exit (first POI, W2, W5, W12) | **visit PASS 2026-10-15** (owner, "M4.O2 result" below), reached by `player.moveto` and a console click: the crate stands on the bedroom's east wall line and was never found on foot, so the pass shows presence, not reachability (`docs/poi/first-poi.md`, "Correction"); **save/reload O2-08 PASS 2026-11-01**, unattended (`docs/game-observations.md` `game-20261101-04`). **W12 PASS 2026-11-17** (owner walk W-08, `game-20261117-08`: installed through MO2, every manifest line `ok`, the rifle named; owner report) |
| 3 | In-game check of overhaul build 2 (M3.O5, steps and checklist in `docs/overhaul/build-2.md`, "In game"; package `out\overhaul\package\KitKraftGunOverhaul-0.2.0.zip`), on `KitKraft Dev` with a disposable save | M3 exit (overhaul) | **PASS 2026-10-15** ("M3.O5 result" below; ledger `game-20261015-02`). Left: only the save/reload row O5-15, which an agent game session can run; nothing for the owner |
| 4 | ~~Answer the brief rows B8, B11 and B12~~ | overhaul build 2 (M3.E9 rerun) | **answered 2026-10-15**: B8 dropped, B11 base 18 (drum untouched, 48), B12 as built; applied in build 2 |
| 5 | Close every Claude Code session that has the repo open. Then double-click `C:\Modding\kitcraft\dist\KitKraft\Finish Move.cmd` (it warns, asks for `YES`, and runs `C:\Modding\scripts\finish_kitkraft_move.ps1` from a copy in TEMP; docs/QUICKSTART.md section 8). The script renames `C:\Modding\kitcraft` to `C:\Modding\kitkraft`, re-points the MCP registrations, and runs `git worktree prune`, so run it only when no lane worktree is in use. Open new sessions on `C:\Modding\kitkraft` | the folder name every doc already uses | **done in practice** (checked 2026-11-15, KK-321): `C:\Modding\kitcraft` no longer exists and the repo, its worktrees and `dist\KitKraft` are under `C:\Modding\kitkraft` |
| 6 | Decide when to start the workbench (lane W, M4.W0..W6, ADR 0007 Amendment 1). The M4.W0 gate needs M4.D1 (`refs`) and M4.D2 (placement context), and neither is on `main`. The deferred CK sessions (M3.O1, M4.O1) wait for M4.W2..W5 | M3.O1 and M4.O1 (CK), with them every `ck-diff` upgrade | **decided 2026-10-15: started**. Lane W built the read-only workbench (`kit ui`, `docs/workbench.md`) |
| 7 | Rule on two licenses in the workbench's dependency tree (ADR 0007, "Dependency list"). First, **BSL-1.0** (`clipboard-win`, `error-code`): eframe always pulls these in for the copy buttons; the license is permissive but is not named in the MIT/Apache/Zlib/BSD rule. Second, whether the optional `default_fonts` feature may ever be enabled (OFL-1.1 and Ubuntu Font Licence). It is off: the window uses the Windows system fonts | the first workbench merge to `main` | **decided 2026-11-01** (sheet items 9 and 19): BSL-1.0 allowed for the clipboard crates; system fonts only, `default_fonts` stays off (ADR 0007 "Dependency list") |
| 9 | ~~Rule on **ADR 0010** (agent-driven game sessions on `KitKraft Dev`, M4.O6)~~ | lane C (M4.T1..T10 under their ids, M5.C1, M5.C2): unattended closing of in-game rows, then brief-to-mod end to end | **decided 2026-10-15: approved in principle** (`docs/foothold/owner-decisions-2026-10-15.md` item 6, addendum item 5). Conditions and the sub-question answers are ADR 0010 Amendment 1: isolated dev profile, disposable saves, explicit authorized paths, backups and restore-only rollback, timeouts, screenshots in scope (input injection deferred), statuses not-run/pass/fail/inconclusive with missing evidence = inconclusive, manual acceptance retained |
| 10 | M4.O7 (path (b) of M5's shortest validated path): install Console API (Nexus 4280), Console Command Runner Updated (Nexus 7318) and optionally Console Output To File (Nexus 3142) into the `KitKraft Dev` profile through MO2; add `[General] bAlwaysActive=1` and the `[Papyrus]` logging keys to the profile's `StarfieldCustom.ini`; windowed in the profile's `StarfieldPrefs.ini`; create a disposable base save `KitKraftBase` in a quiet interior and note its SHA-256; note the versions in `docs/source-registry.md`. Agents do not download or install these (Nexus needs the owner's login) | M4.T1 (V0: `GetSFSEVersion` over HTTP on 1.16.244), then M4.T3 + M5.C1 and M5.C2, the first unattended save/reload | **done 2026-11-01 (owner)**: profile-specific INIs on (`LocalSettings=true`); Console API and Web Application, Console Command Runner Updated and Address Library for SFSE Plugins enabled in `Kitkraft Dev` only; `[General] bAlwaysActive=1` and the three `[Papyrus]` keys in the profile's `StarfieldCustom.ini`; `bFull Screen=0` (borderless windowed); base save `KitKraftBase.sfs` in `CityNewAtlantisApartmentTEMPLATE`, SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`. In game the console showed SFSE 0.2.21 on runtime `01100F40` (1.16.244). **The endpoint is on port 45555, not 55555**: Windows excludes TCP 55477-55576 on this box (Hyper-V/WSL reservations), the plugin logged `http server not started (unable to start)`, and the owner set `iPort=45555` in the mod's `sfse_plugin_console_api.ini`. Follow-ups done the same night: the game profile's url is 45555 and preflight refuses an excluded or mismatched port (`console-port`, `3d1408b`); V0 over HTTP passed (`docs/game-observations.md` `game-20261101-T1`) |
| 8 | Run the workbench walkthrough against `KitKraft Dev` when the workbench changes (it is `#[ignore]`d, so `cargo xtask ci` skips it). PowerShell: `$env:KITKRAFT_WORKBENCH_PROFILE = 'C:\Modding\MO2-Starfield\profiles\KitKraft Dev'`, then `cargo test --release -p kitkraft-ui-egui --test dev_profile -- --ignored --nocapture`; it reads the profile and writes nothing but the engine's index cache (`docs/workbench.md`, "Panels") | confidence that `docs/workbench.md` matches the real profile | pending (owner), after each workbench round |
| 11 | Install KitKraft for daily use (lane F, docs/QUICKSTART.md): close every Claude Code session, double-click `C:\Modding\kitcraft\dist\KitKraft\Install.cmd`, read the dry run (the same as "Install dry run" below), type `y`. Then start Claude Code: `/mcp` should list `kitkraft`, and `kit doctor` should end with no FIX on the MCP lines. Do it after row 5, or let `Finish Move.cmd` re-point it afterwards | `kit` on PATH in every terminal; the MCP server registered as `kitkraft` at the dist build | **done in practice** (checked 2026-11-15, KK-321): `kit` on the PATH resolves to `C:\Modding\kitkraft\dist\KitKraft\kit` and the `kitkraft` MCP server is registered and answering in Claude Code sessions |
| 12 | M5.O1 (path (a) of M5's shortest validated path): an owner-assisted CK sitting from the steps in this row, which are enough on their own (M5.A1's session script, when written, adds the ledger detail and the later sittings): **F29** (in the CK, place `Crate_MilitarySupply_01` `Starfield.esm:1E4FBD` in `CityNewAtlantisApartmentTEMPLATE` at the first POI's position, persistent, EditorID `KitKraftFirstPOICrate`, save with `Starfield.esm` as the only master) and **F30** (open `out\poi\KitKraftFirstPOI.esm` in the CK and save an unchanged copy). Screenshots and files into `observe\<date>-F29\` and `observe\<date>-F30\`. Since 2026-11-01 an agent may run CK sittings without you under ADR 0012 Amendment 2 (accepted); a CK save still needs your OK per sitting. You allowed F30 on 2026-11-01, and F29 once F30 comes back clean (sheet item 21), under the same rules: dev profile, files into `observe\`, the CK's settings files restored | M5.A3: the first CK differential (Gate K0 "CK can load/re-export"), run with `cargo xtask observe`, `kit roundtrip --verify --decode` and `kit diff`; every `ck-diff` cell of `docs/foothold/gap-matrix.md` | **F30 first half done 2026-11-01, agent-run** (`docs/ck-observations.md` `ck-20261101-01`): the CK loaded our plugin with no warning naming it; it would not take the master-flagged file as the active file, so nothing was saved and F30's save and the differential are open. **F30 second half done 2026-11-01, agent-run** (`ck-20261101-02`): with the owner's OK the CK opened a non-master `.esp` twin as the active file and saved it unchanged; both records byte-identical, four TES4 differences unexplained (`cargo xtask oracle ck`: fail, match 1/1). The owner counted that clean. **F29 done 2026-11-01, agent-run** (`ck-20261101-03`, the owner counted F30 as clean): the CK placed the crate, saved `KitKraftFirstPOI-CK.esp` and converted it to `.esm`; position exact, rotation 3.13999915, CELL override byte-identical to ours; the CK made the reference temporary (no Persistent checkbox in the Reference dialog). **M5.A3 done 2026-11-01** (`ck-20261101-04`): every difference explained, by the corpus or as CK bookkeeping (`cargo xtask oracle ck`: F30 clean, F29 ours-missing, 0 unexplained); `CELL` and `REFR` are `ck-differential-tested` pass. Owner decision 2026-11-02: a placed reference defaults to temporary, as the CK does (already the default of `kit place` and `[[place]]`; existing recipes keep `persistent = true`, `docs/design/persistence.md` "Default: temporary"); `validate`'s `cell.incc` no longer warns on a non-master plugin without `INCC` |
| 13 | M5.O2: confirm the dependency list `crates/game` adds (an HTTP client and a process crate) at the M4.T3 merge (ADR 0010 Amendment 1 (c)). Blocks the merge only if a license is not MIT or Apache-2.0 | M4.T3 merge, so path (b) | **M4.T3 merged 2026-10-16** with no third-party crate (ADR 0010 "Dependency list": workspace crates only, all MIT OR Apache-2.0), so it went in under the director assumption above; **closed 2026-11-01** (sheet item 10): the owner confirmed it, nothing to review |
| 14 | M4.O4: in-game session 2 on `KitKraft Dev` with a disposable save, template O4 in `docs/game-observations.md`: the ownership and actors recipes (O4-01..O4-04), the outpost object and fabricator recipes (O4-05, O4-06; `docs/recipes/workshop.md`, `fabricator.md`), and the **scripts recipe in game** (O4-07; `docs/recipes/scripts.md`, "In game", plugin `KitKraftScripts.esm`) | M5.B3 (the scripted activator in game, Gate K0) unless lane C's M4.T6 closes O4-07 first; M5.O3; M4 exit | **partly done by agent sessions 2026-11-03** (`docs/game-observations.md`, re-graded on controller `e6028aaa`): O4-03 pass (`game-20261103-06` T-01 to T-03), O4-04 persistence pass (`-06` T-05), O4-07 the scripts recipe fires on `OnLoad` (`-04` T-05, Gate K0). **Owner walk 2026-11-17** (`game-20261117-08`): O4-06 pass (W-07, crafted; 6 shells by the console), O4-07 `OnActivate` pass (W-06, owner's press; Papyrus `KKW-A01` and the global 0 -> 1); O4-01 and O4-02 pass for a reference's own owner (W-01 to W-05, owner report) and **fail as the recipe** (W-11: the item looked at had its own owner, so the cell override is untested). Still open: O4-01 and O4-02 on an embassy item with no owner of its own (the next walk's W-11), O4-05 (no outpost; not run) |
| 15 | M4.O1: the CK sittings for fixtures F20..F28 (`plan/execution-plan.md` M4, "Fixture matrix (M4.H1)"), owner-assisted; a computer-use agent may drive the CK while you watch. Deferred on 2026-10-14 until the workbench existed (row 6); the workbench is built, so the deferral's condition is met. Best after M5.O1 (row 12), whose F29/F30 sitting sets up the CK. Since 2026-11-01 each sitting may be an agent run (ADR 0012 Amendment 2), but each needs the owner's OK on its own (sheet item 21) | M5.A5 (fixtures through the CK differential), and through it FTH-EXP-01, -04, -08 and -10; M4.A1..A6's rules raised to `ck-diff` | **partly done 2026-11-17, agent-run**: F28 and F23 pass, F27 inconclusive, F20 not run (and so F22, F25, F26, which build on it); F21 and F24 not run. No owner OK needed per sitting any more (ADR 0012 Amendment 3). **F20, F22, F25, F26 carried into M7** (owner ruling 2026-11-17, "close M4 with F20 carried into M7"); left for M4: F27 (re-sit), F21, F24 |
| 16 | M6 "Cockpit" and M7 "CK parity" owner decisions (`plan/execution-plan.md` "M6" and "M7", lane O). **Decided 2026-10-16** (owner, in chat, verbatim: "Approve Amendment 3 and ADR 0012, start lane V mockups"): **M6.O3**, ADR 0007 Amendment 3 accepted as written. That is the cockpit write scope: `ledger`, `deploy --dev`, `deploy.rollback`, `game.session`, `game.probe` and `game.stop`, each previewed and confirmed per click, and nothing else; the window never launches the game itself, and `testplan` stays out. It also confirms the director's decision on rollback in and `testplan` out. **M7.O1**, ADR 0012 (`plan/adr/0012-ck-interface-clean-room.md`, the CK interface clean-room rule) accepted. Lane V started: M6.V1 and M6.V2 landed. **M6.O1 decided 2026-10-16: Workbench** (owner, in chat, verbatim: "Workbench, themes look great only keep the mocks if there's a good reason."); `workbench` is the default, `studio` selectable, `classic` legacy, the mockups removed. **Still open on 2026-10-16** (decided 2026-11-01, see State): **M6.O2**, fonts, either Windows system fonts (the default) or a compiled-in OFL font or MIT/Apache icon font (row 7's BSL-1.0 ruling is still open beside it); **M7.O5**, the sources for Starfield NIF and geometry layout. Later: M6.O4 (the cockpit sign-off at the end of M6; a check at 125 and 150 percent is optional, lane X is deferred), M7.O2 and M7.O3 (CK interface sittings, combinable with rows 12 and 15), M7.O4 (Amendment 4, the editor write scope) and M7.O6 (A/B sign-off). **Director decisions then pending your confirmation** (confirmed 2026-11-01) (`plan/execution-plan.md` "M6", "Decisions taken by the director 2026-10-16, pending owner"; ADR 0002 Amendment 1): `crates/nif` stays behind the engine and the L2 `render` slot is retired; M7.S5 geometry as base64 buffers in the JSON envelope; CK menu order `unresearched` until sitting 1 | Open: any M6.V4 icon font and a compiled-in font for the type scale (O2), M7.S4 and so M7.R3 to M7.R5 (M7.O5), M6 exit (O4). Unblocked by the 2026-10-16 rulings: M6.G3 to M6.G6 (each still on its other dependencies), M7.K2 onward, M6.V3 onward (O1) | **O3, M7.O1 and O1 decided 2026-10-16**; **O2, M7.O5 and M7.O4 decided 2026-11-01** (sheet items 19, 20, 22: system fonts only; the NIF memo's recommendations 1 to 3, so M7.S4 is unblocked; ADR 0007 Amendment 4 accepted with model A, one confirmed write per OK click, so M7.E2 to M7.E6 are unblocked), and the director decisions listed here confirmed (items 3, 4, 8). Later: M6.O4 and M7.O6 (sign-offs); M7.O2 and M7.O3 can be automated sittings (ADR 0012 Amendment 2) |
| 17 | Rule whether a static strings scan of `CreationKit.exe` (printable text read from the binary's bytes, no disassembly, debugging or memory) is allowed: AGENTS.md "Clean room" allows the CK only "run and observed as a black box", ADR 0005 §3 forbids disassembly, decompilation, debugging and memory inspection, and the scan is neither. Answer as an ADR 0005 amendment or an AGENTS.md clarification. Until then: no further static analysis, and `docs/ck-observations.md` "M5.A4" is retained but not cited ("Director assumptions pending owner") | M5.A4 (unattended CK route) | **decided 2026-11-01: no** (sheet item 18). Static string scans of `CreationKit.exe` are not allowed (AGENTS.md "Clean room", ADR 0012 Amendment 2 "Owner ruling 2026-11-01"); the M5.A4 entry is deleted. The unattended route is ADR 0012 Amendment 2 |

**Director ruling of 2026-10-15, confirmed by the owner 2026-11-01** (sheet item 9): eframe's clipboard support brings `clipboard-win` and `error-code` under BSL-1.0 (Boost). BSL-1.0 is permissive with no copyleft and is compatible with MIT/Apache-2.0; applying owner clarification 2 (no pause unless the license category differs), the workbench was merged with this noted. The clipboard crates stay.

**M3.O5 result (2026-10-15):** PASS. Owner ran every `player.additem` row of docs/overhaul/build-2.md on the vanilla `KitKraft Dev` profile with build 2 loaded: C1-C7 and C9-C14 pass (C8 dropped). Rifle shows "Old Earth Marksman Rifle". Gun overhaul closed; M3 exit criteria met except the deferred CK session.

**M4.O2 result (2026-10-15):** PASS. On the vanilla `KitKraft Dev` profile the owner reached the crate with `coc CityNewAtlantisApartmentTEMPLATE` + `player.moveto 04001000` (runtime slot from the save, not MO2's index). Save/quit/reload persistence row: **passed 2026-11-01 in an unattended `kit game session`** (`docs/game-observations.md` `game-20261101-04`: save, move away, load, back in the save's cell; the crate at its position after the reload).

## Open items

Owner decisions and work that is owed, newest first. When one closes, delete its bullet
(the commit that closes it is the record).

- **Docs survey of 2026-11-15, options C and D** (not decided): generate the gap matrix's
  family columns and tally from `cargo xtask parity` so tickets stop hand-editing them (C);
  move the matrix out of `docs/foothold/` and the Foothold history into `plan/archive/` (D).
- **Five decisions in `plan/next-phase/README.md`** ("Five decisions that are yours"):
  decision 1 (merge the sprint) is answered; 2 (where the mod ideas are), 3 (agent game
  sessions overnight and one desk sitting), 4 (start "beyond", engine-first) and 5 (small
  and medium plugins) are open.
- **CK comparison, still differing with the accepted rules** (`ck-20261117-65`): `STAT`,
  `CELL` (one lighting value), `ACHR` (`XLRL`), `NPC_` (`RNAM`), `PNDT`, `STDT` (`XMPM`
  order) and the dialogue types (the CK adds branch records); `SFTR` is reopened (a new one
  with `CNAM` and `DNAM` crashes the CK on load).
- **Not re-run**: the verification mod on its new build (its 25 passes were seen on the
  build before the default-member fixes); `validate_corpus_official_files` after the
  `EQUP.ANAM` reserved-ID pin (76 minutes, outside the gate).
- **Owed from the owner rulings of 2026-11-07** (archive, "Owner rulings, 2026-11-07
  (midday)" and "Afternoon of 2026-11-07"): the send-order question (ruling 2) was run on
  2026-11-16 and 2026-11-17; answers sent during a `coc` or within about 6 s of the ready
  signal are lost, not late, and the owner's reading of that is open; and, for the owner, whether the window's Data dialog should follow ruling 3 (a
  master-flagged file of the owner's may be the active file) or keep the CK's refusal
  (`ck-20261101-01`).
- **One desk sitting** (archive, "Owner decisions (2026-11-05)" items 8, 11 and 12): the M6
  end-to-end run from the window by computer use, then the owner's M6.O4 sign-off; the
  real-window checks the 2026-11-07 notes list (a load from a real MO2 profile, the Object
  Window's speed, the viewport on the dev profile, a placed reference in the Cell View);
  what UI Automation cannot see in the CK. The fixture saves are no longer part of it
  (F28 and F23 pass; F20 and F27 are agent work). `docs/bench/README.md` is the step list.
- **O5-15** (the overhaul's save/reload row) needs a dev-deployed copy of the overhaul: the
  controller refuses the copy the owner installed by hand. An agent session can do both
  now, on the owner's go for a game launch. The owner walk tried it as W-09/W-10
  (`game-20261117-08`): the save `KitKraftWalk` was never written, so nothing was reloaded;
  `kit walk` now checks the save landed before asking for the load.
- **Ownership recipe in game** (O4-01, O4-02 as the recipe): the next walk's W-11 on an
  embassy item with no owner of its own (a Digipick or a to-go cup), with a control
  without the plugin (`docs/recipes/ownership.md`, `docs/gaps.md`).
- **Reflection payloads** (`docs/research/reflection-scan-2026-10-31.md`): `REFL`, `XNSE`,
  `PTCL`, `PCCC`, `RDIF` and `PSDF` carry their own class and field tables, so a reader
  over `crates/doc/src/refl.rs` can type them from the corpus alone. Not queued.
- **Harness, outside the repo**: the campaign RFC's remaining changes (the failure ledger,
  per-class budgets, a campaign trunk; archive, "Next" item 2).

## Blockers

- None for the agent queue.
- Owner-gated: a game launch needs the owner's go for that session; the window run and
  M6.O4 need the owner at the desk. CK sittings no longer wait on an OK (ADR 0012
  Amendment 3).
- F20 (a new interior cell made in the CK) has no known route by sent input; F22, F25 and
  F26 build on it. All four were carried into M7 (owner, 2026-11-17), so this blocks M7's
  exit, not M4's.

## Toolchain (verified on the dev box)

- rustc 1.99.0 (b940084d7 2026-09-28)
- cargo 1.99.0 (5f94df478 2026-08-27)
- clippy 0.1.99 (b940084d7e 2026-09-28)
- rustfmt 1.10.0-stable (b940084d7e 2026-09-28)
- MSVC 14.37 linker

## Landed lines (retired 2026-11-15)

Don't add a "Landed" line for a ticket: its merge commit and its row in
`plan/tickets/README.md` are the record. A run started from an older prompt may still append
one at the end of this file, below this paragraph; that is harmless, and the merge session
deletes it (or moves a fact that belongs in a doc to that doc).
