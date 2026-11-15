# Next phase: summary for the owner

Written overnight, 2026-11-14, by a planning agent. Plans and analysis only: nothing was
built, run, committed or pushed, and no other checkout was touched. These files sit
uncommitted in worktree `C:\Modding\kitkraft-worktrees\planning-20261114` (branch
`plan/next-phase-20261114`, cut from `main` at `a0b8b642`).

**Committed 2026-11-15 (KK-324) with this note; the files below are as written, with
`[done 2026-11-15: ...]` marks where `main` has moved since.** What changed:

- `sprint/2026-11-09` is merged (`23d62954`, 2026-11-15): decision 1 below is answered. It
  brought KK-250, KK-251 and `masters.unflagged` (KK-262 also landed it, `53531a13`).
- KK-252 to KK-262, KK-265 and KK-266 landed through Opus lanes on the night of 2026-11-14
  (`2060e072`, `720c8d76`, `53531a13`): seven types typed, typed-write tests for twelve,
  clone-sample tests for four more. KK-263 never ran (held); KK-264's four `docs/gaps.md`
  rows were fixed by KK-321. `plan/tickets/README.md` has their rows (KK-323).
- Conditions (hole H1): `docs/design/conditions.md` and KK-267 to KK-272, merged `8d6c998a`:
  `CTDA` typed and writable member by member (tests through `clone` and `set`), the type byte packed into
  operator and flags, `Function` an enum, `CITC` kept in step, a conditions recipe. The
  alias and packdata parameters stay raw (EL-03).
- Dialogue (hole H4, `DIAL` and `INFO`): `docs/design/dialogue.md` and KK-274 to KK-279,
  merged `92773276`: `new` and `clone` write a topic under a quest and a response under a
  topic; unverified against the CK (`docs/gaps.md`). `SCEN` is unchanged.
- Component blocks (hole H10): `docs/design/component-blocks.md` and KK-280 to KK-288,
  merged `957773f5`: eight more layouts typed as lead-signature groups (22 of 64 kinds).
  ADR 0015, the component-name decider, was deferred on `main` (`98f94072`), then accepted
  by the owner the same day ("Go ahead and build it") and is being built (KK-292, not on
  `main` when this was committed).
- Drafts: `drafts/` here is not read by the overnight picker (it reads only
  `plan/tickets/KK-*.toml`); each draft keeps `id = "DRAFT"` and needs promotion first.

| File | What it is |
| ---- | ---------- |
| `roadmap-remaining.md` | What is left per milestone, what waits on you, what can be ticketed |
| `mod-ideas-support.md` | Your mods against what KitKraft supports, the ranked hole list, the first ten items |
| `beyond.md` | The M8 list reviewed, then 17 ranked "beyond" candidates with evidence |
| `research/` | The two outside-research reports with their source URLs |
| `drafts/` | Five ticket drafts, `id = "DRAFT"`, parsed as TOML, not linted, not run |

## Where the project is

- The mapped ticket scope is done or running: KK-243 to KK-251 this weekend, KK-252 to
  KK-266 tonight. Schema depth by ticket is nearly exhausted; what remains is format work
  (the component-name decider, the reflection reader).
- **`sprint/2026-11-09` is 144 commits ahead of `main` and unmerged** (last commit
  2026-11-10). It holds most of M7's recent progress (33 interface rows reproduced, 4
  improved, 127 observed; meshes in the Render Window; session replay) and it already
  contains KK-250, KK-251 and the `masters.unflagged` rule that tonight's KK-262 writes
  again. `ROADMAP.md` "Where we are" is dated 2026-10-30 and predates two sprints.
