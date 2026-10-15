# Owner decisions, 2026-10-15: KitKraft M4 review and Foothold scope correction

Owner's words, verbatim (chat, 2026-10-15). Amended the same day by
`docs/foothold/owner-addendum-2026-10-15.md`. Binding on the plan;
`plan/adr/0011-foothold-benchmark.md` records the decision.

Fable's review is accepted. The earlier Foothold engineering brief is a target-capability
inventory, not an accurate description of current implementation status.

## Primary scope ruling

This development push is for KitKraft, not for delivering the Foothold mod.

Foothold remains our principal reference application and capability benchmark. You may create
substantial prototypes, workshop fixtures, scripted claim controllers, resource producers,
example POIs, and working test mods when they advance KitKraft's reusable capabilities.

These development artifacts may later be transferred to the independent Foothold project for
expansion and refinement.

However, no Foothold-specific quest writing, balancing, content completion, asset polish, or
release readiness is required for KitKraft milestones.

Preserve the existing gun overhaul and POI plugin as regression/validation assets. Do not expand
them as standalone content projects during this push.

## Decisions

1. **Game version and DLC:** Target the locally installed Starfield/Creation Kit versions,
   recording exact versions and master hashes. Free Lanes is associated with SFBGS00D.esm,
   Terran Armada with SFBGS050.esm and its ship-blueprint companion, and Moon Jumper with
   SFBGS047.esm. Verify all local master dependencies empirically.

   Prefer Starfield.esm-only fixtures where feasible. Optional Free Lanes and other
   content-specific fixtures are authorized; no paid DLC becomes a mandatory core dependency.

2. **Creation Kit sources:** APPROVED for local extraction of ContentResources.zip under
   C:\Modding\tools for legitimate CK/Papyrus development, compiler reference tests, and API
   discovery. Do not commit, redistribute, publicly upload, or bundle Bethesda source files or
   compiler binaries. Keep them outside the repository and record provenance and hashes.

3. **Sim Settlements 2:** Separate prospective project, neither replaced by Foothold nor part
   of this push. Shared infrastructure is encouraged. Content porting requires its own
   permissions and technical assessment.

4. **Tickets and reporting:** APPROVED. Use binding, concise reporting requirements in
   AGENTS.md. Keep detailed tickets and dependencies in versioned Markdown. Distinguish general
   KitKraft capability tickets from Foothold-specific experimental tickets. Don't duplicate the
   entire backlog into agent context.

5. **Assets:** APPROVED. Reuse vanilla mesh kits for initial fixtures and prototypes. This is a
   sequencing preference, not a permanent prohibition on custom meshes or future art pipelines.

6. **ADR 0010 automation:** APPROVED IN PRINCIPLE. Build an unattended runtime experiment lane
   using isolated game profiles, disposable saves, explicit authorized paths, backups,
   reproducible setup, logs, screenshots, timeout handling, and rollback. Never run tests
   against personal saves or unrelated mod installations. Treat missing observational evidence
   as inconclusive, not passed. Retain manual acceptance where automated evidence is
   insufficient.

## Next priorities, after the current QA wave

A. CK oracle lane: establish the first golden CK-authored world/POI fixtures. Add CK
   import/export differential testing to cargo xtask oracle.

B. Papyrus lane: test the scripted activator in game and compare KitKraft-generated PEX with
   the official compiler. Compare normalized semantics, not only raw bytes.

C. Runtime lane: establish repeatable, isolated automated game tests. Record actual
   save/reload and game-runtime results.

D. Authoring lane: complete the support-depth matrix; prioritize typed-write capability and
   reference integrity for LCTN, TERM, PKIN, and FACT.

E. Persistence/workshop lane: experimentally investigate claimed-site identity, resets,
   interior construction, Free Lanes shared storage, and ownership behavior.

F. Documentation: create an evidence-backed engine limitation register and reconcile the
   Foothold capability backlog against M4 reality.

**Existing technical policy remains in effect:** pinned xEdit definitions, license separation,
opaque passthrough, independent implementation, SFSE support when justified, safe deployment,
and bounded director reports.

## Acceptance criteria

Do not count schema presence as runtime feature support.

Maintain distinct statuses for parsed, typed-readable, typed-writable, CK-differential-tested,
game-runtime-tested, and save/reload-verified capabilities.

Every reported successful experiment must link reproducible evidence.

Keep prototypes separate from KitKraft production infrastructure so they can be exported into
the eventual Foothold repository without extracting game-specific behavior from core services.

No broad refactor or content-production swarm is authorized by this decision.

**Deliverable:** An M4-accurate gap matrix, updated dependency graph, next-sprint ticket plan,
and the shortest validated path to generalized CK-backed world authoring and automated runtime
testing.
