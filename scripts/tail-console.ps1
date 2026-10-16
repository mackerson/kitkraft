<#
.SYNOPSIS
  Show the game's console history log: the live one a session reads, or a session's copy.

.DESCRIPTION
  A thin wrapper over `kit` (M4.T10 (d)); it only reads.

  - With -Probes and -Mo2: `kit --json game session --dry-run` names the logs the session
    would copy (the game profile's [[log]] paths, resolved); this tails the one named
    `console-history` (-Wait follows it while a session runs).
  - With -Session <yyyy-mm-dd-Tnn>: tails the copy the session made under
    observe\<session>\logs\ (after `kit game session` or `kit game stop`).

  The log's path is grade D until M4.T1 observes that a session's commands appear in it
  (docs/game-sessions.md, "Game profiles").

.EXAMPLE
  scripts\tail-console.ps1 -Probes out\OldSovietMarksmanRifle\testplan\probes.toml -Mo2 C:\Modding\MO2-Starfield -Wait
.EXAMPLE
  scripts\tail-console.ps1 -Session 2026-10-16-T01
#>
param(
    [string] $Probes,
    [string] $Mo2,
    [string] $Session,
    [string] $Observe,
    [int] $Tail = 40,
    [switch] $Wait,
    [string] $DevProfile = 'KitKraft Dev'
)

$repo = Split-Path -Parent $PSScriptRoot
if ($Session) {
    $root = if ($Observe) { $Observe } else { Join-Path $repo 'observe' }
    $log = Get-ChildItem -Path (Join-Path (Join-Path $root $Session) 'logs') -Filter 'console-history-*' -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $log) { Write-Output "No console-history log copied for session $Session under $root (a log the game did not write is recorded as missing in the session's report)."; exit 1 }
    $path = $log.FullName
} else {
    if (-not ($Probes -and $Mo2)) { Write-Output 'Give -Session, or -Probes and -Mo2.'; exit 2 }
    if (-not (Get-Command kit -ErrorAction SilentlyContinue)) { Write-Output 'kit is not on PATH (see scripts\launch-dev.ps1).'; exit 2 }
    $text = (& kit --json game session --mo2 $Mo2 --probes $Probes --profile $DevProfile --dry-run) -join "`n"
    if ($LASTEXITCODE -ne 0) { Write-Output $text; exit $LASTEXITCODE }
    $entry = ($text | ConvertFrom-Json).data.plan.logs | Where-Object { $_.name -eq 'console-history' } | Select-Object -First 1
    if (-not $entry) { Write-Output 'The game profile names no console-history log.'; exit 1 }
    $path = $entry.path
}
Write-Output "== $path"
if (-not (Test-Path -LiteralPath $path)) { Write-Output 'Not written (yet).'; exit 1 }
if ($Wait) { Get-Content -LiteralPath $path -Tail $Tail -Wait } else { Get-Content -LiteralPath $path -Tail $Tail }
