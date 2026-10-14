# MCP server

`kit mcp` serves the engine command catalog over the Model Context Protocol on stdio. It is
an adapter: every tool resolves to `engine::execute`, the same path the CLI uses. The tool
count stays small on purpose; capability grows by adding commands, not tools.

Status: implemented (M2.F2..F6): `crates/mcp` (`server.rs` protocol, `tools.rs` tool
shapes from the catalog, `backend.rs` calls onto the engine) and the `kit mcp` launcher.
Every tool resolves to `Engine::execute` on the server's one engine, so the server keeps
one `Session` for its process: a `load` is seen by every later call. Every catalog
command is `Implemented` (M2.F8: the `catalog_closure` test fails on a `Planned` one); a
command that does only part of its job says what it does not do in `limitations[]`. A
`Deferred` command (none today) answers with a `deferred` tool error naming its reason.

Front-end launchers (`kit mcp`, a future `kit ui`) start a front end; they are not actions
and are deliberately not `CommandSpec`s. Everything else a user can do is.

## Start it

```powershell
kit mcp                                   # empty session
kit mcp --profile "C:\Modding\MO2-Starfield\profiles\Default"   # preload a load order
```

`--profile` runs the `load` command before serving (a warm load of the owner's 84-plugin
profile takes under a second; the first, cold one indexes every plugin). A preload failure
(a bad path, an unreadable profile) goes to stderr and into the server's `instructions`,
and the server starts with an empty session instead of exiting.
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
   with 8 tools; Claude Code names them `mcp__kitcraft__execute`, `mcp__kitcraft__coverage`
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
  `0`/`private`; the catalog and the validation rules: one hour, `public`). `initialize` and `ping` are not methods of this revision: a modern
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
| `inspect_record` | `inspect`'s args (`{form, plugin?, profile?, plugins?, ...}`) | One record across the load order (every version, the winner, typed values, lstrings), or in one plugin |
| `query_records`  | `query`'s args (`{type, where?, limit?, cursor?}`) | Records of one type matching a field predicate; paged        |
| `diff_plugins`   | `diff`'s args (`{a, b, form?}`)           | Record-level semantic diff between two plugins                        |
| `coverage`       | `coverage`'s args (`{plugin, decode?, signatures?, top?}`) | Record types and subrecords vs the schema; a summary by default over MCP |
| `record_schema`  | `schema`'s args (`{type?, member?, deep?}`) | What the schema defines for a record type: members with type, source citation and confidence (M2.F9) |
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
| `kitcraft://commands`   | The catalog: id, label, menu_path, shortcut, aliases, summary, args_schema (parsed), status (`implemented`/`planned`/`deferred`), deferred_reason, deferred_to, `cli` (`kit ba2 extract`), `tools` (the MCP tools that run it: `execute`, `command_line`, and its shortcut if any) |
| `kitcraft://load-order` | The session's load-order listing as `load` reported it (`Session::load_order`); `{"loaded": false, "plugins": [], ...}` before any load |
| `kitcraft://validate-rules` | `validate`'s rule registry in run order: `rules[]` (`id`, `severity` of the main finding, `input` = `plugin` or `scripts`, `summary`, `run_only_this`), the script `denylist` (`class`, `function`, `why`), `docs` (`docs/validate.md`) and a note on which rules also warn |

The catalog and the rules change only with the binary: over 2026-07-28 they are served
with `cacheScope: "public"` and a one-hour `ttlMs`; the load order with `0`/`private`.

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
- Paths (`plugin`, `out`, and every other path argument of every command) are not
  confined to `KITCRAFT_DATA_DIR` or any other root: the server reads and writes wherever
  the `kit` process may, with the user's permissions. A relative path resolves against
  the `kit` process's working directory, which for `kit mcp` is whatever directory the
  MCP client launched it in (for Claude Code, normally the project directory). Pass
  absolute paths. The only guards on `out` are the existing-file refusal (`overwrite`)
  and the same-file refusal above.

### Load-order arguments (`load`, `inspect`, `query`, `conflicts`)

`profile` (an MO2 profile dir, read-only) or `plugins` (files in load order; bare names
resolve against `data_dir`), plus `data_dir` and `no_cache`.

**`data_dir` defaults to `KITCRAFT_DATA_DIR`** (every command that takes it: `load`,
`inspect`, `query`, `conflicts`, `set`, `build`, `validate`, `diff`) when the argument is
absent and the variable is set and not empty; an explicit `data_dir` always wins. For a
`profile` the MO2 instance's `gamePath` + `Data` comes first, and `KITCRAFT_DATA_DIR` is
used only when the instance names no `gamePath` (a limitation says so). String tables for a
`plugins` list are read next to the first plugin unless `data_dir` is given. Given, the command loads first and the load order replaces the
session's; absent, the command uses the session's load order (an MCP server keeps one;
each `kit` process starts empty, so CLI users pass `--profile` to each command). No load
order at all is a `failed` error. Indexes are cached under `%LOCALAPPDATA%\kitcraft\cache`
(`KITCRAFT_CACHE_DIR`): a warm load of the owner's 84-plugin profile takes about 0.7 s.
Rules, evidence and assumptions: `docs/load-order.md`.

