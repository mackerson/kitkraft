# KitKraft quick start

For the owner, on the dev box. No environment variables, no `--data-dir`: KitKraft finds
the game through Steam and MO2 at `C:\Modding\MO2-Starfield` (see "How KitKraft finds
things" below). The reference for every command is `kit help <command>`; the rest of
`docs/` is for agents and contributors.

## 1. Build the folder

Once, and again after pulling changes:

```text
cargo xtask dist
```

This release-builds `kit.exe` and `kit-ui.exe` and writes
`C:\Modding\kitcraft\dist\KitKraft\` (`C:\Modding\kitkraft\dist\KitKraft\` after the
folder move): the two programs, the double-click files below and a `README.txt`.
`--out <dir>` writes somewhere else. The `dist\` folder is not committed.

Rebuilding works while Claude Code or the workbench runs. Windows does not let a running
`kit.exe` (the MCP server is one) be overwritten, but lets it be renamed, so the build
renames it aside to `kit.exe.stale-<unix-seconds>` in the same folder, copies the new
build in, and prints one line per swap and how many stale copies are left (the same for
`kit-ui.exe`). No process is stopped. After rebuilding, restart Claude Code or the
workbench to pick up the new build; stale copies are cleaned on the next run.

`--sync-bin` also refreshes `bin\kit.exe` and `bin\kit-ui.exe` in the main checkout
(`C:\Modding\kitcraft\bin\`, gitignored; the path older docs and the pre-rename MCP
registration use), with the same rename rule. It is opt-in so that a build from a lane
worktree never replaces those programs unasked.

| File | What it does |
| ---- | ------------ |
| `KitKraft Workbench.cmd` | Opens the read-only workbench on the KitKraft Dev profile (the profile picker when there is none) |
| `KitKraft Shell.cmd` | A PowerShell window with `kit` on PATH, starting in the repository; runs `kit doctor` first |
| `Validate Plugin.cmd` | Drag plugins onto it: `kit validate --plugin <file>` for each, then waits so you can read the result |
| `Load Dev Profile.cmd` | Runs `kit load --profile "KitKraft Dev" --from-save newest` and shows the result |
| `Deploy to Dev.cmd` | Drag a plugin or a packaged folder onto it: dry run, asks y/n, then `kit deploy --dev` into its `KitKraft Dev - <plugin>` mod |
| `Install.cmd` / `Uninstall.cmd` | PATH and MCP registration (section 2) |
| `Finish Move.cmd` | Renames `C:\Modding\kitcraft` to `C:\Modding\kitkraft` (section 8) |

Every file quotes its paths, so the folder may have spaces in its name, and every one
waits for a key press before closing when something went wrong.

## 2. Install

1. Close every Claude Code session, the Claude app included (Claude Code keeps
   `.claude.json` in memory and writes it back, which would drop the new entry).
   `Install.cmd` checks first: while a `claude.exe` process runs (the Claude desktop app
   and the native `claude` CLI both run as `claude.exe`) it stops with a one-line message
   and changes nothing. `kit install --apply` refuses the same way whenever the plan
   rewrites `.claude.json`; `--force-while-running` skips the check. Nothing is stopped
   or killed. A CLI installed through npm runs as `node.exe` and is not detected.
2. Double-click `Install.cmd`. It prints every change first and asks `y`/`n`:
   - adds the `dist\KitKraft` folder to your user PATH (the old value is printed and
     saved in `dist\KitKraft\backups\`);
   - registers `kit.exe` as the MCP server `kitkraft` in `.claude.json` in your user
     folder, replacing the pre-rename `kitcraft` entry (its arguments are kept; a backup
     copy `.claude.json.kitkraft-backup-<time>` is written first);
   - writes the same entry into the repository's `.mcp.json` (gitignored);
   - makes the server preload the **KitKraft Dev** profile (`--profile`, read-only):
     the profile `kit doctor` reports as `dev_profile` (folder `Kitkraft Dev` on the dev
     box, name matched ignoring case). An entry registered with another profile, such as
     `Default` from an earlier install, is re-pointed, and the plan shows it as a diff of
     the entry (`- args: ...` / `+ args: ...`). Without a KitKraft Dev profile it falls
     back to MO2's active profile and prints a `limitation:` line saying which and why.
     `kit install --profile <name-or-path>` picks another one;
   - records what it replaced in `dist\KitKraft\install-state.json`, for `Uninstall.cmd`.

   What Install.cmd changes in `.claude.json` besides the entry: the file is parsed and
   written back whole, so its keys come out sorted and numbers in a normalized spelling
   (for example `1e3` becomes `1000.0`); every value is kept. The backup copy keeps the
   original bytes.
3. Open a new terminal (`kit help` works anywhere) and start Claude Code again. `/mcp`
   lists `kitkraft`.

Running it again changes nothing. From a terminal: `kit install` (a dry run) and
`kit install --apply`. What it would change on the dev box today is in
`plan/STATUS.md` ("Install dry run").

## 3. Check the setup

Double-click `KitKraft Shell.cmd`, or run `kit doctor`. Each line names what was found,
the exact path, how it was found, and ends with `OK` or a one-line fix. On the dev box
(2026-10-15, from `dist\KitKraft\kit.exe`, before `Install.cmd`):

```text
KitKraft (kit)         C:\Modding\kitcraft\dist\KitKraft\kit.exe [this program, version 0.0.0, sha256 f35024118acc]  OK
Game install           C:\Program Files (x86)\Steam\steamapps\common\Starfield [Steam registry HKCU\Software\Valve\Steam SteamPath + libraryfolders.vdf (lists app 1716740)]  OK
Data dir               C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data [the Data folder of the game install]  OK
Game version           1.16.244.0 [file version of C:\Program Files (x86)\Steam\steamapps\common\Starfield\Starfield.exe]  OK
MO2 instance           C:\Modding\MO2-Starfield [default location]  OK
MO2 profiles           Default, KitKraft Stack, Kitkraft Dev (active) [folders with modlist.txt in C:\Modding\MO2-Starfield\profiles; active = selected_profile in C:\Modding\MO2-Starfield\ModOrganizer.ini]  OK
KitKraft Dev profile   C:\Modding\MO2-Starfield\profiles\Kitkraft Dev [profile `Kitkraft Dev` (name matched ignoring case)]  OK
xEdit (xSFEdit64)      C:\Modding\tools\xEdit\xSFEdit64.exe [default tool folder C:\Modding\tools\xEdit]  OK
xDump64                C:\Modding\tools\xEdit\xDump64.exe [default tool folder C:\Modding\tools\xEdit]  OK
Papyrus compiler       C:\Program Files (x86)\Steam\steamapps\common\Starfield\Tools\Papyrus Compiler\PapyrusCompiler.exe [in the game install folder]  OK
ContentResources.zip   C:\Program Files (x86)\Steam\steamapps\common\Starfield\Tools\ContentResources.zip [in the game install folder]  OK
SFSE                   C:\Program Files (x86)\Steam\steamapps\common\Starfield\sfse_loader.exe (for game 1.16.244) [sfse_loader.exe and sfse_<version>.dll in the game folder]  OK
Index cache            C:\Users\macke\AppData\Local\kitkraft\cache (84 files, 246 MiB) [default %LOCALAPPDATA%\kitkraft\cache]  OK
MCP (user)             kitcraft -> C:/Modding/kitcraft/bin/kit.exe (exists, a different build) [mcpServers.kitcraft in C:\Users\macke\.claude.json]  FIX: pre-rename name; run Install.cmd to register it as `kitkraft`
MCP (project)          kitcraft -> C:/Modding/kitcraft/bin/kit.exe (exists, a different build) [mcpServers.kitcraft in C:\Modding\kitcraft\.mcp.json]  FIX: pre-rename name; run Install.cmd to register it as `kitkraft`

Everything KitKraft needs was found. Optional items marked FIX are only needed for what they name.
Defaults: --data-dir and profile names fall back to what is detected here (argument > environment > detected).
limitation: registry values are read through reg.exe; a path with characters outside the console code page may read wrong
```

The two `MCP` lines are what `Install.cmd` fixes. Each also names the profile its
registration preloads (`profile <dir>`, or `no --profile`); one preloading `Default`
gets `FIX: live game profile; re-run Install.cmd to use KitKraft Dev`. `kit doctor --get <what>` prints one
path alone (`game_dir`, `data_dir`, `mo2`, `active_profile`, `dev_profile`, `cache_dir`,
`kit`), which is how the `.cmd` files ask for the MO2 folder.

## 4. Open the workbench

Double-click `KitKraft Workbench.cmd` (or run `kit ui --dev`). It opens the KitKraft Dev
profile when doctor finds one. Keep the console window open; closing it closes the
workbench. What the workbench shows is in `docs/workbench.md`.

## 5. Run a QA check

Drag a plugin onto `Validate Plugin.cmd`. From a terminal:

```text
kit validate --plugin out\overhaul\KitKraftGunOverhaul.esm
```

Masters (`Starfield.esm` and the rest) are found in the game's Data folder without
`--data-dir`. Result on the gun overhaul (build 2), through `Validate Plugin.cmd`:

```text
=== Validating "KitKraftGunOverhaul.esm"
KitKraftGunOverhaul.esm: passed (0 error(s), 0 warning(s); rules: masters.missing, masters.order, formid.dangling, lstring.copied-id, override.stripped-name, cell.missing-parent, ref.orphan, ref.outside-block, ref.persistent-flag, ref.persistence, cell.actor-without-navmesh, cell.incc, navmesh.unchanged, record.flags-body, papyrus.missing-script, papyrus.property-unresolved)
limitation: formid.dangling checked 1772 FormIDs; 77 subrecords are opaque or undecoded (FormIDs inside them unchecked) and 0 FormIDs could not be resolved without guessing a master's scale

Result: PASSED - no problems found.
Press any key to continue . . .
```

The rules are listed in `docs/validate.md`.

## 6. Deploy to the dev profile

Only into the **KitKraft Dev** profile, never `Default` (your playthrough). Package first
(`kit package ...`, `docs/packaging.md`), then drag the package folder (or a plugin)
onto `Deploy to Dev.cmd`:

1. it runs `kit deploy <source> <MO2> --dev --dry-run --profile "KitKraft Dev"` and
   shows what would be written;
2. it asks `y`/`n`;
3. on `y` it deploys into `C:\Modding\MO2-Starfield\mods\KitKraft Dev - <plugin>\`.

The first time, enable that mod and its plugin in MO2's KitKraft Dev profile, or close
MO2 and run `kit deploy enable --dev <MO2> --mod <plugin stem>`. Deploy itself never
edits a profile or another mod (ADR 0009, `docs/deploy.md`); rollback is
`kit deploy rollback --dev <MO2> <mod>`.

`Load Dev Profile.cmd` then reads the profile and its newest save, so the runtime IDs
(`prid ...` in the console) are confirmed.

## 7. Where things live

| What | Where |
| ---- | ----- |
| Programs and double-click files | `C:\Modding\kitcraft\dist\KitKraft\` (rebuilt by `cargo xtask dist`) |
| Install record and backups | `dist\KitKraft\install-state.json`, `dist\KitKraft\backups\`, and `.claude.json.kitkraft-backup-<time>` next to `.claude.json` |
| Index cache | `%LOCALAPPDATA%\kitkraft\cache` (safe to delete; rebuilt on the next load). `kit doctor` shows its size |
| MCP registration | `%USERPROFILE%\.claude.json` (`mcpServers.kitkraft`) and the repository's `.mcp.json` |
| Built plugins and packages | `out\` in the repository (`out\overhaul\`, `out\poi\`) |
| Dev deployments | `C:\Modding\MO2-Starfield\mods\KitKraft Dev - <plugin>\` |

## 8. Finish the folder move

Once: run it only after closing every Claude Code session (the Claude app included),
then double-click `Finish Move.cmd`. It warns,
asks you to type `YES`, then runs `C:\Modding\scripts\finish_kitkraft_move.ps1` (rename,
MCP paths, `git worktree prune`). When `Install.cmd` was run before, it then re-runs
`kit install --apply` from the new folder so the PATH entry follows. Open new sessions on
`C:\Modding\kitkraft`.

## 9. Uninstall

Double-click `Uninstall.cmd` (dry run first, then `y`). It takes the folder off the PATH,
removes the `kitkraft` entries that point into it, restores the entries install replaced
(the pre-rename `kitcraft` one), and deletes the files install created. The `dist` folder
itself stays; delete it if you like.

## How KitKraft finds things

`kit doctor` shows all of it. Every command uses the same answers:

| Item | Looked for, in order |
| ---- | -------------------- |
| Game install | Steam's registry (`HKCU\Software\Valve\Steam` `SteamPath`), its `libraryfolders.vdf` (the library listing Starfield, app 1716740), then the common Steam and Xbox folders |
| Data dir | `<game>\Data`, else the MO2 instance's `gamePath` |
| MO2 instance | `KITKRAFT_MO2_DIR`, then `C:\Modding\MO2-Starfield`, then MO2's registry (`CurrentInstance`) and global instances in `%LOCALAPPDATA%\ModOrganizer` (the same order for `kit doctor`, the workbench's start screen, profile names and `kit install`) |
| Profiles | folders with `modlist.txt` in the instance's profiles folder; active = MO2's `selected_profile` |
| xEdit, xDump64 | `KITKRAFT_SF1EDIT`, MO2's executables list, `C:\Modding\tools\xEdit` |
| Papyrus compiler, `ContentResources.zip` | the game's `Tools` folder (Creation Kit) |

Precedence everywhere: **argument > environment variable > detected**. So
`--data-dir` and `KITKRAFT_DATA_DIR` still work and win, and a bare `--profile` name
(`--profile "KitKraft Dev"`) is that profile of the detected MO2 instance.
`KITKRAFT_DETECT=0` turns the detected defaults off (`kit install`'s default MCP
arguments included; `kit doctor` and `profiles` still detect). The variables are in
`docs/corpus.md` ("Environment").
