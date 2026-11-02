# Game observations (in-game ledger, M4.H4)

The ledger for what the owner sees in the running game: the first POI and the rename
package (session O2, M4.O2), the recipes and experiments (session O4, M4.O4), the gun
overhaul (session O5, M3.O5) and every persistence check. It is the format `kit testplan`
reports into (its `checklist.md` ends with "add an entry to `docs/game-observations.md`").
CK observations have their own ledger, `docs/ck-observations.md`.

Rules that bind every entry (AGENTS.md, "Owner's live game is off limits"):

- **MO2 profile `KitKraft Dev` only, on a disposable save.** Never `Default`.
- A plugin that owns forms (the POI, the actors recipe) is never removed from a save that
  is not disposable; override-only plugins (the rename, the overhaul, the ownership
  recipe) may be dropped.
- Screenshots stay in `C:\Modding\kitkraft\observe\<yyyy-mm-dd>-<session>\` (gitignored);
  an entry names the files, it does not embed them.
- **A step that could not run is a result.** Write what happened (the console's message,
  the batch file not found) and mark it `not-run` (older entries write `not run`); never leave a row blank without a
  reason.
- Runtime IDs depend on the load order. The IDs below are what `kit testplan` printed for
  the owner's order on 2026-10-14 (`docs/load-order.md`, assumption
  `runtime-slots-by-scale`); when the console rejects one, use the EditorID form given in
  the same row and write down the ID the game shows.

## Entry format

Id `game-<yyyymmdd>-<nn>` (session date, two-digit sequence; one entry per session sitting).

Header:

| Field | Content |
| ----- | ------- |
| Id | `game-20261016-01` |
| Session | O2, O4 or O5 (and the plan task: M4.O2, M4.O4, M3.O5) |
| Date | yyyy-mm-dd |
| Game build | `Starfield.exe` version (file Properties > Details) |
| MO2 profile | `KitKraft Dev` |
| Save | the disposable save's name (and whether it is a new game) |
| Plugins | each KitKraft plugin enabled: file name, SHA-256 on disk, manifest check (`ok` lines from `docs/packaging.md` "Check the installed files"), position in the plugin list |
| Other changes | anything else enabled or changed in the profile since the last entry |
| Screenshots | folder under `observe\` |

Steps: the rows of the session template below, each filled with what was seen.

| Column | Content |
| ------ | ------- |
| Row | The template row id (`O2-07`) |
| Step | What was done (console command, menu action) |
| Forms | FormID as `Plugin.esm:XXXXXX`, EditorID, runtime ID used |
| Expected | Copied from the template |
| Seen | What the game showed, in words; console output copied as text |
| Result | `pass`, `fail`, `inconclusive` (with the reason), `not-run` (with the reason; older entries write `not run`) or `n/a` |

`inconclusive` (ADR 0010 Amendment 1 condition 7, M5.C1) means the row ran but its
evidence is missing or ambiguous. Examples: no console output to match, no number in it, a
log that was not written, a screenshot the row asks for that was not taken, or a timeout.
Missing evidence is never `pass`. A `manual` row that has only a screenshot stays
`inconclusive` until the owner looks at it.

Entries written by `kit game session` (`docs/game-sessions.md`) use the same tables. The
header adds what Amendment 1 condition 5 needs to rerun the session: the game and script
extender builds, the console tools' versions, the base save's SHA-256, every plugin's
SHA-256 and the `probes.toml` hash. The step table adds an Evidence column (screenshot
file names). A `Finding:` line names every profile file that changed during the session.
The controller restores each one from its backup, and the session is then `inconclusive`.
| Evidence | Screenshot file names |

A `fail` or a surprise becomes a finding: one line under the entry naming the lane it goes
to, filed as a Request in `plan/STATUS.md` (as M3.H3 does for CK findings). Grade D claims
in `docs/design/persistence.md` (`PS-nn`) that a row answers are named in the row; the
answer upgrades that claim (to A when reproduced, with the entry id as its source).

## Persistence protocol

Rows marked "persistence" use these steps, in order, and record each:

1. **Leave**: `coc` to another cell (an interior is quickest, for example
   `coc CityNewAtlantisLodgeInt`, `Starfield.esm:06BB29`; any cell other than the test
   cell works). Note the in-game time.
2. **Short absence**: `wait` one game hour in that other cell.
3. **Save and reload**: save (a new disposable save), quit to the main menu, load that save.
4. **Return**: the session's batch file or the `coc` and `player.moveto` of the row.
5. **Long absence** (rows that say so): from a cell in another star system, pass more than
   168 game hours (`iHoursToRespawnCell`, PS-21), by waiting or sleeping in 24-hour steps.
   Note how the time was passed and how long it was: whether waiting counts towards a
   reset is itself a question (PS-22).

Record for every object of the row: present / missing / moved / doubled, and its position
(`prid <id>`, then `getpos x`, `getpos y`, `getpos z`).

## Template: session O2 (M4.O2, first POI and the rename package)

Plugins: `KitKraftFirstPOI.esm` (1,821 bytes, SHA-256
`13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1`, package `out\poi\package`;
owns one form) and the rename package (`OldSovietMarksmanRifle.esm`, 2,221 bytes, SHA-256
`9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27`; override only). Steps
from `docs/poi/first-poi.md` and `docs/packaging.md`.

| Row | Step | Forms | Expected | Seen | Result |
| --- | ---- | ----- | -------- | ---- | ------ |
| O2-01 | Rename plugin in game (M2.O6): `player.additem 0021BBCD 1`, open the inventory | WEAP `Starfield.esm:21BBCD` `RussianHuntingRifle` | Shown as "Old Soviet Marksman Rifle" | done 2026-10-14: entry `game-20261014-01` | **pass** |
| O2-02 | Install the rename **package** through MO2 (*Install a new mod from an archive*), run the manifest check | `OldSovietMarksmanRifle.esm` | Every manifest line `ok`; MO2 asks for no data folder (W12) | | |
| O2-03 | Install the POI package the same way and run the manifest check | `KitKraftFirstPOI.esm` | Every line `ok`; the plugin sits after `Starfield.esm` | | |
| O2-04 | Copy `kitkraft-test.txt` (from `out\poi\testplan`) next to `Starfield.exe`; in game `bat kitkraft-test` | - | The three commands run in order (grade D: whether the game reads batch files from there) | | |
| O2-05 | `coc CityNewAtlantisApartmentTEMPLATE` (or step 1 of the batch) | CELL `Starfield.esm:21966F` "Player House Apartment" | The player stands in the apartment | | |
| O2-06 | `player.moveto KitKraftFirstPOICrate`, then click the crate in the console | REFR `KitKraftFirstPOI.esm:001000` `KitKraftFirstPOICrate`, runtime `04001000` on Kitkraft Dev (confirmed by its newest save, `docs/poi/first-poi.md`; the earlier `28001000` was the old slot rule's) | The player is moved to the crate; the console shows `04001000` (write down the ID shown) | | |
| O2-07 | `prid 04001000` (or `prid KitKraftFirstPOICrate`), `getpos x`, `getpos y`, `getpos z`; screenshot | base STAT `Starfield.esm:1E4FBD` `Crate_MilitarySupply_01` | A military supply crate on the floor at (-17.0, -4.0, 4.63), facing like the wall piece `Starfield.esm:1B21E0` next to it | | |
| O2-08 | **Persistence**, protocol steps 1-4 | the crate | The crate is where it was, once (PS-02) | | |
| O2-09 | **Persistence**, protocol step 5 (long absence), then return | the crate; a vanilla item dropped in the apartment before leaving (note its base) | The crate is unchanged (persistent, PS-26); write down whether the dropped item is still there (answers PS-22/PS-26 for a temporary, player-made reference) | | |
| O2-10 | Before leaving: `player.placeatme 1E4FBD 1`, note the new reference's ID (`prid`); then protocol steps 1-4, and after O2-09's long absence look again | base `Starfield.esm:1E4FBD` | Unknown (PS-09, grade D): present or gone after the save, and after the long absence | | |
| O2-11 | `help KitKraftFirstPOICrate 4` | the crate | One line naming the reference and its runtime ID | | |
| O2-12 | Note whether the apartment's location (`help` on the cell's location, or the map) lists anything new; nothing to do if not visible | the crate's cell's location | Informational for PS-04 (no `ACPR` was written) | | |
| O2-13 | Archive naming (M4.G4): only if a package with a `- Main.ba2` was installed | - | `n/a` for the POI and rename packages (neither has an archive) | | |

## Template: session O4 (M4.O4, recipes and experiments)

Plugins: `KitKraftOwnership.esm` (15,053 bytes, SHA-256
`cc77ed38e904679536475db31371752f50176baa3aaa948d1848072cd26cc259`; override only),
`KitKraftActors.esm` (15,190 bytes, SHA-256
`837b63719261e6cebb7cd848d184795293c5a7cb9fce28f828a53790e588f27a`; owns one form). Rows for
M4.E7 (outpost object, fabricator recipe) and M4.E9 (scripts) wait for those recipes; they
are listed so the session can be planned. Runtime IDs: use the ones the recipe's
`kit testplan` prints for the session's load order.

| Row | Step | Forms | Expected | Seen | Result |
| --- | ---- | ----- | -------- | ---- | ------ |
| O4-01 | Ownership: `coc CityNewAtlantisFreestarEmbassy`; look at an item on a shelf | CELL `Starfield.esm:017307`; `XOWN.Owner` `Starfield.esm:24F839` -> `PlayerFaction` `Starfield.esm:01C21C` | The prompt says "Take", not "Steal" (`docs/recipes/ownership.md`) | | |
| O4-02 | Take the item | as O4-01 | No crime, no bounty; a "Steal" prompt on one item may be that item's own owner (`REFR` `XOWN`): note which | | |
| O4-03 | Actors: `coc CityNewAtlantisFreestarEmbassy`, `player.moveto KitKraftTestActor` (or its runtime ID) | ACHR `KitKraftActors.esm:001000` `KitKraftTestActor`, base NPC_ `Starfield.esm:271133` `Vendor_ChunksEmployee_M` | A Chunks employee near (4.9, 2.4); within a minute it walks (sandboxes) rather than standing frozen | | |
| O4-04 | Actors: **persistence**, protocol steps 1-4 | the actor | The actor is in the embassy again (where in it may differ: it moves) | | |
| O4-05 | Outpost object (M4.E7, W3): build the cloned object from the outpost build menu | per `docs/recipes/workshop.md` (not written yet) | Listed in the build menu and builds; waits for M4.E7 | | not run (recipe pending) |
| O4-06 | Fabricator recipe (M4.E7, W8) | per `docs/recipes/fabricator.md` (not written yet) | The recipe appears at a fabricator and crafts; waits for M4.E7 | | not run (recipe pending) |
| O4-07 | Script event (M4.E9, W6): the compiled script on the POI's crate shows its message on the chosen event | per `docs/recipes/scripts.md` (not written yet) | The message appears; waits for M4.E9 | | not run (recipe pending) |
| O4-08 | Procedural placement experiment (M4.E10) on a fresh landing | per M4.E10's testplan (waits for M4.D3) | Recorded either way | | not run (experiment pending) |
| O4-10 | PS-06: a script calls `SetPersistLoc` on a reference it created; leave the location, save, reload, return | per the M4.E9 script recipe | Where the reference is found (grade D) | | not run (needs M4.E9) |
| O4-11 | PS-09: a script `PlaceAtMe` without `abForcePersist`; persistence protocol 1-4, then 5 | as O4-10 | Present or gone at each step (grade D) | | not run (needs M4.E9) |
| O4-12 | PS-10: a script property naming a **temporary** vanilla reference; leave, save, reload; check the reference's state from the script | as O4-10 | Whether the property still resolves (grade D) | | not run (needs M4.E9) |
| O4-13 | PS-12: `disable` a persistent reference from the console (`prid`, `disable`), then persistence protocol 1-4 and 5 | the POI crate `KitKraftFirstPOI.esm:001000` | Whether it stays disabled after the save and after the long absence (grade D); `enable` it afterwards | | |
| O4-14 | PS-14b: a quest alias with "create ref temp" creating an object; leave, save, reload | needs CK fixture F25 | Present or gone (grade C/D) | | not run (needs F25) |
| O4-15 | PS-16: an alias filled with a temporary reference; stop the quest; leave, save, reload | needs F25 | Whether the reference keeps its changes while filled and after `Clear`/`Stop` (grade D) | | not run (needs F25) |
| O4-16 | PS-19: a script logging `OnCellAttach`, `OnLoad`, `OnCellLoad` with timestamps; enter the cell; then save and load the save inside the cell | as O4-10 | The order of the three events; whether `OnLoad` fires after loading in the cell (grade D) | | not run (needs M4.E9) |
| O4-17 | PS-20/PS-37: at an outpost, build an object with a logging script; walk away inside the same worldspace, return; save, reload | needs M4.E7 and M4.E9 | Which unload events arrive; whether the built object survives a long absence (grade D) | | not run (needs M4.E7, M4.E9) |
| O4-18 | PS-23/PS-26: `prid` a persistent and a temporary vanilla object in one cell, move both (`setpos x <value>`), then persistence protocol 1-5 | any interior with both kinds (note their FormIDs) | After the long absence: which one returned to its plugin position (grade D) | | |
| O4-19 | PS-24/PS-27/PS-28: repeat O4-18 in a location flagged never-resets (`LC165BuriedTempleLocation` `Starfield.esm:18AE75`, or a location the script flags with `SetNeverResets`) | as named | Whether the temporary object stays moved (grade D) | | |
| O4-20 | PS-32: land twice at one procedurally generated site, leaving a dropped item there the first time | the site's planet and coordinates | Whether the POI and the item are the same on the second landing (grade D) | | |
| O4-21 | PS-39: a script `SetFactionOwner` on a cell, then persistence protocol 1-5 | as O4-10 | Whether the owner change survives the long absence (grade D) | | not run (needs M4.E9) |

## Template: session O5 (M3.O5, gun overhaul build 2)

Plugin: `KitKraftGunOverhaul.esm` (45,271 bytes, SHA-256
`5e2f84e7b1dfd6a7876664a3bf203c3bbeff8679907bccd240c6ecb148f982d1`; package
`KitKraftGunOverhaul-0.2.0.zip`, SHA-256
`ec9ab1b3f59be2e2d207aed1a5103fd12471a14b498ff6fedcac414d3c045703`; override only: it may be
dropped from the save). Install and steps: `docs/overhaul/build-2.md`, "In game". Each
weapon is added with `player.additem <FormID> 1` (runtime ID = FormID, since
`Starfield.esm` loads first); ammo with `player.additem 0004AD3E 300` (7.77mm Caseless),
`002B5590 300` (6.5mm CT), `002B5595 300` (11mm Caseless).

| Row | Check | Forms | Expected (build 2) | Seen | Result |
| --- | ----- | ----- | ------------------ | ---- | ------ |
| O5-01 | C1: Old Earth Pistol and XM-2311, standard magazine and damage | WEAP `Starfield.esm:26D96C` (M1919), `Starfield.esm:24561C` (XM2311) | 7 rounds each; damage 15% below vanilla (23.8 and 30.6 base) | | |
| O5-02 | C2: hunting rifle name in inventory and at a weapon workbench | WEAP `Starfield.esm:21BBCD` `RussianHuntingRifle` | "Old Earth Marksman Rifle" (an instance-name prefix is still a pass) | | |
| O5-03 | C3: Maelstrom capacity, rate, damage, kick | WEAP `Starfield.esm:2984DF`; AMDL `Starfield.esm:1B01CA`; receiver OMOD `Starfield.esm:077F13` | 24 rounds; about 5.6 shots/s (vanilla 7.5); +35% damage (5.4); stronger vertical kick | | |
| O5-04 | C4: Kraken ammo and damage | WEAP `Starfield.esm:21FEB4`; ammo AMMO `Starfield.esm:04AD3E`, list `Starfield.esm:00E841` | Uses 7.77mm Caseless; +20% damage (3.6) | | |
| O5-05 | C5: Urban Eagle repeated trigger pulls | WEAP `Starfield.esm:26D96D` `UrbanEagle` | About 4.4 shots/s, no 1-second lockout | | |
| O5-06 | C6: Lawgiver aimed follow-up shots | WEAP `Starfield.esm:02D7F4`; receiver OMOD `Starfield.esm:057555` | About 0.64 s between aimed shots | | |
| O5-07 | C7: Tombstone damage and sustained recoil | WEAP `Starfield.esm:02EB36`; AMDL `Starfield.esm:11287A` | +25% damage (23.75); 50% stronger kick | | |
| O5-08 | C8: dropped by the owner (2026-10-15) | - | nothing to check | | n/a |
| O5-09 | C9: Beowulf ammo and magazine | WEAP `Starfield.esm:04716C`; ammo `Starfield.esm:2B5590`, list `Starfield.esm:12F0CE` | 6.5mm CT; 20 rounds with the standard magazine | | |
| O5-10 | C10: Hard Target with the Large Magazine (weapon workbench) | WEAP `Starfield.esm:0546CC`; OMOD `Starfield.esm:12D03F` `mod_HardTarget_Mag_L` | 5 standard; 7 with the Large Magazine (vanilla 10); "not checked" if the mod is not offered | | |
| O5-11 | C11: Drum Beat default build | WEAP `Starfield.esm:18DE2C`; AMDL `Starfield.esm:179DBD`, `Starfield.esm:2BF29F`; receiver OMOD `Starfield.esm:04A750` | 48 rounds with the default drum (base 18 + 30); 12 shots/s (vanilla 16); +50% damage (12); 60% more kick | | |
| O5-12 | C12: Shotty magazines (weapon workbench) | WEAP `Starfield.esm:26D960`; OMODs `Starfield.esm:132C4A` (Large), `132C49` (Extended), `132C48` (Extra Large) | 6 standard; Large 14, Extended 10, Extra Large 22 | | |
| O5-13 | C13: Breach follow-up shots | WEAP `Starfield.esm:0547A3` | 1.5 shots/s, about 0.67 s between shots | | |
| O5-14 | C14: Microgun spin-up and damage | WEAP `Starfield.esm:0546CD` | Spin-up about 0.1 s (if `FireSeconds` is the spin-up); +50% damage (13.5) | | |
| O5-15 | **Persistence**: save with the weapons in the inventory, quit, reload | the weapons above | Every weapon keeps the values seen above after the reload | | |
| O5-16 | Drop the plugin (override only): disable the mod, load the O5-15 save | the weapons | The weapons return to vanilla values and names; no warning about missing content (the plugin owns no forms) | | |

## Ledger

### game-20261014-01: the rename plugin (M2.O6), recorded after the fact

| Field | Content |
| ----- | ------- |
| Session | M2.O6 (before this ledger existed; from `plan/STATUS.md`, "M2.O6 result") |
| Date | 2026-10-14 |
| Game build | not recorded |
| MO2 profile | not recorded (a disposable save, loaded through MO2) |
| Plugins | `OldSovietMarksmanRifle.esm`, SHA-256 `9c0b28a6...` (as built, `docs/smoke-rename.md`); installed by hand, not from a package |

| Row | Step | Forms | Expected | Seen | Result |
| --- | ---- | ----- | -------- | ---- | ------ |
| O2-01 | `player.additem 0021BBCD 1`, inventory | WEAP `Starfield.esm:21BBCD` | "Old Soviet Marksman Rifle" | The item showed as "Old Soviet Marksman Rifle" | **pass** |

Finding: the first in-game confirmation of `set`/`validate`/`build` and of inline UTF-8
lstrings in a non-localized override of a localized master (already in `plan/STATUS.md`).

### game-20261015-02: gun overhaul build 2 (M3.O5), transcribed from the owner's report

Transcribed by lane F (M5.F4, 2026-10-16) from the owner's report in `plan/STATUS.md`
("M3.O5 result (2026-10-15)"). Grade: **owner report**. The owner gave a verdict per check,
not the values seen, so the Seen column holds only what was reported. The hashes were
recovered from the files afterwards; the owner did not report them.

| Field | Content |
| ----- | ------- |
| Session | O5 (M3.O5) |
| Date | 2026-10-15 |
| Game build | 1.16.244.0. Source: the headers of the profile's saves written during the sitting, `Autosave2_..._000051_20261015205716` and `Exitsave0_..._000052_20261015205905` (`kit load --profile "Kitkraft Dev" --from-save <save>`: `confirmed`, `game_version` 1.16.244.0). `kit doctor` reads the same `Starfield.exe` file version on 2026-10-16 |
| MO2 profile | `Kitkraft Dev` (the owner: "the vanilla `KitKraft Dev` profile") |
| Save | Not named by the owner. The profile's saves `..._000051` and `..._000052` (2026-10-15, 16:57 and 16:59 local time) are the first to list `KitKraftGunOverhaul.esm`; both are disposable dev-profile saves |
| Plugins | `KitKraftGunOverhaul.esm`, 45,271 bytes, SHA-256 `5e2f84e7b1dfd6a7876664a3bf203c3bbeff8679907bccd240c6ecb148f982d1`: the installed copy in `C:\Modding\MO2-Starfield\mods\KitKraftGunOverhaul\` and the build output `out\overhaul\KitKraftGunOverhaul.esm` both have it, and the package manifest `out\overhaul\package\KitKraftGunOverhaul\kitkraft-manifest.json` (SHA-256 `73527bcc...4891`) names it. The mod folder's `kitkraft-manifest.json` is byte-identical to that manifest, and MO2's `meta.ini` names `installationFile=KitKraftGunOverhaul-0.2.0.zip` (package SHA-256 `ec9ab1b3...5703`, as in the template). Runtime slot `05`, confirmed by save `..._000052`, after `KitKraftFirstPOI.esm` (`04`) |
| Other changes | `KitKraftFirstPOI.esm` also enabled (game-20261015-03). `OldSovietMarksmanRifle.esm` is not in the profile's `plugins.txt` (checked 2026-10-16) |
| Screenshots | None recorded (`observe\` has no folder for this session) |

| Row | Check | Forms | Expected (build 2) | Seen | Result |
| --- | ----- | ----- | ------------------ | ---- | ------ |
| O5-01 | C1 | WEAP `Starfield.esm:26D96C`, `Starfield.esm:24561C` | as the template | owner: pass | **pass** (owner report) |
| O5-02 | C2 | WEAP `Starfield.esm:21BBCD` | "Old Earth Marksman Rifle" | owner: the rifle shows "Old Earth Marksman Rifle" | **pass** (owner report) |
| O5-03 | C3 | WEAP `Starfield.esm:2984DF`; AMDL `Starfield.esm:1B01CA`; OMOD `Starfield.esm:077F13` | as the template | owner: pass | **pass** (owner report) |
| O5-04 | C4 | WEAP `Starfield.esm:21FEB4` | as the template | owner: pass | **pass** (owner report) |
| O5-05 | C5 | WEAP `Starfield.esm:26D96D` | as the template | owner: pass | **pass** (owner report) |
| O5-06 | C6 | WEAP `Starfield.esm:02D7F4`; OMOD `Starfield.esm:057555` | as the template | owner: pass | **pass** (owner report) |
| O5-07 | C7 | WEAP `Starfield.esm:02EB36`; AMDL `Starfield.esm:11287A` | as the template | owner: pass | **pass** (owner report) |
| O5-08 | C8 | - | dropped by the owner | - | n/a |
| O5-09 | C9 | WEAP `Starfield.esm:04716C` | as the template | owner: pass | **pass** (owner report) |
| O5-10 | C10 | WEAP `Starfield.esm:0546CC`; OMOD `Starfield.esm:12D03F` | as the template | owner: pass | **pass** (owner report) |
| O5-11 | C11 | WEAP `Starfield.esm:18DE2C`; AMDL `Starfield.esm:179DBD`, `Starfield.esm:2BF29F`; OMOD `Starfield.esm:04A750` | as the template | owner: pass | **pass** (owner report) |
| O5-12 | C12 | WEAP `Starfield.esm:26D960`; OMOD `Starfield.esm:132C4A`, `132C49`, `132C48` | as the template | owner: pass | **pass** (owner report) |
| O5-13 | C13 | WEAP `Starfield.esm:0547A3` | as the template | owner: pass | **pass** (owner report) |
| O5-14 | C14 | WEAP `Starfield.esm:0546CD` | as the template | owner: pass | **pass** (owner report) |
| O5-15 | Persistence: save, quit, reload with the weapons | the weapons | as the template | not reported | not run (not reported) |
| O5-16 | Drop the plugin, load the O5-15 save | the weapons | as the template | not reported | not run (not reported) |

Findings: none new. The overhaul's game cells in `docs/foothold/gap-matrix.md` (`WEAP`,
`AMDL`, `OMOD`, targeted edits) cite this entry. Their save/reload cells stay `not-run`
(O5-15).

### game-20261015-03: first POI found in game (M4.O2), transcribed from the owner's report

Transcribed by lane F (M5.F4, 2026-10-16) from the owner's report in `plan/STATUS.md`
("M4.O2 result (2026-10-15)") and commit `a5f08c9`. Grade: **owner report**. The hash was
recovered from the files afterwards.

| Field | Content |
| ----- | ------- |
| Session | O2 (M4.O2), first part: the crate only |
| Date | 2026-10-15 |
| Game build | 1.16.244.0 (save headers, as in game-20261015-02) |
| MO2 profile | `Kitkraft Dev` |
| Save | Not named by the owner. Every modded save of the profile from that day lists `KitKraftFirstPOI.esm`: `Autosave1_..._000039_20261015204005` (16:40 local time, before the overhaul was added), `..._000051` and `..._000052` |
| Plugins | `KitKraftFirstPOI.esm`, 1,821 bytes, SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1`: the installed copy in `C:\Modding\MO2-Starfield\mods\Kitkraft POI dev\` (file time 2026-10-14 21:18, before the session), the build output `out\poi\KitKraftFirstPOI.esm` and the template above agree. **Not installed from a package**: the mod folder was made by hand (its `meta.ini` names no installation file), it holds no `kitkraft-manifest.json`, and `out\poi` holds no package, so no manifest check was possible. Runtime slot `04`, confirmed by saves `..._000051` and `..._000052` (`docs/poi/first-poi-investigation.md`) |
| Other changes | `KitKraftGunOverhaul.esm` added later the same day (game-20261015-02). The rename package was not installed |
| Screenshots | None recorded |

| Row | Step | Forms | Expected | Seen | Result |
| --- | ---- | ----- | -------- | ---- | ------ |
| O2-02 | Install the rename package, manifest check | `OldSovietMarksmanRifle.esm` | as the template | not reported | not run (not reported) |
| O2-03 | Install the POI package, manifest check | `KitKraftFirstPOI.esm` | as the template | installed by hand, not from a package | not run (no package used) |
| O2-04 | `bat kitkraft-test` | - | as the template | not reported | not run (not reported) |
| O2-05 | `coc CityNewAtlantisApartmentTEMPLATE` | CELL `Starfield.esm:21966F` | The player stands in the apartment | owner: used it to reach the cell | **pass** (owner report) |
| O2-06 | `player.moveto 04001000` (the runtime ID, not the EditorID form) | REFR `KitKraftFirstPOI.esm:001000`, runtime `04001000` | The player is moved to the crate | owner: reached the crate. Clicking the crate to read its ID was not reported | **pass** (owner report) |
| O2-07 | `prid`, `getpos`, screenshot | base STAT `Starfield.esm:1E4FBD` | crate at (-17.0, -4.0, 4.63) | not reported | not run (not reported) |
| O2-08 | Persistence, protocol steps 1-4 | the crate | The crate is where it was, once | "pending owner confirmation" | not run (pending owner) |
| O2-09 | Persistence, step 5 | the crate | as the template | not reported | not run (not reported) |
| O2-10 | `player.placeatme 1E4FBD 1`, persistence | base `Starfield.esm:1E4FBD` | unknown (PS-09) | not reported | not run (not reported) |
| O2-11 | `help KitKraftFirstPOICrate 4` | the crate | as the template | not reported | not run (not reported) |
| O2-12 | The apartment's location | - | informational (PS-04) | not reported | not run (not reported) |
| O2-13 | Archive naming | - | `n/a` for these packages | - | n/a |

Findings: (1) The first in-game sighting of a new form that KitKraft wrote: a persistent
`REFR` placed in a carried copy of a vanilla interior cell loads and is found at runtime ID
`04001000`, the full slot the saves give. That supports the slot rule
`runtime-slots-by-scale` for one full slot (`docs/load-order.md`); no `FE` or `FD` ID was
checked. (2) An earlier sitting the same day tried `02001000`, which was the wrong ID, and
`28001000` was the testplan's wrong prediction; `docs/poi/first-poi-investigation.md`
records that sitting, and it has no entry here. The POI cells of
`docs/foothold/gap-matrix.md` (`TES4`, `GRUP`, `CELL`, `REFR`, master scales, new forms)
cite this entry; every save/reload cell stays `not-run` (O2-08).

### game-20261016-T1: session facts (M4.T1), research; fact (a) attempted, (b) to (h) wait on M4.O7

| Field | Content |
| ----- | ------- |
| Session | M4.T1 (lane C), research by an agent; the game was not launched and nothing was written under `C:\Modding\MO2-Starfield` |
| Date | 2026-10-16 |
| Game build | not involved |
| MO2 | 2.5.2 revision 9c130cbf, usvfs 0.5.6.1 (`logs\mo_interface.log` line 2, written by MO2 on 2026-10-15) |
| MO2 profile | none used; `Kitkraft Dev` and `ModOrganizer.ini` read only |
| Screenshots | none |

| Row | Fact | Observation | Grade | Result |
| --- | ---- | ----------- | ----- | ------ |
| a | The MO2 command line that starts a named executable on a named profile | `ModOrganizer.exe --help` was **not run**. `tasklist` showed `ModOrganizer.exe` (pid 8784) running: the owner's instance. ADR 0010 section 1 treats a second MO2 on the instance as unsafe, and starting the binary could write into the instance. Observed instead, from files MO2 wrote: version 2.5.2, a portable instance, and the executable titles `1 SFSE` (`sfse_loader.exe`), `2 Starfield`, `3 Explore Virtual Folder`, `4 CreationKit`, `5 Creation Kit` (`[customExecutables]`). Candidate command line: `ModOrganizer.exe -p "<profile>" "moshortcut://:SFSE"` | A for the version and the titles; D for the candidate | not-run (the owner's MO2 was running) |
| b | V0: Console API 2.17.0 answers `GetSFSEVersion` over HTTP | Console API is not installed in `Kitkraft Dev` | - | not-run (needs M4.O7) |
| c..h | `bAlwaysActive`, `load <save>`, `bat`, runtime IDs, `{Form:...}`, the Papyrus log path | - | - | not-run (needs M4.O7) |

Finding (for the owner, M4.O7): `Kitkraft Dev\settings.ini` has `LocalSettings=false`
(`LocalSaves=true`). The profile's `StarfieldCustom.ini` has only `[Archive]` keys and an
empty `[General]`, so none of the session switches are set. Turn on profile-specific game
INI files for the profile in MO2 before setting the switches there, or the game reads the
INIs under `My Games` instead. `kit game session` refuses this state (`profile-settings`,
`ini-switch`).

To close (a): with MO2 closed, run `ModOrganizer.exe --help` from
`C:\Modding\MO2-Starfield` and copy its output into this row. Then set `[launch] evidence`
in `crates/game/profiles/starfield.toml` to `game-20261016-T1/a`, or correct the
command line there first if the help text shows a different one.

### game-20261018-01: workbench ledger, 2026-10-18, `KitKraft Dev`

| Field | Content |
| ----- | ------- |
| Session | Rows recorded from the workbench's Ledger panel (`kit ledger`), one per confirmed click |
| Date | 2026-10-18 |
| Game build | not recorded |
| MO2 profile | `KitKraft Dev` |

| Row | Step | Forms | Expected | Seen | Result |
| --- | ---- | ----- | -------- | ---- | ------ |
| custom | custom: ADR 0009 Am. 2: MO2 2.5.2 keeps a deploy.enable edit when it opens KitKraft Dev | `Minimum Enemy Level.esm`, `KitKraftMELDLC_ShatteredSpace.esm`, `KitKraftMELDLC_TerranArmada.esm` | - | Owner opened MO2 on Kitkraft Dev after kit deploy enable (backup 20261019T013513Z-26076): Minimum Enemy Level and KitKraft Dev - KitKraftMELDLC_ShatteredSpace / _TerranArmada enabled, their plugins ticked and last in that order. | pass |
| custom | custom: ADR 0009 Am. 3: MO2 2.5.2 shows a deploy-written meta.ini (version, category) | `MEL DLC - Shattered Space.esm`, `MEL DLC - Terran Armada.esm` | - | Owner opened MO2 on Kitkraft Dev after deploy --category Gameplay of the renamed MEL DLC packages and deploy enable/disable: KitKraft Dev - MEL DLC - Shattered Space / Terran Armada enabled, version 1.0.0, category Gameplay, plugins last; old KitKraftMELDLC_* mods disabled. | pass |

### game-20261101-T1: session facts (M4.T1), agent-run on `Kitkraft Dev`; (a), (b) and (d) observed

| Field | Content |
| ----- | ------- |
| Session | M4.T1, run by an agent with the owner's approval in chat on 2026-11-01 ("go ahead with 2 and 3"); script `observe/2026-11-01-T1-v0/v0.ps1`, log `v0.log` (run 2) and `v0-run1.log` (run 1), screenshots beside them (gitignored) |
| Date | 2026-11-01, 01:35 (run 1) and 01:41 (run 2) |
| Game build | 1.16.244.0; SFSE 0.2.21 (runtime `01100F40`); Console API and Web Application, Console Command Runner Updated 1.11.0, Address Library for SFSE Plugins (owner-installed, STATUS row 10) |
| MO2 | 2.5.2, portable `C:\Modding\MO2-Starfield`, closed before each run |
| MO2 profile | `Kitkraft Dev`, profile-specific INIs and saves; base save `KitKraftBase.sfs`, SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf` |
| Endpoint | `http://127.0.0.1:45555/console` (the plugin's `iPort=45555`: Windows excludes TCP 55477-55576 on this box, so 55555 could not bind) |

| Row | Fact | Observation | Grade | Result |
| --- | ---- | ----------- | ----- | ------ |
| a | The MO2 command line that starts a named executable on a named profile | `ModOrganizer.exe -p "Kitkraft Dev" "moshortcut://:SFSE"` with MO2 closed started SFSE and the game (window after 14 s); the same form with `Creation Kit` started the CK in the probe of 01:21. MO2 stayed open after the game ended and closed on a window close | A | pass |
| b | V0: Console API answers `GetSFSEVersion` over HTTP | At the main menu, `POST /console` with the body `GetSFSEVersion` answered HTTP 200 with the console echo and `SFSE version: 0.2.21, release idx 26, runtime 01100F40` (run 2, 91 s after launch, after a 75 s settle) | A | pass |
| b' | Readiness | The port accepts connections about 5 s after launch, long before the game can take a command. In run 1 a `GetSFSEVersion` sent 8 s after launch answered HTTP 200 with an empty body, and the game process ended within 15 s with no Application event; run 2, which sent nothing until the game window had been up 75 s, ran normally. So: an open port is not readiness, and a command sent during startup can end the game (one occurrence, cause not isolated) | B | finding |
| d | `load <save>` from the main menu | `load KitKraftBase` answered HTTP 200 (echo only) and the game loaded the save: `player.getpos x/y/z` gave -13.22, 4.69, 4.66 and the screenshot shows the apartment interior. `GetInCell CityNewAtlantisApartmentTEMPLATE` answered "Item ... not found for parameter Cell": that EditorID is not accepted there, so a cell check needs another form | A for the load; the cell check form is open | pass |
| h | The Papyrus log path | `Documents\My Games\Starfield\Logs\Script\Papyrus.0.log` was rewritten during run 2 (01:43), so the profile's `[Papyrus]` switches take effect under MO2. `StarfieldConsoleHistory.log` did not change: commands sent over the API are not appended to it, or it is written only on a normal exit (the game was ended without one) | A for Papyrus; history unresolved | pass (Papyrus) |
| i | A reference's EditorID in the console | `prid KitKraftFirstPOICrate` answered "Item 'KitKraftFirstPOICrate' not found for parameter ObjectReferenceID."; `help KitKraftFirstPOICrate 4` and `help KitKraftFirstPOI 4` listed nothing. Reference EditorIDs do not resolve at runtime (`v1-timing.log`, `v2-ids.log`, 02:04 and 02:07) | A | finding |
| e | Runtime IDs | `prid 06001000` selected our crate: `getpos x/y/z` gave -17.00, -4.00, 4.63 (the plan's position) and `getdistance player` 9.48. 06 is the slot `kit testplan` infers by `runtime-slots-by-scale` for the load order of the session (our plugin last, after `kit deploy enable`); `prid 04001000`, the slot the base save's plugin list gave, was "not found". So the game numbers by the current load order, not the save's | A | pass (inferred slots); the `--from-save` rule is wrong when the order changed |
| j | A cell's EditorID and FormID in `getincell` | `player.getincell CityNewAtlantisApartmentTEMPLATE` printed nothing after the echo; `player.getincell 0021966F` and `player.getincell 21966F` printed `GetInCell >> 1.00` | A | finding |
| k | Commands that do not exist or answer late | `isdisabled`: "Script command \"isdisabled\" not found."; with no reference selected (after a failed `prid`) `getpos`, `getdistance` print only the echo; right after `player.moveto 06001000` the next `player.getpos x` printed only the echo (the move in progress) | A | finding |
| l | Answer shape | The echo, then `Command: <command>`, then result lines such as `GetPos: X >> -17.00`, `GetInCell >> 1.00`, `GetDistance >> 9.48`; failures as an `Item ... not found` or `Script command ... not found` line. Each call took about 120 ms (the plugin's `iExecTimeout=100`) | A | finding |
| c, e..g | `bAlwaysActive` behaviour unfocused, `bat`, runtime IDs, `{Form:...}` | not tried | - | not-run |

The game was ended by stopping its process (no save, no `qqq`), as `game.stop` does.

### game-20261101-01: agent session 2026-11-01-T01 (`kit game session`), KitKraftFirstPOI.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-01-T01`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-01 |
| Status | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:/Modding/MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261101-01` |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `observe/2026-11-01-poi-plan/probes.toml` SHA-256 `e739f90babeecf0ccaedf89d955d50ce02ef9afb016c76e5c05982ac5482458e` |
| Screenshots | `C:\Modding\kitcraft\observe\2026-11-01-T01` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | coc CityNewAtlantisApartmentTEMPLATE: the cell of REFR `KitKraftFirstPOICrate` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |
| T-02 | player.moveto KitKraftFirstPOICrate: stand at REFR `KitKraftFirstPOICrate` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 04001000 | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |
| T-03 | help KitKraftFirstPOICrate 4 names REFR `KitKraftFirstPOICrate` with its runtime ID (confirmed-by-save) | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 04001000 | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |
| T-04 | prid KitKraftFirstPOICrate; getpos x/y/z: REFR `KitKraftFirstPOICrate` at (-17.0, -4.0, 4.6) | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 04001000 | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |
| T-05 | persistence 1-3: leave, save to the session's own save, load it | - | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |
| T-06 | persistence 4: return; every new reference where it was | KitKraftFirstPOICrate | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |

Finding: timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)

Cause (director, 2026-11-01): the game never started. MO2's `logs\mo_interface.log` shows `Error 5 ERROR_ACCESS_DENIED` spawning `sfse_loader.exe`: the session was started from an agent's sandboxed shell, whose child processes may not start the hooked loader. Rerun from an unsandboxed shell as `game-20261101-02`. The controller behaved as designed: nothing sent, every row not-run, profile files unchanged.

### game-20261101-02: agent session 2026-11-01-T02 (`kit game session`), KitKraftFirstPOI.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-01-T02`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-01 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:/Modding/MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261101-02` |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `observe/2026-11-01-poi-plan/probes.toml` SHA-256 `e739f90babeecf0ccaedf89d955d50ce02ef9afb016c76e5c05982ac5482458e` |
| Screenshots | `C:\Modding\kitcraft\observe\2026-11-01-T02` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell CityNewAtlantisApartmentTEMPLATE` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE | regex >>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; player.getincell CityNewAtlantisApartmentTEMPLATE -> player.getincell CityNewAtlantisApartmentTEMPLATE Command: player.getincell CityNewAtlantisApartmentTEMPLATE | fail (/>>\s*1(\.0+)?\s*$/ does not match `player.getincell CityNewAtlantisApartmentTEMPLATE Command: player.getincell CityNewAtlantisApartmentTEMPLATE`) | - |
| T-02 | `player.moveto KitKraftFirstPOICrate`, `prid KitKraftFirstPOICrate`, `getdistance player` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 04001000 | number 0 +/- 500 | player.moveto KitKraftFirstPOICrate -> player.moveto KitKraftFirstPOICrate Command: player.moveto KitKraftFirstPOICrate; prid KitKraftFirstPOICrate -> prid KitKraftFirstPOICrate Command: prid KitKraftFirstPOICrate; getdistance player -> getdistance player Command: getdistance player | inconclusive (no number in `getdistance player Command: getdistance player`) | - |
| T-03 | `help KitKraftFirstPOICrate 4` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 04001000 | contains 04001000 | help KitKraftFirstPOICrate 4 -> help KitKraftFirstPOICrate 4 | fail (`04001000` not in `help KitKraftFirstPOICrate 4`) | - |
| T-04 | `prid KitKraftFirstPOICrate`, `getpos x`, `getpos y`, `getpos z`, `isdisabled`, `prid 04001000`, `getpos x` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 04001000 | number -17 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5; regex >>\s*0(\.0+)?\s*$; number -17 +/- 0.5 | prid KitKraftFirstPOICrate -> prid KitKraftFirstPOICrate Command: prid KitKraftFirstPOICrate; getpos x -> getpos x Command: getpos x; getpos y -> getpos y Command: getpos y; getpos z -> getpos z Command: getpos z; isdisabled -> isdisabled Command: isdisabled; prid 04001000 -> prid 04001000 Command: ... | fail (/>>\s*0(\.0+)?\s*$/ does not match `isdisabled Command: isdisabled`) | T-04-1.png |
| T-05 | `coc CityNewAtlantisLodgeInt`, `save KitKraftSession-20261101-02`, `load KitKraftSession-20261101-02`, `player.getincell CityNewAtlantisLodgeInt` | - | regex >>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; save KitKraftSession-20261101-02 -> save KitKraftSession-20261101-02 Command: save KitKraftSession-20261101-02; load KitKraftSession-20261101-02... | fail (/>>\s*1(\.0+)?\s*$/ does not match `player.getincell CityNewAtlantisLodgeInt Command: player.getincell CityNewAtlantisLodgeInt`) | - |
| T-06 | `coc CityNewAtlantisApartmentTEMPLATE`, `prid KitKraftFirstPOICrate`, `getpos x`, `getpos y`, `getpos z` | KitKraftFirstPOICrate | number -17 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; prid KitKraftFirstPOICrate -> prid KitKraftFirstPOICrate Command: prid KitKraftFirstPOICrate; getpos x -> getpos x Command: getpos x; getpos y -> getpos y Command: getpos y; ge... | inconclusive (no number in `getpos x Command: getpos x`) | T-06-1.png |

Finding: C:/Modding/MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (acad980c9b1f9aca7505410c35323e626a2f65967f349d93e80810d5d6673315 -> 5c8e159639ca0b87d16777c0a70ede3565a64673bf3e51f2665fe1d46698f326): restored from C:\Modding\kitcraft\observe\2026-11-01-T02\backup\01-plugins.txt

Cause (director, 2026-11-01): the session ran end to end (launch, settle, load, six rows, session save and load, stop, restore), but the probes were not executable as written: they address the crate and the cell by EditorID, which the console does not resolve, use `isdisabled`, which does not exist, and expect the base save's runtime slot 04 while the game used 06 (see `game-20261101-T1` rows e, i to l). The `plugins.txt` change is the game's or MO2's rewrite during the session; the controller restored it. `kit testplan` is being fixed to use runtime FormIDs, the current load order's slots and a wait after moves.

### game-20261101-03: agent session 2026-11-01-T03 (`kit game session`), KitKraftFirstPOI.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-01-T03`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-01 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:/Modding/MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261101-03` |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `observe/2026-11-01-poi-plan2/probes.toml` SHA-256 `bb3f88e7c5f041ebf0e57818760f1d7de843fccbfca8e22d1482ab6ded3d3170` |
| Screenshots | `C:\Modding\kitcraft\observe\2026-11-01-T03` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| T-02 | `player.moveto 06001000`, `prid 06001000`, `getdistance player` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 06001000 | number 0 +/- 500 | player.moveto 06001000 -> player.moveto 06001000 Command: player.moveto 06001000; (waited 10s); prid 06001000 -> prid 06001000 Command: prid 06001000; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.86 | **pass** | - |
| T-03 | `prid 06001000`, `getisid 001E4FBD` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 06001000 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 06001000 -> prid 06001000 Command: prid 06001000; getisid 001E4FBD -> getisid 001E4FBD Command: getisid 001E4FBD GetIsID >> 1.00 | **pass** | - |
| T-04 | `prid 06001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 06001000 | number -17 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 06001000 -> prid 06001000 Command: prid 06001000; getpos x -> getpos x Command: getpos x GetPos: X >> -17.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-04-1.png |
| T-05 | `coc CityNewAtlantisLodgeInt`, `save KitKraftSession-20261101-03`, `load KitKraftSession-20261101-03`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29 | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); save KitKraftSession-20261101-03 -> save KitKraftSession-20261101-03 Command: save KitKraftSession-20261101-03; load KitKraftSessi... | **pass** | - |
| T-06 | `coc CityNewAtlantisApartmentTEMPLATE`, `prid 06001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftFirstPOICrate, 06001000 | number -17 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); prid 06001000 -> prid 06001000 Command: prid 06001000; getpos x -> getpos x Command: getpos x GetPos: X >> -17.00; getpos... | **pass** | T-06-1.png |

Finding: C:/Modding/MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (c61020098aa6d2c789186aa1daf8628dbcecf38c25fd4fc0db222ab985e9516f -> c7bacaacfa976b844dccec4ed93c3eca9ec913f0c1f7e81e6dc5ec0a454e76f2): restored from C:\Modding\kitcraft\observe\2026-11-01-T03\backup\01-plugins.txt

Director's reading (2026-11-01): **rows T-01 to T-04 and T-06 are real passes**, the first unattended in-game check of a KitKraft plugin: the crate `KitKraftFirstPOI.esm:001000` is in game at runtime ID `06001000`, in cell `0021966F` (`GetInCell >> 1.00`), its base is `001E4FBD` (`GetIsID >> 1.00`), at (-17.00, -4.00, 4.63) (the plan's position), and `player.moveto` lands beside it (`GetDistance >> 0.86`); after leaving and returning (T-06) it is still there. **T-05 is a false pass**: `load KitKraftSession-20261101-03` answered "Save file not found." because it was sent about 120 ms after `save`, before the file existed (it was written at 02:54); the final `getincell` held only because the player never left the cell. Save/reload stays `not-run`. The controller is being changed to fail a row on any console "not found" line and testplan to wait after `save`. The status is inconclusive because `plugins.txt` changes in every session: MO2 rewrites it when it launches the game (opening and closing MO2 alone leaves it unchanged); the controller restored it.

### game-20261101-04: agent session 2026-11-01-T04 (`kit game session`), KitKraftFirstPOI.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-01-T04`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-01 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:/Modding/MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261101-04` |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `observe/2026-11-01-poi-plan3/probes.toml` SHA-256 `6152085153e2c0d59bf84a2298758fbf3fa7e1bdfad2216fb3cd962cf13d35d8` |
| Screenshots | `C:\Modding\kitcraft\observe\2026-11-01-T04` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| T-02 | `player.moveto 06001000`, `prid 06001000`, `getdistance player` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 06001000 | number 0 +/- 500 | player.moveto 06001000 -> player.moveto 06001000 Command: player.moveto 06001000; (waited 10s); prid 06001000 -> prid 06001000 Command: prid 06001000; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.86 | **pass** | - |
| T-03 | `prid 06001000`, `getisid 001E4FBD` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 06001000 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 06001000 -> prid 06001000 Command: prid 06001000; getisid 001E4FBD -> getisid 001E4FBD Command: getisid 001E4FBD GetIsID >> 1.00 | **pass** | - |
| T-04 | `prid 06001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 06001000 | number -17 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 06001000 -> prid 06001000 Command: prid 06001000; getpos x -> getpos x Command: getpos x GetPos: X >> -17.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-04-1.png |
| T-05 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261101-04`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261101-04`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00; save KitKraftSession-... | **pass** | - |
| T-06 | `coc CityNewAtlantisApartmentTEMPLATE`, `prid 06001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftFirstPOICrate, 06001000 | number -17 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); prid 06001000 -> prid 06001000 Command: prid 06001000; getpos x -> getpos x Command: getpos x GetPos: X >> -17.00; getpos... | **pass** | T-06-1.png |

