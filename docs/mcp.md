# MCP server

`kit mcp` serves the engine command catalog over the Model Context Protocol on stdio. It is
an adapter: every tool resolves to `engine::execute`, the same path the CLI uses. The tool
count stays small on purpose; capability grows by adding commands, not tools.

Status: the server is planned (M2). The engine side it adapts exists since M1.11:
`kitcraft_engine::execute(id: &str, args: serde_json::Value) -> Result<Value, EngineError>`
returns the envelope below, and `kitcraft_engine::parse_line` is the `command_line`
grammar. This document is the contract the implementation follows. Implemented commands
today: `roundtrip` (M1.12), `coverage` (M1.14) and a minimal single-plugin `inspect`; the
rest are `Planned`.

Front-end launchers (`kit mcp`, a future `kit ui`) start a front end; they are not actions
and are deliberately not `CommandSpec`s. Everything else a user can do is.

## Start it

```powershell
kit mcp                                   # empty session
kit mcp --profile "C:\Modding\MO2-Starfield\profiles\Default"   # preload a load order
```

Client config (any MCP client that launches stdio servers):

```json
{ "mcpServers": { "kitcraft": { "command": "C:\\path\\to\\kit.exe", "args": ["mcp"] } } }
```

## Tools

| Tool             | Arguments                         | Does                                                                       |
| ---------------- | --------------------------------- | -------------------------------------------------------------------------- |
| `execute`        | `{command, args}`                 | Run any catalog command by id with JSON args matching its `args_schema`    |
| `command_line`   | `{text}`                          | Run a CLI-style line, e.g. `"inspect 0001A2B3"`; same parser as `kit`      |
| `inspect_record` | `{form, plugin}`                  | One record: winning version, every override, decoded fields, opaque ranges (today: one plugin, header + opaque subrecords) |
| `query_records`  | `{type, where?, limit?, cursor?}` | Records of one type matching a field predicate; paged                      |
| `diff_plugins`   | `{a, b, form?}`                   | Record-level semantic diff between two plugins                             |
| `render`         | -                                 | Later (needs `crates/render`)                                              |

`inspect_record`, `query_records`, and `diff_plugins` are shortcuts for `execute` with
`inspect`, `query`, and `diff`; they exist because agents call them constantly.

Resources:

| URI                     | Contents                                                      |
| ----------------------- | ------------------------------------------------------------- |
| `kitcraft://commands`   | The catalog: id, label, aliases, summary, args_schema, status |
| `kitcraft://load-order` | Current session's plugins, masters, and indices               |

## Result envelope

Every successful tool result is one JSON object:

```json
{
  "command": "inspect",
  "data": { "...": "command-specific" },
  "coverage": {
    "status": "partial",
    "decoded_subrecords": 14,
    "opaque_subrecords": 2,
    "not_examined": []
  },
  "limitations": [
    "WEAP.WAM2 bytes 12..20 are opaque: no schema definition",
    "Starfield.esm is localized; FULL shown as string ID 0x0001F3A2 (strings not loaded)"
  ],
  "evidence": [
    { "file": "Starfield.esm", "offset": 123456789, "length": 38,
      "record": "WEAP:0001A2B3", "subrecord": "FULL" }
  ]
}
```

Rules:

- The five members are always present, in every result of every command.
- `coverage.status` is `complete`, `partial`, or `unknown`. `complete` only when every byte
  of every examined record was decoded. `decoded_subrecords` / `opaque_subrecords` are
  `null` when the command did not count them (never `0` for "not counted").
- `evidence[].record` and `evidence[].subrecord` are `null` when the range is not inside a
  record (e.g. a group header).
- A command whose handler is still a stub returns `data: null`, `coverage.status:
  "unknown"`, and `limitations: ["not implemented"]`; its catalog `status` is `Planned`.
- `limitations[]` is never omitted; an empty array is a claim of no known limitation.
- Every value read from a plugin is traceable to an `evidence` entry: file name, absolute
  byte offset, length, record, subrecord.
