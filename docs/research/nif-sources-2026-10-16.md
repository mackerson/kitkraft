# Sources for Starfield NIF and mesh layout (memo for M7.O5)

Lane B, 2026-10-16, preparing M7.S4. A memo for the owner's ruling M7.O5: which sources
may describe Starfield's `.nif` and geometry layout, under which license category, and
what each would give M7.R3 (bounding boxes) and M7.S5 (positions, indices, normals). No
code was written and no code was downloaded. Public repository front pages, license files
and community articles were read through a web fetch.

**Disclosure.** To check whether niftools' `nif.xml` covers Starfield, a web fetch
summarized the first part of that file: it reported only that the file has Starfield
version identifiers (a version entry and version-expression tokens for Starfield). No
block or field layout was read. Given its GPL-3.0 license (below), lane B does not read it
further, and that glimpse informs nothing in KitKraft.

## What we already know from our own corpus

From `docs/research/asset-formats.md` (`ba2.list` over the official archives,
`corpus:`): 120,543 `.nif` entries (1.49 GB extracted) and 720,957 `.mesh` entries
(19.2 GB extracted) in 11 to 13 archives. Starfield keeps geometry outside the `.nif`:
the `.mesh` files are where vertices and indices live, and the `.nif` holds the scene
graph and references to them. A community article (below) says the same; our counts are
the evidence.

The plugins carry bounds too: the base objects M7 places (`STAT`, `CONT`, `ACTI`,
`MSTT`, `FURN`, `DOOR`, `MISC`, `LIGH`, ...) have an `OBND` subrecord (object bounds,
two corners, each three 32-bit floats in Starfield, 24 bytes), typed in our schema (`crates/schema/defs/common/ObjectBounds.toml`,
cited to the pinned xEdit file). That is a box per base object with no NIF at all.

## Candidates

| Source | What it is | License | Would give | Category by analogy (ADR 0004/0005/0006) |
| ------ | ---------- | ------- | ---------- | ---------------------------------------- |
| `OBND` in the plugins | Object bounds of the base record | Bethesda data, read in place; layout from the pinned xEdit file (MPL-2.0, approved) | An axis-aligned box per base object, in plugin units, for every placed reference | Already permitted (ADR 0004, `corpus:`) |
| Our own corpus study of `.nif`/`.mesh` | Hex dumps and statistics of the official files, read in place (`ba2.extract` to a temp folder) | Bethesda data, never committed | Whatever the bytes prove: header, block list, the mesh reference, then vertex and index buffers, each fact validated on every occurrence | Level 1 under ADR 0006 §1 (corpus is the tiebreaker); slow but clean |
| niftools `nif.xml` (<https://github.com/niftools/nifxml>) | Declarative XML description of the NIF format, with Starfield versions | **GPL-3.0** (repository `LICENSE`, 2026-10-16) | Block and field layout of Starfield NIFs, the external-mesh reference, perhaps bounds fields | Closest analogy: Mutagen's record XML, a GPL description of Bethesda formats, which the registry keeps **never read**. AGENTS.md "Clean room": no GPL code that handles Bethesda formats. So **forbidden** unless the owner rules otherwise (an owner decision of the kind ADR 0004 reserved: a materially different license) |
| NifSkope (<https://github.com/niftools/nifskope>) | NIF viewer and editor; upstream lists no Starfield support | BSD-3-Clause (`LICENSE.md`) | Nothing for Starfield upstream | Its source is another implementation of the format: ADR 0006 §2 forbids reading those "whatever their license". As a program: external-oracle-only, but it does not open Starfield files |
| fo76utils' NifSkope fork (<https://github.com/fo76utils/nifskope>) | NifSkope 2.0.dev9 fork with Starfield support (materials, format changes, command-line options for Starfield geometry) | Not stated on the front page; a `LICENSE.md` is linked, presumably NifSkope's BSD-3 (not checked) | Run as a program: a second opinion on a model's bounds, vertex and triangle counts, and the meshes a `.nif` references, compared with ours | **external-oracle-only (run, never read)**, like SF1Edit; installed by the owner, never bundled; its license to be confirmed before any run |
| Community format notes, e.g. the Nexus Mods article on the organization of Starfield's meshes (<https://www.nexusmods.com/starfield/articles/268>) | Prose overview: geometry moved to `.mesh` files under `geometries\<folder>\<name>.mesh`, the `.nif` keeps transforms, references, physics and bones, `BSGeometry` points at external meshes; it says it does not go into technical detail | Site terms; reuse license not established | Where to look, not layouts | **docs-only** (like ORTHAM, BML-*): what to test, never cited as a layout source |
| Blender bridges (the Starfield geometry bridge credited to SesamePaste233, its Void SFG Bridge fork), nifly, PyNifly | Importers and exporters of Starfield `.nif`/`.mesh` | GPL family (Blender add-ons are GPL by Blender's terms; nifly GPL-3.0) | Full layouts | **forbidden** (GPL code handling Bethesda formats) |

## Recommendation

1. **M7.R3 (boxes) needs no NIF source.** Draw each placed reference's box from its base
   object's `OBND`, which is typed already; show "no bounds" as a limitation for a base
   without `OBND`. This unblocks M7.R3 without M7.O5 if the director agrees, and keeps
   `kitkraft-nif` out of the box path.
2. **For geometry (M7.S5), rule the corpus study as the only layout source**: our own hex
   dumps and statistics of `.nif` and `.mesh` from the official archives, each fact
   validated on every occurrence (ADR 0006 §1 level 1), with community notes as docs-only
   pointers. Record a NIF row set in `docs/source-registry.md`: NIFXML forbidden,
   NIFSKOPE (upstream) forbidden to read, NIFSKOPE-SF (the fork) external-oracle-only once
   its license is confirmed, NEXUS-MESH-NOTES docs-only, the GPL bridges and nifly
   forbidden.
3. **Do not approve `nif.xml`** as a reference, even read-only: it is GPL-3.0 and the
   repository's rule for GPL descriptions of Bethesda formats (Mutagen's XML) is "never
   read". If the owner wants it anyway, that is a new ADR 0004-style approval with a pinned
   revision, and lane B would still write layouts only after corpus validation.
4. Expect M7.S5 to be a research ticket first: the `.mesh` layout (vertex encoding,
   index width, normals) is not described by any permitted source, so the first deliverable
   is a corpus study in `docs/research/`, then code.

## Owner decision asked (M7.O5)

- Approve recommendations 1 to 3 (the source categories above), or
- name a different category for `nif.xml` or the NifSkope fork.
