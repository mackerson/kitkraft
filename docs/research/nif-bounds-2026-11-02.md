# NIF block structure and stored bounds: corpus study (M7.S4)

Pipeline R, 2026-11-02. The layout facts `crates/nif` relies on, each one found in and
checked against every official Starfield model. No description of the format was read
(`docs/source-registry.md`, "Sources for NIF and mesh layout": `nif.xml`, NifSkope source,
nifly and the GPL bridges are forbidden). Grades follow ADR 0005 Amendment 3; every claim
below is grade A (reproducible test over the whole corpus) unless it says otherwise.

**Disclosure.** The writer is an Opus model with general background knowledge of the NIF
format family from training, which may come in part from sources the registry forbids. No
such source was opened in this work. Every claim is grounded only in the counts below, and
the field names are ours: a field whose meaning the bytes do not show is named by its
place (`u8_43`, `u32_44`, `u32_52`, `after_extra`, `opaque`), never by a description.
Every field the reader reads that the corpus shows as one constant value (H2's `u8_43`
and `u32_44`, H4's `u32` after the string table, H6's `u32` 1, B2's `after_extra`) is read
as that value, and any other value is refused as a layout we do not read
(`Error::Unsupported`, with offset). B6's byte is the first slot flag of the mesh study's G1, read by `Nif::slots`. A fact we could not show from the bytes is
not claimed.

## Corpus and method

- **The models.** Every `.nif` the game ships: 120,543 entries in 13 archives, read through
  `kitkraft-ba2` (`Archive::open`, `extract_vec`) with no archive written, plus the 4 loose
  files under `Data\EditorFiles\Primitives`: **120,547 models**, 2,007,554 blocks. Per
  archive: Meshes01 31,058, MeshesPatch 29,849, LODMeshesPatch 19,540, LODMeshes 19,535,
  ShatteredSpace Main01 9,198, Meshes02 7,552, SFBGS050 Main 1,495, FaceMeshes 1,282,
  SFBGS00D Main 934, SFBGS003 Main 85, SFBGS004 Main 6, SFBGS008 Main 5, SFBGS047 Main 4.
  The total matches `docs/research/asset-formats.md` (120,543 archive entries).
- **How.** Hex dumps of a few dozen models gave hypotheses; each hypothesis became a check
  run over all 120,547; a claim is written here only when its check had no exception.
  The probes (Python and a small Rust extractor over `kitkraft-ba2`) stayed in a private
  temp folder outside the repository; no Bethesda byte is in the repository.
- **The test of record** is `crates/nif/tests/corpus.rs`
  (`every_claim_holds_on_every_reachable_official_model`). It asserts every claim below on
  the loose models under `KITKRAFT_DATA_DIR` and, when `KITKRAFT_NIF_DIR` names a folder of
  models extracted from the official archives, on those too. `kitkraft-nif` is L0 and may
  not depend on `kitkraft-ba2` (same layer), so the archive-in-place run belongs to the
  engine's `tests/mesh_corpus.rs` (M7.S4 engine side). Run 2026-11-02 over all 120,547:
  pass, 0 violations, 36 s (release).
- **Worked example** used for offsets below: `meshes\architecture\city\newatlantis\
  setdressing\na_addon_ceiling_floordecals01.nif` in `Starfield - Meshes01.ba2` (966 bytes,
  8 blocks), and the loose `EditorFiles\Primitives\VCPreviewCube01.nif`.

All numbers are little-endian. "Reference" means a `u32` block index, with `0xFFFFFFFF`
meaning none.

## File layout (claims H1 to H6)

| Claim | Offset | Content | Evidence (all 120,547 models) |
| ----- | ------ | ------- | ----------------------------- |
| H1 | 0 | The 39-byte line `Gamebryo File Format, Version 20.2.0.7` and a line feed | every model |
| H2 | 39 | `u32` `0x14020007` (its bytes spell 20, 2, 0, 7, the line's version); 43: `u8` 1; 44: `u32` 12; 48: `u32` block count *N*; 52: a `u32`: 175 (61,276), 173 (58,137), 172 (683), 174 (447), 170 (the 4 loose) | every model |
| H3 | 56 | A short string (`u8` length *n*, *n* bytes), a `u32`, two more short strings. Archived models: `00` / 3 / `00` / a 1- to 13-byte string ending in `00`; the loose ones: `BSFBXNiConverter\0` / 2 / `Static\0` / 7 bytes. Lengths of the last: 7 (98,270), 1 (20,820), 4 (535), 2 (424), 3 (282), 5 (118), 6 (93), 9 (3), 13 (2) | every model; the last string always ends in a 0 byte |
| H4 | after H3 | `u16` type count *T*; *T* names (`u32` length, bytes); *N* `u16` type indices, each below *T*; *N* `u32` block sizes; `u32` string count *S*, `u32` equal to the longest string's length, *S* strings (`u32` length, bytes); a `u32` that is 0 in every model: its meaning is not known, and the reader refuses any other value | every model; in the example the type count (5) is at 0x48, the type indices at 0xa0, the sizes at 0xb0, the string table (7 strings, longest 80) at 0xd0 |
| H5 | after H4 | The *N* block bodies, back to back, each exactly its H4 size | the header, the blocks and the footer account for every byte of every model |
| H6 | end | A `u32` that is 1 in every model (the reader refuses any other value), then a block index: 0 in every model, and the walk from it reaches every geometry (C1a), so we call it the root (example: `01 00 00 00 00 00 00 00` at 0x3be) | every model |

30 block types occur. By block count: BSGeometry and NiIntegerExtraData 484,820 each in the
archives (484,824 with the loose 4), BSLightingShaderProperty 483,837, NiNode 175,165,
BSXFlags 99,906, bhkNPCollisionObject 62,705, NiStringExtraData 61,701, bhkPhysicsSystem
42,411, BSSkin::Instance and BSSkin::BoneData 23,209, SkinAttach 22,378,
BSWeakReferenceNode 19,154, NiIntegersExtraData 15,672, then 17 types with fewer than 2,000
blocks each (BSFaceGenNiNode 1,552 down to BSValueNode 10). Only the types below are decoded.

## Node and geometry blocks (claims B1 to B6)

| Claim | Block offset | Content | Evidence |
| ----- | ------------ | ------- | -------- |
| B1 | - | The node family is `NiNode`, `BSFaceGenNiNode`, `BSWeakReferenceNode`, `NiBillboardNode`, `NiSwitchNode`, `BSBlastNode`, `BSOrderedNode`, `BSValueNode`; `BSGeometry` is the only type with a bound | 196,103 node blocks and 484,824 geometry blocks decode as below |
| B2 | 0 | The shared prefix: `u32` name (an index into the string table, or `0xFFFFFFFF`); `u32` count *k* and *k* references to extra-data blocks; a `u32` that is `0xFFFFFFFF` in every block (`after_extra`; the reader refuses any other value); a `u32` not interpreted (`opaque`: values such as `0x0000000E`, `0x0400000E`, `0x2000000F`); 3 `f32` translation; 9 `f32` rotation, row by row; `f32` scale; a reference to the collision block or none. 72 + 4*k* bytes | every node and geometry block: names in range, every extra reference is a `*ExtraData`, `BSXFlags`, `BSBound`, `BoneTranslations`, `SkinAttach` or `BSConnectPoint::*` block, every collision reference (62,705) is a `bhkNPCollisionObject`, and (grade B, for the name) the root block's `name` string equals its model's file stem, ignoring case, in 49,931 of the 120,543 archived models and contains it or is contained in it in 63,200 more (113,131, 94%), every value finite, every scale positive, every rotation's determinant within 0.01 of 1 |
| B3 | 72 + 4*k* | Nodes: `u32` child count and the child references. A `NiNode` ends there (175,169 of 175,169); the other node types carry more bytes after the list (`BSFaceGenNiNode` and `NiBillboardNode` 2, `BSBlastNode` 3, `BSValueNode` 5, `NiSwitchNode` 6, `BSOrderedNode` 17, `BSWeakReferenceNode` 96 to 781,644 in 5,378 distinct lengths; an earlier draft said 96 to 180, from a truncated listing), not decoded | every node; 3 null children in all, all in `NiNode`s |
| B4 | 72 + 4*k* | Geometry: 10 `f32`, a sphere (centre, radius) then a box (centre, half extents). Example: block 2 at 0x21c, bound at 0x268: sphere (2.0, 0.35, 0.002) radius 2.03, box centre (2.0, 0.35, 0.002) half extents (2.0, 0.35, 0.0000003): a flat decal 4 by 0.7 | every geometry block |
| B5 | same | Exactly one of three patterns. **Stored**: finite values, box half extents not negative, the two centres equal, and the radius between the largest half extent and the half diagonal (each within 0.1%): 461,059. **Sphere only**: a sphere and all six box values `f32::MAX` (`ff ff 7f 7f`): 556, mostly ship modules (307) and architecture, 53 in LOD models. **Not stored**: a zero sphere and the six `f32::MAX`: 23,209, which are exactly the geometry blocks with a skin reference (next row) | 461,059 + 556 + 23,209 = 484,824; none outside the three |
| B6 | 112 + 4*k* | Three references: skin (a `BSSkin::Instance` or none), shader (always set: `BSLightingShaderProperty` 483,841, `BSEffectShaderProperty` 983), alpha (none, or an `NiAlphaProperty`: 1,077); then a byte that is 1 in every block. The rest (the mesh list; loose models keep their geometry inline) is M7.S5's: `docs/research/mesh-layout-2026-11-02.md` G1 shows that byte is the first of four slot flags | every geometry block |

## The model's box (claim C1)

- **C1a (grade A).** Walking from the root through node children reaches every geometry
  block of every model exactly once (484,824 of 484,824; no child is reached twice), so the
  node tree is a tree and the union of geometry bounds covers every geometry. 46 node blocks
  are not reached from the root (no geometry hangs under them).
- **C1b (grade B, cross-check).** A geometry's bound is in its own frame: the model box is the
  union of each geometry's box corners carried by its own transform and then each
  ancestor's (`p' = scale * rotation * p + translation`, rotation row by row), in the units
  of `OBND`. Evidence: for the 33,684 base objects of the 14 official ESMs whose `MODL`
  names a model with a bound and that have a 24-byte `OBND` (STAT, MSTT, GBFM, MISC, ACTI,
  BOOK, ARMO, FURN, CONT and others; the patch archive's copy where a model is in two), the model box equals `OBND`
  within 1 cm on every corner for **23,261** (69%), within 5 cm for 24,822, within 1 m for
  29,233, and the median ratio of `OBND` extent to model-box extent is 1.0000. Applying only
  the ancestors' transforms (box in the parent's frame) matches 17,604 within 1 cm, and
  reading the rotation column by column matches 22,011: both rejected. The rest is not
  explained: of the 8,862 that miss by more than 5 cm, `OBND` is 0 for 1,293, and models whose geometry uses
  `BSEffectShaderProperty` miss more often (2,544 of 3,321), so the CK's `OBND` is not
  always this box. That is why C1b is a cross-check, not a claim on every model.
- **What has no box.** Of the 120,547 models, 95,281 get a box; 19,436 have no geometry
  block (mostly the scene-only files of Meshes02 and MeshesPatch), and 5,830 have only
  skinned geometry (Not stored, B5: actors, clothes, face meshes). Those are reported as
  "no bound in the model", never as a box. A sphere-only geometry contributes its sphere's
  box.

## What `crates/nif` does with this

`Nif::parse` checks H1, H2's version, H4's indices and H5/H6's exact accounting and refuses
anything else with a typed error and the offset; `Nif::node` and `Nif::geometry` decode B2
to B6 (references out of range are errors, values outside B5's three patterns are
`Bound::Unrecognised`, not a box); `Nif::model_bounds` does C1, visiting each block at most
once. Unit tests feed synthetic models (built by the test from this layout, no game bytes):
every truncation, a trailing byte, another version, out-of-range types, roots and
children, a cycle, huge counts, and a value other than the observed constant in each of
the three single-value fields (H4's `u32`, H6's `u32`, B2's `after_extra`).

## Use in the viewport (M7.R3, 2026-11-02)

The Render Window draws each reference with its base's model box (`mesh.bounds`, claim
C1) in place of the base's `OBND`. Both are in the base's own frame, in the same units
(C1b, median extent ratio 1.0000). When the model gives no box, the window falls back to
`OBND`; when neither gives one, it uses the type's shape, with a limitation. No new layout
fact is used: the viewport reads only the engine's envelope. On two official cells
(`tests/mesh_corpus.rs` `v3_mesh_bounds_for_the_bases_of_a_cell`, grade B: two cells, not
the corpus):

- `CityNewAtlantisApartmentTEMPLATE`: of 183 distinct bases, 170 have a model box (135 of
  them within 1 cm of `OBND`), and 13 are refused for no `MODL`.
- NewAtlantis 2,1: of 249 bases, 202 have a model box (178 within 1 cm), and 47 are
  refused for no `MODL`.

## Not known (limitations)

- The meaning of H2's `u8_43`, `u32_44` and `u32_52`, H3's strings, H4's `u32` after the
  string table, H6's `u32` 1, B2's `after_extra` and `opaque`, the bytes
  B3 leaves after the children of seven node types, and everything after B6's byte.
- Why `OBND` and the model box differ for 31% of base objects (C1b).
- Whether a skinned model's bound can be had without the skin data (B5: it is not stored).
