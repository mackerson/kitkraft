# AGENTS.md

Operating manual for every agent (and human) working in this repo. Read it in full before your
first edit. `CLAUDE.md` points here.

## What this is

KitCraft (working name) is an open-source, clean-room Creation Kit for Starfield, written in
Rust. It reads and writes Starfield plugins (`.esm`/`.esp`/`.esl`), archives (`.ba2`), compiled
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
marker in `src/lib.rs` (or `src/main.rs`).

| Layer | Crate            | Package            | Purpose                                                            |
| ----- | ---------------- | ------------------ | ------------------------------------------------------------------ |
| L0    | `crates/schema`  | `kitcraft-schema`  | Record definitions as data (TOML), codegen to typed records        |
| L0    | `crates/esm`     | `kitcraft-esm`     | TES4 container: groups, records, subrecords, compression, ONAM     |
| L0    | `crates/ba2`     | `kitcraft-ba2`     | GNRL v2/v3 + DX10 read/write (port of `ba2x.py`)                   |
| L0    | `crates/pex`     | `kitcraft-pex`     | Papyrus `.pex` read/disasm (port of `pexdis.py`), assemble later   |
| L0    | `crates/strings` | `kitcraft-strings` | `.STRINGS` / `.DLSTRINGS` / `.ILSTRINGS`                           |
| L0    | `crates/nif`     | `kitcraft-nif`     | Read-only mesh/material metadata, no renderer                      |
| L1    | `crates/doc`     | `kitcraft-doc`     | Load order, FormID resolution across masters, overrides, conflicts |
| L2    | `crates/render`  | `kitcraft-render`  | Viewport (later, not created yet)                                  |
| L3    | `crates/io`      | `kitcraft-io`      | MO2 profile reader, `plugins.txt`, Creations manifests             |
| L4    | `crates/engine`  | `kitcraft-engine`  | `CommandSpec` catalog, sessions, undo, validation rules            |
| L5    | `crates/mcp`     | `kitcraft-mcp`     | MCP server: `server.rs`, `tools.rs`, `backend.rs`                  |
| L5    | `crates/ui-egui` | `kitcraft-ui-egui` | UI (later, swappable, not created yet)                             |
| APP   | `apps/kit-cli`   | `kit-cli`          | `kit` binary, one `main.rs`                                        |
| APP   | `apps/kit`       | `kit`              | Desktop app (later, not created yet)                               |
| TOOL  | `xtask`          | `xtask`            | `cargo xtask ci`, `cargo xtask layers`                             |

Rules:

- Depend downward only. Same-layer edges are illegal unless listed in `SAME_LAYER_ALLOWED` in
  `xtask/src/main.rs` with a reason in `plan/adr/0002-layering.md`. Today the only one is
  `esm -> schema`.
- Nothing depends on an APP or TOOL package.
- Nothing below L5 depends on a UI toolkit (egui, eframe, winit).
- Library crates (L0..L5) use `thiserror`; `anyhow` is for `apps/` and `xtask/` only.
- A new crate needs an entry in `LAYER_TABLE` (xtask) and in the table above. Moving a crate
  between layers needs an ADR.

## Everything is a command

Every user-visible action is exactly one `CommandSpec` in `kitcraft_engine::catalog()`:

| Field         | Meaning                                                                     |
| ------------- | --------------------------------------------------------------------------- |
| `id`          | Stable dotted id: `inspect`, `ba2.extract`. CLI spells it `kit ba2 extract` |
| `label`       | Human label for menus and the command palette                               |
| `menu_path`   | `/`-separated menu location in the future UI                                |
| `shortcut`    | Default key binding, or none                                                |
| `aliases`     | Other names accepted by the CLI, `command_line`, and the palette            |
| `summary`     | One line for `kit help` and MCP tool listings                               |
| `args_schema` | JSON Schema for the arguments object                                        |
| `status`      | `Planned` or `Implemented`                                                  |

Execution takes the JSON arguments and returns a JSON result. **`execute` never opens a
dialog, prompts, or blocks on input.** Interactive front ends collect arguments first, then
call the same command. If an action cannot be expressed as a command with JSON arguments, it
is not designed yet.

