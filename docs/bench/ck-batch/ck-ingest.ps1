# The after-step of a CK bench run (docs/bench/ck-sitting.md, "After each plugin"): KitKraft's
# plugin against the CK's re-export of it, every difference classed by `cargo xtask oracle ck`,
# and from that the ck_open / ck_reexport verdicts per record type, drafted as
# docs/schema/evidence.toml entries (status ck-differential-tested) and a docs/ck-observations.md
# ledger entry, so results go in the same night.
#
#   # a bench sitting folder (result.json, kitkraft.esp, ck.esp from ck-plugin.ps1):
#   powershell -File docs\bench\ck-batch\ck-ingest.ps1 -Sitting C:\Modding\kitkraft-observe\ck-2026-11-16\bench-03-KCB_G1_Core -Manifest <plugins>\manifest.json -Kit <kit.exe>
#   # an older fixture folder (two files, the types named by hand):
#   powershell -File docs\bench\ck-batch\ck-ingest.ps1 -Ours <kitkraft.esm> -Ck <ck.esm> -SittingId ck-20261101-02 -Types CELL,REFR -Kit <kit.exe>
#   # then, after reading the drafts: the same command with -Apply (edits the two docs here)
#
# Writes <sitting>\ingest\ (or -Work): fixture\<id>\ (the oracle's input), oracle\ (its report),
# verdicts.json, evidence-entries.toml, ledger-entry.md, handoff.yaml. -Apply also appends the
# ledger entry to docs/ck-observations.md and the evidence entries to docs/schema/evidence.toml
# (a ledger pointer added to an existing entry of the same type and status); nothing else.
# Verdicts: ck_open comes from the run (result.json) or -CkOpen; a type's ck_reexport is `pass`
# when the CK's file round-trips and the oracle has no unexplained or ours-missing item for any
# record of that type, otherwise `inconclusive` (to be classed by hand, M5.A3), never `pass`.
# Rules: the oracle runs with -Rules (default docs/bench/ck-batch/explained-differences.toml,
# the explained-difference rules the owner accepted on 2026-11-17, "Accept all ten"); a pass
# that rests on one names the rule in its drafted note. -NoRules runs the oracle without them.
param(
    [string]$Sitting = '',
    [string]$Manifest = '',
    [string]$Ours = '',
    [string]$Ck = '',
    [string]$SittingId = '',
    [string[]]$Types = @(),
    [string]$CkOpen = 'pass',
    [string]$CkVersion = '',
    [string]$FixtureToml = '',
    [string]$Work = '',
    [string]$Kit = $(if ($env:KITKRAFT_KIT) { $env:KITKRAFT_KIT } else { '' }),
    [string]$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path,
    [string]$DocsRoot = '',
    [string]$Rules = '',
    [switch]$NoRules,
    [switch]$Apply
)
$ErrorActionPreference = 'Stop'
if ($Types.Count -eq 1 -and $Types[0] -like '*,*') { $Types = $Types[0] -split ',' }
$utf8 = New-Object System.Text.UTF8Encoding $false
function Sha([string]$p) { (Get-FileHash -Algorithm SHA256 -LiteralPath $p).Hash.ToLowerInvariant() }
function Q([string]$s) { '"' + ($s -replace '\\', '\\' -replace '"', '\"') + '"' }

