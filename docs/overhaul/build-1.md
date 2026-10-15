# Gun overhaul build 1 (M3.E9)

The first real overhaul build: the owner's brief (`docs/overhaul/brief.md`, rows B1..B14)
translated into `docs/overhaul/overhaul.toml` (spec v2, every class and rule naming its
row) and built on 2026-10-14 against the owner's `KitKraft Dev` profile. The plan, as
`kit build --plan` prints it: `docs/overhaul/build-1-plan.md`. The spec is checked by
`crates/engine/tests/overhaul_build.rs` on a fixture of the real records.

**Summary.** 42 field changes on 17 records, all overrides of `Starfield.esm` forms (13 WEAP,
4 AMDL). 7 rows implemented, 4 partial, 3 open (B8 has no target in the data; B10 and
the open parts of B3, B6, B11 and B12 need OMOD property values, which the schema does not
type yet).

## Artifacts

| File | Size | SHA-256 |
| :--- | ---: | :--- |
| `C:\Modding\kitkraft\out\overhaul\KitKraftGunOverhaul.esm` | 43,955 | `a44d49bb0dc68941183185823231e0d887dfed30a9b0577504b4374a95342aae` |
| `C:\Modding\kitkraft\out\overhaul\package\KitKraftGunOverhaul-0.1.0.zip` | 9,288 | `c66a142672769e2dd37c7b54100144b5ad59c53fed6d254a094d745c6658d86c` |
| `...\package\KitKraftGunOverhaul\kitkraft-manifest.json` | | `6ca9ef01314fb21a0ca88f446c13a0393b140b474bb4033bd0e0af80747575e5` |
| `docs/overhaul/overhaul.toml` (as packaged, commit `d75f646`) | 11,843 | `d29d48a34067d2f50269980950bc75aa4188bbcfda4869d32a64290c65723f8f` |

The plugin: `TES4` flags `0x00000001` (`.esm`, not localized, no small/medium bit), masters
`Starfield.esm` and `OldSovietMarksmanRifle.esm`, 17 records in two top-level groups
(WEAP 13, AMDL 4), no compressed records, every lstring of the copied WEAP records inlined
as text. **Overrides only:** no new forms (every record is an existing `Starfield.esm`
form), no leveled-list injection (the two ammo lists are pointed to, not edited), no
scripts. The plugin can be dropped from a save.

Why the second master: in `KitKraft Dev` the Old Earth Hunting Rifle's winning version is
the rename smoke test (`OldSovietMarksmanRifle.esm`), so `build` copies that version and
lists its plugin as a master, which also keeps the overhaul loading after it. Built
against a load order without the smoke test, the only master would be `Starfield.esm`
(the fixture test shows it). If the smoke test is removed from the profile, rebuild.

## Commands and results (release build of commit `0d0d255`)

