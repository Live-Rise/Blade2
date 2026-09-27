<#
  selftest_tools.ps1 - STATIC (no-GUI) self-check of the rust/tests tool library. Ledger #143.

  WHY THIS EXISTS
    rust/tmp/ is gitignored (.gitignore:11) and has been wiped by the sync disk twice. The
    GUI harness used to dot-source a helper from there and, when it was gone, fall back
    SILENTLY to a best-effort nudge that cannot beat the Windows foreground lock - so the
    screenshots written afterwards were frames of whatever the operator was looking at.
    Fake evidence. That whole failure mode is now forbidden by construction, and this file
    is the guard that proves it without touching a window.

  WHAT IT CHECKS (all statically decidable - NO exe is started, NO window created, NO
  screenshot taken, NO UIA, NO SendInput; the only API used is the PS language parser)
    1) ParseFile      : every rust/tests/*.ps1 parses with 0 syntax errors
    2) encoding       : UTF-8 BOM present and the body decodes as strict UTF-8
                        (repo-verified: PS 5.1 mis-parses a BOM-less .ps1 that carries 中文)
    3) entry points   : the functions the other scripts call really are defined (AST walk)
    4) sync-disk deps : no CODE token references a helper under a tmp path / the retired
                        library, and every load statement resolves next to $PSScriptRoot
    5) no-silent-fallback: gui_uia.ps1 must (a) load tests/shot_harness.ps1, (b) reach a
                        `throw` when it is missing, (c) contain no legacy-nudge code path
    6) coverage       : unknown *.ps1 in tests/ is still parsed + BOM-checked, so a new tool
                        cannot sneak in unverified

  USAGE
    powershell -NoProfile -ExecutionPolicy Bypass -File rust\tests\selftest_tools.ps1
    exit 0 = all green, exit 1 = at least one FAIL line.
    Add -Quiet to print only the FAIL/RESULT lines (per-file PASS spam off).
#>
[CmdletBinding()]
param(
    [string]$Dir = '',
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

function W([string]$m) { [Console]::Out.WriteLine($m) }
$script:FailCount = 0
$script:PassCount = 0
function Pass([string]$what) { $script:PassCount++; if (-not $Quiet) { W ('PASS ' + $what) } }
function Fail([string]$what) {
    W ('FAIL ' + $what)
    $script:FailCount++
}

# The retired sync-disk library name, built by concatenation so that this file's own source
# never contains the literal and therefore never trips its own rule 4.
$script:LibName = 'shot' + 'lib'

# ---------------------------------------------------------------- required entry points
# Left = file name, Right = function names that MUST be defined in it (AST FunctionDefinitionAst).
$script:Required = @{
    'gui_uia.ps1'       = @('W', 'Fail-ShotHarness', 'Raise-Foreground-For-Shot', 'Raise-For-Shot', 'Drop-After-Shot', 'Find-HwndByTitle', 'Stop-ProcTree', 'Get-AppExitCode')
    'shot_harness.ps1'  = @('Invoke-ShotForeground', 'Invoke-QaShot', 'Get-QaShotStats', 'Reset-ShotRound', 'Get-ShotNoteTail', 'Write-ShotLog', 'Get-ShotPathFromTag')
    'gui_probe.ps1'     = @('Find-Windows')
    'selftest_tools.ps1' = @('Read-Script', 'Check-File')
}

# ---------------------------------------------------------------- helpers
# ParseFile gives back the AST plus every token (comments included) plus syntax errors.
# Splitting comments from code is what makes rule 4 honest: a comment MAY say
# "history: the retired <lib> is gone", the code may not.
function Read-Script([string]$Path) {
    $toks = $null
    $errs = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$toks, [ref]$errs)
    $code = New-Object System.Text.StringBuilder
    foreach ($t in @($toks)) {
        if ($t.Kind -eq 'Comment') { continue }
        [void]$code.Append($t.Extent.Text)
        [void]$code.Append(' ')
    }
    $fns = @($ast.FindAll( { param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true) |
        ForEach-Object { [string]$_.Name })
    $cmds = @($ast.FindAll( { param($n) $n -is [System.Management.Automation.Language.CommandAst] }, $true) |
        ForEach-Object { [string]$_.Extent.Text })
    return [pscustomobject]@{
        Path     = $Path
        Ast      = $ast
        Tokens   = @($toks)
        Errors   = @($errs)
        Code     = $code.ToString()
        Fns      = $fns
        Commands = $cmds
    }
}

function Get-Bytes([string]$Path) { return ,[byte[]]([System.IO.File]::ReadAllBytes($Path)) }

# ---------------------------------------------------------------- per-file checks
function Check-File([string]$Path) {
    $name = [System.IO.Path]::GetFileName($Path)
    $bytes = Get-Bytes $Path
    $body = $bytes
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) { $body = $bytes[3..($bytes.Length - 1)] }
    $hi = @($body | Where-Object { $_ -ge 0x80 }).Count

    # 2a) UTF-8 BOM
    $bom = ''
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
        $bom = 'ef bb bf'
        Pass ($name + ' BOM=utf-8-bom first3=' + $bom)
    } else {
        $head = ($bytes | Select-Object -First 3 | ForEach-Object { '{0:x2}' -f $_ }) -join ' '
        Fail ($name + ' BOM=MISSING first3=' + $head + ' (PS 5.1 reads a BOM-less file as ANSI: any 中文 in it breaks parsing - task #22 class of bug). hi-bytes=' + $hi)
    }

    # 2b) body must decode as STRICT UTF-8 (throws on an invalid sequence)
    try {
        $strict = New-Object System.Text.UTF8Encoding($false, $true)
        [void]$strict.GetString($bytes)
        Pass ($name + ' utf8-strict=ok bytes=' + $bytes.Length + ' hi-bytes=' + $hi)
    } catch {
        Fail ($name + ' utf8-strict=INVALID: ' + (($_.Exception.Message -replace '\s+', ' ')))
    }

    $r = Read-Script $Path

    # 1) syntax
    if ($r.Errors.Count -eq 0) {
        Pass ($name + ' PARSE-ERRORS=0 tokens=' + $r.Tokens.Count)
    } else {
        Fail ($name + ' PARSE-ERRORS=' + $r.Errors.Count)
        foreach ($e in $r.Errors) {
            $ln = 0
            try { $ln = $e.Extent.StartLineNumber } catch { }
            W ('       [' + $ln + '] ' + $e.Message)
        }
    }

    # 3) required entry points
    if ($script:Required.ContainsKey($name)) {
        foreach ($fn in $script:Required[$name]) {
            if ($r.Fns -contains $fn) { Pass ($name + ' function=' + $fn + ' defined') }
            else { Fail ($name + ' function=' + $fn + ' MISSING (callers would hit a CommandNotFoundException mid-run)') }
        }
    }

    # 4a) no CODE reference to the retired sync-disk helper
    if ($r.Code -match [regex]::Escape($script:LibName)) {
        Fail ($name + ' DEPENDENCY=code still mentions the retired sync-disk helper (' + $script:LibName + ')')
    } else {
        Pass ($name + ' dependency=code free of the retired sync-disk helper')
    }

    # 4b) no CODE token may name a *.ps1 living under any tmp directory (gitignored = evapouring)
    $tmpHit = @($r.Tokens | Where-Object { $_.Kind -ne 'Comment' -and $_.Extent.Text -match '(?i)[\\/]?\btmp[\\/][^\s]*\.ps1' })
    if ($tmpHit.Count -eq 0) {
        Pass ($name + ' dependency=no .ps1 path under a tmp/ directory in code')
    } else {
        foreach ($t in $tmpHit) {
            Fail ($name + ' DEPENDENCY=sync-disk path in code at line ' + $t.Extent.StartLineNumber + ': ' + ($t.Extent.Text -replace '\s+', ' '))
        }
    }

    # 4c) every load statement (. foo / & foo / Import-Module) must resolve next to this script
    $loads = @($r.Commands | Where-Object { $_ -match '^\s*(?:\.|&)\s' -or $_ -match '(?im)^\s*(?:Import-Module|ipmo)\s' })
    $badLoads = @($loads | Where-Object { $_ -match '(?i)tmp' -or $_ -match [regex]::Escape($script:LibName) })
    if ($badLoads.Count -gt 0) {
        foreach ($l in $badLoads) { Fail ($name + ' LOAD=' + ($l -replace '\s+', ' ') + ' reaches outside the tracked tests/ directory') }
    } elseif ($loads.Count -eq 0) {
        Pass ($name + ' load-statements=none')
    } else {
        Pass ($name + ' load-statements=' + $loads.Count + ' all tracked-relative')
    }

    # 5) the anti-fake-evidence contract, per file
    if ($name -eq 'gui_uia.ps1') {
        if ($r.Code -match 'Join-Path\s+\$PSScriptRoot\s+[\x27"]shot_harness\.ps1[\x27"]') {
            Pass 'gui_uia.ps1 loads tests/shot_harness.ps1 by $PSScriptRoot (same directory, git-tracked)'
        } else {
            Fail 'gui_uia.ps1 no longer pins the helper to $PSScriptRoot\shot_harness.ps1'
        }
        # missing library must THROW, not degrade: the Test-Path guard has exactly one body.
        if ($r.Code -match '(?is)if\s*\(\s*-not\s*\(\s*Test-Path.{0,160}?ShotHarnessPath.{0,160}?\)\s*\)\s*\{\s*Fail-ShotHarness') {
            Pass 'gui_uia.ps1 missing-library branch calls Fail-ShotHarness (no else, no degrade)'
        } else {
            Fail 'gui_uia.ps1 the Test-Path guard is no longer a one-way street to Fail-ShotHarness'
        }
        if ($r.Code -match '(?is)function\s+Fail-ShotHarness[^}]*\{\s*W[^\r\n]*\r?\n[^\r\n]*RESULT=error[^\r\n]*\r?\n\s*throw\s') {
            Pass 'gui_uia.ps1 Fail-ShotHarness ends in `throw` (terminating: the run cannot continue to a screenshot)'
        } else {
            Fail 'gui_uia.ps1 Fail-ShotHarness does not throw - a missing library would degrade silently again'
        }
        # The old availability flag is named by concatenation at runtime, so this file never
        # carries the forbidden literal in its own source (see rule 4a, which scans itself too).
        $deadFlag = $script:LibName + 'ok'
        foreach ($dead in @('fg-match=legacy-nudge', 'falling back to the legacy nudge', $deadFlag)) {
            if ($r.Code -match [regex]::Escape($dead)) { Fail ('gui_uia.ps1 still contains the silent-fallback code path "' + $dead + '"') }
            else { Pass ('gui_uia.ps1 silent-fallback code path removed: ' + $dead) }
        }
        # The nudge primitives must not be callable from this script any more: the only
        # remaining mention may be the P/Invoke declaration inside the Add-Type here-string.
        $nudgeCalls = [regex]::Matches($r.Code, '\[Win32Uia\]::(SwitchToThisWindow|SetForegroundWindow)\s*\(').Count
        if ($nudgeCalls -eq 0) { Pass 'gui_uia.ps1 nudge/activate P/Invoke has zero PowerShell call sites (declaration only)' }
        else { Fail ('gui_uia.ps1 has ' + $nudgeCalls + ' live call site(s) to the nudge primitives - the fallback is still reachable') }
        if ($r.Code -match 'SHOT-HARNESS=') { Pass 'gui_uia.ps1 prints the SHOT-HARNESS= evidence token' }
        else { Fail 'gui_uia.ps1 lost the SHOT-HARNESS= evidence line' }
    }
    if ($name -eq 'shot_harness.ps1') {
        if ($r.Code -match 'PW_RENDERFULLCONTENT' -and $r.Code -match 'PrintWindow') {
            Pass 'shot_harness.ps1 keeps PrintWindow + PW_RENDERFULLCONTENT (the anti-white-frame bit, task #35)'
        } else {
            Fail 'shot_harness.ps1 lost the PrintWindow/PW_RENDERFULLCONTENT path'
        }
        if ($r.Code -match 'PER_MONITOR_AWARE_V2|DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2') {
            Pass 'shot_harness.ps1 sets PMv2 itself (task #22 DPI trap)'
        } else {
            Fail 'shot_harness.ps1 lost its own DPI-awareness call'
        }
    }
}

# ---------------------------------------------------------------- main
if ($Dir -eq '') { $Dir = $PSScriptRoot }
if (-not (Test-Path -LiteralPath $Dir)) { Fail ('directory not found: ' + $Dir); W 'SELFTEST-TOOLS RESULT=error'; exit 1 }
$Dir = (Resolve-Path -LiteralPath $Dir).Path
W ('SELFTEST-TOOLS start=' + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss') + ' dir=' + $Dir + ' (static only: no exe, no window, no capture, no UIA, no SendInput)')

$files = @(Get-ChildItem -LiteralPath $Dir -Filter '*.ps1' -File | Sort-Object Name)
if ($files.Count -eq 0) { Fail 'no *.ps1 found in tests/ - the tool library vanished' }
foreach ($f in $files) {
    W ('--- ' + $f.Name + ' (' + $f.Length + ' B, ' + @([System.IO.File]::ReadAllLines($f.FullName)).Count + ' lines) ---')
    Check-File $f.FullName
}

# Cross-file invariant: gui_uia.ps1 must depend on exactly one helper, and it must be tracked.
$uia = Join-Path $Dir 'gui_uia.ps1'
if (Test-Path -LiteralPath $uia) {
    $cu = Read-Script $uia
    $tracked = @(Get-ChildItem -LiteralPath $Dir -Filter '*.ps1' -File | ForEach-Object { $_.Name })
    if ($tracked -contains 'shot_harness.ps1') { Pass 'cross-file: shot_harness.ps1 sits next to gui_uia.ps1 in tests/ (git-tracked once git add runs - see the report)' }
    else { Fail 'cross-file: tests/shot_harness.ps1 is missing - gui_uia.ps1 will throw by design, no run at all' }
    if ($cu.Code -match '(?i)tmp[\\/][^\s"''<>|]*\.ps1') {
        Fail 'cross-file: gui_uia.ps1 code still resolves a *.ps1 under a tmp/ directory'
    } else {
        Pass 'cross-file: gui_uia.ps1 resolves no *.ps1 under a tmp/ directory (rust/tmp/ is gitignored and gets wiped)'
    }
    # Fail-ShotHarness must be the ONLY thing the missing-helper guard can reach, and the
    # harness path must be a sibling of this script, never a child of the sync disk.
    if ($cu.Code -match '(?is)Fail-ShotHarness.{0,400}?\bthrow\s') {
        Pass 'cross-file: Fail-ShotHarness is terminating (throw), so no screenshot code runs without the library'
    } else {
        Fail 'cross-file: Fail-ShotHarness has no throw - the degraded path is reachable again'
    }
}

$rc = 0
if ($script:FailCount -gt 0) { $rc = 1 }
W ('SELFTEST-TOOLS files=' + $files.Count + ' pass=' + $script:PassCount + ' FAILS=' + $script:FailCount)
if ($rc -eq 0) { W 'SELFTEST-TOOLS RESULT=ok' } else { W 'SELFTEST-TOOLS RESULT=FAIL' }
exit $rc
