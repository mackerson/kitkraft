# KitKraft project analytics: tokens, sessions, who did what

Generated 2026-10-16 from Claude Code transcripts under `C:\Users\macke\.claude\projects\C--\` (13 session files plus 98 subagent files) and the git history of `main` at `27cff47`. Token figures are counts reported by the API in each assistant message `usage` object; no dollar costs (no pricing source is in the repo). Transcripts were read as data; only the first 100 characters of each session's first prompt are quoted, to identify it.

## 1. Headline numbers

| Measure | Value |
|---|---|
| Session files (all projects in `C--`) | 13, 2026-08-21 to 2026-10-16 |
| KitKraft sessions | 3 (director `ac6575a7` plus two short side sessions), 98 subagents spawned from the director |
| KitKraft active days (UTC) | 3: 2026-10-14, 2026-10-15, 2026-10-16 |
| Tokens processed, all 13 sessions + subagents | 4,013.9M |
| Tokens processed, KitKraft only | 3,800.2M (94.7% of all) |
| KitKraft split | cache read 3,736.0M (98.3%), cache write 59.3M, uncached input 44,754, output 4.8M |
| KitKraft tokens, Opus 5.5 | 3,501.1M (output 4.2M, cache read 98.4%) |
| KitKraft tokens, Fable 5.1 | 296.8M (output 0.6M, cache read 97.2%) |
| KitKraft tokens, Haiku 4.5 | 2.3M (output 0.0M, cache read 92.7%) |
| Commits on main | 520 (441 non-merge, 79 merge), 2026-10-14 to 2026-10-16 UTC |
| Commits attributed to a transcript | 515 of 520 (99.0%) |
| Lines added / removed (non-merge commits) | +287,431 / -36,211 |
| Rust lines at HEAD, hand-written | 132,425 lines in 252 files (126,075 non-blank) |
| Rust lines at HEAD, generated | 71,299 in `crates/esm/src/generated` (2 files) + 75,225 in `@generated` golden fixtures (2 files) |
| `#[test]` functions | 940 (none in generated files) |
| ADRs / docs files / record defs | 11 ADRs (`plan/adr/`) / 66 files in `docs/` (48 .md) / 98 `crates/schema/defs/*.toml` (+ shared defs in `defs/common/`) |

"Tokens processed" sums every assistant turn's input, cache write, cache read and output. Cache reads dominate: each turn re-reads the whole cached context, so the number measures context carried, not text written.

## 2. Sessions

