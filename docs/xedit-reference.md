# xEdit reference: pinned revision, license, coverage

Verification record for the owner decision of 2026-10-14 (ADR 0004): upstream xEdit's
Starfield definitions, `Core/wbDefinitionsSF1.pas`, are the canonical implementation
reference for Starfield record schemas, version-pinned, used as source material only.
This file records what was checked on 2026-10-14, and how. It contains no xEdit code or
comments; the counts and line ranges below are our own measurements.

**Status: for owner review.** Lane B does not transcribe anything until the owner has
reviewed this file and ADRs 0004, 0005 (Amendment 2) and 0001 (Amendment 1).

## Repository and pinned revision

Checked through the GitHub REST API (unauthenticated `curl`) on 2026-10-14.

| Item                                   | Value                                                                    |
| -------------------------------------- | ------------------------------------------------------------------------ |
| Repository                             | <https://github.com/TES5Edit/TES5Edit>                                    |
| Default branch                         | `dev-4.1.6` (`master` is stale: last commit 2018-12-10)                  |
| Head of `dev-4.1.6`                    | `9fb016884bec138ea6c7b872cec831537d464c3e` (2026-09-06)                   |
| Last commit touching the file          | `4440381a3cff2174d173489085fcb8c9987ac71a` (2026-09-05)                   |
| File at the pin                        | `Core/wbDefinitionsSF1.pas`, git blob `ace733b52e259e559aed0f4c71edf9e3432fc62b` |
| Size, lines                            | 789,023 bytes, 19,376 lines, ASCII                                       |
| SHA-256 of the file at the pin         | `9f01921ab92b3965d9c747ab11ea0c5d8dd66a9ecf1df62d94f9eb50952346d5`        |
| Latest GitHub release                  | `xedit-4.1.5f`, published 2024-04-28; 1,459 commits behind `dev-4.1.6`    |
| Newest tag                             | `xedit-4.1.5q` (lightweight tag, commit `fd1e3602...`, 2026-06-15, no GitHub release); 220 commits behind `dev-4.1.6`, and 29 commits since then changed the file |
| Branch the starfieldwiki `/CTDA` page links | `dev-4.1.5` (head `9058a794...`, 2024-06-10); the file there is the 788,387-byte version also tagged `xedit-4.1.5` (2023-11-03) |

The 29 changes after `xedit-4.1.5q` include Starfield `OMOD` property decoding and `ARMO`
fields, both in or next to the lane B priority list.

### Recommendation: pin `9fb016884bec138ea6c7b872cec831537d464c3e`

- It is the head of the default branch, where Starfield work happens. Its copy of
  `wbDefinitionsSF1.pas` is byte-identical to the one at `4440381a`, the file's latest
  change, so no Starfield fix is missed.
- It is one repository-wide snapshot: the helper units that file uses
  (`wbDefinitionsCommon.pas`, `wbDefinitionsSignatures.pas`) are read at the same commit if
  they are ever needed (see "Open questions").
- No tag is better. The only formal release (`4.1.5f`, 2024) predates most Starfield
  work; the newest tag (`4.1.5q`) lacks 29 changes to the file; `dev-4.1.5`, the branch the
  `/CTDA` page cites, stopped in 2024.
- A full SHA cannot move. Re-pinning is a deliberate act: amend ADR 0004 with the new SHA,
  update `XEDIT_REVISION` in `crates/schema/src/load.rs`, and re-check every definition
  whose cited line range changed.

Raw file URL at the pin (for whoever transcribes; never commit the file, `*.pas` is
gitignored):
`https://raw.githubusercontent.com/TES5Edit/TES5Edit/9fb016884bec138ea6c7b872cec831537d464c3e/Core/wbDefinitionsSF1.pas`

## License

- The repository's `LICENSE.txt` at the pin; GitHub's license API reports SPDX identifier
  **`MPL-2.0`**, and the file's first line is "Mozilla Public License Version 2.0". The
  definitions file's own header states the same license (MPL 2.0).
- Not "Incompatible With Secondary Licenses": no Exhibit B notice in the license or the
  file header.

What MPL-2.0 means for us (our reading, not legal advice):

1. MPL is file-level copyleft: its obligations attach to Covered Software, meaning MPL files
   and any file that contains MPL-licensed code, and only when such a file is distributed.
2. Facts about a file format (signatures, field order, widths, types, flag bits, enum
   values, FormID target types) are not code; using them to write our own TOML does not
   put MPL code into our files, so those files stay MIT OR Apache-2.0.