Every result over a load order (`load`, `inspect`, `query`, `conflicts`) repeats the
limitations that affect it, whether it loaded the plugins itself or used the session's:
ordering errors (a master loading after a plugin that names it), plugins that failed to
load or have no file, masters missing from the load order, runtime IDs as an assumption
(`runtime-slots-by-scale`), plugins whose own records resolve by the `full-style-self`
assumption, records of one plugin resolving to the same form, records whose FormIDs do not
resolve, the profile's warnings, which plugins are active by an assumption, and the
`hardcoded-official-slots` order.

String tables (M2.D7): the load order keeps the tables of its localized plugins in the
`language` given to `load` (default `en`), read on first use per plugin from a loose
`Strings\<plugin>_<language>.{strings,dlstrings,ilstrings}` (through the MO2 overlay, or the
Data dir) or from the plugin's `<stem> - *.ba2` archives (Localization first). A resolved
lstring carries `string_id`, `text`, `string_table` (`Starfield - Localization.ba2:strings/
starfield_en.strings`), `string_kind` and `string_offset`, with an evidence entry at that
offset in the table; a table or ID that cannot be found leaves `string_id` alone and a
limitation saying where it looked.

### `load` (M2.D8)

Arguments `{profile | plugins, data_dir?, language?, no_cache?}`; CLI `kit load --profile
<dir>`. Fills the session; the MCP `kitcraft://load-order` resource serves the same JSON.

`data`: `loaded`, `source` (`profile:<dir>` or `plugins`), `language`, `plugins[]`
(`index`, `name`, `path`, `active` (the rule that made it active: `Starfield.ccc`,
`hardcoded-official`, `plugins.txt`, `mo2-unmanaged:<entry>`,
`blueprint-companion-of:<base>`, or `listed`), `provider` (`overwrite`, `mod:<name>`,
`data`), `flags` (`0x%08X`), `flag_names`, `scale` (`full` | `medium` | `small` | null),
`localized`, `form_version`, `masters[]` `{name, index}` (index null when missing),
`missing_masters`, `late_masters` (masters loading after the plugin), `resolvable` and
`resolve_error`, `records`, `unresolved_records`, `full_style_self_records`,
`full_style_self_masked_records`, `assumed_records` (the first ten records resolved by an
assumption: `{raw_form_id, form, basis}`), `duplicate_records`,
`runtime_slot` (`05`, `FD:01`, `FE:00A`; assumed), `cache` (`hit` | `built` |
`uncached`), `error` (a plugin that failed to parse stays listed with no records)),
`inactive` (plugins the profile lists but does not activate), `missing_active_plugins`
(active per the profile but with no file; M3.D1), `records`, `versions`,
`cache_dir`, `cache_hits`, `index_ms`, `table_ms`, `elapsed_ms`, `summary`. Evidence: each
plugin's TES4 header. Coverage `not_examined` lists the plugins that failed to load.

### `inspect` (M2.D9; single-plugin mode kept)

Arguments `{form, plugin?, profile?, plugins?, data_dir?, no_cache?}`.

