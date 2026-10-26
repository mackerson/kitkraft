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

- the armor `OMOD`s by two routes — `template` (every `OMOD` an `ARMO` object template
  names, i.e. a template item's `Includes.Mod`) and `attach-point` (every `OMOD` whose
  `DATA` attach point is a keyword some `ARMO`'s `APPR` lists) — then the union;
- for the union's `OMOD` `DATA` properties, one sorted line per property code with its
  count and value types, and, for the FormID-valued ones (value types 4 and 6), the
  record types those FormIDs name, resolved through the record index of the file that
  holds them; a FormID that resolves to nothing is counted as `unresolved`, never
  panicked on;
- the same two kinds of line for the properties of `ARMO` template items themselves.

## Printed counts

```
armor-omods route template: 335 OMODs
armor-omods route attach-point: 252 OMODs
armor-omods: 445 OMODs, 1148 properties
armor-omod property AENC: 44 (value types 4)
armor-omod property AENC targets: ENCH 43, unresolved 1
armor-omod property WENC: 36 (value types 4)
armor-omod property WENC targets: ENCH 36
armor-omod property WSPD: 3 (value types 1)
armor-omod property ADMG: 145 (value types 6)
armor-omod property ADMG targets: DMGT 143, unresolved 2
armor-omod property WDMG: 3 (value types 1)
armor-omod property GWEI: 1 (value types 1)
armor-omod property AVAL: 79 (value types 0, 1)
armor-omod property GVAL: 40 (value types 1)
armor-omod property WSDM: 1 (value types 1)
armor-omod property WAIM: 1 (value types 4)
armor-omod property WAIM targets: AMDL 1
armor-omod property WMNR: 1 (value types 1)
armor-omod property WNPR: 1 (value types 1)
armor-omod property WMXR: 1 (value types 1)
armor-omod property AMLS: 3 (value types 4)
armor-omod property AMLS targets: LMSW 3
armor-omod property WSTS: 1 (value types 1)
armor-omod property ARAT: 72 (value types 0, 1)
armor-omod property AACT: 309 (value types 6)
armor-omod property AACT targets: AVIF 305, unresolved 4
armor-omod property ACPT: 2 (value types 0, 1)
armor-omod property WDTV: 5 (value types 6)
armor-omod property WDTV targets: DMGT 3, unresolved 2
armor-omod property AKEY: 229 (value types 4)
armor-omod property AKEY targets: KYWD 198, unresolved 31
armor-omod property WKEY: 171 (value types 4)
armor-omod property WKEY targets: KYWD 154, unresolved 17
armor-template property 0xe: 418 (value types 4)
armor-template property 0xe targets: LMSW 395, unresolved 23
```

(`0xe` is property 14, the small-integer property code; it displays as hex because it is
not a four-letter code.)

## What the counts say

Armor mods carry **stats and a material swap**, not stats only.

- The `OMOD` `DATA` payloads of the 445 armor `OMOD`s (the union of the two routes)
  carry 1,148 property entries. Most are plain stats or FormID references to stat-like
  records: `AACT` (309, a FormID+float naming an `AVIF` actor-value form), `AKEY`
  (229, naming `KYWD` keywords), `WKEY` (171, naming `KYWD`), `ADMG` (145, a
  FormID+float naming a `DMGT` damage form), `AVAL` (79), `ARAT` (72), `AENC` (44,
  naming an `ENCH` enchantment), `GVAL` (40), `WENC` (36, naming an `ENCH`), plus a
  long tail of single or few-entry codes (`WSPD`, `WDMG`, `GWEI`, `WSDM`, `WAIM`,
  `WMNR`, `WNPR`, `WMXR`, `WSTS`, `ACPT`, `WDTV`).
- One of those is a material swap: `AMLS` — the editor's "armor layered material swap"
  code — appears 3 times, and all 3 name an `LMSW` (layered material swap) record.
- The `ARMO` object-template items carry their own property, 14, 418 times, and it too
  is a FormID naming an `LMSW` (395 resolved). This is the property the vanilla
  `Clothes_*` colourways use: the material swap is named by the template item, not by
  an `OMOD`.

So the answer to the ticket's question: an armor `OMOD`'s `DATA` is mostly stats
(value/rating/damage/enchantment/keyword entries), but it can also carry a material
swap — `AMLS` naming an `LMSW` — and the vanilla clothing colourways attach their
`LMSW` through the object-template side (property 14 of the `OBTS` template item),
i.e. the template route's mechanism, while the stat `OMOD`s are the template item's
included mods. The attach-point route (252 `OMOD`s) is how the `OMOD`s' own attach
points match an `ARMO`'s `APPR`.

## What is not known

- In-game meaning is not observed. The property codes (`AACT`, `AMLS`, `WENC`, …, and
  the small-integer 14) are display facts from the editor's property-name tables; the
  editor picks which table to show from the containing record's signature (`WEAP`,
  `ARMO`, `NPC_`), so a given numeric code can display under different names in
  different record types. Nothing here is a playtested behaviour.
- A handful of FormID values do not resolve to any record in the official corpus
  (`unresolved` above): `AENC` 1, `ADMG` 2, `AACT` 4, `WDTV` 2, `AKEY` 31, `WKEY` 17,
  template property 14 23. They are counted, not interpreted; they name records that
  are not among the 14 official ESMs.
- `AMLS` (in an `OMOD`) and property 14 (in a template item) both reach `LMSW`, but
  which one the engine consults for a given swap, and whether both can be present, is
  not established by the corpus alone.
