# Developing KitKraft with the local agent

ADR 0014 made this the default way to move KitKraft forward. A local Qwen3.8-27B model drives
Claude Code. It works through one ticket per run, unattended, inside the overnight harness in
`F:\AI\Scripts\overnight`. A read-only Opus check-in advises it, and KitKraft's own gate grades
it. Paid usage is limited to the check-ins, the final QA and the morning merge.

## Run a ticket

1. One-off. Run it in a Windows terminal:

   ```bat
   overnight kk-setup
   ```

   It installs Rust 1.99 at `/opt/rust`, mirrors this repo into `/opt/kitkraft/repo.git`, and
   vendors every crate into `/opt/kitkraft/vendor`. The sandbox has no network, so these copies
   are all it builds from. It also writes `/opt/kitkraft/paths.env`: where this checkout is and
   where the Starfield Data folder is. The runner, the grader and the agent's sandbox read every
   path from it, so nothing assumes drive C:. The checkout is found as `<drive>:\Modding\kitkraft`
   or `kitcraft`. For a checkout anywhere else, pass it once, `overnight kk-setup
   F:\path\to\kitkraft`, and it is remembered. The Data folder comes from Steam: its registry
   entry, then every library in `libraryfolders.vdf`. To override it, run kk_setup.sh with
   `KK_DATA=...` set. Every queue reruns kk_setup.sh when it starts, so the mirror follows
   `main` without you.
2. Close Starfield, LM Studio and any browser that holds GPU memory: `overnight` refuses to
   start while more than 3,000 MiB of it is in use. Then start the run:

   ```bat
   overnight kitkraft KK-001 4
   ```

   That starts a queue with a 4-hour budget (the default when the hours are left out) for
   ticket `plan/tickets/KK-001.toml`, with a grade every 30 minutes and one as soon as the agent
   stops (unless it was graded in the last 5 minutes). To queue several tickets in one budget, join them with `+`:
   `overnight kitkraft KK-002+KK-003+KK-004 8`; `overnight kitkraft auto 8` queues every
   runnable ticket (`plan/tickets/README.md`, "Queue"). A preflight first drops any ticket that
   cannot run. The rest run one after another, each from `main`, each for up to its `budget_h`
   (default 1.5 hours; during a campaign, `ticket_h` in `campaign.json` replaces that default)
   or the time left, and none starts with less than 30 minutes left. When every ticket has had
   its turn, each one left unfinished gets one overtime run of up to an hour, continuing from
   its bundle, while time lasts.
   Opus checks in at about minutes 10 and 25, while the agent is choosing its approach. After
   that, each check-in ends with a `NEXT:` line that sets the gap to the next one, held between
   15 and 60 minutes: sooner when the agent is stuck or off track. Without that line the gap is
   30 minutes. A gap counts from the end of the previous check-in, so the minutes drift (10, 26,
   52 is typical), and no check-in starts in a run's last 15 minutes (20, or half the fallback
   gap, whichever is less). Check-ins see the agent's recent commands and their results as well
   as its commits. A check-in that finds the ticket cannot pass as written adds a `DEFECT:`
   line; the run then ends within 15 minutes (the deadline moves to 15 minutes from then when
   more time than that is left), keeping its work, and the merge review sees a
   `ticket_defect` event (logged only when the deadline moved). Nothing else acts on it: until someone fixes or holds the ticket, the
   queue can still give it overtime and `auto` picks it again.
   A check-in or QA session that ends in an API error, without a result, or with a non-zero exit
   gives the agent nothing: it is logged in `events.jsonl` as `checkin_error` or `qa_error`, and
   the agent carries on without a review (since 2026-11-06; before, an "API Error" reply was
   handed over as the review).
   When every check passes, Opus does a final QA of the whole change. It looks for what a passing
   grade misses: weakened tests, Windows behaviour changes, silent workarounds and claims the agent
   could not verify. `VERDICT: CHANGES` sends its notes back to the agent and regrades 15 minutes
   later, at most twice. `VERDICT: ACCEPT` ends the run. If the run ends with the checks passing
   but no accepted QA, a last QA is written for the merge review.
   QA also runs when every acceptance command and the gate pass and only `scope` fails. It is
   given the paths outside `owns` and says which are generator output, harness artifacts or real
   overreach; it asks the agent only to revert real overreach. Scope is the merge's decision.
   A queue picks and runs its tickets from a snapshot of `main`'s `plan/tickets/` taken when it
   starts, the same commit its code comes from. A ticket edited mid-queue takes effect at the
   next queue.
