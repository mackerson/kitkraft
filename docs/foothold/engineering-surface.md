# KitKraft → Foothold: Engineering surface, dependency map and acceptance checklist

**Prepared:** 2026-10-14  
**Intended reader:** KitKraft swarm director, parser/schema/toolchain lanes, Creation Kit integration team, Foothold design and QA.  
**Status:** Forward-looking engineering specification and investigation backlog. **Not** a claim that all engine capabilities have been proven.  
**Context:** KitKraft can now successfully round-trip an edited `WEAP` record (gun rename). Foothold is the proposed Starfield outpost-reclamation and unique-site resource-loop mod. Preserve the current M1 trajectory; use this as an additional target-driven backlog, not authorization for a sweeping refactor.

## Project north star

**Explore → Discover → Secure → Claim → Restore → Operate → Trade → Grow → Return.**

The player may reclaim *eligible* abandoned installations (hangars, labs, factories), retain and repair their recognizable structures, decorate and build around them, produce resources at unusual biological/geological sites, employ staff, and experience persistent consequences. Exploration grants useful places, not merely survey percentages. Keep the host game's aesthetic and existing systems intact.

**Design invariants**

- The *site* is the central entity: history, identity, ownership, state, existing structures, production, staff, and persistence all attach to a specific physical site.
- Never treat a visually identical POI template as a unique site instance. Template/base FormID ≠ occurrence identity.
- Byte exactness is necessary for safe rewriting but insufficient for gameplay correctness. A data-valid plugin may still contain a broken quest, missing workshop, or unstable POI.
- Prefer non-destructive overrides, new forms, native workshop systems, and Papyrus. Adopt SFSE/native hooks only after a scoped, reproducible CK/Papyrus limitation.
- Protect player investment: no disappearing inventories, settlements, or repairs on cell reset, return travel, save/load, time skips, or updates.
- The first released version may support only a *curated list* of confirmed persistent locations. Arbitrary procedural POI claiming is a separately gated research milestone.
- Preserve opaque/unknown record data. Never infer record definitions or master types. The schema must distinguish source fact, implementation interpretation, and CK/game validation.
- Avoid overwriting canonical vanilla POI templates, quest-critical actors, navmeshes, and markers globally. Prefer instances or additive form records with safe references.
- Keep feature modules interoperable while avoiding early abstract-factory complexity; ensure game-specific schema/semantic boundaries exist.
- **Do not expand scope because AI makes implementation cheap.** Gameplay coherence, asset quality, compatibility, and tested persistence remain scarce resources.

## Labels and completion convention

- **P0 — critical path:** required to demonstrate an authored, persistent, claimable abandoned site.
- **P1 — Foothold core:** required for the first compelling resource/outpost gameplay loop.
- **P2 — expansion:** deeper faction, NPC, UI, procedural coverage, and polish.
- **EXP — investigation:** engine behavior is *not* yet established; no implementation promise until a repro passes.
- **DEFER — explicitly outside the initial vertical slice.**

Every checked task should link to (a) a commit/PR, (b) a fixture or artifact, (c) verification log, and (d) owner. A checkbox represents demonstrable capability, **not** just a schema declaration. For any runtime-sensitive capability, require game-build/version, CK version, seed/location and fresh-save vs upgraded-save information.

---

# A. KitKraft authoring/toolchain foundation

## A1. Container and record writer [P0]

- [ ] Demonstrate byte-exact no-op round trips on a corpus across **multiple record classes and GRUP nesting patterns** (not just `WEAP`).
- [ ] Read/write the `TES4` header, masters list, `GRUP`s, record headers, compressed/uncompressed records, subrecords and extended-size `XXXX` cases.
- [ ] Preserve record order, unknown record bodies, unknown subrecords, base-form components and opaque/reflection payloads without dropped bytes.
- [ ] Distinguish **parse**, **write no-op**, **semantic edit**, and **CK/game load** as four separate gates.
- [ ] Support targeted edits without rewriting unrelated records; diff byte spans and report unexpected changes.
- [ ] Create **new** forms (not just rename/override existing ones), with collision-safe identifiers and well-formed group placement.
- [ ] Create and preserve references between newly created forms and existing masters.
- [ ] Detect invalid record sizes, invalid references and corrupted/unsupported structures before writing.
- [ ] Compare writes against independent oracles (SF1Edit/xEdit, Mutagen/Spriggit where practical).
- [ ] Generate minimal reproducible fixtures from clean Creation Kit exports and retain the baseline plugin hash.

## A2. Master-aware identity and dependency graph [P0]

- [ ] Parse full/medium/small plugin flags as independent dimensions where appropriate; never conflate master flag, size tier, update or blueprint status.
- [ ] Model raw serialized FormIDs separately from resolved semantic FormKeys (origin/master/local identity).
- [ ] Resolve masters using their actual type and ordering; **fail explicitly** when required masters are absent.
- [ ] Support new form allocation, new master dependencies, override semantics and collision/duplicate detection.
- [ ] Correctly re-encode FormIDs on output when a master is added/reordered or a new record is introduced.
- [ ] Provide a referential-integrity scanner across typed FormIDs and known embedded VM/script or other references.
- [ ] Report unresolved references without silently converting them into zero, player records or unrelated forms.
- [ ] Freeze scale/identity for any plugin released as a dependency. Changing scale later is a compatibility break.
- [ ] Validate game version, DLC masters and creation dependencies; test current official-master and `BlueprintShips-*` behavior before assuming vanilla load-order semantics.
- [ ] Avoid forcing a runtime master graph for read-only raw inspection when no semantic resolution is requested.

