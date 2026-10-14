# MCP server

`kit mcp` serves the engine command catalog over the Model Context Protocol on stdio. It is
an adapter: every tool resolves to `engine::execute`, the same path the CLI uses. The tool
count stays small on purpose; capability grows by adding commands, not tools.

Status: implemented (M2.F2..F6): `crates/mcp` (`server.rs` protocol, `tools.rs` tool
shapes from the catalog, `backend.rs` calls onto the engine) and the `kit mcp` launcher.
Every tool resolves to `Engine::execute` on the server's one engine, so the server keeps
one `Session` for its process: a `load` is seen by every later call. Which commands do
real work is the catalog's `status` (`kit help`, or the `kitcraft://commands` resource);
the rest answer with a `not_implemented` or `deferred` tool error.

Front-end launchers (`kit mcp`, a future `kit ui`) start a front end; they are not actions
and are deliberately not `CommandSpec`s. Everything else a user can do is.

## Start it

```powershell
kit mcp                                   # empty session
kit mcp --profile "C:\Modding\MO2-Starfield\profiles\Default"   # preload a load order
```

`--profile` runs the `load` command before serving. If it fails (until M2.D8 lands it
always does: `load` is not implemented yet) the error goes to stderr and into the
server's `instructions`, and the server starts with an empty session instead of exiting.
`kit mcp` exits 0 when its stdin closes and 2 on a stdio error. stdout carries only MCP
messages; logs go to stderr.

Client config (any MCP client that launches stdio servers):

```json
{ "mcpServers": { "kitcraft": { "command": "C:\\path\\to\\kit.exe", "args": ["mcp"] } } }
```

## Use from Claude Code

1. Build the release binary: `cargo build --release -p kit-cli` (gives
   `C:\Modding\kitcraft\target\release\kit.exe` with the default target dir).
2. Either copy `.mcp.json.example` at the repo root to `.mcp.json` (project scope; the
   file is gitignored, never committed; adjust the paths) and approve the server when
   Claude Code asks, or register it for yourself only:

   ```powershell
   claude mcp add --scope local kitcraft -- C:\Modding\kitcraft\target\release\kit.exe mcp --profile "C:\Modding\MO2-Starfield\profiles\Default"
   ```

3. Restart Claude Code (or start a new session). `/mcp` should list `kitcraft` as connected
   with 7 tools; Claude Code names them `mcp__kitcraft__execute`, `mcp__kitcraft__coverage`
   and so on.
4. Headless check without the UI:

   ```powershell
   claude -p "Use the kitcraft tools: run coverage on <Data>\Constellation.esm, then roundtrip it with verify, and show the envelopes" --mcp-config .mcp.json --allowedTools "mcp__kitcraft__*"
   ```

Rebuild `kit.exe` and restart the session after a change to the catalog or the server;
Claude Code reads the tool list at connect time.

## Protocol

JSON-RPC 2.0 over stdio, one message per line (no embedded newlines), UTF-8. Hand-written
over `serde_json` (`crates/mcp/src/server.rs`): the official Rust SDK `rmcp` (3.5.1,
Apache-2.0) needs `tokio`, `tokio-util`, `futures`, `chrono` and `tracing` for what is a
single-threaded line loop here, and the dual-era handling below would still be ours.

The server implements MCP revision **2026-07-28** and, for clients that still do the
handshake, the earlier revisions **2025-11-25** and **2025-06-18** (a "dual-era" server in
the 2026-07-28 versioning terms). `2025-03-26` and `2024-11-05` are not offered: they
require JSON-RPC batches, which this server refuses (`-32600`), so a client asking for
them is answered with `2025-11-25`. `server/discover`'s `supportedVersions` and a
`-32022` error's `supported` list every revision the process serves, newest first
(`2026-07-28`, `2025-11-25`, `2025-06-18`), so a client can pick a handshake-era one.

- **Modern requests** carry `params._meta["io.modelcontextprotocol/protocolVersion"]`
  (must be `2026-07-28`, else error `-32022` with `data: {supported, requested}`) and
  `["io.modelcontextprotocol/clientCapabilities"]` (missing: `-32602`). No handshake.
  `server/discover` returns `supportedVersions`, `capabilities`, `instructions`. Every
  result carries `resultType: "complete"` and `_meta["io.modelcontextprotocol/serverInfo"]`;
  list, read and discover results carry `ttlMs`/`cacheScope` (the load-order resource:
  `0`/`private`). `initialize` and `ping` are not methods of this revision: a modern
  request for either is `-32601`.
- **Legacy requests** (no version in `_meta`, or a handshake-era one): `initialize`
  answers with the client's `protocolVersion` when it is one of the two above, otherwise
  `2025-11-25`, plus `capabilities`, `serverInfo` and `instructions`.
  `notifications/initialized` and `ping` work; requests before `initialize` are served
  too.
