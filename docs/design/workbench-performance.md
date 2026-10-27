# Workbench performance: measurement protocol (M6 lane P)

**Status: protocol, with an engine-side baseline and the first window measurements
(M6.P1, "Window measurements" at the end).** Written 2026-10-16 for M6
"Cockpit" (`ROADMAP.md`, `plan/execution-plan.md` "M6", lane P). It says what to measure
in the workbench, how, and which numbers count as good enough. The targets are
**hypotheses**: M6.P1 measures first and confirms or revises them, and no performance
ticket (M6.P2..P7) merges without a before-and-after row in "Results" below.

Why: on 2026-10-16 the owner chose "look, feel, performance" as one of the two top
pains. The director's brief details it (slow loads, cramped tables, the window opening
minimized from a background shell and not repainting). This document turns "slow" into
numbers we can check.

## Rules

- **Release build only.** `cargo build --release -p kit -p kit-cli` (or the
  `dist\KitKraft` folder from `cargo xtask dist`). A debug `kit-ui` is about 5x slower to
  load a profile (`docs/workbench.md`, "Launch") and is never measured.
- **The dev profile only:** `C:\Modding\MO2-Starfield\profiles\Kitkraft Dev`, read-only.
  Never `Default` (AGENTS.md, "Owner's live game is off limits"). Record its plugin count
  and record count with each run, because the profile changes (84 plugins on 2026-10-15,
  16 on 2026-10-16).
- **Build identity.** Record the SHA-256 of `kit.exe` and `kit-ui.exe` and the output of
  `kit --version`: `kit 0.0.0 (<commit>, <date>, <profile>)` since M6.P6b, with `-dirty`
  after a commit built from a tree with changes. Build with `cargo xtask ui` or
  `cargo xtask dist`, which set the commit and date; another build says `unknown` and is
  not measured. Every envelope carries the same as `build`. The window's About line will
  read it too (lane V).
