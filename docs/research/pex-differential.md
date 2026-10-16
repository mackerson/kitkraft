# PEX differential: 50 base-game scripts (M5.B2)

Lane B, 2026-10-16. `kit papyrus compile` against Bethesda's `PapyrusCompiler.exe` run by
hand, on a seeded sample of 50 base-game sources, compared by normalized semantics
(`cargo xtask oracle papyrus`, `docs/oracles.md` "Papyrus compiler oracle"). Our own eight
oracle scripts, `KitKraft:HelloOnLoad` among them, were run the same way on 2026-10-15
(`docs/oracles.md`, "Results"); this file adds the base-game sample.

No Bethesda source or compiled script is in the repository: the sources were read in place
from the owner's extraction (`docs/source-registry.md`, CK-CONTENT-RESOURCES), compiled into
a temporary folder that the harness deletes, and this report carries names, hashes and
counts only. No source text is quoted.

## Headline

- **50 of 50 scripts semantically equivalent**: 0 different after normalization, 0 failed
  to compile on either side.
- **0 of 50 byte-identical, 50 of 50 the same length** on both sides (201,021 bytes,
  4,727 instructions in all). Every byte difference is accounted for by the normalized
  fields below.
- **New since the eight-script run**: the official compiler also varies, between two
  compiles of one source with the same switches, the order of the **functions in a
  state** (18 scripts), of the **states** (2) and of a **struct's members** (2), besides
  the string table, user flags, properties and variables already recorded. Reproduced on
  the official compiler alone (below), so these are the compiler's, not KitKraft's.
- Nothing to file: no semantic difference, so no fix to `kit papyrus compile`.

## Method