Exception: front-end launchers (`kit mcp`, a future `kit ui`) start another front end over the
catalog instead of acting on data, so they are plain CLI subcommands and need no `CommandSpec`.

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

- **Every fact has one permitted source, listed in `plan/adr/0005-spec-sources-per-fact.md`.**
  UESP has no Starfield format page; its Skyrim pages cover only the shared container. Corpus
  statistics give the inventory, CK differential observation gives Starfield field layouts,
  and the Creation Kit wiki gives field names. Our own hex dumps of the official ESMs are the
  tiebreaker. Record facts (signatures, sizes, offsets, types) in `crates/schema` definitions
  with their ADR 0005 `source`; never paste UESP or wiki prose into code or docs, and never
  write a Starfield layout from memory.
- **CreationKit.exe may be run and observed as a black box only**: open plugins, save, take
  screenshots, diff its output against ours. Never disassemble, decompile, debug-attach,
  memory-dump, or point a reverse-engineering tool (Ghidra, IDA, REA, x64dbg) at it or at
  Starfield.exe. Never extract or ship its assets. Observations stay local in `observe/`
  (gitignored).
- **Never read Mutagen, Spriggit, or xEdit source**, or any other GPL/LGPL/AGPL code that
  handles Bethesda formats. Not to "check one thing", not via a search result snippet. If you
  have seen such code, say so and do not implement that area. xEdit's MPL source is also off
  limits until `plan/adr/0004-xedit-source-policy.md` is decided by the owner.
- Running Spriggit as an external binary (ESM <-> YAML) for oracle comparisons is allowed;
  reading its source is not.
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
- Bound recursion depth (nested groups) and iteration counts.
- Errors are typed (`thiserror`) and carry the byte offset where parsing failed.
- **Every crash fix ships with a regression test** that feeds the minimized input that
  crashed.

## Testing and the CI gate

`cargo xtask ci` is the gate. It runs, in order:

1. `cargo xtask layers` (layer graph, markers, banned dependencies)
2. `cargo fmt --all -- --check`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace`

Nothing is committed unless the gate passes. Corpus tests read the official ESMs in place via
`KITCRAFT_DATA_DIR` (see `docs/corpus.md`) and skip with a message when it is unset or
missing, so the gate runs on machines without the game. There is no remote CI yet; the local
gate is the CI.

## Commits

- Commit after every arc that builds and passes `cargo xtask ci`. Small, frequent commits.
- Local repo only, branch `main`, no remote yet. Do not add a remote or push.
- Message: imperative subject naming the crate (`esm: preserve XXXX oversize prefix`), body
  says why. Update `plan/STATUS.md` in the same commit when a task finishes.

## Parallel agents

- Each agent builds into its own target dir: `CARGO_TARGET_DIR=C:\Modding\kitcraft\.targets\<agent>`
  (PowerShell: `$env:CARGO_TARGET_DIR = "C:\Modding\kitcraft\.targets\<agent>"`). Delete it
  when you finish. `.targets/` is gitignored.
- Only edit crates you own per `plan/execution-plan.md`. Need a change elsewhere? Note it in
  `plan/STATUS.md` under "Requests" and keep going on your own lane.
- Shared files (`Cargo.toml` workspace deps, `AGENTS.md`, `docs/`) take small, separate
  commits so they merge cleanly.
- Read `plan/STATUS.md`, then `plan/execution-plan.md`, before starting. Update STATUS when
  you finish a task.

## Owner decisions

Don't stop to ask unless the decision is one of these. Everything else: decide and record it.

1. Project name (`KitCraft` is a placeholder).
2. xEdit MPL source policy (`plan/adr/0004-xedit-source-policy.md`, status OPEN; default is
   option A: the ADR 0005 sources only, xEdit not read).
3. Which video critique is the design brief for the gun overhaul.
4. License (proposed MIT OR Apache-2.0; scaffolded that way).
5. Whether and when to contact kinggath about Sim Settlements 2.
6. Adding a git remote or publishing anything.

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
| `docs/corpus.md`         | Roundtrip corpus: which files, where, how it is used          |
