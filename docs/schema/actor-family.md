# Actor family: what the schema says, and what it does not

For mod authors who want to read or patch Starfield's actors with KitKraft: actors (NPC_),
leveled actors (LVLN), races (RACE), outfits (OTFT), classes (CLAS), companion affinity
events (AFFE), perks (PERK), spells (SPEL) and magic effects (MGEF). The definitions are in
`crates/schema/defs/<SIG>.toml`; `kit schema --type NPC_` prints any of them with sources
and confidence.

Everything below was measured on the 14 official ESMs on 2026-10-14. Layouts come from the
pinned xEdit definitions (`wbDefinitionsSF1.pas` and `wbDefinitionsCommon.pas` at
`9fb01688`, ADR 0004); each definition cites its line ranges. "corpus" confidence means
every official record of the type decodes with every subrecord placed in a member, nothing
left over, and encodes back to the same bytes. Members that never occur in an official file
stay at `reference` confidence (documented upstream, not yet seen).

## Coverage

Subrecord counts are occurrences in the 14 files. "Typed" counts subrecords placed in a
member with a layout (a struct with one raw field still counts); "opaque" counts subrecords
kept as raw bytes (opaque members and component-block bodies).

| Type | Records | Files | Subrecords | Typed | Opaque | Opaque members | Never seen (`reference`) |
| ---- | ------: | ----: | ---------: | ----: | -----: | -------------- | ------------------------ |
| NPC_ | 8,447 | 6 | 1,121,511 | 1,110,969 | 10,542 | VMAD, component blocks, NAM5, ONA2 tail, COED | SNBH, DEFL, destructible block, attacks, OCOR, GWOR, FCPL, RCLR, RPLD, RPLI, COED, CS3K, the template FULL |
| LVLN | 298 | 5 | 5,342 | 5,321 | 21 | VMAD, component block, COED | OPDS, both XALG, list conditions, LVLG, COED, LLKC, MOLM, MODC, XFLG |
| OTFT | 698 | 5 | 1,396 | 1,396 | 0 | VMAD, component block | VMAD, component block |
| CLAS | 8 | 1 | 40 | 32 | 8 | VMAD, component block, DATA | VMAD, component block, ICON |
| AFFE | 1,593 | 4 | 24,261 | 24,261 | 0 | - | - |
| RACE | 47 | 7 | 12,997 | 12,874 | 123 | VMAD, component blocks, junk FLLD, chargen NNAM, SPED | VMAD, XALG, APPR, SNMV, FDDS, NAM8 |
| PERK | 608 | 8 | 30,209 | 25,608 | 4,601 | VMAD, effect DATA, EPFD, ATAV | - |
| SPEL | 662 | 6 | 13,305 | 13,305 | 0 | VMAD, component block | VMAD, component block, MDOB |
| MGEF | 989 | 6 | 5,389 | 5,194 | 195 | VMAD, component block | MDOB, ESCE |

Every NPC_ record in the official files is compressed; the check decodes each one with the
container's default decode limits. The out-of-tree pass over all 8,447 takes 5.4 s in
release (all 14 files, decompression included); `kitkraft-esm`'s `corpus_full_roundtrip`
typed check (M2.A3) with this family in place reports 60,077 typed records, 2,155,517
placed subrecords and 0 conflicts on the 14 files.

New shared definitions (`defs/common/`): `AttackData` (ATKD..ATKT; RACE, and NPC_ at reference) and `Effect`
(EFID..MUID; SPEL, and every other magic item once its family references it). Existing
blocks reused: `Condition`, `Keywords`, `BfcbBlock`, `ObjectBounds`,
`ObjectPaletteDefaults`, `Transforms`; at the integrator's ACTOR merge (M4.BX1) the
family's own sound reference was folded into the existing `SoundReference` (the same 40
bytes, flat: `StartEvent`, `StopEvent`, `Condition`, `StartForm`; NPC_ CS3S, RACE
WED0/WED1, SPEL PUSH/PDSH, MGEF ESSH), LVLN's model group into `Model` and NPC_'s
destructible block into `Destructible`.

## Actor templates and inheritance

An NPC_ can take whole sections from another actor instead of storing them:

- `ACBS.TemplateFlags` says which sections are inherited: `Traits`, `Stats`, `Factions`,
  `SpellList`, `AiData`, `AiPackages`, `ModelAnimation`, `BaseData`, `Inventory`,
  `Script`, `DefPackList`, `AttackData`, `Keywords`, `ReactionRadius`, `CombatStyle`.
- `TPLT` (`DefaultTemplate`) is the template for every inherited section; `TPTA`
  (`TemplateActors`, 15 FormIDs in the same order as the flags) names a template per
  section, 0 meaning none. Templates can be an NPC_, a leveled actor (LVLN) or a BMMO.
  4,749 official actors have TPLT and 6,082 have TPTA.
- `LTPT` (`LegendaryTemplate`) and `LTPC` (`LegendaryChance`, a global) give a second
  template used for legendary spawns (46 and 1,392 actors).
