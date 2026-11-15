# Known gaps

Check this before claiming something works. Remove a row only with the commit that closes it.

| Gap                                                               | Impact                                                   | Closes in        |
| ----------------------------------------------------------------- | -------------------------------------------------------- | ---------------- |
| The workbench's edit dialog (M7.E2, `dialogs/set.rs`) edits scalar fields only: struct, list, lstring and link (FormID) fields are shown read-only | Such fields are edited with `kit set` (CLI) or MCP `execute`, not in the window | a later M7.E ticket (member editing; E's report, L6) |
| Every catalog command is `Implemented` (lane E closed `set`, `validate`, `build`); `PLANNED_IN_M2` still lists them | None for users; the stale list only prints | lane F (prune) |
| `inspect`, `query`, `diff` and `conflicts` decode fields only where lane B's schema gives a layout: today EDID, FULL, DESC and KSIZ/KWDA on the defined types (AMDL, AMMO, COBJ, GLOB, GMST, KYWD, OMOD, WEAP, ZOOM, TES4) and the GMST/GLOB values. There `diff`/`conflicts`/`inspect`'s `vs_previous` compare member by member with FormIDs resolved by plugin name (M3.D2); elsewhere payloads are compared as bytes | Most Starfield payloads stay hex and unknown to `where`; their diffs are per subrecord and byte range, and FormIDs inside them are compared as stored (`payloads_comparable` says when that is safe) | lane B round 3 (each newly typed member is compared by value with no change in lane D) |
| Patch FormIDs are copied raw: a record joins an existing patch only when its FormIDs mean the same there, compared by scale class since M5.D12 (else remapped through the M2.B3 visitors, which is refused when a subrecord is of unknown layout, or has non-zero bytes the schema does not type inside a known layout; fixed-size untyped fields such as OMOD `DATA.Unknown2` and field-decided unions are known layout since M3.E9 round 2, and every official OMOD has them zero) | Records from plugins with different master orders go to a new patch | M3.E10 (after M3.B1) |
| Inline text in patches is written as UTF-8; whether the game reads inline FULL as UTF-8 or a code page is not observed (M3.O2 with fixture F13, M2.O6) | Non-ASCII names may show wrong in game; ASCII names are unaffected | M3.O2 (F13) |
| Recompression is 99.88%, not 100%, identical (160 of 136,741 records differ at level 9): 12 NAVM were written at level 8, 147 NPC_/CELL differ only in the distance of their last match (not reproducible by any zlib-rs setting), 1 NAVM is unexplained (M3.A1, "Recompression parity" below) | An edited compressed record may differ from what the official tools or the CK would write; unedited ones, and decoded ones touched without a change, are always byte-identical | M3 (CK observation F07; M3.H3) |
| `HEDR` record count has two meanings in the corpus: records including `TES4` without groups (12 files), records excluding `TES4` plus groups (both `BlueprintShips` files) | Written verbatim unless records or groups are added or removed; then recomputed by the majority meaning, which may not be what the CK writes for a `BlueprintShips`-style plugin | M3.H3 (CK observation) |
| Copies from small or medium plugins (M5.D12, 2026-10-18): a record whose winner is small or medium, or has small or medium masters, is copied as stored when the patch's masters keep the source's scale classes and their order (each class index resolved through `kitkraft_doc::MasterMap` and re-encoded maps to itself); otherwise remapped by class. A small or medium source's `full-style-self` spelling (own form stored in the full class) is remapped where the header or a typed member uses it; inside payloads no layout types it cannot be seen | Such a copy carries a limitation naming the unchecked payloads (no official file uses `full-style-self` in record headers); a remap through an opaque subrecord (NPC_ `NAM5`) is still refused, so mixing DLCs in one patch can still fail; new forms into a patch with a small or medium master are still refused (lane A allocator); patches themselves are full-scale only | M5.D12 follow-ups (opaque payload layouts, scaled allocator), M3.O2 (in-game check) |
| TES4 bits 0x100 (small), 0x400 (medium), 0x800 (blueprint) are named from the pinned xEdit file and their FormID encoding is corpus-confirmed (`docs/load-order.md`); runtime slots follow the owner's saves (game 1.16.244.0: per-scale order, blueprint plugins last among full), and one full slot is verified in game (`04001000`, M4.O2), but no `FE`/`FD` ID has been checked in the console yet | Runtime IDs in results are inferred (`runtime-slots-by-scale`) unless `load --from-save` confirms them by a save of the same plugin set in the same slots (game-20261101-T1: a save of another order is stale); `esm::tes4` still names only 0x1 and 0x80 | M3.O1, M3.O2, M4.O2 |
| `formid.dangling` cannot see FormIDs inside opaque subrecords (counted as `unchecked_subrecords`), nor resolve a small or medium plugin's own records stored full-style (`full-style-self`, counted as unresolved). Since M3.E3 scripts inside archives are scanned and `FE`/`FD` FormIDs are checked | FormIDs in untyped payloads are not checked | lane B round 3 (typed layouts); M3.O2 (`full-style-self`) |
| Streaming `rewrite_file` writes `TES4` before it sees the edits | When an edit adds or removes records or groups, the `HEDR` count must be patched afterwards: `rewrite_file_to_path` does it; a caller streaming to its own sink applies `RewriteReport::hedr_fix` (`HedrFix::apply` on a seekable sink) | permanent (by design of the one-pass stream) |
| Group type 10 is not in UESP's table (Starfield-only)            | Preserved and walked; its label is shown as a FormID     | M3 (CK observation) |
| Schema (2026-10-31, `docs/record-parity.md`): 181 record definitions for the 186 record types of the 14 official files. 91 have typed members (layouts transcribed from the pinned xEdit file and checked against the corpus: lane B rounds, M4, and the KK-2xx depth tickets); 87 are inventory definitions, which name every signature the corpus shows but keep its payload opaque (M4.BX2, KK-1xx); 3 are lstring-only (DMGT, FXPD, PMFT: M3.B8). The other lstring-only definitions of M3.B8 have since been typed or turned into inventories. The five types without a definition (MRPH, CNDF, VTYP, MTPT, ASPC) are queued as KK-224. `esm` decodes through them (`Record::typed`, `Plugin::typed`, `Record::set_typed`) (`inspect`, `where` and `diff` decode every defined subrecord through `kitkraft_doc::typed`, which reads the same values as the generated decoder behind `Plugin::typed` on every record of the 14 files: M3.D6, `docs/corpus.md`) | Other Starfield-specific payloads (KYWD FNAM and VMAD, record types without a definition, every member of an inventory definition but its EditorID and shared groups, the subrecords an lstring-only definition keeps raw) are opaque; `inspect` shows hex | later lane B rounds (full definitions from the pinned xEdit file) |
| lstring IDs (FULL, DESC, GMST `s` values) checked against the string tables on all 14 files (M3.D6, `docs/corpus.md`): all resolve except 85 ARMO FULL IDs of Starfield.esm (in none of its English tables) and the 96 of BlueprintShips-SFBGS050.esm, which is localized but ships no string tables (30 of its IDs exist in SFBGS050's tables; whether the game reads a blueprint companion's strings there is not observed) | Those names show as unresolved strings with the reason, never guessed | in-game or CK observation (M3.O1/M3.O2) |
| `coverage` (and `docs/record-parity.md`) classify subrecords by signature only | A `typed` subrecord whose payload does not fit its layout still counts as typed | lane C (decode through `Plugin::typed`) |
| Discriminated unions: `decider = "edid-prefix"` (a whole subrecord, by the EditorID's first byte, M2.B9) and, since lane B round 4, `decider = "field:<Sibling>"` (a struct field, by an earlier sibling's value, optionally refined by a second sibling; the object mod property's FunctionType, Value1 and Value2 use it). A layout chosen by another subrecord, by flags or by a value outside the same struct still cannot be expressed. Kept raw for that reason: the component data inside every BFCB..BFCE block (chosen by the BFCB component type), VMAD script data, the condition data CTDA (comparison value and parameters chosen by flags and the function index), the LVLI COED owner data (by the owner's type) and the INNR XNAM property target (kept as an integer; its names depend on UNAM) | Such payloads stay opaque (lossless); FormIDs inside them are not visited, so remapping refuses records that carry them | M3.B9 |
| World-family payloads kept raw: VMAD, NAVM NVNM and MNAM, REFR XSL1 and XNSE, CELL/REFR XTV2, the PDTO data union, BFCB block bodies, and fields xEdit leaves unknown | Shown as bytes; FormIDs inside them are not visited (patch remapping refuses records that hold them) | later format work (counted arrays inside elements, flag-gated fields, non-EditorID deciders); XNSE is reflection data that names its own classes and fields, so a schema-driven reader over `crates/doc/src/refl.rs` can read it and visit its `RefID`s with no upstream source (`docs/research/reflection-scan-2026-10-31.md`) |
| ~~`doc::typed::field_type` resolves a subrecord's layout by signature: REFR has two `INAM` layouts (patrol idle FormID, pack-in u16) and WRLD `PNAM`/`NAM*` also occur, with other sizes, inside raw BFCB bodies~~ Resolved in M4.BX1: `kitkraft_doc::typed` places every subrecord by position (`placements`, `layout_at`); a BFCB-body subrecord is raw, and a `where` path over occurrences with different layouts is `Ambiguous` (`INAM[1]` picks one) | - | done (M4.BX1) |
| `Plugin::upsert` places a record along a group path copied from an existing record (`PluginIndex::group_headers`); nothing derives the path (block and subblock labels, type 6/8/9 children) for a new cell or a reference without a source record | New cells and new placed references need a hand-built path | lane A (`docs/schema/world-family.md`, "What lane A needs") |
| Group type 10 is not in UESP's table (Starfield-only)            | Preserved and walked; its label is shown as a FormID     | M3.H3 (CK observation) |
| Group structure a new plugin gets is the official pipeline's (M4.A1, A2: `cells::PlacementStyle::OFFICIAL`: type 9 before type 8, `NAVM` first, group stamps 0, empty top-level groups for `NAVM DIAL INFO DLBR SCEN`). Observed from the CK for a one-cell plugin only (F29, F30; `docs/ck-observations.md` `ck-20261101-04`): the same group path as ours, every group stamped version control `0x3561` and the field after a children group's label 7, 3 or 1 (types 6, 9, 8), where ours writes 0 as the official files; a temporary reference in type 9. Group type 10 (quest children) has no name on a permitted source | The order and stamps the CK gives a plugin with several cells, worlds or quests are not observed. The game reads ours (`game-20261101-04`); a CK-saved file has not been run in game | M4.O1 (F20, F21) |
| New forms in small or medium plugins are refused (`forms::NewForms`); a full-scale plugin's first new object ID comes from corpus defaults (the `HEDR` next object ID, else past the highest own ID) | `new`, `clone` and `place` into a small or medium plugin fail with the reason. The CK numbers new forms differently: in a small-master-capable `.esp` its first new reference was `000807` (F29, `ck-20261101-03`), ours `001000` | M4.O1 (F06, F11, F26) |
| Reference composition writes record flag 0x800 (initially disabled) only when allowed: its meaning has a `uesp-skyrim` source only, below `corpus`. (`DATA` Z and rotation, also allowed-only before, now come from the world family's REFR/ACHR definitions at `corpus`, M4.BX1) | A placement must name `flag.initially_disabled` in `allow_reference` | M4.O1 (CK observation) |
| `ONAM` membership follows the corpus rule (M4.A6), but its order is the writer's choice (file order): the official order is not reproducible; what the CK writes for an edited vanilla reference is not observed | A patch's `ONAM` may list the same forms in another order than the CK would | M4.O1 (F23) |
| NAVM passthrough copies `NAVM` records only; whether the CK also changes `NAVI` (or other records) when it generates navmesh is not observed | An imported navmesh may lack a `NAVI` update the game needs | M4.O1 (F22) |
| lstring IDs (FULL, DESC, GMST `s` values) are typed by size and the localized flag only; no corpus check that each ID exists in the string tables | A wrong ID shows as an unresolved string, not as a decode error | M3.D6 (corpus check of every lstring ID) |
| M4.BX2 inventories (LAYR LSCR): only EDID, the corpus table's lstrings, the keyword list and the component frame are typed; every other payload is opaque (named after its signature); members the pinned xEdit file lists that no official record has are not defined | `inspect`/`query` show those members as raw bytes and `kit set` takes them only as `hex:`; a plugin with a member the corpus lacks keeps it raw (`extra`), lossless but untyped | the family lane that takes the type (`plan/STATUS.md` records the owner) |
| String ID 0 (14 official GMST string settings) is in no string table | Such values stay unknown in `query` rather than being read as empty strings | when a source says what ID 0 means |
| MO2 rules not yet checked in MO2 itself: mod priority direction (UI check), `lockedorder.txt` format (empty in the owner's profile), whether `loadorder.txt` lists inactive plugins, how unmanaged DLC plugins are activated, which version wins when a user plugin overrides a blueprint plugin's record (the blueprint plugins take the last full slots), how the game interleaves official plugins of different scales (the saves give each scale's order only; assumption `hardcoded-official-slots`) | `docs/load-order.md` marks each as observed in files only or as an assumption | M3.O2, M3.D5 |
| Small/medium plugins that store their own records full-style resolve by the assumption `full-style-self` (object ID masked to the plugin's scale, per Ortham's example); FormIDs past every class (4 records of `HG_ShipCargoHoldAtHome.esm`) stay unresolved | Results name the plugins concerned and say per version how it resolved (`resolved_by`); unresolved records are left out of overrides and queries | M3.O2 (in-game check), M3.D5 |
| `ba2.pack` stores no DDS header; extraction synthesizes the one `Archive2.exe` writes | A texture packed with a different (valid) header extracts with the same pixels and a different header | permanent (the format has no header field) |
| `ba2.pack` refuses non-ASCII archive paths. Archive2's behaviour is now observed (M3.G3, `docs/corpus.md` "Non-ASCII names in Archive2"): Windows-1252 names, hashes that stop at the first non-ASCII byte (so such names collide and Archive2 silently keeps one), run-dependent hashes for twelve bytes, and no support for characters outside Windows-1252. How the game looks up such names is not observed | Files with accented or other non-ASCII names cannot be packed; archives Archive2 made with them are read, extracted under Archive2's names and hash-checked | an in-game lookup check (not planned in M3) |
| Archives cannot hold empty files: `black-box: Archive2.exe 1.2.0.1` skips them ("Skipped zero-length file") and no official record has size 0 | `ba2.pack` skips empty files and lists them in `limitations[]`; the `kitkraft-ba2` `pack` API refuses them | permanent (Archive2 and the corpus agree) |
| DDS header choices for DXGI formats absent from the official archives are unobserved | Those formats extract with the DX10 extension header, which Archive2 may not choose | when such a texture is observed |
| `.pex` assembly, editing and decompiling to `.psc` | Scripts can be disassembled and compiled from source only | after M3 (not in M3) |
| No coverage-guided fuzzing (cargo-fuzz needs nightly). M3.A5 added seeded structure-aware mutation on stable Rust (size fields, `XXXX`, decompressed sizes, flags, nesting, recompressed bodies; `docs/corpus.md`, "Hostile input"), clean on 2,000 inputs per small file and per fixture in the ignored run and on 100,000 per input once | Mutations are seeded, not guided by coverage, so paths no mutation reaches stay untested | cargo-fuzz when a nightly toolchain is allowed (not planned) |
| The workspace's `unsafe_code = "deny"` lint (`Cargo.toml`, ADR 0013; `cargo xtask layers` admits only the viewport's `gl.rs`) covers workspace crates only. zlib-rs (our flate2 backend) is pure Rust but uses `unsafe` internally: 469 source lines of zlib-rs 0.6.8 mention it | Decompression of hostile streams relies on that crate's own unsafe code being sound | permanent (dependency review) |
| The workbench viewport's OpenGL calls (`crates/ui-egui/src/viewport/gl.rs`) are `unsafe`: every `glow` call is an `unsafe fn` (ADR 0013). `cargo xtask layers` keeps `unsafe` to that one file | The viewport's soundness rests on that file's own checks (its objects, its slice lengths, egui_glow's current context); no plugin data reaches it except as finite matrices | while the viewport draws with `glow` |
| The Render Window's camera controls (right-drag orbit, middle-drag pan, wheel zoom, WASD fly) and its field of view are KitKraft's choice, not observed in the CK | A CK user may find other bindings | M7.O2 (sitting 1), then M7.M5 |
| Spriggit round trip lost data on 2 of 5 real mods tested          | Not an oracle; inventory facts only (ADR 0001 §4)        | permanent        |
| SF1Edit installed is xEdit 4.1.5f (2024), older than the pin; only its console dumper runs headless (`docs/oracles.md`) | The cross-check (`cargo xtask oracle sf1edit`) cannot judge GPOG/GPOF, light masters (SFBGS047) or newer WEAP components; its editor script mode needs a click | M3.B11 (rerun as members get typed); a build at the pin (owner) |
| Base form component blocks (BFCB..BFCE) stay raw in every planet-family type (and elsewhere): upstream reads each block by its component name, a union the format cannot express. GBFM alone has 50,639 blocks of 27 kinds; STDT/PNDT names, keywords and models and the PCM placement properties live in them (`docs/schema/planet-family.md`) | Those values are not typed, not visited by `form_ids()`/`lstrings()`, and show as raw subrecords in `inspect` | a component-name decider (format work, lane B). Nine of the 64 component kinds (14,209 of 349,154 blocks) hold reflection payloads (`REFL`, `PTCL`, `PCCC`) that describe themselves and are read without the decider; the other 55 hold ordinary subrecords and need it (`docs/research/reflection-scan-2026-10-31.md`) |
| Planet family: CTDA comparison value/parameters/reference, VMAD, GBFM NVNM (GBFM OBTS is typed through `ObteBlock` since M4.BX1), SUNP REFL/RDIF (helper unit not authorized), STDT ONAM/WED0, BIOM BNAM and unnamed GNAM slots, PNDT unnamed PPBD/HNAM/FNAM slots are opaque | Raw bytes only; FormIDs inside them are not checked | deciders (format work), CK observation (STATUS Requests to lane H). REFL/RDIF need no upstream helper after all: the payload carries its own class and field table, so a schema-driven reader over `crates/doc/src/refl.rs` reads it from the corpus (`docs/research/reflection-scan-2026-10-31.md`) |
| Items and workshop family (STAT MISC CONT ACTI FURN MSTT TERM TMLM ARMO IDLE ANIO CPTH): VMAD, component blocks (a STAT's keywords live in one), NVNM, CTDA unions, COED's middle slot, FURN NNAM (string pairs), TERM WBDT, TMLM ISET/XLOC padding are opaque; XALG is a plain integer everywhere, and the unnamed bits of the shared `BipedSlots` type (BO64 on ARMO and ARMA) stay raw, bit 63 included (`docs/schema/items-workshop-family.md`; OBTS is typed since the integrator folded the object template into `ObteBlock`, M4.BX1) | Raw bytes only; FormIDs inside them are not checked | deciders and a component-name decider (format work), CK observation (STATUS Requests to lane H) |
| Bit 63 of the shared `BipedSlots` type (BO64 of ARMO and ARMA) cannot be named: upstream calls it "FaceLights", but its mask 0x8000000000000000 does not fit the signed `i64` the schema reads bit values as, so the bit is kept raw and unnamed (`defs/common/BipedSlots.toml`) | The bit roundtrips in the payload but cannot be read, set or cleared by name | lane B (bit values read as unsigned) |
| Signature-based layout lookups (`kitkraft_doc::typed::field_type`, engine `fields::layout_for`) cannot tell positions apart: PNDT CNAM/FNAM/GNAM have two layouts; PCBN, PKIN and GBFM component blocks reuse top-level signatures (NAM1..NAM5, VNAM, CNAM, ANAM). The generated decoder is positional and exact | `inspect`/`where`/`set` may misread or refuse those subrecords; lane D's all-files typed corpus test panics on PKIN VNAM; a GBFM object-template FULL is left out of the schema for now | lanes D and E (STATUS Requests) |
| Quest family: VMAD after its 6-byte header (QUST, PACK, INFO, SCEN: scripts, property values, fragments), PACK data-input CNAM, PTDA target, PLDT/PLVD value and PDTO data, INFO ENAM flag sets, QUST ALFG, ALAM and create-matching ALCA, INFO INTV/NAM9, SCEN VNAM are opaque (`docs/schema/quest-family.md`) | Script properties, package targets and fragment names are raw; FormIDs inside them are not visited by `refs` or `formid.dangling` | deciders (format work), CK observation (STATUS Requests to lane H) |
| SCEN action type-specific subrecords are kept raw (`any_until` the action's end marker): upstream selects their layout by the action type, and signatures change meaning between action types | A scene's dialogue topics, packages, nested scenes, references and camera shots are not typed or visited; `refs` cannot follow scene -> topic | a decider on a sibling value (format work, lane B) |
| CHAL keeps all its data in one component block (activities, their FULL/DESC string IDs, conditions, rewards) | Challenge text is not typed; the lstrings are not visited | a component-name decider (format work, lane B) |
| A new world has no cells yet (KK-221): `new` writes the WRLD alone, without a persistent cell and without an exterior grid or cells | `place` is expected to add them; a new world is not game-tested | `place` for worlds (later lane D); M3.O2 (in-game check) |
| No hosted CI (the remote is public GitHub, `github.com/mackerson/kitkraft`, since 2026-11-01) | The local `cargo xtask ci` run is the only gate | owner |

## Recompression parity (measured 2026-10-14)

Question: if we decompress a record and compress it again, do we get the original bytes?
Unedited records never go through this (the writer re-emits their original compressed
bytes); it decides what an edited compressed record looks like. Measured over every
compressed record in the 14 official ESMs (136,741 records; `corpus_recompression_parity`).

All 136,741 streams start with zlib header `78 DA` (maximum-compression level class).

| Compressor                                        | Identical at any level | Level 7 | Level 8 | Level 9           | Levels 0-6 |
| ------------------------------------------------- | ---------------------- | ------- | ------- | ----------------- | ---------- |
| flate2 default backend (miniz_oxide)              | 3,366 (2.5%)           | 0       | 0       | 3,366             | 0          |
| C zlib 1.3 via libz-sys (out-of-tree measurement) | 117,664 (86.0%)        | 116,970 | 117,664 | 117,664           | 0          |
| zlib-rs (workspace backend since adaf097)         | 136,593 (99.9%)        | 2,310   | 2,320   | 136,581 (99.88%)  | 0          |

Per file with zlib-rs at level 9: 100% for BlueprintShips-SFBGS050, SFBGS004, SFBGS008,
SFBGS047 and ShatteredSpace; misses are 149 in Starfield.esm, 3 in BlueprintShips-Starfield,
3 in SFBGS003, 2 in SFBGS006, 2 in SFBGS00D, 1 in SFBGS050. The workspace now builds flate2
with only the zlib-rs backend and recompresses edited records at level 9.

### The 160 misses (M3.A1, 2026-10-14)

Every miss is listed in `docs/corpus.md` ("Recompression misses"), with its file, type,
FormID, offset, sizes, first differing stream byte and class. The listing is
`corpus_recompression_misses` (`crates/esm/tests/recompress.rs`); the classes come from an
out-of-tree study that decodes both streams to LZ77 tokens and searches compressor
settings (procedure below).

Per file and class (E, N, U as defined after the tables):

| File                         | Compressed records | Misses | E   | N  | U |
| ---------------------------- | ------------------ | ------ | --- | -- | - |
| Starfield.esm                | 91,149             | 149    | 136 | 12 | 1 |
| BlueprintShips-Starfield.esm | 1,218              | 3      | 3   | 0  | 0 |
| SFBGS003.esm                 | 1,034              | 3      | 3   | 0  | 0 |
| SFBGS006.esm                 | 360                | 2      | 2   | 0  | 0 |
| SFBGS00D.esm                 | 5,935              | 2      | 2   | 0  | 0 |
| SFBGS050.esm                 | 3,184              | 1      | 1   | 0  | 0 |
| The other 8 files            | 33,861             | 0      | 0   | 0  | 0 |
| Total                        | 136,741            | 160    | 147 | 12 | 1 |

Per record type (all files):

| Type | Compressed records | Misses | Rate  | E   | N  | U | Inflated size of the misses (min / median / max) |
| ---- | ------------------ | ------ | ----- | --- | -- | - | ------------------------------------------------ |
| NPC_ | 8,447              | 118    | 1.4%  | 118 | 0  | 0 | 643 / 4,511 / 6,206                              |
| CELL | 32,230             | 29     | 0.09% | 29  | 0  | 0 | 282 / 605 / 36,764                               |
| NAVM | 92,764             | 13     | 0.01% | 0   | 12 | 1 | 10,355 / 29,109 / 103,552                        |

No other record type has a miss. Every miss has zlib header `78 DA`, one stream that ends
exactly at the end of the record (no trailing bytes) and a correct Adler-32.

- **E, end-of-input match (147: 118 NPC_, 29 CELL, in six files).** The original stream
  and level 9's carry the same LZ77 tokens except the last one: a match that runs to the
  end of the record's data, of the same length, at a larger distance in the original (NPC_:
  length 35 at distance 38 where level 9 picks 37 in 117 of 118, the other 31 at 33 for
  32; CELL: lengths 4 to 28). The streams differ only in their last bytes, except in four
  where the changed distance code also changes the dynamic Huffman tables (from stream
  byte 3, 3, 4 and 91 on). In all 147, the data byte that follows the original's
  candidate is `00` and the one after level 9's is not. zlib-rs 0.6.8 stops at the first
  hash-chain candidate whose match reaches the end of the input; a compressor that keeps
  comparing past the end (where its buffer holds whatever it held before) picks a farther
  candidate whenever those bytes extend it.
- **N, level 8 (12 NAVM, Starfield.esm).** Reproduced exactly by zlib-rs at level 8 (two of
  them also at level 7), window 15, memory level 8 (most also 9, two also 7); the streams
  differ from level 9's near the start (data byte 14 in ten of them). All 12 sit in one
  group, the temporary children of interior cell `00000025` (`CELL > block 7 > sub-block
  3`), as does the U record.
- **U, unexplained (1 NAVM, `001BA29F`, 103,552 bytes, same group).** No setting tried
  reproduces it; it differs from data byte 24 on and has 4 deflate blocks (level 9: 3).

What was tried, out of tree (a scratch crate outside the workspace, zlib-rs 0.6.8 through
its `__internal-api` feature, the official files read in place; nothing of it is in the
repository or the build):

| Variant | Misses reproduced |
| ------- | ----------------- |
| Levels 1-9 x window bits 9-15 x memory level 1-9 x strategy default, filtered, Huffman-only, RLE, fixed (full stream) | the 12 N |
| The same grid, comparing everything after the 2-byte zlib header (a header written separately) | the 12 N |
| Levels 7-9 tuned: good 4-258 x lazy 16-258 x nice 32-258 x chain 256-65,535 (7 x 5 x 4 x 9 values) | the 12 N (chain <= 1,024) |
| Input fed in 512 B to 256 KiB pieces, no / partial / sync / full / block flush between pieces, levels 7-9 | the 12 N |
| One stream reused for every record of a file in file order (reset between records, window kept), level 9 | none (same as fresh) |
| C zlib 1.3.2 (libz-sys), levels 1-9 x memory level 1-9 x strategies 0-4, window 15 | none |
| miniz_oxide 0.9.1, levels 0-10 | none |
| zlib-rs patched to keep searching after a match reaches the end of the input (zero bytes past the input), levels 7-9, every record | 14 of the 148 that no stock level reproduces; at level 9 it matches 136,172 records instead of 136,581 |
| The same patched zlib-rs, one stream reused per file in file order (window kept), level 9 | 18 of the 147 E; 30 that stock level 9 reproduces now differ (136,569 identical) |

Conclusions: class N is a setting (level 8), probably a separate navmesh pass. Class E is
not a setting of any compressor tried; its last token tracks the byte after each candidate,
and changing what lies past the input changes which records match (zero fill: 14, a
window carried over in file order: 18). So its last token most likely depends on memory
past the end of the input (what the official tool's buffer held from earlier work), and a
compressor that sees only a record's own bytes is not expected to reproduce all of them.
Whether the CK's own writes show the same effect, and which bytes the CK writes for these
records, is F07's question (M3.H1 candidates in `plan/STATUS.md`). The default writer is
unchanged (zlib-rs, level 9) until M3.H3 says otherwise.

## Spriggit 0.41.0 is not a byte-exact Starfield roundtrip

Measured 2026-10-14 on copies of real mods (originals untouched), with Spriggit run only as
an external binary (ESM -> YAML -> ESM; no source read). Setup, commands and per-file
timings: `C:\Modding\tools\spriggit\README.md`.

| Plugin                       | Size in -> out       | What changed                                                                                                         |
| ---------------------------- | -------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Richer Merchants.esm         | 12,703 B -> 11,863 B | **Data loss.** Every leveled list with more than one entry kept only its first entry (15 entries lost). Spriggit's own post-serialize check passed anyway. |
| Ship Remote Control.esm      | 6,114 B -> 6,645 B   | **Data loss.** A terminal menu lost 2 of its 3 menu items.                                                           |
| CulinaryGreenhouses.esm      | 31,196 B -> 31,207 B | Cosmetic only: -0.0 written as +0.0, records recompressed, header/timestamp fields rewritten.                        |
| Hatch-22.esm                 | 174,311 B -> 175,246 B | Cosmetic (-0.0 vs +0.0, recompression, timestamps) plus one content change: in 19 GBFM records a second FLKW was written back as FLFM. The tool README also notes the TES4 ONAM was dropped. |
| ConstellationDecorations.esm | 4,158 B -> 4,158 B   | Byte-identical.                                                                                                      |

Conclusion: Spriggit cannot be our roundtrip oracle, and its YAML is not a safe step-2
shortcut for editing real plugins (it silently drops data and its own check does not
notice). Our own container (`crates/esm`, `kit roundtrip --verify`) is the roundtrip
reference; Spriggit output may be used only as a hint, and any plugin it writes must be
diffed against the input before use.

## Interface gaps (M7)

The M7 exit (ROADMAP M7, bullets 1 and 2) asks every observed row of `docs/ck-parity.md`
"Interface parity" to be `reproduced` or `improved`, or to name a documented gap here with
its reason. Each row of the first table quotes one `gap:` reason of that table verbatim (the
text after `` `gap:` `` in "Maps to", which a test holds equal to `CK_ITEMS` in
`crates/ui-egui/src/menu.rs`), lists the parity rows that give it, and names what owns it:
a ticket or milestone, "permanent" (by design, no command will be added), "out of scope"
(ROADMAP M7 excludes it) or "unscheduled" (not in M7, no ticket yet). A reason is never
reworded here: a row and its quote change together, with `CK_ITEMS`.

Counted 2026-11-10 (sprint 2026-11-09, C2): 51 reasons covering 106 `menu item` rows;
4 more gaps named by a pointer from 5 rows that map a command; 10 rows
whose mapping is not known yet; 4 `unresearched` rows. The NavMesh `menu` row is `reproduced` (the
menu is drawn with its items disabled and the reason, as ROADMAP M7 asks), not a gap row.

| Reason (verbatim from `docs/ck-parity.md`) | Parity rows (`menu item`) | Rows | Owned by |
| ------------------------------------------ | ------------------------- | ---- | -------- |
| "no command: the Preferences window (M7.M3) keeps workbench settings in ui.json" | File > Preferences... | 1 | permanent: UI state in ui.json (M7.M3) |
| "no command: the workbench theme is a UI setting (M6)" | File > Theme (experimental) | 1 | permanent: UI state in ui.json (M6) |
| "no command compacts FormIDs" | File > Compact Active File Form IDs for ... | 1 | unscheduled: not in M7, no ticket |
| "no command converts an existing file's header (see the row "Convert the active file to a master")" | File > Convert Active File to ... | 1 | unscheduled: not in M7, no ticket |
| "not offered: KitKraft has no Bethesda.net upload or login. Observed disabled (data loaded, no active file)" | File > Upload Plugin and Archive to Bethesda.net, File > Logout | 2 | permanent: not offered |
| "not offered: KitKraft has no Bethesda.net upload or login" | File > Login to Bethesda.net | 1 | permanent: not offered |
| "no command: closing the window" | File > Exit | 1 | permanent: the window's own close, not a command |
| "no redo (M7.E6 undoes the last edit only)" | Edit > Redo | 1 | M8.3 (undo history) |
| "no command: moving and copying references in the Render Window is M8 (ROADMAP M7)" | Edit > Cut Render, Edit > Copy Render, Edit > Paste Render, Edit > Paste in Place | 4 | M8.7 (placement editing in the viewport) |
| "no search-and-replace command" | Edit > Search & Replace... | 1 | M8.1 (search everywhere); the replace half needs a further ADR 0007 amendment |
| "no search by condition" | Edit > Find by Condition | 1 | M8.1 (search everywhere) |
| "no command: the Render Window tab lists its own bindings (M7.R1)" | Edit > Render Window Hotkeys | 1 | M7.M5 (shortcuts), after OW-C B2 |
| "no command: the Render Window tab is read-only (M7.R) and has no such view option yet" | View > Toggle Render Features, View > Refresh Render Window, View > Depth Biasing | 3 | unscheduled: no M7.R ticket names it |
| "no command: KitKraft has no toolbar" | View > Toolbar | 1 | permanent: KitKraft has no toolbar |
| "no command: window arrangement is workbench UI state, kept in ui.json (M7.M0)" | View > Statusbar, View > Open Windows..., View > Object Window Layouts, Docks > Save Current Layout, Docks > Save Layout as..., Docks > Recent Layouts, Docks > Manage Layouts..., Help > Snap Windows | 8 | permanent: UI state, not a catalog command (M7.M0) |
| "no command: no preview window yet" | View > Preview Window | 1 | unscheduled: not in M7, no ticket |
| "no command: animation is not in M7" | View > Preview Animations, Gameplay > Animations... | 2 | unscheduled: not in M7, no ticket |
| "no batch action over references" | View > Reference Batch Action Window | 1 | unscheduled: not in M7, no ticket |
| "no command for editor layers" | View > Layers | 1 | unscheduled: not in M7, no ticket |
| "no command for editor layers. Observed disabled (data loaded, no active file)" | View > Default Layer Manager | 1 | unscheduled: not in M7, no ticket |
| "no command: materials are M8 (ROADMAP M7)" | View > Material Editor, View > Material Palette, View > Material Swap Editor | 3 | M8.6 (materials, textures, lighting) |
| "no command: the Render Window tab is read-only (M7.R) and has no such view option yet. Observed disabled (data loaded, no active file)" | View > Current Cell Only | 1 | unscheduled: no M7.R ticket names it |
| "no command: the Render Window camera (M7.R1) has no fixed views yet" | View > Isometric, View > Top | 2 | M7.R1 rest (camera, after OW-C B9) |
| "no command: particle effects are not in M7" | View > Particle Editor | 1 | unscheduled: not in M7, no ticket |
| "no command: no galaxy view (`planet` reads one planet's content)" | Galaxy > Galaxy View | 1 | unscheduled: not in M7, no ticket |
| "no command: world generation, tests and exports are not in M7" | Galaxy > Block Pattern Editor, Galaxy > Generate Locations, Galaxy > Gen Overlay Placement Data, Galaxy > Generate Biome Placements, World > World Testing And Reports, World > View Render Test Failures..., World > Create Local Maps, World > Validate Room/Portal Alignment, World > Generate LOS, World > Generate Max Height Data, World > Export MapMarker Data | 11 | unscheduled: not in M7, no ticket |
| "no command: the Render Window shows the cell chosen in the Cell View" | World > Go to Location | 1 | permanent: the Cell View chooses the cell (M7.W2) |
| "no command: physics simulation is not in M7" | World > Run Havok Sim | 1 | unscheduled: not in M7, no ticket |
| "no command: lighting is M8 (ROADMAP M7)" | World > Animate Lighting and Effects | 1 | M8.6 (materials, textures, lighting) |
| "no command: world generation, tests and exports are not in M7. Observed disabled (data loaded, no active file)" | World > Align Tanget Space at NIF Intersection | 1 | unscheduled: not in M7, no ticket |
| "no command: the Render Window draws no weather" | World > Force Weather, World > Toggle Wind | 2 | unscheduled: not in M7, no ticket |
| "no command: terrain editing is not in M7" | Terrain > Landscape Editing, Terrain > Landscape Cutting, Terrain > Heightmap Autopaint, Terrain > Heightmap Import, Terrain > Export Landscape..., Terrain > Reimport All Blocks, Terrain > Cell Terrain Data Viewer | 7 | out of scope: ROADMAP M7 "Deliberately not in M7" |
| "no command: object palettes are not in M7" | Terrain > Object Palette Editing (Qt) | 1 | unscheduled: not in M7, no ticket |
| "navmesh generation is out of scope (ROADMAP M7)" | NavMesh > Generate Current Cell, NavMesh > Generate Loaded Area, NavMesh > Generate WorldSpace, NavMesh > Cycle View Mode, NavMesh > Draw Cover, NavMesh > Draw Traversal, NavMesh > Other Options, NavMesh > Clean NavMesh Splines | 8 | out of scope: ROADMAP M7 "Deliberately not in M7" |
| "no command: dialogue and voice tools are not in M7" | Character > Filtered Dialogue..., Character > Export Dialogue..., Character > Export Speech Challenge Scenes..., Character > Export Dialogue For Specific Voices, Character > Export Companion Affinity Event, Character > List Neutral Emotion Dialogue, Character > List Dialogue Needing Review, Character > Quest Voice Asset Totals, Character > Edit Player Dialogue, Character > Generate Player Address Lip Files | 10 | unscheduled: not in M7, no ticket |
| "no command: character data exports are not in M7" | Character > Export Relationship Data | 1 | unscheduled: not in M7, no ticket |
| "no command: character appearance tools are not in M7" | Character > Update NPC Body Tint Colors, Character > Update FaceBones Model Availability | 2 | unscheduled: not in M7, no ticket |
| "no command: character appearance tools are not in M7. Observed disabled (data loaded, no active file)" | Character > Export FaceGen Customization Textures | 1 | unscheduled: not in M7, no ticket |
| "no command for quest aliases" | Character > Quest Aliases | 1 | unscheduled: not in M7, no ticket |
| "no script list; `papyrus.disasm` reads one compiled script" | Gameplay > Papyrus Script Manager | 1 | unscheduled: not in M7, no ticket |
| "no command: no export of script attachments" | Gameplay > Export Papyrus Attach Data... | 1 | unscheduled: not in M7, no ticket |
| "no command: creature tools are not in M7" | Gameplay > Creature Creator | 1 | unscheduled: not in M7, no ticket |
| "no command for the default objects list" | Gameplay > Default Objects... | 1 | unscheduled: not in M7, no ticket |
| "no command: workshop tools are not in M7" | Gameplay > Workshop Editor | 1 | unscheduled: not in M7, no ticket |
| "no command: audio tools (Wwise, FaceFX, voice files) are not in M7" | Audio > Animation Sound Tag Overrides..., Audio > Process Local Voice WAVs, Audio > Build Soundbank for Active File, Audio > Reload Wwise Data, Audio > Run FaceFX Compiler | 5 | unscheduled: not in M7, no ticket |
| "no CK help text is reproduced; KitKraft's Help menu has its own (`schema`, `doctor`)" | Help > Contents | 1 | permanent: clean room (ADR 0012) |
| "no command: KitKraft lists its own in ATTRIBUTION.md" | Help > Third-Party Software Licenses | 1 | permanent: ATTRIBUTION.md |
| "no command: no viewer for this data in M7" | World > View BetaComment Data | 1 | unscheduled: not in M7, no ticket |
| "no command: no migration of script property data" | Gameplay > Migrate Papyrus Property Data... | 1 | unscheduled: not in M7, no ticket |
| "no command: no camera path editing in M7" | Gameplay > Camera Paths... | 1 | unscheduled: not in M7, no ticket |
| "no command: KitKraft's own About line is on the Start tab" | Help > About | 1 | permanent: the Start tab's About line |

Rows that map a command and still lack what the CK does; their "Maps to" points here:

| Gap | Parity rows | Owned by |
| --- | ----------- | -------- |
| No save step: each confirmed edit is written into the active file when it is made (ADR 0007 Amendment 4, model A), so there is nothing unsaved for File > Save Plugin to write | menu item "File > Save Plugin"; workflow "Save the active file" | permanent: ADR 0007 Amendment 4, model A |
| No "Select Target Plugin" file dialog: a new active file is named before any edit, in the Data dialog's "Active file path:" field (KitKraft's addition, outside the game's `Data` folder), not chosen when saving | dialog "Select Target Plugin (File > Save Plugin with no active file)" | permanent: the Data dialog's path field (M7.M2) |
| No drag placement: `place` takes the base and a typed position from the Cell View's place bar (M7.E4); dragging an Object Window row into the Render Window is not built, the Render Window being read-only | workflow "Place an object into a cell" | M8.7 (placement editing in the viewport) |
| No conversion of an existing file's header to a master; `build` sets the header flags of a file it writes | workflow "Convert the active file to a master" | unscheduled: no ticket |

Observed, mapping not known yet (not a KitKraft gap): the item's window or submenu was never
opened. They stay `observed` until a sitting opens them:

| Reason (verbatim) | Parity row | Observed by |
| ----------------- | ---------- | ----------- |
| "not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)" | File > Import/Export | OW-C launch 1, B10 (submenus, optional; `plan/owner-sitting-2026-11-06.md`) |
| "not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)" | Edit > Picking Preferences | no OW-C step names it: the rest of sitting 1 (M7.O2) |
| "not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)" | View > Packin | OW-C launch 1, B10 (submenus, optional; `plan/owner-sitting-2026-11-06.md`) |
| "not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)" | View > Scene Info Window | no OW-C step names it: the rest of sitting 1 (M7.O2) |
| "not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)" | View > Show/Hide | no OW-C step names it: the rest of sitting 1 (M7.O2) |
| "not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)" | View > Inspector | no OW-C step names it: the rest of sitting 1 (M7.O2) |
| "not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)" | View > Properties | no OW-C step names it: the rest of sitting 1 (M7.O2) |
| "not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)" | View > Volumes | no OW-C step names it: the rest of sitting 1 (M7.O2) |
| "not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)" | View > Reflection Probe | no OW-C step names it: the rest of sitting 1 (M7.O2) |
| "not known yet: grouped with the material items, its window not opened (sitting 1 rest, M7.O2)" | View > AVMS | no OW-C step names it: the rest of sitting 1 (M7.O2) |

`unresearched` rows, with the step of the owner's CK window (OW-C, `plan/owner-sitting-2026-11-06.md`)
that observes each:

| Parity row | Observed by |
| ---------- | ----------- |
| window "Render Window" | OW-C launch 1: B8 loads a cell into it, B9 tries the camera on it |
| control "Render Window camera: orbit, pan, zoom, fly (mouse and keys)" | OW-C launch 1: B9 (camera inputs), B2 (Render Window Hotkeys bindings) |
| shortcut "Keyboard shortcuts, every one observed" | OW-C launch 1: B1 (menu shortcuts), B2 (Render Window Hotkeys bindings) |
| workflow "New form, duplicate form" | OW-C launch 2: A1 (F28, a vanilla object duplicated); no step makes a new form |

### Edit dialog labels with no field path (M7.K4)

Sitting 2 (`ck-20261102-03`) observed the labels of seven edit dialogs; M7.K4
(`crates/ui-egui/layouts/sitting2.toml`) maps each label to the `set` field path it edits or
names it a gap with its reason. These are the gaps, one row per distinct reason (verbatim from
the TOML), with the labels it covers. Counted 2026-11-10 from D's K4 fix round (`b0364b2b`, with `09a71528`'s wording of "On Local Map"; as merged at `51d5af8b`):
23 reasons covering 93 labels (105 labels are mapped). WEAP was not observed and has none.

| Reason (verbatim from `sitting2.toml`) | Labels (type "label") | Labels | Owned by |
| ------------------------------------ | --------------------- | ------ | -------- |
| "a button or control, not a field" | AMMO "Edit", COBJ "New", LCTN "Select Reference", LCTN "Select Horse Marker", LCTN "Analyze Loot", TERM "Edit", TERM "Add", TERM "Remove", CONT "Edit", CONT "Preview Level", CONT "Preview Calculated Result", CONT "Collect From 3D", CONT "Add", CONT "Remove", STAT "Edit" | 15 | permanent: not a field |
| "AMMO's schema has no PRPS; not known which field it edits" | AMMO "Properties" | 1 | schema (lane B): no typed member yet |
| "not known which field it edits" | AMMO "Art", AMMO "Sound", LCTN "Level", TERM "Make Wall Terminal", TERM "Make Desk Terminal", TERM "Properties", CONT "Properties" | 7 | a CK edit of the field (no OW-C step names one) |
| "a button or control, not a field (it adds the DEST group)" | AMMO "Add Destruction Data", TERM "Add Destruction Data", CONT "Add Destruction Data" | 3 | permanent: not a field |
| "no member of this type's schema" | COBJ "List Contains Variants", COBJ "Extra Scrapping Items", LCTN "Encounter Zone", LCTN "Horse Marker Ref", LCTN "Location Ref Type Filter", CONT "Take All", CONT "Mod Data", CONT "Display Slots", STAT "High cost Path", STAT "Has Platform/Language Specific Textures", STAT "Show in world map (ref must be in sky cell)" | 11 | schema (lane B): no typed member yet |
| "not known which field it edits (the CTDA conditions or a flag)" | COBJ "Match Conditions" | 1 | a CK edit of the field (no OW-C step names one) |
| "not known which field it edits (REPR holds repair components, not a recipe)" | COBJ "Workbench Repair Recipe" | 1 | a CK edit of the field (no OW-C step names one) |
| "a record header flag; `set` edits subrecords only" | LCTN "Interior cells use ref location for world map player marker", LCTN "Public Area", LCTN "Off Limits", TERM "Random Anim Start", CONT "Random Anim Start", CONT "Obstacle", STAT "Pack-In Use Only", STAT "Used As Platform", STAT "Obstacle", STAT "Is Marker", STAT "Headtrack Marker", STAT "Non Occluder", STAT "Heading Marker", STAT "Has Currents", KYWD "Restricted" | 15 | unscheduled: `set` edits no record header flag |
| "a column of the PRPS list: `set` takes the list whole (PRPS = [...]); no field path names one row's member" | LCTN "Actor Value", LCTN "Amount", TERM "Actor Value", TERM "Amount", CONT "Actor Value", CONT "Amount", STAT "Actor Value", STAT "Amount" | 8 | unscheduled: `set` names no row of a list subrecord |
| "LCTN has two PRPS lists; not known which one it edits" | LCTN "Base Actor Values" | 1 | a CK edit of the field (no OW-C step names one) |
| "a group caption, not a field" | LCTN "Encounter Leveling Data", LCTN "Ownership", CONT "NavMesh Generation Import Option", CONT "Scripts", STAT "NavMesh Generation Import Option", STAT "Flutter Data" | 6 | permanent: not a field |
| "an owner kind; both kinds edit DATA.Owner and the sitting did not record which is chosen" | LCTN "NPC", LCTN "Faction" | 2 | a CK edit of the field (no OW-C step names one) |
| "two terminal-menu members (DNAM, NTRM); not known which one it edits" | TERM "Desktop Menu" | 1 | a CK edit of the field (no OW-C step names one) |
| "a base-form component, kept raw in the BFCB block" | TERM "Animation Graph Component" | 1 | schema (lane B): no typed member yet |
| "an owner kind; both kinds edit COED.Owner and the sitting did not record which is chosen" | CONT "NPC", CONT "Faction" | 2 | a CK edit of the field (no OW-C step names one) |
| "COED.GlobalOrRank is kept raw (a global or a rank)" | CONT "Global Variable", CONT "Required Rank" | 2 | schema (lane B): no typed member yet |
| "one label for two members (WED0, WED1)" | CONT "Sounds (Open, Close, Select Audio Data)" | 1 | a CK edit of the field (no OW-C step names one) |
| "a navmesh import option; no header flag or member of this type's schema holds it" | CONT "Collision Geometry", STAT "Collision Geometry" | 2 | schema (lane B): no typed member yet |
| "a navmesh import option, kept in the record header flags; `set` edits subrecords only" | CONT "Bounding Box", CONT "Filter", CONT "Only Cut", CONT "Ground", STAT "Bounding Box", STAT "Filter", STAT "Only Cut", STAT "Ignore Erosion", STAT "Ground" | 9 | unscheduled: `set` edits no record header flag |
| "the OBTE block is a group of combinations, not one field" | CONT "Object Template" | 1 | schema (lane B): no typed member yet |
| "STAT's schema has no Keywords (KSIZ, KWDA) member" | STAT "Keywords" | 1 | schema (lane B): no typed member yet |
| "not known whether it edits STLS (the schema's AmbientSound)" | STAT "Looping Sound (Select Audio Data)" | 1 | a CK edit of the field (no OW-C step names one) |
| "probably the inverse of the HiddenFromLocalMap record header flag; `set` edits subrecords only" | STAT "On Local Map" | 1 | unscheduled: `set` edits no record header flag |