**Load-order mode** (a load order given or in the session): `form` is a formref
(`Starfield.esm:01A2B3`: defining plugin and object ID, never 0), a hex FormID read as
stored in `plugin` (the plugin's own master numbering; bare hex without `plugin` is
refused unless it is an EditorID), `runtime:<hex>` (a runtime ID under the runtime-slot
assumption), or an EditorID (ASCII case-insensitive). `data`:
`mode: "load_order"`, `load_order`, `language`, `form`, `matched_by` (`formref` |
`form_id` | `runtime_id` | `editor_id`), `global` (formref), `defining_plugin`,
`object_id`, `runtime_id` and `runtime_id_basis` (`assumed:runtime-slots-by-scale`),
`signature`, `editor_id` (the winner's), `winner`, `winner_deleted`, `winner_source`
(M3.D1: `{safe, blocking[], unrelated[]}`, whether the winner is a safe source to copy;
rules in `docs/load-order.md`, "Safe sources"), `summary` (says
"(deleted)" after a deleted winner), and `versions[]` in load
order: `plugin`, `load_index`, `winner`, `deleted` (record flag 0x20), `resolved_by`
(`direct` | `assumed:full-style-self` | `assumed:full-style-self-masked`), `raw_form_id`
(as stored in that plugin),
`record` (header fields), `subrecord_offsets`, `subrecords[]` (as below, plus `text` for
EDID; for FULL/DESC the lstring members above; `member` (the schema's member name) and
`value` (decoded through the schema: structs as objects, FormIDs as `{raw, form}`, the null
FormID as `{raw: "00000000", form: null, null: true}`) where
lane B's definitions give a layout), `vs_previous` (null
for the first version, else `{class: identical-to-previous | deleted | differs,
flags_differ, order_differs, subrecords[] {signature, occurrence, change, member?,
fields?, as_bytes?}}`, typed members compared by value as in `diff`) and
`payloads_comparable` (whether the two plugins encode FormIDs alike; when false a byte
difference may be encoding only). Evidence: every version's header and every payload. A
`value` decoded through a member whose definition has `reference` confidence (documented in
the pinned xEdit file, not yet confirmed by our corpus or the CK) is named in
`limitations` (`... reference confidence ...: WEAP DNAM.Data.Speed`; M3.D3).

**Single-plugin mode** (only `plugin` given): CLI `kit inspect <form> --plugin <file>`.
`form` is a FormID (1-8 hex digits, optional `0x`, matched against the record header as
stored in the file) or an EditorID (ASCII case-insensitive). A hex form that matches no
FormID is retried as an EditorID. Not found is a `failed` error.

`data`: `plugin`, `form`, `matched_by` (`form_id` | `editor_id`), `record` (`signature`,
`form_id`, `editor_id`, `flags` as `0x%08X`, `compressed`, `offset`, `data_size`,
`timestamp`, `version_control`, `form_version`, `unknown`, `groups` = enclosing groups
outermost first as `{offset, type, label}`), `subrecord_offsets` (`file`, or
`decompressed` for a compressed record), and `subrecords[]` with `signature`, `size`,
`oversized` (written with an `XXXX` prefix), `offset` (start of the subrecord, including
any `XXXX` prefix), `data_offset`, `payload_hex` (first 256 bytes, uppercase hex) and
`payload_truncated`. Evidence: the record header, then each payload (or, for a compressed
record, the compressed body). Coverage is `partial`; every subrecord counts as opaque.
Limitations always say that single-plugin mode lists raw payloads (schema-typed values
are shown in load-order mode) and that overrides in other plugins are not considered.

### `query` (M2.D10, M2.D11)

Arguments `{type, where?, scope = "winner" | "all", limit = 50 (max 1000), cursor?}` plus
the load-order arguments; CLI `kit query --type WEAP --where "FULL ~ 'Rifle'" --profile
<dir>` (strings take `"` or `'`; in PowerShell use `'` inside a `"..."` argument, since
PowerShell 5.1 drops embedded `"` when calling a program).

Candidates are the winning version of every form of `type` (`winner`) or every version
(`all`), ordered by the load-order index of the plugin holding the version, then FormID.
`data`: `type`, `where`, `scope`, `limit`, `matched`, `unknown` (records on this page that
could not be decided: not matched), `scanned`, `candidates`, `next_cursor` (null on the
last page), `results[]` (`form`, `signature`, `editor_id`, `full` (`{text}` inline, or the
lstring members above), `plugin`, `load_index`, `raw_form_id`, `offset`, `runtime_id`, `winner`,
`deleted`, `resolved_by`),
`unknown_records[]` (first 20: `form`, `plugin`, `reasons`), `elapsed_ms`, `summary`.
Evidence: each result's record header and its FULL payload. A cursor is opaque; one from
another load order, type, `where` or scope is a `failed` error. Paging returns the same set
as one large page.

`where` grammar (`crates/doc/src/query.rs`):

```
query   = or ;
or      = and { "or" and } ;
and     = not { "and" not } ;
not     = "not" not | atom ;
atom    = "(" or ")" | "has" SIG | path op value ;
path    = SIG { "." member } [ "[" index "]" ] | "@form" | "@plugin" | "@edid" | "@flags" ;
op      = "==" | "!=" | "<" | "<=" | ">" | ">=" | "~" | "!~" | "^=" | "&" ;
value   = number | hex | string | formref | "true" | "false" | "null" ;
formref = plugin-file-name ":" hex ;          (Starfield.esm:0001A2B3; object ID not 0)
```

- Keywords, signatures and member names are ASCII case-insensitive. Text comparisons
  (`==`, `!=`, `~` contains, `!~` does not contain, `^=` starts with) are ASCII
  case-insensitive. Ordering operators need a number; `@form` takes `==`/`!=` and a
  formref (bare, or quoted when the file name has spaces); `@plugin` and `@edid` take
  text; `@flags` (record flags) takes numbers. `&` tests bits: `@flags & 0x20` is true when
  every bit of the mask (a non-negative integer) is set; it applies to integer fields too.
  `null` is the null FormID (`00000000`): `KWDA == null`; it takes `==`/`!=` only.
- A repeated subrecord matches if any occurrence matches; `[n]` picks occurrence `n`
  (0-based).
- Field paths are decoded through lane B's schema definitions (`crates/doc/src/typed.rs`):
  `SIG.Member.Member` walks struct members, arrays fan out (any element matches), and a
  path may start with the subrecord's own member name (`KSIZ.Count` = `KSIZ`). FormID
  members compare as formrefs (`KWDA == Starfield.esm:0004A0BB`); lstrings compare as
  their resolved text. A record type or subrecord without a definition falls back to
  `EDID` as text and `FULL`/`DESC` as lstrings. A path that reads a member whose
  definition has `reference` confidence is named in `limitations` (M3.D3), as in `inspect`.
- A subrecord the record does not have makes a comparison false and `has` false. A value
  of a union member (the schema picks its variant per record) of another kind than the
  literal (a GMST string variant against `DATA > 100`) is another variant: not equal, not
  ordered and does not contain, so false (`!=` and `!~` true). Any other kind mismatch (a
  FormID or boolean against a number, text against a number) is unknown, with the reason.
  Unknown also: a member
  inside bytes the schema keeps opaque or a payload its layout does not cover, a subrecord
  without any definition, a string ID in no table, a FormID that does not resolve, a record
  that cannot be read, a record missing from the index (`@flags` too), and any field path or
  `has` on a deleted version (record flag 0x20: its payload is not the form's data).
  `and`/`or`/`not` follow Kleene logic; an unknown record is not matched, is counted, and
  its reasons are listed.
- Errors (`failed`) carry a 1-based column: `where: column 6: ...`. Text is capped at 4,096
  characters and nesting at 64.

### `diff` (M2.D12)

Arguments `{a, b, form?, data_dir?, limit = 200}`; CLI `kit diff <a> <b>`. Stateless (no
load order needed). Each file's FormIDs are resolved through its own masters by name;
masters' scales are read from their headers next to each file or in `data_dir` (default
`KITCRAFT_DATA_DIR`). A file's
own records match the other file's records of it when the other lists it as a master, and
match each other otherwise (an edited copy). If a master cannot be found but both files
list the same masters in the same order and have the same own scale, records are matched by
raw FormID (`keyed_by: "raw"`, forms labelled `raw:XXXXXXXX`); otherwise the diff fails
naming the master. Formrefs spell plugin names as the files do (`Base.esm:000800`).

`data`: `a`, `b`, `form`, `keyed_by` (`name` | `raw`), `counts {added, removed, changed,
unchanged}`, `added[]` / `removed[]` (`form`, `signature`, `raw_form_id`, `offset`), and
`changed[]`: `form`, `signature`, `class`, `a`/`b` (`raw_form_id`, `offset`, `flags`,
`compressed`), `flags_differ`, `order_differs`, `subrecords[]` (`signature`, `occurrence`,
`change`: `added` | `removed` | `changed`, `a`/`b` spans `{file_offset, body_offset,
length}`, `first_difference` (null when the bytes are equal and only what they mean
differs), and for a subrecord the schema gives a layout (M3.D2): `member` (the schema's
member name) and `fields[]` `{path, before, after}` (paths like `KWDA.Forms[1]`, `DATA.Value`,
`DNAM[1].Speed`; FormIDs as `{form}` formrefs, `{form: null, null: true}` or `{form: null,
raw, error}`; an opaque member as `{opaque: {payload_offset, length, file_offset}}`; null on
the side where the member is absent), or `as_bytes` with the reason it is compared as
bytes). `counts.equal_by_value`: records whose bytes differ only in how typed FormIDs are
encoded (another master order), counted as unchanged. Lists stop at `limit` (counts are
complete). Records whose bodies and flags are byte-identical count as unchanged without
being parsed when both files encode FormIDs alike; otherwise they are compared by value.
Evidence: both sides of every listed record and differing payload.

### `conflicts` (M2.D13, port of `esmscan.py`)

Arguments `{master?, plugin?, type?, only_conflicts = false, count_classes = false, limit =
50 (max 1000), cursor?}` plus the load-order arguments; CLI `kit conflicts --master
Starfield.esm --profile <dir>` (alias `esmscan`).

`data`: `counts[]` (`master`, `plugin`, `total`, `types {SIG: n}`: override records of
`master`'s forms by `plugin`, as `esmscan.py` prints them), `override_records`, `forms`
(overridden forms passing the filters), `items[]` (a page: `form`, `signature`,
`editor_id`, `base_present`, `versions` (plugin names in load order), `overrides[]`:
`plugin`, `previous`, `class` (`identical-to-previous` | `deleted` | `differs` |
`first-version`), `resolved_by`, `comparable`, `subrecords[]` (`signature`, `occurrence`,
`change`, and `member`/`fields[]`/`as_bytes` as in `diff`: typed members compared by value
with FormIDs resolved through each version's plugin, so an override that only re-encodes
the same forms is `identical-to-previous`; M3.D2), `flags_differ`), `class_counts` (this page),
`class_counts_all` (every matching form, with `count_classes`), `next_cursor`, `summary`.
Evidence: each override's record header and its differing payloads.
`only_conflicts` keeps forms that two or more plugins override.

Checked 2026-10-14 against `esmscan.py` run read-only on the owner's Default profile
(master `Starfield.esm`, the other 83 plugins): the same 59 plugins and identical per-type
counts, 8,234 override records in total.
### `coverage` (M1.14, M3.F2)

Arguments `{plugin, decode = true, signatures?, top?}`; CLI `kit coverage <plugin>
[--no-decode] [--no-signatures] [--top N]`. Streams the
plugin and classifies every subrecord occurrence against the builtin schema by its record
type and signature: `typed` (the type's definition names it with a layout and the payload
decoded through it), `failed` (named by a typed member, but the payload does not fit the
layout or the definition does not accept the subrecord at its position; kept as raw
bytes), `opaque` (named, kept as raw bytes) or `unknown` (the type has no definition, or
the definition does not name the signature; subrecords inside an `any_until` run count
here). Every record of a defined type is decoded through its generated definition with
the plugin's context (form version, TES4 localized flag: what `Plugin::typed` does) and
encoded again; a subrecord position the encoding does not reproduce is a re-encode
conflict. With `decode` (the default) compressed records are
decompressed under the default `DecodeLimits` and counted; one that fails to decompress is
listed in `coverage.not_examined` (first 20, then a count), not fatal.

Size controls, which never change the totals: `signatures: false` sets each type's
`signatures` to `null`; `top: N` lists only the N types with the most records (`0` lists
none) and adds a limitation naming how many of how many are listed. Omitted, `kit` (and
`cargo xtask parity`) list everything; over MCP they default to `false` and `20`.

`data`: `plugin`, `file`, `bytes`, `decode`, `records`, `groups`, `compressed_records`,
`compressed_examined`, `record_types_seen`, `record_types_defined`, `typed_records`
(records decoded through a definition), `subrecords` (occurrences), `occurrences {typed,
failed, opaque, unknown}`, `decoded_by_confidence {reference, corpus, ck-diff, verified}`
(typed occurrences by the confidence of the member that decoded them; the weakest when
several members share a signature), `reencode_conflicts`, `typed_failures[]` (the first
20 failed or re-encoded subrecords: `record` = `SIG:FORMID`, `signature`, `index` in the
record, absolute `offset` or `null` inside a compressed record, `kind` `not_decoded` or
`reencoded`), `signatures` and `top` (as applied), `record_types_listed`, `summary`, and
`record_types[]` sorted by record count (then signature), each with `signature`,
`definition` (the schema name or `null`), `records`, `compressed`, `compressed_examined`,
`typed_records`, `subrecords`, `occurrences`, `decoded_by_confidence`,
`reencode_conflicts`, `distinct {seen, typed, opaque, unknown}` (distinct signatures by
their class in the definition), `first_offset`, and `signatures[]` (`signature`, `count`,
`class`, `failed` = occurrences that did not decode, `confidence` = the weakest of the
members using it or `null`, `members` = member paths in the definition, `first_record` =
`SIG:FORMID` of the first record holding it; `null` with `signatures: false`).

Coverage: `decoded_subrecords` is the `typed` occurrence count, `opaque_subrecords` the
`failed` plus `opaque` plus `unknown` count; `status` is `complete` only when those are
zero, nothing failed to re-encode and every compressed record was examined. Evidence: the
whole file, then the first record of each listed type. Limitations always state how
subrecords are classified; they add a count line when a typed subrecord failed or a
position did not re-encode, and with `--no-decode` that compressed bodies were skipped
(also in `not_examined`). `cargo xtask parity` runs this command over the corpus to write
`docs/record-parity.md`.

### `schema` (M2.F9)

Arguments `{type?, member?, deep = false}`; CLI `kit schema` (every definition), `kit schema
--type WEAP` (alias `defs`), `kit schema --type WEAP --member KWDA`; MCP tool
`record_schema`. Reads no plugin: it reports what the builtin schema
(`crates/schema/defs`, lane B) defines, so an agent can see which members a record type
has, their types, and how far each is checked before it writes a `query` predicate or a
`set` field path. `type` is case-insensitive.

- Without `type`: `records[]` (`type`, `name`, `confidence`, `subrecords`, `typed`,
  `opaque`, `confidence_counts {reference, corpus, ck-diff, verified}`), `headers[]`,
  `common[]` (shared groups and types), the same totals over every definition, `summary`
  (one line per record type in text mode).
- With `type` (a record definition): `defined: true`, `kind: "record"`, `name`,
  `confidence`, `source`, `evidence`, `note`, `header_flags`, the counts above for the
  type, and `members[]` in definition order, **top level only** unless `deep`: a subrecord
  is `{kind: "subrecord", signature, name, card, type, typed, size, confidence, source}`
  (`type` is a label such as `zstring`, `u32`, `formid<KYWD>`, `[formid<KYWD>; rest]`,
  `opaque(24)`, `union by edid-prefix (5 variants)`); a group is `{kind: "group", name,
  card, shared, signatures, confidence, source}`; an `any_until` run names its terminator.
  `deep` adds `evidence` and `note`, a group's `members`, a struct's `fields` (`name`,
  `type`, form-version gates, `confidence`, and `source` where it differs from the
  member's), enum `values`, flag `bits` and union `variants` (`when`, `name`, `type`).
  `member` (a name or signature, ASCII case-insensitive, also inside a group) returns that
  one member, deep; an unknown one is `failed` and lists the members.
- With a header signature (`GRUP`): `kind: "header"`, `size`, `fields[]`.
- With a type that has no definition: `defined: false` and `defined_types`; a limitation
  says its subrecords are unknown to the schema (not absent).

Coverage is `complete` with null counts (no plugin bytes are decoded) and `evidence` is
empty: the values are definitions, each carrying its own source citation. Limitations
always say so, count the `reference`-confidence definitions (documented upstream, not yet
corpus-checked) and the opaque ones, and note when only the top level is listed.

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
- `papyrus.compile {source, out_dir, imports?, sources_zip?, flags?, compiler?, optimize =
  false}`; CLI `kit papyrus compile <source> <out_dir> [--imports <dir> ...]
  [--sources-zip <Tools\ContentResources.zip>]`. Runs Bethesda's
  `PapyrusCompiler.exe` (black box) on one script by name: a `.psc` path becomes its name
  with its folder imported first; `.ppj` projects are refused. Arguments are only the
  object, `-f=`, `-i=`, `-o=` and `-op`; release and final mode can never be requested
  (any extra argument such as `release` is `invalid_args`). The flags file defaults to
  `Starfield_Papyrus_Flags.flg` from the first import that has it. The base sources are
  the `Scripts\Source` folder of `Tools\ContentResources.zip`: with `sources_zip` (M3.G1)
  its `.psc` and `.flg` files are extracted once into the KitCraft cache
  (`KITCRAFT_CACHE_DIR`, else `%LOCALAPPDATA%\kitcraft\cache`, folder
  `papyrus-sources\<key>`, keyed by every entry's name, size and CRC-32, written under a
  temporary name and renamed when complete) and imported after `imports`; the zip is
  read in place, its entry names are checked (ASCII letters, digits, `_`, `-`, `.`,
  spaces; no `..`, device names or case collisions) and sizes capped before anything is
  written. The first call extracts 5,034 files (12.3 MB, about 10 s); later calls only
  read the zip's directory. Without `sources_zip` the caller passes an extraction in
  `imports`. `data`: `source`, `script`, `out_dir`, `compiler`, `sources_zip` (`zip`,
  `dir`, `files`, `bytes`, `skipped`, `cached`, or `null`; the zip is the first
  `evidence` entry), `args`, `exit_code`, `passed` (the
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
- Both take `tolerant = false` (M3.G2). Tables are parsed strictly by default: the
  header's data size must equal the bytes after the directory, as in every official
  table, and a mismatch fails with a hint. With `tolerant: true` a table padded past its
  data size is read too; each padding run (bytes after the declared data, and
  unreferenced bytes at the end of the declared data) is a `limitations` entry naming
  the table, offset, length and whether it is all zeros, and is listed in `data.padding`
  (`table`, `where` = `after_data` or `data_tail`, `offset`, `length`, `zeros`); strings
  must still lie inside the declared data. `strings.merge` writes the official layout,
  without padding, and says so. `data.tolerant` echoes the mode.

### `set` (M2.E5)

Arguments `{form, fields, into, plugin?}` plus the load-order arguments above (`profile` |
`plugins`, `data_dir`, `language`, `no_cache`), or the session's load order. CLI
`kit set <form> <into> FULL="..." [--profile DIR | --plugins A.esm ... --data-dir D]`, or
`kit set <form> FULL="..." --into X.esp ...` (the `FIELD=value` shorthand, see
`command_line` grammar).

- `form`: `Plugin.esm:00ABCDEF` (formref), an EditorID, or a hex FormID as stored in
  `plugin`. Its winning version in the load order is the record copied.
- `fields`: field path -> value (`crate::edit::fields`): `FULL`, `FULL[1]`, `DNAM.Member`,
  `DNAM.Member[2]`; values are JSON numbers or text parsed by the schema type (integers by
  width, `f32`/`f64`, enums and flags by name, zstring, lstring, FormIDs as formrefs mapped
  into the patch's masters), `{"mul": x}` / `{"add": x}` on typed numbers, `hex:0A0B` for
  opaque payloads (the only form they take). A bad path or value is `invalid_args` at
  `/fields/<path>` with a rule id (`path-syntax`, `no-subrecord`, `no-member`, `type`,
  `range`, `opaque`, `formref`, `layout`).
- `into`: the patch, created or updated (`.esm` gets the master flag, `.esp` does not;
  `.esl` is refused). Never a plugin of the load order, never inside the game Data dir,
  the user Data dir or an MO2 instance or profile (any depth). The writer rules (masters,
  lstrings inline, remapping, header, scaled sources refused) are in
  `crates/engine/src/edit/patch.rs` and `docs/corpus.md` "Writing rules". The write goes
  to a staged copy, `validate`'s plugin rules run on it, and `into` changes only when the
  records written have no error (`docs/validate.md`).
- `allow_untyped_lstrings` (default `false`, CLI `--allow-untyped-lstrings`): copy
  non-zero 4-byte subrecords of unknown layout from a localized source as stored (each
  named in a limitation) instead of refusing; they may be string IDs.

`data`: `target`, `file`, `created`, `masters`, `flags`, `bytes`, `summary`, and
`records[]` (parents copied to hold a cell/world/topic child come first, `parent: true`)
with `signature`, `form_id` (as stored in the patch), `form`, `editor_id`, `source`,
`outcome` (`inserted` | `replaced`), `fields[]` (`path`, `before`, `after`, `changed`;
an lstring of a localized source shows `{"string_id"}` before), `inlined_lstrings[]`
(`at`, `string_id`, `text`, `table`), `offset`/`length` in the patch, and `copied_from`
(`plugin`, `load_index`, `offset`, `record`, `versions_in_load_order`). Evidence: the
copied record in its source file, the record in the patch, each edited payload in the
patch. `validate`: the run on the written plugin (`passed`, `errors`, `warnings`,
`findings`, `rules`). Limitations: the load order's, the M2.E3 `ck-diff` part not observed
yet (with the header subrecords deliberately not written), a master list extended,
FormIDs remapped, lstrings typed by the corpus table, inline text with bytes >= 0x80,
subrecords copied under `allow_untyped_lstrings`, validate errors on records the call did
not write.

### `validate` (M2.E6, M2.E7, M2.E8)

Arguments `{plugin?, scripts?, plugin_archives?, rules?}` (`plugin`, `scripts`, or both)
plus the load-order arguments (all optional). `scripts` takes `.pex` files, directories
of them, and `.ba2` archives (each `.pex` entry is read in memory; a finding names it
`<archive>/<path in archive>`, an entry that does not parse is a limitation);
`plugin_archives: true` (CLI `--plugin-archives`, needs `plugin`) adds the plugin's own
`<stem> - *.ba2` archives (M3.E3). **Masters of a standalone plugin are searched in
order:** the named (`profile`/`plugins`) or session load order, the plugin's own
directory, then `data_dir` (default `KITCRAFT_DATA_DIR`). So with the variable set,
`kit validate --plugin out\X.esm` finds `Starfield.esm` with no other argument. A bare
`plugin` name that is not in the working directory resolves in `data_dir`. CLI
`kit validate --plugin X.esp [--data-dir D]`, `kit validate --scripts dir\with\pex`;
`--rules masters.missing --rules lstring.copied-id` limits the run (an unknown id is
`invalid_args` at `/rules/<i>`). The rules, their severities and sources:
`docs/validate.md`.

`data`: `plugin`, `scripts`, `passed` (no `error` finding; `kit` exits 1 when false),
`errors`, `warnings`, `rules` (ids run), `findings[]` (first 1,000: `rule`, `severity`,
`plugin`, `record` = `SIGN:FORMID` or `Script.Function`, `subrecord`, `offset`,
`message`), `findings_total`, `masters[]` (`name`, `path`, `flags`, `localized`),
`records`, `groups`, `subrecords`, `checked_form_ids`, `unresolved_form_ids`,
`unchecked_subrecords`, `scripts_checked`, `summary`. Coverage is `complete` only when no
FormID was left unchecked; opaque subrecords count as `opaque_subrecords`. Evidence: one
byte at each finding's offset.

### `build` (M2.E9; spec v2 M3.E6)

Arguments `{spec, out, plan = false, overwrite = false, allow_untyped_lstrings = false}`
plus the load-order arguments (used when the spec has no `[load_order]`). CLI
`kit build rename.toml <out> [--plan] [--overwrite]`.

**Spec version 2** (`crates/engine/src/build/spec.rs` has the full grammar): `[class.<name>]`
tables (`type`, optional `where`: every winning record of the type when absent, optional
`brief`), `[[rule]]` entries naming a `class`, with `fields`, optional `via` (typed FormID
member paths followed hop by hop: `via = ["KWDA"]` reaches a weapon's keywords),
`allow_reference` and `brief`; `[[set]]` as in version 1 (plus `allow_reference`);
top-level `strict`; `[plugin] style = "kit"` (the `ck` style is refused until M3.A3).
Operators on typed numbers: a value, `{ mul }`, `{ add }`, `{ min }` (at least),
`{ max }` (at most), `{ clamp = [lo, hi] }`; other types take only a value. In version 2 a
record reached by several rules or classes is written once (the same write twice is one
write; `changes[].by` lists every entry that asked for it), two different writes to one
field are an error naming both keys, a field path the record type's schema does not have
is `unknown member`, a rule that matches nothing is a warning (`data.warnings`, and an
error with `strict = true`), and a write to a `reference`-only member is refused unless
the entry has `allow_reference = true` (then named in `limitations`). Version 1 specs
build as before. `plan: true` writes nothing to `out` (the plugin is built and validated
in the OS temp dir only) and returns `plan`, `changes[]` (`form`, `signature`,
`editor_id`, `path`, `before`, `after`, `changed`, `by`), `changed_fields`, `classes[]`
(`class`, `type`, `where`, `brief`, `matched`, `undecided`), `rules[]` (`rule`, `class`,
`brief`, `matched`, `via[]` hops with `from`/`reached`/`unfollowed`, `records`,
`fields_written`, `fields_changed`), `validate` and `passed`; a plan with nothing to write
(a fresh `overhaul.scaffold` spec) reports 0 changes. `out` is checked like `set`'s `into` (never a
load-order plugin, the game or user Data dir, or MO2) before anything is written, and an
existing `out` is replaced only with `overwrite`. The spec (TOML):

```toml
version = 1
[plugin]
localized = false                     # the only value accepted
[load_order]                          # optional: profile | plugins, data_dir, no_cache, language
plugins = ["Starfield.esm"]
data_dir = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
[[rule]]                              # every winning record of `type` matching `where`
type = "WEAP"
where = 'EDID ~ "Rifle"'
fields = { FULL = "Renamed" }          # or { "SIG.Member" = { mul = 0.8 } } on a typed number
[[set]]                               # one record, as `set`
form = "Starfield.esm:0021BBCD"
fields = { FULL = "Old Soviet Marksman Rifle" }
```

Rules expand over their `query` matches (winners, load order then FormID); `[[set]]`
entries apply after rules; a field written twice (the same path, or a whole subrecord and
a member of it) is an error naming both keys. The plugin is written fresh into a staging
directory under the OS temp dir (stale ones from crashed runs are removed), validated,
and moved to `out` only when `validate` finds no error; otherwise nothing is written and
the command fails with the errors. Spec errors are `failed` with `spec: <key>: ...`
(`spec: set[0].fields.full: ...`); TOML `inf`/`nan` values are range errors. `data` is `set`'s plus `spec`, `rules[]` (`rule`,
`matched`, `undecided`), `validate` (`passed`, `errors`, `warnings`, `findings`, `rules`)
and `passed`. Two builds of one spec give the same bytes, and a one-`[[set]]` spec gives
the bytes `set` writes.

### Coming in M3: `refs` (M3.D4)

Not in the catalog yet: it lands as one `CommandSpec` with M3.D4, and lane D documents its
result shape here when it does; until then `execute` answers `unknown_command`. It gets no
MCP shortcut tool (the tool list stays at eight): `execute` and `command_line` reach it,
and `kitcraft://commands` lists it with its `cli` words, like every other command.

- `refs {form, profile?, plugins?, ...}` (lane D); CLI `kit refs <form>`. The CK's "Use
  Info": every record whose header or typed FormID members name `form` across the load
  order (the shared load-order arguments above), with evidence per reference and the
  count of opaque payloads not scanned in `limitations`. Read only.

### `overhaul.scaffold` (M3.E7; CLI `kit overhaul`)

Arguments `{type, where?, group_by?, out?, overwrite = false, max_groups = 40}` plus the
load-order arguments. CLI `kit overhaul WEAP --group-by KWDA --out weapons.toml
--profile <dir>` (or `kit overhaul --type WEAP ...`). Emits a starter `build` spec v2 for
the winning records of `type` matching `where`: one class per value of `group_by` (a
`where` path such as `KWDA`, `@plugin` or `DNAM.Member`; a record with several values
joins one class per value; without it, one class), the largest `max_groups` as classes
and the rest listed in a comment, each class with its `where` (exactly its group), its
match count and the min/median/max of every typed numeric member of the type as comments,
a `brief` placeholder and a commented-out rule. Every rule is commented out, so
`build --plan` on a fresh scaffold reports 0 changes with each class matching what the
scaffold counted. The text is deterministic; the `[load_order]` repeats the
`profile`/`plugins`/`data_dir`/`language`/`no_cache` arguments. With `out` (a `.toml`
file, never in the game or user Data dir or MO2; an existing one only with `overwrite`)
it is written; without, `data.spec_text` (and the summary) holds it. `data`: `type`,
`where`, `group_by`, `out`, `written`, `bytes`, `records`, `undecided`, `ungrouped`,
`group_undecodable`, `unexpressible`, `classes[]` (`class`, `where`, `matched`, `label`,
`stats[]` with `member`, `records`, `undecodable`, `min`, `median`, `max`), `omitted[]`,
`numeric_members[]` (`path`, `member`, `kind`, `confidence`, `in_array`), `spec_text`,
`summary`. On the owner's Default profile, `kit overhaul WEAP --group-by KWDA` takes
0.8 s warm (release, 541 winning WEAP, 2026-10-14).

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
- `FIELD=value` shorthand (lane E): a command with exactly one object argument (`set`'s
  `fields`) also takes its entries as words: `kit set <form> FULL="Old Soviet Marksman
  Rifle" --into X.esp`, `DNAM.Speed=1.5`, `FULL[1]=x`, `MODL=hex:0A0B`. A word is an
  assignment when it starts with a field path (four characters of `A-Z`, `0-9`, `_`, then
  `.member` / `[n]` steps) and `=`; the value is always the string after the first `=`,
  exactly what `--fields '{"PATH":"value"}'` gives (the command parses it against the
  field's type). Assignments merge with `--fields`; a path given twice is a `usage` error.
  After `--` such a word is positional.
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
4. `execute {command: "set", args: {form: "Starfield.esm:0021BBCD", fields: {"FULL": "Old Soviet Marksman Rifle"}, into: "C:\\Modding\\kitcraft\\out\\MyPatch.esm"}}`
5. `execute {command: "validate", args: {plugin: "C:\\Modding\\kitcraft\\out\\MyPatch.esm"}}`:
   catches a localized FULL copied as a string ID before the game does.
