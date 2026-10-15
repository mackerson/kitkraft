# KitKraft

An open-source, clean-room Creation Kit for Starfield, in Rust.

*Naming (owner, 2026-10-14): the name is KitKraft; identifiers are `kitkraft-*`,
`KITKRAFT_*`, `kitkraft://` (the binary stays `kit`). The pre-rename `KITCRAFT_*`
environment variables and `kitcraft-manifest.json` are still read for one release
(deprecated; [docs/corpus.md](docs/corpus.md), "Environment").*

KitKraft reads and writes Starfield plugins (`.esm`/`.esp`/`.esl`), archives (`.ba2`), compiled
Papyrus (`.pex`) and string tables. Every action is one command, reachable the same way from
the `kit` CLI, from an MCP server for agents, and later from a desktop UI. Results carry their
evidence (byte offsets, coverage, limitations) so a person or an agent can check them.

## Status: pre-alpha

- **M1 done:** every official Starfield ESM (14 files) loads and writes back
  byte-identical (`kit roundtrip --verify`).
- **M2 commands live:** every catalog command is implemented: load orders from MO2
  profiles, inspect, query, diff, conflicts, set, build, validate, schema, archives
  (`.ba2`), Papyrus (`.pex`) and string tables. What a command does not do yet is stated
  in each result's `limitations`, and in [docs/gaps.md](docs/gaps.md).
- **MCP registered:** `kit mcp` serves the same catalog to Claude Code and any other MCP
  client ([docs/mcp.md](docs/mcp.md), "Use from Claude Code").

Next: the owner's in-game check of the rename patch (M2.O6), typed record layouts (lane B
round 3), then M3. See [ROADMAP.md](ROADMAP.md) and [plan/STATUS.md](plan/STATUS.md).

## Quick start

**Owner on the dev box: read [docs/QUICKSTART.md](docs/QUICKSTART.md).** `cargo xtask dist`
builds a folder of double-click files (workbench, shell, validate, deploy to the dev
profile, install/uninstall), and `Install.cmd` puts `kit` on your PATH and registers the
MCP server. No environment variables are needed.

