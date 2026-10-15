# Recipe: a buildable outpost object (W3, M4.E7)

Goal: a new object in the outpost build menu, cloned from a vanilla buildable, with its
own recipe. Spec: `docs/recipes/workshop.toml`. The plugin owns two new forms (a `STAT`
and a `COBJ`), so it is for a disposable save only: never remove it from a save in
progress (AGENTS.md, "Owner's live game").

## What registers a buildable object, from the official files

Every claim below is what `Starfield.esm` holds, read with `kit query`, `kit inspect` and
`kit refs` (load order `--plugins Starfield.esm`, 2026-10-15). How the game uses it is
grade D until the in-game check (M4.O4). Background and counts over all 596 outpost
recipes: `docs/schema/items-workshop-family.md`, "The outpost build menu".

| Fact | Evidence |
| ---- | -------- |
| The outpost menu's categories are the 17 keywords `OutpostCategory*` (KYWD, keyword type 9); `OutpostCategoryDecorations` is `Starfield.esm:293ADF` | `kit query KYWD --where 'EDID ~ "OutpostCategory"' --plugins Starfield.esm` |
| A recipe is filed under a category by its `BNAM` (`WorkbenchKeyword`): 30 recipes have `BNAM` = `OutpostCategoryDecorations` and no condition (`CTDA`) | `kit query COBJ --where 'BNAM == Starfield.esm:293ADF and not has CTDA' --plugins Starfield.esm` |
| `co_Outpost_Decor_CoffeeMaker01` (`Starfield.esm:039657`): `BNAM` and `FNAM` `OutpostCategoryDecorations`, `FVPA` aluminum (`0057D6`) 1, copper (`0057CF`) 1 and sealant (`07782D`) 1, `CNAM` `Starfield.esm:136FDF`, `NNAM` 0, `TNAM` 0, `LRNM` 3, `RECF` 0, no conditions, record flags 0 | `kit inspect Starfield.esm:039657 --plugins Starfield.esm` |
| Nothing points at that recipe: no form list, no other record lists it | `kit refs Starfield.esm:039657 --incoming --plugins Starfield.esm`: 0 referrers (within what the reverse index searches; its limitations name the types it cannot read) |
| `CoffeeBrewingMachine01` (`Starfield.esm:136FDF`) is a `STAT` with a model, bounds, `PTT2` transforms (preview `Starfield.esm:178A45`), a keyword component and `FULL` "Coffee Maker"; the recipe is one of its 121 referrers, the other 120 are placed references | `kit inspect Starfield.esm:136FDF`; `kit refs Starfield.esm:136FDF --incoming` |
| A recipe whose created object is a form list carries record flag `0x04000000`; one whose created object is a `STAT`, `MSTT`, `PKIN` or `CONT` does not (17 of 17 and 0 of 13 among the 30 decoration recipes above) | `kit inspect` of each recipe the query above returns, and of its `CNAM` |

So, from the data: a recipe is found by its `BNAM` keyword; no list registers it. The
safe start is a copy of a vanilla recipe with its `CNAM` changed, and a copy of the
vanilla object, so that model, bounds, transforms and keywords come along (the family
doc's "What a new buildable object needs").

## The spec

- `[[clone]]` `coffee`: `Starfield.esm:136FDF` as `KitKraftCoffeeMaker`, `FULL` "KitKraft
  Coffee Maker" (written inline: the patch is not localized).
- `[[clone]]` `coffee_recipe`: `Starfield.esm:039657` as
  `KitKraft_co_Outpost_Decor_CoffeeMaker`, `CNAM = "new:coffee"`. A field value
  `new:<name>` names a form a `[[new]]` or `[[clone]]` of the same build writes (M4.E7,
  `docs/mcp.md`, "`build`"); the writer stores its FormID once it is allocated.

Everything else of both records is the vanilla bytes: the recipe keeps its category,
filter, costs, sounds, tier and learn method.

## Build and checks

```text
kit build docs/recipes/workshop.toml out/recipes/KitKraftWorkshop.esm
kit validate --plugin out/recipes/KitKraftWorkshop.esm
kit roundtrip out/recipes/KitKraftWorkshop.esm --verify
kit inspect KitKraftWorkshop.esm:001001 --plugins Starfield.esm --plugins out/recipes/KitKraftWorkshop.esm
kit testplan out/recipes/KitKraftWorkshop.esm out/recipes/workshop-test --plugins Starfield.esm
```

2026-10-15 (release build): 726 bytes, SHA-256
`f9dc6d24681776b535091efbf35be1f3040e9c040ea3a8f5abecc71d751203a0`, one master
(`Starfield.esm`), new forms `KitKraftWorkshop.esm:001000` (`STAT`) and `:001001`
(`COBJ`, `CNAM` = `01001000`, the clone); `validate` 0 errors, 0 warnings; `roundtrip
--verify` identical. The recipe's `DESC` (string ID 0 in the vanilla record) is written
as the empty string. `testplan` lists no console command (no reference or item is new),
so the steps below are the plan.

## In game (owner, M4.O4; KitKraft Dev, disposable save)

1. Enable `KitKraftWorkshop.esm` on **KitKraft Dev** only; load a disposable save near an
   outpost beacon (or place one).
2. Open the outpost build menu, category **Decorations**. **Expect:** "KitKraft Coffee
   Maker" next to the vanilla "Coffee Maker", with the same cost.
3. Build it (`player.additem 000057D6 5`, `player.additem 000057CF 5` and
   `player.additem 0007782D 5` give aluminum, copper and sealant if needed). **Expect:**
   it places and looks like the vanilla coffee maker.
4. Record the result in `docs/game-observations.md` (`game-<yyyymmdd>-<nn>`, with the hash
   above). Missing from the menu is a result too: then the first suspects are the
   beacon's category keywords and what the beacon's script filters on (read with
   `kit papyrus events`, never changed), as the family doc says.

## Open points

- Whether the menu reads the category from `BNAM`, `FNAM` or both (the clone keeps both
  equal, as the vanilla recipe has them): CK question 1 in the family doc.
- What record flag `0x04000000` on a recipe means (it tracks a form-list `CNAM` in the
  sample; upstream's name for it is not checked here).
