<#
.SYNOPSIS
  Open a game session's observe\ folder (screenshots, copied logs, backups, report.json).

.DESCRIPTION
  A thin wrapper (M4.T10 (d)); it only reads and opens Explorer. The session id comes from
  -Session, or from a `kit mod run` report (-Report out\<name>\report.json, `session.session`),
  else the newest observe\<yyyy-mm-dd>-T<nn>\ folder; with none, observe\ itself.

.EXAMPLE
  scripts\open-observe.ps1 -Report out\OldSovietMarksmanRifle\report.json
#>
param(
    [string] $Session,
    [string] $Report,
    [string] $Observe
)

$repo = Split-Path -Parent $PSScriptRoot
$root = if ($Observe) { $Observe } else { Join-Path $repo 'observe' }
if (-not $Session -and $Report) {
    $run = Get-Content -LiteralPath $Report -Raw | ConvertFrom-Json
    if ($run.kind -ne 'kitkraft-mod-run') { Write-Output "$Report is not a kit mod run report."; exit 2 }
    $Session = $run.session.session
    if (-not $Session) { Write-Output "No session ran in that run ($($run.result)); see its report.md, Session."; exit 1 }
}
$dir = $root
if ($Session) {
    $dir = Join-Path $root $Session
} else {
    $newest = Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d{4}-\d{2}-\d{2}-T\d+$' } |
        Sort-Object Name -Descending | Select-Object -First 1
    if ($newest) { $dir = $newest.FullName }
}
if (-not (Test-Path -LiteralPath $dir)) { Write-Output "$dir does not exist (no session has written there)."; exit 1 }
Write-Output "Opening $dir"
Invoke-Item -LiteralPath $dir
