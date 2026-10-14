# Packaging a mod (`kit package`)

`package` turns a plugin KitCraft built (plus optional loose files and compiled scripts)
into a folder Mod Organizer 2 can install, a `.zip` of it, and a manifest that proves what
is inside. It never installs anything: it writes only under the `out` folder you name, and
refuses an `out` inside the game's `Data` folder, `Documents\My Games\Starfield`, or a Mod
Organizer instance or profile (`C:\Modding\MO2-Starfield` and everything under it,
`mods\` included). Installing is your step, in MO2 (owner decision M4.O5, default).

## What it does

1. Runs `validate` on the plugin (and on the scripts given). A plugin with any error is
   refused and nothing is written. Pass the same `--profile`, `--plugins` or `--data-dir`
   you would pass to `validate` so its masters are found (`KITCRAFT_DATA_DIR` is used by
   default).
2. Builds `<out>\<name>\` (default name: the plugin's file name without extension):
   - the plugin at the root;
   - `--loose <folder>`: a folder laid out like `Data` (`Meshes\...`, `Textures\...`,
     `Interface\...`), copied at the same paths;
   - `--scripts <file or folder>` (repeatable): compiled `.pex` files, placed at
     `Scripts\<namespace>\<name>.pex` from the script name inside each file;
   - with `--ba2`, loose files and scripts are packed instead: `<plugin> - Main.ba2`
     (everything but textures) and `<plugin> - Textures.ba2` (the `.dds` files), the names
     the official plugins' archives use (`docs/corpus.md`, "Archive names"). Version 2
     (zlib) by default, as the official plugins' archives; `--ba2-version 3` for LZ4.
3. Writes `kitcraft-manifest.json` at the root of the folder: every file with its size and
   SHA-256, the plugin's masters and record counts per type, the KitCraft version, the
   SHA-256 of the build spec (`--spec <file.toml>`), the `validate` result, and the files
   inside each archive with their SHA-256. No dates and no absolute paths: the same inputs
   always give the same bytes.
4. Writes `<out>\<name>.zip` (or `<name>-<version>.zip` with `--version`), the folder's
   contents at the zip's root; `--no-zip` skips it.

An existing `<out>\<name>` is replaced only with `--overwrite`, and only if it holds a
`kitcraft-manifest.json` (a folder `package` made).

Example (the rename smoke test, M2.O6):

```
kit package C:\Modding\kitcraft\out\OldSovietMarksmanRifle.esm C:\Modding\kitcraft\out\packages --version 0.1.0
```

## Install it in MO2 (owner)

Use the `KitKraft Dev` profile and a disposable save (AGENTS.md: the `Default` profile and
the live playthrough are off limits).

1. In MO2, switch to the `KitKraft Dev` profile.
2. *Install a new mod from an archive* (the box icon) and pick `<name>-<version>.zip`.
   MO2 proposes the mod name; keep it or type `<name>`. The zip's root is the Data root,
   so MO2 should not ask for a data folder; if it does, the layout is wrong: stop and
   report it.
   Alternative without the zip: right-click the mod list > *Create empty mod* > `<name>`,
   then copy the contents of `<out>\<name>\` into the new mod's folder.
3. Enable the mod; in the right pane enable its plugin and place it after its masters
   (for a patch: after the plugin that wins the records it changes).
4. Check the install against the manifest (below), then launch through MO2.
5. To remove it: disable the mod (a plugin that adds forms is never removed from a save in
   progress; use a disposable save).

## Check the installed files against the manifest

In PowerShell, from the installed mod folder (`C:\Modding\MO2-Starfield\mods\<name>`; this
only reads):

```
$m = Get-Content .\kitcraft-manifest.json -Raw | ConvertFrom-Json
foreach ($f in $m.files) {
  $h = (Get-FileHash -Algorithm SHA256 -LiteralPath $f.path).Hash.ToLower()
  "{0}  {1}" -f ($(if ($h -eq $f.sha256) { 'ok  ' } else { 'DIFF' })), $f.path
}
```

Every line should read `ok`. Record the result (and the zip's SHA-256 from `kit package`'s
JSON output) in `docs/game-observations.md` with the in-game observation (M4.O2).

## Checked so far

- 2026-10-14: the rename plugin packaged; the manifest's plugin SHA-256 equals the one in
  `docs/smoke-rename.md` (`9C0B28A6...4078EA27`).
- 2026-10-14, `black-box: Archive2.exe 1.2.0.1`: a `--ba2` package's `- Main.ba2` and
  `- Textures.ba2` open and extract in Archive2 with the same file list; the general files
  are byte-identical, the texture's pixels are identical and its header is Archive2's own
  128-byte form (a DDS header is not stored in a DX10 archive; `ba2.pack`'s limitation).
- Not yet: the in-game check that the game loads a mod's `<plugin> - Main.ba2` (M4.O2).
