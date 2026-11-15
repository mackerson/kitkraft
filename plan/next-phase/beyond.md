# Beyond the Creation Kit: the current list reviewed, and candidates

Written 2026-11-14. Inputs: `ROADMAP.md` "M8: Beyond the CK (sketch)", `plan/execution-plan.md`
"M8 (sketch)", the two research files in `research/`, and the hole list in
`mod-ideas-support.md`. Sizes are rough: S about one to three tickets, M about a week of
nightly queue, L several weeks, XL a milestone. Evidence strength is the helpers'
judgement from documentation and forum posts; no survey exists for any of it.

## What the roadmap says today

`ROADMAP.md` lists nine M8 bullets; the execution plan turns them into eight example
tickets (M8.1 to M8.8). M8 has no exit criteria, and it says each write needs a further
ADR 0007 amendment and the agent panel an ADR for the MCP journal.

| M8 item | Review |
| ------- | ------ |
| Search everywhere (M8.1) | Keep, and widen: one palette over commands and records. The catalog already gives every command a label, menu path and aliases, so the command half is nearly free. Strong outside evidence (the CK lacks filters that the community patch adds; Blender's operator search). The CK's own Find Text and filter rows are already mapped to `query` |
| Inline validation and conflicts on every record (M8.2) | Keep. Add what the research says makes validation liked elsewhere: suppressions, quick fixes, rules listed as data. `kitkraft://validate-rules` is already the last of these |
| Diff against masters on any record | Keep; mostly exists (`inspect`'s `vs_previous`, `diff`, `conflicts`). It is a UI ticket, not an engine one |
| Undo history from the engine's diff (M8.3) | Keep. `revert` exists; what is missing is a session journal of confirmed writes. Share that journal with the agent panel: one design, two views |
| Agent panel over an MCP journal (M8.4) | Keep and move up. Agents are this project's main authors (owner, 2026-10-16), and the outside evidence is that AI edits are trusted when they arrive as a plan, a reviewable diff and one undoable step |
| Multi-plugin load-order view (M8.8) | Keep. This is the conflict matrix modders use another tool for today; `conflicts` has the data |
| Probe results beside records (M8.5) | Keep; unique to KitKraft (nothing else has the ledger). Small once the ledger rows carry form IDs |
| Viewport materials, textures, lighting (M8.6) | Demote. XL, needs a materials source ruling, and the research found material *workflow* complaints (database, exporters), not a wish for a prettier editor viewport. The sprint branch already draws flat-shaded meshes |
| Snap-to-grid placement editing (M8.7) | Keep, after M7's placement from the windows is proven in a real window |

What the list lacks is anything outside the window. Seven of its nine items are UI. The
research says the most-valued things other ecosystems have are **project-level**: text you
can put in git, clean output, repeatable patch pipelines, a fast test loop. KitKraft's
engine is unusually well placed for those, and they do not wait for M7.

## Ranked candidates

Ranked by (evidence of demand) x (how many of the owner's mods it helps) / (size and
blockers). "Fits the rules" is about AGENTS.md "Clean room" and the write-scope ADRs.

| # | Feature | What it is | Who asks, or what proves it | Size | Depends on | Fits the rules |
| - | ------- | ---------- | --------------------------- | ---- | ---------- | -------------- |
| 1 | **Clean by construction, and a clean check** | `validate` rules for identical-to-master overrides and for records a tool touched without changing; `kit diff` between two versions of one plugin as a delta report; a "nothing unintended changed" line in every build report | The most pervasive CK complaint sampled (dirty edits need another tool); xEdit's cleaning and compare. KitKraft already writes only touched records and already classifies `IdenticalToPrevious` | S | nothing; first ticket drafted (`drafts/validate-override-identical.toml`) | Yes |
| 2 | **Plugin as a text project** | `kit export` a plugin to one deterministic text file per record and `kit import` back, byte-identical; opaque payloads as hex; the schema version pinned in the project; a git diff driver | CK version control is unused by teams; file-per-entity text is the industry answer (Unreal, Godot, Unity's merge tool); the existing Bethesda tool for this exists and, per `docs/gaps.md`, lost data on 2 of 5 real mods here. Byte-exact round trip is KitKraft's edge | L (M for export and import of typed types; more for a merge driver) | schema depth decides how readable the text is; a format design note first | Yes, **if the format is designed from our own schema names**. Do not look at the existing tool's output field names (AGENTS.md says so explicitly). Needs an ADR |
| 3 | **One-command test loop, with documented limits** | `kit mod run` live end to end (build, validate, deploy, launch to a save, probes, report), a watch mode that reruns on spec change, and a page that says exactly what needs a restart and what a console command can refresh | The owner's own top pain ("game loop integration"); hot reload is the best-evidenced feature in other ecosystems (Cyberpunk, Minecraft, Stardew tools), always with stated limits | M | game sessions (exist); `mod.run`'s session stage was refused until M4.T1, which passed 2026-11-01: whether it now runs live is unverified; probes for override-only edits (hole H6) | Yes: console endpoint and files only (ADR 0010). No injection, no hooks |
| 4 | **Search everywhere and a command palette** (M8.1) | One box: commands from the catalog, records by EditorID, FormID, name, text, script name; results grouped by type; the same query from CLI and MCP | CK lacks filters the community patch adds; Blender's search; a paid add-on exists to add data search there | M (S for the command half) | an engine search index beside the index cache | Yes (read-only) |
| 5 | **Validation people keep on: rules as data, suppressions, quick fixes, a CI recipe** | Per-project suppressions with a reason; a severity gate; a suggested fix on a finding that is itself a previewed command (`set`, `revert`); a documented GitHub Actions recipe for a mod repository | Paradox's CWTools (rules as data, ignore settings, a CI action); Unreal's validation plugin (third parties sell default rules because none ship); an issue browser with fixes in a Quake editor | S to M, in slices | `validate` (22 rules after the sprint merge); the write-scope ADR for fixes applied from the window | Yes; a fix is an ordinary gated write |
| 6 | **Agent panel and transactional agent edits** (M8.4 with M8.3) | A journal of what each MCP session ran and wrote; in the window, each step with its diff and "revert this step" or "revert the session"; an MCP "plan" mode that returns the dry runs of a batch before any write | Roblox's assistant (planning mode, undo for AI edits); the reported failure mode elsewhere is opaque AI edits. Every KitKraft write already has `dry_run` and `revert` | M | the journal ADR; shares its store with undo history | Yes; needs the ADR the roadmap already names |
| 7 | **Selector patches and list merging** | Build spec rules that select records by query ("every armor with keyword K") already exist in spec v2; add list append and remove (hole H5), a conditional block (only if plugin P is loaded), a "why did this rule apply or not" report, and a generated merge patch for leveled and form lists across the load order | Synthesis pipelines, bashed patches, RimWorld patch operations, Stardew's Content Patcher (conditions and a patch summary). Four of the owner's seven mods hit H5 | M | `set-list-append` draft; `conflicts` | Yes |
| 8 | **Load-order matrix and guided conflict resolution** (M8.8, widened) | Plugins as columns, records as rows, winners marked; "make a resolution patch" that carries chosen fields forward, with the unmergeable listed plainly | xEdit's conflict view is the reason modders keep it open beside the CK; Witcher 3's script merger; Irony's per-definition conflicts | M to L | `conflicts`, `diff`; remap limits in `docs/gaps.md` ("Patch FormIDs") cap what can be merged | Yes |
| 9 | **The schema as documentation** | Hover help on every field: name, type, where the fact comes from, confidence, and whether it has been seen in game; `kit schema` published as versioned machine-readable docs per game version | "No documentation" is the second most pervasive complaint; Factorio's versioned machine-readable docs. KitKraft already stores source, evidence and confidence per member, which no other tool shows | S (in-tool), S to M (published) | nothing for the in-tool half | In-tool: yes. **Publishing: owner ruling.** The definitions were written from the pinned xEdit file as source material (ADR 0004); they are ours, but a public reference site is a publishing act (owner decision 6) and deserves a licence note |
| 10 | **Plugin flags and master hygiene** | Convert an existing file to small, medium or full; compact FormIDs; drop unused masters; a pre-publish check (size limits, loose files, flags) | Complaints 5 and 12; the wiki's current advice is a hex editor; the CK ranking on the sprint branch names header conversion the one P1 feature with no command | M | CK fixtures F06, F11, F26 for confidence, or a ruling to build at grade D (hole H9) | Yes; refuses rather than guesses until evidence exists |
| 11 | **Scaffolds from recipes** | `kit new --from <recipe>`: the nine recipes in `docs/recipes/` become parameterised starting points (a terminal, a location, a colourway, a recipe), and "new from this vanilla record" in the window | MCreator and Blockbench wizards; Misode's schema-driven forms; thin community documentation makes worked examples valuable | S to M | recipes exist and are corpus-tested | Yes |
| 12 | **Session safety: journal, autosave, crash recovery** | Every confirmed write journaled before it lands; reopen after a crash offers the journal; the active file is never left half-written | Crashes and lost work is the top complaint. KitKraft has no crash history to speak of, so this is cheap insurance, not a fix | S | the journal of item 6 | Yes |
| 13 | **Environment lock and doctor** | `kit env lock` and `verify`: game, DLC, CK, compiler and plugin hashes in one file a session checks before it runs; `kit doctor` checks the Papyrus flags file and source folders | Wabbajack and packwiz lockfiles; Papyrus setup complaints. `docs/compatibility.md` holds this by hand today | S | - | Yes |
| 14 | **Grid view and CSV round trip** | `query` to CSV, edit in a spreadsheet, `set` from CSV as one previewed batch; in the window, a multi-record grid edit | Unreal data tables; LDtk's simple export; "no multi-record edit" (unverified as a complaint) | S to M | batch preview | Yes |
| 15 | **Live record inspector** | While a session runs, show a selected form's live values beside its record values | Godot's remote scene tree | M to L | which values the console can read reliably is unknown (all answer formats are grade D) | Yes, console only |
| 16 | **Dialogue and quest previewer** | Step through a quest's stages and a topic's lines with conditions evaluated against a chosen state | Ink's editor (replay to the same point) | L | holes H4 and H1 first | Yes |
| 17 | **Viewport materials and lighting** (M8.6) | As the roadmap has it | Weak outside demand found | XL | a materials source ruling | Needs a ruling |
| - | Real-time multi-user editing | - | Weakest evidence for modders; text projects plus git cover collaboration | XL | - | Not recommended |
| - | Navmesh, terrain, voice and lip generation | - | Real gaps, but they need the official editor's middleware (helper's inference) | - | - | Out of scope (ROADMAP "not in M7") |

## Ticketable before M7 closes

None of these touches `crates/ui-egui`, so they can run on the nightly queue beside the M7
work: candidate 1 whole; candidate 7's list edits; the engine halves of 4 (search index),
5 (suppressions, severity gate), 13 and 14; the design notes for 2 and 6 (Claude agent,
docs only).

## What I would change in `ROADMAP.md` "M8"

1. Split M8 into **M8a "Project" (engine and CLI, no window)**: candidates 1, 2, 3, 5, 7,
   10, 13; and **M8b "Editor"**: 4, 6, 8, 9, 11, 12 and the existing viewport items. M8a
   does not depend on M7 and can start now.
2. Give M8a exit criteria in the project's usual form, for example: a real mod repository
   round-trips export and import byte-identically; a build from a spec is reported clean;
   `kit mod run` closes its probes on the dev profile with no hand step.
3. Move the agent panel ahead of viewport materials.

These are suggestions; the roadmap is the owner's.

## Confidence

- The CK complaints are well attested for stability, documentation, dirty edits, load time
  and master flags; thin for undo, modal dialogs and multi-record editing.
- "Users value X" for other tools rests on documentation and scattered posts. The one
  place the evidence is concrete and local is candidate 2: this repo measured the existing
  text-serialisation tool losing data.
- Sizes assume the sprint branch is merged and are guesses; none was estimated from code.