# ---- inputs -----------------------------------------------------------------------------------
$result = $null; $entry = $null; $plugin = ''
if ($Sitting) {
    $Sitting = (Resolve-Path -LiteralPath $Sitting).Path
    $result = Get-Content -Raw -LiteralPath (Join-Path $Sitting 'result.json') | ConvertFrom-Json
    $plugin = $result.plugin
    if (-not $SittingId) { $SittingId = $result.sitting }
    if (-not $Ours) { $Ours = Join-Path $Sitting 'kitkraft.esp' }
    if (-not $Ck) { $Ck = Join-Path $Sitting 'ck.esp' }
    $CkOpen = $result.ck_open
    if (-not $CkVersion) { $CkVersion = $result.ck_version }
    if (-not $Work) { $Work = Join-Path $Sitting 'ingest' }
}
if ($Manifest) {
    $m = Get-Content -Raw -LiteralPath $Manifest | ConvertFrom-Json
    if (-not $plugin) { $plugin = [IO.Path]::GetFileName($Ours) }
    $entry = $m.plugins | Where-Object { $_.file -eq $plugin } | Select-Object -First 1
    if (-not $entry) { throw "$plugin is not in $Manifest" }
    if (-not $Types.Count) { $Types = @($entry.types) }
    if ((Sha $Ours) -ne $entry.sha256) { throw "refused: $Ours is not the plugin the manifest built ($($entry.sha256.Substring(0, 12)))" }
}
if (-not $Ours -or -not (Test-Path -LiteralPath $Ours)) { throw "no KitKraft file (-Ours or <sitting>\kitkraft.esp)" }
if ($SittingId -notmatch '^ck-\d{8}-\d{2}$') { throw "refused: sitting id '$SittingId' is not ck-<yyyymmdd>-<nn>" }
if (-not $Types.Count) { throw 'no record types (-Manifest or -Types)' }
if (-not $Work) { $Work = Join-Path ([IO.Path]::GetTempPath()) "kk-ingest-$SittingId" }
New-Item -ItemType Directory -Force $Work | Out-Null
$hasCk = $Ck -and (Test-Path -LiteralPath $Ck)

# ---- the oracle -------------------------------------------------------------------------------
$verdict = $null; $report = $null
if ($hasCk) {
    $ext = [IO.Path]::GetExtension($Ours)
    $fx = Join-Path $Work "fixture\$SittingId"
    if (Test-Path -LiteralPath $fx) { Remove-Item -Recurse -Force -LiteralPath $fx }
    New-Item -ItemType Directory -Force $fx | Out-Null
    Copy-Item -LiteralPath $Ours -Destination (Join-Path $fx "kitkraft$ext")
    Copy-Item -LiteralPath $Ck -Destination (Join-Path $fx "ck$([IO.Path]::GetExtension($Ck))")
    # match: every record KitKraft wrote that is its own (master index 01), as kitkraft<ext>:XXXXXX.
    $match = @()
    if ($entry) {
        foreach ($r in $entry.records) {
            if ($r.form -like "$($entry.file):*" -and $r.form -match ':0*([0-9A-Fa-f]{1,6})$') { $match += "kitkraft$($ext):$('{0:X6}' -f [Convert]::ToInt32($Matches[1], 16))" }
        }
    }
    $t = @("fixture = $(Q $SittingId)", "title = $(Q "CK bench: $plugin opened as the active file and saved in place")")
    if ($CkVersion) { $t += "ck_version = $(Q $CkVersion)" }
    $t += "ours = $(Q "kitkraft$ext")"; $t += "ck = $(Q ("ck" + [IO.Path]::GetExtension($Ck)))"
    if ($match.Count) { $t += "match = [$(($match | Select-Object -Unique | ForEach-Object { Q $_ }) -join ', ')]" }
    $t += ''
    # The CK's save conventions classed in M5.A3 (ck-20261101-04), as F30's fixture.toml has them.
    foreach ($n in @(
        @('TES4', 'HEDR', 'the CK''s save convention: record count = records below TES4 plus groups, next object ID = its own counter (F30, F29)'),
        @('TES4', 'DATA', 'the CK writes MAST without the 8-byte DATA that follows every MAST in the official files'),
        @('TES4', 'BNAM', 'the CK names its branch ("Main", shown in its title bar); 12 of the 14 official files carry a BNAM, ours none'))) {
        $t += '[[normalization]]'; $t += "record = $(Q $n[0])"; $t += "subrecord = $(Q $n[1])"; $t += "reason = $(Q $n[2])"; $t += 'ledger = "ck-20261101-02"'; $t += ''
    }
    if ($FixtureToml) {
        $extra = Get-Content -Raw -LiteralPath $FixtureToml
        $i = $extra.IndexOf('[[normalization]]')
        if ($i -ge 0) { $t += '# from ' + $FixtureToml; $t += $extra.Substring($i) }
    }
    [IO.File]::WriteAllText((Join-Path $fx 'fixture.toml'), ($t -join "`r`n"), $utf8)
    $od = Join-Path $Work 'oracle'
    if ($Kit) { $env:KITKRAFT_KIT = $Kit }
    $ruleArgs = @()
    if (-not $NoRules) {
        if (-not $Rules) { $Rules = Join-Path $Repo 'docs\bench\ck-batch\explained-differences.toml' }
        if (-not (Test-Path -LiteralPath $Rules)) { throw "no rules file $Rules (or pass -NoRules)" }
        $ruleArgs = @('--rules', (Resolve-Path -LiteralPath $Rules).Path)
    }
    $prev = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    Push-Location $Repo
    try { $verdict = (& cargo xtask oracle ck $fx --out $od @ruleArgs 2>&1 | Out-String).Trim() } finally { Pop-Location; $ErrorActionPreference = $prev }
    $rp = Join-Path $od "$SittingId\report.json"
    if (-not (Test-Path -LiteralPath $rp)) { throw "the oracle wrote no report ($rp): $verdict" }
    $report = Get-Content -Raw -LiteralPath $rp | ConvertFrom-Json
    Write-Host $report.verdict_line
}

