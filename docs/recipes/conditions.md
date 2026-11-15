# Recipe: a crafting recipe gated by a condition (KK-272)

Goal: the industrial workbench offers 12g shotgun shells only once the player has
researched "Ammunition 1", the gate the vanilla shell recipe has, written as a new
condition member by member (`docs/design/conditions.md`). Spec:
`docs/recipes/conditions.toml`. The plugin owns one new form (a `COBJ`), so it is for a
disposable save only: never remove it from a save in progress (AGENTS.md, "Owner's live
game").

## The vanilla condition, from the official files

Read with `kit inspect` (load order `--plugins Starfield.esm`, 2026-11-15); what the game
does with it is grade D until the in-game check below.

| Fact | Evidence |
| ---- | -------- |
| `co_ammo_12gShotgunShell` (`Starfield.esm:2BBFAC`) has one condition: operator `EqualTo`, no flags, comparison value 1.0, function 874, parameter 1 `Starfield.esm:2C0D72`, run on the subject, reference unused, parameter 3 -1. Its two unused bytes after the function are `F9 44`, not zeros | `kit inspect Starfield.esm:2BBFAC` (the `CTDA` payload `000000000000803F6A03F944720D2C00...FFFFFFFF`) |
| Function 874 is `IsResearchComplete`, whose parameter 1 is a research project | the pinned function table (`common/Condition.toml`, `Function`; `wbDefinitionsSF1.pas@9fb01688#L150-L761`) |
| `Starfield.esm:2C0D72` is the research project `Weapon_Research_Ammo_01`, "Ammunition 1": "Discover how to create ammunition for multiple weapons at an Industrial Workbench." | `kit inspect Starfield.esm:2C0D72` |
| `co_mfg_Tier01_AdaptiveFrame` (`Starfield.esm:2457C1`) is an industrial workbench recipe with no condition and no `CITC` (a `COBJ` has no condition count) | `docs/recipes/fabricator.md`; `crates/schema/defs/COBJ.toml` |

## The spec

One `[[clone]]` of `co_mfg_Tier01_AdaptiveFrame` as `KitKraft_co_ConditionedShotgunShells`
with the fabricator recipe's edits (`CNAM` `AmmoShotgun`, `NNAM` 6, an inline `DESC`,
`DATA` 0) and one condition. The clone has none, so `CTDA` (occurrence 0) adds it at the
record's condition slot, after `BNAM`; it starts as 32 zero bytes and the fields fill it:

| Field | Value | Why |
| ----- | ----- | --- |
| `CTDA.Type.Operator` | `EqualTo` | the operator part of the type byte (bits 5..7) |
| `CTDA.Type.Flags` | `0` | no `Or`, no `UseGlobal` (bits 0..4) |
| `CTDA.ComparisonValue.Float` | `1.0` | the `Float` variant, which a type byte without `UseGlobal` selects |
| `CTDA.Function` | `IsResearchComplete` | by name (874 also works) |
| `CTDA.Parameter1.ResearchProject` | `Starfield.esm:2C0D72` | the variant function 874 selects |
| `CTDA.RunOn` | `Subject` | the player at the workbench, as the vanilla condition |
| `CTDA.Parameter3.Integer` | `-1` | as every official condition that does not use it |

The keys may come in any order: the writer applies the edits of one subrecord deciders
first (the type byte before the comparison value, the function before parameter 1, run on
before the reference and parameter 3), whatever order a TOML table or JSON object gives
them in.

## Build and checks

```text
kit build docs/recipes/conditions.toml out/recipes/KitKraftConditions.esm
kit inspect KitKraftConditions.esm:001000 --plugins Starfield.esm out/recipes/KitKraftConditions.esm
kit validate --plugin out/recipes/KitKraftConditions.esm
kit roundtrip out/recipes/KitKraftConditions.esm --verify
```

`crates/engine/tests/conditions_recipe_corpus.rs`
(`the_conditions_recipe_builds_and_every_claim_holds`, ignored; run in release) builds it
and checks every claim: one new form, the condition's members read back as the table
says, its bytes equal the vanilla condition's except the two unused bytes (written as
zeros), the research project link resolves in `refs`, `validate` 0 errors and 0 warnings,
`roundtrip --verify` identical, and a second build is byte-identical.

2026-11-15 (release build): 421 bytes, SHA-256
`a9a3c57eca99be2867678480b76abb90bede7bd772a393dfa9825dc988565f05`, one master
(`Starfield.esm`), new form `KitKraftConditions.esm:001000` (`COBJ`); its condition
`000000000000803F6A030000720D2C00000000000000000000000000FFFFFFFF` against the vanilla
`...6A03F944...`; `validate` 0 errors, 0 warnings; `roundtrip --verify` identical.

## In game (owner; KitKraft Dev, disposable save)

1. Enable `KitKraftConditions.esm` on **KitKraft Dev** only; load a disposable save in
   which "Ammunition 1" is not researched.
2. `player.additem 000057C7 10` and `player.additem 000057D6 10` (iron, aluminum).
3. Use an industrial workbench. **Expect:** no "12g Shotgun Shell" recipe with the
   description above (or a locked one, as the vanilla recipe shows).
4. Complete the research "Ammunition 1" (at a research lab). **Expect:** the recipe
   appears and crafts 6 shells for 1 iron and 1 aluminum.
5. Record the result in `docs/game-observations.md` (`game-<yyyymmdd>-<nn>`, with the hash
   above).

## Open points

- Whether the game reads the two unused bytes at all (the vanilla record carries
  leftovers there, KitKraft writes zeros); the in-game check settles it for this record.
- How a locked recipe shows at the workbench (hidden or greyed out) is not observed.
