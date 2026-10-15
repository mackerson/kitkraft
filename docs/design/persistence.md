# Persistence, resets and procedural locations (research, M4.H3)

**Status: research skeleton.** What Starfield keeps and what it resets for placed
references, quest aliases and procedurally placed points of interest, and what a mod that
claims and restores POIs and registers outpost objects must do to survive cell resets and
saves. Written 2026-10-15 for workstream W5 (`plan/execution-plan.md`, M4.H3); it feeds the
`ref.persistence` rule (`docs/validate.md`), `kit testplan`'s persistence steps and the
owner's sessions M4.O2 and M4.O4 (`docs/game-observations.md`).

Game build: the corpus of `corpus/manifest.toml` (Starfield.exe 1.16.244.0, the owner's
install of 2026-10-14) and its `Tools\ContentResources.zip`. Behaviour can change with a
game update; every claim below is for this build.

## How to read this document

Each claim has an id (`PS-nn`), a grade and a source.

- **Grades** follow ADR 0005 Amendment 3: **A** upstream code, official tool output or a
  fixture we reproduce; **B** documented experiment or maintained documentation; **C**
  secondary overview or analogy (another game, a setting's name); **D** hypothesis. The
  claim is graded, not the source. A grade D claim is written as a question and names the
  in-game step that would answer it (`docs/game-observations.md` row ids `O2-..`/`O4-..`).
- **Sources.** `corpus:` our own tools on the official files (`kit query`, `kit coverage`,
  `kit inspect`, lane A's corpus rules); `psc:` a declaration in the official Papyrus
  sources (`Tools\ContentResources.zip`, `Scripts\Source\<Script>.psc:<line>`), checked with
  `kit papyrus events` (Appendix A lists every one). Where a claim rests on the comment
  Bethesda wrote next to a declaration, it is paraphrased in our words, never quoted (ADR
  0006 Amendment 1), and graded B: it is the vendor's own documentation, not an
  observation. `schema:` a member of our definitions, which cite the pinned xEdit file
  (`docs/xedit-reference.md`). `dossier:` `docs/research/starfield-dossier-2026-10-14.md`.
- Nothing here comes from SFSE, CommonLibSF or any decompiled engine code (ADR 0008).
  No CK wiki prose is copied.

### Sources consulted this round, and what came of each

| Source | Registry key | Result |
| ------ | ------------ | ------ |
| Official Papyrus sources, through `kit papyrus events` | CORPUS-ARCHIVES | Every event and function named below exists; signatures and file:line in Appendix A (grade A for existence) |
| The 14 official ESMs, through `kit` | CORPUS | Counts and values in Appendix B |
| Creation Kit wiki, <https://www.creationkit.com/index.php?title=Persistence_(Papyrus)> | CK-WIKI | Not readable: the site redirects to `wiki.bethesda.net/Main/Maintenance/` (checked 2026-10-15), as CK-HELP says. No claim rests on it |
| UESP mirrors of the Skyrim and Fallout 4 CK wikis (`ck.uesp.net`, `falloutck.uesp.net`) | not registered | Refused automated reading (HTTP 403, a bot check that we do not bypass). Not used; a registry row is needed before they are cited (`docs/source-registry.md`) |
| Papyrus Index | PAPYRUS | Cross-check only: its `ObjectReference` event list was compared with Appendix A and agrees; the note is in `plan/STATUS.md` (the registry allows its citation only there or in a test). No fact here is taken from it |
| The dossier, section 6 | dossier | Papyrus semantics and runtime behaviour are a separate pipeline from data mods; the CK and a disposable save are the acceptance oracles (its section 5, last rows). Grade of what it says about persistence: none; it makes no persistence claim |
| Community guides and forum threads (POI cooldowns, outpost object events) | not registered | Seen as search-result titles only, not read (several refuse automated reading). Their topics appear below only as grade D questions, never as findings |

Earlier Creation Engine games' documentation is not used as a source this round (it could
not be read). Where this document asks whether Starfield behaves like those games, that is
a hypothesis (D) and says so.

## Vocabulary (our words)

- **Placed reference**: a `REFR`, `ACHR` or another placed record inside a cell's children
  group, naming a base form.
- **Persistent / temporary**: stored in the cell's persistent children (group type 8) or
  temporary children (type 9). What the engine does differently with each is the subject of
  this document.
- **Attached / loaded cell**: the Papyrus sources distinguish a cell that is "attached"
  (in the loaded area around the player) from one that is loaded (PS-14).
- **Reset**: the engine putting a cell, location, reference or quest back to its plugin
  state (PS-21..PS-28).
- **Alias**: a quest's named slot for a reference, a location or a collection of
  references (`QUST` alias blocks).

## Persistent references on disk

| Id | Claim | Grade | Source | Test |
| -- | ----- | ----- | ------ | ---- |
| PS-01 | A placed reference has record flag `0x400` set exactly when it sits in its cell's persistent children (group type 8). This holds for all 6,714,413 cell children of the 14 official files, without exception | A | corpus: lane A's M4.A1 rule `ref.persistent-flag` (`docs/validate.md`) | - (fact) |
| PS-02 | `kit place` with `persistent = true` writes the reference into type 8 with `0x400`; the first POI's crate `KitKraftFirstPOI.esm:001000` is written that way (group path CELL top > block 9 > sub-block 9 > cell children > persistent children) | A | corpus: `docs/poi/first-poi.md` (build, `inspect`, `validate` 0/0) | O2-07, O2-08 |
| PS-03 | Locations keep lists of persistent references: `LCTN` `LCPR` (the location's own list) in 517 of the 6,017 locations of `Starfield.esm`; an `ACPR` list ("added": a plugin adding references to a master's location) and an `RCPR` list ("removed") are defined; `ACPR` occurs 4 times in the official files, `RCPR` never | A | schema: `LCTN.toml` (`AddedPersistentRefs`, `PersistentRefs`, `RemovedPersistentRefs`); corpus: `kit coverage Starfield.esm` | - |
| PS-04 | Does the engine need a new persistent reference listed in its location's `ACPR`/`LCPR` (for example to find it with location-based alias fills or `GetRefTypeAliveCount`)? Does the CK write an `ACPR` override of the location when it saves such a reference? | D | question; psc: `Location.GetRefTypeAliveCount` (`Location.psc:46`) | CK fixture F20/F25 (M4.O1, deferred); O2-12 |
| PS-05 | A `REFR` can name a "persist location" (`XLCN`, 118 of 3,291,860 `REFR` in `Starfield.esm`), and Papyrus can set one on a reference created in game (`ObjectReference.SetPersistLoc(Location)`, whose official comment says it applies to references created at runtime) | A (record, declaration), B (meaning) | schema: `REFR.toml` `PersistLocation`; corpus; psc: `ObjectReference.psc:1096` | - |
| PS-06 | What a persist location changes in game (which cell the reference is kept with when its location unloads) | D | question | O4-10 |
| PS-07 | Location reference types (`XLRT`) are on 8,321 `REFR` of `Starfield.esm`; locations carry `LCSR` (special references, 714) and `LCUN`/`LCUR` (unique actors and base forms, 153 each) | A | corpus: `kit coverage` | - |

## What keeps a reference alive at runtime

| Id | Claim | Grade | Source | Test |
| -- | ----- | ----- | ------ | ---- |
| PS-08 | `ObjectReference.PlaceAtMe` creates references at run time; its `abForcePersist` parameter defaults to `false`, `abDeleteWhenAble` to `true`, and it can fill an alias directly (`akAliasToFill`). Its official comment says persistence is forced only when asked | A (signature), B (meaning) | psc: `ObjectReference.psc:841` | - |
| PS-09 | Is a reference created by `PlaceAtMe` without `abForcePersist` cleaned up by the engine once nothing holds it (no script variable, property or alias)? Does it survive leaving and re-entering the cell, a save and reload, and a cell reset? | D | question (earlier games behaved so; not read this round) | O2-10 (console `placeatme`), O4-11 (script, needs M4.E9) |
| PS-10 | Does a script property that points at a placed reference keep that reference persistent even when the plugin stores it in temporary children? `ref.persistence` warns on exactly this case as grade D | D | question; `docs/validate.md` `ref.persistence` | O4-12 |
| PS-11 | `ObjectReference.Delete` marks a reference for deletion; `Disable`/`Enable` hide and show it, `IsDisabled`/`IsDeleted`/`Is3DLoaded` report state | A (declarations), B (meaning) | psc: `ObjectReference.psc:299, 302, 325, 723, 729, 705` | - |
| PS-12 | Is a disabled persistent reference's disabled state kept across save and reload and across a cell reset? (The usual "place it persistent and initially disabled, enable it from a script" pattern depends on it) | D | question | O4-13 |

## Quest aliases

| Id | Claim | Grade | Source | Test |
| -- | ----- | ----- | ------ | ---- |
| PS-13 | `Starfield.esm` quests hold 19,079 reference aliases (`ALST`), 3,111 location aliases (`ALLS`) and 4,859 reference-collection aliases (`ALCS`). Fill types seen: a specific reference (`ALFR`, 1,070), a unique actor (`ALUA`, 2,502), a created object (`ALCO`, 1,257, placed by `ALCA` 1,270), a reference of a location (`ALRT` ref type 6,605, `ALFA` 6,981), an external alias (`ALEQ`, 1,041), an event's data (`ALFE`, 750), a specific location (`ALFL`, 705) and a planet content request (`ALPS`, 75) | A | corpus: `kit coverage Starfield.esm` (QUST subrecord counts); schema: `QUST.toml`, `common/QuestReferenceAlias.toml` | - |
| PS-14a | Reference-alias flags are defined for: reserves the reference, optional, quest object, essential, protected, allow disabled, allow destroyed, initially disabled, "create ref temp", and more (`QuestReferenceAlias.toml` `FNAM`) | A (layout) | schema: `common/QuestReferenceAlias.toml` (xEdit at the pin) | - |
| PS-14b | What "create ref temp" does to a created object's persistence (by its name: the created reference is temporary, not persistent) | C | name only | O4-14 |
| PS-15 | Aliases are changed by script with `ReferenceAlias.ForceRefTo`, `ForceRefIfEmpty`, `Clear` (the official comment says clearing fails on non-optional aliases) and `RefCollectionAlias.AddRef`/`RemoveRef`; alias scripts receive `OnAliasInit`, `OnAliasReset` (the owning quest was reset) and `OnAliasShutdown` | A (declarations), B (meaning) | psc: `ReferenceAlias.psc:14, 18, 8`; `RefCollectionAlias.psc:539, 560`; `Alias.psc:22, 26, 31` | - |
| PS-16 | Does filling a reference into an alias of a running quest keep it in memory (and its changes in the save) while the alias holds it, and does `Clear` or `Quest.Stop` release it? | D | question | O4-15 |
| PS-16b | Quests start, stop and reset with `Quest.Start`, `Stop`, `Reset`, `IsRunning`; a quest script receives `OnQuestInit`, `OnQuestShutdown`, `OnReset` and, for story-manager quests, `OnStoryActorAttach` | A (declarations), B (meaning) | psc: `Quest.psc:227, 238, 189, 164, 252, 257, 290, 306` | - |

## Load, attach and reset events

| Id | Claim | Grade | Source | Test |
| -- | ----- | ----- | ------ | ---- |
| PS-17 | `OnLoad` (the object finished loading; the official comment says it fires on every load), `OnUnload`, `OnCellAttach` and `OnCellDetach` (the object's parent cell attached or detached), `OnCellLoad` (every object of the parent cell loaded; the official comment marks its limits as unknown to its authors) and `OnReset` (the reference was reset) are declared on `ObjectReference`, and each is also declared on `ReferenceAlias`, `RefCollectionAlias` (with the sender reference as a parameter) and `ActiveMagicEffect` | A (declarations), B (meaning) | psc: `ObjectReference.psc:1657, 1754, 1595, 1599, 1603, 1709` and the alias and effect scripts (Appendix A) | - |
| PS-18 | A cell is "attached" when it is in the loaded area (`Cell.IsAttached`), separately from `Cell.IsLoaded`; a location reports `Location.IsLoaded` and sends `OnLocationLoaded` | A (declarations), B (meaning) | psc: `Cell.psc:16, 22`; `Location.psc:73, 142` | - |
| PS-19 | In which order do `OnCellAttach`, `OnLoad` and `OnCellLoad` fire on entering a cell, and does `OnLoad` fire again after a save is loaded with the player already in the cell? | D | question | O4-16 |
| PS-20 | Do objects built at an outpost receive `OnUnload`/`OnCellDetach` when the player moves about inside the outpost's worldspace, as ordinary references do? | D | question | O4-17 |

## Cell and location resets

| Id | Claim | Grade | Source | Test |
| -- | ----- | ----- | ------ | ---- |
| PS-21 | `Starfield.esm` sets `iHoursToRespawnCell` = 168, `iHoursToRespawnCellCleared` = 480 and `iDaysToRespawnVendor` = 2 | A | corpus: `kit inspect Starfield.esm:0D3646`, `:0D3647`, `:0A2D7A` | - |
| PS-22 | By their names, a cell's temporary contents come back after 168 game hours (7 days) away, or 480 hours (20 days) once "cleared", and vendors restock every 2 days | C | setting names only | O2-09 |
| PS-23 | Scripts can force resets: `Cell.Reset` (the official comment: flags the cell for a reset on its next load), `Location.Reset` (resets the encounter zones and interior cells that use the location), `Location.ForceResetWithActors` (the same, actors included), `ObjectReference.Reset` (optionally moving the reference) | A (declarations), B (meaning) | psc: `Cell.psc:25`; `Location.psc:105, 108`; `ObjectReference.psc:962` | O4-18 |
| PS-24 | A location can be exempt from resets: `LCTN` `DATA` has a "never resets" level flag, set on 17 of the 6,017 locations of `Starfield.esm` (for example `VecteraWorldSpaceLocation`, `LC165BuriedTempleLocation`); `Location.SetNeverResets(bool)` changes it at run time; the "workshop" level flag is set on none | A (data, declaration), B (meaning) | schema: `LCTN.toml` `DATA.LevelFlags`; corpus: `kit query LCTN --where 'DATA.LevelFlags & 1'` (17), `& 8` (0); psc: `Location.psc:126` | - |
| PS-25 | `Starfield.esm` has no encounter-zone (`ECZN`) records, although `Location.Reset`'s official comment still speaks of encounter zones | A | corpus: `kit coverage Starfield.esm` (no `ECZN` type) | - |
| PS-26 | After a reset, does a plugin's **persistent** reference keep its plugin position and enable state, while **temporary** references and container contents return to their plugin state? Does a reset undo a script's `MoveTo` (`ObjectReference.psc:826`) or `Disable` on a persistent reference? | D | question | O2-09, O4-18 |
| PS-27 | Is a location flagged "never resets" (data or `SetNeverResets`) enough to keep a claimed POI's temporary objects as the player left them? | D | question | O4-19 |
| PS-28 | `Location.IsExplored`, `SetExplored` and `HasEverBeenExplored` exist; whether "explored" or "cleared" picks the 168-hour or the 480-hour timer is not known. There is no `IsCleared`/`SetCleared` in Starfield's Papyrus sources | A (existence and absence), D (link to the timers) | psc: `Location.psc:64, 114, 58`; `kit papyrus events --name IsCleared` (0 matches) | O4-19 |

## Procedural locations (planet content)

| Id | Claim | Grade | Source | Test |
| -- | ----- | ----- | ------ | ---- |
| PS-29 | `Starfield.esm` holds 5 `PCMT` (planet content manager trees), 336 `PCBN` (branch nodes), 1,805 `PCCN` (content nodes), 11,281 `PKIN` (pack-ins), 51 `LVLP` (leveled pack-ins), 431 `BIOM`, 1,765 `PNDT` (planets) and 80,584 `RFGP` (untyped yet; M4.BX2) | A | corpus: `kit coverage Starfield.esm` | - |
| PS-30 | Settings named `uPlanetContentMinRequestsPerBlock`, `uPlanetContentMaxRequestsPerBlock`, `uPlanetContentMinWorldspaceRequestsPoints`, `uPlanetContentMaxWorldspaceRequestsPoints` and `fPlanetContentMaxNewWorldspacesPerScan` exist | A (existence), C (meaning by name) | corpus: `kit query GMST --where "EDID ~ 'PlanetContent'"` | - |
| PS-31 | Quest location aliases can be filled by a planet content request (`ALPS` marker with conditions and a request keyword, 75 in `Starfield.esm`), and scripts receive `OnPlanetSiteSelectEvent(Location)` | A | schema: `QUST.toml` `PlanetContentRequest`; corpus; psc: `ScriptObject.psc:476` | - |
| PS-32 | Which generated POIs are kept per landing site and which are regenerated on a later landing, and whether a reference a plugin adds to a pack-in or a generated cell persists with the site | D | question; M4.D3 (`docs/design/planet-content.md`, not on `main`) and the M4.E10 experiment | O4-20 |
| PS-33 | A plugin cannot predict a procedural POI's cell or FormIDs: they are made at run time and live only in the save, which KitKraft does not read (W5, "Cannot") | A (scope) | `plan/execution-plan.md` W5 | - |

## Outposts and workshop registration

| Id | Claim | Grade | Source | Test |
| -- | ----- | ----- | ------ | ---- |
| PS-34 | Outpost building raises `OnWorkshopObjectPlaced`, `OnWorkshopObjectRemoved` (the object went back to the workshop), `OnWorkshopObjectMoved` and `OnWorkshopObjectGrabbed` on `ObjectReference` (and on aliases and effects); a beacon placement raises `OnOutpostPlaced(ObjectReference)` on `Actor`, aliases and effects | A (declarations), B (meaning) | psc: `ObjectReference.psc:1774, 1778, 1770, 1766`; `Actor.psc:1083` (Appendix A) | O4-05 |
| PS-35 | A reference finds its outpost with `ObjectReference.GetWorkshop()`; `SetWorkshopState`, `StartWorkshop`, `GetWorkshopOwnedObjects` and `GetWorkshopResourceObjects` exist; the official `OutpostEventHandlerParent` script declares `HandleOnWorkshopObjectPlaced`/`Removed` | A | psc: `ObjectReference.psc:636, 1114, 1168, 568, 575`; `OutpostEventHandlerParent.psc:4, 8` | - |
| PS-36 | What makes an object buildable at an outpost (its `COBJ`, keywords, form lists) is M4.E7's question, answered from vanilla records with `query` and `refs` | - | `plan/execution-plan.md` M4.E7 | O4-05 |
| PS-37 | Are outpost-built objects saved as created references (persistent with the outpost) and kept through cell resets of their cell? | D | question | O4-17 |

## Ownership (claiming)

| Id | Claim | Grade | Source | Test |
| -- | ----- | ----- | ------ | ---- |
| PS-38 | A cell's owner is data (`CELL` `XOWN`, 65 cells of `Starfield.esm`); a location also names an owner (`LCTN` `DATA.Owner`); scripts change them with `Cell.SetFactionOwner`/`SetActorOwner`, `Location.SetFactionOwner` and `Cell.SetPublic`; a single reference has its own `ObjectReference.SetFactionOwner`/`SetActorOwner` | A | schema: `common/OwnerData.toml`, `LCTN.toml`; corpus; psc: `Cell.psc:31, 28, 50`, `Location.psc:117`, `ObjectReference.psc:1049, 997` | O4-01, O4-02 |
| PS-39 | Does a runtime owner change (`SetFactionOwner`) survive a cell reset, or does the reset restore the plugin's `XOWN`? | D | question | O4-21 |

## What a claim/restore and outpost mod must do

Design rules for the owner's mod (claim a POI, restore it, register objects with an outpost),
from the claims above. Each is as strong as its weakest claim.

1. **Place everything a script, alias or location must find as persistent** (`[[place]]`
   `persistent = true`; type 8 and `0x400` as in PS-01/PS-02). `validate` warns
   (`ref.persistence`) when a temporary reference is named from outside its cell. Grade:
   A for what KitKraft writes; D for the engine needing it (PS-10).
2. **Keep the mod's state off temporary references.** Store "claimed / restored" in a
   quest script's variables, a `GLOB`, or a quest stage, not on a temporary reference or a
   container that can reset (PS-22, PS-26). D until O2-09 and O4-18 answer.
3. **Re-apply state on every load, idempotently.** `OnLoad` and `OnCellAttach` fire again
   on each load (PS-17, B): a handler that checks the stored state and sets ownership,
   enable state or linked objects to match it survives a reset whatever the reset did.
   The firing order is open (PS-19).
4. **Hold claimed references in aliases while claimed.** `ForceRefTo` on claim, `Clear` on
   restore (aliases must be optional to clear, PS-15); alias scripts get the same load
   events. Whether the alias keeps the reference alive is PS-16 (D).
5. **Prefer data for ownership; use script for the switch.** An override's `XOWN`
   (M4.E8, `docs/recipes/ownership.md`) sets the start state; `Cell.SetFactionOwner` at
   claim time changes it in the save. Whether a reset restores the data owner is PS-39 (D):
   rule 3 covers it either way.
6. **Decide each claimed location's reset policy explicitly.** Either `SetNeverResets(true)`
   on claim (PS-24, B; effect PS-27, D) or accept resets and rely on rule 3.
7. **Script-created objects:** pass `abForcePersist = true` (and an alias to fill) to
   `PlaceAtMe` for anything that must outlive the cell's next unload (PS-08, PS-09).
8. **Outpost objects:** react to `OnWorkshopObjectPlaced`/`Removed` (PS-34) and find the
   outpost with `GetWorkshop()` (PS-35); registration itself is M4.E7's open question.
9. **Procedural POIs cannot be targeted by FormID.** Reach them through quest aliases
   filled at run time (location aliases with conditions, `ALPS` planet content requests,
   PS-31) and keep what you learn in the quest, not in the plugin (PS-33).
10. **Never remove a plugin that owns forms from a save in progress** (AGENTS.md, "Owner's
    live game is off limits"); a persistent reference is such a form. If state proves
    impossible to keep with these tools, a save hook is a runtime-need record (ADR 0008
    §5), not something this design assumes.

## Gotchas

- **Persistent is a group, not just a flag.** A reference with `0x400` in temporary
  children (or the reverse) matches no official file (PS-01); `validate`'s
  `ref.persistent-flag` refuses it.
- **Encounter zones are gone from the data** (PS-25): reset behaviour in Starfield hangs on
  locations and cells, so a mod cannot borrow an encounter zone's settings.
- **Runtime IDs are load-order dependent** (`docs/load-order.md`): the crate is `04001000`
  on Kitkraft Dev only; `kit testplan` names references by EditorID first and says whether
  each numeric ID is confirmed by a save or predicted.
- **What the save holds is invisible to KitKraft.** Changes made in game (moves, disables,
  owners, alias fills) live in the save, which KitKraft does not read (W5 "Cannot"); it
  reads only a save's plugin lists (`load --from-save`, `docs/load-order.md`, "Save
  headers").
- **The 168/480-hour timers count game time away from the cell** (PS-22, C): `wait` in the
  same cell may not count; O2-09 tests it from another cell.
- **Removing a plugin from a save** leaves its persistent references orphaned there; only
  disposable saves are used for tests (AGENTS.md).

## Owner session items

Each open question becomes a row in the owner's ledger (`docs/game-observations.md`). Rows
that need a script wait for M4.E9 (script attachment) and are marked so there.

| Question | Ledger row | Needs |
| -------- | ---------- | ----- |
| PS-09 (`PlaceAtMe` without force-persist across leave, save, reset) | O2-10, O4-11 | none for O2-10; M4.E9 for O4-11 |
| PS-10 (script property keeps a temporary reference persistent) | O4-12 | M4.E9 |
| PS-12 (disabled state across save and reset) | O4-13 | none (console `disable`) |
| PS-14b ("create ref temp" alias flag) | O4-14 | a quest fixture (F25) |
| PS-16 (alias keeps a reference alive) | O4-15 | F25 |
| PS-19 (event order, `OnLoad` after a save is loaded in the cell) | O4-16 | M4.E9 |
| PS-20, PS-37 (outpost objects: unload events, persistence) | O4-17 | M4.E7 |
| PS-22, PS-26 (reset timers; persistent vs temporary after a reset) | O2-09, O4-18 | none (console only) |
| PS-24/27/28 (never-resets and explored vs cleared) | O4-19 | M4.E9 for `SetNeverResets` |
| PS-32 (procedural POIs per landing) | O4-20 | M4.D3, M4.E10 |
| PS-39 (runtime owner change across a reset) | O4-21 | M4.E9 |
| PS-04 (location lists for new persistent references) | O2-12 | none |
| PS-06 (persist location) | O4-10 | M4.E9 |

## Appendix A: Papyrus verification (`kit papyrus events`)

Lane G asked (M4.G2, `plan/STATUS.md` Requests) that every event and function this
document names be checked against the official sources. Run on 2026-10-15 with the release
build of this branch, `KITKRAFT_DATA_DIR` = the game's `Data` (the zip is
`..\Tools\ContentResources.zip`), one command per name:

```powershell
kit papyrus events --name <Name> --declarations-only --json
```

Below: the name, the number of declarations, and each declaring script with its file and
line. Every name the document uses matched; `IsCleared`, `SetCleared` and `IsPersistent`
match nothing, which PS-28 relies on (no other name is claimed absent).

```text
OnLoad                         4: ActiveMagicEffect ActiveMagicEffect.psc:125; ObjectReference ObjectReference.psc:1657; RefCollectionAlias RefCollectionAlias.psc:644; ReferenceAlias ReferenceAlias.psc:424
OnUnload                       4: ActiveMagicEffect ActiveMagicEffect.psc:242; ObjectReference ObjectReference.psc:1754; RefCollectionAlias RefCollectionAlias.psc:781; ReferenceAlias ReferenceAlias.psc:537
OnCellAttach                   4: ActiveMagicEffect ActiveMagicEffect.psc:63; ObjectReference ObjectReference.psc:1595; RefCollectionAlias RefCollectionAlias.psc:582; ReferenceAlias ReferenceAlias.psc:370
OnCellDetach                   4: ActiveMagicEffect ActiveMagicEffect.psc:67; ObjectReference ObjectReference.psc:1599; RefCollectionAlias RefCollectionAlias.psc:586; ReferenceAlias ReferenceAlias.psc:374
OnCellLoad                     4: ActiveMagicEffect ActiveMagicEffect.psc:71; ObjectReference ObjectReference.psc:1603; RefCollectionAlias RefCollectionAlias.psc:590; ReferenceAlias ReferenceAlias.psc:378
OnReset                        5: ActiveMagicEffect ActiveMagicEffect.psc:185; ObjectReference ObjectReference.psc:1709; Quest Quest.psc:290; RefCollectionAlias RefCollectionAlias.psc:700; ReferenceAlias ReferenceAlias.psc:484
OnInit                         1: ScriptObject ScriptObject.psc:413
OnWorkshopObjectPlaced         4: ActiveMagicEffect ActiveMagicEffect.psc:262; ObjectReference ObjectReference.psc:1774; RefCollectionAlias RefCollectionAlias.psc:805; ReferenceAlias ReferenceAlias.psc:561
OnWorkshopObjectRemoved        4: ActiveMagicEffect ActiveMagicEffect.psc:266; ObjectReference ObjectReference.psc:1778; RefCollectionAlias RefCollectionAlias.psc:809; ReferenceAlias ReferenceAlias.psc:565
OnWorkshopObjectMoved          4: ActiveMagicEffect ActiveMagicEffect.psc:258; ObjectReference ObjectReference.psc:1770; RefCollectionAlias RefCollectionAlias.psc:801; ReferenceAlias ReferenceAlias.psc:557
OnWorkshopObjectGrabbed        4: ActiveMagicEffect ActiveMagicEffect.psc:254; ObjectReference ObjectReference.psc:1766; RefCollectionAlias RefCollectionAlias.psc:797; ReferenceAlias ReferenceAlias.psc:553
OnOutpostPlaced                4: ActiveMagicEffect ActiveMagicEffect.psc:571; Actor Actor.psc:1083; RefCollectionAlias RefCollectionAlias.psc:969; ReferenceAlias ReferenceAlias.psc:774
OnPlanetSiteSelectEvent        1: ScriptObject ScriptObject.psc:476
OnLocationLoaded               1: Location Location.psc:142
OnAliasInit                    1: Alias Alias.psc:22
OnAliasReset                   1: Alias Alias.psc:26
OnAliasShutdown                1: Alias Alias.psc:31
OnQuestInit                    1: Quest Quest.psc:252
OnQuestShutdown                1: Quest Quest.psc:257
OnStoryActorAttach             1: Quest Quest.psc:306
PlaceAtMe                      1: ObjectReference ObjectReference.psc:841
SetPersistLoc                  1: ObjectReference ObjectReference.psc:1096
Reset                          6: AffinityEvent AffinityEvent.psc:7; Cell Cell.psc:25; InputEnableLayer InputEnableLayer.psc:161; Location Location.psc:105; ObjectReference ObjectReference.psc:962; Quest Quest.psc:189
ForceResetWithActors           1: Location Location.psc:108
SetNeverResets                 1: Location Location.psc:126
Delete                         2: InputEnableLayer InputEnableLayer.psc:12; ObjectReference ObjectReference.psc:299
Disable                        1: ObjectReference ObjectReference.psc:302
Enable                         1: ObjectReference ObjectReference.psc:325
IsDisabled                     1: ObjectReference ObjectReference.psc:729
IsDeleted                      1: ObjectReference ObjectReference.psc:723
Is3DLoaded                     1: ObjectReference ObjectReference.psc:705
MoveTo                         1: ObjectReference ObjectReference.psc:826
GetRefTypeAliveCount           1: Location Location.psc:46
IsAttached                     1: Cell Cell.psc:16
IsLoaded                       2: Cell Cell.psc:22; Location Location.psc:73
SetPublic                      1: Cell Cell.psc:50
SetFactionOwner                3: Cell Cell.psc:31; Location Location.psc:117; ObjectReference ObjectReference.psc:1049
SetActorOwner                  2: Cell Cell.psc:28; ObjectReference ObjectReference.psc:997
IsExplored                     1: Location Location.psc:64
SetExplored                    1: Location Location.psc:114
HasEverBeenExplored            1: Location Location.psc:58
ForceRefTo                     1: ReferenceAlias ReferenceAlias.psc:14
ForceRefIfEmpty                1: ReferenceAlias ReferenceAlias.psc:18
Clear                          2: LocationAlias LocationAlias.psc:4; ReferenceAlias ReferenceAlias.psc:8
AddRef                         2: RefCollectionAlias RefCollectionAlias.psc:539; SQ_PreventRecalcScript SQ_PreventRecalcScript.psc:6
RemoveRef                      2: RefCollectionAlias RefCollectionAlias.psc:560; SQ_PreventRecalcScript SQ_PreventRecalcScript.psc:16
Start                          2: Quest Quest.psc:227; Scene Scene.psc:7
Stop                           4: EffectShader EffectShader.psc:7; Quest Quest.psc:238; Scene Scene.psc:10; VisualEffect VisualEffect.psc:7
IsRunning                      2: Actor Actor.psc:410; Quest Quest.psc:164
GetWorkshop                    1: ObjectReference ObjectReference.psc:636
SetWorkshopState               1: ObjectReference ObjectReference.psc:1114
StartWorkshop                  1: ObjectReference ObjectReference.psc:1168
GetWorkshopOwnedObjects        1: ObjectReference ObjectReference.psc:568
GetWorkshopResourceObjects     1: ObjectReference ObjectReference.psc:575
HandleOnWorkshopObjectPlaced   1: OutpostEventHandlerParent OutpostEventHandlerParent.psc:4
HandleOnWorkshopObjectRemoved  1: OutpostEventHandlerParent OutpostEventHandlerParent.psc:8
IsCleared                      0 matches
SetCleared                     0 matches
IsPersistent                   0 matches
```

## Appendix B: corpus checks

All on `Starfield.esm` alone (`--plugins Starfield.esm`), release build, 2026-10-15:

| Check | Command | Result |
| ----- | ------- | ------ |
| Record and subrecord counts (PS-03, PS-05, PS-07, PS-13, PS-25, PS-29) | `kit coverage <Data>\Starfield.esm --json` (`record_types[].records`, `.signatures[].count`) | as quoted in the claims |
| Reset settings (PS-21) | `kit query GMST --where "EDID ~ 'Respawn'"`, then `kit inspect` each | `iHoursToRespawnCell` 168, `iHoursToRespawnCellCleared` 480, `iDaysToRespawnVendor` 2 |
| Planet content settings (PS-30) | `kit query GMST --where "EDID ~ 'PlanetContent'"` | 6 settings (5 named in PS-30, plus an overlay water-depth threshold) |
| Never-resets locations (PS-24) | `kit query LCTN --where 'DATA.LevelFlags & 1'` (paged) | 17 of 6,017; workshop flag (`& 8`) 0 |
| Persistent flag vs group (PS-01) | lane A's `ref.persistent-flag` corpus run (`docs/validate.md`) | 6,714,413 cell children, no exception |
