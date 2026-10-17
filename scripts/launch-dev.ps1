<#
.SYNOPSIS
  Run a plugin's probes on the KitKraft Dev profile, through `kit game session` only.

.DESCRIPTION
  A thin wrapper over `kit` (M4.T10 (d)); the CLI stays the one implementation. It runs
  this checkout's bin\kit.exe, else dist\KitKraft\kit.exe, never a `kit` from PATH.

  1. `kit --json game session --dry-run` gives the plan (the MO2 command line the session
     would start) and every preflight refusal; nothing is written.
  2. Starting the dev profile through MO2 is not verified yet (M4.T1 (a)). Until the game
     profile's [launch] section names a ledger entry that docs/game-observations.md holds,
     this prints the candidate command line and refuses to run it (exit 1).
  3. With the launch fact recorded and no refusal, it runs `kit game session` (synchronous),
     the only launcher an agent may use (AGENTS.md, ADR 0010). It never starts MO2 or the
     game itself, and never on the Default profile (the controller refuses it by name).

.EXAMPLE
  scripts\launch-dev.ps1 -Probes out\OldSovietMarksmanRifle\testplan\probes.toml -Mo2 C:\Modding\MO2-Starfield -BaseSaveSha256 <hex>
#>
param(
    [Parameter(Mandatory = $true)] [string] $Probes,
    [Parameter(Mandatory = $true)] [string] $Mo2,
    [string] $BaseSaveSha256,
    [string] $DevProfile = 'KitKraft Dev',
    [string] $Game = 'starfield'
)

$repo = Split-Path -Parent $PSScriptRoot
# This checkout's own kit (Fable review of wave 3, note 7): bin\kit.exe (`cargo xtask dist
# --sync-bin`), else dist\KitKraft\kit.exe (`cargo xtask dist`); never whatever `kit` is
# first on PATH, which may be another checkout's or an older build.
$kit = @(
    (Join-Path $repo 'bin\kit.exe'),
    (Join-Path $repo 'dist\KitKraft\kit.exe')
) | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
if (-not $kit) {
    Write-Output ('No kit.exe in this checkout: neither {0} nor {1} exists. Build it with `cargo xtask dist` (dist\KitKraft) or `cargo xtask dist --sync-bin` (bin\).' -f (Join-Path $repo 'bin\kit.exe'), (Join-Path $repo 'dist\KitKraft\kit.exe'))
    exit 2
}
Write-Output ('kit: {0}' -f $kit)
$common = @('--mo2', $Mo2, '--probes', $Probes, '--profile', $DevProfile, '--game', $Game)
if ($BaseSaveSha256) { $common += @('--base-save-sha256', $BaseSaveSha256) }

$text = (& $kit --json game session @common --dry-run) -join "`n"
if ($LASTEXITCODE -ne 0) { Write-Output $text; exit $LASTEXITCODE }
$result = $text | ConvertFrom-Json
$launch = $result.data.plan.launch
$quoted = $launch.args | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }
Write-Output ('Candidate launch command (grade D until M4.T1 (a) observes it): "{0}" {1}' -f $launch.program, ($quoted -join ' '))

$ledger = Join-Path $repo 'docs\game-observations.md'
$recorded = $false
# The evidence names a whole ledger entry id (`game-20261016-T1`, optionally written
# `game:<id>/<row>`); it counts only when the ledger has that entry's heading
# (`### <id>: ...`), matched anchored, so a short or partial token never matches text
# elsewhere in the file.
$entry = ''
if ($launch.evidence) {
    $entry = ([string]$launch.evidence).Trim() -replace '^game:', ''
    $entry = ($entry -split '/', 2)[0].Trim()
}
if ($entry -match '^game-\d{8}-[A-Za-z0-9]+$') {
    $heading = '^###\s+' + [regex]::Escape($entry) + ':'
    $recorded = [bool](Select-String -LiteralPath $ledger -Pattern $heading -CaseSensitive -Quiet)
}
if (-not $recorded) {
    Write-Output ('Refused: docs/game-observations.md has no ledger entry `{0}` for the launch fact (the game profile''s [launch] evidence must name a whole entry id, game-<yyyymmdd>-<nn>, whose heading the ledger holds). M4.T1 (a) records it; until then nothing is launched.' -f $launch.evidence)
    exit 1
}
if ($result.data.refusals.Count -gt 0) {
    foreach ($r in $result.data.refusals) { Write-Output ('refusal [{0}]: {1}' -f $r.rule, $r.message) }
    exit 1
}
& $kit game session @common
exit $LASTEXITCODE