# ---- verdicts ---------------------------------------------------------------------------------
$rtOk = $report -and $report.ck.roundtrip.passed
$items = @(if ($report) { $report.items })
$fileOpen = @($items | Where-Object { $_.record -eq 'TES4' -and $_.class -ne 'explained' })
$rows = @()
foreach ($ty in $Types) {
    $bad = @($items | Where-Object { $_.record -eq $ty -and $_.class -ne 'explained' })
    $n = if ($entry) { @($entry.records | Where-Object signature -eq $ty).Count } else { $null }
    $re = if ($entry -and $n -eq 0) { 'not-run' }
          elseif (-not $hasCk) { if ($result -and $result.ck_reexport -eq 'inconclusive') { 'inconclusive' } else { 'not-run' } }
          elseif (-not $rtOk) { 'inconclusive' }
          elseif ($bad.Count) { 'inconclusive' }
          else { 'pass' }
    $why = if ($entry -and $n -eq 0) { 'no record of the type in the plugin' }
           elseif (-not $hasCk) { 'no CK file (no save, or the save wrote nothing)' }
           elseif (-not $rtOk) { 'the CK''s file does not round-trip' }
           elseif ($bad.Count) { "$($bad.Count) difference(s) to class by hand: " + (($bad | ForEach-Object { "$($_.class) $($_.form) $($_.subrecord) $($_.change)" }) -join '; ') }
           else {
               # A pass that rests on an accepted rule (a transform rule of -Rules) names it.
               $ruled = @($items | Where-Object { $_.record -eq $ty -and $_.class -eq 'explained' -and $_.because -match '\[([^\]]+)\] \((ck-[0-9-]+)\)$' } | ForEach-Object {
                   if ($_.because -match '\[([^\]]+)\] \((ck-[0-9-]+)\)$') { "$($_.subrecord) $($Matches[1]) ($($Matches[2]))" } } | Select-Object -Unique)
               if ($ruled.Count) { 'every record of the type came back with no unexplained difference; rests on the accepted rule(s) of explained-differences.toml: ' + ($ruled -join ', ') }
               else { 'every record of the type came back with no unexplained difference' }
           }
    $ev = if ($re -eq 'not-run' -and $entry -and $n -eq 0) { 'not-run' } elseif ($CkOpen -eq 'fail') { 'fail' } elseif ($CkOpen -eq 'pass' -and $re -eq 'pass') { 'pass' } elseif ($CkOpen -eq 'pass' -or $CkOpen -eq 'inconclusive' -or $re -eq 'inconclusive') { 'inconclusive' } else { 'not-run' }
    $rows += [pscustomobject]@{ type = $ty; records = $n; ck_open = $CkOpen; ck_reexport = $re; evidence = $ev; why = $why }
}
$plugRe = if (-not $hasCk) { 'not-run' } elseif ($rtOk -and -not @($items | Where-Object class -ne 'explained').Count) { 'pass' } else { 'inconclusive' }
$out = [pscustomobject]@{
    sitting = $SittingId; plugin = $(if ($plugin) { $plugin } else { [IO.Path]::GetFileName($Ours) })
    ours = $Ours; ours_sha256 = (Sha $Ours); ck = $(if ($hasCk) { $Ck }); ck_sha256 = $(if ($hasCk) { Sha $Ck })
    ck_version = $CkVersion; oracle = $(if ($report) { $report.verdict_line }); tes4_open = $fileOpen.Count
    ck_open = $CkOpen; ck_reexport = $plugRe; types = $rows
}
$out | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 (Join-Path $Work 'verdicts.json')

