# Record definition format

One TOML file per record type, named `<SIGNATURE>.toml` (`WEAP.toml`, `NPC_.toml`), plus
fixed binary headers (`GRUP.toml`, `RECORD.toml`) and shared definitions in `common/`
(`common/BfcbBlock.toml`). The loader (`kitcraft_schema::load_dir` / `load_sources`)
rejects anything that breaks the rules below, with the file name and a path such as
`member[3].field[1]`. ADR 0003 explains why definitions are data.

## Provenance (mandatory)

ADR 0005 (`plan/adr/0005-spec-sources-per-fact.md`, Amendment 2) sets the source
hierarchy: container facts from UESP's Skyrim pages validated on the corpus, record
definitions from upstream xEdit at a pinned revision (ADR 0004), empirical validation
(corpus, CK differential observation, SF1Edit comparison), and secondary references. Every
file names its source in one of these forms, and the loader rejects any other prefix in
`source` and `evidence`:

- `uesp-skyrim: <page URL>`: shared container facts from the UESP Skyrim pages
  (`https://en.uesp.net/wiki/Skyrim_Mod:...`), confirmed against the corpus (TES4, GRUP,
  the record header, string types). Never for Starfield record bodies. Should carry
  `confidence = "corpus"` or better; `reference` is rejected (see Confidence).
- `xedit: wbDefinitionsSF1.pas@<sha>#L<start>-L<end>`: a Starfield record definition read
  from upstream xEdit's `Core/wbDefinitionsSF1.pas`. `<sha>` must be the revision ADR 0004
  pins, `9fb016884bec138ea6c7b872cec831537d464c3e` (full 40 lower-case hex); the line range
  is 1-based and inclusive. Cite the record's own range in `source` and any helper ranges
  in `evidence`. Requires `confidence`. Facts only: no code, comments or labels copied
  wholesale; see `docs/xedit-reference.md` for the construct-to-TOML map.
- `uesp-starfield: <page URL>`: the UESP Starfield pages at
  `https://starfieldwiki.net/wiki/Starfield_Mod:Mod_File_Format...`, a secondary reference.
  The loader rejects a citation of the `/CTDA` page; cite xEdit at the pin instead.
- `corpus: <evidence note>`: facts from our own scans of the official ESMs (signatures,
  sizes, counts, order) and differential comparisons against SF1Edit on fixtures (name the
  SF1Edit build).
- `ck-diff: <observation id>`: Creation Kit differential observation
  (`docs/ck-observations.md`).
- `ck-wiki: <page title>`: field names and meanings, in our own words.

Further citations go in `evidence = ["corpus: ...", ...]`, and each file opens with a
comment saying what each source contributed. Never paste prose or code from any source;
record structured facts only. Never consult Mutagen or Spriggit (source or its YAML field
names) for anything. Never substitute a Skyrim definition for a Starfield record body.

### Confidence

`confidence` says how far a definition has been checked (ADR 0005 Amendment 2):

| Value       | Meaning                                                      | Needs a citation of        |
| ----------- | ------------------------------------------------------------ | -------------------------- |
| `reference` | From xEdit only, not yet confirmed                           | `xedit:`                   |
| `corpus`    | Sizes, counts and order confirmed on the 14 official ESMs    | `corpus:`                  |
| `ck-diff`   | Confirmed by CK differential observation                     | `ck-diff:`                 |
| `verified`  | Both of the above                                            | `corpus:` and `ck-diff:`   |

It is required when any citation is `xedit:`, optional otherwise, and checked against the
citations whenever it is present; `reference` is rejected next to a `uesp-skyrim:`
citation. ADR 0005 Amendment 2 also wants it on every `uesp-skyrim:` definition; the loader
will require that once `TES4`, `GRUP` and `RECORD` carry it (a defs change also means
rerunning `cargo xtask codegen`; see `plan/STATUS.md`). The loader validates `confidence`;
the typed model does not carry it yet.

## Files

| File                     | `kind`              | Keyed by                              |
| ------------------------ | ------------------- | ------------------------------------- |
| `<SIG>.toml`             | `record` (default)  | `signature`, which must match the stem |
| `<SIG>.toml`             | `header` with `signature` | `name`; the signature must match the stem (`GRUP.toml`) |
| `<UPPER>.toml`           | `header` without `signature` | `name`; the stem is any upper-case word (`RECORD.toml`) |
| `common/<Name>.toml`     | `group` or `type`   | `name`, which must match the stem     |

