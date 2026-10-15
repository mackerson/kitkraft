# Gun overhaul build 2 (M3.E9 round 2)

The second overhaul build: the owner's brief (`docs/overhaul/brief.md`, rows B1..B14) in
`docs/overhaul/overhaul.toml`, rebuilt on 2026-10-15 against the owner's `KitKraft Dev`
profile now that lane B round 4 types the OMOD property values
(`DATA.Properties[n].Value1.{Int,Float,Bool,FormId,Enum}`, refused unless the entry's
ValueType selects that variant). The plan, as `kit build --plan` prints it:
`docs/overhaul/build-2-plan.md`. Build 1 (what changed since): `docs/overhaul/build-1.md`.
The spec is checked by `crates/engine/tests/overhaul_build.rs` on a fixture of the real
records.

**Summary.** 46 field changes on 21 records, all overrides of `Starfield.esm` forms (13 WEAP,
4 AMDL, 4 OMOD). 13 rows implemented, 1 dropped (B8, by the owner on 2026-10-15), none
partial or open. New since build 1: the default receivers' own fire rate (B3, B6, B11) and
the Hard Target's large magazine (B10); the owner's readings of B11 and B12 (2026-10-15)
make those rows complete as built.

## Artifacts

| File | Size | SHA-256 |
| :--- | ---: | :--- |
| `C:\Modding\kitcraft\out\overhaul\KitKraftGunOverhaul.esm` | 45,271 | `5e2f84e7b1dfd6a7876664a3bf203c3bbeff8679907bccd240c6ecb148f982d1` |
| `C:\Modding\kitcraft\out\overhaul\package\KitKraftGunOverhaul-0.2.0.zip` | 9,784 | `ec9ab1b3f59be2e2d207aed1a5103fd12471a14b498ff6fedcac414d3c045703` |
| `...\package\KitKraftGunOverhaul\kitkraft-manifest.json` | 1,230 | `73527bcc45b883bf54fc2586dca5a42d7947d99702656b7f92291b1ebe604891` |
| `docs/overhaul/overhaul.toml` (as packaged, commit `c100a18`) | 14,232 | `308389b63f28bcfe50568f70005575d9b8d4256febc9ee8128c429596fd3e518` |

The `out` folder is `C:\Modding\kitcraft\out` on this machine (the checkout has not been
moved to `C:\Modding\kitkraft` yet). `package\KitKraftGunOverhaul-0.1.0.zip` next to the
new zip is build 1's, left as it was; the package folder now holds build 2.

The plugin: `TES4` flags `0x00000001` (`.esm`, not localized, no small/medium bit), one
master `Starfield.esm`, 21 records in three top-level groups (WEAP 13, AMDL 4, OMOD 4), no
compressed records, every lstring of the copied records inlined as text. **Overrides
only:** no new forms, no leveled-list injection (the two ammo lists are pointed to, not
edited), no scripts. The plugin can be dropped from a save.

Why one master now: build 1 also had `OldSovietMarksmanRifle.esm` (the rename smoke test),
which won the hunting rifle in `KitKraft Dev`. The profile's `plugins.txt` and
`modlist.txt` still list it, but no folder under `C:\Modding\MO2-Starfield\mods` provides it
any more (checked 2026-10-15), so the load order skips it and the rifle's winner is
`Starfield.esm`. The overhaul sets the rifle's `FULL` itself, so it wins the name either way
as long as it loads after the smoke test.

### Engine change this build needed (step 1)

An OMOD override could not have its FormIDs remapped (and validate counted every OMOD
`DATA` as unchecked) because `has_opaque` treated every opaque byte and every union as
unknown layout. It now flags only unknown layout (a whole opaque payload, a rest-of-payload
run, an EditorID-prefix union); fixed-size untyped fields between typed neighbours
(`DATA.Unknown1`, `DATA.Unknown2`, the property padding) and sibling-decided unions are
known layout, never an lstring (the Resupply rule is about whole 4-byte payloads of unknown
layout). Whether such bytes could hold a FormID is checked on the value: a remap is refused
when they are 4 or more non-zero bytes. All 3,330 official OMODs have them zero (8,215
properties, 14 ESMs). Test: `edit::patch_tests::omod_overrides_of_localized_sources_copy_and_remap`
(an OMOD from a localized source copies with FULL inlined, a property edit and its FormIDs
remapped, byte-identical otherwise; a non-zero `Unknown2` refuses the remap; an untyped
4-byte subrecord is still refused as a possible string ID). This build itself needs no remap
(every record comes from `Starfield.esm`).

