# FTH-EXP-03 prototype: scripted claim terminal

Serves **FTH-EXP-03** (stable claim; `plan/execution-plan.md`, Foothold experiments;
`docs/foothold/engineering-surface.md` C2 and J; the persistence claims PS-38 and PS-39 of
`docs/design/persistence.md`). This is the agent-executable half: the spec, our script,
the built plugin, its checks and the probes that read the claim state before and after.
The game rows are the owner's or lane C's (M5.C2 or the M5.O4 sitting) and are all
`not-run`.

Last built with KitKraft at `97dc5ee` (release `kit.exe` of that commit), 2026-10-16,
game 1.16.244.0, `Starfield.esm` the only master. The plugin owns new forms and a script:
disposable saves on **KitKraft Dev** only, never removed from a save in progress.

In the lifecycle model of `../README.md` this prototype is the transition
**eligible -> claimed**: `KitKraftSiteClaimed` is the site's persistent state (0 eligible,
1 claimed) and the activation is the transition, which may happen once.

## What it builds

| Form | What | From |
| ---- | ---- | ---- |
| `FACT` `:001000` `KitKraftFootholdOwnerFaction` | the owner of a claimed site | `[[clone]]` of `FC05_FurnitureOwnershipFaction` (`Starfield.esm:1EF91D`): a vanilla faction that exists only to own things (`DATA` flags 0, no relations, no ranks; its one referrer gives it to an alias) |
| `TMLM` `:001001` `KitKraftClaimTerminal_Menu` | the terminal's menu: title "KitKraft Site Claim", one body text (ours, inline), no items | `[[clone]]` of `RL048LabTerminal_Desktop` (`181AF7`: one body text, no items, conditions or script) |
| `TERM` `:001002` `KitKraftClaimTerminal` | "KitKraft Claim Terminal", `DNAM` = the menu above | `[[clone]]` of `RL048LabTerminal` (`181B08`): standing terminal model, furniture marker, keywords and the vanilla `TerminalScript` (no properties) kept |
| `ACTI` `:001003` `KitKraftClaimSwitch` | "KitKraft Claim Switch" | `[[clone]]` of `ActivatorSwitch01` (`03CCF4`), the scripts recipe's base |
| `GLOB` `:001004` `KitKraftSiteClaimed` | `FLTV` 0.0 | `[[new]]` |
| `REFR` `:001005` `KitKraftClaimTerminalRef` | the terminal at (-15, -2, 4.63), persistent | `[[place]]` |
| `REFR` `:001006` `KitKraftClaimSwitchRef` | the switch at (-14, -2, 4.63), persistent, `VMAD` `KitKraft:SiteClaim` with `Claimed` = the `GLOB`, `OwnerFaction` = the `FACT`, `Label` "KitKraft first POI site" | `[[place]]` with `scripts`, as `docs/recipes/scripts.md` attaches `HelloOnLoad` |

Both references are in the first POI's cell, `CityNewAtlantisApartmentTEMPLATE`
(`Starfield.esm:21966F`, interior, no `XOWN` in `Starfield.esm`), two and three units from
the first POI crate (-17, -4, 4.63), on its floor height (`docs/poi/first-poi.md`). The cell
is carried as an unchanged copy, as the first POI plugin carries it.

**The script** (`scripts/KitKraft/SiteClaim.psc`, ours) extends `ObjectReference`. On
`OnActivate` in its auto state `Unclaimed` it moves to state `Busy` before any call that can
yield, ignores anyone but the player, and if `Claimed` is still 0 sets it to 1, adds the
player to `OwnerFaction`, sets the parent cell's faction owner to it
(`Cell.SetFactionOwner`, PS-38), traces and notifies, then moves to `Claimed`. In `Busy` and
`Claimed` an activation only traces. If the global is already 1 (an earlier save, the
console), the first activation changes nothing either. Every declaration it uses is from
`kit papyrus events` (`ObjectReference.psc:1591` `OnActivate`, `:517` `GetParentCell`,
`GlobalVariable.psc:4, 7`, `Cell.psc:31`, `Actor.psc:602` `AddToFaction`, `Game.psc:239`,
`Debug.psc:100` `Trace`).

## Build and checks

```text
kit papyrus compile prototypes/foothold/exp03-claim-terminal/scripts/KitKraft/SiteClaim.psc out/prototypes/foothold/exp03/Scripts --sources-zip "<Starfield>\Tools\ContentResources.zip"
kit build prototypes/foothold/exp03-claim-terminal/claim.toml out/prototypes/foothold/exp03/KitKraftFootholdClaim.esm
kit validate --plugin out/prototypes/foothold/exp03/KitKraftFootholdClaim.esm
kit roundtrip out/prototypes/foothold/exp03/KitKraftFootholdClaim.esm --verify
kit refs KitKraftFootholdClaim.esm:001006 --plugins Starfield.esm --plugins out/prototypes/foothold/exp03/KitKraftFootholdClaim.esm
kit testplan out/prototypes/foothold/exp03/KitKraftFootholdClaim.esm out/prototypes/foothold/exp03/testplan --plugins Starfield.esm --spec prototypes/foothold/exp03-claim-terminal/claim.toml
```

