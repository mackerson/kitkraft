# How well KitKraft supports the owner's mods, and the holes

Written 2026-11-14 from `main` at `a0b8b642`. Nothing was built or run; every "supported
today" cell is read off the repo's own evidence (`docs/foothold/gap-matrix.md`,
`docs/schema/evidence.toml`, `docs/gaps.md`, the recipes and briefs).

## The source: no "mod ideas" list was found

**I did not find a document that lists the owner's mod ideas as such.** Where I looked:

- `C:\Modding\kitkraft`: `docs/`, `plan/`, the repo root, `prototypes/`; file names matching
  idea, wish, backlog, brief, scenario, use-case, concept, pitch; a content search for "mod
  idea(s)", "wishlist", "target mod(s)", "ideas for mods" over `*.md`, `*.toml`, `*.txt`
  (no match); `\bideas?\b` in `docs/`, `plan/`, `prototypes/` (two unrelated hits).
- Git: file names on all 143 branches; `git log --all -i --grep=idea` (nothing).
- Beside the repo: `C:\Modding` to depth 5 (`everyday-outfits`, `kitkraft-observe`,
  `scripts`, `tools`, `kk-wt`), the other worktrees' `docs/` and `plan/`, the overnight
  folder `F:\AI\Scripts\overnight` (its "wishlist" is about the harness), and the project's
  Claude memory folder.
- Not searched: the owner's `Documents` folder (I listed its top level by mistake while
  looking, saw folders named `Kit` and `Obsidian`, and stopped; personal folders are
  outside the brief), cloud documents, chat history.

**Substitute, clearly labelled:** the mods the owner has actually written down in or beside
the repo. These are briefs and plans, not an ideas list, so this analysis may miss ideas
that live only in the owner's head or notes. If a list exists, point me at it and section 1
is redone; sections 2 and 3 are built to absorb new ideas (each hole names what it needs).

| # | Mod | Owner's document | State |
| - | --- | ---------------- | ----- |
| 1 | Gun overhaul, next build | `docs/overhaul/brief.md` | Build 2 passed in game; the brief's "out of scope" list is the next build's wish list |
| 2 | MEL DLC extension | `docs/mel-dlc/brief.md` | Built for two DLCs; bounty targets blocked; nothing run in game |
| 3 | Everyday Outfits | `C:\Modding\everyday-outfits\plan\PLAN.md` | Phase 1 in progress; seven requests filed against KitKraft, most landed |
| 4 | Foothold, Gate F0 (claim and restore one authored site) | `docs/foothold/engineering-surface.md` sections B, C, F | Data fixtures built; game rows mostly not run |
| 5 | Foothold, Gate F1 (resources, manufacturing, contracts, staff) | same, sections D, E | Not started beyond schema |
| 6 | Hostile encounters (raids on a site) | `docs/research/hostile-encounters-2026-10-16.md`, `prototypes/hostile-fixture/` | A fixture plugin built; not run in game |
| 7 | Sim Settlements 2 style port | `PLAN.md` "Rung 3" | "Later" (owner decision 5); a stress test, not a plan |

The rename and the first POI are the pipeline's smoke tests and are done; they are not
analysed.

## 1. Per mod

Status words follow the six statuses of AGENTS.md "Reporting". "Writable" means a test
writes every typed member (`docs/schema/evidence.toml`); "untested" means the generic path
accepts the type and nothing proves it.

### 1. Gun overhaul, next build

Needs, from the brief's out-of-scope list that data can reach: new ammunition records for
re-chambered weapons, with their projectile, and a way for the player to obtain them
(recipes, vendor and loot lists).

