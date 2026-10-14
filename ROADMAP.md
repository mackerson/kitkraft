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

Load-order resolution and the first useful commands, over CLI and MCP.

- `kit load`, `inspect`, `query`, `set`, `diff`, `validate`; `kit mcp` stdio server.
- Ports: `ba2x.py` -> `ba2`, `pexdis.py` -> `pex`, `esmscan.py` -> `doc`/`engine`.
- Validation rules seeded from known bugs: localized FULL copied as a string ID, stripped
  FULL/NTRM (the Resupply door), god-mode Papyrus calls.
- Smoke test: rename one WEAP via `kit set` -> `kit validate` -> launch via MO2 -> name shows
  in game.

## M3: CK parity checklist

Black-box the real Creation Kit: open the same plugin, save, diff its output against ours.

- Reproduce CK's ONAM, override, and compression choices on save.
- Rank CK editor features in `docs/ck-parity.md`; viewport explicitly deferred.

## M4+: editor

- egui UI over the command catalog (`crates/ui-egui`, `apps/kit`).
- NIF and material metadata, worldspace data.
- Render viewport (`crates/render`), last.
