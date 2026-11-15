# Research: what modders criticise in the Creation Kit

Helper report (one research subagent, 2026-11-14), lightly edited. Scope: user-visible
behaviour, forum threads, articles and patch feature lists only. No tool source code was
read, no record layouts or format details were collected, and nothing here is a fact about
Starfield data (AGENTS.md "Clean room").

**Evidence caveat (the helper's own).** Most items come from search-result summaries of
forum threads and articles, not full reads. Nexus, Steam, UESP and several news sites
blocked or truncated direct fetches, and Reddit was effectively unreachable. "Frequency"
means within what was sampled. Items marked *unverified* are general knowledge with no
source found. Treat the ranking as judgement, not measurement.

## 1. Stability and crashes

Starfield CK:

- Crashes when opening a plugin first made in xEdit (suspected unrecognised dependency).
  Once. forums.nexusmods.com/topic/13493071
- First-run crash or failure to launch, worked around by deleting a saved-settings file.
  Several times. forums.nexusmods.com/topic/13494407
- Fails on the Game Pass install (missing Steam DLL). Several times.
  forums.nexusmods.com/topic/13500036
- Crash on save under Linux/Proton. Once.
  steamcommunity.com/app/2722710/discussions/0/4414173015254695073
- In-game crash that appears only after navmeshing an exterior cell. Once.
  forums.nexusmods.com/topic/13498237

Predecessors:

- "Crashes every 5 to 15 minutes; save constantly" is pervasive across years of Skyrim
  threads. forums.nexusmods.com/topic/5005390, /7215246, /8010808
- The CK reports a successful save but the plugin file is gone. Once.
  forums.nexusmods.com/topic/13305246
- No reliable autosave or recovery: *unverified*.

## 2. Load times and performance

- About 4 to 5 minutes to start and become responsive after loading the base master.
  Several times. fz.se/forum/post/4521137, Steam CK discussions.
- 10 to 17 GB of RAM with only the base master loaded; 32 GB advised. Several times.
  steamcommunity.com/app/2722710/discussions/0/4414173015254695073
- "Fast file loading" is a headline feature of the community patch (CKPE), which implies
  stock loading is slow. github.com/Perchik71/Creation-Kit-Platform-Extended/wiki
- Save times balloon with many navmeshes. Once.

## 3. Workflow friction

- The CK creates identical-to-master records and wild edits without the author knowing, so
  cleaning in xEdit is mandatory. Pervasive (predecessors; the mechanism is assumed to carry
  over). tes5edit.github.io/docs/7-mod-cleaning-and-error-checking.html,
  nexusmods.com/skyrimspecialedition/articles/6568
- Every loaded plugin becomes a master of the active file, so master management needs care
  or external tools. Several times. modding.wiki/en/starfield/developers/tutorials/ESMtoESP
- Small/medium/full master flags are confusing; the wiki's fix for a wrongly flagged plugin
  is hex-editing the header or remaking the plugin. Several times. Same page.
- Light and full plugins behaved as separate load orders, breaking references for some
  record types (2023, before the CK; current status not confirmed). nexusmods.com/news/14888
- Load-time warning spam with "Yes to all" as standard advice (Skyrim era). Several times.
  forums.nexusmods.com/topic/555018
- Stock CK lacks filter-by-active-forms, cell filtering and Data-dialog filtering, all of
  which CKPE adds.
- Modal dialogs and no multi-record edit: *unverified*; no direct thread found.

## 4. Missing or broken Starfield features

- Materials: the Material Editor cannot create a new material in one configuration; the CK
  misbehaves with unpacked materials; third-party exporters break on game updates; a
  retexture looks right in the CK and black in game. Several times.
  forums.nexusmods.com/topic/13492017, /13493339, /13492548; nexusmods.com/starfield/mods/7830
- Planets and biomes: the workflow needs externally extracted biome-map data and
  undocumented steps. Several times. nexusmods.com/starfield/articles/643,
  forums.nexusmods.com/topic/13491705
- Papyrus: flags file and source folders are not set up out of the box, so compiles fail
  until fixed by hand. Several times.
  steamcommunity.com/app/2722710/discussions/0/4509877188788079155,
  forums.nexusmods.com/topic/13488656
- No official script source for Shattered Space. Once. nexusmods.com/starfield/mods/10320
- Voice and lip sync: lip generation failures; voice files in two formats; paths with no
  spaces; voice type must match body. Several times. facefx.com/node/4600,
  dev.starfieldwiki.net/wiki/Starfield_Mod:Audio_File_Conversion
- Creations and Xbox: 2 GB per mod, 100 GB total, no loose files, no script-extender mods.
  Several times. purexbox.com (2024-07 and 2026-01 articles).
- Ship builder data, facegen, animation tooling, particle editor: little direct evidence.

## 5. Documentation and learning curve

- No public documentation at launch; modders were pointed at the Skyrim and Fallout 4
  wikis. Pervasive. nexusmods.com/news/14993, forums.nexusmods.com/topic/13490609
- Community wikis are thin: the UESP-family Starfield CK page has empty placeholder
  sections. starfieldwiki.net/wiki/Starfield_Mod:Starfield_Creation_Kit
- Video playlists filled the gap.

## 6. Version control and collaboration

- Built-in Version Control is off by default and needs network shares, ini edits and
  per-machine user entries. falloutck.uesp.net/wiki/Version_Control
- Teams avoid it: project discipline, a merge queue or passing the file around, with xEdit
  to resolve conflicts before merging. Several times. ck.uesp.net/wiki/Version_control,
  forums.nexusmods.com/topic/4733045, /2057774, /624893
- Check-in fails on large masters. Once. forums.nexusmods.com/topic/817974
- Not found: statements from total-conversion teams, and anything Starfield-specific.

## 7. UI and UX

- Fallout 4 CK dialogs are fixed-size and are cut off on some displays or DPI scales.
  Several times. forums.nexusmods.com/topic/11003798, /11123038, /4880070, /4628475
- Dark theme and Unicode plugin names exist only through CKPE (predecessors).
- The Starfield CK does have dockable tool windows and saved layouts, a real improvement.
- Undo reliability, multi-monitor and shortcut complaints: no direct evidence; *unverified*.

## 8. Community fixes that prove demand

- CKPE (successor to the SSE CreationKit Fixes) lists: fast file loading, dark theme, UI
  fixes, warnings redirected to a log, an error blacklist, Unicode plugin names, creating
  masters and opening them as plugins, loading plugins as masters, active-form and cell
  filters, Data-dialog filtering.
- A user reported fewer than 10 crashes in over 100 hours after installing it.
- **CKPE's wiki lists no Starfield CK version as "Active"** while its README claims support
  from 1.15.222.0. Starfield coverage looks patchy.
- xEdit is still used for conflict detection, cleaning, cross-plugin comparison, bulk
  scripts, flag and master fixes; its own docs discourage it for extensive content creation.

## 9. Scene health, 2025 to 2026

- The Community Patch went unmaintained from September 2024; in May 2025 its lead cited
  replayability, paid mods and newer games. **Tools were not named as a reason.**
  pcgamer.com, videogamer.com, opencritic.com/news/16269
- Outlets called the scene "almost dead" relative to other Bethesda games; another countered
  that it is still active. tech4gamers.com, dexerto.fr
- A 2023 complaint said modding was not a design consideration and that xEdit support took
  over 400 hours. gfinityesports.com
- Official patches continue (May 2025: a plugin-load crash fix and a 2 GB upload cap). A
  reported September 2026 CK update is snippet-only.
- No 2026 modder-sentiment survey found.

## Top 12, with what a third-party editor could do

| # | Complaint | Frequency | Addressable by an editor that reads and writes plugins itself |
| - | --------- | --------- | ------------------------------------------------------------- |
| 1 | Crashes and lost work | pervasive | Yes: own stability, autosave, journaling |
| 2 | No or thin documentation | pervasive | Yes for fields (inline help from the schema); not for engine behaviour it does not know |
| 3 | Dirty edits, identical-to-master records, wild edits | pervasive | Yes, strongly: write only changed records, diff before save |
| 4 | Slow load, high RAM | several | Yes: lazy or indexed loading |
| 5 | Master and flag handling (small, medium, full) | several | Yes: plugin-level logic with validation |
| 6 | xEdit needed for conflicts and clean-up | pervasive | Yes: a built-in conflict view |
| 7 | Unusable version control for teams | several | Yes: text-diffable export and merge are plugin-side |
| 8 | Materials workflow | several | Partly: authoring possible, in-game parity not guaranteed |
| 9 | Papyrus setup, missing sources | several | Partly: verify and set up the environment; cannot supply sources |
| 10 | Planet and biome tooling | several | Partly: records yes, terrain preview hard |
| 11 | Fixed-size dialogs, DPI, theme | several (predecessors) | Yes: a new UI |
| 12 | Creations and Xbox limits, upload flow | several | No (platform policy); pre-upload checks only |

Voice and lip generation and navmesh are probably not addressable without the official
editor's bundled middleware. That is the helper's inference, not a sourced finding.

## Sources

- https://github.com/Perchik71/Creation-Kit-Platform-Extended (README and wiki front page only)
- https://www.nexusmods.com/news/14993 and /news/14888 (snippets; fetch blocked)
- https://www.nexusmods.com/skyrimspecialedition/mods/20061 and /mods/71371 (snippets)
- https://www.nexusmods.com/skyrimspecialedition/articles/6568
- https://www.nexusmods.com/starfield/articles/643, /mods/7830, /mods/10320 (snippets)
- https://forums.nexusmods.com/topic/ followed by 13493071, 13494407, 13500036, 13498237,
  13490609, 13492017, 13493339, 13492548, 13491705, 13488656, 13305246, 5005390, 7215246,
  8010808, 11081428, 555018, 11003798, 11123038, 4880070, 4628475, 4733045, 2057774, 624893,
  817974 (snippets)
- https://steamcommunity.com/app/2722710/discussions/0/4414173015254695073 and
  /4509877188788079155 (snippets)
- https://www.fz.se/forum/post/4521137/anmal (snippet)
- https://starfieldwiki.net/wiki/Starfield_Mod:Starfield_Creation_Kit (fetched)
- https://dev.starfieldwiki.net/wiki/Starfield_Mod:Audio_File_Conversion
- https://modding.wiki/en/starfield/developers/tutorials/ESMtoESP
- https://tes5edit.github.io/docs/5-conflict-detection-and-resolution.html and
  /7-mod-cleaning-and-error-checking.html
- https://falloutck.uesp.net/wiki/Version_Control ; https://ck.uesp.net/wiki/Version_control
- https://facefx.com/node/4600
- https://www.purexbox.com/news/2026/01/star-wars-genesis-creator-explains-why-viral-starfield-mod-is-never-coming-to-xbox (fetched)
- https://purexbox.com/news/2024/07/bethesda-makes-changes-to-starfield-mods-following-negative-effects-on-xbox
- https://www.pcgamer.com/games/rpg/even-starfields-community-patch-modders-are-growing-disenchanted-with-the-sci-fi-rpg-as-volunteers-depart-in-droves-if-nobody-comes-forward-we-may-have-to-retire-the-project
- https://www.pcgamer.com/heres-what-skyrim-and-fallout-modders-want-to-see-in-starfield-mods
- https://www.videogamer.com/news/biggest-starfield-modders-leaving-bethesda-sci-fi-rpg-behind
- https://opencritic.com/news/16269/starfield-community-patch-team-struggling-to-find-volunteers-as-modders-lose-interest
- https://www.gfinityesports.com/news/starfield-modders-feel-abandoned-by-bethesda-and-bethesda-agrees
- https://tech4gamers.com/starfield-mod-community-pretty-much-dead
- https://www.dexerto.fr/starfield/moddeurs-starfield-affirment-pas-abandonne-jeu-1534167/
- https://www.techdirt.com/2024/06/20/bethesda-reignites-the-paid-mods-controversey-with-starfield-creation-kit/
- https://www.pcgamesn.com/starfield/creation-kit-mods
- https://www.gamespark.jp/article/2025/05/23/152945.html
- https://news.instant-gaming.com/en/articles/6866-the-creation-kit-update-for-starfield-is-now-available (snippet)