## A3. Schema and code-generation capability [P0]

- [ ] Add a schema support matrix distinguishing `read-only`, `opaque roundtrip`, `typed read`, `typed write`, `new-record authoring`, and `CK/game validated`.
- [ ] Cite Starfield-specific xEdit `wbDefinitionsSF1.pas` and applicable `wbDefinitionsCommon.pas` helpers with fixed commit, source line span, and confidence; allow other explicitly reviewed helper sources when needed.
- [ ] Preserve unknown Starfield **base form component** blocks; test nested/repeated/optional components and field ordering.
- [ ] Implement condition bytecode/form-parameter and script properties only as supported and validated; do not transfer Skyrim interpretations blindly.
- [ ] Provide declarative field-level constraints, enum masks, optionality, alignment, count/length dependencies and cross-record typed references.
- [ ] Emit meaningful diagnostics for missing parent group, required field, record flag, broken link and unsupported semantic modification.
- [ ] Build a structural diff command and an optional `CK export → inspect → edit → export → game test` trace manifest.
- [ ] Ensure schema evolution does not modify no-op output when new fields are recognized.

## A4. Local creation, compilation and deployment [P0]

- [ ] Pin Starfield executable, DLC versions, CK build, xEdit revision, KitKraft revision and source fixtures in an environment manifest.
- [ ] Establish a scripted Creation Kit *golden-output* workflow (manual GUI operation is acceptable at first; automate only reproducible steps).
- [ ] Distinguish working `.esp` from published `.esm` workflow; prove generated/edited output opens in CK and works when loaded in game.
- [ ] Build an isolated game data deployment/uninstall procedure; never overwrite original vanilla masters or source assets.
- [ ] Support optional source `.psc` compilation to `.pex` with the actual installed **Starfield** Papyrus compiler/imports/flags (test CK installation paths; do not assume Skyrim layout).
- [ ] Package assets and scripts as needed in valid Starfield BA2 archives, while allowing loose-file iteration; test both.
- [ ] Emit a packaging manifest listing every generated form, script, mesh, texture, dependency and external asset license.
- [ ] Compare actual CK-generated outputs against KitKraft output for each newly supported editing capability.

---

# B. World, POI and site identity

## B1. World-space addressing and placement [P0]

- [ ] Typed read/write support for **`LCTN`, `CELL`, `WRLD`, `REFR`** and required placed-object group structure; no unsupported assumptions about cell children.
- [ ] Represent exterior and interior cells, parent worldspace/location, transforms, enable/disable state, editor IDs and aliases.
- [ ] Create new references to existing static meshes, activators, markers, containers, doors and furniture.
- [ ] Model persistent versus temporary references and their game/CK semantics with explicit validation.
- [ ] Link fast-travel markers, landing markers, map markers, doors, linked refs and locations without dangling links.
- [ ] Support existing-site **additions** without inadvertently changing a globally reused POI template.
- [ ] Create one standalone authored planetary overlay/site using CK output as golden fixture.
- [ ] Verify worldspace and cell changes by entering/exiting the location in game, save/reload, revisiting, and a clean uninstall/reinstall in test profiles.

## B2. Procedural generation/planetary content [P0 research, P2 implementation] [EXP]

- [ ] Investigate **`PNDT`, `PCMT`, `PCBN`, `PCCN`, `PKIN`, `LVLP`** and related records/conditions used by the Planet Content Manager. Do not presume all PCM logic is expressible solely through these records.
- [ ] Distinguish a fixed authored overlay, a radiant POI template, a procedurally chosen POI *instance*, and a planetary trait location.
- [ ] Determine whether instance identifiers and cell/worldspace mappings are stable across saves, visits, time advancement, game updates, and platform sessions.
- [ ] Test whether a claimed procedurally placed POI can be pinned without globally pinning every instance of its template.
- [ ] Identify and exclude quest-bound POIs, quest aliases, unique faction headquarters, enemy-spawn dependencies, mission targets, and repeatable radiant targets.
- [ ] Test the Planet Content Manager behavior when an instance becomes player-owned: replacement, deallocation, respawn, planet scanning, map markers, landing target and concurrent quest allocation.
- [ ] Design a curated eligibility registry first: site type, template ID, instance key, exclusions, structural constraints, ownership state, version.
- [ ] Produce a reproducible **instance identity/reset experiment** with at least two visually identical POIs in different locations; prove no conflation.
- [ ] **Gate arbitrary POI claiming** behind positive instance-identity, non-reset and quest-interference evidence. Failed experiment → authored-site-only release; do not fake persistence.

---

# C. Construction, ownership and restoration

## C1. Build-zone and workshop integration [P0]

