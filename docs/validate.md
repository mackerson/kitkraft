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
`data_dir`, which defaults to `KITKRAFT_DATA_DIR` (case-insensitive file names). Their
headers give flags (localized, scale); their record indexes are built when
`formid.dangling` or `override.stripped-name` runs. The MCP resource
`kitkraft://validate-rules` serves the rule list below as JSON.

## Rules

| Rule | Severity | What it finds | Why / source |
| ---- | -------- | ------------- | ------------ |
| `masters.missing` | error | A `MAST` the plugin lists that is in none of the places above | The game refuses to load a plugin with a missing master |
| `masters.order` | error | A master listed before one of its own masters | Masters load in list order; a master must follow its own masters |
| `formid.dangling` | error / warning | A FormID (record header, or a typed member through the generated visitors, M2.B3) whose top byte names no master slot (**error**); a typed member naming a form of the plugin itself that the plugin does not have (**error**: a dangling self-reference, what a verbatim copy from a small or medium plugin leaves; Fable review round 2, finding 3); one whose master has no such form (**warning**: an injected record, or a reference to a deleted form; the record-header case stays a warning). A typed member naming an engine-reserved form (an object ID below 0x800 in `Starfield.esm`'s index space that it does not define) is not a finding: it is counted as `reserved_form_ids` ("Engine-reserved object IDs" below) | File-relative FormIDs (`plan/execution-plan.md` "Format facts"); full-scale slots skip small and medium masters, and `FE`/`FD` FormIDs index the small and medium masters (lane D's class encoding, `kitkraft_doc::RawRef`, `docs/load-order.md` "FormIDs"; M3.E3). A slot after a master of unknown scale, and a small or medium plugin's own record stored full-style (lane D's assumption `full-style-self`), are counted as `unresolved_form_ids`, never guessed; opaque or undecoded subrecords are counted as `unchecked_subrecords` (they shrink as lane B round 3 types land) |
| `lstring.copied-id` | error / warning | In a non-localized plugin, a 4-byte lstring payload (FULL, DESC, a template FULL, an lstring of the corpus table below such as ACTI `ATTX` or MESG `ITXT`, ...) in a record that overrides a record of a localized master, or one whose bytes are not text: "localized string id copied into non-localized plugin" (**error**). In such an override, a non-zero 4-byte payload of unknown layout (no schema layout or opaque bytes, and not classed by the table) may be a copied ID the rule cannot see: **warning** naming the subrecord | The Resupply lesson (`C:\Modding\scripts\fix_resupply_door.py`, ours): the game reads the ID's bytes as the name. Lstring layouts: lane B's schema (M2.B2), `FULL`/`DESC` by the `uesp-skyrim` fact elsewhere, then the corpus lstring table. A legitimate 3-character name in such an override is flagged too (the owner's rule: flag every 4-byte lstring there). A zero payload is not warned about (it reads as the empty string either way), nor one the table classes as not an lstring |
| `override.stripped-name` | error / warning | An override that has no `FULL` (or `NTRM`) where the previous version in load order (the last of the plugin's masters holding the form) has one: **error**, except on `CELL`, `LCTN`, `NPC_`, `QUST` and `WRLD` overrides (**warning**) | The Resupply door: Resupply 2.6 dropped the door's FULL, which removed its name and the "open" prompt on every door of that type. The official DLCs drop names on 92 overrides, all of those five types (`corpus:` `validate_corpus_official_files`: SFBGS003 4 CELL + 20 LCTN, SFBGS008 5 LCTN, SFBGS00D 6 CELL + 6 LCTN + 1 NPC_ + 2 WRLD, SFBGS047 1 WRLD, SFBGS050 11 CELL + 13 LCTN + 3 NPC_ + 6 QUST + 12 WRLD, ShatteredSpace 1 CELL + 1 QUST), so there it is a warning |
| `cell.missing-parent` | error / warning | A cell child (`REFR`, `ACHR`, `PGRE`, `PHZD`, `NAVM`) outside a cell's persistent or temporary children, or a world (type 1) or cell (type 6) children group whose `WRLD`/`CELL` neither the plugin nor its masters have (**error**); a master's record that the plugin does not carry before its children group (**warning**) | Lane A's M4.A1 corpus rules (`kitkraft_esm::cells`, `docs/corpus.md` "Cell and world groups"): every cell child of the 14 official files sits in type 8 or 9, and every children group directly follows the record its label names. A record-less children group still loads in the game as far as known, so a master's record not carried is a warning; `place` always carries it |
| `ref.orphan` | error | A `REFR` or `ACHR` (not deleted) without `NAME`, or whose `NAME` resolves to no record of the plugin or its masters | `corpus:` `crates/esm/tests/refs.rs`: every reference of the corpus that is not deleted has one `NAME`, naming a base record (one exception in `Starfield.esm`, `REFR:00014893` -> `000001FB`, listed in `validate_corpus_official_files`) |
| `ref.outside-block` | error / warning | An interior `CELL` whose block and sub-block are not the last two decimal digits of its object ID, an exterior `CELL` whose `XCLC` grid is not in its block and sub-block (sub-block = grid / 8, block = sub-block / 4), or without `XCLC` (**error**; a **warning** in a small or medium plugin); an exterior reference whose `DATA` position is outside its cell's 100-unit square (**warning**) | Lane A's M4.A1 rules, asserted on all 17,020 interior and 22,682 exterior official cells; 99.98 % of exterior references are inside their cell, so the rest is a warning |
| `ref.persistent-flag` | error | Record flag `0x400` set on a record in temporary children (type 9) or clear in persistent children (type 8), set on a `NAVM`, or on a `CELL` that is not a world's persistent cell (or clear on one that is) | Lane A's M4.A1 rule: flag `0x400` and type 8 together on all 6,714,413 official cell children, no exception |
| `ref.persistence` | warning | A reference in temporary children that a record outside cell children (a quest alias, a location, a package, a script property where typed) or a door link (`XTEL`) of the plugin names | Grade D (ADR 0005 Amendment 3): the engine is expected to need such references persistent; unverified until the game is observed (M4.O2, `docs/design/persistence.md`). Only references the plugin itself holds are checked |
| `cell.actor-without-navmesh` | warning | An `ACHR` in a cell that has no `NAVM` in the plugin's temporary children of that cell, nor in any master's | Actors path on navmesh; KitKraft cannot generate it (W9): the owner generates it in the Creation Kit (F22) and `build` imports it (`[[import]]`) |
| `cell.incc` | error / warning | `TES4` `INCC` differs from the number of interior `CELL` records in the plugin (**error**); interior cells and no `INCC` (**warning**) | `corpus:` `docs/corpus.md` "Writing rules": `INCC` equals the interior cell count in all 14 official files |
| `navmesh.unchanged` | warning | A `NAVM` override whose subrecords equal its master's version | An unchanged navmesh override changes nothing in game, conflicts with any other plugin's navmesh for the cell and needs an `ONAM` entry (lane A's M4.A6 rule) |
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
`wbDefinitionsSF1.pas` first for record definitions. When this table was built the pinned
file was not yet available locally; it has been since lane B round 3
(`C:\Modding\tools\xedit-ref\`, `docs/xedit-reference.md`), and the schema now types
lstrings from it wherever a definition names them. The table stays a corpus measurement
for the signatures the schema keeps opaque or leaves undefined: `corpus:` the 13 official
ESMs that ship English string tables
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

Seeded from our own disassembly (`C:\Modding\scripts\pexdis.py`, then `kitkraft-pex`) of
`C:\Modding\scripts\noc_godmode_patch` (our god-mode patch for NPC Outfit Changer),
observed 2026-10-14:

| Native | Seen in | Why it is listed |
| ------ | ------- | ---------------- |
| `Debug.SetGodMode` | `CS7_SpaceshipLandingQuestScript.pex`: `callstatic debug SetGodMode ::nonevar True` once and `False` three times (the patch's watchdog); `SQ_Airlock_QuestScript.pex`: none (the patch removed its pre-emptive `SetGodMode(True)`, `src/SQ_Airlock_QuestScript.psc` line 149) | A script that turns god mode on and is interrupted (fast travel, a load) leaves the player invulnerable in the save; that was the Outfit Changer bug |

`CassiopeiaPapyrusExtender.IsInGodMode` (a read) is not listed. Scripts are read as loose
`.pex` files, directories of them, or `.ba2` archives given in `scripts` (every `.pex`
entry, through lane G's `kitkraft-ba2`, at most 64 MiB each; a finding names
`<archive>/<path>`, an entry that does not parse is listed in `limitations` and skipped);
`plugin_archives` adds the plugin's own `<stem> - *.ba2` (M3.E3, test
`papyrus_denylisted_call_in_archives`). On the local patch the test `papyrus_denylisted_call_on_the_local_noc_patch`
expects 4 findings for the landing script and none for the airlock script.

## Engine-reserved object IDs

`formid.dangling` skips a typed member whose FormID names an object ID below 0x800 in
`Starfield.esm`'s index space that `Starfield.esm` does not define (`validate::is_reserved`,
`RESERVED_OBJECT_IDS`). Such IDs are counted in `data.reserved_form_ids`, per record type and member path in
`data.reserved_by_member`, and named in `limitations`; they are neither errors nor warnings.

Why (ADR 0005 Amendment 4): no official file defines these IDs, yet the official files name
them from typed members that decode identically everywhere (corpus fact, grade A): the player
reference 0x14 in reference slots (enable parents, attach refs, linked refs, location enable
parents, quest alias specific references), actor values 0x2D0..0x3AC in PRPS property lists
(NPC_, RACE, PKIN and the items family) and in FLST actor-value lists, REFR NAME 0x1FB, and
SMEN PNAM 0x5B (a story manager root). The reading that they are engine-reserved default forms
is grade D until the Creation Kit is observed resolving and keeping them (STATUS Requests to
lane H). What does not change: a low ID that `Starfield.esm` defines is an ordinary form; a
low ID under any other master, or in a record header (an override of a form the master
lacks), is checked as before.

`validate_corpus_official_files` pins the count per plugin, record type and member path (`RESERVED` in
`crates/engine/tests/validate_corpus.rs`), so a family that types a new member naming such
IDs changes the expectation with its evidence instead of hiding it. The test
`formid_dangling_skips_engine_reserved_ids` covers the rule on synthetic plugins.

## FormID 0xFFFFFFFF

A typed member holding 0xFFFFFFFF names no form, like 0: `formid.dangling` does not check
it, and patch remapping keeps it as is (`validate::NONE_FORM_ID`). Upstream allows the value
where a member may be unset (INFO `TRDA` Emotion, xedit `wbDefinitionsSF1.pas` L12816;
53,844 response lines of `Starfield.esm` hold it), and as a FormID its top byte would name a
master slot no plugin has.

## On the official files

`validate_corpus_official_files` (`crates/engine/tests/validate_corpus.rs`, `#[ignore]`,
release) runs every rule on the 14 official ESMs; the result is recorded in
`plan/STATUS.md` (zero errors; each warning kind with its reason). Since M3.E3
(2026-10-14, 83 s) `FE`/`FD` FormIDs are checked too: `unresolved_form_ids` is 0 on all
14 files, the findings are unchanged (0 errors; 92 `override.stripped-name` and 15
`formid.dangling` warnings), and `unchecked_subrecords` (opaque payloads) is, e.g.,
21,273,913 in `Starfield.esm` and 850 in `Constellation.esm`.