- Unknown is reported as unknown. A field missing from the schema is never shown as absent
  from the record.
- Writes (`set`, `build`) report the target file, the records changed, and nothing else
  touched.
- `data.passed` (boolean) is present on commands that run a check (`roundtrip` with
  `verify`, later `validate`). `false` means the command ran and the check failed; `kit`
  exits 1. `null` or absent means no pass/fail claim was made.
- `data.summary` (string, optional) is a one-line human summary; `kit` prints it instead
  of the `data` JSON in text mode.

## Command results

### `roundtrip` (M1.12)

Arguments `{plugin, verify = false, out?}`. Streams the plugin through the `kitcraft-esm`
driver one top-level group at a time, rewrites it, and compares the output with the input.

```json
{
  "command": "roundtrip",
  "data": {
    "plugin": "C:\\...\\Constellation.esm", "identical": true, "verify": true,
    "passed": true, "bytes": 39210, "bytes_compared": 39210, "records": 57,
    "groups": 7, "compressed_records": 0, "elapsed_ms": 0, "out": null,
    "mismatch": null,
    "summary": "Constellation.esm: identical (39210 bytes, 57 records, 7 groups, 0 compressed records) in 0 ms"
  },
  "coverage": { "status": "complete", "decoded_subrecords": null,
                "opaque_subrecords": null, "not_examined": [] },
  "limitations": ["container level: every group, record and subrecord header is parsed and rewritten; subrecord payloads are compared as opaque bytes and compressed record bodies are re-emitted as read, not decompressed"],
  "evidence": [{ "file": "Constellation.esm", "offset": 0, "length": 39210,
                 "record": null, "subrecord": null }]
}
```

- `passed` is `identical` when `verify` is set, `null` otherwise.
- On a mismatch: `mismatch: {offset, expected, actual, path}` (`expected`/`actual` are byte
  values, `null` past the end of the data; `path` is the driver's
  `GRUP WEAP (type 0) @123 > WEAP 0001A2B3 @147 > FULL @171`), one `evidence` entry of
  length 1 at `offset` with the record (`WEAP:0001A2B3`) and subrecord taken from the path,
  `coverage.status: "partial"`, the unread byte range in `not_examined`, and a limitation
  saying the counts stop at the mismatch. The counts cover only what was read.
- `out` writes the rebuilt bytes. It is refused when it names the input file; a failed
  roundtrip removes its partial output; after a mismatch the file stops at the top-level
  entry that differs (stated as a limitation).

### `inspect` (minimal)

Arguments `{form, plugin}`; CLI `kit inspect <form> <plugin>`. `form` is a FormID (1-8 hex
digits, optional `0x`, matched against the record header as stored in the file) or an
EditorID (ASCII case-insensitive). A hex form that matches no FormID is retried as an
EditorID. Not found is a `failed` error.

`data`: `plugin`, `form`, `matched_by` (`form_id` | `editor_id`), `record` (`signature`,
`form_id`, `editor_id`, `flags` as `0x%08X`, `compressed`, `offset`, `data_size`,
`timestamp`, `version_control`, `form_version`, `unknown`, `groups` = enclosing groups
outermost first as `{offset, type, label}`), `subrecord_offsets` (`file`, or
`decompressed` for a compressed record), and `subrecords[]` with `signature`, `size`,
`oversized` (written with an `XXXX` prefix), `offset` (start of the subrecord, including
any `XXXX` prefix), `data_offset`, `payload_hex` (first 256 bytes, uppercase hex) and
`payload_truncated`. Evidence: the record header, then each payload (or, for a compressed
record, the compressed body). Coverage is `partial`; every subrecord counts as opaque.
Limitations always include `typed decode pending schema wiring` and the single-plugin
note.

### `coverage` (M1.14)

