# Recipe: attach a compiled script to a reference (W6, M4.E9)

Goal: a script compiled by KitKraft, attached to a new reference through `VMAD`, runs on
an event chosen from `kit papyrus events` and shows a message in game. Spec:
`docs/recipes/scripts.toml`; script source:
`docs/recipes/scripts/KitKraft/HelloOnLoad.psc` (ours). The plugin owns one new form (a
`REFR`) and a script, so it is for a disposable save only: never remove it from a save
in progress (AGENTS.md, "Owner's live game").

## What KitKraft knows, and from where

| Fact | Source |
| ---- | ------ |
| `VMAD` is the shared `ScriptAttachment` block: `Version` 6, `ObjectFormat` 2, the script count, then each script (name, flags, properties: name, type byte, flags, value by type), then a fragment block on quests, dialogue, packages, scenes and perks | `xedit: wbDefinitionsSF1.pas@9fb01688#L3232-L3411` (lane BQ's `common/ScriptAttachment.toml` types the header; `crates/engine/src/edit/vmad.rs` reads and writes the script list) |
| Every `VMAD` of the 14 official files (12,857: REFR 5,608, INFO 2,040, QUST 1,703, ACTI 850, ...) decodes and re-encodes byte-identically, all version 6 / object format 2, 43,246 object values with the two unused bytes zero | `crates/engine/tests/vmad_corpus.rs` (`every_official_vmad_roundtrips`, release) |
| Attaching a script to a corpus copy keeps the vanilla scripts and fragment bytes as they were (the reference `Starfield.esm:0011FD`: its script, then the new one) | `vmad_corpus.rs` (`attaching_to_a_corpus_copy_keeps_the_block`) |
| `ObjectReference` declares `Event OnLoad()` (`ObjectReference.psc` line 1657) and `Debug` declares `Function Notification(string asNotificationText) native global` (`Debug.psc` line 60) and `Function Trace(string asTextToPrint, int aiSeverity = 0) native global` (`Debug.psc` line 100) | `kit papyrus events --script ObjectReference --event OnLoad --declarations-only`; `kit papyrus events --script Debug --function Notification`; `kit papyrus events --script Debug --function Trace` (2026-11-02) |
| `ActivatorSwitch01` (`Starfield.esm:03CCF4`, "Switch") is an `ACTI` with a model and no `VMAD` | `kit query ACTI --where 'not has VMAD and has MODL and has FULL and not EDID ~ "Marker"' --plugins Starfield.esm` |
| A script is found as `Scripts\<Namespace>\<Name>.pex` next to the plugin, in the Data folder, or in the archives of the load order (`ObjectReference.pex` is in `Starfield - Misc.ba2`); its properties are its `.pex` property table and its parents' | `crates/engine/src/validate/attached.rs` (grade D for the game's own lookup order) |

## Compile, build, check

```text
kit papyrus compile KitKraft:HelloOnLoad <abs>\out\recipes\Scripts --imports <abs>\docs\recipes\scripts --sources-zip "<Starfield>\Tools\ContentResources.zip"
kit build docs/recipes/scripts.toml out/recipes/KitKraftScripts.esm
kit validate --plugin out/recipes/KitKraftScripts.esm
kit roundtrip out/recipes/KitKraftScripts.esm --verify
kit testplan out/recipes/KitKraftScripts.esm out/recipes/scripts-test --plugins Starfield.esm
```

`papyrus.compile` is given the namespaced name with the source root imported. Since
2026-10-15 the path form works too, with relative paths (papyrus oracle P2, P3,
`docs/oracles.md`): `kit papyrus compile docs\recipes\scripts\KitKraft\HelloOnLoad.psc
out\recipes\Scripts --sources-zip "<Starfield>\Tools\ContentResources.zip"` compiles
`KitKraft:HelloOnLoad` with `docs\recipes\scripts` imported. The build finds the `.pex` next to the plugin
(`out\recipes\Scripts\KitKraft\HelloOnLoad.pex`), checks `Text` (String) and `Times`
(Int) against its property table and the parent `ObjectReference.pex`, and writes the
`VMAD` on the new reference. An unknown script is refused (`papyrus.missing-script`), as
is an undeclared property or a value of the wrong type (`papyrus.property-unresolved`);
`validate` applies the same two rules to any plugin.