- `ACBS.Flags.CalcForEachTemplate` and `AutoCalcStats` change how inherited stats are
  computed; `PcLevelMult` turns `ACBS.Level` into a multiplier of the player's level,
  which is why `Level` is kept raw (see below).

When you patch an actor that inherits a section, the values stored in that section of
its own record are ignored by the game; edit the template, or clear the flag. KitKraft
does not resolve templates for you: `inspect` shows the stored values.

## Leveled actors

LVLN is a leveled list of NPC_ and LVLN entries. Per entry (`Entry` group): `LVLO` with
`Level`, the actor (`Npc`), `Count` and the entry's `ChanceNone`, followed by optional
conditions. List-level members: `ChanceNone` (LVLD, a float), `MaxCount` (0 everywhere),
`Flags` (`CalculateFromAllLevels`, `CalculateForEachItemInCount`, `CalculateAll`,
`EvalAsStack`, `AllowShiftUp`, `DoAllBeforeRepeating`, ...), `UseGlobal`, an
`OverrideName` (3 records) and a model block (10 records carry a MODL; every record has
FLLD).

`LLCT` (`EntryCount`) is a separate subrecord holding the number of entries. It is stored,
not derived: the format cannot tie a count subrecord to a group's repetitions, so if you
add or remove entries, update `EntryCount` in the same edit. The same holds for NPC_'s
`Perks.Count` (PRKZ), `ContainerItems.Count` (COCT), `ActorSounds.Count` (CS3H),
`TintCount` (EDCT) and `ObjectTemplate.Count` (OBTE), and for PERK's
`ActivityTracker.Count` (ATCP) and `Argument.ConditionCount` (CITC).

## Outfits

OTFT is a list of ARMO and LVLI FormIDs (`Items`, 0 to 3 in the official files). An NPC_
wears `DefaultOutfit` (DOFT) and, in space, `SpaceOutfit` (SOFT). Starting inventory is
separate: `ContainerItems` (COCT, then CNTO entries of item and count).

## Perks, spells and effects

How the types link, by member path:

- NPC_ `Perks.Perk` (PRKR: `Perk`, `Rank`) lists perks the actor has; NPC_ and RACE
  `ActorEffects` (SPLO) list spells and abilities.
- PERK `Rank.Effect` entries have a `Header` (PRKE: `Type` is `QuestStage`, `Ability` or
  `EntryPoint`, plus rank and priority). The effect's `Data` (an ability spell, a quest
  stage, or an entry point and function) is a union selected by `Type`, kept raw; so is
  the entry point's parameter `Data` (EPFD), selected by `FunctionParameters.Type` (EPFT).
  Perk conditions (`PerkCondition`: a tab index and its conditions) are typed.
- PERK `Data` has `Category`, `SkillGroup`, `CrewAssignmentRequirement` and `Flags`
  (`Trait`, `Playable`, `Hidden`, `Crew`, `Background`, `HiddenOnDataMenu`). Ranks carry
  their own `Description`, `UpgradeCost` and an optional `ActivityTracker` (the challenge
  that unlocks the next rank).
- SPEL `Data` (SPIT) has the cost, flags, `Type` (`Spell`, `Ability`, `Addiction`, ...),
  cast and target types and the casting perk; its `Effects` (shared `Effect` group) point
  at MGEF through `BaseEffect` (EFID) with `Magnitude`, `Area`, `Duration` (EFIT),
  conditions and optional override globals.
- MGEF `Data` (136 bytes) has the `Archetype` (`ValueModifier`, `Stimpak`, `Damage`,
  `Boostpack`, ... 56 values), the magic skill and resist actor values, art, shaders,
  `EquipAbility` (a SPEL), `PerkToApply` (a PERK), projectile and explosion, cost, flags and
  taper values. Three slots whose meaning depends on the archetype stay raw (see below).

## Crew and companions: where the data lives

- **Companions.** The five NPC_ records with `AffinityEventQuest` (QSTA) and
  `AffinityEventDialogue` (BNAM) are the companions (`Companion_Andreja`,
  `Companion_Barrett`, `Companion_SarahMorgan`, `Companion_SamCoe`, `COM_CoraCoe`). AFFE
  records are the events they react to: each `Reaction` pairs a companion (`Actor`, an
  NPC_) with a global holding the reaction (`ReactionValue`); `EventSize`,
  `DistanceToPlayer`, `CooldownInMinutes`, `RequiredFaction` and `AddToEvent` qualify the
  event. What triggers an event and what approval does are quest, dialogue and script
  behaviour (QUEST family; scripts stay opaque VMAD).
- **Crew.** Crew skills are perks: PERK `Data.Flags.Crew` and
  `Data.CrewAssignmentRequirement` (`AnyRole`, `Ship`, `Outpost`); an actor's crew skills
  are its `Perks.Perk` entries with their `Rank`. What makes an actor hireable or
  assignable is not in a member of this family: look at its factions (`Factions`, SNAM:
  faction and rank), keywords, scripts (VMAD, opaque) and the quests that recruit it.
