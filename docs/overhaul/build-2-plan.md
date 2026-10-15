# Gun overhaul build 2: the plan

The output of `kit build --plan` for `docs/overhaul/overhaul.toml` (M3.E9 round 2), as
printed, run on 2026-10-15 from commit `c100a18` (release build) against the owner's
`KitKraft Dev` profile, which the spec's `[load_order]` names. A plan writes nothing. Each
change line reads: brief row, spec rule, record, member, value before -> after. Values are
as stored (f32 in their shortest form; FormIDs as raw hex in the new plugin's numbering,
where `0004AD3E` is Starfield.esm's 7.77mm Caseless, `002B5590` its 6.5mm CT, `0000E841`
LLW_Ammo_Beowulf and `0012F0CE` LLW_Ammo_Kraken; an lstring before is its string ID).

New in build 2: the four OMOD lines (B3, B6, B11 default receivers' `WSPS`; B10 large
magazine `ACPT`), written through the property value paths lane B round 4 typed. The
rename smoke test (`OldSovietMarksmanRifle.esm`) is still active in the profile's
`plugins.txt` but no mod provides it any more, so the rifle's winner, and the only master,
is `Starfield.esm` (build 1 had the smoke test as a second master).

Per-row status, reasons and the in-game checklist: `docs/overhaul/build-2.md`.