3. Check on it with `overnight status`, or stop it with `overnight stop`. The final grade still
   runs after a stop, after the harness runs `cargo fmt --all`, as it does at the end of every
   run. A stop is deliberate: the guard does not resume it, and the stopped queue leaves no
   `.pending`, so the guard merges its runs only while a campaign is running (its `orphans`
   step, "The guard and campaigns"); they can always be merged by hand ("Merge a finished
   ticket").
4. Results land in `F:\AI\Scripts\overnight\runs\kk-KK-001-<date-time>\`, one folder per ticket
   (an overtime run's folder ends in `-ot`, and its bundle holds the earlier run's work too):
   - `KK-001.bundle`: the agent's commits on branch `ticket/KK-001`. Work left uncommitted at the
     end is committed by the harness as `fixup! <the run's last commit>`, for the merge to fold
     in with `git rebase -i --autosquash`. That commit takes tracked files and new files under
     `owns` only.
   - `left-out.txt`: untracked files outside `owns` that were not committed, if there were any.
   - `scores.jsonl`: every grade.
   - `events.jsonl`: every event of the run, `checkin_error` and `qa_error` included.
   - `grades\<label>\`: one log per check, plus `grade.json`.
   - `checkins\`: the Opus notes. The final QA notes are `qa-<n>-min<m>.md`, ending in the
     verdict.
   - `report.html`.

## Merge a finished ticket

The local agent never pushes. Review and merge happen on Windows. When a queue ends it leaves
`runs\kkq-<stamp>.pending`, a list of its runs, and the guard runs the morning merge on it
(`harness\kk-merge.ps1`). A headless Opus session (`harness\kk-merge-prompt.md`) reviews each
run's branch and merges what holds up into `main`. The script then runs `cargo xtask ci` on
`main` (with `KITKRAFT_DATA_DIR` set when the Data folder is found), at most 2 hours per gate, and hands the result back to the
session, up to three gates. The session pushes once the gate is green, refreshes the mirror,
writes `runs\kkq-<stamp>-merge.md` and says `DONE`; the `.pending` becomes `.merged`. With neither
a `DONE` nor a report, or with merged commits on `main` that no green gate covered, it becomes `.failed`
instead, and the guard retries it the next times nothing is running, up to three more tries
(counted in `kkq-<stamp>.tries`).

A run whose `events.jsonl` has a `qa_error` and no QA verdict had no QA: the merge review is
then the only review of that change. A `checkin_error` means one check-in's notes never reached
the agent; the run went on without them.

To merge a run by hand:

```bat
cd C:\Modding\kitkraft
git fetch F:\AI\Scripts\overnight\runs\kk-KK-001-<date-time>\KK-001.bundle ticket/KK-001:ticket/KK-001
git log --oneline main..ticket/KK-001
git diff --stat main ticket/KK-001
```

Read the last QA note in `checkins\` first: it lists what to check by hand. Then run
`cargo xtask ci` on the branch with `KITKRAFT_DATA_DIR` set. The Linux grade cannot see
Windows-only behaviour, so this second gate matters. Merge only when it is green. A short Claude
review, `/code-review` on the branch, is the cheap second opinion. The next queue refreshes the
mirror when it starts, so its tickets start from the new `main`.

## The guard and campaigns

`overnight kitkraft` also starts a guard on Windows (`harness\guard.ps1`, log
`F:\AI\Scripts\overnight\guard.log`). It keeps the PC awake, holds a WSL connection open while
the queue runs and, if WSL goes down, restarts it and resumes the queue where it stopped. Every
10 minutes it checks whether the queue is overdue, still running 90 minutes or more past its end
time, and if so stops it as `overnight stop` would. Only a queue whose stamp is in a running
process's command line counts (since 2026-11-06: a stopped queue's leftover `kkq-<stamp>.end`
had made the guard stop a new queue at minute 7). When nothing is running it merges what is
pending, then exits, unless a campaign is on.

A campaign runs queues back to back until a deadline:
`powershell -File F:\AI\Scripts\overnight\harness\kk-campaign.ps1 -Start -Until '<yyyy-MM-dd HH:mm>'`
(`-SegmentHours`, default 8; `-Workers`, default 1). Its settings are
`F:\AI\Scripts\overnight\campaign.json`, read again at every tick. Whenever nothing is running,
the guard merges and then ticks the campaign. A tick ends the campaign when less than 1 hour 45
minutes is left (the last 45 minutes are for the last merge), waits 30 minutes while disk is low
or more than 6,000 MiB of GPU memory is in use, refills the backlog with
`plan/tickets/generate.py` when fewer than 6 tickets are runnable, and starts the next segment
as an `auto` queue. Between segments it prunes old grader copies and removes the `kkq-*` state
of queues that were stopped rather than finished (kept while a resume is pending), and the guard
writes a `.pending` for finished runs a stopped queue left unlisted, so they merge too. So
`overnight stop` during a campaign ends only the current segment. To end the campaign, run
`kk-campaign.ps1 -Stop`: the current segment finishes and merges. `kk-campaign.ps1 -Status`
prints `runs\campaign-status.md`.

## What the grader checks

`F:\AI\Scripts\overnight\harness\kk_grade.py` runs every check as user `nobody` in bubblewrap. It
works on a copy of the agent's working tree, offline, with the Starfield Data folder bound
read-only:

| Check | Passes when |
| ----- | ----------- |
| `scope` | Every changed path is under the ticket's `owns`. `plan/STATUS.md`, for the one "Landed" line, is always allowed. |
| each `[[accept]]` | Its command exits 0 within the ticket's `timeout_min` (default 45). |
| `gate` | `cargo xtask ci` exits 0 on Linux, within the same limit. |

The score is the share of checks that pass. The ticket is done when all of them pass.

`scope` diffs the working tree, uncommitted edits included, against the merge-base of `main`
and the run's HEAD. It does not diff against the ticket's recorded `base`, so a run that merges
or rebases `main` into its WIP branch is judged only on its own changes. The recorded base
still decides where the run starts and what its bundle spans. Untracked files outside `owns`
are not a failure: they are removed from the grading copy, listed as warnings in `grade.json`,
and left out of the bundle.

## What the agent can and cannot do

- **Can:** read and edit the repository, run `cargo`, `git`, `python3` and read-only shell tools.
  It can also read the game's Data folder at `/opt/kk-data` (`KITKRAFT_DATA_DIR`), and the
  pinned xEdit file at `/opt/xedit-ref` when `tools\xedit-ref` sits beside the checkout.
- **Cannot:** reach the network, add crates, see the grader's logs, read `/mnt`, or launch the
  game, MO2 or the Creation Kit. It learns only which checks still fail, with the tail of each
  one's output, after a grade it triggered by stopping and at the start of an overtime run.
- **Should not read:** `plan/STATUS.md` and `plan/execution-plan.md`. They are too large for its
  context (134,000 tokens, Claude Code's cap in the sandbox, under the server's 139,264-token
  (136k) window), so every ticket's brief carries what it needs.

## Known limits (2026-10-23, updated 2026-11-07)

- Tests that read the Data folder go through WSL's `/mnt/<drive>`, which is slow. A full `cargo test`
  took more than 10 minutes on Linux, against about 5 on Windows. Copying the masters onto WSL's
  own disk would speed it up.
- Looking at the window, CK sittings, in-game checks and license rulings stay with Claude or the
  owner. Tickets cover engine, schema, docs, test and workbench work that tests can check.
- The first results were for tinysql, a small greenfield Python task; the merged tickets since
  are in `plan/tickets/README.md`. KitKraft is about 130,000 lines of Rust, and small tickets
  still do best while large ones stall. Cut tickets small.
