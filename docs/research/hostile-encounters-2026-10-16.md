# Hostile encounters before hooks (M5.H1)

Date: 2026-10-16. Research note for ticket M5.H1 (`plan/execution-plan.md`, whose rows
name this dated file; this is that deliverable). Owner brief: `docs/foothold/owner-addendum-2026-10-15.md`
items 3 and 4. No runtime hook is proposed here before the vanilla data and Papyrus paths
were read; no Foothold attack design is made. Nothing in this note was observed in game:
every runtime claim is grade D until a ledger row exists, so **no row was added to
`docs/game-observations.md`** (the facts below are corpus facts and stay here).

## How to read this note

- **Commands.** Every corpus claim cites the read-only `kit` command that shows it. `kit`
  is `C:\Modding\kitcraft\bin\kit.exe` (SHA-256 `4fa997f83437f442b025f761a6d78fccb94d3e0874b11082eb14bc8c80d0672d`,
  built 2026-10-16) with `KITKRAFT_DATA_DIR` set to the game's `Data` folder (game
  1.16.244, the corpus of `docs/corpus.md`). Every `query`, `inspect` and `refs` ran with
  the 14 official plugins, each after its own `--plugins` flag, in this order:
  `--plugins Starfield.esm --plugins Constellation.esm --plugins OldMars.esm --plugins
  BlueprintShips-Starfield.esm --plugins SFBGS007.esm --plugins SFBGS008.esm --plugins
  SFBGS006.esm --plugins SFBGS003.esm --plugins SFBGS004.esm --plugins ShatteredSpace.esm
  --plugins SFBGS00D.esm --plugins SFBGS047.esm --plugins SFBGS050.esm --plugins
  BlueprintShips-SFBGS050.esm` (written `[14]` below). One `--plugins` followed by several
  names does not work (`kit` exits 2, "unexpected argument"). `@n` is the byte offset `inspect`
  reports for the subrecord. `papyrus events` reads `Tools\ContentResources.zip` in place;
  `papyrus disasm` reads Bethesda's compiled scripts from `Starfield - Misc.ba2` (both
  level-1 corpus, ADR 0006 Amendment 1; logic is described in our words, nothing quoted).
- **Grades** (ADR 0005 Amendment 3): **A** read by our tool from the official files and
  reproducible with the cited command; **B** documented by an author about their own
  work; **C** secondary, by analogy with an older game, or by naming; **D** hypothesis.
- **Condition functions** are numbers: `crates/schema/defs/common/Condition.toml` types
  each function's parameters from xEdit but does not transcribe the names, so a function
  is cited as `fn <index>` with its parameter types; any name given is grade C.
- **DLC names** are by EditorID prefix (grade C): `SFBGS00D.esm` records carry `SFFL_`
  (Free Lanes), `SFBGS050.esm` records carry `SFTER_` (Terran Armada).

## 1. How vanilla stages Starborn, Armillary and outpost encounters

### 1.1 The outpost attack chain, end to end (Starfield.esm, extended by SFBGS050)

```text
OutpostBeaconScript (on the outpost beacon)          [disasm outpostbeaconscript.pex]
  OnLoad / OnWorkshopObjectPlaced (resource object)
    -> StartTimer(random 12..48 s)
    -> OnTimer -> SQ_Parent.CheckForAttack(beacon)   [disasm sq_parentscript.pex]
         gates: beacon.AllowAttacks; player on the beacon's planet;
                planet LCTN actor values = cooldowns; special cases; dice
         -> Keyword.SendStoryEvent(<attack keyword>, akRef1 = beacon)
SMEN 029152 (ENAM "SCPT": script event)
  SMBN SQ_MainBranch 203D52
    SMBN SQ_OutpostAttackBranch 349C9F
      SMQN ..NodeSpecial_Starborn 349CA0  (keyword 349C9E; RNAM 576 h) -> QUST 349C9C
      SMQN SQ_OutpostAttackNode 349CA1    (keyword 04643E)            -> QUST 341FCD, 0418B8, 08708E
      SMQN ..NodeCreatures 34D3E7          (keyword 346518)            -> QUST 361A85, 361C0E, 361C0D, 3076FA
      SMQN SFTER_..TerranXTech SFBGS050:069E03 (keyword SFBGS050:069E00) -> QUST SFBGS050:069DFF
attack QUST (ENAM "SCPT"): aliases fill from the event, attackers created at a landing
  marker, travel package to the beacon; stage 50 contact, stage 100 all dead -> stop;
  script cleanup when all dead or the player leaves the planet
```

