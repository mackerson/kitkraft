# Dialogue: new and cloned topics and responses

Design note for KK-274 (implemented by KK-275 and KK-276). It says how `new` and `clone`
write a record that lives in nested groups, a dialogue topic (`DIAL`) under its quest
and a dialogue response (`INFO`) under its topic, and what stays unverified. Scenes
(`SCEN`, raw action bodies), voice files and lip generation are out of scope.

**Unverified (owner decision, 2026-11-15).** Dialogue ships before a Creation Kit
observation of a plugin with quest children. Group type 10 (quest children) has no name
on a permitted source, and no CK-saved multi-quest plugin has been compared with ours
(`docs/gaps.md`). Everything below is read off the 14 official files (`corpus`, grade A
for what the files hold, D for what the CK or the game would do). Nothing here claims CK
parity or game behaviour.

## What the official files show

Measured 2026-11-15 over the 14 files (a group walk of every top-level `QUST` group;
the script is our own and is not kept):

| Fact | Count |
| ---- | ----: |
| Type 10 groups, each immediately after the `QUST` record its label names | 1,808 of 1,808 |
| ... of which under an override of a master's quest (the `QUST` record is in the file) | 13 |
| Type 7 groups, each immediately after the `DIAL` record its label names | 81,287 of 81,287 |
| `DIAL` whose `QNAM` is the quest of its type 10 group | 87,031 of 87,031 |
| `DLBR` whose `QNAM` is the quest of its type 10 group | 135 of 135 |
| `DIAL` with no type 7 group: all have `TIFC` 0 and an empty `TIFL` | 5,744 |
| A file's own `DIAL`: `TIFL` holds exactly the `INFO`s of its type 7 group, `TIFC` their count | 81,267 of 81,267 |
| ... `TIFL` in the group's order (FormID ascending) | 73,278 (the rest list another order) |
| An override of a master's `DIAL`: `TIFC` = `TIFL` length; `TIFL` = its group's `INFO`s, or more (the master's too) | 20; 17 equal, 3 more |
| `DIAL` `BNAM` naming a branch of the same quest | 2,420 of 2,425 (5 name a branch elsewhere) |
| Type 10 and type 7 group stamps (timestamp, version control, unknown) | all 0 |
| Records in type 10: `DLBR`, `DIAL`, `SCEN`, each by FormID; in type 7: `INFO` by FormID | every group |
| `DIAL` record flags | all 0 |

## Arguments

`new` and `clone` take one more argument, `parent`, on every front end (CLI `--parent`,
the engine command's JSON, MCP `execute`, the build spec's `[[new]]` and `[[clone]]`):

- `parent` is a form as `set` names one (`Plugin.esm:XXXXXX`, an EditorID; a hex FormID
  is read in `plugin` for `clone`). In a build spec it may also be `new:<name>`, a
  `[[new]]` or `[[clone]]` of the same build.
- Required for `DIAL` (and `DLBR` if it is typed) and `INFO`: `new.no-parent` /
  `clone.no-parent`. `clone` does not default to the source's parent; its refusal names
  the source's parent so it can be passed on.
- Refused when given for a type of its own top-level group: `new.parent-not-nested`.
- The parent must exist (in the load order, or the build) and not be deleted:
  `new.parent-missing`. It must be of the right type: a `QUST` for a `DIAL`, a `DIAL` for
  an `INFO` (`new.parent-type`, naming the type found). A `new:<name>` of the wrong type
  is refused the same way.
- `SCEN` stays refused (`new.not-top-level`): its action bodies are raw.
- `clone` of a nested record without a `parent` keeps refusing (`clone.no-parent`); a
  clone is never placed under its source's parent implicitly.

## What is written

For a new or cloned `DIAL` under quest `Q`:

- `QNAM` is set to `Q` (in the target's FormID space). A `fields` value for `QNAM` that
  names another quest is refused (`new.parent-mismatch`): the corpus has no topic whose
  `QNAM` differs from its group's quest.
- A clone's `TIFL` is emptied and `TIFC` set to 0: a clone owns no responses (responses
  are cloned one by one with the clone as their parent). Its `BNAM` is kept as stored and
  a limitation says when it names a branch of another quest.
- Its group path: the top-level `QUST` group, then the type 10 group labelled with `Q`'s
  stored FormID, placed right after the `QUST` record.

For a new or cloned `INFO` under topic `T` (of quest `Q`):

- Its group path: top-level `QUST`, type 10 labelled `Q`, type 7 labelled `T`, placed
  right after the `DIAL` record.
- `T`'s `TIFL` gets the new FormID appended and `TIFC` is set to the `TIFL` length (the
  writer does it once the FormID is allocated). Appending is our choice: the order the CK
  gives a new response in `TIFL` is not observed.

The parent records:

- A master's quest (or topic) is carried into the patch as an unedited override of its
  winning version, as `place` carries a cell. That is what the 13 official overrides
  show, but it makes the patch override the quest: a later plugin that edits the same
  quest conflicts with it. Each result says so in `limitations`.
- A master's topic that gets a new response is an override with an edited `TIFL`/`TIFC`,
  and its own quest is carried before it. As an override in a type 10 group, the topic
  is listed in `ONAM` by the M4.A6 rule. New records are not overrides and are not
  listed; the carried quest is top-level and is not listed.
- A quest or topic new in the same build (`new:<name>`) is written by its own entry; the
  child's labels and `QNAM` take its FormID once it is allocated.
- A quest or topic already in the target is used where it is (with `active_file`, the
  target's own records are in the load order; without it a target's own form cannot be
  named, as for every other `new` field).

## Groups, stamps, order

- Missing groups are created by `kitkraft_esm` (`Plugin::upsert`), which already refuses
  a children group whose owner record is not in its parent group. The writer places the
  parents first: every new nested record is written after the records of the call, a
  topic before its responses.
- Stamps and labels are the official style (`PlacementStyle::OFFICIAL`): 0, as all
  official type 10 and type 7 groups. A new plugin also gets the empty top-level `DIAL`,
  `INFO`, `DLBR` and `SCEN` groups the official files have (existing writer rule).
- Order inside a type 10 group: `DLBR`, `DIAL`, `SCEN`, each by FormID; inside a type 7
  group: by FormID (existing writer rule, `build.rs` `record_key`).
- Group labels are FormIDs in the target's space. A copied record whose FormIDs are
  remapped (masters in another order) has the labels of its children groups remapped too.

## Unverified

- Group type 10 has no permitted-source name; its meaning is read off the corpus only.
- What the CK writes for a new topic or response: group stamps (the one-cell CK plugin
  stamped its groups `0x3561`, `ck-20261101-04`), whether it carries the quest override,
  the `TIFL` order, the record header stamps (`new`'s usual limitation).
- Whether the game shows a new topic or response: not run. Voice files and lip data are
  not written, so a response plays silently at best.
- Confidence of every new fact: `corpus`. `ck_open`, `ck_reexport`, `game_runtime` and
  `save_reload` stay `not-run`.
