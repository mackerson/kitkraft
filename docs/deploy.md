# Development deployment (`kit deploy --dev`)

M4.E11, ADR 0009 (owner ruling 2026-10-14). `package` stays target-independent and never
writes into MO2. `deploy` is a separate, explicitly invoked development deployment into
one isolated, KitKraft-managed MO2 mod folder. It is the only place KitKraft writes inside
a Mod Organizer instance. Code: `crates/engine/src/deploy/`, handler
`crates/engine/src/handlers/deploy.rs`.

## Operating notes

- **Run it with the game and the Creation Kit closed.** Both hold plugin and archive files
  open; a held file makes a folder rename fail, and the call then rolls back (nothing is
  lost, but nothing is deployed). Refresh MO2 (F5) after each deployment.
- **The `Default` gate is policy, not confinement.** `deploy` reads MO2's active profile
  from `ModOrganizer.ini` (`selected_profile`), which MO2 writes only when it exits or
  switches profiles; while MO2 runs, the file can name a profile other than the one on
  screen. The gate refuses what it can see (`Default`, or no profile at all); it cannot
  stop MO2 from showing the dev mod to `Default`. The writes themselves are confined to the
  dev mod's own folders whatever the profile.
- **MO2 lists the dev mod and its `.previous` in every profile, `Default` included**, as
  disabled entries at the bottom of the left pane (MO2 adds every new folder under `mods\`
  to each profile's list, disabled). This is harmless (a disabled mod contributes no
  files) but visible; leave them disabled outside `KitKraft Dev`.

## Commands

| Command           | CLI                                                    | Writes |
| ----------------- | ------------------------------------------------------ | ------ |
| `deploy`          | `kit deploy --dev <source> <mo2> [--profile P] [--dry-run]` | yes (one dev mod) |
| `deploy.rollback` | `kit deploy rollback --dev <mo2> <mod> [--to ID] [--profile P] [--dry-run]` | yes (one dev mod) |
| `deploy.status`   | `kit deploy status <mo2> [--mod M] [--profile P]`      | no     |

Arguments (JSON, same over MCP `execute` / `command_line`, which carry `destructiveHint`):

- `source`: a plugin (`.esm`, `.esp`, `.esl`) or a package folder (the plugin at the root,
  loose files and `.ba2` archives at their Data paths). A folder's
  `kitkraft-manifest.json` (written by `package`, M4.E5) is verified and not deployed.
- `mo2`: the MO2 instance root (the folder holding `ModOrganizer.ini`). Required on every
  call; no configuration file, environment variable or session supplies it.
- `dev`: required and must be `true` (`--dev`). The schema admits only `true`, and the
  handler checks it again.
- `profile`: the MO2 profile to check (read-only diagnostics). Never `Default`. A
  `profile` that is refused (`Default`, missing, a bad name) is never read for
  diagnostics. When `ModOrganizer.ini` names no active profile, `profile` must name an
  existing profile other than `Default` or the call is refused (`live-profile-unknown`).
- `dry_run`: report the plan, the refusals and every path that would be touched; write
  nothing. The result's `data.passed` is `false` when the real call would be refused
  (CLI exit 1).
- `mod` (`rollback`, `status`): `KitKraft Dev - <stem>` or just `<stem>`.
- `to` (`rollback`): the deployment id `.previous` must hold; refused otherwise.

## Layout in `mods\`

For a plugin `MyMod.esm`:

| Path                                          | What                                          |
| --------------------------------------------- | --------------------------------------------- |
| `KitKraft Dev - MyMod\`                       | The live dev mod: the files and `kitkraft-deploy.json` |
| `KitKraft Dev - MyMod.previous\`              | The deployment it replaced (the rollback copy) |
| `KitKraft Dev - MyMod.staging\`               | Only while a deployment runs                  |
| `KitKraft Dev - MyMod.retired\`               | Only while a deployment commits (the older copy being dropped) |
| `KitKraft Dev - MyMod.kitkraft-journal.json`  | Only while an operation is in flight          |
| `KitKraft Dev - MyMod.kitkraft-lock`          | Only while a `deploy` or `rollback` runs (the lock) |

The folder name is `KitKraft Dev - ` plus the plugin's file stem. A stem Windows forbids
(a reserved device name such as `CON`, a character in `<>:"/\|?*`, a trailing dot or
space) is refused, never rewritten. A package with several plugins at its root needs a
package manifest whose `plugins` list names the first; one plugin per dev mod.