## Commands and results (release build of commit `c100a18`)

```
kit build docs\overhaul\overhaul.toml C:\Modding\kitcraft\out\overhaul\KitKraftGunOverhaul.esm --plan
  plan: 46 field change(s) on 21 record(s) ... (nothing written); validate passed
kit build docs\overhaul\overhaul.toml C:\Modding\kitcraft\out\overhaul\KitKraftGunOverhaul.esm --overwrite
  KitKraftGunOverhaul.esm: created (45271 bytes, masters Starfield.esm)
kit validate --plugin C:\Modding\kitcraft\out\overhaul\KitKraftGunOverhaul.esm --profile "C:\Modding\MO2-Starfield\profiles\KitKraft Dev"
  KitKraftGunOverhaul.esm: passed (0 error(s), 0 warning(s); rules: masters.missing, masters.order,
  formid.dangling, lstring.copied-id, override.stripped-name, cell.missing-parent, ref.orphan,
  ref.outside-block, ref.persistent-flag, ref.persistence, cell.actor-without-navmesh, cell.incc,
  navmesh.unchanged, record.flags-body)
  limitation: formid.dangling checked 1772 FormIDs; 77 subrecords are opaque or undecoded
  limitation: active plugins with no file (not loaded): OldSovietMarksmanRifle.esm
kit roundtrip C:\Modding\kitcraft\out\overhaul\KitKraftGunOverhaul.esm --verify
  KitKraftGunOverhaul.esm: identical (45271 bytes, 22 records, 3 groups, 0 compressed records)
kit package C:\Modding\kitcraft\out\overhaul\KitKraftGunOverhaul.esm C:\Modding\kitcraft\out\overhaul\package
    --profile "C:\Modding\MO2-Starfield\profiles\KitKraft Dev" --spec docs\overhaul\overhaul.toml --version 0.2.0 --overwrite
  KitKraftGunOverhaul: 2 file(s), 45271 bytes in `...\package\KitKraftGunOverhaul`,
  zip `...\package\KitKraftGunOverhaul-0.2.0.zip` (9784 bytes); validate passed (0 warning(s))
kit deploy --dev --dry-run C:\Modding\kitcraft\out\overhaul\package\KitKraftGunOverhaul C:\Modding\MO2-Starfield --profile "KitKraft Dev"
  deploy --dev --dry-run: REFUSED `KitKraft Dev - KitKraftGunOverhaul`
  target: C:\Modding\MO2-Starfield\mods\KitKraft Dev - KitKraftGunOverhaul
  files: 1 added, 0 replaced, 0 removed, 0 unchanged
  refusal: refused [live-profile] C:\Modding\MO2-Starfield\ModOrganizer.ini: MO2's active profile is
    `Default`, the owner's live game; switch MO2 to the `KitKraft Dev` profile first
  warning: mod `KitKraft Dev - KitKraftGunOverhaul` is not listed yet in profile `KitKraft Dev`
  warning: plugin `KitKraftGunOverhaul.esm` is not active in profile `KitKraft Dev`'s plugins.txt
  would touch (6): journal, staging folder, the plugin, kitkraft-deploy.json, the rename into
    `mods\KitKraft Dev - KitKraftGunOverhaul`, journal removal
  (exit 1: the refusal is the live-profile guard; nothing was written, `mods\` checked afterwards)
```

With the plugin appended to the load order, `kit inspect Starfield.esm:12D03F` shows it
winning (2 versions, `DATA.Properties[0].Value1` Int 5 in `Starfield.esm`, 2 in the
overhaul). Validate's count of subrecords whose FormIDs go unchecked is 77 (build 1: 307,
on 17 records); since step 1 a known layout whose untyped bytes are all zero counts as
checked.

## How the brief maps to records

