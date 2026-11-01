# Reflection payloads across the official files (2026-10-31)

A read-only scan of the 14 files of `corpus/manifest.toml` for subrecords in Bethesda's
reflection serialization, the chunk format `crates/doc/src/refl.rs` decodes for `REFL`
(KK-007). Question: how much of the surface the schema does not type yet describes its own
layout, so that a reader needs no upstream source.

**Answer: six signatures, every occurrence self-describing, 46,863 payloads in all; that
is about 1.3 % of the 3,588,646 subrecord occurrences the schema keeps opaque or leaves
unknown.** Most are opaque; those inside component blocks a type keeps raw count as
unknown (PNDT's 1,704 `PCCC`, for one; the scan did not split the two). The payloads name their classes and fields, so the "helper in
another upstream unit" (`wbDefinitionsReflection.pas`, not an authorized source under ADR
0004 clarification 2) is not needed to read them: the corpus is the source. The bulk of
the untyped surface, the base form component blocks, is not in this format.

## Method

A throwaway Python walker (not in the repo) read each file in place: groups, records
(zlib-inflated where compressed), subrecords (`XXXX` sizes honored), tracking the BFCB
component name between `BFCB` and `BFCE`. A payload counts as reflection data when it
starts with the `BETH` tag; for those, the `STRT` string table and the `CLAS` chunks were
parsed as `refl.rs` documents them (class name, field count, then per field a name
offset, a type, an offset and a size). The scan wrote nothing and needed no game, CK or
xEdit. Its `REFL` count, 29,897 of them 17,410 `LMSW`, is the count `refl_corpus` pins.

## Where the format occurs

| Signature | Payloads | Record types (largest first) |
| --- | ---: | --- |
| `REFL` | 29,897 | LMSW 17,410, STAT 3,498, FURN 1,659, PKIN 941, MTPT 906, BOOK 810, EFSQ 728, ACTI 499, TERM 496, FFKW 493, CELL 388, CONT 238, IMAD 224, IMGS 217, MSTT 217, EXPL 207, and others |
| `XNSE` | 11,433 | REFR (navmesh spline data: `NavMeshSplineExtraData::ChunkData`, its `References` and `RefID`s) |
| `PTCL` | 2,733 | IPCT 982, STAT 749, MSTT 369, PROJ 268, EXPL 169, and others |
| `PCCC` | 1,825 | PNDT 1,704, STDT 121 (`HoudiniDataComponent`: `HDAFilePath`, `AssetName`, named parameters) |
| `RDIF` | 898 | ATMO 597, WTHS 169, IMGS, SUNP (a parent's settings plus differences) |
| `PSDF` | 77 | |

Every occurrence of the six signatures is in this format (no exceptions in 69 record
type / signature pairs). Across the payloads, 214 classes with 994 fields are defined
(first definition seen per class name; the scan did not check that every payload defines
a class identically). Among them: weather (`BGSWeatherSettingsForm` and its colour, fog,
precipitation, lightning and wind settings), image spaces (`ImageSpaceSettings`: exposure,
bloom, tone mapping, depth of field), atmospheres (`BGSAtmosphere`: scattering, stars,
celestial bodies), sun presets (`BSGalaxy::BGSSunPresetForm`), LOD settings
(`BGSLodOwnerComponent`), effect sequences, particle attachments
(`BSAttachConfig::ParticleAttachment`), float curves, Wwise sound hooks, material swaps.

Field types are a `0xFFFFFFxx` built-in or another class. 351 of the 994 fields name a
class; the built-ins seen are `02` (173 fields), `11` (168), `10` (134), `03` (82), `05`
(53), `0D` (21), `09` (5), `04` (3), `0F` (2) and `0C` (2). `refl.rs` already knows
`0D` as the tag of a form reference; the sizes and names suggest `10` is a bool and `11`
a float, which a decoder must establish by byte-identical re-encoding, not by name.

## The component blocks

349,154 BFCB blocks of 64 component kinds. Nine kinds carry reflection payloads, 14,209
blocks (4 %): `BGSLodOwner_Component` 5,784 and `BGSEffectSequenceComponent` 3,144
(`REFL`), `ParticleSystem_Component` 2,733 (`PTCL`), `HoudiniData_Component` 1,825
(`PCCC`), `ReflectionProbes_Component` 388, `BGSMaterialPropertyComponent` 177,
`LensFlareAttachmentFormComponent` 95, `LightAttachmentFormComponent` 58 and
`BGSSpacePhysicsComponent` 5 (`REFL`). The other 55 kinds hold ordinary subrecords
(contact shadow `FLCS` 147,487 blocks, keywords `KSIZ`/`KWDA` 95,999, volumes `VLMS`
23,159, object palette defaults, full name, animation graph, model and the rest), whose
layouts xEdit reads by component name: that is the component-name decider (format work,
`docs/gaps.md`), not this.

## What it is useful for

- **A schema-driven value reader for all six signatures.** `refl.rs` decodes the chunks of
  `REFL` only and reads values for `LMSW` only; the other 12,487 `REFL`s and the 16,966
  payloads of the other five signatures are kept as bytes. Reading values from the
  payload's own `CLAS` chunks would give named, typed values for the classes above, with
  the corpus as the source. The one thing to establish is the built-in type table, and
  byte-identical re-encoding of all 46,863 payloads checks it.
- **FormIDs inside them.** Form references (`0xFFFFFF0D`) inside these payloads are not
  visited today, which is why patch remapping refuses records that hold them
  (`docs/gaps.md`, world-family payloads). A reader that walks the classes can visit them.
- **A small lever for the unknown subrecords.** Of the 740,681 unknown occurrences
  (`docs/record-parity.md`), most are ordinary component-block bodies, which need the
  component-name decider, and fields xEdit leaves unknown; only the reflection payloads
  inside raw component blocks are in this format.
