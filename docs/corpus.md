# Roundtrip corpus

The M1 gate is a byte-identical rewrite of every official Starfield ESM. Those files are
Bethesda's, so the corpus is **referenced in place and never copied into the repository**.

## Location

Tests and tools find the game data through `KITCRAFT_DATA_DIR`:

```powershell
$env:KITCRAFT_DATA_DIR = "C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data"
```

When the variable is unset or the directory is missing, corpus tests print one skip line and
pass, so `cargo xtask ci` works on machines without the game. The data directory is read-only
to us: nothing is ever written there. Roundtrip output, when written at all, goes to a temp
directory or `corpus/out/` (gitignored).

## Files

`corpus/manifest.toml` pins each file by size and SHA-256 as installed on the dev box on
2026-10-14. A game update changes these; tests report a hash mismatch as "corpus changed",
not as a roundtrip failure, and the manifest is updated in its own commit.

| File                         | Size (bytes)  | In PLAN.md M1 list        |
| ---------------------------- | ------------- | ------------------------- |
| Starfield.esm                | 1,457,098,709 | yes                       |
| Constellation.esm            | 39,210        | yes                       |
| OldMars.esm                  | 24,358        | yes                       |
| BlueprintShips-Starfield.esm | 303,656,303   | yes                       |
| BlueprintShips-SFBGS050.esm  | 16,136,720    | yes                       |
| SFBGS003.esm                 | 33,279,036    | yes                       |
| SFBGS004.esm                 | 254,945       | yes                       |
| SFBGS006.esm                 | 7,315,458     | yes                       |
| SFBGS007.esm                 | 27,312        | yes                       |
| SFBGS008.esm                 | 11,302,160    | yes                       |
| SFBGS00D.esm                 | 102,054,268   | yes                       |
| SFBGS047.esm                 | 472,022       | yes ("all SFBGS*.esm")    |
| SFBGS050.esm                 | 106,017,249   | yes ("all SFBGS*.esm")    |
| ShatteredSpace.esm           | 501,394,621   | M0 "every ESM under Data" |

Total about 2.54 GB. Fast tests use the three smallest (`OldMars.esm`, `SFBGS007.esm`,
`Constellation.esm`); the full run is `#[ignore]` and runs in release:

```powershell
cargo test -p kitcraft-esm --release -- --ignored corpus
```

## How it is used

1. **Roundtrip (M1):** parse each file, write it back, compare byte for byte. A mismatch is
   reported as file, absolute offset, and group/record path.
2. **Coverage (M1.14):** count record types and subrecord signatures across the corpus and
   compare with schema definitions; output is `docs/record-parity.md`.
3. **Hostile input (M1.7):** truncations and seeded bit flips of the small files must yield
   errors, never panics.
4. **Oracle (optional):** Spriggit, run as an external binary, can turn a file into YAML for
   spot comparisons with our decoded view. Its source is never read.

## Run log

Record each full run here: date, commit, files identical/total, wall time, peak memory.

| Date | Commit | Identical | Wall time | Peak memory | Notes |
| ---- | ------ | --------- | --------- | ----------- | ----- |
