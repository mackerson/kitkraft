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
| O2-11 | Find the crate by its runtime ID: `prid <the plan's runtime ID>` (`06001000` on Kitkraft Dev in `game-20261101-04`), then `getisid 001E4FBD`; as a control in the same session, `help Crate_MilitarySupply_01 4` (a base form's EditorID) | the crate; base STAT `Starfield.esm:1E4FBD` | `prid` selects the crate and `getisid` answers 1; the control lists the base form (so `help` answers within the console tool's capture window). A reference's own EditorID is not looked up: the console does not resolve it (`game-20261101-T1` row i). Corrected 2026-11-02 (ruling R19): the row first expected `help KitKraftFirstPOICrate 4` to name the reference | | |
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

Correction (2026-11-17, `game-20261117-08`): the crate was reached by `player.moveto`, not
on foot. The owner never found it walking ("I was also never able to find that crate, and
I think now the reason for that is it's outside the bedroom"); the cell's floor pieces put
(-17, -4) on the bedroom's east wall line. O2-05 and O2-06 stand as written: they show the
reference loads in the carried cell, not that a player can reach it (`docs/poi/first-poi.md`,
"Correction").

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
| O2-06, moved to the crate | probe | T-02 | `player.moveto 06001000`, then `getdistance player` answered 0.86 | **pass** |
| O2-06, the ID shown on a click | manual | - | Nothing was clicked: input injection is deferred (ADR 0010 Amendment 1 (b)). `prid` with the ID the plan predicted (T-03) does not show what a click shows. The slot is known from `game-20261101-T1` row e | not-run (regraded 2026-11-02, review L1: the row was graded pass from T-03, which checks another expectation) |
| O2-07, position | probe | T-04 | `getpos x/y/z` answered -17.00, -4.00, 4.63, the plugin's `DATA` | **pass** |
| O2-07, facing | probe (screenshot) | T-04 (`T-04-1.png`) | No rotation getter was sent. The facing is in the screenshot only, which nobody has looked at against the wall piece `Starfield.esm:1B21E0` | **inconclusive** (screenshot only, owner to look) |
| O2-08, position after a reload | probe | T-05, T-06 | Step 1 (leave): `coc CityNewAtlantisLodgeInt`, `GetInCell 0006BB29 >> 1.00`. Step 3 (save and reload): `save KitKraftSession-20261101-04`, then `coc` to the apartment (`GetInCell 0021966F >> 1.00`), `load` of that save, and back in the Lodge (`GetInCell 0006BB29 >> 1.00`), so the load happened. Step 4 (return): T-06 `getpos` gave -17.00, -4.00, 4.63 again | **pass** |
| O2-08, "once" | probe | - | A second copy of the crate would have another reference ID, which `prid 06001000` does not see; no row looks for one | **inconclusive** (no duplicate check) |
| O2-08, protocol steps 2 and 3 as written | - | T-05 | Step 2 (`wait` one game hour) is not sent (no console command chosen, grade D). Step 3 loads from the world, not after quitting to the main menu | deviation, recorded |
| O2-09 | manual | - | The long absence (168 game hours) has no chosen console route (FTH-EXP-07) | not-run |
| O2-10 | manual | - | `placeatme` makes a reference whose ID is known only at run time | not-run |
| O2-11, as written | probe | - (`game-20261101-T1` row i) | The template expects `help KitKraftFirstPOICrate 4` to name the reference. `game-20261101-T1` row i: `prid KitKraftFirstPOICrate` answered "Item ... not found for parameter ObjectReferenceID", and `help KitKraftFirstPOICrate 4` listed only its two empty section headers, within the console tool's 100 ms capture window. A reference's EditorID does not resolve in the console, so the expectation cannot hold: the template is wrong | **fail** (the template's expectation, not the plugin; ruling R19: the template is corrected and the row re-run in X9) |
| O2-11, intent (find the reference) | probe | - | Not graded from T-03, which is O2-06's evidence (a runtime ID the plan predicted, not the lookup O2-11 asks for). Ruling R19: the corrected row runs in X9 before any grade | not-run (regraded 2026-11-02, review M2: was pass from T-03) |
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
  "Template: session O2"); ruling R19 (18:10) has it corrected and re-run in X9.
- O2-11's `help` result has no positive control: no `help <base form EditorID> 4` was sent
  through the console tool in the same 100 ms capture window. X9 sends
  `help Crate_MilitarySupply_01 4` beside the corrected row (review L4).
- Regraded 2026-11-02 after review (rulings R19, review M2 and L1): O2-06 is split, its
  click half not-run; O2-11 "intent" pass -> not-run. Nothing else changed.
- Template corrected 2026-11-02 (ruling R19): "Template: session O2" row O2-11 now finds the
  crate by its runtime ID with a `help <base EditorID> 4` control, instead of
  `help KitKraftFirstPOICrate 4`. The fail above grades the old text; the corrected row is
  re-run in X9 before it gets a grade.
- O2-08's "once" has no probe. A count of the plugin's base in the cell after the reload would
  close it; no console getter for that was chosen (the session lane, `kit testplan`).

### game-20261103-01: agent session 2026-11-03-T03 (`kit game session`), OldSovietMarksmanRifle.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-03-T03`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-03 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261103-03` |
| Plugins | `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` |
| Probes | `C:\Modding\kitkraft-worktrees\x\out\rename-plan\probes.toml` SHA-256 `d84e011aab7bed95fb67a99e2d0e17eb1ad3debbe34429d5726afb4cfa55dc05` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-03-T03` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `help "Old Soviet Marksman Rifle" 4` | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | contains 0021BBCD | help "Old Soviet Marksman Rifle" 4 -> help "Old Soviet Marksman Rifle" 4 | fail (`0021BBCD` not in `help "Old Soviet Marksman Rifle" 4`) | - |
| T-02 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261103-03`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261103-03`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29; save KitKraftSession-20261103-03 -> save KitKraftSession-20261103-03 Command: save KitKraftSession... | fail (/(?m)>>\s*1(\.0+)?\s*$/ does not match `player.getincell 0006BB29 Command: player.getincell 0006BB29`) | - |
| T-03 | persistence 4: return and look | - | persistence 4: return and look | - | not-run (manual: the plugin places no new reference with a position and a runtime ID to check after the reload) | - |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (c6fe5357313ff2a5e6967bd0b6430468e2ca6dfc95359bae9c7902d5b2f17dc2 -> e98f0a7950a816430df61854c64a4dbd184c8f35cbfaac6a752947ab4d1d1d72): restored from C:\Modding\kitkraft-observe\2026-11-03-T03\backup\01-plugins.txt

Writer's reading (2026-11-02, X5, M4.T4 V1; rulings R20 and R22). The controller graded T-01
and T-02 `fail`. Each failed step got an answer that held only the echo
(`help "Old Soviet Marksman Rifle" 4 -> help "Old Soviet Marksman Rifle" 4`, and
`player.getincell 0006BB29` right after the `coc` and its 10 s settle). An echo-only answer
is a timing anomaly, not a result, and other lanes were building at the time. Under R20 these
rows read as **inconclusive**, never `fail`. The same echo-only answers came in the session
before, `2026-11-03-T02`, on the same plugin and the same probes. Its ledger entry was not
written because the ledger path was relative, which the controller refuses as an
unauthorized write; `C:\Modding\kitkraft-observe\2026-11-03-T02\report.json` keeps it. That
makes two attempts. Later steps of T-02 did answer (though the later `GetInCell >> 1.00`
lines may be the Lodge check's late answer, R28, so they show the console was live, not
that the load happened): the `coc` to the apartment printed its
script lines, and `player.getincell 0021966F` printed a result. So the console was live and
only the first answer after a move, or the long `help` output, missed the tool's capture
window (`iExecTimeout=100`, `game-20261101-T1` row l). The first try, `2026-11-03-T01`, never
reached the game: MO2 could not start `sfse_loader.exe` ("Error 5 ERROR_ACCESS_DENIED", MO2's
`mo_interface.log`). It had been launched from a restricted shell, and it is `not-run` with
nothing sent. The `player.additem` part of V1 is **not-run**, because testplan writes no
additem row for an override of a vanilla WEAP (R22). V1 is therefore not closed.

Request (session lane, `crates/game`): treat an echo-only answer as `inconclusive` in the
matcher, as the probe file's notes already say. Then retry once with a longer wait, or a
second `help` after a pause, before a row is graded. Compare V1 again with the owner's
`game-20261014-01` once T-01 has a real answer.

### game-20261103-02: agent session 2026-11-03-T04 (`kit game session`), KitKraftMiniSite.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-03-T04`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-03 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261103-04` |
| Plugins | `KitKraftMiniSite.esm` SHA-256 `05aac03135515034e4efb8428d35bcaae3dfb9217d9a332141a802818d495d80` |
| Probes | `C:\Modding\kitkraft-worktrees\x\out\minisite\plan-dev\probes.toml` SHA-256 `9c5b41cfbf868a515930b47a5fc83dddb2addd8611875628d46db7d906c8ce57` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-03-T04` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `help KitKraftLocTypeMiniSite 4` | KitKraftMiniSite.esm:001000, KitKraftLocTypeMiniSite, 09001000 | contains 09001000 | help KitKraftLocTypeMiniSite 4 -> help KitKraftLocTypeMiniSite 4 | fail (`09001000` not in `help KitKraftLocTypeMiniSite 4`) | - |
| T-02 | `help "KitKraft Mini Site Menu" 4` | KitKraftMiniSite.esm:001001, KitKraftMiniSiteMenu, 09001001 | contains 09001001 | help "KitKraft Mini Site Menu" 4 -> help "KitKraft Mini Site Menu" 4 | fail (`09001001` not in `help "KitKraft Mini Site Menu" 4`) | - |
| T-03 | `help "KitKraft Site Terminal" 4` | KitKraftMiniSite.esm:001002, KitKraftMiniSiteTerminal, 09001002 | contains 09001002 | help "KitKraft Site Terminal" 4 -> help "KitKraft Site Terminal" 4 | fail (`09001002` not in `help "KitKraft Site Terminal" 4`) | - |
| T-04 | `help KitKraftMiniSitePackIn 4` | KitKraftMiniSite.esm:001003, KitKraftMiniSitePackIn, 09001003 | contains 09001003 | help KitKraftMiniSitePackIn 4 -> help KitKraftMiniSitePackIn 4 | fail (`09001003` not in `help KitKraftMiniSitePackIn 4`) | - |
| T-05 | `help "KitKraft Site Switch" 4` | KitKraftMiniSite.esm:001004, KitKraftMiniSiteSwitch, 09001004 | contains 09001004 | help "KitKraft Site Switch" 4 -> help "KitKraft Site Switch" 4 | fail (`09001004` not in `help "KitKraft Site Switch" 4`) | - |
| T-06 | `help "KitKraft Mini Site Crew" 4` | KitKraftMiniSite.esm:001005, KitKraftMiniSiteFaction, 09001005 | contains 09001005 | help "KitKraft Mini Site Crew" 4 -> help "KitKraft Mini Site Crew" 4 | fail (`09001005` not in `help "KitKraft Mini Site Crew" 4`) | - |
| T-07 | `help "KitKraft Mini Site" 4` | KitKraftMiniSite.esm:001006, KitKraftMiniSiteLocation, 09001006 | contains 09001006 | help "KitKraft Mini Site" 4 -> help "KitKraft Mini Site" 4 | fail (`09001006` not in `help "KitKraft Mini Site" 4`) | - |
| T-08 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F | fail (/(?m)>>\s*1(\.0+)?\s*$/ does not match `player.getincell 0021966F Command: player.getincell 0021966F`) | - |
| T-09 | `player.moveto 09001007`, `prid 09001007`, `getdistance player` | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007 | number 0 +/- 500 | player.moveto 09001007 -> player.moveto 09001007 Command: player.moveto 09001007; (waited 10s); prid 09001007 -> prid 09001007 Command: prid 09001007; getdistance player -> getdistance player Command: getdistance player | inconclusive (no number after `>>` in `getdistance player Command: getdistance player`) | - |
| T-10 | `prid 09001007`, `getisid 09001004` | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001007 -> prid 09001007 Command: prid 09001007; getisid 09001004 -> getisid 09001004 Command: getisid 09001004 | fail (/(?m)>>\s*1(\.0+)?\s*$/ does not match `getisid 09001004 Command: getisid 09001004`) | - |
| T-11 | `prid 09001007`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007 | number -19 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 09001007 -> prid 09001007 Command: prid 09001007; getpos x -> getpos x Command: getpos x usage: help <matchstring> <filter> <form type> filters: 0-all 1-functions, 2-settings, 3-glob...; getpos y -> getpos y Command: getpos y; getpos z -> getpos z Command: getpos z | inconclusive (no number after `>>` in `getpos x Command: getpos x usage: help <matchstring> <filter> <form type> filters: 0-all 1-functions, 2-settings, 3-globals, 4-other forms form type is 4 charac...`) | T-11-1.png |
| T-12 | Papyrus log: KitKraft:HelloOnLoad on the base of REFR `KitKraftMiniSiteSwitchRef` writes "KitKraft: mini site switch loaded" | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007, Scripts\KitKraft\HelloOnLoad.pex | papyrus-log KitKraft: mini site switch loaded | - | fail (`KitKraft: mini site switch loaded` not in `[22:52:29] Papyrus log opened (PC-64) [22:52:29] Update budget: 1.200000ms (Extra tasklet budget: 1.200000ms, Load screen budget: 500.000000ms) [22:52:29] Memor...`) | - |
| T-13 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| T-14 | `player.moveto 09001008`, `prid 09001008`, `getdistance player` | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008 | number 0 +/- 500 | player.moveto 09001008 -> player.moveto 09001008 Command: player.moveto 09001008; (waited 10s); prid 09001008 -> prid 09001008 Command: prid 09001008; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.84 | **pass** | - |
| T-15 | `prid 09001008`, `getisid 09001002` | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001008 -> prid 09001008 Command: prid 09001008; getisid 09001002 -> getisid 09001002 Command: getisid 09001002 GetIsID >> 1.00 | **pass** | - |
| T-16 | `prid 09001008`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008 | number -21 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 09001008 -> prid 09001008 Command: prid 09001008; getpos x -> getpos x Command: getpos x GetPos: X >> -21.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-16-1.png |
| T-17 | TerminalScript on the base of REFR `KitKraftMiniSiteTerminalRef`: its effect | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008, Scripts\TerminalScript.pex | TerminalScript on the base of REFR `KitKraftMiniSiteTerminalRef`: its effect | - | not-run (manual: the compiled script is not next to the plugin (Scripts\TerminalScript.pex; `build` and `package` put it there), so its Papyrus log lines cannot be predicted) | - |
| T-18 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| T-19 | `player.moveto 09001009`, `prid 09001009`, `getdistance player` | KitKraftMiniSite.esm:001009, KitKraftMiniSitePackInRef, 09001009 | number 0 +/- 500 | player.moveto 09001009 -> player.moveto 09001009 Command: player.moveto 09001009; (waited 10s); prid 09001009 -> prid 09001009 Command: prid 09001009; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.25 | **pass** | - |
| T-20 | `prid 09001009`, `getisid 09001003` | KitKraftMiniSite.esm:001009, KitKraftMiniSitePackInRef, 09001009 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001009 -> prid 09001009 Command: prid 09001009; getisid 09001003 -> getisid 09001003 Command: getisid 09001003 GetIsID >> 0.00 | fail (/(?m)>>\s*1(\.0+)?\s*$/ does not match `getisid 09001003 Command: getisid 09001003 GetIsID >> 0.00`) | - |
| T-21 | `prid 09001009`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSite.esm:001009, KitKraftMiniSitePackInRef, 09001009 | number -23 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 09001009 -> prid 09001009 Command: prid 09001009; getpos x -> getpos x Command: getpos x GetPos: X >> -23.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-21-1.png |
| T-22 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261103-04`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261103-04`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00; save KitKraftSession-... | **pass** | - |
| T-23 | `coc CityNewAtlantisApartmentTEMPLATE`, `prid 09001007`, `getpos x`, `getpos y`, `getpos z`, `coc CityNewAtlantisApartmentTEMPLATE`, `prid 09001008`, `getpos x`, `getpos y`, `getpos z`, `coc CityNewAtlantisApartmentTEMPLATE`, `prid 09001009`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSiteSwitchRef, 09001007, KitKraftMiniSiteTerminalRef, 09001008, KitKraftMiniSitePackInRef, 09001009 | number -19 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5; number -21 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5; number -23 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); prid 09001007 -> prid 09001007 Command: prid 09001007; getpos x -> getpos x Command: getpos x GetPos: X >> -19.00; getpos... | **pass** | T-23-1.png |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (273e2a7bfc0ca9aedaba9c3567a436eb2879d32fbe70cd2abf57beaefcce301c -> 3c65835703a6ac1ed1935d0a3cde08fe5d1d71a1540934e4c8617ffc21e00b4c): restored from C:\Modding\kitkraft-observe\2026-11-03-T04\backup\01-plugins.txt

### game-20261103-03: agent session 2026-11-03-T05 (`kit game session`), KitKraftMiniSite.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-03-T05`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-03 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261103-05` |
| Plugins | `KitKraftMiniSite.esm` SHA-256 `05aac03135515034e4efb8428d35bcaae3dfb9217d9a332141a802818d495d80` |
| Probes | `C:\Modding\kitkraft-worktrees\x\out\minisite\plan-dev\probes.toml` SHA-256 `9c5b41cfbf868a515930b47a5fc83dddb2addd8611875628d46db7d906c8ce57` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-03-T05` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `help KitKraftLocTypeMiniSite 4` | KitKraftMiniSite.esm:001000, KitKraftLocTypeMiniSite, 09001000 | contains 09001000 | help KitKraftLocTypeMiniSite 4 -> (only the echo; again in 2000 ms); help KitKraftLocTypeMiniSite 4 -> (only the echo; again in 4000 ms); help KitKraftLocTypeMiniSite 4 -> help KitKraftLocTypeMiniSite 4 | inconclusive (`help KitKraftLocTypeMiniSite 4` answered with only its echo, 3 time(s): no result to judge (a timing anomaly, R20)) | - |
| T-02 | `help "KitKraft Mini Site Menu" 4` | KitKraftMiniSite.esm:001001, KitKraftMiniSiteMenu, 09001001 | contains 09001001 | help "KitKraft Mini Site Menu" 4 -> (only the echo; again in 2000 ms); help "KitKraft Mini Site Menu" 4 -> (only the echo; again in 4000 ms); help "KitKraft Mini Site Menu" 4 -> help "KitKraft Mini Site Menu" 4 | inconclusive (`help "KitKraft Mini Site Menu" 4` answered with only its echo, 3 time(s): no result to judge (a timing anomaly, R20)) | - |
| T-03 | `help "KitKraft Site Terminal" 4` | KitKraftMiniSite.esm:001002, KitKraftMiniSiteTerminal, 09001002 | contains 09001002 | help "KitKraft Site Terminal" 4 -> (only the echo; again in 2000 ms); help "KitKraft Site Terminal" 4 -> help "KitKraft Site Terminal" 4 KYWD: KitKraftLocTypeMiniSite (09001000) '' | fail (`09001002` not in `help "KitKraft Site Terminal" 4 KYWD: KitKraftLocTypeMiniSite (09001000) ''`) | - |
| T-04 | `help KitKraftMiniSitePackIn 4` | KitKraftMiniSite.esm:001003, KitKraftMiniSitePackIn, 09001003 | contains 09001003 | help KitKraftMiniSitePackIn 4 -> (only the echo; again in 2000 ms); help KitKraftMiniSitePackIn 4 -> (only the echo; again in 4000 ms); help KitKraftMiniSitePackIn 4 -> help KitKraftMiniSitePackIn 4 TMLM:  (09001001) 'KitKraft Mini Site Menu' | fail (`09001003` not in `help KitKraftMiniSitePackIn 4 TMLM:  (09001001) 'KitKraft Mini Site Menu'`) | - |
| T-05 | `help "KitKraft Site Switch" 4` | KitKraftMiniSite.esm:001004, KitKraftMiniSiteSwitch, 09001004 | contains 09001004 | help "KitKraft Site Switch" 4 -> (only the echo; again in 2000 ms); help "KitKraft Site Switch" 4 -> (only the echo; again in 4000 ms); help "KitKraft Site Switch" 4 -> help "KitKraft Site Switch" 4 | inconclusive (`help "KitKraft Site Switch" 4` answered with only its echo, 3 time(s): no result to judge (a timing anomaly, R20)) | - |
| T-06 | `help "KitKraft Mini Site Crew" 4` | KitKraftMiniSite.esm:001005, KitKraftMiniSiteFaction, 09001005 | contains 09001005 | help "KitKraft Mini Site Crew" 4 -> (only the echo; again in 2000 ms); help "KitKraft Mini Site Crew" 4 -> (only the echo; again in 4000 ms); help "KitKraft Mini Site Crew" 4 -> help "KitKraft Mini Site Crew" 4 | inconclusive (`help "KitKraft Mini Site Crew" 4` answered with only its echo, 3 time(s): no result to judge (a timing anomaly, R20)) | - |
| T-07 | `help "KitKraft Mini Site" 4` | KitKraftMiniSite.esm:001006, KitKraftMiniSiteLocation, 09001006 | contains 09001006 | help "KitKraft Mini Site" 4 -> help "KitKraft Mini Site" 4 TERM:  (09001002) 'KitKraft Site Terminal' | fail (`09001006` not in `help "KitKraft Mini Site" 4 TERM:  (09001002) 'KitKraft Site Terminal'`) | - |
| T-08 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; (waited 10s); player.getincell 0021966F -> (only the echo; again in 2000 ms); player.getincell 0021966F -> (only the echo; again in 4000 ms); player.getincell 0021966F -> playe... | inconclusive (`player.getincell 0021966F` answered with only its echo, 3 time(s): no result to judge (a timing anomaly, R20)) | - |
| T-09 | `player.moveto 09001007`, `prid 09001007`, `getdistance player` | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007 | number 0 +/- 500 | player.moveto 09001007 -> player.moveto 09001007 Command: player.moveto 09001007; (waited 10s); prid 09001007 -> prid 09001007 Command: prid 09001007; getdistance player -> (only the echo; again in 2000 ms); getdistance player -> (only the echo; again in 4000 ms); getdistance player -> getdistance p... | inconclusive (`getdistance player` answered with only its echo, 3 time(s): no result to judge (a timing anomaly, R20)) | - |
| T-10 | `prid 09001007`, `getisid 09001004` | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001007 -> prid 09001007 Command: prid 09001007; getisid 09001004 -> (only the echo; again in 2000 ms); getisid 09001004 -> getisid 09001004 Command: getisid 09001004 GetIsID >> 1.00 | **pass** | - |
| T-11 | `prid 09001007`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007 | number -19 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 09001007 -> prid 09001007 Command: prid 09001007; getpos x -> getpos x Command: getpos x GetPos: X >> -19.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-11-1.png |
| T-12 | Papyrus log: KitKraft:HelloOnLoad on the base of REFR `KitKraftMiniSiteSwitchRef` writes "KitKraft: mini site switch loaded" | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007, Scripts\KitKraft\HelloOnLoad.pex | papyrus-log KitKraft: mini site switch loaded | - | fail (`KitKraft: mini site switch loaded` not in `[23:18:16] Papyrus log opened (PC-64) [23:18:16] Update budget: 1.200000ms (Extra tasklet budget: 1.200000ms, Load screen budget: 500.000000ms) [23:18:16] Memor...`) | - |
| T-13 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| T-14 | `player.moveto 09001008`, `prid 09001008`, `getdistance player` | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008 | number 0 +/- 500 | player.moveto 09001008 -> player.moveto 09001008 Command: player.moveto 09001008; (waited 10s); prid 09001008 -> prid 09001008 Command: prid 09001008; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.83 | **pass** | - |
| T-15 | `prid 09001008`, `getisid 09001002` | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001008 -> prid 09001008 Command: prid 09001008; getisid 09001002 -> getisid 09001002 Command: getisid 09001002 GetIsID >> 1.00 | **pass** | - |
| T-16 | `prid 09001008`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008 | number -21 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 09001008 -> prid 09001008 Command: prid 09001008; getpos x -> getpos x Command: getpos x GetPos: X >> -21.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-16-1.png |
| T-17 | TerminalScript on the base of REFR `KitKraftMiniSiteTerminalRef`: its effect | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008, Scripts\TerminalScript.pex | TerminalScript on the base of REFR `KitKraftMiniSiteTerminalRef`: its effect | - | not-run (manual: the compiled script is not next to the plugin (Scripts\TerminalScript.pex; `build` and `package` put it there), so its Papyrus log lines cannot be predicted) | - |
| T-18 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| T-19 | `player.moveto 09001009`, `prid 09001009`, `getdistance player` | KitKraftMiniSite.esm:001009, KitKraftMiniSitePackInRef, 09001009 | number 0 +/- 500 | player.moveto 09001009 -> player.moveto 09001009 Command: player.moveto 09001009; (waited 10s); prid 09001009 -> prid 09001009 Command: prid 09001009; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.25 | **pass** | - |
| T-20 | `prid 09001009`, `getisid 09001003` | KitKraftMiniSite.esm:001009, KitKraftMiniSitePackInRef, 09001009 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001009 -> prid 09001009 Command: prid 09001009; getisid 09001003 -> getisid 09001003 Command: getisid 09001003 GetIsID >> 0.00 | fail (/(?m)>>\s*1(\.0+)?\s*$/ does not match `getisid 09001003 Command: getisid 09001003 GetIsID >> 0.00`) | - |
| T-21 | `prid 09001009`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSite.esm:001009, KitKraftMiniSitePackInRef, 09001009 | number -23 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 09001009 -> prid 09001009 Command: prid 09001009; getpos x -> getpos x Command: getpos x GetPos: X >> -23.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-21-1.png |
| T-22 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261103-05`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261103-05`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00; save KitKraftSession-... | **pass** | - |
| T-23 | `coc CityNewAtlantisApartmentTEMPLATE`, `prid 09001007`, `getpos x`, `getpos y`, `getpos z`, `coc CityNewAtlantisApartmentTEMPLATE`, `prid 09001008`, `getpos x`, `getpos y`, `getpos z`, `coc CityNewAtlantisApartmentTEMPLATE`, `prid 09001009`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSiteSwitchRef, 09001007, KitKraftMiniSiteTerminalRef, 09001008, KitKraftMiniSitePackInRef, 09001009 | number -19 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5; number -21 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5; number -23 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); prid 09001007 -> prid 09001007 Command: prid 09001007; getpos x -> getpos x Command: getpos x GetPos: X >> -19.00; getpos... | **pass** | T-23-1.png |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (273e2a7bfc0ca9aedaba9c3567a436eb2879d32fbe70cd2abf57beaefcce301c -> 3c65835703a6ac1ed1935d0a3cde08fe5d1d71a1540934e4c8617ffc21e00b4c): restored from C:\Modding\kitkraft-observe\2026-11-03-T05\backup\01-plugins.txt

Writer's reading of `game-20261103-02` (T04, old controller) and `game-20261103-03` (T05,
the retrying controller of `ad3602d4`). X8, M5.D7 game half; rulings R20 and R22. The
entries' dates and session ids are UTC; the sessions ran 2026-11-02 22:39 to 23:22 local
(EST). T04 (22:52) ran before `ad3602d4` (22:59), so before the retry R27 asked for.

**Persistence and the placed references.**

- T-22 and T-23 **pass** in both sessions. The mini-site was saved in the Lodge and
  reloaded, and the reload was proved by the cell. The terminal, switch and pack-in
  references stood at their `DATA` positions afterwards.
- Every `getincell` answer in T-22 is `GetInCell >> 1.00`, so a reply shifted by one request
  would also pass. The pass stands because the stream is aligned there: no echo-only answer
  from T-19 to T-23 in T05 or from T-13 on in T04, and T-23's `getpos` answers come labelled
  X, Y, Z in order with each reference's own x. X9 added the negative control after a load
  (`game-20261103-07` H-01, `GetInCell >> 0.00` for the apartment after the load into the
  Lodge), but the re-grade on `e6028aaa` makes it inconclusive (ambiguous: an earlier
  `getincell` send had no answer), so it is still to be run.
- T-13 to T-16, T-18, T-19 and T-21 **pass** in T05: each reference's cell, `player.moveto`
  and `getpos`. T-10 and T-11 (the switch's reference) also pass in T05.
- M5 row 2's game and save/reload rows are **filled** by these sessions (M5.D7 game half).

**The `help` rows show a console-path lag, not missing forms.**

- In T05 the answers to `help` come back late. T-03's answer holds T-01's result
  (`KYWD: KitKraftLocTypeMiniSite (09001000)`). T-04's holds T-02's
  (`TMLM: (09001001) 'KitKraft Mini Site Menu'`). T-07's holds T-03's
  (`TERM: (09001002) 'KitKraft Site Terminal'`).
- So three of the seven new forms are listed under their predicted runtime IDs (the KYWD,
  TMLM and TERM; T-10 and T-15 show the ACTI and the TERM as bases). The PKIN, FACT and
  LCTN are not seen. The console tool hands each `help` result to a later request.
- The matcher's `fail` on T-03, T-04 and T-07 is that misattribution, so these rows are
  **inconclusive** (R20). So are T-01, T-02, T-05, T-06, T-08 and T-09, where three sends
  each got only the echo.
- This is the console path more than build load: the retries waited 2 s and 4 s, and a
  late answer still went to the next command.
- In T04 (`game-20261103-02`, the controller before `ad3602d4`) T-01 to T-08 and T-10 got
  only the echo, and T-12 read before a fresh load: all **inconclusive** (R20, R27), not the
  `fail` the controller wrote. T-11's `getpos x` got a late `help` usage text (R28):
  inconclusive. T04's passes T-13 to T-16, T-18, T-19 and T-21 to T-23 stand: from T-13 on
  every answer carries its own command's label, in order.

**T-12, the switch's base-form script (`papyrus-log`).**

- The line was not written late. T05's log (`2026-11-03-T05\logs\papyrus-Papyrus.0.log`):
  the base save loads at 23:19:28 to 23:19:36, into the apartment; no Trace through T-08,
  T-13 and T-18 (each a `coc` into the apartment the player is already in); `Saving game` at
  23:21:34 (T-22, in the Lodge); the Trace at 23:21:44 (line 194, T-22's `coc` back to the
  apartment); `Loading game` at 23:21:57 (T-22's load, into the Lodge); the Trace at
  23:22:05 (line 268, T-23's `coc` to the apartment). T04's log has the same pattern: lines
  195 and 269, at 22:55:13 after the save at 22:55:03 and at 22:55:33 after the load at
  22:55:27. The re-grade T10 again: lines 194 and 268, 00:20:21 and 00:20:42.
- T-12 read the log between 23:20:39 (`T-11-1.png`) and 23:21:02 (`T-16-1.png`), before any
  fresh load of the switch's cell, so it is **inconclusive** (R20) in T04, T05 and T10.
- Finding: the switch's OnLoad Trace followed each fresh load of its cell after a trip to
  the Lodge, and neither the base-save load into the cell nor a `coc` into the occupied
  cell (the scripts recipe in T06 shows the same, `game-20261103-04`).
- So the script compiled by kit, attached to the cloned ACTI base, ran on OnLoad and wrote its
  Trace. That is Gate K0's "fires" evidence for OnLoad (M5.B3, OnLoad half), from a log, not
  from the row.
- Request (testplan and session lane): run a `papyrus-log` row after a leave-and-return of
  the cell, or read the log again at session end and record which load preceded the line.
- Request (session lane, R28): route each reply line to the request it names. Done in
  `aa31a9c9` (corrected in `e6028aaa`); re-graded in `game-20261103-08`.

**T-20 (pack-in reference) is a real `fail`; its cause is open.**

- `prid 09001009` with `getisid 09001003` answered `GetIsID >> 0.00` with a real number.
  The reference was selected (T-19's `GetDistance >> 0.25`, T-21's x -23.00), and the
  answer is its own, labelled. The same in T04 and in the re-grade T10 (`game-20261103-08`).
- Request (testplan, `handlers/testplan.rs`): nothing observed separates two causes:
  testplan expects the wrong base for a placed PKIN reference, or the game does not base a
  placed PKIN reference on the PKIN as KitKraft wrote it. A check that tells them apart
  (for example `getisid` on the PKIN's contents, or `help` on the reference's base) is
  needed before the row is fixed. M5.D7 is blocked on it.

**The rest.**

- T-17 (the vanilla `TerminalScript` on the cloned terminal) is manual: its `.pex` is not
  next to the plugin.
- The dev mod `KitKraft Dev - KitKraftMiniSite` (a package: the plugin and its `.pex`) was
  deployed, enabled for the sessions and disabled after. Only its lines changed, per
  `deploy enable` and `disable`'s printed edits (not kept; each edit's profile backup is under
  `%LOCALAPPDATA%\kitkraft\profile-backups\`).

### game-20261103-04: agent session 2026-11-03-T06 (`kit game session`), KitKraftScripts.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-03-T06`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-03 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261103-06` |
| Plugins | `KitKraftScripts.esm` SHA-256 `b20cf5770b3d32016274546f6c7478e687d58e130904c0c2367bfa56c11a1d45` |
| Probes | `C:\Modding\kitkraft-worktrees\x\out\x9\scripts\plan\probes.toml` SHA-256 `697304014e96591f49b3cca65482a085416771544b01b025dcefe5e54aa5220a` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-03-T06` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisFreestarEmbassy`, `player.getincell 00017307` | Starfield.esm:017307, CityNewAtlantisFreestarEmbassy, 00017307 | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisFreestarEmbassy -> coc CityNewAtlantisFreestarEmbassy Command: coc CityNewAtlantisFreestarEmbassy; (waited 10s); player.getincell 00017307 -> (only the echo; again in 2000 ms); player.getincell 00017307 -> player.getincell 00017307 Command: player.getincell 00017307 GetInCell >> 1... | **pass** | - |
| T-02 | `player.moveto 09001000`, `prid 09001000`, `getdistance player` | KitKraftScripts.esm:001000, KitKraftHelloSwitch, 09001000 | number 0 +/- 500 | player.moveto 09001000 -> player.moveto 09001000 Command: player.moveto 09001000; (waited 10s); prid 09001000 -> prid 09001000 Command: prid 09001000; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.44 | **pass** | - |
| T-03 | `prid 09001000`, `getisid 0003CCF4` | KitKraftScripts.esm:001000, KitKraftHelloSwitch, 09001000 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001000 -> prid 09001000 Command: prid 09001000; getisid 0003CCF4 -> getisid 0003CCF4 Command: getisid 0003CCF4 GetIsID >> 1.00 | **pass** | - |
| T-04 | `prid 09001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftScripts.esm:001000, KitKraftHelloSwitch, 09001000 | number 4 +/- 0.5; number 4 +/- 0.5; number 0 +/- 0.5 | prid 09001000 -> prid 09001000 Command: prid 09001000; getpos x -> getpos x Command: getpos x GetPos: X >> 4.00; getpos y -> getpos y Command: getpos y GetPos: Y >> 4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 0.00 | **pass** | T-04-1.png |
| T-05 | Papyrus log: KitKraft:HelloOnLoad on REFR `KitKraftHelloSwitch` writes "KitKraft: hello from a script attached by kit" | KitKraftScripts.esm:001000, KitKraftHelloSwitch, 09001000, Scripts\KitKraft\HelloOnLoad.pex | papyrus-log KitKraft: hello from a script attached by kit | - | **pass** | - |
| T-06 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261103-06`, `coc CityNewAtlantisFreestarEmbassy`, `player.getincell 00017307`, `load KitKraftSession-20261103-06`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisFreestarEmbassy, 00017307 | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt GetInFaction >> 1.00 GetInFaction >> 1.00 Is On Player ...; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00; save KitKraftSession-... | **pass** | - |
| T-07 | `coc CityNewAtlantisFreestarEmbassy`, `prid 09001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftHelloSwitch, 09001000 | number 4 +/- 0.5; number 4 +/- 0.5; number 0 +/- 0.5 | coc CityNewAtlantisFreestarEmbassy -> coc CityNewAtlantisFreestarEmbassy Command: coc CityNewAtlantisFreestarEmbassy IsInSpace >> 0.00 GetGlobalValue >> 0.00 ...; (waited 10s); prid 09001000 -> prid 09001000 Command: prid 09001000; getpos x -> getpos x Command: getpos x GetPos: X >> 4.00; getpos y -... | **pass** | T-07-1.png |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (9f4a042ba76fb75e08543e97c76dc3399f3bff6a144aa4d74947137a6d88415a -> 639a7fd6a68c99f67406c4f8acaab0bf55c763acddfa397fa5502e25254c230d): restored from C:\Modding\kitkraft-observe\2026-11-03-T06\backup\01-plugins.txt

