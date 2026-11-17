# Owner walk: one sitting for the M4 rows only eyes can judge

You play; `kit walk` watches the console. It tells you what to do at each station, waits
for you to press Enter, reads the game's console before and after, and writes everything
to a folder under `C:\Modding\kitkraft-observe\`, ending with a draft ledger entry an agent
can enter for you.

It never starts or stops the game or MO2, never saves or loads, never moves you, and
writes nothing outside its own folder. It refuses the `Default` profile. The plugin owns
new forms, so play on a disposable save of `Kitkraft Dev` only (the base save
`KitKraftBase` is one).

**Time:** about 15 to 20 minutes: 2 to 3 to launch and load, about 8 for the apartment
stations, 5 for the outpost at the end. Built and tested against a fake console only (no
game was run to prepare this).

## What it covers

| Station | Where | M4 row | What you judge |
| ------- | ----- | ------ | -------------- |
| W-00 | apartment | (check-in) | nothing: the console checks you are there and the plugin loaded |
| W-01 | apartment | O4-01 | the owned mug's prompt reads **Steal** |
| W-02 | apartment | O4-02 | taking it: marked stolen, no bounty |
| W-03 | apartment | O4-01 | control, an unowned mug: **Take** |
| W-04 | apartment | O4-01 | a mug the player owns: **Take** |
| W-05 | apartment | O4-01, O4-02 | the owned locker, and its three wrenches come out stolen |
| W-06 | apartment | `OnActivate` | press the switch: a notification (the console and the Papyrus log decide) |
| W-07 | apartment | O4-06 | the placed Industrial Workbench lists the KKW42 shotgun-shell recipe and crafts it |
| W-08 | anywhere | W12 | the rename package: the hunting rifle reads **Old Soviet Marksman Rifle** |
| W-09 | apartment | O5-15 | the overhaul's magazines (Maelstrom 24, Hard Target 5, Drum Beat 48), then you save |
| W-10 | apartment | O5-15 | you load that save; the magazines are the same |
| W-11 | embassy | O4-01, O4-02 | the ownership recipe: embassy items read **Take**, no crime |
| W-12 | an outpost | O4-05 | **KKW40 Coffee Maker** is in Build > Decorations and builds |

The apartment stations stand in a row on the floor, from the first mug to the workbench.
Every object's name starts with `KKW` and its station's number.

## Before you start (MO2 and the game closed)

In PowerShell, from the checkout that holds the build (`out\owner-walk\` and
`outename\`; an agent hands it over built, with a `kit` new enough to have `kit walk`:
`kit help` lists `walk`):

```powershell
kit deploy out\owner-walk\pkg\KitKraftOwnerWalk C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev" --dry-run
kit deploy out\owner-walk\pkg\KitKraftOwnerWalk C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev"
kit deploy enable C:\Modding\MO2-Starfield --mods KitKraftOwnerWalk --dev --profile "Kitkraft Dev" --dry-run
kit deploy enable C:\Modding\MO2-Starfield --mods KitKraftOwnerWalk --dev --profile "Kitkraft Dev"
```

Check: the plugin `KitKraftOwnerWalk.esm` is 25,171 bytes, SHA-256
`12acf98f4c832f92de4acb938824d7e340643961b6ccba176185ff9a781f4083` (the deploy dry run
prints it). Keep `KitKraftVerify.esm` disabled: it uses the same row of floor.

Then in MO2, on `Kitkraft Dev`:

1. For W12: untick `KitKraft Dev - OldSovietMarksmanRifle`, then *Install a new mod from
   an archive* and pick `out\rename\pkg\OldSovietMarksmanRifle.zip`. MO2 should not ask
   for a data folder. Enable it, and run the manifest check of `docs\packaging.md`
   ("Check the installed files against the manifest") in the new mod's folder: every line
   should read `ok`. Remember the answer for W-08.
2. For O5-15: `KitKraftGunOverhaul.esm` stays enabled (it is).
3. Launch **SFSE** from MO2, load `KitKraftBase` (it is in the apartment).
4. Put the terminal where you can see it beside the game (a second screen, or a corner):
   screenshots are of the whole screen, taken when you press Enter.

## Start the recorder

In a terminal, once you stand in the game:

```powershell
kit walk docs\bench\owner-walk\stations.toml --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe
```

It prints the plugin hashes, the runtime IDs it will use and the stations, then the first
station. At every station:

- do what it says in the game (open and close the console with `~` as usual);
- come back to the terminal and press **Enter**, or type a letter and an optional note:
  - `p the prompt read Steal` it looked right;
  - `f the shells did not appear` it looked wrong;
  - `s no outpost nearby` skip this one;
  - `q` stop here (the rest is recorded as not run).

Stations about what you see (prompts, names, menus) need your `p` or `f`; Enter alone
leaves them inconclusive. After Enter the recorder waits until the game answers the
console again and stays quiet for 8 seconds before reading (commands sent while a cell
loads are lost), so give it a few seconds.

## The stations, and what you should see

- **W-00** `coc CityNewAtlantisApartmentTEMPLATE`, close the console, Enter. Expected: it
  passes on its own. If it fails, type `q`: the plugin is not loaded where the recorder
  thinks (see Troubleshooting).
- **W-01** Look at **KKW10 Owned Mug** (do not take it). Expected: the prompt says
  **Steal** in red.
- **W-02** Take it, find it in the inventory. Expected: marked stolen; no bounty message.
- **W-03** **KKW11 Free Mug**: look, then take. Expected: **Take**, not red.
- **W-04** **KKW12 Your Mug**: look, then take. Expected: **Take**.
- **W-05** **KKW20 Owned Locker**: note the prompt, open it, take the three **KKW13 Locker
  Wrench**. Expected: the locker shows as owned (red); the wrenches come out stolen.
- **W-06** Activate **KKW30 Walk Switch** once. Expected: "KitKraft walk: the switch was
  pressed" on screen.
- **W-07** Use the **Industrial Workbench** at the end of the row (you have been given 10
  iron and 10 aluminum). Expected: under Manufactured, "12g Shotgun Shell" with a
  description starting KKW42, costing 1 iron and 1 aluminum, no research; craft it once.
- **W-08** You are given the hunting rifle. Expected: Inventory > Weapons reads **Old
  Soviet Marksman Rifle**. Type `p` only if the manifest check was all `ok` too.
- **W-09** You are given Maelstrom, Hard Target and Drum Beat. Expected magazines: 24, 5,
  48. Then the console: `save KitKraftWalk`, close it, wait a moment, Enter.
- **W-10** The console: `load KitKraftWalk` (or Esc > Load). Back in the apartment, close
  the console, check the three magazines again. Expected: the same. (The recorder gave you
  a "KKW14 Reload Marker" after the save; it must be gone after the load, which proves the
  load.)
- **W-11** The console: `coc CityNewAtlantisFreestarEmbassy`. Look at an item on a shelf,
  then take it. Expected: **Take**, no crime. A Steal on one item may be that item's own
  owner: say which in the note.
- **W-12** Go to an outpost: one of yours, or land anywhere outposts are allowed and place
  a beacon from the scanner. You have been given aluminum, copper and sealant. Build mode
  > Decorations: **KKW40 Coffee Maker** next to the vanilla Coffee Maker, same cost.
  Expected: it builds and looks like the vanilla one.

Then quit the game as you like. The recorder prints where it wrote.

## What you get

`C:\Modding\kitkraft-observe\walk-<date>-<time>\`:

| File | What |
| ---- | ---- |
| `transcript.txt` | every console command the recorder sent and the game's whole reply, your typed answers, each station's result, with times |
| `ledger-draft.md` | the draft entry for `docs/game-observations.md`: header (game build, SFSE version from the endpoint, plugin hashes), one row per station (pass, fail, inconclusive or not-run; your answers marked "owner report") |
| `report.json` | the same rows in the session controller's shape, for `cargo xtask session-evidence` |
| `papyrus-tail.log` | the Papyrus log written during the walk (for W-06) |
| `screenshots\W-nn.png` | one per station that asks for it |

Hand the folder's path to an agent: it renumbers the entry, enters it and the evidence.

## Undo

The plugin owns forms: never load a save made with it in your real playthrough (none is:
`KitKraftWalk` is in `Kitkraft Dev`'s own saves). To take it out (MO2 and the game
closed):

```powershell
kit deploy disable C:\Modding\MO2-Starfield --mods KitKraftOwnerWalk --dev --profile "Kitkraft Dev" --dry-run
kit deploy disable C:\Modding\MO2-Starfield --mods KitKraftOwnerWalk --dev --profile "Kitkraft Dev"
```

If a later build was deployed over this one, `kit deploy rollback C:\Modding\MO2-Starfield
KitKraftOwnerWalk --dev --profile "Kitkraft Dev" --dry-run` (then without `--dry-run`)
swaps the previous one back. For W12, untick the installed `OldSovietMarksmanRifle`
package and tick `KitKraft Dev - OldSovietMarksmanRifle` again in MO2.

## Troubleshooting

- **W-00 fails with `Item '...' not found`:** the plugin's runtime slot is not the one the
  recorder inferred. Quit (`q`), read the slot the game uses (in the console, click a KKW
  object: the first two hex digits of its ID), and start again with `--slot <those two
  digits>`.
- **"waiting for the game to answer the console" never ends:** the Console API mod is not
  enabled, or the game is on its main menu or a loading screen. The recorder gives up
  after 3 minutes; the station is then inconclusive, and the walk goes on.
- **A row says `answer lost`:** the game dropped an answer twice and the re-read too; the
  row is inconclusive, not failed. Nothing to do.

## Files

| File | What |
| ---- | ---- |
| `owner-walk.toml` | the `kit build` recipe (reuses the workshop, fabricator, ownership and scripts recipes and the verification mod's items) |
| `scripts\KitKraft\WalkSwitch.psc` | the switch's script: `OnActivate` adds 1 to `KKW02Glob`, writes `KKW-A01 ...` to the Papyrus log, shows a notification |
| `stations.toml` | the stations `kit walk` reads (format: `crates/game/src/walk.rs`) |

Rebuilding (an agent's job; from the repo root, `KITKRAFT_DATA_DIR` set,
`$SF = C:\Program Files (x86)\Steam\steamapps\common\Starfield`):

```powershell
kit papyrus compile docs\bench\owner-walk\scripts\KitKraft\WalkSwitch.psc out\owner-walk\Scripts --sources-zip "$SF\Tools\ContentResources.zip"
kit build docs\bench\owner-walk\owner-walk.toml out\owner-walk\KitKraftOwnerWalk.esm --overwrite
kit validate --plugin out\owner-walk\KitKraftOwnerWalk.esm --plugins Starfield.esm
kit roundtrip out\owner-walk\KitKraftOwnerWalk.esm --verify
kit package out\owner-walk\KitKraftOwnerWalk.esm out\owner-walk\pkg --scripts out\owner-walk\Scripts\KitKraft\WalkSwitch.pex --spec docs\bench\owner-walk\owner-walk.toml --plugins Starfield.esm --overwrite
kit build docs\smoke-rename.toml out\rename\OldSovietMarksmanRifle.esm --overwrite
kit package out\rename\OldSovietMarksmanRifle.esm out\rename\pkg --spec docs\smoke-rename.toml --plugins Starfield.esm --overwrite
```

Checked 2026-11-17 (headless): `WalkSwitch.pex` SHA-256
`e468882bd5c0b818e5dbe7025f5e11afe1b3233e8ca841e744df3605827d49d8`; the plugin as above
(21 records; two builds byte-identical); validate 0 errors, 0 warnings; roundtrip
identical; package validate passed; the rename plugin `9c0b28a6...4078ea27`, as
`docs/smoke-rename.md`; `kit walk ... --slot 2C --plugin out\owner-walk\KitKraftOwnerWalk.esm
--dry-run` against `Kitkraft Dev` resolved every EditorID; the deploy dry run plans 2 files
into `KitKraft Dev - KitKraftOwnerWalk`.

What no game has shown yet: whether a reference's own `XOWN` (a faction, the player's
actor base) gives the Steal prompt, whether the placed vanilla workbench works as a
fabricator, whether the console answers `GetStolenItemValueNoCrime`, `GetStolenItemValue`
and `GetCrimeGold` (recorded, not graded), and the screenshots' content.
