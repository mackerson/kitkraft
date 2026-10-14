# ADR 0004: xEdit source policy

- Status: **OPEN (owner decision)**
- Date: 2026-10-14
- Default until decided: **UESP only. Nobody reads xEdit source.**

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
| A      | UESP + corpus only; ambiguous fields stay opaque                                       | Lowest     | Slower  |
| B      | UESP first; xEdit as a tiebreak by a designated reader, logged per field, never copied | Low-medium | Faster  |
| C      | xEdit as a primary reference                                                           | High       | Fastest |

## Recommendation

B, if the owner accepts the risk: one designated agent or person reads xEdit only for fields
UESP and the corpus leave ambiguous, writes a dated entry per field in
`plan/xedit-tiebreaks.md` (field, what UESP says, what the corpus shows, what was decided),
and never pastes structure or names wholesale. Other agents never read xEdit.

## Decision

Pending. Until the owner records a decision here, option A applies.
