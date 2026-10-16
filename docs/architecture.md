# Architecture

KitKraft is a Rust workspace arranged as a strict layer stack (ADR 0002). Bytes enter at L0,
become a resolved document at L1, are acted on by commands at L4, and reach people and agents
through thin front ends at L5 and in `apps/`.

## Layers

```
            apps/kit-cli (kit)          apps/kit (later)
                    |                          |
L5        mcp ------+                    ui-egui (later)
                    |                          |
L4               engine   CommandSpec catalog, execute(json) -> json, sessions, undo, validation
                    |
L3                 io     MO2 profile, plugins.txt, Creations manifests -> load order
                    |
L1                 doc    load order, FormID resolution, overrides, conflict graph, strings
                    |
L0   schema <- esm   ba2   pex   strings   nif
```

Arrows point down only. The single same-layer edge is `esm -> schema`. There is no L2: the
viewport is L5 view code inside `ui-egui` (`src/viewport/`), fed by engine commands (ADR 0002
Amendment 1, carried out by M7.R1).

| Crate     | Layer | Owns                                                                            | Status       |
| --------- | ----- | ------------------------------------------------------------------------------- | ------------ |
| `schema`  | L0    | Record definitions (TOML), loader, validator, codegen library                   | stub         |
| `esm`     | L0    | TES4 container parse/write, compression, opaque preservation, generated records | stub         |
| `ba2`     | L0    | Archive read/write                                                              | stub         |
| `pex`     | L0    | Papyrus bytecode read/disassemble                                               | stub         |
| `strings` | L0    | Localized string tables                                                         | stub         |
| `nif`     | L0    | Mesh/material metadata                                                          | stub         |
| `doc`     | L1    | `LoadOrder`, FormID resolution, override chains, conflicts                      | stub         |
| `io`      | L3    | Mod-manager and game config readers                                             | stub         |
| `engine`  | L4    | `CommandSpec` catalog (first cut written), execution, evidence envelope         | catalog only |
| `mcp`     | L5    | `server.rs`, `tools.rs`, `backend.rs`                                           | stub         |
| `kit-cli` | APP   | `kit` binary, one `main.rs`                                                     | help only    |
| `xtask`   | TOOL  | `ci`, `layers` (implemented); `codegen`, `parity` (M1)                          | layers done  |

## Data flow

```
plugin bytes (file, read-only)
  -> esm::Plugin           exact tree: header, groups, records, subrecords (raw or compressed),
                           every header field kept verbatim, absolute offsets recorded
  -> typed view            generated record structs from schema defs; unknown subrecords opaque
  -> doc::LoadOrder        masters resolved, FormIDs normalized, override chains, winners
  -> engine command        execute(id, json args) -> json result with evidence envelope
  -> front end             CLI text or --json, MCP tool result, later UI panes
```

Write path: a command modifies records in a target plugin model -> `esm` serializes,
recomputing sizes only along modified paths -> untouched data is emitted exactly as read.

## Principles

- **Preserve before understand.** The container never needs a schema to roundtrip. Decoding
  is a view over bytes we keep.
- **Unknown is not absent.** Every result says what was decoded, what was opaque, and what
  was not looked at (`coverage`, `limitations[]`).
- **Evidence by offset.** Values read from a plugin carry `(file, offset, length)` so claims
  can be checked with a hex editor.
- **One path per action.** CLI, MCP, scripts, and UI all call `engine::execute`. Commands
  never open dialogs.
- **Hostile input.** Parsers are bounded and checked; no panics (see AGENTS.md).
- **Stream large files.** `Starfield.esm` is about 1.46 GB; the file driver works one
  top-level group at a time. No `mmap` (it needs `unsafe`, which is forbidden).

## Key types (planned)

| Type                                | Crate  | Notes                                                         |
| ----------------------------------- | ------ | ------------------------------------------------------------- |
| `esm::Plugin`                       | esm    | Header record + top-level groups; source path and file length |
| `esm::Group`, `Record`, `Subrecord` | esm    | Raw header fields, children/body, absolute source offset      |
| `esm::Body`                         | esm    | `Raw(Vec<Subrecord>)` or `Compressed { original, decoded }`   |
| `schema::RecordDef`                 | schema | Signature, name, source, ordered subrecord defs               |
| `doc::LoadOrder`                    | doc    | Plugins in order, master maps, override index                 |
| `engine::CommandSpec`               | engine | id, label, menu_path, shortcut, aliases, args_schema, status  |
| `engine::Envelope`                  | engine | `data`, `limitations`, `coverage`, `evidence`                 |
