# Recipe: the Navy Tan colourway as two new forms (KK-020)

Goal: exercise the build spec v3's `swap_overrides` (`[[clone]]` and `[[set]]` take it,
KK-019's conversion) by re-authoring the game's own Navy Tan colourway: a `[[clone]]` of
the outfit's material swap (`Starfield.esm:1F1DCA`) whose every override points at a path
under `KitKraft/Colourway/` (each file name kept), and a `[[clone]]` of the outfit
(`Starfield.esm:1F1DC9`) whose object-template property 14 names our swap instead of the
vanilla one. `Starfield.esm` is the only master; the plugin owns two new forms. Spec:
`docs/recipes/colourway.toml`. The `.mat` files themselves are not written by KitKraft
(see "What is not done"); the plugin only names them.

## What KitKraft knows, and from where

| Fact | Source |
| ---- | ------ |
| `LMS_Clothes_Mei_Devine_NavyTan` is `LMSW` `Starfield.esm:1F1DCA`; its `REFL` has exactly four entries, keys `Clothes\Outfit_Mei_Devine\Outfit_Mei_Devine_{UpperBody,LowerBody,Tights,Tights_M}.mat`, overrides `Clothes\Outfit_Mei_Devine\Outfit_Mei_Devine_NavyTan_{UpperBody,LowerBody,Tights,Tights_M}.mat` | `corpus: kit inspect Starfield.esm:1F1DCA --full` |
| `Clothes_Mei_Devine_NavyTan` is `ARMO` `Starfield.esm:1F1DC9` (FULL "Navy Tan Dress"); its one `OBTS` combination has three includes (the OMODs `25AF26`, `25AF27`, `13DDFE`) and one property: property 14, ValueType 4 (FormIdInt), FunctionType Set, Value1's FormId `Starfield.esm:1F1DCA` — the outfit's material swap | `corpus: kit inspect Starfield.esm:1F1DC9 --full` |
| The parallel pair: `Clothes_Mei_Devine_CreamBlue` is `ARMO` `Starfield.esm:1F1DCE` (FULL "Cream and Blue Dress") with the same four `MODL` addons (`14410A`, `144109`, `165724`, `006A88`), and its property 14 names `LMSW` `Starfield.esm:1F1DCB`, which holds the same four keys with the CreamBlue overrides — this is how an outfit names its colourway | `corpus: kit inspect Starfield.esm:1F1DCE --full`; `corpus: kit inspect Starfield.esm:1F1DCB --full` |
| Property 14's FormId is a field path: `OBTS[0].Properties[0].Value1.FormId` — `Value1` is a union whose variant for ValueType 4 is `FormId`, and a path step may name a union variant; the `refs` layer names the same edge `OBTS.TemplateItem.Properties[0].Value1.FormId` | `crates/schema/defs/common/ObjectModProperty.toml`; `crates/engine/src/edit/fields.rs` (module docs, unions); `kit refs KitKraftRecipeColourwayOutfit` on the built plugin |
| `swap_overrides` in a v3 spec: `[[clone]]` and `[[set]]` take `[{ key, override }]`, converted by the same `swaps.rs::convert` the `clone`/`set` commands use; `[[new]]` and `[[place]]` with it are spec errors naming the key; `/` is stored as `\`, overrides must end `.mat`; the build result's `swap_overrides` is one report per entry, each `[{ key, from, to }]` | `crates/engine/src/build/spec.rs`; `crates/engine/src/handlers/build.rs`; `crates/engine/src/edit/swaps.rs`; `crates/engine/src/handlers/build_v3_tests.rs` (`swap_overrides_in_a_spec_are_checked_by_entry_kind`) |
| `build` allocates `[[clone]]` FormIDs first, in spec order, from `0x001000`, so `new:swap` names the first `[[clone]]` | the build's output (`LMSW :001000`, `ARMO :001001`); `docs/recipes/mini-site.toml` (header) |
| The `.mat` files are JSON material files third-party mods ship in the game's `Data` folder; KitKraft has not ruled on that source category, so it does not write them | `docs/research/materials-lmsw-2026-10-23.md` |

## What the plugin links

```text
ARMO KitKraftRecipeColourwayOutfit (clone of 1F1DC9, our FULL)
    --OBTS property 14 (Value1.FormId)--> LMSW KitKraftRecipeColourwaySwap (ours, :001000)
    --MODL x4--> 14410A, 144109, 165724, 006A88 (the four vanilla armor addons)
    --OBTS includes x3--> 25AF26, 25AF27, 13DDFE (the three vanilla OMODs)
    plus the vanilla RNAM, KWDA, PTT2, DAMA and APPR links of 1F1DC9
