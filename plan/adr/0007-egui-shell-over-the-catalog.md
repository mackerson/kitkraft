# ADR 0007: An egui shell over the command catalog

- Status: **ACCEPTED-WITH-SCOPE** (owner ruling 2026-10-14, M3.O6; see "Amendment 1"
  at the end: a minimal read-only developer workbench after M4's inspection APIs
  stabilize, the full authoring UI in M5). Was: Proposed.
- Date: 2026-10-14
- Scope: `crates/ui-egui` (L5, not created), `apps/kit` (APP, not created), a `kit ui`
  launcher in `apps/kit-cli`
- Relates to: ADR 0002 (layering), AGENTS.md "Everything is a command" and "CLI / MCP / UI
  parity", `docs/mcp.md` (the result envelope), `plan/execution-plan.md` M3.F4, M3.F5,
  M3.O6

## Context

`PLAN.md` puts the UI at M4+ ("egui UI over the command catalog"); M3's ROADMAP bullet
asks only for this proposal, so the owner can decide whether a first shell is worth
building in M3 (M3.F5) or waits for M4. Binding constraint 6 of M3 (scope discipline)
says the UI is an ADR only unless the owner opts in.

What a UI would sit on already exists and is exercised by two front ends:

- **The catalog.** `kitkraft_engine::catalog()` has 19 `CommandSpec`s, each with `id`,
  `label`, `menu_path` (`Record/Inspect`, `File/Build Patch...`, `Archive/Extract...`;
  eight top-level menus today), `shortcut` (5 set: `Ctrl+O`, `Enter`, `Ctrl+F`, `Ctrl+B`,
  `F7`), `aliases`, `summary`, `args_schema` and `status`. Labels ending in `...` follow the
  desktop convention for an action that opens a form first.
- **Arguments.** `args_schema` is a small JSON Schema subset: `object` with
  `properties`, `required`, `additionalProperties: false`; members of type `string`
  (73 occurrences), `boolean` (21), `integer` (10, some with `minimum`), `array` of
  strings (11), `enum` (3), `default` (24), `minLength`, one `pattern`. `params()` already
  turns a schema into the ordered parameter list the CLI parser and `kit help` use;
  a form generator needs nothing more.
- **Execution.** `Engine::execute(id, args) -> Result<Value, EngineError>`, synchronous,
  never prompting (AGENTS.md). Handlers are `Send + Sync`; the `Session` keeps its slots
  as `Box<dyn Any + Send>`, so the engine can live on a worker thread.
- **Results.** Every result is the envelope `{command, data, coverage, limitations,
  evidence}` with `data.summary` as the one-line text, `data.passed` on checks, and
  evidence as `(file, offset, length, record, subrecord)`. Errors have a stable `kind`.
- **Layers.** `kitkraft-ui-egui` (L5) and `kit` (APP) are already in xtask's
  `LAYER_TABLE`; `UI_TOOLKIT` bans `egui`, `eframe`, `winit`, `wgpu`, `rfd`, `iced`,
  `slint` below L5, directly, transitively and as dev-dependencies.

## Options

### A. A minimal egui shell in M3 (M3.F5)

`crates/ui-egui` (L5, depends on `kitkraft-engine` only) holds a toolkit-free shell model
plus its egui views; `apps/kit` is the desktop binary; `kit ui` in `apps/kit-cli` is a
front-end launcher (no `CommandSpec`, like `kit mcp`). Scope, and nothing more:

1. **Command palette and menus** built from the catalog: menus from `menu_path`, palette
   matching `label`, `id` and `aliases`, keys from `shortcut`; `Planned`/`Deferred`
   entries shown disabled with their reason.
2. **Argument form** generated from `args_schema`: text fields for strings (a path field
   gets a file picker in the UI layer only: `rfd` is allowed at L5 and APP), checkboxes,
   integer fields honouring `minimum`, enum drop-downs, string lists; defaults pre-filled;
   `required` enforced by the engine's own validation, whose `invalid_args` violations
   (`{pointer, rule, message}`) are shown next to the field they point at.
3. **Result view** from the envelope: `summary` and `passed` first, then `limitations`
   (never hidden), `coverage`, an evidence table, and the raw `data` JSON collapsed.
4. **Session**: one `Engine` on a worker thread; the UI sends `(id, args)` over a channel
   and shows a busy state, so a 50 s `coverage` of `Starfield.esm` never freezes the
   window. One command at a time, as in the MCP server.

Testability (M3.F5's acceptance): the shell model (catalog -> menus, schema -> form,
form -> args JSON, envelope -> view rows) has no egui types, so a test drives a command
through exactly the path a click takes, without a window. egui code is a thin view over it.

Costs:

- **Dependencies.** `eframe` with the `glow` (OpenGL) or `wgpu` backend, `egui`, `winit`,
  `accesskit` and their platform crates: on the order of 150-250 crates, all at L5/APP,
  licenses MIT/Apache/Zlib/BSD-style (to be listed with `cargo tree` before adding; any
  other license goes to the owner). Clean builds of the workspace get noticeably slower
  (minutes for the UI tree on first build); `cargo xtask ci` would build it on every run.