| # | Fact | Grade | Evidence |
| - | ---- | ----- | -------- |
| V1 | `SQ_OutpostAttackBranch` `Starfield.esm:349C9F` (SMBN): parent `SQ_MainBranch` `203D52`, previous node `0913C5`, no conditions (`CITC` 0), `XNAM` MaxConcurrentQuests 0. `SQ_MainBranch`'s parent is the SMEN `Starfield.esm:029152`, whose `ENAM` is `SCPT` (winner `ShatteredSpace.esm`, which overrides it) | A | `kit inspect Starfield.esm:349C9F [14]` (`PNAM` @1294895812); `kit inspect Starfield.esm:203D52 [14]`; `kit inspect Starfield.esm:029152 [14]` (`ENAM` @495552760) |
| V2 | Four quest nodes hang under the branch. Each has one condition `fn 576` (parameters Event, EventData) with Event `0x314B0000` (bytes `00 00 4B 31`, our reading: event data member `K1`, the event keyword; grade C) and EventData = the keyword: Starborn `349CA0` -> KYWD `SQ_OutpostEventAttackSpecial_Starborn` `349C9E`, QUST `349C9C`, `RNAM` HoursUntilReset 576.0; generic `349CA1` -> KYWD `SQ_OutpostEventAttack` `04643E`, QUST `341FCD`, `0418B8`, `08708E`; creatures `34D3E7` -> KYWD `SQ_OutpostEventAttackCreatures` `346518`, QUST `361A85`, `361C0E`, `361C0D`, `3076FA` (`DNAM` 1); Terran `SFBGS050.esm:069E03` -> KYWD `SFBGS050.esm:069E00`, QUST `SFBGS050.esm:069DFF`, previous node `34D3E7` | A (data), C (`K1` meaning) | `kit inspect Starfield.esm:349CA0 [14]` (`CTDA` @1294971471, `RNAM` @1294971549); `kit inspect Starfield.esm:349CA1 [14]`; `kit inspect Starfield.esm:34D3E7 [14]`; `kit inspect SFBGS050.esm:069E03 [14]` |
| V3 | No record overrides the branch, the three vanilla nodes, `SQ_Parent` or the vanilla attack quests: one version each. Terran Armada joins by **adding** a node whose `SNAM` points after the creatures node, not by editing vanilla | A | `kit inspect` of `349C9F`, `349CA0`, `349CA1`, `34D3E7`, `07092C`, `341FCD`, `349C9C` `[14]` ("1 version(s)"); `kit inspect SFBGS050.esm:069E03 [14]` (`SNAM` @103377990) |
| V4 | The only referrer of each attack keyword is its node's condition (no record names them otherwise; `VMAD` values are not indexed, see L1) | A, limited | `kit refs Starfield.esm:04643E --incoming [14]`; same for `349C9E`, `346518`, `SFBGS050.esm:069E00` |
| V5 | `SQ_ParentScript` declares the attack inputs: `ActorValue OutpostLastAttackTimestamp` (209), `OutpostLastAttackRollTimestamp` (212), `OutpostCargoLinkAV` (215), `Keyword SQ_OutpostEventAttack` (218), `SQ_OutpostEventAttackCreatures` (221), `OutpostAttackSpecialCase[] OutpostAttackSpecialCases` (224, not const), struct `OutpostAttackSpecialCase` {`specialCaseCondition` ConditionForm, `specialCaseAttackChance` float, `storyEventKeyword` Keyword} (202), and `CheckForAttack(OutpostBeaconScript outpostRef, bool bForceAttack = false, int iForceAttackType = 0, float fDieRoll = 0.0)` (1272) | A | `kit papyrus events --search OutpostAttack`; `kit papyrus events --script SQ_ParentScript` (202..224, first page); `kit papyrus events --script SQ_ParentScript --function CheckForAttack` (1272: without `--function` it is past the first page of 100 of 411); `kit --json papyrus events --script SQ_ParentScript --kind struct --name OutpostAttackSpecialCase` |
| V6 | `CheckForAttack`, in order: returns when the beacon's `AllowAttacks` is false; when the player is not on the beacon's planet; when the beacon's location has no parent of type `LocTypeMajorOrbital` (the planet `LCTN`). Cooldowns are **actor values on the planet location**, compared with `Utility.GetCurrentGameTime()`: the last roll (`OutpostLastAttackRollTimestamp`, minimum 0.04 day) and the last attack (`OutpostLastAttackTimestamp`, minimum 7 days); `bForceAttack` skips both. Then each special case whose condition is true on the beacon rolls its chance with `Game.GetDieRollSuccess` (or uses `fDieRoll`) and, on success, sends its keyword with `SendStoryEventAndWait(akRef1 = beacon)`, returning when a quest started; a special case with no keyword adds its chance to the generic chance instead. Generic chance = 0.1 + 0.01 x (sum of the beacon's resource actor values listed by `MB_Parent.resourceAVs`) + 0.05 x the beacon's cargo-link value + those added chances. A roll at or under 0.08 (or `iForceAttackType` > 1) sends the creatures keyword; else a roll at or under the chance (or `iForceAttackType` 1) sends the generic keyword. Every completed roll writes the roll timestamp on the planet | A (bytecode), B/D (runtime) | `kit papyrus disasm scripts/sq_parentscript.pex --archive "Starfield - Misc.ba2"` (function `CheckForAttack`; script variables `minDaysSinceLastAttackRoll` 0.04, `minDaysSinceLastAttack` 7, `attackChanceBase` 0.1, `attackChanceResourceMult` 0.01, `attackChanceCargoLinkMult` 0.05, `attackChanceCreature` 0.08) |
| V7 | `OutpostBeaconScript`: `bool AllowAttacks = true` (const, conditional, 31); `OnLoad` adds `LocTypeOutpost` to the beacon's location and calls `StartAttackTimer`; `OnWorkshopObjectPlaced` calls it when the placed object has resource values; the timer is random between `fAttackTimerMinSeconds` 12 and `fAttackTimerMaxSeconds` 48 (real seconds) and its `OnTimer` calls `SQ_Parent.CheckForAttack(self, false, 0, 0)`. So **attacks are only rolled while the beacon is loaded**, that is while the player is there | A (bytecode), D (runtime) | `kit papyrus events --script OutpostBeaconScript`; `kit papyrus disasm scripts/outpostbeaconscript.pex --archive "Starfield - Misc.ba2"` |
| V8 | `SQ_OutpostAttackScript` (parent of the attack quest scripts): properties `OutpostBeacon`, `AttackTarget`, `Trigger` (aliases), `Attackers` (ref collection), `CountsAsAttack = true`. `OnQuestStarted` writes `OutpostLastAttackTimestamp` on the planet (when `CountsAsAttack`) and registers for `REParentScript`'s `RECheckForCleanup`; `CheckForCleanup` stops the quest when every attacker is dead **or the attack target is not on the player's planet**; `OnQuestInit` disables the `Trigger` alias, `OnQuestShutdown` re-enables it | A (bytecode), D (runtime) | `kit papyrus disasm scripts/sq_outpostattackscript.pex --archive "Starfield - Misc.ba2"`; declarations: `kit papyrus events --search OutpostAttack` (`SQ_OutpostAttackScript.psc` 3..125) |
| V9 | `SQ_OutpostAttackHumansScript` (human attackers): lands the ships (`LandShip`), sets the attackers' aggression actor value when they spawn, makes them call the native `Actor.AttackOutpostObjects(beacon)` when they enter the outpost location and every 3 s after a new object is placed, re-targets the beacon the player places inside the build radius (`OnOutpostPlaced`). **When the attack target unloads while the attack has started and attackers are alive, it calls `SetDestroyed(true)` on every destructible outpost object (`GetDestructibleOutpostObjects`) that is not 3D-loaded.** Shutdown disables the ships | A (bytecode), D (runtime) | `kit papyrus disasm scripts/sq_outpostattackhumansscript.pex --archive "Starfield - Misc.ba2"` (`remote_ReferenceAlias_OnUnload`); `Actor.psc:69`, `ObjectReference.psc:434`, `ObjectReference.psc:1036` (`kit papyrus events --script Actor --name AttackOutpostObjects`, ...) |
| V10 | The Starborn quest `SQ_OutpostAttackSpecial_Starborn01` `349C9C`: `ENAM` `SCPT`; quest condition `fn 837` on CNDF `SQ_OutpostAttack_CND_Special_MQ204` `2BAA4B`, run on the event's `R1` reference (Parameter3 `0x3152`, grade C for the reading); stages 50 ("attackers reach the outpost or fight the player") and 100 ("all dead, stop") in our words; aliases `OutpostBeacon` (from event data `R1`), `AttackTarget` (created `Starfield.esm:00003B` at the beacon), `LandingLocation` (`fn 837` on CNDF `SQ_OutpostAttack_CND_ValidLocation` `341FD0`, and `fn 1` against GLOB `SQ_OutpostAttackMaxDistance` `341FCF` = 2000.0, closest to the beacon), `Trigger`, `Player`, `MapMarker`, `Marker_Center`, `Marker_ShipLandingA` (`fn 560` keyword `SQ_PlayerShipLandingMarkerInUse` `2D17DE` = 0), `ShipA..C` (packages `15DCF2`, `15DCF3`), ref collection `Attackers` (`ALMI` 6, created from NPC_ `LvlStarborn_Aggro` `27025D` at the landing marker, level 1, linked to the target by `RELinkSandbox` `03085E` and `RELinkCenterMarker` `207498`, package `SQ_OutpostAttackTravelToTarget` `346853`). `DNAM` flags 1024 | A | `kit inspect Starfield.esm:349C9C [14]` (`CTDA` @1283735901, landing-location `CTDA` @1283736344 and @1283736382, `ALCO` @1283737867, `ALMI` @1283737804); `kit inspect Starfield.esm:341FCF [14]` (`FLTV` @1413181) |
| V11 | The Starborn gate is the Armillary. CNDF `2BAA4B` holds two conditions: `fn 59` (Quest `MQ204` `2C6D74`, stage 620) == 1, and `fn 14` (actor value `MQ_ArmillaryResourceAV` `1F36BF`) >= 1. The AV's description text (first bytes read) ties its value to the Armillary having been built (our paraphrase). Read: **Starborn attack an outpost that hosts the Armillary, after MQ204 stage 620** | A (data), C (`fn 59` = stage done, by analogy) | `kit inspect Starfield.esm:2BAA4B [14]` (`CTDA` @1427936965, @1427937003; decoded by hand from the 32-byte layout of `Condition.toml`, CNDF is not typed); `kit inspect Starfield.esm:2C6D74 [14]`; `kit inspect Starfield.esm:1F36BF [14]` (`NLDT` @1290585989) |
| V12 | The landing-location gate `341FD0`: `fn 563` (LCRT `ShipLandingMarkerRefType` `009105`) == 1, and either `fn 562` keyword `LocTypeOE_Keyword` `1A5468` or `fn 562` keyword `LocTypeDungeon` `0254BC` (OR flags) | A (data), C (operator reading) | `kit inspect Starfield.esm:341FD0 [14]`; `kit inspect` of `009105`, `1A5468`, `0254BC` `[14]` |
| V13 | The Terran Armada quest `SFBGS050.esm:069DFF` copies the Starborn template (same stages, aliases, packages, `ALMI` 6); its gate CNDF `SFBGS050.esm:069E01` is `fn 14` on `Y2_ResourceInorgUniqueX-Tech` (`SFBGS00D.esm:05F4DC`) >= 1, so **X-Tech extractors draw Terran attacks, and Terran Armada depends on a Free Lanes actor value**; attackers are NPC_ `SFTER_LvlTerranMixed_OutpostAttack_Aggro` `SFBGS050.esm:07A167` (list `07A166`) | A | `kit inspect SFBGS050.esm:069DFF [14]` (`ALCO` @102654553); `kit inspect SFBGS050.esm:069E01 [14]`; `kit inspect SFBGS00D.esm:05F4DC [14]` (AVIF `Y2_ResourceInorgUniqueX-Tech`, one version) |
| V14 | Human attack quest `SQ_OutpostAttack01Humans` `341FCD`: same shape, `ALMI` 10, `Attackers` has no `ALCO` (filled by script from the landed ships); creature quest `361A85`: quest condition `fn 916` (ActorBase `2BFF10`), four creature collections created from `2A4365`, `2C0B46`, `1E46FE`, `2C0B45` (`ALMI` 3, 6, 8, 10), script `SQ_OutpostAttackCreaturesScript` (first `VMAD` bytes) | A | `kit inspect Starfield.esm:341FCD [14]`; `kit inspect Starfield.esm:361A85 [14]`; `kit --json inspect Starfield.esm:361A85 [14]` (`VMAD` payload strings) |
| V15 | Every outpost attack quest and the overlay quest `OE_KMK_HostileHumans` `2308A3` compare a distance-like `fn 1` against GLOB `SQ_OutpostAttackMaxDistance` (2000.0): one GLOB tunes all of them | A (data), C (`fn 1` meaning) | `kit refs Starfield.esm:341FCF --incoming [14]` (10 referrers) |
| V16 | Factions: Starborn templates (e.g. NPC_ `EncStarborn01Template` `2956D3`) are in `StarbornEncounterFaction` `018EDE` and `GroupFactionStarborn` `04CE27`; `018EDE` is Enemy to `PlayerFaction` `01C21C` and `PlayerAllyFaction` `106C30`. Turrets (`OutpostTurretLaser01` `1E6640`) are in `PlayerAllyFaction`, `PlayerFriendFaction` `106C31`, `TurretFaction` `11EA70`, `OutpostAllyFaction` `1D8429`; `OutpostAllyFaction` is Ally to `PlayerFaction` and Friend to `OutpostFaunaFaction` `3C8596` | A | `kit inspect` of `2956D3`, `018EDE`, `1E6640`, `1D8429` `[14]` (`XNAM` Relations; reaction names from `crates/schema/defs/FACT.toml`) |
| V17 | Leveled actors: `LvlStarborn_Aggro` `27025D` templates every trait from LVLN `LCharStarborn` `270261` (8 entries, levels 1 to 100), legendary chance GLOB `Legendary_Starborn_Chance` `06AAC8` | A | `kit inspect Starfield.esm:27025D [14]` (`TPLT` @106, `LTPC` @116); `kit inspect Starfield.esm:270261 [14]` |
| V18 | The attack travel package `346853`: location = linked reference by keyword `RELinkSandbox` `03085E`, radius 20, template `Sandbox_WeaponOutOptions` `346854` | A | `kit inspect Starfield.esm:346853 [14]` (`PLDT` @1286623202) |

