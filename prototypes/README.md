# Prototypes

Exportable prototypes and test mods (ADR 0011, `plan/adr/0011-foothold-benchmark.md`,
decision 2). One folder per prototype, each self-contained: build specs (`*.toml`), our
own `.psc` sources, and a `README.md` with its purpose, the KitKraft commit it was last
built with, and the ticket ids it serves. Build output goes to `out/` (gitignored).

## Export rule

- A prototype uses KitKraft **only through `kit`** (CLI or MCP). Nothing under
  `crates/*`, `apps/*` or `xtask` reads `prototypes/`; when a prototype needs a
  capability, the capability is added to the core first as a general command or recipe
  (`docs/recipes/`), then used here.
- **Nothing in a prototype's folder refers to a path outside it except `kit` itself** and
  the game's own files it names by FormID. So a folder can leave the repo as it is:
  `git subtree split --prefix=prototypes/<name>` (history kept) or a plain copy.
- Prototypes are not shippable mods and are not deployed anywhere but the **KitKraft Dev**
  profile through `kit deploy --dev` (ADR 0009); every one that owns forms is for a
  disposable save only (AGENTS.md, "Owner's live game is off limits").

| Folder | Ticket | What |
| ------ | ------ | ---- |
| `hostile-fixture/` | M5.H2 (first half) | The generic hostile-encounter fixture of `docs/research/hostile-encounters-2026-10-16.md` section 4 |