- **Effort.** About one lane-F session for the four items above with tests; more for
  polish (layout persistence, theming), which A excludes.
- **Risk to M3.** None to the critical paths (lane B round 3, CK observation and the
  overhaul do not touch L5), but it competes with hardening work for lane F's time, and
  the first GPU/driver issue on the owner's machine is support work.

### B. Defer the shell to M4 (the `PLAN.md` default)

Nothing is built in M3. The contract a UI needs (catalog fields, `args_schema`, envelope,
error kinds) keeps being enforced by the CLI and MCP tests, so nothing is lost by waiting;
new commands keep landing with a `label`, `menu_path` and a schema a form can render.
M4 starts from this ADR's option A scope.

Costs: no visual front end until M4; the owner keeps using `kit` and an MCP client (Claude
Code) to drive the overhaul.

### C. Another toolkit

- **iced** (MIT): Elm-style, also immediate enough for forms; smaller ecosystem for
  docking and tables; a similar dependency footprint (`winit`, `wgpu`).
- **Slint** (GPLv3, royalty-free or commercial licenses): declarative `.slint` UI files;
  its license options differ from ours (MIT OR Apache-2.0) and would need an owner
  licensing decision, which constraint 6 rules out without new evidence.
- **A web front end** (Tauri or a local HTTP server over `execute`): reuses the JSON
  surface directly, but adds a JavaScript toolchain and a second process model; the
  viewport (M4+, `crates/render`) is easier to embed next to egui with `wgpu` than in a
  webview.

egui's fit: immediate mode suits generated forms (no widget tree to keep in sync with a
schema), `egui_extras` has tables for evidence, AccessKit gives screen-reader support, and
the `wgpu` backend is the same stack a later `crates/render` viewport would use. Its
weakness, custom layout polish, matters least for a command shell.

## Rules any option keeps

- No logic the CLI and MCP lack: the shell builds `args` and calls `execute`; anything
  else it needs becomes a command or moves into a library crate below it.
- `execute` still never opens a dialog; file pickers and confirmations happen while the
  form is filled, before the call.
- Layer bans unchanged: no UI toolkit below L5, no `anyhow` in `crates/ui-egui`.
- The viewport stays out of scope (M4+, `crates/render`).
- A UI-only setting (window size, recent commands) lives in the UI crate's own config,
  never in a plugin or the engine session.

## Recommendation

**B: defer to M4.** M3's exit criteria are about data (round 3 types, CK evidence, the
overhaul build); none needs a window, and the owner's sessions (O0..O5) are the scarce
resource, not lane F's time. The contract a shell needs is already held in place by the
CLI and MCP tests, so building it in M4 costs no rework. If the owner wants to drive the
overhaul from a window rather than from `kit` or Claude Code, choose **A** with exactly
the four items above; it does not move any M3 critical path. C is not recommended.

## Decision

Decided 2026-10-14 by the owner: neither A nor B as written. See "Amendment 1: owner
ruling 2026-10-14" below, which wins where it differs from the text above.

## Consequences

- A: M3.F5 is scheduled for lane F; `crates/ui-egui` and `apps/kit` are created; AGENTS.md
  "Layers" marks them as existing; the dependency list goes into this ADR before merge.
- B: M3.F5 is carried to M4 with this ADR as its scope; `PLAN.md` and ROADMAP stay as
  they are.

## Amendment 1: owner ruling 2026-10-14 (M3.O6)

Recorded in the owner's meaning: the M5 deferral of egui is **partially overturned**.

1. The complete authoring UI stays deferred to **M5**.
2. An early, **minimal developer workbench** is introduced once the underlying read-only
   inspection APIs are stable.
3. It prioritizes: record browsing, structural diffing, schema and provenance
   inspection, FormID and reference diagnostics, and build validation.
4. The UI is an observability and development surface, not a competing implementation of
   application logic: every operation uses the same underlying services as the CLI.
5. It is integrated into the roadmap without interrupting running lanes or a broad
   architecture rewrite.

### Scope of the workbench

- **Where.** `crates/ui-egui` (L5, depends on `kitkraft-engine` only) and `apps/kit` (APP),
  both already in xtask's `LAYER_TABLE`; a `kit ui` launcher in `apps/kit-cli` (a
  front-end launcher, no `CommandSpec`, like `kit mcp`). egui/eframe as in option A; the
  dependency list (`cargo tree`, licenses) goes into this ADR before the first UI merge,
  and any license outside MIT/Apache/Zlib/BSD-style goes to the owner. This is the one
  exception to M4 binding constraint 6 ("no new crate in M4"), by the owner's ruling.
- **Over `Engine::execute` only.** The shell builds `(id, args)` and renders the result
  envelope. It has no parser, resolver, diff or validation code of its own; a view that
  needs data no command returns gets a field in that command's result (by the lane owning
  it) or a new command first.
