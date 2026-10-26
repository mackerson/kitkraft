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

## Variant: a new faction owns the cell

M5.D5 asks for the other case: not the vanilla owner changed to the vanilla
`PlayerFaction`, but a faction the plugin itself creates owns the cell (the Foothold
claim terminal prototype's owner). Spec: `docs/recipes/ownership-faction.toml`; a second
`[[set]]` of the same cell points `XOWN.Owner` at the new faction with `new:owner`, as
`docs/recipes/mini-site.toml` names a new location with `new:location`.

| Fact | Source |
| ---- | ------ |
| The new faction is `KitKraftOwnershipFaction.esm:001000`: a `FACT`, object `0x1000` of master `01`, EditorID `KitKraftRecipeOwnerFaction`, `FULL` "KitKraft Recipe Owner Faction" | `kit build` of the spec reports the new form; the `edid`, `FULL` and the first `0x001000` allocation are the spec's own values |
| The cell's winning `XOWN.Owner` names the new faction, resolved, not dangling | `kit refs Starfield.esm:017307 --plugins Starfield.esm --plugins out/recipes/KitKraftOwnershipFaction.esm`: `XOWN.Owner.Owner -> KitKraftOwnershipFaction.esm:001000 FACT KitKraftRecipeOwnerFaction` |
| `XOWN.Owner` may name a `FACT` (or an `NPC_`) | `xedit: wbDefinitionsCommon.pas@9fb01688#L9367-L9391` (the `OwnerData` shared type, `crates/schema/defs/common/OwnerData.toml`) |
| A `FACT` with only `FULL` builds and validates clean | `docs/recipes/mini-site.toml` builds the same FULL-only `FACT` (`KitKraftMiniSiteFaction`); this build's `validate` reports 0 errors and 0 warnings; `FULL` is an optional lstring (`crates/schema/defs/FACT.toml`) |

The faction carries only members the recipe can source from `FACT.toml`: `FULL`. No
relations, ranks, crime values or flags are set.

### Build

```text
kit build docs/recipes/ownership-faction.toml out/recipes/KitKraftOwnershipFaction.esm
kit validate --plugin out/recipes/KitKraftOwnershipFaction.esm
kit roundtrip out/recipes/KitKraftOwnershipFaction.esm --verify
kit testplan out/recipes/KitKraftOwnershipFaction.esm out/recipes/ownership-faction-test --profile <MO2>\profiles\Default
```

2026-10-26 (release build): 15,228 bytes, SHA-256
`e3575858608ad4894af50984430879c41d2e2179ccf3358636c07899f1d09af4`, one master (`Starfield.esm`); `validate`
0 errors and 0 warnings; `roundtrip --verify` identical. The plugin owns one form (the faction) and one override
(the cell's `XOWN.Owner`); the cell's `FULL` is written inline (patches are
non-localized); everything else is the winner's bytes except `XOWN.Owner`.

### What is not known

- Whether the game treats the player as a trespasser or a thief in a cell its own new
  faction owns: **not known**. The override variant's in-game check (above, M4.O4)
  covers the `PlayerFaction` case only; this case is grade D until an observation.
- Whether the game accepts a `FACT` that carries no `DATA` or `CRVA` (this faction has
  only `FULL`): **not observed** — the 953 corpus FACTs all carry `DATA`, `CRVA` and
  `VENV` (`crates/schema/defs/FACT.toml`).
- How a player would join the new faction: `player.addtofaction <xx>001000 0` in the
  console is the check, where `<xx>` is the plugin's load-order index (the built
  plugin's FormID is `01001000`; in game the leading byte is the plugin's position in
  the load order) (grade D; not run).

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
