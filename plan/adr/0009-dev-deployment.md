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
  written by `package` (it carries `kitkraft-manifest.json`); `mo2` is the MO2 instance
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
  `kitkraft-manifest.json` entry, or nothing is deployed.

### Manifest

`kitkraft-deploy.json` in the target records: the marker (`kitkraft-dev-mod`, mod name),
the deployment id, time, KitKraft version (file and marker names use
the `kitkraft-` prefix; the identifier rename is done), the source package's manifest hash, every file
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

## Amendment 1 (2026-10-14): the implementation's departures, accepted

M4.E11 implemented this ADR from the owner's M4.E11 brief, which differs from the text
above in five places. `docs/deploy.md` ("Where this differs from ADR 0009's text") records
them; they are accepted as the design (director's ruling, recorded by the integrator in
M4.BX1). Where this amendment and the sections above disagree, this amendment wins.

1. **Rollback copy inside `mods\`.** The previous deployment is kept as
   `<target>.previous` beside the live folder, and the new one is staged in
   `<target>.staging` inside `mods\`, so every swap is a same-volume folder rename. This
   replaces the per-file temp + rename with backups in a KitKraft state folder outside MO2
   (three kept): one previous deployment is kept, and `deploy.rollback`'s `to` only
   confirms which one `.previous` holds. The `.previous` folder is a separate MO2 mod the
   owner leaves disabled.
2. **Journal beside the folders.** The operation journal is
   `<target>.kitkraft-journal.json`, a file in `mods\` (MO2 never lists a file as a mod),
   not in a state folder, so recovering an interrupted call needs nothing but the MO2
   instance.
3. **`dry_run` instead of `plan`.** The read-only preview is the `dry_run` argument (CLI
   `--dry-run`), the brief's word.
4. **`kitkraft-deploy.json`.** The ownership marker and deployment manifest written into
   the dev mod is `kitkraft-deploy.json` (with the `kitkraft-dev-mod` kind), using the final
   product name; the package manifest it reads is still `package`'s
   `kitkraft-manifest.json`.
5. **Unmanaged files refuse a deploy.** A file in the live folder that its manifest does
   not list (other than MO2's `meta.ini`) refuses the deploy (`target-modified`) instead
   of being left in place, because the folder swap would move it into `.previous` and a
   later deploy would drop it.

Unchanged: profiles, `ModOrganizer.ini`, other mods and the game folder are never written;
the `Default` profile is refused by name (AGENTS.md, 2026-10-14). Still open (Requests):
the MO2 instance check lives in the engine (`deploy/instance.rs`), not yet in `crates/io`
as the Consequences above say.
## Amendment 2 (2026-10-18): explicit dev-profile enable, owner ruling

Owner ruling (Michael, 2026-10-18, in session, quoted): "Let's amend ADR 0009 and add an
explicit dev-profile enable command (MO2 closed, Kitkraft Dev only)", and, on the plan,
"Approved as planned, including third-party mods by exact folder name in KitKraft Dev only."

Where this amendment and the sections above (or Amendment 1's closing paragraph)
disagree, this amendment wins. `deploy`, `deploy.rollback` and `deploy.status` are
unchanged: they never write a profile. The one exception is a separate pair of commands.

1. **`deploy.enable` and `deploy.disable`** (CLI `kit deploy enable --dev <mo2> --mod <m>
   ...`, `kit deploy disable ...`) are the only commands that write an MO2 profile. They
   are opt-in per call exactly as `deploy` is: `dev: true`, `mo2` and the mods are required
   on every call, nothing supplies them by default, they carry `destructiveHint` over MCP,
   and agents run them only when the owner asks in that session.
2. **The dev profile only.** The profile must be `KitKraft Dev` (matched ignoring case, so
   MO2's `Kitkraft Dev` folder counts). `Default` is refused by name (`live-profile`), and
   every other profile is refused (`not-dev-profile`).
3. **MO2 and the game closed.** The call is refused when `ModOrganizer.exe`, the game, the
   script extender, the editor or MO2's helper processes are running (the Starfield game
   profile's blocking list), and when the process list cannot be read: MO2 rewrites the
   profile's lists while it runs and when it exits.
4. **Three files, named lines only.** Only `modlist.txt`, `plugins.txt` and
   `loadorder.txt` change, and only the lines for the named mods and their plugins:
   `enable` turns `-<mod>` into `+<mod>` (or adds `+<mod>` under the header comments, where
   MO2 lists a new mod) and moves each plugin last as `*<plugin>` in `plugins.txt` and
   last in `loadorder.txt`; `disable` turns `+` into `-` and drops the plugin's `*` in
   place. Every other byte is kept (line endings, BOM, header, final newline). A list the
   editor cannot give back exactly (no prefix, mixed line endings, not UTF-8) or a
   non-empty `lockedorder.txt` refuses the call. `ModOrganizer.ini`, the profile's INIs,
   saves, `archives.txt` and every mod folder are never written.
5. **Which mods.** A `KitKraft Dev - <stem>` folder must carry its `kitkraft-deploy.json`
   marker (its plugin is the manifest's); any other mod is named by its exact folder name
   under `mods\` (its plugins are the `.esm`/`.esp`/`.esl` files at its root). Rollback and
   staging copies, missing folders and a dev mod with a pending deploy journal are
   refused.
6. **Backup, verify, restore.** Before writing, the three files are copied with their
   SHA-256 into `%LOCALAPPDATA%\kitkraft\profile-backups\<id>\` (outside MO2, so MO2 never
   sees stray files) with a `backup.json`; each file is replaced by temp file and rename
   and read back; any failure writes every file back from the backup. `dry_run` lists
   every line that would change and writes nothing. `disable` is the undo.

Consequences: the manual "enable the mod in MO2" step after `deploy` becomes optional; a
game session (ADR 0010) still verifies the profile is unchanged across a session, so the
enable runs before one. Whether MO2 keeps the edit when it next opens the profile is a
black-box fact recorded in `docs/game-observations.md` (owner check), not assumed.
Still the owner's: switching MO2's active profile, editing `Default`, or letting
`mod.run` enable on its own.

## Amendment 3 (2026-10-18): deploy writes the dev mod's `meta.ini`, owner ruling

Owner ruling (Michael, 2026-10-18, in session, quoted): "Do both: deploy writes a meta.ini
(version, category, comment) into its own dev mod", approved as planned.

1. **Inside the dev mod only.** `deploy` and `deploy.rollback` write MO2's `meta.ini` in
   the dev mod they manage, through the same staging, journal and swap as the other files.
   It is never listed in the deployment manifest, and nothing outside the dev mod changes
   (`categories.dat` is read, never written).
2. **New or updated.** With no `meta.ini`, a minimal one in MO2's observed layout is
   written. With one, only `[General]`'s `version=` and `comments=` are replaced, plus
   `category=` when the call names a category. Missing keys are added after `[General]`,
   and every other line, MO2's included, is kept. Rollback sets `version=` and
   `comments=` to the restored build's values.
3. **Where the values come from.** `version` is the argument, else the package's, else
   the date. `category` is a name in `<mo2>\categories.dat`, and an unknown name refuses
   the call. `comments` is `KitKraft dev build <deployment id>`. The deployment manifest
   records the version (an optional field; older manifests have none).
4. **MO2 open.** `deploy` still runs with MO2 open and warns that it should be
   refreshed. Whether MO2 writes its cached copy back when it exits is a black-box fact,
   and the owner's check is recorded in `docs/game-observations.md`.

Amendment 1's point that `meta.ini` is carried over still holds; this amendment adds
the keys above.
