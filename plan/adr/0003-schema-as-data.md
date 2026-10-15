# ADR 0003: Record schema as data, Rust by codegen

- Status: Accepted
- Date: 2026-10-14

## Context

Starfield has well over a hundred record types, each with ordered, optional, repeating
subrecords and packed field layouts. Hand-writing a Rust struct and parser per record is
slow, error-prone, and hard to review against the spec. We also need the same definitions at
runtime: coverage reports, `kit query` field paths (`<SIG>.<FieldName>`), validation
messages, and the MCP `inspect_record` output.

## Decision

1. Record definitions live as TOML under `crates/schema/defs/`, one file per record type
   (`WEAP.toml`), written by us from the sources ADR 0005 permits for each fact.
2. `kitkraft-schema` loads and validates definitions and provides a codegen library that turns
   them into Rust source. It does not depend on the container crate.
3. Generated typed records are written into `crates/esm/src/generated/` by
   `cargo xtask codegen` and checked in. `cargo xtask ci` runs `codegen --check` and fails on
   stale output. No `build.rs`: builds stay fast and generated code is reviewable in diffs.
4. Every typed record keeps the subrecords it does not understand as opaque bytes in their
   original order, so writing an untouched record is byte-identical.
5. Each definition, and each field whose layout or name has a different source, names its
   source in the form ADR 0005 gives (`uesp-skyrim:`, `corpus:`, `ck-diff:`, `ck-wiki:`).
   UESP and wiki prose is not copied. (Amended 2026-10-14: there is no UESP Starfield page.)

Illustrative shape (the real format is fixed in M1.8):

```toml
signature = "WEAP"
name = "Weapon"
source = "corpus: subrecord inventory, see docs/record-parity.md"

[[subrecord]]
signature = "EDID"
name = "EditorID"
type = "zstring"

[[subrecord]]
signature = "FULL"
name = "Name"
type = "lstring"          # string ID when the plugin is localized, inline text otherwise

[[subrecord]]
signature = "<SIG>"            # a subrecord seen in the corpus
name = "<Name>"                # from the CK wiki, in our own words
type = "struct"
source = "ck-diff: <observation id>"
fields = [
  { name = "<FieldName>", type = "u8" },   # placeholder, not a real layout
  # fields only where the source is unambiguous; the rest stays opaque
]
```

## Consequences

- Coverage is measurable: definitions vs subrecords seen in the corpus
  (`docs/record-parity.md`).
- Field names become the public query vocabulary; renaming one is a breaking change for
  `kit query` and MCP users.
- Partial definitions are fine. An unknown field run is an opaque byte range, not an error.
