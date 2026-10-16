# Prototypes

Exportable prototypes, fixtures and test mods that advance KitKraft's reusable
capabilities, kept apart from production code (ADR 0011 §2,
`plan/adr/0011-foothold-benchmark.md`).

- One folder per prototype: `prototypes/<name>/`, or under a group such as
  `prototypes/foothold/` for the Foothold experiments (`FTH-EXP-nn`).
- Each is self-contained: build specs (`*.toml`), our own `.psc` sources, probe files and a
  `README.md` with its purpose, the KitKraft commit it was last built with, and the ticket
  ids it serves.
- Prototypes use KitKraft only through `kit` (CLI or MCP); nothing under `crates/*`,
  `apps/*` or `xtask` reads `prototypes/`. Build output goes to `out/` (gitignored).
- `Starfield.esm` is the preferred only master; no Bethesda data or source is committed.
- Export: `git subtree split --prefix=prototypes/<name>` (history kept) or a plain copy.
