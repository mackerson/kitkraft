# Validation rules

`kit validate` (`crates/engine/src/validate/`, M2.E6..E8) checks one plugin against its
masters, and compiled scripts against a denylist. Every finding is
`{rule, severity, plugin, record, subrecord, offset, message}`; any `error` makes
`data.passed` false and `kit` exit 1. Arguments and result shape: `docs/mcp.md`
("`validate`"). Each rule has a positive and a negative synthetic fixture in
`crates/engine/src/validate/tests.rs` (no third-party data: the Resupply door is rebuilt
from scratch).

Masters are found in the named or session load order, next to the plugin, then in
`data_dir` (case-insensitive file names). Their headers give flags (localized, scale);
their record indexes are built when `formid.dangling` or `override.stripped-name` runs.

## Rules

| Rule | Severity | What it finds | Why / source |
| ---- | -------- | ------------- | ------------ |
| `masters.missing` | error | A `MAST` the plugin lists that is in none of the places above | The game refuses to load a plugin with a missing master |
| `masters.order` | error | A master listed before one of its own masters | Masters load in list order; a master must follow its own masters |
| `formid.dangling` | error / warning | A FormID (record header, or a typed member through the generated visitors, M2.B3) whose top byte names no master slot (**error**); one whose master has no such form (**warning**: an injected record, or a reference to a deleted form) | File-relative FormIDs (`plan/execution-plan.md` "Format facts"); full-scale slots skip small and medium masters (lane D, `docs/load-order.md`). `FE`/`FD` FormIDs and slots after a master of unknown scale are counted as `unresolved_form_ids`, never guessed; opaque or undecoded subrecords are counted as `unchecked_subrecords` |
| `lstring.copied-id` | error | In a non-localized plugin, a 4-byte lstring payload (FULL, DESC, a template FULL, ...) in a record that overrides a record of a localized master, or one whose bytes are not text: "localized string id copied into non-localized plugin" | The Resupply lesson (`C:\Modding\scripts\fix_resupply_door.py`, ours): the game reads the ID's bytes as the name. Lstring layouts: lane B's schema (M2.B2), `FULL`/`DESC` by the `uesp-skyrim` fact elsewhere. A legitimate 3-character name in such an override is flagged too (the owner's rule: flag every 4-byte lstring there) |
| `override.stripped-name` | error / warning | An override that has no `FULL` (or `NTRM`) where the previous version in load order (the last of the plugin's masters holding the form) has one: **error**, except on `CELL`, `LCTN`, `NPC_`, `QUST` and `WRLD` overrides (**warning**) | The Resupply door: Resupply 2.6 dropped the door's FULL, which removed its name and the "open" prompt on every door of that type. The official DLCs drop names on 92 overrides, all of those five types (`corpus:` `validate_corpus_official_files`: SFBGS003 4 CELL + 20 LCTN, SFBGS008 5 LCTN, SFBGS00D 6 CELL + 6 LCTN + 1 NPC_ + 2 WRLD, SFBGS047 1 WRLD, SFBGS050 11 CELL + 13 LCTN + 3 NPC_ + 6 QUST + 12 WRLD, ShatteredSpace 1 CELL + 1 QUST), so there it is a warning |
| `record.flags-body` | error / warning | A compressed record whose body does not decompress (**error**); in a localized plugin an lstring that is not 4 bytes, in a non-localized one an lstring that is not one NUL-terminated string (**error**); a `HEDR` record count that is neither "records including TES4" nor "records without TES4 plus groups" (**warning**) | Container rules (`crates/esm`); the two `HEDR` meanings from the corpus (`docs/corpus.md`, TES4 header inventory and Writing rules) |
| `papyrus.denylisted-call` | warning | A `callstatic` / `callmethod` in a `.pex` to a denylisted native | Denylist below |

## Denylist (`papyrus.denylisted-call`)

Seeded from our own disassembly (`C:\Modding\scripts\pexdis.py`, then `kitcraft-pex`) of
`C:\Modding\scripts\noc_godmode_patch` (our god-mode patch for NPC Outfit Changer),
observed 2026-10-14:

| Native | Seen in | Why it is listed |
| ------ | ------- | ---------------- |
| `Debug.SetGodMode` | `CS7_SpaceshipLandingQuestScript.pex`: `callstatic debug SetGodMode ::nonevar True` once and `False` three times (the patch's watchdog); `SQ_Airlock_QuestScript.pex`: none (the patch removed its pre-emptive `SetGodMode(True)`, `src/SQ_Airlock_QuestScript.psc` line 149) | A script that turns god mode on and is interrupted (fast travel, a load) leaves the player invulnerable in the save; that was the Outfit Changer bug |

`CassiopeiaPapyrusExtender.IsInGodMode` (a read) is not listed. Scripts are read as loose
`.pex` files or directories; scripts inside archives are not scanned yet (a limitation in
the result). On the local patch the test `papyrus_denylisted_call_on_the_local_noc_patch`
expects 4 findings for the landing script and none for the airlock script.

## On the official files

`validate_corpus_official_files` (`crates/engine/tests/validate_corpus.rs`, `#[ignore]`,
release) runs every rule on the 14 official ESMs; the result is recorded in
`plan/STATUS.md` (zero errors; each warning kind with its reason).
