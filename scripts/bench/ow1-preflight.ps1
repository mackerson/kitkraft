# OW-1 headless preflight (docs/bench/ow1-prep.md). Starts nothing, writes nothing but its
# to stdout. Run it at the desk before step 1 of run sheet OW-1
# (docs/game-sessions.md), with MO2 and the game closed.
#
#   powershell -NoProfile -File scripts\bench\ow1-preflight.ps1 -Kit <path to kit.exe>
#
# Every check prints PASS, FAIL or INFO; the exit code is the number of FAILs.

param(
    [string]$Kit = "C:\Modding\kitkraft\.targets\x\release\kit.exe",
    [string]$Mo2 = "C:\Modding\MO2-Starfield",
    [string]$ProfileName = "Kitkraft Dev",
    [string]$Worktree = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$Observe = "C:\Modding\kitkraft-observe"
)

$BaseSave = "KitKraftBase"
$BaseSha = "fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf"
$PluginSha = "9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27"
$PluginMod = "KitKraft Dev - OldSovietMarksmanRifle"
$Plugin = "OldSovietMarksmanRifle.esm"
$Port = 45555
$MinFreeGB = 5

$script:fails = 0
function Say([string]$grade, [string]$what, [string]$detail) {
    if ($grade -eq "FAIL") { $script:fails++ }
    Write-Output ("{0,-4} {1}: {2}" -f $grade, $what, $detail)
}
function Sha([string]$path) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return $null }
    (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
}

$profileDir = Join-Path $Mo2 "profiles\$ProfileName"
$probes = Join-Path $Worktree "crates\game\probes\send-order.toml"
$ledger = Join-Path $Worktree "docs\game-observations.md"
Write-Output "OW-1 preflight $(Get-Date -Format s) on $env:COMPUTERNAME; worktree $Worktree"
$head = git -C $Worktree rev-parse --short HEAD 2>$null
Say "INFO" "worktree HEAD" "$head"

# 1. Nothing of the game's runs (preflight rule `running`; R22: MO2 closed).
$procs = Get-Process Starfield, sfse_loader, CreationKit, ModOrganizer -ErrorAction SilentlyContinue
if ($procs) { Say "FAIL" "processes" ("running: " + (($procs | ForEach-Object { "$($_.Name) ($($_.Id))" }) -join ", ")) }
else { Say "PASS" "processes" "no Starfield, sfse_loader, CreationKit or ModOrganizer" }

# 2. MO2's selected profile (preflight rule `selected-profile`).
$ini = Join-Path $Mo2 "ModOrganizer.ini"
$sel = (Select-String -LiteralPath $ini -Pattern '^selected_profile=' -ErrorAction SilentlyContinue | Select-Object -First 1).Line
if ($sel -match [regex]::Escape($ProfileName)) { Say "PASS" "selected profile" $sel }
else { Say "FAIL" "selected profile" "$sel (the owner selects '$ProfileName' in MO2, then closes MO2)" }

# 3. The base save and its SHA-256 (rule `base-save`).
$save = Join-Path $profileDir "saves\$BaseSave.sfs"
$h = Sha $save
if ($null -eq $h) { Say "FAIL" "base save" "$save missing" }
elseif ($h -eq $BaseSha) { Say "PASS" "base save" "$save $h" }
else { Say "FAIL" "base save" "$save is $h, the run sheet names $BaseSha" }

