# Recipe: place a vanilla NPC in a vanilla cell (W9, M4.E8)

Goal: an `ACHR` of a vanilla NPC in a vanilla interior cell that has navmesh, so that the
actor stands and moves there. Spec: `docs/recipes/actors.toml`.

## What KitKraft knows, and from where

| Fact | Source |
| ---- | ------ |
| A placed actor is an `ACHR` whose `NAME` is an `NPC_`; `DATA` holds position then rotation | `xedit: wbDefinitionsSF1.pas@9fb01688` (`ACHR`, the world family; `crates/schema/defs/ACHR.toml`), `corpus` confidence; lane A's `compose_reference` writes it (`place` picks `ACHR` for an `NPC_` base) |
| `Vendor_ChunksEmployee_M` (`Starfield.esm:271133`, "Chunks Employee") is an `NPC_` | `kit query NPC_ --where 'EDID ~ "Vendor"' --plugins Starfield.esm` |
| `CityNewAtlantisFreestarEmbassy` (`Starfield.esm:017307`) has one `NAVM` in its temporary children and six `ACHR`, one placed at (4.86, 2.43, 0.0) | a header walk of the cell's children group in `Starfield.esm` (2026-10-14) |
| An actor needs navmesh in its cell to move | the validate rule `cell.actor-without-navmesh` (warning) checks it from the plugin and its masters; KitKraft cannot generate navmesh (W9: the CK does, fixture F22, and `[[import]]` copies it) |
| How the NPC behaves (packages, dialogue, combat) | **not known**: its own records decide it; the in-game check below |

The rule found the gap first: the same spec with the first POI's cell
(`CityNewAtlantisApartmentTEMPLATE`, no `NAVM`) builds with the warning "cell 0021966F has
no NAVM in this plugin or its masters". The embassy has navmesh, so the warning is gone.

## Build

```text
kit build docs/recipes/actors.toml out/recipes/KitKraftActors.esm
kit validate --plugin out/recipes/KitKraftActors.esm
kit roundtrip out/recipes/KitKraftActors.esm --verify
kit testplan out/recipes/KitKraftActors.esm out/recipes/actors-test --profile <MO2>\profiles\Default
```

2026-10-14 (release build): 15,190 bytes, SHA-256
`837b63719261e6cebb7cd848d184795293c5a7cb9fce28f828a53790e588f27a`, one master
(`Starfield.esm`); `validate` 0 errors and 0 warnings; `roundtrip --verify` identical. The
`ACHR` `KitKraftTestActor` is `01001000`, persistent, in the embassy's persistent children;
the embassy is carried (its `FULL` written inline). The plugin owns a form: never remove it
from a save that is not disposable.

## In game (owner, M4.O4; KitKraft Dev, disposable save)

1. Enable `KitKraftActors.esm` on **KitKraft Dev** only; load a disposable save.
2. `coc CityNewAtlantisFreestarEmbassy`, then `player.moveto KitKraftTestActor` (or the
   runtime ID `kit testplan` prints).
3. **Expect:** a Chunks Employee stands near the embassy's actor at (4.9, 2.4); within a
   minute it walks somewhere (sandbox) rather than standing frozen.
4. Leave, save, reload, return (the persistence steps of `kit testplan`'s checklist).
5. Record the result in `docs/game-observations.md` (`game-<yyyymmdd>-<nn>`, with the hash
   above).