### game-20261103-05: agent session 2026-11-03-T07 (`kit game session`), KitKraftFirstPOI.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-03-T07`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-03 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261103-07` |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `C:\Modding\kitkraft-worktrees\x\out\x9\poi\plan-h\probes.toml` SHA-256 `61df2ae10c6a0f090bbba3c5ea829e53af86adc607ffa3b3ca6a06e756103fb6` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-03-T07` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| T-02 | `player.moveto 06001000`, `prid 06001000`, `getdistance player` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 06001000 | number 0 +/- 500 | player.moveto 06001000 -> player.moveto 06001000 Command: player.moveto 06001000; (waited 10s); prid 06001000 -> prid 06001000 Command: prid 06001000; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.86 | **pass** | - |
| T-03 | `prid 06001000`, `getisid 001E4FBD` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 06001000 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 06001000 -> prid 06001000 Command: prid 06001000; getisid 001E4FBD -> getisid 001E4FBD Command: getisid 001E4FBD GetIsID >> 1.00 | **pass** | - |
| T-04 | `prid 06001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftFirstPOI.esm:001000, KitKraftFirstPOICrate, 06001000 | number -17 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 06001000 -> prid 06001000 Command: prid 06001000; getpos x -> getpos x Command: getpos x GetPos: X >> -17.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-04-1.png |
| T-05 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261103-07`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261103-07`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00; save KitKraftSession-... | **pass** | - |
| T-06 | `coc CityNewAtlantisApartmentTEMPLATE`, `prid 06001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftFirstPOICrate, 06001000 | number -17 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); prid 06001000 -> prid 06001000 Command: prid 06001000; getpos x -> getpos x Command: getpos x GetPos: X >> -17.00; getpos... | **pass** | T-06-1.png |
| H-01 | `help Crate_MilitarySupply_01 4` | Starfield.esm:1E4FBD, Crate_MilitarySupply_01, 001E4FBD | contains 001E4FBD | help Crate_MilitarySupply_01 4 -> (no line of its own: help Crate_MilitarySupply_01 4 ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------...; again in 2000 ms); help Crate_MilitarySupply_01 4 -> (only the echo; again in 4000 ms); help Crate_MilitarySupply_01 4 -> help Crate_Militar... | inconclusive (`help Crate_MilitarySupply_01 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (241f36a91ff43cb296567fa26396a082cb6fac84e99105d7aa8e1ad1121a87aa -> c7bacaacfa976b844dccec4ed93c3eca9ec913f0c1f7e81e6dc5ec0a454e76f2): restored from C:\Modding\kitkraft-observe\2026-11-03-T07\backup\01-plugins.txt

### game-20261103-06: agent session 2026-11-03-T08 (`kit game session`), KitKraftActors.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-03-T08`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-03 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261103-08` |
| Plugins | `KitKraftActors.esm` SHA-256 `837b63719261e6cebb7cd848d184795293c5a7cb9fce28f828a53790e588f27a` |
| Probes | `C:\Modding\kitkraft-worktrees\x\out\x9\actors\plan\probes.toml` SHA-256 `ee9013c6de730bbbe45b8f035efbb57805371d0f836700d0b270e7a67d003df5` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-03-T08` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisFreestarEmbassy`, `player.getincell 00017307` | Starfield.esm:017307, CityNewAtlantisFreestarEmbassy, 00017307 | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisFreestarEmbassy -> coc CityNewAtlantisFreestarEmbassy Command: coc CityNewAtlantisFreestarEmbassy; (waited 10s); player.getincell 00017307 -> player.getincell 00017307 Command: player.getincell 00017307 GetInCell >> 1.00 | **pass** | - |
| T-02 | `player.moveto 09001000`, `prid 09001000`, `getdistance player` | KitKraftActors.esm:001000, KitKraftTestActor, 09001000 | number 0 +/- 500 | player.moveto 09001000 -> player.moveto 09001000 Command: player.moveto 09001000; (waited 10s); prid 09001000 -> prid 09001000 Command: prid 09001000; getdistance player -> getdistance player Command: getdistance player GetDistance >> 2.56 | **pass** | - |
| T-03 | `prid 09001000`, `getisid 00271133` | KitKraftActors.esm:001000, KitKraftTestActor, 09001000 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001000 -> prid 09001000 Command: prid 09001000; getisid 00271133 -> getisid 00271133 Command: getisid 00271133 GetIsID >> 1.00 | **pass** | - |
| T-04 | `prid 09001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftActors.esm:001000, KitKraftTestActor, 09001000 | number 4 +/- 0.5; number 4 +/- 0.5; number 0 +/- 0.5 | prid 09001000 -> prid 09001000 Command: prid 09001000; getpos x -> getpos x Command: getpos x GetPos: X >> 5.16; getpos y -> getpos y Command: getpos y GetPos: Y >> 3.58; getpos z -> getpos z Command: getpos z GetPos: Z >> 0.01 | fail (5.16 is not within 0.5 of 4) | T-04-1.png |
| T-05 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261103-08`, `coc CityNewAtlantisFreestarEmbassy`, `player.getincell 00017307`, `load KitKraftSession-20261103-08`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisFreestarEmbassy, 00017307 | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt GetInFaction >> 1.00 GetInFaction >> 1.00 Is On Player ...; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00; save KitKraftSession-... | **pass** | - |
| T-06 | `coc CityNewAtlantisFreestarEmbassy`, `prid 09001000`, `getpos x`, `getpos y`, `getpos z` | KitKraftTestActor, 09001000 | number 4 +/- 0.5; number 4 +/- 0.5; number 0 +/- 0.5 | coc CityNewAtlantisFreestarEmbassy -> coc CityNewAtlantisFreestarEmbassy Command: coc CityNewAtlantisFreestarEmbassy IsInSpace >> 0.00 GetGlobalValue >> 0.00 ...; (waited 10s); prid 09001000 -> prid 09001000 Command: prid 09001000; getpos x -> getpos x Command: getpos x GetPos: X >> 5.16; getpos y -... | fail (5.16 is not within 0.5 of 4) | T-06-1.png |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (fe9769f15ec3f12ec6d1c67b237cbc5143d9fae21eb49d476799e032060b8b5e -> 35dddeb5a7c042b97ef164fbd15bae38c4e6fb6b589b81468d78dcc86e3f48ef): restored from C:\Modding\kitkraft-observe\2026-11-03-T08\backup\01-plugins.txt

### game-20261103-07: agent session 2026-11-03-T09 (`kit game session`), KitKraftOwnership.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-03-T09`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-03 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261103-09` |
| Plugins | `KitKraftOwnership.esm` SHA-256 `cc77ed38e904679536475db31371752f50176baa3aaa948d1848072cd26cc259` |
| Probes | `C:\Modding\kitkraft-worktrees\x\out\x9\own\plan-h\probes.toml` SHA-256 `337a69ea459cd329e40d8cbe7cb49cea21991869297e9ceda0f481334fc54735` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-03-T09` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261103-09`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261103-09`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt; (waited 10s); player.getincell 0006BB29 -> (only the echo; again in 2000 ms); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00; save KitKraftSes... | **pass** | - |
| T-02 | persistence 4: return and look | - | persistence 4: return and look | - | not-run (manual: the plugin places no new reference with a position and a runtime ID to check after the reload) | - |
| H-01 | `player.getincell 0021966F` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*0(\.0+)?\s*$ | player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 0.00 | **pass** | - |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (5473d73674950cdb2a6fb785f8ffad696c0b37f8ec41d8e85666a378ab219c6e -> d9fd5a53a2ac454010de294edb8dab8b05e8523f0752b5ac3915ef206047ef2d): restored from C:\Modding\kitkraft-observe\2026-11-03-T09\backup\01-plugins.txt

### game-20261103-08: agent session 2026-11-03-T10 (`kit game session`), KitKraftMiniSite.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-03-T10`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-03 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261103-10` |
| Plugins | `KitKraftMiniSite.esm` SHA-256 `05aac03135515034e4efb8428d35bcaae3dfb9217d9a332141a802818d495d80` |
| Probes | `C:\Modding\kitkraft-worktrees\x\out\minisite\plan-dev\probes.toml` SHA-256 `9c5b41cfbf868a515930b47a5fc83dddb2addd8611875628d46db7d906c8ce57` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-03-T10` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `help KitKraftLocTypeMiniSite 4` | KitKraftMiniSite.esm:001000, KitKraftLocTypeMiniSite, 09001000 | contains 09001000 | help KitKraftLocTypeMiniSite 4 -> (only the echo; again in 2000 ms); help KitKraftLocTypeMiniSite 4 -> (only the echo; again in 4000 ms); help KitKraftLocTypeMiniSite 4 -> help KitKraftLocTypeMiniSite 4; (late) help KitKraftLocTypeMiniSite 4 -> KYWD: KitKraftLocTypeMiniSite (09001000) '' (in the rep... | **pass** | - |
| T-02 | `help "KitKraft Mini Site Menu" 4` | KitKraftMiniSite.esm:001001, KitKraftMiniSiteMenu, 09001001 | contains 09001001 | help "KitKraft Mini Site Menu" 4 -> (only the echo; again in 2000 ms); help "KitKraft Mini Site Menu" 4 -> (only the echo; again in 4000 ms); (a line for row T-01 `help KitKraftLocTypeMiniSite 4` re-queued to it: KYWD: KitKraftLocTypeMiniSite (09001000) ''); help "KitKraft Mini Site Menu" 4 -> help ... | inconclusive (`help "KitKraft Mini Site Menu" 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| T-03 | `help "KitKraft Site Terminal" 4` | KitKraftMiniSite.esm:001002, KitKraftMiniSiteTerminal, 09001002 | contains 09001002 | help "KitKraft Site Terminal" 4 -> (only the echo; again in 2000 ms); help "KitKraft Site Terminal" 4 -> (only the echo; again in 4000 ms); help "KitKraft Site Terminal" 4 -> help "KitKraft Site Terminal" 4 | inconclusive (`help "KitKraft Site Terminal" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| T-04 | `help KitKraftMiniSitePackIn 4` | KitKraftMiniSite.esm:001003, KitKraftMiniSitePackIn, 09001003 | contains 09001003 | help KitKraftMiniSitePackIn 4 -> (only the echo; again in 2000 ms); help KitKraftMiniSitePackIn 4 -> (no line of its own: help KitKraftMiniSitePackIn 4 usage: help <matchstring> <filter> <form type> filters: 0-all 1-functi...; again in 4000 ms); help KitKraftMiniSitePackIn 4 -> help KitKraftMiniSite... | inconclusive (`help KitKraftMiniSitePackIn 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| T-05 | `help "KitKraft Site Switch" 4` | KitKraftMiniSite.esm:001004, KitKraftMiniSiteSwitch, 09001004 | contains 09001004 | help "KitKraft Site Switch" 4 -> (only the echo; again in 2000 ms); help "KitKraft Site Switch" 4 -> (only the echo; again in 4000 ms); help "KitKraft Site Switch" 4 -> help "KitKraft Site Switch" 4 | inconclusive (`help "KitKraft Site Switch" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| T-06 | `help "KitKraft Mini Site Crew" 4` | KitKraftMiniSite.esm:001005, KitKraftMiniSiteFaction, 09001005 | contains 09001005 | help "KitKraft Mini Site Crew" 4 -> (only the echo; again in 2000 ms); help "KitKraft Mini Site Crew" 4 -> (only the echo; again in 4000 ms); help "KitKraft Mini Site Crew" 4 -> help "KitKraft Mini Site Crew" 4 | inconclusive (`help "KitKraft Mini Site Crew" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| T-07 | `help "KitKraft Mini Site" 4` | KitKraftMiniSite.esm:001006, KitKraftMiniSiteLocation, 09001006 | contains 09001006 | help "KitKraft Mini Site" 4 -> (only the echo; again in 2000 ms); help "KitKraft Mini Site" 4 -> (only the echo; again in 4000 ms); help "KitKraft Mini Site" 4 -> help "KitKraft Mini Site" 4 | inconclusive (`help "KitKraft Mini Site" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| T-08 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; (waited 10s); player.getincell 0021966F -> (only the echo; again in 2000 ms); player.getincell 0021966F -> (only the echo; again in 4000 ms); player.getincell 0021966F -> playe... | **pass** | - |
| T-09 | `player.moveto 09001007`, `prid 09001007`, `getdistance player` | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007 | number 0 +/- 500 | player.moveto 09001007 -> player.moveto 09001007 Command: player.moveto 09001007; (waited 10s); prid 09001007 -> prid 09001007 Command: prid 09001007; getdistance player -> (only the echo; again in 2000 ms); getdistance player -> (only the echo; again in 4000 ms); getdistance player -> getdistance p... | **pass** | - |
| T-10 | `prid 09001007`, `getisid 09001004` | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001007 -> prid 09001007 Command: prid 09001007; (a line for row T-08 `player.getincell 0021966F` re-queued to it: GetInCell >> 1.00); (a line for row T-08 `player.getincell 0021966F` re-queued to it: GetInCell >> 1.00); (a line for row T-08 `player.getincell 0021966F` re-queued to it: GetInCe... | **pass** | - |
| T-11 | `prid 09001007`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007 | number -19 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 09001007 -> prid 09001007 Command: prid 09001007; getpos x -> (only the echo; again in 2000 ms); getpos x -> getpos x Command: getpos x GetPos: X >> -19.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-11-1.png |
| T-12 | Papyrus log: KitKraft:HelloOnLoad on the base of REFR `KitKraftMiniSiteSwitchRef` writes "KitKraft: mini site switch loaded" | KitKraftMiniSite.esm:001007, KitKraftMiniSiteSwitchRef, 09001007, Scripts\KitKraft\HelloOnLoad.pex | papyrus-log KitKraft: mini site switch loaded | - | fail (`KitKraft: mini site switch loaded` not in `[00:16:40] Papyrus log opened (PC-64) [00:16:40] Update budget: 1.200000ms (Extra tasklet budget: 1.200000ms, Load screen budget: 500.000000ms) [00:16:40] Memor...`) | - |
| T-13 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| T-14 | `player.moveto 09001008`, `prid 09001008`, `getdistance player` | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008 | number 0 +/- 500 | player.moveto 09001008 -> player.moveto 09001008 Command: player.moveto 09001008; (waited 10s); prid 09001008 -> prid 09001008 Command: prid 09001008; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.83 | **pass** | - |
| T-15 | `prid 09001008`, `getisid 09001002` | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001008 -> prid 09001008 Command: prid 09001008; getisid 09001002 -> getisid 09001002 Command: getisid 09001002 GetIsID >> 1.00 | **pass** | - |
| T-16 | `prid 09001008`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008 | number -21 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 09001008 -> prid 09001008 Command: prid 09001008; getpos x -> getpos x Command: getpos x GetPos: X >> -21.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-16-1.png |
| T-17 | TerminalScript on the base of REFR `KitKraftMiniSiteTerminalRef`: its effect | KitKraftMiniSite.esm:001008, KitKraftMiniSiteTerminalRef, 09001008, Scripts\TerminalScript.pex | TerminalScript on the base of REFR `KitKraftMiniSiteTerminalRef`: its effect | - | not-run (manual: the compiled script is not next to the plugin (Scripts\TerminalScript.pex; `build` and `package` put it there), so its Papyrus log lines cannot be predicted) | - |
| T-18 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | Starfield.esm:21966F, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| T-19 | `player.moveto 09001009`, `prid 09001009`, `getdistance player` | KitKraftMiniSite.esm:001009, KitKraftMiniSitePackInRef, 09001009 | number 0 +/- 500 | player.moveto 09001009 -> player.moveto 09001009 Command: player.moveto 09001009; (waited 10s); prid 09001009 -> prid 09001009 Command: prid 09001009; getdistance player -> getdistance player Command: getdistance player GetDistance >> 0.25 | **pass** | - |
| T-20 | `prid 09001009`, `getisid 09001003` | KitKraftMiniSite.esm:001009, KitKraftMiniSitePackInRef, 09001009 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 09001009 -> prid 09001009 Command: prid 09001009; getisid 09001003 -> getisid 09001003 Command: getisid 09001003 GetIsID >> 0.00 | fail (/(?m)>>\s*1(\.0+)?\s*$/ does not match `GetIsID >> 0.00`) | - |
| T-21 | `prid 09001009`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSite.esm:001009, KitKraftMiniSitePackInRef, 09001009 | number -23 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | prid 09001009 -> prid 09001009 Command: prid 09001009; getpos x -> getpos x Command: getpos x GetPos: X >> -23.00; getpos y -> getpos y Command: getpos y GetPos: Y >> -4.00; getpos z -> getpos z Command: getpos z GetPos: Z >> 4.63 | **pass** | T-21-1.png |
| T-22 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261103-10`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261103-10`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00; save KitKraftSession-... | **pass** | - |
| T-23 | `coc CityNewAtlantisApartmentTEMPLATE`, `prid 09001007`, `getpos x`, `getpos y`, `getpos z`, `coc CityNewAtlantisApartmentTEMPLATE`, `prid 09001008`, `getpos x`, `getpos y`, `getpos z`, `coc CityNewAtlantisApartmentTEMPLATE`, `prid 09001009`, `getpos x`, `getpos y`, `getpos z` | KitKraftMiniSiteSwitchRef, 09001007, KitKraftMiniSiteTerminalRef, 09001008, KitKraftMiniSitePackInRef, 09001009 | number -19 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5; number -21 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5; number -23 +/- 0.5; number -4 +/- 0.5; number 4.63 +/- 0.5 | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); prid 09001007 -> prid 09001007 Command: prid 09001007; getpos x -> getpos x Command: getpos x GetPos: X >> -19.00; getpos... | **pass** | T-23-1.png |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (8b04153c827a569008ac764bf28f0f41a0b5d97f715cfe1df77df30d4c61fe22 -> 3c65835703a6ac1ed1935d0a3cde08fe5d1d71a1540934e4c8617ffc21e00b4c): restored from C:\Modding\kitkraft-observe\2026-11-03-T10\backup\01-plugins.txt

### game-20261103-09: agent session 2026-11-03-T11 (`kit game session`), OldSovietMarksmanRifle.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-03-T11`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-03 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261103-11` |
| Plugins | `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` |
| Probes | `C:\Modding\kitkraft-worktrees\x\out\rename-plan\probes.toml` SHA-256 `d84e011aab7bed95fb67a99e2d0e17eb1ad3debbe34429d5726afb4cfa55dc05` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-03-T11` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `help "Old Soviet Marksman Rifle" 4` | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | contains 0021BBCD | help "Old Soviet Marksman Rifle" 4 -> (only the echo; again in 2000 ms); help "Old Soviet Marksman Rifle" 4 -> (only the echo; again in 4000 ms); help "Old Soviet Marksman Rifle" 4 -> help "Old Soviet Marksman Rifle" 4 | inconclusive (`help "Old Soviet Marksman Rifle" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| T-02 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261103-11`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261103-11`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt; (waited 10s); player.getincell 0006BB29 -> (only the echo; again in 2000 ms); player.getincell 0006BB29 -> (only the echo; again in 4000 ms); player.getincell 0006BB29 -> player.getincell 0006BB29 Comman... | **pass** | - |
| T-03 | persistence 4: return and look | - | persistence 4: return and look | - | not-run (manual: the plugin places no new reference with a position and a runtime ID to check after the reload) | - |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (a07058b8fe30fa352d8ca79a0b9ec042f2f30f5031d16f61ec3d2d9aa4c4f23b -> e98f0a7950a816430df61854c64a4dbd184c8f35cbfaac6a752947ab4d1d1d72): restored from C:\Modding\kitkraft-observe\2026-11-03-T11\backup\01-plugins.txt

Writer's reading of `game-20261103-04` to `-09` (sessions 2026-11-03-T06 to -T11, run
2026-11-03 00:00 to 00:25 local, 05:00 to 05:25 UTC). X9 (M4.T6 sessions, M5.B3 OnLoad half)
and the two re-grade sessions; rulings R19, R20, R27 and R28. Every session ran on the
correlating controller of `aa31a9c9`, from `KitKraftBase`, on `Kitkraft Dev`. Each
`plugins.txt` change is MO2's rewrite at launch, restored by the controller (as since
`game-20261101-04`).

**T06 (`-04`), the scripts recipe (`KitKraftScripts.esm` with `HelloOnLoad.pex`, 1,064 bytes,
deployed as a dev package): 7 pass.**

- T-05 `papyrus-log` **pass**: `KitKraft: hello from a script attached by kit` is in the
  session's log at 00:02:38, 00:03:16 and 00:03:36 (`2026-11-03-T06\logs\papyrus-Papyrus.0.log`
  lines 144, 291, 377), each a few seconds after a fresh load of the activator's cell (the
  base-save load at 00:02:34 and T-01's `coc` to the embassy; T-06's trip to the Lodge, then
  the cell again). The row read the log after the first.
- So the script `kit` compiled and attached ran on OnLoad in game: Gate K0's "fires" half for
  OnLoad is **evidenced** by a row, not only by a log read afterwards (M5.B3, OnLoad half;
  O4-07). OnActivate stays open (input injection is deferred, ADR 0010 Amendment 1 (b)).
- T-01 to T-04 (cell, moveto, base `0003CCF4`, position (4, 4, 0)), T-06 (save in the Lodge,
  reload proved by the cell) and T-07 (position after the reload) **pass**.

**T07 (`-05`), the first POI, with one hand-added row: 6 pass, 1 inconclusive.**

- O2-11, corrected template (R19): T-03, `prid 06001000` then `getisid 001E4FBD`, answered
  `GetIsID >> 1.00`: **pass**. The crate is found by its runtime ID and its base is
  `Crate_MilitarySupply_01`.
- H-01, hand-added (not written by testplan; review L4): the control `help
  Crate_MilitarySupply_01 4` is **inconclusive**. Its replies held the `help` section
  headers (`----VIEWER STRINGS NOT AVAILABLE----`, `----OTHER FORMS----`) and no form line in
  three sends. So `help` on a base form's EditorID lists nothing through the console tool
  either, and the old O2-11 `help` lookup's empty answer says nothing about the reference.
- T-01, T-02, T-04 to T-06 **pass**, as in `game-20261101-04`.

**T08 (`-06`), the actors recipe: 4 pass, 2 fail.**

- O4-03: the actor's cell (T-01), `player.moveto` (T-02, `GetDistance >> 2.56`) and base
  (T-03, `getisid 00271133 >> 1.00`) **pass**.
- T-04 and T-06 **fail** on position: the actor answered (5.16, 3.58, 0.01) against its
  placed `DATA` (4, 4, 0), 1.2 m off, and the same after the reload. Cause open: an actor's
  AI may walk it off its placed spot (the X9 plan expected that), or the placement is off.
  Request (testplan, `handlers/testplan.rs`): for an `ACHR`, check the cell and a distance,
  not the placed position.
- O4-04 persistence: T-05 (save in the Lodge, reload proved by the cell) **pass**; after the
  reload the actor stood where it stood before (T-06's numbers equal T-04's).

**T09 (`-07`), ownership: 2 pass, 1 not-run.**

- T-01 persistence **pass**. T-02 is manual (the plugin places no new reference). O4-01 and
  O4-02 are UI rows and stay manual.
- H-01, hand-added (review L6): after T-01's load into the Lodge, `player.getincell 0021966F`
  answered `GetInCell >> 0.00`: **pass**. The negative control discriminates: a `1.00`
  shifted from another request would have failed it.

**O5-15, the overhaul's persistence: not-run.** The controller refuses a plugin that `kit
deploy --dev` did not install (`x9-s5.err`: `plugin-not-deployed`). The overhaul in
`Kitkraft Dev` is the owner's own mod `KitKraftGunOverhaul`, not a dev deploy, and a dev copy
beside it would change the owner's load order. Request (owner): say whether a dev copy may
replace it for one session. O5-16 not-run with it.

**The re-grades (R28), on the correlating controller.**

- T10 (`-08`), the mini-site, the same `probes.toml` as T05 (SHA-256 equal): 14 pass, 6
  inconclusive, 2 fail, 1 not-run as written; with T-12 read as below, 14, 7, 1, 1.
  - T-01 **pass**: its `KYWD: KitKraftLocTypeMiniSite (09001000)` came in T-02's reply and
    was re-queued to it (`seen`: "(late) ... in the reply to ..."). Correlation held.
  - T-08 to T-11 **pass** (they were inconclusive in T05). Their answers came late, in the
    replies to T-10's `getisid`, each labelled by its function (`GetInCell >> 1.00`,
    `GetDistance >> 10.44`), and went to their own requests.
  - T-02 to T-07 **inconclusive**: no line naming them arrived after three sends each (T-04
    got a `help` usage text, which names no request). Misrouting is gone; missing answers
    are not.
  - T-12 written `fail`: the row read the log between 00:19:15 and 00:19:38 (`T-11-1.png`,
    `T-16-1.png`), before any fresh load of the switch's cell; the Trace is at 00:20:21 and 00:20:42 (log lines 194 and 268), right after
    T-22's return from the Lodge and T-23's. **Inconclusive** (R20), as for T04 and T05.
  - T-20 **fail** again (`GetIsID >> 0.00`, own labelled reply, the third session).
- T11 (`-09`), V1 (`OldSovietMarksmanRifle.esm`, the X5 plan): T-01 `help` **inconclusive**
  (only the echo, three sends; no late line), T-02 persistence **pass**, T-03 manual. V1
  stays open: the rename has no correlated `help` answer yet, and additem is not-run (R22).
- Finding: a `help` answer that never arrives is the last one before a non-`help` row, or
  one whose later replies are all echo-only, as if the tool returns a `help` result only
  with a later request. Request (session lane): after a `help` row with no correlated
  reply, send one harmless command (for example `getpos x` on the player) and route its
  reply, before the next row.

**Unprobeable rows, with the getter each lacks.**

- OnActivate (Gate K0): needs activation input; injection is deferred.
- O2-09 (the long absence, PS-26) and O2-10 (`placeatme`, PS-09): no session step for a
  reset or a run-time placement.
- O4-01, O4-02 (ownership UI), the actors' look (screenshot and a person), T-17-style rows on
  vanilla scripts with no `.pex` beside the plugin (`testplan/traces.rs` reasons), and the
  manual "return and look" rows of plugins that place nothing.

Re-grade of `game-20261103-04` to `-09` (review `reviews/x-controller-R27-R28.md`, NOT
MERGEABLE on `aa31a9c9`; corrected controller `e6028aaa`). These six sessions were graded by
`aa31a9c9`, which could give a getter's line to a request when another request of the same
getter was still waiting for an answer (H1), and could lose a failure line on a retry (H2).
The re-grade is offline, from each session's saved transcript (`2026-11-03-T06` to `-T11`,
`report.json`, the `seen` of every row: each send, its retries and every re-queued line),
applying `e6028aaa`'s rules; no session was re-run. No reply in these sessions holds a console
failure line, so H2 and M2 change nothing. H1 changes every row whose getter line could also
belong to an earlier getter of the same function (same axis for `getpos`) that had more sends
than answers:

| Entry | Row | `aa31a9c9` | Re-graded (`e6028aaa`) | Why |
| ----- | --- | ---------- | ---------------------- | --- |
| `-04` (T06) | T-06 persistence | pass | **inconclusive (ambiguous reply)** | T-01's `player.getincell 00017307` was sent twice and answered once; each later `GetInCell >> 1.00` could be that late answer (`GetInCell` names no argument) |
| `-07` (T09) | T-01 persistence | pass | **inconclusive (ambiguous reply)** | its first `player.getincell 0006BB29` was sent twice, answered once (review probe P2's shape) |
| `-07` (T09) | H-01 negative control | pass | **inconclusive (ambiguous reply)** | the same outstanding send; the `0.00` may be its answer or the apartment's |
| `-08` (T10) | T-16, T-21, T-23 | pass | **inconclusive (ambiguous reply)** | T-11's `getpos x` was sent twice and answered once; each later `GetPos: X` line could be its late answer |
| `-09` (T11) | T-02 persistence | pass | **inconclusive (ambiguous reply)** | its first `player.getincell 0006BB29` was sent three times, answered once |

Every other pass stands: its getter had no other waiting request of the same function and axis
when its line came (T06 T-01 to T-05 and T-07; T07 all six; T08 T-01, T-02, T-03, T-05; T10
T-01, T-08 to T-11, T-13 to T-15, T-18, T-19, T-22, where T-08's and T-09's three sends each
got their three late answers before any other getter of their function was sent). The fails
stand (T08 T-04, T-06; T10 T-20: own labelled answers). Totals after the re-grade: T06 6 pass,
1 inconclusive; T07 6 pass, 1 inconclusive; T08 4 pass, 2 fail; T09 0 pass, 2 inconclusive,
1 not-run; T10 11 pass, 10 inconclusive (with T-12 as read above), 1 fail, 1 not-run; T11 0
pass, 2 inconclusive, 1 not-run.

So the reloads proved by the cell in X9 (T06 T-06, T09 T-01, T11 T-02) and L6's negative
control are **not** evidence any more; T10's T-22 (the mini-site's save in the Lodge and
reload) is. Request (session lane): before the next getter of a function that still has
sends without answers, wait for those answers (bounded), so a persistence row is not left
ambiguous by a retry earlier in the session.

### game-20261110-01: agent session 2026-11-10-T01 (`kit game session`), OldSovietMarksmanRifle.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-10-T01`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-10 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261110-01` |
| Plugins | `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` |
| Probes | `C:\Modding\kitkraft-worktrees\x\crates\game\probes\send-order.toml` SHA-256 `a8638979acccf83615e02ec81ba60465bf46ea93470390439b354da4f0a1350c` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-10-T01` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| SO-01 | control: coc to the Lodge, settle, then player.getincell on time | CityNewAtlantisLodgeInt, 0006BB29 | - | - | not-run (timeout after 180s waiting for the base save to load (`GetSFSEVersion` answered `GetSFSEVersion Command: GetSFSEVersion SFSE version: 0.2.21,...`, but 2 earlier poll(s) still owe their answer, so the line is theirs (send order), not ready)) | - |
| SO-02 | order: three player.getpos sent behind an unsettled coc answer late, X then Y then Z | CityNewAtlantisApartmentTEMPLATE, 0021966F | - | - | not-run (timeout after 180s waiting for the base save to load (`GetSFSEVersion` answered `GetSFSEVersion Command: GetSFSEVersion SFSE version: 0.2.21,...`, but 2 earlier poll(s) still owe their answer, so the line is theirs (send order), not ready)) | - |
| SO-03 | FR4 shape: prid of the rename's base EditorID behind an unsettled coc fails late | OldSovietMarksmanRifle.esm:0021BBCD, RussianHuntingRifle, CityNewAtlantisLodgeInt | - | - | not-run (timeout after 180s waiting for the base save to load (`GetSFSEVersion` answered `GetSFSEVersion Command: GetSFSEVersion SFSE version: 0.2.21,...`, but 2 earlier poll(s) still owe their answer, so the line is theirs (send order), not ready)) | - |
| SO-04 | FR4 shape: the same prid again, settled; its reply carries SO-03's late lines first | OldSovietMarksmanRifle.esm:0021BBCD, RussianHuntingRifle | - | - | not-run (timeout after 180s waiting for the base save to load (`GetSFSEVersion` answered `GetSFSEVersion Command: GetSFSEVersion SFSE version: 0.2.21,...`, but 2 earlier poll(s) still owe their answer, so the line is theirs (send order), not ready)) | - |
| SO-05 | FR3 shape: a reference-prefix command on a reference that does not exist fails late | KitKraftSendOrderRef, CityNewAtlantisApartmentTEMPLATE | - | - | not-run (timeout after 180s waiting for the base save to load (`GetSFSEVersion` answered `GetSFSEVersion Command: GetSFSEVersion SFSE version: 0.2.21,...`, but 2 earlier poll(s) still owe their answer, so the line is theirs (send order), not ready)) | - |
| SO-06 | FR3 shape: the next row's getter, whose reply carries SO-05's late failure and getter line first | CityNewAtlantisApartmentTEMPLATE, 0021966F | - | - | not-run (timeout after 180s waiting for the base save to load (`GetSFSEVersion` answered `GetSFSEVersion Command: GetSFSEVersion SFSE version: 0.2.21,...`, but 2 earlier poll(s) still owe their answer, so the line is theirs (send order), not ready)) | - |
| SO-07 | FR2 shape on time: load of the session save before it exists answers `Save file not found.` | KitKraftSession-* | - | - | not-run (timeout after 180s waiting for the base save to load (`GetSFSEVersion` answered `GetSFSEVersion Command: GetSFSEVersion SFSE version: 0.2.21,...`, but 2 earlier poll(s) still owe their answer, so the line is theirs (send order), not ready)) | - |
| SO-08 | FR2 shape late: load of the session save behind an unsettled coc and a waiting getter | KitKraftSession-*, CityNewAtlantisLodgeInt, 0006BB29 | - | - | not-run (timeout after 180s waiting for the base save to load (`GetSFSEVersion` answered `GetSFSEVersion Command: GetSFSEVersion SFSE version: 0.2.21,...`, but 2 earlier poll(s) still owe their answer, so the line is theirs (send order), not ready)) | - |

Finding: timeout after 180s waiting for the base save to load (`GetSFSEVersion` answered `GetSFSEVersion Command: GetSFSEVersion SFSE version: 0.2.21,...`, but 2 earlier poll(s) still owe their answer, so the line is theirs (send order), not ready)
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (286a54cf44506fa62b9596270f03b618a5e337b9d991a579d840a9a622cb9b2d -> e98f0a7950a816430df61854c64a4dbd184c8f35cbfaac6a752947ab4d1d1d72): restored from C:\Modding\kitkraft-observe\2026-11-10-T01\backup\01-plugins.txt

Writer's reading (sprint 2026-11-09, pipeline G, OW-1; the send-order confirming session, P2).

**Result: every row `not-run`; no probe ran.** Ruling 2 (send order) is untested by this
session; it was neither held nor contradicted.

What the transcript shows. This is the first session with a recorded transcript (M4.T7):
`C:\Modding\kitkraft-observe\2026-11-10-T01\report.json`, 38 sends, SHA-256
`0bd7f1d6e0003ee2381bbbf071a72f90b94da6d70ac63756bde866f2011993d0`.
- Send 1, the launch poll at 91 s: `GetSFSEVersion` answered its echo and
  `SFSE version: 0.2.21, release idx 26, runtime 01100F40`. Ready.
- Send 2, `load KitKraftBase`: answered its echo only, as the load always does.
- Sends 3 and 4, the base-save wait's first polls: answered only their echo; the game was
  loading the save.
- Sends 5 to 38, one every 5 s for about 170 s: each answered its echo and exactly **one**
  `SFSE version` line. The two lines sends 3 and 4 owed never arrived.

So the game **dropped** the answers of the polls it got while loading the save; it did not send
them late. The controller's readiness count (3b fix rounds 2 and 3: an echo-only poll owes its
line, and a reply is ready only when no older poll is owed) counted sends 3 and 4 as owed
forever. Every later line paid only its own poll, so the base-save wait timed out at 180 s and
all eight rows were `not-run`. This is the "lost readiness line ends the session at the next
wait" case that review L1'' named. It is now observed: the game loses an echo-only poll's
answer, which is the opposite of the late delivery that the count assumes.

Why this is not a contradiction of ruling 2: no line arrived out of order; two answers never
arrived at all. It does contradict the readiness count's premise that every poll's answer
eventually arrives, so as it stands any session whose base load produces an echo-only poll
cannot run. The fix is a grader decision (reported to the director at once, 15:25): for
example, an echo-only poll owes nothing, as the game has shown, or the debt is cleared at
each new wait. It needs a reviewed change before the next window.

Run facts.
- Binaries: `kit` and `kit-ui` built from `sprint/2026-11-09` @ `46519937` (`kit.exe`
  `c23c43cb241eb62a803a84c1ced177fa548c800320fb20cdd9f8fe674c14dbf5`, `kit-ui.exe`
  `a49770955f65f38bc20179bb3322086929167b61a780c173030034a79d9ab84f`).
- Probes: `crates/game/probes/send-order.toml` SHA-256
  `a8638979acccf83615e02ec81ba60465bf46ea93470390439b354da4f0a1350c`.
- Plugin `9c0b28a6...78ea27`; base save `fbed2cc2...eb5ccf`.
- Preflight, 15:02 to 15:19:
  - `Default` was refused (`live-profile`).
  - The first dev dry run was refused for `selected-profile` (MO2 selected `Default`; the
    owner changed it to `Kitkraft Dev` and closed MO2, confirmed 15:15) and for
    `plugin-not-deployed` (cleared by `deploy enable`).
  - Launched 15:19:13; the game window was up at 91 s; ended 15:23:48.
- List files (SHA-256):

  | File | Before enable | After enable | After disable |
  | ---- | ------------- | ------------ | ------------- |
  | `modlist.txt` | `14271bda...8344` | `4c97ae05...698f` | `9fae148b...a7f1` |
  | `plugins.txt` | `a3ff729f...b7e7` | `286a54cf...b2d` | `55912d83...a290` |
  | `loadorder.txt` | `672775b1...71b7` | `46b28b56...cd27` | `46b28b56...cd27` |

  - During the session MO2 rewrote `plugins.txt` (`286a54cf` → `e98f0a79`); the controller
    restored it. This is the known MO2 rewrite at launch (`game-20261103-01`).
  - After `deploy disable`, `plugins.txt` and `loadorder.txt` keep an inactive
    `OldSovietMarksmanRifle.esm` line: disable deactivates the plugin, it does not remove it.
  - `modlist.txt` also has two new disabled lines that no KitKraft command wrote:
    `-GRiNDTerra Planetary POI Generation` and `-Community Spaceship Expansion`. They are
    probably MO2's own, from the owner opening it to select the profile.

### game-20261116-01: agent session 2026-11-16-T01 (`kit game session`), OldSovietMarksmanRifle.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-16-T01`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-16 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261116-01` |
| Plugins | `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` |
| Probes | `crates\game\probes\send-order.toml` SHA-256 `a8638979acccf83615e02ec81ba60465bf46ea93470390439b354da4f0a1350c` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-16-T01` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| SO-01 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29 | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00 | **pass** | - |
| SO-02 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getpos x`, `player.getpos y`, `player.getpos z` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex GetPos: X >> -?\d; regex GetPos: Y >> -?\d; regex GetPos: Z >> -?\d | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; player.getpos x -> (only the echo; again in 2000 ms); player.getpos x -> player.getpos x Command: player.getpos x GetPos: X >> -8.00; p... | **pass** | - |
| SO-03 | `coc CityNewAtlantisLodgeInt`, `prid RussianHuntingRifle`, `player.getpos z` | OldSovietMarksmanRifle.esm:0021BBCD, RussianHuntingRifle, CityNewAtlantisLodgeInt | regex GetPos: Z >> -?\d | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; prid RussianHuntingRifle -> prid RussianHuntingRifle Command: prid RussianHuntingRifle; player.getpos z -> (only the echo; again in 2000 ms); pl... | **pass** | - |
| SO-04 | `prid RussianHuntingRifle`, `player.getpos x` | OldSovietMarksmanRifle.esm:0021BBCD, RussianHuntingRifle | regex GetPos: X >> -?\d | prid RussianHuntingRifle -> prid RussianHuntingRifle Command: prid RussianHuntingRifle Item 'RussianHuntingRifle' not found for parameter ObjectRefe...; (ambiguous: row SO-02 `player.getpos x`, row SO-04 `player.getpos x` could each claim `GetPos: X >> 11.01`; given to none); player.getpos x -> (no ... | fail (step 1 `prid RussianHuntingRifle`: the console answered `Item 'RussianHuntingRifle' not found for parameter ObjectReferenceID.`) | - |
| SO-05 | `coc CityNewAtlantisApartmentTEMPLATE`, `KitKraftSendOrderRef.enable`, `player.getpos x` | KitKraftSendOrderRef, CityNewAtlantisApartmentTEMPLATE | regex GetPos: X >> -?\d | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; KitKraftSendOrderRef.enable -> KitKraftSendOrderRef.enable Command: KitKraftSendOrderRef.enable; player.getpos x -> (only the echo; aga... | inconclusive (ambiguous reply: `player.getpos x` and `player.getpos x` (row SO-02), `player.getpos x` (row SO-04) are the same getter still waiting for an answer, and the line `GetPos: X >> -8.00` names neither apart; the text cannot tell them apart: no result to judge (a timing anomaly, R20, R28)) | - |
| SO-06 | `player.getincell 0021966F` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00; (waited 10s) | **pass** | - |
| SO-07 | `player.getincell 0021966F`, `load KitKraftSession-20261116-01`, `player.getincell 0021966F` | KitKraftSession-* | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00; load KitKraftSession-20261116-01 -> load KitKraftSession-20261116-01 Command: load KitKraftSession-20261116-01 Save file not found.; (waited 10s); player.getincell 0021966F -> player.getincel... | fail (step 2 `load KitKraftSession-20261116-01`: the console answered `Save file not found.`) | - |
| SO-08 | `coc CityNewAtlantisLodgeInt`, `player.getpos y`, `load KitKraftSession-20261116-01`, `player.getincell 0006BB29` | KitKraftSession-*, CityNewAtlantisLodgeInt, 0006BB29 | regex GetPos: Y >> -?\d; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; player.getpos y -> (only the echo; again in 2000 ms); player.getpos y -> (only the echo; again in 4000 ms); player.getpos y -> player.getpos y C... | fail (step 3 `load KitKraftSession-20261116-01`: the console answered `Save file not found.`) | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (286a54cf44506fa62b9596270f03b618a5e337b9d991a579d840a9a622cb9b2d -> e98f0a7950a816430df61854c64a4dbd184c8f35cbfaac6a752947ab4d1d1d72): restored from C:\Modding\kitkraft-observe\2026-11-16-T01\backup\01-plugins.txt

Writer's reading (bench 2026-11-16, block 1, OW-1; agent-run unattended on the owner's go of
2026-11-16 just after midnight, "go for it! you've got the system all night"). Covers this entry and its
retry `game-20261116-02`, whose transcript is identical send for send.

**Result: send order inconclusive (untested): no line arrived late, in either attempt.**
Answers to commands sent while a `coc` (or the base `load`) is moving are **dropped, not
delayed**. Not a contradiction by the run sheet's definition, but see SO-03 below; reported
to the director.

**For the owner to look at (SO-03):** send 14 `prid RussianHuntingRifle` answered only its
echo; send 17 `player.getpos z` answered `GetPos: Z >> 0.01`; send 18, the same `prid` sent
again, answered `Item 'RussianHuntingRifle' not found for parameter ObjectReferenceID.`.
Two `prid`s, one failure line. If that line was send 18's own (send 14's dropped), nothing
was out of order. If it was send 14's, arriving late, then send 17's `Z` came before an
earlier request's line: the run sheet's contradiction. The text cannot tell the two
identical lines apart; this reading takes the first, because every other echo-only send in
both attempts lost its answer. Your call whether that is enough to keep ruling 2 (send
order) standing; both transcripts are in
`C:\Modding\kitkraft-observe\2026-11-16-T01\report.json` and `-T02\`, sends 14 to 18.

Rows (both attempts the same): SO-01 `pass` (the control holds: the session tested
something); SO-02 `pass`; SO-03 `pass` (predicted not pass); SO-04 `fail` (its own `prid`,
as predicted); SO-05 `inconclusive` (ambiguous `GetPos: X` lines, as the grader routes
them); SO-06 `pass`; SO-07 `fail` (`Save file not found.` on time, as predicted); SO-08
`fail` (its `load`'s own line, on time, not late). SO-05 is not `pass` and SO-06 not `fail`:
the grade-visible contradiction shape did not occur.

What the transcript shows (`report.json` 36 sends each; T01 SHA-256
`3c656b6cf1e676509283f619624aea299f0f3b424454959bad5f96871f265276`, T02
`42f31616ce5425ba4faace3a22f7ab4eca73e669875485b22d6891eca938d685`):
- Every reply holds the lines of **one** request only, its own. No reply carried a line of
  an earlier request; no `(late)` routing occurred.
- Every send the game took while a `coc` (or the base `load`) was still moving answered
  only its echo, and **its result never arrived anywhere**: sends 3 and 4 (base-wait polls),
  9 (`player.getpos x`: one `X` line for sends 9 and 10), 14 (`prid RussianHuntingRifle`:
  its failure line never came), 15 and 16 (`getpos z`: one `Z` line for three sends), 23
  (`KitKraftSendOrderRef.enable`: no failure line ever), 24 (one `X` line short), 32 and 33.
  This is the T01 drop of 2026-11-10 (`game-20261110-01`), now seen for getters, `prid` and a
  reference-prefix command too, not only for polls.
- SO-03: the run sheet's literal SO-03 contradiction shape ("a `GetPos: Z` line of SO-03
  before any failure line, while SO-03's `prid` reply was only its echo") is met: send 17's
  `Z` line came before the only failure line (send 18). It is a reordering only if send 18's
  line was send 14's late line and send 18's own line was dropped. The two lines are
  identical and cannot be told apart (run sheet: evidence neither way); every other
  echo-only send in both attempts lost its answer rather than receiving it late, and a
  settled `prid` (send 18, 14 s after the `coc`) answers in its own reply. So the reading is
  "dropped", not "out of order". SO-03 grades `pass` only because the failure the game never
  sent is invisible: the limit no grader passes (R41, `docs/game-sessions.md` "Statuses").
- Consequence for the grader: under these timings the late stretch the probes rely on does
  not exist; the game drops, it does not defer. A row that sends a failing command behind an
  unsettled move can pass. Rows should wait out every move before sending what they grade.

Run facts: `kit.exe` release `0bbcfc6d...317172` (built from `a618e7d6`; `main` at
`2ef0c0cc`, docs only since). Preflight `fails: 0` 03:06; `deploy enable --mods
OldSovietMarksmanRifle` nothing to change. T01 launched 03:09:43, first poll answered at
91.4 s, base ready at send 5 (101.8 s, the R41 pacing poll worked in game for the first
time), ended 03:12:47; T02 03:13:45 to about 03:17, the same timings within 0.1 s. List
files before, after T01, after T02: `modlist.txt` `bfa10ad4`, `plugins.txt` `286a54cf`,
`loadorder.txt` `46b28b56`, `lockedorder.txt` `9962fbb0` throughout (MO2's known
`plugins.txt` rewrite at launch was restored by the controller both times). No timeout, no
diagnostics finding, no stuck screenshot. The rename plugin stays enabled (OW-2 needs it).

### game-20261116-02: agent session 2026-11-16-T02 (`kit game session`), OldSovietMarksmanRifle.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-16-T02`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-16 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261116-02` |
| Plugins | `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` |
| Probes | `crates\game\probes\send-order.toml` SHA-256 `a8638979acccf83615e02ec81ba60465bf46ea93470390439b354da4f0a1350c` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-16-T02` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| SO-01 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29 | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00 | **pass** | - |
| SO-02 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getpos x`, `player.getpos y`, `player.getpos z` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex GetPos: X >> -?\d; regex GetPos: Y >> -?\d; regex GetPos: Z >> -?\d | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; player.getpos x -> (only the echo; again in 2000 ms); player.getpos x -> player.getpos x Command: player.getpos x GetPos: X >> -8.00; p... | **pass** | - |
| SO-03 | `coc CityNewAtlantisLodgeInt`, `prid RussianHuntingRifle`, `player.getpos z` | OldSovietMarksmanRifle.esm:0021BBCD, RussianHuntingRifle, CityNewAtlantisLodgeInt | regex GetPos: Z >> -?\d | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; prid RussianHuntingRifle -> prid RussianHuntingRifle Command: prid RussianHuntingRifle; player.getpos z -> (only the echo; again in 2000 ms); pl... | **pass** | - |
| SO-04 | `prid RussianHuntingRifle`, `player.getpos x` | OldSovietMarksmanRifle.esm:0021BBCD, RussianHuntingRifle | regex GetPos: X >> -?\d | prid RussianHuntingRifle -> prid RussianHuntingRifle Command: prid RussianHuntingRifle Item 'RussianHuntingRifle' not found for parameter ObjectRefe...; (ambiguous: row SO-02 `player.getpos x`, row SO-04 `player.getpos x` could each claim `GetPos: X >> 11.01`; given to none); player.getpos x -> (no ... | fail (step 1 `prid RussianHuntingRifle`: the console answered `Item 'RussianHuntingRifle' not found for parameter ObjectReferenceID.`) | - |
| SO-05 | `coc CityNewAtlantisApartmentTEMPLATE`, `KitKraftSendOrderRef.enable`, `player.getpos x` | KitKraftSendOrderRef, CityNewAtlantisApartmentTEMPLATE | regex GetPos: X >> -?\d | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; KitKraftSendOrderRef.enable -> KitKraftSendOrderRef.enable Command: KitKraftSendOrderRef.enable; player.getpos x -> (only the echo; aga... | inconclusive (ambiguous reply: `player.getpos x` and `player.getpos x` (row SO-02), `player.getpos x` (row SO-04) are the same getter still waiting for an answer, and the line `GetPos: X >> -8.00` names neither apart; the text cannot tell them apart: no result to judge (a timing anomaly, R20, R28)) | - |
| SO-06 | `player.getincell 0021966F` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00; (waited 10s) | **pass** | - |
| SO-07 | `player.getincell 0021966F`, `load KitKraftSession-20261116-02`, `player.getincell 0021966F` | KitKraftSession-* | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00; load KitKraftSession-20261116-02 -> load KitKraftSession-20261116-02 Command: load KitKraftSession-20261116-02 Save file not found.; (waited 10s); player.getincell 0021966F -> player.getincel... | fail (step 2 `load KitKraftSession-20261116-02`: the console answered `Save file not found.`) | - |
| SO-08 | `coc CityNewAtlantisLodgeInt`, `player.getpos y`, `load KitKraftSession-20261116-02`, `player.getincell 0006BB29` | KitKraftSession-*, CityNewAtlantisLodgeInt, 0006BB29 | regex GetPos: Y >> -?\d; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; player.getpos y -> (only the echo; again in 2000 ms); player.getpos y -> (only the echo; again in 4000 ms); player.getpos y -> player.getpos y C... | fail (step 3 `load KitKraftSession-20261116-02`: the console answered `Save file not found.`) | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (286a54cf44506fa62b9596270f03b618a5e337b9d991a579d840a9a622cb9b2d -> e98f0a7950a816430df61854c64a4dbd184c8f35cbfaac6a752947ab4d1d1d72): restored from C:\Modding\kitkraft-observe\2026-11-16-T02\backup\01-plugins.txt

Writer's reading: the retry of `game-20261116-01` (run sheet: two attempts), whose reading covers both. The same 36 sends, the same replies and the same grades; no line arrived late. Send order: inconclusive after two attempts; request to the session lane: rows that wait out each move, or a shape that makes the game defer rather than drop.

### game-20261116-03: agent session 2026-11-16-T03 (`kit game session`), KitKraftVerify.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-16-T03`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-16 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261116-03` |
| Plugins | `KitKraftVerify.esm` SHA-256 `a9632f11d0ad0ade56170fa424c48d5f5a29bb8475c8977e8870a0289be39faa` |
| Probes | `out\verify\probes.toml` SHA-256 `27cdbf4d5b902012e48c84f291b935f777f0a7ca2dff0e5f5567c359a4679293` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-16-T03` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| V-01 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| V-20 | `prid 0A00102F`, `getisid 0A001000` | KitKraftVerify.esm:00102F, KKV70Reporter, 0A00102F, KKV10Acti, 0A001000 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A00102F -> prid 0A00102F Command: prid 0A00102F; getisid 0A001000 -> getisid 0A001000 Command: getisid 0A001000 GetIsID >> 1.00 | **pass** | - |
| V-21 | `prid 0A001030`, `getisid 0A001001` | KitKraftVerify.esm:001030, KKV71StatRef, 0A001030, KKV11Stat, 0A001001 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001030 -> prid 0A001030 Command: prid 0A001030; getisid 0A001001 -> getisid 0A001001 Command: getisid 0A001001 GetIsID >> 1.00 | **pass** | - |
| V-22 | `prid 0A001031`, `getisid 0A001002` | KitKraftVerify.esm:001031, KKV72MsttRef, 0A001031, KKV12Mstt, 0A001002 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001031 -> prid 0A001031 Command: prid 0A001031; getisid 0A001002 -> getisid 0A001002 Command: getisid 0A001002 GetIsID >> 1.00 | **pass** | - |
| V-23 | `prid 0A001032`, `getisid 0A001003` | KitKraftVerify.esm:001032, KKV73FurnRef, 0A001032, KKV13Furn, 0A001003 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001032 -> prid 0A001032 Command: prid 0A001032; getisid 0A001003 -> getisid 0A001003 Command: getisid 0A001003 GetIsID >> 1.00 | **pass** | - |
| V-24 | `prid 0A001033`, `getisid 0A001004` | KitKraftVerify.esm:001033, KKV74FlorRef, 0A001033, KKV14Flor, 0A001004 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001033 -> prid 0A001033 Command: prid 0A001033; getisid 0A001004 -> getisid 0A001004 Command: getisid 0A001004 GetIsID >> 1.00 | **pass** | - |
| V-25 | `prid 0A001034`, `getisid 0A001005` | KitKraftVerify.esm:001034, KKV75DoorRef, 0A001034, KKV15Door, 0A001005 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001034 -> prid 0A001034 Command: prid 0A001034; getisid 0A001005 -> getisid 0A001005 Command: getisid 0A001005 GetIsID >> 1.00 | **pass** | - |
| V-26 | `prid 0A001035`, `getisid 0A001006` | KitKraftVerify.esm:001035, KKV76LighRef, 0A001035, KKV16Ligh, 0A001006 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001035 -> prid 0A001035 Command: prid 0A001035; getisid 0A001006 -> getisid 0A001006 Command: getisid 0A001006 GetIsID >> 1.00 | **pass** | - |
| V-27 | `prid 0A001036`, `getisid 0A001007` | KitKraftVerify.esm:001036, KKV77ContRef, 0A001036, KKV17Cont, 0A001007 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001036 -> prid 0A001036 Command: prid 0A001036; getisid 0A001007 -> getisid 0A001007 Command: getisid 0A001007 GetIsID >> 1.00 | **pass** | - |
| V-28 | `prid 0A001037`, `getisid 0A001009` | KitKraftVerify.esm:001037, KKV78TermRef, 0A001037, KKV19Term, 0A001009 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001037 -> prid 0A001037 Command: prid 0A001037; getisid 0A001009 -> getisid 0A001009 Command: getisid 0A001009 GetIsID >> 1.00 | **pass** | - |
| V-30 | `prid 0A001039`, `getisid 0A00100C` | KitKraftVerify.esm:001039, KKV80MiscRef, 0A001039, KKV30Misc, 0A00100C | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001039 -> prid 0A001039 Command: prid 0A001039; getisid 0A00100C -> getisid 0A00100C Command: getisid 0A00100C GetIsID >> 1.00 | **pass** | - |
| V-31 | `prid 0A00103A`, `getisid 0A00100B` | KitKraftVerify.esm:00103A, KKV81NpcRef, 0A00103A, KKV21Npc, 0A00100B | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A00103A -> prid 0A00103A Command: prid 0A00103A; getisid 0A00100B -> getisid 0A00100B Command: getisid 0A00100B GetIsID >> 1.00 | **pass** | - |
| V-40 | `coc CityNewAtlantisLodgeInt`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisLodgeInt, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpa... | **pass** | - |
| V-P00 | Papyrus log: KKV-P00 reporter loaded (ACTI VMAD) | KKV70Reporter, 0A00102F | papyrus-log KKV-P00 reporter loaded | - | **pass** | - |
| V-P01 | Papyrus log: KKV-P01 GLOB value=4242 (GLOB) | KKV70Reporter, 0A00102F | papyrus-log KKV-P01 GLOB value=4242 | - | **pass** | - |
| V-P02 | Papyrus log: KKV-P02 FLST size=2 (FLST) | KKV70Reporter, 0A00102F | papyrus-log KKV-P02 FLST size=2 | - | **pass** | - |
| V-P03 | Papyrus log: KKV-P03 FLST has-mug=True has-wrench=True (FLST) | KKV70Reporter, 0A00102F | papyrus-log KKV-P03 FLST has-mug=True has-wrench=True | - | **pass** | - |
| V-P04 | Papyrus log: KKV-P04 MISC gold=777 (MISC) | KKV70Reporter, 0A00102F | papyrus-log KKV-P04 MISC gold=777 | - | **pass** | - |
| V-P05 | Papyrus log: KKV-P05 KYWD on-reporter=True (KYWD) | KKV70Reporter, 0A00102F | papyrus-log KKV-P05 KYWD on-reporter=True | - | **pass** | - |
| V-P06 | Papyrus log: KKV-P06 FACT player-in=True (FACT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P06 FACT player-in=True | - | **pass** | - |
| V-P07 | Papyrus log: KKV-P07 PERK player-has=True (PERK) | KKV70Reporter, 0A00102F | papyrus-log KKV-P07 PERK player-has=True | - | **pass** | - |
| V-P08 | Papyrus log: KKV-P08 AVIF player-value=7 (AVIF) | KKV70Reporter, 0A00102F | papyrus-log KKV-P08 AVIF player-value=7 | - | **pass** | - |
| V-P09 | Papyrus log: KKV-P09 QUST running=True stage10-done=True (QUST) | KKV70Reporter, 0A00102F | papyrus-log KKV-P09 QUST running=True stage10-done=True | - | **pass** | - |
| V-P10 | Papyrus log: KKV-P10 LCTN reporter-in=True (LCTN) | KKV70Reporter, 0A00102F | papyrus-log KKV-P10 LCTN reporter-in=True | - | **pass** | - |
| V-P11 | Papyrus log: KKV-P11 CONT mugs=3 (CONT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P11 CONT mugs=3 | - | **pass** | - |
| V-P12 | Papyrus log: KKV-P12 LVLI wrench-added=1 (LVLI) | KKV70Reporter, 0A00102F | papyrus-log KKV-P12 LVLI wrench-added=1 | - | **pass** | - |
| V-P20 | Papyrus log: KKV-P20 ACTI base-is=True (ACTI) | KKV70Reporter, 0A00102F | papyrus-log KKV-P20 ACTI base-is=True | - | **pass** | - |
| V-P21 | Papyrus log: KKV-P21 STAT base-is=True (STAT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P21 STAT base-is=True | - | **pass** | - |
| V-P22 | Papyrus log: KKV-P22 MSTT base-is=True (MSTT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P22 MSTT base-is=True | - | **pass** | - |
| V-P23 | Papyrus log: KKV-P23 FURN base-is=True (FURN) | KKV70Reporter, 0A00102F | papyrus-log KKV-P23 FURN base-is=True | - | **pass** | - |
| V-P24 | Papyrus log: KKV-P24 FLOR base-is=True (FLOR) | KKV70Reporter, 0A00102F | papyrus-log KKV-P24 FLOR base-is=True | - | **pass** | - |
| V-P25 | Papyrus log: KKV-P25 DOOR base-is=True (DOOR) | KKV70Reporter, 0A00102F | papyrus-log KKV-P25 DOOR base-is=True | - | **pass** | - |
| V-P26 | Papyrus log: KKV-P26 LIGH base-is=True (LIGH) | KKV70Reporter, 0A00102F | papyrus-log KKV-P26 LIGH base-is=True | - | **pass** | - |
| V-P27 | Papyrus log: KKV-P27 CONT base-is=True (CONT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P27 CONT base-is=True | - | **pass** | - |
| V-P28 | Papyrus log: KKV-P28 TERM base-is=True (TERM) | KKV70Reporter, 0A00102F | papyrus-log KKV-P28 TERM base-is=True | - | **pass** | - |
| V-P30 | Papyrus log: KKV-P30 MISC base-is=True (MISC) | KKV70Reporter, 0A00102F | papyrus-log KKV-P30 MISC base-is=True | - | **pass** | - |
| V-P31 | Papyrus log: KKV-P31 NPC_ base-is=True (NPC_) | KKV70Reporter, 0A00102F | papyrus-log KKV-P31 NPC_ base-is=True | - | **pass** | - |
| V-P29 | Papyrus log: KKV-P29 PKIN base-is=True (PKIN) | KKV70Reporter, 0A00102F | papyrus-log KKV-P29 PKIN base-is=True | - | **pass** | - |
| V-P98 | Papyrus log: KKV-P98 PKIN reference base= (PKIN (diagnostic)) | KKV70Reporter, 0A00102F | papyrus-log KKV-P98 PKIN reference base= | - | **pass** | - |
| V-P99 | Papyrus log: KKV-P99 done (-) | KKV70Reporter, 0A00102F | papyrus-log KKV-P99 done | - | **pass** | - |
| V-I-MISC | `player.additem 0A00100C 1`, `player.getitemcount 0A00100C` | KitKraftVerify.esm:00100C, KKV30Misc, 0A00100C | regex >>\s*[1-9] | player.additem 0A00100C 1 -> player.additem 0A00100C 1 Command: player.additem 0A00100C 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A00100C -> player.getitemcount 0A00100C Command: player.getitemcount 0A00100C GetItemCount >> 1.00 | **pass** | - |
| V-I-ALCH | `player.additem 0A00100E 1`, `player.getitemcount 0A00100E` | KitKraftVerify.esm:00100E, KKV32Alch, 0A00100E | regex >>\s*[1-9] | player.additem 0A00100E 1 -> player.additem 0A00100E 1 Command: player.additem 0A00100E 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A00100E -> player.getitemcount 0A00100E Command: player.getitemcount 0A00100E GetItemCount >> 1.00 | **pass** | - |
| V-I-BOOK | `player.additem 0A00100F 1`, `player.getitemcount 0A00100F` | KitKraftVerify.esm:00100F, KKV33Book, 0A00100F | regex >>\s*[1-9] | player.additem 0A00100F 1 -> player.additem 0A00100F 1 Command: player.additem 0A00100F 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A00100F -> player.getitemcount 0A00100F Command: player.getitemcount 0A00100F GetItemCount >> 1.00 | **pass** | - |
| V-I-KEYM | `player.additem 0A001010 1`, `player.getitemcount 0A001010` | KitKraftVerify.esm:001010, KKV34Keym, 0A001010 | regex >>\s*[1-9] | player.additem 0A001010 1 -> player.additem 0A001010 1 Command: player.additem 0A001010 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A001010 -> player.getitemcount 0A001010 Command: player.getitemcount 0A001010 GetItemCount >> 1.00 | **pass** | - |
| V-I-ARMO | `player.additem 0A001011 1`, `player.getitemcount 0A001011` | KitKraftVerify.esm:001011, KKV35Armo, 0A001011 | regex >>\s*[1-9] | player.additem 0A001011 1 -> player.additem 0A001011 1 Command: player.additem 0A001011 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A001011 -> player.getitemcount 0A001011 Command: player.getitemcount 0A001011 GetItemCount >> 1.00 | **pass** | - |
| V-I-ARMO-EQ | `player.equipitem 0A001011`, `player.getequipped 0A001011` | KKV35Armo, 0A001011 | regex (?m)>>\s*1(\.0+)?\s*$ | player.equipitem 0A001011 -> player.equipitem 0A001011 Command: player.equipitem 0A001011 GetIsRace HumanRace >> 1.00 IsTrueForConditionForm:  >> 1.0...; player.getequipped 0A001011 -> player.getequipped 0A001011 Command: player.getequipped 0A001011 GetEquipped >> 1.00 | **pass** | - |
| V-I-LVLI | `player.additem 0A00102E 1`, `player.getitemcount 0A00100D` | KKV06Lvli, 0A00102E, KKV31Misc, 0A00100D | regex >>\s*[1-9] | player.additem 0A00102E 1 -> player.additem 0A00102E 1 Command: player.additem 0A00102E 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A00100D -> player.getitemcount 0A00100D Command: player.getitemcount 0A00100D GetItemCount >> 1.00 | **pass** | - |
| V-C-GLOB | `getglobalvalue KKV01Glob` | KKV01Glob, 0A001029 | number 4242 +/- 0.5 | getglobalvalue KKV01Glob -> getglobalvalue KKV01Glob Command: getglobalvalue KKV01Glob Item 'KKV01Glob' not found for parameter Global.  Compiled sc...; (late, after its answer) getglobalvalue KKV01Glob -> GetGlobalValue >> 0.00 (in the reply to `coc CityNewAtlantisLodgeInt`; not graded); (late, aft... | fail (step 1 `getglobalvalue KKV01Glob`: the console answered `Item 'KKV01Glob' not found for parameter Global.`) | - |
| V-C-FACT | `player.getinfaction 0A00102B` | KKV03Fact, 0A00102B | regex (?m)>>\s*1(\.0+)?\s*$ | player.getinfaction 0A00102B -> player.getinfaction 0A00102B Command: player.getinfaction 0A00102B GetInFaction >> 1.00 | **pass** | - |
| V-C-PERK | `player.hasperk 0A001013` | KKV41Perk, 0A001013 | regex (?m)>>\s*1(\.0+)?\s*$ | player.hasperk 0A001013 -> player.hasperk 0A001013 Command: player.hasperk 0A001013 Perk Rank >> 1; (late failure) player.hasperk 0A001013 -> Item 'KKV04Qust' not found for parameter Quest. (in the reply to `getstage KKV04Qust`); (late failure) player.hasperk 0A001013 -> Item 'KKV04Qust' not found f... | inconclusive (late failure: `player.hasperk 0A001013`: the console answered `Item 'KKV04Qust' not found for parameter Quest.`, late, in the reply to `getstage KKV04Qust` (R20, R28)) | - |
| V-C-AVIF | `player.getav KKV40Avif` | KKV40Avif, 0A001012 | number 7 +/- 0.5 | player.getav KKV40Avif -> (no line of its own: player.getav KKV40Avif Command: player.getav KKV40Avif GetActorValue: KKV40Avif >> 7.00; again in 2000 ms); player.getav KKV40Avif -> (no line of its own: player.getav KKV40Avif Command: player.getav KKV40Avif GetActorValue: KKV40Avif >> 7.00; again in ... | inconclusive (late failure: `player.getav KKV40Avif`: the console answered `Item 'KKV04Qust' not found for parameter Quest.`, late, in the reply to `getstage KKV04Qust` (R20, R28)) | - |
| V-C-QUST | `getstage KKV04Qust` | KKV04Qust, 0A00102C | number 10 +/- 0.5 | (a failure line for row V-C-PERK `player.hasperk 0A001013` arrived in this reply: `Item 'KKV04Qust' not found for parameter Quest.`); (a failure line for row V-C-AVIF `player.getav KKV40Avif` arrived in this reply: `Item 'KKV04Qust' not found for parameter Quest.`); getstage KKV04Qust -> (no line of... | inconclusive (late failure: `getstage KKV04Qust`: the console answered `Item 'KKV04Qust' not found for parameter Quest.`, late, in the reply to `getstage KKV04Qust` (R20, R28)) | - |
| V-S-01 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261116-03`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261116-03`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | (a line for row V-C-GLOB `getglobalvalue KKV01Glob` re-queued to it: GetGlobalValue >> 0.00); (a line for row V-C-GLOB `getglobalvalue KKV01Glob`, which had all its answers: GetGlobalValue >> 0.00; not graded); coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLo... | **pass** | - |
| V-S-02 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | (a line for row V-C-GLOB `getglobalvalue KKV01Glob`, which had all its answers: GetGlobalValue >> 0.00; not graded); (a line for row V-C-GLOB `getglobalvalue KKV01Glob`, which had all its answers: GetGlobalValue >> 0.00; not graded); coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartme... | **pass** | - |
| V-S-20 | `prid 0A00102F`, `getisid 0A001000` | KKV70Reporter, 0A00102F, KKV10Acti, 0A001000 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A00102F -> prid 0A00102F Command: prid 0A00102F; getisid 0A001000 -> getisid 0A001000 Command: getisid 0A001000 GetIsID >> 1.00 | **pass** | - |
| V-S-21 | `prid 0A001030`, `getisid 0A001001` | KKV71StatRef, 0A001030, KKV11Stat, 0A001001 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001030 -> prid 0A001030 Command: prid 0A001030; getisid 0A001001 -> getisid 0A001001 Command: getisid 0A001001 GetIsID >> 1.00 | **pass** | - |
| V-S-22 | `prid 0A001031`, `getisid 0A001002` | KKV72MsttRef, 0A001031, KKV12Mstt, 0A001002 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001031 -> prid 0A001031 Command: prid 0A001031; getisid 0A001002 -> getisid 0A001002 Command: getisid 0A001002 GetIsID >> 1.00 | **pass** | - |
| V-S-23 | `prid 0A001032`, `getisid 0A001003` | KKV73FurnRef, 0A001032, KKV13Furn, 0A001003 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001032 -> prid 0A001032 Command: prid 0A001032; getisid 0A001003 -> getisid 0A001003 Command: getisid 0A001003 GetIsID >> 1.00 | **pass** | - |
| V-S-24 | `prid 0A001033`, `getisid 0A001004` | KKV74FlorRef, 0A001033, KKV14Flor, 0A001004 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001033 -> prid 0A001033 Command: prid 0A001033; getisid 0A001004 -> getisid 0A001004 Command: getisid 0A001004 GetIsID >> 1.00 | **pass** | - |
| V-S-25 | `prid 0A001034`, `getisid 0A001005` | KKV75DoorRef, 0A001034, KKV15Door, 0A001005 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001034 -> prid 0A001034 Command: prid 0A001034; getisid 0A001005 -> getisid 0A001005 Command: getisid 0A001005 GetIsID >> 1.00 | **pass** | - |
| V-S-26 | `prid 0A001035`, `getisid 0A001006` | KKV76LighRef, 0A001035, KKV16Ligh, 0A001006 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001035 -> prid 0A001035 Command: prid 0A001035; getisid 0A001006 -> getisid 0A001006 Command: getisid 0A001006 GetIsID >> 1.00 | **pass** | - |
| V-S-27 | `prid 0A001036`, `getisid 0A001007` | KKV77ContRef, 0A001036, KKV17Cont, 0A001007 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001036 -> prid 0A001036 Command: prid 0A001036; getisid 0A001007 -> getisid 0A001007 Command: getisid 0A001007 GetIsID >> 1.00 | **pass** | - |
| V-S-28 | `prid 0A001037`, `getisid 0A001009` | KKV78TermRef, 0A001037, KKV19Term, 0A001009 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001037 -> prid 0A001037 Command: prid 0A001037; getisid 0A001009 -> getisid 0A001009 Command: getisid 0A001009 GetIsID >> 1.00 | **pass** | - |
| V-S-30 | `prid 0A001039`, `getisid 0A00100C` | KKV80MiscRef, 0A001039, KKV30Misc, 0A00100C | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001039 -> prid 0A001039 Command: prid 0A001039; getisid 0A00100C -> getisid 0A00100C Command: getisid 0A00100C GetIsID >> 1.00 | **pass** | - |
| V-S-31 | `prid 0A00103A`, `getisid 0A00100B` | KKV81NpcRef, 0A00103A, KKV21Npc, 0A00100B | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A00103A -> prid 0A00103A Command: prid 0A00103A; getisid 0A00100B -> getisid 0A00100B Command: getisid 0A00100B GetIsID >> 1.00 | **pass** | - |
| V-S-I-MISC | `player.getitemcount 0A00100C` | KKV30Misc, 0A00100C | regex >>\s*[1-9] | player.getitemcount 0A00100C -> player.getitemcount 0A00100C Command: player.getitemcount 0A00100C GetItemCount >> 1.00 | **pass** | - |
| V-S-I-ALCH | `player.getitemcount 0A00100E` | KKV32Alch, 0A00100E | regex >>\s*[1-9] | player.getitemcount 0A00100E -> player.getitemcount 0A00100E Command: player.getitemcount 0A00100E GetItemCount >> 1.00 | **pass** | - |
| V-S-I-BOOK | `player.getitemcount 0A00100F` | KKV33Book, 0A00100F | regex >>\s*[1-9] | player.getitemcount 0A00100F -> player.getitemcount 0A00100F Command: player.getitemcount 0A00100F GetItemCount >> 1.00 | **pass** | - |
| V-S-I-KEYM | `player.getitemcount 0A001010` | KKV34Keym, 0A001010 | regex >>\s*[1-9] | player.getitemcount 0A001010 -> player.getitemcount 0A001010 Command: player.getitemcount 0A001010 GetItemCount >> 1.00 | **pass** | - |
| V-S-I-ARMO | `player.getitemcount 0A001011` | KKV35Armo, 0A001011 | regex >>\s*[1-9] | player.getitemcount 0A001011 -> player.getitemcount 0A001011 Command: player.getitemcount 0A001011 GetItemCount >> 1.00 | **pass** | - |
| V-S-I-LVLI | `player.getitemcount 0A00100D` | KKV31Misc, 0A00100D | regex >>\s*[1-9] | player.getitemcount 0A00100D -> player.getitemcount 0A00100D Command: player.getitemcount 0A00100D GetItemCount >> 1.00 | **pass** | - |
| V-S-GLOB | `getglobalvalue KKV01Glob` | KKV01Glob | number 4242 +/- 0.5 | getglobalvalue KKV01Glob -> getglobalvalue KKV01Glob Command: getglobalvalue KKV01Glob Item 'KKV01Glob' not found for parameter Global.  Compiled sc... | fail (step 1 `getglobalvalue KKV01Glob`: the console answered `Item 'KKV01Glob' not found for parameter Global.`) | - |
| V-S-FACT | `player.getinfaction 0A00102B` | KKV03Fact | regex (?m)>>\s*1(\.0+)?\s*$ | player.getinfaction 0A00102B -> player.getinfaction 0A00102B Command: player.getinfaction 0A00102B GetInFaction >> 1.00 | **pass** | - |
| V-S-PERK | `player.hasperk 0A001013` | KKV41Perk | regex (?m)>>\s*1(\.0+)?\s*$ | player.hasperk 0A001013 -> player.hasperk 0A001013 Command: player.hasperk 0A001013 Perk Rank >> 2; (late failure) player.hasperk 0A001013 -> Item 'KKV04Qust' not found for parameter Quest. (in the reply to `getstage KKV04Qust`); (late failure) player.hasperk 0A001013 -> Item 'KKV04Qust' not found f... | fail (/(?m)>>\s*1(\.0+)?\s*$/ does not match `player.hasperk 0A001013 Command: player.hasperk 0A001013 Perk Rank >> 2`) | - |
| V-S-AVIF | `player.getav KKV40Avif` | KKV40Avif | number 7 +/- 0.5 | player.getav KKV40Avif -> (no line of its own: player.getav KKV40Avif Command: player.getav KKV40Avif GetActorValue: KKV40Avif >> 7.00; again in 2000 ms); player.getav KKV40Avif -> (no line of its own: player.getav KKV40Avif Command: player.getav KKV40Avif GetActorValue: KKV40Avif >> 7.00; again in ... | inconclusive (late failure: `player.getav KKV40Avif`: the console answered `Item 'KKV04Qust' not found for parameter Quest.`, late, in the reply to `getstage KKV04Qust` (R20, R28)) | - |
| V-S-QUST | `getstage KKV04Qust` | KKV04Qust | number 10 +/- 0.5 | (a failure line for row V-S-PERK `player.hasperk 0A001013` arrived in this reply: `Item 'KKV04Qust' not found for parameter Quest.`); (a failure line for row V-S-AVIF `player.getav KKV40Avif` arrived in this reply: `Item 'KKV04Qust' not found for parameter Quest.`); (a failure line for row V-C-QUST ... | inconclusive (`getstage KKV04Qust`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-TMLM | `help "KKV18 Terminal Menu" 4`, `prid 00000014` | KitKraftVerify.esm:001008, KKV18Tmlm, 0A001008 | contains 0A001008 | help "KKV18 Terminal Menu" 4 -> (no line of its own: help "KKV18 Terminal Menu" 4 ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS---------------...; again in 2000 ms); help "KKV18 Terminal Menu" 4 -> (only the echo; again in 4000 ms); help "KKV18 Terminal Menu" 4 -> help "KKV18 Terminal Men... | inconclusive (`help "KKV18 Terminal Menu" 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-PKIN | `help KKV20Pkin 4`, `prid 00000014` | KitKraftVerify.esm:00100A, KKV20Pkin, 0A00100A | contains 0A00100A | help KKV20Pkin 4 -> (no line of its own: help KKV20Pkin 4 usage: help <matchstring> <filter> <form type> filters: 0-all 1-functions, 2-settin...; again in 2000 ms); help KKV20Pkin 4 -> (only the echo; again in 4000 ms); help KKV20Pkin 4 -> help KKV20Pkin 4; prid 00000014 -> prid 00000014 Command: pr... | inconclusive (`help KKV20Pkin 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-MESG | `help "KKV50 Message" 4`, `prid 00000014` | KitKraftVerify.esm:001016, KKV50Mesg, 0A001016 | contains 0A001016 | help "KKV50 Message" 4 -> (only the echo; again in 2000 ms); help "KKV50 Message" 4 -> (only the echo; again in 4000 ms); help "KKV50 Message" 4 -> help "KKV50 Message" 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help "KKV50 Message" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-SOUN | `help KKV51Soun 4`, `prid 00000014` | KitKraftVerify.esm:001017, KKV51Soun, 0A001017 | contains 0A001017 | help KKV51Soun 4 -> (only the echo; again in 2000 ms); help KKV51Soun 4 -> (only the echo; again in 4000 ms); help KKV51Soun 4 -> help KKV51Soun 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV51Soun 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-IRES | `help "KKV52 Resource" 4`, `prid 00000014` | KitKraftVerify.esm:001018, KKV52Ires, 0A001018 | contains 0A001018 | help "KKV52 Resource" 4 -> (only the echo; again in 2000 ms); help "KKV52 Resource" 4 -> (no line of its own: help "KKV52 Resource" 4 usage: help <matchstring> <filter> <form type> filters: 0-all 1-functions, 2...; again in 4000 ms); help "KKV52 Resource" 4 -> help "KKV52 Resource" 4; prid 00000014 ... | inconclusive (`help "KKV52 Resource" 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-TXST | `help KKV53Txst 4`, `prid 00000014` | KitKraftVerify.esm:001019, KKV53Txst, 0A001019 | contains 0A001019 | help KKV53Txst 4 -> (only the echo; again in 2000 ms); help KKV53Txst 4 -> (only the echo; again in 4000 ms); help KKV53Txst 4 -> help KKV53Txst 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV53Txst 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-HAZD | `help "KKV54 Hazard" 4`, `prid 00000014` | KitKraftVerify.esm:00101A, KKV54Hazd, 0A00101A | contains 0A00101A | help "KKV54 Hazard" 4 -> (only the echo; again in 2000 ms); help "KKV54 Hazard" 4 -> (only the echo; again in 4000 ms); help "KKV54 Hazard" 4 -> help "KKV54 Hazard" 4; prid 00000014 -> prid 00000014 Command: prid 00000014; (late) help "KKV54 Hazard" 4 -> HAZD:  (0A00101A) 'KKV54 Hazard' (in the repl... | **pass** | - |
| V-H-LVLN | `help KKV55Lvln 4`, `prid 00000014` | KitKraftVerify.esm:00101B, KKV55Lvln, 0A00101B | contains 0A00101B | help KKV55Lvln 4 -> (only the echo; again in 2000 ms); help KKV55Lvln 4 -> (only the echo; again in 4000 ms); help KKV55Lvln 4 -> help KKV55Lvln 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV55Lvln 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-LVLP | `help KKV56Lvlp 4`, `prid 00000014` | KitKraftVerify.esm:00101C, KKV56Lvlp, 0A00101C | contains 0A00101C | help KKV56Lvlp 4 -> (only the echo; again in 2000 ms); help KKV56Lvlp 4 -> (only the echo; again in 4000 ms); help KKV56Lvlp 4 -> help KKV56Lvlp 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV56Lvlp 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-PACK | `help KKV57Pack 4`, `prid 00000014` | KitKraftVerify.esm:00101D, KKV57Pack, 0A00101D | contains 0A00101D | help KKV57Pack 4 -> (only the echo; again in 2000 ms); (a line for row V-H-HAZD `help "KKV54 Hazard" 4` re-queued to it: HAZD:  (0A00101A) 'KKV54 Hazard'); help KKV57Pack 4 -> (no line of its own: help KKV57Pack 4 HAZD:  (0A00101A) 'KKV54 Hazard'; again in 4000 ms); help KKV57Pack 4 -> help KKV57Pac... | inconclusive (`help KKV57Pack 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-RSPJ | `help "KKV58 Research" 4`, `prid 00000014` | KitKraftVerify.esm:00101E, KKV58Rspj, 0A00101E | contains 0A00101E | help "KKV58 Research" 4 -> (only the echo; again in 2000 ms); help "KKV58 Research" 4 -> (only the echo; again in 4000 ms); help "KKV58 Research" 4 -> help "KKV58 Research" 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help "KKV58 Research" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-WKMF | `help KKV59Wkmf 4`, `prid 00000014` | KitKraftVerify.esm:00101F, KKV59Wkmf, 0A00101F | contains 0A00101F | help KKV59Wkmf 4 -> (only the echo; again in 2000 ms); help KKV59Wkmf 4 -> (only the echo; again in 4000 ms); help KKV59Wkmf 4 -> help KKV59Wkmf 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV59Wkmf 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-WWED | `help KKV60Wwed 4`, `prid 00000014` | KitKraftVerify.esm:001020, KKV60Wwed, 0A001020 | contains 0A001020 | help KKV60Wwed 4 -> (only the echo; again in 2000 ms); help KKV60Wwed 4 -> (only the echo; again in 4000 ms); help KKV60Wwed 4 -> help KKV60Wwed 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV60Wwed 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-ENCH | `help "KKV61 Enchantment" 4`, `prid 00000014` | KitKraftVerify.esm:001021, KKV61Ench, 0A001021 | contains 0A001021 | help "KKV61 Enchantment" 4 -> (only the echo; again in 2000 ms); help "KKV61 Enchantment" 4 -> (only the echo; again in 4000 ms); help "KKV61 Enchantment" 4 -> help "KKV61 Enchantment" 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help "KKV61 Enchantment" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-COBJ | `help KKV62Cobj 4`, `prid 00000014` | KitKraftVerify.esm:001022, KKV62Cobj, 0A001022 | contains 0A001022 | help KKV62Cobj 4 -> (only the echo; again in 2000 ms); help KKV62Cobj 4 -> (only the echo; again in 4000 ms); help KKV62Cobj 4 -> help KKV62Cobj 4; prid 00000014 -> prid 00000014 Command: prid 00000014 usage: help <matchstring> <filter> <form type> filters: 0-all 1-functions, 2-settin... | inconclusive (`help KKV62Cobj 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-REGN | `help KKV63Regn 4`, `prid 00000014` | KitKraftVerify.esm:001023, KKV63Regn, 0A001023 | contains 0A001023 | help KKV63Regn 4 -> (only the echo; again in 2000 ms); help KKV63Regn 4 -> (only the echo; again in 4000 ms); help KKV63Regn 4 -> help KKV63Regn 4 usage: help <matchstring> <filter> <form type> filters: 0-all 1-functions, 2-settings, 3-globals, 4-oth...; prid 00000014 -> prid 00000014 Command: prid ... | inconclusive (`help KKV63Regn 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-PCBN | `help KKV64Pcbn 4`, `prid 00000014` | KitKraftVerify.esm:001024, KKV64Pcbn, 0A001024 | contains 0A001024 | help KKV64Pcbn 4 -> (only the echo; again in 2000 ms); help KKV64Pcbn 4 -> (only the echo; again in 4000 ms); help KKV64Pcbn 4 -> help KKV64Pcbn 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV64Pcbn 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-PCCN | `help KKV65Pccn 4`, `prid 00000014` | KitKraftVerify.esm:001025, KKV65Pccn, 0A001025 | contains 0A001025 | help KKV65Pccn 4 -> (only the echo; again in 2000 ms); help KKV65Pccn 4 -> (only the echo; again in 4000 ms); help KKV65Pccn 4 -> help KKV65Pccn 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV65Pccn 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-SFBK | `help KKV66Sfbk 4`, `prid 00000014` | KitKraftVerify.esm:001026, KKV66Sfbk, 0A001026 | contains 0A001026 | help KKV66Sfbk 4 -> (only the echo; again in 2000 ms); help KKV66Sfbk 4 -> (only the echo; again in 4000 ms); help KKV66Sfbk 4 -> help KKV66Sfbk 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV66Sfbk 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-SFPT | `help KKV67Sfpt 4`, `prid 00000014` | KitKraftVerify.esm:001027, KKV67Sfpt, 0A001027 | contains 0A001027 | help KKV67Sfpt 4 -> (only the echo; again in 2000 ms); help KKV67Sfpt 4 -> (only the echo; again in 4000 ms); help KKV67Sfpt 4 -> help KKV67Sfpt 4 usage: help <matchstring> <filter> <form type> filters: 0-all 1-functions, 2-settings, 3-globals, 4-oth...; prid 00000014 -> prid 00000014 Command: prid ... | inconclusive (`help KKV67Sfpt 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-SFTR | `help KKV68Sftr 4`, `prid 00000014` | KitKraftVerify.esm:001028, KKV68Sftr, 0A001028 | contains 0A001028 | help KKV68Sftr 4 -> (only the echo; again in 2000 ms); help KKV68Sftr 4 -> (only the echo; again in 4000 ms); help KKV68Sftr 4 -> help KKV68Sftr 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV68Sftr 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-OTFT | `help KKV43Otft 4`, `prid 00000014` | KitKraftVerify.esm:001015, KKV43Otft, 0A001015 | contains 0A001015 | help KKV43Otft 4 -> (only the echo; again in 2000 ms); help KKV43Otft 4 -> (only the echo; again in 4000 ms); help KKV43Otft 4 -> help KKV43Otft 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV43Otft 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-KYWD | `help KKV42Kywd 4`, `prid 00000014` | KitKraftVerify.esm:001014, KKV42Kywd, 0A001014 | contains 0A001014 | help KKV42Kywd 4 -> (only the echo; again in 2000 ms); help KKV42Kywd 4 -> (only the echo; again in 4000 ms); help KKV42Kywd 4 -> help KKV42Kywd 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV42Kywd 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-GLOB | `help KKV01Glob 4`, `prid 00000014` | KitKraftVerify.esm:001029, KKV01Glob, 0A001029 | contains 0A001029 | help KKV01Glob 4 -> (only the echo; again in 2000 ms); help KKV01Glob 4 -> (only the echo; again in 4000 ms); help KKV01Glob 4 -> help KKV01Glob 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV01Glob 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-FLST | `help KKV02Flst 4`, `prid 00000014` | KitKraftVerify.esm:00102A, KKV02Flst, 0A00102A | contains 0A00102A | help KKV02Flst 4 -> (only the echo; again in 2000 ms); help KKV02Flst 4 -> (only the echo; again in 4000 ms); help KKV02Flst 4 -> help KKV02Flst 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV02Flst 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-FACT | `help "KKV03 Faction" 4`, `prid 00000014` | KitKraftVerify.esm:00102B, KKV03Fact, 0A00102B | contains 0A00102B | help "KKV03 Faction" 4 -> (only the echo; again in 2000 ms); help "KKV03 Faction" 4 -> (only the echo; again in 4000 ms); help "KKV03 Faction" 4 -> help "KKV03 Faction" 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help "KKV03 Faction" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-QUST | `help "KKV04 Quest" 4`, `prid 00000014` | KitKraftVerify.esm:00102C, KKV04Qust, 0A00102C | contains 0A00102C | help "KKV04 Quest" 4 -> (only the echo; again in 2000 ms); help "KKV04 Quest" 4 -> (only the echo; again in 4000 ms); help "KKV04 Quest" 4 -> help "KKV04 Quest" 4; prid 00000014 -> prid 00000014 Command: prid 00000014 usage: help <matchstring> <filter> <form type> filters: 0-all 1-functions, 2-setti... | inconclusive (`help "KKV04 Quest" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-LCTN | `help "KKV05 Location" 4`, `prid 00000014` | KitKraftVerify.esm:00102D, KKV05Lctn, 0A00102D | contains 0A00102D | help "KKV05 Location" 4 -> (only the echo; again in 2000 ms); help "KKV05 Location" 4 -> (only the echo; again in 4000 ms); help "KKV05 Location" 4 -> help "KKV05 Location" 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help "KKV05 Location" 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-AVIF | `help KKV40Avif 4`, `prid 00000014` | KitKraftVerify.esm:001012, KKV40Avif, 0A001012 | contains 0A001012 | help KKV40Avif 4 -> (only the echo; again in 2000 ms); help KKV40Avif 4 -> (only the echo; again in 4000 ms); help KKV40Avif 4 -> help KKV40Avif 4; prid 00000014 -> prid 00000014 Command: prid 00000014 | inconclusive (`help KKV40Avif 4` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-PERK | `help "KKV41 Perk" 4`, `prid 00000014` | KitKraftVerify.esm:001013, KKV41Perk, 0A001013 | contains 0A001013 | help "KKV41 Perk" 4 -> (only the echo; again in 2000 ms); help "KKV41 Perk" 4 -> (no line of its own: help "KKV41 Perk" 4 usage: help <matchstring> <filter> <form type> filters: 0-all 1-functions, 2-set...; again in 4000 ms); help "KKV41 Perk" 4 -> help "KKV41 Perk" 4; prid 00000014 -> prid 00000014... | inconclusive (`help "KKV41 Perk" 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (d5372bd7813ed0f50a868b06fd2d87dd4953e2da4d43c05eb1da3531ab06622b -> bf7b351f15b7a7ffed7612f1f0808446b4952af140bdf7665f9312402bcbdb3e): restored from C:\Modding\kitkraft-observe\2026-11-16-T03\backup\01-plugins.txt

Writer's reading (bench 2026-11-16, block 2, verification mod attempt 1 of 3; agent-run
unattended on the owner's go). **69 pass, 3 fail, 34 inconclusive, 0 not-run; no crash, no
timeout.** `report.json` SHA-256 `d5326e0e473a70e954fbf711333c2060e27cd33cac27e9d9f25a62cb2ec1b47b`.
The evidence is entered from the corrected re-run `game-20261116-04`; this entry agrees with it
on every row both graded.

- Blocks A, B, C (placed references, the reporter's 27 Papyrus lines, inventory) all pass.
  The reporter ran on each fresh load of the cell (log 03:20:06 and 03:20:56). V-P29
  `KKV-P29 PKIN base-is=true` and V-P98 `base=[Form < (0A00100A)>]` answer KK-250's question
  in favour of the pack-in. The reporter prints `true`, not `True`; the matcher passed it.
- The 3 fails and 5 of the inconclusives are **command shapes**, not the plugin:
  `getglobalvalue KKV01Glob` and `getstage KKV04Qust` answered `Item '...' not found for
  parameter Global.` / `Quest.` (the console resolves no GLOB or QUST EditorID; the runtime ID
  works, `game-20261116-04`); `player.getav KKV40Avif` answered `GetActorValue: KKV40Avif >>
  7.00` three times, but the `number` matcher read the `40` of the EditorID and the
  correlator then routed `getstage`'s failure line to the waiting rows (V-C-PERK, V-C-AVIF,
  V-C-QUST, V-S-AVIF `inconclusive (late failure)`); `player.hasperk` answers `Perk Rank >> 1`
  (V-S-PERK failed on `Perk Rank >> 2`, the ONE regex).
- Block G: 29 of 30 `help <term> 4` rows inconclusive. Without a form type the game printed
  the answer about 20 s later or not at all (HAZD's line arrived 21 s late, in PACK's row,
  and passed V-H-HAZD); the `usage:` header lines appeared in unrelated replies.
- V-S-01 passed (the reload happened). Save/reload rows: see `game-20261116-04`'s reading on
  which of them count.

### game-20261116-04: agent session 2026-11-16-T04 (`kit game session`), KitKraftVerify.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-16-T04`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-16 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261116-04` |
| Plugins | `KitKraftVerify.esm` SHA-256 `a9632f11d0ad0ade56170fa424c48d5f5a29bb8475c8977e8870a0289be39faa` |
| Probes | `out\verify\probes2.toml` SHA-256 `cb31c941ed966dba5731df31c296df9fc7ea1f64be1ec93f84c43ec15c0fccd4` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-16-T04` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| V-01 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| V-20 | `prid 0A00102F`, `getisid 0A001000` | KitKraftVerify.esm:00102F, KKV70Reporter, 0A00102F, KKV10Acti, 0A001000 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A00102F -> prid 0A00102F Command: prid 0A00102F; getisid 0A001000 -> getisid 0A001000 Command: getisid 0A001000 GetIsID >> 1.00 | **pass** | - |
| V-21 | `prid 0A001030`, `getisid 0A001001` | KitKraftVerify.esm:001030, KKV71StatRef, 0A001030, KKV11Stat, 0A001001 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001030 -> prid 0A001030 Command: prid 0A001030; getisid 0A001001 -> getisid 0A001001 Command: getisid 0A001001 GetIsID >> 1.00 | **pass** | - |
| V-22 | `prid 0A001031`, `getisid 0A001002` | KitKraftVerify.esm:001031, KKV72MsttRef, 0A001031, KKV12Mstt, 0A001002 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001031 -> prid 0A001031 Command: prid 0A001031; getisid 0A001002 -> getisid 0A001002 Command: getisid 0A001002 GetIsID >> 1.00 | **pass** | - |
| V-23 | `prid 0A001032`, `getisid 0A001003` | KitKraftVerify.esm:001032, KKV73FurnRef, 0A001032, KKV13Furn, 0A001003 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001032 -> prid 0A001032 Command: prid 0A001032; getisid 0A001003 -> getisid 0A001003 Command: getisid 0A001003 GetIsID >> 1.00 | **pass** | - |
| V-24 | `prid 0A001033`, `getisid 0A001004` | KitKraftVerify.esm:001033, KKV74FlorRef, 0A001033, KKV14Flor, 0A001004 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001033 -> prid 0A001033 Command: prid 0A001033; getisid 0A001004 -> getisid 0A001004 Command: getisid 0A001004 GetIsID >> 1.00 | **pass** | - |
| V-25 | `prid 0A001034`, `getisid 0A001005` | KitKraftVerify.esm:001034, KKV75DoorRef, 0A001034, KKV15Door, 0A001005 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001034 -> prid 0A001034 Command: prid 0A001034; getisid 0A001005 -> getisid 0A001005 Command: getisid 0A001005 GetIsID >> 1.00 | **pass** | - |
| V-26 | `prid 0A001035`, `getisid 0A001006` | KitKraftVerify.esm:001035, KKV76LighRef, 0A001035, KKV16Ligh, 0A001006 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001035 -> prid 0A001035 Command: prid 0A001035; getisid 0A001006 -> getisid 0A001006 Command: getisid 0A001006 GetIsID >> 1.00 | **pass** | - |
| V-27 | `prid 0A001036`, `getisid 0A001007` | KitKraftVerify.esm:001036, KKV77ContRef, 0A001036, KKV17Cont, 0A001007 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001036 -> prid 0A001036 Command: prid 0A001036; getisid 0A001007 -> getisid 0A001007 Command: getisid 0A001007 GetIsID >> 1.00 | **pass** | - |
| V-28 | `prid 0A001037`, `getisid 0A001009` | KitKraftVerify.esm:001037, KKV78TermRef, 0A001037, KKV19Term, 0A001009 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001037 -> prid 0A001037 Command: prid 0A001037; getisid 0A001009 -> getisid 0A001009 Command: getisid 0A001009 GetIsID >> 1.00 | **pass** | - |
| V-30 | `prid 0A001039`, `getisid 0A00100C` | KitKraftVerify.esm:001039, KKV80MiscRef, 0A001039, KKV30Misc, 0A00100C | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001039 -> prid 0A001039 Command: prid 0A001039; getisid 0A00100C -> getisid 0A00100C Command: getisid 0A00100C GetIsID >> 1.00 | **pass** | - |
| V-31 | `prid 0A00103A`, `getisid 0A00100B` | KitKraftVerify.esm:00103A, KKV81NpcRef, 0A00103A, KKV21Npc, 0A00100B | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A00103A -> prid 0A00103A Command: prid 0A00103A; getisid 0A00100B -> getisid 0A00100B Command: getisid 0A00100B GetIsID >> 1.00 | **pass** | - |
| V-40 | `coc CityNewAtlantisLodgeInt`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisLodgeInt, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpa... | **pass** | - |
| V-P00 | Papyrus log: KKV-P00 reporter loaded (ACTI VMAD) | KKV70Reporter, 0A00102F | papyrus-log KKV-P00 reporter loaded | - | **pass** | - |
| V-P01 | Papyrus log: KKV-P01 GLOB value=4242 (GLOB) | KKV70Reporter, 0A00102F | papyrus-log KKV-P01 GLOB value=4242 | - | **pass** | - |
| V-P02 | Papyrus log: KKV-P02 FLST size=2 (FLST) | KKV70Reporter, 0A00102F | papyrus-log KKV-P02 FLST size=2 | - | **pass** | - |
| V-P03 | Papyrus log: KKV-P03 FLST has-mug=True has-wrench=True (FLST) | KKV70Reporter, 0A00102F | papyrus-log KKV-P03 FLST has-mug=True has-wrench=True | - | **pass** | - |
| V-P04 | Papyrus log: KKV-P04 MISC gold=777 (MISC) | KKV70Reporter, 0A00102F | papyrus-log KKV-P04 MISC gold=777 | - | **pass** | - |
| V-P05 | Papyrus log: KKV-P05 KYWD on-reporter=True (KYWD) | KKV70Reporter, 0A00102F | papyrus-log KKV-P05 KYWD on-reporter=True | - | **pass** | - |
| V-P06 | Papyrus log: KKV-P06 FACT player-in=True (FACT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P06 FACT player-in=True | - | **pass** | - |
| V-P07 | Papyrus log: KKV-P07 PERK player-has=True (PERK) | KKV70Reporter, 0A00102F | papyrus-log KKV-P07 PERK player-has=True | - | **pass** | - |
| V-P08 | Papyrus log: KKV-P08 AVIF player-value=7 (AVIF) | KKV70Reporter, 0A00102F | papyrus-log KKV-P08 AVIF player-value=7 | - | **pass** | - |
| V-P09 | Papyrus log: KKV-P09 QUST running=True stage10-done=True (QUST) | KKV70Reporter, 0A00102F | papyrus-log KKV-P09 QUST running=True stage10-done=True | - | **pass** | - |
| V-P10 | Papyrus log: KKV-P10 LCTN reporter-in=True (LCTN) | KKV70Reporter, 0A00102F | papyrus-log KKV-P10 LCTN reporter-in=True | - | **pass** | - |
| V-P11 | Papyrus log: KKV-P11 CONT mugs=3 (CONT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P11 CONT mugs=3 | - | **pass** | - |
| V-P12 | Papyrus log: KKV-P12 LVLI wrench-added=1 (LVLI) | KKV70Reporter, 0A00102F | papyrus-log KKV-P12 LVLI wrench-added=1 | - | **pass** | - |
| V-P20 | Papyrus log: KKV-P20 ACTI base-is=True (ACTI) | KKV70Reporter, 0A00102F | papyrus-log KKV-P20 ACTI base-is=True | - | **pass** | - |
| V-P21 | Papyrus log: KKV-P21 STAT base-is=True (STAT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P21 STAT base-is=True | - | **pass** | - |
| V-P22 | Papyrus log: KKV-P22 MSTT base-is=True (MSTT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P22 MSTT base-is=True | - | **pass** | - |
| V-P23 | Papyrus log: KKV-P23 FURN base-is=True (FURN) | KKV70Reporter, 0A00102F | papyrus-log KKV-P23 FURN base-is=True | - | **pass** | - |
| V-P24 | Papyrus log: KKV-P24 FLOR base-is=True (FLOR) | KKV70Reporter, 0A00102F | papyrus-log KKV-P24 FLOR base-is=True | - | **pass** | - |
| V-P25 | Papyrus log: KKV-P25 DOOR base-is=True (DOOR) | KKV70Reporter, 0A00102F | papyrus-log KKV-P25 DOOR base-is=True | - | **pass** | - |
| V-P26 | Papyrus log: KKV-P26 LIGH base-is=True (LIGH) | KKV70Reporter, 0A00102F | papyrus-log KKV-P26 LIGH base-is=True | - | **pass** | - |
| V-P27 | Papyrus log: KKV-P27 CONT base-is=True (CONT) | KKV70Reporter, 0A00102F | papyrus-log KKV-P27 CONT base-is=True | - | **pass** | - |
| V-P28 | Papyrus log: KKV-P28 TERM base-is=True (TERM) | KKV70Reporter, 0A00102F | papyrus-log KKV-P28 TERM base-is=True | - | **pass** | - |
| V-P30 | Papyrus log: KKV-P30 MISC base-is=True (MISC) | KKV70Reporter, 0A00102F | papyrus-log KKV-P30 MISC base-is=True | - | **pass** | - |
| V-P31 | Papyrus log: KKV-P31 NPC_ base-is=True (NPC_) | KKV70Reporter, 0A00102F | papyrus-log KKV-P31 NPC_ base-is=True | - | **pass** | - |
| V-P29 | Papyrus log: KKV-P29 PKIN base-is=True (PKIN) | KKV70Reporter, 0A00102F | papyrus-log KKV-P29 PKIN base-is=True | - | **pass** | - |
| V-P98 | Papyrus log: KKV-P98 PKIN reference base= (PKIN (diagnostic)) | KKV70Reporter, 0A00102F | papyrus-log KKV-P98 PKIN reference base= | - | **pass** | - |
| V-P99 | Papyrus log: KKV-P99 done (-) | KKV70Reporter, 0A00102F | papyrus-log KKV-P99 done | - | **pass** | - |
| V-I-MISC | `player.additem 0A00100C 1`, `player.getitemcount 0A00100C` | KitKraftVerify.esm:00100C, KKV30Misc, 0A00100C | regex >>\s*[1-9] | player.additem 0A00100C 1 -> player.additem 0A00100C 1 Command: player.additem 0A00100C 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A00100C -> player.getitemcount 0A00100C Command: player.getitemcount 0A00100C GetItemCount >> 1.00 | **pass** | - |
| V-I-ALCH | `player.additem 0A00100E 1`, `player.getitemcount 0A00100E` | KitKraftVerify.esm:00100E, KKV32Alch, 0A00100E | regex >>\s*[1-9] | player.additem 0A00100E 1 -> player.additem 0A00100E 1 Command: player.additem 0A00100E 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A00100E -> player.getitemcount 0A00100E Command: player.getitemcount 0A00100E GetItemCount >> 1.00 | **pass** | - |
| V-I-BOOK | `player.additem 0A00100F 1`, `player.getitemcount 0A00100F` | KitKraftVerify.esm:00100F, KKV33Book, 0A00100F | regex >>\s*[1-9] | player.additem 0A00100F 1 -> player.additem 0A00100F 1 Command: player.additem 0A00100F 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A00100F -> player.getitemcount 0A00100F Command: player.getitemcount 0A00100F GetItemCount >> 1.00 | **pass** | - |
| V-I-KEYM | `player.additem 0A001010 1`, `player.getitemcount 0A001010` | KitKraftVerify.esm:001010, KKV34Keym, 0A001010 | regex >>\s*[1-9] | player.additem 0A001010 1 -> player.additem 0A001010 1 Command: player.additem 0A001010 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A001010 -> player.getitemcount 0A001010 Command: player.getitemcount 0A001010 GetItemCount >> 1.00 | **pass** | - |
| V-I-ARMO | `player.additem 0A001011 1`, `player.getitemcount 0A001011` | KitKraftVerify.esm:001011, KKV35Armo, 0A001011 | regex >>\s*[1-9] | player.additem 0A001011 1 -> player.additem 0A001011 1 Command: player.additem 0A001011 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A001011 -> player.getitemcount 0A001011 Command: player.getitemcount 0A001011 GetItemCount >> 1.00 | **pass** | - |
| V-I-ARMO-EQ | `player.equipitem 0A001011`, `player.getequipped 0A001011` | KKV35Armo, 0A001011 | regex (?m)>>\s*1(\.0+)?\s*$ | player.equipitem 0A001011 -> player.equipitem 0A001011 Command: player.equipitem 0A001011 GetIsRace HumanRace >> 1.00 IsTrueForConditionForm:  >> 1.0...; player.getequipped 0A001011 -> player.getequipped 0A001011 Command: player.getequipped 0A001011 GetEquipped >> 1.00 | **pass** | - |
| V-I-LVLI | `player.additem 0A00102E 1`, `player.getitemcount 0A00100D` | KKV06Lvli, 0A00102E, KKV31Misc, 0A00100D | regex >>\s*[1-9] | player.additem 0A00102E 1 -> player.additem 0A00102E 1 Command: player.additem 0A00102E 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0A00100D -> player.getitemcount 0A00100D Command: player.getitemcount 0A00100D GetItemCount >> 1.00 | **pass** | - |
| V-C-GLOB | `getglobalvalue 0A001029` | KKV01Glob, 0A001029 | regex (?m)>>\s*4242(\.0+)?\s*$ | getglobalvalue 0A001029 -> getglobalvalue 0A001029 Command: getglobalvalue 0A001029 GetGlobalValue >> 4242.00; (late, after its answer) getglobalvalue 0A001029 -> GetGlobalValue >> 0.00 (in the reply to `coc CityNewAtlantisLodgeInt`; not graded); (late, after its answer) getglobalvalue 0A001029 -> G... | **pass** | - |
| V-C-FACT | `player.getinfaction 0A00102B` | KKV03Fact, 0A00102B | regex (?m)>>\s*1(\.0+)?\s*$ | player.getinfaction 0A00102B -> player.getinfaction 0A00102B Command: player.getinfaction 0A00102B GetInFaction >> 1.00 | **pass** | - |
| V-C-PERK | `player.hasperk 0A001013` | KKV41Perk, 0A001013 | regex Perk Rank >>\s*[1-9] | player.hasperk 0A001013 -> player.hasperk 0A001013 Command: player.hasperk 0A001013 Perk Rank >> 1 | **pass** | - |
| V-C-AVIF | `player.getav KKV40Avif` | KKV40Avif, 0A001012 | regex (?m)>>\s*7(\.0+)?\s*$ | player.getav KKV40Avif -> (no line of its own: player.getav KKV40Avif Command: player.getav KKV40Avif GetActorValue: KKV40Avif >> 7.00; again in 2000 ms); player.getav KKV40Avif -> (no line of its own: player.getav KKV40Avif Command: player.getav KKV40Avif GetActorValue: KKV40Avif >> 7.00; again in ... | inconclusive (`player.getav KKV40Avif`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-C-QUST | `getstage 0A00102C` | KKV04Qust, 0A00102C | regex (?m)>>\s*10(\.0+)?\s*$ | getstage 0A00102C -> getstage 0A00102C Command: getstage 0A00102C GetStage >> 10.00 | **pass** | - |
| V-S-01 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261116-04`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261116-04`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | (a line for row V-C-GLOB `getglobalvalue 0A001029`, which had all its answers: GetGlobalValue >> 0.00; not graded); (a line for row V-C-GLOB `getglobalvalue 0A001029`, which had all its answers: GetGlobalValue >> 0.00; not graded); coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: ... | **pass** | - |
| V-S-02 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | (a line for row V-C-GLOB `getglobalvalue 0A001029`, which had all its answers: GetGlobalValue >> 0.00; not graded); (a line for row V-C-GLOB `getglobalvalue 0A001029`, which had all its answers: GetGlobalValue >> 0.00; not graded); coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartment... | **pass** | - |
| V-S-20 | `prid 0A00102F`, `getisid 0A001000` | KKV70Reporter, 0A00102F, KKV10Acti, 0A001000 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A00102F -> prid 0A00102F Command: prid 0A00102F; getisid 0A001000 -> getisid 0A001000 Command: getisid 0A001000 GetIsID >> 1.00 | **pass** | - |
| V-S-21 | `prid 0A001030`, `getisid 0A001001` | KKV71StatRef, 0A001030, KKV11Stat, 0A001001 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001030 -> prid 0A001030 Command: prid 0A001030; getisid 0A001001 -> getisid 0A001001 Command: getisid 0A001001 GetIsID >> 1.00 | **pass** | - |
| V-S-22 | `prid 0A001031`, `getisid 0A001002` | KKV72MsttRef, 0A001031, KKV12Mstt, 0A001002 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001031 -> prid 0A001031 Command: prid 0A001031; getisid 0A001002 -> getisid 0A001002 Command: getisid 0A001002 GetIsID >> 1.00 | **pass** | - |
| V-S-23 | `prid 0A001032`, `getisid 0A001003` | KKV73FurnRef, 0A001032, KKV13Furn, 0A001003 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001032 -> prid 0A001032 Command: prid 0A001032; getisid 0A001003 -> getisid 0A001003 Command: getisid 0A001003 GetIsID >> 1.00 | **pass** | - |
| V-S-24 | `prid 0A001033`, `getisid 0A001004` | KKV74FlorRef, 0A001033, KKV14Flor, 0A001004 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001033 -> prid 0A001033 Command: prid 0A001033; getisid 0A001004 -> getisid 0A001004 Command: getisid 0A001004 GetIsID >> 1.00 | **pass** | - |
| V-S-25 | `prid 0A001034`, `getisid 0A001005` | KKV75DoorRef, 0A001034, KKV15Door, 0A001005 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001034 -> prid 0A001034 Command: prid 0A001034; getisid 0A001005 -> getisid 0A001005 Command: getisid 0A001005 GetIsID >> 1.00 | **pass** | - |
| V-S-26 | `prid 0A001035`, `getisid 0A001006` | KKV76LighRef, 0A001035, KKV16Ligh, 0A001006 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001035 -> prid 0A001035 Command: prid 0A001035; getisid 0A001006 -> getisid 0A001006 Command: getisid 0A001006 GetIsID >> 1.00 | **pass** | - |
| V-S-27 | `prid 0A001036`, `getisid 0A001007` | KKV77ContRef, 0A001036, KKV17Cont, 0A001007 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001036 -> prid 0A001036 Command: prid 0A001036; getisid 0A001007 -> getisid 0A001007 Command: getisid 0A001007 GetIsID >> 1.00 | **pass** | - |
| V-S-28 | `prid 0A001037`, `getisid 0A001009` | KKV78TermRef, 0A001037, KKV19Term, 0A001009 | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001037 -> prid 0A001037 Command: prid 0A001037; getisid 0A001009 -> getisid 0A001009 Command: getisid 0A001009 GetIsID >> 1.00 | **pass** | - |
| V-S-30 | `prid 0A001039`, `getisid 0A00100C` | KKV80MiscRef, 0A001039, KKV30Misc, 0A00100C | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A001039 -> prid 0A001039 Command: prid 0A001039; getisid 0A00100C -> getisid 0A00100C Command: getisid 0A00100C GetIsID >> 1.00 | **pass** | - |
| V-S-31 | `prid 0A00103A`, `getisid 0A00100B` | KKV81NpcRef, 0A00103A, KKV21Npc, 0A00100B | regex (?m)>>\s*1(\.0+)?\s*$ | prid 0A00103A -> prid 0A00103A Command: prid 0A00103A; getisid 0A00100B -> getisid 0A00100B Command: getisid 0A00100B GetIsID >> 1.00 | **pass** | - |
| V-S-I-MISC | `player.getitemcount 0A00100C` | KKV30Misc, 0A00100C | regex >>\s*[1-9] | player.getitemcount 0A00100C -> player.getitemcount 0A00100C Command: player.getitemcount 0A00100C GetItemCount >> 1.00 | **pass** | - |
| V-S-I-ALCH | `player.getitemcount 0A00100E` | KKV32Alch, 0A00100E | regex >>\s*[1-9] | player.getitemcount 0A00100E -> player.getitemcount 0A00100E Command: player.getitemcount 0A00100E GetItemCount >> 1.00 | **pass** | - |
| V-S-I-BOOK | `player.getitemcount 0A00100F` | KKV33Book, 0A00100F | regex >>\s*[1-9] | player.getitemcount 0A00100F -> player.getitemcount 0A00100F Command: player.getitemcount 0A00100F GetItemCount >> 1.00 | **pass** | - |
| V-S-I-KEYM | `player.getitemcount 0A001010` | KKV34Keym, 0A001010 | regex >>\s*[1-9] | player.getitemcount 0A001010 -> player.getitemcount 0A001010 Command: player.getitemcount 0A001010 GetItemCount >> 1.00 | **pass** | - |
| V-S-I-ARMO | `player.getitemcount 0A001011` | KKV35Armo, 0A001011 | regex >>\s*[1-9] | player.getitemcount 0A001011 -> player.getitemcount 0A001011 Command: player.getitemcount 0A001011 GetItemCount >> 1.00 | **pass** | - |
| V-S-I-LVLI | `player.getitemcount 0A00100D` | KKV31Misc, 0A00100D | regex >>\s*[1-9] | player.getitemcount 0A00100D -> player.getitemcount 0A00100D Command: player.getitemcount 0A00100D GetItemCount >> 1.00 | **pass** | - |
| V-S-GLOB | `getglobalvalue 0A001029` | KKV01Glob | regex (?m)>>\s*4242(\.0+)?\s*$ | getglobalvalue 0A001029 -> getglobalvalue 0A001029 Command: getglobalvalue 0A001029 GetGlobalValue >> 4242.00 | **pass** | - |
| V-S-FACT | `player.getinfaction 0A00102B` | KKV03Fact | regex (?m)>>\s*1(\.0+)?\s*$ | player.getinfaction 0A00102B -> player.getinfaction 0A00102B Command: player.getinfaction 0A00102B GetInFaction >> 1.00 | **pass** | - |
| V-S-PERK | `player.hasperk 0A001013` | KKV41Perk | regex Perk Rank >>\s*[1-9] | player.hasperk 0A001013 -> player.hasperk 0A001013 Command: player.hasperk 0A001013 Perk Rank >> 2 | **pass** | - |
| V-S-AVIF | `player.getav KKV40Avif` | KKV40Avif | regex (?m)>>\s*7(\.0+)?\s*$ | player.getav KKV40Avif -> (no line of its own: player.getav KKV40Avif Command: player.getav KKV40Avif GetActorValue: KKV40Avif >> 7.00; again in 2000 ms); player.getav KKV40Avif -> (no line of its own: player.getav KKV40Avif Command: player.getav KKV40Avif GetActorValue: KKV40Avif >> 7.00; again in ... | inconclusive (`player.getav KKV40Avif`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-S-QUST | `getstage 0A00102C` | KKV04Qust | regex (?m)>>\s*10(\.0+)?\s*$ | getstage 0A00102C -> getstage 0A00102C Command: getstage 0A00102C GetStage >> 10.00 | **pass** | - |
| V-H-TMLM | `help "KKV18 Terminal Menu" 4 TMLM`, `prid 00000014` | KitKraftVerify.esm:001008, KKV18Tmlm, 0A001008 | contains 0A001008 | help "KKV18 Terminal Menu" 4 TMLM -> help "KKV18 Terminal Menu" 4 TMLM ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- TMLM:  (0...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |
| V-H-PKIN | `help KKV20Pkin 4 PKIN`, `prid 00000014` | KitKraftVerify.esm:00100A, KKV20Pkin, 0A00100A | contains 0A00100A | help KKV20Pkin 4 PKIN -> (no line of its own: help KKV20Pkin 4 PKIN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV20Pkin 4 PKIN -> (no line of its own: help KKV20Pkin 4 PKIN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV20Pkin 4 PKIN`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-MESG | `help "KKV50 Message" 4 MESG`, `prid 00000014` | KitKraftVerify.esm:001016, KKV50Mesg, 0A001016 | contains 0A001016 | help "KKV50 Message" 4 MESG -> help "KKV50 Message" 4 MESG ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- MESG:  (0A00101...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |
| V-H-SOUN | `help KKV51Soun 4 SOUN`, `prid 00000014` | KitKraftVerify.esm:001017, KKV51Soun, 0A001017 | contains 0A001017 | help KKV51Soun 4 SOUN -> (no line of its own: help KKV51Soun 4 SOUN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV51Soun 4 SOUN -> (no line of its own: help KKV51Soun 4 SOUN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV51Soun 4 SOUN`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-IRES | `help "KKV52 Resource" 4 IRES`, `prid 00000014` | KitKraftVerify.esm:001018, KKV52Ires, 0A001018 | contains 0A001018 | help "KKV52 Resource" 4 IRES -> (no line of its own: help "KKV52 Resource" 4 IRES ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS---------------...; again in 2000 ms); help "KKV52 Resource" 4 IRES -> (no line of its own: help "KKV52 Resource" 4 IRES ----VIEWER STRINGS NOT AVAILABLE-------- ... | inconclusive (`help "KKV52 Resource" 4 IRES`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-TXST | `help KKV53Txst 4 TXST`, `prid 00000014` | KitKraftVerify.esm:001019, KKV53Txst, 0A001019 | contains 0A001019 | help KKV53Txst 4 TXST -> (no line of its own: help KKV53Txst 4 TXST ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV53Txst 4 TXST -> (no line of its own: help KKV53Txst 4 TXST ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV53Txst 4 TXST`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-HAZD | `help "KKV54 Hazard" 4 HAZD`, `prid 00000014` | KitKraftVerify.esm:00101A, KKV54Hazd, 0A00101A | contains 0A00101A | help "KKV54 Hazard" 4 HAZD -> help "KKV54 Hazard" 4 HAZD ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- HAZD:  (0A00101A...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |
| V-H-LVLN | `help KKV55Lvln 4 LVLN`, `prid 00000014` | KitKraftVerify.esm:00101B, KKV55Lvln, 0A00101B | contains 0A00101B | help KKV55Lvln 4 LVLN -> (no line of its own: help KKV55Lvln 4 LVLN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV55Lvln 4 LVLN -> (no line of its own: help KKV55Lvln 4 LVLN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV55Lvln 4 LVLN`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-LVLP | `help KKV56Lvlp 4 LVLP`, `prid 00000014` | KitKraftVerify.esm:00101C, KKV56Lvlp, 0A00101C | contains 0A00101C | help KKV56Lvlp 4 LVLP -> (no line of its own: help KKV56Lvlp 4 LVLP ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV56Lvlp 4 LVLP -> (no line of its own: help KKV56Lvlp 4 LVLP ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV56Lvlp 4 LVLP`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-PACK | `help KKV57Pack 4 PACK`, `prid 00000014` | KitKraftVerify.esm:00101D, KKV57Pack, 0A00101D | contains 0A00101D | help KKV57Pack 4 PACK -> (no line of its own: help KKV57Pack 4 PACK ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV57Pack 4 PACK -> (no line of its own: help KKV57Pack 4 PACK ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV57Pack 4 PACK`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-RSPJ | `help "KKV58 Research" 4 RSPJ`, `prid 00000014` | KitKraftVerify.esm:00101E, KKV58Rspj, 0A00101E | contains 0A00101E | help "KKV58 Research" 4 RSPJ -> help "KKV58 Research" 4 RSPJ ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- RSPJ:  (0A0010...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |
| V-H-WKMF | `help KKV59Wkmf 4 WKMF`, `prid 00000014` | KitKraftVerify.esm:00101F, KKV59Wkmf, 0A00101F | contains 0A00101F | help KKV59Wkmf 4 WKMF -> (no line of its own: help KKV59Wkmf 4 WKMF ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV59Wkmf 4 WKMF -> (no line of its own: help KKV59Wkmf 4 WKMF ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV59Wkmf 4 WKMF`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-WWED | `help KKV60Wwed 4 WWED`, `prid 00000014` | KitKraftVerify.esm:001020, KKV60Wwed, 0A001020 | contains 0A001020 | help KKV60Wwed 4 WWED -> help KKV60Wwed 4 WWED ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- WWED: KKV60Wwed (0A00...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |
| V-H-ENCH | `help "KKV61 Enchantment" 4 ENCH`, `prid 00000014` | KitKraftVerify.esm:001021, KKV61Ench, 0A001021 | contains 0A001021 | help "KKV61 Enchantment" 4 ENCH -> help "KKV61 Enchantment" 4 ENCH ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- ENCH:  (0A0...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |
| V-H-COBJ | `help KKV62Cobj 4 COBJ`, `prid 00000014` | KitKraftVerify.esm:001022, KKV62Cobj, 0A001022 | contains 0A001022 | help KKV62Cobj 4 COBJ -> (no line of its own: help KKV62Cobj 4 COBJ ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV62Cobj 4 COBJ -> (no line of its own: help KKV62Cobj 4 COBJ ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV62Cobj 4 COBJ`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-REGN | `help KKV63Regn 4 REGN`, `prid 00000014` | KitKraftVerify.esm:001023, KKV63Regn, 0A001023 | contains 0A001023 | help KKV63Regn 4 REGN -> (no line of its own: help KKV63Regn 4 REGN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV63Regn 4 REGN -> (no line of its own: help KKV63Regn 4 REGN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV63Regn 4 REGN`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-PCBN | `help KKV64Pcbn 4 PCBN`, `prid 00000014` | KitKraftVerify.esm:001024, KKV64Pcbn, 0A001024 | contains 0A001024 | help KKV64Pcbn 4 PCBN -> (no line of its own: help KKV64Pcbn 4 PCBN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV64Pcbn 4 PCBN -> (no line of its own: help KKV64Pcbn 4 PCBN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV64Pcbn 4 PCBN`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-PCCN | `help KKV65Pccn 4 PCCN`, `prid 00000014` | KitKraftVerify.esm:001025, KKV65Pccn, 0A001025 | contains 0A001025 | help KKV65Pccn 4 PCCN -> (no line of its own: help KKV65Pccn 4 PCCN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV65Pccn 4 PCCN -> (no line of its own: help KKV65Pccn 4 PCCN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV65Pccn 4 PCCN`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-SFBK | `help KKV66Sfbk 4 SFBK`, `prid 00000014` | KitKraftVerify.esm:001026, KKV66Sfbk, 0A001026 | contains 0A001026 | help KKV66Sfbk 4 SFBK -> (no line of its own: help KKV66Sfbk 4 SFBK ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV66Sfbk 4 SFBK -> (no line of its own: help KKV66Sfbk 4 SFBK ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV66Sfbk 4 SFBK`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-SFPT | `help KKV67Sfpt 4 SFPT`, `prid 00000014` | KitKraftVerify.esm:001027, KKV67Sfpt, 0A001027 | contains 0A001027 | help KKV67Sfpt 4 SFPT -> (no line of its own: help KKV67Sfpt 4 SFPT ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV67Sfpt 4 SFPT -> (no line of its own: help KKV67Sfpt 4 SFPT ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV67Sfpt 4 SFPT`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-SFTR | `help KKV68Sftr 4 SFTR`, `prid 00000014` | KitKraftVerify.esm:001028, KKV68Sftr, 0A001028 | contains 0A001028 | help KKV68Sftr 4 SFTR -> (no line of its own: help KKV68Sftr 4 SFTR ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV68Sftr 4 SFTR -> (no line of its own: help KKV68Sftr 4 SFTR ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV68Sftr 4 SFTR`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-OTFT | `help KKV43Otft 4 OTFT`, `prid 00000014` | KitKraftVerify.esm:001015, KKV43Otft, 0A001015 | contains 0A001015 | help KKV43Otft 4 OTFT -> (no line of its own: help KKV43Otft 4 OTFT ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV43Otft 4 OTFT -> (no line of its own: help KKV43Otft 4 OTFT ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV43Otft 4 OTFT`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-KYWD | `help KKV42Kywd 4 KYWD`, `prid 00000014` | KitKraftVerify.esm:001014, KKV42Kywd, 0A001014 | contains 0A001014 | help KKV42Kywd 4 KYWD -> help KKV42Kywd 4 KYWD ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- KYWD: KKV42Kywd (0A00...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |
| V-H-GLOB | `help KKV01Glob 4 GLOB`, `prid 00000014` | KitKraftVerify.esm:001029, KKV01Glob, 0A001029 | contains 0A001029 | help KKV01Glob 4 GLOB -> (no line of its own: help KKV01Glob 4 GLOB ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV01Glob 4 GLOB -> (no line of its own: help KKV01Glob 4 GLOB ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV01Glob 4 GLOB`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-FLST | `help KKV02Flst 4 FLST`, `prid 00000014` | KitKraftVerify.esm:00102A, KKV02Flst, 0A00102A | contains 0A00102A | help KKV02Flst 4 FLST -> (no line of its own: help KKV02Flst 4 FLST ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- u...; again in 2000 ms); help KKV02Flst 4 FLST -> (no line of its own: help KKV02Flst 4 FLST ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------... | inconclusive (`help KKV02Flst 4 FLST`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-FACT | `help "KKV03 Faction" 4 FACT`, `prid 00000014` | KitKraftVerify.esm:00102B, KKV03Fact, 0A00102B | contains 0A00102B | help "KKV03 Faction" 4 FACT -> help "KKV03 Faction" 4 FACT ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- FACT:  (0A00102...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |
| V-H-QUST | `help "KKV04 Quest" 4 QUST`, `prid 00000014` | KitKraftVerify.esm:00102C, KKV04Qust, 0A00102C | contains 0A00102C | help "KKV04 Quest" 4 QUST -> (no line of its own: help "KKV04 Quest" 4 QUST ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS------------------...; again in 2000 ms); help "KKV04 Quest" 4 QUST -> (no line of its own: help "KKV04 Quest" 4 QUST ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER... | inconclusive (`help "KKV04 Quest" 4 QUST`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-LCTN | `help "KKV05 Location" 4 LCTN`, `prid 00000014` | KitKraftVerify.esm:00102D, KKV05Lctn, 0A00102D | contains 0A00102D | help "KKV05 Location" 4 LCTN -> (no line of its own: help "KKV05 Location" 4 LCTN ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS---------------...; again in 2000 ms); help "KKV05 Location" 4 LCTN -> (no line of its own: help "KKV05 Location" 4 LCTN ----VIEWER STRINGS NOT AVAILABLE-------- ... | inconclusive (`help "KKV05 Location" 4 LCTN`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| V-H-AVIF | `help KKV40Avif 4 AVIF`, `prid 00000014` | KitKraftVerify.esm:001012, KKV40Avif, 0A001012 | contains 0A001012 | help KKV40Avif 4 AVIF -> help KKV40Avif 4 AVIF ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- AVIF: KKV40Avif (0A00...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |
| V-H-PERK | `help "KKV41 Perk" 4 PERK`, `prid 00000014` | KitKraftVerify.esm:001013, KKV41Perk, 0A001013 | contains 0A001013 | help "KKV41 Perk" 4 PERK -> help "KKV41 Perk" 4 PERK ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- PERK:  (0A001013) ...; prid 00000014 -> prid 00000014 Command: prid 00000014 | **pass** | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (d5372bd7813ed0f50a868b06fd2d87dd4953e2da4d43c05eb1da3531ab06622b -> bf7b351f15b7a7ffed7612f1f0808446b4952af140bdf7665f9312402bcbdb3e): restored from C:\Modding\kitkraft-observe\2026-11-16-T04\backup\01-plugins.txt

Writer's reading (bench 2026-11-16, block 2, attempt 2 of 3: the full 106 rows with the
command shapes corrected in `docs/bench/verify-mod/make_probes.py`). **84 pass, 0 fail, 22
inconclusive, 0 not-run; no crash, no timeout.** `report.json` SHA-256
`81eda5b4ec0d13bfa21ba345b0a480e33adea1abf17233eb4e6ba97bbd68f882`. The evidence entries for
the verification mod in `docs/schema/evidence.toml` cite this entry. Launched 03:26:52,
rows from about 101 s, done in about 6 minutes of rows.

Working shapes (first sent by any session tonight): `getglobalvalue 0A001029` ->
`GetGlobalValue >> 4242.00`; `player.getinfaction 0A00102B` -> `GetInFaction >> 1.00`;
`player.hasperk 0A001013` -> `Perk Rank >> 1` (no decimals); `getstage 0A00102C` -> `GetStage >>
10.00`; `player.getav KKV40Avif` -> `GetActorValue: KKV40Avif >> 7.00` (EditorID accepted);
`player.additem <id> 1` + `player.getitemcount <id>` -> `GetItemCount >> 1.00`;
`player.equipitem <id>` + `player.getequipped <id>` -> `GetEquipped >> 1.00`;
`help <term> 4 <TYPE>` answers in its own reply (`TYPE: <edid> (<id>) '<full>'`, an empty list
when nothing matches).

- **Field checks that passed (26 types):** ACTI, STAT, MSTT, FURN, FLOR, DOOR, LIGH, CONT,
  TERM, MISC, NPC_ and ACHR (getisid on the placed references and the reporter's base checks);
  PKIN (V-P29, V-P98 as in -03); GLOB (V-P01, V-C-GLOB); FLST (V-P02, V-P03); KYWD (V-P05);
  FACT (V-P06, V-C-FACT); PERK (V-P07, V-C-PERK); AVIF (V-P08); QUST (V-P09, V-C-QUST); LCTN
  (V-P10: the added `XLCN` held, so the load order was right); LVLI (V-P12, V-I-LVLI); ALCH,
  BOOK, KEYM, ARMO (additem/getitemcount; ARMO also equipped).
- **Presence only (6 types, entered `inconclusive`, not counted as a pass):** TMLM, MESG,
  HAZD, RSPJ, WWED, ENCH: `help` listed the form under its runtime ID; no field was seen.
- **Inconclusive:** V-C-AVIF and V-S-AVIF: the game answered `7.00` each time, but the
  correlator matches a getter's line by its function name and `getav` is not
  `GetActorValue` (a grader gap; `getactorvalue` is no console function, `game-20261116-05`).
  20 `help` rows (PKIN, SOUN, IRES, TXST, LVLN, LVLP, PACK, WKMF, COBJ, REGN, PCBN, PCCN, SFBK,
  SFPT, SFTR, OTFT, GLOB, FLST, QUST, LCTN): an empty list. Their term is an EditorID the game
  does not keep at runtime (only KYWD, AVIF and WWED EditorIDs were listed) or a FULL name it
  did not index (IRES, QUST, LCTN); nothing about the form is shown either way.
- **Save/reload:** V-S-01 passed (save in the Lodge, coc away, load, back in the Lodge).
  After it getisid still named each cloned base (V-S-20 to V-S-31) and the five items added
  before the save were still held (V-S-I-*): entered for ACTI, STAT, MSTT, FURN, FLOR, DOOR,
  LIGH, CONT, TERM, MISC, NPC_, ACHR, ALCH, BOOK, KEYM, ARMO. V-S-GLOB, -FACT, -PERK and -QUST
  passed as graded but **do not show persistence**: V-S-02's `coc` reloaded the test cell and
  the reporter's OnLoad ran again (Papyrus log 03:30:12, after the `load` at 03:29:50) and
  re-applied the faction, perk (rank 2), actor value and quest stage before those rows read
  them; the global only holds the plugin's own value. Not entered.
- Profile: MO2's known `plugins.txt` rewrite at launch, restored by the controller.
- Deploy and list files (SHA-256, first 8): `kit deploy --dev` made `KitKraft Dev -
  KitKraftVerify` (deploy id `20261116T081741Z-dc62882c`); `deploy enable` appended the plugin
  last, after `KitKraftFirstPOI.esm` and `OldSovietMarksmanRifle.esm` (slot `0A`). Before
  enable `modlist` `bfa10ad4`, `plugins` `286a54cf`, `loadorder` `46b28b56`; after enable
  `be7fbfcc`, `d5372bd7`, `c3ebe55c` (unchanged by all three sessions); after `deploy
  disable` (08:41 UTC, the bench sheet's block 2 last step; no later block needs the plugin)
  `feb70629`, `facb98ed`, `c3ebe55c`: the mod line stays as `-` and the plugin line stays
  inactive, as disable does. `lockedorder` `9962fbb0` throughout. The mod folder stays deployed.

### game-20261116-05: agent session 2026-11-16-T05 (`kit game session`), KitKraftVerify.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-16-T05`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-16 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261116-05` |
| Plugins | `KitKraftVerify.esm` SHA-256 `a9632f11d0ad0ade56170fa424c48d5f5a29bb8475c8977e8870a0289be39faa` |
| Probes | `out\verify\probes3.toml` SHA-256 `0a91a9583f92a40ca561c93d62bca64106c264c3723c4eb7479b1085861121f7` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-16-T05` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| V-01 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| V-40 | `coc CityNewAtlantisLodgeInt`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisLodgeInt, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpa... | **pass** | - |
| V-C-AVIF | `player.getactorvalue KKV40Avif` | KKV40Avif, 0A001012 | regex (?m)>>\s*7(\.0+)?\s*$ | player.getactorvalue KKV40Avif -> (no line of its own: player.getactorvalue KKV40Avif Command: player.getactorvalue KKV40Avif Unknown variable or function ...; again in 2000 ms); player.getactorvalue KKV40Avif -> (no line of its own: player.getactorvalue KKV40Avif Command: player.getactorvalue KKV40... | inconclusive (`player.getactorvalue KKV40Avif`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (d5372bd7813ed0f50a868b06fd2d87dd4953e2da4d43c05eb1da3531ab06622b -> bf7b351f15b7a7ffed7612f1f0808446b4952af140bdf7665f9312402bcbdb3e): restored from C:\Modding\kitkraft-observe\2026-11-16-T05\backup\01-plugins.txt

Writer's reading (bench 2026-11-16, block 2, attempt 3 of 3: a split file, rows V-01,
V-40 and V-C-AVIF, to try `player.getactorvalue` so the correlator would match
`GetActorValue`). The game answered `Unknown variable or function 'getactorvalue'.` /
`Syntax Error` / `Could not parse this line.` three times: `getactorvalue` is not a Starfield
console function, so `getav` stays the shape. The grader did not read those lines as a
console failure (V-C-AVIF `inconclusive`, not `fail`): `Unknown variable or function` is a
failure shape the controller does not know yet. `report.json` SHA-256
`2b94ab34e66b5af75b2c1db2fee57878745ad2a8c214bb46ce7f42103f0f4f15`. No evidence entered.

### game-20261116-06: agent session 2026-11-16-T06 (`kit game session`), OldSovietMarksmanRifle.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-16-T06`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-16 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261116-06` |
| Plugins | `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` |
| Probes | `C:\Modding\kitkraft-worktrees\bench-ck\out\rename-plan\probes.toml` SHA-256 `d84e011aab7bed95fb67a99e2d0e17eb1ad3debbe34429d5726afb4cfa55dc05` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-16-T06` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `help "Old Soviet Marksman Rifle" 4` | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | contains 0021BBCD | help "Old Soviet Marksman Rifle" 4 -> help "Old Soviet Marksman Rifle" 4 | fail (`0021BBCD` not in `help "Old Soviet Marksman Rifle" 4`) | - |
| T-02 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261116-06`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261116-06`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt; (waited 10s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29; save KitKraftSession-20261116-06 -> save KitKraftSession-20261116-06 Command: save KitKraftSession... | fail (/(?m)>>\s*1(\.0+)?\s*$/ does not match `player.getincell 0006BB29 Command: player.getincell 0006BB29`) | - |
| T-03 | persistence 4: return and look | - | persistence 4: return and look | - | not-run (manual: the plugin places no new reference with a position and a runtime ID to check after the reload) | - |

Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (facb98ed72807eee801dc1d84aadf8698b6811d08c09c68f6866d2c92c8df2bf -> e98f0a7950a816430df61854c64a4dbd184c8f35cbfaac6a752947ab4d1d1d72): restored from C:\Modding\kitkraft-observe\2026-11-16-T06\backup\01-plugins.txt

Writer's reading (bench 2026-11-16, the **input-injection session**, 03:51 to 04:12, agent-run
unattended on the owner's go of that night; brought in from the `bench-ck` worktree on
2026-11-16). `game.stop` wrote this entry as `game-20261116-01` into
`C:\Modding\kitkraft-worktrees\bench-ck\docs\game-observations.md`, a checkout with no
2026-11-16 entries, so its id collided with OW-1's `game-20261116-01` on `main`; it is
renumbered here to the next free id of the day and the copy in `bench-ck` was reverted
(uncommitted there). Session id, session save name and observe folder keep their `T06`.

- **What the session was for:** keeping the game up for the input experiment (bench sheet
  block "input"), not for its probe rows. It was started through a long-lived `kit mcp`
  (`game.session` with `start: true`, dist build `5f889c96` of 2026-11-02, so neither the
  release `kit.exe` of that night nor the R41 controller), kept idle between commands, and
  ended with `game.stop`. Its probes are the rename plugin's `out\rename-plan\probes.toml`
  (`d84e011a...a55dc05`).
- **Rows:** T-01 and T-02 `fail` with replies that hold only the echo, as they fail on that
  build; T-03 `not-run` (manual). **No evidence is
  entered from this entry.** Its rows grade the old controller, not the plugin.
- **Input results** (the reason for the session): `docs/bench/input-results.md`, from
  `C:\Modding\kitkraft-observe\scripts\input\RESULTS-20261116.md` (SHA-256
  `2ad508f1...f9f46`) and `results-20261116.json` (`ef1b0717...861b6f`), per-run reports and
  every sent input under that folder's `runs\` (ADR 0010 Amendment 2's record of sent
  input; the controller's `report.json` holds none until KK-362). Session `report.json`
  `C:\Modding\kitkraft-observe\2026-11-16-T06\report.json` SHA-256
  `2e8e365df15195cbc3c2e3e903e9aa8cffa27e153252f474afc216b89aa809c5`.
- **Side effect:** Tab in the world opened the Data menu and the game wrote an autosave
  (`Autosave1_20261116M546564_000038_20261116090732_1_0_4.sfs`, 04:07) into the dev
  profile's own saves, `C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\saves\`, beside
  `KitKraftBase.sfs`. No owner save was touched and no menu save was made; nothing was
  deleted. `KitKraftBase.sfs` still hashed `fbed2cc2...eb5ccf` (read at 05:00 that morning);
  the next preflight re-hashes it.
- **Profile:** `plugins.txt` was rewritten at launch (MO2's known rewrite) and restored by
  the controller from `C:\Modding\kitkraft-observe\2026-11-16-T06\backup\01-plugins.txt`;
  its "before" hash `facb98ed...` is the state after block 2's `deploy disable`. An MO2
  dialog "waiting on an application to close before exiting" sat over the game for the
  first minutes and went away by itself.

### game-20261117-01: agent session 2026-11-17-T01 (`kit game session`), KitKraftVerify.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-17-T01`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-17 |
| Status | inconclusive (2 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261117-01` |
| Plugins | `KitKraftVerify.esm` SHA-256 `a9632f11d0ad0ade56170fa424c48d5f5a29bb8475c8977e8870a0289be39faa` |
| Probes | `docs\bench\verify-mod\rerun.toml` SHA-256 `4e08e915c0a81ac619142c5db12d1612bc6aacb987be1f731266b77351db6a67` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-17-T01` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| V-01 | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE; (waited 10s); player.getincell 0021966F -> player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00 | **pass** | - |
| V-40 | `coc CityNewAtlantisLodgeInt`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F` | CityNewAtlantisLodgeInt, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpa... | **pass** | - |
| V-P00 | Papyrus log: KKV-P00 reporter loaded (ACTI VMAD) | KKV70Reporter, 0A00102F | papyrus-log KKV-P00 reporter loaded | - | **pass** | - |
| V-C-AVIF | `player.getav KKV40Avif` | KKV40Avif, 0A001012 | regex (?m)>>\s*7(\.0+)?\s*$ | player.getav KKV40Avif -> player.getav KKV40Avif Command: player.getav KKV40Avif GetActorValue: KKV40Avif >> 7.00 | **pass** | - |
| V-H-PKIN | Papyrus log: KKV-H20 PKIN present=True (the PKIN KKV20Pkin is loaded; presence only) | KitKraftVerify.esm:00100A, KKV20Pkin, 0A00100A | papyrus-log KKV-H20 PKIN present=True | - | **pass** | - |
| V-H-OTFT | Papyrus log: KKV-H43 OTFT present=True (the OTFT KKV43Otft is loaded; presence only) | KitKraftVerify.esm:001015, KKV43Otft, 0A001015 | papyrus-log KKV-H43 OTFT present=True | - | **pass** | - |
| V-H-SOUN | Papyrus log: KKV-H51 SOUN present=True (the SOUN KKV51Soun is loaded; presence only) | KitKraftVerify.esm:001017, KKV51Soun, 0A001017 | papyrus-log KKV-H51 SOUN present=True | - | **pass** | - |
| V-H-IRES | Papyrus log: KKV-H52 IRES present=True (the IRES KKV52Ires is loaded; presence only) | KitKraftVerify.esm:001018, KKV52Ires, 0A001018 | papyrus-log KKV-H52 IRES present=True | - | **pass** | - |
| V-H-TXST | Papyrus log: KKV-H53 TXST present=True (the TXST KKV53Txst is loaded; presence only) | KitKraftVerify.esm:001019, KKV53Txst, 0A001019 | papyrus-log KKV-H53 TXST present=True | - | **pass** | - |
| V-H-LVLN | Papyrus log: KKV-H55 LVLN present=True (the LVLN KKV55Lvln is loaded; presence only) | KitKraftVerify.esm:00101B, KKV55Lvln, 0A00101B | papyrus-log KKV-H55 LVLN present=True | - | **pass** | - |
| V-H-LVLP | Papyrus log: KKV-H56 LVLP present=True (the LVLP KKV56Lvlp is loaded; presence only) | KitKraftVerify.esm:00101C, KKV56Lvlp, 0A00101C | papyrus-log KKV-H56 LVLP present=True | - | **pass** | - |
| V-H-PACK | Papyrus log: KKV-H57 PACK present=True (the PACK KKV57Pack is loaded; presence only) | KitKraftVerify.esm:00101D, KKV57Pack, 0A00101D | papyrus-log KKV-H57 PACK present=True | - | **pass** | - |
| V-H-WKMF | Papyrus log: KKV-H59 WKMF present=True (the WKMF KKV59Wkmf is loaded; presence only) | KitKraftVerify.esm:00101F, KKV59Wkmf, 0A00101F | papyrus-log KKV-H59 WKMF present=True | - | **pass** | - |
| V-H-COBJ | Papyrus log: KKV-H62 COBJ present=True (the COBJ KKV62Cobj is loaded; presence only) | KitKraftVerify.esm:001022, KKV62Cobj, 0A001022 | papyrus-log KKV-H62 COBJ present=True | - | **pass** | - |
| V-H-REGN | Papyrus log: KKV-H63 REGN present=True (the REGN KKV63Regn is loaded; presence only) | KitKraftVerify.esm:001023, KKV63Regn, 0A001023 | papyrus-log KKV-H63 REGN present=True | - | **pass** | - |
| V-H-PCBN | Papyrus log: KKV-H64 PCBN present=True (the PCBN KKV64Pcbn is loaded; presence only) | KitKraftVerify.esm:001024, KKV64Pcbn, 0A001024 | papyrus-log KKV-H64 PCBN present=True | - | **pass** | - |
| V-H-PCCN | Papyrus log: KKV-H65 PCCN present=True (the PCCN KKV65Pccn is loaded; presence only) | KitKraftVerify.esm:001025, KKV65Pccn, 0A001025 | papyrus-log KKV-H65 PCCN present=True | - | **pass** | - |
| V-H-SFBK | Papyrus log: KKV-H66 SFBK present=True (the SFBK KKV66Sfbk is loaded; presence only) | KitKraftVerify.esm:001026, KKV66Sfbk, 0A001026 | papyrus-log KKV-H66 SFBK present=True | - | **pass** | - |
| V-H-SFPT | Papyrus log: KKV-H67 SFPT present=True (the SFPT KKV67Sfpt is loaded; presence only) | KitKraftVerify.esm:001027, KKV67Sfpt, 0A001027 | papyrus-log KKV-H67 SFPT present=True | - | **pass** | - |
| V-H-SFTR | Papyrus log: KKV-H68 SFTR present=True (the SFTR KKV68Sftr is loaded; presence only) | KitKraftVerify.esm:001028, KKV68Sftr, 0A001028 | papyrus-log KKV-H68 SFTR present=True | - | **pass** | - |
| V-H-GLOB | Papyrus log: KKV-H01 GLOB present=True (the GLOB KKV01Glob is loaded; presence only) | KitKraftVerify.esm:001029, KKV01Glob, 0A001029 | papyrus-log KKV-H01 GLOB present=True | - | **pass** | - |
| V-H-FLST | Papyrus log: KKV-H02 FLST present=True (the FLST KKV02Flst is loaded; presence only) | KitKraftVerify.esm:00102A, KKV02Flst, 0A00102A | papyrus-log KKV-H02 FLST present=True | - | **pass** | - |
| V-H-QUST | Papyrus log: KKV-H04 QUST present=True (the QUST KKV04Qust is loaded; presence only) | KitKraftVerify.esm:00102C, KKV04Qust, 0A00102C | papyrus-log KKV-H04 QUST present=True | - | **pass** | - |
| V-H-LCTN | Papyrus log: KKV-H05 LCTN present=True (the LCTN KKV05Lctn is loaded; presence only) | KitKraftVerify.esm:00102D, KKV05Lctn, 0A00102D | papyrus-log KKV-H05 LCTN present=True | - | **pass** | - |
| V-P99 | Papyrus log: KKV-P99 done (-) | KKV70Reporter, 0A00102F | papyrus-log KKV-P99 done | - | **pass** | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\modlist.txt changed during the session (be7fbfcc2876fb56eb263d7314992db34ff7610460087af11c03ab2a4ecef882 -> c86da9c82eb997514a15f83c29fe5e8d43df7d38772c1f1d61cbc8e585864271): restored from C:\Modding\kitkraft-observe\2026-11-17-T01\backup\00-modlist.txt
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (d5372bd7813ed0f50a868b06fd2d87dd4953e2da4d43c05eb1da3531ab06622b -> bf7b351f15b7a7ffed7612f1f0808446b4952af140bdf7665f9312402bcbdb3e): restored from C:\Modding\kitkraft-observe\2026-11-17-T01\backup\01-plugins.txt

Writer's reading (bench night 2026-11-17, blocks 0 and 1, agent-run unattended on the owner's
go "I'm going to bed you are go", 2026-11-17 about 00:20; A1 yes, A2 yes, A3 not available,
A4 to A13 settled by ADR 0012 Amendment 3 and ADR 0010 Amendment 3). Release binaries hashed
at 00:24: `kit.exe` `627a8a6d...0ae732`, `kit-ui.exe` `250a5208...6401c2` (from `c32becce`,
not rebuilt). Worktree `bench-night` at `d8c3b1c3`.

- **Block 0 (preflight, 00:24):** `scripts\bench\ow1-preflight.ps1`: `fails: 0`; MO2's
  selected profile `Kitkraft Dev`; base save `fbed2cc2...eb5ccf`; list files before:
  `modlist.txt` `feb70629...0be26d`, `plugins.txt` `facb98ed...8df2bf`, `loadorder.txt`
  `c3ebe55c...c219b4`, `lockedorder.txt` `9962fbb0...52812d`. Builds: `KitKraftVerify.esm`
  `a9632f11...e39faa` (unchanged), new `VerifyReport.pex`
  `d366bdeddafb8bddd0a52802099132ab7df4118e8f59ac6e6d60d722fea2c043`;
  `KitKraftVerifyQuest.esm` `94d2ad19...1dea3f6d`, validate 0 errors 0 warnings, roundtrip
  identical; rename plugin `9c0b28a6...78ea27`. The rename `probes.toml` hashes
  `3aa2d0cf...18ea2f`, not the sheet's `d84e011a...a55dc05`: the difference is KK-360's T-02
  `additem` row (the sheet's hash is the file before KK-360); the plugin hash is unchanged.
  The CK self-tests of block 0 were left to the CK lane.
- **Block 1:** `deploy --dev` replaced one file (the `.pex`), `deploy enable` set the two
  lines back (`KitKraftVerify.esm` last). **25 of 25 rows pass**: V-01, V-40, V-P00 and
  V-P99 (the reporter ran, from 00:28:10 to 00:28:11 in the Papyrus log), V-C-AVIF on
  `GetActorValue: KKV40Avif >> 7.00` (the `getav` correlation), and the 20 V-H rows on the
  reporter's `KKV-H<nn> <TYPE> present=true` lines (`logs\papyrus-Papyrus.0.log`).
  Game time 2 min 21 s. `report.json` SHA-256 `4ec27de9...c31fe5`.
- **Evidence entered** (`docs/schema/evidence.toml`): 15 types new as `game-runtime-tested`
  `inconclusive`, "Presence only" (OTFT, SOUN, IRES, TXST, LVLN, LVLP, PACK, WKMF, COBJ, REGN,
  PCBN, PCCN, SFBK, SFPT, SFTR); AVIF's pass gains this entry (V-C-AVIF now graded). PKIN,
  GLOB, FLST, QUST and LCTN already pass on field checks; their presence rows add nothing.
- **Status `inconclusive`** is the profile check only: MO2 rewrote `modlist.txt` and
  `plugins.txt` at launch, both restored from the session's backups (known, no grade change).
### game-20261117-02: agent session 2026-11-17-T02 (`kit game session`), KitKraftVerifyQuest.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-17-T02`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-17 |
| Status | inconclusive (2 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261117-02` |
| Plugins | `KitKraftVerifyQuest.esm` SHA-256 `94d2ad19feaa10bd8f8ac5da6b80d5f55bb0c2bfe6211cfe666ffb681dea3f6d` |
| Probes | `docs\bench\verify-mod\verify-quest-probes.toml` SHA-256 `a8511c2db0f78388ef36f565a303516f0b46a13887b689e36f4739f7a1c58c21` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-17-T02` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| Q-01 | `prid 00000014` | KKQ01Qust | papyrus-log KKQ-Q01 quest script running | prid 00000014 -> prid 00000014 Command: prid 00000014; (waited 15s) | **pass** | - |
| Q-02 | Papyrus log: KKQ-Q02 the quest property names the placed KKQ70Marker | KKQ01Qust, KKQ70Marker | papyrus-log KKQ-Q02 marker-set=True base-is=True | - | **pass** | - |
| Q-03 | Papyrus log: KKQ-Q03 the stage 10 fragment ran (QUST VMAD fragment block) | KKQ01Qust | papyrus-log KKQ-Q03 stage 10 fragment ran | - | fail (`KKQ-Q03 stage 10 fragment ran` not in `[00:29:24] Papyrus log opened (PC-64) [00:29:24] Update budget: 1.200000ms (Extra tasklet budget: 1.200000ms, Load screen budget: 500.000000ms) [00:29:24] Memor...`) | - |
| Q-04 | Papyrus log: KKQ-Q04 stage 10 done after SetStage | KKQ01Qust | papyrus-log KKQ-Q04 stage10-done=True | - | **pass** | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\modlist.txt changed during the session (2861c6e11f945e646a0377c5623755d26723281714b78b1323a7c8ac25377eef -> d44a4b1f585a3f5eaef1b1450afdc882fbcb394b4f49b978864eda2d245fed7f): restored from C:\Modding\kitkraft-observe\2026-11-17-T02\backup\00-modlist.txt
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (7ed6358c2d1633684bb4d9484896171bdb785d3a370c34c700eea50d97031d1a -> ad0d22e8a668bcb502914d31ead2ed1769c8b1d1fd474623be211368d6f31b47): restored from C:\Modding\kitkraft-observe\2026-11-17-T02\backup\01-plugins.txt

Writer's reading (bench night 2026-11-17, block 2). `deploy --dev` added three files
(`KitKraftVerifyQuest.esm`, `VerifyQuestReport.pex`
`73a7dfce50b7cce548c83b195c618c81054f90c9fbb5bf8bab5a5ec375231a9e`, `QF_KKQ01Qust.pex`
`a8e0380928367b1e69ba4da9b9f3652a48a10c56b8f9a6a073edfd17104ad4f8`); `deploy enable` added
its three lines last, after `KitKraftVerify.esm`.

- **Q-01, Q-02, Q-04 pass**: at 00:30:55 the log holds `KKQ-Q01 quest script running`,
  `KKQ-Q02 marker-set=true base-is=true`, `KKQ-Q04 stage10-done=true`. The quest started on
  the base save's load, ran the quest script kit attached, its object property named the
  crate the same build placed, and `SetStage 10` took.
- **Q-03 fails**: no `KKQ-Q03` line, and no line naming `QF_KKQ01Qust` at all (no bind
  error either) in `logs\papyrus-Papyrus.0.log`. Stage 10 was set (Q-04), so the fragment
  block kit wrote did not run its stage 10 fragment, or the game did not bind the fragment
  script. A real finding against the fragment writer (`kit build` `stages = [{ stage = 10 }]`);
  not re-tried tonight. Next look: the written `VMAD` fragment block beside an official one
  with one stage fragment (`crates/engine/tests/quest_vmad_corpus.rs` checks the format
  only).
- **Evidence:** QUST's existing pass gains this entry's pointer with the Q-03 failure in its
  note; nothing new. Afterwards `deploy disable` turned both verification mods off.
  `report.json` SHA-256 `c447cc23...9487ad`.
### game-20261117-03: agent session 2026-11-17-T03 (`kit game session`), OldSovietMarksmanRifle.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-17-T03`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-17 |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261117-03` |
| Plugins | `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` |
| Probes | `C:\Modding\kitkraft-worktrees\bench-night\out\rename-plan\probes.toml` SHA-256 `3aa2d0cf4a15e783f0e8c34f734d019dfbd5d3d9a46093c50ffe5e0d2918ea2f` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-17-T03` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | help "Old Soviet Marksman Rifle" 4: WEAP `RussianHuntingRifle` | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |
| T-02 | player.additem 0021BBCD 1: WEAP `RussianHuntingRifle` in the inventory | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |
| T-03 | persistence 1-3: leave, save to the session's own save, go elsewhere, load it | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |
| T-04 | persistence 4: return and look | - | - | - | not-run (timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)) | - |

Finding: timeout after 300s waiting for the console endpoint (no main window of Starfield.exe yet; nothing sent)
Finding: diagnostics at the wait before the probes, 312s after the start: running: ModOrganizer.exe; main window of Starfield.exe: no; screenshot: C:\Modding\kitkraft-observe\2026-11-17-T03\screenshots\stuck-1.png
Finding: log papyrus was not written during the session (C:\Users\macke\Documents/My Games/Starfield/Logs/Script/Papyrus.0.log has the length and modified time it had before the launch); recorded as missing
Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\modlist.txt changed during the session (6bed5567b6a78dd628a7f509834ff28409bae2d8a11ad012a876fbab0e0e8d7a -> 11a3be5c7bebc4f76556fb91a0f1dc7b38a35261e94752d578d63aaa3731bc88): restored from C:\Modding\kitkraft-observe\2026-11-17-T03\backup\00-modlist.txt

Writer's reading (bench night 2026-11-17, OW-2 part 1 by the CLI, first attempt). To keep the
game up for the input scenarios the session was started with `game.session` `start: true`
through a long-lived `kit mcp` of the release `kit.exe`, held by a small script
(`hold_session.py`, in the agent's scratch folder, not kept). That script was started from
Git Bash. MO2 came up but showed "Cannot launch program: Cannot start sfse_loader.exe"
(`screenshots\stuck-1.png`), so no game ran; nobody could click the dialog, and the
controller's launch wait gave up at 300 s and ended MO2. Every row `not-run`; nothing was
sent to a game. Cause not proven; the retry from PowerShell with the same script and
arguments (`game-20261117-04`) launched normally, so the Git Bash environment the script
inherited is the likely difference. `report.json` SHA-256 `41c11c91...8fd9e7`.
### game-20261117-04: agent session 2026-11-17-T04 (`kit game session`), OldSovietMarksmanRifle.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-17-T04`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-17 |
| Status | inconclusive (2 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261117-04` |
| Plugins | `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` |
| Probes | `C:\Modding\kitkraft-worktrees\bench-night\out\rename-plan\probes.toml` SHA-256 `3aa2d0cf4a15e783f0e8c34f734d019dfbd5d3d9a46093c50ffe5e0d2918ea2f` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-17-T04` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `help "Old Soviet Marksman Rifle" 4` | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | contains 0021BBCD | help "Old Soviet Marksman Rifle" 4 -> (no line of its own: help "Old Soviet Marksman Rifle" 4 ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS---------...; again in 2000 ms); help "Old Soviet Marksman Rifle" 4 -> (only the echo; again in 4000 ms); help "Old Soviet Marksman Rifle" 4 -> help "... | inconclusive (`help "Old Soviet Marksman Rifle" 4`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| T-02 | `player.additem 0021BBCD 1`, `player.getitemcount 0021BBCD` | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | regex >>\s*[1-9] | player.additem 0021BBCD 1 -> player.additem 0021BBCD 1 Command: player.additem 0021BBCD 1; player.getitemcount 0021BBCD -> (only the echo; again in 2000 ms); player.getitemcount 0021BBCD -> (only the echo; again in 4000 ms); player.getitemcount 0021BBCD -> player.getitemcount 0021BBCD Command: playe... | **pass** | - |
| T-03 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261117-04`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261117-04`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 10s); player.getincell 0006BB29 -> (only the echo; again in 2000 ms); player.getincell 0006BB29 -> (only the echo; again in 4000 ms); pl... | inconclusive (`player.getincell 0006BB29` answered with only its echo, 3 time(s): no correlated reply: no result to judge (a timing anomaly, R20, R28)) | - |
| T-04 | persistence 4: return and look | - | persistence 4: return and look | - | not-run (manual: the plugin places no new reference with a position and a runtime ID to check after the reload) | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\modlist.txt changed during the session (6bed5567b6a78dd628a7f509834ff28409bae2d8a11ad012a876fbab0e0e8d7a -> 11a3be5c7bebc4f76556fb91a0f1dc7b38a35261e94752d578d63aaa3731bc88): restored from C:\Modding\kitkraft-observe\2026-11-17-T04\backup\00-modlist.txt
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (9d5991a1fe06991a33e2503a9215a407e8137dd2cd505347573e13e5db55a289 -> e98f0a7950a816430df61854c64a4dbd184c8f35cbfaac6a752947ab4d1d1d72): restored from C:\Modding\kitkraft-observe\2026-11-17-T04\backup\01-plugins.txt

Writer's reading (bench night 2026-11-17, OW-2 part 1 by the CLI, retry, with the input
scenarios; `game.session` `start: true` through a long-lived `kit mcp` of the release
`kit.exe`, started from PowerShell, ended with `game.stop` at 490 s).

- **Rows:** T-02 (KK-360's `additem` row, its first run against the real game) **pass**:
  `player.additem 0021BBCD 1`, then `GetItemCount >> 1.00` on the third try (two echo-only
  answers first). T-01 `inconclusive` again, also on a `game.probe` re-run with a screenshot
  (`T-01-1.png`): the `help` answer (`----VIEWER STRINGS NOT AVAILABLE----`) came back
  without a line the correlator could tie to the request. T-03 `inconclusive`: every
  `getincell` answered `GetInCell >> 1.00` late and the correlator could not tell the three
  identical requests apart (ambiguous, given to none). T-04 manual, `not-run`.
- **Seen on screen (screenshot evidence, no row):** the weapon workbench's inventory list
  showed "OLD SOVIET MARKSMAN RIFLE", the renamed `RussianHuntingRifle` that T-02 added
  (`C:\Modding\kitkraft-observe\scripts\input\runs\20261117-adhoc-sc4\SC4-retry-after-E-5s.png`).
- **OW-2 window checks not run:** the window was not opened (A3: nobody could click Allow
  on the computer-use dialog). Steps 1.1, 1.2, 1.5 to 1.7 and 1.9 to 1.11 in the window,
  KK-032 to KK-036 and KK-040, part 2 (`KC_F31.esp`, which is made in the window) and
  part 3 stayed `not-run`. What ran by CLI: `deploy` preview (`0 added, 0 replaced,
  1 unchanged`, the "not last" warning) and apply (nothing written), `deploy status`,
  `deploy enable --mods OldSovietMarksmanRifle` to move its lines last (dry run first), and
  this session with a `game.probe` re-run and a screenshot.
- **Input scenarios** (executor `C:\Modding\kitkraft-observe\scripts\input\`, foreground
  guard on, run without `--ask`, screenshots read by the agent; every input in each run's
  `inputs.jsonl`): **SC-4 pass on a retry**: the scenario's aim left the crosshair above
  the bench (`runs\20261117-004233-scenarios`, inconclusive); 300 more counts of pitch down
  showed "Weapon Workbench", and E opened the workbench menu, Tab left it
  (`runs\20261117-adhoc-sc4\`). **SC-2 pass**: `player.moveto 000057BE` (Vasco) showed
  "Talk (E)", E opened his dialogue with four options, E picked the first and he answered
  (`runs\20261117-004401-scenarios`); Tab during his line did nothing, Tab at the options
  closed the dialogue (`runs\20261117-adhoc-sc2\`). **SC-5 pass**: pause menu, Save, a new
  save `Save1_20261117M546564_000030_20261117054544_1_0_4.sfs` (SHA-256
  `eb9486f3...ea3186`) in `C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\saves\`, moved
  to x = -16, loaded it from the menu, x back at -12.99 (expected -13.22 within 0.5)
  (`runs\20261117-004532-scenarios`; its two scripted Tabs after the save went to the menus, and no autosave appeared). Tab was otherwise sent only to leave the workbench and the dialogue.
- **Saves:** the session save `KitKraftSession-20261117-04` and the SC-5 save are in the dev
  profile's `saves\` (allowed by ADR 0010 Amendment 3); nothing was deleted. The owner's
  `Documents\My Games\Starfield\Saves` listing (119 files, names, sizes, write times) is
  identical before and after the night's sessions. `KitKraftBase.sfs` still hashes
  `fbed2cc2...eb5ccf`.
- **After:** no Starfield, SFSE, MO2 or CK process; no `ABORT` file. List files after:
  `modlist.txt` `6bed5567...3bd7a`, `plugins.txt` `9d5991a1...49a289`, `loadorder.txt`
  `22e50236...249ac`, `lockedorder.txt` unchanged; the rename plugin stays enabled and last,
  both verification mods deployed and disabled. `report.json` SHA-256 `4bfffe8d...5c114e`.

### game-20261117-05: agent session 2026-11-17-T05 (`kit game session`), KitKraftVerifyQuest.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-17-T05`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-17 |
| Status | inconclusive (2 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261117-05` |
| Plugins | `KitKraftVerifyQuest.esm` SHA-256 `7192216dabb39280b40a85283442dc63ad22e0c2b1f7823d26fa9de4c2b2f0e5` |
| Probes | `docs\bench\verify-mod\verify-quest-probes.toml` SHA-256 `c59da097804a506bf15a6bab9d91c4597bbb809573ad1424a157dcd67474fcff` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-17-T05` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| Q-01 | `prid 00000014` | KKQ01Qust | papyrus-log KKQ-Q01 quest script running | prid 00000014 -> prid 00000014 Command: prid 00000014; (waited 15s) | **pass** | - |
| Q-02 | Papyrus log: KKQ-Q02 the quest property names the placed KKQ70Marker | KKQ01Qust, KKQ70Marker | papyrus-log KKQ-Q02 marker-set=True base-is=True | - | **pass** | - |
| Q-03 | Papyrus log: KKQ-Q03 the stage 10 fragment ran (QUST VMAD fragment block) | KKQ01Qust | papyrus-log KKQ-Q03 stage 10 fragment ran | - | **pass** | - |
| Q-04 | Papyrus log: KKQ-Q04 stage 10 done after SetStage | KKQ01Qust | papyrus-log KKQ-Q04 stage10-done=True | - | **pass** | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\modlist.txt changed during the session (4b16aa2c1fa8ec6cde2e9386c05fc4de7fd2f45a90c7ed79803c90c1c5e1b03e -> d12954f5fa75b12b1e6bd87747d5d89e122471b94c1c9410e6e5bfebe488eed2): restored from C:\Modding\kitkraft-observe\2026-11-17-T05\backup\00-modlist.txt
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (65bf61161827c8154e5f3e3001df3e938cca8baa64060aa565253bad76b2e39b -> 48daa0205a602f4a1fd189ad2799c21e2d242176279ab96367bb469e9d4582d0): restored from C:\Modding\kitkraft-observe\2026-11-17-T05\backup\01-plugins.txt

Writer's reading (bench night 2026-11-17, second desk window, 02:24 to 02:28; the Q-03
re-test). One variable changed against `game-20261117-02`: stage 10 of `KKQ01Qust` got its
log entry (`QSDT = 0` in the stage's fields of `docs/bench/verify-mod/verify-quest.toml`).
`kit inspect` of the built QUST shows `INDX 0A000000`, then `QSDT 00`, `QSRD 00000000` and
the empty `QSRD` end marker (the reward count and end the schema requires, written as
zeros); before, `INDX` had nothing after it. Plugin 3,261 bytes, SHA-256
`7192216dabb39280b40a85283442dc63ad22e0c2b1f7823d26fa9de4c2b2f0e5` (was `94d2ad19...`);
validate 0 errors 0 warnings, roundtrip identical. Probes file
`verify-quest-probes.toml` changed only in its `sha256` line (file SHA-256
`c59da097804a506bf15a6bab9d91c4597bbb809573ad1424a157dcd67474fcff`). Both `.pex` unchanged
(`73a7dfce...`, `a8e03809...`), release `kit.exe` `627a8a6d...` as before.

- **4 of 4 rows pass.** At 02:27:05 the Papyrus log holds `KKQ-Q01 quest script running`,
  `KKQ-Q02 marker-set=true base-is=true`, **`KKQ-Q03 stage 10 fragment ran`** and
  `KKQ-Q04 stage10-done=true`.
- **Reading:** the stage fragment kit writes into the quest `VMAD` runs when its stage has a
  log entry; without one (`game-20261117-02`) it did not run and nothing was logged. This
  confirms the two agents' diagnosis for this one quest (one stage, one fragment, log entry
  index 0); other stage shapes are untested. The fragment writer itself needed no change;
  a recipe stage that carries a fragment needs a log entry (a `validate` rule or a default
  for `new` would catch it: ticket idea).
- `report.json` SHA-256 `4ad813c8199d56035f038afcad30a819128061b7020c5fc6922d4dbe87268a70`.
  The game and MO2 were closed by the controller; `KitKraftVerifyQuest` stays enabled for
  the Q-05 session that may follow.

### game-20261117-06: agent session 2026-11-17-T06 (`kit game session`), KitKraftVerifyQuest.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-17-T06`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-17 |
| Status | inconclusive (2 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261117-06` |
| Plugins | `KitKraftVerifyQuest.esm` SHA-256 `f341e23a2bd926d6d445dcf1ea0a0b9d0a2d81fa731d635ec23149a20b445e62` |
| Probes | `docs\bench\verify-mod\verify-quest-probes.toml` SHA-256 `ed08aa2deefbaef040b65b8859c3f5e788d1c7978440d00b25c772270380952e` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-17-T06` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| Q-01 | `prid 00000014` | KKQ01Qust | papyrus-log KKQ-Q01 quest script running | prid 00000014 -> prid 00000014 Command: prid 00000014; (waited 15s) | **pass** | - |
| Q-02 | Papyrus log: KKQ-Q02 the quest property names the placed KKQ70Marker | KKQ01Qust, KKQ70Marker | papyrus-log KKQ-Q02 marker-set=True base-is=True | - | **pass** | - |
| Q-03 | Papyrus log: KKQ-Q03 the stage 10 fragment ran (QUST VMAD fragment block) | KKQ01Qust | papyrus-log KKQ-Q03 stage 10 fragment ran | - | **pass** | - |
| Q-04 | Papyrus log: KKQ-Q04 stage 10 done after SetStage | KKQ01Qust | papyrus-log KKQ-Q04 stage10-done=True | - | **pass** | - |
| Q-05 | Papyrus log: KKQ-Q05 the alias script on MarkerAlias runs (QUST VMAD alias section; NOT RUN) | KKQ01Qust, KKQ70Marker | papyrus-log KKQ-Q05 alias script running count=7 ref-is-marker=True | - | **pass** | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\modlist.txt changed during the session (4b16aa2c1fa8ec6cde2e9386c05fc4de7fd2f45a90c7ed79803c90c1c5e1b03e -> d12954f5fa75b12b1e6bd87747d5d89e122471b94c1c9410e6e5bfebe488eed2): restored from C:\Modding\kitkraft-observe\2026-11-17-T06\backup\00-modlist.txt
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (65bf61161827c8154e5f3e3001df3e938cca8baa64060aa565253bad76b2e39b -> 48daa0205a602f4a1fd189ad2799c21e2d242176279ab96367bb469e9d4582d0): restored from C:\Modding\kitkraft-observe\2026-11-17-T06\backup\01-plugins.txt

Writer's reading (bench night 2026-11-17, 02:29 to 02:33; Q-05, the alias script). Plugin
and probes are not from this branch: a release `kit.exe` (`kit-cli` only, SHA-256
`bbbd1c04715f7b2fe12f10ab100fe7988ca9a83f3be5c37e8179c072bf03425b`) was built from
`ea53c68f` (branch `opus/aliasscripts-20261117`) in a temporary worktree and target, both
removed afterwards. Its `verify-quest.toml` was used with one line changed: `QSDT = 0`
added to the quest's fields, as on this branch (`game-20261117-05`), so that Q-03 is not
the variable. Three scripts compiled with `kit papyrus compile` and the base-game sources
(`VerifyAliasReport.pex` `393a96dc8dacaa0a46eea04ceb6d52b60fb077cc953eff807cfcd25ee7b6299f`,
`VerifyQuestReport.pex` `eb512b75...ecf1264`, `QF_KKQ01Qust.pex` `5ccba81f...5d46b50`).
Plugin 3,413 bytes, SHA-256
`f341e23a2bd926d6d445dcf1ea0a0b9d0a2d81fa731d635ec23149a20b445e62` (that commit's
`e6b58d33...` plus the 23 bytes of the stage's log entry); the QUST reads `INDX QSDT QSRD
QSRD ANAM ALST ALID FNAM ALFG ALFR VTCK ALED`; validate 0 errors 0 warnings, roundtrip
identical. Probes: that commit's `verify-quest-probes.toml` with only its `sha256` line set to
the build (file SHA-256 `ed08aa2deefbaef040b65b8859c3f5e788d1c7978440d00b25c772270380952e`).
Deployed and run with this branch's release `kit.exe` (`627a8a6d...`).

- **5 of 5 rows pass.** At 02:33:00 the Papyrus log holds `KKQ-Q01`, **`KKQ-Q05 alias script
  running count=7 ref-is-marker=true`**, `KKQ-Q02`, `KKQ-Q03 stage 10 fragment ran` and
  `KKQ-Q04`. The alias script kit wrote into the alias section of the quest `VMAD` ran on
  the alias forced to the placed crate, and read its property `Count = 7` (default 0).
- **After:** `deploy disable --mods KitKraftVerifyQuest` (dry run first). The dev mod
  `KitKraft Dev - KitKraftVerifyQuest` now holds this alias build, not the
  `7192216d...` build of `game-20261117-05`. `report.json` SHA-256
  `ad842e4d7ab0c9bd81006a2833f2c9cfee4a788316b8806a8f04b816d43ee8d7`.

### game-20261117-07: agent session 2026-11-17-T07 (`kit game session`), OldSovietMarksmanRifle.esm

| Field | Content |
| ----- | ------- |
| Session | `2026-11-17-T07`, run by the session controller (ADR 0010, lane C); not an owner sitting |
| Date | 2026-11-17 |
| Status | inconclusive (2 profile file(s) changed during the session and were restored or reported) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | `sfse_loader.exe` 0.1.1.1 |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console port | 45555; checked against 7 excluded TCP range(s); `Console API and Web Application` [WebConsole] iPort=45555 |
| MO2 profile | `Kitkraft Dev` (instance C:\Modding\MO2-Starfield) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261117-07` |
| Plugins | `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` |
| Probes | `docs\bench\ow2-rerun-probes.toml` SHA-256 `e457fac2dbd61f2665b29d04a1ca973e8909b35164df55adbcffbaef3d49ed90` |
| Screenshots | `C:\Modding\kitkraft-observe\2026-11-17-T07` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `help "Old Soviet Marksman Rifle" 4 WEAP`, `prid 00000014` | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | contains 0021BBCD | help "Old Soviet Marksman Rifle" 4 WEAP -> (no line of its own: help "Old Soviet Marksman Rifle" 4 WEAP ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS----...; again in 2000 ms); help "Old Soviet Marksman Rifle" 4 WEAP -> (no line of its own: help "Old Soviet Marksman Rifle" 4 WEAP ----VIEW... | inconclusive (`help "Old Soviet Marksman Rifle" 4 WEAP`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| T-01b | `help Soviet 4 WEAP`, `prid 00000014` | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | contains 0021BBCD | help Soviet 4 WEAP -> (no line of its own: help Soviet 4 WEAP ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS-------------------- usag...; again in 2000 ms); help Soviet 4 WEAP -> (no line of its own: help Soviet 4 WEAP ----VIEWER STRINGS NOT AVAILABLE-------- ----OTHER FORMS---------------... | inconclusive (`help Soviet 4 WEAP`: no correlated reply in 3 answer(s) (only its echo or other requests' lines): no result to judge (a timing anomaly, R20, R28)) | - |
| T-02 | `player.additem 0021BBCD 1`, `player.getitemcount 0021BBCD` | WEAP 0021BBCD as stored in OldSovietMarksmanRifle.esm, RussianHuntingRifle, 0021BBCD | regex >>\s*[1-9] | player.additem 0021BBCD 1 -> player.additem 0021BBCD 1 Command: player.additem 0021BBCD 1 GetGlobalValue >> 2.00 GetGlobalValue >> 2.00 GetGlobalValu...; player.getitemcount 0021BBCD -> player.getitemcount 0021BBCD Command: player.getitemcount 0021BBCD GetItemCount >> 1.00 | **pass** | - |
| T-03 | `coc CityNewAtlantisLodgeInt`, `player.getincell 0006BB29`, `save KitKraftSession-20261117-07`, `coc CityNewAtlantisApartmentTEMPLATE`, `player.getincell 0021966F`, `load KitKraftSession-20261117-07`, `player.getincell 0006BB29` | CityNewAtlantisLodgeInt, 0006BB29, CityNewAtlantisApartmentTEMPLATE, 0021966F | regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$; regex (?m)>>\s*1(\.0+)?\s*$ | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 30s); player.getincell 0006BB29 -> player.getincell 0006BB29 Command: player.getincell 0006BB29 GetInCell >> 1.00; save KitKraftSession-... | **pass** | - |
| SO-A | `coc CityNewAtlantisApartmentTEMPLATE`, `player.getpos x`, `player.getpos y`, `player.getpos z` | CityNewAtlantisApartmentTEMPLATE | regex GetPos: X >> -?\d; regex GetPos: Y >> -?\d; regex GetPos: Z >> -?\d | coc CityNewAtlantisApartmentTEMPLATE -> coc CityNewAtlantisApartmentTEMPLATE Command: coc CityNewAtlantisApartmentTEMPLATE IsInSpace >> 0.00 GetGlobalValue >> 0...; player.getpos x -> (only the echo; again in 2000 ms); player.getpos x -> player.getpos x Command: player.getpos x GetPos: X >> -8.00; p... | **pass** | - |
| SO-B | `coc CityNewAtlantisLodgeInt`, `player.getpos x`, `player.getpos y`, `player.getpos z` | CityNewAtlantisLodgeInt | regex GetPos: X >> -?\d; regex GetPos: Y >> -?\d; regex GetPos: Z >> -?\d | coc CityNewAtlantisLodgeInt -> coc CityNewAtlantisLodgeInt Command: coc CityNewAtlantisLodgeInt IsInSpace >> 0.00 GetGlobalValue >> 0.00 GetGlobalValue...; (waited 30s); (ambiguous: row SO-A `player.getpos x`, row SO-B `player.getpos x` could each claim `GetPos: X >> 11.01`; given to none); player.g... | inconclusive (ambiguous reply: `player.getpos x` and `player.getpos x` (row SO-A) are the same getter still waiting for an answer, and the line `GetPos: X >> 11.01` names neither apart; the text cannot tell them apart: no result to judge (a timing anomaly, R20, R28)) | - |
| SO-C | `load KitKraftSession-20261117-07`, `player.getpos x`, `player.getpos y`, `player.getpos z` | CityNewAtlantisLodgeInt | regex GetPos: X >> -?\d; regex GetPos: Y >> -?\d; regex GetPos: Z >> -?\d | load KitKraftSession-20261117-07 -> load KitKraftSession-20261117-07 Command: load KitKraftSession-20261117-07; (readiness poll) GetSFSEVersion -> GetSFSEVersion Command: GetSFSEVersion; (readiness poll) GetSFSEVersion -> GetSFSEVersion Command: GetSFSEVersion; (readiness poll) GetSFSEVersion -> Get... | inconclusive (ambiguous reply: `player.getpos x` and `player.getpos x` (row SO-A), `player.getpos x` (row SO-B) are the same getter still waiting for an answer, and the line `GetPos: X >> 11.00` names neither apart; the text cannot tell them apart: no result to judge (a timing anomaly, R20, R28)) | - |

Finding: log console-history was not written during the session (C:\Users\macke\Documents/My Games/Starfield/StarfieldConsoleHistory.log has the length and modified time it had before the launch); recorded as missing
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\modlist.txt changed during the session (9476b9444abd44ae8a9e22e350ee16194af604b00e0f1c6ee39387289ecfc630 -> 4d50841844b04a3e26fa4f43ff89c11fccc96789467a8e37199ed7ace866d3f3): restored from C:\Modding\kitkraft-observe\2026-11-17-T07\backup\00-modlist.txt
Finding: C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\plugins.txt changed during the session (d49cb3a06d1949775a8423ee7eff476aeafd075c98e3f9eae5a065532ac8aee4 -> e98f0a7950a816430df61854c64a4dbd184c8f35cbfaac6a752947ab4d1d1d72): restored from C:\Modding\kitkraft-observe\2026-11-17-T07\backup\01-plugins.txt

Writer's reading (bench night 2026-11-17, 02:35 to 02:41; OW-2 Rename re-run with send-order
rows for SO-03). Probes `docs/bench/ow2-rerun-probes.toml` (SHA-256
`e457fac2dbd61f2665b29d04a1ca973e8909b35164df55adbcffbaef3d49ed90`), hand-written from the
testplan file of `game-20261117-04` with only shape and pacing changed (its header says
what). Release `kit.exe` `627a8a6d...`; the rename plugin `9c0b28a6...` enabled and last, both
verification mods disabled. Times below are ms after launch, from `report.json`
(`transcript`).

- **Why -04's rows were inconclusive.** T-03: the three `player.getincell 0006BB29` sent
  10.1 s, 12.2 s and 16.4 s after `coc CityNewAtlantisLodgeInt` all answered only their
  echo; every later `GetInCell >> 1.00` then had several identical getters waiting for it,
  so the correlator gave it to none. T-01: the answer was not late; it came in its own
  reply and listed nothing (`----VIEWER STRINGS NOT AVAILABLE----`, `----OTHER FORMS----`,
  no form line).
- **T-03 pass** with the Lodge `coc` waited 30 s and the apartment `coc` 20 s: each of the
  three getters answered `GetInCell >> 1.00` in its own reply on the first send (145,972;
  176,332; 201,887), around the save and the load of `KitKraftSession-20261117-07`. The
  check is unchanged. It proves the reload; no reference's state after it is read
  (T-04 stays manual), so no save/reload evidence is entered.
- **T-01 and T-01b inconclusive, not a timing matter.** `help "Old Soviet Marksman Rifle" 4
  WEAP` and `help Soviet 4 WEAP` each answered at once, three times, with the empty list
  and the line `usage: help <matchstring> <filter> <form type>`. On this build `help`
  found no WEAP by the renamed FULL (by a quoted phrase or by one word). The rename shows
  in the game's UI (the workbench list, `game-20261117-04`'s reading), not through `help`.
  A check that can grade T-01 needs another getter (ticket idea: read the name another way,
  e.g. a Papyrus `GetName` reporter as the verification mod does).
- **T-02 pass** again (`GetItemCount >> 1.00` on the first send). The `additem` reply carried
  unrelated `GetGlobalValue >> 2.00` lines (the game's own condition checks, as after
  every `coc`).
- **Send order (SO-03's question), what one session shows:**

  | When sent | Command | Sent at | Reply |
  | --------- | ------- | ------: | ----- |
  | 105 ms after `coc` (apartment, 202,003) | `player.getpos x` | 202,108 | echo only |
  | retry | `player.getpos x` | 204,215 | `GetPos: X >> -8.00` |
  | then | `player.getpos y`, `z` | 204,339, 204,445 | own lines, on time |
  | 30 s after `coc` (Lodge, 219,556) | `player.getpos x` | 249,667 | `GetPos: X >> 11.01` (own reply) |
  | 4.4 s after the readiness poll's last echo-only poll (load at 256,232) | `player.getpos x` | 266,660 | echo only |
  | retry | `player.getpos x` | 268,765 | echo only |
  | retry | `player.getpos x` | 272,883 | `GetPos: X >> 11.00` |
  | then | `player.getpos y`, `z` | 272,999, 273,119 | own lines, on time |

  Reading: every reply carried **at most one** `GetPos: X` line, and no X line ever arrived
  in the reply to a later `y`, `z` or other request. If the echo-only sends' answers had
  only been late, some later reply would have held two X lines; none did. So the answers
  to getters sent while a `coc` (about 2 s) or a `load` (about 16 s after it) was still
  moving were **lost, not delayed**, and nothing arrived out of order: the same reading as
  OW-1 (`game-20261116-01`, `-02`). It does not decide SO-03's two identical `prid` failure
  lines; it is the evidence for the owner's ruling on send order. Also seen: the readiness
  poll after a `load` declared ready on three echo-only polls (266,546), about 6 s before the
  game answered getters (R41's rule counts an echo as a reply); a getter sent then is lost.
  SO-B and SO-C grade `inconclusive` only because the correlator still counted SO-A's lost
  X getter as waiting; SO-A passes.
- `report.json` SHA-256 `f1925596b061b5f483dd6f1c27ccbd3d0a17f765cc876f400693fac6c45df37d`.
  The game and MO2 were closed by the controller; the session save
  `KitKraftSession-20261117-07` is in the dev profile.

### game-20261117-08: owner walk walk-2026-11-17-211238 (`kit walk`), KitKraftOwnerWalk.esm, OldSovietMarksmanRifle.esm, KitKraftGunOverhaul.esm

Entered from the recorder's draft (`game-20261117-walk2112` in its `report.json`, renumbered
here) by an agent on 2026-11-17; no game was run to enter it. Grade: rows judged by the
owner are **owner report**; the console numbers beside them are recorded, and a number
whose answer was lost is not used. An earlier attempt the same evening
(`walk-2026-11-17-205043`) stopped at W-00 (the stations stood outside the apartment's
walls, before the deck of `5dd4d588`); it is not evidence and has no entry.

| Field | Content |
| ----- | ------- |
| Session | `walk-2026-11-17-211238`, an owner sitting recorded by `kit walk` (the owner played; the recorder only read the console) |
| Date | 2026-11-17 (the session name is UTC; the walk ran from about 16:12 to 16:38 local time, UTC-5) |
| Status | fail (W-10, W-11; owner report) |
| Game build | `Starfield.exe` 1.16.244.0 |
| Script extender | SFSE 0.2.21, release idx 26, runtime 01100F40 (the console endpoint's answer to `GetSFSEVersion`) |
| Console tools | Console API and Web Application 2.19.0.0; Console Command Runner Updated 1.11.0.0 |
| Console endpoint | http://127.0.0.1:45555/console |
| MO2 profile | `Kitkraft Dev` (C:\Modding\MO2-Starfield\profiles\Kitkraft Dev) |
| Save | started from `KitKraftBase` (the owner's); the walk sends no save or load. W-09's `KitKraftWalk` was never written (W-10 below) |
| Plugins | `KitKraftOwnerWalk.esm` 26,001 bytes, SHA-256 `d7c8305c7e06b979be5a143a30c7eeea59db83a3518cb7b45735cf21e2ac6e0a` (the build `out\owner-walk\` of `5dd4d588`), position 18, slot 08, confirmed in game by W-00 (`getglobalvalue 0800100B` answered); `OldSovietMarksmanRifle.esm` SHA-256 `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` (installed from the package, W12), position 17, slot 07; `KitKraftGunOverhaul.esm` SHA-256 `5e2f84e7b1dfd6a7876664a3bf203c3bbeff8679907bccd240c6ecb148f982d1`, position 14, slot 04 |
| Stations | `docs\bench\owner-walk\stations.toml` SHA-256 `d51745c124644d13d5b98b1ba9e80e2fa01c6f8499a0d1f1af5d40859054d3ad` (as of `5dd4d588`) |
| Runtime IDs | slot 08: KKW01Fact 0800100A, KKW02Glob 0800100B, KKW10Misc 08001000, KKW11Misc 08001001, KKW12Misc 08001002, KKW13Misc 08001003, KKW14Misc 08001004, KKW50OwnedMugRef 08001016 |
| Papyrus log | `Documents\My Games\Starfield\Logs\Script\Papyrus.0.log`; the walk's part copied as `papyrus-tail.log` |
| Screenshots | `C:\Modding\kitkraft-observe\walk-2026-11-17-211238\screenshots\` (W-01, W-03 to W-08, W-10, W-11) |
| Files | `report.json` SHA-256 `a406f9604aae325cbfc655a0dc326e93f7719eb920f3270a936a2f210dfcbd1b`; `transcript.txt` `783a0953156079dcfdd353236dddf0e974c7e42c7f2bcb0969905216215d5e2e`; `ledger-draft.md` `a06d82f526d0d22387b40790a3cc3d6e6a09703c4b311a09ee5e040bf52eb4eb`; `papyrus-tail.log` `e622370c329004c5939b42e4cec31c603e6bfa9056b532d723f854b76868a40b` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| W-00 | check-in: `coc CityNewAtlantisApartmentTEMPLATE` (owner), then the console | GLOB `KitKraftOwnerWalk.esm:00100B` `KKW02Glob`, 0800100B | In the apartment; the walk plugin's global answers under its runtime ID | `player.getincell 0021966F` -> `GetInCell >> 1.00`; `getglobalvalue 0800100B` -> `GetGlobalValue >> 0.00` | **pass** | transcript +19.6 s |
| W-01 (O4-01) | look at `KKW10 Owned Mug` (REFR XOWN -> FACT `KKW01Fact`) | REFR KKW50OwnedMugRef 08001016, MISC KKW10Misc 08001000, FACT KKW01Fact 0800100A | The prompt reads Steal (red) | owner: p | **pass** (owner report) | W-01.png |
| W-02 (O4-02) | take it | as W-01 | One more, marked stolen; no bounty | owner: p (stolen, no bounty). Console: KKW10 Owned Mug 0 -> 1. `GetStolenItemValueNoCrime`, `GetStolenItemValue`, `GetCrimeGold`: answer lost (echo only, 4 sends each, before and after), so no console number supports "stolen" or "no bounty" | **pass** (owner report) | - |
| W-03 (O4-01) | control: `KKW11 Free Mug`, no owner; look, take | REFR KKW51FreeMugRef, MISC KKW11Misc 08001001 | Take, not red | owner: p. KKW11 Free Mug 0 -> 1; the stolen-goods getter's answer lost | **pass** (owner report) | W-03.png |
| W-04 (O4-01) | `KKW12 Your Mug`, REFR XOWN -> NPC_ `Starfield.esm:000007` (Player); look, take | REFR KKW52YourMugRef, MISC KKW12Misc 08001002 | Take | owner: p. KKW12 Your Mug 0 -> 1 | **pass** (owner report) | W-04.png |
| W-05 (O4-01, O4-02) | `KKW20 Owned Locker` (REFR XOWN -> KKW01Fact); open, take the three wrenches | REFR KKW53LockerRef, CONT KKW20Cont, MISC KKW13Misc 08001003 | The locker reads owned; the wrenches come out stolen | owner: p. KKW13 Locker Wrench 0 -> 3; the stolen-goods getter's answer lost | **pass** (owner report) | W-05.png |
| W-06 (O4-07 `OnActivate`) | the owner activates `KKW30 Walk Switch` once | REFR KKW54SwitchRef (VMAD `KitKraft:WalkSwitch`), ACTI KKW30Acti, GLOB KKW02Glob 0800100B, `Scripts\KitKraft\WalkSwitch.pex` SHA-256 `e468882b...827d49d8` | The notification; KKW02Glob +1; the Papyrus log has `KKW-A01` | owner: p (the notification). Console: `KKW02Glob` 0 -> 1. Papyrus log: `[16:20:57] KKW-A01 activated by-player=true count=1` | **pass** (console and log; the notification by owner report) | W-06.png, papyrus-tail.log |
| W-07 (O4-06) | at the placed Industrial Workbench (`Starfield.esm:1F50AD`), craft `KKW42Cobj` | COBJ KKW42Cobj, REFR KKW55BenchRef, AMMO `Starfield.esm:0547A1` | Listed under Manufactured, 1 iron and 1 aluminum, no research; crafts 6 shells | owner: p (listed, crafted). Console: 12g Shotgun Shell 0 -> 6 (NNAM 6). Iron and aluminum counts: answer lost, so the cost rests on the owner's report | **pass** (owner report; the 6 shells by the console) | W-07.png |
| W-08 (W12) | the rename package installed through MO2, its manifest check; the rifle in the inventory | WEAP `Starfield.esm:21BBCD` `RussianHuntingRifle`, `OldSovietMarksmanRifle.esm` | "Old Soviet Marksman Rifle"; every manifest line `ok` | owner: p (the name, and the manifest check all `ok`, as the station asks). Hunting rifle count 2 -> 2 (read after the setup's additem) | **pass** (owner report) | W-08.png |
| W-09 (O5-15, before the save) | the three weapons' magazines, then `save KitKraftWalk` (owner) | WEAP `Starfield.esm:2984DF`, `0546CC`, `18DE2C`; `KitKraftGunOverhaul.esm` | Magazines 24, 5, 48; the save is written | owner: p (24, 5, 48 and "the save finished"). The save was **not** written (W-10 below). Counts 3, 1, 1 (after the setup's additem) | magazines **pass** (owner report); the save **fail** (corrected here: the draft passed the row on the owner's p) | - |
| W-10 (O5-15) | `load KitKraftWalk` (owner), check the magazines | as W-09; MISC KKW14Misc 08001004 (reload marker) | Back from the save (marker gone), magazines 24, 5, 48 | owner: f "save not found, also can't find in menu". Reload marker 1 -> 1 (no load happened); still in the apartment (`GetInCell >> 1.00`). W-10.png shows the Load menu: the 4:21 PM autosave, the 3:57 PM exit save, the 3:56 PM autosave and a 12:45 AM save, no `KitKraftWalk` | **fail** (owner report; the marker agrees) | W-10.png |
| W-11 (O4-01, O4-02) | `coc CityNewAtlantisFreestarEmbassy` (owner); look at an item on a shelf, take it | CELL `Starfield.esm:017307`, XOWN `24F839` -> `PlayerFaction` `01C21C` in this plugin | Take, no crime (`docs/recipes/ownership.md`); a Steal may be the item's own owner | owner: f "yellow mustard vase said steel". In the embassy (`player.getincell 00017307` -> `GetInCell >> 1.00`; W-11.png is the status screen, "Jemison - Freestar Collective Embassy"). The stolen-goods and crime-gold getters: answer lost | **fail** (owner report) | W-11.png |
| W-12 (O4-05) | outpost: build `KKW40 Coffee Maker` | COBJ KKW41Cobj, STAT KKW40Stat | Listed and builds | owner: s "no outpost" (a level-one character) | not-run (skipped by the owner: no outpost) | - |

Owner's answers (verbatim, from `transcript.txt`): W-01 to W-09 `p`; W-10 `f "save not
found, also can't find in menu"`; W-11 `f "yellow mustard vase said steel"`; W-12 `s "no
outpost"`.

Writer's reading (2026-11-17, from the walk's folder and the disk, read only; no game run).

- **What passed.** A reference's own `XOWN` takes effect: a new faction with only a name
  as owner gives Steal (W-01, W-05), the player's actor base as owner gives Take (W-04),
  no owner gives Take (W-03); the cloned locker's `CNTO` gave its three cloned wrenches
  (W-05). `OnActivate` fired on the script `kit` attached to a placed reference when the
  owner pressed it (W-06): the counter and the log line are machine evidence, the press a
  person's. The cloned fabricator recipe works at a placed vanilla workbench (W-07). The
  rename package installs through MO2 and its manifest checks `ok` (W-08, W12). Every
  ownership and crime number the console was asked for (`GetStolenItemValueNoCrime`,
  `GetStolenItemValue`, `GetCrimeGold`) came back as its echo only, four sends each, in
  every station: whether the console answers those functions is still not observed, and
  no row rests on them. The deck of ten `ComIntAddonRmSmMidBotB01` tiles held the owner
  and the stations (W-01 to W-07; `docs/mcp.md`, "Placing in an interior").
- **W-10: the save of W-09 was never written.** On disk: no `KitKraftWalk.sfs` in the
  profile's `saves` folder (listed; the owner's primary saves folder was not looked at).
  The folder's files written during the walk are `Autosave2_..._20261117212100` (16:21:00)
  and `Autosave3_..._20261117213301` (16:33:01). The Papyrus log has a `Saving game...`
  block at exactly those two times and none other; W-09 ran from 16:26:22 to 16:28:36
  (`transcript.txt` +824 s to +958 s; walk start 21:12:38 UTC). W-10.png, taken at the
  owner's answer, shows the Load menu without it. The game's console history
  (`Documents\My Games\Starfield\StarfieldConsoleHistory.log`, read only, modified 16:31:32)
  ends `coc CityNewAtlantisApartmentTEMPLATE`, `load KitKraftWalk`, `coc
  CityNewAtlantisFreestarEmbassy`, with no `save KitKraftWalk`; but it also lacks W-01's
  `player.moveto 08001016`, which the owner must have typed to stand on the deck, so it is
  not a complete record. The recorder saw nothing of it: it reads only the replies to its
  own commands, and its transcript has no line about a save. The reload marker logic
  behaved: `KKW14Misc` was added at +967 s, after W-09's answer, and read 1 before and 1
  after, which says no load happened even without the owner's note. **Cause (inference,
  not shown):** the `save KitKraftWalk` command did not run in the game (not submitted, or
  mistyped): a console `save` does land in this folder when the session controller sends
  it (`KitKraftSession-20261117-07.sfs`, `game-20261117-07`). The draft passed W-09 on the
  owner's `p`; its "the save is written" part is a fail, corrected above. **Changed:** `kit
  walk` now lists the profile's `saves` folder after W-09 (`makes_save`), asks again while
  `KitKraftWalk.sfs` is missing and fails W-09 if it never appears; W-10 (`needs_save`) is
  not asked without it (`crates/game/src/walk.rs`, `docs/bench/owner-walk/README.md`).
  O5-15 stays open.
- **W-11: the item has its own owner.** The walk plugin carries the override: its `CELL`
  `Starfield.esm:017307` wins in `Kitkraft Dev` (`kit inspect`: 2 versions, winner
  `KitKraftOwnerWalk.esm`, `XOWN.Owner` `24F839` -> `01C21C`, every other subrecord the
  winner's but the inline `FULL`). The vanilla cell's owner is `FreestarEmbassyFaction`
  (`Starfield.esm:24F839`, FACT). The embassy has 9 vase references, all `MISC` and all
  with their own `REFR` `XOWN` -> `CrimeFactionFreestar` (`Starfield.esm:0638E5`); the two
  yellow ones are `VaseTableMed01_Yellow01` (`Starfield.esm:03DC66`, at (3.86, 2.82, 0.44))
  and `VaseTableLG03_Yellow01` (`2D5C0C`, at (-15.33, 9.89, 0.20)). Of the cell's 453
  references of takeable or lootable bases, 417 carry their own owner (360 of 384 `MISC`,
  all 26 `ALCH`, 26 of 27 `LVLI`, 3 `FLOR`, 1 `CONT`: `CrimeFactionFreestar`; one `BOOK`,
  `3C805A`: `PlayerFaction`), and 36 none (24 `MISC`, 1 `BOOK`, 10 `CONT`, 1 `LVLI`). The
  recipe changes only the cell's `XOWN`, so a Steal on the vase is what its own owner
  predicts **if** a reference's owner outranks the cell's; that order is not observed (the
  apartment, where W-01 to W-05 ran, has no cell owner). So W-11 does not show the recipe
  wrong: it did not test it. The recipe's claim (`docs/recipes/ownership.md`: the cell is
  owned by `24F839`, and with `PlayerFaction` as owner its items read Take) rests on the
  records only; the in-game half was never observed before this walk and still is not.
  **Next probe:** in the embassy with the walk plugin, look at an item with **no** owner of
  its own, e.g. the two Digipicks `03E159` and `03E166` with the folder, tablet and pen at
  about (-13.2, -4.5, 0.6), or the to-go coffee cups `0F18EE` to `0F18F2` at about
  (-9.9, 1.8, 0.8): Take means the cell override took effect. Control: the same item with
  the plugin disabled should read Steal (the vanilla cell owner). `stations.toml` W-11 now
  says so. The station's watch `GetStolenItemValueNoCrime 0024F839` names the cell's owner,
  not an item's; its answer was lost anyway.
- **Corrections the walk exposed** (not results of this walk's rows): the first POI's
  crate (-17, -4, 4.63) stands on the bedroom's east wall line, and the owner never found it
  on foot; M4.O2's visit pass (`game-20261015-03`) was a `player.moveto` and a console
  click and shows presence and persistence, not reachability. The verification mod's
  references (y -8) are behind the bedroom's south wall; their console passes stand.
  `docs/poi/first-poi.md` and `docs/bench/verify-mod.md` ("Correction"), `docs/gaps.md`.

Findings: (1) W-10, the owner's save not written: fixed in the recorder (above), O5-15
open. (2) W-11, the ownership recipe untested on an item without its own owner: the next
probe above; `docs/gaps.md`. (3) The stolen-goods and crime-gold console functions answered
nothing in this walk (ticket idea: read them through Papyrus, as the verification mod's
reporter reads other state).
