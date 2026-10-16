# ADR 0007: An egui shell over the command catalog

- Status: **ACCEPTED-WITH-SCOPE** (owner ruling 2026-10-14, M3.O6; see "Amendment 1"
  at the end: a minimal read-only developer workbench after M4's inspection APIs
  stabilize, the full authoring UI in M5). Was: Proposed. "Amendment 2" (2026-10-15):
  one permitted write, the `ledger` row in `docs/game-observations.md`, confirmed per
  click. "Amendment 3" (**ACCEPTED**, owner ruling 2026-10-16, M6.O3; proposed the same
  day): the M6 cockpit write scope, `ledger`, `deploy --dev`, `deploy.rollback`,
  `game.session`, `game.probe` and `game.stop`, each previewed and confirmed per click,
  and nothing else.
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
  webview. [2026-10-16: the viewport is L5 view code in `crates/ui-egui/src/viewport/`
  and the L2 `render` slot is retired, ADR 0002 Amendment 1.]

egui's fit: immediate mode suits generated forms (no widget tree to keep in sync with a
schema), `egui_extras` has tables for evidence, AccessKit gives screen-reader support, and
the `wgpu` backend is the same stack a later `crates/render` viewport would use (now
`crates/ui-egui/src/viewport/` on eframe's glow backend, ADR 0002 Amendment 1). Its
weakness, custom layout polish, matters least for a command shell.

## Rules any option keeps

- No logic the CLI and MCP lack: the shell builds `args` and calls `execute`; anything
  else it needs becomes a command or moves into a library crate below it.
- `execute` still never opens a dialog; file pickers and confirmations happen while the
  form is filled, before the call.
- Layer bans unchanged: no UI toolkit below L5, no `anyhow` in `crates/ui-egui`.
- The viewport stays out of scope (M4+, `crates/render`). [Superseded 2026-10-16 by ADR
  0002 Amendment 1: the viewport is M7 lane R, L5 view code in
  `crates/ui-egui/src/viewport/`, fed by engine commands only.]
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
  viewport (then `crates/render`; since ADR 0002 Amendment 1, `crates/ui-egui/src/viewport/`,
  M7 lane R).

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
["glow"]` (OpenGL through `glutin`; no `wgpu`, no bundled fonts, no AccessKit platform
adapter, no web features; Linux adds `x11`/`wayland` in a target table). Correction
(Fable review, 2026-10-15): this first said "no AccessKit", which is wrong. `egui` itself
depends on `accesskit` 0.24.1 (MIT OR Apache-2.0) unconditionally; what eframe's
`accesskit` feature (off here) adds is the platform adapter (`accesskit_winit` and the
OS crates), which is not built. `accesskit` is counted in the first row below. `apps/kit` adds only `clap`, which
the workspace already has. `cargo tree -p kitkraft-ui-egui -e normal` on Windows lists 117
distinct external packages, the engine's existing ones included. `Cargo.lock` grows from 67
to 344 packages because it records the macOS, Linux and Android crates eframe could use;
no existing locked version changed. No workspace package gained a build script.

Licenses of the 117, by expression:

| License expression | Packages | Status |
| ------------------ | -------- | ------ |
| MIT OR Apache-2.0 (in any spelling; incl. `accesskit` 0.24.1, a non-optional `egui` dependency) | 81 | allowed |
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

## Amendment 2: the ledger is the single permitted write, to a repo doc, confirmed per click (lane W, 2026-10-15)

Amendment 1 made the workbench read-only. The owner's in-game QA (lane W's "human QA"
wave) needs one exception, so the results seen in game land where the plan reads them
without a hand edit: a row of the in-game ledger, `docs/game-observations.md`.

- **What may be written.** Exactly one catalog command runs in a writing form from the
  workbench: `ledger` (`writes: true`). It appends one dated row (check `C1`..`C14`,
  `POI`, `persistence` or `custom`; `pass`, `fail` or `note`; FormIDs; what was seen) to
  `docs/game-observations.md`, under an entry `game-<yyyymmdd>-<nn>: workbench ledger,
  <date>, <profile>` at the end of the file. It never rewrites what is there. The engine
  refuses every other path (the file must be named so, in `docs`, next to the checkout's
  `AGENTS.md`, start with the ledger's title and have its `## Ledger` section, be reached
  through no link, and lie outside the game, the user Data folder and MO2) and the
  `Default` profile as the profile a check ran on. It is a repo doc, never game data.