MO2 lists every folder under `mods\`, so after a redeploy it shows
`KitKraft Dev - MyMod.previous` as a separate mod. Leave it disabled. MO2 keeps a mod's
version, category and notes in `meta.ini` in the mod folder (ADR 0009 Amendment 3):

- **First deploy:** with no `meta.ini` yet, `deploy` writes a minimal one in MO2's
  layout: `[General]`, `gameName=Starfield`, `modid=0`, `version=`, `comments=KitKraft dev
  build <id>`, `category="<id>,"` when `category` is given, and `notes=`. CRLF.
- **Redeploy:** an existing `meta.ini` (MO2 adds keys of its own) is carried over to the
  new live folder with only `version=` and `comments=` replaced in `[General]`, plus
  `category=` when one is given. Missing keys are added after `[General]`, and every other
  line is kept.
- **Rollback:** sets `version=` and `comments=` to the restored build's values.
- **`version`:** the argument, else the package manifest's `version`, else the date
  (`YYYY.M.D.HHMM`, UTC). The deployment manifest records it.
- **`category`:** a name MO2's `categories.dat` lists (`Gameplay`, `Ships`, ...),
  matched ignoring case. Unknown names refuse with rule `category`, and KitKraft never
  edits `categories.dat`.
- **MO2 running:** `deploy` warns that you should refresh MO2 with F5. Whether MO2 writes
  its cached `meta.ini` back when it exits is not observed yet.
- **Manifest:** `meta.ini` is never listed in the deployment manifest.

### `kitkraft-deploy.json`

The ownership marker and manifest, written last into the staged folder (a folder that has
it is complete):

```json
{
  "kind": "kitkraft-dev-mod",
  "format": 1,
  "mod": "KitKraft Dev - MyMod",
  "plugin": "MyMod.esm",
  "deployment_id": "20261014T221530Z-1a2b3c4d",
  "previous_deployment_id": "20261014T210200Z-9f8e7d6c",
  "deployed_at": "2026-10-14T22:15:30Z",
  "deployed_at_unix": 1792016130,
  "kitkraft_version": "0.0.0",
  "source": {
    "path": "C:\\Modding\\out\\MyMod",
    "kind": "folder",
    "package_manifest": "C:\\Modding\\out\\MyMod\\kitkraft-manifest.json",
    "package_manifest_sha256": "..."
  },
  "files": [
    {"path": "MyMod.esm", "size": 1234, "sha256": "...", "source": "C:\\Modding\\out\\MyMod\\MyMod.esm"}
  ]
}
```

A folder of the target name is written only when this file is present and its `kind` and
`mod` name this dev mod. Anything else of that name (case-insensitively) is refused.

`previous_deployment_id` is history: the deployment this one was **deployed over** (live
when it was written, or, with no live folder, the one `.previous` held). A later rollback
or redeploy does not rewrite it, so it need not name what `.previous` holds now. The
deployment a rollback restores is `.previous`'s own manifest id, which `deploy.status`
reports as `previous.manifest.deployment_id`.

## Algorithm

`deploy`:

1. Resolve the instance from `ModOrganizer.ini` (read-only): `mods\` (`mod_directory`,
   default `<base>\mods`), `profiles\`, `overwrite\`, `downloads\`, `selected_profile`,
   `gamePath` (a UTF-8 byte order mark is ignored). Refuse if MO2's active profile or
   `profile` is `Default`, and when the INI names no active profile unless `profile`
   names an existing profile other than `Default` (the gate fails closed).
2. Read and hash the source (SHA-256 of every file); verify it against
   `kitkraft-manifest.json` when there is one.
3. Recover a journal left by an interrupted call (below).
4. Check the slots: the live folder and `.previous` must be absent or KitKraft's; the live
   folder must match its manifest (a file edited by hand, missing, or not listed is
   reported and the deploy refused before any write). `.previous`, which a redeploy
   drops, must match its manifest too (`previous-modified`; MO2's `meta.ini` allowed).
   If the files are identical to the live deployment, stop: nothing is written.
5. Write the journal, create `.staging`, copy every file (hashing on the way, then reading
   the copy back and checking its SHA-256), carry `meta.ini` over, write the manifest.
6. Swap by renaming folders, all inside `mods\` (one volume): `.previous` -> `.retired`,
   live -> `.previous`, `.staging` -> live (the commit), then remove `.retired` and the
   journal. Renames are retried briefly when Windows reports a file in use.

On a failure in steps 5 and 6 the same recovery runs at once (automatic rollback), and the
error lists every path touched. `deploy.rollback` swaps `.previous` and the live folder the
same way (live -> `.staging`, `.previous` -> live, `.staging` -> `.previous`), so a rollback
can be rolled back. MO2's `meta.ini` follows the live folder: rollback copies it into the
new live folder after the swap, so `.previous` is never written before it is restored.
One previous deployment is kept.

### Journal recovery

The journal records the operation and the deployment ids live before and after. A journal
found at the start of `deploy` or `rollback` is recovered before anything else, and only
after the call has taken the lock:

- the new deployment is live (committed): finish the cleanup (`.retired`, `.staging`);
  `.retired` is removed only when it still matches its manifest (else `previous-modified`
  and the journal stays);
- otherwise: remove `.staging` (deploy), put the old live folder back from `.previous`
  (deploy) or `.staging` (rollback), and `.retired` back to `.previous`.

Recovery also removes our own temp names (`meta.ini.tmp`, `kitkraft-deploy.json.tmp`) left
at the root of the live folder or `.previous` by a crash mid-write, which would otherwise
read as unmanaged files. `deploy.status` and `--dry-run` only report a pending journal. `deploy.status` lists
every `KitKraft Dev - ...` entry; one it cannot read is reported with `state: "error"`
(and the reason) instead of failing the listing; the others carry `state: "ok"`. A `*.journal.json.tmp` left by
a crash while the journal itself was written is removed (nothing else had changed).

## Refusals

Every refusal names its rule and the path: `refused [rule] <path>: <why>`.

| Rule                  | When                                                                 |
| --------------------- | -------------------------------------------------------------------- |
| `dev-flag-required`   | `dev` is not `true` (the schema already rejects it as `invalid_args`) |
| `mo2-instance`        | `mo2` has no `ModOrganizer.ini`, or its `mods\` or `profiles\` folder is missing |
| `live-profile`        | MO2's active profile, or `profile`, is `Default` (any case)          |
| `live-profile-unknown`| `ModOrganizer.ini` names no active profile (`selected_profile` missing or empty) and `profile` does not name an existing profile other than `Default` |
| `profile-missing` / `profile-name` | `profile` is not a profile folder of the instance       |
| `reparse-point`       | A component of `mo2`, `mods\`, the target, or an entry in the source or a dev mod folder is a symbolic link or junction |
| `location`            | `mods\` is, or is inside, the game's Data folder or the user Data folder (`Documents\My Games\Starfield\Data`); `mods\` is the profiles, overwrite or downloads folder, or one of them is inside `mods\`; the instance is inside the game or user Data folder or another instance (`guard_location`, shared with `set` and `build`), or the source overlaps the dev mod's folders |
| `source`              | Not a plugin or a folder with one plugin at its root                 |
| `source-path`         | A name Windows forbids, `meta.ini` or `kitkraft-deploy.json` at the package root, a deployed path longer than 259 characters (measured under the longest slot, `<target>.previous\`, where every file lives after the next deploy, and for the longest internal temp names, `<target>.previous\kitkraft-deploy.json.tmp` and the journal's `.tmp`), too deep or more than 20,000 files |
| `plugin-name`         | The plugin stem is a name Windows forbids                            |
| `package-manifest`    | A file differs from `kitkraft-manifest.json`, is not listed, or a listed file is missing |
| `target-collision`    | The target, `.previous`, `.staging` or `.retired` exists and is not this dev mod's |
| `target-modified`     | The live folder differs from its manifest (modified, missing or unmanaged files) |
| `previous-modified`   | `.previous` (or `.retired`) differs from its manifest (MO2's `meta.ini` allowed) and the call would restore or drop it: `rollback`, a redeploy that replaces the rollback copy, or a journal recovery that removes `.retired` |
| `nothing-to-roll-back`| No live deployment, or no `.previous`                                |
| `rollback-target`     | `to` is not the id `.previous` holds                                 |
| `source-changed` / `copy-verify` | A file changed while being copied, or the copy's hash differs |
| `journal`             | A journal that cannot be read or names another mod                   |
| `locked`              | `<target>.kitkraft-lock` exists: another `deploy` or `rollback` is running on this dev mod, or a killed one left it (delete it once none runs) |
| `io`                  | A file system error (the message names the action)                   |

## Never touched

`deploy`, `deploy.rollback` and `deploy.status` never write: MO2 profiles and everything in them (`modlist.txt`, `plugins.txt`, `loadorder.txt`,
`archives.txt`, profile INIs), `ModOrganizer.ini`, `overwrite\`, `downloads\`, every other
folder under `mods\`, the game folder. Tests check the profiles, `ModOrganizer.ini`, an
unrelated mod, `overwrite\` and `downloads\` byte for byte before and after every
operation, and that every touched path is one of the dev mod's own.

`deploy` never enables the mod. Either run `kit deploy enable` (next section), or, after
the first deployment, follow the report's `next_steps` in MO2:

1. Switch to the `KitKraft Dev` profile (never `Default`, the live game).
2. Refresh MO2 (F5).
3. Enable `KitKraft Dev - <stem>` in the left pane, in the `KitKraft Dev` profile only
   (once; it stays enabled across redeploys).
4. In the Plugins tab, tick the plugin and drag it to the bottom so it loads last.
5. Leave `KitKraft Dev - <stem>.previous` disabled.
6. Test on a disposable save only.

The report also reads (never writes) the profile named by `profile`, else MO2's active
profile, else `KitKraft Dev` (never `Default`), and warns when the mod is not enabled or
the plugin is not active and last in its `plugins.txt`.

## Enabling in the dev profile: `deploy.enable` / `deploy.disable` (ADR 0009 Amendment 2)

The one explicit profile write. With MO2 and the game closed:

```powershell
kit deploy enable --dev C:\Modding\MO2-Starfield --mod "Minimum Enemy Level" --mod KitKraftMELDLC_TerranArmada --dry-run
kit deploy enable --dev C:\Modding\MO2-Starfield --mod "Minimum Enemy Level" --mod KitKraftMELDLC_TerranArmada
kit deploy disable --dev C:\Modding\MO2-Starfield --mod KitKraftMELDLC_TerranArmada
```

- **Profile:** `KitKraft Dev` only (any case: MO2's `Kitkraft Dev` folder counts).
  `Default` is refused by name (`live-profile`), any other profile with `not-dev-profile`.
- **Mods:** a `KitKraft Dev - <stem>` folder (or `<stem>`) must carry its
  `kitkraft-deploy.json` marker, and its plugin is the manifest's. Any other mod is named
  by its exact folder name, and its plugins are the `.esm`/`.esp`/`.esl` files at its root.
- **`enable`:** `-<mod>` becomes `+<mod>`, or `+<mod>` is added under the header comments.
  Each plugin moves last as `*<plugin>` in `plugins.txt` and last in `loadorder.txt`, in
  the order the mods are named.
- **`disable`:** `+<mod>` becomes `-<mod>`, and each plugin's `*` is dropped in place.
- **Unchanged:** every other byte of the three files (line endings, BOM, header, final
  newline), and every other file. A second identical call writes nothing (`unchanged`).
- **Backup:** `%LOCALAPPDATA%\kitkraft\profile-backups\<id>\` holds the three files as they
  were plus `backup.json` (paths and SHA-256). A failed write puts every file back.

| Rule | When |
| :--- | :--- |
| `dev-flag-required` | `dev: true` missing |
| `running` | MO2, the game, SFSE, the CK or MO2's helpers are running, or the process list cannot be read |
| `live-profile` / `not-dev-profile` / `profile-missing` | the profile is `Default`, another profile, or not found |
| `mod-missing` / `mod-name` / `rollback-copy` | no such folder under `mods\`, a forbidden name, a `.previous`/`.staging`/`.retired` copy |
| `not-kitkraft` | a `KitKraft Dev - ` folder without its marker |
| `journal-pending` | the dev mod has an interrupted deploy or rollback |
| `profile-format` | a list line without a `+`/`-`/`*` prefix, a mod listed twice, mixed line endings, not UTF-8 |
| `unmanaged-mod` | the mod's `modlist.txt` line is `*` (unmanaged) |
| `locked-order` | `lockedorder.txt` lists plugins |
| `backup-dir` / `backup` / `verify` | no backup folder, a backup copy that does not match, a written file that does not read back |

Whether MO2 keeps the edit when it next opens the profile is checked by the owner once and
recorded in `docs/game-observations.md`.

## Example: dry run against the owner's instance

Run manually on 2026-10-14 with a 15-byte stand-in plugin, read-only (`--dry-run` writes
nothing; `mods\` was checked afterwards). MO2's active profile is `Default`, so the real
call is refused:

```
> kit deploy --dev --dry-run ...\scratchpad\dryrun\KC_DevDemo.esm C:\Modding\MO2-Starfield --profile "KitKraft Dev"
deploy --dev --dry-run: REFUSED `KitKraft Dev - KC_DevDemo`
target: C:\Modding\MO2-Starfield\mods\KitKraft Dev - KC_DevDemo
files: 1 added, 0 replaced, 0 removed, 0 unchanged
refusal: refused [live-profile] C:\Modding\MO2-Starfield\ModOrganizer.ini: MO2's active profile is `Default`, the owner's live game; switch MO2 to the `KitKraft Dev` profile first
warning: mod `KitKraft Dev - KC_DevDemo` is not listed yet in profile `KitKraft Dev`
warning: plugin `KC_DevDemo.esm` is not active in profile `KitKraft Dev`'s plugins.txt
would touch (6):
  write C:\Modding\MO2-Starfield\mods\KitKraft Dev - KC_DevDemo.kitkraft-journal.json
  create-dir C:\Modding\MO2-Starfield\mods\KitKraft Dev - KC_DevDemo.staging
  write C:\Modding\MO2-Starfield\mods\KitKraft Dev - KC_DevDemo.staging\KC_DevDemo.esm
  write C:\Modding\MO2-Starfield\mods\KitKraft Dev - KC_DevDemo.staging\kitkraft-deploy.json
  rename C:\Modding\MO2-Starfield\mods\KitKraft Dev - KC_DevDemo.staging -> C:\Modding\MO2-Starfield\mods\KitKraft Dev - KC_DevDemo
  remove-file C:\Modding\MO2-Starfield\mods\KitKraft Dev - KC_DevDemo.kitkraft-journal.json