# 4. The plugin under test (rule `plugin-not-deployed`) and whether it is enabled now.
$esm = Join-Path $Mo2 "mods\$PluginMod\$Plugin"
$h = Sha $esm
if ($null -eq $h) { Say "FAIL" "plugin" "$esm missing (rebuild and redeploy: docs/game-sessions.md, OW-1 Inputs)" }
elseif ($h -eq $PluginSha) { Say "PASS" "plugin" "$esm $h" }
else { Say "FAIL" "plugin" "$esm is $h, expected $PluginSha" }
if (-not (Test-Path -LiteralPath (Join-Path $Mo2 "mods\$PluginMod\kitkraft-deploy.json"))) {
    Say "FAIL" "plugin" "no kitkraft-deploy.json in the mod (not a deploy --dev mod)"
}
$modOn = Select-String -LiteralPath (Join-Path $profileDir "modlist.txt") -SimpleMatch "+$PluginMod" -Quiet
$espOn = Select-String -LiteralPath (Join-Path $profileDir "plugins.txt") -SimpleMatch "*$Plugin" -Quiet
Say "INFO" "plugin enabled" "modlist.txt: $modOn; plugins.txt: $espOn (step 1, deploy enable, makes both true)"

# 5. The probes file (its hash goes in the entry).
$h = Sha $probes
if ($null -eq $h) { Say "FAIL" "probes" "$probes missing" } else { Say "INFO" "probes" "$probes $h" }

# 6. The console port: free now, and outside Windows' excluded ranges (rule `console-port`).
$listen = Get-NetTCPConnection -LocalPort $Port -ErrorAction SilentlyContinue
if ($listen) { Say "FAIL" "port $Port" ("in use by pid " + (($listen | ForEach-Object OwningProcess | Sort-Object -Unique) -join ", ")) }
else { Say "PASS" "port $Port" "nothing bound" }
$excluded = netsh int ipv4 show excludedportrange protocol=tcp 2>$null
$hit = $false
foreach ($line in $excluded) {
    if ($line -match '^\s*(\d+)\s+(\d+)') {
        if ($Port -ge [int]$Matches[1] -and $Port -le [int]$Matches[2]) { $hit = $true; Say "FAIL" "port $Port" "inside excluded range $($Matches[1])-$($Matches[2])" }
    }
}
if (-not $hit) { Say "PASS" "port $Port" "outside every excluded TCP range" }

# 7. Disk space for the observe folder and the session save.
$drive = (Get-Item -LiteralPath $Observe).PSDrive
$free = [math]::Round($drive.Free / 1GB, 1)
if ($free -ge $MinFreeGB) { Say "PASS" "disk" "$($drive.Name): $free GB free" } else { Say "FAIL" "disk" "$($drive.Name): $free GB free, under $MinFreeGB GB" }

# 8. The kit binary.
$h = Sha $Kit
if ($null -eq $h) { Say "FAIL" "kit" "$Kit missing (build it before the window, never while the game runs)" }
else { Say "INFO" "kit" "$Kit $h" }

# 9. The profile's list files: the 'before' of run sheet step 1.
foreach ($f in "modlist.txt", "plugins.txt", "loadorder.txt", "lockedorder.txt") {
    Say "INFO" "list file" ("$f " + (Sha (Join-Path $profileDir $f)))
}

# 10. The controller's own preflight, as a dry run (writes nothing).
if ($null -ne (Sha $Kit)) {
    $json = & $Kit game session $Mo2 $probes --base-save $BaseSave --base-save-sha256 $BaseSha --profile $ProfileName --observe $Observe --ledger $ledger --dry-run --json 2>&1 | Out-String
    try {
        $d = $json | ConvertFrom-Json
        if ($d.data) { $d = $d.data }
        foreach ($r in $d.refusals) { Say "FAIL" "dry run [$($r.rule)]" $r.message }
        if ($d.would_start) { Say "PASS" "dry run" "would start session $($d.plan.session)" }
        elseif (-not $d.refusals) { Say "FAIL" "dry run" "would not start, no refusal named" }
        $t = $d.plan.timeouts
        if ($t) { Say "INFO" "timeouts" "ready $($t.ready_secs)s (settle $($t.settle_secs)s), load $($t.load_secs)s, probe $($t.probe_secs)s, session $($t.session_secs)s" }
        foreach ($w in $d.plan.authorized_writes) { Say "INFO" "authorized write" "$w" }
    } catch {
        Say "FAIL" "dry run" "output was not JSON: $json"
    }
}

Write-Output "fails: $script:fails"
exit $script:fails
