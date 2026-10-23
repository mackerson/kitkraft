# Developing KitKraft with the local agent

ADR 0014 made this the default way to move KitKraft forward. A local Qwen3.8-27B model drives
Claude Code. It works through one ticket per run, unattended, inside the overnight harness in
`F:\AI\Scripts\overnight`. A read-only Opus check-in advises it, and KitKraft's own gate grades
it. Paid usage is limited to the check-ins and the review before a merge.

## Run a ticket

1. One-off, and again whenever `main` moves or `Cargo.lock` changes. Run it in a Windows
   terminal:

   ```bat
   wsl -d Ubuntu-24.04 -u root -- bash /mnt/f/AI/Scripts/overnight/harness/kk_setup.sh
   ```

   It installs Rust 1.99 at `/opt/rust`, mirrors this repo into `/opt/kitkraft/repo.git`, and
   vendors every crate into `/opt/kitkraft/vendor`. The sandbox has no network, so these copies
   are all it builds from.
2. Close Starfield, LM Studio and any browser that holds GPU memory. Then start the run:

   ```bat
   overnight kitkraft KK-001 4
   ```

   That runs ticket `plan/tickets/KK-001.toml` for at most 4 hours, with a grade every 30 minutes.
   Opus checks in at minutes 3, 8 and 26, while the agent is choosing its approach. After that,
   each check-in picks when the next one comes, between 15 and 60 minutes: sooner when the agent
   is stuck or off track. Check-ins see the agent's recent commands and their results as well as
   its commits.
   When every check passes, Opus does a final QA of the whole change. It looks for what a passing
   grade misses: weakened tests, Windows behaviour changes, silent workarounds and claims the agent
   could not verify. `VERDICT: CHANGES` sends its notes back to the agent and regrades 15 minutes
   later, at most twice. `VERDICT: ACCEPT` ends the run. If the run ends with the checks passing
   but no accepted QA, a last QA is written for the merge review.
3. Check on it with `overnight status`, or stop it with `overnight stop`. The final grade still
   runs after a stop.
4. Results land in `F:\AI\Scripts\overnight\runs\kk-KK-001-<n>\`:
   - `KK-001.bundle`: the agent's commits on branch `ticket/KK-001`.
   - `scores.jsonl`: every grade.
   - `grades\<label>\`: one log per check, plus `grade.json`.
   - `checkins\`: the Opus notes. The final QA notes are `qa-<n>-min<m>.md`, ending in the
     verdict.
   - `report.html`.

## Merge a finished ticket

The local agent never pushes. Review and merge happen on Windows:

```bat
cd C:\Modding\kitcraft
git fetch F:\AI\Scripts\overnight\runs\kk-KK-001-<n>\KK-001.bundle ticket/KK-001:ticket/KK-001
git log --oneline main..ticket/KK-001
git diff --stat main ticket/KK-001
```

Read the last QA note in `checkins\` first: it lists what to check by hand. Then run
`cargo xtask ci` on the branch with `KITKRAFT_DATA_DIR` set. The Linux grade cannot see
Windows-only behaviour, so this second gate matters. Merge only when it is green. A short Claude
review, `/code-review` on the branch, is the cheap second opinion. After merging, rerun
`kk_setup.sh` so the next ticket starts from the new `main`.

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

## What the agent can and cannot do

- **Can:** read and edit the repository, run `cargo`, `git`, `python3` and read-only shell tools.
  It can also read the game's Data folder at `/opt/kk-data` (`KITKRAFT_DATA_DIR`).
- **Cannot:** reach the network, add crates, see the grader's results, read `/mnt`, or launch the
  game, MO2 or the Creation Kit.
- **Should not read:** `plan/STATUS.md` and `plan/execution-plan.md`. They are too large for its
  136k context, so every ticket's brief carries what it needs.

## Known limits (2026-10-23)

- Tests that read the Data folder go through WSL's `/mnt/c`, which is slow. A full `cargo test`
  took more than 10 minutes on Linux, against about 5 on Windows. Copying the masters onto WSL's
  own disk would speed it up.
- The window, CK sittings, in-game checks and license rulings stay with Claude or the owner.
  Tickets cover headless engine, schema, docs and test work.
- Results so far are for tinysql, a small greenfield Python task. KitKraft is about 130,000 lines
  of Rust, so expect small tickets to work and large ones to stall. Cut tickets small.