- Methods: `server/discover`, `initialize`, `ping`, `tools/list`, `tools/call`,
  `resources/list`, `resources/read`, `resources/templates/list` (empty). Capabilities:
  `tools` and `resources`, no `listChanged`, no subscriptions, one page (any `cursor` is
  `-32602`). Notifications (`notifications/cancelled` included) are accepted and never
  answered; requests run one at a time, so a cancel cannot interrupt a running command.
- Malformed JSON or non-UTF-8: `-32700`. Not a request object (batch arrays, `id` that is
  `null`, a float, a bool or an object, wrong `jsonrpc`), or a line over 8 MiB (discarded
  without buffering): `-32600`. Unknown method, or one the request's era lacks: `-32601`.
  `params` not an object: `-32602`.

The session is process state, which the 2026-07-28 statelessness rule discourages (state
across requests should be named by an explicit handle). It is correct for handshake-era
clients, whose session is the stdio process; giving `load` a session handle is an open
question for M3.

## Tools

| Tool             | Arguments                                 | Does                                                                  |
| ---------------- | ----------------------------------------- | --------------------------------------------------------------------- |
| `execute`        | `{command, args?}`                        | Run any catalog command by id (or alias) with JSON args matching its `args_schema` |
| `command_line`   | `{text}`                                  | Run a CLI-style line, e.g. `"inspect 0001A2B3 X.esm"`; same parser as `kit`; `help ...` returns usage text |
| `inspect_record` | `inspect`'s args (`{form, plugin}`)       | One record: header fields and subrecords (later: winner, overrides, typed fields) |
| `query_records`  | `query`'s args (`{type, where?, limit?, cursor?}`) | Records of one type matching a field predicate; paged        |
| `diff_plugins`   | `diff`'s args (`{a, b, form?}`)           | Record-level semantic diff between two plugins                        |
| `coverage`       | `coverage`'s args (`{plugin, decode?, signatures?, top?}`) | Record types and subrecords vs the schema; a summary by default over MCP |
| `roundtrip`      | `roundtrip`'s args (`{plugin, verify?, out?, overwrite?}`) | Parse and rewrite a plugin; `verify` demands identical bytes |
| `render`         | -                                         | Later (needs `crates/render`)                                         |

Tool shapes are generated from the catalog (`tools.rs`): a shortcut's `inputSchema` is its
command's `args_schema` verbatim and its description is the command's `summary` (plus
`planned: ...` or `deferred to M3: ...` when not implemented); `execute`'s `command` enum
and its description (one line per command with its argument names) list the catalog as
it is. Adding or changing a command needs no edit in `crates/mcp`; tests enforce both.
Shortcuts that never write carry `readOnlyHint: true` and `destructiveHint: false`; `execute`,
`command_line` (which reach every writing command) and `roundtrip` (whose `out` replaces an
existing file with `overwrite: true`) carry `readOnlyHint: false`, `destructiveHint: true`.

Every successful call returns the envelope as `structuredContent` and as JSON text in
one `text` content block (the same JSON). The text is ordered `command`, `data` with
`data.summary` first, then the other members, so a client that shows or truncates the
text leads with the one-line summary. `command_line` with `help` returns
`{"help": text}`. There is no `outputSchema` (error results have a different shape).

Size defaults (`MCP_DEFAULTS` in `backend.rs`): arguments that only decide how much a
result lists default differently over MCP than from `kit`, on every route (`execute`,
`command_line`, the shortcuts), and only when the caller leaves them out. Today:
`coverage` gets `signatures: false` and `top: 20`. On `Starfield.esm` that is an 18 KB
envelope instead of 695 KB (the envelope is sent twice, as `structuredContent` and as
text); pass `signatures: true` and a larger `top` (or omit nothing) for the full listing.

Resources:

| URI                     | Contents                                                      |
| ----------------------- | ------------------------------------------------------------- |
| `kitcraft://commands`   | The catalog: id, label, menu_path, shortcut, aliases, summary, args_schema (parsed), status (`implemented`/`planned`/`deferred`), deferred_reason, deferred_to |
| `kitcraft://load-order` | The session's load-order listing as `load` reported it (`Session::load_order`); `{"loaded": false, "plugins": [], ...}` before any load |

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

Arguments `{plugin, verify = false, out?, overwrite = false}`. Streams the plugin through the `kitcraft-esm`
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
- `out` writes the rebuilt bytes to a new file: an existing file is a `failed` error and is
  left untouched unless `overwrite` is `true` (`kit roundtrip X.esm --out Y.esm
  --overwrite`). It is refused when it names the input file, even with `overwrite`; a
  failed roundtrip removes its partial output; after a mismatch the file stops at the
  top-level entry that differs (stated as a limitation).

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

