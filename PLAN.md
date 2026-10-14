# KitCraft — open-source Creation Kit for Starfield, built the "craft suite" way

> Where this brief and AGENTS.md differ, AGENTS.md wins. Source policy (owner decision
> 2026-10-14, ADR 0004 and ADR 0005 Amendment 2): xEdit's `wbDefinitionsSF1.pas`, pinned at
> `9fb016884bec138ea6c7b872cec831537d464c3e`, is the record-definition reference, as source
> material only; Mutagen and Spriggit source stay off limits.

Working name only. Status: proposal, 2026-10-14. Nothing built yet.

## What we're copying (and what we're not)

### The storytold "craft" pattern (artcraft, cadcraft, photocraft, wordcraft, ...)
Observed in storytold/cadcraft (created 2026-10-07, 4.5k stars a week later, 10+ contributors) and its AGENTS.md:

1. **Pure Rust workspace, strict layers.** L0 formats/geometry -> L1 document model -> L2 render ->
   L3 I/O -> L4 engine -> L5 UI + MCP -> apps. Nothing below L5 may depend on the UI toolkit.
   CI (`cargo xtask ci`) enforces the layer graph.
2. **Everything is a command.** One `CommandSpec` per user-visible action (id, label, menu path,
   shortcut, aliases, JSON form). Menus, toolbar, scripts, CLI, control channel and MCP all call
   the same command. Programmatic calls never open dialogs.
3. **CLI + MCP are thin.** `apps/<x>-cli` is one `main.rs`; `crates/mcp` is `server.rs` + `tools.rs`
   + `backend.rs`. cadcraft's MCP exposes only `command_line`, `execute`, `inspect_drawing`,
   `query_entities`, `render`. The power is in the command catalog, not the tool count.
4. **Clean room, black box.** The original app may be run and observed; never disassembled,
   never have assets lifted. GPL/LGPL/AGPL codebases in the same domain: "don't even read their
   sources." Every asset listed in ATTRIBUTION.md.
5. **Agent operating docs.** AGENTS.md + CLAUDE.md, `plan/STATUS.md`, `plan/execution-plan.md`,
   `plan/adr/`, `docs/*-parity.md` checklists and `docs/gaps.md`. "Don't stop to ask unless it
   lists the decision as the owner's." Parallel agents use separate `CARGO_TARGET_DIR`.
6. **No-panic policy.** No unwrap/expect/panic/unsafe in non-test code; input bytes are hostile;
   every crash fix ships with a regression test. Commit after every arc that builds; commit to
   main; no remote yet.

### REA (morluto/rea) — what to take, what to leave
REA = one MCP + CLI (same workflows, same evidence contracts) over Ghidra/IDA/Hopper/JADX/etc.
Useful ideas: evidence-first results with `limitations[]` and coverage status, CLI/MCP parity
with `--json`, snapshots keyed on target bytes + params, "unknown != absent", a nine-stage
reconstruction-readiness ledger, and six tool shapes (`inspect / search / trace / compare /
workflow / observe`).

**Do not point REA at CreationKit.exe or Starfield.exe for this project.** That breaks the
clean-room rule the craft suite depends on, and we don't need it: the shared container is
public (UESP's Skyrim pages), Starfield record definitions come from the pinned xEdit
reference, and the corpus and the CK's own output validate them (see
`plan/adr/0005-spec-sources-per-fact.md`, Amendment 2). REA's process (evidence ledger, parity gates) is what we
copy; its decompilers we skip. REA on our own binaries (crash triage, fuzz results) is fine.

### Scope reality check
The real CK is a 240 MB Qt5 app with a Creation Engine 2 render viewport, worldspace/landscape
editing, navmesh generation, FaceFX/LIP, Wwise, Papyrus, and BA2 packing. "Bit for bit" of the
UI is years. "Bit for bit" of the data is testable in weeks: load every official ESM and
write it back byte-identical. That is milestone 1 and the thing agents actually need.

