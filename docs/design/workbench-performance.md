# Workbench performance: measurement protocol (M6 lane P)

**Status: protocol, with a first engine-side baseline.** Written 2026-10-16 for M6
"Cockpit" (`ROADMAP.md`, `plan/execution-plan.md` "M6", lane P). It says what to measure
in the workbench, how, and which numbers count as good enough. The targets are
**hypotheses**: M6.P1 measures first and confirms or revises them, and no performance
ticket (M6.P2..P7) merges without a before-and-after row in "Results" below.

Why: the owner's 2026-10-16 test ranked "look, feel and performance" as the top pain
(slow loads, cramped tables, the window opening minimized from a background shell and not
repainting). This document turns "slow" into numbers we can check.

## Rules

- **Release build only.** `cargo build --release -p kit -p kit-cli` (or the
  `dist\KitKraft` folder from `cargo xtask dist`). A debug `kit-ui` is about 5x slower to
  load a profile (`docs/workbench.md`, "Launch") and is never measured.
- **The dev profile only:** `C:\Modding\MO2-Starfield\profiles\Kitkraft Dev`, read-only.
  Never `Default` (AGENTS.md, "Owner's live game is off limits"). Record its plugin count
  and record count with each run, because the profile changes (84 plugins on 2026-10-15,
  16 on 2026-10-16).
- **Build identity.** Record the SHA-256 of `kit.exe` and `kit-ui.exe` and the commit they
  were built from. `kit --version` prints `0.0.0` today and no commit, so record the commit
  by hand until M6.P6 adds it to `kit --version` and the window's title bar.
- **Cold and warm index cache, kept apart.**
  - *Warm*: the engine's index cache (`%LOCALAPPDATA%\kitkraft\cache`, `docs/load-order.md`
    "Index cache") already holds every plugin. Run once to warm, then measure.
  - *Cold*: `kit load --no-cache` (CLI) or `KITKRAFT_CACHE_DIR` set to a new empty folder
    outside the repo (window). Never delete the user's cache to get a cold run. Delete the
    temporary folder afterwards.
  - The OS file cache is not flushed. Note whether the run was the first one after a
    reboot.
- **Engine time and window time, kept apart.** The CLI gives engine time
  (`data.elapsed_ms`, `index_ms`, `table_ms` in `load`, "in N ms" in `query`). The window
  adds queueing, envelope transfer, view-model preparation and drawing. M6.P1 adds that
  instrumentation: with `KITKRAFT_WORKBENCH_PERF=1`, `kit-ui` prints `perf:` lines to
  stderr. It writes no file: `kit-ui` is a console program, so redirect stderr to keep
  them.
- **Machine.** The dev box (Windows 11, `docs/QUICKSTART.md`). Record whether
  `Starfield.exe`, `CreationKit.exe` or `ModOrganizer.exe` was running, since each takes
  memory and disk.
- Three runs per row. Report the median, and the maximum when it is more than 1.5x the
  median.

## What to measure

