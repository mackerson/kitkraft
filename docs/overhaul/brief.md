# Gun overhaul design brief (template)

Owner decision 3 (`AGENTS.md`, "Owner decisions"): which video critique is the design
brief for the gun overhaul. The owner fills this file in (`plan/execution-plan.md`,
M3.O4); lane E translates it into `docs/overhaul/overhaul.toml` (M3.E9). Until it is
filled, nothing here is a decision.

**Write every point in your own words.** Do not paste or transcribe the video's text,
captions or script, and do not quote it; a timestamp says where a point comes from. The
brief records what you want changed and how we will know it worked, not what the video
says.

## Source

| Field         | Value |
| ------------- | ----- |
| Title         |       |
| Channel       |       |
| URL           |       |
| Date watched  |       |
| Game version the video shows (if stated) | |

## Critique points

One row per point. `ID` is what the spec cites (`B1`, `B2`, ...): IDs are never reused or
renumbered; a dropped point stays in the table with "dropped" in Desired change.

| ID | Claim (owner's words) | Timestamp | Weapon classes affected | Desired change | Measurable target |
| -- | --------------------- | --------- | ----------------------- | -------------- | ----------------- |
| B1 |                       |           |                         |                |                   |
| B2 |                       |           |                         |                |                   |
| B3 |                       |           |                         |                |                   |

Columns:

- **Claim**: the problem as you see it, in a sentence or two of your own.
- **Timestamp**: `mm:ss` (or `h:mm:ss`) where the video makes the point.
- **Weapon classes affected**: in words (ballistic rifles, laser pistols, shotguns,
  "every weapon with the X keyword", named weapons). Lane E turns each into a spec class
  (`kit overhaul` lists the groups the load order actually has).
- **Desired change**: what should be different in game (more damage at range, less ammo
  weight, slower fire rate), not a field name.
- **Measurable target**: a number or a check someone can verify in game or in
  `kit build --plan` (e.g. "two body shots at 50 m kill a Spacer", "damage x0.8, never
  below 10"). A point with no measurable target cannot be accepted in M3.O5.

## Out of scope

What this overhaul deliberately does not touch, whatever the video says. Fixed by
`plan/execution-plan.md` ("Not in M3"): no new records, no leveled-list injection, no
scripts, and no fields outside the typed weapon records (WEAP, AMMO, PROJ, AMDL, and OMOD,
COBJ, ZOOM where typed). Add your own:

-

## In-game checklist (M3.O5)

Filled from the rows above; run on a disposable save with the built plugin last in the
load order (`docs/overhaul/build-1.md` will carry the exact steps, as
`docs/smoke-rename.md` does for the rename).

| Check | Brief row | Expected | Seen | Pass |
| ----- | --------- | -------- | ---- | ---- |
|       |           |          |      |      |

## Traceability rule

Every class and every rule in `docs/overhaul/overhaul.toml` names the brief row it serves
with `brief = "B<n>"` (spec v2, M3.E6). A class or rule without a row, or naming a row
that is not in the table above, is a review finding: the overhaul is not built from a
spec that has one (M3.E9). A row with no class and rule is listed as open in
`docs/overhaul/build-1.md`. The build report (`kit build --plan`) shows each
rule's `brief`, so every changed field traces back to a row, and each row to a
timestamp of the source.