Arguments `{plugin, decode = true, signatures?, top?}`; CLI `kit coverage <plugin>
[--no-decode] [--no-signatures] [--top N]`. Streams the
plugin and classifies every subrecord occurrence against the builtin schema by its record
type and signature: `typed` (the type's definition names it with a layout), `opaque`
(named, kept as raw bytes) or `unknown` (the type has no definition, or the definition does
not name the signature; subrecords inside an `any_until` run count here). Payloads are not
decoded against the layouts. With `decode` (the default) compressed records are
decompressed under the default `DecodeLimits` and counted; one that fails to decompress is
listed in `coverage.not_examined` (first 20, then a count), not fatal.

Size controls, which never change the totals: `signatures: false` sets each type's
`signatures` to `null`; `top: N` lists only the N types with the most records (`0` lists
none) and adds a limitation naming how many of how many are listed. Omitted, `kit` (and
`cargo xtask parity`) list everything; over MCP they default to `false` and `20`.

`data`: `plugin`, `file`, `bytes`, `decode`, `records`, `groups`, `compressed_records`,
`compressed_examined`, `record_types_seen`, `record_types_defined`, `subrecords`
(occurrences), `occurrences {typed, opaque, unknown}`, `signatures` and `top` (as
applied), `record_types_listed`, `summary`, and `record_types[]`
sorted by record count (then signature), each with `signature`, `definition` (the schema
name or `null`), `records`, `compressed`, `compressed_examined`, `subrecords`,
`occurrences`, `distinct {seen, typed, opaque, unknown}` (distinct signatures),
`first_offset`, and `signatures[]` (`signature`, `count`, `class`, `members` = member
paths in the definition, `first_record` = `SIG:FORMID` of the first record holding it;
`null` with `signatures: false`).

Coverage: `decoded_subrecords` is the `typed` occurrence count, `opaque_subrecords` the
`opaque` plus `unknown` count; `status` is `complete` only when both of those are zero and
every compressed record was examined. Evidence: the whole file, then the first record of
each listed type. Limitations always state that classification is by signature only; with
`--no-decode` they add that compressed bodies were skipped (also in `not_examined`).
`cargo xtask parity` runs this command over the corpus to write `docs/record-parity.md`.

## Errors

`execute` fails with a typed `EngineError`; `kind()` is the stable string front ends
report (`kit --json` prints `{"error": {"kind", "message", "violations"?}}`).

| `kind`            | When                                                              | MCP mapping       |
| ----------------- | ----------------------------------------------------------------- | ----------------- |
| `unknown_command` | No catalog id or alias matches                                    | `isError: true`   |
| `usage`           | `command_line` text did not parse (unknown flag, bad value)       | `isError: true`   |
| `invalid_args`    | Arguments break `args_schema`; carries every `{pointer, rule, message}` | `isError: true` |
| `not_implemented` | Catalog command with no handler yet                               | `isError: true`   |
| `deferred`        | Catalog command deferred to a later milestone (`Status::Deferred`); the message has the reason | `isError: true` |
| `failed`          | The command ran and failed                                        | `isError: true`   |
| `bad_schema`, `registration`, `serialize` | Engine bugs (catalog or handler table)    | `-32603`          |
| (none)            | `tools/call` names no tool, lacks `name`, or `arguments` is not an object | `-32602`  |

An `isError: true` result carries `structuredContent: {"error": {"kind", "message",
"violations"?}}` (the shape `kit --json` prints) and the same JSON as text. Argument
errors are tool results rather than JSON-RPC errors so the model sees the pointer and
rule and can fix its call; MCP 2026-07-28 ("Tools", "Error Handling") classes input
validation as a tool execution error. `execute` and `command_line` validate their own
`{command, args}` / `{text}` with the same validator (`invalid_args` names the tool).
`execute` checks `command` first: an id or alias that is not in the catalog is
`unknown_command` whatever else is wrong with the call (never an `enum` violation listing
the catalog), and `args: null` is taken as `{}` before validation.

Argument validation implements the JSON Schema subset the catalog uses: `type`, `enum`,
`properties`, `required`, `additionalProperties`, `items`, `minItems`/`maxItems`,
`minLength`/`maxLength`, `minimum`/`maximum`; `default`, `description`, `title`,
`examples`, `$schema` are annotations. Any other keyword in a catalog schema is refused
(a test enforces it), so a schema can never silently accept what it claims to reject.
Top-level `default`s are filled in before the handler runs; `null` arguments mean `{}`
(`execute`'s `args: null` included).

- Invalid arguments: an `isError` result with the failing JSON pointer and the schema rule.
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
| 2    | Usage error, unknown command, invalid arguments, not implemented (including a `Planned` command's stub result), deferred, command failure; `kit mcp` on a stdio I/O error |

`kit mcp` exits 0 when its stdin closes, including after a failed `--profile` preload.

With `--json`, stdout carries exactly the `execute` envelope (or the error object), and
human messages go to stderr.

## A typical session

1. `execute {command: "load", args: {profile: "C:\\Modding\\MO2-Starfield\\profiles\\Default"}}`
2. `query_records {type: "WEAP", where: "WDMG.Damage > 100", limit: 20}`
3. `inspect_record {form: "0001A2B3"}`: see the winning override and which plugin set it.
4. `execute {command: "set", args: {form: "0001A2B3", fields: {"FULL": "Old Soviet Marksman Rifle"}, into: "MyPatch.esm"}}`
5. `execute {command: "validate", args: {plugin: "MyPatch.esm"}}`: catches a localized FULL
   copied as a string ID before the game does.
