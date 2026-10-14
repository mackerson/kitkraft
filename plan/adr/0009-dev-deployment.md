# ADR 0009: Development deployment into an isolated MO2 mod

- Status: **Accepted** (owner ruling 2026-10-14; decides `plan/execution-plan.md` M4.O5)
- Date: 2026-10-14
- Scope: design constraints for `kit deploy --dev` (engine commands `deploy`,
  `deploy.rollback`, `deploy.status`); planned as lane E task M4.E11. No code now.
- Relates to: AGENTS.md "Everything is a command" and "Windows specifics" (the MO2
  profile is read, never modified), M4.E5 (`package`), ADR 0002 (`crates/io` is L3, MO2
  I/O), ADR 0008 (runtime component binaries are packaged as opaque files)
- Supersedes: the M4.O5 default text ("`package` ... refuses MO2's `mods\`" stays true for
  `package`; the "Not in M4: writing into ... MO2's `mods\`" bullet gains the one exception
  below)

## Decision (owner ruling 2026-10-14, recorded in the owner's meaning)

M4.O5's default is **approved with an exception**.

1. **Packaging stays target-independent.** `package` writes a folder (and an optional
   zip) outside MO2 and never implicitly writes into an MO2 installation. Normal
   archive-based installation (the owner installs the package through MO2) remains the
   distribution path.
2. **A separate development deployment** exists, invoked explicitly, targeting one
   isolated KitKraft-managed MO2 mod. It requires path validation, safe replacement,
   diagnostic output and rollback.
3. It **never overwrites unrelated managed mods** and **never alters profiles or load
   orders implicitly**.

## Design constraints

### Command shape

- `deploy` (CLI `kit deploy --dev`): `{source, mo2, dev, plan?}`. `source` is a folder
  written by `package` (it carries `kitcraft-manifest.json`); `mo2` is the MO2 instance
  root; `dev` must be `true` on every call, and `deploy` without it is refused (no other
  deployment mode is designed). `plan: true` reports what would change and writes
  nothing.
- `deploy.rollback` (CLI `kit deploy rollback --dev`): `{mo2, mod, to?, dev}` restores the
  previous deployment (or the one named by `to`).
- `deploy.status` (CLI `kit deploy status`): `{mo2, mod?}`, read-only: the deployments
  recorded for KitKraft dev mods in that instance.
- **Opt-in per invocation.** `dev: true` and `mo2` are required arguments on every call.
  No configuration file, environment variable, session default or MCP preload supplies
  them. Over MCP the commands carry `destructiveHint` (M4.F1). Agents do not run `deploy`
  against the owner's MO2 instance unless the owner asks for it in that session; tests use
  a synthetic MO2 instance in a temp dir.
- **This is the only MO2 write path in KitKraft.** Every other command keeps refusing
  outputs under MO2's `mods\`, profiles, `overwrite\` and the game Data dir.

### Target

- The target is exactly one folder: `<mo2>\mods\KitKraft Dev - <name>\`, where `<name>` is
  the file stem of the package's first plugin. The prefix is fixed. A name with characters
  Windows forbids, a reserved device name, or a trailing dot or space is refused, never
  rewritten.
- **Ownership marker.** A KitKraft dev mod contains `kitkraft-deploy.json` (below).
  `deploy` writes into an existing folder of the target name only if that marker is present
  and names this mod; a folder with the same name (case-insensitively) without the marker
  is refused. No other folder under `mods\` is ever opened for writing.

### Path validation

Before any write:

- `mo2` is an MO2 instance: it holds `ModOrganizer.ini`, `mods\` and `profiles\`
  (`crates/io` validates it, read-only).
- Every path is made absolute and canonical; the target must be a direct child of
  `<mo2>\mods`. Any component of `mo2`, `mods\` or the target that is a symbolic link or
  junction (reparse point) is refused, so no write can be redirected outside the mod.
- Every file path from the package manifest is relative, has no `..`, no drive or root, no
  alternate data stream (`:`), and resolves inside the target; the full path stays within
  the Windows path limit KitKraft supports.
- The target is not, and is not inside, the game Data dir, a profile folder, `overwrite\`,
  `downloads\`, or the `source` folder.
- The package is verified first: every file's size and SHA-256 equal its
  `kitcraft-manifest.json` entry, or nothing is deployed.

### Manifest

`kitcraft-deploy.json` in the target records: the marker (`kitkraft-dev-mod`, mod name),
the deployment id, time, KitKraft version (file and marker names follow the pending
identifier rename: `kitcraft-` today, `kitkraft-` after it), the source package's manifest hash, every file
(relative path, size, SHA-256), and the previous deployment id. It is written last and is
the commit point.

### Safe replacement

1. Verify the package (above). Take a journal entry `in-progress` in KitKraft's own state
   folder (outside MO2).
2. Back up the files the current `kitkraft-deploy.json` lists into the state folder
   (`backups\<mod>\<deployment id>\`), hashes verified.
3. For each new or changed file: write `<path>.kitkraft-tmp` next to its final name, flush,
   verify its SHA-256, then rename it over the final name (same directory, so one volume
   and an atomic replace).
4. Delete files the previous deployment listed and the new one does not, only if their
   hash still equals the recorded one. A recorded file whose hash changed (edited by
   hand) is not deleted or overwritten: `deploy` stops before step 3 and reports it.
5. Write the new `kitkraft-deploy.json` by temp file and rename; mark the journal entry
   committed.

Files in the target that no deployment manifest lists are never touched and are reported
as unmanaged. A target file that is locked (the game or the CK holds it) stops the deploy
before step 3. On any failure before step 5 the deployment rolls back automatically
(restore from the step 2 backup, remove `*.kitkraft-tmp`); a journal entry still
`in-progress` at the next call is recovered first, before anything else.

### Rollback

`deploy.rollback` restores a recorded earlier deployment with the same algorithm (so a
rollback can itself be rolled back). The state folder keeps the last three deployments per
mod by default; older backups are pruned only after a successful commit.

### Never touched

MO2 profiles and everything in them (`modlist.txt`, `plugins.txt`, `loadorder.txt`,
`archives.txt`, profile INIs), `ModOrganizer.ini`, `overwrite\`, `downloads\`, any other
folder under `mods\`, and the game folder. Consequence: after the first deployment the owner
enables the new mod and its plugin in MO2 once (and refreshes MO2); `deploy` does not.

### Diagnostics

The result envelope lists: the target path; the deployment id and the previous one; files
added, replaced, removed and unchanged with sizes and hashes; the backup location; refused
or unmanaged files; and, read-only from the profile `crates/io` can read (named by an
optional `profile` argument), whether the mod is enabled and where its plugins sit in
`plugins.txt`, as warnings (for example "mod not enabled in profile Default"). Every
refusal names the rule and the path.

## Consequences

- `plan/execution-plan.md` M4.O5 is decided; M4.E11 (lane E, after M4.E5) implements this;
  `crates/io` gains a read-only MO2 instance check (lane D). `package` is unchanged.
- The first POI and the M2 rename keep the archive-based install; `deploy` is a
  development convenience and is on no critical path.
- AGENTS.md "Windows specifics" gains one line: the MO2 profile is still never modified;
  `kit deploy --dev` (ADR 0009) is the only write into `mods\`.
