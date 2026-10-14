# World records: WRLD, CELL, REFR, ACHR, NAVM, LCTN, LCRT

For mod authors and for the lanes that build on the schema: what each world record carries,
how placed references hang off cells and worldspaces, what stays raw and why, and what the
cell and worldspace groups look like in the official files.

Definitions: `crates/schema/defs/{WRLD,CELL,REFR,ACHR,NAVM,LCTN,LCRT}.toml` and the shared
pieces in `crates/schema/defs/common/` (`PositionRotation`, `OwnerData`, `EnableParent`,
`LinkedReferences`, `GroupedPackIn`, `ColorRgb`, `ColorRgba`, `PropertyValue`,
`RagdollBone`). Layouts come from upstream xEdit at the pinned revision (ADR 0004; every
member cites its lines) and are checked against every record of the 14 official ESMs by
`crates/esm/tests/world_family.rs` (`world_family_full`, 2026-10-14). Counts on this page
come from that run unless they say otherwise.

## Corpus check

Every record of the seven types was read through the record index, decoded through its
generated type and encoded again.

| Type | Records | Compressed | Subrecords | Placed in a member | Raw in a BFCB block | Out of member order (kept raw) | Decode failures | Re-encoded identical |
| ---- | ------: | ---------: | ---------: | -----------------: | ------------------: | -----------------------------: | --------------: | -------------------- |
| WRLD | 583 | 0 | 26,792 | 17,754 | 9,038 | 0 | 0 | all 583 |
| CELL | 40,251 | 32,230 | 268,822 | 267,646 | 392 | 784 (in 502 records) | 0 | all 40,251 |
| REFR | 6,719,725 | 0 | 41,894,388 | 39,772,329 | 307,684 | 1,814,375 (in 889,512 records) | 0 | all 6,719,725 |
| ACHR | 14,731 | 0 | 87,081 | 74,245 | 1,548 | 11,288 (in 6,434 records) | 0 | all 14,731 |
| NAVM | 92,893 | 92,764 | 185,537 | 185,535 | 2 | 0 | 0 | all 92,893 |
| LCTN | 7,316 | 0 | 78,402 | 78,402 | 0 | 0 | 0 | all 7,316 |
| LCRT | 4,779 | 0 | 19,134 | 19,128 | 6 | 0 | 0 | all 4,779 |

"Placed in a member" means the subrecord landed in a typed or opaque member and, for a
typed one, decoded with the declared layout. Nothing placed failed to decode, and every
record writes back byte for byte.

**Out of member order.** xEdit marks CELL, REFR and ACHR members as unordered, and the
official files bear that out: the same record type stores, say, `XLYR` before `XLMS` in one
record and after it in the next. The schema matches members in one fixed order, so the
definitions use the linear order that agrees with the most official records (computed
from every corpus record), and a subrecord that comes earlier than its member allows is
kept raw in the record's `extra` at its position. Its bytes are never lost and the record
still writes back identically, but its value is not in the typed field: read `extra` too
(`Record::extra()`), or treat a missing typed value on these three types as "not read",
not as "absent". The worst cases: REFR `XLYR` (364,914 of 3,352,032 out of order), `XLMS`
(298,641 of 1,464,237), the grouped pack-in block (about 114,000 of 1,736,698); ACHR `XLCN`
(3,212 of 10,948) and `XLYR` (2,731 of 6,955). Each member's `note` gives its own counts.
An `unordered` matching mode would place all of them (requested from lane B in
`plan/STATUS.md`).

## What each record carries

FormIDs are the raw stored values throughout (0 means none); `targets` name the record
types xEdit allows. Resolving them against masters is the `doc` layer's job.

### WRLD: worldspace

Name, location (`XLCN`), biome (`BNAM`), parent worldspace and what it inherits from it
(`WNAM` + `PNAM` flags), climate, water and LOD water, land and water heights (`DNAM`),
map data and map offset (`MNAM`, `ONAM`), flags (`DATA`, `FNAM`), bounds in cells (`NAM0`
min, `NAM9` max), music and ambience, gravity scale (`GNAM`), cells with their own water
height (`XCLW` + `WHGT`), large references per cell (`RNAM`, with its own count), and two
per-cell u32 tables (`OFST` offsets, `CLSZ` sizes; row width = `NAM9.X - NAM0.X + 1`, not
stored, so they are typed as flat lists). Every official WRLD follows xEdit's member order.
Many of the worldspace's settings in the official files live inside a BFCB component block
(`BNAM`, `PNAM`, `NAM1`..`NAM9`, `ZNAM`, conditions `CITC`/`CTDA`), which stays raw.

