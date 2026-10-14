# Roadmap

Milestones are gated by evidence, not dates. A milestone is done when its exit criteria pass
in `cargo xtask ci` (or in a documented corpus run), not when the code exists.

## M0: scaffold

Toolchain on the dev box, repository skeleton, agent operating docs, layer check, local CI.

Exit criteria:
- `rustup` stable with rustfmt and clippy installed.
- `cargo xtask ci` passes on the empty workspace (layers, fmt, clippy, test).
- Corpus = every `.esm` under `Starfield\Data`, listed with hashes in `corpus/manifest.toml`.
- First commit on `main`.

## M1: data parity

`kit roundtrip <esm> --verify` is byte-identical on the full official corpus: `Starfield.esm`,
every `SFBGS*.esm`, `Constellation.esm`, `OldMars.esm`, `BlueprintShips-*.esm`, and
`ShatteredSpace.esm`.

Exit criteria:
- Every corpus file roundtrips byte-identical; a mismatch reports file, record path, and
  offset.
- Subrecords without a schema definition are preserved as opaque bytes, never dropped.
- `docs/record-parity.md` is generated from the corpus (record types, subrecords seen vs
  known, roundtrip status).
- Truncated and bit-flipped corpus inputs return errors, never panic.

## M2: agent surface

"Agents can rip and tear": load-order resolution and the agent-facing commands, over CLI and
MCP. Tasks, lanes (A, B, D, E, F, G, owner) and ordering are in `plan/execution-plan.md`.

- `kit load` from an MO2 profile (read-only) or a plugin list; FormID resolution across
  masters, winning overrides, conflict graph; small/medium plugin forms only as far as a
  permitted source settles them.
- `inspect`, `query` (with the `where` mini-language), `diff` and `conflicts` (port of
  `esmscan.py`) across a load order; localized strings resolved.
- `set` (`FIELD=value`) into a non-localized patch plugin; `validate` (missing masters,
  dangling FormIDs, localized FULL copied as a string ID, stripped FULL/NTRM as in the
  Resupply door, flag/body mismatch, god-mode Papyrus calls); `build` from a minimal TOML
  spec.
- `kit mcp` stdio server (`execute`, `command_line`, `inspect_record`, `query_records`,
  `diff_plugins`, `coverage`), usable from Claude Code.