3. Copying code, comments or other expression (verbatim or lightly edited) into a KitCraft
   file would make that file Covered Software; that is forbidden (ADR 0004).
4. Running SF1Edit as an external program for differential comparison creates no
   obligation; bundling or redistributing it would, so we do not.
5. Attribution: MPL requires none for facts. We cite file, revision and line ranges anyway,
   as provenance (`xedit:` sources), and `NOTICE` names the reference.

## Coverage of the required record types

Measured on the pinned file with a search for each record's definition call and a
parenthesis match to the end of that call (comments and quoted strings skipped). "Lines" is
the span of the record's own definition; records also use helper definitions declared
earlier in the file or in `wbDefinitionsCommon.pas`, which are not counted. The construct
counts are within that span only.

| Record | Defined | Lines (span)        | Lines | Unions | Arrays | FormID refs | Localized strings |
| ------ | ------- | ------------------- | ----- | ------ | ------ | ----------- | ----------------- |
| TES4   | yes     | L9045-L9083         | 39    | 0      | 4      | 2           | 0                 |
| GRUP   | no (see below) | L19130-L19345 group order only | 216 | - | - | - | - |
| GMST   | yes     | L12355-L12365       | 11    | 1      | 0      | 0           | 1                 |
| GLOB   | yes     | L12321-L12352       | 32    | 0      | 0      | 0           | 0                 |
| KYWD   | yes     | L13297-L13311       | 15    | 0      | 0      | 1           | 0                 |
| WEAP   | yes     | L18594-L18833       | 240   | 0      | 0      | 18          | 1                 |
| AMMO   | yes     | L9412-L9470         | 59    | 0      | 0      | 1           | 1                 |
| PROJ   | yes     | L16228-L16325       | 98    | 0      | 0      | 8           | 0                 |
| AMDL   | yes     | L9387-L9409         | 23    | 0      | 0      | 0           | 0                 |
| OMOD   | yes     | L15429-L15476       | 48    | 0      | 4      | 8           | 0                 |
| COBJ   | yes     | L10886-L10950       | 65    | 0      | 2      | 10          | 0                 |
| ZOOM   | yes     | L19109-L19128       | 20    | 0      | 0      | 1           | 0                 |
| LVLI   | yes     | L14013-L14074       | 62    | 0      | 1      | 4           | 1                 |
| NPC_   | yes     | L14993-L15426       | 434   | 1      | 11     | 47          | 2                 |
| CELL   | yes     | L10589-L10743       | 155   | 0      | 2      | 11          | 0                 |
| REFR   | yes (placed-reference form) | L8530-L9041 | 512 | 2 | 10 | 29          | 1                 |
| QUST   | yes     | L16339-L16514       | 176   | 1      | 10     | 14          | 6                 |
| INNR   | yes     | L12887-L13144       | 258   | 1      | 2      | 0           | 1                 |
| FLST   | yes     | L12024-L12053       | 30    | 0      | 1      | 1           | 0                 |

- All 18 record types are defined. The file defines 194 records with the plain record call
  and 3 with the placed-reference variant (`REFR` among them).
- **GRUP** is not a record definition anywhere in the file: group parsing lives in xEdit's
  container code (`wbImplementation.pas`, not consulted). The file only lists the top-level
  group order (192 entries, L19130-L19345). Our GRUP and record headers stay on the
  container sources (ADR 0005 Amendment 2, level 1).
- Spans are short for `OMOD` and `GLOB` because their bodies lean on helpers declared
  elsewhere in the file (for `OMOD`, the property and include lists); a transcription cites
  those ranges in `evidence` too.

## Pascal patterns and their TOML counterparts

For the lane B transcription brief. Counts are occurrences in the pinned file. Descriptions
are ours.

