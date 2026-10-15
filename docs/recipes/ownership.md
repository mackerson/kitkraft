# Recipe: change who owns a cell (W4, M4.E8)

Goal: an override that makes the player the owner of a vanilla interior cell, so that taking
its items is no longer stealing. Spec: `docs/recipes/ownership.toml`.

## What KitKraft knows, and from where

| Fact | Source |
| ---- | ------ |
| A cell's owner is `CELL` `XOWN`, 12 bytes: `Owner` (a FormID naming a `FACT` or an `NPC_`), 4 unused bytes, `NoCrime` (a bool), 3 unused bytes | `xedit: wbDefinitionsCommon.pas@9fb01688#L9367-L9391` (the `OwnerData` shared type, `crates/schema/defs/common/OwnerData.toml`); `corpus`: every `XOWN` of the `CELL`, `REFR` and `ACHR` records of the 14 official files decodes with it (`world_family_full`) |
| `CityNewAtlantisFreestarEmbassy` (`Starfield.esm:017307`) is owned by `Starfield.esm:24F839` | `kit build` of the spec reports `XOWN.Owner "0024F839" -> "0001C21C"` (the value read from the winning version) |
| `PlayerFaction` is `Starfield.esm:01C21C` | `kit query FACT --where 'EDID == "PlayerFaction"' --plugins Starfield.esm` |
| What the game does with a cell owned by `PlayerFaction` | **not known**: the in-game check below (grade D until M4.O4); the CK's own choice of members is fixture F27 |

The cell is a stand-in: lane H's M4.H1 picks the fixture cell for F27 and M4.O4. Of the
New Atlantis interiors, four have an `XOWN` on `Starfield.esm` (`kit query CELL --where 'has
XOWN and EDID ~ "NewAtlantis"'`): the Freestar Collective Embassy, The Lodge, The Well and
Subsection Seven; the embassy is the smallest of them and no quest hub.

## Build

```text
kit build docs/recipes/ownership.toml out/recipes/KitKraftOwnership.esm
kit validate --plugin out/recipes/KitKraftOwnership.esm
kit roundtrip out/recipes/KitKraftOwnership.esm --verify
kit testplan out/recipes/KitKraftOwnership.esm out/recipes/ownership-test --profile <MO2>\profiles\Default
```

2026-10-14 (release build): 15,053 bytes, SHA-256
`cc77ed38e904679536475db31371752f50176baa3aaa948d1848072cd26cc259`, one master
(`Starfield.esm`); `validate` 0 errors and 0 warnings; `roundtrip --verify` identical. The
plugin is an override only (it owns no forms), so it can be dropped from a save. The cell's
`FULL` is written inline (patches are non-localized); everything else is the winner's
bytes except `XOWN.Owner`.

## Query by owner

`kit query CELL --where 'XOWN.Owner == Starfield.esm:01C21C' --plugins Starfield.esm
--plugins out/recipes/KitKraftOwnership.esm` is the intended check. Today it matches
nothing and reports 65 cells it cannot decide: `XOWN` sits where the ordered `CELL`
definition does not place it in those cells, so its layout is unknown there (the schema's
`unordered` record mode, a Request to lane B, `plan/STATUS.md`). Until then use
`kit inspect Starfield.esm:017307 --plugins Starfield.esm --plugins
out/recipes/KitKraftOwnership.esm`, which shows `XOWN` per version.

## In game (owner, M4.O4; KitKraft Dev, disposable save)

1. Enable `KitKraftOwnership.esm` on **KitKraft Dev** only; load a disposable save.
2. `coc CityNewAtlantisFreestarEmbassy`.
3. Look at an item on a shelf. **Expect:** the prompt says "Take", not "Steal".
4. Take it. **Expect:** no crime, no bounty.
5. Record the result in `docs/game-observations.md` (`game-<yyyymmdd>-<nn>`, with the hash
   above). A "Steal" prompt is a result too: the item may carry its own owner (`REFR`
   `XOWN`), which this recipe does not change.