The 24-byte record header lives in `RECORD.toml`: it has no fixed signature (its first
field is the record's own), so it is a header without one. `GRUP.toml` is the group
header. Headers generate a struct named after `name` (`RecordHeader`, `GroupHeader`).

`src/lib.rs` embeds every file in `BUILTIN_DEFINITIONS`; `tests/defs.rs` fails with the
exact lines to add or remove when that list and this directory differ.

## Top level

| Key         | Required        | Meaning                                                         |
| ----------- | --------------- | --------------------------------------------------------------- |
| `signature` | records         | Four printable ASCII characters; optional on headers; not allowed on shared definitions |
| `name`      | yes             | PascalCase name                                                 |
| `source`    | yes             | Primary source, in an ADR 0005 form (see above)                 |
| `kind`      | no              | `record` (default), `header`, `group` or `type`                 |
| `evidence`  | no              | Further sources, same forms                                     |
| `confidence`| with `xedit:` citations | `reference`, `corpus`, `ck-diff` or `verified` (see Confidence) |
| `note`      | no              | Free text (our own words)                                       |
| `flags`     | no (records)    | Record-header flag bits for this type: `[["Master", 0x1], ...]` |
| `member`    | records, groups | Ordered list of members                                         |
| `size`      | headers         | Total header size in bytes; must equal the sum of `field` sizes |
| `field`     | headers         | Ordered list of fields                                          |
| `type` ...  | types           | A shared type: `type` plus its type keys, as on a field         |

## Members

`[[member]]` entries are listed **in on-disk order**. Each member is exactly one of:

**Subrecord** (`sig = "EDID"`): `name` (PascalCase, unique in its list after snake_case
conversion; `Extra` is reserved), `type` (see Types), `card`, optional `size`,
`size_by_version`, `note`, plus the type keys (`targets`, `count`, `element`, `base`,
`values`, `bits`, `field`).

**Group** (`group = "Master"`): a run of consecutive subrecords that belong together. The
group's members go in nested `[[member.member]]` tables. Keys: `card`, `note`. A group must
start with a subrecord or another group.

**Shared group** (`group_ref = "BfcbBlock"`): the group defined in
`common/BfcbBlock.toml`, expanded in place. Keys: `card`, `note`, and optional `name`
(default: the shared name; set it to use one shared group twice in a list). Codegen emits
the group's type once, named after the shared definition.

**Any-until** (`any_until = "BFCE"`, `name = "Body"`): any number of subrecords of any
signature, kept as raw bytes, up to (not including) the first subrecord with the given
signature. A subrecord member with that signature must follow later in the same list. Use
it for blocks whose contents are not modelled yet.

`card` is `required` (exactly once, the default), `optional` (0..1) or `repeated` (0..n,
consecutive). The same signature may appear at two positions if the members have different
names (WEAP has two `FLLD` slots).

## Sizes

- `size = N` on a subrecord declares the payload size. For fixed layouts it must equal the
  computed size; declaring a size on a variable layout (strings, rest or `count_from`
  arrays, `if_remaining` fields) is an error. On `type = "opaque"` it *is* the opaque
  length.
- When a layout changes between form versions, gate fields with `since = V` / `until = V`
  (inclusive) and declare `size_by_version = [{ max = 99, size = 4 }, { min = 100, size = 12 }]`.
  Ranges must not overlap. Each declared size is checked at every version inside its range
  where the layout can change: the range bounds and every gated field's `since`,
  `since - 1`, `until` and `until + 1`. On a variable or opaque layout the entries are
  recorded facts only.

## Types

| `type`    | Size    | Extra keys                         | Notes                                                    |
| --------- | ------- | ---------------------------------- | -------------------------------------------------------- |
| `u8` `i8` `u16` `i16` `u32` `i32` `u64` `i64` `f32` `f64` | 1-8 | - | Little-endian                                  |
| `formid`  | 4       | `targets = ["KYWD", ...]`          | Omitted `targets` means the target type is not known yet |
| `sig`     | 4       | -                                  | Four raw characters (`char[4]`)                          |
| `zstring` | rest    | -                                  | NUL-terminated; must be last in a struct                 |
| `lstring` | rest    | -                                  | String-table ID (`u32`) if the plugin is localized, else zstring; must be last |
| `wstring` | 2 + n   | -                                  | `u16` length, then that many bytes, no terminator        |
| `bstring` | 1 + n   | -                                  | `u8` length, then that many bytes, no terminator         |
| `enum`    | base    | `base`, `values = [["Top", 0]]`    | Integer base; unknown values are preserved               |
| `flags`   | base    | `base`, `bits = [["Master", 0x1]]` | Unsigned base; unknown bits are preserved                |
| `array`   | n*elem  | `element`, `count` or `count_from`, element keys | See Arrays                                 |
| `struct`  | sum     | `field = [{ name, type, ... }]`    | Fields in order                                          |
| `opaque`  | `size`  | `size` (omit for "rest of data")   | Bytes we do not decode yet. Never guess a layout         |
| `empty`   | 0       | -                                  | Zero-length marker subrecord                             |
| `ref:Name`| shared  | - (a member may add `size`)        | The shared type `common/Name.toml`                       |

`wstring` and `bstring` follow the string conventions on
<https://en.uesp.net/wiki/Skyrim_Mod:File_Format_Conventions> (container fact, ADR 0005
`uesp-skyrim`); the bytes are kept uninterpreted.