- **Progress.** `kit --progress` prints one stderr line per progress event (M6.P2b:
  `load` sends one per plugin indexed, after a first `index 0/N` before indexing starts);
  L4's "first progress" is the time to the first such line.
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
| I1 | The largest DLC master: `SFBGS00D.esm`, 434,991 records (`docs/corpus.md`), through `coverage` | CLI time, then window click to drawn | engine baseline below; window + 0.5 s |
| I2 | `SFBGS00D.esm` through `conflicts --plugin` (one page of 50) | CLI time and envelope size, then the window | under 3 s and under 2 MB (needs the engine request M6.P5b) |
| I3 | `query REFR` with a page of 500, winner scope, on the profile | CLI time, window click to drawn | engine baseline below; window + 0.3 s |
| I4 | `refs --incoming` on the first rifle, reverse index cold and warm | CLI and window | cold baseline 21 s (84 plugins, `docs/workbench.md`); warm under 1 s |
| T1 | Scrolling a 10,000-row table (query rows, Links rows) | `perf: frame` p50, p95 and max while scrolling for 5 s | p95 under 16 ms, max under 50 ms |
| T2 | Scrolling the raw `data` JSON of the I2 envelope | as T1 | as T1 |
| T3 | Frame time while a long command runs (the engine is on its worker) | as T1, during L4 | no frame over 50 ms |
| T4 | Switching tabs with large results held (L2 + I2 + I3 loaded) | `perf: frame` on each switch | under 50 ms |
| U1 | *(optional, with M6.O4's optional scaling check; lane X is deferred)* Startup and frames at 125 % and 150 % Windows scaling | S1 and T1 again at each scale | no worse than 1.2x the 100 % numbers |

## First baseline (engine side, 2026-10-16)

Measured by the roadmap lane in under 10 minutes, CLI only (no window), all read-only.

- Build: `C:\Modding\kitcraft\bin\kit.exe`, SHA-256
  `a25ab516acb49bfaf5e0a5304b154df17e782d0cf8fe34e34e14214f84e62a06` (the same file as
  `dist\KitKraft\kit.exe`, built 2026-10-15 21:48; the commit is not embedded, see
  "Build identity"). `main` was at `27cff47` when measured. `dist\KitKraft\kit-ui.exe`:
  SHA-256 `6cd592e6fc930113ac688f773b5ed3103a4c7f85ee9981945bf479546652e5af` (not run).
- Profile: `Kitkraft Dev`, 16 plugins, 7,490,111 records. `ModOrganizer.exe` was running;
  `Starfield.exe` and `CreationKit.exe` were not.
- Single runs, so these are indications, not medians: this baseline did not apply the
  three-run rule above. A review re-ran it on the same `kit.exe` (SHA-256 `a25ab516...`)
  the same day; its runs are in "Results" and widen the `conflicts` row to a range.

| Id | Command | Result |
| -- | ------- | ------ |
| L1 | `kit load --profile <dev> --json`, warm (14 of 16 hits; the 2 misses are KitKraft's own `out\` builds) | 0.65 to 0.73 s wall; `elapsed_ms` 652 (`index_ms` 298, `table_ms` 287) |
| L3 | `kit load --profile <dev> --no-cache --json` | 3.50 and 3.53 s wall; `index_ms` 3,095 and 3,183 |
| I1 | `kit coverage SFBGS00D.esm` | 2.67 s wall (the review: 2.91 s); 434,991 records of 100 types; envelope 0.63 MB |
| I2 | `kit conflicts --profile <dev> --plugin SFBGS00D.esm` (first page, 50 rows) | **26.7 to 41.6 s wall** across runs under different load (this lane 26.7 s; the review 38.6 s alone, 41.6 s beside other work; the solo run is 1.45x this lane's); **envelope 29,788,408 bytes (29.8 MB)** in every run (`data.items` about 9 MB of JSON for 50 rows; 36,811 evidence ranges) |
| I3 | `kit query REFR --profile <dev> --limit 500` | 1.73 s wall (825 ms in the engine; 6,713,081 winners) |
| -- | `kit query WEAP --profile <dev> --where "FULL ~ 'Rifle'"` | 2.27 s wall (7 matches) |

What it suggests (to be confirmed by M6.P1 in the window):

1. A warm profile load is under a second in the engine. If the window takes much longer,
   the time is spent in the window (queueing, transfer, view-model preparation, drawing),
   not in indexing. The 10 to 12 s in `docs/workbench.md` were measured with 84 plugins.
2. A cold load is about 3.5 s for this profile, long enough to need a progress line
   (M6.P2) but not a redesign.
3. The large cost is `conflicts` on a big DLC master: a 30 MB envelope for one page, and
   26.7 to 41.6 s depending on what else the machine is doing. No
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
It travels as base64 little-endian `f32` and `u32` buffers inside the JSON envelope, made
by the engine so the CLI and MCP get identical results, at about 1.33x the raw bytes
(director decision 2026-10-16, `plan/execution-plan.md` "M6"). V4 measures that
encoding. Only if V4 fails with it is the remedy an ADR for a typed side channel that is
still produced by the engine. The window must not start reading archives itself.

## Results

One row per measurement run. M6.P1 adds the first window rows. Every lane P ticket adds
a before-and-after row for the measures it targets.

| Date | Ticket | Build (kit-ui SHA-256, commit) | Profile (plugins, records) | Cache | Measure | Median | Max | Notes |
| ---- | ------ | ------------------------------ | -------------------------- | ----- | ------- | ------ | --- | ----- |
| 2026-10-16 | roadmap | kit `a25ab516...` (CLI only), main `27cff47` | Kitkraft Dev (16, 7,490,111) | warm | L1 | 0.65 s | 0.73 s | engine 652 ms |
| 2026-10-16 | roadmap | as above | as above | cold | L3 | 3.50 s | 3.53 s | `--no-cache` |
| 2026-10-16 | roadmap | as above | as above | warm | I1 | 2.67 s | - | single run |
| 2026-10-16 | roadmap | as above | as above | warm | I2 | 26.7 s | - | 29.8 MB envelope; single run; with the review's runs the range is 26.7 to 41.6 s |
| 2026-10-16 | roadmap | as above | as above | warm | I3 | 1.73 s | - | single run |
| 2026-10-16 | review re-run | kit `a25ab516...` (same file, CLI only) | as above | warm | L1 | 0.70 and 0.73 s (two runs) | - | engine 625 ms; reproduces the roadmap row |
| 2026-10-16 | review re-run | as above | as above | cold | L3 | 3.64 s | - | `index_ms` 3,202; single run; reproduces |
| 2026-10-16 | review re-run | as above | as above | warm | I1 | 2.91 s | - | 434,991 records; single run; reproduces |
| 2026-10-16 | review re-run | as above | as above | warm | I2 | 38.6 s alone; 41.6 s under concurrent load | - | envelope 29,788,408 bytes, as the roadmap run; the solo run is 1.45x the roadmap's 26.7 s |

| 2026-10-16 | M6.P5b before | kit `c980c786...` (CLI only, release), branch of `5d03d01` | Kitkraft Dev (16, 7,490,111) | warm | I2 | 2.06 s | 2.49 s | `conflicts --plugin SFBGS00D.esm`, stdout to a file; envelope 28,749,976 bytes (122,934 subrecord differences on the page, 36,811 evidence ranges); `ModOrganizer.exe` running |
| 2026-10-16 | M6.P5b after | kit `59314faf...` (CLI only, release), lane D round 3 | as above | warm | I2 | 1.60 s | 1.93 s | `detail: summary` (the new default): envelope 51,717 bytes; `detail: fields` 22,148,397 bytes in 2.56 s; `detail: full` 28,758,276 bytes in 2.81 s (single runs) |
| 2026-10-16 | M6.P3b before | kit `c980c786...` | as above | cold / warm | I4 | 34.5 s cold; 1.52 s warm | - | `refs Starfield.esm:02EB36 --incoming` (Tombstone, 14 referrers); cold = fresh cache dir (record index and reverse index built: index 28.8 s); warm = the next two processes (reverse index from disk, 257 ms) |
| 2026-10-16 | M6.P3b after | kit `59314faf...` | as above | cold / warm / session | I4 | 31.9 s cold; 1.04 s warm; 1 ms second call in one session | - | as above, reverse index format 2 (VMAD objects, M5.D6): cold index 26.8 s, warm 174 ms from disk; in one session (`Engine`, MCP, the window) the index is kept with the load order and the second call takes 1 ms (`tests/lane_d3_corpus.rs`, Starfield.esm alone: first 220 ms from the disk cache, second 1 ms) |
| 2026-10-16 | M7.R2 | kit-ui `344caf4b...` (release), lane R round 2 branch | Kitkraft Dev (16) | warm | V2 | 0.06 ms CPU, 0.23 ms GPU at 20,000 markers | 1.30 ms (the upload frame) | instanced (one call per shape for faces and edges, 9 draws in all), see-through faces, 1384 x 764 px, RTX 4090; markers tiled from `CityNewAtlantisApartmentTEMPLATE` with `KITKRAFT_VIEWPORT_STRESS` (the screenshot tour's frames, 4 per run, single runs). Beyond the cap: 100,000 markers 0.09 ms CPU / 1.20 ms GPU (upload frame 9.0 ms); 300,000 0.08 / 3.36 (18.3); 1,000,000 0.03 / 13.9 (45.7). GPU time grows about 14 us per 1,000 markers, so 60 fps holds to about 1,000,000 on this GPU; V2's target (p95 under 16 ms with 5,000) holds with room |
| 2026-10-16 | M7.R2 | as above | as above | warm | draw cap | 20,000 references | - | `DRAW_CAP` in `panels/render.rs`: set by fetching, not drawing (above). A page of 1,000 references with `with_bounds` costs about 0.1 s in the engine once the reverse index is warm (`cell --world akilacity --x 1 --y 1 --limit 1000`, three CLI runs: `elapsed_ms` 283, 303, 285 with bounds, 273 to 282 without, of which 172 to 188 ms loading the reverse index from disk); the largest cell met (akilacity 1,1, 8,824 references) is 9 pages, 20,000 would be 20. A larger cell shows a limitation line and the first 20,000 |
| 2026-10-16 | M7.R3 | as above | as above | warm / cold | V3 (OBND variant) | about 0.1 s engine time per 1,000 references warm in a session; 0.28 s per CLI process | - | the bounds come from `OBND` through `cell --with-bounds` (each distinct base read once), not from `mesh.bounds` (blocked on M7.O5). Cold, the first `cell` of a load order builds the reverse index (`CityNewAtlantisApartmentTEMPLATE` on Starfield.esm alone: 23.6 s of a 23.7 s call, as I4 cold) |
| 2026-10-16 | M6.P1 before (window) | kit-ui `ddd8e1c4...`, `753eeb9` (main `54fddac` + the trace) | Kitkraft Dev (16, 7,490,111) | warm / cold | see "Window measurements" | | | three runs each; the window table below |
| 2026-10-16 | M6.P2a..P7 after (window) | kit-ui `96b544c1...`, `36848f7` | as above | warm / cold | see "Window measurements" | | | three runs each, same bench, same machine state |
| 2026-10-16 | after, merged with main | kit-ui `eb21a11a...`, `0e664d7` | as above | warm / cold | see "Window measurements" | | | Diff now asks `conflicts` for `detail: fields` (9.0 MB instead of 13.5 MB) |
| 2026-10-27 | M6.P5a raw before | release test build, commit `a63c6b3` (no kit-ui SHA-256) | none (synthetic envelope) | none | `Prepared::new` on the synthetic envelope: heap_bytes=47882862 (500,000 lines kept) | 96.43 ms | - | Linux sandbox, release test build, synthetic envelope (150,000 rows, ~30 MB; 600,006 pretty lines truncated to 500,000); no profile, no cache, no window; median of 5 runs. The window rows (T2, I2 in the window) are the owner's to re-run |
| 2026-10-27 | M6.P5a raw after | release test build, commit `d97b712` (no kit-ui SHA-256) | as above | as above | as above: heap_bytes=27799950 (500,000 lines kept) | 89.59 ms | - | as above; the one buffer (`RawLines`) cuts the raw lines' heap from 47,882,862 to 27,799,950 bytes (-42%) and `Prepared::new` from 96.43 ms to 89.59 ms |
| 2026-10-27 | M6.P6a debug opt-level 0 | debug test build, commit `e1c4151` (no kit-ui SHA-256) | none (headless bench: 200 frames over a 100,000-row table, one wheel step per frame) | none | `headless_frame_bench` frame time | p50 1.30 / p95 1.60 ms | 3.27 ms | Linux sandbox, headless egui, no GPU, no window; median of 3 runs; a fresh `cargo build --workspace --tests` takes 54.79 s wall |
| 2026-10-27 | M6.P6a debug opt-level 2 | as above, `[profile.dev.package."*"] opt-level = 2` (reverted after measuring) | as above | none | as above | p50 0.12 / p95 0.19 ms | 0.81 ms | as above; dependencies at opt-level 2, workspace crates at the dev defaults (the bench still reports `profile=debug`); the fresh build takes 97.20 s wall (+77 percent) |
| 2026-10-27 | M6.P6a release | release test build, commit `e1c4151` | as above | none | as above | p50 0.05 / p95 0.08 ms | 0.65 ms | as above; release profile, for reference |

Neither the roadmap baseline nor the review re-run applied the three-run rule; M6.P1's
rows are the first that must. The lane D round 3 rows are medians of three CLI runs
(stdout to a file). The 26.7 to 41.6 s I2 of the first baseline did not reproduce on
this build and profile (2.0 to 2.5 s for the same 28.7 MB envelope); the earlier runs
used the 2026-10-15 `bin\kit.exe` and may have printed to a console, so the engine time
was never the 30 s. `detail: summary` meets I2's target (under 3 s and 2 MB). I4's warm
target (under 1 s) holds in the engine (174 ms) and in a session (1 ms); a whole CLI
process with the profile load is 1.0 s.

## Window measurements (M6.P1, lane P window side, 2026-10-16)

**How.** `KITKRAFT_WORKBENCH_PERF=1` prints `perf:` lines to stderr (`crates/ui-egui/src/perf.rs`
lists them): time since process start, request queued, done (wait, engine run, first
progress event, envelope bytes), prepared (where and how long), drawn (click to the first
frame that drew it) and the frame time of that frame, and frame-time p50/p95/max per tab or
bench phase. Frame time is eframe's per-frame CPU time (layout, tessellation, painting;
without the vsync wait), counted once per frame. `KITKRAFT_WORKBENCH_PERF_BENCH=1` with
`--profile` runs one scripted pass and closes: after the startup `load`, `query REFR` with
1000 rows on Records, 1 s still, 3 s wheel-scrolling down and 2 s up over the query table
(T1), then `conflicts --plugin SFBGS00D.esm` on Diff (`KITKRAFT_WORKBENCH_PERF_DIFF`
picks the plugin) and 5 s of scrolling the Diff tab. S1 and S3 come from a launch without
arguments, killed after 6 s. The scripts are not in the repo (`observe\laneP\`, ignored);
they set `KITKRAFT_CACHE_DIR` to a lane folder (warm: warmed once by `kit load`; cold: a
new empty folder per run, deleted afterwards).

**Machine and builds.** The dev box; `ModOrganizer.exe` running, `Starfield.exe` and
`CreationKit.exe` not; other lanes were compiling at the same time (6 to 13 `cargo` and
`rustc` processes), which explains the spread of some maxima. Release builds stamped with
`KITKRAFT_BUILD_COMMIT`/`KITKRAFT_BUILD_DATE` as `cargo xtask ui` does: before
`kit 0.0.0 (753eeb9, 2026-10-16, release)`, kit-ui SHA-256
`ddd8e1c40f6e12c8512802d00b11e788bc828a6361f1cd71f523e8aa898a5278`, kit
`434d20aa8d5db2ccedf0ac2ab87c4f9673369500dd070e7ba0e0c4286b4d9c87`; after
`kit 0.0.0 (36848f7, 2026-10-16, release)`, kit-ui
`96b544c199fc016329672b5f3c5e238e792f5ecfbcf37857c78a822b7e5c3522`, kit
`a3715d98fb4c85fd3401996a432ce75e70a6f3ef3e5aface36824a31f0b70bbd`; merged
`kit 0.0.0 (0e664d7, 2026-10-16, release)`, kit-ui
`eb21a11af84869e2ae156bc50a3f96e6f8cdd5ec166a93a7f10b202a44346212`. Profile Kitkraft Dev,
16 plugins, 7,490,111 records. Not the first run after a reboot.

Medians of three runs (max in brackets when over 1.5x the median):

| Id | Measure | Before (`753eeb9`) | After (`36848f7`) | Merged (`0e664d7`) | Target |
| -- | ------- | ------------------ | ----------------- | ------------------ | ------ |
| S1 | Process start to first frame painted (Start tab) | 0.38 s | 0.35 s | 0.40 s (0.88 s) | under 1.0 s: **confirmed**; about 0.3 s of it is eframe and OpenGL creating the window, before any workbench code |
| S2 | Window state from a background shell | not minimized, **not focused** (all runs) | not minimized; not focused at the first frame | not minimized; **focused 65 to 90 ms after the first frame** (all runs; the start-up `Focus` request) | visible, restored, focused: **met** (owner check stays M6.O4) |
| S3 | `profiles` on the Start tab, queued to drawn | 84 ms | 77 ms | 80 ms | under 0.3 s: **confirmed** |
| L1 | Warm load, CLI engine time (`elapsed_ms`) | 781 ms (wall 0.89 s) | 651 ms (0.73 s) | 615 ms (0.71 s) | baseline (0.65 s): reproduces |
| L2 | Warm load, launch queue to Load table drawn | 0.96 s | 0.75 s | 0.75 s | L1 + 0.5 s: **confirmed** (the window adds about 0.1 s: the load waits behind `schema` and `profiles`, about 55 ms, and the first frame) |
| L3 | Cold load, CLI engine time | 3.99 s | 3.81 s | 3.60 s (6.13 s) | baseline (3.5 s): reproduces under load |
| L4 | Cold load, queued to drawn; first progress event | 4.44 s (7.25 s); first progress 48 ms | 4.41 s; 43 ms | 6.52 s (load from other lanes; fastest 4.01 s); 91 ms (one run 2.7 s) | L3 + 1.0 s and first progress under 0.5 s: **confirmed**; the progress is now on screen (M6.P2a) |
| T1 | Scrolling the 1000-row `query REFR` table (the engine's page maximum; no table in the window reaches 10,000 rows), frame time p50 / p95 / max, warm | 13.1 / 19.7 (51.3) / 90.3 ms | **0.7 / 0.9 / 1.7 ms** | 1.2 / 1.4 / 1.7 ms | p95 under 16 ms, max under 50 ms: **failed before, met after** (M6.P4) |
| T1 | as above, cold-cache session | 11.1 (51.4) / 21.3 (75.1) / 85.0 ms | 0.7 / 0.9 / 1.7 ms | 1.1 / 1.4 / 4.4 ms | as above |
| T3 | Longest frame on the Load tab while the load runs, warm / cold | 68 / 64 ms | 48 / 48 ms (warm run 1: 470 ms, its first frame) | 69 (506) / 66 ms | no frame over 50 ms: **revised**: every maximum is the window's first frame (shader and font atlas set-up, 50 to 500 ms), not a frame during the load; the Load tab's frame p95 stays under 7 ms |
| R1 | Records first paint: the frame that receives the 1000-row query (0.36 MB), warm / cold | 44 / 61 (284) ms | **1.6 / 1.9 ms** | 1.8 / 2.6 ms | new row (M6.P5a): under 16 ms |
| R2 | Records query, click to drawn, warm | 1.30 s | 1.05 s | 0.97 s | the engine's 0.9 to 1.4 s dominates |
| D1 | Diff first paint: the frame that receives `conflicts --plugin SFBGS00D.esm` (13.5 MB; 9.0 MB at `detail: fields`), warm / cold | 351 (632) / 466 ms | **4.6 / 4.5 ms** | 6.4 / 6.6 ms | new row (M6.P5a): no frame over 50 ms while a big envelope arrives: **failed before, met after** |
| D2 | Diff, click to drawn, warm / cold | 1.74 (3.72) / 2.79 s | 1.86 / 1.92 s | 2.82 / 2.11 s | the engine (1.4 to 2.2 s here) dominates; the view and raw lines now take 0.26 s on the worker instead of the UI thread |
| T2 | Scrolling the Diff tab after D1, p95 / max, warm | 1.4 / 3.3 ms | 1.3 / 2.1 ms | 1.9 / 2.3 ms | as T1: met before and after (the raw JSON was already virtualized; the record grids now are too) |

What the numbers say, and what changed because of them:

1. The window adds little to the engine on a warm load (L2 - L1 is about 0.1 s): nothing
   to fix there beyond showing progress on a cold load (M6.P2a; the first event comes
   about 45 ms after the load starts).
2. The cost was in the frames: the 1000-row query table laid out every row each frame
   (T1 p95 20 ms, maxima near 100 ms), and the frame that received a reply built its
   view-model, pretty-printed the raw JSON and dropped the envelope on the UI thread (the
   Diff frame 350 to 470 ms, the query frame 44 to 61 ms). M6.P4 and M6.P5a fixed those;
   after them no measured frame except the very first exceeds 10 ms.
3. P3a (no second `load` of the same order) saves a whole L2 or L4 per repeated click;
   it is tested in the shell model (`app::tests`), not timed here.
4. Not measured yet (still open in M6.P1): L5 (memory), I1, I3 and I4 in the window, T4
   (tab switches with large results held), U1 (scaling, optional). The bench and the
   trace cover them once someone drives those requests.

Window quirks (M6.P7), checked in the trace: `--profile` and argument-less launches from a
background shell (`Start-Process`, also with `-WindowStyle Minimized`) never opened
minimized on this box; the screenshot tour launched that way finished all 13 tabs. If a
window does open minimized it is restored within the first 10 s, and the tour waits for
the restore instead of hanging. The window asks for focus once; Windows granted it in
every run (`window focus focused=Some(true)`).

## Debug builds and opt-level (M6.P6a)

**Question.** A debug `kit-ui` is about five times slower than release (`docs/workbench.md`
"Launch"), so `cargo xtask ui` builds release. Would `[profile.dev.package."*"]
opt-level = 2` in the workspace `Cargo.toml` (dependencies optimized, our crates not) make
debug runs usable, and what would it cost the gate? Adopt only if the gate's time grows by
under 10 percent. This ticket measures and recommends; it does not adopt (`Cargo.toml` is
unchanged).

**How.** `perf::tests::headless_frame_bench` (an ignored test, commit `e1c4151`): a headless
`egui::Context` without a window, `theme::apply` once, then `ctx.run_ui` for 200 frames with
a `RawInput` of `theme::WINDOW`'s size over a `virtual_table` of 100,000 rows with three
cells each, scrolling by one mouse-wheel step over the table's rectangle every frame. Three
runs per profile; the medians are below and in the Results rows. Build cost: a fresh
`cargo build --workspace --tests` in a new target folder (a gitignored `.scratch/` folder;
this sandbox has no writable `/tmp`), deleted afterwards, wall time.

| Profile | p50 | p95 | max |
| ------- | --- | --- | --- |
| debug, opt-level 0 (as today) | 1.30 ms | 1.60 ms | 3.27 ms |
| debug, dependencies opt-level 2 | 0.12 ms | 0.19 ms | 0.81 ms |
| release (reference) | 0.05 ms | 0.08 ms | 0.65 ms |

Machine: Linux sandbox, headless egui, no GPU, no window.

**Build cost.** The fresh build takes 54.79 s wall without the setting and 97.20 s with it:
**+77 percent**. The gate's dominant cost is compiling the dev-profile dependency tree (its
build, clippy and test steps all compile it), so the gate would grow by far more than 10
percent (inferred from the fresh build; the gate itself was not timed).

**Result.** With the setting the frame times fall about elevenfold (p50 1.30 ms to 0.12 ms):
debug runs would be usable. The 10 percent rule says **do not adopt** — the build cost grows
77 percent. The setting was reverted after measuring. The dev box (Windows) may differ, and
the window measurement there (a real window with a GPU, as the M6.P1 rows) is the owner's.