2026-10-15 (release build): `HelloOnLoad.pex` 911 bytes, SHA-256
`c01efb0167fad5cf29a0a8ad6379657b0b3722bf1c36aea46d194cc9f6f7ee42` (PapyrusCompiler
4.7.0.5, not release mode). 2026-11-02, the Trace variant (M4.T6: `OnLoad` first writes
`Text` to the Papyrus log with `Debug.Trace`, then shows the notifications): 1,064 bytes,
SHA-256 `3b7272417bd258dc4ba96162bddf4e5a1ae568a2fa7c5edee42e6bd48c534def`, compiled the
same way (`kit papyrus compile KitKraft:HelloOnLoad ... --sources-zip`), and its
disassembly opens `OnLoad` with `callstatic debug Trace ::nonevar ::Text_var 0`. The plugin
does not hold the script, so it is unchanged: `KitKraftScripts.esm` 15,297 bytes, SHA-256
`b20cf5770b3d32016274546f6c7478e687d58e130904c0c2367bfa56c11a1d45`, one master
(`Starfield.esm`), new `REFR` `KitKraftScripts.esm:001000` (`KitKraftHelloSwitch`) in the
carried cell `CityNewAtlantisFreestarEmbassy`; `validate` 0 errors, 0 warnings (the 16
plugin rules, the two papyrus rules included); `roundtrip --verify` identical.
`testplan`: `coc CityNewAtlantisFreestarEmbassy`, `player.moveto 01001000`, `prid
01001000` (runtime IDs under the assumption `runtime-slots-by-scale`). Since M4.T6 its
`probes.toml` has row `T-05`, a `papyrus-log` step expecting "KitKraft: hello from a script
attached by kit": `testplan` reads the compiled script next to the plugin, finds the
`Debug.Trace` of the `Text` property and takes the value the reference's `VMAD` sets
(`crates/engine/src/handlers/testplan/traces.rs`). Whether the game writes that line into
`Papyrus.0.log`, and when, is for a session to show (grade D).

## In game (owner, M4.O4; KitKraft Dev, disposable save)

1. Copy `out\recipes\Scripts\KitKraft\HelloOnLoad.pex` to `Data\Scripts\KitKraft\` of the
   **KitKraft Dev** mod set (or package both files with `kit package`) and enable
   `KitKraftScripts.esm` on **KitKraft Dev** only; load a disposable save.
2. `coc CityNewAtlantisFreestarEmbassy`. **Expect:** on arrival, the notification
   "KitKraft: hello from a script attached by kit" twice (`Times` = 2 from `VMAD`, not the
   script's default 1).
3. `player.moveto 01001000`: the switch stands at (4, 4, 0) in the embassy.
4. With the Trace variant (2026-11-02), the Papyrus log (`Papyrus.0.log`, with the dev
   profile's `[Papyrus]` logging switches on) holds the same text once per load.
5. Record the result in `docs/game-observations.md` (`game-<yyyymmdd>-<nn>`, with both
   hashes above). One notification means the property value was not read; none means the
   script did not run (check `Data\Scripts` first).

## Limits

- An object property's value is not checked against the declared object type (a formref
  of any record type is written); forms in object properties must be in the record's
  masters or written by the same call.
- `Var` properties are not attached: no official record stores a value for one (no
  type 6 or 16 value in the 14 files, corpus scan 2026-11-17), so there is no layout to
  follow. Struct properties are (below, "Struct values").
- The order in which the game looks up a script (loose, plugin archives, other archives) is
  assumed, not observed.
- Quest stage fragments (`stages` on a `QUST`'s script entry, `docs/mcp.md` `set`) and
  scripts on quest aliases (`alias`, below) are written in the official layout (four
  official quest `VMAD`s rebuilt byte for byte, `crates/engine/tests/quest_vmad_corpus.rs`).
  `validate` refuses the write when a fragment's stage or log entry is missing
  (`quest.fragment-stage`) or its function is not in the compiled script
  (`quest.fragment-function`; a warning when no `.pex` is found to check).
- An alias script is not checked to extend `ReferenceAlias`, `RefCollectionAlias` or
  `LocationAlias`; an alias value's property type is not checked to be an alias type, and
  another quest's alias (`{quest, alias}`) is not checked against that quest; `validate`
  does not read the alias section (`docs/validate.md`, `papyrus.property-unresolved`).

## Scripts on quest aliases

A script entry on a `QUST` may name one of the quest's aliases with `alias`: its ID (an
integer) or its name (`ALID`, any case). The script then goes into that alias's entry of
the `VMAD` fragment block, not the quest's script list; properties are typed and checked
exactly as for any script. The quest must declare the alias by the time scripts are
attached, so a `fields` edit of the same `[[new]]` or `[[set]]` counts:

```toml
[[new]]
name = "quest"
type = "QUST"
edid = "KKQ02Quest"
fields = { ANAM = 1, ALST = 0, ALID = "Target", ALED = "" }
scripts = [
  { name = "KitKraft:VerifyAliasReport", alias = "Target", properties = { Count = 7 } },
]
```

The alias's other required members (`FNAM` flags, `ALFG`, `VTCK`) are written as zeros
when not given. Its flags can be given by name (`FNAM = "Optional|QuestObject"`): `FNAM`
is also an objective's flags, and one `FNAM` goes to the group the fields start (here the
alias, with `ALST`; before 2026-11-17 `build` refused it, `new.no-group-lead`).
`MatchingRefActorsOnly` is written `00 00 04 00`, as `Starfield.esm` `QUST` `000D40ED`'s
alias 1 stores it (`quest_fragment_tests::an_alias_takes_its_flags_from_fields`). Only one
stage, one log entry and one alias can be written through a `[[new]]`'s `fields` today: a
second instance of a repeated group (`INDX[1]`, `QSDT[1]`) is refused
(`new.group-incomplete`).

A stage fragment needs its stage and a log entry in it: give the stage one with `QSDT`
(`"INDX.StageIndex" = 10, QSDT = 0`; `build` adds the reward count and end, `QSRD`).
Every official fragment whose stage exists names an existing log entry (13,813 of 13,947;
the other 134 name a stage their quest lacks), and in game a fragment on a stage without
that log entry does not run, while the same fragment with `QSDT = 0` does (bench quest
row Q-03, sessions `game-20261117-05` and `-06` on the bench branch). `build` refuses the
former (`quest.fragment-stage`).

### Struct values

A struct property (a compiled script names its type `Script#Struct`, an array of them
`Script#Struct[]`) takes a table of its members, each typed by the struct's declaration in
the script's `.pex` and written in declaration order, only the members given:

```toml
properties = { One = { Count = 3, Target = "KC_Crate" }, Many = [{ Count = 1 }, { Count = 2 }] }
```

Every official struct member is a scalar or an object with flags 1, and no struct holds a
struct or an array (245 struct values and 1,408 struct arrays, corpus scan 2026-11-17);
`attach` rebuilds all 644 official quest `VMAD`s it can write that hold a struct or struct
array (3,786 struct values) byte for byte (`quest_vmad_corpus.rs`). A struct declared only
in the base game's sources (no `.pex` found) is refused: its members are unknown.

| Fact | Source |
| ---- | ------ |
| The block's alias section: a u16 entry count; each entry the alias as an object value (unused, alias ID, FormID), its own version and object format (i16 each), then a script list | `xedit: wbDefinitionsSF1.pas@9fb01688#L3484-L3531` (`kitkraft_doc::vmad::QuestFragments`) |
| All 4,404 official entries hold the quest's own FormID, unused 0, version 6, object format 2 and at least one script; 3,865 name a reference alias, 535 a collection alias, none a location alias, 4 an alias the quest does not declare (SFBGS050.esm `001C7187`, `01031F1D`); 312 of 1,050 blocks are not in alias ID order | `quest_vmad_corpus.rs` (release, ignored), 2026-11-17 |
| `attach` rebuilds `Starfield.esm` `QUST` `002B180A` (alias scripts only: 7 entries, 34 properties, 1,309 bytes) and `002AAE8D` (3 quest scripts, 106 stage fragments, 36 alias entries holding 65 scripts, 395 properties, 22,265 bytes) byte for byte, each the largest such quest whose properties are all scalars, objects, alias values or arrays of these | same test |
| `SQ_CCT_Enviro_Raking00` (`Starfield.esm` `QUST` `000D40ED`): `inspect` shows its whole `VMAD` (108 bytes) as `0600 0200 0000` (no scripts), `03 0000 0000` (bind data 3, no fragments, no fragment script), `0100` (one entry), `0000 0100 ED400D00` (unused, alias 1, the quest), `0600 0200 0100` and the script; the entry `kit` writes for `KitKraftVerifyQuest.esm` has the same fields in the same order | `kit inspect 0D40ED --plugin Starfield.esm`, 2026-11-17 |

So KitKraft writes a new entry in that one shape, appends entries in the order given, adds
a second script on the same alias to its entry, and updates the properties of a script of
the same name. A block holding only alias scripts has an empty fragment script name and no
fragments, as 26 official quests have. `alias` together with `stages` is refused (an alias
holds scripts, not fragments), and an alias the quest does not declare is refused with
`quest.alias-missing`, listing the quest's aliases. A location alias may take a script
(the layout is the same) though no official quest has one. Whether the game runs it is
not observed (`docs/bench/verify-mod/verify-quest.toml`, row Q-05, not run).

### Alias values

An object property can hold a quest alias, as 9,946 properties of official quest scripts,
7,164 of fragment scripts and 1,042 of alias scripts hold one of their own quest's aliases
(and 314, 119 and 19 another quest's): the object value carries the quest's FormID and
the alias ID. In a spec, `{ alias = "Target" }` (an ID or a name) is an alias of the quest
the script is attached to (on a `QUST` only, refused with `quest.alias-missing` like an
alias script), and `{ quest = "Starfield.esm:0123AB", alias = 3 }` an alias of another
quest (a formref, EditorID or `new:<name>`; the alias by ID, not checked). Arrays of them
work as any object array.

```toml
scripts = [
  { name = "KitKraft:VerifyQuestReport", properties = { TargetAlias = { alias = "Target" } } },
]
```
