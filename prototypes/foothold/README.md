# Foothold prototypes

Foothold is KitKraft's reference application and benchmark (ADR 0011): these prototypes
exercise KitKraft's general capabilities against what a settlement mod needs, and they are
not Foothold content. No quest writing, narrative, balance or polish belongs here. Each
folder serves one or more Foothold experiments (`FTH-EXP-nn`, `plan/execution-plan.md`)
and has its own README with its purpose, the KitKraft commit it was last built with, what
was and was not authored, the unknowns, and the ticket's report block.

| Folder | Serves | What |
| ------ | ------ | ---- |
| `exp01-authored-site/` | FTH-EXP-01 | an authored exterior site with a `LocTypeOutpost` location and an outpost beacon, in two arms (KitKraft-authored references; vanilla's beacon pack-in by reference) |
| `exp03-claim-terminal/` | FTH-EXP-03 | a claim terminal and a scripted claim switch that sets `KitKraftSiteClaimed` once, a faction that owns the claimed cell, and the probes that read the claim before and after |

## Site lifecycle: the state model

Engineering surface C2 asks for a formal site lifecycle:
`unknown -> discovered -> hostile/contested -> secured -> eligible -> claimed -> restoring
-> operational`, plus `abandoned/lost` if needed. The prototypes take it as their state
model. Each state is defined by what can be observed, not by a flag a script chooses to set;
**eligibility is independent of hostiles being cleared** (C2): an active quest target, an
inhabited civilian facility or a repeatable mission node never becomes eligible by being
cleared.

| State | Meaning | How a prototype observes or stores it | Instantiated by |
| ----- | ------- | ------------------------------------- | --------------- |
| unknown | the player has not found the site | the site's map marker is not discovered | FTH-EXP-01 (its map marker; marker data is N2 there) |
| discovered | the player has found it | the map marker is discovered (visible, fast-travel target) | FTH-EXP-01, row W-07 |
| hostile/contested | enemies hold it | actors of a hostile faction alive in the site's location | not prototyped (FTH-EXP-03 later rounds; M5.H1 for encounters) |
| secured | no hostile remains | the same count reaches 0 (`Location.GetRefTypeAliveCount`, PS-04) | not prototyped |
| eligible | the site may be claimed | authored sites: eligible by construction (a curated, authored record; procedural sites only after FTH-EXP-06 and FTH-EXP-07, engineering surface B2) | FTH-EXP-03: `KitKraftSiteClaimed` = 0 at a site that has the claim switch |
| claimed | the player owns it | `KitKraftSiteClaimed` = 1, the owner faction on the cell (and, later, the location and references), the player in that faction; one claim per site, idempotent | FTH-EXP-03, rows C-02 to C-04 |
| restoring | repair stages under way | enable/disable groups per stage (FTH-EXP-04) | not prototyped |
| operational | the workshop works at the site | build mode opens at the beacon; objects place, power, move and survive reload | FTH-EXP-01, rows W-02 to W-06 |
| abandoned/lost | the player gave it up, or lost it | a documented transition back to eligible, with no silent loss (FTH-EXP-12; ADR 0011 §7: no arbitrary loss of the player's investment) | not prototyped |

Rules the prototypes follow, and later ones should:

- **One stored state per site.** FTH-EXP-03 stores it in one `GLOB` per site (0 eligible,
  1 claimed). Whether later states extend the same global (an index into this table) or a
  quest stage holds them is open until FTH-EXP-03's save/reload and reset rows pass
  (PS-39, U3 there).
- **Transitions are idempotent.** A transition that has happened changes nothing when
  triggered again (the claim switch's `Busy` and `Claimed` states).
- **State is observable by probes.** Every state a prototype stores has a console probe
  (`getglobalvalue`, `getinfaction`) or a manual row that says what to look at.
- **Persistence is not assumed.** A state that must survive a save, a reload or a cell reset
  is `not-run` until a probe shows it (`docs/design/persistence.md`).

## Export rule

This folder must stay exportable into the future Foothold repository without pulling
anything out of KitKraft's core (ADR 0011 §2):

- Everything here is self-contained: build specs (`*.toml`), our own Papyrus sources
  (`scripts/`), hand-written probe files and READMEs. Nothing refers to a path outside
  `prototypes/foothold/` except `kit` itself and the build outputs under `out/`
  (gitignored, never committed).
- Prototypes use KitKraft only through `kit` (CLI or MCP). Nothing under `crates/*`, `apps/*`
  or `xtask` reads this folder. A capability a prototype lacks is added to the core first as
  a general command or recipe, then used here (the "Not authored" tables name each one).
- Masters: `Starfield.esm` only. No DLC master, no Bethesda data, no Bethesda script source
  in the folder; vanilla meshes are used by reference (clones keep the vanilla model paths).
- Export with history: `git subtree split --prefix=prototypes/foothold`; or a plain copy.
