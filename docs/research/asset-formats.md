# Asset formats in the official archives (M4.G3, scoping)

Research, not implementation (W11's first deliverable). What the 90 official `.ba2`
archives hold, by file extension; what KitKraft does with each today; whether a source
permitted by ADR 0006 describes it; and which workstream would need to author it. No
format beyond the archive container was read for this: every number comes from `ba2.list`
(`kit --json ba2 list <archive> --limit 100000000`) over `KITKRAFT_DATA_DIR`, 2026-10-14,
aggregated by a throwaway script outside the repository (`corpus:`).

## Totals

- 90 archives, 1,763,906 entries, 148,713,870,342 bytes on disk (138.5 GiB).
- Container kinds: 65 GNRL v2 (zlib), 10 DX10 v2 (zlib), 15 DX10 v3 (LZ4 blocks; all
  `Starfield - Textures*` and `LODTextures*`). Every one lists through `ba2.list` without
  an error.
- Extracted size per extension below is the sum of `size` (what `ba2.extract` writes);
  "stored" is the sum of `stored` (bytes in the archive). Extensions are lower-cased from
  the last `.` of the entry's file name.

## By extension

| Ext | Entries | Extracted bytes | Stored bytes | Archives | What KitKraft does today | Permitted source for the format (ADR 0006) | Workstream that would author it |
| --- | ------: | --------------: | -----------: | -------: | ------------------------ | ------------------------------------------- | ------------------------------- |
| `.dds` | 144,283 | 103,775,549,416 | 63,443,882,224 | 29 | list, extract (DX10 records get a synthesized DDS header), pack (`ba2.pack --format dx10`) | header: `ms-docs:` (level 3); pixel data is not interpreted | W11: textures only as DDS the user supplies, packed |
| `.wem` | 382,013 | 31,480,817,725 | 31,480,817,725 | 31 | list, extract, pack (opaque bytes) | none | W10/W11 (voice, sound): out of M4 scope |
| `.ffxanim` | 330,723 | 23,881,561,752 | 23,881,561,752 | 11 | list, extract, pack | none | W10 (lip sync): out of scope |
| `.mesh` | 720,957 | 19,197,559,374 | 11,134,604,464 | 11 | list, extract, pack | none | W11: no mesh authoring (clones reuse vanilla models) |
| `.btd` | 2,258 | 15,843,651,053 | 15,843,651,053 | 9 | list, extract, pack | none | terrain: out of scope ("Not in M4") |
| `.nif` | 120,543 | 1,487,827,892 | 831,003,501 | 13 | list, extract, pack; `crates/nif` is a placeholder (no reader yet) | none | W11: read-only metadata at most |
| `.dat` | 1,357 | 1,217,593,167 | 622,399,488 | 11 | list, extract, pack | none | none known |
| `.biom` | 1,452 | 951,656,620 | 31,536,625 | 2 | list, extract, pack | none | W7: procedural biome data (`Starfield - PlanetData.ba2`); inventoried, not read |
| `.af` | 20,370 | 264,263,250 | 191,514,489 | 6 | list, extract, pack | none | W9 (animation): out of scope |
| `.cdb` | 8 | 228,107,823 | 140,646,794 | 8 | list, extract, pack | none | W11: materials live here (`Starfield - Materials.ba2` is one `.cdb`); no `.mat` files exist in any official archive |
| `.fxp` | 2 | 205,399,857 | 205,399,857 | 2 | list, extract, pack | none | none (compiled shaders) |
| `.txt` | 9,436 | 155,646,037 | 68,467,186 | 8 | list, extract, pack | none | none |
| `.swf` | 263 | 125,303,509 | 125,303,509 | 3 | list, extract, pack | none | W11 (interface): out of scope |
| `.ilstrings` | 126 | 111,079,525 | 111,079,525 | 13 | read (`strings.dump`), merge, list, extract, pack | `uesp-skyrim:` String Table page, corpus-confirmed | W12 (localized output is not in M4) |
| `.psfx` | 831 | 107,696,517 | 107,696,517 | 1 | list, extract, pack | none | W11 (particles): out of scope |
| `.json` | 28 | 98,855,077 | 98,855,077 | 9 | list, extract, pack | none | none |
| `.bnk` | 38 | 76,664,520 | 76,664,520 | 8 | list, extract, pack | none | W11 (Wwise banks): out of scope |
| `.blob` | 11 | 47,081,468 | 47,081,468 | 6 | list, extract, pack | none | none known |
| `.csv` | 14 | 28,149,780 | 28,146,634 | 2 | list, extract, pack | none | none |
| `.agx` | 378 | 22,359,414 | 2,444,110 | 5 | list, extract, pack | none | W9 (animation graphs): out of scope |
| `.strings` | 126 | 19,655,804 | 19,655,804 | 13 | read, merge, list, extract, pack | `uesp-skyrim:` String Table page | W12 |
| `.dlstrings` | 126 | 13,731,264 | 13,731,264 | 13 | read, merge, list, extract, pack | `uesp-skyrim:` String Table page | W12 |
| `.pex` | 5,708 | 11,850,248 | 11,850,248 | 5 | read and disassemble (`papyrus.disasm`), scan (`validate`), compile through Bethesda's compiler, pack | `ours: pexdis.py` + corpus | W6: scripts compiled by `papyrus.compile`, packed or loose |
| `.afx` | 20,315 | 11,243,863 | 4,339,256 | 6 | list, extract, pack | none | W9: out of scope |
| `.gfx` | 42 | 5,269,067 | 5,269,067 | 3 | list, extract, pack | none | W11 (interface): out of scope |
| (none) | 11 | 3,790,723 | 2,168,647 | 8 | list, extract, pack | none | none |
| `.png` | 114 | 2,107,771 | 2,107,771 | 1 | list, extract, pack | none | W11 (interface): out of scope |
| `.rig` | 442 | 1,473,831 | 648,636 | 6 | list, extract, pack | none | W9: out of scope |
| `.bgsdb` | 3 | 1,132,220 | 431,350 | 3 | list, extract, pack | none | none known |
| `.fbx` | 1 | 682,096 | 682,096 | 1 | list, extract, pack | none | none |
| `.ma` | 1 | 627,748 | 129,577 | 1 | list, extract, pack | none | none |
| `.xtbl` | 12 | 152,098 | 14,185 | 1 | list, extract, pack | none | none |
| `.lod` | 1,903 | 38,060 | 33,486 | 5 | list, extract, pack | none | terrain/LOD: out of scope |
| `.ffxactor` | 4 | 13,344 | 13,344 | 2 | list, extract, pack | none | W10: out of scope |
| `.ffxbones` | 4 | 1,752 | 1,752 | 2 | list, extract, pack | none | W10: out of scope |
| `.bin` | 1 | 2,112 | 2,112 | 1 | list, extract, pack | none | none |
| `.rx` | 1 | 78 | 78 | 1 | list, extract, pack | none | none |
| `.bat` | 1 | 40 | 40 | 1 | list, extract, pack | none | none |

"list, extract, pack" means the bytes move through `ba2.list`, `ba2.extract` and
`ba2.pack` unchanged; KitKraft does not read inside them. A row with "none" has no
permitted description: reading such a format would need an ADR 0006 amendment naming a
source (level 1 corpus statistics alone do not describe semantics) before any reader is
written.

## By archive group

| Archives | Kind | Content (top extensions by entries) |
| -------- | ---- | ----------------------------------- |
| `Starfield - Textures01..11`, `TexturesPatch01..02`, `LODTextures01..02` (15) | DX10 v3, LZ4 | `.dds` only |
| `<Plugin> - Textures` (9: Constellation, OldMars, SFBGS003/004/006/00D/047/050, ShatteredSpace) | DX10 v2, zlib | `.dds` only |
| `Starfield - Meshes01..02`, `MeshesPatch`, `FaceMeshes` | GNRL | `.mesh` (geometry), `.nif` (scene files), a few `.dat` |
| `Starfield - FaceAnimation01..04`, `FaceAnimationPatch` | GNRL | `.ffxanim` (plus 2 `.ffxactor`, 2 `.ffxbones` each in 04 and the patch) |
| `Starfield - Voices01..02`, `VoicesPatch`, `<Plugin> - Voices_<lang>` | GNRL | `.wem` only |
| `Starfield - WwiseSounds01..05`, `WwiseSoundsPatch` | GNRL | `.wem`, `.bnk`, a few `.json` |
| `Starfield - Terrain01..04`, `TerrainPatch` | GNRL | `.btd` only |
| `Starfield - Animations` | GNRL | `.af`, `.afx`, `.txt`, `.agx`, `.rig`, `.xtbl`, `.ma`, `.csv` |
| `Starfield - Interface` | GNRL | `.swf`, `.gfx`, `.txt`, `.png`, `.json`, `.bat` |
| `Starfield - Misc` | GNRL | 4,740 `.pex`, 11 `.csv`, `.dat`, `.bin` |
| `Starfield - Materials` | GNRL | one `.cdb` |
| `Starfield - Shaders`, `CreationKit - Shaders` | GNRL | one `.fxp` each |
| `Starfield - Particles` | GNRL | `.psfx` |
| `Starfield - PlanetData` | GNRL | `.biom` |
| `Starfield - DensityMaps` | GNRL | `.dds` (7,761) |
| `Starfield - GeneratedTextures` | DX10 v2 | `.dds` |
| `Starfield - LODMeshes`, `LODMeshesPatch` | GNRL | `.nif`, `.lod`, one `.bgsdb` |
| `<Plugin> - Main`, `ShatteredSpace - Main01..02` | GNRL | everything a plugin ships besides textures and voices: `.mesh`, `.ffxanim`, `.wem`, `.pex`, string tables, `.cdb`, `.btd`, ... |
| `<Plugin> - Localization`, `Starfield - Localization` | GNRL | string tables only (9 languages x 3 kinds) |

Naming of the archives against their plugins is M4.G4 (`docs/corpus.md`, "Archive names").

## What this means for the owner's workstreams

- **W6 (scripts)**: `.pex` is the only executable content a data mod ships; KitKraft
  compiles through Bethesda's compiler and packs or copies the result. Nothing else is
  needed from this inventory.
- **W11 (UI, maps, visual assets, audio)**: no format a W11 deliverable would author has a
  permitted description today (`.nif`/`.mesh`, the `.cdb` material database, `.swf`/`.gfx`
  interface files, `.wem`/`.bnk` audio, `.psfx` particles). KitKraft's role stays: pack
  what the user supplies (DDS textures through DX10 packing, other files through GNRL),
  and name the archives as the game loads them (M4.G4). `crates/nif` keeps its read-only
  metadata scope and has no M4 work (execution plan W11).
- **W7 (planets)**: biome data outside the records lives in `.biom` files in
  `Starfield - PlanetData.ba2` (1,438) and `ShatteredSpace - Main01.ba2` (14); inventoried
  only. Whether procedural placement depends on them is M4.D3's open question.
- **Materials**: the plan's `.mat` row has no counterpart in the official archives; the
  materials ship as a component database (`.cdb`). A material edit would mean reading
  that database, which no permitted source describes.

## Reproduce

Build `kit` (`cargo build --release -p kit-cli`), then for each `<Data>\*.ba2` run
`kit --json ba2 list "<archive>" --limit 100000000` and sum `data.entries[].size` and
`.stored` by the lower-cased extension of `name`. About 3 minutes on the dev box.
