# AGENTS.md

Operating manual for every agent (and human) working in this repo. Read it in full before your
first edit. `CLAUDE.md` points here.

## What this is

KitKraft is an open-source, clean-room Creation Kit for Starfield, written in Rust.

Naming note (owner, 2026-10-14): the name is **KitKraft**; write it so in prose.
Identifiers were renamed (M4.BX1 integration): packages `kitkraft-*`, crate paths
`kitkraft_*`, env vars `KITKRAFT_*`, MCP server `kitkraft`, resource URIs `kitkraft://`;
the binary stays `kit`. The old `KITCRAFT_*` env vars and `kitcraft-manifest.json` are
read as a deprecated fallback for one release (`docs/corpus.md`, "Environment"); write
only the new names. It reads and writes Starfield plugins (`.esm`/`.esp`/`.esl`), archives (`.ba2`), compiled
Papyrus (`.pex`) and string tables, and exposes every action as one command reachable from a
CLI, an MCP server, and later a UI. The first provable goal (M1) is data parity: load every
official ESM and write it back byte-identical.

## Start every session

1. Read `plan/STATUS.md` (where we are), then `plan/execution-plan.md` (what is next and who
   owns it).
2. Read `docs/architecture.md` if you will touch more than one crate.
3. Check `docs/gaps.md` before claiming something works.
4. Pick up the first unblocked task you own. Don't stop to ask unless the decision is listed
   as the owner's (see "Owner decisions" below). Otherwise decide, record why in the commit
   message or an ADR, and keep going.

## Layers

The workspace is a strict layer stack. A crate may depend only on crates in a lower layer.
`cargo xtask layers` enforces this from each crate's `Cargo.toml` and its `//! LAYER: Ln`
marker in `src/lib.rs` (or `src/main.rs`), from a scan of its sources, and from the graph
`cargo metadata` resolves (so transitive and dev-dependencies count too).

| Layer | Crate            | Package            | Purpose                                                            |
| ----- | ---------------- | ------------------ | ------------------------------------------------------------------ |
| L0    | `crates/schema`  | `kitkraft-schema`  | Record definitions as data (TOML), codegen to typed records        |
| L0    | `crates/esm`     | `kitkraft-esm`     | TES4 container: groups, records, subrecords, compression, ONAM     |
| L0    | `crates/ba2`     | `kitkraft-ba2`     | GNRL v2/v3 + DX10 read/write (port of `ba2x.py`)                   |
| L0    | `crates/pex`     | `kitkraft-pex`     | Papyrus `.pex` read/disasm (port of `pexdis.py`), assemble later   |
| L0    | `crates/strings` | `kitkraft-strings` | `.STRINGS` / `.DLSTRINGS` / `.ILSTRINGS`                           |
| L0    | `crates/nif`     | `kitkraft-nif`     | Read-only mesh/material metadata, no renderer                      |
| L1    | `crates/doc`     | `kitkraft-doc`     | Load order, FormID resolution across masters, overrides, conflicts |
| L2    | `crates/render`  | `kitkraft-render`  | Viewport (later, not created yet)                                  |
| L3    | `crates/io`      | `kitkraft-io`      | MO2 profile reader, `plugins.txt`, Creations manifests             |
| L4    | `crates/engine`  | `kitkraft-engine`  | `CommandSpec` catalog, sessions, undo, validation rules            |
| L5    | `crates/mcp`     | `kitkraft-mcp`     | MCP server: `server.rs`, `tools.rs`, `backend.rs`                  |
| L5    | `crates/ui-egui` | `kitkraft-ui-egui` | Read-only workbench (ADR 0007 Am. 1); swappable; engine only       |
| APP   | `apps/kit-cli`   | `kit-cli`          | `kit` binary, one `main.rs`                                        |
| APP   | `apps/kit`       | `kit`              | `kit-ui` binary: the workbench window (`kit ui` starts it)         |
| TOOL  | `xtask`          | `xtask`            | `cargo xtask ci`, `cargo xtask layers`                             |

Rules:

