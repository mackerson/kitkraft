# Load order

How KitCraft turns an MO2 profile into an ordered list of plugins, and how it resolves
FormIDs across that list. Every rule names its evidence: a file and line we observed, a
corpus test, or a public document (cited by URL). Rules marked **assumption** carry a name
that results and code use; they hold until the named observation confirms or replaces them.

MO2 is GPL-3 software that handles Bethesda formats; its source is never read (AGENTS.md,
"Clean room"). The rules below come from the files MO2 writes, MO2's public user
documentation, and community documentation. libloadorder, esplugin and LOOT code are GPL
too and were not read; LOOT could later serve as an external oracle (run as a program).

Evidence profile: `C:\Modding\MO2-Starfield\profiles\Default` (the owner's live install,
read-only), observed 2026-10-14. Code: `crates/io` (profile, overlay, plan) and
`crates/doc` (index, cache, resolution, overrides).

## MO2 profile (M2.D1)

| File | Rule | Evidence |
| ---- | ---- | -------- |
| `modlist.txt` | One mod per line after a `#` comment header. Prefix `+` enabled, `-` disabled, `*` unmanaged. Names ending `_separator` are separators, not mods. | Default profile: 98 `+` lines, 6 `*` lines, 0 `-`, 0 separators; header line 1. Separators and `-` are handled per the MO2 UI's meaning but **not observed** in this profile (fixture-tested only). |
| `modlist.txt` order | **The first line is the highest priority** (wins file conflicts). | (1) nolvus.net's MO2 customization appendix: modlist.txt is "ordered as install order in the MO2 left panel" and "the order is reversed in the file" (https://nolvus.net/appendix/installer/customization); (2) STEP's MO2 guide: higher priority overrides lower, and Overwrite is the "fake mod located at the bottom of the Left Pane" (https://stepmodifications.org/wiki/Guide:Mod_Organizer), so the left pane runs low to high and the file high to low; (3) file check: `StarUI Inventory - Free Lanes Compatibility Patch` (line 95) ships the same 6 `Interface\*.swf` files as `StarUI Inventory` (line 97), and `StarUI HUD - Free Lanes Compatibility Patch` (line 91) the same `Interface\*.gfx` as `StarUI HUD` (line 94): a compatibility patch is meant to win, and it is listed first. **Still open: the M2.O4 UI check** (MO2's own view of one of those files, which mod it shows as providing it). |
| `*Unmanaged: X` | An entry for content MO2 does not manage: DLC and Creations that live in the game's Data dir. `X` is the plugin stem. | Default profile lines 100-105: `BlueprintShips-SFBGS050`, `SFBGS004`, `SFBGS00D`, `SFBGS047`, `SFBGS050`, `ShatteredSpace`; each matches a `.esm` in the game Data dir, none has a folder under `mods\`. |
| `plugins.txt` | `#` comment header; `*Name.esm` = enabled, a line without `*` = disabled. Lists only plugins the user manages: none of the `Starfield.ccc` plugins and none of the unmanaged ones. | Default profile: 70 lines, all `*`; exactly the 70 plugin files at the root of the 98 enabled mod folders. Disabled lines **not observed** (fixture-tested). |
| `loadorder.txt` | Every plugin in load order, official ones included. | Default profile: 84 lines = 8 `Starfield.ccc` + 6 unmanaged + 70 `plugins.txt`. nolvus.net (above): "ordered as load order in the MO2 right panel". Whether MO2 also lists *inactive* plugins here is **not observed** (every plugin of this profile is active). |
| `lockedorder.txt` | Locked load-order positions. | Default profile: header comment only, so the entry format is **not observed**; a non-empty file produces a warning and is not applied. |
| `Starfield.ccc` | The game's always-loaded official plugins, in order. | Default profile: the 8 plugins Ortham lists as hardcoded (below), in loadorder.txt's order. Ortham: plugins in `Starfield.ccc` "load as active in specified order". |
| `archives.txt` | Archive file names, one per line. | Default profile: 136 lines (`Starfield - Animations.ba2`, ...). Read and listed; not yet used (string tables, M2.D7). |
| `overwrite\` | Highest priority of all: beats every mod. | STEP guide (above): Overwrite sits at the bottom of the left pane. Default profile: holds saves backups and photos only, no plugin. |
| `ModOrganizer.ini` | `[General] gamePath=@ByteArray(<path>)` with `\\` for `\`; the game Data dir is `<gamePath>\Data`; `mods\`, `overwrite\`, `profiles\` sit next to the ini (portable instance). | Default instance: `gamePath=@ByteArray(C:\\Program Files (x86)\\Steam\\steamapps\\common\\Starfield)`; no `[Settings]` directory keys. `base_directory` / `mod_directory` / `overwrite_directory` (with `%BASE_DIR%`) are honoured if present but **not observed**. |
| `meta.ini` at a mod's root | MO2's bookkeeping, not game data: skipped by the overlay. | Every mod folder has one (`mods\Resupply - ...\meta.ini`). **Assumption** `mo2-meta-ini-hidden`. |

Text files are read as UTF-8 with a BOM stripped and invalid bytes replaced; `\n` and `\r\n`
both end lines. The owner's files are LF-only, without BOM.

### Overlay

A path relative to Data resolves to the first provider that has it: `overwrite\`, then
enabled mods from the highest priority down, then the game Data dir. Matching is ASCII
case-insensitive and `\` equals `/`. Disabled mods, separators and unmanaged entries
provide nothing; an enabled mod without a folder is reported (`missing_mod_dirs`).
`crates/io` fixtures cover priority, overwrite, disabled mods, separators, missing folders,
case clashes, BOM and CRLF.

## Active plugins

A plugin is active when the first of these rules applies (`kitcraft_io::Active`):

| Rule (label) | Evidence |
| ------------ | -------- |
| `Starfield.ccc` | Listed in the profile's `Starfield.ccc` (Ortham, below). |
| `hardcoded-official` (**assumption**) | One of `Starfield.esm`, `Constellation.esm`, `OldMars.esm`, `BlueprintShips-Starfield.esm`, `SFBGS003.esm`, `SFBGS006.esm`, `SFBGS007.esm`, `SFBGS008.esm` (Ortham 2024: "hardcoded to always be active"), plus `SFBGS00D.esm`, `SFBGS047.esm`, `SFBGS050.esm` (LOOT 0.29.0, 2026-04-11: new hardcoded official masters, https://github.com/loot/loot/blob/master/docs/app/changelog.rst), when the file exists. The list changes with game builds. |
| `plugins.txt` | A `*` line. |
| `mo2-unmanaged:<entry>` (**assumption**) | The plugin of a `*Unmanaged: X` modlist entry. Evidence it is loaded: `ShatteredSpace.esm` is in neither `plugins.txt` nor `Starfield.ccc` nor the LOOT list, yet it is a master of the active `SVF-ShatteredSpace-Patch.esm`, so the game must load it. How the game learns about it (its own Creations list) is **not observed**. |
| `blueprint-companion-of:<base>` | `BlueprintShips-<base>.esm` when `<base>` is active (LOOT 0.29.0: BlueprintShips plugins "displayed as active when there is an active plugin that matches the BlueprintShips plugin's suffix"; 0.29.1: they are no longer written to `plugins.txt`). In the Default profile both companions are already active by other rules (`BlueprintShips-Starfield.esm` through `Starfield.ccc`, `BlueprintShips-SFBGS050.esm` through its unmanaged entry), so the rule is fixture-tested only. |

Order: `loadorder.txt`; an active plugin it does not list is appended with a warning.
Where blueprint masters really load is **open**: LOOT 0.29.0 writes their timestamps "to
match their load order" because the game drops them from `plugins.txt`, which suggests the
game orders them by timestamp. KitCraft uses the `loadorder.txt` position.

On the Default profile (`crates/io/tests/default_profile.rs`, counts computed from the files):
84 plugins, all active: 8 `Starfield.ccc`, 3 hardcoded, 70 `plugins.txt`, 3 unmanaged
(`SFBGS004.esm`, `ShatteredSpace.esm`, `BlueprintShips-SFBGS050.esm`); every one resolves
to an existing file (70 from mod folders, 14 from the game Data dir), and the active list in
order equals `loadorder.txt`. The profile files' sizes and mtimes are unchanged by the run.

## Plugin flags and scale

TES4 record flags (`xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e`,
the `TES4` flag list, relayed by `docs/research/starfield-dossier-2026-10-14.md` section 2):
Master 0x001, Optimized 0x010, Localized 0x080, Small 0x100, Update 0x200, Medium 0x400,
Blueprint 0x800. (libloadorder issue #97 once said 0x10000 for medium; xEdit and Ortham's
later article say 0x400, and so does the corpus: SFBGS003/006 carry 0x400.)

**Scale** (full, medium, small) comes from the 0x100 and 0x400 bits; master, update and
blueprint are separate flags, not scales (`kitcraft_doc::flags`). Both scale bits at once has
no known meaning: resolution through such a plugin is refused. Corpus (`docs/corpus.md`):
0x100 on 6 files, 0x400 on 2, 0x800 on both BlueprintShips files, 0x200 on none.

Largest own object ID per file (`crates/doc/tests/corpus.rs`, ignored test, 2026-10-14):

| Plugin | Flags | Scale | Own records | Largest own object ID |
| ------ | ----- | ----- | ----------- | --------------------- |
| Starfield.esm | 0x81 | full | 3,829,246 | 0x3FFD59 |
| Constellation.esm | 0x181 | small | 56 | 0x4A |
| OldMars.esm | 0x181 | small | 38 | 0x29 |
| BlueprintShips-Starfield.esm | 0x881 | full | 1,502,975 | 0x2E6292 |
| SFBGS007.esm | 0x181 | small | 62 | 0x893 |
| SFBGS008.esm | 0x181 | small | 146 | 0x879 |
| SFBGS006.esm | 0x481 | medium | 60,890 | 0xF6A1 |
| SFBGS003.esm | 0x481 | medium | 64,384 | 0xFFDF |
| ShatteredSpace.esm | 0x81 | full | 1,029,437 | 0x1764C2 |
| SFBGS004.esm | 0x181 | small | 871 | 0xEE4 |
| SFBGS00D.esm | 0x81 | full | 434,663 | 0xAF140 |
| SFBGS050.esm | 0x81 | full | 478,881 | 0xAF5C5 |
| BlueprintShips-SFBGS050.esm | 0x881 | full | 79,974 | 0xDD1A8 |
| SFBGS047.esm | 0x181 | small | 40 | 0x2A |

Every small file stays within 12 bits and every medium file within 16, as the LOOT guide's
limits say (small: 4,095 records each; medium: 65,535;
https://loot.github.io/docs/help/changing-plugin-types-in-starfield/).

## FormIDs (M2.D3)

A FormID stored in a file is **raw**: it means nothing without the file's masters *and
their scales*. A resolved identity is `GlobalForm {plugin, object_id}` (the defining
plugin's load-order index and the object ID); results print it as `Plugin.esm:XXXXXX`.

Raw encoding in a file with masters `M0..Mn-1` (Ortham, "Load order in Starfield", 2024,
worked example with a full, a small and a medium master,
https://blog.ortham.net/posts/2024-06-28-load-order-in-starfield/):

| High byte | Class | Index | Object ID |
| --------- | ----- | ----- | --------- |
| `FE` | small | bits 12-23 | 12 bits |
| `FD` | medium | bits 16-23 | 16 bits |
| other | full | bits 24-31 | 24 bits |

The index counts **only the file's masters of that class**, in master-list order; the
index equal to that count, in the file's own class, is the file itself. The file does not
say what scale its masters are, so resolution loads every master's header first; a
missing master makes every FormID of the file unresolvable (a typed error,
`ResolveError::MissingMaster`), never a guess.

Corpus confirmation (`crates/doc/tests/corpus.rs`, `corpus:` source):

- Own records of small files are stored `FE<n>xxx` with `n` = number of small masters:
  Constellation, OldMars, SFBGS004/007/008 use `FE000`, SFBGS047 (masters starfield full,
  sfbgs004 small, sfbgs00d full) uses `FE001`. Medium files store `FD00xxxx`. Full files use
  the master count as usual (BlueprintShips-SFBGS050: `03`).
- SFBGS047's two overrides are stored `01xxxxxx` and exist in `sfbgs00d.esm`, its second
  *full* master but third master overall: the full index skips the small master. Under the
  plain "index into the master list" reading they would point at `sfbgs004.esm`.
- All 14 official files, loaded in the Default profile's order, resolve every master and
  every record header FormID. Of 8,425 overrides, all but 15 name a record that exists in
  the master; the 15 are `SFBGS050.esm` records addressed to `SFBGS00D.esm` objects that
  file does not contain (5 REFR, 9 NAVM, 1 PROJ).

### Own records stored full-style (assumption `full-style-self`)

Ortham's article also gives a medium plugin whose new record is stored `02051808` and
appears in game as `FD011808`: a small or medium file may store its *own* records in the
full class, with the index equal to its number of full masters. No official file does
this (`crates/doc/tests/corpus.rs` asserts zero), but on the owner's Default profile 27
third-party small or medium plugins do (for example `Resupply.esm` stores `01000CA6` with
one full master; `Hatch-22.esm`, masters `Starfield.esm` (full) and `SFBGS006.esm`
(medium), stores its own records `01xxxxxx`, so the index counts full masters only).
KitCraft resolves these to the file itself, counts them per plugin
(`full_style_self_records` in `load`) and names the plugins in a limitation. Third-party
files are the user's load order here, not evidence for the rule (M2.O5); the rule's source
is Ortham's example.

Still unresolved on the Default profile: the 4 records of `HG_ShipCargoHoldAtHome.esm`,
which has one master and stores its records `02xxxxxx`. They stay typed errors (left out of
overrides and queries, named in `load`'s limitations).

FormIDs inside subrecord payloads follow the same encoding but are only reachable once
typed layouts say where they are (M2.A3, M2.B3); until then `diff` and `conflicts` compare
payload bytes as stored and say so.

### Runtime IDs (M2.D3 c)

**Assumption** `runtime-slots-by-scale` (Ortham 2024; LOOT guide): active plugins take
slots in load order per scale: full plugins `00`..`FC` (runtime ID `SSxxxxxx`), medium
plugins `FD00`..`FDFF` (`FDSSxxxx`), small plugins `FE000`..`FEFFF` (`FESSSxxx`). Update
plugins (0x200) take full slots (Ortham: since game version 1.12.30.0); blueprint plugins
(0x800) are treated as full because they set neither scale bit. **Not observed** in the
game console or the CK on this machine (M2.O4); results that show a runtime ID say it is
assumed. Our scripts README says the in-game high byte is "the plugin's load-order index",
which matches the full-slot case.

## Index cache (M2.D5)

`%LOCALAPPDATA%\kitcraft\cache\<fnv64(path)>.kcidx`, or `KITCRAFT_CACHE_DIR`; never in the
game Data dir or a profile. An entry stores the absolute path, file size, mtime (ns),
format version and an FNV-1a checksum; any mismatch, truncation or corruption rebuilds the
entry. Format: `crates/doc/src/cache.rs`.

Timings (release, this machine):

| Load | Cold (no cache) | Warm |
| ---- | --------------- | ---- |
| 14 official files (`crates/doc/tests/corpus.rs`) | 6.8 s | 0.65 s |
| Default profile, 84 plugins, 7,739,193 records (`kit load --profile`, wall clock) | 6.0 s | 0.68 s |

On the warm Default profile (release, `kit` wall clock including the load): `query --type
WEAP --where "FULL ~ 'Old Earth Hunting Rifle'"` about 1 s (541 candidates; the string tables
of every plugin with a WEAP are read on first use), `inspect
Starfield.esm:001116` 0.8 s, `conflicts --count-classes` (15,586 overridden forms, 15,893
override records: 8,206 deleted, 6,031 differs, 1,641 identical to the previous version,
15 first versions without a base record) 2.8 s.

## String tables (M2.D7)

A localized plugin's lstrings resolve through `strings/<stem>_<language>.{strings,
dlstrings,ilstrings}` (`kitcraft_doc::strings`): a loose file under `Strings\` wins (through
the MO2 overlay: overwrite, mods by priority, then Data), else the plugin's `<stem> - *.ba2`
archives in the overlay or Data, Localization archives first, then Main (placement per
lane G's corpus survey in `plan/STATUS.md`; archive names from the official files). The ID
is looked up in every table found. Checked: `Starfield.esm` WEAP `21BBCD`
(`RussianHuntingRifle`) FULL `0x0002E469` resolves to "Old Earth Hunting Rifle" in
`Starfield - Localization.ba2:strings/starfield_en.strings`
(`crates/engine/tests/load_order_corpus.rs`). Not settled: string ID 0 occurs in 14 official
GMST string settings and is in no table; it stays unknown rather than being read as an empty
string.