- Ports: `ba2x.py` -> `ba2` (GNRL + DX10; list, extract, pack), `pexdis.py` -> `pex`
  (`papyrus disasm`; `papyrus compile` wraps Bethesda's compiler, never `-release`), string
  tables (`strings dump`/`merge`).
- Typed records wired into the reader; weapon record types (WEAP, AMMO, PROJ, AMDL, OMOD,
  COBJ, ZOOM) typed as far as the owner's xEdit decision (ADR 0004) allows.
- Smoke test: rename the Old Earth Hunting Rifle to "Old Soviet Marksman Rifle" via `kit set`
  (and `kit build`) -> `kit validate` -> the owner loads it through MO2 and checks the name in
  game.

Exit criteria:
- `kit mcp` works from Claude Code with at least inspect, query, roundtrip and coverage.
- The rename plugin is built and validated; the owner's in-game result is recorded.
- Every catalog command is Implemented or explicitly deferred with a reason.
- `cargo xtask ci` is green and the full corpus roundtrip is still byte-identical.

## M3: CK parity checklist and the gun overhaul

Black-box the real Creation Kit: open the same plugin, save, diff its output against ours;
and use the typed weapon records for rung 2 of the step 2 ladder. Tasks, lanes (A, B, D, E,
F, G, H, owner) and ordering are in `plan/execution-plan.md`; the owner's six ADR 0004
clarifications bind every task.

- Lane B round 3 from the pinned xEdit files (once staged locally): WEAP, AMMO, PROJ, AMDL,
  OMOD, COBJ, ZOOM, then LVLI, FLST, INNR, and the 105 corpus lstring signatures typed;
  confidence per ADR 0004; the SF1Edit differential once SF1Edit is installed.
- CK differential observation (ADR 0005 §2): a fixture matrix (empty plugin, one and two
  masters, mixed master tiers, full/medium/small exports, WEAP/AMMO/GMST edits, a compressed
  record, `XXXX`, localized vs not) saved by the owner or a desktop-control agent; findings
  in `docs/ck-observations.md` raise confidence to `ck-diff`.
- Reproduce the CK's TES4 header, ONAM, override and compression choices on save behind a
  `ck` write style; the omitted TES4 `SNAM`/`BNAM`/`CHGL` and the 160 compressed records
  that do not recompress identically come first.
- Rank CK editor features in `docs/ck-parity.md`; viewport explicitly deferred.
- Gun overhaul kit: `build` spec v2 (classes, type-wide rules over typed weapon fields,
  link following, dry run), `kit overhaul` scaffold, a design brief the owner fills from
  the chosen critique, and a first real build on the owner's profile.
- Oracles run as programs, never read: SF1Edit and Spriggit on fixtures, disagreements
  recorded as evidence in `docs/oracles.md`.
- Hardening from `docs/gaps.md` and the open cross-lane requests.
- UI: an ADR 0007 proposal (egui shell over the command catalog) as an owner decision; no
  implementation unless the owner opts in.

Exit criteria:
- Round 3 types decode and re-encode identical on all 14 corpus files, every member sourced.
- Every fixture the CK produced has ledger entries; `style: "ck"` reproduces the CK's bytes
  for each expressible edit fixture, or every difference is explained.
- The overhaul build validates, and the owner's in-game result on a disposable save is
  recorded.
- `cargo xtask ci` is green and the full corpus roundtrip is still byte-identical.

## M4: coverage program

Cover the record families the owner's mod needs, create new forms and place them in the
world, and prove each of the owner's twelve workstreams (binary authoring, worlds and POIs,
workshop, ownership, persistence and procedural locations, Papyrus, planets and resources,
manufacturing, NPCs, quests, UI and assets, packaging) with a first deliverable. Tasks,
lanes (A, schema families BR/BW/BP/BQ/BC/BI with the integrator BX, D, E, F, G, H, owner),
the workstream-to-capability matrix and ordering are in `plan/execution-plan.md`; the six
ADR 0004 clarifications still bind every task, and there is no architecture restart (no
new crate, no layer change).

- Schema family lanes, each typed from the pinned xEdit files with lane B's acceptance:
  WORLD and PLANET (running, including the Planet Content Manager records and leveled
  pack-ins), QUEST, ACTOR, ITEMS/WORKSHOP; an integrator regenerates generated output and
  parity after each merge.
- Builder: cell and world child groups (block and sub-block structure from the corpus),
  reference creation with position and rotation, a new-form allocator, `NAVM` passthrough
  from CK-saved plugins (KitCraft does not generate navmesh).
- Commands: `new`, `clone`, `place`, build spec v3, `refs` across families, `planet`,
  `package` (MO2-ready folder, optional BA2, hashed manifest), `testplan` (console batch
  files, disposable-save checklist), `papyrus.events` (signatures from the 5,034 base
  sources); validation rules for cells.
- Research: persistence and the planet content system as design documents with sources;
  UI, asset and audio formats scoped only.
- Owner: a CK session for world fixtures (including navmesh), two in-game sessions on
  disposable saves, decisions on SFSE-dependent features (default no) and on installing
  into MO2 directly (default no). Sim Settlements 2 stays "later".

Exit criteria:
- The first POI placed by KitCraft is visited in game on a disposable save, with its
  persistence steps recorded.
- Every P0 workstream's first deliverable is done; every P1 one is done or carried with the
  reason; UI, assets and audio have their scoping document.
- The QUEST, ACTOR and ITEMS/WORKSHOP families decode and re-encode identical on all 14
  corpus files, every member sourced; the builder passes the cell rebuild gate.
- `cargo xtask ci` is green and the full corpus roundtrip is still byte-identical.

## M5+: editor

- egui UI over the command catalog (`crates/ui-egui`, `apps/kit`), unless the owner's
  ADR 0007 decision brings it earlier.
- NIF and material metadata.
- Procedural placement and SFSE-dependent features, if M4's research and the owner's
  decision call for them.
- Render viewport (`crates/render`), last.
