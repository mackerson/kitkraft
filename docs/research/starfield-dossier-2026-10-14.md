# Starfield / Creation Kit prior-art and source-policy dossier

**Prepared:** 2026-10-14  
**Audience:** ArtCraft director, Lane A (binary container), Lane B (record schemas and code generation), Lane C (CLI, conversion, integration), review/policy agents.  
**Status:** Research handoff, not an independently verified complete binary specification; no Starfield game data is included.  
**Objective:** Remove avoidable research and provenance debate without licensing shortcuts or guesses about untested game behavior.

## Executive decisions

1. **The missing UESP Starfield format page does not mean a missing format reference.** The Starfield-specific xEdit definition file exists at `Core/wbDefinitionsSF1.pas` (~19,376 GitHub source lines on `dev-4.1.6`), and the file explicitly bears the MPL 2.0 notice. Use it as a primary *implementation reference* with a pinned commit; do not claim it is Bethesda's normative specification.
2. **Keep authoring code independently implemented.** Binary facts, identifiers, field widths, and other format observations can be independently described and verified. Copying source expressions, source file structure, or mechanically translated definitions deserves a separate license treatment. Preserve notices and obligations where copyrighted material is reused. A privately run project need not solve every future redistribution choice in advance.
3. **Use multiple independent validators.** For record semantics use SF1Edit/xEdit and Mutagen, for isolated container and plugin metadata use esplugin, for transformation use Spriggit, and for load order use libloadorder/LOOT. Separate processes and independently generated observations are preferable to importing GPL implementation code into a differently licensed core.
4. **Binary passthrough and typed semantic coverage are separate maturity levels.** Missing definitions must not force dropped bytes or guessed layout. Unknown records/subrecords/components need ordered raw preservation. Round-trip byte equivalence is a distinct target from normalized semantic equivalence.
5. **Starfield FormID resolution is master-aware.** At-rest FormIDs can use distinct indices for full/medium/small masters, and a child file does not fully self-describe the types of its masters. Isolated parsing is valid; authoritative link resolution requires dependency information.
6. **Version the behavioral facts.** Starfield's load order, blueprint handling, and DLC format support changed after launch, including in 2026. Pin game build, tool revisions, and source versions in every fixture and result.

## 1. Source registry

