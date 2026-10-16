# Load order

How KitKraft turns an MO2 profile into an ordered list of plugins, and how it resolves
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
| `ModOrganizer.ini` | `[General] gamePath=@ByteArray(<path>)` with `\\` for `\`; the game Data dir is `<gamePath>\Data`; `mods\`, `overwrite\`, `profiles\` sit next to the ini (portable instance). | Default instance: `gamePath=@ByteArray(C:\\Program Files (x86)\\Steam\\steamapps\\common\\Starfield)`; no `[Settings]` directory keys. `base_directory` / `mod_directory` / `profiles_directory` / `overwrite_directory` / `download_directory` (with `%BASE_DIR%`) are honoured if present but **not observed**; an empty value counts as absent and a relative one is taken against the instance root. One reader, `kitkraft_io::Instance` (read-only; also `selected_profile` and `gameName`, and which folders are links or junctions), serves the profile reader and `deploy`. |
| `meta.ini` at a mod's root | MO2's bookkeeping, not game data: skipped by the overlay. | Every mod folder has one (`mods\Resupply - ...\meta.ini`). **Assumption** `mo2-meta-ini-hidden`. |
| `ModOrganizer.ini` `[customExecutables]` | MO2's executable list: `size`, then `<n>\title`, `<n>\binary`, `<n>\arguments`, `<n>\workingDirectory` per entry. The title is what an MO2 command line names (below). | Owner's instance, 2026-10-16 (read only): 5 entries; `1\title=SFSE` (`sfse_loader.exe`), `2` `Starfield`, `3` `Explore Virtual Folder`, `4` `CreationKit`, `5` `Creation Kit`. Grade A for the file content. |
| `settings.ini` (profile) | `[General] LocalSaves` and `LocalSettings`: whether the profile keeps its own saves (`saves\`) and its own game INIs. The session controller requires both `true` (`docs/game-sessions.md`). | `Kitkraft Dev`, 2026-10-16: `LocalSaves=true`, `LocalSettings=false`. With `false`, the profile's `StarfieldCustom.ini` is not the one the game reads; which file the game does read under MO2 is **not observed** (M4.T1 (h)). |

### Starting an executable on a profile (M4.T1 (a))

The command line that makes MO2 start one named executable on one named profile is
**not observed**. `ModOrganizer.exe --help` (MO2 2.5.2 revision 9c130cbf, from
`logs\mo_interface.log`) was not run on 2026-10-16. The owner's `ModOrganizer.exe` was
running then, and ADR 0010 section 1 treats a second MO2 on the instance as unsafe. The
controller's candidate is `ModOrganizer.exe -p "<profile>" "moshortcut://:<title>"`, grade D
(`crates/game/profiles/starfield.toml`, `[launch]`). It has no `evidence`, so no session
can launch until the help text is recorded: ledger entry `game-20261016-T1`, row (a).

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

A plugin is active when the first of these rules applies (`kitkraft_io::Active`):

| Rule (label) | Evidence |
| ------------ | -------- |
| `Starfield.ccc` | Listed in the profile's `Starfield.ccc` (Ortham, below). |
| `hardcoded-official` (**assumption**) | One of `Starfield.esm`, `Constellation.esm`, `OldMars.esm`, `BlueprintShips-Starfield.esm`, `SFBGS003.esm`, `SFBGS006.esm`, `SFBGS007.esm`, `SFBGS008.esm` (Ortham 2024: "hardcoded to always be active"), plus `SFBGS00D.esm`, `SFBGS047.esm`, `SFBGS050.esm` (LOOT 0.29.0, 2026-04-11: new hardcoded official masters, https://github.com/loot/loot/blob/master/docs/app/changelog.rst), plus `ShatteredSpace.esm` and `SFBGS004.esm` (the owner's saves, below: the `KitKraft Dev` profile's `plugins.txt` lists all five of `ShatteredSpace`, `SFBGS004`, `SFBGS00D`, `SFBGS047`, `SFBGS050` without `*`, and every save of that profile lists them as loaded, game 1.16.244.0), when the file exists. The list changes with game builds. |
| `plugins.txt` | A `*` line. |
| `mo2-unmanaged:<entry>` (**assumption**) | The plugin of a `*Unmanaged: X` modlist entry. First evidence it is loaded: `ShatteredSpace.esm` is in neither `plugins.txt` (as `*`) nor `Starfield.ccc` nor the LOOT list, yet it is a master of the active `SVF-ShatteredSpace-Patch.esm`, so the game must load it; the saves confirm it, and it is now `hardcoded-official`. On both owner profiles only `BlueprintShips-SFBGS050.esm` is still active by this rule (it is also the companion of `SFBGS050`). How the game learns about such plugins (its own Creations list) is **not observed**. |
| `blueprint-companion-of:<base>` | `BlueprintShips-<base>.esm` when `<base>` is active (LOOT 0.29.0: BlueprintShips plugins "displayed as active when there is an active plugin that matches the BlueprintShips plugin's suffix"; 0.29.1: they are no longer written to `plugins.txt`). In the Default profile both companions are already active by other rules (`BlueprintShips-Starfield.esm` through `Starfield.ccc`, `BlueprintShips-SFBGS050.esm` through its unmanaged entry), so the rule is fixture-tested only. |

### Order (assumption `hardcoded-official-slots`)

1. The official plugins (`Starfield.ccc`, `hardcoded-official`, `mo2-unmanaged`, and
   blueprint companions of these) in the game's official order
   (`kitkraft_io::plan::OFFICIAL_ORDER`): `Starfield`, `Constellation`, `OldMars`,
   `BlueprintShips-Starfield`, `SFBGS004`, `SFBGS007`, `SFBGS008`, `SFBGS003`, `SFBGS006`,
   `ShatteredSpace`, `SFBGS00D`, `SFBGS050`, `BlueprintShips-SFBGS050`, `SFBGS047`.
   Official plugins the list does not name follow, in the ccc's order, then
   `loadorder.txt`'s.
2. Everything else in `loadorder.txt` order; an active plugin it does not list is appended
   with a warning.

Evidence (the game's own record, "Save headers" below; game 1.16.244.0, 2026-10-15): every
save of the owner's lists, per scale, the official plugins first, in this order: full
`Starfield`, `ShatteredSpace`, `SFBGS00D`, `SFBGS050`; small `Constellation`, `OldMars`,
`SFBGS004`, `SFBGS007`, `SFBGS008`, `SFBGS047`; medium `SFBGS003`, `SFBGS006`. Then the
user plugins of each scale in `loadorder.txt`'s order, then (full only) the two blueprint
plugins. So the game follows neither MO2's `loadorder.txt` (which lists `ShatteredSpace`,
`SFBGS004`, `SFBGS00D`, `SFBGS047`, `SFBGS050` after the managed plugins; on `Default`,
`stroudpremiumedition.esm` names `sfbgs00d.esm` and `SFBGS047.esm` as masters) nor the
`Starfield.ccc` order (`SFBGS006` before `SFBGS003`, `BlueprintShips-Starfield` before
`SFBGS007`). A save lists each scale apart, so **how the scales interleave is not
observed**; it matters only for which of two official plugins of different scales wins a
form both override, and runtime slots do not depend on it. The list therefore keeps the
previous order wherever the saves allow it: the ccc's, with `SFBGS004` moved before
`SFBGS007` and `SFBGS003` before `SFBGS006` (the two corrections the saves force), then the
other official plugins in the order the `Default` `loadorder.txt` lists them. Results over
a profile name the assumption and the plugins it moved from their `loadorder.txt`
position.

**Blueprint plugins** (TES4 flag 0x800) take the full runtime slots after every user
plugin ("Runtime IDs" below), but stay with the official plugins in this order: ten user
plugins on `Default` name `BlueprintShips-Starfield.esm` as a master, and which version of
a form the game uses when one of them overrides a blueprint record is **not observed**; a
master before its dependents is the safe reading (and keeps those ten free of ordering
errors).

Any master that still loads after a plugin naming it (`late_masters` in `load`) is an
**ordering error**, carried in the limitations of every result over that load order
(`load`, `inspect`, `query`, `conflicts`), since overrides and winners involving it may be
wrong. `diff` takes no load order.

Effect on the Default profile (`crates/io/tests/default_profile.rs`, ignored release test
`hardcoded_slots_effect_on_the_default_profile`, rerun 2026-10-15 with the save-corrected
order): in `loadorder.txt` order 1 master link points later (`stroudpremiumedition.esm`
before `SFBGS047.esm`); in this order none. 14 overridden forms change winner, every one
from a slotted official plugin (`SFBGS050` 11, `SFBGS00D` 2, `BlueprintShips-SFBGS050` 1)
to a managed mod that loads after it (`TIG_Shipyards` 4, `stroudpremiumedition` 3,
`MB_Scrappy` 3, `Resupply`, `outfit_variety_paradiso`, `NPC Outfit Changer`,
`Better_Living` 1 each). The two save-forced corrections inside the official block change
no winner on this profile. (2026-10-14, first version of the rule: 2 late links, 15
forms.)

On the Default profile (`crates/io/tests/default_profile.rs`, counts computed from the files):
84 plugins, all active: 8 `Starfield.ccc`, 5 hardcoded, 70 `plugins.txt`, 1 unmanaged
(`BlueprintShips-SFBGS050.esm`); every one resolves
to an existing file (70 from mod folders, 14 from the game Data dir), and the active list is
exactly `loadorder.txt`'s entries in the order above. The profile files' sizes and mtimes
are unchanged by the run.

## Plugin flags and scale

TES4 record flags (`xedit: wbDefinitionsSF1.pas@9fb016884bec138ea6c7b872cec831537d464c3e`,
the `TES4` flag list, relayed by `docs/research/starfield-dossier-2026-10-14.md` section 2):
Master 0x001, Optimized 0x010, Localized 0x080, Small 0x100, Update 0x200, Medium 0x400,
Blueprint 0x800. (libloadorder issue #97 once said 0x10000 for medium; xEdit and Ortham's
later article say 0x400, and so does the corpus: SFBGS003/006 carry 0x400.)

**Scale** (full, medium, small) comes from the 0x100 and 0x400 bits; master, update and
blueprint are separate flags, not scales (`kitkraft_doc::flags`). Both scale bits at once has
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
master that is unavailable makes every FormID of the file unresolvable, never a guess
(ADR 0004, owner approval rule 5). The errors are typed (`kitkraft_doc::ResolveError`):
`MissingMaster` (not in the load order), `MasterNotLoaded` (in the load order, but its file
failed to load), `PluginNotLoaded` (the file itself failed to load), `BadScale` (both scale
bits set), `IndexOutOfRange`, and `Null`.

**The null FormID.** A raw `00000000` is the null reference, not a form: it never resolves
(`ResolveError::Null`, never `Starfield.esm:000000`). A typed FormID member holding it
shows as `{"raw": "00000000", "form": null, "null": true}` in `inspect` and as the value
`null` in `where` (`KWDA == null`); a formref with object ID 0 does not parse.

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
KitKraft resolves these to the file itself, with the stored 24-bit object ID **masked to
the file's own scale** (`0xFFFF` medium, `0xFFF` small), as in Ortham's example, where
`02051808` appears as `FD011808`: object `1808`, the `05` dropped. So `01012345` in a medium
file with one full master is that file's object `2345` (`crates/doc/src/form.rs` and
`load_order.rs` tests). Every version says how it resolved (`resolved_by`: `direct`,
`assumed:full-style-self`, or `assumed:full-style-self-masked` when the mask dropped set
bits) in `inspect`, `query` and `conflicts`; `load` counts them per plugin
(`full_style_self_records`, `full_style_self_masked_records`) and lists the first ten
(`assumed_records`). Two records of one plugin that resolve to the same form (a masked ID
landing on another record, or a repeated FormID) are both kept, the later record last, and
reported (`duplicate_records`, and a limitation). Third-party files are the user's load
order here, not evidence for the rule (M2.O5); the rule's source is Ortham's example.
Default profile, 2026-10-14: 63,798 records of 27 plugins resolve this way, none needs the
mask, and no plugin has duplicates.

Still unresolved on the Default profile: the 4 records of `HG_ShipCargoHoldAtHome.esm`,
which has one master and stores its records `02xxxxxx`. They stay typed errors (left out of
overrides and queries, named in `load`'s limitations).

FormIDs inside subrecord payloads follow the same encoding but are only reachable once
typed layouts say where they are (M2.A3, M2.B3); until then `diff` and `conflicts` compare
payload bytes as stored and say so.

### Runtime IDs (M2.D3 c)

**Assumption** `runtime-slots-by-scale` (Ortham 2024; LOOT guide; the owner's saves):
active plugins take slots in load order per scale: full plugins `00`..`FC` (runtime ID
`SSxxxxxx`), medium plugins `FD00`..`FDFF` (`FDSSxxxx`), small plugins `FE000`..`FEFFF`
(`FESSSxxx`), **except that full plugins with the blueprint flag (0x800) take the full
slots after every other full plugin** (in load order among themselves). The slot class
comes from the **scale bits only** (0x100 small, 0x400 medium, else full;
`kitkraft_doc::form::runtime_slots`): the update flag (0x200) does not change it, and a
blueprint plugin with a scale bit keeps its small or medium count (no save shows one).
An earlier version of this page said update plugins take full slots (citing Ortham for
game version 1.12.30.0); the two readings differ only for an update plugin that also sets
a scale bit, which no corpus file and no Default-profile plugin does.

Evidence (game 1.16.244.0, 2026-10-15; "Save headers" below): the saves' lists give
`KitKraft Dev` full `00` Starfield, `01` ShatteredSpace, `02` SFBGS00D, `03` SFBGS050,
`04` KitKraftFirstPOI, `05` KitKraftGunOverhaul (once enabled), then the two blueprint
plugins; `Default` (`Save56`, 84 plugins) 41 full, 33 small, 10 medium, the blueprint
plugins at `27` and `28` after `EasyLockpick_by_Kor.esp` at `26`. With the order above
and this rule, `kit load --profile` reproduces every slot of both profiles' newest saves
(and of `Save56`): `differs_from_inferred` is empty. In game (M4.O2, 2026-10-15) the
owner reached the POI's crate with `player.moveto 04001000` on `KitKraft Dev`: one full
slot observed in the console, the one this rule and the save give. Before this
fix `load` put `BlueprintShips-Starfield` at `01`, so every user full plugin came out two
slots high (the POI at `06`; MO2 showed `02`).

Runtime IDs stay **inferred** unless a save confirms them: `load --profile <dir>
--from-save <path|newest>` reads the save's plugin lists and, when they name exactly the
load order's active plugins under the scales their headers give, adopts the save's slots
(each result then says `runtime_basis: confirmed-by-save` and names the save). A save of
another plugin set confirms nothing: `load` lists the plugins only in the load order, only
in the save, or under another scale, and the IDs stay inferred. `newest` is the newest
`.sfs` in `<profile>\saves` when the profile's `settings.ini` says `LocalSaves=true` (MO2's
profile-specific saves), else in `%USERPROFILE%\Documents\My Games\Starfield\Saves`; a
`sLocalSavePath` in the game's INIs is not read. Our scripts README says the in-game high
byte is "the plugin's load-order index", which matches the full-slot case.

`inspect` reads a bare hex `form` only as stored in a named `plugin`; a runtime ID needs
the explicit `runtime:` prefix (`runtime:FE00A801`), so a raw FormID is never taken for a
runtime one by accident.

### Save headers (corpus)

`kitkraft_io::save` reads only a save's plugin lists, read-only (nothing in `kitkraft-io`
opens a file for writing; a save is never written). Every fact here is a **corpus** fact
from the owner's saves, read from copies (game 1.16.244.0, 2026-10-15): `KitKraft Dev`
`Save1` (`000006`, vanilla), `Autosave1` (`000039`, the POI), `Autosave2` (`000051`) and
`Exitsave0` (`000052`, the POI and the gun overhaul), and `Default`'s `Save56` (`040127`,
84 plugins). No outside source was used; the ignored test `owner_saves` in
`crates/io/src/save.rs` re-reads a folder of copies (`KITKRAFT_SAVES`).

Container, little-endian:

| Offset | Size | Meaning | All five saves |
| ------ | ---- | ------- | -------------- |
| 0x00 | 4 | magic `BCPS` | yes |
| 0x04 | 4 | container version | 1 |
| 0x08 | 8 | offset of the chunk size table | 0x48 |
| 0x18 | 8 | offset of the first compressed chunk | 0x80 (12 chunks), 0x2C0 (`Save56`, 157 chunks) |
| 0x20 | 8 | total uncompressed size | 3,016,523 (`Autosave1`) to 41,106,480 (`Save56`) |
| 0x30 | 4 | uncompressed chunk size | 0x40000 |
| 0x38 | 8 | chunk alignment | 16 |
| 0x44 | 4 | compression tag | `ZIP ` |
| table | 4 per chunk | compressed size of each chunk; count = ceil(total / chunk size), then `padding\0` up to the data | yes |

Each chunk is one zlib stream (header `78`), the next one starting at the following
multiple of the alignment. The plugin lists sit in the first chunk in all five saves (the
reader inflates more chunks only if a list runs past one).

Decompressed data:

| Field | Meaning | All five saves |
| ----- | ------- | -------------- |
| 12 bytes | `SFS_SAVEGAME` | yes |
| u32 | size of the header that follows | 0x76..0x8C (it holds the player's name and location, which vary) |
| header | starts with a u32 | 27 |
| u8 | read as a form version, not interpreted | 153 |
| wstring x2 | the game version, twice | `1.16.244.0` |
| u32 | size of the plugin block | 261 (`Save1`) .. 4,657 (`Save56`) |
| u8 count + entries | full plugins, in slot order | 6, 7, 8, 8, 41 |
| u16 count + entries | small plugins, in `FE` slot order | 6, 6, 6, 6, 33 |
| u32 count + entries | medium plugins, in `FD` slot order | 2, 2, 2, 2, 10 |
| the rest of the block | not interpreted | 2 bytes, `01 00` |

A wstring is a u16 length and that many bytes, no terminator. An entry is a wstring (the
file name), a u8 0 or 1, and after a 1 a u32 (0), a wstring
(`18446744073709551615.undefined`) and a u8 (0); official plugins carry 0, every other
plugin 1. That the order within each list is the runtime slot order is read from the
layout (each list starts with the official plugins, `Starfield.esm` first) and is
**verified in game** (M4.O2, 2026-10-15, `plan/STATUS.md`): on `KitKraft Dev`
`player.moveto 04001000` reached the POI's crate, the fifth full entry of that profile's
saves; `02001000` had behaved as the exterior reference `SFBGS00D.esm:001000`, the third
(`docs/poi/first-poi-investigation.md`). The reader refuses
anything else (another magic or tag, a flag other than 0 or 1, lists that end past the
block, a chunk that runs past the file or inflates past the chunk size), with the offset.

## Deleted records

Record flag 0x20 marks a deleted record (UESP, Skyrim mod file format, a container fact).
`inspect` shows `deleted` on every version and `winner_deleted` (with "(deleted)" in the
summary and a limitation); `query` results carry `deleted`, and `where` never evaluates a
deleted version's payload: field paths and `has` are unknown there (with the reason),
while the index paths `@form`, `@plugin`, `@edid` and `@flags` still work (`@flags & 0x20`
finds deleted versions). Default profile: 8,206 of the 15,893 override records are deleted.

## Safe sources (M3.D1)

A command that copies a form's winning version (`set`, `build`) needs that winner to be the
one the game would use. `LoadOrder::source_check` (`crates/doc/src/source.rs`) splits the
load order's problems into those that can change the winner or the meaning of its FormIDs
(`blocking`) and the rest (`unrelated`):

- blocking: no version, a deleted winner, a plugin holding a version of the form that loads
  before one of its masters, a plugin after the winner that failed to load (its records are
  unknown), or one after the winner with unresolved records that could be this form (it is
  the defining plugin or names it as a master, and a stored object ID equals the form's
  under the 24-, 16- or 12-bit width);
- unrelated: everything else, including active plugins with no file (the game skips them
  too; a plugin that needs one as a master is judged by the unresolved-records rule).

The winner's own masters are always loaded and scaled: a version exists only when its
plugin's FormIDs resolve. `inspect` shows the verdict as `winner_source`. Default profile:
the rifle `Starfield.esm:0021BBCD` is safe; its only unrelated problem is the 4 unresolved
records of `HG_ShipCargoHoldAtHome.esm`.

## Index cache (M2.D5)

`%LOCALAPPDATA%\kitkraft\cache\<fnv64(path)>.kcidx`, or `KITKRAFT_CACHE_DIR`; never in the
game Data dir or a profile. Format: `crates/doc/src/cache.rs` (format version 3).

**Staleness rule** (exactly what is compared; anything else rebuilds the entry, never
trusts it):

1. the entry's FNV-1a checksum verifies, and its format version and the esm index format
   version match;
2. its stored absolute path equals the plugin's;
3. the plugin's current size and modification time (nanoseconds since the Unix epoch, as
   the file system reports them) equal the stored ones;
4. a content sample equals the stored one: FNV-1a over the file size and the first and
   last 64 KiB of the file (the first part holds the TES4 header: flags and masters).

A plugin whose modification time is unknown or 0 is never cached or read from the cache.
Not detected: an edit that keeps the size and the mtime and changes only bytes outside the
two sampled ends (tools that write plugins update the mtime). Tests: a new mtime, a new
size, and the same size and mtime with changed content each rebuild; mtime 0 is never
stored.

Timings and memory (release, this machine; peak working set sampled from the process):

| Load | Cold (no cache) | Warm |
| ---- | --------------- | ---- |
| 14 official files (`crates/doc/tests/corpus.rs`) | 6.8 s | 0.65 s |
| Default profile, 84 plugins, 7,739,193 records (`kit load --profile`, wall clock) | 4.7 s, peak working set 482 MB | 0.73 s, 376 MB |

The review of 2026-10-14 measured the same load at 478 MB cold and 375 MB warm peak working
set. Memory is dominated by the per-plugin indexes (32 bytes per record) and the override
table (16 bytes per record).

On the warm Default profile (release, `kit` wall clock including the load): `query --type
WEAP --where "FULL ~ 'Old Earth Hunting Rifle'"` about 1.7 s (1.0 s in process; 541
candidates; the string tables of every plugin with a WEAP are read on first use), `inspect
Starfield.esm:001116` 0.7 s, `conflicts --count-classes` (15,586 overridden forms, 15,893
override records: 8,206 deleted, 6,028 differs, 1,644 identical to the previous version,
15 first versions without a base record) 2.7 s. (Before `hardcoded-official-slots`: 6,031
differs and 1,641 identical; reordering changes which version is "previous".) With
field-level comparison (M3.D2; the profile then had 85 plugins): 15,587 forms, 15,894
override records: 8,206 deleted, 6,029 differs, 1,644 identical, 15 first versions, still
2.7 s warm.

## Reverse links (M4.D1)

`refs --incoming` reads a reverse index per plugin (`kitkraft_doc::backrefs`): for every
record of a type whose definition has a `formid` member, the raw values the generated
decoder's `form_ids()` yields (`00000000` and `FFFFFFFF` left out), as `(raw value, record
index)` pairs, plus the cell children groups (types 6, 8, 9) as record ranges with the
cell's and world's stored FormIDs. A query turns the target into the raw values each plugin
would store for it (its master class index; for a small or medium plugin's own forms also
the `full-style-self` spelling, and the masked spellings by a range scan) and looks them up
by binary search; nothing is resolved by guessing. The index is cached next to the record
index (`<hash>.kclnk`) under the same staleness rule, plus the format version, a
fingerprint of the builtin schema definitions (what is typed decides what is indexed) and
the record count; an entry that fails any check is rebuilt. Not searched, and counted in
the result: records of types with no definition (100 types, 241,228 records on the owner's
profile, most of them `RFGP`) and the subrecords the decoder keeps raw.

| Load order (release, `kit refs <form> --incoming --profile`, wall clock with the load) | Cold (record index warm) | Warm |
| --- | --- | --- |
| `KitKraft Dev` profile, 84 plugins (2026-10-15) | 18.9 s (index 18.1 s, 84 entries, 220 MB on disk) | 0.9 s (index 0.17 s) |
| The 14 official files (`crates/engine/tests/refs_corpus.rs`) | 19.0 s | - |

Warm, `refs WeaponTypeBallistic --incoming` (94 referrers) and `refs Starfield.esm:21BBCD
--depth 2` (357 links from 71 forms) take 0.9 s and 0.8 s wall clock on the same profile,
most of it the load. The vanilla `STAT` `NA_Hard_RetWallASupport_03` lists its two placed
references in world `Starfield.esm:01251B` with their cells.

## String tables (M2.D7)

A localized plugin's lstrings resolve through `strings/<stem>_<language>.{strings,
dlstrings,ilstrings}` (`kitkraft_doc::strings`): a loose file under `Strings\` wins (through
the MO2 overlay: overwrite, mods by priority, then Data), else the plugin's `<stem> - *.ba2`
archives in the overlay or Data, Localization archives first, then Main (placement per
lane G's corpus survey in `plan/STATUS.md`; archive names from the official files). The ID
is looked up in every table found. Checked: `Starfield.esm` WEAP `21BBCD`
(`RussianHuntingRifle`) FULL `0x0002E469` resolves to "Old Earth Hunting Rifle" in
`Starfield - Localization.ba2:strings/starfield_en.strings`
(`crates/engine/tests/load_order_corpus.rs`). Not settled: string ID 0 occurs in 14 official
GMST string settings and is in no table; it stays unknown rather than being read as an empty
string.