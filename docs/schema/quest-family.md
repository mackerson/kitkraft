# Quest family: what the schema says, and what it does not

For mod authors who want to read or patch Starfield's quests, dialogue and AI with
KitCraft: quests (QUST) with their stages, objectives and aliases; dialogue topics (DIAL),
their responses (INFO) and scenes (SCEN); AI packages (PACK); factions (FACT); the story
manager tree (SMEN, SMBN, SMQN); and challenges (CHAL). The definitions are in
`crates/schema/defs/<SIG>.toml`; `kit schema --type QUST` prints any of them with sources
and confidence.

Everything below was measured on the 14 official ESMs on 2026-10-14. Layouts come from the
pinned xEdit definitions (`wbDefinitionsSF1.pas` and `wbDefinitionsCommon.pas` at
`9fb01688`, ADR 0004); each definition cites its line ranges. "corpus" confidence means
every official record of the type decodes with every subrecord placed in a member, nothing
left over, and encodes back to the same bytes. Members that never occur in an official file
stay at `reference` confidence (documented upstream, not yet seen).

## Coverage

"Typed" counts leaf members (subrecords, after expanding shared groups) whose whole
payload has a layout; "with raw slots" are structs that keep a padding or union slot as
bytes (CTDA's union fields, VMAD after its header, unused padding); "raw" members are kept
entirely as bytes.

| Type | Records | Files | Leaf members: typed / with raw slots / raw | Raw members | Never seen (`reference`) |
| ---- | ------: | ----: | ------------------------------------------ | ----------- | ------------------------ |
| FACT | 953 | 5 | 21 / 5 / 3 | component block | VMAD, ranks, CITC conditions |
| QUST | 2,610 | 7 | 162 / 12 / 11 | component block, ALFG, create-matching ALCA, ALAM, COED | SNAM, COED |
| PACK | 4,578 | 5 | 38 / 12 / 4 | component block, data-input CNAM | component block, CNAM combat style |
| DIAL | 87,031 | 6 | 11 / 0 / 3 | component block | - |
| INFO | 162,028 | 6 | 49 / 5 / 6 | component block, NAM9, INTV, animation DTGT | TPIC, TROI, SOFT, SPOR, SHRT, DTGT, ACEP, animation conditions |
| SCEN | 9,673 | 6 | 51 / 4 / 2 | action type-specific subrecords, VNAM | - |
| SMBN | 173 | 5 | 8 / 1 / 0 | - | - |
| SMQN | 471 | 5 | 13 / 1 / 0 | - | - |
| SMEN | 22 | 3 | 9 / 1 / 0 | - | conditions |
| CHAL | 10 | 2 | 1 / 0 / 3 | component block (all challenge data) | - |

All 267,549 records decode and re-encode byte-identically, with nothing in `extra`, through
an out-of-tree check over the generated decoder (every official record of the ten types;
none of them is compressed). The whole typed check of the ten types over the 14 files
takes about 6 s in release (INFO and DIAL dominate: 162 k and 87 k records). INFO was
expected to be order-unstable like REFR; it is not: every official INFO follows the
upstream member order, so no member falls back to `extra`.

New shared definitions (`defs/common/`): `ScriptAttachment` (the VMAD payload; a type),
`PackageLocation` (FACT PLVD, PACK PLDT), `QuestReferenceAlias` (a reference alias, used
twice by QUST), `SoundReference` (INFO WED0 and RVSH). The condition block is PLANET's
`Condition`, reused unchanged. Its optional string parameters CIS1 and CIS2 are
`reference` in that file, but they occur here (CIS1 in QUST, PACK and SCEN; CIS2 in QUST,
PACK, INFO and SCEN) and decode identically; raising them to `corpus` is a request to the
integrator, who owns the shared file.

## Where the records live

- QUST, PACK, FACT, SMBN, SMQN, SMEN and CHAL are in their top-level groups.
- DIAL and SCEN records are **not** in top-level groups: every official one sits in the
  children group (group type 10) of its quest. The group's label is the quest's FormID: all 87,031
  topics name that same quest in `Quest` (QNAM). Scenes are looser: 6,028 of 9,673 name
  their enclosing quest in `ParentQuest` (PNAM), 3,645 name another quest.
- INFO records sit in the children group (group type 7) of their topic. Within each file every one of the 162,028 responses is
  listed in its topic's `Infos` (TIFL); 25 TIFL entries name a response that is not in
  the topic's group of the same file.

## Quests (QUST)

**General data.** `General` (DNAM, 12 bytes): flags (start game enabled, run once,
displayed in HUD, ...), priority, padding. `QuestType` (QTYP) and `QuestFaction` (FTYP)
are keywords, not records of their own (FTYP is not a FACT). `Event` (ENAM) is the story
manager event that starts the quest, a four-character code (ADIA, CLOC, SCPT, AOBJ, ...).
`Location`, `QuestTimeLimit` (a global), `SourceQuest`, `TextDisplayGlobals` (QTGL, one
subrecord per global), `Filter` and `Summary` follow.

**Two condition lists.** Dialogue conditions come first, then an empty `NEXT` marker, then
the story manager conditions. Neither has a CITC count in the official files.

**Stages.** Each `Stage` is an `INDX` (stage index, flags such as run on start, start quest
timer, ready to display on UI) followed by `LogEntry` groups: stage flags (complete or fail
the quest), conditions, a note, a script flag comment, the log text (`CNAM`, a string-table
ID), then the reward block: `RewardCount` (QSRD, 4 bytes), `Reward` groups (XP global,
currency global, item lines, conditions), the closing `RewardEnd` (QSRD again, empty) and
an optional affinity change. 19,406 stages in 1,528 quests.

**Objectives.** `Objective`: index (QOBJ), flags (ORed with previous, no stats tracking),
display text (NNAM, a string-table ID), then `Target` groups: QSTA (the reference alias
ID, flags such as compass marker ignores locks, a keyword) with its conditions. 3,352
objectives in 630 quests.

**Aliases.** `NextAliasId` (ANAM) is followed by the aliases, of three kinds:

- *Reference alias* (`QuestReferenceAlias`, ALST..ALED): ID, name, flags (optional,
  quest object, essential, allow reuse, ...), then **one fill type**, chosen by which
  subrecords are present: a specific reference (ALFR), the unique actor (ALUA) or generic
  base form (ALUB), a reference in a location alias (ALFA + ALRT location ref type), an
  alias of another quest (ALEQ + ALEA), a created reference (ALCO object, ALCA at/in which
  alias, ALCL level), a created matching reference (ALCM + ALCA), or a found reference (in
  a collection ALCS, closest to ALCC, from an event ALFE + ALFD, near an alias ALNA + ALNT
  + ALNR). Then match conditions, keywords, an inventory (COCT + CNTO), override package
  lists, linked aliases, display name, death item, forced voice, spells, factions,
  packages, voice types and terminals. 25,882 in the official files.
- *Location alias* (ALLS..ALED): ID, name, flags, a fill type (specific location, the
  location of a reference alias by keyword, an external alias, or from an event), match
  conditions, an optional planet content request (ALPS, its conditions, the request
  keyword), closest-to / current system / current planet aliases, a location type
  keyword, a display name. 3,852.
- *Reference collection alias* (ALCS, ALMI max initial fill, ALAM, then a full reference
  alias that every member is filled like). 3,995.

The three kinds interleave in creation order, which a forward-only member list cannot
follow. The schema therefore stores a list of `Aliases` entries; each holds a run of
reference aliases, then location aliases, then collection aliases, and the next alias of
an earlier kind starts a new entry. Reading the entries in order, and each entry's three
lists in order, gives the quest's alias list; nothing is lost or reordered on write.

**Mission board** (151 quests): the mission type keyword, a summary, up to six detail
lines (title, primary, secondary text) and keywords.

**Scripts.** VMAD on 1,703 quests: the header (version 6, object format 2, script count)
is typed; the scripts, their properties and the stage and alias fragments are raw (see
"Opaque members").

## Dialogue: topics, responses, scenes

**DIAL (topic).** `Quest` (QNAM), an optional `Branch` (BNAM, a DLBR) for player dialogue,
`Priority` (50 by default), keyword and affinity event, `Data` (topic flags, category:
player, command, scene, combat, favor, detection, service, miscellaneous; a subtype
index), the same subtype as a four-character code (`SubtypeName`, SNAM: CUST, SCEN, HELO,
...), and `Infos` (TIFL), the list of its responses, with their count in TIFC. Only 265
topics have an EditorID; 301 have a player-visible name (FULL).

**INFO (response).** `ResponseFlags` (ENAM: a flags word and the reset time), optional
`SharedInfo` (DNAM: reuse another response's lines) and `InfoGroup` (GNAM: 61,464 responses
belong to an info group), then the `Response` lines (164,632 in all): TRDA (emotion
keyword, voice file number, emotion out), per-voice-type emotions (TROT), the text (NAM1,
a string-table ID), script notes, edits and alternate lip text (NAM2..NAM4), a text hash,
an optional animation block (BNAM idle, subgraph, body/face/flavor archetype keywords,
markers, delay, action, ended by XNAM), an optional head-tracking block between two HNAM
markers, and a sound. After the lines: conditions (57,098 in 37,623 responses; no CITC),
the prompt (RNAM, the player's line), speaker, `StartScene` (TSCE), voice override,
quest stages to set on begin/end (TIQS), the scene phase to start, a reset global,
subtitle priority, affinity event, speech challenge and perk.

**SCEN (scene).** Flags, then `Phase`s (between two HNAM markers: name, start and
completion conditions each ended by NEXT, editor width, flags, quest stages), `Actor`s
(alias ID, flags, behaviour flags), `Action`s, the `ParentQuest`, last action index,
conditions, quest stages, notes, template scene, index, priority, subtitle priority
override, and the speech challenge block (risk, reward, mandatory and excluded next
challenges, AND/OR keywords, perks). Every action has a typed header: `Type` (dialogue,
package, timer, player dialogue, start scene, radio, move, camera, FX, animation,
timeline), name, notes, actor alias, index, flags, start and end phase; the rest of the
action is raw up to its empty ANAM end marker (see "Opaque members"). 46,885 actions.

**How the pieces link.**

```
QUST --(children group, type 10)--> DIAL, SCEN
DIAL.Quest (QNAM) ------------------> QUST            (the owning quest)
DIAL --(children group, type 7)---> INFO
DIAL.Infos (TIFL) ------------------> INFO            (the topic's response list)
INFO.InfoGroup (GNAM), SharedInfo --> INFO
INFO.StartScene (TSCE) -------------> SCEN            (a response that starts a scene)
SCEN.ParentQuest (PNAM) ------------> QUST
SCEN dialogue action (raw DATA) ----> DIAL            (the topic an actor says)
PACK data input (STSC) -------------> SCEN
SMQN.Quest (NNAM) ------------------> QUST            (story manager starts quests)
```

A scene's dialogue actions name their topic in a DATA subrecord inside the raw
type-specific part, so `refs` cannot follow scene -> topic yet; topic -> quest,
response -> scene and scene -> quest are typed.

## AI packages (PACK)

`PackData` (PKDT: general flags as a number, package or template, interrupt override,
preferred speed, interrupt flags), `Schedule` (PSDT: month, day of week, date, hour,
minute, duration), conditions (no CITC), idle animations, owner quest, anim flavor,
`Counter` (PKCU: data input count, the package template it instantiates, a version
counter). `PackageData` holds the template's input values: each `Value` is a type name
(ANAM) followed by at most one of a raw value (CNAM), a `Location` (PLDT: type, raw value,
radius, collection index), a `Target` (PTDA type and raw target, a count/distance/index,
then CITC and conditions) or a `Scene` (STSC). Package templates (188) also carry the
procedure tree: `Branch` groups in tree order (branch type, note, CITC and conditions,
PRCB root branch count and flags, procedure type, flags, data input indexes PKC2, flag
overrides PFO2). Two data input lists exist: the first (UNAM only) before XNAM, the
second (UNAM, name, public flag) after the tree. Every package ends with the on-begin,
on-end and on-change blocks (marker, idle, topics).

## Factions (FACT)

`Relations` (XNAM, 1,342 in 469 factions): a faction or race, a modifier and a combat
reaction (neutral, enemy, ally, friend), one subrecord per relation. `Flags` (DATA:
hidden from NPCs, track crime, vendor, can be owner, the ignore-crime bits, ...),
`CrimeValues` (CRVA: arrest, attack on sight, murder/assault/trespass/pickpocket gold,
steal and smuggle multipliers, escape, piracy), `Prisons` (PRIS: door marker, ship landing
marker, cargo and inventory contraband containers, companion wait marker; 9 factions),
the vendor block (`VendorBuySellList` FLST, `MerchantContainer` REFR, `VendorValues`
hours, radius, buys stolen / everything not in list / non-stolen; `VendorLocation` on 80
factions), voice, and the herd and create-group blocks. No official faction has ranks or
conditions.

## Story manager

Like the planet content trees, the story manager tree is linked **upwards**: every node
names its parent in `ParentNode` (PNAM) and its previous sibling in `PreviousNode` (SNAM).

- **SMEN** (22): an event node, a root; `Type` (ENAM) is the event as a four-character
  code. Its `ParentNode` is FormID 0x0000005B in all 22
  official event nodes; no story manager node or quest has that FormID in Starfield.esm
  (an engine-level root, not checked further).
- **SMBN** (173): a branch node with CITC + conditions, flags (random, warn if no child
  quest started) and max concurrent quests.
- **SMQN** (471): a quest node with conditions, flags (adds do all before repeating,
  shares event, num quests to run), max concurrent quests, quests to run, hours until
  reset, and the quest list: `QuestCount` (QNAM) then `Quest` entries (NNAM quest, optional
  RNAM reset time); up to 46 quests per node.

Adding a quest to the story manager means a SMQN whose PNAM names the branch (or event
node) and whose SNAM names the previous sibling, with QNAM kept equal to the number of
NNAM entries (counts are stored, not derived).

## Opaque members, and why

| Member | Why it is raw | Effect |
| ------ | ------------- | ------ |
| VMAD after its 6-byte header (all ten types; 4,413 payloads in QUST, PACK, INFO, SCEN) | each script property's value layout is chosen by its type byte, and the fragment block differs per record type; a union by a sibling field | scripts, properties and fragments are not typed; object FormIDs in them are not visited by `refs` or `formid.dangling` |
| CTDA comparison value, parameters 1-3, reference (every condition) | unions selected by the type byte, the function index and Run On (PLANET's `Condition`) | condition targets (globals, keywords, references) are not visited |
| Base form component blocks (FACT 1, QUST 1, DIAL 4, INFO 6, CHAL 10) | each block's layout is chosen by its component name | CHAL's whole content (activities, their FULL/DESC string IDs, conditions, rewards) is raw |
| SCEN action type-specific subrecords | layout chosen by the action type; the same signature means different things in different action types (HNAM marker/float/alias, SNAM phase/time, DATA topic) | topics, packages, scenes, references and keywords named by actions are not visited |
| PACK data-input CNAM, PTDA target, PLDT/PLVD value, PDTO data | unions selected by the input type text or a type field | the targets are not visited |
| INFO ENAM flags | two flag sets, chosen by the record's InfoGroup header flag | kept as a 16-bit number |
| QUST ALFG | a float upstream, but every official value is 0 or the bit pattern 0x00000002 | kept as 4 raw bytes until the CK settles it |
| QUST create-matching ALCA | 6 bytes upstream, 4 in all 13 official occurrences | raw |
| Unnamed upstream: QUST ALAM, INFO INTV and NAM9 (text hash), SCEN VNAM, FACT CRVA slot | no name or meaning in the sources | raw, named after the signature |

The DIAL subtype table (183 values) and the condition function table are not transcribed;
they stay numbers. PACK general flags stay a number (their names are in an upstream
helper outside the approved files), and PACK XNAM stays a number (upstream marks its
value names as guesses).

## Open questions (filed to lane H, `plan/STATUS.md` Requests)

1. QUST ALFG: float or integer? What CK field writes the value 2?
2. QUST create-matching ALCA: what the 4 bytes hold (upstream says a 32-bit alias plus a
   16-bit create mode).
3. QUST ALAM (collection aliases): what the CK field is (0 except twice 0x40000000).
4. INFO ENAM: which flag set the CK shows for info groups, and which ENAM bits the CK sets.
5. SCEN VNAM (20 bytes on every scene) and INFO INTV (4 bytes after TSCE): what CK fields
   they are.
6. SMEN PNAM: every event node's parent is FormID 0x0000005B (see "Story manager").
7. Whether the CK writes a CITC before quest, scene, package and response conditions on a
   new record (no official record of those has one).
