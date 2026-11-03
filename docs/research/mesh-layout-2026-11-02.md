# Starfield `.mesh` layout: corpus study (M7.S5, study part)

Pipeline R, 2026-11-02. This study records the layout facts `mesh.geometry` will need:
positions, triangle indices and normals, and how a model's geometry block names its
meshes. Every claim was found in the bytes of the official files and checked against
every one of them. No description of the format was read (`docs/source-registry.md`,
"Sources for NIF and mesh layout"). The method, the disclosure and the naming rule are
those of `docs/research/nif-bounds-2026-11-02.md`:

- the writer has general background knowledge of the format family from training;
- no forbidden source was opened;
- a field whose meaning the bytes do not show is named by its place.

Grades follow ADR 0005 Amendment 3. A claim is grade A when a test checks it on every
official file, grade B when the evidence is statistical.

## Corpus and method

- **Files.**
  - All **720,957** `.mesh` entries of the 11 archives that hold them, about 19.2 GB
    extracted. They were read in place through `kitkraft-ba2`. Some meshes exist in more
    than one archive (a base archive and its patch), and every copy is counted.
  - All 120,543 archived `.nif` models, for the slots that name the meshes.
  - Per archive: MeshesPatch 324,491; Meshes01 288,231; Meshes02 62,685; ShatteredSpace
    Main01 23,570; SFBGS050 Main 12,923; FaceMeshes 4,832; SFBGS00D Main 3,460; SFBGS003
    Main 474; SFBGS004 Main 150; SFBGS047 Main 79; SFBGS008 Main 62.
- **How.**
  - Hex dumps of the meshes of the NIF study's decal model gave the first hypotheses.
  - A strided sample of 2,000 meshes from every archive refined them.
  - Each hypothesis then became a check over every file.
  - Probes stayed in a private temp folder; no game byte is in the repository.
- **Test of record.** `crates/engine/tests/mesh_corpus.rs`
  `every_official_mesh_matches_the_layout_study`, ignored by default; run it with
  `cargo test -p kitkraft-engine --release --test mesh_corpus -- --ignored`.
  - Run 2026-11-02, about 90 to 106 s: **0 failures over 720,957 meshes** for the structural claims, each asserted at the count stated here; the normals are counted, not asserted.
  - It is written in the test, with no engine API, so `mesh.geometry` stays Deferred
    (a catalog seam).
- **Worked example.** The NIF study's decal,
  `meshes\architecture\city\newatlantis\setdressing\na_addon_ceiling_floordecals01.nif`.
  - Its block 2 (offset 0x21c) names
    `geometries\5eb2c642863e7a9bf5fd\b0f72db71f345718d55a.mesh` (3,024 bytes).
  - The offsets below are those of this file.

All numbers are little-endian.

## How a model names its meshes (G1, G2)