| Requirement | Today | Hole |
| ----------- | ----- | ---- |
| Override WEAP, AMDL, OMOD by rules | writable, game pass (`game-20261015-02`) | - |
| New `AMMO` and `PROJ` | typed; write untested | H7 |
| Recipe for the new ammo (`COBJ`) | writable; recipe exists (`docs/recipes/fabricator.md`); not seen in game | H3 |
| Add the ammo to vanilla vendor and loot lists (`LVLI`) | `LVLI` writable, but a field takes a whole array only, so adding one entry means restating the vanilla list and wins every conflict | **H5** |
| Check stats in game without the owner | testplan writes no probe rows for override-only stat edits | **H6** |
| Save/reload row O5-15 | not run; the controller refuses the hand-installed copy | H3 |
| Meshes, animations, audio, staged reloads | out of reach of records (brief) | none (permanent) |

### 2. MEL DLC extension

Needs: override `NPC_` level fields on DLC enemy templates by rule; one patch per source
plugin or a remap; in-game level checks.

| Requirement | Today | Hole |
| ----------- | ----- | ---- |
| Rule-driven `NPC_` overrides | built and validated for two DLCs; `NPC_` write is untested as a type | **H2** |
| One patch across DLCs | refused: a remap through `NPC_`'s opaque `NAM5` is refused (`docs/gaps.md`, "Patch FormIDs") | **H2** |
| Bounty targets in a medium-scale plugin (B2) | the brief says blocked; the scale-aware copy (M5.D12) merged on 2026-10-23, five days after the brief. **Probably unblocked, not re-tried** [unverified] | H2 (re-run) |
| Level check in game (`getlevel` on a spawned enemy) | "testplan writes no console rows for override-only level edits" (brief) | **H6** |
| Health measurement with and without MEL (B3) | manual | H6 |
| Any in-game row | not run | H3 |

### 3. Everyday Outfits

Needs: new `ARMO` from vanilla `ARMA` parts; material swaps (`LMSW`); new `.mat` and
textures packed; a tailoring recipe; later, distribution to vendors and NPCs; new meshes
from Blender (outside KitKraft).

| Requirement | Today | Hole |
| ----------- | ----- | ---- |
| `ARMO`, `ARMA`, `OTFT` new and clone | writable (KK-021) | - |
| `LMSW` swaps on `set`, `clone`, build spec | byte-exact (KK-019, KK-020); colourway recipe not seen in game | H3 |
| Check that a swap's `.mat` exists | rejected draft: vanilla materials live in the materials database, which KitKraft does not read | **H11** |
| Mesh and model checks | `asset.missing-model` (KK-018, KK-223) | - |
| Apparel probes | add, equip, look (KK-010) | - |
| Tailoring recipe | recipe exists; not seen in game | H3 |
| Vendors and NPC wardrobes (`LVLI`, `OTFT`, `NPC_`) | whole-array writes only; `NPC_` untested; "the one place a script injector may be needed" (PLAN) | **H5**, H2 |
| Texture and archive packing | `ba2.pack`, `package`; no packed plugin run in game | H3 |
| Workbench listing (owner task O2) | manual | owner |
| Meshes, morphs, weights | Blender pipeline, outside KitKraft; the viewport draws meshes only on the sprint branch | none here |

### 4. Foothold, Gate F0

Needs: an authored site (location, cell, references, map marker), a claim terminal with a
script, ownership, a workshop build zone, restoration stages by enable groups, persistence.