Finding: C:/Modding/MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (c61020098aa6d2c789186aa1daf8628dbcecf38c25fd4fc0db222ab985e9516f -> c7bacaacfa976b844dccec4ed93c3eca9ec913f0c1f7e81e6dc5ec0a454e76f2): restored from C:\Modding\kitcraft\observe\2026-11-01-T04\backup\01-plugins.txt

Director's reading (2026-11-01): **all six rows are real passes**, unattended. Besides T-01 to T-04 as in `game-20261101-03`, the persistence row now proves a reload: in the Lodge (`0006BB29`) the session saved, moved to the apartment (`GetInCell 0021966F >> 1.00`), loaded the save and was back in the Lodge (`GetInCell 0006BB29 >> 1.00`); T-06, after that reload, found the crate at (-17.00, -4.00, 4.63) again. This closes the first POI's save/reload row (M4.O2, `plan/STATUS.md` row 2), which waited on the owner. The status reads inconclusive only because MO2 rewrites `plugins.txt` when it launches the game (restored by the controller; a follow-up should treat that rewrite as expected).

### game-20261102-01: M4.T5, V2 read off `game-20261101-04` (desk entry; no game run)

Desk work on 2026-11-02 (sprint pipeline X, ticket X4); no game was launched. M4.T5 (V2)
asks for every O2 template row filled from a controller session, the manual rows named, the
`fail`s filed, and PS-02, PS-09 and PS-26 graded. `game-20261101-04` is the first session
whose six probe rows are all real passes (the director's reading under that entry). This
entry maps each O2 row to that session's rows, or to the earlier entries where the session
has none. Every result below cites the row it rests on; nothing is upgraded beyond what a
row shows.

| Field | Content |
| ----- | ------- |
| Id | `game-20261102-01` |
| Session read | `game-20261101-04` (agent session `2026-11-01-T04`, `Kitkraft Dev`, base save `KitKraftBase` SHA-256 `fbed2cc2...eb5ccf`, `KitKraftFirstPOI.esm` SHA-256 `13c22b48...57b5f1`, probes `observe/2026-11-01-poi-plan3/probes.toml` SHA-256 `61520851...13d35d8`); also `game-20261014-01`, `game-20261015-03` and `game-20261101-T1` |
| Status of the session | `inconclusive` as written, from one profile-file finding only (MO2 rewrote `plugins.txt` at launch; restored). No row's evidence is affected: the six rows ran on the profile as the preflight found it |
| Runtime ID | The crate is `06001000` in this load order (the plugin last after `kit deploy enable`, `game-20261101-T1` row e). The template's `04001000` was the owner's slot of 2026-10-15; the ID depends on the load order, not on the plugin |

| Template row | Kind (`docs/game-sessions.md`) | Session rows | What they show | Result |
| ------------ | ------------------------------ | ------------ | -------------- | ------ |
| O2-01 | probe | - (not in the POI plan) | The rename in game, owner, `game-20261014-01` | **pass** (owner, 2026-10-14) |
| O2-02 | manual | - | Installing a package through MO2 is the owner's; the session deploys with `kit deploy --dev`, not a package | not-run (owner, W12) |
| O2-03 | manual | - | As O2-02; the session's copy was deployed by `kit deploy --dev`, and its SHA-256 matches the template's | not-run (owner) |
| O2-04 | manual | - | The controller never sends `bat`; T-01 to T-04 send the batch's commands one by one | not-run (`bat` stays grade D) |
| O2-05 | probe | T-01 | `coc CityNewAtlantisApartmentTEMPLATE`, then `player.getincell 0021966F` answered `GetInCell >> 1.00` | **pass** |
| O2-06 | probe | T-02, T-03 | `player.moveto 06001000`, then `getdistance player` answered 0.86; `prid 06001000` with `getisid 001E4FBD` answered 1.00. That is the crate, with the plugin's base, by its runtime ID. Clicking the crate is replaced by `prid`, since input injection is deferred | **pass** |
| O2-07, position | probe | T-04 | `getpos x/y/z` answered -17.00, -4.00, 4.63, the plugin's `DATA` | **pass** |
| O2-07, facing | probe (screenshot) | T-04 (`T-04-1.png`) | No rotation getter was sent. The facing is in the screenshot only, which nobody has looked at against the wall piece `Starfield.esm:1B21E0` | **inconclusive** (screenshot only, owner to look) |
| O2-08, position after a reload | probe | T-05, T-06 | Step 1 (leave): `coc CityNewAtlantisLodgeInt`, `GetInCell 0006BB29 >> 1.00`. Step 3 (save and reload): `save KitKraftSession-20261101-04`, then `coc` to the apartment (`GetInCell 0021966F >> 1.00`), `load` of that save, and back in the Lodge (`GetInCell 0006BB29 >> 1.00`), so the load happened. Step 4 (return): T-06 `getpos` gave -17.00, -4.00, 4.63 again | **pass** |
| O2-08, "once" | probe | - | A second copy of the crate would have another reference ID, which `prid 06001000` does not see; no row looks for one | **inconclusive** (no duplicate check) |
| O2-08, protocol steps 2 and 3 as written | - | T-05 | Step 2 (`wait` one game hour) is not sent (no console command chosen, grade D). Step 3 loads from the world, not after quitting to the main menu | deviation, recorded |
| O2-09 | manual | - | The long absence (168 game hours) has no chosen console route (FTH-EXP-07) | not-run |
| O2-10 | manual | - | `placeatme` makes a reference whose ID is known only at run time | not-run |
| O2-11, as written | probe | T-03 | The template expects `help KitKraftFirstPOICrate 4` to name the reference. `game-20261101-T1` row i: it lists nothing, because a reference's EditorID does not resolve in the console. The expectation cannot hold, and the template is wrong | **fail** (the template's expectation, not the plugin) |
| O2-11, intent (find the reference) | probe | T-03 | Found by runtime ID: `prid 06001000`, `getisid 001E4FBD >> 1.00` | **pass** |
| O2-12 | manual | - | The map and location UI | not-run (PS-04 stays D) |
| O2-13 | manual | - | Neither package has an archive | n/a |

