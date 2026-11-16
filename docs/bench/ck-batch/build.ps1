# Builds the CK bench batch (docs/bench/ck-sitting.md, "The batch") from the recipes in this
# folder: one plugin per group of groups.txt (the bodies of types/<TYPE>.toml joined), one per
# type (the fallback when a group plugin fails in the CK), one per fixture recipe in fixtures/.
# Every plugin must pass `kit validate` with 0 errors and `kit roundtrip --verify`, or the
# build fails. Writes <Out>\manifest.json (what ck-batch.ps1 runs), manifest.md and
# deploy.txt (the MO2 commands for the bench, each with --dry-run first).
#
#   $env:KITKRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
#   powershell -File docs\bench\ck-batch\build.ps1 -Kit <kit.exe> -Out C:\Modding\kitkraft-observe\ck-2026-11-16\plugins
#
# Writes only under -Out (never MO2, the game or a Data folder; `kit build` refuses those too).
param(
    [string]$Kit = $(if ($env:KITKRAFT_KIT) { $env:KITKRAFT_KIT } else { 'kit' }),
    [Parameter(Mandatory)][string]$Out,
    [string]$Mo2 = 'C:\Modding\MO2-Starfield',
    [string]$Profile = 'Kitkraft Dev'
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
if (-not $env:KITKRAFT_DATA_DIR) { throw 'set KITKRAFT_DATA_DIR to the game Data folder (read only)' }
$full = [IO.Path]::GetFullPath($Out)
foreach ($bad in @($env:KITKRAFT_DATA_DIR, $Mo2)) {
    if ($full.StartsWith([IO.Path]::GetFullPath($bad).TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw "refused: -Out is under $bad" }
}
New-Item -ItemType Directory -Force $full, (Join-Path $full 'specs'), (Join-Path $full 'types'), (Join-Path $full 'fixtures') | Out-Null
$utf8 = New-Object System.Text.UTF8Encoding $false

function Invoke-Kit([string[]]$KitArgs) {
    $prev = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    try { $o = & $Kit @KitArgs --json 2>$null | Out-String } finally { $ErrorActionPreference = $prev }
    $j = $o | ConvertFrom-Json
    if ($j.error) { throw "kit $($KitArgs[0]) failed: $($j.error.message)" }
    $j
}
function Sha([string]$p) { (Get-FileHash -Algorithm SHA256 -LiteralPath $p).Hash.ToLowerInvariant() }
function Get-Body([string]$Type) {
    $t = Get-Content -Raw -LiteralPath (Join-Path $here "types\$Type.toml")
    $i = $t.IndexOf('# --- body ---')
    if ($i -lt 0) { throw "types\$Type.toml has no '# --- body ---' line" }
    "# ---- $Type (types/$Type.toml) ----`r`n" + $t.Substring($i + '# --- body ---'.Length).Trim() + "`r`n"
}

# One plugin: build, validate, roundtrip; its manifest entry.
function Build-One([string]$Spec, [string]$File, [string]$Kind, [string]$Group, [string[]]$Types) {
    $b = Invoke-Kit @('build', $Spec, $File, '--overwrite')
    if (-not $b.data.passed) { throw "$File did not validate in build" }
    $v = Invoke-Kit @('validate', '--plugin', $File, '--plugins', 'Starfield.esm')
    $r = Invoke-Kit @('roundtrip', $File, '--verify')
    $recs = @($b.data.records | ForEach-Object { [pscustomobject]@{ signature = $_.signature; form = $_.form; editor_id = $_.editor_id; source = $_.source; outcome = $_.outcome } })
    # [[place]] references are in data.records already; data.placed only adds any it lacks.
    $recs += @($b.data.placed | Where-Object { $recs.form -notcontains $_.form } | ForEach-Object { [pscustomobject]@{ signature = $_.signature; form = $_.form; editor_id = $_.editor_id; source = 'place'; outcome = 'placed' } })
    $e = [pscustomobject]@{
        file = [IO.Path]::GetFileName($File)
        path = $File.Substring($full.Length + 1)
        kind = $Kind; group = $Group; types = $Types
        editor_ids = @($recs | Where-Object { $_.editor_id } | ForEach-Object editor_id | Select-Object -Unique)
        records = $recs
        bytes = (Get-Item -LiteralPath $File).Length
        sha256 = (Sha $File)
        flags = $b.data.flags
        validate_errors = $v.data.errors; validate_warnings = $v.data.warnings
        roundtrip_identical = [bool]$r.data.identical
    }
    $ok = ($e.validate_errors -eq 0) -and $e.roundtrip_identical
    Write-Host ('{0,-24} {1,6} bytes  validate {2} error(s) {3} warning(s)  roundtrip {4}  {5}' -f $e.file, $e.bytes, $e.validate_errors, $e.validate_warnings, $(if ($e.roundtrip_identical) { 'identical' } else { 'DIFFERS' }), ($Types -join ' '))
    if (-not $ok) { throw "$($e.file): validate errors $($e.validate_errors), roundtrip identical $($e.roundtrip_identical)" }
    if ($e.flags -ne '0x00000000') { throw "$($e.file): TES4 flags $($e.flags); the CK takes only a non-master file as its active file" }
    $e
}

$entries = @()
$header = "version = 3`r`n`r`n[load_order]`r`nplugins = [`"Starfield.esm`"]`r`n`r`n"
$groups = @(Get-Content -LiteralPath (Join-Path $here 'groups.txt') | Where-Object { $_ -match '^\s*KCB_\w+\s*:' })
$seen = @{}
foreach ($line in $groups) {
    $stem, $rest = $line -split ':', 2
    $stem = $stem.Trim(); $types = @($rest.Trim() -split '\s+' | Where-Object { $_ })
    foreach ($t in $types) { if ($seen.ContainsKey($t)) { throw "type $t is in two groups" }; $seen[$t] = $stem }
    $spec = Join-Path $full "specs\$stem.toml"
    $text = "# Generated by docs/bench/ck-batch/build.ps1 from groups.txt and types/*.toml; do not edit.`r`n" + $header + (($types | ForEach-Object { Get-Body $_ }) -join "`r`n")
    [IO.File]::WriteAllText($spec, $text, $utf8)
    $entries += Build-One $spec (Join-Path $full "$stem.esp") 'group' $stem $types
}
$all = @(Get-ChildItem -LiteralPath (Join-Path $here 'types') -Filter '*.toml' | ForEach-Object BaseName)
$missing = @($all | Where-Object { -not $seen.ContainsKey($_) })
if ($missing.Count) { throw "types in no group: $($missing -join ' ')" }
foreach ($t in $all) {
    $entries += Build-One (Join-Path $here "types\$t.toml") (Join-Path $full ("types\KCB_T_{0}.esp" -f ($t -replace '_', ''))) 'type' $seen[$t] @($t)
}
foreach ($f in Get-ChildItem -LiteralPath (Join-Path $here 'fixtures') -Filter '*.toml') {
    $types = @()
    $entries += Build-One $f.FullName (Join-Path $full "fixtures\KCB_$($f.BaseName).esp") 'fixture' $f.BaseName $types
}
# Fixture plugins: the types they hold, from their records.
foreach ($e in $entries | Where-Object kind -eq 'fixture') { $e.types = @($e.records | ForEach-Object signature | Select-Object -Unique) }

$kitInfo = (Invoke-Kit @('roundtrip', (Join-Path $full $entries[0].path), '--verify')).build
$m = [pscustomobject]@{
    schema = 'kitkraft-ck-bench-manifest/1'
    built = (Get-Date -Format 'yyyy-MM-dd HH:mm:ss')
    kit = $Kit; kit_build = $kitInfo
    commit = $(try { (& git -C $here rev-parse HEAD 2>$null) } catch { $null })
    plugins = $entries
}
$m | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 (Join-Path $full 'manifest.json')

$md = @("# CK bench batch, built $($m.built)", '', "kit: $Kit; recipes at commit $($m.commit)", '',
    '| plugin | kind | types | records | bytes | SHA-256 | validate | roundtrip |', '| ------ | ---- | ----- | ------: | ----: | ------- | -------- | --------- |')
foreach ($e in $entries) { $md += "| $($e.file) | $($e.kind) | $($e.types -join ' ') | $(@($e.records).Count) | $($e.bytes) | $($e.sha256.Substring(0, 12)) | $($e.validate_errors) errors, $($e.validate_warnings) warnings | $(if ($e.roundtrip_identical) { 'identical' } else { 'differs' }) |" }
$md | Set-Content -Encoding utf8 (Join-Path $full 'manifest.md')

# The MO2 commands for the bench (ADR 0009: kit deploy --dev into its own mod; kit deploy
# enable into Kitkraft Dev's list files only, with MO2, the game and the CK closed). The group
# and fixture plugins go in; a type plugin only when its group needs splitting.
$run = @($entries | Where-Object { $_.kind -ne 'type' })
$d = @('# Deploy (run with MO2, the game and the CK closed; each with --dry-run first, then without)', '')
foreach ($e in $run) { $d += "kit deploy `"$(Join-Path $full $e.path)`" $Mo2 --dev --dry-run" }
$d += ''
$d += "kit deploy enable $Mo2 --mods $(($run | ForEach-Object { [IO.Path]::GetFileNameWithoutExtension($_.file) }) -join ' ') --dev --profile `"$Profile`" --dry-run"
$d += ''
$d += '# After the sitting (MO2 closed):'
$d += "kit deploy disable $Mo2 --mods $(($run | ForEach-Object { [IO.Path]::GetFileNameWithoutExtension($_.file) }) -join ' ') --dev --profile `"$Profile`" --dry-run"
$d += ''
$d += '# A type plugin, only when its group must be split (example):'
$d += "kit deploy `"$(Join-Path $full 'types\KCB_T_GLOB.esp')`" $Mo2 --dev --dry-run"
$d += "kit deploy enable $Mo2 --mods KCB_T_GLOB --dev --profile `"$Profile`" --dry-run"
$d | Set-Content -Encoding utf8 (Join-Path $full 'deploy.txt')
Write-Host "built $($entries.Count) plugin(s): $($full)\manifest.json, manifest.md, deploy.txt"
