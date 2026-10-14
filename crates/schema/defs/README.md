# Record definition format

One TOML file per record type, named `<SIGNATURE>.toml` (`WEAP.toml`, `NPC_.toml`). The
loader (`kitcraft_schema::load_dir` / `load_sources`) rejects anything that breaks the rules
below, with the file name and a path such as `member[3].field[1]`. ADR 0003 explains why
definitions are data.

## Provenance (mandatory)

ADR 0005 (`plan/adr/0005-spec-sources-per-fact.md`) decides which source is permitted for
which kind of fact. Every file names its source in one of the ADR's forms, and the loader
rejects any other prefix in `source` and `evidence`:

- `uesp-skyrim: <page URL>`: shared container facts from the UESP Skyrim pages, confirmed
  against the corpus (TES4, GRUP).
- `corpus: <evidence note>`: inventory facts from our own scans of the official ESMs
  (signatures, sizes, counts, order).
- `ck-diff: <observation id>`: Starfield field layouts from Creation Kit differential
  observation (`docs/ck-observations.md`).
- `ck-wiki: <page title>`: field names and meanings, in our own words.

Further citations go in `evidence = ["corpus: ...", ...]`, and each file opens with a
comment saying what each source contributed. Never paste prose from any source; record
structured facts only. Never consult Mutagen, Spriggit (source or its YAML field names), or
xEdit for names or layouts.

## Top level

| Key         | Required | Meaning                                                            |
| ----------- | -------- | ------------------------------------------------------------------ |
| `signature` | yes      | Four printable ASCII characters; must match the file name          |
| `name`      | yes      | PascalCase name of the record type                                 |
| `source`    | yes      | Primary source, in an ADR 0005 form (see above)                    |
| `kind`      | no       | `record` (default) or `header` (fixed binary header, e.g. `GRUP`)  |
| `evidence`  | no       | Further sources, same forms                                        |
| `note`      | no       | Free text (our own words)                                          |
| `flags`     | no       | Record-header flag bits for this type: `[["Master", 0x1], ...]`    |
| `member`    | no       | Ordered list of members (records only)                             |
| `size`      | headers  | Total header size in bytes; must equal the sum of `field` sizes    |
| `field`     | headers  | Ordered list of fields (headers only)                              |

## Members

`[[member]]` entries are listed **in on-disk order**. Each member is exactly one of:

**Subrecord** (`sig = "EDID"`): `name` (PascalCase, unique in its list after snake_case
conversion; `Extra` is reserved), `type` (see Types), `card`, optional `size`,
`size_by_version`, `note`, plus the type keys (`targets`, `count`, `element`, `base`,
`values`, `bits`, `field`).

**Group** (`group = "Master"`): a run of consecutive subrecords that belong together. The
group's members go in nested `[[member.member]]` tables. Keys: `card`, `note`. A group must
start with a subrecord or another group.

**Any-until** (`any_until = "BFCE"`, `name = "Body"`): any number of subrecords of any
signature, kept as raw bytes, up to (not including) the first subrecord with the given
signature. A subrecord member with that signature must follow later in the same list. Use
it for blocks whose contents are not modelled yet.

`card` is `required` (exactly once, the default), `optional` (0..1) or `repeated` (0..n,
consecutive). The same signature may appear at two positions if the members have different
names (WEAP has two `FLLD` slots).

## Sizes

- `size = N` on a subrecord declares the payload size. For fixed layouts it must equal the
  computed size; declaring a size on a variable layout (zstring, rest arrays) is an error.
  On `type = "opaque"` it *is* the opaque length.
- When a layout changes between form versions, gate fields with `since = V` / `until = V`
  (inclusive) and declare `size_by_version = [{ max = 99, size = 4 }, { min = 100, size = 12 }]`.
  Each declared size is checked at its range bounds. On a variable or opaque layout the
  entries are recorded facts only.

## Types

| `type`    | Size    | Extra keys                         | Notes                                                    |
| --------- | ------- | ---------------------------------- | -------------------------------------------------------- |
| `u8` `i8` `u16` `i16` `u32` `i32` `u64` `i64` `f32` `f64` | 1-8 | - | Little-endian                                  |
| `formid`  | 4       | `targets = ["KYWD", ...]`          | Omitted `targets` means the target type is not known yet |
| `sig`     | 4       | -                                  | Four raw characters (`char[4]`)                          |
| `zstring` | varies  | -                                  | NUL-terminated; must be last in a struct                 |
| `lstring` | varies  | -                                  | String-table ID if the plugin is localized, else zstring |
| `enum`    | base    | `base`, `values = [["Top", 0]]`    | Integer base; unknown values are preserved               |
| `flags`   | base    | `base`, `bits = [["Master", 0x1]]` | Unsigned base; unknown bits are preserved                |
| `array`   | n*elem  | `element`, `count`, element keys   | `count = N` or `"rest"` (fills the payload, must be last) |
| `struct`  | sum     | `field = [{ name, type, ... }]`    | Fields in order                                          |
| `opaque`  | `size`  | `size` (omit for "rest of data")   | Bytes we do not decode yet. Never guess a layout         |
| `empty`   | 0       | -                                  | Zero-length marker subrecord                             |

Array elements must be fixed-size and non-zero (scalars, `formid`, `sig`, `enum`, `flags`,
fixed `opaque`, or a `struct` of those). Element keys (`targets`, `base`, `values`, `bits`,
`field`) sit on the array itself. Enum and flag names are PascalCase and `Other` is
reserved. Fields take `name`, `type`, the type keys, `since`, `until` and `note`.

## Rules of thumb

- Unknown or ambiguous layouts are `opaque`, never guessed. A partial definition is fine.
- Field and member names are the public query vocabulary (`WFIR.ShotsPerSecond`); renaming
  one is a breaking change.
- Order comes from the source page or the corpus. Where two members never occur together,
  their relative order is unknown; say so in a `note`.

## Current set

| File        | Kind      | Source                                   | Layouts                                   |
| ----------- | --------- | ---------------------------------------- | ----------------------------------------- |
| `TES4.toml` | record    | uesp-skyrim TES4 page + corpus           | HEDR/CNAM/SNAM/MAST/DATA/ONAM/INTV/INCC typed; TNAM/BNAM/CHGL opaque |
| `GRUP.toml` | header    | uesp-skyrim Mod File Format page + corpus | Fully typed (label kept as 4 raw bytes)  |
| `GMST.toml` `GLOB.toml` `KYWD.toml` | record | corpus scan | Inventory only: order and cardinality; every payload opaque |
| `WEAP.toml` | record    | corpus scan                              | Inventory only; layout pending ADR 0005   |

Inventory definitions name members after their signature (`Edid`, `Wfir`, `Flld2`). These
are placeholders: giving a member a real name later is a breaking change for queries.

## Example

```toml
signature = "TES4"
name = "PluginHeader"
source = "uesp-skyrim: https://en.uesp.net/wiki/Skyrim_Mod:Mod_File_Format/TES4"

[[member]]
sig = "HEDR"
name = "Header"
type = "struct"
size = 12
field = [
  { name = "Version", type = "f32" },
  { name = "RecordCount", type = "u32" },
  { name = "NextObjectId", type = "u32" },
]

[[member]]
group = "Master"
card = "repeated"

[[member.member]]
sig = "MAST"
name = "FileName"
type = "zstring"
```

`crates/schema/tests/fixtures/ZZZZ.toml` is a synthetic file that uses every feature.