The persistence claims of `docs/design/persistence.md` that these rows test:

| Claim | Grade before | What `game-20261101-04` shows | Grade after |
| ----- | ------------ | ----------------------------- | ----------- |
| PS-02 (`kit place` with `persistent = true` writes type 8 with `0x400`; the crate is so written) | A (corpus) | Its tests O2-07 and O2-08 pass for position. The persistent crate loads at its `DATA` position and is there after a save and reload. That it is there once is not checked | **A**, unchanged (a fact about the file). Its in-game test now has evidence: O2-07 position and O2-08 position after a reload pass; O2-08 "once" is inconclusive |
| PS-09 (is a `PlaceAtMe` reference without `abForcePersist` cleaned up, and does it survive a save and reload?) | D | Nothing: O2-10 has no probe, and the session placed nothing at run time | **D**, unchanged; test O2-10 not-run (O4-11 needs a script) |
| PS-26 (after a reset, does a persistent reference keep its plugin state while temporary ones return?) | D | Nothing: a save and reload is not a reset, and O2-09's long absence was not run | **D**, unchanged; tests O2-09 and O4-18 not-run |

Findings, one line each:

- O2-11's template expectation is wrong. `help <reference EditorID> 4` lists nothing
  (`game-20261101-T1` row i). The row should say: find the reference by its runtime ID
  (`prid`, `getisid`), as T-03 does. For the template's owner (pipeline X, `docs/game-observations.md`
  "Template: session O2").
- O2-08's "once" has no probe. A count of the plugin's base in the cell after the reload would
  close it; no console getter for that was chosen (the session lane, `kit testplan`).