| xEdit construct (family)                          | Count      | What it describes                                             | Our TOML (`crates/schema/defs/README.md`)                       |
| ------------------------------------------------- | ---------- | ------------------------------------------------------------- | ---------------------------------------------------------------- |
| `wbRecord`, `wbRefRecord`                         | 194, 3     | A record type: signature, label, record flags, member list     | `<SIG>.toml`, `kind = "record"`, `flags`                         |
| `wbRStruct`, `wbRStructSK`                        | 193, 38    | Consecutive subrecords that belong together                    | `group = "Name"` member with `[[member.member]]`                 |
| `wbRArray`, `wbRArrayS`                           | 98, 52     | A subrecord or group that repeats                              | `card = "repeated"`                                              |
| `wbStruct`, `wbStructSK`                          | 479, 72    | Fields in order inside one payload                             | `type = "struct"`, `field = [...]`                               |
| `wbArray`, `wbArrayS`                             | 138, 62    | Elements inside a payload: fixed count, counted, or to the end | `type = "array"` with `count = N`, `count_from`, or `"rest"`; an inline count prefix becomes an explicit integer field plus `count_from` |
| `wbInteger` with `itU8` ... `itS64`               | 1,114      | Integer of a given width and signedness                        | `u8` ... `i64`                                                   |
| `wbFlags`, `wbEnum` (as an integer's formatter)   | 246, 168   | Named bits or named values of an integer                       | `type = "flags"` / `"enum"` with `base`, `bits` / `values`        |
| `wbFloat`                                         | 1,049      | 32-bit float (unless stated otherwise)                         | `f32`                                                            |
| `wbFormIDCk`, `wbFormIDCkNoReach`, `wbFormID`     | 784, 61, 16 | FormID, with or without allowed target signatures             | `formid` with `targets = [...]`                                  |
| `wbString`, `wbStringKC`                          | 278, 1     | Text; NUL-terminated unless a size or length prefix is given   | `zstring`, or `wstring`/`bstring` for length-prefixed            |
| `wbLString`, `wbLStringKC`                        | 14, 44     | Localized text (string-table ID in localized plugins)          | `lstring`                                                        |
| `wbByteArray`, `wbUnknown`, `wbUnused`            | 33, 108, 93 | Raw, unknown or unused bytes                                  | `opaque` (with `size` when fixed)                                |
| `wbEmpty`                                         | 91         | Zero-length marker subrecord                                   | `empty`                                                          |
| `wbUnion`, `wbRUnion`, `wbFormaterUnion` + a decider function | 39, 18, 1 | Layout chosen at load time by a decider           | Not expressible yet: stays `opaque` (README "Not supported")     |
| Shared helpers (`wbVec3`, EditorID, name, keyword-list helpers from `wbDefinitionsCommon.pas`) | e.g. `wbVec3` 31 | Reused fragments | `defs/common/` shared groups and `ref:` types |
| Conflict priorities (`cpNormal`, `cpIgnore`, `cpBenign`), display and callback hooks (`SetToStr`, summary keys, after-load handlers) | - | xEdit's UI and conflict-detection behaviour | Not transcribed. A hook that implies a format fact (a unit, a scale) becomes a `note` in our words |
| `wbAddGroupOrder`                                 | 192        | Top-level group order in a plugin                              | Writer group order; confirm against the corpus (lane A)          |

## Transcription rules (summary; ADRs 0004 and 0005 are normative)

- Read the file at the pinned SHA only, from a local copy outside the repository; delete
  it afterwards. Never commit `.pas` files (gitignored).
- Write each definition from scratch in our TOML structure. No code, comments, or prose
  copied; names follow `defs/README.md` (our PascalCase vocabulary, CK wiki names where
  they exist; an xEdit label that is an `Unknown` placeholder never becomes a name).
- Cite `source = "xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e#L<start>-L<end>"`
  (helper ranges in `evidence`, same form) and set `confidence`: `reference` (xEdit only),
  `corpus` (sizes, counts and order confirmed on the 14 official ESMs), `ck-diff` (confirmed
  by CK differential observation), `verified` (corpus and ck-diff).
- Never substitute a Skyrim (or Fallout 4) definition for a Starfield record body.
- Unions stay opaque until the format grows a construct for them.
- Every typed layout is checked against every occurrence in the corpus before it leaves
  `reference`.

## Open questions and things not verified

- `wbDefinitionsCommon.pas` holds helpers the Starfield file uses (EditorID, names,
  keywords, conditions, script data). The owner decision names `wbDefinitionsSF1.pas`;
  whether the common helpers may be cited the same way is for the owner. Until then the
  loader accepts only `wbDefinitionsSF1.pas`, and members built from those helpers stay on
  their current sources or opaque.
- Spans are mechanical (parenthesis matching); a transcriber confirms each range when
  citing it.
- SF1Edit (the program) is not installed on this machine; installing a build that matches
  the pin is an M2 task (ADR 0005 Amendment 2). Release `4.1.5f` is 1,459 commits older
  than the pin, so differential results against it must note the version skew.
- The MPL summary above is an engineering reading, not legal advice.
