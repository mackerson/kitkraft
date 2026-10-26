# Armor object mods: which properties they carry (2026-10)

Research, not implementation. Written for the `everyday-outfits` clothing-mod pipeline,
which wants to customise outfits with object mods (`OMOD`) and object templates.

The question: in the official Starfield ESMs, which `ObjectModProperty` values do armor
mods carry — stats only, or also a model or material swap?

Source category: `corpus:` only (the 14 official ESMs, read with the survey test below).
The property codes are named from the editor's display tables (read for that purpose
only); the in-game meaning of a code is not observed and is not claimed here.

## How the test answers it

`armor_omod_properties_full` in `crates/esm/tests/armor_omods.rs` (ignored; it streams
all 14 official ESMs, so it runs in release):

```
cargo test -p kitkraft-esm --release --test armor_omods -- --ignored --nocapture
```

It decodes every `ARMO` and `OMOD` it reads (nothing left in `extra`, re-encodes byte
for byte, as `omod_properties.rs` checks) and prints:

- one line per file counting that file's records whose owner FormID resolves to no
  file (all zero in the printed run);
- the armor `OMOD`s by two routes — `template` (every `OMOD` an `ARMO` object template
  names, i.e. a template item's `Includes.Mod`) and `attach-point` (every `OMOD` whose
  `DATA` attach point is a keyword some `ARMO`'s `APPR` lists) — then the union;
- for the union's `OMOD` `DATA` properties, one sorted line per property code (sorted
  by the code's display name) with its count and value types, and, for the
  FormID-valued ones (value types 4 and 6), the record types those FormIDs name,
  resolved through the record index of the file that holds them; a FormID that names
  no record is counted as `unresolved`, and `0x00000000` is counted as `null` — neither
  is ever panicked on;
- the same two kinds of line for the properties of `ARMO` template items themselves;
- a final `armor-omods unresolved: <n>` line — every FormID the survey could not
  resolve to a record of the corpus (record owners, template includes, `APPR` keywords,
  and the armor `OMOD`s' attach points and property values), with a few distinct raw
  examples per property — and an `armor-omods null: <n>` line for the explicit
  `0x00000000` values among the same references, which name nothing by definition.

FormIDs are raw and file-relative (`docs/load-order.md`, "FormIDs"): the high byte
`0xFE` or `0xFD` marks the small or medium class, any other high byte the full class,
and the index counts only the reading file's masters of that class, so the survey
resolves each reference against the master list of the file that holds it, with
master names matched case-insensitively, as `onam.rs` and `crates/doc` do.

Records are keyed by the file their FormID *resolves to*, not the file they were read
from: a DLC that overrides a base-game record stores the base record's FormID, so the
override and the original share one key, and the file processed later in the manifest
(the game's load order) holds the data. This holds for `ARMO` records too: their
`APPR` keywords, template includes, and template-item properties are taken from the
last file that stores the record, so each overridden `ARMO` is counted once.

## Printed counts

```
armor-omods owners BlueprintShips-SFBGS050.esm: 0 records whose owner FormID resolves to no file
armor-omods owners BlueprintShips-Starfield.esm: 0 records whose owner FormID resolves to no file
armor-omods owners Constellation.esm: 0 records whose owner FormID resolves to no file
armor-omods owners OldMars.esm: 0 records whose owner FormID resolves to no file
armor-omods owners SFBGS003.esm: 0 records whose owner FormID resolves to no file
armor-omods owners SFBGS004.esm: 0 records whose owner FormID resolves to no file
armor-omods owners SFBGS006.esm: 0 records whose owner FormID resolves to no file
armor-omods owners SFBGS007.esm: 0 records whose owner FormID resolves to no file
armor-omods owners SFBGS008.esm: 0 records whose owner FormID resolves to no file
armor-omods owners SFBGS00D.esm: 0 records whose owner FormID resolves to no file
armor-omods owners SFBGS047.esm: 0 records whose owner FormID resolves to no file
armor-omods owners SFBGS050.esm: 0 records whose owner FormID resolves to no file
armor-omods owners ShatteredSpace.esm: 0 records whose owner FormID resolves to no file
armor-omods owners Starfield.esm: 0 records whose owner FormID resolves to no file
armor-omods route template: 225 OMODs
armor-omods route attach-point: 250 OMODs
armor-omods read: 3318 OMODs
armor-omods: 311 OMODs, 1145 properties
armor-omod property AACT: 309 (value types 6)
armor-omod property AACT targets: AVIF 307, unresolved 2
armor-omod property AACT unresolved examples: 0x000002E2 in Starfield.esm, 0x000003B7 in SFBGS00D.esm
armor-omod property ACPT: 2 (value types 0, 1)
armor-omod property ADMG: 144 (value types 6)
armor-omod property ADMG targets: DMGT 144
armor-omod property AENC: 44 (value types 4)
armor-omod property AENC targets: ENCH 43, null 1
armor-omod property AKEY: 229 (value types 4)
armor-omod property AKEY targets: KYWD 229
armor-omod property AMLS: 3 (value types 4)
armor-omod property AMLS targets: LMSW 3
armor-omod property ARAT: 72 (value types 0, 1)
armor-omod property AVAL: 78 (value types 0, 1)
armor-omod property GVAL: 40 (value types 1)
armor-omod property GWEI: 1 (value types 1)
armor-omod property WAIM: 1 (value types 4)
armor-omod property WAIM targets: AMDL 1
armor-omod property WDMG: 3 (value types 1)
armor-omod property WDTV: 5 (value types 6)
armor-omod property WDTV targets: DMGT 5
armor-omod property WENC: 36 (value types 4)
armor-omod property WENC targets: ENCH 36
armor-omod property WKEY: 170 (value types 4)
armor-omod property WKEY targets: KYWD 170
armor-omod property WMNR: 1 (value types 1)
armor-omod property WMXR: 1 (value types 1)
armor-omod property WNPR: 1 (value types 1)
armor-omod property WSDM: 1 (value types 1)
armor-omod property WSPD: 3 (value types 1)
armor-omod property WSTS: 1 (value types 1)
armor-template property 0xe: 418 (value types 4)
armor-template property 0xe targets: LMSW 418
armor-omods unresolved: 2
armor-omods null: 61
```

(`0xe` is property 14, the small-integer property code; it displays as hex because it is
not a four-letter code.)

## What the counts say

Armor mods carry **stats and a material swap**, not stats only.

- The `OMOD` `DATA` payloads of the 311 armor `OMOD`s (the union of the two routes)
  carry 1,145 property entries. Most are plain stats or FormID references to stat-like
  records: `AACT` (309, a FormID+float naming an `AVIF` actor-value form — 307 resolve,
  and the 2 that do not are the survey's only unresolved values), `AKEY` (229, naming
  `KYWD` keywords), `WKEY` (170, naming `KYWD`), `ADMG` (144, a FormID+float naming a
  `DMGT` damage form), `AVAL` (78), `ARAT` (72), `AENC` (44, naming an `ENCH`
  enchantment — 43 resolve, 1 is `null`), `GVAL` (40), `WENC` (36, naming an `ENCH`),
  plus a long tail of single or few-entry codes (`WSPD`, `WDMG`, `GWEI`, `WSDM`,
  `WAIM`, `WMNR`, `WNPR`, `WMXR`, `WSTS`, `ACPT`, `WDTV`).
- One of those is a material swap: `AMLS` — the editor's "armor layered material swap"
  code — appears 3 times, and all 3 name an `LMSW` (layered material swap) record.
- The `ARMO` object-template items carry their own property, 14, 418 times, and it too
  is a FormID naming an `LMSW` (all 418 resolve). This is the property the vanilla
  `Clothes_*` colourways use: the material swap is named by the template item, not by
  an `OMOD`.

So the answer to the ticket's question: an armor `OMOD`'s `DATA` is mostly stats
(value/rating/damage/enchantment/keyword entries), but it can also carry a material
swap — `AMLS` naming an `LMSW` — and the vanilla clothing colourways attach their
`LMSW` through the object-template side (property 14 of the `OBTS` template item),
i.e. the template route's mechanism, while the stat `OMOD`s are the template item's
included mods. The attach-point route (250 `OMOD`s) is how the `OMOD`s' own attach
points match an `ARMO`'s `APPR`.

## What is not known

- In-game meaning is not observed. The property codes (`AACT`, `AMLS`, `WENC`, …, and
  the small-integer 14) are display facts from the editor's property-name tables; the
  editor picks which table to show from the containing record's signature (`WEAP`,
  `ARMO`, `NPC_`), so a given numeric code can display under different names in
  different record types. Nothing here is a playtested behaviour.
- Only two FormID values do not resolve to any record of the corpus (the `unresolved`
  line above): the two `AACT` values, `0x000002E2` stored in `Starfield.esm` and
  `0x000003B7` stored in `SFBGS00D.esm`. Both are full-class references whose object
  ID lands in `Starfield.esm` — the first as the file's own class, the second through
  `SFBGS00D`'s first master — and `Starfield.esm` holds no record at either object ID.
  They are counted and printed, not interpreted; what those two `AVIF` references
  should name is not established by the corpus alone. Every other non-null FormID the
  survey resolves — every record owner (the per-file owner lines are all zero), every
  `APPR` keyword, template include, attach point, and property value — resolves to a
  file, and to a record in it, except the two above.
- The `null` line (61) counts explicit `0x00000000` values among the references above:
  the armor `OMOD`s' attach points (an `OMOD` reached only by the template route may
  have none), `APPR` keywords and template includes; the one null property value
  (`AENC`) shows on its `targets` line instead. The null FormID names no form by definition, so
  the survey counts these separately from `unresolved` rather than treating them as
  missing records. (`armor-omods read` is the number of distinct `OMOD`s across the 14
  files, an override counted once.)
- `AMLS` (in an `OMOD`) and property 14 (in a template item) both reach `LMSW`, but
  which one the engine consults for a given swap, and whether both can be present, is
  not established by the corpus alone.