- **Confirmed per click.** Without `confirm: true`, `ledger` writes nothing and returns
  the text it would append (the panel's Preview); the gate lets that through like any
  read. The writing form passes the gate (`gate::PERMITTED_WRITE`) only in a request
  built by `Request::confirmed`, which only the Ledger panel's confirm button makes, once
  per click; a test scans the crate's sources for it. A confirmed request for any other
  command is refused.
- **Still through `Engine::execute`.** The panel builds `(id, args)` and the engine does
  the work, as for every other view; the CLI (`kit ledger ... --confirm`) and MCP reach the
  same command.
- **The gate's disjointness test** now holds: the allowlist has no writing command,
  `WRITING` equals the catalog's `writes` set, guarded commands refuse their writing
  arguments, and the set of writing commands the workbench can run unguarded is exactly
  `{ledger}`.
- **UI state is not a write of this kind.** The window remembers the start screen's last
  profile and the docs checkout in `%LOCALAPPDATA%\kitkraft\ui.json` (the "UI-only
  setting" of "Rules any option keeps"). The Help tab reads the repo's docs (text, not
  game data) to show the owner's QA material; it runs no command and writes nothing.

Everything else in Amendment 1 stands: no `set`, `build` output, `package`, `deploy` or
other writing command, and no form for one, until the M5 authoring UI.

## Amendment 3 (accepted 2026-10-16, owner decision M6.O3): the cockpit write scope (roadmap lane, 2026-10-16)

- Status: **ACCEPTED** (owner ruling 2026-10-16, M6.O3; see "Owner ruling 2026-10-16" at
  the end of this amendment). Was: Proposed, and nothing in it took effect until the owner
  accepted it; until then Amendment 2 stood as written, with the ledger as the only write.
- Context: the owner's 2026-10-16 decisions (`ROADMAP.md`, "M6: Cockpit"). The UI is for
  "Both, cockpit first". M6 makes the workbench the human cockpit beside the agents:
  review what an agent built, diff, validate, run the in-game checks and record the
  ledger. Agents stay the main authors through MCP. The owner's other top pain was "game
  loop integration"; the director's brief spells it out as no deploy, launch, probe or
  screenshot flow in the window, and a ledger filled by hand. The read-only wall was
  **not** named as a pain,
  so this amendment opens as little as the game loop needs and no editing.

### What the cockpit may run in a writing form in M6

There are exactly six commands, each an existing or planned catalog command with
`writes: true`. The window runs each one only through `Engine::execute`, and only after a
confirm click:

| Command | Writes | Preview form (passes the gate like a read) | Writing form (confirmed request only) | Exists |
| ------- | ------ | ------------------------------------------ | ------------------------------------- | ------ |
| `ledger` | One row appended to `docs/game-observations.md` (Amendment 2) | `confirm` absent or false | `confirm: true` | yes |
| `deploy` (`kit deploy --dev`, ADR 0009) | One isolated `KitKraft Dev - <plugin>` mod in an MO2 instance | `dry_run: true` (the plan and every path it would touch) | gate: `dev: true`, `dry_run` absent or false, `profile` present and not `Default`. The panel fills `mo2` and `profile` from the loaded dev profile (see "What the gate checks") | yes |
| `deploy.rollback` (ADR 0009, `docs/deploy.md`) | Swaps one dev mod's `.previous` deployment back in, keeping the replaced one as the new `.previous` (so a rollback can itself be rolled back) | `dry_run: true` (the plan and every path it would touch) | as `deploy`; the panel also fills `mod` and `to` (the `.previous` id that `deploy.status` showed) | yes |
| `game.session` (M4.T3, ADR 0010 and its Amendment 1) | What ADR 0010 Amendment 1 condition 3 authorizes: `observe\<session>\`, one ledger entry, `KitKraftSession-*` saves, restore-only rollback | `dry_run: true` (the session plan) | `start: true`, `dry_run` absent or false: returns a session id at once; the session runs on in the engine and the window follows it with `game.status <id>` (read-only, on `ALLOWLIST`) | **no**: M4.T3 |
| `game.probe` (M4.T3) | As `game.session`, for one probe or a screenshot in a running session | its dry-run form, if M4.T3 gives it one; otherwise none, and only the confirmed form runs | the confirmed request | **no**: M4.T3 |
| `game.stop` (M4.T3) | Ends a running session: terminates the game, completes its ledger entry and verifies the profile's list files and INIs (ADR 0010's end-of-session steps) | `dry_run: true` (what stopping that session would do) | the session id, `dry_run` absent or false | **no**: M4.T3 |

Why `deploy.rollback` is in: it is `Implemented`, `writes: true`, has `dry_run`, requires
`dev: true` and refuses `Default` in the engine, so it meets the five-point rule below; and
a cockpit that could deploy but not take a deploy back would leave the owner a worse way
back than forward. It is a safety control.

Why `testplan` stays out: it writes a checklist and `probes.toml` into an output folder.
That is file output, not a safety control, and the cockpit does not need to make it: the
agent that built the plugin runs `testplan` over MCP or the CLI, and the cockpit passes
the resulting `probes.toml` path to `game.session`.

Everything else stays as Amendments 1 and 2 have it. That includes `testplan`, `package`,
`build` output, `set`, `new`, `clone`, `place`, `mod.run` (M4.T10) and the other writing
commands. The editor's writes are M7 and need their own
amendment (Amendment 4, M7.E1). Read-only commands such as `deploy.status`, `doctor` and
`game.status` (if the catalog marks it non-writing) move onto `ALLOWLIST` like any read
and need no amendment. A `game.status` that the catalog marks writing waits for a further
amendment.

**The window never launches the game itself.** No process spawning, no MO2 command line
and no HTTP to the Console API in `crates/ui-egui`. Until `game.session`, `game.probe`
and `game.stop` are `Implemented` in the catalog, the cockpit shows their buttons disabled, with the
ticket id (`M4.T3`) as the reason. The button's state is read from the catalog
(`find_command` and its `status`), not hard-coded, so it turns on with the engine command
and with nothing else.

### The rule for adding a write

A writing command joins the cockpit's scope only when all of these hold:

1. It is one catalog command with `writes: true`, `Implemented`, reachable from the CLI
   and MCP with the same arguments (AGENTS.md "CLI / MCP / UI parity").
2. It has a preview form that writes nothing (`dry_run`, `plan`, or `confirm` false), or
   the amendment says why it cannot have one.
3. The engine itself refuses `Default` and every path outside the command's own
   authorized list. The gate checks what it can from the arguments alone before the
   engine sees the request (belt and braces), but never instead of the engine.
4. Exactly one panel file builds its confirmed request, once per click, after a confirm
   box that shows what will be written (the preview's text).
5. An amendment to this ADR names it, and the owner accepts the amendment. This one
   proposes five additions (`deploy`, `deploy.rollback`, `game.session`, `game.probe`,
   `game.stop`). M7's editor writes are Amendment 4.

### What the gate checks, and what the panel checks

The gate (`crates/ui-egui/src/gate.rs`) is a const table and a pure check over the
request's id, arguments and confirmation. It holds no session state and does not know
which profile the window has loaded. For `deploy` and `deploy.rollback` it checks only:
`dev: true`; for the writing form, `dry_run` absent or false; and `profile` present and
not `Default` (case-insensitive). The **panel**, not the gate, compares `mo2` and
`profile` with the loaded dev profile and refuses to open the confirm box when they
differ. The engine refuses `Default` on its own, from MO2's active profile, whatever the
arguments say.

### The test that enforces it

The existing disjointness test in `crates/ui-egui/src/gate.rs`
(`allowlist_is_disjoint_from_writes_and_writing_equals_it`) is extended. M6.G1 makes the
change, with no new write enabled:

- `PERMITTED_WRITE` becomes `PERMITTED_WRITES: &[PermittedWrite]`. Each entry has the
  canonical id, the argument test that tells its preview from its writing form, the one
  panel file allowed to build its confirmed request, and its `why`.
- The assertions:
  - `ALLOWLIST` holds no command the catalog marks `writes`.
  - `WRITING` equals the catalog's `writes` set.
  - Every `GUARDED` entry refuses at least one argument.
  - **The set of writing commands the window can run in a writing form is exactly the ids
    of `PERMITTED_WRITES`.** In M6 that is a subset of `{ledger, deploy, deploy.rollback,
    game.session, game.probe, game.stop}`: an id whose command is not yet in the catalog
    is listed only once it exists.
  - A second constant in the test, `COCKPIT_SCOPE`, is checked against it. Adding an id to
    `PERMITTED_WRITES` that is not in `COCKPIT_SCOPE` fails the test, and `COCKPIT_SCOPE`
    changes only with an accepted amendment to this ADR (the test's doc comment says so).
    M6.G1 lands it as `["ledger"]`, Amendment 2's scope, so it needs no ruling; once the
    owner accepts this amendment it may grow to `["ledger", "deploy", "deploy.rollback",
    "game.session", "game.probe", "game.stop"]`, entry by entry with M6.G3 to M6.G5.
- The source scan (`only_the_ledger_panel_builds_a_confirmed_request`) is generalized.
  For each permitted write, `Request::confirmed(<id>, ..)` may appear only in `gate.rs`
  and that entry's panel file. A confirmed request for an id outside
  `PERMITTED_WRITES` is refused (as today for everything but `ledger`).
- Each new entry adds its own tests:
  - The preview passes unconfirmed.
  - The writing form is refused unconfirmed.
  - The confirmed writing form reaches the engine. Deploy is tested against a synthetic
    MO2 instance; `game.*` against M4.T3's fixture instance and fake endpoint.
  - `Default` is refused by the gate and by the engine.

### UI state and development aids (unchanged in kind)

The window may also write `%LOCALAPPDATA%\kitkraft\ui.json`. M6 adds the window's size,
position, maximized state, theme, density and zoom to it (M6.P7, M6.V3). It still names no
plugin, profile file or game folder. The screenshot tour stays opt-in by environment
variable and refuses a folder that `guard_output_dir` rejects. The performance trace
(M6.P1) goes to stderr only and writes no file.

### Dependency additions M6 and M7 may make (each confirmed at its merge, as in "Dependency list")

| Crate | Used by | License | State |
| ----- | ------- | ------- | ----- |
| `glow` 0.17.0 | M7.R1, the GL viewport through egui's `PaintCallback` on eframe's glow backend (reached as `eframe::glow`; no new framework) | MIT OR Apache-2.0 OR Zlib (checked 2026-10-16 in the local registry copy) | already in the tree through `eframe`'s `glow` feature |
| `glam` | M7.R1 camera and transform math (M7.R1 may merge during M6) | MIT OR Apache-2.0 (to be checked with `cargo tree` and the crate's manifest at the merge) | new |
| `egui_extras` (tables; image loaders only if M6.G5 needs them) | M6.P4 virtualized tables, M6.G5 screenshots | MIT OR Apache-2.0 (egui's own repository; to be checked at the merge with its transitive tree, e.g. `image` and its codecs) | new |
| eframe's `accesskit` feature (`accesskit_winit` and the platform crates) | M6.X3 screen reader support (deferred with lane X, 2026-10-16) | expected MIT OR Apache-2.0; the full transitive list goes here before the merge | new feature |
| A docking crate (for example `egui_dock`) | M7.M0 decides whether the CK-style window layout needs one | to be listed if chosen | not chosen |

Any license outside MIT/Apache/Zlib/BSD-style goes to the owner. That includes the
BSL-1.0 clipboard crates already pending (`plan/STATUS.md`, owner row 7) and any OFL font
(M6.O2: fonts stay system fonts unless the owner rules otherwise). All icons and art are
our own.

### Consequences

- `plan/execution-plan.md` "M6": M6.G1 implements the mechanics with
  `PERMITTED_WRITES = COCKPIT_SCOPE = [ledger]` and does not wait on this ruling. After
  it, M6.G3 adds `deploy` and `deploy.rollback`, M6.G4 `game.session` and `game.stop`,
  M6.G5 `game.probe`. M6.G4 and M6.G5 wait on M4.T3.
- `docs/workbench.md` "What it can run" is rewritten when M6.G1 merges, and on each
  addition.
- AGENTS.md "Layers": the `ui-egui` row ("Read-only workbench (ADR 0007 Am. 1)") becomes
  "Workbench (ADR 0007 Am. 1 to 3): reads, plus the confirmed writes of Amendment 3" when
  the owner accepts this amendment, not before.

### Owner ruling 2026-10-16 (M6.O3)

The owner, in chat, 2026-10-16, verbatim: "Approve Amendment 3 and ADR 0012, start lane V
mockups".

- **Accepted as written**, with no change. The cockpit may run exactly six commands in a
  writing form: `ledger`, `deploy` (`--dev`), `deploy.rollback`, `game.session`,
  `game.probe` and `game.stop`. Each is previewed and confirmed per click, and nothing
  else is allowed. The window still never launches the game itself, and `testplan` stays
  out.
- The ruling also confirms the director's decision 3 of 2026-10-16 (`plan/execution-plan.md`
  "M6"), which this amendment contains: rollback in, `testplan` out, and `game.stop` with
  M4.T3's start, poll and stop split. The director's other decisions of that day are not
  part of this ruling (`plan/STATUS.md` row 16).
- **What changes now.** `COCKPIT_SCOPE` may grow past `["ledger"]`, one entry at a time,
  each with the ticket that implements it. M6.G3 adds `deploy` and `deploy.rollback`, and
  that is its first "done when". M6.G4 adds `game.session` and `game.stop`, and M6.G5
  adds `game.probe`. An id joins only once its command is in the catalog, so the `game.*`
  ids wait on M4.T3. Recording the ruling changes no code: until those tickets merge, the
  ledger stays the only write the window runs.
- **Consequences checked against the M6 tickets** (`plan/execution-plan.md` "M6", lane G,
  2026-10-16). They match:
  - M6.G1 (the mechanics, scope `[ledger]`) never waited on the ruling.
  - The ruling unblocks M6.G3 to M6.G6, and each still waits on its other dependencies
    listed there. M6.G3 needs M6.G2. M6.G4 needs M4.T3, M5.C1, M6.P2b and M4.T2. M6.G5
    needs M6.G4 and M4.T3. M6.G6 needs M6.G5 and M5.C1.
  - M6.G6 adds no id. It prefills the existing confirmed `ledger` request.
  - AGENTS.md "Layers" takes the `ui-egui` row wording above, in a separate small commit.
