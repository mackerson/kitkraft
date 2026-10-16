# Developer workbench

The workbench is a read-only egui window over `Engine::execute` (ADR 0007 Amendment 1,
plan M4 lane W). It is a way to look at the data, not a second implementation of anything.
Each view runs one engine command, or a fixed sequence of commands, and shows the result
envelope. The full authoring UI is M5. One write is permitted (Amendment 2): the Ledger
tab appends the owner's in-game result as a row of `docs/game-observations.md`, a repo
doc, one confirm click per row.

## Launch

Double-click `KitKraft Workbench.cmd` in the folder `cargo xtask dist` writes
(docs/QUICKSTART.md). It runs `kit ui --dev`: the KitKraft Dev profile `kit doctor`
finds, else the profile picker. From a terminal:

```text
kit ui --dev
kit ui --profile "KitKraft Dev"           # a bare name is a profile of the detected MO2 instance
kit ui --plugins Starfield.esm --plugins MyPatch.esp [--data-dir <Data>]
```

The Data dir defaults to the detected game install (argument > `KITKRAFT_DATA_DIR` >
detected). Without the dist folder: `cargo build --release -p kit -p kit-cli`.

Build with `--release`. A debug build works but is slow and large: loading the KitKraft
Dev profile took 53 s and about 2 GB of memory in a debug build (the 10 to 12 s under
"Panels" below were warm-cache loads). A debug `kit-ui` prints a one-line warning saying
so when it starts.

`kit ui` starts `kit-ui.exe` from the same folder and passes the arguments through. You can
also run `kit-ui` directly, or use `cargo run --release -p kit -- --profile <dir>`. The
arguments are the same as for `kit mcp`. When they are given, the window opens with a
`load`. With none (`kit ui` alone, or a double click on `kit-ui.exe`), it opens on the
**Start** tab: the MO2 instance, a Data-dir line, and one load button per profile (see
"Panels").

`kit-ui.exe` is a console program, so started from Explorer or a shortcut it opens a
console window next to the workbench. That window shows the debug-build warning and any
startup error (for example a missing font); closing it closes the workbench. Started from
a terminal, it uses that terminal.

The engine runs on its own thread and handles one command at a time. The top bar shows the
command running and how many are queued behind it. Buttons and links that run a command
are disabled while one runs. **Drop pending** restarts the engine thread: queued requests
are dropped, the running command's reply is discarded (a command cannot be interrupted;
the old thread finishes it in the background), and the session is emptied, so load again.
If the window is closed during a `load`, the index-cache file it was writing is left as a
temp file; the next start removes it.

## What it can run (and what it cannot)

`crates/ui-egui/src/gate.rs` is the only module that holds the `Engine`. Every request is
checked against its allowlist by canonical id, so an alias cannot get around it:

- **Allowed:** `load`, `inspect`, `query`, `diff`, `conflicts`, `validate`, `coverage`,
  `schema`, `refs`, `planet` and `profiles`, none of which the catalog marks `writes`.
  (`refs --incoming` fills the engine's reverse-link cache, as `load` fills the index
  cache.)
- **Guarded:** `roundtrip`, which writes with `out`, runs only with `verify: true`, and never
  with `out` or `overwrite`.
- **The one permitted write** (`gate::PERMITTED_WRITE`, ADR 0007 Amendment 2): `ledger`.
  Without `confirm` it is a preview and passes like a read. With `confirm: true` it passes
  only as a request built by `Request::confirmed`, which only the Ledger tab's confirm
  button makes, one per click. The engine itself refuses any file but
  `<checkout>\docs\game-observations.md` and the `Default` profile.
- **Refused before the engine sees the request:** every other catalog command and alias.
  That includes `set`, `build` (in every mode, including `--plan`), `new`, `clone`, `place`,
  `package`, `deploy*`, `testplan`, `overhaul.scaffold`, `ba2.*`, `strings.*` and
  `papyrus.*`.