KitKraft finds the game (Steam's registry and libraries) and MO2
(`C:\Modding\MO2-Starfield`) by itself; `kit doctor` shows what it found and how.
Precedence for every command: argument > environment variable > detected.

```powershell
cargo xtask dist                          # dist\KitKraft\kit.exe, kit-ui.exe and the .cmd files
kit doctor                                # what was found, each line OK or a fix

# Load an MO2 profile (read-only) and look around; a bare name is a profile of the detected MO2
kit load  --profile "KitKraft Dev"
kit query --type WEAP --where "FULL ~ 'Old Earth Hunting Rifle'" --profile "KitKraft Dev"
kit schema --type WEAP                    # what the schema knows about a record type

# Write a patch: rename the rifle into a new plugin, then check it
kit set Starfield.esm:0021BBCD FULL="Old Soviet Marksman Rifle" --into out\OldSovietMarksmanRifle.esm --plugins Starfield.esm
kit validate --plugin out\OldSovietMarksmanRifle.esm

# Serve the same commands to an agent over MCP (stdio)
kit mcp --profile "C:\Modding\MO2-Starfield\profiles\Default"
```

Each `kit` run is its own session, so commands that need a load order take `--profile` or
`--plugins` each time; an MCP session loads once and keeps it. `kit help` lists every
command, `kit help <command>` its arguments, and `--json` prints the full result envelope
(data, coverage, limitations, byte-offset evidence).

## Command catalog

Each line is a CLI subcommand, an MCP-reachable command, and a future menu item.

```
kit load  --profile "C:\Modding\MO2-Starfield\profiles\Default"   # or --plugins A.esm B.esp
kit inspect <FormID|EditorID> --profile ...   # every override, the winner, typed values
kit query  --type WEAP --where "EDID ~ 'Rifle'"
kit conflicts --master Starfield.esm          # which plugins override what (esmscan.py port)
kit set    <form> FULL="Old Soviet Marksman Rifle" --into MyPatch.esm
kit diff   A.esm B.esm [--form ...]           # record-level diff with byte ranges
kit build  overhaul.toml <out.esm>            # data-driven patch compiler, validated
kit validate --plugin <plugin>                # missing masters, bad links, localized FULL
                                              #   copies, stripped FULL/NTRM, god-mode calls
kit schema --type WEAP [--deep]               # members, types, sources, confidence
kit coverage <plugin>                         # record types vs what the schema defines
kit roundtrip <esm> --verify                  # byte-identical rewrite, the M1 gate
kit ba2 {list,extract,pack}
kit papyrus {disasm,compile}                  # wraps Bethesda's compiler; never -release
kit strings {dump,merge}
kit doctor [--get data_dir|mo2|dev_profile|...] # what is detected here; each line OK or a fix
kit mcp [--profile DIR]                       # stdio MCP server over the same catalog
kit ui [--dev]                                # the read-only workbench
kit install | uninstall [--apply]             # PATH + MCP registration (CLI only; dry run by default)
```

MCP tools: `execute` (any command), `command_line`, and the shortcuts `inspect_record`,
`query_records`, `diff_plugins`, `coverage`, `record_schema`, `roundtrip`; resources
`kitkraft://commands`, `kitkraft://load-order`, `kitkraft://validate-rules`. See
[docs/mcp.md](docs/mcp.md).

## Build

Requires a stable Rust toolchain (`rustup`; `rust-toolchain.toml` selects stable with
rustfmt and clippy).

```powershell
cargo build --workspace
cargo run -p kit-cli -- help
cargo xtask ci        # the gate: layer check, fmt, clippy -D warnings, tests
git config core.hooksPath .githooks   # once per clone: pre-commit runs layers + fmt check
```

Corpus tests read your own game install through `KITKRAFT_DATA_DIR` (set it for them; they
do not use detection) and are skipped without it; see [docs/corpus.md](docs/corpus.md).

## Layout

```
crates/   schema esm ba2 pex strings nif (L0)  doc (L1)  io (L3)  engine (L4)  mcp (L5)
apps/     kit-cli (kit), kit (kit-ui, the workbench)
xtask/    cargo xtask ci | layers | codegen | parity | dist
plan/     STATUS.md, execution-plan.md, adr/
docs/     architecture, mcp, parity checklists, gaps, corpus
corpus/   README only; the corpus lives in your game install
```

Contributors and agents: read [AGENTS.md](AGENTS.md) first.

## Clean room

Formats are implemented from the shared container described on UESP's (Skyrim) wiki pages,
Starfield record definitions informed by xEdit's MPL-2.0 `wbDefinitionsSF1.pas` (and the
helpers it uses in `wbDefinitionsCommon.pas`) at one pinned revision (used as reference only: our definitions are written independently and no xEdit
code is copied or linked), and validated by our own inspection of files we own and
black-box observation of the Creation Kit's saved output; the hierarchy is in
[ADR 0005](plan/adr/0005-spec-sources-per-fact.md) (Amendment 2) and
[ADR 0004](plan/adr/0004-xedit-source-policy.md). The Creation Kit is observed as a black
box only. No GPL Bethesda-format code (Mutagen, Spriggit) is read. No Bethesda data ships
in this repository.

## Policies (owner, 2026-10-14)

- **Runtime extensions** ([ADR 0008](plan/adr/0008-runtime-extensions.md)): SFSE and similar
  frameworks are supported where technically justified, behind their own boundary; the
  core crates never depend on them, and SFSE binaries are never bundled.
- **Development deployment** ([ADR 0009](plan/adr/0009-dev-deployment.md)): `package` stays
  target-independent; only an explicit `kit deploy --dev` writes into MO2, into one
  isolated KitKraft-managed mod, never into profiles or other mods.
- **Developer workbench** ([ADR 0007](plan/adr/0007-egui-shell-over-the-catalog.md),
  Amendment 1): a read-only egui workbench over the same commands as the CLI comes once
  the inspection APIs are stable; the full authoring UI is M5.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.

Not affiliated with or endorsed by Bethesda Softworks, ZeniMax Media, or Microsoft. See
[NOTICE](NOTICE).
