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
| `ObjectReference` declares `Event OnLoad()` (`ObjectReference.psc` line 1657) and `Debug` declares `Function Notification(string asNotificationText) native global` (`Debug.psc` line 60) | `kit papyrus events --script ObjectReference --event OnLoad --declarations-only`; `kit papyrus events --script Debug --function Notification` |
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

`papyrus.compile` is given the namespaced name with the source root imported (a `.psc`
path would compile as `HelloOnLoad` and the compiler refuses the name mismatch), and
absolute paths: the compiler resolves a relative import against `out_dir` (a request to
lane G is in `plan/STATUS.md`). The build finds the `.pex` next to the plugin
(`out\recipes\Scripts\KitKraft\HelloOnLoad.pex`), checks `Text` (String) and `Times`
(Int) against its property table and the parent `ObjectReference.pex`, and writes the
`VMAD` on the new reference. An unknown script is refused (`papyrus.missing-script`), as
is an undeclared property or a value of the wrong type (`papyrus.property-unresolved`);
`validate` applies the same two rules to any plugin.

2026-10-15 (release build): `HelloOnLoad.pex` 911 bytes, SHA-256
`c01efb0167fad5cf29a0a8ad6379657b0b3722bf1c36aea46d194cc9f6f7ee42` (PapyrusCompiler
4.7.0.5, not release mode); `KitKraftScripts.esm` 15,297 bytes, SHA-256
`b20cf5770b3d32016274546f6c7478e687d58e130904c0c2367bfa56c11a1d45`, one master
(`Starfield.esm`), new `REFR` `KitKraftScripts.esm:001000` (`KitKraftHelloSwitch`) in the
carried cell `CityNewAtlantisFreestarEmbassy`; `validate` 0 errors, 0 warnings (the 16
plugin rules, the two papyrus rules included); `roundtrip --verify` identical.
`testplan`: `coc CityNewAtlantisFreestarEmbassy`, `player.moveto 01001000`, `prid
01001000` (runtime IDs under the assumption `runtime-slots-by-scale`).

## In game (owner, M4.O4; KitKraft Dev, disposable save)

1. Copy `out\recipes\Scripts\KitKraft\HelloOnLoad.pex` to `Data\Scripts\KitKraft\` of the
   **KitKraft Dev** mod set (or package both files with `kit package`) and enable
   `KitKraftScripts.esm` on **KitKraft Dev** only; load a disposable save.
2. `coc CityNewAtlantisFreestarEmbassy`. **Expect:** on arrival, the notification
   "KitKraft: hello from a script attached by kit" twice (`Times` = 2 from `VMAD`, not the
   script's default 1).
3. `player.moveto 01001000`: the switch stands at (4, 4, 0) in the embassy.
4. Record the result in `docs/game-observations.md` (`game-<yyyymmdd>-<nn>`, with both
   hashes above). One notification means the property value was not read; none means the
   script did not run (check `Data\Scripts` first).

## Limits

- An object property's value is not checked against the declared object type (a formref
  of any record type is written); forms in object properties must be in the record's
  masters or written by the same call.
- `Var` and struct properties are not attached (their layout is read and kept, but a JSON
  value cannot pick a variant yet).
- The order in which the game looks up a script (loose, plugin archives, other archives) is
  assumed, not observed.