```
kit build docs\overhaul\overhaul.toml C:\Modding\kitkraft\out\overhaul\KitKraftGunOverhaul.esm --plan
  plan: 42 field change(s) on 17 record(s) ... (nothing written); validate passed
kit build docs\overhaul\overhaul.toml C:\Modding\kitkraft\out\overhaul\KitKraftGunOverhaul.esm
  KitKraftGunOverhaul.esm: created (43955 bytes, masters Starfield.esm, OldSovietMarksmanRifle.esm)
kit validate --plugin C:\Modding\kitkraft\out\overhaul\KitKraftGunOverhaul.esm --profile "C:\Modding\MO2-Starfield\profiles\KitKraft Dev"
  KitKraftGunOverhaul.esm: passed (0 error(s), 0 warning(s); rules: masters.missing, masters.order,
  formid.dangling, lstring.copied-id, override.stripped-name, record.flags-body)
  limitation: formid.dangling checked 1725 FormIDs; 307 subrecords are opaque or undecoded
kit roundtrip C:\Modding\kitkraft\out\overhaul\KitKraftGunOverhaul.esm --verify
  KitKraftGunOverhaul.esm: identical (43955 bytes, 18 records, 2 groups, 0 compressed records)
kit package C:\Modding\kitkraft\out\overhaul\KitKraftGunOverhaul.esm C:\Modding\kitkraft\out\overhaul\package
    --profile "C:\Modding\MO2-Starfield\profiles\KitKraft Dev" --spec docs\overhaul\overhaul.toml --version 0.1.0
  KitKraftGunOverhaul: 2 file(s), 43955 bytes in `...\package\KitKraftGunOverhaul`,
  zip `...\package\KitKraftGunOverhaul-0.1.0.zip` (9288 bytes); validate passed (0 warning(s))
kit deploy --dev --dry-run C:\Modding\kitkraft\out\overhaul\package\KitKraftGunOverhaul C:\Modding\MO2-Starfield --profile "KitKraft Dev"
  deploy --dev --dry-run: REFUSED `KitKraft Dev - KitKraftGunOverhaul`
  target: C:\Modding\MO2-Starfield\mods\KitKraft Dev - KitKraftGunOverhaul
  files: 1 added, 0 replaced, 0 removed, 0 unchanged
  refusal: refused [live-profile] C:\Modding\MO2-Starfield\ModOrganizer.ini: MO2's active profile is
    `Default`, the owner's live game; switch MO2 to the `KitKraft Dev` profile first
  warning: mod `KitKraft Dev - KitKraftGunOverhaul` is not listed yet in profile `KitKraft Dev`
  warning: plugin `KitKraftGunOverhaul.esm` is not active in profile `KitKraft Dev`'s plugins.txt
  would touch (6):
    write C:\Modding\MO2-Starfield\mods\KitKraft Dev - KitKraftGunOverhaul.kitkraft-journal.json
    create-dir C:\Modding\MO2-Starfield\mods\KitKraft Dev - KitKraftGunOverhaul.staging
    write C:\Modding\MO2-Starfield\mods\KitKraft Dev - KitKraftGunOverhaul.staging\KitKraftGunOverhaul.esm
    write C:\Modding\MO2-Starfield\mods\KitKraft Dev - KitKraftGunOverhaul.staging\kitkraft-deploy.json
    rename C:\Modding\MO2-Starfield\mods\KitKraft Dev - KitKraftGunOverhaul.staging -> C:\Modding\MO2-Starfield\mods\KitKraft Dev - KitKraftGunOverhaul
    remove-file C:\Modding\MO2-Starfield\mods\KitKraft Dev - KitKraftGunOverhaul.kitkraft-journal.json
  (exit 1: the refusal is the live-profile guard; nothing was written, `mods\` checked afterwards)
```

With the plugin appended to the load order, `kit inspect` shows it winning: the rifle
(`Starfield.esm:21BBCD`, 3 versions) and the Maelstrom (`2984DF`, `WAM2.AmmoCapacity` 24,
`WFIR.ShotsPerSecond` 5.696).

## How the brief maps to records

What the records say, found with `kit inspect` on the default object template of each
weapon (its OMODs and their property lists, decoded from the `DATA` bytes against the
pinned xEdit property list, `wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L6894-L6938`
and its WEAP property names `#L2995-L3179`):

- **Damage** is `WEAP.WDMG.BaseDamage` (typed, `corpus`). No default OMOD of the 14
  weapons sets it; only the quality-tier OMODs scale it (MUL+ADD `WDMG`). `DAMA` is
  `Amount` 1 on every target and the three AMMO records have `DNAM.Damage` 0, so neither
  is the per-shot number.
- **Magazine capacity** is `WEAP.WAM2.AmmoCapacity`. Standard magazines carry no capacity
  property; larger magazines **add** to it (`ACPT`, Int, ADD) through OMOD property values.
- **Fire rate.** In 447 of the 450 official WEAP records `WFIR.ShotsPerSecond` x
  `WFIR.AttackDelaySeconds` = 1, and `WFIR.AttackAnimationDuration` is a fixed multiple of
  the delay (1x in 205 records, 20x in 103, ...), so a fire-rate factor r is written as
  ShotsPerSecond x r, AttackDelaySeconds / r, AttackAnimationDuration / r. But several
  default receivers **set** the rate themselves (`FORF` Override Rate of Fire = true and
  `WSPS` Shots per Second, Float, SET): Maelstrom `077F13` (7.5), Lawgiver `057555` (1.0),
  Drum Beat `04A750` (16), also Beowulf, Shotty, Grendel, Kodama, AA-99. For those, the
  WEAP values are overridden on every instance with that receiver. Urban Eagle (`068A23`)
  and Breach (`090A40`) receivers set no rate, so their WEAP values apply.
