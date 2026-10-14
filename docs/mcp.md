# MCP server

`kit mcp` serves the engine command catalog over the Model Context Protocol on stdio. It is
an adapter: every tool resolves to `engine::execute`, the same path the CLI uses. The tool
count stays small on purpose; capability grows by adding commands, not tools.

Status: planned (M2). This document is the contract the implementation follows.

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

- `coverage.status` is `complete`, `partial`, or `unknown`. `complete` only when every byte
  of every examined record was decoded.
- `limitations[]` is never omitted; an empty array is a claim of no known limitation.
- Every value read from a plugin is traceable to an `evidence` entry: file name, absolute
  byte offset, length, record, subrecord.
- Unknown is reported as unknown. A field missing from the schema is never shown as absent
  from the record.
- Writes (`set`, `build`) report the target file, the records changed, and nothing else
  touched.

## Errors

- Invalid arguments: JSON-RPC error `-32602` with the failing JSON pointer and the schema rule.
- Command failures (unreadable file, parse error, missing master): a tool result with
  `isError: true` and a message that includes the byte offset when the failure is in a
  plugin.
- The server never panics on input; a malformed plugin is an error result.
- Commands never prompt. Anything that would need a dialog in the UI is an argument here.

## A typical session

1. `execute {command: "load", args: {profile: "C:\\Modding\\MO2-Starfield\\profiles\\Default"}}`
2. `query_records {type: "WEAP", where: "WDMG.Damage > 100", limit: 20}`
3. `inspect_record {form: "0001A2B3"}`: see the winning override and which plugin set it.
4. `execute {command: "set", args: {form: "0001A2B3", fields: {"FULL": "Old Soviet Marksman Rifle"}, into: "MyPatch.esm"}}`
5. `execute {command: "validate", args: {plugin: "MyPatch.esm"}}`: catches a localized FULL
   copied as a string ID before the game does.
