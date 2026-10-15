# Gun overhaul design brief

Owner decision 3 (`AGENTS.md`, "Owner decisions"): which video critique is the design
brief for the gun overhaul. Filled in by the owner on 2026-10-14 (`plan/execution-plan.md`,
M3.O4); lane E translates it into `docs/overhaul/overhaul.toml` (M3.E9).

Every point is in the owner's own words. The video's text, captions and script are not
transcribed or quoted; a timestamp says where a point comes from. The brief records what the
owner wants changed and how we will know it worked, not what the video says.

## Source

| Field | Value |
| :--- | :--- |
| **Title** | Everything WRONG With The Guns In Starfield |
| **Channel** | ItsYaBoyBrandyBoy |
| **URL** | https://www.youtube.com/watch?v=YCyUFvTGa68 |
| **Date watched** | 2026-10-14 |
| **Game version the video shows (if stated)** | Not explicitly stated (published October 19, 2023; corresponds to Starfield v1.7.36 launch-window build) |

## Critique points

One row per point. ID is what the spec cites (B1, B2, ...): IDs are never reused or
renumbered; a dropped point stays in the table with "dropped" in Desired change.

| ID | Claim (owner's words) | Timestamp | Weapon classes affected | Desired change | Measurable target |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **B1** | The vintage .45 caliber handgun holds two extra rounds beyond what a standard single-stack frame of its physical dimensions can fit, and its base lethality is overtuned. | 02:29 | Old Earth Pistol, XM-2311 | Reduce base magazine capacity to match historic single-stack dimensions and lower base damage. | Base magazine capacity equals 7 rounds; base damage x0.85 (never below 15). |
| **B2** | A specialized Soviet suppressed marksman rifle is mislabeled as a civilian hunting weapon. | 05:29 | Old Earth Hunting Rifle | Rename the weapon to reflect its military marksman role. | Display name in `kit build --plan` and in-game inventory reads "Old Earth Marksman Rifle". |
| **B3** | A bulky rifle chambered in a large cased telescoped cartridge behaves like a low-caliber submachine gun, firing too fast with negligible kick, weak per-shot impact, and an oversized magazine capacity. | 11:08 | Maelstrom | Lower magazine size and fire rate while increasing per-shot damage and recoil. | Base magazine capacity equals 24 rounds; fire rate x0.75; base damage x1.35; recoil kick x1.5. |
| **B4** | A compact machine pistol shares the exact same rifle-grade cartridge as the Maelstrom while dealing far less damage, making it an inefficient ammo sink with no distinct role. | 13:16 | Kraken | Re-chamber the weapon to use compact caseless pistol ammunition and tighten close-range burst lethality. | Chambered ammo record set to `7.77mm Caseless`; base damage x1.2. |
| **B5** | A heavy .43 caliber hand cannon has an artificially throttled rate of fire that makes semi-automatic follow-up shots feel sluggish and unresponsive. | 19:13 | Urban Eagle | Remove the artificial fire delay so the weapon cycles as fast as the player pulls the trigger. | Rate of fire / semi-auto fire delay x1.75 (minimum 3.5 shots per second). |
| **B6** | The lever-action marksman rifle suffers from a heavy delay between shots, making manual cycling feel sluggish in combat. | 27:45 | Lawgiver | Speed up the cycling rate between shots to improve responsiveness. | Rate of fire x1.5 (cycle delay under 0.65 seconds). |
| **B7** | A heavy 11mm automatic rifle lacks the physical kick expected from a large-bore cartridge, handling too smoothly like a submachine gun. | 29:03 | Tombstone | Increase recoil kick and per-shot stopping power to reflect its large caliber. | Recoil kick x1.5; base damage x1.25. |
| **B8** | Automatic weapons suffer from a severe per-shot damage penalty compared to semi-automatic configurations, making full-auto fire ammo-inefficient across the board. | 33:02 | Every automatic receiver OMOD across ballistic rifles and SMGs (Grendel, Beowulf, Maelstrom, Drum Beat, AA-99, Kodama) | dropped (owner, 2026-10-15: the automatic receiver OMODs carry no damage property, so there is no per-shot penalty to reduce; `docs/overhaul/build-2.md`, B8) | none (dropped) |
| **B9** | A precision battle rifle uses the exact same submachine gun cartridge as the Grendel while dealing four times the damage, invalidating ammo progression. | 34:57 | Beowulf | Re-chamber the rifle into a heavier intermediate rifle cartridge so it no longer shares submachine gun ammo. | Chambered ammo record set to `6.5mm CT` (or `11mm Caseless`); base magazine capacity equals 20 rounds. |
| **B10** | The extended magazine attachment for the .50 caliber anti-materiel rifle doubles capacity without changing the physical size of the magazine. | 36:56 | Hard Target (Extended Magazine OMOD) | Reduce the capacity bonus of the extended magazine mod to a realistic count for its frame. | Extended magazine OMOD sets total capacity to 7 rounds (down from 10). |
| **B11** | A heavy 11mm battle rifle holds an unrealistic 60 rounds in a small drum and fires with almost no recoil and low per-shot impact. | 37:56 | Drum Beat | Cut magazine capacity to match the physical drum size, slow the fire rate, and boost both damage and recoil. | Base magazine capacity equals 18 rounds; fire rate x0.75; base damage x1.5; recoil kick x1.6. |
| **B12** | A compact combat shotgun holds twice as many shells in both its standard and extended magazines as its small frame could physically fit. | 41:13 | Shotty | Halve the base and extended magazine capacities. | Base magazine capacity equals 6 rounds; large/extended magazine OMOD capacity equals 14 rounds. |
| **B13** | The heavy revolving shotgun has an overly throttled rate of fire on top of its cylinder rotation, making follow-up shots unresponsive. | 44:21 | Breach | Increase the rate of fire by reducing the post-shot delay. | Rate of fire x1.5 (fire delay reduced by 33%). |
| **B14** | The rotary heavy weapon suffers from a sluggish spin-up delay before firing, combined with weak per-bullet damage that wastes 7.77mm ammunition before the barrels reach top speed. | 46:26 | Microgun | Eliminate or minimize the barrel spin-up delay and raise base damage per projectile. | Spin-up delay time x0.1 (or 0.0s); base damage x1.5. |

## Out of scope

What this overhaul deliberately does not touch, whatever the video says. Fixed by
`plan/execution-plan.md` ("Not in M3"): no new records, no leveled-list injection, no
scripts, and no fields outside the typed weapon records (`WEAP`, `AMMO`, `PROJ`, `AMDL`, and
`OMOD`, `COBJ`, `ZOOM` where typed). Owner additions:

* **3D Mesh & Model Geometry:** Fixing elongated trigger guards (01:30), missing 1911 plunger tubes (01:59), square barrel bores (18:51), missing triggers on the Coachman (26:09), extreme bullpup bolt clearances (33:43), and decorative underbarrel blocks on the Eon and Sidestar (17:03, 19:58).
* **First- and Third-Person Animations:** Fixing the reversed pump-action cycling on the Old Earth Shotgun and Pacifier (07:11, 43:01), missing empty-reload slide locks (02:10), third-person caseless shell ejection on the Coachman (25:27), and reload animation bugs (08:23).
* **NPC Dialogue & Audio Assets:** Fixing the parent NPC voice line calling the 1911 a "collectible revolver" (03:04) or replacing weapon firing sound effects (18:05).
* **New Custom Ammunition Records:** Creating brand-new custom `.50 Beowulf` or standalone `7.77mm Flechette` `AMMO` records for the Beowulf, Kraken, or Kodama (13:53, 35:57, 39:48), as M3 prohibits adding new records (re-chambering is restricted to existing vanilla `AMMO` records).
* **Engine Features:** Implementing partial/staged reload mechanics across all weapons (02:22) or canted iron sight toggling (42:15).

## In-game checklist (M3.O5)

Filled from the rows above; run on a disposable save with the built plugin last in the load
order (`docs/overhaul/build-1.md` will carry the exact steps, as `docs/smoke-rename.md` does
for the rename).

| Check | Brief row | Expected | Seen | Pass |
| :--- | :--- | :--- | :--- | :--- |
| **C1** | B1 | Old Earth Pistol and XM-2311 show 7-round base capacity and 15% lower base damage in inventory/workbench. | | |
| **C2** | B2 | `Old Earth Hunting Rifle` appears as `Old Earth Marksman Rifle` in inventory and weapon workbench. | | |
| **C3** | B3 | Maelstrom displays 24-round capacity, 25% slower fire rate, 35% higher damage, and visibly stronger vertical kick when firing. | | |
| **C4** | B4 | Kraken uses `7.77mm Caseless` ammo instead of `6.5mm CT` and shows 20% higher base damage. | | |
| **C5** | B5 | Urban Eagle fires rapidly on repeated trigger pulls (~3.5+ RPS) without a 1-second lockout between shots. | | |
| **C6** | B6 | Lawgiver cycles between shots in under 0.65s when firing consecutive aimed shots. | | |
| **C7** | B7 | Tombstone deals 25% more base damage and exhibits 50% stronger recoil kick during sustained fire. | | |
| **C8** | B8 | dropped (row B8 dropped by the owner, 2026-10-15): nothing to check. | | |
| **C9** | B9 | Beowulf consumes `6.5mm CT` (or `11mm Caseless`) instead of `7.77mm Caseless` and holds 20 rounds in its standard magazine. | | |
| **C10** | B10 | Hard Target with Large/Extended Magazine attached holds 7 rounds total instead of 10. | | |
| **C11** | B11 | Drum Beat holds 18 rounds base, fires 25% slower, deals 50% more damage, and kicks 60% harder. | | |
| **C12** | B12 | Shotty holds 6 rounds with standard magazine and 14 rounds with extended magazine attached. | | |
| **C13** | B13 | Breach fires follow-up shots 50% faster with reduced post-shot delay. | | |
| **C14** | B14 | Microgun fires nearly instantaneously upon pulling the trigger with 50% higher base damage per round. | | |

## Traceability rule

Every class and every rule in `docs/overhaul/overhaul.toml` names the brief row it serves
with `brief = "B<n>"` (spec v2, M3.E6). A class or rule without a row, or naming a row that
is not in the table above, is a review finding: the overhaul is not built from a spec that
has one (M3.E9). A row with no class and rule is listed as open in `docs/overhaul/build-1.md`.
The build report (`kit build --plan`) shows each rule's brief, so every changed field traces
back to a row, and each row to a timestamp of the source.