- **Recoil kick** is `AMDL.ANAM.RecoilDegreesPerShotX` and `...Y` (xEdit's "Recoil -
  Degrees Per Shot X/Y", `#L9399-L9400`; typed, `corpus`), on the aim model the weapon or
  its default receiver uses (`WAIM`, FormIdInt, SET). Each target AMDL is used only by that
  weapon and its receivers (a scan of the WEAP and OMOD records of the 14 official ESMs and
  every plugin in `C:\Modding\MO2-Starfield\mods`); Tombstone's is shared with its NPC
  variant `Crew_Elite_Tombstone`. Other recoil fields (spring force, arc, hip mult, base
  stability) are left as they are.
- **Spin-up.** `WFIR.FireSeconds` is 1.0 on the Microgun and about 1.2e-7 on every
  ordinary gun; it is non-zero only on wind-up weapons (grenades and mines, creature
  ranged attacks, the Shattered Space particle machine guns, Novablast, MagSniper,
  AutoRivet): 62 of 541 winning WEAP in `KitKraft Dev`. xEdit's OMOD property name for it
  is "Fire (Seconds)" (`WFSC`, `#L3143`). That it is the spin-up delay is inferred, not
  observed (C14 checks it).
- **Ammo.** `WAM2.AmmoType` (the chambered AMMO) and `WAM2.AmmoList` (the per-weapon NPC
  ammo list, LVLI). Re-chambering swaps both: the Kraken and Beowulf lists have the same
  shape (one entry pointing to `LL_Ammo_777mm` `279280` or `LL_Ammo_65mm` `279283`), so the
  two weapons trade lists.
- **OMOD property values are opaque** in the schema (`ObjectModProperty.Value1/Value2`,
  `crates/schema/defs/common/ObjectModProperty.toml`): xEdit types them by a union decided
  by the sibling `ValueType` (`#L1981-L2031`), and the schema format can only decide a
  union by the EditorID's first letter (`docs/gaps.md`, unions). A "narrow typed union" is
  therefore not a TOML-only addition to `OMOD.toml`: it needs a sibling-field decider in
  the loader, codegen, `kitkraft_doc::typed` and `edit::fields` (lane B, M3.B9; filed in
  `plan/STATUS.md`, Requests). This build does not write OMOD values as raw `hex:`.

No member needed `allow_reference`: every member written is `corpus` confidence.

## Per row

| Row | Status | Records | Member(s) written | Before -> after |
| :-- | :-- | :-- | :-- | :-- |
| B1 | implemented | WEAP `26D96C` M1919 (Old Earth Pistol), `24561C` XM2311 | `WAM2.AmmoCapacity`; `WDMG.BaseDamage` x0.85 | 9 -> 7 (both); 28 -> 23.8, 36 -> 30.6 |
| B2 | implemented | WEAP `21BBCD` RussianHuntingRifle | `FULL` | "Old Soviet Marksman Rifle" (smoke test; vanilla "Old Earth Hunting Rifle") -> "Old Earth Marksman Rifle" |
| B3 | partial | WEAP `2984DF` Maelstrom; AMDL `1B01CA` Maelstrom_FA_AM | `WAM2.AmmoCapacity`; `WDMG.BaseDamage` x1.35; `WFIR` rate x0.75; `ANAM.RecoilDegreesPerShotX/Y` x1.5 | 40 -> 24; 4 -> 5.4; 7.595 -> 5.696 shots/s (delay 0.1317 -> 0.1756 s); X 0.1 -> 0.15, Y 0.3 -> 0.45 |
| B4 | implemented | WEAP `21FEB4` Kraken | `WAM2.AmmoType`; `WAM2.AmmoList`; `WDMG.BaseDamage` x1.2 | 6.5mm CT `2B5590` -> 7.77mm Caseless `04AD3E`; LLW_Ammo_Kraken `12F0CE` -> LLW_Ammo_Beowulf `00E841`; 3 -> 3.6 |
| B5 | implemented | WEAP `26D96D` UrbanEagle | `WFIR` rate x1.75 | 2.5 -> 4.375 shots/s (min 3.5 met); delay 0.4 -> 0.2286 s |
| B6 | partial | WEAP `02D7F4` Lawgiver | `WFIR` rate x1.5625 | 1.0 -> 1.5625 shots/s; delay 1.0 -> 0.64 s |
| B7 | implemented | WEAP `02EB36` Tombstone; AMDL `11287A` Tombstone_FA_AM | `WDMG.BaseDamage` x1.25; `ANAM.RecoilDegreesPerShotX/Y` x1.5 | 19 -> 23.75; X 0.175 -> 0.2625, Y 0.375 -> 0.5625 |
| B8 | open | OMOD `0191EF`, `13136D`, `077F13`, `04A750`, `08E0FD`, `04A7AC` (class only) | none | no damage property to change |
| B9 | implemented | WEAP `04716C` Beowulf | `WAM2.AmmoType`; `WAM2.AmmoList`; `WAM2.AmmoCapacity` | 7.77mm Caseless `04AD3E` -> 6.5mm CT `2B5590`; `00E841` -> `12F0CE`; 30 -> 20 |
| B10 | open | OMOD `12D03F` mod_HardTarget_Mag_L (class only) | none | needs `ACPT` ADD 5 -> 2 (total 10 -> 7) |
| B11 | partial | WEAP `18DE2C` DrumBeat; AMDL `179DBD` Drumbeat_SA_AM, `2BF29F` Drumbeat_FA_AM | `WAM2.AmmoCapacity`; `WDMG.BaseDamage` x1.5; `WFIR` rate x0.75; `ANAM.RecoilDegreesPerShotX/Y` x1.6 (both aim models) | 30 -> 18; 8 -> 12; 16 -> 12 shots/s (delay 0.0625 -> 0.0833 s); SA X 0.185 -> 0.296, Y 0.32 -> 0.512; FA X 0.15 -> 0.24, Y 0.4 -> 0.64 |
| B12 | partial | WEAP `26D960` Shotty; OMOD `132C4A`, `132C49`, `132C48` (class only) | `WAM2.AmmoCapacity` | 12 -> 6 |
| B13 | implemented | WEAP `0547A3` Breach | `WFIR` rate x1.5 | 1.0 -> 1.5 shots/s; delay 1.0 -> 0.667 s (-33%) |
| B14 | implemented | WEAP `0546CD` Microgun | `WFIR.FireSeconds` x0.1; `WDMG.BaseDamage` x1.5 | 1.0 -> 0.1 s; 9 -> 13.5 |

`WFIR` rate = `ShotsPerSecond`, `AttackDelaySeconds` and `AttackAnimationDuration`
together (above). Per-class change counts (fields changed / records): B1 4/2, B2 1/1,
B3 7/2, B4 3/1, B5 3/1, B6 3/1, B7 3/2, B8 0/0, B9 3/1, B10 0/0, B11 9/3, B12 1/1, B13 3/1,
B14 2/1; every written field changed.

### Why partial or open

- **B3 (fire rate).** The Maelstrom's default receiver `077F13`
  (mod_Maelstrom_Receiver_FullAuto_Standard) sets `FORF` = true and `WSPS` = 7.5, so the
  WEAP rate written here is overridden on a default instance. Closing it: `WSPS` 7.5 ->
  5.625 on `077F13` (and 3 -> 2.25 on the semi-auto receiver `068A1F` if the owner wants
  every receiver slowed). Capacity, damage and recoil are done.
- **B6.** The Lawgiver's default receiver `057555` sets `FORF` = true and `WSPS` = 1.0;
  closing it: `WSPS` 1.0 -> 1.5625. Factor: x1.5 gives a 0.667 s delay, above the row's
  "under 0.65 seconds", so the spec uses x1.5625 (0.64 s), which meets both numbers.
- **B8.** The six automatic receivers set `FORF`, `WSPS`, `WAIM`, `WTMC` and keywords;
  their shared template `1341F4` (_Template_mod_Weapon_Receiver_FullAuto) sets `FTYP`
  (firing type automatic), `GVAL` (value, MUL+ADD 0.1), keywords and an actor value
  `0026AD4E` (ADD 1). None has a damage property, so there is no per-shot penalty to
  reduce: semi-auto and full-auto receivers of the same weapon give the same `WDMG`. The
  row needs the owner's restatement (for instance, a damage factor on the automatic
  receivers, which would add a `WDMG` MUL+ADD property to an OMOD, or a base-damage change
  per weapon). The class `b8_automatic_receivers` lists the six records.