- Depend downward only. Same-layer edges are illegal unless listed in `SAME_LAYER_ALLOWED` in
  `xtask/src/main.rs` with a reason in `plan/adr/0002-layering.md`. Today the only one is
  `esm -> schema`.
- Nothing depends on an APP or TOOL package, dev-dependencies included.
- `kitkraft-ui-egui` depends on `kitkraft-engine` alone among workspace crates, dev-dependencies
  included (`INTERNAL_ONLY` in `xtask/src/graph.rs`), and reaches the engine only through its
  read-only gate (`crates/ui-egui/src/gate.rs`, a test scans the crate's sources).
- Nothing below L5 depends on a UI toolkit, directly, transitively, or as a dev-dependency.
  Banned by name prefix: `egui`, `eframe`, `winit`, `wgpu`, `rfd`, `iced`, `slint`.
- Library crates (L0..L5) use `thiserror`; `anyhow` and `eyre` (by prefix, transitively) are
  for `apps/` and `xtask/` only.
- Dev-dependencies between workspace crates follow the same direction rules; an upward one
  needs an entry in `DEV_ALLOWED` (`xtask/src/graph.rs`, empty today) and a reason in ADR 0002.
- No `build.rs` (`BUILD_RS_ALLOWED` is empty). No `#[path = ...]`, `include!`,
  `include_str!` or `include_bytes!` whose path leaves the package directory; use a string
  literal or `concat!(env!("CARGO_MANIFEST_DIR"), "...")` so the check can resolve it.
- A new crate needs an entry in `LAYER_TABLE` (xtask) and in the table above. Moving a crate
  between layers needs an ADR.

## Everything is a command

Every user-visible action is exactly one `CommandSpec` in `kitkraft_engine::catalog()`:

| Field         | Meaning                                                                     |
| ------------- | --------------------------------------------------------------------------- |
| `id`          | Stable dotted id: `inspect`, `ba2.extract`. CLI spells it `kit ba2 extract` |
| `label`       | Human label for menus and the command palette                               |
| `menu_path`   | `/`-separated menu location in the future UI                                |
| `shortcut`    | Default key binding, or none                                                |
| `aliases`     | Other names accepted by the CLI, `command_line`, and the palette            |
| `summary`     | One line for `kit help` and MCP tool listings                               |
| `args_schema` | JSON Schema for the arguments object                                        |
| `writes`      | Can write a file or into MO2 with some arguments (MCP hints, workbench gate) |
| `status`      | `Implemented`, `Deferred { reason, milestone }`, or `Planned` (none at M2)  |

Execution takes the JSON arguments and returns a JSON result. **`execute` never opens a
dialog, prompts, or blocks on input.** Interactive front ends collect arguments first, then
call the same command. If an action cannot be expressed as a command with JSON arguments, it
is not designed yet.

Exception: front-end launchers (`kit mcp`, `kit ui`) start another front end over the
catalog instead of acting on data, so they are plain CLI subcommands and need no `CommandSpec`.
So is front-end setup (`kit install` / `kit uninstall`: the user PATH and the MCP
registration, `kitkraft_engine::install`); keeping it off the catalog keeps it off MCP, so
a connected agent cannot rewrite the PATH or its own registration. `kit doctor` is a
catalog command (read-only), and what it detects is every command's default
(argument > environment > detected; docs/QUICKSTART.md).

## CLI / MCP / UI parity

- `kit <command>` (with `--json`), the MCP `execute`/`command_line` tools, and UI menu items
  all resolve to the same `CommandSpec` and the same code path. No front end has logic the
  others lack.
- `apps/kit-cli` is one `main.rs`. `crates/mcp` is `server.rs` + `tools.rs` + `backend.rs`.
  If either grows format or editing logic, move it down into a library crate.
- Results carry evidence: `limitations[]`, `coverage`, and byte offsets (file, offset,
  length) for every value read from a plugin. Unknown is reported as unknown, never as
  absent. See `docs/mcp.md`.
- A new command lands with its CLI form, its MCP reachability, and a test that calls it
  through `execute` with JSON.

## Clean room

- **Sources follow the owner's four-level hierarchy** (ADR 0004, decided 2026-10-14; ADR 0005
  Amendment 2): (1) container: UESP's Skyrim pages, validated on the Starfield corpus;
  (2) record definitions: upstream xEdit `Core/wbDefinitionsSF1.pas` and the helpers it
  uses in `Core/wbDefinitionsCommon.pas`, both pinned at the same SHA
  `9fb016884bec138ea6c7b872cec831537d464c3e` (`docs/xedit-reference.md`); (3) empirical
  validation: corpus statistics, CK differential observation, differential comparison
  against SF1Edit; (4) secondary: UESP's Starfield pages on starfieldwiki.net (thin; cite as
  `uesp-starfield:`; `/Raw_Data` only cross-checks our corpus inventory; for `/CTDA` cite
  xEdit instead), the CK wiki, community docs. Our own hex dumps of the official ESMs are
  the tiebreaker. Record facts in `crates/schema` definitions with their `source`,
  `evidence` and `confidence` (`reference`, `corpus`, `ck-diff`, `verified`); never paste
  prose or code from any source, and never write a Starfield layout from memory.