| Item | Value |
| ---- | ----- |
| Compiler | `Tools\Papyrus Compiler\PapyrusCompiler.exe` 4.7.0.5, SHA-256 `a1b7faeef399ee76b49b81d0d05ba3cc47a7fa7ec47ea355430c7ea24f660a97` (PAPYRUS-COMPILER) |
| Sources | `C:\Modding\tools\ck-sources\Scripts\Source` (5,033 `.psc`), extracted from `Tools\ContentResources.zip` SHA-256 `b5f3f66d6041193c2db139c50a8a4a2da7a80c862a95ba5eec929c7a2c42983c` |
| Hand route ("official") | From an empty working directory: `<Name> -f=<sources>\Starfield_Papyrus_Flags.flg -i=<script root>;<sources> -o=<temp>`; no `-op`, never `-release`/`-final` |
| KitKraft route | `kit papyrus compile <sources>\<path>.psc <temp> --sources-zip <Tools\ContentResources.zip>` (release `kit` at the lane's commit), which runs the same compiler with `-ignorecwd`, the script's root and its own extraction of the zip as imports |
| Comparison | Both `.pex` through `kit papyrus disasm --format json`; **equivalent** = equal after normalization (compile time, user and machine, source name's folder and case, debug modification time, offsets and sizes dropped; string table, user flags, debug tables, and each object's variables, properties, structs and members, states and their functions compared as sets keyed by name); instructions, signatures, types, flags and values compared as they are |
| Command | `cargo xtask oracle papyrus <50 paths> --out observe\oracle\papyrus-b2` (report kept locally, gitignored); exit 0 |
| Not covered | `-op` (the CK's own choice of switches is not observed: no CK sitting has happened); the CK's compile route itself |

### The sample

Seeded and stratified, so it can be drawn again: every `.psc` under the folder, sorted by
path (case-insensitive), put in one bucket by the first rule its path matches
(`Fragments\Quests\` quest fragment; `Fragments\TopicInfos\` or `Fragments\Scenes\`
dialogue fragment; other `Fragments\` other fragment; name contains `alias`; name contains
`quest` or ends in `qs`; name contains `activator`, `trigger`, `button` or `switch`; name
contains `effect`; name contains `util`, `library`, `math` or `debug`; no folder: top-level;
else other), then `random.Random(20261016).sample` per bucket: 8 quest, 5 quest fragment, 8
alias, 7 activator, 6 utility, 4 magic effect, 3 dialogue fragment, 2 other fragment, 3
top-level, 4 other. The rules look at names only, so a few land in a neighbouring bucket
(a debug quest fragment under utility); the spread is what matters. Bucket sizes over the
5,033: quest fragment 1,471, dialogue fragment 1,216, top-level 979, other 398, other
fragment 326, quest 239, alias 226, activator 118, magic effect 47, utility 13.

## Per script

Source is the first 16 hex digits of the `.psc` file's SHA-256. Instructions are counted
in our disassembly. Bytes are official / KitKraft. Differing bytes are positional, over the
common length. Normalized differences (fields that differed and were normalized):
**T** compile time; **S** string table order; **U** user flag order; **P** property order;
**V** variable order; **F** order of the functions in a state; **St** state order; **M**
struct member order; **+offsets** the byte offsets that move with a reordering.

| # | Script | Bucket | Source SHA-256 | Instructions | Bytes | Differing bytes | Verdict | Normalized differences |
| - | ------ | ------ | -------------- | -----------: | ----: | --------------: | ------- | ---------------------- |
| 1 | `CYDetonationHelperQuestScript` | quest | `0d66e11347f2378f` | 95 | 4,279 / 4,279 | 1,135 | equivalent | T P V S +offsets |
| 2 | `SFBGS00D:Fragments:Quests:QF_SFFL_QuestFragmentScript` | quest | `cd9f8fcd93acc901` | 2198 | 75,788 / 75,788 | 53,148 | equivalent | T P F V S U +offsets |
| 3 | `MQTutorialQuestPlayerScript` | quest | `dce93a0a9c925fd0` | 6 | 691 / 691 | 51 | equivalent | T S U |
| 4 | `MQ00QuestScript` | quest | `8c784a6c86e06d3c` | 143 | 6,373 / 6,373 | 987 | equivalent | T P V S U +offsets |
| 5 | `RI07_QuestScript` | quest | `f5e891df183ef165` | 25 | 1,416 / 1,416 | 485 | equivalent | T F S U +offsets |
| 6 | `RIR03_QuestScript` | quest | `09ee95de426a1d83` | 28 | 1,562 / 1,562 | 294 | equivalent | T P V S U +offsets |
| 7 | `LC082_VigilanceQuestScript` | quest | `0b37b61b42914658` | 335 | 13,621 / 13,621 | 7,704 | equivalent | T P F M V S U +offsets |
| 8 | `MQ_TempleQuestScript` | quest | `8f270a2bd3ef2e18` | 174 | 7,153 / 7,153 | 3,361 | equivalent | T P F V S U +offsets |
| 9 | `Fragments:Quests:QF_MB_Passenger01Far01_000E75D5_5` | quest-fragment | `07f6dbd71dbcdb3b` | 3 | 558 / 558 | 61 | equivalent | T S U |
| 10 | `Fragments:Quests:QF_OE_KT_Trait_Wanted_Bounty_000E872F` | quest-fragment | `8761ff4355d54b08` | 233 | 12,091 / 12,091 | 5,814 | equivalent | T P F V S U +offsets |
| 11 | `Fragments:Quests:QF_DialogueWaggonerFarm_0100086F` | quest-fragment | `7d8f0b8bb7c07090` | 8 | 848 / 848 | 107 | equivalent | T P F V S U +offsets |
| 12 | `Fragments:Quests:QF_City_NA_FAB_Quest08_00368857` | quest-fragment | `fc16391c46bb32bb` | 18 | 1,339 / 1,339 | 1 | equivalent | T |
| 13 | `Fragments:Quests:QF_UC_NA_CL_FarmScene_00390D2F` | quest-fragment | `af9c1f26b5cbe062` | 15 | 1,357 / 1,357 | 1 | equivalent | T |
| 14 | `OE_MissionTerminalAliasScript` | alias | `aa3f8f5165d990f1` | 9 | 722 / 722 | 19 | equivalent | T S U |
| 15 | `UC07_HeatleechCollAliasScript` | alias | `d10f8baa24f2b0be` | 35 | 1,802 / 1,802 | 1,277 | equivalent | T P F V S U +offsets |
| 16 | `GenericBrawlOpponentAliasScript` | alias | `6e3690fa0a9332e8` | 134 | 4,700 / 4,700 | 1,226 | equivalent | T F S U +offsets |
| 17 | `DefaultAliasOnOpen` | alias | `2ebaa857da6e5ff3` | 12 | 1,181 / 1,181 | 36 | equivalent | T S U |
| 18 | `JobAliasOnUnloadScript` | alias | `f956bd94caa99faa` | 6 | 424 / 424 | 1 | equivalent | T |
| 19 | `DefaultAliasOnShipSystemRepaired` | alias | `f5b67abcf9d2f493` | 92 | 4,368 / 4,368 | 479 | equivalent | T P M V S U +offsets |
| 20 | `COM_SMQ01_AliasScript` | alias | `bfd6df67fd9db5df` | 45 | 3,586 / 3,586 | 890 | equivalent | T P V S +offsets |
| 21 | `DefaultAliasOnEquipped` | alias | `1063ddb5d646ed58` | 13 | 1,215 / 1,215 | 1 | equivalent | T |
| 22 | `TestEnableDisableRefOnTrigger` | activator | `08a0a5478a197473` | 62 | 1,636 / 1,636 | 1 | equivalent | T |
| 23 | `TestKurtCreatureRigSwitchScript` | activator | `4322a405b17fa281` | 12 | 873 / 873 | 61 | equivalent | T S U |
| 24 | `TriggerAmbushOnMarkerHit` | activator | `9a66d8dae562a87b` | 60 | 2,862 / 2,862 | 24 | equivalent | T S U |
| 25 | `SFBGS00D:SFBGS00DDR01SlipTriggerScript` | activator | `07624cf0bfa1fb75` | 29 | 1,583 / 1,583 | 807 | equivalent | T F St S U +offsets |
| 26 | `TestFABMessageTrigger` | activator | `5fd34f24cc128665` | 9 | 748 / 748 | 36 | equivalent | T S U |
| 27 | `ShipbuilderMenuActivator` | activator | `629c44f81b762e31` | 80 | 3,038 / 3,038 | 222 | equivalent | T P V S U +offsets |
| 28 | `ShipLiftActivator` | activator | `4dc7484f51d67657` | 73 | 4,326 / 4,326 | 2,301 | equivalent | T P F St V S U +offsets |
| 29 | `SFBGS00D:Fragments:Quests:QF_SFFL_R01_Debug_02029A38` | utility | `4593b0d96fc92247` | 14 | 1,246 / 1,246 | 261 | equivalent | T P V S U +offsets |
| 30 | `BEDebugScript` | utility | `3185021c9fc3227a` | 183 | 7,266 / 7,266 | 639 | equivalent | T P V S +offsets |
| 31 | `Utility_FAB_Script` | utility | `514f6dc3284a30c6` | 18 | 1,134 / 1,134 | 1 | equivalent | T |
| 32 | `Utility` | utility | `76ebdb211f49a58d` | 138 | 5,297 / 5,297 | 2,803 | equivalent | T F S U +offsets |
| 33 | `Math` | utility | `121e87471cf4b67a` | 65 | 2,683 / 2,683 | 511 | equivalent | T F S U +offsets |
| 34 | `DebugSceneScript` | utility | `9e7732e250833b13` | 30 | 1,218 / 1,218 | 642 | equivalent | T F S U +offsets |
| 35 | `SpaceProjectileHazardEffectScript` | magic-effect | `5afd1a22625f4fa3` | 1 | 610 / 610 | 31 | equivalent | T S U |
| 36 | `LegendaryCreatureItemEffectScript` | magic-effect | `2bcc5b0f21fdf47c` | 4 | 1,794 / 1,794 | 830 | equivalent | T P V S U +offsets |
| 37 | `TerrormorphMindControlEffectScript` | magic-effect | `ff4fd22030e4dcc5` | 113 | 5,073 / 5,073 | 2,524 | equivalent | T P F V S U +offsets |
| 38 | `TargetingEffectScript` | magic-effect | `5d2430ff6fcd3e5a` | 14 | 1,189 / 1,189 | 1 | equivalent | T |
| 39 | `Fragments:TopicInfos:TIF_CREW_EliteCrew_AutumnMac_0052A0FB` | dialogue-fragment | `adfde53835c9005c` | 0 | 290 / 290 | 1 | equivalent | T |
| 40 | `Fragments:TopicInfos:TIF_UC08_004E480D` | dialogue-fragment | `010eaa7621ad578d` | 0 | 380 / 380 | 1 | equivalent | T |
| 41 | `Fragments:TopicInfos:TIF_DialogueCydonia_0049065D` | dialogue-fragment | `ba202e0d841307a4` | 3 | 595 / 595 | 61 | equivalent | T S U |
| 42 | `Fragments:Terminals:TERM_RI01_ApplicationFamilia_00227D8A` | other-fragment | `8b416ea070fbd95e` | 8 | 901 / 901 | 85 | equivalent | T F S U +offsets |
| 43 | `Fragments:Packages:PF_SQ_OutpostCargoLinkTravel_002457B0` | other-fragment | `fff0f337fb564d66` | 11 | 736 / 736 | 25 | equivalent | T S U |
| 44 | `MQ204PlayerShipScript` | native-or-base | `36e2e3feac266e89` | 0 | 242 / 242 | 36 | equivalent | T S U |
| 45 | `SettleRedMileAirlockControlScript` | native-or-base | `5113384c20111a31` | 20 | 982 / 982 | 440 | equivalent | T P F V S U +offsets |
| 46 | `MQ105PlayerScript` | native-or-base | `878aba5b795be696` | 10 | 926 / 926 | 149 | equivalent | T P V S U +offsets |
| 47 | `SFBGS00D:Fragments:Quests:QF_SFFL_MS01_CrimeDialogue_02034E5D` | other | `086da345f3863e44` | 96 | 4,817 / 4,817 | 2,213 | equivalent | T P F V S U +offsets |
| 48 | `FX:ExplosionOnAnimEvent` | other | `a7225535c06c76dd` | 31 | 1,422 / 1,422 | 494 | equivalent | T F S U +offsets |
| 49 | `SFBGS00D:Fragments:Terminals:TERM_SFFL_AnchorpointSH_Clue_0107DCA4` | other | `0c833bd4bbd3ee17` | 10 | 1,124 / 1,124 | 1 | equivalent | T |
| 50 | `SFBGS00D:Fragments:TopicInfos:TIF_SFFL_DialogueMilliwhale_0108C2B9` | other | `f1ab2cd35935d0f4` | 6 | 956 / 956 | 36 | equivalent | T S U |

Counts over the 50: T 50; S 40; U 37; P 20; V 20; F 18; St 2; M 2. Ten scripts differ
in the compile time alone (one byte).

## The reorderings are the compiler's

The eight-script run found the string table, user flags, properties and backing variables
reordered between compiles. To check that the three new orders (functions, states, struct
members) are also the compiler's own and not caused by the switches KitKraft adds, the
official compiler was run several times by hand on the same source with the same switches,
and the name lists of the disassemblies compared (grade A: reproducible runs of the tool,
black box; outputs deleted afterwards):

| Source | Runs | Orders that varied between runs |
| ------ | ---: | ------------------------------- |
| `LC082_VigilanceQuestScript` | 4 | properties, variables, user flags, functions in a state, struct members |
| `Utility` | 4 | user flags, functions in a state |
| `ShipLiftActivator` | 10, then 3 | properties, variables, user flags; in the second series also the states (`'' waiting done busy` once, then two other orders) |
| `ShipLiftActivator` with KitKraft's switches (`-ignorecwd`, the cache's flags file and import) | 3 | the states (one of three runs) |

So no order of a named table in a `.pex` written by this compiler is stable between two
compiles; the oracle's set comparison is the right reading, and a byte comparison of two
compiles of one source says nothing. Instructions inside a function did not move in any
run.

## Consequences

- `kit papyrus compile` needs no change: on 50 base-game scripts of every kind sampled it
  yields what the hand-run compiler yields, up to the compiler's own nondeterminism.
- Any later check that compares compiled scripts (a CK sitting's `.pex`, M5.B3's in-game
  script) compares through the normalized disassembly, never bytes.
- Open: the CK's own compile route (its switches, `-op` or not) is observed only in a CK
  sitting; `docs/ck-sessions/f29-f30.md` lists it as an optional observation.