- [ ] Author **`ACTI`, `KYWD`, `PKIN`, `REFR`, `LCTN`** and related link/property data for a claim terminal or outpost beacon.
- [ ] Reproduce the CK workflow that associates a **`LocTypeOutpost`** location, an outpost-beacon-style activator, its map marker, linked references and a valid build radius.
- [ ] Prove a building menu launches at the authored hangar and objects actually place, move, connect, survive reload and display correct ownership.
- [ ] Investigate native outpost-builder versus *interior decorator* functionality; use separate controllers if necessary. Do not assume identical item/menu support.
- [ ] Keep a single Foothold site record coordinating its exterior workshop and any interior decorators; ensure the site survives either controller being unloaded.
- [ ] Verify building restrictions, build radius, collision, snap rules, clearance, terrain slope, no-build markers, doors, player/ship landing interference.
- [ ] Confirm any overlapping/adjacent vanilla outpost cannot double-register or conflict with claimed POI.
- [ ] Test generator → power consumer, extractor → storage, storage → fabricator, cargo → storage and constructible snapping at reclaimed sites.
- [ ] Wire `OnWorkshopObjectPlaced/Moved/Removed` and related events only where required; avoid indefinite heavy polling.
- [ ] Test crew stations and assignment **separately** from basic construction: published POI builder examples do not guarantee working crew support.
- [ ] Preserve vanilla outpost build limits, skills, recipes and Free Lanes' shared-outpost-container behavior unless deliberately overridden.
- [ ] Show a useful diagnostic when build mode exists but the underlying workshop registration/resource graph is incomplete.

## C2. Securing and claiming [P0]

- [ ] Formalize site lifecycle: `unknown → discovered → hostile/contested → secured → eligible → claimed → restoring → operational` plus `abandoned/lost` if needed.
- [ ] Define *eligibility* independently from *hostiles cleared*: an active quest target, inhabited civilian facility or repeatable mission node should not silently become claimable.
- [ ] Prove one claim interaction with a terminal/activator and confirmation UX.
- [ ] Implement a scripted claim controller with persistent site state and idempotent activation (two activations cannot grant two claims).
- [ ] Support ownership effects at reference, cell and/or location scope as observed in testing; do not assume setting one owner updates all theft, trespass or AI behaviors.
- [ ] Update faction hostility/trespass/stealing rules only for appropriate objects and actors; don't accidentally grant ownership of unclaimed NPC inventories.
- [ ] Handle unresolved combat, enemy respawns, remaining witnesses, turrets, robotic guards, traps and still-active quest aliases.
- [ ] Introduce claim costs/clearance levels only after the basic successful claim path works.
- [ ] Reconcile with vanilla player outpost limits, maximum count, map display, transfer rights, fast-travel markers and ship landing locations.
- [ ] Test claims made under different faction allegiance states and character progression on *fresh and existing* saves.

## C3. Restoration, cleanup and fixed equipment [P0]

- [ ] Define restoration tiers per site: `secured`, `utilities online`, `habitable`, `production online`, `fully restored`.
- [ ] Use deliberate enable/disable groups, stages or switched references to clear debris, repair doors, turn on lighting, restore terminals and machinery.
- [ ] Preserve site identity: *repair the existing hangar*, not replace it with a visually unrelated outpost kit.
- [ ] Identify references that must never be scrappable: structural shell, quest props, pathing markers, workshop controllers, linked references, triggers and zone boundaries.
- [ ] Test activation and interaction of restored doors, elevators, workbenches, containers and machinery in each restoration stage.
- [ ] Verify collision, navmesh, cover, line of sight, occlusion, path transitions, interior/exterior load doors and lighting after changes.
- [ ] Add a clear salvage/scrap whitelist and clean rollback semantics; forbid deleting persistent quest references.
- [ ] Store restoration state separately from player construction placements so repair stage changes do not remove built furniture.
- [ ] Test staged visuals after save/load, cell unload/reload, long absence, conflict with a second mod and a scripted update.
- [ ] Make dismantling/abandoning a claimed site an explicit player action with a safe inventory recovery workflow.

---

# D. Site-specific resources, manufacturing and economy

## D1. Planetary trait, biology and geology [P1]

- [ ] Identify the CK records/references/conditions that recognize *a particular* mineral formation, fungal colony or surveyed trait. **Investigate** trait-marker and survey-state binding; don't infer it from `PNDT` alone.
- [ ] Define a trait-site descriptor: site ID, trait class, source form(s), allowed extractor(s), conditions, discovery requirements, yield/regen rules and environmental constraints.
- [ ] Implement one authored **fungal** or **crystal** site with a unique extractor and one new item; no random planetary integration yet.
- [ ] Ensure the extractor must be on or linked to the *specific site*; scanning an unrelated copy of the trait must not unlock production anywhere on the planet.
- [ ] Author relevant **`FLOR`, `IRES`, `MISC`, `GLOB`, `AVIF`, `KYWD`** and reference types only when demonstrated necessary for that resource model.
- [ ] Define renewable vs finite vs anomalous source semantics. Each needs its own harvesting model and UI feedback.
- [ ] Prove harvest controls: no duplicates on load, no production from unloaded or unfunded generators, no negative inventory, no excess resource beyond storage cap.
- [ ] Investigate engine resource production behavior while the site is unloaded, especially where documented Papyrus resource functions are limited by loaded state.
- [ ] Verify survey/trait discoverability, environmental hazards, planet constraints and case-specific surface attachment.
- [ ] Create a feedback loop for overharvesting or contamination **only after** production and persistence are solid.

## D2. Research, recipes, manufacturing and storage [P1]

