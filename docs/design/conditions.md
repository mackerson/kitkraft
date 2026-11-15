# Authoring conditions (`CTDA`)

**Status: design, KK-267 (2026-11-15).** How a condition is addressed and written through
`set`, `new`, `clone`, `place` and a `build` spec's `[[set]]` / `[[new]]` / `[[clone]]`
fields. It governs KK-268 to KK-272. Layouts are those of
`crates/schema/defs/common/Condition.toml` (`xedit: wbDefinitionsSF1.pas@9fb01688#L4750-L4840`
and the helpers it cites); nothing here is a new format fact.

## What a condition is

One condition is one instance of the shared `Condition` group: a 32-byte `CTDA`, then an
optional `CIS1` and an optional `CIS2` (string parameters). Records hold them as a
repeated group (`Conditions`, `Entries.Conditions`, `Stage.LogEntry.Conditions`, ...),
in most types directly after a `CITC` condition count (the list is in "CITC" below).

## Addressing

Conditions are addressed with the field paths every write command already takes
(`crates/engine/src/edit/fields.rs`); no new syntax.

- `CTDA[n]` is the record's `n`-th `CTDA` in on-disk order, counted across every
  condition group of the record (`CTDA` alone is `CTDA[0]`). `CIS1[n]` / `CIS2[n]` count
  their own signature, not the condition they belong to.
- Members: `CTDA[n].Type` (the raw byte), `CTDA[n].Type.Operator`, `CTDA[n].Type.Flags`
  (KK-269), `CTDA[n].ComparisonValue.Float` or `.Global`, `CTDA[n].Function` (a name or an
  index, KK-270), `CTDA[n].Parameter1.<Variant>`, `CTDA[n].Parameter2.<Variant>`,
  `CTDA[n].RunOn`, `CTDA[n].Reference.Reference`, `CTDA[n].Parameter3.<Variant>`. A
  variant step is accepted only when the entry's own decider selects it (the union rule of
  `crates/schema/defs/README.md`): `Global` needs the `UseGlobal` flag, `Parameter1.Faction`
  a function whose first parameter is a faction, `Reference.Reference` `RunOn =
  Reference`, `Parameter3.LinkedKeyword` `RunOn = LinkedReference`.
- Deciders first. In one `fields` object the writer applies the member edits of one
  subrecord in the definition's field order (whole-subrecord edits first), not in the
  object's key order. JSON objects reach the engine with sorted keys, so before this
  `CTDA.Reference.Reference` (sorted before `CTDA.RunOn`) and `CTDA.ComparisonValue.Global`
  (before `CTDA.Type`) were refused for a decider not yet written (KK-268 fixes this).

## Adding a condition

Name the next occurrence: on a record with `k` conditions, `CTDA[k]` (and its members)
adds one. The writer creates it at its schema slot (`scaffold::add_slots`, `slot_of`):
the `k`-th `CTDA` slot of the record in member order, the last repeated slot taking the
rest. In a type with one condition group that is "append to the group". A gap
(`CTDA[k+1]` on a record with `k`) is refused for a copied record, as for every signature.
The new `CTDA` starts as 32 zero bytes: operator `EqualTo`, no flags, comparison value
0.0, function 0 (`GetWantBlocking`), run on `Subject`; then the edits are written. A
`CIS1` / `CIS2` is added the same way, by naming its next occurrence; it lands at its slot
after the `CTDA` it follows.

Limits (kept, documented in the recipe): a record with several condition groups (LVLI's
list and entry conditions, PERK ranks, QUST stages) places the `k`-th occurrence by the slot
rule only, so a condition cannot be put into a chosen instance of a nested repeated group;
and a string parameter cannot be attached to a chosen condition except by occurrence order.

## Removing a condition

`"CTDA[n]": null` removes the `n`-th condition: its `CTDA` and the `CIS1` / `CIS2` directly
after it (KK-271). Removals are applied after every other edit of the record, highest
occurrence first, so every occurrence number in one `fields` object means the record as
copied. A removal together with an edit of a member of the same occurrence is refused, and
so is a removal past the record's last condition. `new` has nothing to remove and refuses
it. `null` on a whole `CTDA` meant nothing before (a struct takes members or `hex:`).