### CELL: cell

Flags (`DATA`: interior, has water, public area, ...), grid position for exterior cells
(`XCLC`: X, Y, land flags), lighting for interior cells (`XCLL`, 108 bytes: colours, fog,
directional light, gravity, interior type), lighting template, water height, location,
image space, acoustic space, music, sky region, owner (`XOWN`, `XRNK`), ship blueprint snap
links (`XBPS`), linked references, and the `XCLA`/`XCLD` pairs xEdit leaves unnamed. 32,230
of 40,251 CELLs are zlib-compressed.

### REFR and ACHR: placed object, placed NPC

`NAME` is the base object (REFR: any referenceable type; ACHR: an NPC_) and `DATA` its
placement (`PositionRotation`: position X, Y, Z, then rotation X, Y, Z, floats). Around them:
scale (`XSCL`), layer (`XLYR`), reference group (`XRFG`), source pack-in (`XPCS`) and the
grouped pack-in block (`XWPK`..`XWPK`), layered material swaps (`XLMS`), enable parent
(`XESP`), linked references (`XLKR`/`XLKT`), owner, location refs (`XLRT`, `XLCN`), and for
REFR: light data (`XLIG`, `XALD`, `XLRD`, `XLBD`, `XLCD`, `XLFD`, `XLGD`, ...), projected
decals (`XPDD`/`XPDO`), teleport destination (`XTEL`: door, placement, flags), map markers
(`XMRK` group: flags, name, type, category), lock (`XLOC`), primitives (`XPRM`), patrol data,
ship arrival, navmesh door link, properties (`XPPS`). REFR record-header flags change meaning
with the base object's type (xEdit decides them by `NAME`'s record type), so REFR lists no
flag names; ACHR does.

### NAVM: navmesh

Almost all compressed (92,764 of 92,893; 129 are deleted stubs with no subrecords). The
geometry (`NVNM`, up to 750,400 bytes, `XXXX`-sized when large) and the pre-cut map
(`MNAM`, empty in every official record) stay raw (see below).

### LCTN: location

Persistent, unique and special references, initially disabled and enable-parent lists, and
the worldspace cells the location covers, each in three flavours: `AC..` (added by a plugin
to a master's location), `LC..` (the location's own) and `RC..` (removed). Then name,
keywords, properties (`PRPS`, at two positions), ownership and encounter levels (`DATA`),
parent location, crime faction, world marker and radius, star and planet IDs.

### LCRT: location reference type

A colour, the keyword type (`TNAM`, kept as its u32 value), the unknown `FNAM`, and members
no official LCRT has (notes, name, attraction rule, Flash linkage name).

## What stays opaque, and why

| Member | Why it is raw |
| ------ | ------------- |
| BFCB..BFCE blocks (every type) | Component data; the block body is kept as raw subrecords (shared `BfcbBlock`) |
| `VMAD` (all) | Script data; xEdit builds it from unions chosen by deciders the schema cannot express |
| NAVM `NVNM` | Nested counted arrays and version-gated parts |
| NAVM `MNAM`, REFR `XSL1` | Lists whose entries carry their own counted lists; an array element cannot hold a counted array |
| CELL and REFR `XTV2` | 56-byte traversal entries whose FormID is present only when a flag says so |
| REFR `XNSE` | Reflection data from a helper not read yet |
| REFR `PDTO.Data` | A DIAL FormID or a 4-character subtype depending on `PDTO.Kind` (a union); the kind is typed |
| WRLD `HNAM`, LCRT `FNAM`, ACHR `XGDS`, REFR `XCZA`, `XLTW`, LCTN `NAM0` | xEdit gives no meaning (unknown or unused) |
| WRLD second `WLEV` | Cell data xEdit keeps as bytes; not in any official WRLD |
| Unnamed struct fields (`Bytes12`, `Float20`, ...) | Typed by position as xEdit lays them out (floats as `f32`, unknown bytes opaque); named by byte offset |

Enumerations xEdit lists but that are long or unnamed are kept as integers: LCRT `TNAM`
(keyword type), REFR map marker `TNAM` (70 marker types), REFR `BOLV`, CELL `XCLC.LandFlags`.

## How placed references hang off cells and worldspaces

Placed references are not linked to their cell by any field: the group they sit in says
where they are. The group label of every child group is the FormID (or block number, or
grid) of what it belongs to.

