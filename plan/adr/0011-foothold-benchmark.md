# ADR 0011: Foothold is the benchmark, KitKraft is the deliverable

- Status: **Accepted** (owner ruling 2026-10-15: `docs/foothold/owner-decisions-2026-10-15.md`
  decisions 1 to 5, amended the same day by `docs/foothold/owner-addendum-2026-10-15.md`
  items 1 to 4; both verbatim in the repo)
- Date: 2026-10-15
- Scope: what Foothold is to this repository, where prototypes live, which game content a
  fixture may depend on, how the Creation Kit's script sources are used, and what the
  Foothold backlog (`docs/foothold/engineering-surface.md`) means for the plan. Plans M5
  (`plan/execution-plan.md`). No code.
- Relates to: ADR 0001 (clean room), ADR 0006 Amendment 1 (`.psc` declarations read from
  `ContentResources.zip`), ADR 0008 (runtime extensions), ADR 0009 (dev deployment), ADR
  0010 Amendment 1 (unattended runtime lane), AGENTS.md "Owner decisions" item 5 (Sim
  Settlements 2)

## Context

On 2026-10-15 the owner issued a Foothold capability inventory
(`docs/foothold/engineering-surface.md`), written against an older picture of KitKraft,
and ruled the same day on how it relates to the plan. The inventory lists everything a
settlement mod would need; the ruling says which part of that is this project's work.
`docs/foothold/gap-matrix.md` reconciles the inventory with what `main` proves.

## Decision

