# ADR 0012: CK interface parity, clean room

- Status: **ACCEPTED** (owner ruling 2026-10-16, M7.O1). The owner, in chat, verbatim:
  "Approve Amendment 3 and ADR 0012, start lane V mockups". The rule had been described
  in `plan/execution-plan.md` (M7 binding constraint 1, ticket M7.K1) and `ROADMAP.md`
  ("M7: CK parity") but had no file yet. This file was written at the ruling from that
  description, so it starts as Accepted and never existed as a Proposed draft (M7.K1 is
  closed by it).
- Date: 2026-10-16
- Scope: how M7 reproduces the Creation Kit's interface (windows, menus, dialogs, labels,
  workflows, keyboard shortcuts, camera controls) in `crates/ui-egui`. It covers which
  source may inform an interface element, what is never copied, and what evidence a row
  of `docs/ck-parity.md` "Interface parity" needs at each status. It also sets when the
  NIF source ruling (M7.O5) must be made. No code.
- Relates to: ADR 0001 (clean room; the CK as a black box), ADR 0004 and ADR 0005
  Amendment 2 (the four-level source hierarchy for data facts), ADR 0006 (archive, script
  and string sources), ADR 0002 Amendment 1 (the viewport is L5 view code fed by engine
  commands), ADR 0007 Amendments 3 and 4 (the cockpit and editor write scopes), AGENTS.md
  "Clean room", `docs/ck-parity.md`, `docs/ck-observations.md`

## Context

The owner's UI track decision of 2026-10-16 (`plan/STATUS.md`, "Open owner decisions",
row "UI track") puts CK parity after the cockpit. In M7 a Creation Kit user should find
each window, menu item, dialog and shortcut in KitKraft where they expect it.

Data facts already have their rules: ADR 0004 and ADR 0005 grade every source of a record
layout. The interface is a different kind of material. It is what the CK's windows look
like and how they behave, and two risks come with it:

- **Copying protected expression.** That covers icons, images, help text, resource
  files, and menu or string tables pulled out of the binary.
- **Recollection taken for evidence.** The seeded menu order in `docs/ck-parity.md` is
  the owner's memory of the CK, not an observation (director decision 4, 2026-10-16).

## Decision

### 1. The only interface source is the owner-assisted sitting

