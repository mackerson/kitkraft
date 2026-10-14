# ADR 0004: xEdit source policy

- Status: **DECIDED 2026-10-14 by the owner: option B, as set out in "Decision
  (2026-10-14)" below** (xEdit's `wbDefinitionsSF1.pas`, version-pinned, is the
  record-definition reference)
- Date: 2026-10-14
- Pinned revision: `9fb016884bec138ea6c7b872cec831537d464c3e` (TES5Edit/TES5Edit,
  `Core/wbDefinitionsSF1.pas`; verification in `docs/xedit-reference.md`)
- Amends: ADR 0001 (Amendment 1), ADR 0005 (Amendment 2)
- The Context, Options, Recommendation and Update sections are kept as written before the
  decision; where they differ from the Decision section, the Decision section wins.

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

See "Decision (2026-10-14)" at the end of this ADR. Option A no longer applies.

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

## Decision (2026-10-14)

Decided by the owner on 2026-10-14. In the owner's words, the absence of a UESP Starfield
plugin-format page is a **source-policy blocker, not an absence-of-specification blocker**:
the specification exists, as upstream xEdit's Starfield definitions.

### What is decided

1. **Option B, amended.** Upstream xEdit (SF1Edit) `Core/wbDefinitionsSF1.pas` is the
   canonical implementation reference for Starfield record schemas. It is not limited to
   tiebreaks and not limited to one designated reader: any agent transcribing definitions
   may read it, under the constraints below. Per-definition provenance in the schema
   replaces the per-field `plan/xedit-tiebreaks.md` log proposed in the Recommendation.
2. **Source hierarchy** (ADR 0005 Amendment 2 is normative):
   1. Container layer: existing Bethesda-family documentation (UESP's Skyrim pages) with
      Starfield-specific validation against the corpus.
   2. Record-definition layer: the version-pinned upstream SF1Edit definitions.
   3. Empirical validation: Starfield plugin fixtures (the 14 official ESMs, our own
      synthetic plugins), CK differential observation, and differential comparison
      against SF1Edit run as an external program.
   4. Secondary references: UESP's Starfield pages and other community documentation.
3. **Pinned revision:** `9fb016884bec138ea6c7b872cec831537d464c3e`, the head of the default
   branch `dev-4.1.6` on 2026-10-14. Its `wbDefinitionsSF1.pas` (789,023 bytes, 19,376
   lines, git blob `ace733b5...`) is identical to the file at its latest change
   (`4440381a`, 2026-09-05). No tag is a better pin: the only formal release (`4.1.5f`,
   2024) predates most Starfield work, the newest tag (`4.1.5q`) lacks 29 later changes to
   the file, and `dev-4.1.5` (cited by the starfieldwiki `/CTDA` page) stopped in 2024.
   Details in `docs/xedit-reference.md`. Changing the pin needs an amendment to this ADR,
   an update of `XEDIT_REVISION` in `crates/schema/src/load.rs`, and a re-check of every
   definition whose cited lines moved.
4. **License:** MPL-2.0, confirmed at the pin. Facts taken from the file carry no MPL
   obligation; we cite provenance anyway, and `NOTICE` names the reference.

### Constraints

- **Source material only.** The Pascal file is read to learn facts (signatures, member
  order, widths, types, flag bits, enum values, FormID targets). Nothing in KitCraft
  depends on it at build or run time: the runtime parser is never coupled to Pascal, never
  generated from it, and never loads it.
- **No reproduction of xEdit's internal architecture.** Our model stays the TOML format in
  `crates/schema/defs/README.md` and the crates in AGENTS.md; we do not mirror xEdit's
  element interfaces, deciders, conflict priorities, or callbacks.
- **No verbatim copying** of code, comments, or other prose, not even lightly edited.
  Definitions are written independently in our TOML structure. Short functional labels may
  coincide with xEdit's where they describe the same thing; placeholder labels
  ("Unknown") never become names, and the CK wiki remains the preferred naming source.
- **Per-definition provenance**, enforced by the schema loader:
  `source = "xedit: wbDefinitionsSF1.pas@<40-hex sha>#L<start>-L<end>"`. Helper ranges go
  in `evidence` in the same form. The SHA must be the pinned revision. (The owner's
  shorthand was `xedit:wbDefinitionsSF1.pas@<sha>#L<start>-L<end>`; the space after the
  colon matches every other source form.)
- **Confidence**, a `confidence` key required on every definition that cites xEdit and
  optional elsewhere:
  - `reference`: from xEdit only, not yet confirmed.
  - `corpus`: sizes, counts and order confirmed against the 14 official ESMs.
  - `ck-diff`: confirmed by CK differential observation.
  - `verified`: both `corpus` and `ck-diff`.

  The loader checks that the claimed level has matching citations (`reference` needs an
  `xedit:` citation, `corpus` a `corpus:` one, `ck-diff` a `ck-diff:` one, `verified` both
  of the last two).
- **No silent Skyrim substitution.** Skyrim (or Fallout 4) definitions are never used for
  Starfield record bodies; they may describe the shared container only, and each such use
  carries `confidence = "corpus"` or better (ADR 0005 Amendment 2; the loader enforces it
  for `uesp-skyrim:` citations).
- **Unions** (layouts chosen by a decider) stay opaque until the format can express them.
- The downloaded `.pas` file lives outside the repository and is deleted after use; `*.pas`
  is gitignored.

### Still forbidden

- **Mutagen and Spriggit** source, generated code, record XML, and Spriggit's output field
  names and key structure: GPL-3.0, unchanged (ADR 0001, ADR 0005 §3).
- Any other GPL/LGPL/AGPL code that handles Bethesda formats.
- Disassembly, decompilation, debugging or memory inspection of `CreationKit.exe` or
  `Starfield.exe` (ADR 0001).

### Secondary sources

- UESP's Starfield pages per ADR 0005 Amendment 1, now level 4 (secondary).
- The starfieldwiki `/CTDA` page is no longer forbidden on its own: its stated source
  (xEdit) is now permitted. Cite xEdit directly instead. The loader keeps rejecting
  `uesp-starfield:` citations of `/CTDA`, so conditions are always cited at the pinned
  revision rather than through a page that follows the stale `dev-4.1.5` branch.

### Consequences

- Lane B can type Starfield record layouts without waiting for one CK observation per
  field. CK differential observation and the corpus move from "only source" to validation
  that raises `confidence`.
- A `confidence = "reference"` definition is usable but must be reported as unconfirmed
  wherever its values are shown (a `limitations[]` entry, `docs/mcp.md`) until the corpus
  confirms it.
- Lane B is paused until the owner has reviewed `docs/xedit-reference.md` and these ADRs;
  then round 3 transcribes, in order: WEAP, AMMO, PROJ, AMDL, OMOD, COBJ, ZOOM; then GMST,
  GLOB, KYWD typed; then LVLI, FLST, INNR (`plan/STATUS.md`).
- `wbDefinitionsCommon.pas` (helpers the Starfield file uses) is not covered by this
  decision; the loader accepts only `wbDefinitionsSF1.pas` until the owner extends it.