```
GRUP type 0 "WRLD"                       top group
  WRLD <world>                           the record
  GRUP type 1, label = world FormID      world children, right after its WRLD
    CELL <persistent cell>               flags 0x400; 549 of 581 worlds have one
    GRUP type 6, label = that cell
      GRUP type 8 / type 9               its references
    GRUP type 4, label = block (Y, X)    exterior block
      GRUP type 5, label = subblock (Y, X)
        CELL <exterior cell>             XCLC = grid X, Y
        GRUP type 6, label = cell FormID cell children, right after its CELL
          GRUP type 8, label = cell FormID   persistent: REFR, ACHR, PGRE, PHZD
          GRUP type 9, label = cell FormID   temporary: REFR, ACHR, NAVM, PGRE, PHZD

GRUP type 0 "CELL"                       top group, interior cells
  GRUP type 2, label = block number
    GRUP type 3, label = subblock number
      CELL <interior cell>
      GRUP type 6, label = cell FormID
        GRUP type 8 / type 9
```

### Group counts in the corpus (all 14 files, from the index)

| Type | Meaning | Groups | Holds (direct children, all files) |
| ---- | ------- | -----: | ---------------------------------- |
| 1 | World children | 581 | 549 CELL (persistent), 545 type 6, 2,299 type 4 |
| 2 | Interior cell block | 85 | 695 type 3 |
| 3 | Interior cell subblock | 695 | 17,020 CELL, 16,979 type 6 |
| 4 | Exterior cell block | 2,299 | 2,461 type 5 |
| 5 | Exterior cell subblock | 2,461 | 22,682 CELL, 4,734 type 6 |
| 6 | Cell children | 22,258 | 3,757 type 8, 21,663 type 9 |
| 7 | Topic children | 81,287 | 162,028 INFO (not a world group) |
| 8 | Cell persistent children | 3,757 | 115,119 REFR, 780 ACHR, 53 PGRE, 33 PHZD |
| 9 | Cell temporary children | 21,663 | 6,604,606 REFR, 13,951 ACHR, 92,893 NAVM, 2,546 PGRE, 417 PHZD |
| 10 | Starfield-only (quest-related) | 1,808 | 87,031 DIAL, 135 DLBR, 9,673 SCEN, 81,287 type 7 |

Two of the 583 WRLD records have no world children group. A file with interior cells has up
to 10 interior blocks (most have all 10).

### Rules the official files follow (checked by `world_family_full`)

| Rule | Agreement |
| ---- | --------- |
| A type 1 group follows its WRLD (the last record before it in the top group), label = the WRLD's FormID | 581 / 581 (asserted) |
| A type 6 group follows its CELL (the last record before it in the same parent group), label = the CELL's FormID | 22,258 / 22,258 (asserted) |
| Type 8 and 9 groups sit in the type 6 group with the same label | 25,420 / 25,420 (asserted) |
| Exterior subblock label = (Y div 8, X div 8) of the cell's `XCLC`, stored as two i16, Y first | 22,682 / 22,682 (asserted) |
| Exterior block label = (Y div 32, X div 32), same encoding | 22,682 / 22,682 (asserted) |
| Interior block label = (FormID & 0xFFFFFF) mod 10, subblock = (FormID & 0xFFFFFF) div 10 mod 10 | 17,019 / 17,020 (reported) |
| Inside a type 6 group, type 8 comes before type 9 | not a rule: [9] only 18,484, [9 8] 2,364, [8 9] 815, [8] only 578, empty 17 |

`div` rounds toward negative infinity. The one interior exception is `SFBGS047.esm` CELL
`FE001016` (block 2, subblock 2): its low 12 bits, 0x016 = 22, fit the rule, so for that
plugin the object part is the low 12 bits rather than the low 24; that is one observation,
not a checked rule. The persistent and temporary groups of one cell appear in either order:
a writer should keep the order it read and, for a new cell, has no corpus default to copy
(the most common shape is a temporary group only).

### What lane A needs for child-group placement

To add a REFR or ACHR to a cell in a plugin being built, the builder needs the cell's
group path. `Plugin::upsert` follows a path copied from an existing record
(`PluginIndex::group_headers`) and creates missing groups along it; nothing derives the
path for a new cell or for a reference with no source record. The path is:

1. Interior cell: top `CELL` group, then type 2 (block number) and type 3 (subblock
   number) from the cell FormID's object part (rule above), then the CELL, then its type 6
   group right after it, then type 8 (persistent, record flag 0x400) or type 9.
2. Exterior cell: the world's type 1 group right after the WRLD, type 4 and type 5 labelled
   from the cell's `XCLC` grid (rules above), the CELL, its type 6, then type 8 or 9.
3. A world's persistent cell sits directly in the type 1 group, not in a block.
4. NAVM records go in the cell's type 9 group.

The request is filed in `plan/STATUS.md` (Requests, lane A).
