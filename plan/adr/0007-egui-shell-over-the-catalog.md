# ADR 0007: An egui shell over the command catalog

- Status: **Proposed** (owner decision M3.O6; nothing below is decided until the owner
  records it here)
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

- **The catalog.** `kitcraft_engine::catalog()` has 19 `CommandSpec`s, each with `id`,
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
- **Layers.** `kitcraft-ui-egui` (L5) and `kit` (APP) are already in xtask's
  `LAYER_TABLE`; `UI_TOOLKIT` bans `egui`, `eframe`, `winit`, `wgpu`, `rfd`, `iced`,
  `slint` below L5, directly, transitively and as dev-dependencies.

## Options

### A. A minimal egui shell in M3 (M3.F5)

`crates/ui-egui` (L5, depends on `kitcraft-engine` only) holds a toolkit-free shell model
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

_Pending: the owner records "A (build M3.F5)" or "B (defer to M4)" here, with the date
(M3.O6)._

## Consequences

- A: M3.F5 is scheduled for lane F; `crates/ui-egui` and `apps/kit` are created; AGENTS.md
  "Layers" marks them as existing; the dependency list goes into this ADR before merge.
- B: M3.F5 is carried to M4 with this ADR as its scope; `PLAN.md` and ROADMAP stay as
  they are.