| Requirement | Today | Hole |
| ----------- | ----- | ---- |
| `LCTN`, `CELL` (interior), `REFR`, `ACTI`, `KYWD`, `PKIN`, `TERM`, `TMLM`, `FACT`, `GLOB` | writable; mini-site and exp01, exp03 built; mini-site ran in agent sessions on 2026-11-03 with one fail of open cause (T-20) | H3 |
| A site on a planet surface: new exterior cell or worldspace content | `new` refuses an exterior cell (`new.cell-exterior`); a new `WRLD` has no cells; `place` cannot add them | **H8** |
| Claim eligibility, terminal entries and recipes gated by conditions | conditions are copied as stored; **no test writes a `CTDA`** (gap matrix, "Conditions") | **H1** |
| Scripted claim: compile, attach, fire | compile and attach proven; fire in game: session `game-20261103-04` ran (see ledger); `OnActivate` variant open | H3 |
| Restoration stages (enable parents, initially disabled) | flag 0x800 only with `allow_reference`; enable-parent members typed on `REFR` | H3 (observe), minor |
| Navmesh for changed interiors | cannot generate; CK import path never exercised (F22) | H14 (permanent, CK) |
| Build mode at the site | FTH-EXP-01 game rows not run | H3 |
| Save, reload, revisit | proven for one crate only | H3 |
| Release as a small or medium master | new forms in small or medium plugins refused; no header conversion | **H9** |

### 5. Foothold, Gate F1

Adds: resource (`IRES`), flora (`FLOR`), research (`RSPJ`), recipes, a contract quest with
dialogue, staff NPCs with packages.

| Requirement | Today | Hole |
| ----------- | ----- | ---- |
| `IRES`, `FLOR` | typed; write tests are tonight's KK-259, KK-265 | H7 (in progress) |
| `RSPJ`, `PERK`, `AVIF` | typed; write untested | H7 |
| Recipe and research conditions | not writable | **H1** |
| Contract quest: stages, objectives, aliases | `QUST` writable; alias flags and fragments raw | H12 |
| Dialogue: `DIAL`, `INFO`, `SCEN` | `new` refuses nested types; `SCEN` actions raw (48 of 89 signatures unknown) | **H4** |
| Staff: `NPC_`, `PACK`, placed `ACHR` | `ACHR` writable; `NPC_`, `PACK` untested; package data inputs raw | H2, H7 |
| Planet content records | typed; write untested; component bodies raw | H10 |
| Localised text for release | `strings.*` exists; CK behaviour unobserved (F12) | H15 |

### 6. Hostile encounters

| Requirement | Today | Hole |
| ----------- | ----- | ---- |
| Quest controller, globals, script | built (`KitKraftHostileFixture.esm`, the first `QUST` from `new`) | H3 |
| Story-manager nodes and their conditions | typed; write untested; conditions not writable | **H1**, H7 |
| Leveled actors (`LVLN`), encounter `NPC_` | untested | H2, H7 |
| Placement outdoors | exterior cells refused for new cells | H8 |
| In-game run | not run | H3 |

### 7. Sim Settlements 2 style port

Not analysed in depth: it is "later" and gated on permission. What it would add to the hole
list is mostly scale: hundreds of scripts built as one project with dependency order
(H13), dialogue and scenes (H4), conditions everywhere (H1), and a Fallout 4 game profile
(out of scope today).

## 2. Consolidated holes, ranked by mods unblocked

Seven mods. "Unblocks" counts mods where the hole stops a stated requirement, not where it
is merely useful.