- **xEdit's Pascal is source material only.** Read the pinned `wbDefinitionsSF1.pas` and
  `wbDefinitionsCommon.pas` from copies outside the repo (`*.pas` is gitignored; delete them
  after use), and write our TOML independently: no code or comments copied, no
  reproduction of xEdit's architecture, no runtime or build dependency on Pascal. Cite
  `xedit: wbDefinitionsSF1.pas@<sha>#L<a>-L<b>` (or `wbDefinitionsCommon.pas@<sha>...`),
  both at the same pinned SHA. A common-file helper is cited only for what a Starfield
  definition uses it for. Another upstream helper file needs only a documented dependency
  (`docs/xedit-reference.md`, `docs/source-registry.md`) under the same rules (ADR 0004,
  "Owner approval 2026-10-14").
- **Never use a Skyrim definition for a Starfield record body.** Skyrim facts describe the
  shared container only, with `confidence = "corpus"` or better.
- **CreationKit.exe may be run and observed as a black box only**: open plugins, save, take
  screenshots, diff its output against ours. Never disassemble, decompile, debug-attach,
  memory-dump, or point a reverse-engineering tool (Ghidra, IDA, REA, x64dbg) at it or at
  Starfield.exe. Never extract or ship its assets. Observations stay local in `observe/`
  (gitignored).
- **Never read Mutagen or Spriggit source**, or any other GPL/LGPL/AGPL code that handles
  Bethesda formats. Not to "check one thing", not via a search result snippet. If you have
  seen such code, say so and do not implement that area.
- Running Spriggit or SF1Edit as an external binary for comparisons is allowed; reading
  Spriggit's source (or using its output field names) is not, and neither tool is bundled.
- `C:\Modding\scripts` is our own code and may be ported freely.
- Every third-party asset (icon, font, sample file) is listed in `ATTRIBUTION.md` before it is
  committed. No Bethesda data in the repo, ever.

## Never panic

Plugins come from the internet. Treat every input byte as hostile.

- No `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, or `unsafe`
  outside tests. `[workspace.lints]` denies them; `unsafe_code` is forbidden outright.
- No slice indexing (`buf[i]`, `buf[a..b]`): use `get()` / `get(a..b)` and return an error.
- Checked arithmetic on every size and offset from input (`checked_add`, `checked_mul`,
  `usize::try_from`). Cap allocations by the bytes actually remaining before allocating.
  `clippy::arithmetic_side_effects` is denied workspace-wide. Clippy has no allow-in-tests
  setting for it, so a test module that needs plain `+`/`-` puts
  `#[allow(clippy::arithmetic_side_effects)]` on its `mod tests` (or `#![allow(...)]` at the
  top of an integration test file).
- Bound recursion depth (nested groups) and iteration counts.
- Errors are typed (`thiserror`) and carry the byte offset where parsing failed.
- **Every crash fix ships with a regression test** that feeds the minimized input that
  crashed.