1. **KitKraft is the deliverable; Foothold is the reference application and benchmark.**
   Owner: "This development push is for KitKraft, not for delivering the Foothold mod."
   Foothold's needs choose which general capabilities to prove next and serve as their
   acceptance test. No Foothold quest writing, balancing, content completion, asset polish
   or release readiness is part of any KitKraft milestone (decisions, "Primary scope
   ruling"; addendum item 2). The gun overhaul and the first POI plugin are kept as
   regression and validation assets and are not expanded.
2. **Prototypes live in `prototypes/`, apart from production code.** Prototypes, workshop
   fixtures, scripted controllers, resource producers and test mods are allowed when they
   advance KitKraft's reusable capabilities, and must stay separate so they can be
   exported into the future Foothold repository without pulling game-specific behaviour
   out of core services (decisions, "Acceptance criteria"). Layout:
   - `prototypes/<name>/`, one folder per prototype (`prototypes/foothold/` for the
     Foothold ones), each self-contained: build specs (`*.toml`), our own `.psc` sources,
     and a `README.md` with its purpose, the KitKraft commit it was last built with, and
     the ticket ids (`FTH-EXP-nn`) it serves. Build output goes to `out/` (gitignored), as
     now.
   - Prototypes use KitKraft only through `kit` (CLI or MCP). Nothing under `crates/*`,
     `apps/*` or `xtask` reads `prototypes/`. When a prototype needs a capability, it is
     added to the core first as a general command or recipe, then used.
   - Export is `git subtree split --prefix=prototypes/foothold` (history kept) or a plain
     copy; nothing in the folder refers to a path outside it except `kit` itself.
   - Why `prototypes/` and not `mods/`: "mods" already names MO2's `mods\` folder, which
     ADR 0009 makes the one place `deploy --dev` writes. A repo folder with the same name
     invites the very confusion ADR 0009 guards against, and it suggests shippable mods,
     which these are not. `docs/recipes/` stays the home of general, game-agnostic
     recipes (one capability each, tested). A recipe that grows Foothold-specific
     behaviour moves to `prototypes/foothold/`.
3. **DLC and versions.** Target the locally installed game and CK, and record exact
   versions and master hashes. The owner's mapping, to be verified empirically (M5.F3):
   Free Lanes = `SFBGS00D.esm`; Terran Armada = `SFBGS050.esm` plus
   `BlueprintShips-SFBGS050.esm`; Moon Jumper = `SFBGS047.esm`. What the files already
   show (`kit load`, 2026-10-15, at `fcb443a`):
   - `SFBGS00D.esm` is a full master on `Starfield.esm`.
   - `SFBGS050.esm` is a full master on `Starfield.esm` **and on `SFBGS00D.esm`**.
   - `BlueprintShips-SFBGS050.esm` is a full blueprint master (flag 0x800) on
     `Starfield.esm`, `SFBGS00D.esm` and `SFBGS050.esm`.
   - `SFBGS047.esm` is a small master on `Starfield.esm`, `SFBGS004.esm` and
     `SFBGS00D.esm`.

   The files do not say which store product ships each one; M5.F3 records that from the
   install. Rules:
   - Fixtures prefer `Starfield.esm` as their only master.
   - Fixtures for Free Lanes or other content are allowed, are named as optional, and
     record their extra masters.
   - **No paid DLC is a mandatory dependency** of KitKraft, of a general recipe, or of a
     test that `cargo xtask ci` runs. Corpus tests over the official files keep skipping a
     file that is absent.
4. **Creation Kit sources: local extraction approved.** `Tools\ContentResources.zip` may
   be extracted under `C:\Modding\tools` for CK and Papyrus development, compiler
   reference tests and API discovery (decision 2). The extraction is never committed,
   redistributed, uploaded or bundled, and no Bethesda source file or compiler binary
   enters the repository. Each extraction is recorded in `docs/source-registry.md` (row
   CK-CONTENT-RESOURCES) with the zip's SHA-256 and size, the game build it came with, the
   folder, the date and the file count. ADR 0006 Amendment 1 is unchanged: facts are cited
   as `corpus:` with file and line, comments are never quoted, and reading the zip in
   place stays allowed. Observed 2026-10-15: `C:\Modding\tools\ck-sources\` exists, with
   54,396 files (5,038 of them under `Scripts\`) written 2026-10-15 19:26, next to the zip
   (SHA-256 `b5f3f66d...2c42983c`). Whether it is a complete, unmodified extraction is
   M5.B1's check.
5. **Sim Settlements 2** is Foothold's spiritual and mechanical predecessor. It is not a
   porting project and not part of this push (addendum item 1, which corrects decision
   3). Shared infrastructure is welcome; porting any SS2 content needs its own
   permissions and technical assessment. AGENTS.md owner decision 5 (contacting kinggath)
   is unchanged.
6. **Assets: vanilla mesh kits first.** Fixtures and prototypes reuse vanilla meshes,
   materials and textures by reference (clones keep the vanilla model paths). This is a
   sequencing preference, not a ban on custom meshes or a future art pipeline (decision 5).
7. **Player-agency constraints are recorded as future Foothold design constraints**, not
   as KitKraft milestones (addendum item 4): attacks optional and configurable; meaningful
   defenses; reliable notifications; nonpunitive unattended resolution; no arbitrary loss
   of accumulated player investment. A KitKraft fixture in these areas must not make them
   impossible (for example, an encounter fixture has a cancel path and a cooldown), and
   that is all it owes them.
8. **Hostile-encounter orchestration enters the backlog as a generic fixture**, not as
   the Foothold attack system (addendum item 3). First a research ticket (M5.H1):
   Starborn and Armillary encounters, vanilla outpost assaults and prior-art attack
   managers, in the corpus and in our docs, before any new runtime hook is proposed (ADR
   0008 §5 governs any hook), covering two of the addendum's bullets in its words:
   "Combat behavior, settlement defenses, damage, and recovery." and "Compatibility with
   vanilla outpost assaults and current DLC behavior." Then, only if the research finds a
   data and Papyrus path, one generic fixture (M5.H2): configurable trigger, actors,
   faction, spawn location and cooldown; deterministic test triggering; combat behavior,
   settlement defenses, damage and recovery; cancel and cleanup; save/reload; attended and
   unattended runs; compatibility with vanilla outpost assaults and current DLC behavior.
   (Corrected in the M5 review fix round, 2026-10-15, before any ticket ran: the first
   text omitted those two bullets of addendum item 3.)
9. **Tickets.** General KitKraft capability tickets are `M5.<lane><n>`; Foothold
   experiments are `FTH-EXP-nn` (the engineering surface's EXP-01..EXP-12, then later
   ones). Detailed tickets and their dependencies live in versioned Markdown
   (`plan/execution-plan.md` M5, `docs/foothold/dependency-graph.md`,
   `docs/foothold/next-sprint.md`). AGENTS.md carries only the reporting rules (decision
   4: "Don't duplicate the entire backlog into agent context").

## Consequences

- `plan/execution-plan.md` gains M5, "CK-backed world authoring and automated runtime
  testing", with the owner's priorities A to F as lanes and a lane H research ticket.
  Lane T (M4.T1..T10) becomes M5 lane C under its existing ids.
- AGENTS.md gains a binding "Reporting" section (ticket block, status vocabulary,
  evidence rule).
- `docs/foothold/gap-matrix.md` gives the M4-accurate status of every family and
  capability the inventory names, with evidence. `docs/foothold/engineering-surface.md`
  stays verbatim and is not an agent's to-do list.
- `docs/source-registry.md` gains CK-CONTENT-RESOURCES.
- `prototypes/` is created by the first ticket that needs it, with its README; nothing is
  moved there now.
- No refactor, no new crate and no content-production lane follow from this ADR
  (decisions, "Acceptance criteria"; addendum, last paragraph).