LMSW KitKraftRecipeColourwaySwap (clone of 1F1DCA)
    no form links: its four .mat paths are strings the schema keeps raw
```

## Build, check

```text
kit build docs/recipes/colourway.toml out/recipes/KitKraftColourway.esm
kit validate --plugin out/recipes/KitKraftColourway.esm
kit roundtrip out/recipes/KitKraftColourway.esm --verify
kit refs KitKraftRecipeColourwayOutfit --plugins Starfield.esm --plugins out/recipes/KitKraftColourway.esm
kit testplan out/recipes/KitKraftColourway.esm out/recipes/colourway-test --plugins Starfield.esm --spec docs/recipes/colourway.toml
```

2026-10-27 (branch `ticket/KK-020`): `KitKraftColourway.esm` 1,749 bytes, SHA-256
`bd796e3f126df9eb64bce1040eeb5527a0308853573e158ce4ffa41f192ed7e0`, one master
(`Starfield.esm`), new forms `:001000` (`LMSW KitKraftRecipeColourwaySwap`) and
`:001001` (`ARMO KitKraftRecipeColourwayOutfit`); `validate` 0 errors, 0 warnings;
`roundtrip --verify` identical; `refs` of the outfit: 24 outgoing links, 15 resolved and
9 null (the vanilla zero slots), none dangling, and `OBTS.TemplateItem.Properties[0]
.Value1.FormId` resolves to `KitKraftColourway.esm:001000`; `inspect --full` of the swap
lists the four vanilla keys, each with its `KitKraft\Colourway\...` override; the build
result's `swap_overrides` report gives each entry's key, the vanilla override and the
new one. Spec SHA-256
`b1c135feafa42be6577421e84bc1401ef7096bec18691109d9909919214a4bcc`. `testplan` writes
`checklist.md`, `kitkraft-test.txt` (2 commands: add and equip the outfit) and
`probes.toml` (6 probes, KitKraft Dev profile). `crates/engine/tests/colourway_recipe_corpus.rs`
repeats all of it through `execute` (release, `--ignored`), including a second build that
is byte-identical.

## In game (the owner; KitKraft Dev, disposable save)

Not run. `kit game session` runs `probes.toml`; until then, by hand:

1. Copy `out\recipes\KitKraftColourway.esm` and the four `.mat` files under
   `Data\KitKraft\Colourway\` (the mod ships them; see below) into the **KitKraft Dev**
   mod set and load a disposable save.
2. `player.additem 01001001 1`, then `player.equipitem 01001001`. **Expect:** the
   "KitKraft Recipe Colourway" dress in the inventory, equipped.
3. Third person (or `tfc` free camera), screenshot the worn outfit: the colourway shows
   as the shipped `.mat` files define it; what the game does with the four missing paths
   (before the files are shipped) is not observed.
4. Record the results in `docs/game-observations.md` with the hashes above, then the
   save/reload rows.

## What is not done

- **The `.mat` files themselves.** The swap names four paths under
  `KitKraft/Colourway/`; the files are JSON the format is researched in
  `docs/research/materials-lmsw-2026-10-23.md`. The owner has not ruled on that source
  category, so KitKraft does not write the files — the mod ships them, and this recipe
  only writes the two forms that name them.
- **The in-game check** above: for the owner on the KitKraft Dev profile with a
  disposable save; not run.