- [ ] Author **`COBJ`** recipes and required keyword/category/condition references; compare outputs against CK-built recipes.
- [ ] Author **`RSPJ`** research with prerequisites, skills, input materials, unlocks and localization when appropriate.
- [ ] Connect new raw resources to at least one usable crafted product; verify crafting consumes inputs and produces correct items.
- [ ] Create an extractor/processor requiring power, and wire output to vanilla storage. Make container and resource ownership semantics explicit.
- [ ] Verify links among power, extractor, warehouse, fabricator, cargo and cross-outpost storage after cell unload/reload.
- [ ] Respect Free Lanes' **shared outpost container**; don't reimplement global storage unless it cannot meet the specific intended behavior.
- [ ] Define production rate, operational staffing bonus, power draw, maintenance consumption, raw/processed value and selling/buying balance as data.
- [ ] Prove time advancement cannot trivially produce infinite value via waiting, repeated reactivation, save/load or fast-travel cycles.
- [ ] Test item value and availability at vendors only if Foothold adds sale contracts or a legitimate buyer; money does not magically appear from extracting resources.
- [ ] Build a traceable production audit: `site → producer → input state → elapsed ticks → output → destination`, with safe reconnect/resume behavior.

## D3. Economic and environmental gameplay [P1–P2]

- [ ] Define at least one repeatable **contract** to export the new resource with a visible payout and delivery endpoint.
- [ ] Add a reason to develop the site: improved yield, research unlock, faction contract, specialist placement, or restoration capability.
- [ ] Balance opportunity cost against ordinary scavenging, trading and vanilla outposts.
- [ ] Decide how contracts react to NG+, faction allegiance, criminal possession, cargo loss and vendor resets.
- [ ] [P2] Add logistics chains, supply shortages, automated jobs or corporate competition only if the initial loop is fun during playtesting.
- [ ] [P2] Expose conservation-versus-exploitation choices with consequences and recovery times, but never punish players through opaque, irreversible disappearance of a site.

---

# E. Staff, quests, factions, and player experience

## E1. NPC/crew and navigation [P1]

- [ ] Author **`NPC_`, `ACHR`, `PACK`, `FACT`, `FURN`, `NAVM`** and relevant linked marker/reference data, with independent write and runtime validation.
- [ ] Show one hired worker successfully assigned to the reclaimed site (not merely present).
- [ ] Verify crew station count, work assignments, beds, home location, ownership, menus, outfit/equipment, schedules and sleep cycles.
- [ ] Test cross-cell pathing, exterior ↔ interior doors, catwalks, stairs, gravity variation and restored-stage collision.
- [ ] Test construction after NPC navmesh bake; record which changes are dynamically obstacle-aware versus require CK-authored navmesh edits.
- [ ] Verify idle behavior, workbench interaction, guarding, attacks, death, dismissal, unassignment, travel and save/load recovery.
- [ ] Decide whether hired crew are *ship crew*, *vanilla outpost crew*, or *Foothold colonists*. Avoid silently merging unrelated assignment systems.
- [ ] Ensure NPC sandbox packages do not send workers into unsafe or unloaded spaces.
- [ ] Test worker effects on production at unloaded sites before advertising offline-work bonuses.
- [ ] [P2] Add defense raids or faction reclamation attempts only after NPC routing and ownership work reliably.

## E2. Quests, dialogue and player choice [P1]

- [ ] Author **`QUST`, `SCEN`, `DIAL`, `INFO`**, conditions, aliases and script-fragment data as required by *one small quest*.
- [ ] Produce a simple quest: investigate abandoned site → remove hostiles → inspect claim terminal → pay/secure title → claim → restore generator → collect first product.
- [ ] Cover fail/cancel/alternate outcomes, re-entrancy, refusal, wrong stage, NPC death, quest completion outside expected order and already-claimed state.
- [ ] Avoid overriding a vanilla quest or globally rewriting POI actors; add Foothold-specific content/aliases wherever possible.
- [ ] Create a minimal *faction-sensitive* response (UC/Freestar/Fleet or equivalent) once state is available. Check the actual Starfield condition and actor flag surfaces in CK.
- [ ] Store choices and consequences in one authoritative quest/site state; do not maintain contradictory script/quest/global copies.
- [ ] Test quest journal, stage updates, markers, objective completion, quest aliases, dialogue availability and save/load mid-scene.
- [ ] Keep text and voiced dialog independent; release text-first if actor voice production or permissions are not ready.
- [ ] [P2] Add repeatable contract quests and encounter consequences after the first authored story works.

## E3. User interface and progression [P1]

- [ ] Present site eligibility and claim consequences through vanilla terminal/dialogue affordances first.
- [ ] Show site status, repair jobs, resources, production rates, crew and connections without demanding a custom native HUD.
- [ ] Integrate with Starfield's **2026 Database** and map surfaces only after inspecting actual extension points; otherwise ship a quest/journal or terminal-based ledger.
- [ ] Ensure map/fast-travel links do not duplicate or hijack another mod's marker.
- [ ] Display location ownership and contract state accurately after rapid travel, quest updates and NG+.
- [ ] Localize all displayed names, instructions, terminal entries and quest strings; handle missing strings and fallback text.
- [ ] Keep accessibility in mind: readable states, text feedback, distinguishable status without color-only cues, minimal menu churn.
- [ ] [P2] Evaluate a purpose-built Foothold site-management UI only if vanilla terminal/menu solutions fail user testing.