| Session | KitKraft | First (UTC) | Last (UTC) | Turns | Model(s) | Input | Cache write | Cache read | Output | Total | Agents spawned | First prompt (100 chars) |
|---|---|---|---|---:|---|---:|---:|---:|---:|---:|---:|---|
| `775b38eb` | no | 2026-08-21 16:10 | 2026-08-21 23:28 | 117 | Opus 5 | 232 | 860,601 | 20,346,643 | 166,229 | 21,373,705 | 0 | Hey claude! windows keeps failing an update, want to help me diag? |
| `f7f7eb19` | no | 2026-08-21 20:02 | 2026-08-22 15:17 | 200 | Opus 5 | 400 | 1,258,478 | 37,942,618 | 188,417 | 39,389,913 | 0 | Hey claude! check this out, and let's see if we can do it here - I've got the model pulled in lm stu |
| `cc0bafe5` | no | 2026-08-25 15:30 | 2026-08-25 15:31 | 5 | Opus 5 | 10 | 21,837 | 272,050 | 2,639 | 296,536 | 0 | Hey claude! Can you help me download all this? { "instructions": "Download each file using the expor |
| `711a5cb4` | no | 2026-08-27 04:07 | 2026-08-27 14:42 | 41 | Opus 5 | 82 | 224,817 | 4,947,873 | 86,565 | 5,259,337 | 0 | This windows box could use a clean up: can you please look at what packages are installed and what's |
| `2f7dfd84` | no | 2026-08-28 15:26 | 2026-08-28 16:17 | 18 | Opus 5 | 36 | 49,615 | 1,348,572 | 29,013 | 1,427,236 | 0 | Can you kill the adobe updater for me? Also let's make sure it never boots with windows again |
| `e67fe7d5` | no | 2026-09-09 02:08 | 2026-09-12 00:32 | 69 | Opus 5 | 138 | 472,460 | 8,254,658 | 103,937 | 8,831,193 | 0 | Psst, this within your moral params? https://www.makeuseof.com/how-to-install-nvidia-dlss-5-on-almos |
| `43b8e998` | no | 2026-09-10 14:23 | 2026-09-11 17:56 | 104 | Opus 5, Fable 5.1 | 1,288 | 619,483 | 18,949,786 | 127,599 | 19,698,156 | 0 | @"C:\Users\macke\Downloads\8x16_deck_BOM_and_build_sheet_FINAL.pdf" Hey claude! Can you help me pric |
| `8aa39285` | no | 2026-09-12 00:57 | 2026-10-06 02:14 | 357 | Fable 5.1, Opus 5, Opus 5.5 | 4,434 | 7,288,095 | 108,323,647 | 318,457 | 115,934,633 | 0 | Hey claude - can you help me figure out what's going on with my Starfield install? Any mods? All fil |
| `8589c709` | no | 2026-09-18 16:16 | 2026-09-18 16:26 | 14 | Opus 5 | 28 | 39,986 | 1,151,146 | 8,456 | 1,199,616 | 0 | Hey, I need to blast any / all grokbot bits from space. I'm getting this weird persistent auth issue |
| `ac6575a7` | director | 2026-10-14 14:15 | 2026-10-16 17:17 | 443 | Fable 5.1 | 6,538 | 3,167,480 | 187,392,563 | 412,381 | 190,978,962 | 102 | Hey: Hey, uh, I've just had an idea. I'd like you to go take a look in to the Artcraft saga and then |
| `2d966f9c` | yes | 2026-10-14 20:12 | 2026-10-14 20:12 | 7 | Fable 5.1 | 194 | 47,477 | 582,187 | 2,715 | 632,573 | 0 | Hey claude! can you help me test the following: Fresh-session MCP check (5 min). Open a new Claude C |
| `345611b0` | yes | 2026-10-15 20:25 | 2026-10-15 21:13 | 36 | Fable 5.1 | 972 | 211,405 | 6,783,736 | 57,241 | 7,053,354 | 0 | We've been working on the KitKraft project inside C:\Modding\kitcraft (note the c, old name) and I'm |
| `ffdf6346` | no | 2026-10-15 22:45 | 2026-10-15 22:45 | 4 | Opus 5.5 | 8 | 42,871 | 310,085 | 1,515 | 354,479 | 0 | Hey claude! Is jelllyfin running on this system? |

Turns are distinct assistant API messages (lines sharing a `message.id` counted once, usage taken as the per-field maximum). The director session is still running; its figures stop at 2026-10-16 17:17 UTC. Its final context was 262,966 tokens. Every director turn and the `345611b0` / `2d966f9c` side sessions carry `advisorModel: claude-fable-5-1`; advisor tokens are not broken out in the usage objects.

## 3. Tokens by model

**KitKraft (director, side sessions, all subagents)**

| Model | Turns | Uncached input | Cache write | Cache read | Output | Total | Cache-read share of input side | Share of total |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Opus 5.5 | 11,284 | 22,596 | 51,492,915 | 3,445,405,619 | 4,226,512 | 3,501,147,642 | 98.5% | 92.1% |
| Fable 5.1 | 992 | 22,012 | 7,692,238 | 288,456,628 | 585,355 | 296,756,233 | 97.4% | 7.8% |
| Haiku 4.5 | 18 | 146 | 163,384 | 2,102,413 | 1,314 | 2,267,257 | 92.8% | 0.1% |
| **Total** | | 44,754 | 59,348,537 | 3,735,964,660 | 4,813,181 | 3,800,171,132 | 98.4% | 100% |

**All 13 sessions and subagents**

| Model | Turns | Uncached input | Cache write | Cache read | Output | Total | Cache-read share of input side | Share of total |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Opus 5.5 | 11,339 | 22,706 | 51,752,425 | 3,452,755,814 | 4,275,608 | 3,508,806,553 | 98.5% | 87.4% |
| Fable 5.1 | 1,200 | 27,228 | 14,563,074 | 365,798,374 | 813,355 | 381,202,031 | 96.2% | 9.5% |
| Opus 5 | 665 | 1,330 | 3,747,897 | 117,155,137 | 755,731 | 121,660,095 | 96.9% | 3.0% |
| Haiku 4.5 | 18 | 146 | 163,384 | 2,102,413 | 1,314 | 2,267,257 | 92.8% | 0.1% |
| **Total** | | 51,410 | 70,226,780 | 3,937,811,738 | 5,846,008 | 4,013,935,936 | 98.2% | 100% |

**By role (KitKraft)**

| Role | Agents | Turns | Tool calls | Uncached input | Cache write | Cache read | Output | Total processed | Share | Cache-read share | Median final context | Commits on main |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Director (main session) | 1 | 443 | 390 | 6,538 | 3,167,480 | 187,392,563 | 412,381 | 190,978,962 | 5.0% | 98.3% | 262,966 | 32 |
| Opus writers | 80 | 11,229 | 14,100 | 22,486 | 51,318,395 | 3,439,116,924 | 4,195,946 | 3,494,653,751 | 92.1% | 98.5% | 386,257 | 482 |
| Fable reviewers | 16 | 506 | 933 | 14,308 | 4,265,876 | 93,698,142 | 113,018 | 98,091,344 | 2.6% | 95.6% | 258,844 | 0 |
| Haiku triage | 1 | 18 | 19 | 146 | 163,384 | 2,102,413 | 1,314 | 2,267,257 | 0.1% | 92.8% | 163,395 | 0 |
| Analytics (this report, Opus) | 1 | 55 | 62 | 110 | 174,520 | 6,288,695 | 30,566 | 6,493,891 | 0.2% | 97.3% | 174,530 | 0 |

The director pattern held: the director produced 5.0% of KitKraft tokens over 443 turns, while Opus writers carried 92.1% in their own contexts. The director's own final context was 262,966 tokens after 98 spawns, consistent with each subagent handing back a short report rather than its working context. Side sessions `345611b0` and `2d966f9c` are counted in the model tables above but not in this role table.

## 4. Subagents

All spawns came from the director session via the `Agent` tool (`subagent_type: general-purpose` throughout): 102 calls started 98 agents; the other 4 calls returned no agent and were issued again with the same description. Pairing: each started agent's `*.meta.json` `toolUseId` matches its `Agent` call, and each `<task-notification>` task id matches the subagent file name; all matched.

| Requested model | Isolation | Spawns |
|---|---|---:|
| opus | worktree | 77 |
| fable | none | 13 |
| opus | none | 8 |
| fable | worktree | 3 |
| haiku | none | 1 |

Two token measures are shown. **Final ctx** is `<subagent_tokens>` from the completion notification; it equals the context size of the agent's last turn (checked against the transcripts), not a running total. **Processed** sums every turn in the subagent transcript. Tool calls are counted from the transcript (they match `<tool_uses>`). Duration is the sum of `<duration_ms>` over the agent's notifications (an agent resumed by message notifies again).

| Start (UTC) | Agent | Description | Model | WT | Turns | Tool calls | Final ctx | Processed | Cache read | Output | Duration (min) | Commits on main | Outcome |
|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| 10-14 14:26 | `a4074a6a0` | Scaffold KitCraft repo docs+layout | Opus 5.5 | no | 29 | 45 | 172,813 | 3.6M | 93.9% | 46,596 | 13 | 0 | no commits (research or setup) |
| 10-14 14:35 | `aac088570` | Install Spriggit CLI and test | Opus 5.5 | no | 24 | 28 | 119,593 | 2.2M | 96.1% | 3,472 | 6 | 0 | no commits (research or setup) |
| 10-14 14:39 | `a456e1467` | Get kitcraft CI green, commit M0 | Opus 5.5 | no | 15 | 15 | 95,003 | 1.2M | 95.2% | 1,262 | 2 | 1 | landed on main |
| 10-14 14:39 | `a60525741` | Review kitcraft docs and ADRs | Fable 5.1 | no | 9 | 31 | 158,911 | 1.0M | 86.1% | 1,329 | 7 | 0 | review, read-only; fix round `a519baa99` |
| 10-14 14:43 | `a80d9ec64` | Lane A: esm container crate M1.1-M1.7 | Opus 5.5 | yes | 115 | 162 | 346,052 | 26.1M | 97.7% | 66,785 | 48 | 12 | landed on main |
| 10-14 14:43 | `aac2cf2e2` | Lane B: schema crate M1.8-M1.10 | Opus 5.5 | yes | 116 | 157 | 353,525 | 25.6M | 98.4% | 102,895 | 29 | 6 | landed on main |
| 10-14 14:43 | `a05fcf5c3` | Lane C: engine execute + CLI M1.11 | Opus 5.5 | yes | 45 | 71 | 178,816 | 6.2M | 97.0% | 44,428 | 12 | 5 | landed on main |
| 10-14 14:47 | `a519baa99` | Fix docs/ADRs + harden layer check | Opus 5.5 | yes | 76 | 90 | 226,573 | 11.7M | 98.0% | 48,694 | 15 | 7 | landed on main; fixes review `a60525741` |
| 10-14 15:04 | `a73316d57` | Fix arithmetic lint errors in engine | Opus 5.5 | no | 13 | 13 | 81,726 | 1.0M | 95.4% | 108 | 1 | 1 | landed on main |
| 10-14 15:12 | `ac544bd80` | Review schema crate (Fable) | Fable 5.1 | no | 6 | 22 | 227,925 | 0.8M | 51.2% | 737 | 8 | 0 | review, read-only; fix round `a161f8543` |
| 10-14 15:12 | `aa3175b0f` | Survey UESP Starfield wiki format pages | Opus 5.5 | no | 15 | 28 | 84,606 | 1.1M | 95.8% | 232 | 2 | 0 | no commits (research or setup) |
| 10-14 15:21 | `a161f8543` | Lane B round 2: schema review fixes | Opus 5.5 | yes | 70 | 112 | 363,308 | 18.6M | 97.7% | 110,640 | 27 | 4 | landed on main; fixes review `ac544bd80` |
| 10-14 15:31 | `a3a751d8e` | Review esm container crate (Fable) | Fable 5.1 | no | 16 | 48 | 200,862 | 2.4M | 91.8% | 1,042 | 9 | 0 | review, read-only; fix round `a8c17a903` |
| 10-14 15:31 | `a402f1038` | Lane C round 2: roundtrip CLI M1.12-13 | Opus 5.5 | yes | 69 | 99 | 234,539 | 12.7M | 98.1% | 40,012 | 14 | 5 | landed on main |
| 10-14 15:41 | `a8c17a903` | Lane A round 2: esm review fixes | Opus 5.5 | yes | 113 | 159 | 398,577 | 33.4M | 98.6% | 101,855 | 28 | 8 | landed on main; fixes review `a3a751d8e` |
| 10-14 15:48 | `a8345031e` | Lane C round 3: coverage + codegen wiring | Opus 5.5 | yes | 71 | 75 | 246,852 | 12.9M | 98.2% | 25,173 | 14 | 5 | landed on main |
| 10-14 15:53 | `a2d449997` | Verify xEdit SF1 defs + rewrite source policy | Opus 5.5 | yes | 90 | 134 | 263,591 | 16.4M | 98.4% | 42,519 | 17 | 9 | landed on main |
| 10-14 16:02 | `a72afab79` | Write M2 execution plan | Opus 5.5 | yes | 19 | 23 | 207,937 | 2.8M | 89.5% | 16,559 | 12 | 1 | landed on main |
| 10-14 16:09 | `aef350cd2` | Fable re-review of esm round 2 | Fable 5.1 | no | 31 | 41 | 237,840 | 5.3M | 95.2% | 23,696 | 12 | 0 | review, read-only; fix round not identified |
| 10-14 16:16 | `a561cf297` | M2 Lane A: index, random access, builder | Opus 5.5 | yes | 161 | 202 | 460,885 | 54.9M | 99.1% | 65,773 | 34 | 10 | landed on main |
| 10-14 16:16 | `a8c739a7f` | M2 Lane B ungated: provenance, universals | Opus 5.5 | yes | 209 | 341 | 563,748 | 73.2M | 99.1% | 107,518 | 42 | 6 | landed on main |
| 10-14 16:16 | `af20864b7` | M2 Lane D: load order + doc/io | Opus 5.5 | yes | 272 | 330 | 715,255 | 126.4M | 99.3% | 204,956 | 65 | 20 | landed on main |
| 10-14 16:16 | `a756f61b6` | M2 Lane F: MCP server + surface | Opus 5.5 | yes | 71 | 101 | 294,641 | 14.9M | 97.9% | 52,202 | 17 | 6 | landed on main |
| 10-14 16:16 | `a8f67192f` | M2 Lane G: ba2, pex, strings ports | Opus 5.5 | yes | 181 | 229 | 495,224 | 56.8M | 99.0% | 103,498 | 46 | 18 | landed on main |
| 10-14 16:19 | `aa78eeee4` | Record owner clarifications in ADRs | Opus 5.5 | yes | 41 | 53 | 179,845 | 5.9M | 97.4% | 11,290 | 7 | 1 | landed on main |
| 10-14 16:34 | `a5eab0653` | Fable review of MCP server crate | Fable 5.1 | no | 17 | 59 | 240,169 | 3.0M | 91.7% | 16,137 | 10 | 0 | review, read-only; fix round `aa2641291` |
| 10-14 16:45 | `aa2641291` | Lane F round 2: MCP review fixes | Opus 5.5 | yes | 87 | 126 | 256,562 | 17.4M | 98.6% | 17,556 | 14 | 7 | landed on main; fixes review `a5eab0653` |
| 10-14 16:51 | `a52ab80d3` | M2 Lane E: set, validate, build, rename | Opus 5.5 | yes | 238 | 298 | 651,385 | 100.2M | 99.3% | 81,266 | 59 | 22 | landed on main |
| 10-14 17:03 | `a794d11fb` | Fable review of ba2/pex/strings crates | Fable 5.1 | no | 21 | 62 | 265,777 | 3.9M | 93.1% | 9,685 | 14 | 0 | review, read-only; fix round `aa2b660b5` |
| 10-14 17:17 | `aa2b660b5` | Lane G round 2: review fixes | Opus 5.5 | yes | 118 | 190 | 337,356 | 29.1M | 98.8% | 61,339 | 20 | 9 | landed on main; fixes review `a794d11fb` |
| 10-14 17:22 | `aeb57067e` | Fable review of load-order crates | Fable 5.1 | no | 64 | 78 | 343,541 | 14.9M | 97.7% | 1,991 | 15 | 0 | review, read-only; fix round `a16b4f60e` |
| 10-14 17:38 | `a16b4f60e` | Lane D round 2: review fixes | Opus 5.5 | yes | 147 | 226 | 430,394 | 45.1M | 99.0% | 66,177 | 27 | 10 | landed on main; fixes review `aeb57067e` |
| 10-14 17:52 | `aa4330b7a` | Fable review of set/validate/build | Fable 5.1 | no | 22 | 61 | 272,408 | 3.8M | 93.0% | 4,002 | 12 | 0 | review, read-only; fix round `ae44b27a4` |
| 10-14 17:52 | `a83b39442` | Lane F round 3: closure + usability | Opus 5.5 | yes | 159 | 178 | 431,687 | 47.9M | 99.1% | 28,290 | 34 | 10 | landed on main |
| 10-14 18:04 | `ae44b27a4` | Lane E round 2: review fixes | Opus 5.5 | yes | 185 | 267 | 530,606 | 69.5M | 99.2% | 58,338 | 43 | 8 | landed on main; fixes review `aa4330b7a` |
| 10-14 18:56 | `afbc964c9` | Write M3 execution plan | Opus 5.5 | yes | 18 | 23 | 244,373 | 3.0M | 91.2% | 18,544 | 11 | 1 | landed on main |
| 10-14 19:08 | `a649ccbe4` | M3 Lane A: container tasks | Opus 5.5 | yes | 175 | 188 | 411,512 | 47.5M | 98.7% | 46,833 | 130 | 3 | landed on main |
| 10-14 19:08 | `aba7975ad` | M3 Lane D: doc/io tasks | Opus 5.5 | yes | 180 | 255 | 512,774 | 63.2M | 99.1% | 104,919 | 36 | 5 | landed on main |
| 10-14 19:08 | `ada6106df` | M3 Lane E: set/validate/build tasks | Opus 5.5 | yes | 151 | 192 | 460,655 | 45.9M | 98.9% | 69,750 | 33 | 6 | landed on main |
| 10-14 19:08 | `a22dec496` | M3 Lanes F+G: surface and archives | Opus 5.5 | yes | 208 | 264 | 495,338 | 67.9M | 99.2% | 54,606 | 76 | 10 | landed on main |
| 10-14 19:08 | `ac020547f` | M3 Lane H: oracles and CK observation prep | Opus 5.5 | yes | 144 | 163 | 408,743 | 37.9M | 98.8% | 80,541 | 40 | 8 | landed on main |
| 10-14 19:35 | `a031f4bb0` | Lane B round 3: typed weapon records | Opus 5.5 | yes | 267 | 337 | 581,200 | 91.6M | 99.3% | 59,481 | 73 | 9 | landed on main |
| 10-14 19:49 | `ac8112a70` | Merge lane H branch, resolve xtask conflict | Opus 5.5 | no | 12 | 12 | 83,022 | 0.9M | 94.8% | 179 | 5 | 1 | landed on main |
| 10-14 19:51 | `a04c62979` | Schema lane: world family (WRLD/CELL/REFR) | Opus 5.5 | yes | 225 | 267 | 543,635 | 79.4M | 99.3% | 48,872 | 56 | 9 | landed on main |
| 10-14 19:51 | `af35ef7ef` | Plan M4 coverage program | Opus 5.5 | yes | 35 | 38 | 277,511 | 6.4M | 95.6% | 43,088 | 15 | 1 | landed on main |
| 10-14 19:52 | `a76877ae8` | Schema lane: planet family (PCMT/PCBN/BIOM) | Opus 5.5 | yes | 272 | 318 | 528,157 | 88.0M | 99.4% | 60,872 | 53 | 20 | landed on main |
| 10-14 20:07 | `a1aee24a2` | M4 Lane A: cell/world group placement | Opus 5.5 | yes | 164 | 242 | 588,318 | 64.2M | 99.0% | 67,697 | 63 | 11 | landed on main |
| 10-14 20:44 | `a03e2d2e3` | Lane F: bare plugin names everywhere | Opus 5.5 | yes | 115 | 176 | 285,851 | 22.8M | 98.8% | 34,407 | 32 | 3 | landed on main |
| 10-14 20:45 | `a49a144f5` | BX1 integrator: merge planet family | Opus 5.5 | no | 93 | 97 | 243,110 | 15.8M | 98.6% | 9,295 | 29 | 1 | landed on main |
| 10-14 20:59 | `a80282c1c` | Amend ADRs for SFSE, MO2 deploy, workbench | Opus 5.5 | yes | 42 | 79 | 228,285 | 7.6M | 97.1% | 32,522 | 11 | 1 | landed on main |
| 10-14 20:59 | `aaf00160b` | Run SF1Edit oracle harness | Opus 5.5 | yes | 206 | 234 | 404,715 | 53.9M | 95.7% | 46,189 | 116 | 5 | landed on main |
| 10-14 21:15 | `a9226a26c` | Integrate 4 branches on a branch | Opus 5.5 | yes | 246 | 258 | 465,182 | 72.3M | 96.7% | 25,204 | 138 | 10 | landed on main |
| 10-14 22:07 | `a92bfd318` | Schema lane: quest family | Opus 5.5 | yes | 209 | 235 | 496,788 | 65.4M | 99.2% | 73,575 | 53 | 13 | landed on main |
| 10-14 22:07 | `a2afd9540` | Schema lane: actor family | Opus 5.5 | yes | 179 | 188 | 462,199 | 51.0M | 99.0% | 84,358 | 44 | 13 | landed on main |
| 10-14 22:07 | `a5164a4d5` | Schema lane: items/workshop family | Opus 5.5 | yes | 251 | 277 | 549,947 | 87.9M | 99.3% | 87,083 | 43 | 16 | landed on main |
| 10-14 22:07 | `ad3e9d518` | M4 Lane G: package + papyrus events | Opus 5.5 | yes | 134 | 177 | 437,920 | 39.7M | 98.7% | 116,608 | 35 | 6 | landed on main |
| 10-14 22:07 | `a030186e8` | M4 Lane E: deploy --dev (ADR 0009) | Opus 5.5 | yes | 80 | 87 | 299,018 | 17.0M | 98.0% | 73,428 | 22 | 3 | landed on main |
| 10-14 23:36 | `a6382bdd2` | BX3 integrator: 4 branches + unordered mode | Opus 5.5 | yes | 361 | 403 | 629,264 | 144.8M | 94.4% | 31,254 | 220 | 7 | landed on main |
| 10-14 23:36 | `a53eb30b4` | Overhaul build from the brief (M3.E9) | Opus 5.5 | yes | 151 | 167 | 425,747 | 41.3M | 99.0% | 32,244 | 34 | 4 | landed on main |
| 10-14 23:36 | `a717f4531` | M4 Lane E: place, spec v3, cell validate, testplan | Opus 5.5 | yes | 292 | 377 | 753,702 | 147.2M | 99.0% | 116,274 | 165 | 10 | landed on main |
| 10-14 23:36 | `ab5b17121` | Fable review: M4 Lane A placement | Fable 5.1 | no | 30 | 71 | 323,870 | 7.5M | 95.7% | 6,384 | 24 | 0 | review, read-only; fix round `ae51af1e2` |
| 10-14 23:36 | `a5d1f7c5e` | Fable review: deploy --dev and package | Fable 5.1 | no | 26 | 45 | 253,715 | 4.8M | 94.8% | 6,300 | 14 | 0 | review, read-only; fix round `a999e9647` |
| 10-14 23:50 | `a999e9647` | Deploy/package review fixes | Opus 5.5 | yes | 102 | 145 | 298,049 | 23.2M | 98.6% | 53,401 | 42 | 9 | landed on main; fixes review `a5d1f7c5e` |
| 10-14 23:50 | `ae51af1e2` | Lane A fix round: placement review | Opus 5.5 | yes | 165 | 205 | 395,199 | 46.5M | 96.9% | 47,613 | 60 | 6 | landed on main; fixes review `ab5b17121` |
| 10-15 03:22 | `a8c813041` | Rename identifiers kitcraft to kitkraft | Opus 5.5 | yes | 90 | 104 | 194,385 | 12.9M | 97.1% | 7,414 | 49 | 1 | landed on main |
| 10-15 03:23 | `aa21e1a33` | Fable review: place, spec v3, cell validate | Fable 5.1 | no | 51 | 76 | 352,513 | 11.8M | 97.0% | 10,917 | 16 | 0 | review, read-only; fix round `aab43cce1` |
| 10-15 03:23 | `a20d89883` | Fable review: schema wave 2 + unordered | Fable 5.1 | no | 64 | 64 | 297,159 | 11.9M | 97.5% | 8,350 | 18 | 0 | review, read-only; fix round `a1f529ec3` |
| 10-15 03:51 | `a1f529ec3` | Lane B round 4: OMOD union + review fixes | Opus 5.5 | yes | 307 | 384 | 667,271 | 140.2M | 96.9% | 87,684 | 105 | 8 | landed on main; fixes review `a20d89883` |
| 10-15 03:51 | `aab43cce1` | Lane E fix round: POI review | Opus 5.5 | yes | 136 | 165 | 344,408 | 32.9M | 98.1% | 13,234 | 58 | 5 | landed on main; fixes review `aa21e1a33` |
| 10-15 05:41 | `a7b9f80ec` | Overhaul rerun with typed OMOD values | Opus 5.5 | yes | 115 | 147 | 318,612 | 25.8M | 98.7% | 27,505 | 30 | 5 | landed on main |
| 10-15 05:41 | `af3249b6f` | ADR 0005 A5 + STATUS closure | Opus 5.5 | yes | 44 | 47 | 205,083 | 6.7M | 96.9% | 6,524 | 7 | 1 | landed on main |
| 10-15 05:55 | `a75caa506` | Lane W: minimal developer workbench | Opus 5.5 | yes | 189 | 209 | 456,421 | 58.0M | 97.7% | 94,658 | 106 | 5 | landed on main |
| 10-15 06:55 | `a8ab0d0d2` | Fable review: workbench | Fable 5.1 | no | 31 | 82 | 299,137 | 6.6M | 95.4% | 12,185 | 28 | 0 | review, read-only; fix round `acc75213d` |
| 10-15 06:55 | `a736a0502` | Lane B round 5: container-reference + leftovers | Opus 5.5 | yes | 203 | 270 | 392,009 | 52.2M | 97.8% | 48,562 | 55 | 6 | landed on main |
| 10-15 07:13 | `acc75213d` | Workbench + xtask hardening round | Opus 5.5 | yes | 137 | 166 | 310,360 | 30.7M | 98.0% | 30,261 | 25 | 5 | landed on main; fixes review `a8ab0d0d2` |
| 10-15 07:55 | `a29475fa0` | M4 Lane D: refs and planet commands | Opus 5.5 | yes | 222 | 264 | 580,144 | 84.7M | 99.2% | 108,384 | 52 | 2 | landed on main |
| 10-15 07:55 | `a86d4abc7` | Lane H: persistence doc, ledger, B11 differential | Opus 5.5 | yes | 222 | 256 | 575,929 | 82.4M | 99.3% | 64,689 | 53 | 3 | landed on main |
| 10-15 08:55 | `a654c42f1` | Triage: ungated undone plan tasks | Haiku 4.5 | no | 18 | 19 | n/a | 2.3M | 92.7% | 1,314 | n/a | 0 | triage, read-only |
| 10-15 09:00 | `a9c512f91` | M3.A5 structure-aware mutation fuzz | Opus 5.5 | yes | 88 | 113 | 236,174 | 16.0M | 96.0% | 33,104 | 93 | 2 | landed on main |
| 10-15 09:00 | `a02aa9589` | Lane B: count_from + inventory defs (BX2) | Opus 5.5 | yes | 215 | 241 | 407,447 | 55.0M | 97.2% | 37,431 | 76 | 3 | landed on main |
| 10-15 09:00 | `a90d1ab5b` | Lane E: lstrings from schema, recipes, scripts | Opus 5.5 | yes | 403 | 528 | 738,739 | 177.3M | 99.6% | 99,373 | 79 | 6 | landed on main |
| 10-15 09:00 | `aec5c8962` | Lane W/F: refs+planet panels, shape tests | Opus 5.5 | yes | 190 | 240 | 489,347 | 61.4M | 99.1% | 80,773 | 38 | 3 | landed on main |
| 10-15 20:41 | `a2d3b3d44` | Investigate POI ref not found in game | Opus 5.5 | yes | 49 | 62 | 181,282 | 6.8M | 97.3% | 4,446 | 11 | 1 | landed on main |
| 10-15 21:03 | `aaa2bc10f` | Fix runtime-slot rules + save header reader | Opus 5.5 | yes | 222 | 310 | 488,530 | 74.1M | 98.7% | 58,730 | 114 | 5 | landed on main |
| 10-15 22:24 | `a6c5a02d3` | Lane F: dist, installers, doctor, quickstart | Opus 5.5 | yes | 161 | 212 | 423,638 | 47.2M | 99.0% | 61,977 | 39 | 6 | landed on main |
| 10-15 22:24 | `a552685c3` | Lane W: first-run, help tab, ledger | Opus 5.5 | yes | 131 | 208 | 413,770 | 38.7M | 97.0% | 85,574 | 42 | 4 | landed on main |
| 10-15 23:23 | `a35913ef4` | Fable review: installer safety | Fable 5.1 | yes | 39 | 70 | 265,522 | 7.9M | 96.6% | 7,735 | 32 | 0 | review, read-only; fix round `a7d70f7b4` |
| 10-15 23:23 | `aaa264b83` | Planning lane: M4 gap matrix, ADRs, sprint plan | Opus 5.5 | yes | 82 | 92 | 343,344 | 18.8M | 98.0% | 30,003 | 21 | 7 | landed on main |
| 10-15 23:23 | `ae1434b47` | Lane B: Papyrus compiler oracle | Opus 5.5 | yes | 134 | 156 | 304,617 | 27.7M | 96.0% | 41,185 | 49 | 5 | landed on main |
| 10-15 23:23 | `a6a91fd81` | Lane D: support-depth matrix, typed write LCTN/TERM/PKIN/FACT | Opus 5.5 | yes | 269 | 314 | 749,742 | 122.9M | 99.0% | 41,164 | 83 | 2 | landed on main |
| 10-15 23:42 | `a7d70f7b4` | Fix round: installer review findings | Opus 5.5 | yes | 77 | 105 | 218,472 | 12.9M | 97.0% | 26,638 | 33 | 3 | landed on main; fixes review `a35913ef4` |
| 10-15 23:45 | `a2e29d9cc` | Fable review: M5 plan and gap matrix | Fable 5.1 | no | 11 | 55 | 216,653 | 1.8M | 88.5% | 418 | 10 | 0 | review, read-only; fix round `a6e22f61f` |
| 10-15 23:56 | `a6e22f61f` | Fix round: M5 plan review findings | Opus 5.5 | yes | 89 | 93 | 258,857 | 16.9M | 98.5% | 36,123 | 15 | 8 | landed on main; fixes review `a2e29d9cc` |
| 10-16 00:56 | `a0dc63009` | Fable review: lanes B and D code | Fable 5.1 | no | 68 | 68 | 241,417 | 10.6M | 97.7% | 2,110 | 14 | 0 | review, read-only; fix round `a7aa24584` |
| 10-16 01:10 | `a7aa24584` | Fix round: lanes B and D review findings | Opus 5.5 | yes | 133 | 147 | 307,575 | 28.4M | 98.9% | 12,193 | 33 | 4 | landed on main; fixes review `a0dc63009` |
| 10-16 17:06 | `a94c2fa0a` | Project analytics: tokens, sessions, authorship | Opus 5.5 | no | 55 | 62 | n/a | 6.5M | 96.8% | 30,566 | n/a | 0 | this report (running) |
| 10-16 17:08 | `af1f2a31c` | Roadmap lane: M6 cockpit, M7 editor | Opus 5.5 | yes | 41 | 44 | n/a | 6.4M | 97.0% | 12,596 | n/a | 0 | running at report time |
| 10-16 17:08 | `a95b8cc15` | Fix: install registers MCP on dev profile | Opus 5.5 | yes | 54 | 67 | 198,378 | 8.6M | 97.9% | 17,451 | 17 | 0 | no commit traced to main |

**Top ten subagents by tokens processed**

| Agent | Description | Model | Processed | Output | Commits |
|---|---|---|---:|---:|---:|
| `a90d1ab5b` | Lane E: lstrings from schema, recipes, scripts | Opus 5.5 | 177.3M | 99,373 | 6 |
| `a717f4531` | M4 Lane E: place, spec v3, cell validate, testplan | Opus 5.5 | 147.2M | 116,274 | 10 |
| `a6382bdd2` | BX3 integrator: 4 branches + unordered mode | Opus 5.5 | 144.8M | 31,254 | 7 |
| `a1f529ec3` | Lane B round 4: OMOD union + review fixes | Opus 5.5 | 140.2M | 87,684 | 8 |
| `af20864b7` | M2 Lane D: load order + doc/io | Opus 5.5 | 126.4M | 204,956 | 20 |
| `a6a91fd81` | Lane D: support-depth matrix, typed write LCTN/TERM/PKIN/FACT | Opus 5.5 | 122.9M | 41,164 | 2 |
| `a52ab80d3` | M2 Lane E: set, validate, build, rename | Opus 5.5 | 100.2M | 81,266 | 22 |
| `a031f4bb0` | Lane B round 3: typed weapon records | Opus 5.5 | 91.6M | 59,481 | 9 |
| `a76877ae8` | Schema lane: planet family (PCMT/PCBN/BIOM) | Opus 5.5 | 88.0M | 60,872 | 20 |
| `a5164a4d5` | Schema lane: items/workshop family | Opus 5.5 | 87.9M | 87,083 | 16 |

## 5. Git authorship and lanes

All 520 commits have git author Michael Ackerson; the model shows up only in `Co-Authored-By` trailers.

**Commits per day** (author date)

| Day | UTC | Local (author TZ) |
|---|---:|---:|
| 2026-10-14 | 369 | 401 |
| 2026-10-15 | 126 | 119 |
| 2026-10-16 | 25 | 0 |

**Commits by `Co-Authored-By` trailer**

| Trailer | Non-merge | Merge | Total |
|---|---:|---:|---:|
| Claude Opus 5.5 | 89 | 19 | 108 |
| Claude Fable 5.1 | 352 | 25 | 377 |
| Claude Haiku | 0 | 0 | 0 |
| none | 0 | 35 | 35 |

**Who actually wrote the commits** (matching rules in section 7)

| Match rule | Commits |
|---|---:|
| subject in commit command | 460 |
| hash in output of a commit command | 31 |
| merge subject names agent worktree | 14 |
| git merge command | 10 |
| not traced | 5 |


| Writer | Trailer: Fable 5.1 | Trailer: Opus 5.5 | Trailer: none | Total |
|---|---:|---:|---:|---:|
| Opus 5.5 writer agents | 354 | 107 | 21 | 482 |
| Director (Fable 5.1) | 22 | 0 | 10 | 32 |
| not traced | 0 | 1 | 4 | 5 |
| Side session `345611b0` | 1 | 0 | 0 | 1 |

The trailer is not a reliable "who": 354 commits written by Opus 5.5 agents carry a `Claude Fable 5.1` trailer, consistent with subagents inheriting the director session's attribution line. Fable reviewer agents wrote no commits.

**Merge commits by kind**

| Kind | Count |
|---|---:|
| integration merge (named) | 37 |
| lane syncs main into lane branch | 17 |
| agent syncs main into its worktree | 15 |
| director merge of agent worktree | 10 |

"Director merge of agent worktree" is a `Merge branch 'worktree-agent-…'` commit. Most agent work reached main by fast-forward or through named integration merges, so the 10 director merges undercount landed agent branches.

**Non-merge commits by area** (first subject prefix, e.g. `engine:`; multi-area subjects counted under the first)

| Area | Commits |
|---|---:|
| engine | 95 |
| schema | 87 |
| docs | 77 |
| esm | 35 |
| plan | 27 |
| status | 18 |
| doc | 18 |
| xtask | 14 |
| adr | 9 |
| ui-egui | 9 |
| agents | 6 |
| kit-cli | 6 |
| io | 6 |
| ba2 | 6 |
| mcp | 6 |
| pex | 4 |
| strings | 4 |
| workspace | 4 |
| gitignore | 2 |
| rename | 1 |
| overhaul | 1 |
| deps | 1 |
| readme | 1 |
| notice | 1 |
| (no prefix) | 3 |

**Commits by lane** (lane letter from ticket ids such as `M4.E7` or from "lane X" in the subject; a commit can name two lanes)

| Lane | Commits |
|---|---:|
| B | 78 |
| E | 53 |
| A | 34 |
| G | 31 |
| D | 29 |
| F | 25 |
| H | 15 |
| O | 11 |
| W | 5 |
| T | 1 |
| C | 1 |
| (none named) | 262 |

**Lines added / removed by top-level path** (non-merge commits on main; renames credited to the new path; 5 binary entries skipped)

| Path | Added | Removed | Net |
|---|---:|---:|---:|
| crates/ | 247,902 | 32,849 | 215,053 |
| docs/ | 13,843 | 1,786 | 12,057 |
| xtask/ | 11,775 | 304 | 11,471 |
| plan/ | 6,390 | 546 | 5,844 |
| (root files) | 5,426 | 547 | 4,879 |
| apps/ | 1,996 | 176 | 1,820 |
| corpus/ | 89 | 2 | 87 |
| .githooks/ | 8 | 1 | 7 |
| .cargo/ | 2 | 0 | 2 |
| of which `crates/esm/src/generated/` | 34,355 | 7,032 | 27,323 |

**Rust lines at HEAD by crate** (hand-written; excludes `crates/esm/src/generated` and `@generated` golden fixtures)

| Crate | Lines |
|---|---:|
| crates/engine | 56,924 |
| crates/esm | 17,744 |
| crates/doc | 12,348 |
| xtask | 11,455 |
| crates/ui-egui | 9,358 |
| crates/schema | 8,894 |
| crates/io | 3,928 |
| crates/pex | 3,242 |
| crates/ba2 | 3,183 |
| crates/mcp | 2,529 |
| apps/kit-cli | 1,666 |
| crates/strings | 1,037 |
| apps/kit | 112 |
| crates/nif | 5 |

## 6. What the pattern cost and bought

| Measure | Value |
|---|---|
| Fable review agents | 16, 98.1M processed, 113,018 output tokens, 0 commits |
| Opus writer agents | 80, 3,494.7M processed, 4,195,946 output tokens, 482 commits on main |
| Review : writing tokens (processed) | 1 : 35.6 (2.8%) |
| Review : writing tokens (output only) | 1 : 37.1 (2.7%) |
| Reviews followed by a named fix round | 15 of 16 |
| Fix-round agents | 15, 556.6M processed (15.9% of writer tokens), 101 commits on main |
| Commit subjects naming a fix round, review fix or finding | 20; 20 numbered findings cited |
| Director | 443 turns, 191.0M processed (5.0%), 32 commits (docs, status, merges) |

### Review to fix-round pairs

| Review agent | Review | Review tokens | Fix agent | Fix round | Fix tokens | Fix commits |
|---|---|---:|---|---|---:|---:|
| `a60525741` | Review kitcraft docs and ADRs | 1.0M | `a519baa99` | Fix docs/ADRs + harden layer check | 11.7M | 7 |
| `ac544bd80` | Review schema crate (Fable) | 0.8M | `a161f8543` | Lane B round 2: schema review fixes | 18.6M | 4 |
| `a3a751d8e` | Review esm container crate (Fable) | 2.4M | `a8c17a903` | Lane A round 2: esm review fixes | 33.4M | 8 |
| `aef350cd2` | Fable re-review of esm round 2 | 5.3M | none named | | | |
| `a5eab0653` | Fable review of MCP server crate | 3.0M | `aa2641291` | Lane F round 2: MCP review fixes | 17.4M | 7 |
| `a794d11fb` | Fable review of ba2/pex/strings crates | 3.9M | `aa2b660b5` | Lane G round 2: review fixes | 29.1M | 9 |
| `aeb57067e` | Fable review of load-order crates | 14.9M | `a16b4f60e` | Lane D round 2: review fixes | 45.1M | 10 |
| `aa4330b7a` | Fable review of set/validate/build | 3.8M | `ae44b27a4` | Lane E round 2: review fixes | 69.5M | 8 |
| `ab5b17121` | Fable review: M4 Lane A placement | 7.5M | `ae51af1e2` | Lane A fix round: placement review | 46.5M | 6 |
| `a5d1f7c5e` | Fable review: deploy --dev and package | 4.8M | `a999e9647` | Deploy/package review fixes | 23.2M | 9 |
| `aa21e1a33` | Fable review: place, spec v3, cell validate | 11.8M | `aab43cce1` | Lane E fix round: POI review | 32.9M | 5 |
| `a20d89883` | Fable review: schema wave 2 + unordered | 11.9M | `a1f529ec3` | Lane B round 4: OMOD union + review fixes | 140.2M | 8 |
| `a8ab0d0d2` | Fable review: workbench | 6.6M | `acc75213d` | Workbench + xtask hardening round | 30.7M | 5 |
| `a35913ef4` | Fable review: installer safety | 7.9M | `a7d70f7b4` | Fix round: installer review findings | 12.9M | 3 |
| `a2e29d9cc` | Fable review: M5 plan and gap matrix | 1.8M | `a6e22f61f` | Fix round: M5 plan review findings | 16.9M | 8 |
| `a0dc63009` | Fable review: lanes B and D code | 10.6M | `a7aa24584` | Fix round: lanes B and D review findings | 28.4M | 4 |

**Reading.** The director, running on Fable 5.1, used 5.0% of KitKraft tokens and ended with a 262,966-token context after 98 agents. Opus 5.5 writers did the bulk of the work in their own worktrees: 92.1% of tokens and 482 of 520 commits on main. Review was cheap: 16 read-only Fable reviews cost 1 token for every 36 writer tokens (2.8%), and 15 of them led to a named fix round. Those fix rounds cost 556.6M (15.9% of writing) and landed 101 commits, so for every review token about 5.7 tokens went into fixing what it found. Commit subjects cite 20 numbered findings (11 in the deploy fix round, 8 from the lane E Fable review, 1 in esm); lane E round 2 also reports "13 review findings" without numbering them, so the true finding count is higher. Across everything, 98.3% of tokens were cache reads and output was 0.1%: the spend is context re-read each turn, which is why long-running writers (the top five each passed 120M) dominate the total.

## 7. Method and limitations

- **Sources.** 13 top-level `*.jsonl` files (one per session) and the director's `subagents/agent-*.jsonl` plus `.meta.json` files (98 agents). Files were streamed line by line with `python -I`; 0 lines failed to parse. Scripts and intermediate JSON are in the session scratchpad, not the repo.
- **Deduplication.** One API message is written as several lines (one per content block) with the same `message.id`; usage was counted once per id, taking the per-field maximum (output tokens grow across the lines).
- **Notifications.** `<task-notification>` blocks arrive as `queue-operation`, `attachment` and `user` events; they were deduplicated by task id and values. Notifications without `<subagent_tokens>` (background shell tasks) were ignored. `<subagent_tokens>` is the final context size, not cumulative use, so processed tokens come from the subagent transcripts.
- **Advisor.** Director turns record `advisorModel: claude-fable-5-1`, but usage objects carry a single `iterations` entry of type `message`; advisor tokens, if any, are not visible and are not counted.
- **Live data.** The director session and three agents (this one, the roadmap lane and the MCP install fix) were running while this was measured; their numbers are partial.
- **Commit attribution.** A commit is credited to the earliest transcript that (a) ran a `git commit` command containing the exact subject, or wrote a file (Write tool) whose first line is the subject; else (b) printed the commit hash in the output of its own `git commit -...` command (agents that wrote message files from scripts); merges are matched by the agent id in the merge subject or by the director's `git merge` command. 5 commits could not be traced (four lane-sync merges and one README commit). The `Co-Authored-By` trailer is reported as recorded; section 5 shows it does not match the writing model.
- **Lanes and areas** come from subject text; subjects without a ticket id or "lane X" fall under "(none named)". The review-to-fix pairing in section 6 was made by reading agent descriptions and start times, not from a recorded link.
- **Findings.** Only findings cited by number in commit subjects are counted; the number of findings each review raised is in the review reports, which were not parsed.
- **Days** are UTC for transcripts; commit days are shown in UTC and in the committer's local offset.
- **LOC** is a plain line count of `*.rs` files in `main`'s tree (`git ls-tree`), split into hand-written, `crates/esm/src/generated`, and other files whose first lines say `@generated`. `#[test]` is a text count.
- **Not measured:** dollar cost (no pricing file in the repo), wall-clock time per lane beyond notification durations, tokens of the earlier non-KitKraft sessions' subagents (none were recorded), and work that never reached `main`.