Results (2026-10-16): `SiteClaim.pex` 2,371 bytes, SHA-256
`b58611fe6e513782f7ac80aa34160bce4ad5ac66d5c1805fc9b4824f4bd0df41` (PapyrusCompiler
4.7.0.5, not release mode; a recompile may give other bytes).
`KitKraftFootholdClaim.esm` 4,620 bytes, SHA-256
`f9a58d82cfcb833f00e71348b4e3f87ebfd1796ffbfa0c0b346a65af1c14481f`. The build checked
`Claimed`, `OwnerFaction` and `Label` against the compiled script's property table.
`validate` passed, 0 errors and 0 warnings on all 17 rules (`papyrus.missing-script`,
`papyrus.property-unresolved` and `formid.dangling` included); `roundtrip --verify`
identical (9 records, 10 groups). `refs` resolves every link, none dangling: the switch
reference's `VMAD.Scripts.Data.KitKraft:SiteClaim.Claimed` to the `GLOB` and
`.OwnerFaction` to the `FACT`, and its base; the terminal's `DNAM` to the new menu, its 5
keywords and its furniture template. `inspect` of the menu shows the title and body text
as written. `testplan` wrote `checklist.md`, `kitkraft-test.txt` and `probes.toml` (15
rows: each new form found by EditorID or name, both references' cell, position and runtime
ID, and the persistence rows) into `out/prototypes/foothold/exp03/testplan/`.

## Probes: the claim state before and after

`claim-probes.toml` (here; run after the generated `probes.toml`, same plugin hash):

| Row | Checks |
| --- | ------ |
| C-01 | before: `getglobalvalue KitKraftSiteClaimed` is 0; `player.getinfaction KitKraftFootholdOwnerFaction` is 0 |
| C-02 | first activation (`prid KitKraftClaimSwitchRef`, `activate player`): the global is 1, the player is in the faction, the Papyrus log has "KitKraftSiteClaim: KitKraft first POI site claimed"; screenshot |
| C-03 | second activation: the global is still 1, the log has "... already claimed, nothing changed" |
| C-04 | leave, save, reload: the global is 1 and the faction kept; a third activation still changes nothing |
| C-05 | manual: Take or Steal on an item in the cell, before and after the claim (ownership scope) |
| C-06 | manual: the terminal opens its menu with the KitKraft text |

## Not authored (and why)

| # | What | Why | Closes it |
| - | ---- | --- | --------- |
| N1 | A menu item on the terminal that performs the claim (the confirmation UX of C2) | a `TMLM` item's action is a script fragment in its `VMAD`, which is opaque in the schema (`TMLM` `VMAD`); so the claim is on the switch and the terminal only explains it | lane B (`VMAD` fragments) and M5.D3 (`TERM`/`TMLM` writes) |
| N2 | Body text or items conditioned on `KitKraftSiteClaimed` | conditions are copied as bytes; `build` writes no new `CTDA` | conditions typed for writing (lane D) |
| N3 | Location-scope ownership (an `LCTN` with `DATA.Owner` = the faction) and reference-scope ownership | out of this prototype's scope: the cell scope is the one the script sets; FTH-EXP-03 asks for a row per scope | the next FTH-EXP-03 round (an `LCTN` as FTH-EXP-01's, and `ObjectReference.SetFactionOwner`) |
| N4 | The player joining the owner faction through a quest alias, as vanilla gives `FC05_FurnitureOwnershipFaction` (`QUST` `FC05` `ALFC`) | no quest in this prototype; the script calls `AddToFaction` instead | a quest fixture (F25) |

## Unobserved (each with the experiment that resolves it)

| # | Unknown | Resolved by |
| - | ------- | ----------- |
| U1 | What the console prints for `getglobalvalue`, `getinfaction` and `activate player` (grade D) | C-01 to C-04 in the M5.C2 session (or M5.O4) |
| U2 | Whether object properties naming forms of the same plugin (`GLOB`, `FACT`) resolve at run time (the build checks names and types against the `.pex`, not the object type: a `docs/recipes/scripts.md` limit) | C-02 |
| U3 | Whether `Cell.SetFactionOwner` at run time survives a cell reset, or the reset restores the plugin's (absent) `XOWN` (PS-39) | O4-21 (`docs/design/persistence.md`), then C-05 after a reset |
| U4 | Which ownership scope theft and trespass follow (reference, cell, location; engineering-surface C2) | C-05, then FTH-EXP-03's per-scope rows |
| U5 | Whether two activations queued at the same moment stay idempotent (the `Busy` state is the guard); the console can only activate in sequence | a scripted double activation in a later round; C-03 covers the sequential case |
| U6 | Whether the cloned terminal with the vanilla `TerminalScript` opens the new menu | C-06 |

## Report

```yaml
id: FTH-EXP-03
priority: EXP
owner: lane E
requires: [M5.D3, M5.D5, M5.B3, M5.C2]
output: prototypes/foothold/exp03-claim-terminal/{claim.toml, claim-probes.toml, scripts/KitKraft/SiteClaim.psc}; KitKraftFootholdClaim.esm f9a58d82cfcb833f00e71348b4e3f87ebfd1796ffbfa0c0b346a65af1c14481f; SiteClaim.pex b58611fe6e513782f7ac80aa34160bce4ad5ac66d5c1805fc9b4824f4bd0df41
source_provenance: [corpus:Starfield.esm FACT 1EF91D, TMLM 181AF7, TERM 181B08, ACTI 03CCF4, CELL 21966F; corpus:ContentResources.zip ObjectReference.psc:517 :1591, GlobalVariable.psc:4 :7, Cell.psc:31, Actor.psc:602, Game.psc:239, Debug.psc:100]
validation:
  parse: pass            # kit validate + roundtrip, 2026-10-16 at 97dc5ee
  byte_roundtrip: pass   # kit roundtrip --verify: identical
  ck_open: not-run
  ck_reexport: not-run
  game_runtime: not-run
  save_reload: not-run
limits: [N1-N4 above; ownership at cell scope only; sequential idempotency only (U5)]
result: partial
```

`result` is read off the rows (AGENTS.md, "Reporting"): the two KitKraft rows passed, so
the ticket is `partial`, not `hypothesis`. The experiment itself (game, save and reload,
the CK) is still a hypothesis: none of its rows has run.