### 1.2 Starborn and Armillary encounters outside outposts

| # | Fact | Grade | Evidence |
| - | ---- | ----- | -------- |
| S1 | Starborn quests: overlay (planet) encounters `OE_KMK_Starborn` `34CAAB` with `00`, `01`, `HigherChance` variants; space encounters `SE_Player_KMK01_StarbornPatrol` `34EF7E` and variants; boarding `BE_Generic_Starborn` `08C09E`; the outpost special `349C9C`; Free Lanes `SFFL_SE_JG04_B_Starborn` `SFBGS00D.esm:08F502`. No QUST, LCTN, LVLN, NPC_, FACT or KYWD EditorID contains `Armillary`; the Armillary appears as GLOBs `MQArmillaryCompleteGlobal` `07994F`, `MQArmillaryLocation` `1DF606` (0.0), `MQ_ArmillaryMount_MaxBuildCount` `1F818B`, `MQAlllowArmillaryGravDrive` `3E5D36`, MQ packages and the AV of V11 | A | `kit query QUST --where 'EDID ~ "Starborn"' [14]` (26); `kit query <T> --where 'EDID ~ "Armillary"' [14]` for QUST, LVLN, NPC_, FACT, KYWD, LCTN (0 each), GLOB (4), PACK (9), SCEN (4) |
| S2 | `OE_KMK_Starborn` is listed by six overlay nodes (`OE_Debug_KMK` `08AEDD`, `OE_UncommonNode` `110667`, `OE_CommonNode` `110668`, `OE_HostileHumansNode` `2308A4`, `OE_Ships_RareNode` `286ED7`, `OE_Ships_UncommonNode` `2DD60E`); `SE_Player_KMK01_StarbornPatrol` by `SE_Player_CommonNode` `12C590`, `SE_Player_RareNode` `12C58F` and Free Lanes' `SFFL_SE_FreeLanes_Interdictions` `SFBGS00D.esm:06DE10` and `..NHPInterdictions` `08A1DA` | A | `kit refs Starfield.esm:34CAAB --incoming [14]`; `kit refs Starfield.esm:34EF7E --incoming [14]` |
| S3 | `OE_KMK_Starborn`: `ENAM` `SCPT`, priority 30, quest condition `fn 837` CNDF `SE_CND_StarbornAttackRare` `34FB9C` (`fn 543` on QUST `MQ106` `021BEF` == 1); the `HigherChance` gates (`20E28F`, `1EEE31`) use `fn 543` on `MQ204` and `fn 74` on GLOB `MQArmillaryLocation` (== 0 for overlay, != 2 for space). Its `Starborn` collection is created from the same `LvlStarborn_Aggro` `27025D` (`ALMI` 6) at `Marker_Center`; stages 0 and 100 | A (data), C (`fn 74` = global value, `fn 543` meaning) | `kit inspect Starfield.esm:34CAAB [14]`; `kit inspect` of `34FB9C`, `20E28F`, `1EEE31`, `021BEF`, `1DF606` `[14]` |
| S4 | A GLOB cooldown pattern exists in DLC data: `SFFL_SE_JG04_B_Starborn` (`ENAM` `CLOC`) has the condition `fn 74` on GLOB `GameDaysPassed` `000039` compared (type `0x44`: greater than, use global) with GLOB `SFFL_SE_KavnykStarborn_Cooldown` `SFBGS00D.esm:0950CE` (0.0) | A (data), C (operator reading) | `kit inspect SFBGS00D.esm:08F502 [14]` (`CTDA` line 1); `kit refs SFBGS00D.esm:0950CE --incoming [14]`; `kit inspect Starfield.esm:000039 [14]` |
| S5 | Starborn story-manager tree: `SMBN BEStarbornBranch` `08C09F`, `SMQN BEStarbornQuests` `08C0A0`, `StarbornGenericConvos` `294968`; locations named Starborn are only ship templates in `BlueprintShips-Starfield.esm` | A | `kit query SMQN|SMBN|LCTN --where 'EDID ~ "Starborn"' [14]` |