| Id | Measure | How | Target (hypothesis) |
| -- | ------- | --- | ------------------- |
| S1 | Process start to first painted frame, no arguments (Start tab) | `perf: first-frame` | under 1.0 s |
| S2 | Window state at start from a background shell | Start `kit ui --dev` from a shell that does not have focus; look | visible, restored, focused; never minimized (M6.P7) |
| S3 | `profiles` on the Start tab (MO2 instance scan) | `perf: request profiles` | under 0.3 s |
| L1 | Profile load, warm cache: CLI engine time | `kit load --profile <dev> --json`, `data.elapsed_ms` | baseline below |
| L2 | Profile load, warm cache: click to Load table drawn | `perf: request load` (queued, started, finished, prepared, first drawn) | L1 + 0.5 s |
| L3 | Profile load, cold cache: CLI engine time | `kit load --profile <dev> --no-cache --json` | baseline below |
| L4 | Profile load, cold cache: click to table drawn, with a progress line | `perf:` as L2, plus the time to the first progress event | L3 + 1.0 s; first progress under 0.5 s (M6.P2) |
| L5 | Memory after a warm load | Task Manager, private working set of `kit-ui.exe` | under 1.5 GB |
| I1 | The 435k-record plugin: `SFBGS00D.esm` (434,991 records on 2026-10-16; the owner's "435k-record index") through `coverage` | CLI time, then window click to drawn | engine baseline below; window + 0.5 s |
| I2 | `SFBGS00D.esm` through `conflicts --plugin` (one page of 50) | CLI time and envelope size, then the window | under 3 s and under 2 MB (needs the engine request M6.P5b) |
| I3 | `query REFR` with a page of 500, winner scope, on the profile | CLI time, window click to drawn | engine baseline below; window + 0.3 s |
| I4 | `refs --incoming` on the first rifle, reverse index cold and warm | CLI and window | cold baseline 21 s (84 plugins, `docs/workbench.md`); warm under 1 s |
| T1 | Scrolling a 10,000-row table (query rows, Links rows) | `perf: frame` p50, p95 and max while scrolling for 5 s | p95 under 16 ms, max under 50 ms |
| T2 | Scrolling the raw `data` JSON of the I2 envelope | as T1 | as T1 |
| T3 | Frame time while a long command runs (the engine is on its worker) | as T1, during L4 | no frame over 50 ms |
| T4 | Switching tabs with large results held (L2 + I2 + I3 loaded) | `perf: frame` on each switch | under 50 ms |
| U1 | Startup and frames at 125 % and 150 % Windows scaling | S1 and T1 again at each scale | no worse than 1.2x the 100 % numbers |

## First baseline (engine side, 2026-10-16)

Measured by the roadmap lane in under 10 minutes, CLI only (no window), all read-only.

- Build: `C:\Modding\kitcraft\bin\kit.exe`, SHA-256
  `a25ab516acb49bfaf5e0a5304b154df17e782d0cf8fe34e34e14214f84e62a06` (the same file as
  `dist\KitKraft\kit.exe`, built 2026-10-15 21:48; the commit is not embedded, see
  "Build identity"). `main` was at `27cff47` when measured. `dist\KitKraft\kit-ui.exe`:
  SHA-256 `6cd592e6fc930113ac688f773b5ed3103a4c7f85ee9981945bf479546652e5af` (not run).
- Profile: `Kitkraft Dev`, 16 plugins, 7,490,111 records. `ModOrganizer.exe` was running;
  `Starfield.exe` and `CreationKit.exe` were not.
- Single runs, so these are indications, not medians.

| Id | Command | Result |
| -- | ------- | ------ |
| L1 | `kit load --profile <dev> --json`, warm (14 of 16 hits; the 2 misses are KitKraft's own `out\` builds) | 0.65 to 0.73 s wall; `elapsed_ms` 652 (`index_ms` 298, `table_ms` 287) |
| L3 | `kit load --profile <dev> --no-cache --json` | 3.50 and 3.53 s wall; `index_ms` 3,095 and 3,183 |
| I1 | `kit coverage SFBGS00D.esm` | 2.67 s wall; 434,991 records of 100 types; envelope 0.63 MB |
| I2 | `kit conflicts --profile <dev> --plugin SFBGS00D.esm` (first page, 50 rows) | **26.7 s wall; envelope 29.8 MB** (`data.items` about 9 MB of JSON for 50 rows; 36,811 evidence ranges) |
| I3 | `kit query REFR --profile <dev> --limit 500` | 1.73 s wall (825 ms in the engine; 6,713,081 winners) |
| -- | `kit query WEAP --profile <dev> --where "FULL ~ 'Rifle'"` | 2.27 s wall (7 matches) |

What it suggests (to be confirmed by M6.P1 in the window):

1. A warm profile load is under a second in the engine. If the window takes much longer,
   the time is spent in the window (queueing, transfer, view-model preparation, drawing),
   not in indexing. The 10 to 12 s in `docs/workbench.md` were measured with 84 plugins.
2. A cold load is about 3.5 s for this profile, long enough to need a progress line
   (M6.P2) but not a redesign.
3. The large cost is `conflicts` on a big DLC master: a 30 MB envelope for one page. No
   UI change fixes that. M6.P5b asks the engine owner for a smaller default (evidence and
   field detail on request), and M6.P5a moves envelope preparation off the UI thread so
   the window stays responsive meanwhile.
4. `kit ui` from `C:\Modding\kitcraft\bin` exits 2 ("kit-ui.exe is not next to kit"):
   `bin\` holds `kit.exe` only. M6.P6 fixes the layout, so a release `kit-ui` is the one
   people start.

## Viewport (M7 lane R)

The 3D viewport (`plan/execution-plan.md`, M7 lane R) gets its own rows once M7.R1
lands. M7.R5 measures them first, under the rules above.

| Id | Measure | Target (hypothesis) |
| -- | ------- | ------------------- |
| V1 | Empty viewport (grid, axes, camera) frame time | p95 under 8 ms |
| V2 | Markers for the references of a large exterior cell | p95 under 16 ms with 5,000 markers |
| V3 | Bounding boxes: time from cell selection to all boxes drawn (engine `mesh.bounds`, cold and warm) | under 2 s cold, 0.3 s warm, for 1,000 references |
| V4 | Meshes: time to first mesh, time to the cell's budget, memory | first mesh under 0.5 s; budget reached under 5 s; under 1 GB GPU and process memory added |
| V5 | Meshes skipped for the budget | each one shown as a limitation line, never dropped silently |

Geometry comes through engine commands (M7.S4, M7.S5), never read by the window itself.
If V3 or V4 shows that a JSON envelope is too slow for vertex data, the remedy is an ADR
for a typed side channel that is still produced by the engine. The window must not start
reading archives itself.

## Results

One row per measurement run. M6.P1 adds the first window rows. Every lane P ticket adds
a before-and-after row for the measures it targets.

| Date | Ticket | Build (kit-ui SHA-256, commit) | Profile (plugins, records) | Cache | Measure | Median | Max | Notes |
| ---- | ------ | ------------------------------ | -------------------------- | ----- | ------- | ------ | --- | ----- |
| 2026-10-16 | roadmap | kit `a25ab516...` (CLI only), main `27cff47` | Kitkraft Dev (16, 7,490,111) | warm | L1 | 0.65 s | 0.73 s | engine 652 ms |
| 2026-10-16 | roadmap | as above | as above | cold | L3 | 3.50 s | 3.53 s | `--no-cache` |
| 2026-10-16 | roadmap | as above | as above | warm | I1 | 2.67 s | - | single run |
| 2026-10-16 | roadmap | as above | as above | warm | I2 | 26.7 s | - | 29.8 MB envelope; single run |
| 2026-10-16 | roadmap | as above | as above | warm | I3 | 1.73 s | - | single run |
