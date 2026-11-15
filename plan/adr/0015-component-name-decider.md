# ADR 0015: A component-name decider for base form component blocks

- Status: **Accepted by the owner, 2026-11-15** ("Go ahead and build it"), the same day
  it was first deferred (KK-281 had drafted it; the owner had then kept the lead-signature
  recommendation of `docs/design/component-blocks.md`). The acceptance is scoped as
  "Acceptance" below records: the name decider is built for the `DATA`/`DAT2` kinds first;
  the existing lead-signature groups stay (KK-292).
- Date: 2026-11-15

## Context

A base form component block (`BFCB` name, data, empty `BFCE`) is typed today by optional
groups of `crates/schema/defs/common/BfcbBlock.toml`, each entered at its leading
subrecord: the decoder reads the data by its first subrecord, as upstream's record union does
(`wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e` L6055-L6828). That cannot
tell apart layouts that start with the same subrecord. Upstream has two: a lone `DATA` whose
inner struct is chosen by the component name (L1479-L1517: star data, orbited data, orbital
data, unique overlay list, unique pattern placement, overlay designated placement) and a lone
`DAT2` chosen the same way (L1519-L1551: block height adjustment, surface tree pattern swap,
block editor metadata). In the 14 official files they are 9 of 64 component kinds and 1,197 of
349,154 blocks (census of 2026-11-15, `docs/design/component-blocks.md`). The schema format's
deciders are an EditorID prefix and an earlier sibling field of the same struct (`Decider` in
`crates/schema/src/model.rs`); neither reaches another subrecord's payload.

## Decision

1. A new member kind in a group, a **subrecord union by a sibling's string**:
   `union = "<Name>"` with `decider = "sub:BFCB"` (the payload of an earlier required
   `zstring` sibling of the same group, compared byte for byte without its NUL), and
   `[[member.variant]]` entries, each `when = ["<component name>", ...]` plus the members
   of one layout (as a `group` today). `BfcbBlock` would hold one such union in place of its
   optional layout groups.
2. **Fallback**: a name no variant lists, or data that does not match the chosen variant
   from its first subrecord to `BFCE`, is kept raw in the `any_until` member exactly as read.
   The decoder never reads a block by another name's layout (stricter than upstream, which
   reads mismatched data by its first subrecord), and it never rewrites the name.
3. **Matching**: the generated matcher gains a `MemberDesc` variant whose match looks up the
   deciding sibling's payload in the current group instance; `kitkraft_doc::typed::placements`
   and `kitkraft_engine::edit::fields::layout_at`/`layouts` take the subrecords' payloads (or
   the blocks' names) besides their signatures, so placement and `set` follow the name.
4. **Editing**: `new` selects a block's layout from the component name its `fields` give
   (`BFCB`), falling back to today's signature inference (`scaffold::record_order`) when no
   name is given.
5. **Scope**: only `BfcbBlock` uses it at first. The existing lead-signature groups move
   into variants one by one, each re-checked over every official block
   (`crates/doc/tests/components.rs`), with 0 re-encode conflicts and the 14 files still
   byte-identical.

## Acceptance (2026-11-15, KK-292)

The owner accepted this decision on 2026-11-15 with the words "Go ahead and build it". It is
built in the order point 5 gives, with the smallest first step that keeps the 14 official
files byte-identical:

- The union holds the nine `DATA`/`DAT2` kinds only (star data, orbited data, orbital data,
  unique overlay list, unique pattern placement, overlay designated placement; block height
  adjustment, surface tree pattern swap, block editor metadata). The sixteen lead-signature
  groups of `BfcbBlock` stay as they are, before the union in member order: none of them
  starts with `DATA` or `DAT2`, so no block they read today changes hands. Point 1's "in
  place of its optional layout groups" is the end state point 5 reaches one group at a time;
  each such move is its own ticket, re-checked over every official block.
- So the stricter rule of point 2 ("never reads a block by another name's layout") holds for
  the union's variants: a `DATA` or `DAT2` block is read by a variant only when its name is
  one the variant lists and its data, from its first subrecord to `BFCE`, is a run the
  variant matches. The remaining lead-signature groups still read a block by its first
  subrecord, as before.
- "Matches" is decided by signatures, as every group is today: a payload the chosen
  variant's layout cannot decode is kept raw in the record's `extra`, as any undecodable
  typed subrecord is, so roundtrip stays byte-identical either way.
- Placement (point 3) takes payloads through a key type that carries a subrecord's
  signature and, when the caller has it, its payload. A caller that passes signatures only
  sees every variant as undecided, so it places those subrecords in the raw `any_until`
  member, as before this ADR; every caller that holds the record passes payloads.

## Consequences

- Every component name gets its own variant: `DATA`/`DAT2` bodies become typable (their
  FormIDs, for example the overlay worldspaces and the pattern placement planet, become
  visible to `refs` and `formid.dangling`), and blocks that share a layout (`REFL` under seven
  names, `MODL` under TESModel and TESPlanetModel) are named by their component.
- The generated matcher stops being signature-only: one more runtime path in
  `crates/esm/src/generated/records.rs`, the golden seed, and an API change through
  `kitkraft-doc` and `kitkraft-engine` placement. That is the bulk of the work (several
  tickets), before any new layout is written.
- Layouts whose bodies need other format work (variable-width tails in `VLMS`, traversal
  arrays in `STRD`, vehicle configuration, activity configuration strings, the planet model's
  `XMPM`) still need it; this decider does not provide it.
- Roundtrip stays byte-identical as long as the fallback is raw; a block whose name and data
  disagree (none in the official files) is kept raw rather than read.

## Alternatives

- Continue the lead-signature groups (the recommendation of `docs/design/component-blocks.md`):
  no format change; covers every layout except the `DATA`/`DAT2` ones.
- Type `DATA`/`DAT2` as opaque named members: names the payload, adds nothing to reading.
- A per-record-type union by the record signature: does not help; the nine kinds sit in four
  record types, and three of those types each carry two or more of them (`STDT` both star and orbited data, `SFPT`
  both overlay list and pattern placement `DATA`, `SFBK` editor metadata `DAT2` and two
  `DATA` kinds).