- Proof lags building. 186 record types parse; 24 have write tests (up to 36 if
  all four of tonight's write-test tickets pass); 5 have been seen in game, 2 through the CK, 2 after a reload. Most of your
  recipes and prototypes are built, validated and never run.
- **I did not find a "mod ideas" document.** `mod-ideas-support.md` says where I looked and
  uses the seven mods you have written down instead (gun overhaul next build, MEL DLC,
  Everyday Outfits, Foothold F0 and F1, hostile encounters, the SS2-style port), labelled
  as a substitute.

## Five decisions that are yours

1. **Merge `sprint/2026-11-09`, or say what it waits for.** Until then every UI, validate
   and testplan ticket risks duplicating or conflicting with it, and the planning docs
   describe a project two sprints old. My recommendation: merge first, before anything else
   below. (A trial merge showed one conflicting file; I did not resolve or keep it.)
2. **Where the mod ideas are.** If there is a list, point to it and the support analysis is
   redone against it. If the seven mods above are the list, say so.
3. **Spend game and desk time on proof, or keep building.** The top-ranked hole is not
   code: it is the backlog of in-game rows. Options: (a) standing permission for agents to
   run sessions on `KitKraft Dev` overnight for a named list of plugins; (b) one desk
   sitting that covers the M6 sign-off (run sheet OW-2), the CK fixtures (F20, F23, F27,
   F28, F31) and the send-order session (OW-1); (c) both. I recommend (c), with (a) first.
4. **Does "beyond" start now, engine-first?** `beyond.md` proposes splitting M8 into a
   project half (clean output, text projects for git, the one-command test loop, list
   patches, validation with suppressions) that needs no window and can queue tonight, and
   an editor half that waits for M7. The alternative is to hold all of M8 until M7 closes.
   Two ADRs come with it: the text-project format and the MCP journal.
5. **Small and medium plugins.** New forms in them are refused and no command converts an
   existing header. Either build both at grade D now (refusing where unsure) or wait for CK
   fixtures F06, F11 and F26. This decides whether any of your mods can ship in the form
   Creations expects. Smaller rulings that could ride along are listed in
   `roadmap-remaining.md` (Data dialog rule, Cell View base column, lane X, hosted CI,
   publishing the schema as public docs).

## Recommended order, next two to three weeks of nightly queue

Assumes decision 1 is "merge". Local-agent nights take the headless tickets; Claude
sessions take design notes and anything that relaxes a guard or touches the write path.

**Week 1: close the cheap proof and the commonest workaround.**

- Night 1: merge the sprint; reconcile `ROADMAP.md`, `plan/tickets/README.md` and
  `docs/gaps.md` against it; drop or re-scope KK-262, KK-250, KK-251.
- Typed-write chain: tonight's KK-258 to KK-260 and KK-265, then the three drafts here
  (NPC_, LVLN, PACK; AMMO, PROJ, ZOOM; RSPJ, PERK, AVIF). One per night unless queued as a
  list, since they share three files.
- `set-list-append` (draft), then its build-spec follow-up.
- `validate-override-identical` (draft, held until the sprint's rule is on `main`).
- In parallel, Claude sessions: the two design notes that turn the biggest holes into
  tickets (authoring conditions; nested dialogue records), and the MEL DLC bounty-target
  re-run.
- If decision 3 allows: agent sessions for the recipes never seen in game (location,
  terminal, colourway, workshop, fabricator) and the overhaul's save/reload row.

**Week 2: the test loop and the first "beyond" pieces.**

- Probes for override-only edits (actor level, actor values), once one session has shown
  the console's answer format.
- `kit mod run` live on the dev profile, end to end, on the MEL DLC build.
- Tickets cut from the conditions design note (first: write a condition with typed
  parameters on a `COBJ`).
- Beyond, engine side: validation suppressions and a severity gate; the plugin delta
  report; `kit env lock` and `verify`.
- Design notes: the text-project format (with its ADR draft) and the MCP journal.

**Week 3: larger pieces, as the designs land.**

- Text project: export and import for the typed-writable types, byte-identical on the
  recipes' plugins first.
- The search index in the engine (CLI and MCP first; the window later).
- Member editing in the edit dialog, one field kind per ticket (after a Claude-agent split
  of `dialogs/editor.rs`).
- Exterior cells, or small and medium plugins, according to decision 5 and the fixtures.
- The component-name decider's design split, if schema depth is still wanted.

## How far to trust this

- Read from the repo and cited: the milestone states, the hole list's "today" cells, the
  sprint branch's contents.
- Inferred: which holes each mod hits where your documents do not name record types; all
  sizes.
- Not verified: why the sprint is unmerged; whether MEL DLC's bounty targets now build;
  whether `mod.run`'s session stage runs live; the row-level results of the 2026-11-03
  sessions.
- The outside research rests largely on search summaries, with no survey behind any
  ranking; each research file says what it could not find.
