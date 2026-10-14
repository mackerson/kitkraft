# Source registry

Every outside source KitCraft may consult, run, or must avoid, with our status for each.
Derived from section 1 of the owner-supplied dossier
(`docs/research/starfield-dossier-2026-10-14.md`), plus the pinned xEdit files and the
sources ADR 0005 already used. Normative rules: ADR 0001, ADR 0004 (with its owner
approval), ADR 0005 Amendments 2 and 3. A source not listed here gets a row before it is
cited anywhere.

Statuses:

- **permitted-reference**: may be read and its facts cited, in our own words, under the
  ADR rules (never copy code or prose).
- **external-oracle-only (run, never read)**: may be run as a separate program to compare
  its output with ours; its source is never read, and its output field names and key
  structure are never used. Never bundled.
- **docs-only**: public documentation we may read to learn what to test. Its claims are
  never cited as a layout source; facts are cited through our own `corpus:` or `ck-diff:`
  evidence, naming the registry key in the note. Verify reuse rights before copying any
  text (default: copy nothing).
- **forbidden**: not read, not used, for any purpose (ADR 0005 §3).
- **future**: outside M1/M2 scope; status to be settled when a lane needs it.

Lanes: A container (`esm`), B schema, D doc + io, E set/validate/build, F mcp + surface,
G archives, scripts, strings (`plan/execution-plan.md`).

## Sources

| Key | Source URL | License | Our status | Lanes it serves | Notes |
| --- | ---------- | ------- | ---------- | --------------- | ----- |
| XEDIT-SF1 | <https://github.com/TES5Edit/TES5Edit/blob/9fb016884bec138ea6c7b872cec831537d464c3e/Core/wbDefinitionsSF1.pas> | MPL-2.0 (file header and repo `LICENSE.txt`) | permitted-reference | B (also A for `TES4` flags, D for plugin flags) | Level 2 (ADR 0005 Amendment 2). Pinned SHA `9fb016884bec138ea6c7b872cec831537d464c3e`; 789,023 bytes, 19,376 lines. Cite `xedit: wbDefinitionsSF1.pas@<sha>#L<a>-L<b>`. Read from a copy outside the repo; delete after use. Details: `docs/xedit-reference.md` |
| XEDIT-COMMON | <https://github.com/TES5Edit/TES5Edit/blob/9fb016884bec138ea6c7b872cec831537d464c3e/Core/wbDefinitionsCommon.pas> | MPL-2.0 (file header and repo `LICENSE.txt`) | permitted-reference | B | Authorized by the owner 2026-10-14 (ADR 0004, owner approval, clarification 1). Same pinned SHA; 338,167 bytes, 10,348 lines. Cite `xedit: wbDefinitionsCommon.pas@<sha>#L<a>-L<b>`, only where an SF1 definition invokes the helper; Skyrim-specific readings are not imported. Loader support pending (lane B) |
| XEDIT-CORE | <https://github.com/TES5Edit/TES5Edit> (rest of the repository at the same SHA, and its [license](https://github.com/TES5Edit/TES5Edit/blob/9fb016884bec138ea6c7b872cec831537d464c3e/LICENSE.txt)) | MPL-2.0 | future | A, B | Another helper file is cited only after its dependency is documented in `docs/xedit-reference.md` and here, under the same rules (ADR 0004 clarification 2). Container code (`wbImplementation.pas`) is not consulted: the container layer is level 1 (UESP Skyrim pages plus corpus). Changelog may be read as docs |
| SF1EDIT | xEdit run in Starfield mode (binary built from TES5Edit) | MPL-2.0 | external-oracle-only (run, never read) | B (M2.B11), D | Level 3 differential comparison (ADR 0005 Amendment 2 §2a). Not installed yet (M2.O3); record the build and its skew from the pin. Never bundled |
| MUTAGEN | <https://github.com/Mutagen-Modding/Mutagen> | GPL-3.0 | external-oracle-only (run, never read) | B, D | Source, generated code and record XML are never read (ADR 0005 §3). May be run as a separate process on fixtures; compare values, not names |
| MUTAGEN-BASELINE | <https://github.com/Mutagen-Modding/Mutagen/issues/482> | GitHub issue; related code GPL-3.0 | forbidden | - | A record-by-record coverage list derived from Mutagen's implementation: a "summary, table or field list derived from" a forbidden source (ADR 0005 §3) |
| SPRIGGIT | <https://github.com/Mutagen-Modding/Spriggit> ([docs](https://mutagen-modding.github.io/Spriggit/)) | GPL-3.0 | external-oracle-only (run, never read) | B, F | May be run; its output is used only for byte-level inventory facts. Its output field names, YAML key structure and struct layouts are forbidden (ADR 0001 §4, ADR 0005 §3). Docs read for CLI usage only |
| ESPLUGIN | <https://github.com/Ortham/esplugin> | GPL-3.0 | external-oracle-only (run, never read) | A, D | Starfield-aware plugin header and container parser; useful as an oracle for plugin flags and masters |
| LOADORDER | <https://github.com/Ortham/libloadorder> | GPL-3.0 | external-oracle-only (run, never read) | D | Load-order behaviour oracle. Its issue #97 states the stale medium flag `0x10000`; see ADR 0005 Amendment 3 §5 |
| LOOT | <https://github.com/loot/loot> (application code) | GPL-3.0 | external-oracle-only (run, never read) | D, E | LOOT as a program may be run on a load order to compare sorting and warnings |
| ORTHAM | <https://blog.ortham.net/posts/2024-06-28-load-order-in-starfield/> | Author's public page; reuse license not established | docs-only | A, D | Verify reuse rights before copying text. Strong (grade B) on at-rest FormIDs and master scales; tested on 2023/2024 builds, so runtime load-order claims are regraded for current builds |
| LOOT-GUIDE | <https://loot.github.io/docs/help/changing-plugin-types-in-starfield/> | Public documentation; reuse license not established | docs-only | D, E | Verify reuse rights before copying text. CK `.esp` working file vs `.esm` export; do not change a published master's scale; `FE`/`FD` index spaces and record limits |
| LOOT-CHANGELOG | <https://github.com/loot/loot/blob/master/docs/app/changelog.rst> | In the GPL-3.0 LOOT repository; docs license not separately stated | docs-only | D | Verify reuse rights before copying text. 2026 behaviour: `BlueprintShips-*` implicit activation (0.29.0/0.29.1), extra hardcoded official masters |
| LOOT-MASTERLIST | <https://github.com/loot/starfield> | CC0-1.0 | permitted-reference (data) | D, E | Permitted data: compatibility conditions and warnings may be read and used (CC0). Not a source of record layouts |
| BML-SF | <https://baddogskyrim.github.io/BethesdaLibrary/game-specific/starfield/plugins/> | Website; reuse license not established | docs-only | A, D, G | Verify reuse rights before copying text. Secondary overview (grade C) of the container, base form components, master tiers, BA2 v2/v3 |
| BML-TOOLS | <https://baddogskyrim.github.io/BethesdaLibrary/game-specific/starfield/tools/> | Website; reuse license not established | docs-only | G | Verify reuse rights before copying text. Tool landscape (SF1Dump/xDump, BSArch, mesh and material pipelines); verify against upstream tools |
| CK | <https://store.steampowered.com/app/2722710/Starfield_Creation_Kit/> | Bethesda proprietary, EULA | external-oracle-only (run, never read) | A, B, D, E | Black-box oracle only (ADR 0001 §3): open, edit, save, screenshot, diff its output. Never disassemble, debug, or memory-dump it; its files stay in `observe/` (ADR 0005 §2) |
| CK-HELP | <https://help.bethesda.net/app/answers/detail/a_id/33408/~/where-can-i-get-help-for-the-creation-kit> | Bethesda support page | docs-only | B | Says the official CK wiki is under maintenance; affects `ck-wiki:` naming (see CK-WIKI) |
| PAPYRUS | <https://papyrus.bellcube.dev/starfield/> | Site docs CC BY-SA 4.0, site code MIT; indexed scripts and wiki sources have their own licenses | permitted-reference (future) | G | For Papyrus function and event signatures when script work starts (after M2). Facts in our own words; no CC BY-SA text copied; check the license of the underlying script source per item |
| COMMONLIB | <https://github.com/libxse/commonlibsf> | GPL-3.0-or-later with modding/linking exceptions | external-oracle-only (run, never read) | - (future) | Native runtime plugins are a separate pipeline from data mods; out of scope. Not read |
| SFSE | <https://github.com/ianpatt/sfse> ([license](https://github.com/ianpatt/sfse/blob/master/LICENSE)) | Only `sfse/PluginAPI.h` expressly permissive; the rest is not broadly permissive; readme restricts redistribution | future | - (future) | License caveat: treat everything except `PluginAPI.h` as not reusable; never bundle extender binaries. Revisit only if native runtime work is scoped |
| OPEN-PAPYRUS | <https://open-papyrus.github.io/docs/> | Per-project and per-file licenses, not checked | future | G | Compiler and parser prior art; check licenses and Starfield coverage before any use |

