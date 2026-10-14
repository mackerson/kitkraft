# Development deployment (`kit deploy --dev`)

M4.E11, ADR 0009 (owner ruling 2026-10-14). `package` stays target-independent and never
writes into MO2. `deploy` is a separate, explicitly invoked development deployment into
one isolated, KitKraft-managed MO2 mod folder. It is the only place KitKraft writes inside
a Mod Organizer instance. Code: `crates/engine/src/deploy/`, handler
`crates/engine/src/handlers/deploy.rs`.

## Commands

| Command           | CLI                                                    | Writes |
| ----------------- | ------------------------------------------------------ | ------ |
| `deploy`          | `kit deploy --dev <source> <mo2> [--profile P] [--dry-run]` | yes (one dev mod) |
| `deploy.rollback` | `kit deploy rollback --dev <mo2> <mod> [--to ID] [--profile P] [--dry-run]` | yes (one dev mod) |
| `deploy.status`   | `kit deploy status <mo2> [--mod M] [--profile P]`      | no     |

Arguments (JSON, same over MCP `execute` / `command_line`, which carry `destructiveHint`):

- `source`: a plugin (`.esm`, `.esp`, `.esl`) or a package folder (the plugin at the root,
  loose files and `.ba2` archives at their Data paths). A folder's
  `kitcraft-manifest.json` (written by `package`, M4.E5) is verified and not deployed.
- `mo2`: the MO2 instance root (the folder holding `ModOrganizer.ini`). Required on every
  call; no configuration file, environment variable or session supplies it.
- `dev`: required and must be `true` (`--dev`). The schema admits only `true`, and the
  handler checks it again.
- `profile`: the MO2 profile to check (read-only diagnostics). Never `Default`.
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

The folder name is `KitKraft Dev - ` plus the plugin's file stem. A stem Windows forbids
(a reserved device name such as `CON`, a character in `<>:"/\|?*`, a trailing dot or
space) is refused, never rewritten. A package with several plugins at its root needs a
package manifest whose `plugins` list names the first; one plugin per dev mod.

MO2 lists every folder under `mods\`, so after a redeploy it shows
`KitKraft Dev - MyMod.previous` as a separate mod. Leave it disabled. MO2 writes its own
`meta.ini` into a mod folder; `deploy` and `rollback` carry it over to the new live
folder and never list it in the manifest.

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
    "package_manifest": "C:\\Modding\\out\\MyMod\\kitcraft-manifest.json",
    "package_manifest_sha256": "..."
  },
  "files": [
    {"path": "MyMod.esm", "size": 1234, "sha256": "...", "source": "C:\\Modding\\out\\MyMod\\MyMod.esm"}
  ]
}
```

A folder of the target name is written only when this file is present and its `kind` and
`mod` name this dev mod. Anything else of that name (case-insensitively) is refused.

## Algorithm

`deploy`:

1. Resolve the instance from `ModOrganizer.ini` (read-only): `mods\` (`mod_directory`,
   default `<base>\mods`), `profiles\`, `overwrite\`, `downloads\`, `selected_profile`,
   `gamePath`. Refuse if MO2's active profile or `profile` is `Default`.
2. Read and hash the source (SHA-256 of every file); verify it against
   `kitcraft-manifest.json` when there is one.
3. Recover a journal left by an interrupted call (below).
4. Check the slots: the live folder and `.previous` must be absent or KitKraft's; the live
   folder must match its manifest (a file edited by hand, missing, or not listed is
   reported and the deploy refused before any write). If the files are identical to the
   live deployment, stop: nothing is written.
5. Write the journal, create `.staging`, copy every file (hashing on the way, then reading
   the copy back and checking its SHA-256), carry `meta.ini` over, write the manifest.
6. Swap by renaming folders, all inside `mods\` (one volume): `.previous` -> `.retired`,
   live -> `.previous`, `.staging` -> live (the commit), then remove `.retired` and the
   journal. Renames are retried briefly when Windows reports a file in use.

On a failure in steps 5 and 6 the same recovery runs at once (automatic rollback), and the
error lists every path touched. `deploy.rollback` swaps `.previous` and the live folder the
same way (live -> `.staging`, `.previous` -> live, `.staging` -> `.previous`), so a rollback
can be rolled back. One previous deployment is kept.

### Journal recovery

The journal records the operation and the deployment ids live before and after. A journal
found at the start of `deploy` or `rollback` is recovered before anything else:

- the new deployment is live (committed): finish the cleanup (`.retired`, `.staging`);
- otherwise: remove `.staging` (deploy), put the old live folder back from `.previous`
  (deploy) or `.staging` (rollback), and `.retired` back to `.previous`.

`deploy.status` and `--dry-run` only report a pending journal. A `*.journal.json.tmp` left by
a crash while the journal itself was written is removed (nothing else had changed).

## Refusals

Every refusal names its rule and the path: `refused [rule] <path>: <why>`.

| Rule                  | When                                                                 |
| --------------------- | -------------------------------------------------------------------- |
| `dev-flag-required`   | `dev` is not `true` (the schema already rejects it as `invalid_args`) |
| `mo2-instance`        | `mo2` has no `ModOrganizer.ini`, or its `mods\` or `profiles\` folder is missing |
| `live-profile`        | MO2's active profile, or `profile`, is `Default` (any case)          |
| `profile-missing` / `profile-name` | `profile` is not a profile folder of the instance       |
| `reparse-point`       | A component of `mo2`, `mods\`, the target, or an entry in the source or a dev mod folder is a symbolic link or junction |
| `location`            | `mods\` is inside the game's Data folder (or one of profiles, overwrite, downloads is inside `mods\`), the instance is inside the game or user Data folder or another instance (`guard_location`, shared with `set` and `build`), or the source overlaps the dev mod's folders |
| `source`              | Not a plugin or a folder with one plugin at its root                 |
| `source-path`         | A name Windows forbids, `meta.ini` or `kitkraft-deploy.json` at the package root, a deployed path longer than 259 characters, too deep or too many files |
| `plugin-name`         | The plugin stem is a name Windows forbids                            |
| `package-manifest`    | A file differs from `kitcraft-manifest.json`, is not listed, or a listed file is missing |
| `target-collision`    | The target, `.previous`, `.staging` or `.retired` exists and is not this dev mod's |
| `target-modified`     | The live folder differs from its manifest (modified, missing or unmanaged files) |
| `previous-modified`   | `rollback`: `.previous` differs from its manifest                    |
| `nothing-to-roll-back`| No live deployment, or no `.previous`                                |
| `rollback-target`     | `to` is not the id `.previous` holds                                 |
| `source-changed` / `copy-verify` | A file changed while being copied, or the copy's hash differs |
| `journal`             | A journal that cannot be read or names another mod                   |
| `io`                  | A file system error (the message names the action)                   |

## Never touched

MO2 profiles and everything in them (`modlist.txt`, `plugins.txt`, `loadorder.txt`,
`archives.txt`, profile INIs), `ModOrganizer.ini`, `overwrite\`, `downloads\`, every other
folder under `mods\`, the game folder. Tests check the profiles, `ModOrganizer.ini`, an
unrelated mod, `overwrite\` and `downloads\` byte for byte before and after every
operation, and that every touched path is one of the dev mod's own.

`deploy` never enables the mod. After the first deployment, the report's `next_steps` tell
the owner what to do in MO2:

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
- **Names.** `kitkraft-deploy.json` and the `kitkraft-dev-mod` marker use the final
  product name now (new identifiers, nothing to rename later); the package manifest it
  reads is still `kitcraft-manifest.json`.
- **Unmanaged files** in the live folder (other than MO2's `meta.ini`) refuse the deploy
  instead of being left in place, because the folder swap would otherwise move them into
  `.previous` and later drop them.
- **Refusing `Default`.** Both MO2's active profile (`selected_profile`) and the
  `profile` argument are checked (AGENTS.md, 2026-10-14).

## Limits and open points

- The MO2 instance check lives in the engine (`deploy/instance.rs`, using `kitcraft-io`'s
  INI and profile readers). ADR 0009 puts it in `crates/io` (lane D); a request is in
  `plan/STATUS.md`.
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
- One writer at a time: two `deploy` calls on the same dev mod at once are not locked
  against each other beyond the journal.
