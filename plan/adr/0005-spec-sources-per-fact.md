# ADR 0005: Specification source per fact

- Status: Accepted
- Date: 2026-10-14
- Amends: ADR 0001 §2 (specification sources), ADR 0003 §5 (`source` key)
- Amended: 2026-10-14 by Amendment 1 below (UESP's Starfield pages exist at
  starfieldwiki.net; `uesp-starfield:` source form; `/CTDA` forbidden). The Context section
  is kept as written; where it says UESP has no Starfield page, Amendment 1 corrects it.

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
