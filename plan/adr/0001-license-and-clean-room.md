# ADR 0001: License and clean-room rules

- Status: Accepted (license pending owner confirmation, see "Open" below)
- Date: 2026-10-14

## Context

KitCraft re-implements a proprietary tool (Bethesda's Creation Kit) and proprietary formats.
The craft-suite projects it is modelled on stay credible, and legally safe to contribute to, by
being permissively licensed and strictly clean-room. The Bethesda modding ecosystem's most
complete format code (Mutagen, Spriggit) is GPL-3.0; xEdit is MPL-2.0. Reading copyleft code
and then writing "our own" version of the same thing makes the result arguably derivative.

## Decision

1. **License:** MIT OR Apache-2.0, at the user's option (`LICENSE-MIT`, `LICENSE-APACHE`,
   `NOTICE`). Contributions are accepted under the same dual license.
2. **Specification sources:** one permitted source per kind of fact, as listed in
   `plan/adr/0005-spec-sources-per-fact.md` (amended 2026-10-14; UESP has no Starfield format
   page). In short:
   1. The UESP Skyrim Mod File Format pages, for the shared record container only.
   2. Our own inspection (hex dumps, statistics) of official files on our own machines.
   3. Black-box differential observation of `CreationKit.exe`: run it, change one field in a
      synthetic plugin, save, diff the bytes. This is the only source for Starfield field
      layouts.
   4. The public Creation Kit wiki, for field names and semantics.
3. **Forbidden:**
   - Disassembling, decompiling, debugging, or memory-dumping `CreationKit.exe` or
     `Starfield.exe`, including with REA, Ghidra, IDA, or x64dbg.
   - Reading Mutagen, Spriggit, or any GPL/LGPL/AGPL code that handles Bethesda formats.
   - Reading xEdit source until ADR 0004 says otherwise.
   - Copying UESP prose (CC BY-SA) into code, schema files, or docs. Facts (signatures,
     sizes, offsets, types, flag values) are recorded in our own words and structure.
   - Committing any Bethesda data or asset, including CK screenshots.
4. **Allowed:** running Spriggit (or any other third-party tool) as an external binary;
   porting our own Python tools in `C:\Modding\scripts`; running REA-style analysis on our
   own binaries (crash triage).
   - Third-party tool output may be used **for byte-level inventory facts only**: record
     counts, FormIDs, EditorIDs, subrecord signatures and subrecord sizes.
   - Its field names, YAML/JSON key structure, and struct layouts must not be used to author
     or check anything in `crates/schema/defs/` (or generated code, or docs). Those come only
     from the sources in ADR 0005.
   - Spriggit is not an oracle for roundtrip correctness: it lost data on 2 of 5 real mods
     tested (`docs/gaps.md`).
5. Every non-code third-party asset is listed in `ATTRIBUTION.md` before commit.

### Provenance of `C:\Modding\scripts`

The Python tools in `C:\Modding\scripts` that the `ba2`, `pex` and `doc` crates port
(`ba2x.py`, `pexdis.py`, `esmscan.py`) were written in-session from inspection of the files
themselves, without xEdit or Mutagen source open. They are our own code and may be ported
freely.

## Consequences

- Most Starfield subrecords have no public layout at all. We establish them by CK differential
  observation and corpus statistics (ADR 0005) and mark the remainder opaque, rather than
  consulting other code.
- An agent that has read forbidden code must say so and stay out of that area.
- Opaque-but-preserved data is a first-class outcome, not a failure.

## Open

- The owner confirms the license (PLAN.md open decision 4).
