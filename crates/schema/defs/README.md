# Record definition format

One TOML file per record type, named `<SIGNATURE>.toml` (`WEAP.toml`, `NPC_.toml`), plus
fixed binary headers (`GRUP.toml`, `RECORD.toml`) and shared definitions in `common/`
(`common/BfcbBlock.toml`). The loader (`kitkraft_schema::load_dir` / `load_sources`)
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
  `confidence = "corpus"` or better; `reference` is rejected, and a container fact no
  official file shows is `container-reference` (see Confidence).
- `xedit: <file>@<sha>#L<start>-L<end>`: a Starfield record definition read from upstream
  xEdit's `Core/wbDefinitionsSF1.pas`, or a shared helper the Starfield definitions invoke
  from `Core/wbDefinitionsCommon.pas` (ADR 0004, owner approval 2026-10-14). `<sha>` must
  be the revision ADR 0004 pins, `9fb016884bec138ea6c7b872cec831537d464c3e` (full 40
  lower-case hex), for both files (`kitkraft_schema::XEDIT_REVISION`, `XEDIT_FILES`); the
  line range is 1-based and inclusive. Cite the record's own range in `source` and helper
  ranges in `evidence`. Facts only: no code, comments or labels copied wholesale; see
  `docs/xedit-reference.md` for the construct-to-TOML map.
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

`confidence` says how far a fact has been checked (ADR 0004 owner approval, clarification
3; ADR 0005 Amendment 2):

| Value       | Meaning                                                      | Needs a citation of        |
| ----------- | ------------------------------------------------------------ | -------------------------- |
| `reference` | Documented in an allowed source (xEdit), not yet observed    | `xedit:`                   |
| `container-reference` | A container fact from UESP's Skyrim pages that no official file shows (ADR 0005 Amendment 5); only in `TES4.toml`, `GRUP.toml` and `RECORD.toml` | `uesp-skyrim:` |
| `corpus`    | Observed in a local fixture: sizes, counts and order agree with every occurrence in the 14 official ESMs | `corpus:` |
| `ck-diff`   | Checked against Creation Kit output                          | `ck-diff:`                 |
| `verified`  | Independently validated: both of the above                   | `corpus:` and `ck-diff:`   |

Every definition file states `confidence` (M2.B1); the loader rejects a missing or unknown
value, and a level its citations do not back. `reference` is rejected next to a
`uesp-skyrim:` citation: Skyrim facts describe the shared container only and need the
corpus. Upstream coverage is not understanding: a member xEdit describes stays `reference`
until our own evidence raises it.

`container-reference` is the one exception (ADR 0005 Amendment 5): a container fact cited
from UESP's Skyrim pages with no occurrence in the 14 official files to check it against
(today `TES4.Master.FileSize` and `TES4.TagifyCount`). The loader accepts it only with a
`uesp-skyrim:` citation (own or inherited) and only in the container definitions (`TES4`,
the group header, the record header), never in a record body or a shared definition. It
ranks with `reference` (`Confidence::below_corpus`): `inspect` and `query` name it in
`limitations`, `build` writes it only with `allow_reference`, and `cargo xtask parity`'s
occurrence check skips it as a corpus claim but fails as soon as the corpus shows it (the
member must then be validated and raised to `corpus`).

### Per-member provenance

A subrecord, group, `group_ref` or `any_until` member, and a struct field, may set its own
`source`, `evidence` and `confidence`. The model stores the *effective* provenance on every
node (`kitkraft_schema::Provenance`):

