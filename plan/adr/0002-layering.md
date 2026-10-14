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
| L2    | Rendering (later)                               | render                              |
| L3    | Outside-world I/O (MO2, plugins.txt, manifests) | io                                  |
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

## Consequences

- `io` (L3) sits above `doc` (L1): it builds a load order from an MO2 profile and hands it to
  `doc`. `doc` itself never touches the filesystem layout of a mod manager.
- `render` (L2) can read the document but cannot reach engine commands; it is driven from
  L5.
- The L2 `render` slot is reserved, not designed: no crate exists yet, and it may move to a
  different layer (with an ADR) once the viewport work starts.
- Adding a crate means editing `LAYER_TABLE` and the AGENTS.md table in the same commit.