- An interface row is filled only from an owner-assisted CK sitting (M7.O2, M7.O3; they
  can share a sitting with `plan/STATUS.md` rows 12 and 15). The CK runs through MO2
  (executable #4) as a black box, under ADR 0001 and AGENTS.md "Clean room". A
  computer-use agent may drive it while the owner watches.
- Screenshots and notes from a sitting stay in `observe\` (gitignored) and are never
  committed. A row cites the sitting id and the screenshot file names only.
- Anything that is not a sitting is not interface observation. That includes the
  public CK wiki, tutorials, videos, forum posts, and anyone's recollection. The wiki
  stays a level-4 source for data facts (ADR 0005 Amendment 2, AGENTS.md "Clean room"),
  and no interface row moves to `observed` on its word. Such material may suggest what a
  sitting should look at, nothing more.
- The CK's resource files (menus, dialog templates, string tables, images) are never
  read to extract menus or strings. The CK is never disassembled, decompiled,
  debug-attached or memory-dumped (ADR 0001).

### 2. What may be reproduced

Each of these is reproduced from what a sitting showed and described in our own words in
`docs/ck-parity.md`:

- **Layout**: which windows exist, how they are arranged, the panes inside them, field
  grouping and tab names.
- **Labels**: window titles, menu items, tab, group and field labels. These are the names
  a CK user looks for, so they may be repeated as names. No other CK text is reproduced:
  no tooltips, status-bar text, help pages or message wording (M7.K3: "no CK text beyond
  the labels themselves").
- **Menu structure and order.**
- **Workflows**: the steps a task takes (open a form, edit a field, duplicate, place,
  save).
- **Keyboard shortcuts and camera controls** (mouse and keys), as observed. A control
  not yet observed is marked so, never guessed.

### 3. What is never copied

- CK icons, images, cursors and splash art, its help text, its resource files and its
  binaries. Nothing is extracted from the CK or shipped with KitKraft.
- **All art is ours.** Icons, glyphs, toolbar images and viewport markers are drawn by
  KitKraft (egui's painter, M6.V4). A third-party asset is used only under the license
  rule (ADR 0007, M6.O2) and is listed in `ATTRIBUTION.md` first. It is never the CK's.

### 4. How a row moves: unresearched, observed, reproduced, improved

A row in `docs/ck-parity.md` "Interface parity" moves forward only on the evidence named
here. It never skips a step: an `unresearched` row cannot become `reproduced`.

| Status | Meaning | Evidence the row must carry | Who moves it |
| ------ | ------- | --------------------------- | ------------ |
| `unresearched` | Not yet seen in a sitting | None. "Source" may name a recollection ("owner, 2026-10-16 (recollection)"); that is not evidence, and nothing is built to match it (M7.M1 waits on M7.K3) | Lane K seeds it |
| `observed` | Seen in a sitting | "Source": the sitting id (its entry in `docs/ck-observations.md`, with the date and CK version) and the screenshot file names kept in `observe\`. The row gives our own description of what was seen | Lane K (M7.K3, M7.K4), from the sitting |
| `reproduced` | KitKraft has it as observed | "Evidence": a tour image of KitKraft's window, named beside the observation's screenshot (tour images are kept outside the repo, as M6.V2's are), or the test that checks it (for example a shell-model test of a menu mapping or a shortcut binding) | The lane that builds it. The reviewer compares the tour image or test with the observed description |
| `improved` | KitKraft has it as observed and does more | As `reproduced`, plus one line saying what more. The observed element must still be there, where a CK user expects it | As `reproduced` |

- **Moving back.** If a later sitting (for example on a new CK version) shows that an
  element changed, the row goes back to `observed` with the new sitting id until
  KitKraft matches again.
- **Gaps.** "Maps to" may be `gap:` with the reason. A gap that is KitKraft's gets a
  `docs/gaps.md` row. For M7's exit, an observed row is `reproduced`, `improved`, or a
  documented gap (`ROADMAP.md` "M7: CK parity").
- **The check.** M7.K5's test reads `docs/ck-parity.md`. It fails on a menu-item row that
  maps to no existing catalog id and is not `gap:`, and on a `reproduced` or `improved`
  row that names no tour image or test.

### 5. The NIF source ruling comes before geometry work

The viewport gets bounds and geometry only from engine commands (ADR 0002 Amendment 1).
The layout of Starfield NIF and geometry data is a data fact, not interface. It falls
under the source rules of ADR 0004, ADR 0005 and ADR 0006 by analogy, not under this ADR.
M7.O5, the owner's ruling on which documentation or tools may be used and under which
license category, is made and recorded in `docs/source-registry.md` before M7.S4 starts.
M7.S5 (geometry) follows M7.S4, so it waits on the same ruling.

## Alternatives considered

- **Fill rows from the CK wiki and tutorials.** It would be faster. But those are
  secondary sources, often about older Creation Kits, with their own copyright in the
  screenshots and prose. Rejected for interface rows; the wiki stays a level-4 data
  source.
- **Read the CK's resource files for menus and strings.** It would be exact, but it
  extracts Bethesda assets, which ADR 0001 forbids. Rejected.
- **Do not reproduce the CK's interface at all** and use KitKraft's own design only. This
  was rejected by the owner's UI track decision: M7's goal is that a CK user is at home.

## Consequences

- `plan/execution-plan.md` "M7": M7.K1 is done with this file and M7.O1 is decided
  2026-10-16. Lane K's M7.K2 (the sitting scripts) can start. The sittings themselves are
  still the owner's (M7.O2, M7.O3).
- AGENTS.md "Clean room" gains a pointer to this ADR (M7.K1's "done when", now that M7.O1
  is decided).
- `docs/ck-parity.md` "Interface parity" and `ROADMAP.md` "M7: CK parity" cite it as
  accepted.
- Nothing changes for data facts. ADR 0004 and ADR 0005 and the `ck-diff` confidence
  level (CK differential observation of saved plugins) stand as they are.