# ---- drafts -----------------------------------------------------------------------------------
$evText = @("# Drafted by docs/bench/ck-batch/ck-ingest.ps1 for $SittingId ($($out.plugin)); read before -Apply.")
foreach ($r in $rows | Where-Object { $_.evidence -ne 'not-run' }) {
    $evText += ''; $evText += '[[entry]]'; $evText += "type = $(Q $r.type)"; $evText += 'status = "ck-differential-tested"'
    $evText += "result = $(Q $r.evidence)"; $evText += "evidence = [$(Q "ledger: $SittingId")]"
    $evText += "note = $(Q "CK bench $SittingId ($($out.plugin)): ck_open $($r.ck_open), ck_reexport $($r.ck_reexport); $($r.why)")"
}
[IO.File]::WriteAllText((Join-Path $Work 'evidence-entries.toml'), ($evText -join "`r`n") + "`r`n", $utf8)

$typesLine = ($rows | ForEach-Object { "$($_.type) $($_.ck_reexport)" }) -join ', '
$warn = if ($result) { "$($result.warnings.total) rows, $($result.warnings.named_rows) naming the plugin or its forms (``$($result.warnings.file)``)" } else { 'not recorded here' }
$led = @(
    '', "### ${SittingId}: CK bench, $($out.plugin) opened as the active file$(if ($hasCk) { ' and saved in place' }), agent-run", '',
    "Run by ``ck-plugin.ps1`` (``docs/bench/ck-sitting.md``) through MO2 on ``Kitkraft Dev``. Record in ``$(if ($Sitting) { $Sitting } else { 'C:\Modding\kitkraft-observe\ck-<yyyy-mm-dd>\<name>' })\`` (``session.md``, ``result.json``, ``shots\``, ``kitkraft.esp``, ``ck.esp``); oracle report in its ``ingest\oracle\``.", '',
    '| Field | Content |', '| ----- | ----- |',
    "| Id | ``$SittingId`` |",
    "| CK version | $(if ($CkVersion) { $CkVersion } else { 'not recorded' }), from the title |",
    "| Input | ``$($out.plugin)``, SHA-256 ``$($out.ours_sha256.Substring(0, 8))...``, record types $($Types -join ', ') |",
    "| ck_open | $CkOpen$(if ($result) { ": $($result.reasons.ck_open)" }) |",
    "| Warnings | $warn |",
    "| ck_reexport | $plugRe$(if ($hasCk) { ": the CK's file SHA-256 ``$($out.ck_sha256.Substring(0, 8))...``; $($out.oracle)" } elseif ($result) { ": $($result.reasons.ck_reexport)" }) |",
    "| Per type | $typesLine |",
    "| Open | $(if (@($rows | Where-Object ck_reexport -eq 'inconclusive').Count -or $fileOpen.Count) { 'the oracle''s unexplained items (report.md) are classed by hand (M5.A3); the types stay inconclusive until then' } else { 'nothing' }) |"
)
[IO.File]::WriteAllText((Join-Path $Work 'ledger-entry.md'), ($led -join "`r`n") + "`r`n", $utf8)
$y = @("# validation block for the hand-off (AGENTS.md, Reporting): $($out.plugin), $SittingId", 'validation:', '  parse: pass', '  byte_roundtrip: pass', "  ck_open: $CkOpen", "  ck_reexport: $plugRe", '  game_runtime: not-run', '  save_reload: not-run')
[IO.File]::WriteAllText((Join-Path $Work 'handoff.yaml'), ($y -join "`r`n") + "`r`n", $utf8)
$rows | Format-Table type, records, ck_open, ck_reexport, evidence -AutoSize | Out-String | Write-Host
Write-Host "drafts: $Work\evidence-entries.toml, ledger-entry.md, handoff.yaml, verdicts.json"

