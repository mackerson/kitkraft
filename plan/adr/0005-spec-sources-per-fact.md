# ADR 0005: Specification source per fact

- Status: Accepted
- Date: 2026-10-14
- Amends: ADR 0001 §2 (specification sources), ADR 0003 §5 (`source` key)
- Amended: 2026-10-14 by Amendment 1 below (UESP's Starfield pages exist at
  starfieldwiki.net; `uesp-starfield:` source form; `/CTDA` forbidden). The Context section
  is kept as written; where it says UESP has no Starfield page, Amendment 1 corrects it.
- Amended: 2026-10-14 by Amendment 2 below (owner decision in ADR 0004: four-level source
  hierarchy with version-pinned xEdit definitions; `xedit:` source form and `confidence`).
  Amendment 2 replaces the §1 per-fact table and the xEdit entry in §3; §2 (CK
  differential observation) stays, as a validation method.
- Amended: 2026-10-14 by Amendment 3 below (owner-supplied prior-art dossier adopted as an
  evidence backlog; `docs/source-registry.md`; evidence grades A-D; general source rule;
  `wbDefinitionsCommon.pas` now at level 2 per ADR 0004's owner approval).

## Context

Earlier documents named "the UESP Starfield Mod File Format pages" as the primary spec. A
review on 2026-10-14 established that **UESP has no Starfield mod file format page**: every
`https://en.uesp.net/wiki/Starfield_Mod:...` URL returns 404 and the namespace does not exist.

What UESP does document is the record container that Bethesda's engines share, on the Skyrim
pages:

- <https://en.uesp.net/wiki/Skyrim_Mod:Mod_File_Format>
- <https://en.uesp.net/wiki/Skyrim_Mod:Mod_File_Format/TES4>

Everything Starfield-specific (plugin-size flags, FormID layout of light and medium plugins,
the form version, every record's field layout) is not available from any source we are
permitted to read. It has to be established by us, from the official files and from the real
Creation Kit observed as a black box.

So "the spec" is not one document. Each fact has exactly one kind of permitted source, and the
schema says which one it used.

## Decision

### 1. Permitted source per fact

| Fact                                                                                                                                                                       | Permitted source                                                         | `source` key form           |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | --------------------------- |
| Record header: 24 bytes = signature, `u32` data size, `u32` flags, `u32` FormID, `u16` timestamp, `u16` version control info, `u16` form version, `u16` unknown            | UESP `Skyrim_Mod:Mod_File_Format`, confirmed against the corpus          | `uesp-skyrim: <page>`       |
| GRUP header: 24 bytes, size includes the header                                                                                                                            | UESP `Skyrim_Mod:Mod_File_Format`, confirmed against the corpus          | `uesp-skyrim: <page>`       |
| Subrecord header: 6 bytes (signature, `u16` size); `XXXX` carries a `u32` size for the next subrecord                                                                      | UESP `Skyrim_Mod:Mod_File_Format`, confirmed against the corpus          | `uesp-skyrim: <page>`       |
| Compressed flag `0x00040000`: `u32` decompressed size, then a zlib stream                                                                                                  | UESP `Skyrim_Mod:Mod_File_Format`, confirmed against the corpus          | `uesp-skyrim: <page>`       |
| TES4 flags: master (ESM) `0x1`, localized `0x80`                                                                                                                           | UESP `Skyrim_Mod:Mod_File_Format/TES4`, confirmed against the corpus     | `uesp-skyrim: <page>`       |
| Inventory: record types, record counts, FormIDs, EditorIDs, subrecord signatures, subrecord sizes and their distributions, TES4 flag values and form versions as they occur | Corpus statistics (our own reads of official files on our machine)       | `corpus: <evidence note>`  |
| Starfield-only header facts: small/medium/light plugin flag bits, `FE`/`FD` FormID layout, meaning of the form version                                                     | CK differential observation, cross-checked against corpus statistics     | `ck-diff: <observation id>` |
| Starfield-specific field layouts inside subrecords (for example any `WEAP` subrecord)                                                                                      | CK differential observation, cross-checked against corpus statistics     | `ck-diff: <observation id>` |
| Field names and semantics (what a value means to the game)                                                                                                                 | The public Creation Kit wiki (creationkit.com), names in our own words   | `ck-wiki: <page title>`     |
| Anything none of the above settles                                                                                                                                         | Nothing. The bytes stay opaque and are preserved verbatim                | (no definition)             |

Rules that go with the table:

- A UESP Skyrim fact is a hypothesis for Starfield until the corpus agrees with it on every
  occurrence. The corpus is the tiebreaker.
- A definition may cite more than one source (layout from `ck-diff`, name from `ck-wiki`), but
  each field's layout must come from a row above that permits layouts.
- Facts are recorded in our own words and structure. UESP prose (CC BY-SA) and CK wiki prose
  are never copied into code, schema files, or docs.

### 2. CK differential observation, step by step

The Creation Kit is run as a black box (ADR 0001): we change one thing through its UI and look
only at the file it writes.

1. **Synthetic plugin.** In the CK (launched through MO2), create a new plugin whose only master
   is `Starfield.esm`, and create one new record of the type under study. Save it as
   `observe/<yyyy-mm-dd>-<topic>/base.esp`. Keep a copy; never edit it again.
2. **One edit.** Reopen `base.esp`, open the record's editor dialog, and change exactly one
   field. Use a distinctive sentinel value (for example `1234.5` for a float, `4321` for an
   integer, a single checkbox for a flag). Save as `changed-<field>.esp` in the same folder.
3. **Second value.** Repeat step 2 from `base.esp` with a different sentinel for the same field,
   so a single coincidental byte match cannot be mistaken for the field. For flag fields toggle
   each checkbox on its own.
4. **Byte diff.** Diff `base.esp` against each `changed-*.esp` (decompress the record first if
   the compressed flag is set). The differing bytes give the subrecord signature, the offset
   inside the subrecord, the width, and, from the sentinel, the encoding. A change in record or
   subrecord size, or a subrecord appearing only when a value is non-default, is recorded too.
5. **Cross-check with the corpus.** The inferred layout must fit every occurrence of that
   subrecord in the official files (size consistent, values plausible). If it does not, the
   field stays opaque and the conflict is noted.
6. **Record the finding.** Add a dated entry with an observation id to
   `docs/ck-observations.md` (created with the first observation): record type, the CK dialog
   and field label as shown in the UI, subrecord signature, offset, width, type, the sentinel
   values and the bytes observed, the CK version, and SHA-256 of the base and changed files.
   Offsets and our own sentinel bytes are the evidence; the plugin files themselves stay in
   `observe/`.
7. **Screenshots** of the dialog before and after each edit go to `observe/` only. `observe/`
   is gitignored; no CK screenshot or CK-saved file is ever committed.
8. **Write the definition** in `crates/schema/defs/` with `source = "ck-diff: <id>"`, and a
   field name from the CK wiki (`ck-wiki: <page>`) or, if the wiki is silent, the UI label in our
   own words.

This is slower than reading someone else's definitions. That is the price of the clean room.

### 3. Forbidden sources

Never used to author or check anything in `crates/schema/defs/`, generated code, or docs:

- **Mutagen**: source code, generated code, and its record XML definitions (GPL-3.0).
- **Spriggit**: source code (GPL-3.0), and the **field names, YAML key structure, and struct
  layouts in its output**. Spriggit output may be used only for byte-level inventory facts
  (record counts, FormIDs, EditorIDs, subrecord signatures and sizes); see ADR 0001 §4.
- **xEdit**: source code, including `wbDefinitionsSF1.pas` (MPL-2.0), unless and until ADR 0004
  is decided otherwise by the owner, and then only as that decision allows.
- Summaries, tables, or field lists derived from any of the above (for example the WEAP field
  list that was in `PLAN.md` before this ADR). This includes the UESP Starfield page
  `Starfield_Mod:Mod_File_Format/CTDA`, which says it is derived from xEdit (added by
  Amendment 1), and any other page whose own attribution names one of these tools.
- An agent's own recall of Starfield field layouts. If a layout cannot be cited to a permitted
  source in the table above, it is not written down.
- Disassembly, decompilation, debugging, or memory inspection of `CreationKit.exe` or
  `Starfield.exe` (ADR 0001 §3).

## Consequences

- Every citation of "UESP Starfield" in the plan and docs now points here.
- Container work (lane A, M1) is unaffected: everything it needs is in the UESP Skyrim rows
  and the corpus.
- Schema work (lane B) can list subrecords from the corpus immediately; typed field layouts
  for Starfield-only records wait for CK observations. Until then those subrecords are opaque,
  which keeps the roundtrip byte-identical.
- `docs/ck-observations.md` becomes the evidence log for every Starfield-specific layout.

## Amendment 1 (2026-10-14): the UESP Starfield pages

### Finding

A survey on 2026-10-14 found that UESP's Starfield wiki does exist, on its own host:

- Host <https://starfieldwiki.net>, namespace "Starfield Mod" (namespace id 106), content
  licensed CC BY-SA 4.0. The `en.uesp.net/wiki/Starfield_Mod:` URLs checked earlier return
  404 because the namespace is not served there.
- 23 pages exist under `Starfield_Mod:Mod_File_Format`: the main index, and:

| Page                    | Content                                                                                   | Status under this ADR                                   |
| ----------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `/Raw_Data`             | A subrecord census of `Starfield.esm` (676 KB): subrecord names, counts, min/max/avg sizes; no layouts | Allowed, as an inventory cross-check only (below) |
| `/CTDA`                 | Condition data; its own attribution says it is derived from xEdit                          | **Forbidden** (§3)                                      |
| `/GMST`                 | Game setting values                                                                        | Allowed                                                 |
| `/PERK`                 | Despite its title, documents `OMOD`: typed `DATA`, include and property layouts            | Available, **not used** until the owner decides OMOD scope |
| `/AMMO`                 | Thin: `EDID`, `FULL`, an 8-byte `DATA`, `DESC`, `MODL`, `ONAM`                            | Allowed                                                 |
| `/EQUP`                 | The `EQUP` record                                                                          | Allowed                                                 |
| Stubs: `/ALCH` `/ARMO` `/AVIF` `/BOIM` `/BOOK` `/CELL` `/CLAS` `/ENCH` `/FACT` `/FLOR` `/GBFM` `/LCTN` `/LSCR` `/MGEF` `/TERM` `/TMLM` | Little or no content | Allowed for whatever they state |

- There is no page for `TES4`, `WEAP`, `PROJ`, `AMDL`, `COBJ`, `ZOOM`, or `OMOD` under its
  own title. Those stay on the sources in §1 (UESP Skyrim container pages, corpus, CK
  differential observation, CK wiki).

### Decision

1. New source form `uesp-starfield: <page URL>`, with URLs of the form
   `https://starfieldwiki.net/wiki/Starfield_Mod:Mod_File_Format/<Page>`. The schema loader
   accepts it and rejects a citation of `/CTDA`.
2. A `uesp-starfield` page is treated like a `uesp-skyrim` page: the structured facts it
   states (subrecord order, field widths and names for its record type) are a hypothesis
   until the corpus agrees on every occurrence, and the corpus is the tiebreaker. Facts are
   recorded in our own words and structure; page prose (CC BY-SA) is never copied.
3. `/Raw_Data` is an allowed cross-check for inventory facts (signatures, counts, size
   ranges). Our own corpus scan stays the inventory source cited in definitions: the census
   describes one version of `Starfield.esm` and gives no order or layouts.
4. Before citing any page, check its attribution. A page that says it is derived from
   xEdit, Mutagen, Spriggit, or another source §3 forbids is forbidden too, like `/CTDA`.
5. The `OMOD` layout on `/PERK` is listed as available but is not used until the owner
   decides whether OMOD is in scope.

| Fact                                                                 | Permitted source                                          | `source` key form              |
| -------------------------------------------------------------------- | --------------------------------------------------------- | ------------------------------ |
| Record layouts stated on a UESP Starfield page (except `/CTDA`, and `/PERK` until decided) | UESP `Starfield_Mod:Mod_File_Format/<Page>`, confirmed against the corpus | `uesp-starfield: <page URL>` |
| Inventory cross-check                                                | UESP `Starfield_Mod:Mod_File_Format/Raw_Data`, alongside our own corpus scan | `uesp-starfield: <page URL>` in `evidence` |

### Consequences

- AGENTS.md ("Clean room") still says UESP has no Starfield format page; it should point
  here (requested in `plan/STATUS.md`).
- No definition cites a `uesp-starfield` page yet; current definitions remain corpus
  inventories and UESP Skyrim container facts.

## Amendment 2 (2026-10-14): the owner's source hierarchy

### Why

ADR 0004 was decided by the owner on 2026-10-14: the missing UESP Starfield format page
was a source-policy blocker, not a missing specification. Upstream xEdit's
`Core/wbDefinitionsSF1.pas`, pinned at `9fb016884bec138ea6c7b872cec831537d464c3e`, is now
the reference for Starfield record definitions (constraints in ADR 0004; verification in
`docs/xedit-reference.md`). This amendment replaces the §1 table ("one permitted source
per fact") with the owner's four levels. A fact may now have several sources; the level
says which wins, and `confidence` says how far it has been checked.

### 1. Source hierarchy (replaces the §1 table)

| Level | Layer                 | Source                                                                                   | `source` / `evidence` form |
| ----- | --------------------- | ---------------------------------------------------------------------------------------- | -------------------------- |
| 1     | Container             | Bethesda-family documentation, chiefly UESP's Skyrim pages (record, group and subrecord headers, `XXXX`, compression, TES4 flags), validated for Starfield against the corpus | `uesp-skyrim: <page URL>` |
| 2     | Record definitions    | Upstream xEdit `wbDefinitionsSF1.pas` at the pinned revision                              | `xedit: wbDefinitionsSF1.pas@<sha>#L<start>-L<end>` |
| 3     | Empirical validation  | Starfield plugin fixtures: corpus statistics over the 14 official ESMs and our synthetic plugins | `corpus: <evidence note>` |
| 3     | Empirical validation  | CK differential observation (§2, unchanged)                                              | `ck-diff: <observation id>` |
| 3     | Empirical validation  | Differential comparison against SF1Edit (§2a below)                                      | `corpus: <evidence note>` naming the SF1Edit build and the fixture |
| 4     | Secondary references  | UESP's Starfield pages (Amendment 1), the Creation Kit wiki (names and meanings), other community documentation that names no forbidden source | `uesp-starfield: <page URL>`, `ck-wiki: <page title>` |

Rules:

- **Precedence.** Level 3 evidence overrules levels 1, 2 and 4: when the corpus or the CK's
  own output disagrees with a definition, the bytes win and the conflict is noted in the
  definition. Level 2 overrules level 4 for layouts. Level 4 never establishes a layout on
  its own; it supplies names, meanings, and cross-checks.
- **Confidence.** A definition carries `confidence`:
  `reference` (from xEdit only), `corpus` (sizes, counts and order confirmed against the 14
  official ESMs), `ck-diff` (confirmed by CK differential observation), `verified` (both).
  It is required when any `source` or `evidence` entry is `xedit:` and optional otherwise
  (but see the Skyrim rule below).
  The loader rejects a level the citations do not support (`reference` without an `xedit:`
  citation, `corpus` without `corpus:`, `ck-diff` without `ck-diff:`, `verified` without
  both).
- **Skyrim definitions are for the shared container only.** A Skyrim (or other
  Bethesda-family) fact may be used for the container layer, and each such use must carry
  `confidence = "corpus"` or better after Starfield-specific validation. The loader rejects
  `confidence = "reference"` next to a `uesp-skyrim:` citation now, and will reject a
  missing `confidence` there once `TES4`, `GRUP` and `RECORD` carry it (their `corpus:`
  evidence already supports `corpus`; adding it changes the definitions hash, so it lands
  with a `cargo xtask codegen` rerun in lane B's next schema change). Skyrim definitions are **never** used for
  Starfield record bodies, and never silently substituted where xEdit's Starfield
  definition is missing or unclear: such bytes stay opaque.
- **Names.** Field names come from the CK wiki or the CK's UI labels where they exist,
  otherwise in our own words; a placeholder label from any source ("Unknown") never
  becomes a name.
- Facts are recorded in our own words and structure. No source's code, comments or prose
  (xEdit's MPL code, UESP's or the CK wiki's text) is copied into code, schema files, or
  docs.
- Anything no level settles stays opaque and is preserved verbatim.

### 2. CK differential observation (unchanged)

§2 stays the procedure for CK observations. Its role changes from the only source of
Starfield layouts to empirical validation: an observation confirms (or refutes) a
definition and raises its `confidence` to `ck-diff`, or to `verified` with corpus
agreement. Step 8 now reads: update the definition's `confidence` and add the `ck-diff:`
citation to `evidence` (or to `source` when there is no xEdit definition).

### 2a. Differential comparison against SF1Edit (new validation method)

1. Run SF1Edit (xEdit's Starfield mode) as an external program on a fixture: one of the
   official ESMs, read only, or a synthetic plugin of ours. Never bundle or redistribute
   SF1Edit; running it creates no license obligation (MPL-2.0).
2. Export or record its decoded view of the records under test (field values per record).
3. Decode the same records with KitCraft and compare value by value. A difference is
   either a transcription error on our side, a version skew (the SF1Edit build differs from
   the pinned revision; record both), or an upstream defect (record it; the bytes decide).
4. Record the run as `corpus:` evidence naming the SF1Edit build, the fixture, and the
   result. Agreement on every occurrence in the corpus supports `confidence = "corpus"`.

SF1Edit is **not installed** on this machine yet. Installing a build that matches the
pinned revision (or recording the skew) is an M2 task. Release `4.1.5f` (2024) is 1,459
commits behind the pin.

### 3. Forbidden sources (replaces the xEdit entry in §3)

- **xEdit is no longer forbidden.** `wbDefinitionsSF1.pas` at the pinned revision is level 2
  under the ADR 0004 constraints (source material only; no Pascal coupling; no
  reproduction of xEdit's architecture; no verbatim code or comment copying). Other xEdit
  files (`wbDefinitionsCommon.pas`, the container code) are not covered until the owner
  extends the decision.
- **Mutagen and Spriggit** stay forbidden (GPL-3.0), as in §3, including Spriggit's output
  field names and key structure.
- Pages derived from xEdit: the starfieldwiki `/CTDA` page is no longer forbidden on its own,
  since its source is now permitted, but it follows the stale `dev-4.1.5` branch; cite
  xEdit at the pin instead. The loader keeps rejecting `uesp-starfield:` citations of
  `/CTDA` for that reason. Pages derived from Mutagen or Spriggit stay forbidden.
- An agent's own recall of Starfield layouts stays forbidden; every fact is cited.
- Disassembly or debugging of `CreationKit.exe` or `Starfield.exe` stays forbidden.
- Amendment 1's hold on the `/PERK` page's OMOD layout lapses: OMOD is in the lane B
  priority list and is transcribed from xEdit (level 2); `/PERK` is a level 4 cross-check.

### Consequences

- The schema loader accepts `xedit:` (pinned SHA and line range required) and checks
  `confidence` (`crates/schema/defs/README.md`). The three container definitions that cite
  `uesp-skyrim:` (`TES4`, `GRUP`, `RECORD`) gain `confidence = "corpus"` in lane B's next
  schema change (`plan/STATUS.md`).
- Lane B round 3 transcribes typed layouts from xEdit after the owner reviews this
  amendment, ADR 0004, and `docs/xedit-reference.md`.

## Amendment 3 (2026-10-14): the prior-art dossier as an evidence backlog

### Why

The owner supplied `docs/research/starfield-dossier-2026-10-14.md`, a prior-art and
source-policy survey (sources, plugin facts, a conflict register, a validation plan). The
same day the owner approved ADR 0004 with six clarifications ("Owner approval 2026-10-14"
there). This amendment says how the dossier is used. It is **an evidence backlog, not a
scope change for M1 or M2**: it adds no M2 task, gate or exit criterion, and its fixture
matrix and gates (G0-G5) are picked up only when a lane's existing task needs them.

### 1. Source registry

`docs/source-registry.md` lists every source the dossier names (its section 1), plus the
pinned xEdit files and the sources this ADR already uses, with our status for each:
`permitted-reference`, `external-oracle-only` (run, never read), `docs-only`, `forbidden`,
or `future`. Before a new source is cited anywhere, it gets a registry row. The registry
does not change the hierarchy in Amendment 2; it records where each source sits in it.

### 2. Level 2 now covers `wbDefinitionsCommon.pas`

Amendment 2 §3 said other xEdit files were not covered. The owner's approval of ADR 0004
extends level 2 to `Core/wbDefinitionsCommon.pas` at the same pinned SHA, under the same
rules; further helper files follow ADR 0004 clarification 2 (document the dependency, same
rules, no new policy round unless the license or source category differs).

### 3. Evidence grades (adopted alongside `confidence`)

We adopt the dossier's grading, in our words:

| Grade | Evidence behind the claim                                                        |
| ----- | -------------------------------------------------------------------------------- |
| A     | Upstream code, output of an official tool, or a fixture we can reproduce          |
| B     | A carefully documented experiment or research write-up, or maintained docs        |
| C     | A secondary overview or an analogy from another game or an older build             |
| D     | A hypothesis                                                                     |

- **Grade the claim, not the source.** One source can carry an A claim and a C claim. A
  behavioural claim is graded for the game build it was made on; a 2024 claim about load
  order may be stale on a 2026 build and is regraded (or rechecked) before we rely on it.
- **Grade and `confidence` answer different questions.** The grade says how strong the
  outside evidence for a claim is; `confidence` (ADR 0004 clarification 3) says how far
  our own artifacts have validated it. Grades are recorded where a claim is weighed (the
  registry, conflict notes, `evidence` notes, research docs); the schema loader does not
  check them.
- A grade D claim never establishes a layout or drives a write; it names something to
  test.

### 4. General rule

Replacing any reading of "UESP or stop", in our own words:

Format facts may come from public specifications and documented experiments. Where no
normative specification is published, a version-pinned, licensed community implementation
may serve as an implementation reference, provided every claim taken from it records the
source, revision and license, and claims that affect what we write are validated
independently. A public source is not permission to copy or relicense its expression; code
is copied only under a compatible license with its notices and source obligations met (and
in this project not at all, ADR 0004). Fields we do not support roundtrip opaquely or fail
with an explicit error; a definition is never invented. Third-party programs may be run as
independent test oracles without their code entering the project. Redistributing
third-party code or game assets needs its own distribution and license review.

This is consistent with Amendment 2 and does not loosen §3: GPL/LGPL/AGPL implementations
(Mutagen, Spriggit, esplugin, libloadorder, LOOT's code, CommonLibSF) stay
`external-oracle-only`: we may run them, never read their source, and never use their
output field names.

### 5. Conflict register entries that matter to us

From the dossier's section 5, with where each lands:

| Entry | Our position | Owner of the check |
| ----- | ------------ | ------------------ |
| Medium-plugin flag `0x400` vs `0x10000` | `0x400` is corroborated (pinned xEdit `TES4` flags, a later write-up by the same author); `0x10000` is a stale issue description. Neither is our evidence yet: confirm from TES4 header bytes of a locally exported medium master, and add a regression case | M2.D3 (b, c), M2.O4 session |
| A standalone child plugin cannot resolve its masters' scales | Raw FormIDs and resolved identities are separate (ADR 0004 clarification 5); resolution needs the loaded masters; an unavailable master's scale is reported unknown, never guessed | lane D (`doc`), M2.D3, M2.D4 |
| 2024 load-order claims may be stale on 2026 builds | Load-order behaviour is recorded with the game build it was observed on; 2024 write-ups are grade B for on-disk FormID facts and at most C for current runtime load order | lane D (M2.D1, M2.D3) |
| CK `.esp` working file vs `.esm` export | The CK's working file and the shipped file differ; a published master's scale is not changed. Which extension and flags `build` writes stays an M2.E3 question | M2.E3, M2.E4 |
| Blueprint autoload (2026) | `BlueprintShips-<base>` plugins can become active implicitly with their base plugin (LOOT 0.29 changelog); never treat them as ordinary `plugins.txt` entries without checking a current build | lane D (M2.D1, M2.D2) |

### Consequences

- `docs/source-registry.md` is the list of sources and their status; ADR 0001's clean-room
  rules and §3 above are unchanged by it.
- No M2 task changes. The dossier's fixture types and gates are a backlog for lanes A, B,
  D and E to draw on when an existing task needs them.
- The dossier's suggested provenance manifest (YAML per field) is not adopted: our
  `source`, `evidence` and `confidence` keys already carry that information.

## Amendment 4 (2026-10-14): engine-reserved object IDs, a corpus-derived fact

### Why

Each schema family that typed more FormID members exposed more `formid.dangling` errors in
the official files with the same shape: an object ID below 0x800 in `Starfield.esm`'s index
space that no official file defines. By the ACTOR and ITEMS merges they ran to hundreds
(PRPS actor values on NPC_, RACE, PKIN and the items family, FLST actor-value lists, the
player reference 0x14 in reference slots, REFR NAME 0x1FB, SMEN PNAM 0x5B). Listing each as
a known error hid a single fact behind hundreds of lines.

### Decision

- **The fact** (grade A: a fixture we reproduce, `validate_corpus_official_files`): none of
  the 14 official files defines these object IDs, and the official files name them from
  typed members whose layouts decode identically everywhere else (0 typed conflicts).
- **The reading** (grade D until observed): object IDs below 0x800 in `Starfield.esm`'s
  index space that the file does not define are engine-reserved default forms (the player
  reference, built-in actor values, a story manager root), which xEdit treats as hardcoded
  forms rather than as missing ones. It is recorded as a corpus-derived fact pending a CK
  confirmation (does the CK resolve and keep them on save; STATUS Requests to lane H).
- **What it changes**: `validate`'s `formid.dangling` does not report a typed member naming
  such a form; it counts it as `reserved_form_ids` (per member path in
  `reserved_by_member`) and says so in `limitations`. Low IDs that `Starfield.esm` does
  define are ordinary forms; a low ID under any other master, and a record header (an
  override of a missing low form), are checked as before. `docs/validate.md`
  ("Engine-reserved object IDs") has the rule; the per-plugin, per-member counts are the
  regression expectation of `validate_corpus_official_files` (`RESERVED`).
- It drives no write and no layout: the IDs are copied as stored (§3's rule for grade D
  claims holds).

### Consequences

- The `< 0x800` entries leave `KNOWN_ERRORS`; a change in any reserved count fails the
  corpus test, so a family that types a new member naming such IDs updates the table with
  its evidence.
- A CK observation that the CK rejects or rewrites these IDs reverts the exception (they
  become errors again) and is recorded here as Amendment 5.