- **B10.** `mod_HardTarget_Mag_L` adds capacity with `ACPT` (Int, ADD) = 5 on a base of 5;
  the target total 7 needs ADD 2: an OMOD property value.
- **B11 (fire rate, drum).** The default full-auto receiver `04A750` sets `WSPS` = 16
  (closing: 16 -> 12; the semi-auto receiver `13382F` sets 6). The default Drum Magazine
  `087D4B` adds `ACPT` 30, so a default Drum Beat holds 18 + 30 = 48 after this build
  (vanilla 60); "18 rounds" with the drum needs that ADD at 0 (and the small magazine
  `087D4A`, ADD -15, would then hold 3: the owner should say which magazine the 18 means).
  Capacity, damage and both aim models' recoil are done.
- **B12.** The larger magazines add `ACPT` (Int, ADD): Large `132C4A` +8, Extended
  `132C49` +4, Extra Large `132C48` +16. With the base at 6 they hold 14, 10 and 22. The
  Large Magazine already meets "14"; halving every magazine (vanilla 20/16/28 -> 10/8/14)
  needs +4, +2, +8, so the Extra Large one reaches 14. Which reading the row means is the
  owner's; either way the rest is OMOD values.

Every open part names the OMOD property to change; the values are in the plan's classes
(`b8_automatic_receivers`, `b10_hard_target_large_mag`, `b12_shotty_magazines`) and above.
Once lane B types `ObjectModProperty.Value1` for `ValueType` Int and Float, they become
rules on those classes (or on the receivers by EditorID) with no other change.

