# ADR 0002: Crate layering

- Status: Accepted
- Date: 2026-10-14

## Context

Agents work in parallel on separate crates. Without a hard dependency rule, format crates
start importing engine types, the UI leaks into the model, and every change touches
everything. The craft suite solves this with a fixed layer stack checked in CI.

## Decision

| Layer | Contents                                        | Crates                              |
| ----- | ----------------------------------------------- | ----------------------------------- |
| L0    | Formats: bytes in, bytes out, no policy         | schema, esm, ba2, pex, strings, nif |
| L1    | Document model across a load order              | doc                                 |
| L2    | Rendering (retired by Amendment 1, 2026-10-16)  | render (none; see Amendment 1)      |
| L3    | Outside-world I/O (MO2, plugins.txt, manifests; game sessions, ADR 0010) | io, game                            |
| L4    | Commands, sessions, undo, validation            | engine                              |
| L5    | Front-end adapters                              | mcp, ui-egui (later)                |
| APP   | Binaries                                        | kit-cli, kit (later)                |
| TOOL  | Repo automation                                 | xtask                               |

1. A crate depends only on lower layers. APP and TOOL may depend on any library layer;
   nothing depends on APP or TOOL.
2. Same-layer edges are illegal unless allowlisted in `SAME_LAYER_ALLOWED`
   (`xtask/src/main.rs`) with a justification below.
3. Below L5, no UI toolkit (name prefixes `egui`, `eframe`, `winit`, `wgpu`, `rfd`, `iced`,
   `slint`). Library crates do not use `anyhow` or `eyre`. Both bans apply to the transitive
   normal-dependency closure that `cargo metadata` resolves, not only to direct
   dependencies.
4. The layer of each package is declared twice on purpose: a `//! LAYER:` marker in the
   crate root and `LAYER_TABLE` in xtask. They must agree, so relabelling a crate to permit
   a dependency is a visible two-place change that needs an ADR.
5. `cargo xtask layers` checks `[dependencies]`, `[build-dependencies]`, and their
   `[target.*]` forms from the manifests, then the resolved graph from `cargo metadata`.
   (Amended 2026-10-14; dev-dependencies were exempt before.) `[dev-dependencies]` may not
   point at an APP or TOOL package, may not bring a UI toolkit below L5, and between
   workspace crates follow the normal direction rules unless allowlisted in `DEV_ALLOWED`
   (`xtask/src/graph.rs`) with a justification below. Today the list is empty.
6. No package has a build script unless allowlisted in `BUILD_RS_ALLOWED` (empty), and no
   source file pulls in files outside its package through `#[path]` or the `include!`
   family. Either would let code cross a layer boundary without a dependency edge.

### Allowlisted upward dev-dependency edges (`DEV_ALLOWED`)

None.

### Allowlisted same-layer edges

| From | To     | Why                                                                                 |
| ---- | ------ | ----------------------------------------------------------------------------------- |
| esm  | schema | The container decodes subrecords using definitions; schema stays container-agnostic |

### Packages restricted to listed workspace dependencies (`INTERNAL_ONLY`)

| Package | May depend on (workspace) | Why |
| ------- | ------------------------- | --- |
| ui-egui | engine | ADR 0007 Amendment 1: the workbench is a view over `Engine::execute` with no logic of its own. Reaching `doc`, `esm` or `io` directly would let it parse or resolve on its own, and would tie the swappable toolkit crate to the format crates. Checked on the resolved graph, normal and dev edges (added 2026-10-15, lane W) |

## Consequences

- `io` (L3) sits above `doc` (L1): it builds a load order from an MO2 profile and hands it to
  `doc`. `doc` itself never touches the filesystem layout of a mod manager.
- `render` (L2) can read the document but cannot reach engine commands; it is driven from
  L5.
- The L2 `render` slot is reserved, not designed: no crate exists yet, and it may move to a
  different layer (with an ADR) once the viewport work starts.
- Adding a crate means editing `LAYER_TABLE` and the AGENTS.md table in the same commit.

## Amendment 1 (2026-10-16): the viewport is L5 view code; the L2 `render` slot is retired

- Status: **Accepted by the director**, 2026-10-16. Layering is not on the owner's list
  (AGENTS.md "Owner decisions"); the owner may overrule it (`plan/execution-plan.md`,
  "Decisions taken by the director 2026-10-16, pending owner").
- Context: the 3D viewport is planned as M7 lane R (`plan/execution-plan.md` "M7"), with
  its first ticket (M7.R1) allowed to start during M6. It draws inside the egui window
  through eframe's glow backend, so its code is toolkit code and cannot sit below L5
  (rule 3). The L2 slot above was reserved for it "with an ADR once the viewport work
  starts"; this is that ADR.

Decision:

1. **The viewport is L5 view code** in `crates/ui-egui/src/viewport/` (camera, drawing,
   picking). It is bound by everything that binds `kitkraft-ui-egui`: `INTERNAL_ONLY`
   (the engine alone among workspace crates) and the read-only gate.
2. **Geometry reaches the viewport only through engine commands** (`cell`,
   `mesh.bounds`, `mesh.geometry`; M7.S3 to M7.S5). The window never reads a plugin, a
   `.nif` or a `.ba2` itself.
3. **`crates/nif` stays behind the engine.** There is no L5 -> L0 edge: `kitkraft-ui-egui`
   never depends on `kitkraft-nif` or `kitkraft-ba2`, and `INTERNAL_ONLY` is unchanged.
4. **The L2 `render` slot is retired**, not kept in reserve. A future crate that prepares
   geometry below the engine (a headless mesh decoder, for instance) gets its own row by a
   new amendment when someone proposes it; nothing is reserved for it now. The
   `kitkraft-render` entry leaves xtask's `LAYER_TABLE`, the AGENTS.md layer table and
   `docs/architecture.md` in one commit, with M7.R1 (code change, so not in this docs
   commit). Until then the entry names a crate that does not exist and constrains nothing.

The "Consequences" bullets above about `render` (L2) are superseded by this amendment.

## Row added 2026-10-16 (lane C, M4.T3): `crates/game` at L3

ADR 0010 rule 5 places the session controller (`kitkraft-game`) at L3 beside `io`, depending
on L0 and L1 only; it uses no workspace crate at all. The engine (L4) reads MO2 through
`kitkraft-io` and passes plain facts to it, so there is no same-layer `game -> io` edge and
`SAME_LAYER_ALLOWED` is unchanged. Its external dependencies are `serde`, `serde_json`,
`toml`, `thiserror` and `regex`, all already in the workspace; no HTTP or process crate
(ADR 0010, "Dependency list").
