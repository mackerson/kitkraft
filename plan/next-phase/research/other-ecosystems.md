# Research: what tools in other ecosystems do well

Two helper reports (research subagents, 2026-11-14), lightly edited and merged. Part A is
modding toolchains for other games; part B is engine editors and adjacent authoring tools.
Scope: features and workflows from product pages, documentation, wikis and forum posts. No
tool source code was opened, no file-format or record-layout detail was collected, and
nothing about another Bethesda game is used as a fact about Starfield (AGENTS.md "Clean
room"). Spriggit and Synthesis appear at the level of their public workflow pages only.

**Evidence caveat (the helpers' own).** Most sources were read as search-result summaries;
four pages in part A were fetched in full and none in part B. "What users praise" rests on
documentation, guides, vendor blogs and single forum posts; no user survey was found for
any tool, and Reddit was not reachable. Rankings are judgement. `[U]` marks a claim with no
source consulted (general knowledge).

## Part A: modding toolchains for other games

### Bethesda community tools

| Tool | What users get | Take for KitKraft |
| ---- | -------------- | ----------------- |
| xEdit [1, 2] | Whole-load-order conflict view with a colour legend and a conflicts filter; compare two versions of a plugin and produce a delta patch; command-line auto-clean; user scripts | A "delta between two plugin versions" command; headless clean and check modes |
| Synthesis [3, 4] | Many small patchers run as one pipeline into one output plugin; re-run after any load-order change | Treat the build spec as a re-runnable pipeline stage that adapts to the current load order |
| Wrye Bash bashed patch [5] | Merges list-type records automatically; per-plugin tags declare which changes carry forward | Let authors declare merge intent as metadata a patch generator honours |
| LOOT [6] | Sorting; missing-requirement, incompatibility and cycle checks; community-maintained messages and dirty-plugin warnings | A shareable, community-editable rules list separate from the tool |
| Mod Organizer 2 [7] | Game folder untouched; profiles; a view of the final resolved file tree. Criticism: the shared overwrite folder confuses users | Generated output goes to a named per-build destination, never a shared dump (KitKraft already does this, ADR 0009) |
| zEdit / zMerge [8, partial] | Merging plugins to cut plugin count; guides report edge-case breakage | Merging needs a verification report, not only output |
| Plugin-to-text for git [9, docs only] | Plugin split into a folder of text files; CLI round trip both ways; pinned translator version | Byte-exact round trip plus one file per record gives reviewable diffs; pin the schema version in the project |
| Cathedral Assets Optimizer [8] | Per-game safe profiles; a dry run listing actions first | Dry run on every mutating command |
| DynDOLOD [8, thin] | Derived content generated from the installed load order | Derived-artefact generators re-runnable and kept apart from hand-authored data |
| Wabbajack, Nexus collections [10] | A small instruction file reproduces a whole setup without redistributing assets | A lockfile for the test environment, so game sessions are reproducible |

### Paradox

- **CWTools** [11]: validation and completion in the text editor driven by public rule
  files; per-diagnostic ignore settings; a CI action running the same checks on pull
  requests. Take: validation rules as data, suppressions, a CI action.
- **Error log and hot reload** [12, weak]: a debug launch mode with file watchers; modders
  rely on the error log and also call it noisy. Take: link game-log lines back to records.
- **Irony Mod Manager** [13]: conflicts per definition, not per file; re-flags only what
  changed after a mod updates; merges a collection. Take: incremental conflict re-check.

### Minecraft

- **KubeJS** [14]: per-scope reload commands, with honest documentation of what cannot be
  reloaded. Take: document which record kinds can be refreshed in a running game.
- **Misode generators** [15]: schema-driven browser forms that emit valid files for many
  game versions. Take: generate forms from the typed schemas.
- **packwiz** [16]: a pack described by small TOML files designed for git. Take: dependency
  pinning in the build spec.
- **MCreator, Blockbench** [17]: wizards that start from a vanilla example. Take: "new X
  from a vanilla template" scaffolds.
- **Data generators** [18]: code generates repetitive data files. Take: programmatic
  generation alongside declarative specs.

### Other frameworks

- **RimWorld patch operations** [19]: select and modify others' definitions, with
  conditional patches; community helpers that log the patched state. Take: selector-based
  patches plus a "show the result after patching" command.
- **Factorio** [20, 21]: a staged lifecycle; docs per game version; machine-readable API
  docs in a versioned format for tool builders. Take: publish the schema as versioned,
  machine-readable docs per game version.
- **OpenMW-CS** [22, 23]: everything is a sortable table; records flagged
  Base/Modified/Added/Deleted; project, session and instant filters; a verifier report
  table with jump-to-record. Take: a status column, saved filters, and the validation report
  as a navigable table.
- **Construction Set Extender, CKPE** [24, thin]: fixes and added tools for the official
  editors; feature pages were blocked, so specifics are [U]. Take: users value fixes to
  daily friction over headline features.
- **BG3 modders' multitool** [25]: index and search across unpacked game data; one-step
  packaging. Take: a full-text index over base-game data.
- **WolvenKit and its hot-reload tools** [26]: browse base and mod archives without
  extracting; a mod-only view; hot reload described as cutting iteration to seconds, with
  documented limits. Take: the strongest sourced evidence that iteration time matters.
- **Witcher 3 Script Merger** [27]: detects conflicts, auto-merges text, opens a three-way
  merge for overlaps, lists the unmergeable plainly. Take: three-way merge with an explicit
  "cannot merge, X wins" list.
- **Stardew Content Patcher** [28]: declarative patches with conditions and tokens; in-game
  `patch summary`; a format-version field with a migration guide. Take: conditions in the
  build spec and a "why did this apply or not" summary.
- **Arma Eden** [30], **Halo Forge** [31]: preview from the editor; reusable shared
  compositions and prefabs. Take: shareable compositions.

### Part A synthesis (ranked by evidence strength, the helper's judgement)

1. Fast iterate loop, partial hot reload with documented limits.
2. Record-level conflict view with merge assistance and explicit winners.
3. Text serialisation for git with a reproducible round trip.
4. Re-runnable patcher pipeline into one generated output.
5. Validation as public rule data, with suppressions and CI.
6. Non-destructive deploy and isolated generated output.
7. Declarative conditional patches with a "why applied" summary.
8. Versioned machine-readable schema docs.
9. Dry run on every mutating command.
10. Navigable validation report, status column, saved filters.
11. Diff two versions of a plugin and emit a delta patch.
12. Reproducible environment lockfile.
13. Searchable index of base-game data, mod-only view.
14. Templates, wizards and schema-driven forms.
15. Reusable prefabs and compositions.

Not found: detailed CSE and CKPE feature lists (pages blocked); OpenMW-CS undo and
multi-document design in current manuals; Paradox hot-reload specifics; the official BG3
toolkit and REDkit feature lists.

## Part B: engine editors and adjacent tools

### Unreal

- **One File Per Actor, World Partition**: each actor in its own file, so people editing
  different actors do not collide. Caveat: single actors still cannot be merged, and partial
  submits can leave dangling references. Take: file-per-record serialisation plus a
  dangling-reference check.
- **Data Validation plugin**: rulesets in C++, Blueprint or Python, runnable headless; it
  ships no rules, and third parties sell rule packs with severity thresholds. Take: ship a
  curated default ruleset with severity gates.
- **Reference Viewer, Size Map**: used for build-size triage; users complain it shows
  editor sizes and misses indirect references. Take: a "what does this plugin pull in" view
  measured on the final output.
- **Gauntlet, Automation Test Framework**: launch sessions against a built game and
  validate. Take: declarative multi-step scenarios for the game sessions KitKraft has.
- **UEFN**: revision history and revert inside the editor; pushing changes to a live
  session is reported unreliable. Take: built-in snapshots; make a live push fail loudly.
- [U] Data tables with CSV round trip, editor scripting, play-in-editor. Take: a grid view
  over many records with CSV round trip.

### Unity

- **Smart Merge**: semantic merge of scenes and prefabs; zero-tolerance teams still lock.
  Take: a record-aware three-way merge driver, interactive on a true conflict.
- **Prefab variants and overrides**: Take: show overrides as "this plugin changes these
  fields of that base record" (the override model made visible).
- **The AI assistant**: the first one was poorly received (opaque training data, refusals)
  and was replaced by one with a built-in MCP server. Take: an assistant must act on real
  project data and be transparent.

### Godot

- **Text scene format** designed for version control, with stable IDs. Take: canonical,
  deterministic text with stable IDs.
- **Remote scene tree**: inspect and change the running game. Take: a live record inspector
  over the game-session bridge.

### Level editors

- **TrenchBroom**: an issue browser with one-click fixes (thin source). Take: quick-fix
  actions on validation findings.
- **Tiled**: custom typed property classes, object templates, command-line export.
- **LDtk**: typed entity fields, rule-based auto-layers, and a "super simple export" tier.
  Take: a flat JSON or CSV export per record type for outside tools and agents.
- **Roblox Studio**: live co-editing; an assistant with undo for AI edits and a planning
  mode. Roblox claims 44% of its top 1,000 creators use the assistant or third-party MCP
  tools (vendor figure). Take: AI edits as plan, then apply, undoable as one transaction.

### Cross-cutting

- **Merge avoidance**: consensus that binaries cannot be merged; the answers are
  file-per-entity, text and locking. Take: text plus file-per-record makes locking
  unnecessary for most modders.
- **CI validation**: validation as an early gate; a deliberate warnings-as-errors policy;
  editor-side passes do not prove the packaged build. Take: a ready-made recipe of
  validate, build, in-game smoke test.
- **Search**: Blender's operator search runs any command and shows its menu; a paid add-on
  exists because it does not search data. Take: one palette over commands and records.
- **AI and MCP in game tools**: engine MCP servers are common, one official. Reported
  lessons (mostly vendor sources): discovery of project state matters more than tool count;
  failures are context starvation; per-tool enable and disable helps.
- **Narrative tools**: Ink is text first; Inky replays your choices to the same point after
  every recompile, with errors as you type. Take: a dialogue or quest previewer that
  replays to the same point after each edit.

### Part B synthesis (ranked by evidence strength, with the helper's size guess)

1. Text, file-per-record serialisation with stable IDs (M)
2. Headless validation with a shipped default ruleset and severity gates (S)
3. Reference viewer and dependency graph with dangling-reference detection (M)
4. Record-aware three-way merge driver (L)
5. CI recipe: validate, build, in-game smoke test (S)
6. Live inspection and editing of the running game (L to XL)
7. AI edits as plan, reviewable diff, one undoable transaction (M)
8. MCP discovery-first tools and per-tool toggles (S)
9. Command palette over commands and records (S to M)
10. Quick-fix actions on validation findings (M)
11. Typed templates with defaults, explicit override views (M)
12. Built-in snapshot history and revert (S to M)
13. Simple export tier and bulk grid or CSV editing (S to M)
14. Play-as-you-write dialogue and quest previewer (L)
15. Re-runnable procedural generators (L); real-time multi-user editing (XL), weakest
    evidence for modders

Not found: any user survey; official docs for several Unreal features; Source 2 Hammer
praise; independent reports of MCP failures in game tools; generated quest or dialogue
tooling; Core, Dreams, Halo Forge.

## Sources, part A

1. https://tes5edit.github.io/docs/2-overview.html (fetched)
2. https://tes5edit.github.io/docs/5-conflict-detection-and-resolution.html
3. https://mutagen-modding.github.io/Synthesis/ (documentation page, fetched; no source read)
4. https://nolvus.net/guide/asc/output/synthesis
5. https://forums.nexusmods.com/topic/8312063-bashed-patch-mod-merging-and-leveled-lists-question
6. https://loot.readthedocs.io/en/latest/app/usage/main.html
7. https://stepmodifications.org/wiki/Guide:Mod_Organizer
8. https://stepmodifications.org/forum/topic/17108-cathedral-assets-optimizer-by-g_k
9. https://mutagen-modding.github.io/Spriggit/workflow (documentation page only)
10. https://wiki.wabbajack.org/modlist_author_documentation/Introduction.html
11. https://forum.paradoxplaza.com/forum/threads/tool-cwtools-a-mod-validating-extension-for-vs-code.1066033/ and https://marketplace.visualstudio.com/items/tboby.cwtools-vscode
12. https://admin-forum.paradoxplaza.com/forum/threads/how-to-debug-log-scopes-when-developing-mods.1564821
13. https://bcssov.github.io/IronyModManager/games.html
14. https://kubejs.com/
15. https://minecraft.how/blog/post/misode-data-pack-generator
16. https://docker-minecraft-server.readthedocs.io/en/latest/mods-and-plugins/packwiz/
17. https://learn.microsoft.com/en-us/minecraft/creator/documents/blockbench
18. https://docs.neoforged.net/docs/resources/client/models/datagen
19. https://rimworldwiki.com/wiki/Modding_Tutorials/PatchOperations
20. https://lua-api.factorio.com/latest/auxiliary/data-lifecycle.html
21. https://lua-api.factorio.com/latest/auxiliary/json-docs-runtime.html (fetched)
22. https://openmw.readthedocs.io/en/latest/manuals/openmw-cs/tour.html (fetched)
23. https://openmw-trav.readthedocs.io/en/latest/manuals/openmw-cs/tables-file.html
24. https://forums.nexusmods.com/topic/11037443-construction-kit-extender-more-broken-than-the-normal-thing
25. https://thunderstore.io/c/baldurs-gate-3/p/ShinyHobo/BG3_Modders_Multitool
26. https://wiki.redmodding.org/wolvenkit/wolvenkit-app/editor/asset-browser and https://wiki.redmodding.org/cyberpunk-2077-modding/for-mod-creators/modding-tools/redhottools/rht-hot-reload
27. https://nexusmods.com/witcher3/mods/484
28. https://stardewvalleywiki.com/Modding:Content_pack_frameworks
29. https://civilization.fandom.com/wiki/Modding_(Civ6)/Basics_of_Mod_Creation
30. https://community.bistudio.com/wiki/Eden_Editor:_Introduction
31. https://www.halowaypoint.com/news/halo-infinite-forge-fundamentals
32. https://www.thesixthaxis.com/2018/04/06/far-cry-5-arcade/

## Sources, part B (search summaries; pages not opened in full)

- https://dev.epicgames.com/documentation/unreal-engine/one-file-per-actor-in-unreal-engine
- https://usourcecontrol.com/guides/one-file-per-actor-source-control
- https://www.anchorpoint.app/blog/git-with-unreal-engine-5
- https://dev.epicgames.com/documentation/en-us/unreal-engine/data-validation-in-unreal-engine
- https://forums.unrealengine.com/t/list-of-assets-and-their-size-from-a-cooked-world-partition-umap-package/2656260
- https://forums.unrealengine.com/t/size-of-a-map/2698058
- https://csaf.itch.io/asset-sentinel
- https://dev.epicgames.com/documentation/unreal-engine/multi-user-editing-overview-for-unreal-engine
- https://dev.epicgames.com/documentation/unreal-engine/gauntlet-automation-framework-overview-in-unreal-engine
- https://dev.epicgames.com/documentation/unreal-engine/automation-test-framework-in-unreal-engine
- https://dev.epicgames.com/documentation/fortnite/unreal-revision-control-best-practices-in-unreal-editor-for-fortnite
- https://forums.unrealengine.com/t/push-verse-changes-is-unreliable/1295215
- https://unity.com/blog/author-scenes-and-prefabs-with-verson-control
- https://docs.unity.com/en-us/engine/current/manual/get-started/project-configuration/version-control/smart-merge
- https://issuetracker.unity3d.com/issues/unityyamlmerge-dot-exe-doesnt-correctly-handle-merge-conflicts-in-modified-properties-on-a-prefab-variant
- https://docs.unity3d.com/Packages/com.unity.addressables@2.0/manual/AddressableAssetsOverview.html
- https://discussions.unity.com/t/today-muse-refused-to-edit-a-basic-script-for-me-stating-it-is-better-for-me-to-learn/289247
- https://www.pcgamer.com/artists-and-developers-worried-about-unitys-ai-programs-have-been-given-a-big-shrug
- https://unity.com/blog/mcp-servers-game-development
- https://docs.godotengine.org/en/stable/engine_details/file_formats/tscn.html
- https://docs.godotengine.org/en/4.5/tutorials/scripting/debug/overview_of_debugging_tools.html
- https://www.moddb.com/features/trenchbroom-2s-creator-on-making-a-new-level-editor-for-classic-shooters
- https://developer.valvesoftware.com/wiki/Hammer_(Source_2)
- https://www.mapeditor.org/2022/02/07/tiled-1-8-0-released.html , /2022/06/25/tiled-1-9-released.html , /2024/06/27/tiled-1-11-released.html
- https://ldtk.io ; https://deepnight.itch.io/ldtk
- https://create.roblox.com/docs/studio/team-create
- https://devforum.roblox.com/t/assistant-updates-giving-you-more-control-of-assistant/3555172
- https://about.roblox.com/en-au/newsroom/2026/04/roblox-studio-going-agentic
- https://techcrunch.com/2026/04/16/robloxs-ai-assistant-gets-new-agentic-tools-to-plan-build-and-test-games/
- https://anchorpoint.app/blog/git-vs-perforce-for-game-development
- https://forums.unrealengine.com/t/git-lfs-vs-perforce/60381
- https://docs.blender.org/manual/en/3.0/interface/controls/templates/operator_search.html
- https://dev.epicgames.com/documentation/unreal-engine/procedural-content-generation-framework-in-unreal-engine
- https://www.sidefx.com/docs/houdini/unreal/pcg/overview.html
- https://www.strayspark.studio/blog/unity-mcp-server-vs-unreal-godot-blender-2026 (vendor)
- https://inklestudios.com/ink/ ; https://intfiction.org/t/inky-an-ink-ide-with-web-format-export/10257
- https://blog.arcweave.com/top-10-tools-for-narrative-design (vendor)
