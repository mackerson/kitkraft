# Materials and material swaps: what the corpus and installed mods show (2026-10-23)

Research, not implementation. Written for KitKraft from the `everyday-outfits` pipeline's
work (`C:\Modding\everyday-outfits\docs\material-swaps.md` and `mat-format.md` there).
Source categories (ADR 0004 hierarchy): the `LMSW` facts are `corpus:` (official ESMs,
read with `kit inspect`); the `.mat` facts come from third-party mod files written by the
Creation Kit's material editor, read as data. **The owner has not ruled on that category**
(it resembles CK differential observation, level 3, but the files are other authors'
output, not ours); until then treat it as level 4, secondary, `docs-only`. No mod file was
copied into either repository and no tool source was read.

## LMSW: layered material swap (`corpus:`)

- 17,410 records in the official files (12,919 in `Starfield.esm`); parsed, not typed
  (`docs/record-parity.md`).
- Body: `EDID` and one `REFL` subrecord. `REFL` is Bethesda's reflection serialization:
  a `BETH` header (`08000000 04000000 0B000000` after the tag in `LMS_Clothes_Matteo_Hat`,
  `Starfield.esm:1991B1`), a `STRT` string table (`BGSLayeredMaterialSwap`, `Keywords`,
  `Entries`, `SwapsCollection`, `BGSLayeredMaterialSwap::Entry`, `Key`,
  `KeyMaterialPathForm`, `OverrideMaterialPath`, `BSBind::Snapshot`, `Properties`), then
  `TYPE`, `CLAS`, `OBJT` and `LIST` chunks. The `LIST` payload carries length-prefixed
  material paths; in `1991B1`, one entry maps
  `Clothes\Outfit_Sanctum_Priest\Outfit_Sanctum_Priest_Hat.mat` to
  `Clothes\Outfit_Sanctum_Priest\Outfit_Sanctum_Priest_Hat_Ajun.mat`.
- Used by clothing through the ARMO object template: `OBTS` property 14 holds an `LMSW`
  form. `Clothes_Mei_Devine_NavyTan` (`1F1DC9`) and `_CreamBlue` (`1F1DCE`) list the same
  four ARMAs and differ only there (`1F1DCA` against `1F1DCB`). Some ARMOs also carry
  `MLM2` swaps on their world model.
- `clone` copies an `LMSW` today (dry run, 2026-10-23); `set` cannot edit the paths.
  Request 7 in `plan/STATUS.md` asks lane B to type it. A typed `REFL` reader would also
  serve every other record type that stores reflection data.

## `.mat` files (mod data, owner ruling pending)

- No `.mat` file exists in any official archive (`docs/research/asset-formats.md`);
  vanilla materials live in `Starfield - Materials.ba2`'s `.cdb`. Mods ship `.mat` files
  in their `- Main.ba2` or loose under `Data\Materials\` (610 in five clothing mods), and
  name them from `.nif` files and `LMSW` entries by path.
- Format: JSON. Two shapes: minimal (`Objects`, `Version`; 430 of 610) and editor output
  (adds `Filename`, `Import`, `Summary`; 177). `Objects` is a graph: a root material
  object without an ID, inheriting a vanilla material by path or
  `materials\layered\root\layeredmaterials.mat`, and ID'd layer, UV stream, material,
  texture set and blender objects under `materials\layered\root\*.mat`. Texture sets list
  files by slot: 0 color, 1 normal, 2 opacity, 3 rough, 4 metal, 5 ao, 6 height,
  7 emissive, 10 mask.
- IDs: `res:<A>:<B>:<C>`; `<B>:<C>` is per author (286 distinct pairs), not a path hash.
  File-resource IDs put the extension in the third word (`mat` = `0074616D`).

## What KitKraft could do with this

1. **Type `LMSW`** (Request 7), and so the `REFL` container.
2. **Validate packaged materials** (proposed Request 8): for every `.mat` a package ships,
   check that it parses as JSON, that object IDs are unique, that every ID it references
   is defined in the file or is a known vanilla or root path, that every texture it names
   is in the package or the load order's archives, and that no two plugins' materials
   define the same object ID. `validate` already checks plugin links the same way.
3. **Resolve materials in `inspect`**: from an ARMA's models to `.nif`, `.mat`, `.mesh`
   and textures, as `C:\Modding\everyday-outfits\scripts\extract_vanilla.py` does by
   string scan today. A real NIF reader (Request 4) would replace the scan.
4. **`docs/research/asset-formats.md`**: the `.mat` row can say mods ship JSON `.mat`
   files, with a pointer here.
