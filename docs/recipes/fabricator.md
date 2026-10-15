# Recipe: a new fabricator recipe for an existing item (W8, M4.E7)

Goal: the industrial workbench (the fabricator) offers a recipe it does not have: 12g
shotgun shells from iron and aluminum, with no research unlock. Spec:
`docs/recipes/fabricator.toml`. The plugin owns one new form (a `COBJ`), so it is for a
disposable save only: never remove it from a save in progress (AGENTS.md, "Owner's live
game").

## What registers a crafting recipe, from the official files

Read with `kit query`, `kit inspect` and `kit refs` (load order `--plugins
Starfield.esm`, 2026-10-15); how the game uses it is grade D until the in-game check
(M4.O4).

| Fact | Evidence |
| ---- | -------- |
| The workbench keywords are KYWDs named `Workbench*Keyword`; the industrial workbench's is `WorkbenchIndustrialKeyword` (`Starfield.esm:0AA209`), next to the armor, cooking, chem lab, research, weapon and ship building ones | `kit query KYWD --where 'EDID ~ "Workbench"' --plugins Starfield.esm` |
| 54 recipes have `BNAM` = `WorkbenchIndustrialKeyword`: 32 create a manufactured component (`MISC`, 31 `Mfg_Tier0n_*` and one quest item), 22 create ammunition (`AMMO`); every ammunition recipe has one condition (`CTDA`), and 11 of the 12 tier 1 component recipes have none | `kit query COBJ --where 'BNAM == Starfield.esm:0AA209' --plugins Starfield.esm`, then `kit inspect` of each and of its `CNAM` |
| `co_mfg_Tier01_AdaptiveFrame` (`Starfield.esm:2457C1`): `BNAM` industrial, `FVPA` iron (`0057C7`) 1 and aluminum (`0057D6`) 1, `CNAM` `Mfg_Tier01_AdaptiveFrame`, `NNAM` 1, `TNAM` 0, `LRNM` 3, `DATA` 34, `FNAM` `RecipeManufactured` (`2E2F2D`, keyword type 9, "Manufactured"), `RECF` 0, no condition | `kit inspect Starfield.esm:2457C1`; `kit inspect Starfield.esm:2E2F2D` |
| `co_ammo_12gShotgunShell` (`Starfield.esm:2BBFAC`) makes `AmmoShotgun` (`Starfield.esm:0547A1`, "12g Shotgun Shell"), `NNAM` 6, behind one condition (function 874, its parameters kept raw by the schema) and with no `FNAM` | `kit inspect Starfield.esm:2BBFAC`; `kit inspect Starfield.esm:0547A1` |

So a crafting recipe is, as an outpost recipe, a `COBJ` filed by its `BNAM` workbench
keyword; nothing else lists it. The condition is what hides the vanilla shell recipe until
research; a copy of an unconditioned recipe shows at once (grade D).

## The spec

One `[[clone]]` of `co_mfg_Tier01_AdaptiveFrame` as `KitKraft_co_FabricatorShotgunShells`
with `CNAM` = `Starfield.esm:0547A1`, `NNAM` 6 (as the vanilla shell recipe), `DESC` an
inline text (the vanilla one describes the adaptive frame) and `DATA` 0. `BNAM`, `FNAM`,
`FVPA`, `TNAM`, `LRNM` and `RECF` are the vanilla bytes, so the recipe sits under the
industrial workbench's "Manufactured" filter and costs one iron and one aluminum.

## Build and checks

```text
kit build docs/recipes/fabricator.toml out/recipes/KitKraftFabricator.esm
kit validate --plugin out/recipes/KitKraftFabricator.esm
kit roundtrip out/recipes/KitKraftFabricator.esm --verify
kit testplan out/recipes/KitKraftFabricator.esm out/recipes/fabricator-test --plugins Starfield.esm
```

2026-10-15 (release build): 373 bytes, SHA-256
`1d5cce72d8077b62c26d100efd4887bc79d2ea8dee425ace5e3aaf1d2a902467`, one master
(`Starfield.esm`), new form `KitKraftFabricator.esm:001000` (`COBJ`); `validate` 0 errors,
0 warnings; `roundtrip --verify` identical. `testplan` lists no console command (no
reference or item is new), so the steps below are the plan.

## In game (owner, M4.O4; KitKraft Dev, disposable save)

1. Enable `KitKraftFabricator.esm` on **KitKraft Dev** only; load a disposable save.
2. `player.additem 000057C7 10` and `player.additem 000057D6 10` (iron, aluminum).
3. Use an industrial workbench (the Lodge's, or an outpost one). **Expect:** under
   "Manufactured", a recipe for "12g Shotgun Shell" with the description above, costing
   1 iron and 1 aluminum, available without research.
4. Craft it once. **Expect:** 6 shells in the inventory.
5. Record the result in `docs/game-observations.md` (`game-<yyyymmdd>-<nn>`, with the hash
   above).

## Open points

- Whether the amount produced is `NNAM` as stored or something the workbench scales (the
  vanilla recipe says 6; the in-game count settles it).
- Function 874 of the vanilla shell recipe's condition is not named here (its parameters
  are union fields the schema keeps raw, `common/Condition.toml`).
