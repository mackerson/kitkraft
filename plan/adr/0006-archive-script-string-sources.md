# ADR 0006: Sources for BA2, `.pex` and string-table facts

- Status: Accepted
- Date: 2026-10-14
- Scope: `crates/ba2`, `crates/pex`, `crates/strings` and their engine handlers (M2 lane G)
- Relates to: ADR 0001 (clean room), ADR 0005 Amendment 2 (source hierarchy for plugin
  facts)

## Context

ADR 0005 orders the sources for plugin (`.esm`/`.esp`) facts. Archives (`.ba2`), compiled
Papyrus (`.pex`) and localized string tables are separate formats with separate evidence:
our own Python tools already read them, the official files can be inspected in place, and
Bethesda ships tools (`Archive2.exe`, `PapyrusCompiler.exe`) that can be run and observed.
None of these formats is covered by the pinned xEdit file, and nothing in ADR 0005 names a
source for them. M2 needs a written rule before code relies on any fact about them.

## Decision

### 1. Permitted sources, in order of precedence

| Level | Source | Used for | Cite as |
| ----- | ------ | -------- | ------- |
| 1 | **Corpus**: the official archives, scripts and string tables under `KITCRAFT_DATA_DIR`, read in place by our own code or scripts (hex dumps, statistics, decode-and-compare). Never copied into the repo | The tiebreaker for every fact: header fields as they occur, codecs, hash function, layouts, ordering and deduplication rules | `corpus: <evidence note>` (in code comments and `docs/corpus.md`) |
| 2 | **Our own tools** in `C:\Modding\scripts` (`ba2x.py`, `pexdis.py`, `pexgrep.py`; provenance in ADR 0001) | The reference implementation: GNRL reading, the `.pex` layout and the disassembly text format | `ours: <script>` |
| 2 | **Black-box runs** of `Archive2.exe` and `PapyrusCompiler.exe` (and `PapyrusAssembler.exe`) from the game's `Tools` folder: give them inputs, compare their outputs with ours | Differential checks: Archive2 extraction vs ours, Archive2 opening our archives, compiler output disassembled by us | `black-box: <tool> <version> <observation>` |
| 3 | **Microsoft's DDS documentation** (`DDS_HEADER`, `DDS_PIXELFORMAT`, `DDS_HEADER_DXT10`, DXGI format sizes) on learn.microsoft.com | The DDS header we synthesize for DX10 extraction; bytes-per-block and bits-per-pixel per DXGI format | `ms-docs: <page title>` |
| 3 | **UESP's Skyrim pages**, as hypotheses confirmed on the corpus: `Skyrim_Mod:String_Table_File_Format` | The string-table header, directory and data layout | `uesp-skyrim: <page URL>` |
| 4 | Generic codec documentation and crates under MIT/Apache (zlib via `flate2`/`zlib-rs`, LZ4 block via `lz4_flex`) | Decompression and compression only; they know nothing about Bethesda formats | crate name and version in `Cargo.lock` |

Rules:

- Level 1 overrules everything: a fact our script assumes or a page states is a hypothesis
  until the official files agree on every occurrence. Where they disagree, the bytes win and
  the difference is written down (`docs/corpus.md`).
- No UESP page for BA2 was found (the Starfield wiki has no archive page; UESP's other
  archive pages describe BSA, not BA2). BA2 facts beyond `ba2x.py` come from the corpus.
- `ba2x.py` covers GNRL extraction only. DX10 records, the v3 header field, the codecs, the
  name hash and the packing layout are corpus facts, each recorded in `docs/corpus.md`
  ("Archives") with the inventory that established it.
- Black-box tools are run, never disassembled, decompiled, debugged or memory-dumped. Their
  outputs stay in temp directories and are never committed. `PapyrusCompiler.exe` is never
  passed `-release` (or `-final`); KitCraft's argument builder cannot emit them.
- Facts are written in our own words and structure; no prose from any page is copied.

### 2. Forbidden sources

- Other implementations of these formats, whatever their license: Bethesda Archive
  Extractor (BAE), xEdit's archive code (`wbBSArchive.pas` and friends; ADR 0004 covers
  `wbDefinitionsSF1.pas` only), Champollion and other Papyrus decompilers, Mutagen,
  Spriggit, and archive or `.pex` crates. Not read, not searched for snippets. Generic codec
  crates (level 4) are not format implementations and are allowed as dependencies.
- An agent's recall of these layouts. Every fact is checked against the corpus before code
  relies on it.
- Disassembly or debugging of `Archive2.exe`, `PapyrusCompiler.exe`, their DLLs,
  `CreationKit.exe` or `Starfield.exe` (ADR 0001 §3).

### 3. Data handling

- Official archives, scripts, strings and Papyrus sources (`Tools\ContentResources.zip`)
  are read in place, read-only. Tests stream them; extraction for checks goes to temp
  directories. Nothing from them is committed: fixtures are synthetic, built by our tests.
- `ATTRIBUTION.md` does not change: no third-party bytes enter the repository.

## Consequences

- `crates/ba2`, `crates/pex` and `crates/strings` cite `ours:`, `corpus:`, `ms-docs:` or
  `uesp-skyrim:` in module docs for each fact they rely on; the inventories behind the
  `corpus:` facts are in `docs/corpus.md`.
- Corpus tests skip without `KITCRAFT_DATA_DIR`, like the plugin corpus tests.
- Differential checks against Archive2 and the Papyrus compiler are recorded in
  `plan/STATUS.md` with the tool version and what differed.
- A source not listed here needs an amendment; one that ADR 0005 forbids goes to the owner.