## Testing and the CI gate

`cargo xtask ci` is the gate. It runs, in order:

1. `cargo xtask layers` (layer graph, markers, banned and transitive dependencies,
   dev-dependencies, build scripts, out-of-package `#[path]`/`include!`)
2. `cargo fmt --all -- --check`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace`

Nothing is committed unless the gate passes. A pre-commit hook in `.githooks/` runs the two
fast steps (`cargo xtask layers`, `cargo fmt --all -- --check`); enable it once per clone with
`git config core.hooksPath .githooks`. Corpus tests read the official ESMs in place via
`KITKRAFT_DATA_DIR` (see `docs/corpus.md`) and skip with a message when it is unset or
missing, so the gate runs on machines without the game. There is no remote CI yet; the local
gate is the CI.

## Commits

- Commit after every arc that builds and passes `cargo xtask ci`. Small, frequent commits.
- Local repo only, branch `main`, no remote yet. The remote is https://github.com/mackerson/kitkraft (private). Only the director pushes `main`, after an integration passes the gate; lanes never push, never force-push, and never push their worktree branches.
- Message: imperative subject naming the crate (`esm: preserve XXXX oversize prefix`), body
  says why. Update `plan/STATUS.md` in the same commit when a task finishes.

## Parallel agents

- Each agent builds into its own target dir: `CARGO_TARGET_DIR=C:\Modding\kitkraft\.targets\<agent>`
  (PowerShell: `$env:CARGO_TARGET_DIR = "C:\Modding\kitkraft\.targets\<agent>"`). Delete it
  when you finish. `.targets/` is gitignored.
- Only edit crates you own per `plan/execution-plan.md`. Need a change elsewhere? Note it in
  `plan/STATUS.md` under "Requests" and keep going on your own lane.
- Shared files (`Cargo.toml` workspace deps, `AGENTS.md`, `docs/`) take small, separate
  commits so they merge cleanly.
- Read `plan/STATUS.md`, then `plan/execution-plan.md`, before starting. Update STATUS when
  you finish a task.

## Owner decisions

Don't stop to ask unless the decision is one of these. Everything else: decide and record it.

1. Project name: **decided 2026-10-14**, KitKraft (naming note at the top; identifiers
   renamed in the M4.BX1 integration).
2. xEdit MPL source policy: **decided 2026-10-14** (`plan/adr/0004-xedit-source-policy.md`).
   Still the owner's: changing the pinned revision, or a source file with a materially
   different license or source category (`wbDefinitionsSF1.pas` and
   `wbDefinitionsCommon.pas` are approved at the same SHA).
3. Which video critique is the design brief for the gun overhaul.
4. License (proposed MIT OR Apache-2.0; scaffolded that way).
5. Whether and when to contact kinggath about Sim Settlements 2.
6. Adding a git remote or publishing anything.

Owner policies decided 2026-10-14 (follow them; they are not open):

- **Runtime extensions** (ADR 0008): SFSE and analogous frameworks are supported where
  technically justified, only behind `runtime/` (or an `RT` crate); nothing in `crates/*`,
  `apps/*` or `xtask` depends on them; CommonLibSF is not read or linked until ADR 0008's
  license review; SFSE binaries are never bundled.
- **Development deployment** (ADR 0009): `package` never writes into MO2; `kit deploy
  --dev` is the only MO2 write path, opt-in per call, into one `KitKraft Dev - <plugin>`
  mod, never into profiles, `plugins.txt`/`loadorder.txt` or another mod.
- **Developer workbench** (ADR 0007 Amendment 1): a read-only egui workbench over
  `Engine::execute` only (M4 lane W, after the inspection APIs are stable); no logic of its
  own; the full authoring UI is M5.

## Owner's live game is off limits (2026-10-14)

The owner has an active Starfield playthrough on the MO2 profile `Default`. Rules, binding
for every lane and every command:

- `Default` is read-only input for load-order resolution and never a test target. No agent
  enables, disables, copies or deploys a plugin into it, and `deploy --dev` (ADR 0009) must
  refuse it by name.
- In-game checks run only on the MO2 profile `KitKraft Dev` (a copy of `Default` with
  profile-specific saves) on disposable saves. Owner-run steps in STATUS name that profile.
- Plugins that own forms are never removed from a save in progress. Built artifacts for the
  overhaul stay override-only (no new forms, no leveled-list injection, no scripts) so they
  can be removed safely; anything that owns forms says so in its build report.
- Nothing writes under the game `Data` folder, `Documents\My Games\Starfield`, or
  `C:\Modding\MO2-Starfield` except `deploy --dev` into its own isolated mod folder, and
  (once ADR 0010 is accepted) the game itself while a `kit game session` runs it on the
  dev profile: session saves named `KitKraftSession-*` and the logs the game writes.

**Agent-driven sessions (ADR 0010, proposed 2026-10-15; binding when the owner rules on
M4.O6).** The sentence "the agent cannot launch the game" no longer holds for the dev
profile. An agent may launch Starfield **only** through the lane T session controller
(`kit game session`), which: refuses to start when MO2's selected profile is `Default` or
is not the dev profile, when `Starfield.exe`, `CreationKit.exe` or `ModOrganizer.exe` is
already running (the owner may be playing), when the dev profile lacks the owner-set INI
switches or the Console API plugin, or when the plugin under test was not installed with
`deploy --dev`; talks to the game only over the Console API plugin's loopback HTTP
endpoint, the files the game and its plugins write, and screenshots (never a debugger,
memory read or code injection: ADR 0001); loads the owner's named base save and saves
only to `KitKraftSession-*` names; writes only `observe\` and one entry in
`docs/game-observations.md`; terminates the game at the end and verifies the profile's
list files and INIs are unchanged. Probes with machine-checkable matchers are the verdict;
screenshots are evidence. The third-party plugins it relies on are run-only tools
(`docs/source-registry.md`), never read into `crates/*`. Owner sessions keep the last word
where the two disagree.

## Windows specifics

- Primary dev box is Windows 11. Paths in docs and tests use Windows form; code uses
  `std::path` and never hard-codes separators.
- Shell is PowerShell 5.1 (no `&&`; use `;` and `if ($?)`). Git Bash is also available.
- Starfield data: `C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data` (read only;
  never write there).
- Mod Organizer 2 (portable): `C:\Modding\MO2-Starfield`, profile
  `C:\Modding\MO2-Starfield\profiles\Default`. Read it; never modify the profile.
- Existing Python tools being ported: `C:\Modding\scripts` (`ba2x.py`, `pexdis.py`,
  `esmscan.py`, `celldoors.py`, `pexgrep.py`). They are the behavioural reference for the
  `ba2`, `pex`, and `doc` crates.
- `CreationKit.exe` is launched through MO2 (executable #4) for black-box observation only.
- Large files: `Starfield.esm` is about 1.46 GB. Never load the whole corpus into memory at
  once; stream by top-level group.

## Where things go

| Path                     | Contents                                                      |
| ------------------------ | ------------------------------------------------------------- |
| `plan/STATUS.md`         | Current milestone, done, next, blockers, requests             |
| `plan/execution-plan.md` | Tasks, owners, acceptance criteria, ordering                  |
| `plan/adr/`              | Decisions, numbered, never rewritten (supersede instead)      |
| `docs/architecture.md`   | Layers, data flow, key types                                  |
| `docs/mcp.md`            | MCP tool shapes and the evidence envelope                     |
| `docs/record-parity.md`  | Per-record schema and roundtrip coverage (generated after M1) |
| `docs/ck-parity.md`      | CK feature checklist from black-box observation               |
| `docs/gaps.md`           | Known gaps; check before claiming something works             |
| `docs/xedit-reference.md`| Pinned xEdit revision, license, coverage, Pascal-to-TOML map  |
| `docs/corpus.md`         | Roundtrip corpus: which files, where, how it is used          |