Arguments `{plugin, decode = true}`; CLI `kit coverage <plugin> [--no-decode]`. Streams the
plugin and classifies every subrecord occurrence against the builtin schema by its record
type and signature: `typed` (the type's definition names it with a layout), `opaque`
(named, kept as raw bytes) or `unknown` (the type has no definition, or the definition does
not name the signature; subrecords inside an `any_until` run count here). Payloads are not
decoded against the layouts. With `decode` (the default) compressed records are
decompressed under the default `DecodeLimits` and counted; one that fails to decompress is
listed in `coverage.not_examined` (first 20, then a count), not fatal.

`data`: `plugin`, `file`, `bytes`, `decode`, `records`, `groups`, `compressed_records`,
`compressed_examined`, `record_types_seen`, `record_types_defined`, `subrecords`
(occurrences), `occurrences {typed, opaque, unknown}`, `summary`, and `record_types[]`
sorted by record count (then signature), each with `signature`, `definition` (the schema
name or `null`), `records`, `compressed`, `compressed_examined`, `subrecords`,
`occurrences`, `distinct {seen, typed, opaque, unknown}` (distinct signatures),
`first_offset`, and `signatures[]` (`signature`, `count`, `class`, `members` = member
paths in the definition, `first_record` = `SIG:FORMID` of the first record holding it).

Coverage: `decoded_subrecords` is the `typed` occurrence count, `opaque_subrecords` the
`opaque` plus `unknown` count; `status` is `complete` only when both of those are zero and
every compressed record was examined. Evidence: the whole file, then the first record of
each type. Limitations always state that classification is by signature only; with
`--no-decode` they add that compressed bodies were skipped (also in `not_examined`).
`cargo xtask parity` runs this command over the corpus to write `docs/record-parity.md`.

### `ba2.list`, `ba2.extract`, `ba2.pack` (M2.G4)

Archives are read through `kitcraft-ba2` (GNRL v2/v3, DX10 v2/v3; `docs/corpus.md`
"Archives"). Patterns are regular expressions matched case-insensitively against archive
paths (`scripts/form.pex`). No command writes under a game `Data` directory (a directory
holding `Starfield.esm`, or `KITCRAFT_DATA_DIR`): such an `out_dir` / `out` is a `failed`
error before anything is written. A bad pattern is `invalid_args` (`rule: "pattern"`).

- `ba2.list {archive, filter?, limit = 1000, start = 0}`; CLI `kit ba2 list <archive>`.
  `data`: `archive`, `version`, `kind` (`gnrl`/`dx10`), `codec` (`zlib`/`lz4-block`),
  `compression_field`, `file_count`, `bytes`, `has_names`, `filter`, `matched`, `start`,
  `shown`, `summary`, and `entries[]` with `index`, `name`, `size` (bytes extraction
  produces; for a texture the synthesized DDS header included), `stored`, `compressed`,
  `record_offset`, `data_offset`, and for textures `texture {width, height, mips,
  dxgi_format, cubemap, chunks}`. Evidence: the header, the record table and the name
  table. A page shorter than `matched` adds a limitation.
- `ba2.extract {archive, out_dir, pattern?, overwrite = false}`; CLI
  `kit ba2 extract <archive> <out_dir> [--pattern re]`. Every target is checked first: an
  existing file without `overwrite` fails with nothing written. Each file is streamed and
  checked against its declared size; a failure removes the partial file. `data`:
  `archive`, `out_dir`, `pattern`, `extracted`, `bytes`, `files[]` (first 1,000: `index`,
  `name`, `size`, `path`), `summary`. DX10 entries get a DDS header byte-identical to
  `Archive2.exe` 1.2.0.1's extraction of the official archives (stated as a limitation).
- `ba2.pack {dir, out, format = "gnrl", version = 2, compress = true}`; CLI
  `kit ba2 pack <dir> <out> [--format dx10] [--version 3]`. `dir` is the Data root:
  `dir\scripts\x.pex` is stored as `scripts/x.pex` (lower-cased, `/`). Version 2 packs
  with zlib, 3 with LZ4 blocks; a blob is stored when compression does not shrink it. The
  same inputs give the same bytes. `dx10` takes only `.dds` files and needs pixel data of
  exactly the mip chain's size; DDS headers are not stored (extraction synthesizes them,
  a limitation). `out` may not be inside `dir`; it is written as `<out>.partial` and
  renamed. `data`: `dir`, `out`, `format`, `version`, `compress`, `packed`, `bytes`,
  `files[]` (first 1,000: `name`, `size`, `stored`), `summary`; evidence: the whole
  archive.