- Nothing set: the parent's provenance (the file's, or the enclosing member's or field's).
- `evidence` and/or `confidence` without `source`: the parent's source, the parent's
  evidence followed by the member's own, and the member's level (or the parent's).
- `source` set: the member's own citations only (`source` plus its own `evidence`), and
  `confidence` is then required on the member.

Each level is checked against the effective citations, at every node that sets a key.
Members of a shared definition (`defs/common/`) keep that file's provenance wherever it is
expanded; the `group_ref` member itself (its position and cardinality in the record) has
the record's.

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
| `confidence`| yes             | `reference`, `container-reference`, `corpus`, `ck-diff` or `verified` (see Confidence) |
| `note`      | no              | Free text (our own words)                                       |
| `flags`     | no (records)    | Record-header flag bits for this type: `[["Master", 0x1], ...]` |
| `unordered` | no (records)    | `true`: members may occur in any order (see Unordered records)  |
| `member`    | records, groups | Ordered list of members                                         |
| `size`      | headers         | Total header size in bytes; must equal the sum of `field` sizes |
| `field`     | headers         | Ordered list of fields                                          |
| `type` ...  | types           | A shared type: `type` plus its type keys, as on a field         |
| `size_by_version` | no (types) | A shared type whose layout changes between form versions: its size per version range, checked as on a member (see Sizes) |

## Members

`[[member]]` entries are listed **in on-disk order**. Each member is exactly one of:

**Subrecord** (`sig = "EDID"`): `name` (PascalCase, unique in its list after snake_case
conversion; `Extra` is reserved), `type` (see Types), `card`, optional `size`,
`size_by_version`, `note`, plus the type keys (`targets`, `count`, `element`, `base`,
`values`, `bits`, `field`). Every member kind and every field also takes the provenance
keys `source`, `evidence` and `confidence` (see Per-member provenance).

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
consecutive). In an ordered record the same signature may appear at two positions if the
members have different names (WEAP has two `FLLD` slots); an unordered record does not
allow that at the top level (see Unordered records).

### Unordered records

