# Rename smoke test (M2.E10, `PLAN.md` step 2 rung 1)

Rename the **Old Earth Hunting Rifle** to **Old Soviet Marksman Rifle** in a new plugin,
check it, and load it in the game. Everything up to the in-game check is automated
(`crates/engine/tests/smoke_rename.rs`, through `execute`, skipped without
`KITKRAFT_DATA_DIR`); the in-game check is the owner's (M2.O6).

## The record

| Fact | Value | How it was found |
| ---- | ----- | ---------------- |
| Form | `Starfield.esm:0021BBCD` (WEAP), runtime ID `0021BBCD` | `kit query WEAP --where "FULL ~ 'Old Earth Hunting Rifle'" --plugins Starfield.esm --data-dir <Data>`: 1 match of 406 WEAP |
| EditorID | `RussianHuntingRifle` | same |
| Name | `FULL` = string ID `0x0002E469` = "Old Earth Hunting Rifle" in `strings/starfield_en.strings` of `Starfield - Localization.ba2` | lane D's string tables (M2.D7) through lane G's readers |
| Other lstrings in the record | `DESC` = ID 0 (no text), and 7 template names in the `OBTE` block (`FULL` IDs `0x0002EDD4`..`0x0002EDD8`, `0x000320CD`, `0x00033763`: "Standard", "Standard Low", "Speechless Fire", "Upgraded", "Fallback", "Despondent Assassin", "Simple") | `kit set` reports them as `inlined_lstrings` |
| Winner in the owner's `Default` profile (2026-10-14) | `Starfield.esm`: no other plugin of the 84 overrides this WEAP | `kit inspect Starfield.esm:0021BBCD --profile <Default>`: 1 version |
| Instance naming rules | `INRD` = `0x002377CF` (INNR `dn_CommonGun`): `WNAM` prefixes chosen by keyword (`Titanium`, `Tesla`, `Elemental`, `Explosive`, ...) | corpus read of the INNR record |

Only one WEAP carries the name, so there is no base-vs-variant choice to make.

## Steps

```powershell
$data = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
$out  = 'C:\Modding\kitkraft\out\OldSovietMarksmanRifle.esm'

# 1-2. The load order and the form (Starfield.esm alone: the rifle's only version).
kit query WEAP --where "FULL ~ 'Old Earth Hunting Rifle'" --plugins Starfield.esm --data-dir $data

# 3. Write the plugin, then the same from the spec (same bytes). `set` updates an existing
#    patch in place (its header is kept), so delete $out first to write it fresh; `build`
#    always writes fresh and replaces an existing file only with --overwrite.
kit set Starfield.esm:0021BBCD 'FULL=Old Soviet Marksman Rifle' --into $out --plugins Starfield.esm --data-dir $data
kit build docs\smoke-rename.toml C:\Modding\kitkraft\out\built\OldSovietMarksmanRifle.esm --overwrite

# 4. Check it.
kit validate --plugin $out --plugins Starfield.esm --data-dir $data
kit roundtrip $out --verify

# 5. The new name wins with the plugin appended.
kit inspect Starfield.esm:0021BBCD --plugins Starfield.esm --plugins $out --data-dir $data
```

In PowerShell 5.1 write `'FULL=Old Soviet Marksman Rifle'` (or `FULL="..."` inside
`--%`): PowerShell drops embedded double quotes when it calls a program, and the
shorthand takes everything after `=` anyway.

`kit set ... --profile <Default>` refuses today: the owner's profile loads
`stroudpremiumedition.esm` before two of its masters (`sfbgs00d.esm`, `SFBGS047.esm`), and
until lane D's round 2 makes the winning override a safe source, `set` and `build` stop on
any such load order ("load order unverified"). The rifle's winner in that profile is
`Starfield.esm` anyway (above), so `--plugins Starfield.esm` writes the same record.

## Result (2026-10-14, release build, after the Fable review round 2 fixes)