### `papyrus.disasm`, `papyrus.compile` (M2.G6, M2.G7)

- `papyrus.disasm {pex, archive?, format = "text"}`; CLI `kit papyrus disasm <pex>`
  (alias `pexdis`). With `archive`, `pex` is a path inside that `.ba2`. `format`: `text`
  (`pexdis.py`'s format), `json` (the structured model) or `both`. `data`: `pex`,
  `archive`, `bytes`, `version`, `game_id`, `source`, `objects`, `functions`,
  `instructions`, `text` and/or `script` (header, string count, debug block summary,
  user flags, objects with structs, variables, guards, properties, property functions
  and states; every function with params, locals and instructions with `offset`,
  `opcode`, `op`, `args` / `varargs` as `{type, value, text}`), and `summary` = the text
  without its final newline, so `kit papyrus disasm x.pex` prints what `pexdis.py x.pex`
  prints. Structs are in the model, not the text (a limitation when present).
- `papyrus.compile {source, out_dir, imports?, flags?, compiler?, optimize = false}`; CLI
  `kit papyrus compile <source> <out_dir> --imports <dir> ...`. Runs Bethesda's
  `PapyrusCompiler.exe` (black box) on one script by name: a `.psc` path becomes its name
  with its folder imported first; `.ppj` projects are refused. Arguments are only the
  object, `-f=`, `-i=`, `-o=` and `-op`; release and final mode can never be requested
  (any extra argument such as `release` is `invalid_args`). The flags file defaults to
  `Starfield_Papyrus_Flags.flg` from the first import that has it. The base sources are
  the `Scripts\Source` folder of `Tools\ContentResources.zip`, extracted by the caller.
  `data`: `source`, `script`, `out_dir`, `compiler`, `args`, `exit_code`, `passed` (the
  compiler's success; `kit` exits 1 when false), `stdout`, `stderr`, `pex` (`path`,
  `bytes`, `parsed`, `text`: the output disassembled through `kitcraft-pex`) and
  `summary`. A missing compiler is a `failed` error naming every path looked at
  (`compiler`, `KITCRAFT_PAPYRUS_COMPILER`, `KITCRAFT_DATA_DIR`'s game folder, the Steam
  default).

### `strings.dump`, `strings.merge` (M2.G9)

- `strings.dump {table, archive?, id?, contains?, limit = 100, start = 0}`; CLI
  `kit strings dump <table> [--archive a.ba2] [--id 0x0002E469]`. `table` is a
  `.strings`/`.dlstrings`/`.ilstrings` file, or with `archive` its path inside the `.ba2`
  (`strings/starfield_en.strings` in `Starfield - Localization.ba2`). `id` is `0x` hex or
  decimal; `contains` filters text case-insensitively. `data`: `table`, `archive`, `kind`,
  `count`, `matched`, `start`, `shown`, `summary`, `entries[]` with `id` (`0x%08X`),
  `text` (UTF-8, lossy), `utf8`, `offset` and `length` in the table file, and `bytes_hex`
  when not valid UTF-8. Evidence: each shown string's bytes in the table file (a
  limitation says so when the table came from an archive).
- `strings.merge {tables, out}`: tables of one kind; a later table wins for an ID; the
  output is sorted by ID and written in the official layout, so the same inputs give the
  same bytes. `out` must have the inputs' extension, may not be an input, and is never
  under the game Data directory. `data`: `tables`, `out`, `kind`, `entries`, `bytes`,
  `conflict_count`, `conflicts[]` (first 100: `id`, `replaced`, `winner`, `old`, `new`),
  `identical_repeats`, `summary`.