## In game (owner, M3.O5)

Steps for the owner, on the **`KitKraft Dev` profile only**, with a **disposable save**.
Never on `Default` (AGENTS.md, "Owner's live game is off limits").

1. In MO2 switch to the `KitKraft Dev` profile (the deploy dry run above was refused
   because MO2's active profile is `Default`; that guard is expected).
2. Install the plugin, either way:
   - `kit deploy --dev C:\Modding\kitkraft\out\overhaul\package\KitKraftGunOverhaul C:\Modding\MO2-Starfield --profile "KitKraft Dev"`
     (writes only `mods\KitKraft Dev - KitKraftGunOverhaul\`), then in MO2 press F5; or
   - *Install a new mod from an archive* with
     `C:\Modding\kitkraft\out\overhaul\package\KitKraftGunOverhaul-0.1.0.zip`, named
     `KitKraft Dev - KitKraftGunOverhaul` (`docs/packaging.md`).
3. Enable the mod `KitKraft Dev - KitKraftGunOverhaul`; in the plugin list enable
   `KitKraftGunOverhaul.esm` and place it **after `OldSovietMarksmanRifle.esm`** (last in
   the list is fine: it is a master of the overhaul, and the overhaul must win the
   records it changes). Optionally check the files against the manifest (`docs/packaging.md`).
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
   | Grendel, AA-99, Kodama (B8 comparison) | `player.additem 00028A02 1`, `player.additem 002BF65B 1`, `player.additem 00253A16 1` |
   | 7.77mm Caseless, 6.5mm CT, 11mm Caseless | `player.additem 0004AD3E 300`, `player.additem 002B5590 300`, `player.additem 002B5595 300` |

   The runtime IDs equal the FormIDs because `Starfield.esm` loads first (lane D's
   `runtime-slots-by-scale` assumption; not yet observed in game). Shells for the
   Shotty and Breach and the other calibers come from the weapons' own ammo; add them the
   same way if needed. An `additem` weapon is built from the default object template, so it
   carries the default receivers and magazines named above.
5. Fill the checklist below (screenshots to `observe\<yyyy-mm-dd>-O5\` if possible), then
   disable the mod. The plugin owns no forms and adds no scripts, so dropping it from the
   save is safe.

### Checklist (C1..C14)

"Build 1 should show" is what this build can deliver; where it differs from the brief, the
row is partial or open (above).

| Check | Row | Expected (brief) | Build 1 should show | Seen | Pass |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **C1** | B1 | Old Earth Pistol and XM-2311: 7-round base capacity, 15% lower base damage | 7 rounds with the standard magazine; damage 15% below vanilla | | |
| **C2** | B2 | `Old Earth Marksman Rifle` in inventory and workbench | "Old Earth Marksman Rifle" (an INNR prefix such as "Elemental" on a modded instance is still a pass) | | |
| **C3** | B3 | Maelstrom 24 rounds, 25% slower, 35% more damage, stronger vertical kick | 24 rounds, +35% damage, stronger kick; fire rate probably unchanged (receiver `077F13`) | | |
| **C4** | B4 | Kraken uses `7.77mm Caseless`, 20% more damage | 7.77mm Caseless, +20% damage | | |
| **C5** | B5 | Urban Eagle ~3.5+ shots/s, no 1-second lockout | about 4.4 shots/s on repeated pulls | | |
| **C6** | B6 | Lawgiver cycles in under 0.65 s | probably unchanged (receiver `057555` sets 1 shot/s); if it changes, 0.64 s | | |
| **C7** | B7 | Tombstone +25% damage, 50% stronger kick | +25% damage, 50% stronger kick | | |
| **C8** | B8 | Automatic receivers lose 25% less damage than vanilla | no change (open: no such property) | | |
| **C9** | B9 | Beowulf uses `6.5mm CT` (or `11mm Caseless`), 20 rounds | 6.5mm CT, 20 rounds with the standard magazine | | |
| **C10** | B10 | Hard Target with large magazine holds 7 | still 10 (open) | | |
| **C11** | B11 | Drum Beat 18 rounds, 25% slower, +50% damage, 60% more kick | 48 rounds with the default drum (18 base), +50% damage, 60% more kick; fire rate probably unchanged (receiver `04A750`) | | |
| **C12** | B12 | Shotty 6 rounds standard, 14 with the extended magazine | 6 standard; Large 14, Extended 10, Extra Large 22 | | |
| **C13** | B13 | Breach follow-ups 50% faster | 1.5 shots/s, 0.67 s between shots | | |
| **C14** | B14 | Microgun fires almost at once, +50% damage | spin-up about 0.1 s (if `FireSeconds` is the spin-up), +50% damage | | |

## Limitations

- **OMOD values (B3, B6, B8, B10, B11, B12).** Not written: opaque in the schema (above).
  The fire-rate parts of B3, B6 and B11 are likely invisible in game until the default
  receivers' `WSPS` is changed; the WEAP values still apply to an instance without those
  receivers.
- **Recoil feel.** Only the per-shot recoil degrees (X and Y) are scaled; whether the game's
  felt "kick" follows them linearly, and how the spring force and base stability
  interact, is not verified. The Drum Beat's two aim models are both scaled, so the kick
  changes with either receiver.
- **Damage.** `WDMG.BaseDamage` is the base the inventory shows before quality tiers; the
  values here are relative to vanilla. The quality-tier OMODs (MUL+ADD `WDMG`) still apply.
  B1's floor (never below 15) is not needed: the results are 23.8 and 30.6.
- **Spin-up.** `WFIR.FireSeconds` as the Microgun's spin-up is inferred from the corpus
  (above), not from a CK or game observation. `WCHG` (the Microgun has
  `ChargingAttackEnabled` = 1, `FullPowerSeconds` 0) is not changed.
- **Re-chambering.** The Kraken and Beowulf keep their own `WAM2.OverrideProjectile`
  (`3673BA`, `074D92`), so tracer and ballistics stay the weapon's; the brief asks only for
  the ammo record. NPCs carrying them get the swapped ammo lists.
- **Scope.** Base weapons only: the NPC-only variants (`Crew_Elite_*`, `Companion_Lawgiver`)
  and the research-menu dummies keep vanilla values, except that `Crew_Elite_Tombstone`
  shares Tombstone's aim model (B7 recoil). Alternate receivers (semi-auto Maelstrom and
  Drum Beat, full-auto Beowulf, ...) are not changed.
- **Unverified in game.** Every value above is checked in the records (plan, validate,
  roundtrip, the fixture test); none is observed in game yet (M3.O5). The runtime IDs
  follow lane D's `runtime-slots-by-scale` assumption.
