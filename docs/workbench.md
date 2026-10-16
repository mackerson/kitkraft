# Developer workbench

The workbench is a read-only egui window over `Engine::execute` (ADR 0007 Amendment 1,
plan M4 lane W). It is a way to look at the data, not a second implementation of anything.
Each view runs one engine command, or a fixed sequence of commands, and shows the result
envelope. The UI track continues in M6 "Cockpit", M7 "CK parity" and M8 "Beyond the CK"
("Roadmap" at the end). One write is permitted (Amendment 2): the Ledger
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
detected).

From a checkout, **`cargo xtask ui`** is the one step (M6.P6a): it builds `kit` and
`kit-ui` with `--release`, stamped with the commit and date (below), and starts
`kit ui --dev` from cargo's `release` folder, so the window opens on the KitKraft Dev
profile. Arguments pass through to `kit ui` (`cargo xtask ui --theme studio`); with
`--profile` or `--plugins` the `--dev` is left out, and `--no-dev` opens the profile
picker instead. `cargo xtask dist` (with `--sync-bin` to refresh the main checkout's
`bin\` as well) builds the same release programs into `dist\KitKraft\`.

Use a release build. A debug build works but is slow and large: loading the KitKraft
Dev profile took 53 s and about 2 GB of memory in a debug build (the 10 to 12 s under
"Panels" below were warm-cache loads). So a debug `kit` (`cargo run -p kit-cli -- ui`)
does not start the debug window silently: it starts a release `kit-ui` when it finds one
(cargo's `release` folder beside its `debug` folder, else `bin\` or `dist\KitKraft\` in a
folder above it) and says which on the console; when there is none it prints where it
looked and how to get one (`cargo xtask ui`, `cargo xtask dist`) and exits 2.
`kit ui --allow-debug` runs the debug window beside it anyway. A debug `kit-ui` prints a
one-line warning saying so when it starts.

Which build is running: `kit --version` prints `kit 0.0.0 (<commit>, <date>, <profile>)`,
and every result envelope carries the same as `build` (`docs/mcp.md`, "Result
envelope"). The commit (`-dirty` when the tree had changes) and the date are set by
`cargo xtask ui` and `cargo xtask dist`; any other build says `unknown` for both, and the
profile is `debug` or `release` either way.

`--theme <preset>` picks the visual preset: `workbench` (the default, the owner's choice
M6.O1), `studio`, or `classic` (legacy, the look before the tokens), each optionally
`-light` or `-dark` (otherwise the window follows Windows). Without it, `"theme"` in
`%LOCALAPPDATA%\kitkraft\ui.json` is used. The Start tab's theme line has a picker (preset;
system, light or dark) that applies at once and writes `ui.json`. The presets and their
tokens are in `docs/design/workbench-visual-system.md`.

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

The engine runs on its own thread and handles one command at a time. The status line at
the bottom of the window shows the command running and how many are queued behind it, then
the session (the profile loaded and its plugin count); the top bar holds the tabs. Buttons and links that run a command
are disabled while one runs. **Drop pending** restarts the engine thread: queued requests
are dropped, the running command's reply is discarded (a command cannot be interrupted;
the old thread finishes it in the background), and the session is emptied, so load again.
If the window is closed during a `load`, the index-cache file it was writing is left as a
temp file; the next start removes it.

## What it can run (and what it cannot)

`crates/ui-egui/src/gate.rs` is the only module that holds the `Engine`. Every request is
checked by canonical id, so an alias cannot get around it (ADR 0007 Amendments 1 to 3;
M6.G1 and M6.G3):

- **Allowed:** `load`, `inspect`, `query`, `diff`, `conflicts`, `validate`, `coverage`
  (with `matrix`, the Coverage tab), `schema`, `refs`, `planet`, `profiles`,
  `deploy.status` and `doctor`, none of which the catalog marks `writes`. (`refs
  --incoming` fills the engine's reverse-link cache, as `load` fills the index cache.)
- **Guarded:** `roundtrip`, which writes with `out`, runs only with `verify: true`, and never
  with `out` or `overwrite`.
- **The permitted writes** (`gate::PERMITTED_WRITES`, inside `gate::COCKPIT_SCOPE`, the six
  commands ADR 0007 Amendment 3 names: `ledger`, `deploy`, `deploy.rollback`,
  `game.session`, `game.probe`, `game.stop`). Today three of them are permitted, each
  with a preview that passes like a read and a writing form that passes only as a request
  built by `Request::confirmed`, which one panel file per command makes, once per
  confirm click:

  | Command | Preview (writes nothing) | Writing form | Panel |
  | ------- | ------------------------ | ------------ | ----- |
  | `ledger` | `confirm` absent or false | `confirm: true`, confirmed | Ledger (`panels/ledger.rs`) |
  | `deploy` (`--dev`) | `dry_run: true` | `dry_run` absent or false, confirmed | Cockpit (`panels/cockpit.rs`) |
  | `deploy.rollback` | `dry_run: true` | as `deploy` | Cockpit |

  For `deploy` and `deploy.rollback` the gate also requires `dev: true` in every form,
  refuses `profile` = `Default` (any case) in every form, and requires `profile` in the
  writing form. The Cockpit fills `mo2` and `profile` from the profile the session
  loaded and keeps its confirm box closed when they differ from it, when the loaded
  profile is `Default`, or when the dry run is missing, refused, says nothing would
  change, or ran with other arguments. The engine refuses `Default` on its own, from
  MO2's active profile, whatever the arguments say. The engine itself refuses any ledger
  file but `<checkout>\docs\game-observations.md`.
  `game.session`, `game.probe` and `game.stop` are in the catalog (M4.T3) but not yet
  permitted: their Cockpit buttons stay disabled with M6.G4 or M6.G5 as the reason.
- **Refused before the engine sees the request:** every other catalog command and alias.
  That includes `set`, `build` (in every mode, including `--plan`), `new`, `clone`, `place`,
  `package`, `testplan`, `overhaul.scaffold`, `ba2.*`, `strings.*`, `papyrus.*`,
  `game.*` and (M7, ADR 0007 Amendment 4, proposed) the editor's writes.

Tests enforce this in six ways:

- Each catalog command must be classified as allowed, writing, or unused read-only.
- The allowlist must hold no command the catalog marks `writes`, the gate's writing list
  must equal the catalog's `writes` set, and a guarded command must refuse its writing
  arguments. The writing commands the window can run in a writing form must be exactly
  the ids of `PERMITTED_WRITES`; each must be in `COCKPIT_SCOPE`, in the catalog,
  `writes` and `Implemented`; `COCKPIT_SCOPE` is pinned to Amendment 3's six ids. A
  scratch test adds `testplan`, `set`, `package` and `revert` and checks that each fails
  the rule.
- Every refused id and alias is tried, and each must be refused (a permitted write in its
  writing form, unconfirmed). A confirmed request for any other command is refused.
- A scan of the crate's sources allows `Request::confirmed("<id>", ..)` only in `gate.rs`
  and the panel file of that id's `PERMITTED_WRITES` entry, with the id a string literal.
- Per permitted write: the preview passes unconfirmed, the writing form is refused
  unconfirmed and passes confirmed, `dev` and `Default` are checked, and the confirmed
  deploy and rollback reach the engine on a synthetic MO2 instance, where the engine
  refuses on its own when MO2's active profile is `Default`.
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
- **The dev deployment**, written by the engine's `deploy --dev` or `deploy.rollback` on a
  Cockpit confirm click: one isolated `KitKraft Dev - <plugin>` mod folder (and its
  `.previous`) in the loaded profile's MO2 instance, never a profile, `plugins.txt` or
  another mod (ADR 0009).
- **Screenshots**, only when you set the tour's environment variable (below).

`load` still fills the engine's own index cache, as it does for the CLI and MCP.

### Cockpit and Coverage (M6.G2, M6.G3, M6.G7, M6.G8)

- **Cockpit**: pick the plugin under review from the checkout's `out\` (or type a path).
  **Review** runs `validate`, `roundtrip --verify`, then the plugin against its masters
  (`conflicts --plugin` when the plugin is in the loaded order, with `detail: fields`
  once the engine has it; otherwise a `diff` of each master `validate` found against the
  plugin, which leaves the session's load order alone). Each step shows `not-run` until
  its reply arrives; the "validation" block fills AGENTS.md's six rows from what ran
  (`parse`, `byte_roundtrip`) and leaves the CK and game rows `not-run`. **Status** reads
  the dev mod (`deploy.status`); **Preview deploy** and **Preview roll back** are dry runs
  listing every path; **Deploy...** and **Roll back...** open the confirm box (closed for
  the reasons above), whose button sends the one confirmed request; after a write the
  panel reads the dev mod back. The ledger entries naming the plugin's file (or the
  SHA-256 the deploy preview reports) are listed. The game buttons (Launch and run
  probes, Stop, Re-run probe, Screenshot) take their state from the catalog and the gate.
- **Coverage**: `coverage --matrix` with optional record types, plugin and evidence file;
  one row per type with the six statuses as status components, a filter by type, and a
  click on a type lists each cell's evidence pointers and whether they resolve.

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
  - The tab has two panes: types and the query on the left, the record inspector on the
    right, each scrolling on its own; drag the line between them to resize (the share is
    kept in `ui.json` as `records_split`).
  - Clicking a form runs `inspect` and then `schema --deep` for the same type. The record
    view (right pane) shows each version in load order with its raw FormID, its class against the
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

- **Render Window** (no command yet; M7.R1, 2026-10-16)
  - A 3D viewport drawn with OpenGL through eframe's glow backend: a ground grid (lines
    every 256 units, major every 4,096, out to 8,192), the world axes (+X red, +Y green,
    +Z blue; Z up, game units), a horizon tint, and one placeholder box at the origin. No
    game data: M7.R2 adds the selected cell's references as markers from `cell`.
  - Camera: orbit on right-drag, pan on middle-drag, zoom on the wheel, W A S D to fly
    once the viewport has focus (click it; Shift is faster); "Reset view" goes back to
    the origin. These bindings are not observed against the CK (`docs/ck-parity.md`).
  - The stats overlay ("Stats") shows the viewport's CPU time per frame (last frame, and
    p50, p95 and max over the last 240), draw calls, frames drawn and the size. The
    viewport draws only on input (or once on request), so "frames" stays still while
    nothing moves.

## CK-style windows (M7.M0, M7.M1, M7.M4, M7.W3, M7.W5)

**Decision (M7.M0, lane M, 2026-10-16; director assumption pending the owner):** the
Creation Kit's windows are reproduced as floating `egui::Window`s inside the one main
window, not as a dock layout and not as separate OS windows. Each is toggled from the
View menu and remembers whether it is open, its position and its size in `ui.json`
(`ck_windows`, written when one opens, closes, moves or resizes; never by the tour). The
existing tabs stay as they are and each can also float as such a window (View >
Workbench tabs as windows); a floating tab is drawn only in its window, and the central
panel falls back to the last docked tab. Why: egui's own windows need no new crate (a
docking crate such as `egui_dock` would be a new dependency for ADR 0007 to list), they
keep the cockpit tabs intact, and no CK sitting has yet shown whether the CK docks its
windows. A dock layout comes only if sitting 1 (M7.O2) shows the CK's docks. Code:
`crates/ui-egui/src/menu.rs`, `src/windows/` (the glue `shell.rs` is a child module of
`app.rs`, which calls it through one-line hooks), `src/dialogs/`, and the layout files in
`crates/ui-egui/layouts/`.

- **Menu bar (M7.M1)**, above the tabs, built from the catalog: one item per command, in
  the top-level menu of its `menu_path`, in catalog order, the catalog's shortcut shown
  (bound by M7.M5). The CK's menus come first in the order `docs/ck-parity.md` records,
  which is a recollection, `unresearched` until sitting 1 (each such menu says so in its
  first line); KitKraft's own menus (Record, Tools, Archive, Papyrus, Strings, Game) come
  before Help; a CK menu with no command yet (World, NavMesh, Character, Gameplay) shows a
  disabled line saying why. An item opens the tab or window that runs its command and
  runs nothing itself. Disabled items carry a word beside them and the reason on hover:
  `writes` (outside ADR 0007 Amendment 3's scope, or in it but not yet permitted),
  `deferred` or `planned`, or `kit / MCP` (a read the window does not offer).
- **Warnings (M7.M4)**: the load order's problems (missing or late masters, plugins that
  did not load, active plugins with no file), the `load` limitations, and the Validate
  tab's last `validate` findings, one list with severity as glyph and word, a text filter,
  severity toggles, copy (tab-separated) and a Validate button over the loaded plugins;
  double-click a finding to inspect its record.
- **Use Info (M7.W5)**: `refs --incoming` of the inspected record, following the
  selection while open (one request per new selection), paged; limitations folded under
  one line; double-click a referrer to inspect it.
- **Edit dialog (M7.W3)**: the inspected record's winning version, read-only, its typed
  fields grouped by tab and group per `crates/ui-egui/layouts/<TYPE>.toml` (WEAP, ACTI,
  LCTN, TERM, FACT, PKIN, CELL, REFR ship now); any other type gets a default layout
  built from its `schema` (one group per schema group). Every tab and group name is
  KitKraft's own and marked "layout unobserved" until sitting 2 (M7.O3); labels are the
  schema's member names with the signature beside them; a subrecord the layout does not
  place shows on an "Other" tab. OK stays disabled until M7.E2 (after the owner rules on
  ADR 0007 Amendment 4, M7.O4).

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
8. The Cockpit's review of the first plugin under the checkout's `out\`, `deploy.status`
   for its dev mod and a `deploy --dev --dry-run` against the loaded profile (dry run:
   nothing is written; on an MO2 whose active profile is `Default` the engine refuses it,
   and the panel shows the refusal).
9. `coverage --matrix --types WEAP AMMO OMOD` over the official files (the Coverage tab).

A minimized window does not repaint, so the tour waits until it is restored (seen
2026-10-15: started from a background shell, the window opened minimized; started with
PowerShell's `Start-Process -WindowStyle Normal` it opens restored, 2026-10-16). The Help
tab shows the docs as read at start. It then saves `<dir>/<tab>.png` for each tab
(`start`, `load`, ..., `ledger`, `help`, `cockpit`, `coverage`, `render window`), then the CK-style shell's images (`ck-menu-file`, `ck-menu-view`, `ck-menu-record`, `ck-menu-navmesh`, `ck-warnings`, `ck-use-info`, `ck-edit-weap`, `ck-edit-default` for `Starfield.esm:000007`, and `ck-layout` with every window open and the Schema tab floating), and closes. The tour's window is the usual 1400 x
900 points; the Records tab folds "Record types", so the query table and the selected
record's inspector (the right pane) are in one image, and the scroll is not animated. Add
`--theme <preset>` to capture a preset (the M6.V2 mockups were made so; they were removed
after M6.O1 and are in the history at `48beef1`). Keep tour images outside the repo.
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

## Roadmap

Owner decisions of 2026-10-16 (`ROADMAP.md`, "M6 onward"; tickets in
`plan/execution-plan.md` "M6", "M7", "M8 (sketch)"):

- **M6, Cockpit.** The workbench becomes the human cockpit beside the agents. It gets a
  KitKraft visual system (tokens for light and dark, statuses that are not color-only,
  `docs/design/workbench-style.md`). It gets measured performance work
  (`docs/design/workbench-performance.md`), fixes to the window quirks (restored if it
  starts minimized, geometry kept in `ui.json`) and a release build by default
  (accessibility, scaling and first-run help are deferred, lane X). It also gets the game
  loop through engine commands only: deploy to dev and roll back, then launch, probes and
  screenshots through lane C's `game.session --start`, `game.status <id>` (polled through
  the catalog, so the one engine worker is never held for a whole session), `game.probe`
  and `game.stop`, which are disabled with "M4.T3" until they exist, then a prefilled
  ledger. Its writes are limited to `ledger`, `deploy --dev`, `deploy.rollback`,
  `game.session`, `game.probe` and `game.stop` by ADR 0007 Amendment 3, which the owner
  **accepted** on 2026-10-16 (M6.O3). This page's "What it can run" changes as M6.G1 and
  M6.G3 to M6.G5 merge. Until then the ledger is still the only write. A read-only 3D placeholder (M7.R1) may arrive during M6.
- **M7, CK parity.** The Creation Kit's menus, Data dialog, Object Window, Cell View, edit
  dialogs, Preferences and Warnings window, reproduced from black-box observation and
  recorded in `docs/ck-parity.md` "Interface parity". The Render Window is a read-only 3D
  viewport on the glow backend. Editing goes through `set`, `new`, `clone`, `place` and
  `build`, under ADR 0007 Amendment 4.
- **M8, Beyond the CK.** Search everywhere, inline validation and conflicts, undo
  history, an agent panel, and probe results beside records.

Known today (2026-10-16): `C:\Modding\kitcraft\bin` may still hold `kit.exe` without
`kit-ui.exe` from an older build, so `kit ui` started from there exits 2 and says how to
get one: `cargo xtask dist --sync-bin` puts both programs in `bin\` (M6.P6a).
