<#
.SYNOPSIS
  Run a plugin's probes on the KitKraft Dev profile, through `kit game session` only.

.DESCRIPTION
  A thin wrapper over `kit` (M4.T10 (d)); the CLI stays the one implementation.

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

if (-not (Get-Command kit -ErrorAction SilentlyContinue)) {
    Write-Output 'kit is not on PATH: build it (cargo build --release -p kit-cli) and run `kit install --apply`, or add target\release to PATH.'
    exit 2
}
$repo = Split-Path -Parent $PSScriptRoot
$common = @('--mo2', $Mo2, '--probes', $Probes, '--profile', $DevProfile, '--game', $Game)
if ($BaseSaveSha256) { $common += @('--base-save-sha256', $BaseSaveSha256) }

$text = (& kit --json game session @common --dry-run) -join "`n"
if ($LASTEXITCODE -ne 0) { Write-Output $text; exit $LASTEXITCODE }
$result = $text | ConvertFrom-Json
$launch = $result.data.plan.launch
$quoted = $launch.args | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }
Write-Output ('Candidate launch command (grade D until M4.T1 (a) observes it): "{0}" {1}' -f $launch.program, ($quoted -join ' '))

$ledger = Join-Path $repo 'docs\game-observations.md'
$recorded = $false
if ($launch.evidence) {
    $recorded = Select-String -Path $ledger -SimpleMatch $launch.evidence -Quiet
}
if (-not $recorded) {
    Write-Output 'Refused: docs/game-observations.md has no entry for the launch fact (the game profile''s [launch] evidence is empty or not in the ledger). M4.T1 (a) records it; until then nothing is launched.'
    exit 1
}
if ($result.data.refusals.Count -gt 0) {
    foreach ($r in $result.data.refusals) { Write-Output ('refusal [{0}]: {1}' -f $r.rule, $r.message) }
    exit 1
}
& kit game session @common
exit $LASTEXITCODE