`OldSovietMarksmanRifle.esm`, 2,221 bytes, SHA-256
`9C0B28A6E9C4CDF4C7878CF7DD745A022CF6E0AE943D2AD3DA6C8F044078EA27`, identical from `set`
and `build` (regenerated in `C:\Modding\kitkraft\out\` and `out\built\`). The first
build (2,197 bytes, `73F3F92F27EC1285...194A3C24`) differs only in `TES4`: review finding 5
added `CNAM` "DEFAULT" and `INCC` 0; every byte after `TES4` is the same.

- `TES4`: flags `0x00000001` (master: an `.esm`, like every official file; not localized,
  no small/medium bit), form version 582, `HEDR` 0.96 / 2 records / next object ID
  `0xFF001000`, `CNAM` "DEFAULT", one master `Starfield.esm` (`MAST` + 8-byte `DATA`),
  `INCC` 0 (no interior cells), no `ONAM`; `SNAM`, `BNAM` and `CHGL` deliberately not
  written (`docs/corpus.md` "Writing rules").
- `GRUP WEAP` (top level) holding one record: `WEAP 0021BBCD`, record flags `0x40000000`
  and form version 581 as in `Starfield.esm`, every subrecord in the original order;
  `FULL` = "Old Soviet Marksman Rifle" (26 bytes inline), `DESC` = empty string, the 7
  template names inline; every other subrecord byte-identical to `Starfield.esm`'s
  (2,073 bytes of data against 1,999: the inlined strings).
- `kit validate`: passed, 0 errors, 0 warnings (all six plugin rules; 17 FormIDs checked
  against `Starfield.esm`, 52 opaque subrecords unchecked). `kit roundtrip --verify`:
  identical. `kit inspect` with the plugin appended: 2 versions, winner
  `OldSovietMarksmanRifle.esm`, FULL "Old Soviet Marksman Rifle".

For contrast, the Resupply mistake (`FULL` = the master's 4-byte ID copied as is,
`kit set ... 'FULL=hex:69E40200'`) is refused by `set` itself: it runs the `validate`
plugin rules on the record it wrote, `lstring.copied-id` fails at `FULL[0]`, and nothing
is written (the automated test checks it). The rifle has no subrecord the corpus lstring
table (`docs/validate.md`) adds: its 8 inlined lstrings are the schema's.

## In game (owner, M2.O6)

These steps are run by the owner, or by `kit game session` on `KitKraft Dev` (ADR 0010, `docs/game-sessions.md`) once its launch facts are observed (M4.T1) and the console tools are installed (M4.O7). Use a disposable save.

1. In MO2 (`C:\Modding\MO2-Starfield`), create an empty mod (right-click the mod list >
   *Create empty mod*), name it `Old Soviet Marksman Rifle (kitkraft smoke test)`.
2. Copy `C:\Modding\kitkraft\out\OldSovietMarksmanRifle.esm` into that mod's folder
   (`mods\Old Soviet Marksman Rifle (kitkraft smoke test)\`).
3. Enable the mod; in the plugin list enable `OldSovietMarksmanRifle.esm` and put it at the
   end of the load order. It must load **after** the rifle's current winner
   (`Starfield.esm` in the Default profile today; if another mod is added that overrides
   `Starfield.esm:0021BBCD`, rebuild the patch from that load order and load the patch
   after it). Its only master is `Starfield.esm`, the only master the copied record's
   FormIDs reference.
4. Launch Starfield through MO2, load the disposable save, open the console and run
   `player.additem 0021BBCD 1`.
5. Check the item's name in the inventory and in the weapon's inspect view. Record in
   `plan/STATUS.md` under M2.O6 what was seen, with a screenshot if possible.

Expected: **Old Soviet Marksman Rifle**. The displayed name of a weapon instance is the
base `FULL` plus instance naming rules: this rifle's `INRD` points to INNR `dn_CommonGun`,
whose rules add prefixes chosen by the instance's keywords (legendary and material
effects such as "Titanium", "Elemental", "Tesla"). A plain instance from `additem` should
show the bare new name; an instance with such a modification can show "Elemental Old
Soviet Marksman Rifle" and the like, which is the rename working. If the old name
appears unchanged, check (in this order) that the plugin is active and after the winner,
that no later plugin overrides `0021BBCD`, and whether the shown name comes from an OMOD
or INNR string rather than the WEAP's `FULL`; record the finding and open a follow-up
task. If the name shows as garbage such as `%@`, a string ID was read as text: that is
what `lstring.copied-id` exists to catch, so record it as a validate bug.

Remove the mod (or disable it) after the check; the plugin adds no new forms and no
scripts, so dropping it from a save is safe.