### 1.3 What the corpus says about the six bullets, in short

Outpost attacks are **story-manager script events** sent by a Papyrus roll that runs only
while the outpost is loaded, gated per outpost (`AllowAttacks`), per planet (actor-value
timestamps, 7-day minimum) and per special case (condition form plus chance). The quests
create their attackers through aliases from leveled actors, path them with one package,
and end by script when all attackers die or the player leaves the planet. **Vanilla has
no unattended resolution except one punitive branch**: human attackers alive when the
target unloads destroy every unloaded destructible outpost object (V9). DLC extends the
system by adding nodes, keywords and condition forms (V3, V13) and by GLOB cooldowns in
encounter quests (S4).

### 1.4 Limits of this read

- **L1.** `inspect` shows the first 256 bytes of a payload, so the `VMAD` of
  `SQ_Parent` `07092C` (the special-case array values: which conditions, chances and
  keywords are registered) and the second script of the Starborn and Terran quests were
  not read; `refs` does not follow `VMAD` object properties (M5.D6). How Terran Armada's
  special case gets into the array is **unknown** (no override of `SQ_Parent`, V3; the
  property is not const, V5). Request (lane D, named in this ticket's STATUS line): a
  full-payload `VMAD` view in `inspect`.
- **L2.** CNDF is not typed (`kit coverage --matrix --types CNDF`: typed-readable fail,
  0.0 %); its conditions were decoded by hand.
- **L3.** `papyrus events` reads the base `ContentResources.zip`; DLC scripts were not
  searched.

## 2. Prior-art attack managers (public descriptions only)

Read only from public mod pages, search-engine summaries of them, the authors' own sites
and wikis. Nexus pages refused automated fetches (HTTP 403), so their permission tabs were
not read: their license column says so. No mod archive, script or source was downloaded
or read. Registry rows: `docs/source-registry.md` key `PRIOR-ART-ATTACKS`.

| Mod (game) | License as stated | Triggers | Actors and spawn | Cooldown | Defenses | Unattended | Cancel and cleanup | Persistence | Grade |
| ---------- | ----------------- | -------- | ---------------- | -------- | -------- | ---------- | ------------------ | ----------- | ----- |
| SKK Outpost Attack Manager, SKKmods ([Nexus 6671](https://www.nexusmods.com/starfield/mods/6671); description mirror [Starfield Unity 821](https://starfieldunity.com/mods/821/skk-outpost-attack-manager)) (Starfield) | SKK general terms: personal use only; no porting, republishing or distribution of SKK code ([skkmods.com/starfield](https://skkmods.com/starfield/)); Nexus tab not read | A buildable attack terminal per outpost: "attack now", or a real-time timer (off, 20-40, 40-80, random 20-80 min) | Human (Spacer, Crimson Fleet, Ecliptic, The First, Varuun...) and creature groups as toggles; 1 to 4 groups of 2 to 16; level within a band of the player's; spawn 100-300 m out; aborts with a notice when no spawn point is found | One timed attack queued or active at a time | Hired mercenaries (up to 8 per outpost); global turret build limit 6 or 18 | Timed attacks wait until the player is within 25 m; the outpost is marked and the player notified when a timer fires | Terminal "Cancel Attack"; optional "stop attack on unload" removes attackers that unload; removing the terminal ends attacks there; "Remove Corpses" | Install or remove any time; does not touch the vanilla system (author's claim) | B |
| Outpost Raids, PouYad0 ([Nexus 18058](https://www.nexusmods.com/starfield/mods/18058)) (Starfield) | Not read | Timer | Pirate factions, staged: intercept in orbit, on landing, at the outpost | Not stated | Intercepting the raid | Failing to stop it means the outpost is raided and looted | Not stated | Not stated | C (search summary of the page) |
| Outpost explodes bug fix and Repair All ([Nexus 8292](https://www.nexusmods.com/starfield/mods/8292)) (Starfield) | Not read | Changes when vanilla schedules attacks; resets it on leaving | - | - | - | Addresses generators and extractors failing during an unseen attack (consistent with V9) | Stop the attack and repair everything in one action | Vanilla attacks return on uninstall (author's note) | C (search summary) |
| SKK Settlement Attack System ([Nexus 37393](https://www.nexusmods.com/fallout4/mods/37393), [article 1597](https://www.nexusmods.com/fallout4/articles/1597)) (Fallout 4) | SKK general terms as above | Real-time interval (default 3600 s, +-20 %; -1 random 15 min to 4 h); console "attack now" | Mostly ranged faction groups; independent of vanilla attacks | Interval only | - | The attack holds until the player arrives; a 30-minute failsafe after arrival | Optional stop on unload (actors removed beyond the active area) | Not stated | C (search summary) |
| Workshop Framework, kinggath ([Nexus 35004](https://www.nexusmods.com/fallout4/mods/35004)) and users of its globals: Better Settlement Defence ([63998](https://www.nexusmods.com/fallout4/mods/63998)), Faction Workshop Manager 2 ([104117](https://www.nexusmods.com/fallout4/mods/104117)) (Fallout 4) | Not read | Vanilla: food and water raise attack strength, defense and population resist | Vanilla | Vanilla | Better Settlement Defence: no random loss when defense exceeds food plus water | Vanilla resolves off-screen; FWM2 removes dead actors after unattended assaults | FWM2 cleanup | Framework exposes globals instead of script edits, for compatibility | C |
| Fallout 4 vanilla, analogy ([Fallout wiki](https://fallout.fandom.com/wiki/Fallout_4_settlements)) | Wiki text, not reused | Food and water stored raise the chance | Faction groups at fixed spawn points | - | Defense rating (turrets, guards) | Off-screen fights decided by numbers; a loss damages objects temporarily, loses some stored items, lowers resources and happiness temporarily | Repair at the workshop | - | C |
| Sim Settlements 2 (Fallout 4; [wiki "Hostile Takeover"](https://wiki.simsettlements2.com/gameplay/main-story/chapter2/Hostile-Takeover), [forum](https://simsettlements.com/site/index.php?threads/constant-gunner-attacks-at-my-settlements.21178/)) | Not read (owner decision 5: no contact) | Story quest takes a settlement; later Gunner attacks appear as quest alerts, reportedly only after a visit | Gunners; companions and unique settlers join | - | Military plots supply defense | Players report ignoring them without consequence | Fast travel blocked during the takeover quest | - | C/D (page bodies did not load; summaries and forum) |
| Tundra Defense ([Skyrim 22330](https://www.nexusmods.com/skyrim/mods/22330), [SE 14310](https://www.nexusmods.com/skyrimspecialedition/mods/14310)) (Skyrim) | Not read | Remote-triggered raids at three difficulties; random raids with an on/off option | Raiders, dragons | - | Recruited stationary and patrol guards; upgrades paid with salvaged supplies | - | - | - | C |
| Hearthfire defense for Windstad, Heljarchen, Lakeview ([SE 10101](https://www.nexusmods.com/skyrimspecialedition/mods/10101)) (Skyrim) | Not read | Configurable stacking dragon attacks and bandit raids | Dragons, bandits | Configurable | Walls, towers, 10 guards per house | - | - | - | C |

What the prior art agrees on (grade C, a design input, not a requirement): the player
can turn attacks off or set their frequency; a direct "attack now" control exists for
testing; attacks wait for the player, or are cancelled cleanly when the player leaves;
defenses are things the player builds or hires; the punitive off-screen loss of Fallout 4
and vanilla Starfield (V9) is the behaviour mods most often remove.

## 3. The owner's six mechanics on Starfield's native surfaces

Verdicts follow ADR 0008 §5: a **hook candidate** needs a deliverable, a tried data path
and evidence that it fails. None of the six has that evidence, so **no runtime-need
record is opened** (`docs/design/runtime-extensions.md` is not created).

| # | Mechanic (addendum item 3) | Native surfaces, with evidence | Verdict |
| - | -------------------------- | ------------------------------ | ------- |
| M1 | Configurable encounter triggers, actors, factions, spawn locations, and cooldowns | Trigger: `Keyword.SendStoryEvent` / `SendStoryEventAndWait` (`Keyword.psc:4, 8`) into a story-manager node with conditions (V2), or a quest started directly (`Quest.Start`, `Quest.psc:227`); actors: alias-created actors from an `NPC_` templated on a LVLN (V10, V17) or `ObjectReference.PlaceActorAtMe` (`ObjectReference.psc:863`); factions: `FACT` relations (V16; FACT typed-writable `pass`, `kit coverage --matrix --types FACT`); spawn location: location aliases with conditions on ref types and keywords (V10, V12), linked-ref packages (V18); cooldowns: actor-value timestamps on a location (V6), `SMQN` `RNAM` hours until reset (V2), or GLOB compared with `GameDaysPassed` (S4; GLOB typed-writable `pass`) | **No native hook needed** (A for the surfaces; D that our own records drive them in game) |
| M2 | Quest/script-driven activation and deterministic test triggering | Vanilla already has a deterministic path: `CheckForAttack(beacon, bForceAttack, iForceAttackType, fDieRoll)` skips the cooldowns and fixes the attack type and roll (V5, V6); console `setstage`/`startquest` on our quest; `Quest.OnStageSet` (`Quest.psc:294`) and `ScriptObject.StartTimer`/`OnTimer` (`ScriptObject.psc:242, 458`) for script reaction; `Game.GetDieRollSuccess` takes a forced roll (`Game.psc:187`) | **No native hook needed** (A for declarations; D for console syntax that calls a quest function with an object argument) |
| M3 | Combat behavior, settlement defenses, damage, and recovery | Combat: packages and aggression (V9, V18), `Actor.StartCombat` (`Actor.psc:712`), `Actor.AttackOutpostObjects` native (`Actor.psc:69`); defenses: turret `NPC_` in player-allied factions (V16); damage: `ObjectReference.DamageObject` (293), `SetDestroyed` (1036), `IsDestroyed` (726), `OnDestructionStageChanged` (1627), `GetDestructibleOutpostObjects` (434); recovery: `ClearDestruction` (262), `SetDestroyed(false)`; `Actor.Resurrect` (`Actor.psc:520`) for crew | **No native hook needed** for authoring; behaviour is game-runtime, so every row needs a ledger observation (D) |
| M4 | Attended versus unattended settlement behavior | Vanilla is attended-only plus one punitive branch (V7, V8, V9). Unattended logic can be data and Papyrus: `ScriptObject.StartTimerGameTime`/`OnTimerGameTime` (246, 464) on a running quest, state in GLOBs, actor values on the outpost or planet `LCTN` (V6 pattern), outcomes applied or withheld by script; suppressing vanilla attacks needs no script edit (`AllowAttacks` is per beacon and const, V7; a condition on the branch `349C9F` or its nodes, all unoverridden today, V3) | **No native hook needed** for Foothold's own attacks. **Hook candidate (weak, not opened)** only if Foothold must keep vanilla human attacks and also stop V9's destruction without replacing `SQ_OutpostAttackHumansScript` (a script override conflicts with any other mod that replaces it). Data path to try first: gate the vanilla branch, run our own attacks |
| M5 | Safe cancellation, cleanup, and save/reload persistence | `Quest.Stop` (238) with the vanilla `CheckForCleanup` pattern (V8); `ObjectReference.Delete`/`Disable` (299, 302); `PlaceAtMe` persistence flags (`docs/design/persistence.md` PS-08, open question PS-09); quest stage and GLOB state are save data (grade C, earlier games); `Quest.OnQuestShutdown` (257) | **No native hook needed**; save/reload is unproven until M5.C2-style rows exist (D) |
| M6 | Compatibility with vanilla outpost assaults and current DLC behavior | Add, do not edit: Terran Armada adds a node with `SNAM` after vanilla (V3) and its own keyword and CNDF (V13); vanilla's attack quests can be observed by script (`Quest.OnQuestStarted`, `Quest.psc:261`, remote registration grade C) instead of overriding them; the 2000-unit distance GLOB is shared (V15); Free Lanes uses GLOB cooldowns (S4) and its AV feeds Terran attacks (V13) | **No native hook needed**; the compatibility test is "vanilla, Free Lanes and Terran Armada attacks still fire with our plugin loaded" (D) |

## 4. Proposed generic fixture for M5.H2 (hypothesis)

**Status: hypothesis.** Nothing was built or run for it. It is the smallest plugin that
exercises M1, M2, M3 (spawn and hostility only), M5 and the save/reload half of M4, at the
first POI, with Starfield.esm as the only master.

### 4.1 What it is

- **Where.** The first POI: `CityNewAtlantisApartmentTEMPLATE` `Starfield.esm:21966F`, near
  the crate position of `docs/poi/first-poi.toml` (-17, -4, 4.63). That cell has **no
  navmesh** (`docs/recipes/actors.md`), so spawned actors may not path: the fixture
  checks spawn, faction, hostility and cleanup, not combat movement. A navmeshed variant is
  the actors recipe's embassy cell, later.
- **Trigger.** A console-set quest stage: `setstage KitKraftHostileFixtureQuest 10` spawns,
  stage 20 cancels and cleans up. The quest has **no fragments**: the reaction lives in one
  compiled KitKraft script on a placed controller reference, the mechanism the scripts
  recipe proved at build time (`docs/recipes/scripts.md`: compile, attach through `VMAD`,
  property names and types checked; in game still pending, owner row 14 / O4-07).
- **Actors.** `LvlStarborn_Aggro` `Starfield.esm:27025D`, the vanilla Starborn outpost
  attacker (V10, V17), two of them, placed by `PlaceActorAtMe` at the controller.
- **Cooldown.** A GLOB holding the earliest `GameDaysPassed` of the next spawn (the S4
  pattern); 0 at build, so the first trigger always fires.
- **Telemetry for probes.** The script writes the live spawned count into a GLOB and
  traces each step; no notification is the verdict.

### 4.2 Spec (build v3), with the parts not proven named

```toml
# HYPOTHESIS (M5.H1 proposal for M5.H2). Not built. See docs/research/hostile-encounters-2026-10-16.md section 4.
version = 3

[load_order]
plugins = ["Starfield.esm"]

[[new]]                      # typed-writable GLOB (newform_tests.rs)
name = "spawned"
type = "GLOB"
edid = "KitKraftHostileFixtureSpawned"
fields = { FLTV = 0.0 }

[[new]]
name = "cooldown"
type = "GLOB"
edid = "KitKraftHostileFixtureNextDay"   # GameDaysPassed before which a trigger is refused
fields = { FLTV = 0.0 }

[[new]]                      # GAP G1: QUST is not typed-writable; stages 10 and 20 need the Stage group
name = "quest"
type = "QUST"
edid = "KitKraftHostileFixtureQuest"

[[place]]
name = "controller"
base = "Starfield.esm:03CCF4"            # ActivatorSwitch01, the scripts recipe's base
cell = "Starfield.esm:21966F"            # CityNewAtlantisApartmentTEMPLATE (first POI)
position = [-15.0, -4.0, 4.63]
persistent = true
edid = "KitKraftHostileFixtureController"
brief = "M5.H2"
# GAP G2: object properties (Quest, ActorBase, GlobalVariable) and `new:` names inside
# script properties are not proven; the recipe proved String and Int only.
scripts = [
  { name = "KitKraft:HostileFixtureController", properties = {
      FixtureQuest = "new:quest", SpawnStage = 10, CleanupStage = 20,
      SpawnBase = "Starfield.esm:27025D", SpawnCount = 2,
      SpawnedGlobal = "new:spawned", NextDayGlobal = "new:cooldown", CooldownDays = 0 } },
]
```

The script (to be written in M5.H2, ours, in `docs/recipes/scripts/KitKraft/`), as
behaviour only: on `OnLoad`, register for the quest's remote `OnStageSet`; on the spawn
stage, refuse when `GameDaysPassed` is below the cooldown GLOB, else place `SpawnCount`
actors, keep them in an array, set the spawned GLOB, set the cooldown GLOB to now plus
`CooldownDays`; on the cleanup stage, `Delete` every kept actor and set the GLOB to 0; on
`OnUnload`, do nothing to any object (the nonpunitive constraint, section 5). Every
declaration it needs is in section 3 (`OnStageSet`, `PlaceActorAtMe`, `Delete`,
`GlobalVariable.GetValue`/`SetValue`); `RegisterForRemoteEvent` on a `Quest` is grade C.

Gaps the spec depends on (each a request, not part of M5.H1):

| Gap | What | Fallback that builds today |
| --- | ---- | -------------------------- |
| G1 | `[[new]]` QUST with two stages (Stage group: `INDX`, `QSDT`) and "start game enabled" or console start; QUST is typed-readable `fail` (45.8 %) and not typed-writable (`kit coverage --matrix --types QUST`) | Trigger by GLOB: `set KitKraftHostileFixtureTrigger to 1` (spawn) / `2` (cleanup), the controller polling it on a 1 s `OnTimer` while loaded; or a CK-made quest imported later (owner-assisted, addendum item 5) |
| G2 | Object properties in `scripts` (and `new:` names inside them), checked against the declared type | Hard-code the vanilla forms in the script and keep only Int properties |
| G3 | The scripts recipe in game (O4-07 / M5.B3) | None: M5.H2's game rows wait for it |

### 4.3 Probes `testplan` would emit (M4.T2 format, hypothesis)

Row ids are proposals; every expected value comes from the plugin's own records.

| Row | Command(s) | Matcher | Expected |
| --- | ---------- | ------- | -------- |
| H2-01 | `coc CityNewAtlantisApartmentTEMPLATE`; `help KitKraftHostileFixtureController 4` | contains | the controller's EditorID and runtime ID |
| H2-02 | `prid <controller>`; `getpos x`, `getpos y` | number within 1.0 | -15.0, -4.0 |
| H2-03 | `getstage KitKraftHostileFixtureQuest` | number | 0 |
| H2-04 | `getglobalvalue KitKraftHostileFixtureSpawned` | number | 0 |
| H2-05 | `setstage KitKraftHostileFixtureQuest 10`; wait 2 s; `getglobalvalue KitKraftHostileFixtureSpawned` | number | 2 |
| H2-06 | `Papyrus.0.log` | papyrus-log contains | `KitKraftHostileFixture spawn 2` (the script's trace) |
| H2-07 | screenshot | manual | two Starborn actors hostile to the player (faction `018EDE`, V16) |
| H2-08 | `setstage KitKraftHostileFixtureQuest 20`; `getglobalvalue KitKraftHostileFixtureSpawned` | number | 0 (cleanup) |
| H2-09 | `set KitKraftHostileFixtureNextDay to <GameDaysPassed + 1>`; `resetquest KitKraftHostileFixtureQuest`; `setstage KitKraftHostileFixtureQuest 10`; `getglobalvalue KitKraftHostileFixtureSpawned` | number, plus papyrus-log contains `cooldown` | 0 (refused by the cooldown) |
| H2-10 | after a spawn: `save KitKraftSession-<date>-H2`; `load` it; `coc` back; `getglobalvalue KitKraftHostileFixtureSpawned`; `getstage KitKraftHostileFixtureQuest` | number | 2 and 10 (the save/reload half; whether placed actors persist is PS-09) |
| H2-11 | after a spawn: `coc` to another cell, wait 1 game hour (`set gamehour to ...`), `coc` back; `help KitKraftFirstPOICrate 4` when the POI plugin is loaded | contains, plus manual | the crate still there, nothing destroyed (unattended, nonpunitive) |
| H2-12 | with the plugin loaded: the vanilla deterministic attack (`CheckForAttack` with force and type 1, V6) on a test outpost | manual | a vanilla attack quest still starts (compatibility, M6) |

`validation` today: every row `not-run`; `result: hypothesis`.

## 5. Design constraints from the addendum (verbatim)

Owner, addendum item 4, recorded as future Foothold design constraints: "Record as future
Foothold design constraints: optional/configurable attacks, meaningful defenses, reliable
notifications, nonpunitive unattended resolution, and no arbitrary loss of accumulated
player investment."

How the corpus bears on them (ours, not the owner's): vanilla rolls attacks only while
the player is present (V7) and has a per-beacon off switch that is const (V7), so
"optional/configurable" needs our own gate; vanilla's unattended branch (V9) is the
opposite of "nonpunitive" and "no arbitrary loss", so a Foothold attack system must not
inherit `SQ_OutpostAttackHumansScript`'s unload behaviour; "reliable notifications" has
no vanilla source read here (L1), so it is ours to build and probe.

## 6. Hand-off

```yaml
id: M5.H1
priority: P1
owner: lane H (hostile-encounter research)
requires: []
output: docs/research/hostile-encounters-2026-10-16.md; docs/source-registry.md row PRIOR-ART-ATTACKS
source_provenance: [corpus:kit query/inspect/refs over the 14 official ESMs, corpus:kit papyrus events (ContentResources.zip), corpus:kit papyrus disasm (Starfield - Misc.ba2), docs-only:PRIOR-ART-ATTACKS]
validation:
  parse: not-run
  byte_roundtrip: not-run
  ck_open: not-run
  ck_reexport: not-run
  game_runtime: not-run
  save_reload: not-run
limits: [VMAD beyond 256 bytes unread (L1), CNDF untyped (L2), DLC scripts unsearched (L3), fixture needs QUST stage writes (G1) and object script properties (G2), Nexus permission tabs unread]
result: hypothesis
```