kit: `deploy` check failed
```

(exit code 1). `kit deploy status C:\Modding\MO2-Starfield` on the same instance:

```
deploy status: 0 KitKraft dev mod(s) in C:\Modding\MO2-Starfield\mods
warning: MO2's active profile is `Default` (the live game): deploy and rollback refuse until MO2 is switched to `KitKraft Dev`
touched (0):
```

## Where this differs from ADR 0009's text

The owner's brief for M4.E11 is the spec where the two differ; ADR 0009 Amendment 1
accepts these departures:

- **Rollback copy inside `mods\`.** The brief keeps the previous deployment as
  `<target>.previous` beside the live folder and stages in a temp folder inside `mods\`,
  so every swap is a same-volume folder rename. ADR 0009 described per-file temp + rename
  with backups in a KitKraft state folder outside MO2, keeping three. One previous
  deployment is kept; `to` only confirms which.
- **Journal beside the folders** (`<target>.kitkraft-journal.json` in `mods\`, a file, so
  MO2 never lists it as a mod), not in a state folder, so recovery needs nothing but `mo2`.
- **`dry_run`** (`--dry-run`, the brief's word) instead of ADR 0009's `plan`.
- **Names.** `kitkraft-deploy.json` and the `kitkraft-dev-mod` marker used the final
  product name from the start; the package manifest it reads is `kitkraft-manifest.json`
  (renamed with the identifiers; a package made earlier with `kitcraft-manifest.json` is
  still verified when the new name is absent, deprecated for one release).
- **Unmanaged files** in the live folder (other than MO2's `meta.ini`) refuse the deploy
  instead of being left in place, because the folder swap would otherwise move them into
  `.previous` and later drop them.
- **Refusing `Default`.** Both MO2's active profile (`selected_profile`) and the
  `profile` argument are checked (AGENTS.md, 2026-10-14).

## Limits and open points

- The instance's settings (`ModOrganizer.ini`: the `mods`, `profiles`, `overwrite` and
  `downloads` folders, `gamePath`, `selected_profile` with `@ByteArray(...)` decoded) are
  read by `kitkraft-io`'s read-only `Instance` (ADR 0009), the same reading the profile
  reader uses. The write-side checks (links and junctions refused, guarded folders, the
  `Default` gate) stay in the engine (`deploy/instance.rs`).
- The package manifest is read tolerantly: a `files` array of `{path, sha256, size?}` is
  verified; another shape is named in `limitations` and the files are hashed as found.
  Paired with `package` (M4.E5) in M4.BX1: an end-to-end run on a synthetic MO2 instance
  (`set` rename -> `package` -> `deploy --dev` -> redeploy -> `deploy.rollback` ->
  `deploy.status`) verified the manifest with no limitation, restored the first
  deployment's plugin byte for byte and left the profiles and `ModOrganizer.ini`
  untouched. One mismatch was fixed: `package` writes `plugins` as objects naming the
  plugin in `file`, which `deploy` now reads (`package_command_manifest_names_the_plugin`).
- Whether MO2 or the game holds files open is not detected up front; a rename that fails
  with a file in use stops the deployment and rolls it back. Refresh MO2 after each
  deployment.
- One writer at a time: `deploy` and `rollback` create `<target>.kitkraft-lock` with
  `create_new` before journal recovery and remove it when they return, so a second call
  on the same dev mod is refused (`locked`). A process killed mid-run leaves the lock;
  `deploy.status` reports it, and the owner deletes it once no `kit` runs (the journal is
  then recovered by the next call). `--dry-run` and `deploy.status` take no lock.
