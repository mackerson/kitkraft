# ADR 0004: xEdit source policy

- Status: **OPEN (owner decision)**
- Date: 2026-10-14
- Default until decided: **option A. Nobody reads xEdit source.**

## Context

xEdit's Starfield definitions (`wbDefinitionsSF1.pas`, about 789 KB) are the most complete
public description of Starfield records. xEdit is MPL-2.0: file-level copyleft, weaker than
GPL, and MPL code can coexist with MIT/Apache code if MPL files stay MPL. But we would not be
including xEdit files; we would be writing our own definitions after reading theirs, which
risks our TOML being a derivative of their definitions, and it blurs the clean-room story the
project depends on.

UESP is CC BY-SA and less complete. Our corpus (every official ESM) can settle many
ambiguities statistically.

## Options

| Option | Description                                                                            | Risk       | Speed   |
| ------ | -------------------------------------------------------------------------------------- | ---------- | ------- |
| A      | ADR 0005 sources only (UESP Skyrim container, corpus, CK observation, CK wiki)         | Lowest     | Slowest |
| B      | UESP first; xEdit as a tiebreak by a designated reader, logged per field, never copied | Low-medium | Faster  |
| C      | xEdit as a primary reference                                                           | High       | Fastest |

## Recommendation

B, if the owner accepts the risk: one designated agent or person reads xEdit only for fields
UESP and the corpus leave ambiguous, writes a dated entry per field in
`plan/xedit-tiebreaks.md` (field, what UESP says, what the corpus shows, what was decided),
and never pastes structure or names wholesale. Other agents never read xEdit.

## Decision

Pending. Until the owner records a decision here, option A applies.

## Update 2026-10-14

Status unchanged: **OPEN, owner decision.** The owner has been asked.

The context above assumed UESP documents Starfield records. It does not: there is no UESP
Starfield format page, only the Skyrim pages for the shared container
(`plan/adr/0005-spec-sources-per-fact.md`). That changes what option A costs. Under option A,
**every Starfield-specific field layout comes only from CK differential observation** (one
edit in the CK's UI, save, byte diff, cross-check with the corpus), with names from the CK
wiki. That is correct and clean, but slow: one observation session per field or flag, and
fields the CK UI does not expose stay opaque. Container work (M1 roundtrip) is not affected.
The recommendation is unchanged; the speed column for option A now reads `Slowest`.