As in build 1 (`build-1.md`, "How the brief maps to records"): damage is
`WEAP.WDMG.BaseDamage`, capacity `WEAP.WAM2.AmmoCapacity` plus the magazine OMOD's `ACPT`
(Int, ADD), fire rate the three `WFIR` members together, recoil the aim model's
`ANAM.RecoilDegreesPerShotX/Y`, spin-up `WFIR.FireSeconds`, ammo `WAM2.AmmoType` and
`WAM2.AmmoList`. New: the default receivers of the Maelstrom, Lawgiver and Drum Beat set
`FORF` (Override Rate of Fire, Bool, SET true) and `WSPS` (Shots per Second, Float, SET),
so on a stock gun their `WSPS` replaces the WEAP rate; build 2 scales it by the WEAP's
factor. The property index `n` of each value was read from the record (`kit inspect`), and
the fixture test pins every before value, so a record that moved its properties would fail
the test rather than write the wrong one.

## Per row

| Row | Status | Records | Member(s) written | Before -> after |
| :-- | :-- | :-- | :-- | :-- |
| B1 | implemented | WEAP `26D96C` M1919 (Old Earth Pistol), `24561C` XM2311 | `WAM2.AmmoCapacity`; `WDMG.BaseDamage` x0.85 | 9 -> 7 (both); 28 -> 23.8, 36 -> 30.6 |
| B2 | implemented | WEAP `21BBCD` RussianHuntingRifle | `FULL` | "Old Earth Hunting Rifle" (string `0x0002E469`) -> "Old Earth Marksman Rifle" |
| B3 | implemented | WEAP `2984DF` Maelstrom; AMDL `1B01CA` Maelstrom_FA_AM; OMOD `077F13` default receiver | `WAM2.AmmoCapacity`; `WDMG.BaseDamage` x1.35; `WFIR` rate x0.75; `ANAM.RecoilDegreesPerShotX/Y` x1.5; `DATA.Properties[1].Value1.Float` (WSPS) x0.75 | 40 -> 24; 4 -> 5.4; 7.595 -> 5.696 shots/s; X 0.1 -> 0.15, Y 0.3 -> 0.45; receiver 7.5 -> 5.625 shots/s |
| B4 | implemented | WEAP `21FEB4` Kraken | `WAM2.AmmoType`; `WAM2.AmmoList`; `WDMG.BaseDamage` x1.2 | 6.5mm CT `2B5590` -> 7.77mm Caseless `04AD3E`; `12F0CE` -> `00E841`; 3 -> 3.6 |
| B5 | implemented | WEAP `26D96D` UrbanEagle | `WFIR` rate x1.75 | 2.5 -> 4.375 shots/s; delay 0.4 -> 0.2286 s |
| B6 | implemented | WEAP `02D7F4` Lawgiver; OMOD `057555` default receiver | `WFIR` rate x1.5625; `DATA.Properties[1].Value1.Float` (WSPS) x1.5625 | 1.0 -> 1.5625 shots/s, delay 1.0 -> 0.64 s; receiver 1.0 -> 1.5625 shots/s |
| B7 | implemented | WEAP `02EB36` Tombstone; AMDL `11287A` Tombstone_FA_AM | `WDMG.BaseDamage` x1.25; `ANAM.RecoilDegreesPerShotX/Y` x1.5 | 19 -> 23.75; X 0.175 -> 0.2625, Y 0.375 -> 0.5625 |
| B8 | dropped (owner, 2026-10-15) | none | none | no damage property on the automatic receivers (below) |
| B9 | implemented | WEAP `04716C` Beowulf | `WAM2.AmmoType`; `WAM2.AmmoList`; `WAM2.AmmoCapacity` | 7.77mm Caseless -> 6.5mm CT; `00E841` -> `12F0CE`; 30 -> 20 |
| B10 | implemented | OMOD `12D03F` mod_HardTarget_Mag_L | `DATA.Properties[0].Value1.Int` (ACPT, ADD) | 5 -> 2: total 10 -> 7 on the base 5 |
| B11 | implemented | WEAP `18DE2C` DrumBeat; AMDL `179DBD`, `2BF29F`; OMOD `04A750` default receiver | `WAM2.AmmoCapacity`; `WDMG.BaseDamage` x1.5; `WFIR` rate x0.75; recoil X/Y x1.6 (both aim models); `DATA.Properties[1].Value1.Float` (WSPS) x0.75 | 30 -> 18; 8 -> 12; 16 -> 12 shots/s; SA X 0.185 -> 0.296, Y 0.32 -> 0.512; FA X 0.15 -> 0.24, Y 0.4 -> 0.64; receiver 16 -> 12 shots/s |
| B12 | implemented | WEAP `26D960` Shotty; OMOD `132C4A`, `132C49`, `132C48` (class only) | `WAM2.AmmoCapacity` | 12 -> 6; magazines unchanged: Large 6 + 8 = 14, Extended 6 + 4 = 10, Extra Large 6 + 16 = 22 |
| B13 | implemented | WEAP `0547A3` Breach | `WFIR` rate x1.5 | 1.0 -> 1.5 shots/s; delay 1.0 -> 0.667 s (-33%) |
| B14 | implemented | WEAP `0546CD` Microgun | `WFIR.FireSeconds` x0.1; `WDMG.BaseDamage` x1.5 | 1.0 -> 0.1 s; 9 -> 13.5 |