## Sources already in our hierarchy (ADR 0005)

| Key | Source URL | License | Our status | Lanes it serves | Notes |
| --- | ---------- | ------- | ---------- | --------------- | ----- |
| UESP-SKYRIM | <https://en.uesp.net/wiki/Skyrim_Mod:Mod_File_Format> | CC BY-SA | permitted-reference | A, B | Level 1, container only; `confidence = "corpus"` or better. Cite `uesp-skyrim:` |
| UESP-STARFIELD | <https://starfieldwiki.net/wiki/Starfield_Mod:Mod_File_Format> | CC BY-SA 4.0 | permitted-reference | B | Level 4 (Amendment 1); `/Raw_Data` is an inventory cross-check only; `/CTDA` is rejected by the loader (cite xEdit). Cite `uesp-starfield:` |
| CK-WIKI | <https://www.creationkit.com/> | Not checked; prose never copied | permitted-reference | B | Level 4: names and meanings in our own words. Cite `ck-wiki:`. Under maintenance per CK-HELP |
| CORPUS | The 14 official ESMs on this machine (`corpus/manifest.toml`) and our synthetic plugins | Bethesda data, read in place, never committed | permitted-reference | A, B, D, E | Level 3. Cite `corpus:` |

## Evidence grades

Claims drawn from any row are graded A (upstream code, official tool output, reproducible
fixture), B (documented experiment or maintained docs), C (secondary overview or analogy),
D (hypothesis). Grade the claim, not the source (ADR 0005 Amendment 3 §3).