- **Read-only.** The workbench offers only commands the catalog marks as not writing (the
  same marking that drives MCP's `destructiveHint`, M4.F1); `build` only with
  `plan: true`; `roundtrip` only with `verify` and no `out`. No `set`, `build` output,
  `package`, `deploy` or other writing command, and no argument form for them, until the
  M5 authoring UI.
- **Views** (each one command or a fixed sequence of commands):
  1. Record browser: `load` (profile or plugin list), `query` with `where`, `inspect` with
     the override chain, `limitations`, `coverage` and byte-offset evidence (M4.D2
     placement context where present).
  2. Structural diff: `diff` and `conflicts` with member paths (`fields[]`), before and
     after, opaque ranges as byte ranges.
  3. Schema and provenance: `schema --deep` per record type: members, `source`,
     `evidence`, `confidence`; `reference`-only members marked as unconfirmed.
  4. FormID and reference diagnostics: raw stored FormIDs next to master-aware resolved
     identities (ADR 0004 clarification 5; an unavailable master's scale shown as
     unknown), `refs` (M4.D1), `validate`'s `formid.dangling` and missing-master
     findings.
  5. Build validation: `validate` results by rule, `build` with `plan: true`, and
     `roundtrip --verify`.
- **Engine on a worker thread**, one command at a time, busy state shown (as option A,
  item 4). Shell model without egui types, so tests drive a view through the same path a
  click takes, without a window.
- **Out of scope until M5:** argument forms for writing commands, editing, undo, the
  command palette for writing commands, layout persistence beyond window size, the
  viewport (`crates/render`).

### When

After M4's inspection APIs stabilize: `inspect`, `query`, `diff`, `conflicts`, `refs`
(M4.D1), `schema --deep`, `validate` (with M4.E4's cell rules) and placement context
(M4.D2) are `Implemented`, and the shape of each result's `data` is fixed by a shape test
(M4.W0). Planned as lane W in `plan/execution-plan.md` M4 (M4.W0..W6); on no critical
path. M3.F5 is not built in M3; M4.F3 is superseded by lane W.

The owner also deferred the CK observation sessions (M3.O1, M4.O1) until the workbench
exists, to A/B the CK against KitKraft's inspection surface (an agent with computer use
may drive that session). The workbench is still on no critical path, but the `ck-diff`
confidence upgrades now wait for M4.W2..W5.

### Consequences of the amendment

- `plan/execution-plan.md`: M3.O6 decided; lane W task block in M4 (planning only);
  ROADMAP M4 gains the workbench, M5 keeps the full authoring UI.
- AGENTS.md "Layers": `crates/ui-egui` and `apps/kit` are marked as existing when lane W
  creates them, not before.

## Dependency list (lane W, 2026-10-15; Amendment 1 "before the first UI merge")

`crates/ui-egui` depends on `eframe` 0.36 with `default-features = false, features =
["glow"]` (OpenGL through `glutin`; no `wgpu`, no bundled fonts, no AccessKit, no web
features; Linux adds `x11`/`wayland` in a target table). `apps/kit` adds only `clap`, which
the workspace already has. `cargo tree -p kitkraft-ui-egui -e normal` on Windows lists 117
distinct external packages, the engine's existing ones included. `Cargo.lock` grows from 67
to 344 packages because it records the macOS, Linux and Android crates eframe could use;
no existing locked version changed. No workspace package gained a build script.

Licenses of the 117, by expression:

| License expression | Packages | Status |
| ------------------ | -------- | ------ |
| MIT OR Apache-2.0 (in any spelling) | 81 | allowed |
| MIT | 9 | allowed |
| Apache-2.0 (incl. `winit`, `glutin`); Apache-2.0 AND MIT (`dpi`) | 6 | allowed |
| Zlib, or Zlib with MIT and Apache options | 9 | allowed |
| BSD-3-Clause OR Apache-2.0; BSD-2-Clause OR Apache-2.0 OR MIT; 0BSD OR MIT OR Apache-2.0 | 4 | allowed |
| Unlicense OR MIT | 3 | allowed (MIT option) |
| Apache-2.0 OR GPL-2.0-only (`self_cell`) | 1 | allowed (Apache option) |
| ISC (`libloading`) | 1 | BSD-style |
| (MIT OR Apache-2.0) AND Unicode-3.0 (`unicode-ident`) | 1 | already in the workspace (proc-macro, build time) |
| **BSL-1.0** (`clipboard-win`, `error-code`, through `arboard`) | 2 | **owner decision pending**: not MIT/Apache/Zlib/BSD by name. The Boost licence is permissive and needs no attribution in binaries; eframe's native build enables the clipboard unconditionally, so dropping it means no copy buttons |

Not built: `epaint_default_fonts` ((MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0).
The window reads Segoe UI and Consolas from `%WINDIR%\Fonts` at start, so no font is
compiled in or shipped. `kitkraft-ui-egui`'s `default_fonts` feature turns the bundled
fonts on for a machine without them; it needs the owner's yes before it is used.

Build and test cost on the dev box: a clean debug build of the eframe tree takes about
36 s (in parallel with the rest of the workspace); `cargo xtask ci` with a warm target dir
and the corpus available took 271 s, of which the `kitkraft-ui-egui` and `kit` tests are
under 1 s (view-models against canned envelopes, the gate, the worker; no window is opened).