`WFIR` rate = `ShotsPerSecond`, `AttackDelaySeconds` and `AttackAnimationDuration`
together. Per-row change counts (fields / records): B1 4/2, B2 1/1, B3 8/3, B4 3/1, B5 3/1,
B6 4/2, B7 3/2, B8 0/0, B9 3/1, B10 1/1, B11 10/4, B12 1/1, B13 3/1, B14 2/1; every
written field changed. Each OMOD override differs from its `Starfield.esm` record only in the
4 bytes its rule wrote and in `FULL`/`DESC` written as inline text (the fixture test checks
it byte for byte).

### Notes per row

- **B8 (dropped).** The owner dropped the row on 2026-10-15 (brief.md row B8, C8). For the
  record, what the six automatic receivers carry (`DATA.Properties`, from the records;
  function, value; every one also includes the template `1341F4`):

  | Receiver | Properties |
  | :--- | :--- |
  | Grendel `0191EF` | FORF Bool SET true; WSPS Float SET 17; WAIM FormIdInt SET `1A26BD`; WTMC Int ADD -1; WKEY FormIdInt ADD `374EFA` |
  | Beowulf `13136D` | FORF Bool SET true; WSPS Float SET 15; WAIM FormIdInt SET `1A26C0` |
  | Maelstrom `077F13` | FORF Bool SET true; WSPS Float SET 7.5 (5.625 in build 2); WAIM FormIdInt SET `1B01CA`; WTMC Int ADD -1; WKEY FormIdInt ADD `374EFA` |
  | Drum Beat `04A750` | FORF Bool SET true; WSPS Float SET 16 (12 in build 2); WAIM FormIdInt SET `2BF29F`; WTMC Int ADD -1; WKEY FormIdInt ADD `374EFA` |
  | AA-99 `08E0FD` | FORF Bool SET true; WSPS Float SET 11.2; WAIM FormIdInt SET `225C4A`; WKEY FormIdInt REMOVE `14210A`; WTMC Int ADD -1; WKEY FormIdInt ADD `374EFA` |
  | Kodama `04A7AC` | FORF Bool SET true; WSPS Float SET 17; WAIM FormIdInt SET `1A26BC`; WTMC Int ADD -1; WENC FormIdInt ADD `12D050` (step -215.89); WKEY FormIdInt ADD `3BC136`, `374EFA`, `1283D2` |
  | template `1341F4` (_Template_mod_Weapon_Receiver_FullAuto) | FTYP Enum SET 2; GVAL Float MUL+ADD 0.1; WKEY FormIdInt ADD `14210A`, `0A191C`, `04A0A2`; WACV FormIdFloat ADD `26AD4E` (value 1.0) |

  None is a damage property (`WDMG` or another damage code), so semi- and full-auto
  receivers of one weapon give the same `WDMG`. A lever, if the row comes back: a damage
  property added to these receivers (a new 24-byte entry and `PropertyCount` + 1, which
  the edit paths cannot do yet), or a per-weapon `WDMG.BaseDamage` change.
- **B11.** Owner (2026-10-15): "18 rounds" is the base magazine, the drum untouched. So a
  default Drum Beat (built with the Drum Magazine `087D4B`, `ACPT` ADD 30) holds 18 + 30 =
  **48** (vanilla 60). The Small Magazine `087D4A` (ADD -15) would hold 18 - 15 = **3**
  (vanilla 15); it is not a default part. The semi-auto receiver `13382F` (WSPS SET 6) is
  not changed.
