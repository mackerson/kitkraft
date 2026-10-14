# ADR 0008: Runtime extensions (SFSE, native plugins, analogous frameworks)

- Status: **Accepted** (owner ruling 2026-10-14; decides `plan/execution-plan.md` M4.O3)
- Date: 2026-10-14
- Scope: policy only. No code, no crate and no build change follows from this ADR now; it
  sets the rules the first runtime component will be held to.
- Relates to: ADR 0001 (clean room; `Starfield.exe` is never disassembled, decompiled,
  debug-attached or memory-dumped), ADR 0002 (layering), ADR 0005 Amendment 3 (evidence
  grades), `docs/source-registry.md` (rows SFSE, COMMONLIB), dossier
  `docs/research/starfield-dossier-2026-10-14.md` section 6, ADR 0004 "Owner approval"
  (its six clarifications still bind)
- Supersedes: the M4.O3 default ("no"), and the M4 text that rested on it ("nothing in M4
  depends on them", "CommonLibSF is never read", "Not in M4: SFSE or CommonLibSF plugins")

## Context

M4.O3 asked whether SFSE-dependent features may be planned, with the default **no**:
KitKraft's output would be plugins, archives and Papyrus only. The owner's workstreams
(W3 workshop, W5 save persistence and procedural locations, W6 runtime event handling)
include capabilities that vanilla records and Papyrus may not be able to express. The
dossier (section 6) describes native runtime work as a separate pipeline from data mods,
with distinct distribution obligations and Steam-only runtime compatibility.

Facts on the owner's machine (2026-10-14, file version resources, grade A): `Starfield.exe`
1.16.244.0 (Steam); SFSE installed (`sfse_loader.exe` 1.1.1, `sfse_1_16_244.dll` 0.2.21);
the MO2 instance already carries SFSE-dependent mods (among them "Address Library for SFSE
Plugins").

## Decision (owner ruling 2026-10-14, recorded in the owner's meaning)

The M4.O3 default is **overturned**.

1. KitKraft explicitly supports SFSE, native runtime extensions, and the analogous
   frameworks of other Bethesda games.
2. KitKraft does not require everything to be achievable through vanilla plugin records
   and Papyrus. Runtime extensions are permitted when technically justified (defined
   below).
3. Core parsing, schema and authoring functionality stays independent from game-runtime
   extensions.
4. Optional runtime dependencies are isolated; supported game versions are tracked;
   platform compatibility is documented.
5. The foothold is PC-first.
6. Important gameplay capabilities are not compromised solely to avoid an SFSE dependency.

This is architectural policy. It does not schedule an implementation: the first runtime
component is planned only when a runtime-need record (§5) is accepted, and lands in M5 at
the earliest unless the owner moves it.

## Rules

### 1. Layering: the core never depends on a runtime extension

- Nothing under `crates/*` (L0..L5), `apps/*` or `xtask` depends on SFSE, CommonLibSF,
  Address Library, or any other runtime framework, by build dependency, link, FFI binding,
  vendored header or generated code. The core reads and writes files; it never runs inside
  the game.
- Runtime components live behind one boundary: a top-level `runtime/` directory, one
  folder per component, named `runtime/<game>/<framework>/<component>/` (for example
  `runtime/starfield/sfse/kitkraft-events/`). Each component has its own build (C++/CMake
  or a Rust `cdylib`), its own `LICENSE` or license note, and is not a member of the Cargo
  workspace unless rule 1.4 applies.
- A Rust crate that must know a framework's API (an FFI binding, a version table) is named
  `crates/sfse-*` (or `crates/<framework>-*`) and gets a new layer `RT` in `LAYER_TABLE`
  through an ADR 0002 amendment in the same commit: `RT` may depend on L0 crates only (for
  shared data definitions), and nothing in L0..L5, APP or TOOL may depend on `RT`.
  `cargo xtask layers` gains a ban on framework names below `RT` (`sfse`, `commonlib`,
  `f4se`, `skse`, `nvse`, `obse` by prefix) when the first such crate is added.
- The contract between authored content and a runtime component is data: plugin records,
  Papyrus calls to the component's native functions, and versioned configuration files.
  Packaging a built component is file copying: `package` treats its binary as an opaque
  file at its Data path (`SFSE\Plugins\...`), so no core crate links it.
- Content that uses a runtime component detects its absence and degrades with a stated
  message; a plugin never becomes unloadable, and a save never becomes unloadable, because
  the component is missing or the game version is unsupported.

### 2. Clean room is unchanged

ADR 0001 stands. KitKraft never disassembles, decompiles, debug-attaches to, or
memory-dumps `Starfield.exe` (or `CreationKit.exe`), and never points a reverse-engineering
tool at them. A runtime component uses only what a framework publishes: SFSE's plugin
interfaces, and addresses or IDs a framework or address library publishes for a game
version. Diagnosing a component inside the game is done through its own log files, not a
debugger attached to the game process; changing that is the owner's decision. No game
offsets are discovered by KitKraft.

### 3. Licenses

| Framework | Terms (`docs/source-registry.md`) | Rule |
| --------- | --------------------------------- | ---- |
| SFSE (<https://github.com/ianpatt/sfse>) | Only `sfse/PluginAPI.h` is expressly permissive; the rest is not broadly permissive; its readme restricts redistribution | A runtime component may include `PluginAPI.h` at a pinned SFSE revision, listed in `ATTRIBUTION.md` with its notice. Other SFSE files are read only to learn the API (never copied). **SFSE binaries are never bundled**: the user installs SFSE; a package names the SFSE version it needs |
| CommonLibSF (<https://github.com/libxse/commonlibsf>) | GPL-3.0-or-later with modding/linking exceptions | **Not read, not linked, until reviewed.** Before any component links it: (a) an amendment to this ADR quotes the exception terms verbatim from its `LICENSE`, by URL at a pinned commit; (b) the owner reviews them and decides the component's own license (owner decision 4 covers licensing); (c) the source-registry row is updated. Until then the row's status forbids reading its source |
| Address Library for SFSE Plugins | Not checked | Used only as the user-installed runtime dependency it is; its license is checked and a registry row added before a component depends on it |
| Frameworks for other games (SKSE64, F4SE, xNVSE, OBSE) | Not checked | Each gets a registry row and a license review before use; none is in scope while KitKraft targets Starfield only |

Rules that hold for every framework: a GPL framework's source is never read by an agent
session that also edits `crates/*`, so the core's clean-room provenance is not mixed with
it; the core's license (MIT OR Apache-2.0) does not change because a runtime component
exists; a component's license is decided per component.

### 4. Game-version tracking and platform

Runtime components bind to a game executable version. Every component keeps a row per
supported game version; the table below is the project-wide record, extended when a
component lands or the game updates.

| Game | Store / platform | Game version | Framework | Framework version | Version-independence | KitKraft component | Status |
| ---- | ---------------- | ------------ | --------- | ----------------- | -------------------- | ------------------ | ------ |
| Starfield | Steam, Windows x64 | 1.16.244.0 (owner's install, 2026-10-14) | SFSE | 0.2.21 (`sfse_1_16_244.dll`), loader 1.1.1 | Address Library installed in MO2 (version not recorded) | none | observed only |

Rules:

- **PC-first.** Runtime components target Windows x64 and the Steam build first. Other PC
  stores are supported only where the framework supports them, recorded per row. Consoles
  cannot load native extensions: content for them is data only (plugins, archives,
  Papyrus), and a feature that needs a runtime component is marked "PC only, needs
  <framework> <version>" in its recipe, its package manifest and its testplan.
- A game update that the framework does not yet support is reported by the component at
  load (log and in-game message), never silently ignored; the content still loads (rule
  1, last bullet).
- The data side stays version-tolerant: core crates keep reading plugins of any game
  version they understand, independent of this table.

### 5. What "technically justified" means

A runtime extension is justified when all of these hold, recorded as a **runtime-need
record** (in `docs/design/runtime-extensions.md`, created with the first record):

1. **Need.** A workstream deliverable the owner wants depends on the capability (name the
   W number and the deliverable).
2. **Data path tried.** The vanilla route was investigated with KitKraft's own evidence:
   the record types and members involved (`schema`, `query`, `refs`), the Papyrus events
   and functions available (`papyrus.events` over the base sources), and where needed a
   CK fixture or an in-game observation (`docs/game-observations.md`).
3. **Data path fails or compromises the feature.** The evidence shows the capability is
   absent, or reachable only with an important gameplay compromise (lost state, wrong
   behaviour, unacceptable latency or script load, a reset that cannot be prevented).
   Inconvenience alone (C++ is easier than Papyrus) is not a reason.
4. **Smallest surface.** The component exposes the one capability (a native function, an
   event forwarded to Papyrus, a co-save block), and everything else stays in data.
5. **Degradation and versions.** The record states what the content does without the
   component and which game and framework versions are covered (§4).

Examples from the owner's workstreams (hypotheses until the record's evidence exists):

- **W5, save persistence hooks.** If per-instance state for a procedurally generated
  location or a claimed outpost must survive resets and regeneration, and M4.H3
  (`docs/design/persistence.md`) plus the testplan's leave/save/reload/return steps show
  that persistent references, quest aliases, globals and script variables lose or cannot
  key that state, then a component that stores it in a co-save through the framework's
  serialization interface (if the pinned SFSE version provides one, read from
  `PluginAPI.h`) is justified.
- **W6, runtime event handling.** If a deliverable needs an event that `papyrus.events`
  shows is absent from the base sources (for example a game-system notification no
  script receives), and the only data route is polling on a timer with a measured cost or
  missed events, then a component that forwards that event to Papyrus as a custom event
  is justified.
- **W3, workshop.** If M4.E7 finds that the outpost build menu cannot be extended by
  records alone, a component that registers the entries is a candidate, after the record
  shows which part data cannot reach.
- **Not justified:** anything a record override, GMST, GLOB, leveled list or Papyrus
  script expresses (weapon stats, names, recipes); generating data the CK computes
  (navmesh: the CK generates it, M4.A4); reading or writing save files from the authoring
  tool (the core never does).

## Consequences

- `plan/execution-plan.md` M4.O3 is decided; the M4 matrix rows W3, W5 and W6 name this
  ADR instead of the old default. No M4 task builds a runtime component; an M4 deliverable
  that finds a data path insufficient writes a runtime-need record and is carried with it,
  not dropped.
- `docs/source-registry.md`: SFSE becomes a runtime-boundary source (`PluginAPI.h`
  permitted at a pinned revision, binaries never bundled); CommonLibSF stays unread and
  unlinked until the §3 review.
- The first runtime component brings, in its own commits: the `runtime/` folder or an
  `RT` crate (ADR 0002 amendment, `LAYER_TABLE`, AGENTS.md), its license file, its row in
  §4, and its runtime-need record.
- ROADMAP M5+: runtime extensions where a runtime-need record justifies them.