# ---- -Apply: the two docs ----------------------------------------------------------------------
if ($Apply) {
    if (-not $DocsRoot) { $DocsRoot = $Repo }
    $obs = Join-Path $DocsRoot 'docs\ck-observations.md'
    $evf = Join-Path $DocsRoot 'docs\schema\evidence.toml'
    $ot = [IO.File]::ReadAllText($obs)
    if ($ot -match "(?m)^### $([regex]::Escape($SittingId))\b") { throw "refused: docs/ck-observations.md already has $SittingId" }
    # Each doc keeps its own line endings.
    $nlO = if ($ot.Contains("`r`n")) { "`r`n" } else { "`n" }
    [IO.File]::WriteAllText($obs, (($ot.TrimEnd() + "`n" + ($led -join "`n") + "`n") -replace "`r?`n", $nlO), $utf8)
    $et = [IO.File]::ReadAllText($evf)
    $nlE = if ($et.Contains("`r`n")) { "`r`n" } else { "`n" }
    $anchor = '# Save/reload-verified'
    foreach ($r in $rows | Where-Object { $_.evidence -ne 'not-run' }) {
        $pat = "(?ms)(\[\[entry\]\]\r?\ntype = `"$([regex]::Escape($r.type))`"\r?\nstatus = `"ck-differential-tested`"\r?\nresult = `"(\w+)`"\r?\nevidence = \[)(.*?)(\r?\n?\])"
        $mm = [regex]::Match($et, $pat)
        if ($mm.Success) {
            # An entry exists: add the pointer; a pass stays a pass only if this run passed too.
            $list = $mm.Groups[3].Value
            $newList = if ($list -match '\r?\n') { $list.TrimEnd() + "`r`n  `"ledger: $SittingId`"," } else { $list + ", `"ledger: $SittingId`"" }
            $res = if ($mm.Groups[2].Value -eq 'pass' -and $r.evidence -eq 'pass') { 'pass' } elseif ($r.evidence -eq 'fail') { 'fail' } else { $mm.Groups[2].Value }
            $rep = $mm.Groups[1].Value -replace 'result = "\w+"', "result = `"$res`""
            $et = $et.Substring(0, $mm.Index) + $rep + $newList + $mm.Groups[4].Value + $et.Substring($mm.Index + $mm.Length)
            Write-Host "evidence.toml: $($r.type) ck-differential-tested: ledger: $SittingId added to the existing entry (result $res)"
        } else {
            $block = "[[entry]]`r`ntype = $(Q $r.type)`r`nstatus = `"ck-differential-tested`"`r`nresult = $(Q $r.evidence)`r`nevidence = [$(Q "ledger: $SittingId")]`r`nnote = $(Q "CK bench $SittingId ($($out.plugin)): ck_open $($r.ck_open), ck_reexport $($r.ck_reexport); $($r.why)")`r`n`r`n"
            $i = $et.IndexOf($anchor)
            if ($i -lt 0) { throw "evidence.toml has no '$anchor' line to insert before" }
            $et = $et.Substring(0, $i) + $block + $et.Substring($i)
            Write-Host "evidence.toml: $($r.type) ck-differential-tested $($r.evidence) added"
        }
    }
    [IO.File]::WriteAllText($evf, ($et -replace "`r?`n", $nlE), $utf8)
    Write-Host 'applied: docs/ck-observations.md, docs/schema/evidence.toml (then: cargo test -p kitkraft-engine every_evidence_pointer_resolves; cargo xtask parity)'
}
