# Validation rules

`kit validate` (`crates/engine/src/validate/`, M2.E6..E8) checks one plugin against its
masters, and compiled scripts against a denylist. Every finding is
`{rule, severity, plugin, record, subrecord, offset, message}`; any `error` makes
`data.passed` false and `kit` exit 1. Arguments and result shape: `docs/mcp.md`
("`validate`"). Each rule has a positive and a negative synthetic fixture in
`crates/engine/src/validate/tests.rs` (no third-party data: the Resupply door is rebuilt
from scratch).

`set` and `build` run the plugin rules on what they write before it reaches its
destination (both stage the plugin in a private directory under the OS temp dir): `build`
writes nothing when the new plugin has any error; `set` leaves `into` unchanged when an
error is on a record it wrote (or on the plugin as a whole), and reports errors on records
it did not write as a limitation. Their results carry the run as `data.validate`.

Masters are found in the named or session load order, next to the plugin, then in
`data_dir`, which defaults to `KITCRAFT_DATA_DIR` (case-insensitive file names). Their
headers give flags (localized, scale); their record indexes are built when
`formid.dangling` or `override.stripped-name` runs. The MCP resource
`kitcraft://validate-rules` serves the rule list below as JSON.

## Rules

| Rule | Severity | What it finds | Why / source |
| ---- | -------- | ------------- | ------------ |
| `masters.missing` | error | A `MAST` the plugin lists that is in none of the places above | The game refuses to load a plugin with a missing master |
| `masters.order` | error | A master listed before one of its own masters | Masters load in list order; a master must follow its own masters |
| `formid.dangling` | error / warning | A FormID (record header, or a typed member through the generated visitors, M2.B3) whose top byte names no master slot (**error**); a typed member naming a form of the plugin itself that the plugin does not have (**error**: a dangling self-reference, what a verbatim copy from a small or medium plugin leaves; Fable review round 2, finding 3); one whose master has no such form (**warning**: an injected record, or a reference to a deleted form; the record-header case stays a warning) | File-relative FormIDs (`plan/execution-plan.md` "Format facts"); full-scale slots skip small and medium masters (lane D, `docs/load-order.md`). `FE`/`FD` FormIDs and slots after a master of unknown scale are counted as `unresolved_form_ids`, never guessed; opaque or undecoded subrecords are counted as `unchecked_subrecords` |
| `lstring.copied-id` | error / warning | In a non-localized plugin, a 4-byte lstring payload (FULL, DESC, a template FULL, an lstring of the corpus table below such as ACTI `ATTX` or MESG `ITXT`, ...) in a record that overrides a record of a localized master, or one whose bytes are not text: "localized string id copied into non-localized plugin" (**error**). In such an override, a non-zero 4-byte payload of unknown layout (no schema layout or opaque bytes, and not classed by the table) may be a copied ID the rule cannot see: **warning** naming the subrecord | The Resupply lesson (`C:\Modding\scripts\fix_resupply_door.py`, ours): the game reads the ID's bytes as the name. Lstring layouts: lane B's schema (M2.B2), `FULL`/`DESC` by the `uesp-skyrim` fact elsewhere, then the corpus lstring table. A legitimate 3-character name in such an override is flagged too (the owner's rule: flag every 4-byte lstring there). A zero payload is not warned about (it reads as the empty string either way), nor one the table classes as not an lstring |
| `override.stripped-name` | error / warning | An override that has no `FULL` (or `NTRM`) where the previous version in load order (the last of the plugin's masters holding the form) has one: **error**, except on `CELL`, `LCTN`, `NPC_`, `QUST` and `WRLD` overrides (**warning**) | The Resupply door: Resupply 2.6 dropped the door's FULL, which removed its name and the "open" prompt on every door of that type. The official DLCs drop names on 92 overrides, all of those five types (`corpus:` `validate_corpus_official_files`: SFBGS003 4 CELL + 20 LCTN, SFBGS008 5 LCTN, SFBGS00D 6 CELL + 6 LCTN + 1 NPC_ + 2 WRLD, SFBGS047 1 WRLD, SFBGS050 11 CELL + 13 LCTN + 3 NPC_ + 6 QUST + 12 WRLD, ShatteredSpace 1 CELL + 1 QUST), so there it is a warning |
| `record.flags-body` | error / warning | A compressed record whose body does not decompress (**error**); in a localized plugin an lstring that is not 4 bytes, in a non-localized one an lstring that is not one NUL-terminated string (**error**); a `HEDR` record count that is neither "records including TES4" nor "records without TES4 plus groups" (**warning**) | Container rules (`crates/esm`); the two `HEDR` meanings from the corpus (`docs/corpus.md`, TES4 header inventory and Writing rules) |
| `papyrus.denylisted-call` | warning | A `callstatic` to a denylisted native, or a `callmethod` of a denylisted name on a receiver whose declared type (`self`: the script's object; else a parameter, local or object variable) is the denylisted class. A user script's own method that shares the name (`MyQuest.SetGodMode`) is not a finding, nor a receiver whose type the script does not declare | Denylist below |

## lstring signature table

Lane B's schema types lstrings only where a definition names them. A localized plugin
stores every lstring as a 4-byte string ID, so an lstring the schema keeps opaque (`ATTX`,
`ITXT`, `RNAM`, `SHRT`, BOOK `CNAM`, ...) would be copied into a non-localized patch as
four bytes the game shows as text, and `lstring.copied-id` could not see it (Fable review
round 2, finding 2). `crates/engine/src/edit/lstrings.rs` holds a per-record-type table:
which subrecord signatures are lstrings, which are not, and which the corpus cannot tell.

**Source.** ADR 0004 ("Owner approval", clause 3) puts the pinned xEdit
`wbDefinitionsSF1.pas` first for record definitions, but it is not available locally
(`plan/STATUS.md`, Blockers; not at `C:\Modding\tools\xedit-ref\`), so the table is built
from the corpus: `corpus:` the 13 official ESMs that ship English string tables
(`BlueprintShips-SFBGS050.esm` has none), confidence `corpus`. The test
`crates/engine/tests/lstring_table.rs` (`lstring_table_matches_the_corpus`, `#[ignore]`,
release, 21 s) rebuilds the table and fails on any difference; it is deterministic.

**Method.** For each (record type, subrecord signature), over the distinct non-zero
4-byte values per file, checked against that plugin's own `.STRINGS`, `.DLSTRINGS` and
`.ILSTRINGS` (English, from its archives):

| Class | Rule |
| ----- | ---- |
| lstring | exactly 4 bytes in every occurrence; at least 5 distinct values; more than 95% of them are IDs in the tables; more than 98% of those in one kind of table (the game reads a field from one kind); at most 1% are also the ID of another lstring of the same plugin |
| not an lstring | some occurrence is not 4 bytes (a localized file stores every lstring as a 4-byte ID); or at most 50% resolve; or the resolving ones spread over the kinds (at most 90% in one); or more than 10% are another lstring's ID |
| unclassified | the rest (too few values, only zeros, between the thresholds), and every pair the corpus does not have |

"Another lstring" is decided in two rounds: first `FULL` and `DESC` (the `uesp-skyrim`
fact), then every lstring the first round found. The two extra tests exist because
string IDs and FormIDs share dense ranges in `Starfield.esm`: the plain ">95% resolve"
rule also admitted FormID and small-integer fields (LIGH `KWDA` 97.7% from one repeated
keyword, REFR `XFLG` 100% from four small values, RACE `FMRI`, SCEN `FLMV`), whose values
hit other lstrings' IDs or spread over the three kinds. Every lstring found has 0 values
shared with another lstring and all of them in one kind.

Result (2026-10-14): 185 record types, 105 lstring signatures (FULL on most types; DESC;
and outside the schema e.g. ACTI/FLOR/FURN/NPC_ `ATTX`, NPC_ `SHRT`/`LNAM`, INFO `NAM1`
(ILSTRINGS) and `RNAM`, MESG `ITXT`/`NNAM`, BOOK `ENAM`/`FNAM`, QUST `CNAM`/`NNAM`/`QMDT`,
TMLM `ITXT`/`BTXT`/`UNAM`, WEAP `WABB`, GBFM `HULL`, AMMO/LVLI `ONAM`), and 121 pairs
unclassified (113 of them 4 bytes and always zero in the corpus).

**Use.** The patch writer inlines table lstrings of a localized source like schema ones
(with a limitation naming the pair), and refuses a copy that holds a non-zero 4-byte
subrecord of unknown layout the table leaves unclassified, naming each (`ATTX[0]`);
`allow_untyped_lstrings` (`--allow-untyped-lstrings`) copies them as stored and names them
in a limitation. `validate` uses the table for `lstring.copied-id` (above).

**Limits.** The method cannot see an lstring inside a larger struct (such a subrecord is
never 4 bytes in a localized file); the schema's struct-tail lstrings cover the typed
ones. Pairs only third-party plugins use are unclassified, so copying them from a
localized source needs the escape. The table is replaced by schema typing as lane B
transcribes the pinned definitions.

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