| Key | Source | License / access | Priority | Purpose |
| --- | --- | --- | --- | --- |
| XEDIT-DEF | [xEdit Starfield definitions](https://raw.githubusercontent.com/TES5Edit/TES5Edit/dev-4.1.6/Core/wbDefinitionsSF1.pas) | **MPL-2.0 notice within file**, repository MPL-2.0 | A | Starfield record layout, named fields, components, flags, and conditions; requires pinned SHA |
| XEDIT-CORE | [TES5Edit repository](https://github.com/TES5Edit/TES5Edit) and [license](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/LICENSE.txt) | MPL-2.0 | A | Shared container details, xDump/BSArch tooling, changelog |
| MUTAGEN | [Mutagen repository](https://github.com/Mutagen-Modding/Mutagen) | GPL-3.0 | A for independent implementation, external oracle | Typed Starfield records, serializers, writer/generator architecture and regression tests |
| MUTAGEN-BASELINE | [Starfield baseline issue #482](https://github.com/Mutagen-Modding/Mutagen/issues/482) | GitHub project/issue, related code GPL-3.0 | A/B | Record-by-record passthrough coverage and known unknowns |
| SPRIGGIT | [Spriggit](https://github.com/Mutagen-Modding/Spriggit) and [docs](https://mutagen-modding.github.io/Spriggit/) | GPL-3.0 | A, external oracle | Plugin ↔ version-controlled text workflows, CLI roundtrip, mutation/diff expectations |
| ESPLUGIN | [esplugin](https://github.com/Ortham/esplugin) | GPL-3.0 | A, external oracle | Rust, Starfield-aware plugin metadata/container parsing |
| LOADORDER | [libloadorder](https://github.com/Ortham/libloadorder) | GPL-3.0 | A, external oracle | Dependency and load-order behavior |
| ORTHAM | [Load order in Starfield](https://blog.ortham.net/posts/2024-06-28-load-order-in-starfield/) | Author's public research page; **verify reuse rights before copying text** | A/B, empirically grounded | Crucial FormID-at-rest/master type behavior, example byte-level observations; originally tested against 2023/2024 game builds |
| LOOT-GUIDE | [LOOT: Changing plugin types](https://loot.github.io/docs/help/changing-plugin-types-in-starfield/) | Public documentation, separate copyright check before reuse | A/B | CK `.esp` working-file → `.esm` export; irreversible compatibility of published type changes |
| LOOT-CHANGELOG | [LOOT changelog](https://github.com/loot/loot/blob/master/docs/app/changelog.rst) | Source-hosted docs, check repo license | A/B | **2026 behavior**, notably `BlueprintShips-*` autoload and diagnostic changes |
| LOOT-MASTERLIST | [Starfield LOOT masterlist](https://github.com/loot/starfield) | **CC0-1.0** | A, metadata | Compatibility conditions/warnings independent of plugin format field schemas |
| BML-SF | [Bethesda Modding Library: Starfield plugins](https://baddogskyrim.github.io/BethesdaLibrary/game-specific/starfield/plugins/) | Website; **reuse license not established** | B/C, secondary reference | Human-readable description of TES4/GRUP, 24-byte records, `XXXX`, base form components, master tiers, BA2 v2/v3 |
| BML-TOOLS | [Bethesda Modding Library: tools](https://baddogskyrim.github.io/BethesdaLibrary/game-specific/starfield/tools/) | Website; reuse license not established | C | SF1Dump/xDump, BSArch, geometry/material pipelines; verify details against upstream tools |
| CK | [Starfield Creation Kit Steam app 2722710](https://store.steampowered.com/app/2722710/Starfield_Creation_Kit/) | Bethesda proprietary editor/EULA | A for observed output, not source | Final authoring/export acceptance oracle and local fixture creator |
| CK-HELP | [Bethesda Creation Kit support](https://help.bethesda.net/app/answers/detail/a_id/33408/~/where-can-i-get-help-for-the-creation-kit) | Bethesda support | B | Bethesda says Creation Kit wiki is under maintenance; UESP is an unofficial alternative |
| PAPYRUS | [Papyrus Index — Starfield](https://papyrus.bellcube.dev/starfield/) | Site documentation CC BY-SA 4.0, site code MIT; upstream scripts and wiki sources have their own licenses | B, future | Indexed scripts, function and event signatures for future AI-authored scripted mods |
| COMMONLIB | [libxse/commonlibsf](https://github.com/libxse/commonlibsf) | GPL-3.0-or-later **with explicit modding/linking exceptions** | B, future | Native runtime plugin development (distinct from ESM conversion) |
| SFSE | [ianpatt/sfse](https://github.com/ianpatt/sfse) and [LICENSE](https://github.com/ianpatt/sfse/blob/master/LICENSE) | Only `sfse/PluginAPI.h` expressly permissive; rest is not broadly permissive; redistribution restrictions in readme | B, future | Runtime script extender and plugin API; do not bundle extender binaries without permission |
| OPEN-PAPYRUS | [Open Papyrus](https://open-papyrus.github.io/docs/) | Project-specific files/licenses must be checked | C, future | Papyrus tooling, compiler/parser and reference prior art; Starfield feature coverage requires verification |

**Evidence grading:** A = upstream code, official tool output, or directly reproducible fixture; B = carefully documented experiments/research or maintained docs; C = secondary overview/historical analogy; D = hypothesis. Grade the *claim*, not just the source. A 2024 behavior claim may be stale on a 2026 build.

## 2. Verified Starfield plugin facts / invariants

- Container lineage: TES4-family `TES4` header, `GRUP` hierarchy, record/subrecord streams. xEdit's Starfield definitions set `wbSizeOfMainRecordStruct := 24` and `wbHEDRVersion := 0.96` (both explicitly visible in the primary source). See `XEDIT-DEF` around the `DefineSF1` initializer.
- Starfield base-form components: xEdit defines `wbBaseFormComponents` and `ksrBaseFormComponents`, with component delimiters including `BFCB` / `BFCE`. This is not a safe blind copy of Fallout 4 records. Some generic/reflection payloads remain less comprehensively interpreted.
- xEdit's `TES4` header flag definition includes: Master `0x001`, Optimized `0x010`, Localized `0x080`, Small `0x100`, Update `0x200`, Medium `0x400`, Blueprint `0x800`. Direct source: [raw file](https://raw.githubusercontent.com/TES5Edit/TES5Edit/dev-4.1.6/Core/wbDefinitionsSF1.pas), near `wbRecord(TES4, 'Main File Header'...)`.
- Small/light: `FE` runtime index space, up to 4,095 new records per plugin and 4,096 plugin indices; medium: `FD` runtime index space, up to 65,535 records per plugin and 256 indices. Full uses normal high-order runtime index. Source: [LOOT guide](https://loot.github.io/docs/help/changing-plugin-types-in-starfield/), [Ortham](https://blog.ortham.net/posts/2024-06-28-load-order-in-starfield/).
- **Important correction:** [libloadorder issue #97](https://github.com/Ortham/libloadorder/issues/97) says medium flag `0x10000` in its early description, but the later detailed article by the same author and xEdit's current source both say `0x400`. Prefer the corroborated `0x400`; add a regression case rather than copying issue descriptions blindly.
- **The key cross-lane trap:** A child plugin's at-rest FormIDs can encode references to its full, medium, and small masters in different shapes; it does not carry enough standalone metadata to resolve those master types authoritatively. Load the master graph and determine scales before resolving to a stable `FormKey`. Details and worked hex example: [Ortham research](https://blog.ortham.net/posts/2024-06-28-load-order-in-starfield/). 
- `full/medium/small` is a *scale*, while flags such as master/update/blueprint express other behaviors; do not collapse them all into a single mutually exclusive enum.
- Bethesda's Creation Kit uses `.esp` as a working project and expects creators to export `.esm` for shipping. Once a released master becomes another mod's dependency, changing its scale breaks references; the recommended approach is to keep the released type fixed. Source: [LOOT guide](https://loot.github.io/docs/help/changing-plugin-types-in-starfield/).
- Blueprint special case **changed/clarified in 2026**: [LOOT 0.29.0/0.29.1 changelog](https://github.com/loot/loot/blob/master/docs/app/changelog.rst) explains companion `BlueprintShips-<base>` plugins can become implicitly active when the corresponding base plugin is active, and records invalid filename/flag pairings. Never model blueprint plugins as ordinary user-controlled `plugins.txt` entries without testing a current build.
- Plugin behavior varies by game build. The 2024 Ortham research is unusually strong for on-disk ID semantics, but its 2024 list of implicitly active official masters is historical; LOOT's April 2026 changes mention additional hardcoded official masters. Keep runtime load-order behavior versioned.
- Assets are a **separate domain**: Starfield BA2 uses `BTDX` with observed v2/v3 archive varieties; mesh and material systems changed substantially. Do not scope BA2/NIF/Papyrus into Lane B's ESM record-schema blocker. See [BML Starfield format page](https://baddogskyrim.github.io/BethesdaLibrary/game-specific/starfield/plugins/) for a secondary overview, subject to corroboration.

## 3. Architecture and source policy to unblock Lane B

**Recommended split:**

- **Lane A:** raw container walker/writer, record and `GRUP` headers, lengths, flags, compression, subrecord framing, unknown-data passthrough, physical offsets and stable ordering. Must function with no master files.
- **Lane B:** declarative record schemas and components, typed decoding, stable source provenance per supported field, typed encode/decode of recognized subrecords, opaque preservation of unrecognized material. No global requirement for 100% record coverage.
- **Link-resolution layer (cross-lane):** master discovery and graph, plugin scale, raw FormID → semantic origin/reference, missing-master errors, cross-plugin remapping, game-version-aware load order. No lazy guesses that silently change record identity.
- **Lane C:** inspect/diff/roundtrip CLIs; independent oracles; fixture harness; artifact generation and integration with CK export. Parsing success, byte-preservation success, and game/editor load success are **separate gates**.

**Proposed policy rule (replace an exclusive UESP whitelist):**

> Public specifications and documented experiments may establish format facts. Where there is no published normative specification, version-pinned, licensed, community implementations may serve as implementation references, provided each derived claim records source/revision/license and is independently validated when it affects writes. The presence of a public source is not an authorization to copy or relicense its expressive implementation. Use directly copied code only under a compatible license and fulfill any notice/source requirements. Unsupported fields must round-trip opaquely or fail with explicit errors; never invent a definition. Tooling may execute third-party programs as independent test oracles without incorporating their code into the project's core. Releasing redistributed third-party code or game assets requires a separate distribution/license review.

**Suggested provenance manifest fields:**

```yaml
record: WEAP
field_path: 'components/TESModel_Component/MODL'
observation: 'field signature and path interpretation'
source:
  project: TES5Edit/TES5Edit
  repository: 'https://github.com/TES5Edit/TES5Edit'
  revision: '<immutable commit SHA - resolve and pin>'
  file: 'Core/wbDefinitionsSF1.pas'
  license: 'MPL-2.0'
classification: 'implementation_observation'
validated_by:
  - 'local CK-exported fixture (game build and SHA to be recorded)'
  - 'SF1Edit parsed-view comparison (tool version to be recorded)'
confidence: 'supported'
owned_implementation: true
copy_of_upstream_code: false
```

**Layer checker:** Check each claim has an allowed **kind** (`normative_doc`, `implementation_observation`, `reproduced_experiment`, `local_fixture`, `secondary_doc`), explicit URL/revision/license and scope; reject unattributed literal source copying or silent unverified assertions, not whole projects simply because UESP lacks a corresponding page. Distinguish *referencing* a GPL implementation from *embedding* its source.

## 4. Validation plan: cheap tests before ambitious reverse engineering

Create a fixture matrix using locally owned game/CK output. Do not commit proprietary Bethesda `.esm`, `.ba2`, extracted art, or proprietary tool binaries to a public repository.

### Fixture types

1. Empty/new CK working `.esp` and exported full/medium/small `.esm` where supported.
2. One- and two-master dependencies covering full→full, small→full, medium→full and mixed master tiers; capture all masters alongside the child for resolution tests.
3. Basic typed records such as `GMST`, `KYWD`, `FLST`, `WEAP`, `ARMO`, `NPC_`, `QUST`, `CELL` where conveniently generated by CK; prioritize *actual project needs* rather than all record signatures.
4. Base form components and duplicate/ordered subrecords, plus synthetic unknown four-byte signatures and bytes.
5. `GRUP` hierarchy and edge lengths; compressed records; extended-size `XXXX` subrecords; malformed/truncated sizes; integer overflow attempts.
6. Overrides of existing masters with a new local record that deliberately shares an object-index suffix, to detect identity confusion.
7. Localized strings and references to `.STRINGS` files, tested separately from the binary record writer.
8. Ship blueprint fixtures only after basic M1 closure; use currently supported naming and flags.

### Gates

- **G0 parse:** no out-of-bounds access; handles malformed inputs with precise diagnostics.
- **G1 lossless passthrough:** `emit(parse(original))` byte-identical for supported no-op fixtures, including untouched unknown chunks, flags, and ordering. If writer canonicalizes intentionally, record a separate documented semantic equivalence mode rather than silently weakening this gate.
- **G2 narrow edit:** alter one known field, then independently inspect binary diff; only expected serialized regions and size fields change. Unknown payloads retain bytes and relative sequence.
- **G3 cross-tool:** output reopens correctly in SF1Edit; Mutagen/Spriggit accept compatible fixtures; disagreements become tracked evidence rather than automatic corruption claims.
- **G4 editor/game:** CK opens/exports a sandbox copy; the game launches and loads a tiny test mod on a disposable save. Never use a main save for generated data experiments.
- **G5 dependency semantics:** master tiers and raw vs resolved FormIDs are preserved across round trips, including mixed masters and missing-master negative tests.

Store per-fixture metadata: SHA-256, game build, CK build, SF1Edit revision, Mutagen release, Spriggit version, master list, flags, expected record count, byte-diff/hash results, known unknowns.

## 5. Known conflict / uncertainty register

| Claim or risk | Current assessment | Resolution |
| --- | --- | --- |
| `0x10000` vs `0x400` medium-master flag | Older GitHub issue contains conflicting value; xEdit source and author's later article both say `0x400` | Test a locally exported medium master and document header bytes |
| Does a plugin file alone determine all FormID referents? | No, for mixed master scales | Add explicit master-aware resolver contract |
| Are xEdit's fields a complete formal specification? | No; community implementation with ongoing changes and unknowns | Preserve raw bytes, confidence/provenance, differential tests |
| Can GPL/CC material be consulted privately? | Consultation/observations are distinct from code copying and distribution | Treat license as a code-incorporation/release question; document provenance |
| Is MPL 2.0 automatically compatible with a newly generated schema? | Depends on whether it is original factual description versus covered derivative expression | Keep an explicit `copied_from_upstream` field; review generator output before release |
| Does nonprofit/free distribution bypass source-sharing obligations? | No general exemption | Separate private development from public distribution gate |
| Do 2024 load-order claims still hold unchanged in 2026? | Not necessarily; blueprint handling and official masters evolved | Cross-check LOOT April 2026 and current game build |
| Can a CK-authored `.esp` be installed as a finished mod? | CK workflow recommends `.esm` exports | Separate development artifact from release artifact |
| Can xEdit/Mutagen/LOOT alone prove a mod is safe? | No; correlated implementation gaps exist | CK and disposable-save in-game acceptance tests |
| Is the official CK wiki available? | Bethesda support says under maintenance | Prefer actual official CK behavior and maintained community references |

## 6. Medium-term prior art (future ArtCraft agent modalities)

- **Papyrus semantics:** [Papyrus Index — Starfield](https://papyrus.bellcube.dev/starfield/) indexes function/event signatures; its site code is MIT, but source scripts and wiki-derived data have separate licenses. [Open Papyrus](https://open-papyrus.github.io/docs/) is useful compiler/parser prior art, but verify Starfield-specific support before promising code-generation compatibility.
- **Native runtime behavior:** [CommonLibSF](https://github.com/libxse/commonlibsf) (maintained `libxse` fork), [SFSE](https://github.com/ianpatt/sfse) and [CommonLibSF template](https://github.com/libxse/commonlibsf-template). This is a separate pipeline from data-only CK mods and may impose distinct distribution obligations and Steam-only runtime compatibility.
- **Assets and BA2:** [Starfield tools overview](https://baddogskyrim.github.io/BethesdaLibrary/game-specific/starfield/tools/) points to BSArch, Geometry Bridge, NifSkope variants, material/mesh tooling. Treat this as ecosystem reconnaissance, not a normative schema.
- **Game-engine acceptance:** [Official CK on Steam](https://store.steampowered.com/app/2722710/Starfield_Creation_Kit/) is the tool to validate authoring/export, not a permissive source of game data for public fixture redistribution.

## 7. Suggested split into delegated work tickets

- **Policy agent:** Replace “UESP-or-stop” with allowed evidence classes and provenance manifest; specify conditions for source copying vs factual observation vs oracle usage; separate local development from release obligations. Deliver ADR patch plus two positive/negative layer-check examples.
- **Lane A:** Add mixed-master and raw FormID fixtures. Surface raw IDs without pretending to resolve missing master types; test extended subrecords, preserved unknown chunks, malformed inputs, no-op exactness.
- **Lane B:** Build a prioritized, original schema catalogue from verified record observations, tagging fields as `known`, `partial`, `opaque`, or `untested`. Do not paste xEdit source as project schemas without license review. Compare representative records with Mutagen and SF1Edit; type/decode by confidence.
- **Lane C:** Wrap existing tools as separately invoked validators. Build a fixture report with byte diff, semantic diff, tool versions, master graph, per-case acceptance. Test CK `.esp` → `.esm` export from locally created test artifacts.
- **Independent review:** Verify the `0x400` flag and FormID rules on current CK exports, check that writer does not discard unknown bytes, and audit whether any generator output is copied expressive material rather than independently written descriptions.

### Single paste-ready direction to the director

> Treat the UESP absence as a policy-resolution issue, not an unsolved binary-format mystery. xEdit's Starfield `Core/wbDefinitionsSF1.pas` (MPL-2.0) exists and is substantial. Mutagen (GPL-3.0), esplugin/libloadorder (GPL-3.0), Spriggit (GPL-3.0), LOOT docs, and Ortham's original load-order experiments provide independent validation references. Adopt version-pinned evidence/provenance, separate referring from copying, and preserve unknown bytes rather than inventing record layouts. The most consequential cross-layer requirement is master-aware FormID resolution; Starfield serializes references to mixed master scales in ways that a standalone child file cannot fully disambiguate. Medium flag is `0x400` (ignore stale `0x10000` issue text). Export shipping `.esm` from CK's working `.esp`, do not change a published master's scale, and accommodate 2026 blueprint-plugin autoactivation. Deliver M1 using lossless passthrough, typed schemas only for verified fields, and cross-tool differential tests. Treat public game assets and third-party source distribution as later release gates, not discovery blockers.

## Source-diligence notes

- All paths above point to publicly accessible upstream material as checked 2026-10-14. No upstream files or game data have been embedded in this dossier.
- Links to source branches are for convenience; **replace them with immutable commit permalinks** in the repo's permanent provenance ledger.
- Some GitHub issues and community pages preserve historical statements that are no longer correct. Prioritize code and current tests when references disagree, and document the dispute instead of quietly picking a preferred source.
- This dossier proposes conservative engineering/legal handling and is not a determination of compliance for any particular redistribution arrangement.