Tests enforce this in five ways:

- Each catalog command must be classified as allowed, writing, or unused read-only.
- The allowlist must hold no command the catalog marks `writes`, the gate's writing list
  must equal the catalog's `writes` set, and a guarded command must refuse its writing
  arguments. The writing commands the workbench can run unguarded must be exactly
  `{ledger}`.
- Every refused id and alias is tried, and each must be refused (`ledger` in its writing
  form, unconfirmed). A confirmed request for any other command is refused.
- A scan of the crate's sources allows `Request::confirmed` only in `gate.rs` and the
  Ledger panel (`panels/ledger.rs`).
- A scan of every source file and integration test except `gate.rs` denies the engine
  crate's name (`kitkraft_engine`), so a renamed import (`use ... {execute as go}`) or a
  glob import cannot reach the engine around the gate, as well as `Engine` and the
  execution API's names. `gate.rs` re-exports the few helpers the rest of the crate needs.

On top of that, `cargo xtask layers` checks that `kitkraft-ui-egui` depends on no workspace
crate except `kitkraft-engine` (`INTERNAL_ONLY`).

What the workbench writes, all of it:

- **The UI prefs file**, `%LOCALAPPDATA%\kitkraft\ui.json`: the MO2 instance and profile
  last loaded from the Start tab, and the checkout set on the Help tab. It is the only file
  the window writes on its own, only when one of those buttons is clicked, and it is UI
  state, not game data: no plugin, profile file or game folder is named or touched. Delete
  it to forget the choices.
- **The ledger row**, written by the engine's `ledger` command on a confirm click (above).
- **Screenshots**, only when you set the tour's environment variable (below).

`load` still fills the engine's own index cache, as it does for the CLI and MCP.

## Every result looks the same

Each panel shows the following under its result:

- The `kit` command line that gives the same envelope, with a copy button.
  - For `inspect`, `query`, `conflicts`, `validate`, `refs` and `planet`, the load-order
    arguments are added,
    because each `kit` process starts with an empty session. They are the arguments of the
    `load` that had succeeded when the command ran, not what the Load form says now. When
    no load had succeeded, there is no `kit` line, only the MCP call.
  - Plugins picked from the loaded list go in as full paths. That way the line also works
    in a fresh process for a plugin that lives in an MO2 mod folder.
- The MCP `execute` call, with a copy button.
- The summary, and PASSED or FAILED when the result includes `passed`.
- Coverage. A count of `null` is shown as "not counted".
- Every limitation, always on screen. A long limitation is cut to one line, and you can
  hover to see all of it.
- The panel's own rows.
- The evidence table: the first 300 ranges, collapsed by default.
- The raw `data` JSON, collapsed by default. It is formatted once when the result arrives
  and drawn only where it is scrolled into view, so a big result does not slow the window;
  it is cut after 500,000 lines and lines after 2,000 characters (the MCP call has all).

If the engine returns an error, the panel shows its `kind`, its message and the
`{"error": ...}` JSON, which includes the violations or the places it searched.

## Panels, as run on the KitKraft Dev profile (2026-10-15)

These were checked read-only against `KitKraft Dev` (84 plugins, warm index cache). Two runs
were used: the window's screenshot tour (images kept outside the repo) and
the walkthrough test with `KITKRAFT_WORKBENCH_PROFILE` set. That test runs the same
requests through the same gate, worker and view-models, with no window. It is `#[ignore]`d
(it needs a real profile and is not part of `cargo xtask ci`) and is on the owner's list
in `plan/STATUS.md`. PowerShell:

```text
$env:KITKRAFT_WORKBENCH_PROFILE = 'C:\Modding\MO2-Starfield\profiles\KitKraft Dev'
cargo test --release -p kitkraft-ui-egui --test dev_profile -- --ignored --nocapture
```

