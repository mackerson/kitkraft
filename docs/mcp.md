# MCP server

`kit mcp` serves the engine command catalog over the Model Context Protocol on stdio. It is
an adapter: every tool resolves to `engine::execute`, the same path the CLI uses. The tool
count stays small on purpose; capability grows by adding commands, not tools.

Status: the server is planned (M2). The engine side it adapts exists since M1.11:
`kitcraft_engine::execute(id: &str, args: serde_json::Value) -> Result<Value, EngineError>`
returns the envelope below, and `kitcraft_engine::parse_line` is the `command_line`
grammar. This document is the contract the implementation follows.

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
| `inspect_record` | `{form}`                          | One record: winning version, every override, decoded fields, opaque ranges |
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
| 0    | Success, help, version                                                                    |
| 1    | The command ran and reported a negative result (`roundtrip --verify` mismatch; reserved)  |
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