---

# F. Papyrus and engine extension boundary

## F1. Papyrus end-to-end [P0]

- [ ] Build `.psc` → Starfield `.pex` with the installed, matching CK Papyrus compiler, imports and flags; retain source in repository.
- [ ] Attach scripts and typed properties to new refs, locations and quest aliases through `VMAD`, including fragments where required.
- [ ] Verify script compilation **and** property binding **and** runtime event invocation independently.
- [ ] Implement site registry, claim state, controller discovery and restoration stage as idempotent state transitions.
- [ ] Register events minimally; honor unload/restore conditions and prevent duplicate registration/multiple simultaneous updates.
- [ ] Observe `StartWorkshop`, `GetWorkshop`, `OnWorkshopObjectPlaced/Moved/Removed`, `OnWorkshopOutputLink`, `OnWorkshopNPCTransfer` and relevant location/scan events in a sandbox game.
- [ ] Test `Location.SetFactionOwner`, `Cell.SetFactionOwner`, reference ownership, `Location.SetNeverResets` and `ObjectReference.SetPersistLoc` as **individual behaviors**, not magical combined persistence guarantees.
- [ ] Build recovery if an activator, workshop controller, quest alias, crew member or script property is missing on a save.
- [ ] Enforce no busy waiting or unbounded polling; log expensive loops, worker counts and event bursts.
- [ ] Preserve a stable serialized state schema for version migration and reinitialization.
- [ ] Provide runtime diagnostics: site ID, record IDs, controller registration, ownership flags, workshop link health and resource tick histories.

## F2. SFSE/native boundary [P2 unless an experiment proves otherwise] [EXP]

- [ ] Maintain an **engine limitation register**: experiment, exact API/CK behavior, game version, workaround attempted, observed failure, native hook rationale.
- [ ] Restrict native hooking proposals to a defined blocker: e.g., stable generated-instance identity, protected reset handling, missing workshop registration or missing UI API.
- [ ] If required, evaluate SFSE/CommonLibSF and their actual redistribution/license constraints separately from MPL/GPL implementation references.
- [ ] Version-gate native hooks against Starfield executable, platform and SFSE builds; fail visibly and safely after updates.
- [ ] Separate portable CK/Papyrus modules from optional SFSE-enhanced functionality so a console-compatible subset remains possible.
- [ ] Never ship native binary patches, proprietary game binaries, voice models, or game assets solely because the mod is noncommercial.

---

# G. Asset production and packaging

## G1. Visual, sound and localization assets [P1–P2]

- [ ] Decide whether initial sites use vanilla mesh kits, materials and textures (recommended) or new content; document what can be referenced versus redistributed.
- [ ] Support relevant **`STAT`, `MSTT`, `DOOR`, `CONT`, `FURN`, `LIGH`, `ACTI`, `TERM`, `PKIN`, `TXST`** records and asset links required by authored sites.
- [ ] Test new models, textures, collision and material paths in game and CK; preserve outpost snapping and repair variants.
- [ ] Package Starfield-valid `.ba2` archives and correctly named asset paths; verify packed behavior after deleting loose development files.
- [ ] Add ambient sound and machinery audio only after physical interactions work; use licensed or original assets.
- [ ] Keep custom text, VO, SFX, localization and other binary assets traceable in an asset manifest.
- [ ] For any voice synthesis, use voices with permission; do not silently clone game actors' performances.
- [ ] Create thumbnails, map images, buildable object previews and LOD only where required by the actual user workflow.

## G2. Compatibility and update safety [P0]

- [ ] Keep plugin ownership stable and avoid altering full/medium/small scale after release.
- [ ] Provide load-order and master audits covering Starfield, required DLC, Terran Armada / current content and the 2026 update state.
- [ ] Compare claimed POI changes against popular outpost-builder, POI-spawning, crew/settler and resource mods; identify overlapping record and behavioral ownership.
- [ ] Test both fresh games and mid-playthrough installs; define unsupported transitions explicitly rather than corrupting saves.
- [ ] Test update paths with `site registry v1 → v2` migration; if migration is unsafe, present an actionable error, not a silent reset.
- [ ] Make disabling Foothold while owning claimed sites an explicitly characterized risk; document whether safe uninstall is supported.
- [ ] Build one-command release artifacts with plugin, BA2, scripts, localization, README, dependencies and checksum manifest.
- [ ] Maintain a release notes / compatibility database keyed to exact game build and mod revision.

---

# H. Formal validation and quality gates

## H1. KitKraft core tests [P0]

- [ ] Differential parse/no-op-write against original golden CK output; use structural diff and byte hash.
- [ ] Generate a new reference + linked location + activator; CK loads the plugin without warnings.
- [ ] Generate a script-attached object; inspect VMAD/properties in a second tool; runtime activation fires exactly once.
- [ ] Ensure master-dependent FormIDs decode/encode correctly with mixed full/medium/small masters.
- [ ] Run fuzz/property tests for lengths, truncation, nesting, unknown payloads, compression and references.
- [ ] Demonstrate invalid input fails safely instead of producing a plausible-looking corrupt master.
- [ ] Verify output after re-export by the Creation Kit, not merely after a second KitKraft read.