- **B12.** Owner (2026-10-15): keep as built. Base 6; the Large Magazine `132C4A` keeps its
  vanilla ADD +8, which already totals the row's 14, so no override of it is written (it
  would equal its master); Extended `132C49` totals 10 and Extra Large `132C48` 22.
- **B3, B6.** Only the default receivers are changed; the Maelstrom's semi-auto receiver
  `068A1F` (WSPS SET 3) keeps its rate. B6's factor stays x1.5625 (x1.5 gives a 0.667 s
  delay, above the row's "under 0.65 seconds"; 0.64 s meets both numbers).

## In game (owner, M3.O5)

Steps for the owner, on the **`KitKraft Dev` profile only**, with a **disposable save**.
Never on `Default` (AGENTS.md, "Owner's live game is off limits").

1. In MO2 switch to the `KitKraft Dev` profile (the deploy dry run above was refused
   because MO2's active profile is `Default`; that guard is expected). If build 1 was
   installed by hand, disable or remove that copy first.
2. Install the plugin, either way:
   - `kit deploy --dev C:\Modding\kitcraft\out\overhaul\package\KitKraftGunOverhaul C:\Modding\MO2-Starfield --profile "KitKraft Dev"`
     (writes only `mods\KitKraft Dev - KitKraftGunOverhaul\`), then in MO2 press F5; or
   - *Install a new mod from an archive* with
     `C:\Modding\kitcraft\out\overhaul\package\KitKraftGunOverhaul-0.2.0.zip`, named
     `KitKraft Dev - KitKraftGunOverhaul` (`docs/packaging.md`).
3. Enable the mod `KitKraft Dev - KitKraftGunOverhaul`; in the plugin list enable
   `KitKraftGunOverhaul.esm` and place it **after `OldSovietMarksmanRifle.esm`** (last in
   the list is fine). The smoke test is still listed in the profile but its mod folder is
   gone, so MO2 may not show it; if it is reinstalled, the overhaul must still load after
   it so that the rifle shows "Old Earth Marksman Rifle".
4. Launch Starfield through MO2 on that profile, load (or start) a **disposable save**, and
   in the console (`~`) add each weapon:

   | Weapon | Console |
   | :--- | :--- |
   | Old Earth Pistol | `player.additem 0026D96C 1` |
   | XM-2311 | `player.additem 0024561C 1` |
   | Old Earth Marksman Rifle (was Old Earth Hunting Rifle) | `player.additem 0021BBCD 1` |
   | Maelstrom | `player.additem 002984DF 1` |
   | Kraken | `player.additem 0021FEB4 1` |
   | Urban Eagle | `player.additem 0026D96D 1` |
   | Lawgiver | `player.additem 0002D7F4 1` |
   | Tombstone | `player.additem 0002EB36 1` |
   | Beowulf | `player.additem 0004716C 1` |
   | Hard Target | `player.additem 000546CC 1` |
   | Drum Beat | `player.additem 0018DE2C 1` |
   | Shotty | `player.additem 0026D960 1` |
   | Breach | `player.additem 000547A3 1` |
   | Microgun | `player.additem 000546CD 1` |
   | 7.77mm Caseless, 6.5mm CT, 11mm Caseless | `player.additem 0004AD3E 300`, `player.additem 002B5590 300`, `player.additem 002B5595 300` |

   The runtime IDs equal the FormIDs because `Starfield.esm` loads first (lane D's
   `runtime-slots-by-scale` assumption; not yet observed in game). An `additem` weapon is
   built from the default object template, so it carries the default receiver and
   magazine named above. C10 and C12's larger magazines are attached at a weapon workbench;
   if a mod is not available there yet, mark that check "not checked" rather than failed.
5. Fill the checklist below (screenshots to `observe\<yyyy-mm-dd>-O5\` if possible), then
   disable the mod. The plugin owns no forms and adds no scripts, so dropping it from the
   save is safe.

### Checklist (C1..C14)

| Check | Row | Expected (brief) | Build 2 should show | Seen | Pass |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **C1** | B1 | Old Earth Pistol and XM-2311: 7-round base capacity, 15% lower base damage | 7 rounds with the standard magazine; damage 15% below vanilla | | |
| **C2** | B2 | `Old Earth Marksman Rifle` in inventory and workbench | "Old Earth Marksman Rifle" (an INNR prefix on a modded instance is still a pass) | | |
| **C3** | B3 | Maelstrom 24 rounds, 25% slower, 35% more damage, stronger vertical kick | 24 rounds, +35% damage, stronger kick, about 5.6 shots/s (vanilla 7.5) | | |
| **C4** | B4 | Kraken uses `7.77mm Caseless`, 20% more damage | 7.77mm Caseless, +20% damage | | |
| **C5** | B5 | Urban Eagle ~3.5+ shots/s, no 1-second lockout | about 4.4 shots/s on repeated pulls | | |
| **C6** | B6 | Lawgiver cycles in under 0.65 s | about 0.64 s between aimed shots (1.5625 shots/s) | | |
| **C7** | B7 | Tombstone +25% damage, 50% stronger kick | +25% damage, 50% stronger kick | | |
| **C8** | B8 | dropped (owner, 2026-10-15) | nothing to check | | n/a |
| **C9** | B9 | Beowulf uses `6.5mm CT` (or `11mm Caseless`), 20 rounds | 6.5mm CT, 20 rounds with the standard magazine | | |
| **C10** | B10 | Hard Target with large magazine holds 7 | 5 standard, 7 with the Large Magazine (vanilla 10) | | |
| **C11** | B11 | Drum Beat 18 rounds, 25% slower, +50% damage, 60% more kick | 48 rounds with the default drum (base 18 + 30; the owner's reading), 12 shots/s (vanilla 16), +50% damage, 60% more kick | | |
| **C12** | B12 | Shotty 6 rounds standard, 14 with the extended magazine | 6 standard; Large 14, Extended 10, Extra Large 22 | | |
| **C13** | B13 | Breach follow-ups 50% faster | 1.5 shots/s, 0.67 s between shots | | |
| **C14** | B14 | Microgun fires almost at once, +50% damage | spin-up about 0.1 s (if `FireSeconds` is the spin-up), +50% damage | | |

## Limitations

- **Receivers.** Only the default receivers' rate is changed (B3, B6, B11); the
  alternate receivers keep vanilla rates (Maelstrom semi-auto `068A1F` WSPS 3, Drum Beat
  semi-auto `13382F` WSPS 6), so a gun refitted with one fires at vanilla speed. Whether
  `FORF`/`WSPS` fully decide the in-game rate (and the WEAP `WFIR` values only apply
  without such a receiver) is inferred from the records, not observed (C3, C6, C11).
- **Property indices.** The rules name the property by its index (`Properties[1]`,
  `Properties[0]`), read from the records; a later plugin that reorders those OMODs'
  properties would need the spec updated (the spec's `where` cannot select a property by
  its code). The fixture test fails if a before value changes.
- **Drum Beat magazines.** With the base at 18 the Small Magazine holds 3 (ADD -15); a
  consequence of the owner's reading, not a rule.
- **Recoil feel.** Only the per-shot recoil degrees (X and Y) are scaled; how the game's
  felt kick follows them is not verified.
- **Damage.** `WDMG.BaseDamage` is the base the inventory shows before quality tiers; the
  quality-tier OMODs (MUL+ADD `WDMG`) still apply. B1's floor (15) is not reached.
- **Spin-up.** `WFIR.FireSeconds` as the Microgun's spin-up is inferred from the corpus,
  not observed. `WCHG` is not changed.
- **Re-chambering.** The Kraken and Beowulf keep their own `WAM2.OverrideProjectile`; NPCs
  carrying them get the swapped ammo lists.
- **Scope.** Base weapons only: the NPC-only variants (`Crew_Elite_*`,
  `Companion_Lawgiver`) and the research-menu dummies keep vanilla values, except that
  `Crew_Elite_Tombstone` shares Tombstone's aim model (B7 recoil).
- **Unverified in game.** Every value is checked in the records (plan, validate,
  roundtrip, the fixture test); none is observed in game yet (M3.O5). The runtime IDs
  follow lane D's `runtime-slots-by-scale` assumption.