## Errors

`execute` fails with a typed `EngineError`; `kind()` is the stable string front ends
report (`kit --json` prints `{"error": {"kind", "message", "violations"?}}`).

| `kind`            | When                                                              | MCP mapping       |
| ----------------- | ----------------------------------------------------------------- | ----------------- |
| `unknown_command` | No catalog id or alias matches                                    | `-32602`          |
| `usage`           | `command_line` text did not parse (unknown flag, bad value)       | `-32602`          |
| `invalid_args`    | Arguments break `args_schema`; carries every `{pointer, rule, message}` | `-32602`    |
| `not_implemented` | Catalog command with no handler yet                               | `isError: true`   |
| `failed`          | The command ran and failed                                        | `isError: true`   |
| `bad_schema`, `registration`, `serialize` | Engine bugs (catalog or handler table)    | `-32603`          |

Argument validation implements the JSON Schema subset the catalog uses: `type`, `enum`,
`properties`, `required`, `additionalProperties`, `items`, `minItems`/`maxItems`,
`minLength`/`maxLength`, `minimum`/`maximum`; `default`, `description`, `title`,
`examples`, `$schema` are annotations. Any other keyword in a catalog schema is refused
(a test enforces it), so a schema can never silently accept what it claims to reject.
Top-level `default`s are filled in before the handler runs; `null` arguments mean `{}`.

- Invalid arguments: JSON-RPC error `-32602` with the failing JSON pointer and the schema rule.
- Command failures (unreadable file, parse error, missing master): a tool result with
  `isError: true` and a message that includes the byte offset when the failure is in a
  plugin.
- The server never panics on input; a malformed plugin is an error result.
- Commands never prompt. Anything that would need a dialog in the UI is an argument here.

## `command_line` grammar (shared with `kit`)

`kitcraft_engine::parse_line` / `parse_words` turn words into a command id and JSON
arguments, generated from each `args_schema`; `kit` calls the same parser.

- Command words: `roundtrip`, `ba2 list` (or `ba2.list`), or an alias.
- Required string/number arguments are positional in the schema's `required` order.
- Every argument is also `--name value` / `--name=value` (`_` may be written `-`).
- Booleans: `--verify`, `--no-verify`, `--verify=false`. String lists repeat the flag.
  Object arguments take JSON text: `--fields '{"FULL":"Rifle"}'`. `--` ends flags.
- `help`, `help <command>`, `<command> --help` return help instead of running.
- Quotes (`'` or `"`) group words; there are no backslash escapes, so Windows paths are
  written as-is.

`roundtrip.verify` defaults to `false`: plain `roundtrip` parses and rewrites, `--verify`
also demands byte-identical output.

## CLI exit codes

| Code | Meaning                                                                                   |
| ---- | ----------------------------------------------------------------------------------------- |
| 0    | Success, help, version (includes `roundtrip` without `--verify` that found a difference)  |
| 1    | The command ran and its check failed: `data.passed` is `false` (`roundtrip --verify` mismatch) |
| 2    | Usage error, unknown command, invalid arguments, not implemented (including a `Planned` command's stub result), command failure, `kit mcp` before M2 |

With `--json`, stdout carries exactly the `execute` envelope (or the error object), and
human messages go to stderr.

## A typical session

1. `execute {command: "load", args: {profile: "C:\\Modding\\MO2-Starfield\\profiles\\Default"}}`
2. `query_records {type: "WEAP", where: "WDMG.Damage > 100", limit: 20}`
3. `inspect_record {form: "0001A2B3"}`: see the winning override and which plugin set it.
4. `execute {command: "set", args: {form: "0001A2B3", fields: {"FULL": "Old Soviet Marksman Rifle"}, into: "MyPatch.esm"}}`
5. `execute {command: "validate", args: {plugin: "MyPatch.esm"}}`: catches a localized FULL
   copied as a string ID before the game does.