- **Appearance.** Face and body data are typed on NPC_ (`HeadParts`, `Tint` groups,
  `BodyMorphRegionValues`, `FaceDialPosition`, `FaceMorph`, `MorphBlend`, the colour
  strings, `SkinToneIndex`, `Pronoun`). KitKraft cannot generate face data; copy it from an
  existing actor.

## Opaque members, and why

- **VMAD** (every type): script property values are unions by property type; PERK's adds
  script fragments.
- **Component blocks** (BFCB..BFCE): kept as raw subrecords, as in every family.
- **NPC_ `ACBS.Level`**: a level or, with `PcLevelMult`, a level multiplier in
  thousandths; a union selected by a flag.
- **NPC_ `NAM5`**: unnamed upstream; `FF 00` in all 8,447 records.
- **NPC_ OBTS property values**: since the integrator's ITEMS merge (M4.BX1) the object
  template is the shared `ObteBlock` and OBTS its typed `TemplateItem` (includes and
  property list); each property value is a union and stays raw.
- **NPC_ `RaceOverrides.Overrides`** (ONA2 after its flags): one block per set flag, in
  flag order; presence decided by the flags.
- **`COED`** (NPC_, LVLN): not in the official records; COED's owner union is kept raw.
  (The NPC_ destructible block, also absent from the official records, uses the shared
  `common/Destructible.toml` since the integrator's ACTOR merge, M4.BX1.)
- **RACE `Data.Unnamed`** (DAT2 offsets 92 to 127): unnamed words and an unnamed float
  upstream. RACE chargen `Nnam` (12 bytes) and the junk `Flld` are unnamed too. `SPED`
  (one record) keeps its nested speed tables raw.
- **PERK effect `Data`, `EPFD`**: unions (above). **`ATAV`**: activity configuration text
  without a terminator that may embed FormIDs.
- **CLAS `Data`**: one unnamed 4-byte value.
- **MGEF `Data.AssociatedItem`, `AssociatedActorValue1`, `AssociatedActorValue2`**: unions
  by archetype (a light, bound item, summoned actor, hazard, spell, damage type,
  enchantment, keyword, or an actor value).

Raw FormIDs inside opaque members are not visited by `refs` or `formid.dangling` and are
not remapped on copy; `clone` (M4.E1) is specified to refuse such a source when its
masters differ from the target's.

## Things to know before editing

- Counts are stored, not derived (list above): change them together with their lists.
- `PRPS.ActorValue` on NPC_ and RACE (like PKIN's) names actor values below 0x800 that no
  official file defines: 318 such errors in Starfield.esm (NPC_ 139: 0x385 x102, 0x32D
  x17, 0x330 x7, 0x3A2 x7, 0x39E x5, 0x351 x1; RACE 178: 0x2D8, 0x2D9, 0x2E0 x31 each,
  0x39E, 0x39F, 0x3A0, 0x3A1 x21 each, 0x385 x1), and the same values as warnings in the
  DLC files. They decode identically everywhere; they are most likely forms the engine
  defines itself (a CK question below). One more error is a stale value:
  `NPC_:0018B5F4` (`UC_NA_InfinityLTD_FayeSengsavahn`) stores `RDSA.SpecificReference =
  00577823`, which Starfield.esm does not define, in an entry whose flags do not set
  `UseSpecificReference`.
- Corpus over upstream: NPC_ `EditorId` and `ObjectTemplateEnd` (STOP; absent from 3,551
  records) are optional, NPC_ and RACE `ActorEffects` have no SPCT count, AFFE
  `ReactionValue` is optional (one official reaction lacks it), RACE subgraph entries use
  the order SADD SAKD* STKD* SGNM SAPT* SRAF (upstream marks them unordered).
- PERK `Data` is 4 bytes in every official record; upstream allows a 3-byte form without
  `CrewAssignmentRequirement`, which would stay raw.
- One official PERK condition carries a `StringParameter1` (CIS1, 40 bytes); the shared
  `Condition` block still marks CIS1 `reference` (raising it is the integrator's, who owns
  that block).

## Open questions for a CK observation (lane H)

1. Actor values below 0x800 in `PRPS` (NPC_, RACE, PKIN): does the CK list them (by name)
   in the actor value picker, and does it write them unchanged on save?
2. Does the CK write `STOP` on an NPC_ without an object template, and keep the stored
   counts (PRKZ, COCT, CS3H, EDCT, OBTE, LLCT) in sync when entries are added or removed?
3. `ACBS.Level` with `PcLevelMult`: how the CK shows and writes the multiplier.
4. `TPTA` versus `TPLT`: which template the CK uses for a section whose flag is set and
   whose TPTA slot is 0.
5. `ONA2`: which CK controls set each override flag, and the order of the blocks it writes.
6. Does the CK clear `RDSA.SpecificReference` when `UseSpecificReference` is off?
7. PERK entry points: does the CK's function parameter type (EPFT) match the EPFD sizes
   seen (4 and 8 bytes), and does it ever write a 3-byte PERK `Data`?
8. NPC_ `NAM5` (always `FF 00`), RACE `Data.Unnamed` and the chargen `Nnam`: any CK field
   that changes them.