## H2. Foothold gameplay regression matrix [P0/P1]

- [ ] Claim an eligible authored hangar after hostiles are cleared.
- [ ] Attempt claiming while hostile NPC or quest ownership remains; rejection is safe and explained.
- [ ] Attempt claiming another non-eligible or quest-critical POI; no mutation occurs.
- [ ] Build, move, remove and redecorate workshop objects, both exterior and interior.
- [ ] Use power, storage, repair machinery, crafting and a first site-specific resource producer.
- [ ] Save indoors and outdoors; load; fast travel; return after long travel and in-game waiting.
- [ ] Leave during a partial restoration stage; reload, finish it, and confirm original work isn't undone.
- [ ] Validate reference ownership, theft/trespass, NPC hostilities and cleared-site behavior after multiple reloads.
- [ ] Remove a mod-added machine and verify no orphaned producer updates or ghost outputs.
- [ ] Run two instances of the same template in a lab experiment; site identities do not cross-contaminate.
- [ ] Test partial power, full storage, broken cargo link, unloaded site, out-of-range crew, damaged machine, missing resource site.
- [ ] Test crew station/worker assignment in exterior and interior locations; inspect pathing across load doors.
- [ ] Test odd-order quest actions, duplicate terminal activation, pending scripts and interrupted travel.
- [ ] Test new-game-plus behavior: **choose and document** whether claims reset, carry as metadata only or seed new content. Do not assume all save state survives Unity.
- [ ] Verify overlays/map markers and planet surveys remain stable after acquisition and game update.
- [ ] Compare gameplay logs against intended site-state progression, costs, resources and bounds.

## H3. Performance, stability and compatibility [P0 throughout]

- [ ] Record save-size delta, load time, update-loop activity and event count per claimed site.
- [ ] Set explicit limits on active references, timers, persisted sites, productions, crew and build pieces.
- [ ] Benchmark one, five and twenty claimed sites, including distant/unloaded ones. These are *test targets*, not a guaranteed supported limit.
- [ ] Run multi-hour soak with travel/save/reload at increasing numbers of sites; monitor script queues and reference leakage.
- [ ] Compare resource state with and without Free Lanes shared storage interactions.
- [ ] Test a clean install alongside likely conflicting mods and document the exact conflict behavior.
- [ ] Preserve save backups prior to experimental procedural-POI work; never test destructive engine hooks on the sole real playthrough.
- [ ] Require human gameplay sign-off for build ergonomics, immersion, useful decisions and stability—not just automated pass status.

---

# I. Required record-surface matrix (not a promise of complete schemas)

The following records exist in Starfield-specific xEdit definitions. Prioritize based on **what the feature actually writes**. A support claim must specify typed field coverage, references, emitter correctness and CK/game results. Starfield subrecord/VMAD/component handling is not equivalent to a Skyrim record with the same four-letter signature.

| Priority | Record families | Intended Foothold role | First proving test |
|---|---|---|---|
| **P0** | `TES4`, `GRUP`, compressed records, `XXXX`, master flags | Safe plugin container/dependency output | No-op roundtrip + new form + CK load |
| **P0** | `LCTN`, `CELL`, `WRLD`, `REFR` | Site identity, interior/exterior cells, placed controllers, markers | Add a terminal+marker to authored site |
| **P0** | `ACTI`, `KYWD`, `TERM`, `PKIN` | Beacon/claim terminal, categories, staged scene components | Activate terminal, open build mode |
| **P0** | Script-bearing `VMAD`, scripts/fragment references, conditions | Claim and restoration logic | Compile/bind/run state controller |
| **P0** | `FACT`, `GLOB` | Ownership and small state/eligibility controls | Claim changes expected faction behavior |
| **P1** | `STAT`, `MSTT`, `DOOR`, `CONT`, `FURN`, `LIGH` | Repairable installations, containers and furnishings | Restore a door/generator, build interior |
| **P1** | `COBJ`, `RSPJ`, `IRES`, `MISC`, `FLOR`, `AVIF` | Research, recipes, unique resources, flora interaction | New resource → craft one useful item |
| **P1** | `QUST`, `SCEN`, `DIAL`, `INFO` and quest aliases | Claim quest and roleplay consequences | Full quest with journal and dialogue |
| **P1** | `NPC_`, `ACHR`, `PACK`, `NAVM` | Crew, schedules, occupation, pathing | Worker routes to assigned station |
| **P1 research** | `PNDT`, `PCMT`, `PCBN`, `PCCN`, `LVLP`, `PKIN` | Planet traits, overlays, radiant POIs, content generation | Unique site vs repeated instance experiment |
| **P2** | `PERK`, `FLST`, `LVLI`, `LVLN`, `CURV`, `REGN`, environmental & audio forms | Progression, encounter tables, distribution and polish | Test only where a scoped feature needs them |

**Not just records:** Planet Content Manager algorithms, CK procedures, asset archives, Papyrus compiler/runtime, script-extender native interfaces, generated POI lifecycle, navmesh, UI capabilities and save behavior require their own tools and experiments.

---

# J. Directed discovery experiments (run before making implementation promises)