| Rank | Hole | Mods | Size | Needs the owner? |
| ---- | ---- | ---- | ---- | ---------------- |
| 1 | **H3. In-game evidence backlog.** Recipes, fixtures and specs that are built and never seen in game or reloaded | 7 of 7 | many small sessions | Game time on `KitKraft Dev`; agent sessions are allowed, someone must start them |
| 2 | **H1. Conditions cannot be authored.** No field path writes a `CTDA`; clones keep theirs as stored [done 2026-11-15: KK-267 to KK-272, merged `8d6c998a`; alias and packdata parameters stay raw, EL-03] | 4 (4, 5, 6, 7) | M to L; needs a design note first (function index, parameter unions, EL-03) | No |
| 3 | **H2. `NPC_` writes: no evidence, and remap refused through opaque `NAM5`** | 4 (2, 3, 5, 6) | S for the write test; M for typing `NAM5` or a narrower remap rule | No |
| 4 | **H5. No "add to a list" edit.** A list field takes a whole array, so injecting into a vanilla leveled list, form list or keyword list means restating it | 4 (1, 3, 5, 6) | S to M: an append and remove form of the field path, in `set` and `[[set]]` | No |
| 5 | **H7. Typed-write evidence for the remaining item, actor and quest-adjacent types** (`AMMO`, `PROJ`, `ZOOM`, `NPC_`, `LVLN`, `PACK`, `RSPJ`, `PERK`, story-manager types) | 4 (1, 2, 5, 6) | S each, one chain | No |
| 6 | **H6. Probes for override-only edits**: actor level and actor values on a spawned actor, weapon stats | 3 (1, 2, 3) | S to M, in `testplan/probes.rs`; answer formats are grade D until a session shows them | A session to confirm the formats |
| 7 | **H4. Dialogue and scenes**: `new` refuses `DIAL` and `INFO` (nested groups); `SCEN` actions raw [`DIAL` and `INFO` done 2026-11-15: KK-274 to KK-279, merged `92773276`, unverified against the CK; `SCEN` still raw] | 3 (5, 6, 7) | L; `SCEN` is a generator "hard case" | No, but design first |
| 8 | **H8. New exterior cells and world content** | 3 (4, 5, 6) | M to L; CK fixtures F20, F21 would raise confidence | CK sitting helps |
| 9 | **H9. Small and medium plugins**: new forms refused; no conversion of an existing header | all at release time | M; grade D without F06, F11, F26 | A ruling: build at grade D, or wait for fixtures |
| 10 | **H12. Quest alias flags and script fragments** | 3 (4, 5, 6) | M to L | No |
| 11 | **H10. Component-block bodies raw** (the component-name decider) [partly done 2026-11-15: KK-280 to KK-288, 22 of 64 kinds typed, merged `957773f5`; ADR 0015 accepted by the owner the same day, the decider being built as KK-292] | 2 to 3 (4, 5; planets) | L, format work | No |
| 12 | **H13. Papyrus as a project**: many scripts, dependency order, property checks across them | 2 to 3 (4, 6, 7) | M | No |
| 13 | **H11. Materials database not read**, so `.mat` references cannot be checked | 1 (3) | L, new format, needs a source ruling like M7.O5's | Yes (sources) |
| 14 | **H15. Localised strings on save, CK-checked** | all at release time | S once F12 exists | CK sitting |
| 15 | **H14. Navmesh generation** | 2 (4, 6) | not planned; the CK does it | permanent |

H3 ranks first on count but is not ticket work for the local agent. The best ticket-shaped
support for it is H6 plus anything that makes a session cheaper to start and read.

Things I checked and found **not** to be holes: byte-exact rewrites, new forms in full-scale
plugins, cross-references between new forms, script attachment, pack-in placement,
interior cells, packaging and dev deployment, previews and revert.

## 3. First batch, ticket-sized

Ten items in queue order. Format and rules: `plan/tickets/README.md`. Those marked
**draft** have a `drafts/<slug>.toml` beside this file with `id = "DRAFT"` (five drafts);
the others say what is missing before one can be written.