## Operator and flags (KK-269)

The type byte packs two values (`wbDefinitionsCommon.pas@9fb01688#L3494-L3580`): the
comparison operator in its top three bits and five flags in the low five. The schema gets a
`packed` field type (an integer base split into named bit ranges, each an `enum`, `flags`
or plain integer), and `Type` becomes:

- `Type.Operator` (bits 5..7): `EqualTo` 0, `NotEqualTo` 1, `GreaterThan` 2,
  `GreaterThanOrEqualTo` 3, `LessThan` 4, `LessThanOrEqualTo` 5. Values 6 and 7 have no
  name upstream and are kept as read.
- `Type.Flags` (bits 0..4): `Or` 0x01, `UseAliases` 0x02, `UseGlobal` 0x04, `UsePackdata`
  0x08, `SwapSubjectAndTarget` 0x10.

A part is written in place: the other bits of the byte keep their value. `CTDA.Type` still
takes the whole byte as an integer. The generated type keeps the byte (so every corpus
`CTDA` re-encodes as read) and the deciders that read `Type` (the comparison value) read the
whole byte, as now.

## Function names (KK-270)

`Function` becomes an `enum` (`u16`) with one value per function the pinned table names
(`wbDefinitionsSF1.pas@9fb01688#L150-L761`: the index and the name; the descriptions are
not copied). `CTDA.Function` takes the name (ASCII case-insensitive) or the index. An index
the table does not name decodes and re-encodes as read (an `enum` keeps unknown values).
The parameter unions keep deciding by the stored index.

## CITC (KK-271)

A `CITC` member directly before a `Condition` group in the definition (ACTI, FACT, FLST,
HAZD, LGDI, MUST, PACK, PCBN, PCMT, PERK, SECH, SMBN, SMEN, SMQN, TERM and the
`PlanetContentProperties` component layout; COBJ, LVLI and the quest records' groups have
none) counts the conditions that follow it. It is derived on write, like a `count_from`
array count:

- On a write that adds or removes a `CTDA` of a record (any of `set`, `new`, `clone`,
  `build`; `place` writes no conditions), every `CITC` of the record that heads a condition
  run is set to the number of `CTDA` in the run that follows it. An explicit `CITC` value
  in the same fields is overwritten, and the limitation says so when it differed.
- A run that gets a condition and has no `CITC` where its definition has one gets it
  (added at its slot, as `KSIZ` comes with `KWDA`). Every official record of a type with a
  `CITC` slot carries it when it has conditions.
- A write that changes no condition count leaves every `CITC` as stored, so an unedited or
  member-edited record still re-encodes byte for byte.
- `validate` reports a `CITC` that disagrees with the run after it (`condition.count`,
  `docs/validate.md`), a warning: what the game does with a wrong count is not observed.

## What stays raw, and why

- Parameters 1 and 2 of the functions whose parameter is a reference, an actor or a
  package (EL-03, owner decision: out of scope): the type byte's `UseAliases` /
  `UsePackdata` bit turns them into an alias index or a packdata ID, a second decider the
  format does not express. They are written whole, `CTDA[n].Parameter1 = "hex:..."` (4
  bytes), or with the whole `CTDA` as `hex:`.
- The parameters of a function index the table does not name, the same way.
- The enumerations of enum-typed parameters (axis, sex, casting source, ...) stay `u32`
  numbers; they are not transcribed by this work.
- `Unused1` / `Unused2` stay opaque (written as zeros by a new condition).

## Evidence a status needs

Typed-writable "yes" for conditions (`docs/foothold/gap-matrix.md`, "Statuses") rests on
`crates/engine/src/handlers/condition_tests.rs`, which writes every typed member of a
condition, both comparison value variants and a FormID parameter, reads them back and
checks reference integrity, and on the recipe `docs/recipes/conditions.md`. Typed-readable
stays partial while EL-03 holds. CK-diff and game stay `not-run`.