| Experiment | Controlled setup | Passing observation | Result if it fails |
|---|---|---|---|
| EXP-01 Authored-POI workshop | One new authored exterior hangar with `LocTypeOutpost`, marker and beacon | Place, power, move, reload; all objects persist | Fix authoring/link semantics, not container parser |
| EXP-02 Interior decorator | One connected interior with a decoration activator | Place furnishings, save/reload, enter/exit | Keep interior and exterior workshop scopes separate |
| EXP-03 Stable claim | Quest cleared, scripted claim terminal, owned location | Theft/trespass and ownership behave as intended through travel/save | Explicit per-cell/per-ref controls; don't assume location owner alone suffices |
| EXP-04 Restoration | Enable/disable groups across three repair stages | Door, light, junk removal, navmesh and stored construction survive reload | Re-author stage geometry/ref links |
| EXP-05 Site-dependent resource | New fungal/crystal node with script-conditioned producer | Only that node's linked machine makes resource; no duplicates | Use bespoke scripted production; do not pretend vanilla extractor supports it |
| EXP-06 Instance identity | Two generated locations using same POI template | Stable distinct IDs across saves/time and no cross-claim contamination | **Do not ship arbitrary procedural claims** |
| EXP-07 Reset protection | Claim then leave site far away/long duration | Site and ownership remain; mission spawns don't conflict | Restrict to authored/proven persistent sites |
| EXP-08 Crew/navigation | Crew station and interior/exterior navmesh | Assign worker, observe routine, reload and return | Ship without crew feature initially or provide dedicated interior worker logic |
| EXP-09 Free Lanes interoperability | Two native outposts and one reclaimed site | Shared storage semantics, map Database and vanilla cargo behave predictably | Integrate narrowly; no custom global inventory rewrite |
| EXP-10 Master-aware roundtrip | Plugin referencing full, medium and small masters | Correct identity and write references, validated by xEdit/CK | Block cross-plugin authoring until fixed |
| EXP-11 Packaging | Same authored feature loose vs BA2 packed | Identical runtime behavior | Fix manifest/path/version before release |
| EXP-12 Update/uninstall | Claim site, save, upgrade Foothold, optionally remove | Documented migration; no silent ownership/asset loss | Block update/migration or warn uninstall unsupported |

---

# K. Milestones with hard exit criteria

### Gate K0 — KitKraft generalized authoring foundation (parallel to ongoing M1)

- [ ] Successful `WEAP` edit is preserved as baseline regression.
- [ ] At least three distinct *nonweapon* record-family fixtures roundtrip, including a location/reference family.
- [ ] Create a brand-new multi-record mini-plugin with valid dependencies and no dangling refs.
- [ ] A scripted activator compiles, binds, loads, and fires in game.
- [ ] CK can load/re-export the output and differential comparison is clean.

### Gate F0 — **First Foothold** (P0 vertical slice; fixed authored hangar)

- [ ] Claim eligibility decision works and is guarded against active quest conflicts.
- [ ] Securing/claiming a **single authored installation** changes persistent site state.
- [ ] Exterior build zone works; interior decorative zone works or is explicitly deferred with tested reason.
- [ ] Restore a door or generator in at least two discrete states; site retains recognizable geometry.
- [ ] Move/save/reload/return does not lose player furniture, inventories, repaired state or ownership.
- [ ] Independent CK load and real gameplay acceptance are recorded.

### Gate F1 — **A Reason to Stay** (P1 core loop)

- [ ] Survey one hand-authored fungal or crystalline site; unlock site-specific industrial development.
- [ ] Build a powered producer; deliver output into supported storage.
- [ ] Consume product in research/crafting and complete one valuable contract.
- [ ] Assign and observe one resident/worker at a compatible reclaimed outpost.
- [ ] Play through a small narrative arc with at least two meaningful responses and one persistent consequence.
- [ ] Performance/persistence soak across multiple sites passes on current game build.

### Gate F2 — **The Frontier Opens** (P2, only if experiments pass)

- [ ] Claim a *proven stable* procedurally generated POI instance without affecting identical instances elsewhere.
- [ ] Avoid radiant quest, spawn and reset interference on all supported POI families.
- [ ] Add optional site specialization, faction consequences, economic contracts and settlement population.
- [ ] Validate compatibility/NG+/update policies across the expanded feature set.
- [ ] Ship with explicitly documented eligible locations and unsupported cases.

---

# L. Swarm parallelization and director instructions

**Create bounded research tickets, not one giant “implement outposts” ticket.** Suggested execution order:

1. **KitKraft parser/schema lane:** generic authoring and FormID graph; `LCTN/CELL/WRLD/REFR`, `ACTI/KYWD/TERM`, VMAD and placement primitives; CK fixtures for every typed edit.
2. **CK experiment lane:** EXP-01, EXP-02, EXP-03; author the canonical hangar manually, record steps, export reference plugin and create a small gold corpus.
3. **Papyrus/runtime lane:** source compiler, script binding, site registry and idempotent controller; validate in game, not just from disassembly.
4. **Persistence investigator:** EXP-06, EXP-07. Deliver hard go/no-go for random POI claims; do not block authored Foothold F0.
5. **Resources lane:** read planetary traits, authored fungal/crystal site, one producer/recipe; delay broad distribution until EXP-05 passes.
6. **NPC/navigation lane:** EXP-08 and `NAVM/PACK` authoring needs; can proceed in parallel with resource work.
7. **QA/oracle lane:** regression corpus, CK differential, multi-day gameplay soak, fresh/existing saves, build manifests, performance metrics.
8. **Director:** accept or reject each subfeature by tested capability and scope. Summarize dependencies/failed experiments in ≤40-line lane handoffs. **Do not accept “schema exists” as evidence that the feature works.**