`unordered = true` on a record definition (CELL, REFR, ACHR; xEdit marks them so) says
its members may occur in any order. The decoder (`match_unordered`) matches a subrecord
from the current member on, as for an ordered record, and when no member from there
accepts it, from the first member again; a group still matches one consecutive run in its
own order, and a second occurrence of a member that is not `repeated` stays raw in
`extra`. Every member keeps its path. The generated record stores the member of each
typed value in on-disk order (`disk_order`) and `encode` writes the values back in that
order, so an unedited record re-encodes byte for byte; values added since decoding follow
the recorded ones, member by member, and a record built from scratch is written in
definition order. List the members in the order most records use. Not allowed: a
top-level `any_until` member (it would accept every subrecord out of place), a signature
that can start two top-level members (two subrecord members of that signature, or a
subrecord member and a group whose leading members, up to its first required one, include
it: which member took it would depend on where the matcher stood; signatures inside a
group past its first required member are fine), and the member name `DiskOrder`. Generated helpers: `UNORDERED` (the signatures) and
`match_body(signature, subs, members)`, which picks the right matcher for tools that match
bodies themselves.

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
| `union`   | variant | `decider`, `variants`, `secondary` | A whole subrecord (EditorID prefix) or a struct field (sibling decider); see Unions |

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
  subrecord occurs with two sizes (WEAP `DAMA` stays opaque for
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

### Unions (`type = "union"`)

Two deciders: the record's EditorID prefix, for a whole subrecord, and the value of an
earlier sibling field, for a struct field.

#### By EditorID prefix (whole subrecords)

A whole subrecord whose layout the record's EditorID selects (M2.B9, GMST `DATA`):

```toml
[[member]]
sig = "DATA"
name = "Value"
type = "union"
decider = "edid-prefix"
variants = [
  { when = "f", name = "Float", type = "f32" },
  { when = "s", name = "String", type = "lstring" },
]
```

- `decider = "edid-prefix"`: the first byte of the record's first `EDID` payload, compared
  exactly with each variant's `when` (one ASCII letter or digit, unique). The record must
  have an `EDID` member.
- Variants take `name`, `type` (anything but `struct`, `array` and `union`; `formid` may
  have `targets`, `enum`/`flags` their `base` and `values`/`bits`, `opaque` its `size`)
  and `note`. `Other` is reserved as a variant name.
- Only on subrecord members (not on fields, array elements or shared types). A declared
  `size` must match every variant, so it is not allowed when a variant is variable
  (`lstring`).
- Codegen emits an enum with one variant per layout (`GmstValue::Float(f32)`) and a
  `prefix()` method. A payload whose record has no `EDID`, or whose prefix no variant
  names, or that does not fit the chosen layout, stays raw in `extra`. Editing the EDID's
  first letter does not change an already decoded value's variant.

#### By a sibling field (struct fields)

A struct field whose layout the value of an earlier field of the same struct selects
(lane B round 4, the object mod property in `common/ObjectModProperty.toml`):

```toml
[[field]]
name = "Value1"
type = "union"
decider = "field:ValueType"
secondary = "field:Property"          # optional
variants = [
  { when = 0, name = "Int", type = "i32" },
  { when = [4, 6], name = "FormId", type = "formid" },
  { when = 5, name = "Enum", type = "u32" },
  { when = 5, secondary_when = "WSLV", name = "SoundLevel", type = "u32" },
]
```

- `decider = "field:<Name>"` names the deciding sibling: an **earlier** field of the same
  struct, ungated (no `since`/`until`), not `if_remaining`, and an integer or an `enum` of
  at most 32 bits (a `ref:` to one counts). Each variant's `when` is one value of it or a
  non-empty list; every value must fit the sibling's type.
- `secondary = "field:<Name>"` (optional) names a second earlier sibling with the same
  rules. A variant with `secondary_when` (an integer, or four printable characters read
  as a little-endian `u32`, the way four-character property codes are stored) applies only
  when both values match; a variant without one applies to its `when` values whenever no
  variant with a matching `secondary_when` does. Each (`when` value, `secondary_when`)
  pair is unique.
- Every variant has the **same fixed size** (the loader rejects a variable or different
  size). A decider value no variant names keeps the field's bytes as read, so decoding
  never fails because of the decider and re-encoding is byte-identical.
- Variants take the same keys as EditorID-prefix variants; `Other` is reserved.
- A union field takes no type keys of its own (`size`, `count`, ...), is never gated
  (`since`, `until`, `if_remaining`), and is allowed only as a struct field: not on a
  whole subrecord (it has no siblings), an array element or a shared type. The struct
  itself may be an array element or a shared type (`ObjectModProperty` is both).
- Codegen emits an enum with one variant per layout plus `Other([u8; N])` for the raw
  bytes, read by `read_keyed(key, secondary, ..)` with the sibling values (enums by their
  stored value). `Field::read` without the decider values yields `Other`. Editing the
  decider does not change an already decoded value's variant; write the matching variant.
- The member path of a variant is the field path plus the variant name
  (`DATA.Properties[3].Value1.Float`): the `doc` reader decodes the field as a one-member
  struct named after the variant that applies, and the engine's field paths accept a
  variant step only when the entry's own decider values select that variant.
- FormID and lstring variants are visited (`form_ids()`, `lstrings()`) under the field's
  path (`ObjectModProperty.Value1`).

### Not supported (stays opaque)

Unions decided by another subrecord, by flags, or by a value outside the same struct
(a component block's BFCB type, CTDA's parameters, COED's owner) are not expressible yet.
Such payloads stay `opaque`.

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

`kitkraft_schema::generate` emits, for the whole set: the prelude, each shared definition
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
  in the body as read; `encode` re-inserts each at that index. `Record::extra()` returns
  it for any record type.
- Visitors (M2.B3): every record has `form_ids()` / `form_ids_mut()` over every
  `formid`-typed value and `lstrings()` / `lstrings_mut()` over every `lstring` value, in
  definition order, each paired with the path of the member or field holding it
  (`WEAP.KWDA`, `Link.Form` inside a shared type); `Record` forwards them. FormIDs are the
  raw stored values; resolution against masters belongs to the `doc` layer. Values kept
  raw (`extra`, `any_until` bodies, opaque payloads) are not visited.

## Rules of thumb

- Unknown or ambiguous layouts are `opaque`, never guessed. A partial definition is fine.
- Field and member names are the public query vocabulary (`WFIR.ShotsPerSecond`); renaming
  one is a breaking change.
- Order comes from the source page or the corpus. Where two members never occur together,
  their relative order is unknown; say so in a `note`.
- A signature upstream lists both inside a shared block and right after it (the XFLG
  convention, lane B round 4): the block's member is the typed one and takes an occurrence
  that directly follows the block's members (`Model.Flags`, the model flags: every official
  OMOD XFLG and ARMO's world-model XFLG); the standalone member stays as upstream lists it,
  `opaque` and `reference` until the corpus shows an occurrence the block cannot take, and is
  named after its signature (`Xflg`). Coverage counts each occurrence at the member that
  holds it. REFR's XFLG is a different member altogether (a `u32` of the placed reference).
- A marker upstream lists twice in a row is defined twice only when the corpus has both;
  a block that already ends with the marker takes the first (GBFM: `ObteBlock` ends with
  STOP and `Marker2` takes the second; every official GBFM has both).

## Current set

| File        | Kind      | Source                                   | Layouts                                   |
| ----------- | --------- | ---------------------------------------- | ----------------------------------------- |
| `TES4.toml` | record    | uesp-skyrim TES4 page + corpus           | HEDR/CNAM/SNAM/MAST/DATA/ONAM/INTV/INCC typed (DATA and INTV `container-reference`: no Starfield occurrence); TNAM/BNAM/CHGL opaque |
| `GRUP.toml` | header    | uesp-skyrim Mod File Format page + corpus | Fully typed (label kept as 4 raw bytes)  |
| `RECORD.toml` | header (no signature) | uesp-skyrim Mod File Format page + corpus | Fully typed; only the Compressed flag is named |
| `GMST.toml` | record | corpus scan + xEdit at the pin + UESP Skyrim/Starfield GMST pages | EDID; DATA as a union by EditorID prefix (b/f/i/s/u); XALG as `u64` (M3.B6) |
| `GLOB.toml` | record | corpus scan + UESP Skyrim GLOB page | EDID; FLTV as `f32`; BFCB blocks |
| `KYWD.toml` | record | xEdit at the pin + corpus (M3.B6) | Every member typed except VMAD (script unions) and FNAM (no upstream layout); TNAM a plain `u32` as in LCRT |
| `WEAP.toml` `AMMO.toml` `PROJ.toml` `AMDL.toml` `OMOD.toml` `COBJ.toml` `ZOOM.toml` `LVLI.toml` `FLST.toml` `INNR.toml` | record | xEdit at the pin, member by member + corpus (lane B round 3) | Every member xEdit lists, typed; opaque only where xEdit decides the layout by a union the format cannot express (VMAD, BFCB component data, CTDA, LVLI COED owner data; OMOD property values are field unions since lane B round 4) or does not describe it (WEAP MO4C, OMOD XFLG) |
| 39 more `<SIG>.toml` (ACTI ... WATR) | record | corpus lstring table + xEdit lines (M3.B8) | lstring-only: EDID plus a repeated `LocalizedStrings` group of optional lstring members; every other subrecord stays raw |
| `common/BfcbBlock.toml` | group | xEdit base form components + corpus | BFCB component type and BFCE marker typed; the component data stays raw (a union by component type) |
| `common/ObteBlock.toml` | group | xEdit object template + corpus | OBTE count, combinations (OBTF, FULL, OBTS template item), STOP |
| `WRLD.toml` `CELL.toml` `REFR.toml` `ACHR.toml` `NAVM.toml` `LCTN.toml` `LCRT.toml` | record | xedit (pinned) + corpus (`tests/world_family.rs`) | World family: typed from xEdit, checked on every official record; VMAD, NVNM, NAVM MNAM, XSL1, XTV2, XNSE and unknowns opaque; CELL/REFR/ACHR are `unordered = true` (xEdit marks them so; member list in the most common order, M4.BX1). `docs/schema/world-family.md` |
| `common/PositionRotation.toml` `OwnerData` `EnableParent` `ColorRgb` `ColorRgba` `PropertyValue` `RagdollBone` | type | xedit (pinned) + corpus | Shared payload layouts of the world family (positions and rotations are `Vec3`, OwnerData NoCrime is `Bool8`, M4.BX1) |
| `common/LinkedReferences.toml` `GroupedPackIn.toml` | group | xedit (pinned) + corpus | XLKR*/XLKT and the XWPK..XWPK pack-in block |
| `common/Keywords.toml`  | group | corpus scan (47 record types)   | KSIZ count + KWDA FormID list (targets KYWD) |
| `common/Model.toml`     | group | xEdit generic model block       | MODL, MOLM, FLLD, MODC, XFLG |
| `common/Destructible.toml` | group | xEdit destructible block     | DEST, DAMC, DSDL, stages (DSTD, DSTA, DMDL, FLLD, DMDS, DSTF) |
| `common/SoundReference.toml` `common/ObjectModProperty.toml` `common/Component.toml` `common/Vec3.toml` `common/Bool8.toml` | type | xEdit helpers (+ corpus) | Sound reference (40 bytes), object mod property (24 bytes), crafting component (12 bytes), three floats, one-byte boolean |
| `PCMT` `PCBN` `PCCN` `LVLP` `PKIN` `GBFM` `BIOM` `PNDT` `STDT` `SUNP` `SFBK` `SFPT` `RSGD` `RSPJ` `TRAV` (`.toml`) | record | xedit at the pin + corpus (every official record) | Planet content family (`docs/schema/planet-family.md`): typed except component blocks, VMAD, unions and unnamed slots |
| `common/Condition.toml` | group | xedit L4750-L4840 + corpus (planet family and round 3, merged in M4.BX1) | One CTDA (unions kept as raw 4-byte fields) with optional CIS1/CIS2 (`corpus` since the QUEST merge); used by COBJ, FLST, LVLI, LVLP, PCBN, PCMT, RSPJ and the quest family |
| `common/ObjectBounds.toml`, `ObjectPaletteDefaults`, `Transforms`, `ColorBgra`, `BiomeNoiseLayer`, `ResourceBiomeTier`, `ResourceCellTier` | type | xedit + corpus | Shared payload structs of the planet family |
| `FACT` `QUST` `PACK` `DIAL` `INFO` `SCEN` `SMBN` `SMQN` `SMEN` `CHAL` (`.toml`) | record | xedit at the pin + corpus (every official record) | Quest family (`docs/schema/quest-family.md`): typed except VMAD after its header, CTDA unions, component blocks, SCEN action type-specific subrecords, PACK input unions and unnamed slots |
| `common/ScriptAttachment.toml`, `PackageLocation` | type | xedit + corpus | VMAD header (scripts and fragments raw), the package/vendor location (the quest family's Wwise sound reference folded into `common/SoundReference.toml`, M4.BX1) |
| `common/QuestReferenceAlias.toml` | group | xedit L7996-L8163 + corpus | One quest reference alias (ALST..ALED), used twice by QUST |
| `NPC_` `LVLN` `OTFT` `CLAS` `AFFE` `RACE` `PERK` `SPEL` `MGEF` (`.toml`) | record | xedit at the pin + corpus (every official record) | Actor family (`docs/schema/actor-family.md`): typed except VMAD, component blocks, unions and unnamed slots |
| `common/AttackData.toml`, `common/Effect.toml` | group | xedit + corpus | An attack (ATKD..ATKT); a magic item's effect entry (EFID..MUID, conditions without CITC) |
| `STAT` `MISC` `CONT` `ACTI` `FURN` `MSTT` `TERM` `TMLM` `ARMO` `IDLE` `ANIO` `CPTH` (`.toml`) | record | xedit at the pin + corpus (every official record) | Items and workshop family (`docs/schema/items-workshop-family.md`): typed except VMAD, component blocks, NVNM, unions and raw padding |
| `common/ContainerItems.toml` | group | xedit + corpus | Container entries COCT, CNTO (COED middle slot raw); used by CONT, NPC_ and the quest alias inventory |
| `common/RadioReceiver.toml`, `FurnitureMarkerFlags`, `FurnitureMarkerParameter`, `FurnitureEntryPoint` | type | xedit + corpus (RadioReceiver: xedit only) | Shared payload types of the items and workshop family (its model, destructible, object template, sound reference, object property and colour blocks were folded into `Model`, `Destructible`, `ObteBlock`, `SoundReference`, `PropertyValue` and `ColorRgb`, M4.BX1) |

Universal subrecords (M2.B2): `EDID` is `EditorId` (zstring), `FULL` is `Name` and `DESC`
is `Description` (lstring), and `KSIZ`/`KWDA` are the shared group `Keywords`
(`Count`, `Forms`). Each typed member carries its own `evidence` (the UESP string
conventions page and the universal corpus scan of 2026-10-14) and `confidence = "corpus"`.

Round 3 definitions (M3) cite the pinned xEdit file per member (`source`), with the helper
ranges in `evidence`; a member is `corpus` when the round 3 typed check of every official
record of the type confirms it (`corpus_full_roundtrip` and a per-type scan: nothing left in
`extra`, zero re-encode differences), and `reference` when no official record has it.
Counts stored in their own field or subrecord (`IncludeCount`, `LLCT`, `CITC`, `VNAM`,
`OBTE`) are kept as read: an edit that changes the list must update them. Length prefixes
xEdit folds into its string and array types (a u32 before OMOD's name, PROJ's particle
defines and OMOD's attach parent slots; a u8 before a template item's keywords) are written
as explicit count fields with a `count_from` array.

Opaque inventory members are named after their signature (`Fnam`, `Bnam`). These are
placeholders: giving a member a real name later is a breaking change for queries.

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
