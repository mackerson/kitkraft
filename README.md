# KitCraft

An open-source, clean-room Creation Kit for Starfield, in Rust. *KitCraft is a working name.*

KitCraft reads and writes Starfield plugins (`.esm`/`.esp`/`.esl`), archives (`.ba2`), compiled
Papyrus (`.pex`) and string tables. Every action is one command, reachable the same way from
the `kit` CLI, from an MCP server for agents, and later from a desktop UI. Results carry their
evidence (byte offsets, coverage, limitations) so a person or an agent can check them.

## Status: pre-alpha

Nothing works yet. The repository is a scaffold (milestone M0). The first real goal (M1) is a
byte-identical roundtrip of every official Starfield ESM. See [ROADMAP.md](ROADMAP.md) and
[plan/STATUS.md](plan/STATUS.md).

## Command catalog (first cut)

Each line is a CLI subcommand, an MCP-reachable command, and a future menu item.

```
kit load  --profile "C:\Modding\MO2-Starfield\profiles\Default"   # or an explicit plugin list
kit inspect <FormID|EditorID> [--json]        # resolved record, winning override, all overriders
kit query  --type WEAP --where "EDID ~ Rifle" --json
kit set    <form> FULL="Old Soviet Marksman Rifle" --into MyPatch.esm
kit diff   A.esm B.esm [--form ...]           # record-level semantic diff
kit build  overhaul.toml -> Overhaul.esm      # data-driven patch compiler
kit validate <plugin>                         # missing masters, bad links, localized FULL copies,
                                              #   stripped FULL/NTRM, ...
kit roundtrip <esm> --verify                  # byte-identical rewrite, the M1 gate
kit ba2 {list,extract,pack}
kit papyrus {disasm,compile}                  # wraps Bethesda's compiler; never -release
kit strings {dump,merge}
kit mcp                                       # stdio MCP server over the same catalog
```

MCP tools: `execute`, `command_line`, `inspect_record`, `query_records`, `diff_plugins`
(`render` later). See [docs/mcp.md](docs/mcp.md).

## Build

Requires a stable Rust toolchain (`rustup`; `rust-toolchain.toml` selects stable with
rustfmt and clippy).

```powershell
cargo build --workspace
cargo run -p kit-cli -- help
cargo xtask ci        # the gate: layer check, fmt, clippy -D warnings, tests
git config core.hooksPath .githooks   # once per clone: pre-commit runs layers + fmt check
```

Corpus tests use your own game install and are skipped without it:

```powershell
$env:KITCRAFT_DATA_DIR = "C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data"
cargo test --workspace
```

## Layout

```
crates/   schema esm ba2 pex strings nif (L0)  doc (L1)  io (L3)  engine (L4)  mcp (L5)
apps/     kit-cli
xtask/    cargo xtask ci | layers
plan/     STATUS.md, execution-plan.md, adr/
docs/     architecture, mcp, parity checklists, gaps, corpus
corpus/   README only; the corpus lives in your game install
```

Contributors and agents: read [AGENTS.md](AGENTS.md) first.

## Clean room

Formats are implemented from the shared container described on UESP's (Skyrim) wiki pages,
Starfield record definitions informed by xEdit's MPL-2.0 `wbDefinitionsSF1.pas` at a pinned
revision (used as reference only: our definitions are written independently and no xEdit
code is copied or linked), and validated by our own inspection of files we own and
black-box observation of the Creation Kit's saved output; the hierarchy is in
[ADR 0005](plan/adr/0005-spec-sources-per-fact.md) (Amendment 2) and
[ADR 0004](plan/adr/0004-xedit-source-policy.md). The Creation Kit is observed as a black
box only. No GPL Bethesda-format code (Mutagen, Spriggit) is read. No Bethesda data ships
in this repository.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.

Not affiliated with or endorsed by Bethesda Softworks, ZeniMax Media, or Microsoft. See
[NOTICE](NOTICE).
