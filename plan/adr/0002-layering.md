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
3. Below L5, no UI toolkit (egui, eframe, winit). Library crates do not use `anyhow`.
4. The layer of each package is declared twice on purpose: a `//! LAYER:` marker in the
   crate root and `LAYER_TABLE` in xtask. They must agree, so relabelling a crate to permit
   a dependency is a visible two-place change that needs an ADR.
5. `cargo xtask layers` checks `[dependencies]`, `[build-dependencies]`, and their
   `[target.*]` forms. `[dev-dependencies]` are exempt so tests can use helpers freely.

### Allowlisted same-layer edges

| From | To     | Why                                                                                 |
| ---- | ------ | ----------------------------------------------------------------------------------- |
| esm  | schema | The container decodes subrecords using definitions; schema stays container-agnostic |

## Consequences

- `io` (L3) sits above `doc` (L1): it builds a load order from an MO2 profile and hands it to
  `doc`. `doc` itself never touches the filesystem layout of a mod manager.
- `render` (L2) can read the document but cannot reach engine commands; it is driven from
  L5.
- Adding a crate means editing `LAYER_TABLE` and the AGENTS.md table in the same commit.