Enum and flag names are PascalCase and `Other` is reserved.

### Fields

Fields take `name`, `type`, the type keys, and:

- `since` / `until`: present only for those form versions.
- `if_remaining = true`: present only when bytes remain in the payload when the field is
  reached (a trailing field that some occurrences omit). A partial value is a decode error,
  so the whole subrecord stays raw. It must be the last field; nest a struct for a trailing
  group of fields. Use it only when the field split is a sourced fact, not just because a
  subrecord occurs with two sizes (WEAP `DAMA` and KYWD `DNAM`/`ENAM` stay opaque for
  that reason).
- `note`.

A field that reads to the end of the payload (`zstring`, `lstring`, rest arrays, opaque
without `size`, a struct ending in one of those) or is `if_remaining` must be the last
field. Length-prefixed strings and `count_from` arrays delimit themselves and may appear
anywhere.

### Arrays

- `count = N`: exactly `N` elements (generated as `[T; N]`, so the size cannot drift).
- `count = "rest"`: elements until the payload ends; must be last.
- `count_from = "Count"`: as many elements as the value of the named field, which must be
  an earlier, ungated, non-`if_remaining` integer field of the same struct. Only on struct
  fields (a whole subrecord has no siblings). The count field is written as stored; keep it
  in sync when editing.

Elements must not read to the end and must not be empty: scalars, `formid`, `sig`, `enum`,
`flags`, fixed `opaque`, `wstring`, `bstring`, `ref:` types, or a `struct` of those.
Element keys (`targets`, `base`, `values`, `bits`, `field`) sit on the array itself;
`element = "ref:Point"` uses a shared type.

### Not supported (stays opaque)

Discriminated unions (a layout chosen by the value of another field or subrecord) are not
expressible yet. Such payloads stay `opaque` until the format grows a construct for them.

## Shared definitions (`common/`)

A shared definition is written once and referenced from records:

```toml
# common/BfcbBlock.toml
name = "BfcbBlock"
kind = "group"
source = "corpus: ..."

[[member]]
sig = "BFCB"
name = "Bfcb"
type = "opaque"
# ...
```

```toml
# common/Point.toml
name = "Point"
kind = "type"
source = "corpus: ..."
type = "struct"
field = [{ name = "X", type = "i16" }, { name = "Y", type = "i16" }]
```

Shared definitions may reference each other; cycles are rejected. References are expanded
at load time, so the model never holds an unresolved one (`GroupDef::shared` and
`FieldType::Ref` record where an expansion came from).

## Generated code (summary)

`kitcraft_schema::generate` emits, for the whole set: the prelude, each shared definition
once, a struct per header, a `<Sig>Record` per record, and `enum Record` with
`Record::decode(signature, subs, ctx)`, `encode()`, `signature()` and
`RECORDS: &[(Sig, &[MemberDesc])]`.

- Decoding takes `RawSub { signature, data, oversized }` and `Ctx { form_version,
  localized }`. The container sets `oversized` when an `XXXX` subrecord carried the size,
  and `localized` from the TES4 header flag `0x80`.
- Typed subrecord values are `Sub<T> { value, oversized }` (dereferences to `T`), so an
  `XXXX` prefix comes back exactly where it was read, whatever the payload size. A new
  value has `oversized = false`; the container must still use `XXXX` for payloads over
  `u16::MAX` bytes.
- `lstring` decodes to `LString::Id(u32)` or `LString::Text(ZString)` by `ctx.localized`;
  a payload that does not fit the mode stays raw in `extra`.
- `extra` holds subrecords that matched no member or failed to decode, each with its index
  in the body as read; `encode` re-inserts each at that index.

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
| `RECORD.toml` | header (no signature) | uesp-skyrim Mod File Format page + corpus | Fully typed; only the Compressed flag is named |
| `GMST.toml` `GLOB.toml` `KYWD.toml` | record | corpus scan | Inventory only: order and cardinality; every payload opaque |
| `WEAP.toml` | record    | corpus scan                              | Inventory only; typed layout waits for lane B round 3 (`xedit:`) |
| `common/BfcbBlock.toml` | group | corpus scan (GLOB, KYWD, WEAP)  | BFCB..BFCE block; payloads opaque         |
| `common/ObteBlock.toml` | group | corpus scan (WEAP)              | OBTE..STOP block; payloads opaque         |

Inventory definitions name members after their signature (`Edid`, `Wfir`, `Flld2`). These
are placeholders: giving a member a real name later is a breaking change for queries.

## Example

```toml
signature = "TES4"
name = "PluginHeader"
source = "uesp-skyrim: https://en.uesp.net/wiki/Skyrim_Mod:Mod_File_Format/TES4"
evidence = ["corpus: TES4 of the 14 official ESMs"]
confidence = "corpus"

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

`crates/schema/tests/fixtures/` holds synthetic files that together use every feature
(`ZZZZ.toml`, `YYYY.toml`, `EMPT.toml`, `common/`).
