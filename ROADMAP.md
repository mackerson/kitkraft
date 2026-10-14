# Roadmap

Milestones are gated by evidence, not dates. A milestone is done when its exit criteria pass
in `cargo xtask ci` (or in a documented corpus run), not when the code exists.

## M0: scaffold

Toolchain on the dev box, repository skeleton, agent operating docs, layer check, local CI.

Exit criteria:
- `rustup` stable with rustfmt and clippy installed.
- `cargo xtask ci` passes on the empty workspace (layers, fmt, clippy, test).
- Corpus = every `.esm` under `Starfield\Data`, listed with hashes in `corpus/manifest.toml`.
- First commit on `main`.

## M1: data parity

`kit roundtrip <esm> --verify` is byte-identical on the full official corpus: `Starfield.esm`,
every `SFBGS*.esm`, `Constellation.esm`, `OldMars.esm`, `BlueprintShips-*.esm`, and
`ShatteredSpace.esm`.

Exit criteria:
- Every corpus file roundtrips byte-identical; a mismatch reports file, record path, and
  offset.
- Subrecords without a schema definition are preserved as opaque bytes, never dropped.
- `docs/record-parity.md` is generated from the corpus (record types, subrecords seen vs
  known, roundtrip status).
- Truncated and bit-flipped corpus inputs return errors, never panic.

## M2: agent surface

"Agents can rip and tear": load-order resolution and the agent-facing commands, over CLI and
MCP. Tasks, lanes (A, B, D, E, F, G, owner) and ordering are in `plan/execution-plan.md`.

- `kit load` from an MO2 profile (read-only) or a plugin list; FormID resolution across
  masters, winning overrides, conflict graph; small/medium plugin forms only as far as a
  permitted source settles them.
- `inspect`, `query` (with the `where` mini-language), `diff` and `conflicts` (port of
  `esmscan.py`) across a load order; localized strings resolved.
- `set` (`FIELD=value`) into a non-localized patch plugin; `validate` (missing masters,
  dangling FormIDs, localized FULL copied as a string ID, stripped FULL/NTRM as in the
  Resupply door, flag/body mismatch, god-mode Papyrus calls); `build` from a minimal TOML
  spec.
- `kit mcp` stdio server (`execute`, `command_line`, `inspect_record`, `query_records`,
  `diff_plugins`, `coverage`), usable from Claude Code.
- Ports: `ba2x.py` -> `ba2` (GNRL + DX10; list, extract, pack), `pexdis.py` -> `pex`
  (`papyrus disasm`; `papyrus compile` wraps Bethesda's compiler, never `-release`), string
  tables (`strings dump`/`merge`).
- Typed records wired into the reader; weapon record types (WEAP, AMMO, PROJ, AMDL, OMOD,
  COBJ, ZOOM) typed as far as the owner's xEdit decision (ADR 0004) allows.
- Smoke test: rename the Old Earth Hunting Rifle to "Old Soviet Marksman Rifle" via `kit set`
  (and `kit build`) -> `kit validate` -> the owner loads it through MO2 and checks the name in
  game.

Exit criteria:
- `kit mcp` works from Claude Code with at least inspect, query, roundtrip and coverage.
- The rename plugin is built and validated; the owner's in-game result is recorded.
- Every catalog command is Implemented or explicitly deferred with a reason.
- `cargo xtask ci` is green and the full corpus roundtrip is still byte-identical.

## M3: CK parity checklist

Black-box the real Creation Kit: open the same plugin, save, diff its output against ours.

- Reproduce CK's ONAM, override, and compression choices on save.
- Rank CK editor features in `docs/ck-parity.md`; viewport explicitly deferred.

## M4+: editor

- egui UI over the command catalog (`crates/ui-egui`, `apps/kit`).
- NIF and material metadata, worldspace data.
- Render viewport (`crates/render`), last.