- **Start** (`profiles`; the first tab, and the one shown when `kit-ui` starts without
  arguments)
  - The MO2 instance, as the engine's read-only `profiles` command finds it: the launcher's
    profile names its instance, else the one last used (UI prefs), else the engine looks at
    `KITKRAFT_MO2_DIR`, then the instances `kit doctor` detects, in its order
    (`C:\Modding\MO2-Starfield`, MO2's `CurrentInstance`, `%LOCALAPPDATA%\ModOrganizer`;
    "places searched" lists each). A different instance can be typed in; "Look again"
    reruns the command.
  - A Data-dir line: the instance's `gamePath` + `Data` (else `KITKRAFT_DATA_DIR`), and
    whether `Starfield.esm` is there.
  - One button per profile, each a read-only `load`. `Default` is marked "owner's live
    game, read-only"; `KitKraft Dev` "in-game checks run here"; MO2's selected profile and
    the last one used are noted. A click loads the profile, opens the Load tab and writes
    the choice to the UI prefs file.
  - On the dev box (screenshot tour, 2026-10-15): `C:\Modding\MO2-Starfield`, 3 profiles
    (`Default` marked live, `KitKraft Stack`, `Kitkraft Dev` marked as the dev profile and
    MO2's selected one; the dev match ignores case), Data
    `C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data` from the instance's
    `gamePath`, `Starfield.esm` found. The Ledger tour preview of `C3` named entry
    `game-20261015-01` (new) and wrote nothing; the Help tab rendered all four sections.
- **Load** (`load`)
  - Takes a profile or a plugin list.
  - The profile picker only lists the folders under an MO2 `profiles` directory. `Default`
    is marked as the owner's live game, which can be read but is never changed.
  - Result: one row per plugin with index, name, scale (`unknown` when it was not read),
    flags, record count, runtime slot, the rule that made it active, provider and cache
    state. Missing or late masters and parse errors are shown in red.
  - On KitKraft Dev: 84 plugins and 7,739,193 records loaded in 10 to 12 s from cache. The
    run reported 7 limitations, including the runtime-slot and `full-style-self`
    assumptions, `OldSovietMarksmanRifle.esm` active with no file, and 4 unresolved
    records in `HG_ShipCargoHoldAtHome.esm`.
- **Records**
  - Record types come from `schema`, which lists all 86 defined types. Alternatively,
    `coverage` on one loaded plugin counts records and typed, failed, opaque and unknown
    occurrences per type. Types with no definition are marked "subrecords unknown".
  - Clicking a type runs `query` for it, with a `where` box, winner or all scope, and
    paging.
  - Clicking a form runs `inspect` and then `schema --deep` for the same type. The record
    view shows each version in load order with its raw FormID, its class against the
    previous version (`vs_previous`) and the changed subrecords. Each subrecord shows its
    signature, member, decoded value, offset, size, and the schema member's confidence and
    source. `reference` members are marked unconfirmed, and signatures the schema does not
    name are shown as "not named by the schema", not as absent.
  - Example: `WEAP where FULL ~ 'Rifle'` gave 7 matches. On the first match, `inspect`
    showed decoded values with offsets, FormIDs as `Starfield.esm:12F677 [raw 0012F677]`,
    and one undecoded `QUPA` payload as hex.
- **Links** (`refs`)
  - Outgoing links of a form, breadth first to a depth (1 to 6), or "use info": the
    records whose typed members name it (incoming; winners or every version; a type
    filter). Every record row in the window (query rows, the record view, diff records,
    diagnostics, links and planet rows) has "links" and "use info", which open this tab.
  - One row per edge, read from its `link` or `referrer` marker: hop, the record holding
    the link, member path, raw FormID, status (anything but `resolved` or `null` in red),
    and the target. Forms are links that run `inspect`. A placed referrer shows its cell
    and world; the largest cells are listed separately. An edge with a marker the view
    does not know is counted, not guessed. Pages by `next_cursor`.
  - On KitKraft Dev, the first rifle (`Starfield.esm:02EB36` Tombstone): 146 outgoing
    links (115 resolved, 31 null: transforms, keywords, attach slots, template mods),
    instant. "Use info": 14 referrers, 6 leveled lists and 8 placed references (cells in
    Akila City, `LC006World`, and `SFBGS003.esm`'s Trackers Alliance HQ). That took 21 s
    with the reverse-link index built cold (84 plugins); warm it is under a second.
- **Planet** (`planet`)
  - The Planet Content Manager chain of one record. A tree, branch or content node shows
    its subtree indented by depth (order in the `PRVN` chain, child selection, condition
    count, and the content a node places, which runs `inspect`); a branch or content node
    also its ancestors. A planet, star, biome, location, worldspace or pack-in shows the
    content nodes that place it, each with its branch chain. Every `link` marker in the
    result is listed with where it sits, so a view without its own rows still shows its
    links. "planet chain" on the record view of a planet-family record opens this tab.
  - On KitKraft Dev: `PCM_QuestRequest` is a tree of 390 nodes (46 branches, 343 content
    nodes placing worldspaces, depth 4), 0 unresolved links, in under a second.
- **Diff**
  - Compares two plugins (`diff`) or one plugin against the versions before it
    (`conflicts`, which needs a load).
  - Each differing record is collapsible, with a row per field: subrecord and occurrence,
    change, field path, before and after.
  - FormIDs are shown as formrefs or `NULL`. Opaque members are shown as `opaque bytes @off
    len n`. Both are recognized by their `kind` marker (`formref`, `opaque`; M4.W0), never
    by the shape of the value: an object without a marker is a struct.
  - A subrecord compared only as bytes shows its byte spans for `diff`. For `conflicts` it
    shows "range not reported", because that command returns no spans. Identical byte-only
    rows of one signature are folded into one row such as `NVMI x12 (#0..#11)`.
  - The result can be paged.
  - On the biggest plugin the profile activates: 379 overrides of `Starfield.esm`. The
    first page had 67 records that differ and 26 identical to the previous version.
- **Diagnostics**
  - Runs `inspect` on a formref, EditorID, `runtime:<hex>`, or a hex FormID together with a
    plugin.
  - Shows the identity, the defining plugin, the object ID and the runtime ID with its
    basis.
  - Shows a table of versions: plugin, scale (`unknown` when the load order could not read
    it), raw stored FormID, and `resolved_by`. Assumptions are highlighted.
  - Lists every FormID the winning version stores, raw and resolved, with `NULL` and
    `unresolved` shown as such. Clicking a resolved one diagnoses it.
  - Then runs `validate` with `masters.missing`, `masters.order` and `formid.dangling` on
    the winner's plugin, or on a plugin you choose.
  - On KitKraft Dev: 146 stored FormIDs on the first rifle. The masters and FormID rules
    found nothing.
- **Validate**
  - Runs `validate` with all rules or a chosen list. Findings are grouped by rule, with
    error and warning counts and colors. Each finding shows severity, record, subrecord,
    byte offset (decimal and hex) and message.
  - Also shows the FormIDs checked, the unresolved ones and the unchecked subrecords.
  - `roundtrip --verify` is a separate button. It shows the counts and the first mismatch.
  - On the same plugin: passed with 94 `navmesh.unchanged` warnings, 181,175 FormIDs
    checked, 48,619 subrecords unchecked. The roundtrip was identical: 14,865,464 bytes,
    56,610 records.
- **Schema** (`schema --deep`)
  - Shows the member tree for a type: members, groups, fields and variants, indented. Each
    row shows signature, kind, cardinality, type, confidence (`reference` marked
    unconfirmed) and source. Evidence citations and notes appear on hover.
  - With no type, it lists every definition, and clicking one opens it.
  - WEAP: 64 subrecord definitions (62 typed, 2 opaque; 9 `reference`), 322 rows deep.
- **Ledger** (`ledger`; the one write, ADR 0007 Amendment 2)
  - Records one in-game result in `docs/game-observations.md` of the checkout the Help tab
    found: the row (`C1`..`C14`, `POI`, `persistence`, `custom` with a label), `pass`,
    `fail` or `note`, the FormIDs (commas), what was seen, the profile (default `KitKraft
    Dev`; the engine refuses `Default`) and the date (empty: today in UTC).
  - For `C<n>` and `POI` it shows the ledger's own template row (`O5-<nn>`, `O2-06`): the
    check and what it expects. "from template" fills in that row's FormIDs.
  - **Preview** runs `ledger` without `confirm` and shows the text it would append, and
    which entry it goes into (`game-<yyyymmdd>-<nn>: workbench ledger, <date>, <profile>`,
    new or continued). Nothing is written.
  - **Append to the ledger...** opens a confirm box with the row as it will be written.
    **Confirm: append** sends the one confirmed request; each row needs its own click, and
    the note is cleared after a successful append. The kit line under the result is the
    CLI form (`kit ledger C3 pass --forms ... --confirm`).
- **Help** (no command)
  - The owner's QA material, read from the checkout's docs when the window opens or on
    Reload: "Owner next actions" (`plan/STATUS.md`), overhaul build 2 "In game"
    (`docs/overhaul/build-2.md`: install steps, the `player.additem` table, checklist
    C1..C14), the first POI's "In-game test plan" (`docs/poi/first-poi.md`) and the
    "Persistence protocol" (`docs/game-observations.md`).
  - Plain rendering: headings, paragraphs, lists, tables, code blocks. A click on any code
    (a `player.additem` line, a FormID, a batch line) copies it.
  - The checkout is, in order: the one set here ("Use this checkout", kept in the UI prefs),
    `KITKRAFT_REPO_DIR`, the folders above `kit-ui.exe` (a build under the checkout's
    `target`), the folders above the working directory. A test fails when a doc loses one
    of these sections.

## Screenshot tour (development aid)

Set `KITKRAFT_WORKBENCH_SHOTS=<dir>` and start `kit-ui` with a profile. The tour then
runs these read-only requests:

1. `query WEAP where FULL ~ 'Rifle'`.
2. `inspect`, "use info" (`refs --incoming`) and the diagnostics on its first match.
3. `planet PCM_QuestRequest` (a Planet Content Manager tree of `Starfield.esm`), 60 nodes.
4. `coverage`, `conflicts`, `validate` and `roundtrip --verify` on the biggest plugin the
   profile activates.
5. `schema --deep WEAP`.
6. `profiles` for the profile's instance (the Start tab, as at every start).
7. A `ledger` preview of `C3` with the template's FormIDs, and the confirm box open. The
   tour never clicks it: nothing is written.

A minimized window does not repaint, so the tour waits until it is restored (seen
2026-10-15: started from a background shell, the window opened minimized). The Help tab
shows the docs as read at start. It then saves `<dir>/<tab>.bmp` for each
tab (`start`, `load`, ..., `ledger`, `help`) and closes. Keep the images outside the repo.
The tour refuses a `<dir>` that is, or is inside, the game's Data folder, the user Data
folder (`Documents\My Games\Starfield\Data`), an MO2 instance or profile, or that passes
through a link (the engine's own output guard); the window then says why and runs no tour.

## Not offered yet

- `build --plan` (M4.W6). The gate refuses `build` in every mode for now. Opening it for
  `plan: true` needs an argument guard like the one for `roundtrip`.
- Placement context (M4.D2) is not on `main`.

The view-models read the inspection commands' shapes as `docs/shapes.md` documents them
(M4.W0), and format values by their `kind` marker. Their canned test envelopes for `refs`
and `planet` are checked against the engine's own shapes, so they cannot drift from what
the engine returns.
- Owner decisions on licenses: BSL-1.0 for the clipboard crates, and the optional
  `default_fonts` feature (ADR 0007, "Dependency list").