**Reporting schema for every ticket:**

```yaml
id: FTH-EXP-01
priority: P0
owner: <agent-or-lane>
requires: [<ticket-ids>]
output: <source-files-and-fixture-ids>
source_provenance: [<url-commit-lines>]
validation:
  parse: <pass|fail|not-run>
  byte_roundtrip: <pass|fail|not-run>
  ck_open: <pass|fail|not-run>
  ck_reexport: <pass|fail|not-run>
  game_runtime: <pass|fail|not-run>
  save_reload: <pass|fail|not-run>
limits: [<known-unsupported-behavior>]
result: <proven|partial|blocked|hypothesis>
```

**Director-level feature status vocabulary:** `unresearched → fixture acquired → typed decode → typed write → CK validated → game validated → persistence validated → release-ready`. Never collapse these into one “done” flag.

---

# M. Sources, provenance and version warnings

Primary Starfield format and empirical references:

- [xEdit `wbDefinitionsSF1.pas`, Starfield record and PCM families](https://raw.githubusercontent.com/TES5Edit/TES5Edit/dev-4.1.6/Core/wbDefinitionsSF1.pas) — MPL 2.0, **pin to the approved SHA in KitKraft's ADR/source policy** rather than using a moving branch URL. Independent implementation reference, not Bethesda's normative specification.
- [Bethesda: Free Lanes update, April 7, 2026](https://bethesda.net/en-US/news/starfield-free-lanes-terran-armada) — shared outpost containers, in-game Database, more POIs, recent changes; don't reinvent replaced pain points.
- [Bethesda: Outposts, getting started](https://help.bethesda.net/app/answers/detail/a_id/61083/~/outposts---getting-started---starfield) — power, habitation, crew station, production and basic building behaviors.
- [Nexus: How to allow players to build on POIs](https://www.nexusmods.com/starfield/articles/638) — practical `LocTypeOutpost`, beacon, links, marker and noted crew limitations; test against current build.
- [Mesmacat: Creation Kit guide to outposts and POIs](https://steamcommunity.com/sharedfiles/filedetails/?id=3385012985) — planet overlays, exterior and interior builders, navmesh editing and testing. Sections are incomplete; not a normative spec.
- [Starfield Papyrus Index](https://papyrus.bellcube.dev/starfield/) — signatures for `StartWorkshop`, `GetWorkshop`, placement/output-link events, `Location.SetFactionOwner`, `SetNeverResets`, `SetPersistLoc`; **availability of an API ≠ proof that a use case works**. Site/documentation copyright and any Bethesda-provided source require separate reuse consideration.
- [PCA 2026 Starfield compiler installation guide](https://pca.kiyozz.com/download) — notes that the CK compiler is at `Tools/Papyrus Compiler` and describes extracting `Tools/ContentResources.zip`; verify against locally installed version rather than hardcoding any path.
- [Ortham: Starfield load order and master identity research](https://blog.ortham.net/posts/2024-06-28-load-order-in-starfield/) — master type changes FormID interpretation; historical runtime load order details must be rechecked against 2026.
- [LOOT: Changing plugin types in Starfield](https://loot.github.io/docs/help/changing-plugin-types-in-starfield/) — master scale, .esp → .esm export, dependency stability.
- [Mutagen](https://github.com/Mutagen-Modding/Mutagen), [Spriggit](https://github.com/Mutagen-Modding/Spriggit), [esplugin](https://github.com/Ortham/esplugin), [libloadorder](https://github.com/Ortham/libloadorder) — independent oracles / prior art; review actual licenses and obligations before copying or redistributing any code.

**Evidence rules:** Every claimed Starfield binary fact should cite the project's *approved pinned revision*, not just a changing branch link. Every behavioral claim needs an observed CK/game experiment on a recorded game build. Private research and format facts are not automatically copyrighted code; mechanically translating implementation expressions and redistributing third-party source/assets trigger different license questions. Test novel engine interactions rather than elevating anecdotes into universal guarantees.

---

# N. Explicit non-goals until after the first release

- **DEFER:** claiming *every* procedurally generated POI regardless of its reset, quest and instance rules.
- **DEFER:** a complete dynamic navmesh rebuild framework for arbitrary player-edited complex interiors.
- **DEFER:** replacing the entire vanilla outpost UI, inventory, storage, cargo or production simulation.
- **DEFER:** a new galaxy-wide macroeconomy with thousands of autonomous NPC settlements.
- **DEFER:** SFSE/native hooks without a reproducible blocked CK/Papyrus requirement.
- **DEFER:** full faction campaign/reputation redesign, voice acting, custom creature ecosystems and every possible POI type.
- **DEFER:** requiring PS5/Xbox support on day one; architect for a portable subset but test distribution platforms individually.

**Definition of success:** The player returns to a hangar they personally discovered, cleared and reclaimed. Their repaired lights turn on. The furnishings, workers and goods are exactly where they left them. A unique resource site supplies the tools or revenue that allow the hangar to grow. The game acknowledges that this place belongs to them. That is Foothold.