```
kit build docs\overhaul\overhaul.toml C:\Modding\kitcraft\out\overhaul\KitKraftGunOverhaul.esm --plan
plan: 46 field change(s) on 21 record(s) for C:\Modding\kitcraft\out\overhaul\KitKraftGunOverhaul.esm (nothing written); validate passed
masters: Starfield.esm
classes (18):
  B10  b10_hard_target_large_mag OMOD matched 1
  B11  b11_drum_beat WEAP matched 1
  B11  b11_drum_beat_full_auto_aim AMDL matched 1
  B11  b11_drum_beat_receiver OMOD matched 1
  B12  b12_shotty WEAP matched 1
  B12  b12_shotty_magazines OMOD matched 3
  B13  b13_breach WEAP matched 1
  B14  b14_microgun WEAP matched 1
  B1   b1_old_earth_pistol WEAP matched 2
  B2   b2_hunting_rifle WEAP matched 1
  B3   b3_maelstrom WEAP matched 1
  B3   b3_maelstrom_receiver OMOD matched 1
  B4   b4_kraken WEAP matched 1
  B5   b5_urban_eagle WEAP matched 1
  B6   b6_lawgiver WEAP matched 1
  B6   b6_lawgiver_receiver OMOD matched 1
  B7   b7_tombstone WEAP matched 1
  B9   b9_beowulf WEAP matched 1
rules (20):
  B1   rule[0] class b1_old_earth_pistol: 2 record(s), 4 field(s) written, 4 changed
  B2   rule[1] class b2_hunting_rifle: 1 record(s), 1 field(s) written, 1 changed
  B3   rule[2] class b3_maelstrom: 1 record(s), 5 field(s) written, 5 changed
  B3   rule[3] class b3_maelstrom via WAIM.AimModel (1 -> 1): 1 record(s), 2 field(s) written, 2 changed
  B3   rule[4] class b3_maelstrom_receiver: 1 record(s), 1 field(s) written, 1 changed
  B4   rule[5] class b4_kraken: 1 record(s), 3 field(s) written, 3 changed
  B5   rule[6] class b5_urban_eagle: 1 record(s), 3 field(s) written, 3 changed
  B6   rule[7] class b6_lawgiver: 1 record(s), 3 field(s) written, 3 changed
  B6   rule[8] class b6_lawgiver_receiver: 1 record(s), 1 field(s) written, 1 changed
  B7   rule[9] class b7_tombstone: 1 record(s), 1 field(s) written, 1 changed
  B7   rule[10] class b7_tombstone via WAIM.AimModel (1 -> 1): 1 record(s), 2 field(s) written, 2 changed
  B9   rule[11] class b9_beowulf: 1 record(s), 3 field(s) written, 3 changed
  B10  rule[12] class b10_hard_target_large_mag: 1 record(s), 1 field(s) written, 1 changed
  B11  rule[13] class b11_drum_beat: 1 record(s), 5 field(s) written, 5 changed
  B11  rule[14] class b11_drum_beat via WAIM.AimModel (1 -> 1): 1 record(s), 2 field(s) written, 2 changed
  B11  rule[15] class b11_drum_beat_full_auto_aim: 1 record(s), 2 field(s) written, 2 changed
  B11  rule[16] class b11_drum_beat_receiver: 1 record(s), 1 field(s) written, 1 changed
  B12  rule[17] class b12_shotty: 1 record(s), 1 field(s) written, 1 changed
  B13  rule[18] class b13_breach: 1 record(s), 3 field(s) written, 3 changed
  B14  rule[19] class b14_microgun: 1 record(s), 2 field(s) written, 2 changed
changes (46):
  B1   rule[0] WEAP Starfield.esm:24561C XM2311 WAM2.AmmoCapacity: 9 -> 7
  B1   rule[0] WEAP Starfield.esm:24561C XM2311 WDMG.BaseDamage: 36 -> 30.6
  B1   rule[0] WEAP Starfield.esm:26D96C M1919 WAM2.AmmoCapacity: 9 -> 7
  B1   rule[0] WEAP Starfield.esm:26D96C M1919 WDMG.BaseDamage: 28 -> 23.8
  B2   rule[1] WEAP Starfield.esm:21BBCD RussianHuntingRifle FULL: {"string_id":"0x0002E469"} -> "Old Earth Marksman Rifle"
  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WAM2.AmmoCapacity: 40 -> 24
  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WDMG.BaseDamage: 4 -> 5.4
  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WFIR.AttackAnimationDuration: 1.3166667 -> 1.7555556
  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WFIR.AttackDelaySeconds: 0.13166668 -> 0.17555557
  B3   rule[2] WEAP Starfield.esm:2984DF Maelstrom WFIR.ShotsPerSecond: 7.5949364 -> 5.6962023
  B3   rule[3] AMDL Starfield.esm:1B01CA Maelstrom_FA_AM ANAM.RecoilDegreesPerShotX: 0.1 -> 0.15
  B3   rule[3] AMDL Starfield.esm:1B01CA Maelstrom_FA_AM ANAM.RecoilDegreesPerShotY: 0.3 -> 0.45000002
  B3   rule[4] OMOD Starfield.esm:077F13 mod_Maelstrom_Receiver_FullAuto_Standard DATA.Properties[1].Value1.Float: 7.5 -> 5.625
  B4   rule[5] WEAP Starfield.esm:21FEB4 Kraken WAM2.AmmoList: "0012F0CE" -> "0000E841"
  B4   rule[5] WEAP Starfield.esm:21FEB4 Kraken WAM2.AmmoType: "002B5590" -> "0004AD3E"
  B4   rule[5] WEAP Starfield.esm:21FEB4 Kraken WDMG.BaseDamage: 3 -> 3.6
  B5   rule[6] WEAP Starfield.esm:26D96D UrbanEagle WFIR.AttackAnimationDuration: 0.4 -> 0.22857143
  B5   rule[6] WEAP Starfield.esm:26D96D UrbanEagle WFIR.AttackDelaySeconds: 0.4 -> 0.22857143
  B5   rule[6] WEAP Starfield.esm:26D96D UrbanEagle WFIR.ShotsPerSecond: 2.5 -> 4.375
  B6   rule[7] WEAP Starfield.esm:02D7F4 Lawgiver WFIR.AttackAnimationDuration: 1 -> 0.64
  B6   rule[7] WEAP Starfield.esm:02D7F4 Lawgiver WFIR.AttackDelaySeconds: 1 -> 0.64
  B6   rule[7] WEAP Starfield.esm:02D7F4 Lawgiver WFIR.ShotsPerSecond: 1 -> 1.5625
  B6   rule[8] OMOD Starfield.esm:057555 mod_Lawgiver_Receiver_SemiAuto_Standard DATA.Properties[1].Value1.Float: 1 -> 1.5625
  B7   rule[9] WEAP Starfield.esm:02EB36 Tombstone WDMG.BaseDamage: 19 -> 23.75
  B7   rule[10] AMDL Starfield.esm:11287A Tombstone_FA_AM ANAM.RecoilDegreesPerShotX: 0.175 -> 0.2625
  B7   rule[10] AMDL Starfield.esm:11287A Tombstone_FA_AM ANAM.RecoilDegreesPerShotY: 0.375 -> 0.5625
  B9   rule[11] WEAP Starfield.esm:04716C Beowulf WAM2.AmmoCapacity: 30 -> 20
  B9   rule[11] WEAP Starfield.esm:04716C Beowulf WAM2.AmmoList: "0000E841" -> "0012F0CE"
  B9   rule[11] WEAP Starfield.esm:04716C Beowulf WAM2.AmmoType: "0004AD3E" -> "002B5590"
  B10  rule[12] OMOD Starfield.esm:12D03F mod_HardTarget_Mag_L DATA.Properties[0].Value1.Int: 5 -> 2
  B11  rule[13] WEAP Starfield.esm:18DE2C DrumBeat WAM2.AmmoCapacity: 30 -> 18
  B11  rule[13] WEAP Starfield.esm:18DE2C DrumBeat WDMG.BaseDamage: 8 -> 12
  B11  rule[13] WEAP Starfield.esm:18DE2C DrumBeat WFIR.AttackAnimationDuration: 1.25 -> 1.6666666
  B11  rule[13] WEAP Starfield.esm:18DE2C DrumBeat WFIR.AttackDelaySeconds: 0.0625 -> 0.083333336
  B11  rule[13] WEAP Starfield.esm:18DE2C DrumBeat WFIR.ShotsPerSecond: 16 -> 12
  B11  rule[14] AMDL Starfield.esm:179DBD Drumbeat_SA_AM ANAM.RecoilDegreesPerShotX: 0.185 -> 0.296
  B11  rule[14] AMDL Starfield.esm:179DBD Drumbeat_SA_AM ANAM.RecoilDegreesPerShotY: 0.32 -> 0.51199996
  B11  rule[15] AMDL Starfield.esm:2BF29F Drumbeat_FA_AM ANAM.RecoilDegreesPerShotX: 0.15 -> 0.24000001
  B11  rule[15] AMDL Starfield.esm:2BF29F Drumbeat_FA_AM ANAM.RecoilDegreesPerShotY: 0.4 -> 0.64
  B11  rule[16] OMOD Starfield.esm:04A750 mod_Drumbeat_Receiver_FullAuto_Standard DATA.Properties[1].Value1.Float: 16 -> 12
  B12  rule[17] WEAP Starfield.esm:26D960 Shotty WAM2.AmmoCapacity: 12 -> 6
  B13  rule[18] WEAP Starfield.esm:0547A3 Breach WFIR.AttackAnimationDuration: 1 -> 0.6666667
  B13  rule[18] WEAP Starfield.esm:0547A3 Breach WFIR.AttackDelaySeconds: 1 -> 0.6666667
  B13  rule[18] WEAP Starfield.esm:0547A3 Breach WFIR.ShotsPerSecond: 1 -> 1.5
  B14  rule[19] WEAP Starfield.esm:0546CD Microgun WDMG.BaseDamage: 9 -> 13.5
  B14  rule[19] WEAP Starfield.esm:0546CD Microgun WFIR.FireSeconds: 1 -> 0.1
limitation: runtime slots and IDs follow the assumption `runtime-slots-by-scale` (docs/load-order.md): not yet observed in game or in the CK
limitation: own records of small/medium plugins stored in the full class resolve to the plugin itself by the assumption `full-style-self`, object IDs masked to the plugin's scale (docs/load-order.md); each version says so in `resolved_by`, and `load` lists examples per plugin in `assumed_records`: FunctionalDecor.esm (9489), Resupply.esm (5504), Pegasus_BotanicsUtilities.esm (21053), miss_o_intwalls_03a.esm (3240), towerhab.esm (3416), KZ_Mantis.esm (251), Hatch-22.esm (208), ZY5_TankTop_and_Skirts.esm (115), ZY4_UC_Navy_Officer_Uniforms.esm (109), CargoShipsExpanded.esm (37), ZYJ_Screensaver.esm (10), BetterSpacesuitAutoHide.esm (2), TALIOHS_HAIRBOOSTERPACK.esm (47), ZYJ_Decor_Lights.esm (265), CrowdCitizensTweaks.esm (123), RTM - Apparel Variety Expanded.esm (59), outfit_variety_paradiso.esm (17), UCMO_CE_Tweaks_Visors.esm (21), ShipVendorFramework.esm (320), SP2_LandingBayCargoAccess.esm (9), SP2_ImmersiveLandingRamps.esm (3030), DWN_FunctionalCoffeeAndTeaBrewers.esm (344), SP2_SimpleImmersiveMusicPlayers.esm (192), SVF-ShatteredSpace-Patch.esm (19), Wicked Outpost Projeckt 2.esm (6693), SVF-FreeLanes-Patch.esm (18), ZYEC_ENHANCED_Elevated_Cabins.esm (9207)
limitation: records whose FormID does not resolve are left out of overrides and queries: HG_ShipCargoHoldAtHome.esm (4)
limitation: active by assumption (docs/load-order.md, "Active plugins"): ShatteredSpace.esm (mo2-unmanaged:Unmanaged: ShatteredSpace), SFBGS004.esm (mo2-unmanaged:Unmanaged: SFBGS004), SFBGS00D.esm (hardcoded-official), SFBGS050.esm (hardcoded-official), BlueprintShips-SFBGS050.esm (mo2-unmanaged:Unmanaged: BlueprintShips-SFBGS050), SFBGS047.esm (hardcoded-official)
limitation: load order by the assumption `hardcoded-official-slots` (docs/load-order.md, "Order"): official plugins without a plugins.txt entry load right after the Starfield.ccc plugins: ShatteredSpace.esm, SFBGS004.esm, SFBGS00D.esm, SFBGS050.esm, BlueprintShips-SFBGS050.esm, SFBGS047.esm; loadorder.txt lists ShatteredSpace.esm, SFBGS004.esm, SFBGS00D.esm, SFBGS050.esm, BlueprintShips-SFBGS050.esm, SFBGS047.esm later
limitation: active plugins with no file (not loaded): OldSovietMarksmanRifle.esm
limitation: profile: OldSovietMarksmanRifle.esm is active but no file provides it
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:24561C
limitation: load order problem that does not affect the forms written: active plugins with no file (the game does not load them either): OldSovietMarksmanRifle.esm
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:26D96C
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:21BBCD
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:2984DF
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:1B01CA
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:077F13
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:21FEB4
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:26D96D
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:02D7F4
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:057555
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:02EB36
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:11287A
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:04716C
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:12D03F
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:18DE2C
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:179DBD
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:2BF29F
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:04A750
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:26D960
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:0547A3
limitation: load order problem that does not affect the forms written: HG_ShipCargoHoldAtHome.esm has 4 unresolved record(s) (FormIDs past every master class); none can be a later version of Starfield.esm:0546CD
limitation: header of a new plugin follows the corpus rules in docs/corpus.md (Writing rules): CNAM "DEFAULT" and INCC (interior cell count) as in the two BlueprintShips files; SNAM, BNAM (branch name) and CHGL (changelist), which the official build pipeline writes, are deliberately not written. What the Creation Kit writes for a new non-localized plugin (flags, HEDR next object ID, these subrecords, inline text encoding) is not observed yet (M2.E3 ck-diff part, M2.O4)
limitation: plan: nothing was written to C:\Modding\kitcraft\out\overhaul\KitKraftGunOverhaul.esm
```