| # | Title | Hole | Owns | Acceptance idea | Depends | Draft |
| - | ----- | ---- | ---- | --------------- | ------- | ----- |
| 1 | Typed-write tests for NPC_, LVLN and PACK | H2, H7 | `authoring_tests.rs`, `evidence.toml`, `support-matrix.md` | `npc__new_sets_every_typed_member_and_reads_back` and siblings pass or are ignored with a reason; evidence entries for those that pass | KK-265 (same files) | **draft** `typed-write-npc-lvln-pack` |
| 2 | Typed-write tests for AMMO, PROJ and ZOOM | H7 | same three files | same pattern | item 1 | **draft** `typed-write-ammo-proj-zoom` |
| 3 | Typed-write tests for RSPJ, PERK and AVIF | H7 | same three files | same pattern | item 2 | **draft** `typed-write-rspj-perk-avif` |
| 4 | `set` adds or removes one entry of a list field | H5 | `crates/engine/src/edit/fields.rs`, `edit/mod.rs` tests, `docs/mcp.md` | a path form that appends to `KWDA` and to a struct list keeps the count member equal, refuses a duplicate FormID where the list is a set, and leaves other entries byte-identical | - | **draft** `set-list-append` |
| 5 | Build spec: `[[set]]` takes the list-append form, with a merge note | H5 | `crates/engine/src/build/spec.rs`, `handlers/build.rs`, `build_v3_tests.rs` | a spec appends one entry to a vanilla `LVLI` copied from the winning override; the plan names the override it built on | item 4 | no (wait for item 4's syntax) |
| 6 | `validate` warns on an override identical to the version before it | H3 hygiene; CK criticism 3 | `crates/engine/src/validate/mod.rs`, `validate/tests.rs`, `docs/validate.md` | a patch holding one identical and one changed override gets exactly one `override.identical` warning; `passed` stays true | the sprint merge (same files as `masters.unflagged`) [merged 2026-11-15, `23d62954`] | **draft** `validate-override-identical` |
| 7 | testplan: actor level and actor-value probe rows for an overridden NPC_ | H6 | `crates/engine/src/handlers/testplan/probes.rs`, `probes/tests.rs`, `testplan_tests.rs` | a plan for a plugin overriding an `NPC_`'s level fields has a spawn step and `getlevel` rows marked grade D; override-only plugins no longer get an empty plan | the sprint merge (KK-250, KK-251 own these files) | no: the spawn step and the console's answer format for `getlevel` are not in the repo; one agent session on the MEL DLC build settles them, then this is a 1.5 h ticket |
| 8 | Re-run the MEL DLC bounty-target spec against the scale-aware copy | H2 | `docs/mel-dlc/` (spec, brief status), one corpus test | `mel-dlc-trackers-alliance.toml` builds with its 20 forms or the refusal is recorded with its reason | - | no: it needs a release build against the game data, so it is a Claude-session task, not a headless ticket |
| 9 | Design note: authoring conditions | H1 | `docs/design/conditions.md` (new) | a doc: the field-path shape for a condition, which function parameters are writable today (sibling-decided unions), what stays refused (EL-03), three ticket cuts | - (Claude agent, not the local agent) | no (design) [done 2026-11-15: `docs/design/conditions.md`, KK-267] |
| 10 | Design note: nested records (`DIAL`, `INFO`) through `new`, and what `SCEN` needs | H4 | `docs/design/dialogue.md` (new) | a doc with the group path for a topic and its infos from the corpus, the refusals to lift, ticket cuts | - (Claude agent) | no (design) [done 2026-11-15 for `DIAL` and `INFO`: `docs/design/dialogue.md`, KK-274] |

Why this order: items 1 to 3 are the cheapest proof and unblock four mods' "untested"
cells; item 4 removes the single most common workaround (restating vanilla lists); items 6
to 8 serve the mods that already exist; 9 and 10 turn the two largest holes into tickets.

Not in the first batch, deliberately: H8 (exterior cells) and H9 (small and medium
plugins) each want a ruling or a CK fixture first; H10 is the component-name decider, which
already has its own line in `roadmap-remaining.md`.

## Confidence

- **Repo-proven:** every "today" cell that cites a ticket, test, recipe or `docs/gaps.md`
  row.
- **Inferred:** the mapping of each mod's requirements to record types where the owner's
  document does not name them (the overhaul's next build, the outfits' distribution step,
  hostile encounters' story-manager needs).
- **Unverified:** that MEL B2 now builds; the exact outcomes of the 2026-11-03 sessions for
  actors, ownership and scripts (I read their headings, not each row); that no list-append
  form exists anywhere (I searched `edit/fields.rs`, `set.rs` and the spec's table names and
  found whole-array writes only).
