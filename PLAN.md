# KitCraft — open-source Creation Kit for Starfield, built the "craft suite" way

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
   every crash fix ships with a regression test. Commit after every arc that builds, push to main.

### REA (morluto/rea) — what to take, what to leave
REA = one MCP + CLI (same workflows, same evidence contracts) over Ghidra/IDA/Hopper/JADX/etc.
Useful ideas: evidence-first results with `limitations[]` and coverage status, CLI/MCP parity
with `--json`, snapshots keyed on target bytes + params, "unknown != absent", a nine-stage
reconstruction-readiness ledger, and six tool shapes (`inspect / search / trace / compare /
workflow / observe`).

**Do not point REA at CreationKit.exe or Starfield.exe for this project.** That breaks the
clean-room rule the craft suite depends on, and we don't need it: the shared container is
public (UESP's Skyrim pages), and Starfield-specific layouts come from the corpus and from
observing the CK's own output (see `plan/adr/0005-spec-sources-per-fact.md`). REA's process (evidence ledger, parity gates) is what we
copy; its decompilers we skip. REA on our own binaries (crash triage, fuzz results) is fine.

### Scope reality check
The real CK is a 240 MB Qt5 app with a Creation Engine 2 render viewport, worldspace/landscape
editing, navmesh generation, FaceFX/LIP, Wwise, Papyrus, and BA2 packing. "Bit for bit" of the
UI is years. "Bit for bit" of the data is testable in weeks: load every official ESM and
write it back byte-identical. That is milestone 1 and the thing agents actually need.

## Source material we are allowed to use
| Source | License | Use |
|---|---|---|
| UESP Skyrim Mod File Format pages (no Starfield page exists) | CC-BY-SA docs | Shared record container only; per-fact sources in `plan/adr/0005-spec-sources-per-fact.md`. Facts only, no prose. |
| Creation Kit wiki (public) | site terms | Field names and semantics (ADR 0005). Facts only, no prose. |
| Our own hex dumps of Starfield.esm + DLC ESMs (143 GB Data dir on this machine) | ours | Roundtrip corpus, fuzz corpus. |
| CreationKit.exe (installed, MO2 exe #4) | Bethesda EULA | Black-box observe only: run it, screenshot, diff its ESM output vs ours. The only source for Starfield field layouts (CK differential observation, ADR 0005). |
| xEdit `wbDefinitionsSF1.pas` (789 KB) | MPL-2.0 | Readable as a reference, but treat like GPL for safety: no copying, prefer UESP. Decide in an ADR. |
| Mutagen / Spriggit (Starfield record XML, 948 files) | GPL-3.0 | **Do not read** into a MIT/Apache project. May run Spriggit as an external tool for step 2 now. |
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
kit query  --type WEAP --where "WFIR.ShotsPerSecond > 10" --json
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
Where "gun feel" actually lives in Starfield data (from the public record spec):
- **WEAP.WFIR** firing type, burst count, attack/fire/delay seconds, bolt action/charge,
  shots per second, staged/dual trigger, shell eject.
- **WEAP.WDMG** damage, min/max range, out-of-range mult, crit mult, hit behavior, resistance.
- **WEAP.WAM2** ammo type, capacity, projectiles per shot, projectile override, NPC ammo use.
- **WEAP.WRLO** reload speed/charging/single. **WEAP.WCHG** charge. **WEAP.WAIM** zoom link,
  aim model link, scope, reticle.
- **AMDL** (aim model) = cone of fire, recoil, spread. Most of "the guns feel wrong" is here.
- **PROJ** = speed, gravity, range, hitscan vs projectile, tracer, impact. **AMMO**. **OMOD** =
  weapon mod property changes; **COBJ** = craftability. **ZOOM** = ADS FOV.
- Not in records: animation timing (hkx), audio content (Wwise), models.

The overhaul is a data-driven spec, not hand edits. `overhaul.toml` describes intent
(classes of weapons, rules like "ballistic rifles: projectile not hitscan, recoil curve X,
reload 0.8x") and `kit build` compiles it to one ESM. Agents iterate on the spec, run
`kit query` to check the result, and `kit validate` before every test launch. One plugin, no
leveled-list injection, no scripts, so it can be dropped from a save safely.

We can start rung 1 and 2 before step 1 exists: run Spriggit (ESM -> YAML), let an agent edit
the YAML, Spriggit back to ESM. Needs .NET 8 runtime. The YAML output doubles as the oracle
corpus M1 tests against. Swap the back end to `kit build` when M2 lands.

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
2. Whether xEdit's MPL source may be read for ambiguous subrecords (ADR 0004, still open).
   Until decided, Starfield layouts come only from the ADR 0005 sources, chiefly CK
   differential observation.
3. Which YouTube critique to use as the design brief for the overhaul (link it in plan/).
4. License: MIT + Apache-2.0 dual, matching the craft suite.
5. Whether to contact kinggath about SS2 now or after the overhaul kit proves the pipeline.
