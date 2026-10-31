# Developing KitKraft with the local agent

ADR 0014 made this the default way to move KitKraft forward. A local Qwen3.8-27B model drives
Claude Code. It works through one ticket per run, unattended, inside the overnight harness in
`F:\AI\Scripts\overnight`. A read-only Opus check-in advises it, and KitKraft's own gate grades
it. Paid usage is limited to the check-ins and the review before a merge.

## Run a ticket

1. One-off, and again whenever `main` moves or `Cargo.lock` changes. Run it in a Windows
   terminal:

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
   `KK_DATA=...` set.
2. Close Starfield, LM Studio and any browser that holds GPU memory. Then start the run:

   ```bat
   overnight kitkraft KK-001 4
   ```

   That runs ticket `plan/tickets/KK-001.toml` for at most 4 hours, with a grade every 30 minutes.
   To queue several tickets in one budget, join them with `+`:
   `overnight kitkraft KK-002+KK-003+KK-004 8`. They run one after another, each from `main`,
   and each gets an equal share of the time left. A ticket that finishes early leaves its time to
   the rest, and none starts with less than 30 minutes left.
   Opus checks in at minutes 3, 8 and 26, while the agent is choosing its approach. After that,
   each check-in picks when the next one comes, between 15 and 60 minutes: sooner when the agent
   is stuck or off track. Check-ins see the agent's recent commands and their results as well as
   its commits.
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
   runs after a stop.
4. Results land in `F:\AI\Scripts\overnight\runs\kk-KK-001-<date-time>\`, one folder per ticket:
   - `KK-001.bundle`: the agent's commits on branch `ticket/KK-001`. Work left uncommitted at the
     end is committed by the harness as `fixup! <the run's last commit>`, for the merge to fold
     in with `git rebase -i --autosquash`. That commit takes tracked files and new files under
     `owns` only.
   - `left-out.txt`: untracked files outside `owns` that were not committed, if there were any.
   - `scores.jsonl`: every grade.
   - `grades\<label>\`: one log per check, plus `grade.json`.
   - `checkins\`: the Opus notes. The final QA notes are `qa-<n>-min<m>.md`, ending in the
     verdict.
   - `report.html`.

## Merge a finished ticket

The local agent never pushes. Review and merge happen on Windows:

```bat
cd C:\Modding\kitcraft
git fetch F:\AI\Scripts\overnight\runs\kk-KK-001-<date-time>\KK-001.bundle ticket/KK-001:ticket/KK-001
git log --oneline main..ticket/KK-001
git diff --stat main ticket/KK-001
```

Read the last QA note in `checkins\` first: it lists what to check by hand. Then run
`cargo xtask ci` on the branch with `KITKRAFT_DATA_DIR` set. The Linux grade cannot see
Windows-only behaviour, so this second gate matters. Merge only when it is green. A short Claude
review, `/code-review` on the branch, is the cheap second opinion. After merging, rerun
`overnight kk-setup` so the next ticket starts from the new `main`.

## What the grader checks

`F:\AI\Scripts\overnight\harness\kk_grade.py` runs every check as user `nobody` in bubblewrap. It
works on a copy of the agent's working tree, offline, with the Starfield Data folder bound
read-only:

| Check | Passes when |
| ----- | ----------- |
| `scope` | Every changed path is under the ticket's `owns`. The one STATUS line is also allowed. |
| each `[[accept]]` | Its command exits 0. |
| `gate` | `cargo xtask ci` exits 0 on Linux. |

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
- **Cannot:** reach the network, add crates, see the grader's results, read `/mnt`, or launch the
  game, MO2 or the Creation Kit.
- **Should not read:** `plan/STATUS.md` and `plan/execution-plan.md`. They are too large for its
  136k context, so every ticket's brief carries what it needs.

## Known limits (2026-10-23)

- Tests that read the Data folder go through WSL's `/mnt/<drive>`, which is slow. A full `cargo test`
  took more than 10 minutes on Linux, against about 5 on Windows. Copying the masters onto WSL's
  own disk would speed it up.
- The window, CK sittings, in-game checks and license rulings stay with Claude or the owner.
  Tickets cover headless engine, schema, docs and test work.
- Results so far are for tinysql, a small greenfield Python task. KitKraft is about 130,000 lines
  of Rust, so expect small tickets to work and large ones to stall. Cut tickets small.