## Source material we are allowed to use
| Source | License | Level (ADR 0005 Am. 2) | Use |
|---|---|---|---|
| UESP Skyrim Mod File Format pages | CC-BY-SA docs | 1, container | Shared record container only, validated on the Starfield corpus (`confidence = "corpus"` or better). Never for Starfield record bodies. Facts only, no prose. |
| xEdit `Core/wbDefinitionsSF1.pas` @ `9fb016884bec138ea6c7b872cec831537d464c3e` (789,023 bytes) | MPL-2.0 | 2, record definitions | Canonical reference for Starfield record schemas (ADR 0004). Source material only: TOML written independently, no code or comments copied, no Pascal coupling, `xedit:` provenance + `confidence`. See `docs/xedit-reference.md`. |
| Our own hex dumps of Starfield.esm + DLC ESMs (143 GB Data dir on this machine) | ours | 3, validation | Roundtrip corpus, fuzz corpus, confirmation of every definition (`corpus`). |
| CreationKit.exe (installed, MO2 exe #4) | Bethesda EULA | 3, validation | Black-box observe only: run it, screenshot, diff its ESM output vs ours (CK differential observation, `ck-diff`). |
| SF1Edit (xEdit, run as a program; not installed yet) | MPL-2.0 | 3, validation | Differential comparison of its decoded view with ours on fixtures. Never bundled. |
| UESP Starfield pages (starfieldwiki.net), Creation Kit wiki | CC-BY-SA / site terms | 4, secondary | Cross-checks, field names and semantics. Facts only, no prose. |
| Mutagen / Spriggit (Starfield record XML, 948 files) | GPL-3.0 | forbidden | **Do not read** into a MIT/Apache project. May run Spriggit as an external tool for step 2 now. |
| `C:\Modding\scripts` (ba2x.py, pexdis.py, esmscan.py) | ours | Port to Rust crates; already proven on real files. |

## Workspace layout (mirrors cadcraft)
```
kitcraft/
  AGENTS.md CLAUDE.md ROADMAP.md ATTRIBUTION.md
  plan/STATUS.md plan/execution-plan.md plan/adr/
  docs/architecture.md docs/record-parity.md docs/ck-parity.md docs/gaps.md docs/mcp.md
  crates/
    L0  schema      record definitions as data (TOML), codegen -> typed records
    L0  esm         TES4 container: groups, records, subrecords, compression, localization, ONAM
    L0  ba2         GNRL v2/v3 + DX10 read/write (port of ba2x.py)
    L0  pex         Papyrus .pex read/disasm (port of pexdis.py), later assemble
    L0  strings     .STRINGS/.DLSTRINGS/.ILSTRINGS
    L0  nif         read-only mesh/material metadata (no renderer in M1)
    L1  doc         load order, FormID resolution across masters, overrides, conflict graph
    L2  render      (later) viewport
    L3  io          MO2 profile reader, plugins.txt, Creations manifests
    L4  engine      CommandSpec catalog, sessions, undo, validation rules
    L5  mcp         server.rs tools.rs backend.rs
    L5  ui-egui     (later; swappable)
  apps/kit-cli  apps/kit  xtask/
```

### Command catalog, first cut (each is a CLI subcommand, an MCP tool, and a future menu item)
```
kit load  --profile "C:\Modding\MO2-Starfield\profiles\Default"   # or explicit plugin list
kit inspect <FormID|EditorID> [--json]        # resolved record, winning override, all overriders
kit query  --type WEAP --where "EDID ~ Rifle" --json
kit set    <form> FULL="Old Soviet Marksman Rifle" --into MyPatch.esm
kit diff   A.esm B.esm [--form ...]           # record-level semantic diff
kit build  overhaul.toml -> Overhaul.esm      # data-driven patch compiler (step 2 lives here)
kit validate <plugin>                         # missing masters, bad links, localized FULL copies,
                                              #   stripped FULL/NTRM (the Resupply door bug), etc.
kit roundtrip <esm> --verify                  # byte-identical rewrite, the M1 gate
kit ba2 {list,extract,pack}
kit papyrus {disasm,compile}                  # wraps Bethesda's compiler; never -release
kit strings {dump,merge}
kit mcp                                       # stdio server exposing the same catalog
```
MCP tool shape (REA-style): `execute{command,args}`, `command_line{text}`, `inspect_record`,
`query_records`, `diff_plugins`, `render` (later). Every result carries `limitations[]`,
`coverage`, and byte offsets so the agent can cite evidence.

## Milestones
**M0 (day 1):** rustup + cargo on this box (missing today; node 22 and Python 3.10 present, no
.NET). Repo scaffold, AGENTS.md, layer check in xtask, CI. Record corpus = every ESM under
`Starfield\Data`.

**M1 — data parity:** `kit roundtrip` byte-identical on Starfield.esm + all SFBGS*.esm +
Constellation/OldMars/BlueprintShips. Unknown subrecords preserved as opaque bytes (never lost).
Schema coverage report in `docs/record-parity.md` auto-generated. This is the "bit for bit" that
can be proven.

**M2 — agent surface:** load-order resolution, inspect/query/set/diff/validate, MCP server.
Agents can now rip and tear. Port ba2x/pexdis/esmscan. Validation rules seeded from the gotchas
already found (localized FULL copies, stripped NTRM, god-mode pex calls).

**M3 — CK parity checklist:** black-box the real CK: open the same plugin, save, diff vs ours;
reproduce its ONAM/override/compression choices. CK editor features ranked in
`docs/ck-parity.md`; viewport explicitly deferred.

**M4+:** egui UI over the command catalog, nif/materials, worldspace, then viewport.

## Step 2 — in-game work, as a ladder

### Rung 1: rename "Old Earth Hunting Rifle" -> "Old Soviet Marksman Rifle"
One FULL subrecord on one WEAP record (plus its OMOD-generated instance names via INRD/WNAM
naming rules if the full displayed name is built from parts). Trivial once `kit set` exists;
today it is a 30-second Spriggit or xEdit edit. Known trap from the Resupply bug: Starfield.esm is
localized, so FULL must be written inline in the patch, not copied as a string ID. This is the
smoke test for the whole pipeline: `kit set` -> `kit validate` -> launch via MO2 -> see the name.

### Rung 2: gun overhaul kit
The weapon-relevant record types are **WEAP**, **AMMO**, **PROJ**, **AMDL** (aim model),
**OMOD**, **COBJ** and **ZOOM**. Their field layouts are transcribed from the pinned xEdit
reference (lane B round 3, after owner review), validated on the corpus and by CK
differential observation, and named from the CK wiki (ADR 0005 Amendment 2); they are
deliberately not listed here. (An earlier field list in this section came from a
GPL-derived schema summary and was removed.)
- Not in records: animation timing (hkx), audio content (Wwise), models.

The overhaul is a data-driven spec, not hand edits. `overhaul.toml` describes intent
(classes of weapons, rules like "ballistic rifles: projectile not hitscan, recoil curve X,
reload 0.8x") and `kit build` compiles it to one ESM. Agents iterate on the spec, run
`kit query` to check the result, and `kit validate` before every test launch. One plugin, no
leveled-list injection, no scripts, so it can be dropped from a save safely.

We can start rung 1 and 2 before step 1 exists: run Spriggit (ESM -> YAML), let an agent edit
the YAML, Spriggit back to ESM. Needs .NET 8 runtime. Spriggit is lossy: on 2 of 5 real mods
tested, the ESM -> YAML -> ESM round trip lost data, so its output is not usable as an oracle
for M1 (see `docs/gaps.md`). Swap the back end to `kit build` when M2 lands.

### Rung 3: port Sim Settlements 2
Different animal entirely. SS2 (kinggath, Fallout 4) is hundreds of Papyrus scripts, a
Workshop-framework dependency (F4SE + Workshop Framework), thousands of placed objects, custom
meshes, voiced quests, and a plot/building-plan system that assumes FO4's workshop. Starfield has
outposts, not workshops, and a different Papyrus API surface. Realistic shape:
- **Permission first.** SS2's Nexus permissions do not allow redistribution or porting without
  kinggath's consent. Ask before touching assets or scripts. Without consent the only legal path
  is a clean-room re-design of the *idea* (plots that self-build) on Starfield outpost systems.
- **What the toolchain contributes:** `kit papyrus disasm` on SS2's pex to map the architecture,
  `kit inspect` on its records to inventory asset dependencies, and a Starfield outpost-API
  compatibility report (which FO4 Papyrus calls have no Starfield equivalent).
- **Sequence:** after rung 2, and only once M2's Papyrus tooling exists. Treat it as the
  stress test that decides whether the agent surface is good enough.

## Open decisions (owner's call)
1. Name. "KitCraft" is a placeholder.
2. ~~Whether xEdit's MPL source may be read~~ Decided 2026-10-14 (ADR 0004): the pinned
   `wbDefinitionsSF1.pas` is the record-definition reference, as source material only.
3. Which YouTube critique to use as the design brief for the overhaul (link it in plan/).
4. License: MIT + Apache-2.0 dual, matching the craft suite.
5. Whether to contact kinggath about SS2 now or after the overhaul kit proves the pipeline.
