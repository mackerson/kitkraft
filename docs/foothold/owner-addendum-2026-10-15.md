# Owner addendum, 2026-10-15: final M4 scope clarification

Owner's words, verbatim (chat, 2026-10-15), received with `docs/foothold/engineering-surface.md`.
Binding on the plan; `plan/adr/0011-foothold-benchmark.md` records the decision.

The previous KitKraft M4 decisions are approved, with these amendments:

## 1. Sim Settlements 2 / Foothold relationship

Correction: Sim Settlements 2 is the spiritual and mechanical predecessor of Foothold, not a
separate porting project we're planning. We are developing an original Starfield settlement
experience inspired by its autonomous settlements, living communities, production systems, and
narrative integration. Foothold's specific storyline and game design are being developed
independently of the current KitKraft engineering push.

## 2. KitKraft remains the deliverable

Foothold is a demanding integration benchmark for KitKraft's general-purpose authoring
capabilities, not a mod to finish during M4. Create whatever supporting infrastructure, reusable
gameplay systems, fixtures, and test mods are useful. Preserve these as portable development
artifacts for future Foothold refinement. Do not divert agents into Foothold narrative, balance,
asset polish, or content-production work.

## 3. Add hostile encounter orchestration to the capability backlog

Foothold will eventually support player-controlled or story-triggered attacks against
settlements, including Starborn encounters. KitKraft should support authoring and validating the
underlying mechanics:

* Configurable encounter triggers, actors, factions, spawn locations, and cooldowns.
* Quest/script-driven activation and deterministic test triggering.
* Combat behavior, settlement defenses, damage, and recovery.
* Attended versus unattended settlement behavior.
* Safe cancellation, cleanup, and save/reload persistence.
* Compatibility with vanilla outpost assaults and current DLC behavior.

Investigate existing Starborn/Armillary encounters and prior-art attack managers before
proposing new runtime hooks. A functioning generic hostile-encounter fixture is sufficient for
this development phase. Do not implement the full Foothold attack system.

## 4. Explicit player-agency requirement

Record as future Foothold design constraints: optional/configurable attacks, meaningful
defenses, reliable notifications, nonpunitive unattended resolution, and no arbitrary loss of
accumulated player investment.

## 5. Keep validation honest

CK fixture creation may initially be owner-assisted. Do not assume unattended CK operations
work until proven. Automated runtime experiments must retain distinct not-run, pass, fail, and
inconclusive statuses, with reproducible evidence. Maintain the installed-game-version and DLC
compatibility matrix. Test interactions with Free Lanes and Terran Armada outpost changes where
applicable.

Proceed with the existing CK oracle, Papyrus, runtime automation, world authoring, and
persistence priorities. These amendments do not authorize a new content-development swarm or
delay the current QA work.