| Claim | Where | Content | Evidence |
| ----- | ----- | ------- | -------- |
| G1 | `BSGeometry` block, after the three references of NIF claim B6 | **Four slots.** Each is a `u8` 0 or 1; when 1, it is followed by three `u32` and a path (`u32` length, then the bytes). The block ends after the fourth slot. The NIF study's "byte that is 1 in every block" (B6) is the first slot's flag: the first slot is always present. Patterns: 1 0 0 0 in 263,524 blocks, 1 1 1 1 in 113,180, 1 1 0 0 in 59,304, 1 1 1 0 in 48,812. The third `u32` is 64 in all 981,288 slots. Paths are 41 bytes (`a\b`) in 964,697 slots and 57 bytes (`Geometries\a\b.mesh`) in 16,591. Example: block offset 0x80 holds `01`, 300, 104, 64, 41, the path, then `00 00 00` | every one of the 484,820 archived geometry blocks ends exactly after its fourth slot |
| G2 | the slot path | It names the archive entry `geometries\a\b.mesh` (the `Geometries\` prefix and `.mesh` are added when absent). Every slot's first `u32` equals its mesh's index count, and its second equals its mesh's vertex count | all 981,288 slots resolve to an existing entry (399,384 distinct meshes; 0 unresolved). Counts agree for 1,888,804 slot-and-copy pairs, 0 disagree |

The 4 loose editor primitives (`Data\EditorFiles\Primitives`) are different: their first
slot has an empty path, and the geometry bytes follow inside the NIF. That layout is not
studied here.

## The `.mesh` file (M1 to M6)

*N<sub>i</sub>* is the index count and *N<sub>v</sub>* the vertex count.

| Claim | Offset (example) | Content | Evidence (all 720,957) |
| ----- | ---------------- | ------- | ---------------------- |
| M1 | 0 | `u32` 2, then `u32` *N<sub>i</sub>*, then *N<sub>i</sub>* `u16` **triangle indices**: three per triangle, each below *N<sub>v</sub>*. Example: 2 at 0, 300 at 4, indices from 8 to 0x260 | every file starts with 2. *N<sub>i</sub>* is a multiple of 3 in every file. None of the 1,676,933,949 indices is out of range |
| M2 | after M1 (0x260) | An `f32` **scale** (8.0 in the example); a `u32` (`u32_after_scale`; 0 at 0x264); `u32` *N<sub>v</sub>* (104 at 0x268); then *N<sub>v</sub>* **positions**, each three `i16`, read as `i16 × scale / 32767` (0x26c to 0x4dc) | For each of the 461,055 archived geometries whose box is stored, every archive copy of its first slot's mesh has a position box equal to the NIF's stored box, to within two quantization steps plus 0.0001: **864,598 of 864,598** slot-and-copy pairs. Example: (0, 0, 0.00195) to (4.0001, 0.69997, 0.00195), against the stored (0.000003, 0, 0.0020) to (4.000001, 0.7, 0.0020) |
| M3 | after M2 (0x4dc) | **Five sections** of 4 bytes per vertex. Each is a `u32` count, then count × 4 bytes. The count of sections 1, 4 and 5 is always *N<sub>v</sub>*; the count of sections 2 and 3 is 0 or *N<sub>v</sub>*. Example counts: 104 at 0x4dc, 0 at 0x680, 104 at 0x684, 104 at 0x828, 104 at 0x9cc | present: section 1 in 720,957 files, section 2 in 257,992, section 3 in 343,029, sections 4 and 5 in 720,957 |
| M4 | section 4 (0x82c to 0x9cc) | **Normals.** Each word holds three 10-bit unsigned fields, low bits first, each mapped from 0..1023 to −1..1. Graded B for the meaning: the counts are statistical | of 595,630,925 vectors, 594,817,139 (99.86%) have unit length within 2%. Of the 594,720,456 that touch a non-degenerate triangle, 587,540,340 (98.8%) point the same way as the triangles' face normal (by the index order). Section 5, read the same way, also holds unit vectors (594,836,019 of 595,630,925, 99.87%) but agrees with the face normal for only 291,660,254 of 594,720,456 (49.0%), so it is not the normal |
| M5 | after M3 (0xb70) | A `u32` equal to *N<sub>v</sub>* × `u32_after_scale`, then that many 4-byte entries. Then a `u32` *k* (0 at 0xb74) and *k* lists, each a `u32` count and that many `u16`, every value below *N<sub>v</sub>* | holds in every file. 17,544 files have entries in the first part (so their `u32_after_scale` is not 0); 1,744 have lists |
| M6 | after M5 (0xb78) | Either the file ends there (6,470 files), or there is a `u32` *m* (2 at 0xb78), then *m* × 40 bytes, then 4 closing bytes (0xbcc to 0xbd0 = 3,024) | every file ends exactly there |

Named by place, meaning not shown: `u32_after_scale`; sections 1, 2, 3 and 5; M5's
4-byte entries and its `u16` lists (their values are vertex indices, but their use is not
shown); M6's 40-byte entries and its 4 closing bytes.

## Go/no-go for M7.S5 code

**GO for positions and indices (grade A); normals GO at grade B (statistical, not proven).**

- Every structural claim (G1, G2, M1 to M3, M5, M6) holds on every official mesh and
  model.
- The positions are pinned to the NIF's stored boxes on every geometry that has one.
- The normals rest on two statistical tests (grade B), both over 98%.

The code, a reader in `crates/nif` and the engine's `mesh.geometry`, is a later wave.
The catalog entry is a seam, and its acceptance also needs V4 measured. So this study adds
no STATUS line.

## Not known (limitations)

- What the fields named by place in M2, M3, M5 and M6 mean.
- The about 0.14% of section-4 vectors that are not unit length, and the 1.2% that face
  against their triangles. Whether they are deliberate (hard edges, two-sided cards) is
  not shown.
- The 4 loose primitives' inline geometry.
- Which of a base archive's and its patch's copies the game uses (M7.S4's rule applies:
  the patch).
