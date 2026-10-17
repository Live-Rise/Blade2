<#
  selftest_tools.ps1 - STATIC (no-GUI) self-check of the rust/tests tool library. Ledger #143.

  WHY THIS EXISTS
    rust/tmp/ is gitignored (.gitignore:11) and has been wiped by the sync disk twice. The
    GUI harness used to dot-source a helper from there and, when it was gone, fall back
    SILENTLY to a best-effort nudge that cannot beat the Windows foreground lock - so the
    screenshots written afterwards were frames of whatever the operator was looking at.
    Fake evidence. That whole failure mode is now forbidden by construction, and this file
    is the guard that proves it without touching a window.

  WHAT IT CHECKS (rules 1-14 are statically decidable; rule 15 EXECUTES one helper. Still: NO exe is
  started, NO window created, NO screenshot of the app taken, NO UIA, NO SendInput - the only APIs
  beyond the PS language parser and plain FILE reads are System.Drawing calls on a bitmap this gate
  paints by itself, and the system DLL export tables are parsed out of the bytes on disk, the DLLs
  are never loaded)
    1) ParseFile      : every rust/tests/*.ps1 parses with 0 syntax errors
    2) encoding       : UTF-8 BOM present and the body decodes as strict UTF-8
                        (repo-verified: PS 5.1 mis-parses a BOM-less .ps1 that carries 中文)
    3) entry points   : the functions the other scripts call really are defined (AST walk)
    4) sync-disk deps : no CODE token references a helper under a tmp path / the retired
                        library, and every load statement resolves next to $PSScriptRoot
    5) no-silent-fallback: gui_uia.ps1 must (a) load tests/shot_harness.ps1, (b) reach a
                        `throw` when it is missing, (c) contain no legacy-nudge code path
    6) native ABI     : the C# that Add-Type compiles is read as STRUCTURES and the x64 layout
                        is COMPUTED here - INPUT must total 40B, KEYBDINPUT 24B, MOUSEINPUT 32B,
                        RECT 16B, the union members must overlap at offset 8, every
                        field-bearing struct must carry [StructLayout(...)] with no Pack= /
                        [MarshalAs] drift, and a body that uses StringBuilder must import
                        System.Text (defect T-1: it did not, so nothing compiled and the round
                        ran on an unloaded library; defect T-6: INPUT measured 48B)
    7) native homes   : every DllImport is checked, per declaration, against a symbol->module
                        table, and that table is then proved from the machine's own PE export
                        tables including negative controls (PrintWindow is NOT a gdi32 export).
                        The gate counts the DllImport tokens it sees and refuses to pass a
                        declaration it cannot parse, and a symbol that is not in the table is a
                        FAIL - a new P/Invoke must be registered with a proven module, not
                        trusted because the word appeared in the file.
    8) abi contract   : shot_harness.ps1 must still own a RUNTIME assertion of the same sizes
                        (Test-ShotHarnessAbi), must run it BEFORE capturing in -SelfTest, must
                        keep the headless -AbiOnly entry point, and TapAlt must refuse to call
                        SendInput with a cbSize it did not check.
    9) coverage       : unknown *.ps1 in tests/ is still parsed + BOM-checked, so a new tool
                        cannot sneak in unverified
   10) exit-code      : the two drivers may not print RESULT=ok and exit 0 whatever happened.
       contract         Read structurally: an accumulator initialised to @( ), a marker function
                        that appends to it, the print primitive W routing every failure-prefixed
                        line through the token classifier into that marker, a pinned failure
                        prefix roster (each critical prefix also EMITTED somewhere, counted), and
                        a `if (<acc>.Count -gt 0) { $rc = 1 }` guard that sits BEFORE the green
                        banner (mother copy: shot_harness.ps1's `if ($abiFails -gt 0) { exit 1 }`).
   11) kernel default  : the DEFAULT arm of dr4-drive.ps1 must be the fake kernel (measured
       + launch lock default string length of -FakeKernelArgs > 0), the real arm must need an
                        explicit -RealKernel AND mark REAL-KERNEL= in the same branch, exactly
                        ONE place may write $env:BLADE2_KERNEL_EXE, the launch target must end
                        in .exe, and every process-launch site in the tool library is pinned by
                        file + count + line number (adding one = a FAIL that names it).
   12) find channels  : UIA comparison must read AutomationId as its own channel, ordered before
                        Name, and must count ambiguity (els.Count -gt 1) and regex fallback as
                        problems. An id-only host that never enters the tree (ReferencePalette
                        Border root) is mainline-identical, NOT a product defect - which is why
                        -TolerateAbsent exists and why its default is measured to be empty.
   13) log caliber    : the DIAG log is read as a LITERAL Ordinal substring by both drivers and
                        the second `logre:` verb is a separate, compile-checked regex path; the
                        real row head is `DIAG: DIAG: ` (push_log prefixes what the 18 call sites
                        already typed), so a line-start anchor needle is refused and counted, not
                        trusted. Plus: no hard-coded md5 may survive in a driver comment-or-code,
                        the harness health is re-read at run time, and the driver MEASUREs its own
                        INPUT size instead of asserting 40B in prose.
   14) coverage self- : this gate prints SCOPE-*=<COVERED|UNCOVERED> unconditionally and proves
       declaration       the UNCOVERED half by arithmetic (0 product-source path references,
                        0 compiled Add-Type body => no native capture reachable, 0 cargo invocation).
                        A green pass count is therefore NOT evidence about the fork's pixels; claiming
                        more than that is a FAIL by rule.
   15) crop smoke run : dr4-drive.ps1's per-element crop is RUN, not read. The gate dot-sources the
                        Crop-Dr4Rect function text out of the driver file on disk and calls it on a
                        200x120 bitmap whose every pixel is a closed form of (x,y), then asserts the
                        call did not throw, the file exists, its size matches the requested rect
                        (+-1px, stated), it is not flat-filled, its md5 differs from the frame's, its
                        corner pixels are the pixels of the requested rectangle, that degenerate /
                        throwing / missing-source crops come back as failures instead of skips, and
                        that the pre-fix `[Rectangle]::MakeRectangle` shape still throws when replayed
                        (so this rule cannot quietly stop measuring). Ledger DR6 §1: the reason this
                        exists is that a member name that does not exist parses perfectly.
   16) static member  : every System.*-qualified TYPE LITERAL in each tool script is taken out of
       reflection      the AST and asked of the RUNTIME - does the type resolve, and when the
                        literal is the left side of `::`, does that static member exist on that
                        exact type. Both shapes a grep cannot see: a member on the wrong type
                        (rule 15's MakeRectangle) and a type that is not in ANY loaded assembly
                        here at all. The second one is the worse one: dr4-drive.ps1's `inv:` leg
                        carried a fifth fallback arm on
                        [System.Windows.Automation.Legacy*Pattern(Identifiers)], PS 5.1 has no
                        such type, it throws only at CALL time, and it sat in an empty `catch { }`
                        - so it never ran, never said anything, and INVOKED= still printed green.
                        Scans BOTH AST shapes: TypeExpressionAst (the `::` side) and
                        TypeConstraintAst (a cast), because `[T]$x` is NOT a TypeExpressionAst in
                        5.1 and checking only the first finds half the bug. This gate's own file
                        is exempt from the gated scan - rule 15's negative control is a literal
                        broken line kept in a here-string, and a control must not be able to flip
                        the verdict - but rule 16 still measures and PRINTS this file's own live
                        AST so the exemption cannot hide rot here.

  USAGE
    powershell -NoProfile -ExecutionPolicy Bypass -File rust\tests\selftest_tools.ps1
    exit 0 = all green, exit 1 = at least one FAIL line.
    Add -Quiet to print only the FAIL/RESULT lines (per-file PASS spam off).
    -SelfTest is accepted as an alias of "run the gate": this file IS the self-test, there is
    no separate mode. The ledger caliber line is
    `rust/tests/selftest_tools.ps1 -SelfTest`, so that spelling must not error out (measured
    before it was added: `NamedParameterNotFound`, process code 1 - a caliber command that
    cannot run is a caliber command nobody runs).
#>
[CmdletBinding()]
param(
    [string]$Dir = '',
    [switch]$Quiet,
    [switch]$SelfTest
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
# Version stamp (repo precedent: dr4-drive.ps1 $script:Dr4HarnessVersion / 'HARNESS=' banner).
# The gate families named here are the ones this file owns; a reader of a green run can tell
# WHICH revision of the lock produced it, so a weakened lock cannot be hidden by reusing an
# older PASS count. Rule 6/7/8 landed with agent TL2, the measured banner with T2S.
$script:ToolsSelftestVersion = 'selftest_tools.ps1/5 (parse + BOM + entrypoints + no-tmp-deps + no-silent-fallback + ABI-USING/SHAPE/SIZE/OFFSET/UNION/LAYOUT/STAMP/CONTRACT + NATIVE-HOME/MODULE/ORACLE + EXIT-CODE-CONTRACT/KERNEL-DEFAULT+LAUNCH-ROSTER/FIND-CHANNEL/LOG-CALIBER+NO-STALE-CLAIM/COVERAGE-SELF-DECL + CROP-SMOKE-RUN (rule 15, headless execution of Crop-Dr4Rect) + STATIC-MEMBER-REFLECTION (rule 16, AST + reflection over every System.* type literal) + coverage)'

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
    'dr4-drive.ps1'   = @('W', 'Mark-Dr4Problem', 'Test-Dr4FailureToken', 'Test-Dr4Tolerated', 'Find-El',
                          'Wait-Log', 'Get-Dr4Stats', 'Show-Dr4LogHits', 'Test-Dr4AnchoredNeedle', 'Get-Dr4LogLines',
                          # T4K rule 15: the per-element crop is ONE callable helper so a headless gate can
                          # execute the very code the driver runs (Check-CropSmokeRun takes its text from disk).
                          'Crop-Dr4Rect', 'Take-ElShot',
                          # T5K rule 16: inv: must be ONE callable helper that returns a status table, so a
                          # headless gate can grade which arms it can actually bind. Inline in the step loop
                          # it was only reachable behind a live UIA element - which is how the silent
                          # LegacyIAccessible arm survived.
                          'Invoke-El')
    'gui_uia.ps1'     = @('W', 'Mark-UiaProblem', 'Test-UiaFailureToken', 'Test-UiaTolerated', 'Find-ByName',
                          'Fail-ShotHarness', 'Raise-Foreground-For-Shot', 'Raise-For-Shot', 'Drop-After-Shot',
                          'Find-HwndByTitle', 'Stop-ProcTree', 'Get-AppExitCode', 'Get-LogHit', 'Get-LogCount')
    'shot_harness.ps1'  = @('Invoke-ShotForeground', 'Invoke-QaShot', 'Get-QaShotStats', 'Reset-ShotRound', 'Get-ShotNoteTail', 'Write-ShotLog', 'Get-ShotPathFromTag', 'Test-ShotHarnessAbi', 'Get-ShotHarnessStamp')
    'gui_probe.ps1'     = @('Find-Windows')
    'selftest_tools.ps1' = @('Read-Script', 'Check-File', 'Check-ExitCodeContract', 'Check-KernelDefault',
                             'Check-FindChannels', 'Check-LogCaliber', 'Check-ProcessLaunchRoster',
                             'Check-CoverageSelfDecl', 'Get-FnWindow', 'Lit',
                             # T5K rule 16 - the verifier itself has to be an entry point, or rule 3 cannot
                             # tell a deleted rule from a renamed one.
                             'Check-StaticMemberReflection', 'Measure-StaticMembers', 'Get-LocalTypeDefNames')
}

# ---------------------------------------------------------------- native ABI truth tables (rule 6)
# What the gate is FOR: the last two rounds were green while the code was wrong, because a
# text gate only proves a word is present. Everything below is ARITHMETIC on the declared
# shape, so "INPUT is 40 bytes" is a consequence of the field list instead of a string the
# gate hopes to find. Sizes/alignments are the x64 P/Invoke rules: a marshalled `bool` is
# the 4-byte Win32 BOOL, IntPtr is 8, and a structure aligns to its widest member.
$script:PrimSize = @{
    'byte' = 1; 'sbyte' = 1; 'bool' = 4; 'char' = 2; 'short' = 2; 'ushort' = 2
    'int' = 4; 'uint' = 4; 'float' = 4; 'long' = 8; 'ulong' = 8; 'double' = 8
    'IntPtr' = 8; 'UIntPtr' = 8
}
# The expected field shapes of the structures that feed SendInput (winuser.h). The gate
# recomputes the size from these lists and requires the total it documents.
$script:AbiShape = @{
    'KEYBDINPUT' = @{ layout = 'Sequential'; fields = @(
            @{ name = 'wVk'; type = 'ushort' }, @{ name = 'wScan'; type = 'ushort' },
            @{ name = 'dwFlags'; type = 'uint' }, @{ name = 'time'; type = 'uint' },
            @{ name = 'dwExtraInfo'; type = 'IntPtr' }) }
    'MOUSEINPUT' = @{ layout = 'Sequential'; fields = @(
            @{ name = 'dx'; type = 'int' }, @{ name = 'dy'; type = 'int' },
            @{ name = 'mouseData'; type = 'uint' }, @{ name = 'dwFlags'; type = 'uint' },
            @{ name = 'time'; type = 'uint' }, @{ name = 'dwExtraInfo'; type = 'IntPtr' }) }
    # type at 0 and BOTH union members at 8: that is what makes the x64 total 40 and what the
    # dead `ulong padding` variant (union at 16, total 48) breaks.
    'INPUT' = @{ layout = 'Explicit'; fields = @(
            @{ name = 'type'; type = 'uint'; off = 0 },
            @{ name = 'ki'; type = 'KEYBDINPUT'; off = 8 },
            @{ name = 'mi'; type = 'MOUSEINPUT'; off = 8 }) }
}
$script:AbiTotal = @{ 'INPUT' = 40; 'KEYBDINPUT' = 24; 'MOUSEINPUT' = 32; 'RECT' = 16 }

# ---------------------------------------------------------------- P/Invoke module truth (rule 7)
# symbol -> the DLL that really exports it, and the gate does NOT take this table on faith:
# Check-NativeOracle reads the PE export tables of those DLLs as data and proves every row,
# including the negative controls in $script:NotHome below. An "import library typo" is the
# worst class here: Add-Type compiles it, PowerShell only fails at call time - PrintWindow
# against gdi32 gave a whole round of screenshots nobody could trust (ledger DR5).
$script:NativeHome = @{
    'EnumWindows' = 'user32.dll'; 'IsWindowVisible' = 'user32.dll'
    'GetWindowText' = 'user32.dll'; 'GetWindowTextLength' = 'user32.dll'
    'GetWindowThreadProcessId' = 'user32.dll'; 'GetWindowRect' = 'user32.dll'
    'GetClientRect' = 'user32.dll'; 'GetForegroundWindow' = 'user32.dll'
    'SetForegroundWindow' = 'user32.dll'; 'BringWindowToTop' = 'user32.dll'
    'ShowWindow' = 'user32.dll'; 'IsIconic' = 'user32.dll'; 'SetWindowPos' = 'user32.dll'
    'SwitchToThisWindow' = 'user32.dll'; 'LockSetForegroundWindow' = 'user32.dll'
    'AllowSetForegroundWindow' = 'user32.dll'; 'AttachThreadInput' = 'user32.dll'
    'SetProcessDpiAwarenessContext' = 'user32.dll'; 'SetProcessDPIAware' = 'user32.dll'
    'FindWindow' = 'user32.dll'; 'PrintWindow' = 'user32.dll'
    'SendInput' = 'user32.dll'; 'GetCursorPos' = 'user32.dll'; 'SetCursorPos' = 'user32.dll'
    'mouse_event' = 'user32.dll'; 'GetSystemMetrics' = 'user32.dll'
    'SetProcessDpiAwareness' = 'shcore.dll'
    'GetCurrentThreadId' = 'kernel32.dll'
}
# Negative controls: if one of these ever RESOLVES, the oracle is broken (or the platform
# moved) and the green gate above it means nothing. This is what keeps rule 7 honest.
$script:NotHome = @{
    'PrintWindow' = 'gdi32.dll'; 'SendInput' = 'gdi32.dll'
    'SetProcessDpiAwareness' = 'user32.dll'; 'BitBlt' = 'user32.dll'
    'CreateCompatibleDC' = 'user32.dll'; 'CreateDIBSection' = 'user32.dll'
    'GetCurrentThreadId' = 'user32.dll'
}
# The GDI half of a capture core, for whoever adds it next (declared nowhere today).
$script:GdiHome = @{
    'BitBlt' = 'gdi32.dll'; 'CreateCompatibleDC' = 'gdi32.dll'; 'CreateCompatibleBitmap' = 'gdi32.dll'
    'CreateDIBSection' = 'gdi32.dll'; 'SelectObject' = 'gdi32.dll'; 'DeleteObject' = 'gdi32.dll'
    'DeleteDC' = 'gdi32.dll'; 'GetDeviceCaps' = 'gdi32.dll'
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

# ---------------------------------------------------------------- C# reading (rules 6 + 7)
# The C# that Add-Type compiles lives inside a here-string, which PowerShell's tokenizer
# keeps as ONE code token. So these helpers work on code only: a PS comment cannot feed them
# (rule 4's lesson) and, inside the bodies, Remove-CStyleComments strips the C# comments so
# a commented-out `[DllImport("gdi32.dll")] PrintWindow` can neither fake a passing line nor
# hide a real one. This is the hole that let the DR5 gate be satisfied by words: here the
# gate reads the shape and does the arithmetic.
function Get-TypeDefinitionBodies($Ast) {
    $bodies = @()
    $cmds = @($Ast.FindAll( { param($n) $n -is [System.Management.Automation.Language.CommandAst] }, $true))
    foreach ($c in $cmds) {
        $cn = ''
        try { $cn = [string]$c.GetCommandName() } catch { $cn = '' }
        if ($cn -ne 'Add-Type') { continue }
        $els = @($c.CommandElements)
        for ($i = 0; $i -lt $els.Count; $i++) {
            if ($els[$i] -isnot [System.Management.Automation.Language.CommandParameterAst]) { continue }
            if ([string]$els[$i].ParameterName -ne 'TypeDefinition') { continue }
            $txt = ''
            if ($null -ne $els[$i].Argument) { $txt = [string]$els[$i].Argument.Extent.Text }
            elseif (($i + 1) -lt $els.Count) { $txt = [string]$els[$i + 1].Extent.Text }
            $txt = [regex]::Replace($txt, "(?s)^\s*@'?\r?\n", '')
            $txt = [regex]::Replace($txt, "\r?\n'@\s*$", '')
            $bodies += $txt
        }
    }
    return , $bodies
}

function Remove-CStyleComments([string]$Src) {
    $sb = New-Object System.Text.StringBuilder
    $i = 0
    $n = $Src.Length
    while ($i -lt $n) {
        $c = $Src[$i]
        if ($c -eq '"' -or $c -eq "'") {
            # a string/char literal is copied verbatim: `//` inside it is not a comment
            [void]$sb.Append($c); $i++
            while ($i -lt $n) {
                $d = $Src[$i]; [void]$sb.Append($d); $i++
                if ($d -eq '\') { if ($i -lt $n) { [void]$sb.Append($Src[$i]); $i++ }; continue }
                if ($d -eq $c) { break }
            }
            continue
        }
        if ($c -eq '/' -and ($i + 1) -lt $n -and $Src[$i + 1] -eq '/') {
            while ($i -lt $n -and $Src[$i] -ne "`n") { $i++ }
            continue
        }
        if ($c -eq '/' -and ($i + 1) -lt $n -and $Src[$i + 1] -eq '*') {
            $i += 2
            while (($i + 1) -lt $n -and -not ($Src[$i] -eq '*' -and $Src[$i + 1] -eq '/')) { $i++ }
            $i += 2
            continue
        }
        [void]$sb.Append($c); $i++
    }
    return $sb.ToString()
}

# struct NAME { ... } with brace-matched bodies (nested structs are their own entries, which
# is what RECT-inside-WndEnum needs).
function Get-CStructs([string]$Src) {
    $out = @()
    foreach ($m in [regex]::Matches($Src, '(?s)\bstruct\s+([A-Za-z_]\w*)\s*\{')) {
        $open = $m.Index + $m.Length - 1
        $depth = 0
        $close = -1
        for ($i = $open; $i -lt $Src.Length; $i++) {
            if ($Src[$i] -eq '{') { $depth++ }
            elseif ($Src[$i] -eq '}') { $depth--; if ($depth -eq 0) { $close = $i; break } }
        }
        if ($close -lt 0) {
            $out += [pscustomobject]@{ Name = $m.Groups[1].Value; Body = ''; Decl = ''; Unclosed = $true }
            continue
        }
        $winStart = [Math]::Max(0, $m.Index - 400)
        $win = $Src.Substring($winStart, $m.Index - $winStart)
        $cut = [Math]::Max([Math]::Max($win.LastIndexOf(';'), $win.LastIndexOf('{')), $win.LastIndexOf('}'))
        $decl = if ($cut -ge 0) { $win.Substring($cut + 1) } else { $win }
        $out += [pscustomobject]@{
            Name     = $m.Groups[1].Value
            Body     = $Src.Substring($open + 1, $close - $open - 1)
            Decl     = $decl
            Unclosed = $false
        }
    }
    return , $out
}

# The instance fields of one struct body. Methods, delegates, nested declarations and consts
# are dropped: they carry no instance bytes.
function Get-CStructFields([string]$Body) {
    $fields = @()
    $odd = @()
    $depth = 0
    $chunk = New-Object System.Text.StringBuilder
    foreach ($ch in $Body.ToCharArray()) {
        if ($ch -eq '{') { $depth++; [void]$chunk.Append($ch); continue }
        if ($ch -eq '}') { $depth--; [void]$chunk.Append($ch); continue }
        if ($ch -eq ';' -and $depth -eq 0) {
            $s = $chunk.ToString().Trim()
            [void]$chunk.Clear()
            if ($s -eq '') { continue }
            if ($depth -ne 0) { continue }
            if ($s -match '[{}]') { continue }                      # nested struct / method body
            if ($s -match '(?i)\bconst\b') { continue }              # no instance storage
            $stripped = [regex]::Replace($s, '\[[^\]]*\]', ' ')
            if ($stripped -match '\(') { continue }                   # method / delegate
            $stripped = ($stripped -replace '(?i)\b(public|private|protected|internal|static|readonly|volatile|extern)\b', ' ') -replace '\s+', ' '
            $stripped = $stripped.Trim()
            $fm = [regex]::Match($stripped, '^([A-Za-z_]\w*)(?:\s*(?:<[^>]*>)?(\[[^\]]*\])?)\s+([A-Za-z_]\w*(?:\s*,\s*[A-Za-z_]\w*)*)$')
            if (-not $fm.Success) {
                $odd += $stripped
                continue
            }
            $offM = [regex]::Match($s, '\[FieldOffset\(\s*(\d+)\s*\)\]')
            $marshal = ([regex]::Matches($s, '\[MarshalAs\b')).Count
            foreach ($nm in ($fm.Groups[3].Value -split '\s*,\s*')) {
                $fields += [pscustomobject]@{
                    Type    = $fm.Groups[1].Value
                    Name    = $nm.Trim()
                    Offset  = $(if ($offM.Success) { [int]$offM.Groups[1].Value } else { $null })
                    MarshalAsCount = $marshal
                }
            }
            continue
        }
        [void]$chunk.Append($ch)
    }
    return [pscustomobject]@{ Fields = $fields; Unparsed = $odd }
}

# x64 native layout of a struct, computed the way the marshaler does it. Returns
# @{ Size; Align; Offsets(hashtable name->offset); Problems(array) }.
function Get-CStructSize($St, $ByName, [hashtable]$Memo) {
    if ($Memo.ContainsKey($St.Name)) { return $Memo[$St.Name] }
    $res = [pscustomobject]@{ Size = 0; Align = 1; Offsets = @{}; Problems = @() }
    $Memo[$St.Name] = $res
    if ($St.Unclosed) { $res.Problems += 'unmatched brace in the struct body'; return $res }
    $parsed = Get-CStructFields $St.Body
    foreach ($u in $parsed.Unparsed) { $res.Problems += ('field declaration not understood: ' + ($u -replace '\s+', ' ')) }
    if ($parsed.Fields.Count -eq 0) {
        $res.Problems = @()
        $res.Size = 0
        return $res
    }
    $explicit = ($St.Decl -match 'LayoutKind\.Explicit')
    $sequential = ($St.Decl -match 'LayoutKind\.Sequential')
    if (-not ($explicit -or $sequential)) {
        $res.Problems += ('no [StructLayout(LayoutKind...)] on struct ' + $St.Name + ' (the size then follows the compiler default, which is a portability trap)')
    }
    $packM = [regex]::Match($St.Decl, 'Pack\s*=\s*(\d+)')
    if ($packM.Success) { $res.Problems += ('Pack=' + $packM.Groups[1].Value + ' on struct ' + $St.Name + ' (an explicit pack makes the size depend on how it was compiled)') }
    if ($explicit -and $sequential) { $res.Problems += ('struct ' + $St.Name + ' claims both Sequential and Explicit') }
    $cur = 0
    $maxEnd = 0
    foreach ($f in $parsed.Fields) {
        $fs = 0
        $fa = 0
        if ($script:PrimSize.ContainsKey($f.Type)) {
            $fs = [int]$script:PrimSize[$f.Type]
            $fa = $fs
        }
        elseif ($ByName.ContainsKey($f.Type)) {
            $sub = Get-CStructSize $ByName[$f.Type] $ByName $Memo
            $fs = [int]$sub.Size
            $fa = [int]$sub.Align
            foreach ($p in $sub.Problems) { $res.Problems += ($St.Name + ' <- ' + $f.Type + ': ' + $p) }
        }
        else {
            $res.Problems += ('field type ' + $f.Type + ' of ' + $St.Name + '.' + $f.Name + ' is not a primitive with a fixed marshal size and not a struct declared in the same body')
            continue
        }
        if ($fs -le 0) { $res.Problems += ('field ' + $St.Name + '.' + $f.Name + ' has a zero/unknown size'); continue }
        if ($f.MarshalAsCount -gt 0) { $res.Problems += ('[MarshalAs] on ' + $St.Name + '.' + $f.Name + ' overrides the computed layout (size no longer follows the field list)') }
        if ($explicit) {
            if ($null -eq $f.Offset) { $res.Problems += ('LayoutKind.Explicit field ' + $St.Name + '.' + $f.Name + ' has no [FieldOffset]'); continue }
            $off = [int]$f.Offset
        }
        else {
            if ($null -ne $f.Offset) { $res.Problems += ('[FieldOffset] on ' + $St.Name + '.' + $f.Name + ' while the layout is not Explicit'); continue }
            if ($fa -gt 0) { $off = [int]([Math]::Ceiling($cur / $fa) * $fa) } else { $off = $cur }
            $cur = $off
        }
        $res.Offsets[$f.Name] = $off
        $end = $off + $fs
        if ($end -gt $maxEnd) { $maxEnd = $end }
        if ($fa -gt $res.Align) { $res.Align = $fa }
        $cur = $off + $fs
    }
    $res.Size = [int]([Math]::Ceiling($maxEnd / $res.Align) * $res.Align)
    return $res
}

function Get-CStructInfo($ByName, [string]$Name) {
    # size/offsets for one struct by name, memo fresh per call so callers stay independent
    if (-not $ByName.ContainsKey($Name)) { return $null }
    $memo = @{}
    $r = Get-CStructSize $ByName[$Name] $ByName $memo
    $fields = (Get-CStructFields $ByName[$Name].Body).Fields | ForEach-Object { $_.Name }
    $sorted = @($fields | Sort-Object)
    return [pscustomobject]@{ Size = [int]$r.Size; Offsets = $r.Offsets; Problems = @($r.Problems); Fields = ($sorted -join ',') }
}

# The expected field list of a pinned struct, from $script:AbiShape, in the same sorted,
# comma-joined form Get-CStructInfo reports.
function Format-AbiFieldList($Name) {
    return (($script:AbiShape[$Name].fields | ForEach-Object { $_.name } | Sort-Object) -join ',')
}

function Check-CSharpAbi([string]$Name, [string]$Body) {
    # rule 6 - the injection structures, computed rather than grepped
    $clean = Remove-CStyleComments $Body
    $structs = Get-CStructs $clean
    $byName = @{}
    foreach ($s in $structs) { if (-not $byName.ContainsKey($s.Name)) { $byName[$s.Name] = $s } }

    $usesSb = ($clean -match '(?<![\w.])StringBuilder')
    $hasUsingText = ($clean -match "(?m)^\s*using\s+System\.Text\s*;")
    if ($usesSb -and -not $hasUsingText) {
        Fail ($Name + ' ABI-USING body uses StringBuilder without "using System.Text;" - Add-Type fails with CS0246, the type never loads and EVERY public function is missing (defect T-1: a whole round of screenshots behind a gate that only looked at words)')
    } elseif ($usesSb) {
        Pass ($Name + ' ABI-USING StringBuilder has its "using System.Text;"')
    } else {
        Pass ($Name + ' ABI-USING no unqualified System.Text type used')
    }

    if (-not $byName.ContainsKey('INPUT')) {
        Pass ($Name + ' ABI-INPUT n/a (no INPUT struct declared here)')
        return
    }
    foreach ($pin in @('INPUT', 'KEYBDINPUT', 'MOUSEINPUT')) {
        if (-not $byName.ContainsKey($pin)) {
            Fail ($Name + ' ABI-INPUT declares INPUT but not ' + $pin + ' - the union member is missing, so the shape cannot be 40B')
            continue
        }
        $info = Get-CStructInfo $byName $pin
        $wantTotal = [int]$script:AbiTotal[$pin]
        if ($info.Problems.Count -gt 0) {
            Fail ($Name + ' ABI-SHAPE struct=' + $pin + ' shape is not pinned-computable: ' + (($info.Problems | Select-Object -First 3) -join ' | '))
        }
        $expFields = Format-AbiFieldList $pin
        if ($info.Fields -ne $expFields) {
            Fail ($Name + ' ABI-SHAPE struct=' + $pin + ' fields=[' + $info.Fields + '] expected=[' + $expFields +
                  '] - the field list is the contract; a padding/union field here is defect T-6')
        }
        if ($info.Size -ne $wantTotal) {
            Fail ($Name + ' ABI-SIZE struct=' + $pin + ' computed=' + $info.Size + 'B documented=' + $wantTotal +
                  'B (x64) - SendInput takes cbSize from this struct, so a wrong total injects 0 events and still returns success')
        } else {
            Pass ($Name + ' ABI-SIZE struct=' + $pin + ' computed=' + $info.Size + 'B == documented x64 total')
        }
        foreach ($fe in $script:AbiShape[$pin].fields) {
            if (-not $info.Offsets.ContainsKey($fe.name)) { continue }
            $got = [int]$info.Offsets[$fe.name]
            if ($fe.ContainsKey('off') -and $got -ne [int]$fe.off) {
                Fail ($Name + ' ABI-OFFSET struct=' + $pin + ' field=' + $fe.name + ' offset=' + $got + ' expected=' + $fe.off)
            } elseif (-not $fe.ContainsKey('off')) {
                Pass ($Name + ' ABI-OFFSET struct=' + $pin + ' field=' + $fe.name + ' at ' + $got + ' (sequential, size=' + $info.Size + ')')
            } else {
                Pass ($Name + ' ABI-OFFSET struct=' + $pin + ' field=' + $fe.name + ' at ' + $got + ' == pinned')
            }
        }
    }
    # the union must overlap: if ki and mi sit at different offsets the struct is a pair, not a union
    $ii = Get-CStructInfo $byName 'INPUT'
    if ($null -ne $ii -and $ii.Offsets.ContainsKey('ki') -and $ii.Offsets.ContainsKey('mi')) {
        if ([int]$ii.Offsets['ki'] -eq [int]$ii.Offsets['mi']) {
            Pass ($Name + ' ABI-UNION INPUT.ki and INPUT.mi overlap at ' + [int]$ii.Offsets['ki'] + ' (a real union, total ' + $ii.Size + 'B)')
        } else {
            Fail ($Name + ' ABI-UNION INPUT.ki@' + [int]$ii.Offsets['ki'] + ' and INPUT.mi@' + [int]$ii.Offsets['mi'] +
                  ' do not overlap - that is a struct pair, not the INPUT union, and cbSize is wrong for both kinds')
        }
    }
    if ($byName.ContainsKey('RECT')) {
        $ri = Get-CStructInfo $byName 'RECT'
        if ($ri.Problems.Count -eq 0 -and $ri.Size -eq [int]$script:AbiTotal['RECT']) {
            Pass ($Name + ' ABI-SIZE struct=RECT computed=' + $ri.Size + 'B == documented total')
        } else {
            Fail ($Name + ' ABI-SIZE struct=RECT computed=' + $ri.Size + 'B expected=' + $script:AbiTotal['RECT'] +
                  'B (window geometry feeds the bitmap size; a wrong RECT sizes the frame wrong)' +
                  $(if ($ri.Problems.Count -gt 0) { ' problems: ' + (($ri.Problems | Select-Object -First 2) -join ' | ') } else { '' }))
        }
    }
    # every field-bearing struct must pin its own layout, or the size drifts by compiler default
    foreach ($s in $structs) {
        $p = Get-CStructFields $s.Body
        if ($p.Fields.Count -eq 0) { continue }
        if ($s.Decl -notmatch '\[StructLayout\(') {
            Fail ($Name + ' ABI-LAYOUT struct=' + $s.Name + ' has ' + $p.Fields.Count +
                  ' instance field(s) but no [StructLayout(...)] - the native size then follows the compiler default and can move between builds')
        }
    }
    $noLayout = @($structs | Where-Object { (Get-CStructFields $_.Body).Fields.Count -gt 0 -and $_.Decl -notmatch '\[StructLayout\(' })
    if ($noLayout.Count -eq 0) {
        Pass ($Name + ' ABI-LAYOUT all ' + $structs.Count + ' field-bearing struct declaration(s) carry [StructLayout(...)]')
    }
}

function Check-CSharpDllImports([string]$Name, [string]$Body) {
    # rule 7 - which module each P/Invoke says it lives in, per declaration, comments removed
    $clean = Remove-CStyleComments $Body
    $declCount = ([regex]::Matches($clean, '\[DllImport\s*\(')).Count
    if ($declCount -eq 0) { Pass ($Name + ' NATIVE-HOME n/a (no DllImport declaration in this body)'); return }
    $rx = '\[DllImport\s*\(\s*"([^"]+)"([^)]*)\)\]\s*(?:(?:public|private|internal|static|extern|unsafe|\s)+)*[\w\.<>\[\]]+\s+(\w+)\s*\('
    $ms = @([regex]::Matches($clean, $rx))
    if ($ms.Count -ne $declCount) {
        Fail ($Name + ' NATIVE-HOME sees ' + $declCount + ' DllImport declaration(s) but could only parse ' + $ms.Count +
              ' - a P/Invoke the gate cannot read is a P/Invoke the gate cannot vouch for (put the attribute and the signature on one parseable line)')
        return
    }
    $bad = 0
    $unknown = @()
    foreach ($m in $ms) {
        $mod = $m.Groups[1].Value
        $attrs = $m.Groups[2].Value
        $sym = $m.Groups[3].Value
        $ep = $sym
        $epm = [regex]::Match($attrs, 'EntryPoint\s*=\s*"([^"]+)"')
        if ($epm.Success) { $ep = $epm.Groups[1].Value }
        if (-not $script:NativeHome.ContainsKey($ep)) {
            $unknown += ($ep + '(' + $mod + ')')
            continue
        }
        $want = [string]$script:NativeHome[$ep]
        if ($mod -ne $want) {
            $bad++
            Fail ($Name + ' NATIVE-HOME symbol=' + $ep + ' declared-against=' + $mod + ' true-module=' + $want +
                  ' - Add-Type compiles this anyway and it only fails at call time (or quietly returns 0). That is the defect that made every DR5 frame untrustworthy.')
        }
    }
    if ($unknown.Count -gt 0) {
        Fail ($Name + ' NATIVE-HOME unregistered symbol(s): ' + ($unknown -join ', ') +
              ' - add each to $script:NativeHome with the module proven from the machine PE export tables; a new P/Invoke may not enter the toolchain unverified')
    }
    if ($bad -eq 0 -and $unknown.Count -eq 0) {
        Pass ($Name + ' NATIVE-HOME all ' + $ms.Count + ' DllImport declaration(s) name the module that really exports the symbol')
    }
    # a module name that is not a file in System32 is a DllNotFoundException at first call
    $mods = @($ms | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    $missing = @()
    foreach ($d in $mods) {
        $p = Join-Path $env:WINDIR ('System32\' + $d)
        if (-not (Test-Path -LiteralPath $p)) { $missing += $d }
    }
    if ($missing.Count -gt 0) {
        Fail ($Name + ' NATIVE-MODULE not present in System32: ' + ($missing -join ', ') + ' - every call would throw DllNotFoundException')
    } else {
        Pass ($Name + ' NATIVE-MODULE ' + $mods.Count + ' module(s) named by DllImport all exist in System32 (' + ($mods -join ', ') + ')')
    }
}

function Get-PeExportNames([string]$DllName) {
    # reads the export table out of the DLL FILE - no LoadLibrary, no process, no window
    $r = [pscustomobject]@{ Ok = $false; Why = ''; Names = @() }
    $p = Join-Path $env:WINDIR ('System32\' + $DllName)
    if (-not (Test-Path -LiteralPath $p)) { $r.Why = 'no such file: ' + $p; return $r }
    try {
        $bytes = [System.IO.File]::ReadAllBytes($p)
        if ($bytes.Length -lt 0x400 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { $r.Why = 'not a PE (MZ) file'; return $r }
        $lf = [BitConverter]::ToInt32($bytes, 0x3C)
        if ($bytes[$lf] -ne 0x50 -or $bytes[$lf + 1] -ne 0x45) { $r.Why = 'no PE signature'; return $r }
        $opt = $lf + 24
        $magic = [BitConverter]::ToUInt16($bytes, $opt)
        $dd = if ($magic -eq 0x20b) { $opt + 112 } else { $opt + 96 }
        $expRva = [BitConverter]::ToInt32($bytes, $dd)
        $numSec = [BitConverter]::ToUInt16($bytes, $lf + 6)
        $secOff = $opt + [BitConverter]::ToUInt16($bytes, $lf + 20)
        $sections = @()
        for ($i = 0; $i -lt $numSec; $i++) {
            $o = $secOff + ($i * 40)
            $sections += [pscustomobject]@{
                VA   = [BitConverter]::ToInt32($bytes, $o + 12)
                VSz  = [BitConverter]::ToInt32($bytes, $o + 8)
                Raw  = [BitConverter]::ToInt32($bytes, $o + 20)
            }
        }
        if ($expRva -eq 0) { $r.Why = 'no export directory'; return $r }
        # RVA -> file offset over this file's own section table
        $toOffset = {
            param([int]$rva, $secs)
            foreach ($s in $secs) {
                if ($rva -ge [int]$s.VA -and $rva -lt ([int]$s.VA + [Math]::Max([int]$s.VSz, 1))) {
                    return [int]$s.Raw + ($rva - [int]$s.VA)
                }
            }
            throw ('RVA ' + $rva + ' maps to no section')
        }
        $e = & $toOffset $expRva $sections
        $numNames = [BitConverter]::ToInt32($bytes, $e + 24)
        $nOff = & $toOffset ([BitConverter]::ToInt32($bytes, $e + 32)) $sections
        $names = @()
        for ($i = 0; $i -lt $numNames; $i++) {
            $so = & $toOffset ([BitConverter]::ToInt32($bytes, $nOff + ($i * 4))) $sections
            $sb = New-Object System.Text.StringBuilder
            while ($so -lt $bytes.Length -and $bytes[$so] -ne 0) { [void]$sb.Append([char]$bytes[$so]); $so++ }
            $names += $sb.ToString()
        }        $r.Ok = $true
        $r.Names = $names
        return $r
    } catch {
        $r.Why = 'parse threw: ' + (($_.Exception.Message -replace '\s+', ' '))
        return $r
    }
}

function Test-NativeEntry([hashtable]$Cache, [string]$DllName, [string]$Symbol, [string]$CharSet) {
    # mirrors what the CLR does with ExactSpelling=false: the name, then name+W (Unicode)
    # or name+A (Ansi/default). GetWindowText is only ever exported as GetWindowTextW - a
    # gate that ignored this would call a correct declaration wrong.
    if (-not $Cache.ContainsKey($DllName)) { $Cache[$DllName] = Get-PeExportNames $DllName }
    $pe = $Cache[$DllName]
    if (-not $pe.Ok) { return [pscustomobject]@{ Found = $false; Via = ''; Why = ($DllName + ': ' + $pe.Why) } }
    if ($pe.Names -contains $Symbol) { return [pscustomobject]@{ Found = $true; Via = $Symbol; Why = '' } }
    $sfx = if ($CharSet -eq 'Unicode') { 'W' } else { 'A' }
    if ($pe.Names -contains ($Symbol + $sfx)) { return [pscustomobject]@{ Found = $true; Via = ($Symbol + $sfx); Why = '' } }
    $alt = if ($sfx -eq 'W') { 'A' } else { 'W' }
    if ($pe.Names -contains ($Symbol + $alt)) {
        return [pscustomobject]@{ Found = $false; Via = ($Symbol + $alt); Why = ($DllName + ' exports ' + $Symbol + $alt + ' but the declaration asks for ' + $sfx + ' (CharSet mismatch)') }
    }
    return [pscustomobject]@{ Found = $false; Via = ''; Why = ($DllName + ' exports neither ' + $Symbol + ' nor ' + $Symbol + $sfx) }
}

function Check-NativeOracle {
    # rule 7b - prove $script:NativeHome from the machine, then prove the negative controls.
    # Without this the table is one agent's memory, and memory is what DR5 got wrong.
    $cache = @{}
    $modules = @($script:NativeHome.Values) + @($script:NotHome.Values) + @($script:GdiHome.Values)
    $modules = @($modules | Sort-Object -Unique)
    $unreadable = @()
    foreach ($d in $modules) {
        if (-not $cache.ContainsKey($d)) { $cache[$d] = Get-PeExportNames $d }
        if (-not $cache[$d].Ok) { $unreadable += ($d + ' (' + $cache[$d].Why + ')') }
    }
    if ($unreadable.Count -gt 0) {
        Fail ('NATIVE-ORACLE cannot read the export tables of: ' + ($unreadable -join ', ') + ' - the module truth table is unverified, so rule 7 proves nothing this run')
        return
    }
    $bad = 0
    foreach ($sym in ($script:NativeHome.Keys | Sort-Object)) {
        $mod = [string]$script:NativeHome[$sym]
        # the table only claims WHICH module owns the symbol, so either mangling counts
        $res = Test-NativeEntry $cache $mod $sym 'Ansi'
        if (-not $res.Found) { $res = Test-NativeEntry $cache $mod $sym 'Unicode' }
        if (-not $res.Found) {
            $bad++
            Fail ('NATIVE-ORACLE table row ' + $sym + ' -> ' + $mod + ' does not resolve: ' + $res.Why)
        }
    }
    if ($bad -eq 0) {
        Pass ('NATIVE-ORACLE all ' + $script:NativeHome.Count + ' table row(s) resolve in their named module, read from the PE export tables of ' + ($modules -join ', '))
    }
    $gdi = 0
    foreach ($sym in ($script:GdiHome.Keys | Sort-Object)) {
        $res = Test-NativeEntry $cache ([string]$script:GdiHome[$sym]) $sym 'Ansi'
        if (-not $res.Found) { $gdi++; Fail ('NATIVE-ORACLE gdi row ' + $sym + ' -> ' + $script:GdiHome[$sym] + ' does not resolve: ' + $res.Why) }
    }
    if ($gdi -eq 0) { Pass ('NATIVE-ORACLE all ' + $script:GdiHome.Count + ' gdi32 row(s) (the capture core waiting to be added) resolve against gdi32.dll, not user32.dll') }
    $neg = 0
    foreach ($sym in ($script:NotHome.Keys | Sort-Object)) {
        $wrong = [string]$script:NotHome[$sym]
        $res = Test-NativeEntry $cache $wrong $sym 'Ansi'
        if ($res.Found) {
            $neg++
            Fail ('NATIVE-ORACLE negative control broke: ' + $sym + ' resolves from ' + $wrong + ' (via ' + $res.Via +
                  ') - the oracle is not distinguishing modules any more, so treat every NATIVE-HOME PASS as unproven')
        }
    }
    if ($neg -eq 0) {
        Pass ('NATIVE-ORACLE ' + $script:NotHome.Count + ' negative control(s) all fail to resolve as they must (PrintWindow is NOT a gdi32 export - the DR5 disease, proven from the file)')
    }
}

function Get-ExitOperand([string]$Text, [int]$From, [int]$Len) {
    # First `exit <operand>` inside Text[From .. From+Len). Returns '', '0', '1', '$abiRc' ...
    # 'exit' with no operand counts as '' (cannot be shown to be non-zero).
    $t = $Text
    if ($From -ge $t.Length) { return '' }
    $end = $t.Length
    if ($Len -gt 0 -and ($From + $Len) -lt $end) { $end = $From + $Len }
    $m = [regex]::Match($t.Substring($From, $end - $From), '\bexit\s+(\$\w+|\d+)')
    if (-not $m.Success) { return '' }
    return $m.Groups[1].Value
}

function Check-ShotHarnessAbiContract($R) {
    # rule 8 - the runtime half of the size gate must exist, must run BEFORE any capture, and
    # must be able to end the run non-zero. A static gate alone can be satisfied by a shape
    # the compiler disagrees with; a runtime assert alone can be deleted. Pin both.
    $name = 'shot_harness.ps1'
    $code = $R.Code
    if ($R.Fns -contains 'Test-ShotHarnessAbi') {
        Pass ($name + ' ABI-CONTRACT Test-ShotHarnessAbi is defined (the marshaler, not grep, judges the sizes)')
    } else {
        Fail ($name + ' ABI-CONTRACT Test-ShotHarnessAbi is gone - the injection sizes would again be judged by text only')
        return
    }
    # NOTE: $R.Code is a TOKEN stream, so `[Type]::Member()` arrives as
    # `[Type] :: Member ( )` - every pattern here is whitespace-tolerant for exactly that
    # reason. A literal IndexOf would miss code that is present, which is the safe direction,
    # but it would also let a reflowed line escape a check it should have run.
    if ($code -match '\[\s*ShotHarness32\s*\]\s*::\s*AbiInputSize') {
        Pass ($name + ' ABI-CONTRACT reads the real INPUT size off the marshaler (AbiInputSize)')
    } else {
        Fail ($name + ' ABI-CONTRACT no AbiInputSize call site - the assertion would be a hardcoded number wearing its own name')
    }
    # 'SELFTEST-ABI' as a WORD proves nothing (this gate's own strings contain it). Pin the
    # SHAPE of the red line instead: it must name the structure it is rejecting.
    if ($code -match 'SELFTEST-ABI FAIL structure=') {
        Pass ($name + ' ABI-CONTRACT the red line names the structure (SELFTEST-ABI FAIL structure=...)')
    } else {
        Fail ($name + ' ABI-CONTRACT no SELFTEST-ABI FAIL structure= line - a red assertion that does not say which structure broke is not a diagnosis')
    }
    # The verdict must be CARRIED by a variable assigned from Test-ShotHarnessAbi. A hard stop
    # that keys off a literal or an unrelated flag survives every mutation of the ABI - that is
    # the hole a plain "does the text exit / SELFTEST-RESULT=FAIL exist" check leaves open.
    $abiVars = @()
    foreach ($m in [regex]::Matches($code, '(\$\w+)\s*=\s*\[\s*int\s*\]\s*\(\s*Test-ShotHarnessAbi\s*\)')) {
        $v = $m.Groups[1].Value
        if ($abiVars -notcontains $v) { $abiVars += $v }
    }
    if ($abiVars.Count -gt 0) {
        Pass ($name + ' ABI-CONTRACT the ABI failure count is captured in ' + ($abiVars -join ' ') + ' - a guard can only read the marshaler verdict')
    } else {
        Fail ($name + ' ABI-CONTRACT nothing is assigned from Test-ShotHarnessAbi - the assertions print, but no guard could consume their result')
    }
    $sti = [regex]::Match($code, 'if\s*\(\s*\$SelfTest\s*\)').Index
    if ([regex]::IsMatch($code, 'if\s*\(\s*\$SelfTest\s*\)')) {
        $self = $code.Substring($sti)
        $abiAt = $self.IndexOf('Test-ShotHarnessAbi')
        $shotAt = $self.IndexOf('Invoke-ShotForeground')
        if ($abiAt -lt 0) {
            Fail ($name + ' ABI-CONTRACT -SelfTest no longer calls Test-ShotHarnessAbi - a broken INPUT would capture a whole round before anything noticed')
        } elseif ($shotAt -ge 0 -and $abiAt -gt $shotAt) {
            Fail ($name + ' ABI-CONTRACT -SelfTest captures before asserting the ABI (abi@' + $abiAt + ' shot@' + $shotAt + ') - the order is the point')
        } else {
            Pass ($name + ' ABI-CONTRACT -SelfTest asserts the ABI before it shoots (abi@' + $abiAt + ' < shot@' + $shotAt + ')')
        }
        # the hard stop: if (<abiVar> -gt 0) { ... exit <non-zero> ... SELFTEST-RESULT=FAIL }
        $guard = $null
        foreach ($g in [regex]::Matches($self, 'if\s*\(\s*(\$\w+)\s*(?:-\s*gt\s*0|-\s*ne\s*0|-not\s+eq\s*0)\s*\)\s*\{')) {
            if ($abiVars -contains $g.Groups[1].Value) { $guard = $g; break }
        }
        if ($null -eq $guard) {
            Fail ($name + ' ABI-CONTRACT -SelfTest has no if (<abi-count> -gt 0) { } hard stop keyed on ' +
                  ($(if ($abiVars.Count) { $abiVars -join ' ' } else { 'any ABI variable' })) +
                  ' - the ABI check would be a printout the run walks past')
        } else {
            $op = Get-ExitOperand $self ($guard.Index + $guard.Length) 240
            if ($op -eq '') {
                Fail ($name + ' ABI-CONTRACT the ABI hard stop at +' + $guard.Index + ' contains no exit - the FAIL line is printed and execution continues')
            } elseif ($op -match '^\d+$' -and [int]$op -eq 0) {
                Fail ($name + ' ABI-CONTRACT -SelfTest exits 0 on a failed ABI check (exit ' + $op + ') - red text behind a green code is exactly the fake green this file exists to kill')
            } elseif (-not ($self.Substring($guard.Index) -match 'SELFTEST-RESULT=FAIL')) {
                Fail ($name + ' ABI-CONTRACT -SelfTest stops on ' + $guard.Groups[1].Value + ' but lost the SELFTEST-RESULT=FAIL summary line')
            } else {
                Pass ($name + ' ABI-CONTRACT -SelfTest hard-stops on ' + $guard.Groups[1].Value + ' with exit ' + $op + ' + SELFTEST-RESULT=FAIL (no frame is captured on a bad ABI)')
            }
        }
    } else {
        Fail ($name + ' ABI-CONTRACT cannot find the -SelfTest block at all')
    }
    # -AbiOnly must exit with the number the marshaler returned, not with a constant. Its scope
    # is bounded by the next -SelfTest header so a deleted exit cannot borrow another block's.
    $ao = [regex]::Match($code, 'if\s*\(\s*\$AbiOnly\s*\)\s*\{')
    if (-not $ao.Success) {
        Fail ($name + ' ABI-CONTRACT -AbiOnly block not found - the headless lane cannot re-prove the sizes any more')
    } else {
        $nextIf = [regex]::Match($code.Substring($ao.Index + $ao.Length), 'if\s*\(\s*\$SelfTest\s*\)')
        $win = 1200
        if ($nextIf.Success -and $nextIf.Index -lt $win) { $win = $nextIf.Index }
        $op2 = Get-ExitOperand $code ($ao.Index + $ao.Length) $win
        if ($op2 -eq '') {
            Fail ($name + ' ABI-CONTRACT -AbiOnly has no exit-with-operand - it prints its verdict and hands back 0, so a red run looks green to the caller')
        } elseif ($abiVars -contains $op2) {
            Pass ($name + ' ABI-CONTRACT -AbiOnly exits with ' + $op2 + ', the count the marshaler returned (measured: 48B INPUT shape => exit 2)')
        } elseif ($op2 -match '^\d+$' -and [int]$op2 -ne 0) {
            Pass ($name + ' ABI-CONTRACT -AbiOnly exits with the literal ' + $op2 + ' (non-zero)')
        } else {
            Fail ($name + ' ABI-CONTRACT -AbiOnly exits with ' + $op2 + ' - not the ABI verdict, so a broken INPUT would still report success to the driver')
        }
    }
    # ---- the measured version stamp (agent T2S, closing TL2's last open item). A banner that
    # types its own numbers in is prose wearing a number's clothes - the exact class of miss this
    # file exists for - so the pin is on the CALL SITES: the stamp function must read the layout
    # off the marshaler, and both headless entry points must echo it.
    $stampAt = [regex]::Match($code, 'function\s+Get-ShotHarnessStamp\b')
    if (-not $stampAt.Success) {
        Fail ($name + ' ABI-CONTRACT STAMP no Get-ShotHarnessStamp function - a run cannot state which layout the copy it loaded actually has (T-1 and T-6 both sat behind rounds that printed a confident banner anyway)')
    } else {
        $aoAt2 = [regex]::Match($code, 'if\s*\(\s*\$AbiOnly\s*\)\s*\{')
        $stAt2 = [regex]::Match($code, 'if\s*\(\s*\$SelfTest\s*\)\s*\{')
        $bodyEnd = $code.Length
        if ($aoAt2.Success -and $aoAt2.Index -gt $stampAt.Index) { $bodyEnd = $aoAt2.Index }
        $stampBody = $code.Substring($stampAt.Index, $bodyEnd - $stampAt.Index)
        $measured = @()
        foreach ($mm in [regex]::Matches($stampBody, '\[\s*ShotHarness32\s*\]\s*::\s*(Abi\w+)')) { $measured += $mm.Groups[1].Value }
        if ($measured.Count -ge 5 -and ($measured -contains 'AbiInputSize') -and ($measured -contains 'AbiInputFields')) {
            Pass ($name + ' ABI-CONTRACT STAMP Get-ShotHarnessStamp reads ' + $measured.Count + ' layout number(s) off the marshaler (' +
                  (($measured | Select-Object -Unique) -join ', ') + ') - the banner reports the measured shape, not a typed literal')
        } else {
            Fail ($name + ' ABI-CONTRACT STAMP Get-ShotHarnessStamp obtains ' + $measured.Count +
                  ' number(s) from the marshaler (AbiInputSize + AbiInputFields + the struct sizes are required) - a banner with hardcoded numbers is the prose the gate already rejects')
        }
        $echoed = 0
        if ($aoAt2.Success) {
            $lenAbi = $code.Length - $aoAt2.Index
            if ($stAt2.Success -and $stAt2.Index -gt $aoAt2.Index) { $lenAbi = $stAt2.Index - $aoAt2.Index }
            if ($code.Substring($aoAt2.Index, $lenAbi) -match 'Get-ShotHarnessStamp') { $echoed++ }
        }
        if ($stAt2.Success -and ($code.Substring($stAt2.Index) -match 'Get-ShotHarnessStamp')) { $echoed++ }
        if ($echoed -eq 2) {
            Pass ($name + ' ABI-CONTRACT STAMP both headless paths echo the banner (-AbiOnly and -SelfTest), so no lane runs without stating what it loaded')
        } else {
            Fail ($name + ' ABI-CONTRACT STAMP only ' + $echoed + '/2 headless path(s) echo the banner - a run that never says which copy and layout it used is unauditable afterwards')
        }
    }
    # TapAlt must refuse to reach SendInput with a cbSize the table rejected (defect T-6 died
    # because a wrong size still "returned success"). Structural: guard, then throw, then call.
    $bodies = Get-TypeDefinitionBodies $R.Ast
    $norm = ''
    foreach ($b in $bodies) {
        $c = (Remove-CStyleComments $b) -replace '\s+', ' '
        $tm = [regex]::Match($c, 'TapAlt\(\)\s*\{')
        if ($tm.Success) {
            $start = $tm.Index + $tm.Length
            $depth = 1; $i = $start
            while ($i -lt $c.Length -and $depth -gt 0) {
                if ($c[$i] -eq '{') { $depth++ } elseif ($c[$i] -eq '}') { $depth-- }
                $i++
            }
            $norm = $c.Substring($start, $i - $start)
        }
    }
    if ($norm -eq '') {
        Fail ($name + ' ABI-CONTRACT TapAlt() body not found in any Add-Type body - the foreground-break primitive is not the one this gate was written for')
        return
    }
    $iSize = $norm.IndexOf('Marshal.SizeOf(typeof(INPUT))')
    $iThrow = $norm.IndexOf('throw')
    $iSend = $norm.IndexOf('SendInput(')
    if ($iSize -ge 0 -and $iThrow -ge 0 -and $iSend -ge 0 -and $iThrow -lt $iSend) {
        Pass ($name + ' ABI-CONTRACT TapAlt computes SizeOf(INPUT), throws on a mismatch, and only then calls SendInput (a 48B INPUT can no longer fail silently)')
    } else {
        Fail ($name + ' ABI-CONTRACT TapAlt has no cbSize guard before SendInput (sizeof@' + $iSize + ' throw@' + $iThrow + ' send@' + $iSend +
              ') - the exact fingerprint of defect T-6: 0 events injected, success returned')
    }
}

# ---------------------------------------------------------------- rules 10-14 (T3K knife)
# Why these exist (ledger DR6P §1): the round DR6 is about to run judges PIXELS through these
# drivers, and DR6P measured 7 fake-greens and 4 fake-FAILs in them. The two fatal ones were
# "the driver prints RESULT=ok and exits 0 no matter what happened" and "the default parameter
# arm starts the REAL kernel". A rule that only looks for a word cannot catch either, so every
# check below reads a NUMBER or a STRUCTURE: how many problems the accumulator can hold, which
# token literals the script names, what the default parameter value actually is.
$script:ExitContract = @{
    'dr4-drive.ps1' = @{
        marker   = 'Mark-Dr4Problem'
        cls      = 'Test-Dr4FailureToken'
        acc      = '$script:Dr4Problems'
        tol      = '$script:Dr4Tolerated'
        roster   = '$script:Dr4FailureTokens'
        guard    = 'if ( $script:Dr4Problems . Count -gt 0 ) { $rc = 1 }'
        minMarks = 12
        tokens   = @('NOTFOUND=', 'SET-NOTFOUND=', 'VAL-NOTFOUND=', 'TYPE-NOTFOUND=', 'HOVER-NOTFOUND=',
                     'CLICK-NOTFOUND=', 'ESHOT-NOTFOUND=', 'WAITFORLOG-TIMEOUT=', 'WAITFORLOG-SKIPPED=',
                     'LOGGREP-NOLOG=', 'TYPE-DEGRADED', 'STEP-UNKNOWN=', 'STEP-THREW=', 'QASHOT-SKIPPED',
                     'QASHOT-THREW=', 'ESHOT-SKIPPED', 'SHOT-SKIPPED', 'KEYIN-UNKNOWN=', 'HOVER-NO-RECT=',
                     'CLICK-NO-RECT=', 'SET-BADSPEC', 'ESHOT-BADSPEC', 'TYPE-BADSPEC', 'SET-VALUE-THREW=',
                     'TYPE-THREW=', 'STDOUT-SAVE-THREW=', 'FG-EXHAUSTED=', 'ALT-TAP-INJECTED=', 'TREE-THIN=',
                     'FRAME-BAD=', 'NEEDLE-ANCHOR=', 'FIND-BY-REGEX=', 'AMBIGUOUS-FIND=', 'LOGRE-LINEANCHOR=',
                     'REAL-KERNEL=', 'KERNEL-NOFAKE=', 'DR4-ABI=', 'TOOLCHAIN-BAD=', 'PROC-LAUNCH-NOT-EXE=',
                     # T4K: the crop leg's own failure prefix. Before it existed a failed crop was only ever a
                     # generic STEP-THREW= (or nothing at all) and the leg could not be told apart from working.
                     'ESHOT-CROP-FAILED=',
                     # T5K: four more print sites that used to exist OUTSIDE the roster, so they printed and
                     # walked past the accumulator. NOTINVOKED= is the fail-closed replacement for the deleted
                     # silent LegacyIAccessible arm: without it `inv:` prints INVOKED= with an empty pattern=
                     # and the round is green even though nothing was ever invoked.
                     'NOTINVOKED=', 'SHOTFAILED=', 'ESHOT-EMPTY-RECT=', 'QASHOT-PRINTWINDOW-FAILED',
                     # Pinned into the roster (so it can never go back to being a silent printout) but NOT into
                     # `critical` - see the note under that list for why the tolerated-skip family stays out.
                     'ESHOT-DEGENERATE')
        critical = @('NOTFOUND=', 'WAITFORLOG-TIMEOUT=', 'TREE-THIN=', 'FRAME-BAD=', 'FIND-BY-REGEX=',
                     'AMBIGUOUS-FIND=', 'NEEDLE-ANCHOR=', 'REAL-KERNEL=', 'KERNEL-NOFAKE=', 'DR4-ABI=',
                     'TOOLCHAIN-BAD=', 'LOGGREP-NOLOG=', 'ESHOT-CROP-FAILED=',
                     # T5K: every one of these is a step that produced nothing. Pinned critical = it must stay
                     # in the driver's roster AND keep being emitted, so deleting the lock is a red run.
                     'NOTINVOKED=', 'SHOTFAILED=', 'ESHOT-EMPTY-RECT=', 'QASHOT-PRINTWINDOW-FAILED'
                     # ESHOT-DEGENERATE is deliberately NOT here: a 1-3px element making a degenerate crop is a
                     # property of the element, not a defect of the tool chain. It is in `tokens` (so it can
                     # never go back to being silent) and it stays out of `critical` (so pinning it red would
                     # make every ordinary round red, which is not a fix). The clamp-lost half of degenerate -
                     # the one that IS a geometry defect - is routed to ESHOT-CROP-FAILED= instead.
                     )
    }
    'gui_uia.ps1' = @{
        marker   = 'Mark-UiaProblem'
        cls      = 'Test-UiaFailureToken'
        acc      = '$script:UiaProblems'
        tol      = '$script:UiaTolerated'
        roster   = '$script:UiaFailureTokens'
        guard    = 'if ( $script:UiaProblems . Count -gt 0 ) { $rc = 1 }'
        minMarks = 3
        tokens   = @('NOTFOUND=', 'NOTINVOKED=', 'WAITFORLOG-TIMEOUT=', 'WAITFORLOG-SKIPPED=',
                     'WAITFOR-TIMEOUT', 'WARN=', 'SETTEXT-SKIPPED', 'SETTEXTFAILED=', 'SHOT-SKIPPED',
                     'SHOTFAILED=', 'TREE-THIN=', 'FIND-BY-REGEX=', 'AMBIGUOUS-FIND=')
        critical = @('NOTFOUND=', 'NOTINVOKED=', 'WAITFORLOG-TIMEOUT=', 'WARN=', 'TREE-THIN=',
                     'FIND-BY-REGEX=', 'AMBIGUOUS-FIND=', 'SHOTFAILED=')
    }
}
# The two halves of the "never launch the real kernel" lock (rule 11). Built by concatenation so
# that this file never carries the pattern it is scanning for (repo precedent: $script:LibName).
$script:LaunchPat = '(?i)' + '\[System\.Diagnostics\.Process\]::Start\s*\(' + '|' + 'Start-' + 'Process\s+-FilePath'
$script:LaunchRoster = @{
    'dr4-drive.ps1' = 1; 'gui_uia.ps1' = 2; 'gui_probe.ps1' = 1; 'shot_harness.ps1' = 0; 'selftest_tools.ps1' = 0
}
# What a green run of THIS file is allowed to be evidence about (rule 14 + rule 15). Pinned tokens,
# printed unconditionally; deleting one, or paraphrasing UNCOVERED into COVERED, is a FAIL.
# SCOPE-CROP-SYNTH is the one thing T4K added: rule 15 EXECUTES the driver's crop helper, but only on
# a bitmap this gate paints itself - which is why SCOPE-CAPTURE below stays UNCOVERED instead of being
# widened (the product's real frames are still never touched here).
$script:ScopeTokens = @{
    'SCOPE-TOOLING'       = 'COVERED'
    'SCOPE-PRODUCT-CODE'  = 'UNCOVERED'
    'SCOPE-CAPTURE'       = 'UNCOVERED'
    'SCOPE-PRODUCT-BUILD' = 'UNCOVERED'
    'SCOPE-CROP-SYNTH'    = 'COVERED'
}
# Rule 15 + rule 16 budget: this gate loads managed BCL assemblies by name and that is the ONLY kind
# of Add-Type it may do. A -TypeDefinition / -MemberDefinition body is where P/Invoke would enter, and
# that must stay at zero or SCOPE-CAPTURE is a lie.
#   System.Drawing            rule 15 paints/crops a SYNTHETIC bitmap (never captures a frame)
#   UIAutomationClient        rule 16 reads TYPE METADATA to check that every
#   UIAutomationTypes         [System.Windows.Automation.*] type literal in the drivers really exists
#   System.Windows.Forms      (gui_probe.ps1 writes SendKeys) - metadata only
# Loading a wrapper assembly is NOT using it: rule 16 creates no AutomationElement, walks no UIA tree,
# opens no window and calls no UIA method, and it PROVES that by measuring its own AST for UIA member
# accesses (must be 0). Widening this list is a declared act, not an accident.
$script:GateAssemblyLoads = 4
$script:GateAssemblyNames = @('System.Drawing', 'UIAutomationClient', 'UIAutomationTypes', 'System.Windows.Forms')
# Rule 15 size tolerance, stated in the PASS line instead of hidden in the code: GDI+ rounds the
# requested rectangle to whole pixels, so the crop is allowed to be +-1px off the request. Anything
# wider than that is a real mismatch and is a FAIL.
$script:CropTolPx = 1
# The product-source path pattern, also built by concatenation: this gate scans its own code
# stream with it, so a plain literal here would make the "0 product paths" claim false on itself.
$script:ProdPathPat = '(?i)' + 'main' + '\.rs|' + 'ipc' + '\.rs' + '|' + 'fake_dsh' + '\.rs|' + 'src[\\/]'
# Which bucket names each driver's finder must expose, and in which order (rule 12).
$script:FindContract = @{
    'dr4-drive.ps1' = @{ fn = 'Find-El'; first = 'id'; second = 'name-eq'; re = 'id-re' }
    'gui_uia.ps1'   = @{ fn = 'Find-ByName'; first = 'id-eq'; second = 'name-eq'; re = 'id-re' }
}

function Get-NormCode([string]$Code) { return ($Code -replace '\s+', ' ') }
# A single-quoted literal wrapped in quote characters, so needles may contain ' without
# turning into a quoting trap (the token stream this gate reads keeps the quotes).
function Lit([string]$s) { return ([string][char]39 + $s + [string][char]39) }

# The members of a pinned string-array literal, e.g. $script:Dr4FailureTokens = @('a', 'b').
function Get-ArrayLiteral([string]$Norm, [string]$VarName) {
    $m = [regex]::Match($Norm, [regex]::Escape($VarName) + '\s*=\s*@\s*\(\s*(?<b>[^)]*)\)')
    if (-not $m.Success) { return $null }
    $out = @()
    foreach ($q in [regex]::Matches($m.Groups['b'].Value, "'([^']*)'")) { $out += $q.Groups[1].Value }
    return $out
}

# The body of one function, as it appears in the token stream: from its signature to the next
# 'function ' keyword. Used instead of prose matching so "the guard is inside W()" is structural.
function Get-FnWindow([string]$Norm, [string]$FnName, [int]$Max) {
    $i = $Norm.IndexOf('function ' + $FnName + ' (')
    if ($i -lt 0) { return '' }
    $rest = $Norm.Substring($i + 9)
    $j = $rest.IndexOf('function ')
    $len = $rest.Length
    if ($j -gt 0) { $len = [Math]::Min($len, $j) }
    if ($len -gt $Max) { $len = $Max }
    return $Norm.Substring($i, $len)
}

function Check-ExitCodeContract($R, [string]$Name) {
    # rule 10 - the exit code must be carried by a counted list of printed failures, and the
    # green banner must sit BEHIND that count. An unconditional `RESULT=ok` + `exit 0` is what
    # let DR6P read "a whole round can be wrong while the banner says ok" (its §1-A/§1-B).
    $cfg = $script:ExitContract[$Name]
    if ($null -eq $cfg) { return }
    $norm = Get-NormCode $R.Code
    $acc = $cfg.acc
    if ($norm.Contains($acc + ' = @( )')) {
        Pass ($Name + ' EXIT-CODE accumulator ' + $acc + ' starts empty (so "no problem printed" is a measured zero, not an absent counter)')
    } else {
        Fail ($Name + ' EXIT-CODE no `' + $acc + ' = @( )` initialisation - nothing accumulates the failures, so the tail banner cannot be carrying them')
    }
    if ($R.Fns -contains $cfg.marker) {
        Pass ($Name + ' EXIT-CODE ' + $cfg.marker + ' is defined (the one place a problem is recorded)')
    } else {
        Fail ($Name + ' EXIT-CODE function ' + $cfg.marker + ' is gone - failures would again be printouts the run walks past')
        return
    }
    $mWin = Get-FnWindow $norm $cfg.marker 900
    if ($mWin -ne '' -and $mWin.Contains($acc)) {
        Pass ($Name + ' EXIT-CODE ' + $cfg.marker + ' appends to ' + $acc + ' (a guard can only read what the marker wrote)')
    } else {
        Fail ($Name + ' EXIT-CODE ' + $cfg.marker + ' does not touch ' + $acc + ' - calling it would print and forget')
    }
    # the safety net: every failure-prefixed printout is classified on its way to the console,
    # so no print site can be forgotten by a later edit.
    $wWin = Get-FnWindow $norm 'W' 900
    if ($wWin -eq '') {
        Fail ($Name + ' EXIT-CODE no `function W (` found - the safety net has nowhere to live')
    } elseif ($wWin.Contains($cfg.cls) -and $wWin.Contains($cfg.marker)) {
        Pass ($Name + ' EXIT-CODE the print primitive W classifies through ' + $cfg.cls + ' into ' + $cfg.marker + ' - a failure line cannot be printed without being counted')
    } else {
        Fail ($Name + ' EXIT-CODE W() no longer routes through ' + $cfg.cls + ' + ' + $cfg.marker + ' - every failure prefix outside that net is a silent printout again')
    }
    $tokens = Get-ArrayLiteral $norm $cfg.roster
    if ($null -eq $tokens) {
        Fail ($Name + ' EXIT-CODE ' + $cfg.roster + ' is not a plain string-array literal - the failure-prefix roster cannot be read, so it cannot be verified')
    } else {
        $missing = @($cfg.tokens | Where-Object { $tokens -notcontains $_ })
        if ($missing.Count -eq 0) {
            Pass ($Name + ' EXIT-CODE ' + $cfg.roster + ' covers all ' + $cfg.tokens.Count + ' pinned failure prefixes (declared=' + $tokens.Count + ')')
        } else {
            Fail ($Name + ' EXIT-CODE ' + $cfg.roster + ' lost ' + $missing.Count + ' pinned failure prefix(es): ' + ($missing -join ' ') +
                  ' - dropping a prefix from the roster is how a whole class of failure becomes green again')
        }
    }
    # a roster entry nobody prints is a lock on an empty door: each DR6P-named prefix must also
    # have at least one emitter (counted on the code stream, so a comment cannot satisfy it).
    $orphan = @()
    foreach ($t in $cfg.critical) {
        $n = [regex]::Matches($norm, [regex]::Escape($t)).Count
        if ($n -lt 2 -or ($tokens -notcontains $t)) { $orphan += ($t + ' x' + $n) }
    }
    if ($orphan.Count -eq 0) {
        Pass ($Name + ' EXIT-CODE all ' + $cfg.critical.Count + ' critical failure prefixes are in the roster AND printed somewhere (measured occurrence count >= 2)')
    } else {
        Fail ($Name + ' EXIT-CODE critical prefix(es) with no emitter or no roster entry: ' + ($orphan -join ' ') +
              ' - that class of failure prints and stays green (or is counted but never produced)')
    }
    $markCalls = [regex]::Matches($norm, [regex]::Escape($cfg.marker) + '\s').Count
    if ($markCalls -gt $cfg.minMarks) {
        Pass ($Name + ' EXIT-CODE ' + $markCalls + ' textual occurrences of ' + $cfg.marker + ' (definition + call sites; pinned floor: >' + $cfg.minMarks + ')')
    } else {
        Fail ($Name + ' EXIT-CODE only ' + $markCalls + ' occurrence(s) of ' + $cfg.marker + ' (floor ' + $cfg.minMarks + ') - the criteria that are supposed to end the run non-zero are missing')
    }
    # the hard stop itself, same shape as shot_harness.ps1's `if ($abiFails -gt 0) { ... exit 1 }`
    $guardAt = $norm.IndexOf($cfg.guard)
    $zeroAt = $norm.IndexOf('$rc = 0 ')
    $failAt = $norm.IndexOf('RESULT=FAIL')
    $okAt = $norm.IndexOf('RESULT=ok problems=0')
    if ($guardAt -lt 0) {
        Fail ($Name + ' EXIT-CODE no `' + $cfg.guard + '` - the accumulator is never turned into an exit code')
    } elseif ($zeroAt -lt 0) {
        Fail ($Name + ' EXIT-CODE $rc is never initialised to 0 before the guard - an unset variable is not a verdict')
    } elseif ($failAt -lt 0) {
        Fail ($Name + ' EXIT-CODE no RESULT=FAIL banner at all - a counted failure still has nothing that says so out loud')
    } elseif ($okAt -lt 0) {
        Fail ($Name + ' EXIT-CODE no `RESULT=ok problems=0` banner - the green path does not even state the number it is claiming')
    } elseif ($okAt -lt $guardAt) {
        Fail ($Name + ' EXIT-CODE RESULT=ok problems=0 is printed BEFORE the ' + $acc + ' guard (ok@' + $okAt + ' < guard@' + $guardAt + ') - the banner would lie')
    } else {
        $op = Get-ExitOperand $norm $failAt 240
        if ($op -eq '') {
            Fail ($Name + ' EXIT-CODE the RESULT=FAIL branch has no exit - the red line is printed and the caller still gets 0')
        } elseif ($op -match '^\d+$' -and [int]$op -eq 0) {
            Fail ($Name + ' EXIT-CODE the RESULT=FAIL branch exits with 0 - that is the fake green this rule exists to kill')
        } else {
            $tail200 = $norm.Substring($okAt, [Math]::Min(200, $norm.Length - $okAt))
            if (-not $tail200.Contains('exit 0')) {
                Fail ($Name + ' EXIT-CODE the RESULT=ok banner is not followed by `exit 0` - the green path does not state its own code')
            } else {
                Pass ($Name + ' EXIT-CODE ' + $acc + ' drives a real verdict: RESULT=FAIL then exit ' + $op +
                      ', RESULT=ok problems=0 sits behind the guard (ok@' + $okAt + ' > guard@' + $guardAt + ') and exits 0')
            }
        }
    }
}

function Check-KernelDefault($R, [string]$Name) {
    # rule 11a - the DEFAULT arm must be the fake kernel. Measured before this knife:
    # [string]$FakeKernelArgs = ''  =>  $FakeKernelArgs -eq ''  =>  KERNEL=real  =>  the fork
    # resolves its own kernel (a real node process) and every stub-side expectation is void.
    if ($Name -ne 'dr4-drive.ps1') { return }
    $norm = Get-NormCode $R.Code
    $m = [regex]::Match($norm, "\[ string \] [$]FakeKernelArgs = '(?<v>[^']*)'")
    if (-not $m.Success) {
        Fail 'dr4-drive.ps1 KERNEL-DEFAULT no [string]$FakeKernelArgs string literal default - the fake-kernel arm cannot be shown to be the default'
    } elseif ($m.Groups['v'].Value.Length -eq 0) {
        Fail ('dr4-drive.ps1 KERNEL-DEFAULT -FakeKernelArgs defaults to the EMPTY string (measured length=0) - an empty default IS the real-kernel arm (DR6P §1-A), this is the red line')
    } else {
        Pass ('dr4-drive.ps1 KERNEL-DEFAULT -FakeKernelArgs defaults to ' + (Lit $m.Groups['v'].Value) +
              ' (measured length=' + $m.Groups['v'].Value.Length + ') => the fake kernel is the only default arm')
    }
    if ($norm.Contains('[ switch ] $RealKernel')) {
        Pass 'dr4-drive.ps1 KERNEL-DEFAULT the real kernel needs an explicit -RealKernel switch (there is no fall-through left)'
    } else {
        Fail 'dr4-drive.ps1 KERNEL-DEFAULT -RealKernel is gone - an opt-in red line that cannot be opted into is usually replaced by an always-on one'
    }
    $rkIf = $norm.IndexOf('if ( $RealKernel )')
    $realAt = $norm.IndexOf('KERNEL=real')
    $markAt = $norm.IndexOf('Mark-Dr4Problem ' + (Lit 'REAL-KERNEL='))
    if ($rkIf -lt 0 -or $realAt -lt 0 -or $markAt -lt 0) {
        Fail ('dr4-drive.ps1 KERNEL-DEFAULT real arm is not triad-guarded (RealKernel-if@' + $rkIf + ' KERNEL=real@' + $realAt + ' REAL-KERNEL mark@' + $markAt + ') - the banner and the exit code must both speak on that arm')
    } elseif ($rkIf -lt $realAt -and ($realAt - $rkIf) -lt 500 -and $markAt -gt $rkIf -and ($markAt - $rkIf) -lt 900) {
        Pass 'dr4-drive.ps1 KERNEL-DEFAULT KERNEL=real is only reachable under if ($RealKernel) and marks REAL-KERNEL inside that same branch, so that run can never end green'
    } else {
        Fail ('dr4-drive.ps1 KERNEL-DEFAULT order wrong (if@' + $rkIf + ' banner@' + $realAt + ' mark@' + $markAt + ') - the guard, the banner and the problem mark must be in one branch')
    }
    $inj = [regex]::Matches($R.Code, '[$]env:BLADE2_KERNEL_EXE\s*=').Count
    if ($inj -eq 1) {
        Pass 'dr4-drive.ps1 KERNEL-DEFAULT BLADE2_KERNEL_EXE is written from exactly ONE place (the fake arm); no second injection site can appear unnoticed'
    } else {
        Fail ('dr4-drive.ps1 KERNEL-DEFAULT ' + $inj + ' assignments to $env:BLADE2_KERNEL_EXE (pinned: 1) - more than one site means one of them can name a real kernel')
    }
    if ($norm.Contains('$FakeKernelArgs -eq ' + (Lit '')) -and $norm.Contains((Lit '--pace=0')) -and $norm.Contains('KERNEL-NOFAKE=')) {
        Pass 'dr4-drive.ps1 KERNEL-DEFAULT an emptied -FakeKernelArgs is caught, reported and pinned back to --pace=0 instead of falling into the real arm'
    } else {
        Fail 'dr4-drive.ps1 KERNEL-DEFAULT the emptied-argument escape hatch is not covered any more (no KERNEL-NOFAKE recovery to --pace=0)'
    }
    if ($norm.Contains('-notmatch ' + (Lit '\.exe$')) -and $norm.Contains('PROC-LAUNCH-NOT-EXE=')) {
        Pass 'dr4-drive.ps1 KERNEL-DEFAULT the launch target is refused unless it ends in .exe (a .cmd wrapper such as the npm dsh shim can never be launched from here)'
    } else {
        Fail 'dr4-drive.ps1 KERNEL-DEFAULT the .exe guard on the launch target is gone'
    }
    if ($norm.Contains('KERNEL-LOCK mode=') -and $norm.Contains('BLADE2_KERNEL_EXE=')) {
        Pass 'dr4-drive.ps1 KERNEL-DEFAULT every run prints the KERNEL-LOCK line with the resolved kernel mode and the exact value the fork will inherit'
    } else {
        Fail 'dr4-drive.ps1 KERNEL-DEFAULT the KERNEL-LOCK evidence line is gone - "which kernel did this round actually talk to" is unanswerable again'
    }
}

function Check-FindChannels($R, [string]$Name) {
    # rule 12 - UIA comparison used to read Name only. DR6P §1-B: the DR6 permission buttons,
    # the palette rows and the side-nav entries carry AutomationId, so a Name-only finder returns
    # NOTFOUND for elements that are on screen - a false FAIL that looks like a product defect.
    $cfg = $script:FindContract[$Name]
    if ($null -eq $cfg) { return }
    $norm = Get-NormCode $R.Code
    $win = Get-FnWindow $norm $cfg.fn 2400
    if ($win -eq '') {
        Fail ($Name + ' FIND-CHANNEL function ' + $cfg.fn + ' is gone - every needle would resolve somewhere else')
        return
    }
    $readsId = ($win.Contains('. AutomationId') -or $win.Contains('AutomationIdValue'))
    $firstAt = $win.IndexOf('how = ' + (Lit $cfg.first))
    $secondAt = $win.IndexOf('how = ' + (Lit $cfg.second))
    $reAt = $win.IndexOf('how = ' + (Lit $cfg.re))
    if ($readsId -and $firstAt -ge 0 -and $secondAt -gt $firstAt -and $reAt -gt $secondAt) {
        Pass ($Name + ' FIND-CHANNEL ' + $cfg.fn + ' reads AutomationId as its own channel and orders it BEFORE the Name channel (' +
              $cfg.first + '@' + $firstAt + ' < ' + $cfg.second + '@' + $secondAt + ' < ' + $cfg.re + '@' + $reAt + ')')
    } else {
        Fail ($Name + ' FIND-CHANNEL ' + $cfg.fn + ' is Name-only again (AutomationId read=' + [string]$readsId +
              ' bucket order ' + $firstAt + '/' + $secondAt + '/' + $reAt + ') - an id-only element is reported as missing (DR6P §1-B)')
    }
    if ($win.Contains('AMBIGUOUS-FIND=') -and $win.Contains('els . Count -gt 1')) {
        Pass ($Name + ' FIND-CHANNEL more than one candidate under one needle is counted (the value judged is the measured els.Count), not silently taken as els[0]')
    } else {
        Fail ($Name + ' FIND-CHANNEL the ambiguity criterion is gone - `find:Permission` matching four elements again means clicking one at random')
    }
    if ($win.Contains('FIND-BY-REGEX=') -and $norm.Contains('[ switch ] $LooseFind')) {
        Pass ($Name + ' FIND-CHANNEL the regex fallback bucket is counted as a problem unless -LooseFind says so out loud')
    } else {
        Fail ($Name + ' FIND-CHANNEL the regex-fallback criterion is gone - a half-written needle can drive the wrong element and stay green')
    }
    # The waiver list is what keeps "this element is id-only and therefore never in the tree"
    # (mainline-identical, e.g. the ReferencePalette Border root) from being written as a FAIL -
    # but it must default to waiving NOTHING, otherwise it becomes a silent whitelist.
    $dflt = [regex]::Match($norm, "\[ string \] [$]TolerateAbsent = '(?<v>[^']*)'")
    if (-not $dflt.Success) {
        Fail ($Name + ' TOLERATE no [string]$TolerateAbsent literal default - the waiver list cannot be shown to default to empty')
    } elseif ($dflt.Groups['v'].Value.Length -ne 0) {
        Fail ($Name + ' TOLERATE -TolerateAbsent defaults to a NON-empty list (measured length=' + $dflt.Groups['v'].Value.Length + ') - problems would be waived before anyone asked')
    } else {
        Pass ($Name + ' TOLERATE -TolerateAbsent defaults to the empty string (measured length=0): nothing is waived unless the caller names it')
    }
    if ($norm.Contains('-eq ' + (Lit '')) -and $norm.Contains('return $false')) {
        Pass ($Name + ' TOLERATE the waiver test returns $false on an empty list (fail-closed, spelled out in code)')
    } else {
        Fail ($Name + ' TOLERATE the fail-closed default of -TolerateAbsent is not spelled out any more')
    }
}

function Check-LogCaliber($R, [string]$Name) {
    # rule 13 - two readers, ONE caliber, and the double `DIAG: DIAG:` prefix is a gate not a proverb.
    $norm = Get-NormCode $R.Code
    if ($Name -eq 'dr4-drive.ps1') {
        if ($norm.Contains('$_ -match $arg')) {
            Fail 'dr4-drive.ps1 LOG-CALIBER loggrep is back to `-match $arg`: a needle like [SCROLL] is then a character class and (+ are quantifiers, so the whole log family false-FAILs (DR6P §1-A)'
        } elseif ($norm.Contains('( $arg , [ System.StringComparison ] :: Ordinal )')) {
            Pass 'dr4-drive.ps1 LOG-CALIBER loggrep reads literal Ordinal substrings, the same caliber as waitlog (measured: `$_ -match $arg` occurrences = 0)'
        } else {
            Fail 'dr4-drive.ps1 LOG-CALIBER no Ordinal literal reader in the log leg - needles would be reinterpreted as wildcards/regex'
        }
        if ($norm.Contains((Lit 'logre')) -and $norm.Contains('New-Object System.Text.RegularExpressions.Regex $arg') -and $norm.Contains('LOGRE-BADREGEX=')) {
            Pass 'dr4-drive.ps1 LOG-CALIBER the regex reader is a SEPARATE verb (logre:) that compiles the pattern first and marks LOGRE-BADREGEX when it does not'
        } else {
            Fail 'dr4-drive.ps1 LOG-CALIBER logre: / LOGRE-BADREGEX is gone - a broken pattern would again read as "the app never logged it"'
        }
        $cal = [regex]::Match($R.Code, '\$script:Dr4LogPrefixCaliber\s*=\s*''([^'']*)''')
        if ($cal.Success -and $cal.Groups[1].Value.Contains('DIAG: DIAG:')) {
            Pass ('dr4-drive.ps1 LOG-PREFIX the double-prefix caliber is pinned in $script:Dr4LogPrefixCaliber (' + $cal.Groups[1].Value.Length + ' chars) and echoed on every timeout')
        } else {
            Fail 'dr4-drive.ps1 LOG-PREFIX $script:Dr4LogPrefixCaliber no longer states "DIAG: DIAG:" - every line-start needle becomes a false FAIL again (DR6P §5-3)'
        }
        if ($norm.Contains('Test-Dr4AnchoredNeedle') -and $norm.Contains('NEEDLE-ANCHOR=')) {
            Pass 'dr4-drive.ps1 LOG-PREFIX a ^-anchored needle is rejected on the spot and counted, instead of burning the whole -WaitSec budget and reporting a false absence'
        } else {
            Fail 'dr4-drive.ps1 LOG-PREFIX the anchored-needle gate is gone'
        }
        if ($norm.Contains('LOGGREP-NOLOG=')) {
            Pass 'dr4-drive.ps1 LOG-CALIBER grepping a missing log file is counted (absence of evidence is no longer evidence of absence)'
        } else {
            Fail 'dr4-drive.ps1 LOG-CALIBER the missing-log-file case is a printout again'
        }
        # the stale-claim knife (DR6P §1-A :52-61): the driver may no longer assert anything about
        # the shipped capture core from prose - it must read the file and print measured values.
        $md5Lit = [regex]::Matches($R.Code, '[0-9a-f]{32}').Count
        if ($md5Lit -eq 0) {
            Pass 'dr4-drive.ps1 NO-STALE-CLAIM no 32-hex md5 literal anywhere in the code stream (md5s are computed at run time and printed)'
        } else {
            Fail ('dr4-drive.ps1 NO-STALE-CLAIM ' + $md5Lit + ' hard-coded md5 literal(s) in code - a typed-in hash is exactly the "comment as evidence" habit DR6P §1-A killed')
        }
        if ($norm.Contains('Get-FileHash -Algorithm MD5 -LiteralPath $shPath') -and $norm.Contains('TOOLCHAIN shot_harness.ps1 md5=') -and $norm.Contains('TOOLCHAIN-BAD=')) {
            Pass 'dr4-drive.ps1 NO-STALE-CLAIM the harness health is re-read every run (md5 + using-System.Text) and TOOLCHAIN-BAD is counted when it drifts'
        } else {
            Fail 'dr4-drive.ps1 NO-STALE-CLAIM the live TOOLCHAIN probe of shot_harness.ps1 is gone - prose about it would be believed again'
        }
        if ($norm.Contains(':: SizeOf') -and $norm.Contains('Dr4Native+INPUT') -and $norm.Contains('-ne 40') -and $norm.Contains('exit 5')) {
            Pass 'dr4-drive.ps1 NO-STALE-CLAIM this script MEASUREs its own INPUT size at run time and refuses to start a round on anything but 40B (exit 5)'
        } else {
            Fail 'dr4-drive.ps1 NO-STALE-CLAIM the runtime INPUT size assertion is gone - defect T-6 (48B => 0 events injected, "success" returned) can come back silently'
        }
    }
    if ($Name -eq 'gui_uia.ps1') {
        $ord = [regex]::Matches($R.Code, '::\s*Ordinal').Count
        if ($ord -ge 2) {
            Pass ('gui_uia.ps1 LOG-CALIBER ' + $ord + ' Ordinal readers (failure-token prefix test + Get-LogHit + Get-LogCount), no regex surface for a needle')
        } else {
            Fail ('gui_uia.ps1 LOG-CALIBER only ' + $ord + ' Ordinal reader(s) left in the log leg')
        }
        $cal = [regex]::Match($R.Code, '\$script:UiaLogPrefixCaliber\s*=\s*''([^'']*)''')
        if ($cal.Success -and $cal.Groups[1].Value.Contains('DIAG: DIAG:')) {
            Pass 'gui_uia.ps1 LOG-PREFIX the double-prefix caliber is pinned and appended to every WAITFORLOG-TIMEOUT line'
        } else {
            Fail 'gui_uia.ps1 LOG-PREFIX $script:UiaLogPrefixCaliber is gone - a timeout gives no hint that the row head is DIAG: DIAG:'
        }
        if ($norm.Contains("caliber=" + [string][char]39 + " + " + '$script:UiaLogPrefixCaliber')) {
            Pass 'gui_uia.ps1 LOG-PREFIX the timeout line actually carries the caliber string, so a reader cannot mistake a false absence for a product gap'
        } else {
            Fail 'gui_uia.ps1 LOG-PREFIX WAITFORLOG-TIMEOUT no longer prints the caliber (the pinned string is dead weight in a comment)'
        }
    }
}

function Check-ProcessLaunchRoster([string]$Dir) {
    # rule 11b - "never start the real kernel" has to be an offline-provable lock, so the gate
    # enumerates the launch sites itself, by line number, and pins how many each file may have.
    $total = 0
    $literalTargets = 0
    foreach ($name in @($script:LaunchRoster.Keys | Sort-Object)) {
        $path = Join-Path $Dir $name
        if (-not (Test-Path -LiteralPath $path)) { Fail ('launch-roster: ' + $name + ' vanished'); continue }
        $lines = @([System.IO.File]::ReadAllLines($path))
        $sites = @()
        for ($i = 0; $i -lt $lines.Count; $i++) {
            $l = $lines[$i]
            if ($l.TrimStart().StartsWith('#')) { continue }
            if ($l -match $script:LaunchPat) {
                $sites += ('L' + ($i + 1))
            if ($l -match "'[^']*\.(cmd|exe)") { $literalTargets++ }
            }
        }
        $want = [int]$script:LaunchRoster[$name]
        $total += $sites.Count
        if ($sites.Count -eq $want) {
            Pass ('launch-roster: ' + $name + ' = ' + $sites.Count + ' process-launch site(s) [' + ($sites -join ' ') + '] exactly as pinned')
        } else {
            Fail ('launch-roster: ' + $name + ' has ' + $sites.Count + ' launch site(s) [' + ($sites -join ' ') + '], pinned at ' + $want +
                  ' - a new place that starts a process must be named, reviewed and re-pinned (that is the whole lock)')
        }
    }
    if ($literalTargets -eq 0) {
        Pass ('launch-roster: ' + $total + ' launch site(s) in total, and none of them names a hard-coded *.cmd/*.exe path (every target is a resolved variable)')
    } else {
        Fail ('launch-roster: ' + $literalTargets + ' launch site(s) pass a hard-coded .cmd/.exe literal - the npm dsh shim / node.exe red line is one edit away (DR6P §3 抬头 4)')
    }
}

function Check-CoverageSelfDecl($R, [string]$Name) {
    # rule 14 - what a green run of THIS gate means. It reads five tool scripts, it does not
    # compile the product, it does not capture one frame, it does not touch UIA. Saying less than
    # that is the lie that let `files=5 pass=134 FAILS=0` be quoted as evidence about the fork.
    $norm = Get-NormCode $R.Code
    $found = @{}
    foreach ($m in [regex]::Matches($norm, "W \( '(?<k>SCOPE-[A-Z-]+)=(?<v>COVERED|UNCOVERED)")) {
        $found[$m.Groups['k'].Value] = $m.Groups['v'].Value
    }
    foreach ($k in @($script:ScopeTokens.Keys | Sort-Object)) {
        $want = $script:ScopeTokens[$k]
        if (-not $found.ContainsKey($k)) {
            Fail ($Name + ' COVERAGE no unconditional ' + $k + ' print line - a scope claim that is never printed cannot be audited')
        } elseif ($found[$k] -ne $want) {
            Fail ($Name + ' COVERAGE ' + $k + '=' + $found[$k] + ' but the pinned value is ' + $want +
                  ' - widening the claimed scope without widening the checks is exactly the fake green this rule exists to catch')
        } else {
            Pass ($Name + ' COVERAGE ' + $k + '=' + $want + ' is declared on every run')
        }
    }
    # The UNCOVERED claims must be BACKED BY MEASUREMENT, not by prose.
    $prodRefs = @([regex]::Matches($R.Code, $script:ProdPathPat))
    if ($prodRefs.Count -eq 0) {
        Pass ($Name + ' COVERAGE measured: 0 product-source path references in this file (pattern ' + $script:ProdPathPat +
              '), so SCOPE-PRODUCT-CODE=UNCOVERED is arithmetic, not a disclaimer')
    } else {
        $what = ($prodRefs | ForEach-Object { $_.Value } | Select-Object -First 4) -join ' '
        Fail ($Name + ' COVERAGE ' + $prodRefs.Count + ' product-source path reference(s) appeared in the code stream [' + $what +
              '] while SCOPE-PRODUCT-CODE still says UNCOVERED - one of the two is lying')
    }
    # Add-Type is the only way this tool chain can reach native code (PrintWindow / SendInput /
    # UIA live in the drivers, not here). Two kinds of Add-Type are NOT the same thing and the
    # gate must not blur them (T4K, rule 15):
    #   -AssemblyName   = load a managed BCL assembly. This file loads exactly one, System.Drawing,
    #                     so that rule 15 can paint a synthetic bitmap and run the crop helper.
    #   -TypeDefinition = compile C#, i.e. the only way a DllImport could appear in this gate.
    #                     Zero of those still means no PrintWindow / SendInput is reachable here,
    #                     which is what keeps SCOPE-CAPTURE honest.
    $addTypeDef = 0
    $addAsm = 0
    $addAsmNames = @()
    foreach ($c in @($R.Ast.FindAll( { param($n) $n -is [System.Management.Automation.Language.CommandAst] }, $true))) {
        $cn = ''
        try { $cn = [string]$c.GetCommandName() } catch { $cn = '' }
        if ($cn -ne ('Add-' + 'Type')) { continue }
        $els = @($c.CommandElements)
        $kind = ''
        for ($i = 0; $i -lt $els.Count; $i++) {
            if ($els[$i] -isnot [System.Management.Automation.Language.CommandParameterAst]) { continue }
            $pn = [string]$els[$i].ParameterName
            if ($pn -eq 'TypeDefinition' -or $pn -eq 'MemberDefinition') { $kind = 'body' }
            elseif ($pn -eq 'AssemblyName' -or $pn -eq 'Assembly') {
                $kind = 'asm'
                $lit = ''
                if ($null -ne $els[$i].Argument) { $lit = [string]$els[$i].Argument.Extent.Text }
                elseif (($i + 1) -lt $els.Count) { $lit = [string]$els[$i + 1].Extent.Text }
                $addAsmNames += ($lit -replace "[`"']", '')
            }
        }
        if ($kind -eq 'body') { $addTypeDef++ } elseif ($kind -eq 'asm') { $addAsm++ }
    }
    $cargoCalls = 0
    foreach ($c in @($R.Ast.FindAll( { param($n) $n -is [System.Management.Automation.Language.CommandAst] }, $true))) {
        $cn = ''
        try { $cn = [string]$c.GetCommandName() } catch { $cn = '' }
        if ($cn -eq ('ca' + 'rgo')) { $cargoCalls++ }
    }
    $nativeReaders = [regex]::Matches($R.Code, 'CopyFromScreen|PrintWindow|SendInput|UIAutomation').Count
    if ($addTypeDef -eq 0) {
        Pass ($Name + ' COVERAGE measured: ' + $addTypeDef + ' Add-Type -TypeDefinition/-MemberDefinition body in this file, so no P/Invoke (no PrintWindow, no SendInput, no capture) is even reachable - SCOPE-CAPTURE=UNCOVERED holds: pass=N says nothing about whether screenshots of the app work')
    } else {
        Fail ($Name + ' COVERAGE ' + $addTypeDef + ' compiled C# Add-Type body/bodies in the gate while SCOPE-CAPTURE says UNCOVERED - either widen the claim deliberately or drop the native surface')
    }
    if ($addAsm -le $script:GateAssemblyLoads -and (@($addAsmNames | Where-Object { $_ -notin $script:GateAssemblyNames }).Count -eq 0)) {
        Pass ($Name + ' COVERAGE measured: ' + $addAsm + ' managed assembly load(s) [' + ($addAsmNames -join ' ') +
              '] (budget ' + $script:GateAssemblyLoads + ', names pinned) - they exist for rule 15 to paint/crop a SYNTHETIC bitmap, not to capture anything')
    } else {
        Fail ($Name + ' COVERAGE assembly loads [' + ($addAsmNames -join ' ') + '] exceed the pinned budget/names (' +
              $script:GateAssemblyLoads + ': ' + ($script:GateAssemblyNames -join ' ') + ') - a new managed surface in the gate has to be declared here, not just used')
    }
    if ($nativeReaders -gt 0) {
        Pass ($Name + ' COVERAGE the ' + $nativeReaders + ' native-surface word(s) in this file are scan targets (strings/regex), not calls: compiled Add-Type bodies = ' + $addTypeDef)
    }
    # cargo is the product build; a gate that ran it would be quoting someone else's red as its own green.
    if ($cargoCalls -eq 0) {
        Pass ($Name + ' COVERAGE measured: ' + $cargoCalls + ' cargo command invocation(s) in this file (AST), so SCOPE-PRODUCT-BUILD=UNCOVERED holds and a green gate is not a build verdict')
    } else {
        Fail ($Name + ' COVERAGE ' + $cargoCalls + ' cargo invocation(s) in the gate while SCOPE-PRODUCT-BUILD says UNCOVERED - one of the two is lying')
    }
    # T4K rule 15 turned this gate from "static only" into "static + exactly one executed helper",
    # so the banner sentence that states the limit has to say WHICH limit it means. Pinned as two
    # substrings: the not-doing half and the one-doing half. Losing either half is a FAIL - the first
    # because the pass count loses its disclaimer, the second because the crop leg would be
    # unproven again. NOTE the compare window is the BANNER PRINT SITE itself, not the file: these
    # two sentences are literals in this file either way, and a whole-file Contains would be a check
    # that passes by reading its own needle (fake green by construction).
    # T5K widened both sentences IN LOCKSTEP with the banner print site, because rule 16 loads
    # UIAutomationClient / UIAutomationTypes / System.Windows.Forms as TYPE METADATA. "no UIA" was
    # blunter than the truth; what is actually never done is walking a tree / touching an element.
    $headless = 'headless except rules 15 and 16: no exe is started, no window is opened, no screen/PrintWindow capture, no UIA tree walk and no AutomationElement, no SendInput'
    $oneRun = 'the one driver function this gate EXECUTES is the crop helper (rule 15) on a bitmap it paints itself (synthetic pixels, not a frame of the app), and rule 16 only asks the runtime whether each System.* type literal and static member in the drivers exists (metadata, no element)'
    # Measured before shipping: searching for the banner with a plain literal matched THIS RULE's own
    # source line instead of the banner (index 78632 was the IndexOf call, not the print site), so the
    # window came out 45 chars wide and the check graded itself. Same reason $script:LibName and
    # $script:LaunchPat are built by concatenation: a scanner must not contain its own needle.
    $bannerMark = 'SELFTEST-' + 'TOOLS start='
    $hostMark = 'SELFTEST-' + 'TOOLS host='
    $bi = $norm.IndexOf($bannerMark)
    $bj = $norm.IndexOf($hostMark)
    $bwin = ''
    if ($bi -ge 0 -and $bj -gt $bi) { $bwin = $norm.Substring($bi, $bj - $bi) }
    if ($bwin -eq '') {
        Fail ($Name + ' COVERAGE the start banner print site cannot be located (start@' + $bi + ' host@' + $bj + ') - the scope sentence cannot be audited at all')
    } elseif ($bwin.Contains($headless) -and $bwin.Contains($oneRun)) {
        Pass ($Name + ' COVERAGE the start banner states what the gate does not do AND names the single path it executes (measured inside the banner print site, ' + $bwin.Length + ' chars)')
    } elseif ($bwin.Contains($headless)) {
        Fail ($Name + ' COVERAGE the banner keeps the "does not do" half but lost the sentence naming the executed crop path - rule 15 would run unannounced')
    } else {
        Fail ($Name + ' COVERAGE the start banner lost its headless sentence - downstream readers again have nothing to judge the pass count against')
    }
}

# ---------------------------------------------------------------- rule 15: RUN the crop, do not grep it
# Why this exists (T4K, ledger DR6 §1): rust/tests/dr4-drive.ps1's per-element crop wrote
# `[System.Drawing.Rectangle]::MakeRectangle(...)` - a static member that does not exist on that type
# (measured: Rectangle's statics are Ceiling / FromLTRB / Inflate / Intersect / Round / Truncate / Union
# plus op_*/ReferenceEquals; the outer FromRectangle wrapper is not a Rectangle static either, it lives
# on RectangleF). Every call threw, so the whole `eshot:` leg produced zero crops while the file still
# parsed and still LOOKED like a screenshot path. A static gate structurally cannot see that: the token
# is a plausible method name, so no amount of Contains()/regex ever fires. The only thing that catches
# it is running it once.
#
# What this rule is NOT: it paints its own 200x120 bitmap, it never opens a window, never calls
# PrintWindow/CopyFromScreen, never touches UIA and never starts the driver. So it proves the CROP CALL
# exists and runs - nothing about the product's frames, nothing about the app's pixels, nothing about
# product code (SCOPE-CROP-SYNTH=COVERED next to SCOPE-CAPTURE=UNCOVERED is exactly that distinction).
#
# It executes the helper VERBATIM: the function text is taken off the disk (FunctionDefinitionAst
# Extent.Text), dot-sourced, then called. So this gate cannot drift into testing a copy of the driver.
function Check-CropSmokeRun([string]$Dir, [int]$TolPx) {
    $Name = 'dr4-drive.ps1 CROP-SMOKE'
    Add-Type -AssemblyName System.Drawing
    $drv = Join-Path $Dir 'dr4-drive.ps1'
    if (-not (Test-Path -LiteralPath $drv)) {
        Fail ($Name + ' EXEC - dr4-drive.ps1 is not in the scanned directory (' + $drv + '), the crop leg cannot be run at all')
        return
    }
    $rr = Read-Script $drv
    $fdefs = @($rr.Ast.FindAll( { param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true) |
               Where-Object { $_.Name -eq 'Crop-Dr4Rect' })
    if ($fdefs.Count -ne 1) {
        Fail ($Name + ' EXTRACT - the driver defines Crop-Dr4Rect ' + $fdefs.Count + ' time(s) (pinned: 1). The crop must live in ONE callable helper for a headless gate to be able to run it; inline in Take-ElShot it is only reachable behind a live UIA element, which is how the MakeRectangle bug survived (rule 10 saw a green file on disk)')
        return
    }
    Pass ($Name + ' EXTRACT - Crop-Dr4Rect is exactly one function (' + $fdefs[0].Extent.Text.Length +
          ' chars taken verbatim from the file on disk, not a copy this gate could let drift)')
    $fnText = [string]$fdefs[0].Extent.Text

    $scratch = Join-Path ([System.IO.Path]::GetTempPath()) ('t4k-crop-smoke-' + $PID)
    $fullPng = Join-Path $scratch 'smoke-full.png'
    $cropPng = Join-Path $scratch 'smoke-crop.png'
    $degPng = Join-Path $scratch 'smoke-degenerate.png'
    $badPng = Join-Path $scratch ('smoke-no-such-dir' + [char]92 + 'smoke-throw.png')

    try {
        if (-not (Test-Path -LiteralPath $scratch)) { $null = New-Item -ItemType Directory -Force -Path $scratch }
        foreach ($p in @($fullPng, $cropPng, $degPng)) { if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Force } }

        # --- the known-像素 source: 200x120, every pixel a closed form of (x,y) so any crop of it is
        # predictable without trusting the capture path. No window, no frame, no UIA.
        $SW = 200; $SH = 120
        $srcBmp = New-Object System.Drawing.Bitmap($SW, $SH, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        for ($py = 0; $py -lt $SH; $py++) {
            for ($px = 0; $px -lt $SW; $px++) {
                $srcBmp.SetPixel($px, $py, [System.Drawing.Color]::FromArgb(255,
                    (($px * 7 + $py * 11) -band 0xFF), (($px * 13 + $py * 3) -band 0xFF), (($px + $py * 5) -band 0xFF)))
            }
        }
        $srcBmp.Save($fullPng, [System.Drawing.Imaging.ImageFormat]::Png)
        $srcBmp.Dispose()
        $md5full = (Get-FileHash -Algorithm MD5 -LiteralPath $fullPng).Hash.ToLower()

        # --- dot-source the driver's own helper text, then call it
        . ([ScriptBlock]::Create($fnText))
        if (-not (Get-Command Crop-Dr4Rect -ErrorAction SilentlyContinue)) {
            Fail ($Name + ' LOAD - the extracted Crop-Dr4Rect text did not define a callable function; the crop leg cannot be executed (parse-level rot in the driver would be invisible to rule 1 anyway: the file parses fine while the method name is wrong)')
            return
        }
        Pass ($Name + ' LOAD - the extracted helper compiled and loaded in this process (a syntax-loose crop helper would die right here)')

        $rx = 20; $ry = 30; $rw = 60; $rh = 40
        $r = Crop-Dr4Rect -SourcePath $fullPng -OutPath $cropPng -X $rx -Y $ry -CropW $rw -CropH $rh

        # 1) the call itself must not throw, and must report success
        if ($r.kind -eq 'threw') {
            Fail ($Name + ' EXEC - Crop-Dr4Rect caught its own exception (this is the pre-fix shape: ' + $r.msg + ')')
        } elseif ($r.ok) {
            Pass ($Name + ' EXEC - Crop-Dr4Rect ran the real crop and reported ok=true kind=' + $r.kind +
                  ' (pre-fix the same call threw: MethodNotFound on Rectangle::MakeRectangle, and the whole eshot: leg produced nothing)')
        } else {
            Fail ($Name + ' EXEC - Crop-Dr4Rect returned ok=false kind=' + $r.kind + ' msg=' + $r.msg + ' on a 60x40 crop of a 200x120 frame, which must succeed')
        }
        if (-not $r.ok) { return }

        # 2) the output file exists and is not empty
        if ((Test-Path -LiteralPath $cropPng) -and ([long](Get-Item -LiteralPath $cropPng).Length) -gt 0) {
            Pass ($Name + ' FILE - crop written: ' + $cropPng + ' bytes=' + (Get-Item -LiteralPath $cropPng).Length)
        } else {
            Fail ($Name + ' FILE - the helper reported ok but no crop file is on disk (a green ESHOT= line over a missing png is the fake evidence rule 10 already refuses for frames: FRAME-BAD=)')
        }

        $cb = New-Object System.Drawing.Bitmap($cropPng)
        $md5crop = (Get-FileHash -Algorithm MD5 -LiteralPath $cropPng).Hash.ToLower()
        $cropWmeas = [int]$cb.Width; $cropHmeas = [int]$cb.Height

        # 3) size == requested rect, tolerance +-1px stated out loud
        $dw = [Math]::Abs($cb.Width - $rw); $dh = [Math]::Abs($cb.Height - $rh)
        if ($dw -le $TolPx -and $dh -le $TolPx) {
            Pass ($Name + ' SIZE - crop=' + $cb.Width + 'x' + $cb.Height + ' requested=' + $rw + 'x' + $rh +
                  ' delta=' + $dw + ',' + $dh + ' px (tolerance stated: +-' + $TolPx + 'px, GDI+ rounds the rect to whole pixels)')
        } else {
            Fail ($Name + ' SIZE - crop=' + $cb.Width + 'x' + $cb.Height + ' but the request was ' + $rw + 'x' + $rh +
                  ' (delta ' + $dw + ',' + $dh + ' px beyond the stated +-' + $TolPx + 'px tolerance) - the rect that reaches Clone() is not the rect the caller asked for')
        }

        # 4) non-monochrome, sampled every 3rd px (stated, so the number is reproducible)
        $cols = @{}
        for ($qy = 0; $qy -lt $cb.Height; $qy += 3) {
            for ($qx = 0; $qx -lt $cb.Width; $qx += 3) { $cols[[int]$cb.GetPixel($qx, $qy).ToArgb()] = 1 }
        }
        if ($cols.Count -gt 1) {
            Pass ($Name + ' PIXELS - crop is not a flat fill: ' + $cols.Count + ' distinct colours over a 3px grid (' +
                  ($cb.Width * $cb.Height) + ' px, every 3rd sampled) - a blank crop would have exactly 1')
        } else {
            Fail ($Name + ' PIXELS - the crop holds ' + $cols.Count + ' distinct colour(s) over the sampled grid: that is a blank rectangle, and a blank rectangle is what a broken crop looks like when it still writes a file')
        }

        # 5) the crop md5 differs from the full frame md5
        if ($md5crop -ne $md5full) {
            Pass ($Name + ' MD5-DIFF - crop md5=' + $md5crop + ' != full-frame md5=' + $md5full + ' (so the crop is not a re-save of the whole frame)')
        } else {
            Fail ($Name + ' MD5-DIFF - crop md5 == full md5: nothing was cropped, the helper re-saved the frame and would still print an ESHOT= line')
        }

        # 6) pixel fidelity at the rect origin + far corner: the OFFSET is honoured, not just the size
        $sb2 = New-Object System.Drawing.Bitmap($fullPng)
        $p00 = $cb.GetPixel(0, 0); $q00 = $sb2.GetPixel($rx, $ry)
        $cx2 = $cb.Width - 1; $cy2 = $cb.Height - 1
        $pFF = $cb.GetPixel($cx2, $cy2); $qFF = $sb2.GetPixel($rx + $cx2, $ry + $cy2)
        $sb2.Dispose()
        if (($p00.ToArgb() -eq $q00.ToArgb()) -and ($pFF.ToArgb() -eq $qFF.ToArgb())) {
            Pass ($Name + ' OFFSET - crop(0,0)=' + $q00.Name + '@src(' + $rx + ',' + $ry + ') and crop(' + $cx2 + ',' + $cy2 + ')=' +
                  $qFF.Name + '@src(' + ($rx + $cx2) + ',' + ($ry + $cy2) + '): the requested rectangle really was the cropped one')
        } else {
            Fail ($Name + ' OFFSET - crop corner pixels do not match the requested rectangle: crop(0,0)=' + $p00.ToArgb() +
                  ' vs src(' + $rx + ',' + $ry + ')=' + $q00.ToArgb() + ' | crop far=' + $pFF.ToArgb() + ' vs src far=' + $qFF.ToArgb())
        }
        $cb.Dispose()

        # 7) FAIL-CLOSED controls: a crop that cannot happen must come back as a failure, never as a
        # silent nothing-to-see. Take-ElShot maps both of these onto the roster prefix ESHOT-CROP-FAILED=
        # (rule 10 pins that prefix as critical: in the roster AND printed), so neither can stay green.
        $rd = Crop-Dr4Rect -SourcePath $fullPng -OutPath $degPng -X 5 -Y 5 -CropW 2 -CropH 2
        if ((-not $rd.ok) -and $rd.kind -eq 'degenerate' -and (-not (Test-Path -LiteralPath $degPng))) {
            Pass ($Name + ' FAIL-CLOSED - a 2x2 request returns kind=degenerate ok=false and writes no file (' + $rd.msg + ')')
        } else {
            Fail ($Name + ' FAIL-CLOSED - a 2x2 request returned kind=' + $rd.kind + ' ok=' + $rd.ok +
                  ' file=' + [string](Test-Path -LiteralPath $degPng) + '; a degenerate crop that pretends to be fine is the downgrade-to-skip this repo already forbids')
        }

        $rt = Crop-Dr4Rect -SourcePath $fullPng -OutPath $badPng -X 4 -Y 4 -CropW 40 -CropH 40
        if ((-not $rt.ok) -and $rt.kind -eq 'threw' -and $rt.msg.Length -gt 0) {
            Pass ($Name + ' THROW-VISIBLE - a crop whose save really does throw comes back kind=threw with the exception in hand: ' +
                  $rt.msg.Substring(0, [Math]::Min(90, $rt.msg.Length)) + ' (Take-ElShot prints that as ESHOT-CROP-FAILED=, which rule 10 counts)')
        } else {
            Fail ($Name + ' THROW-VISIBLE - a throwing save returned kind=' + $rt.kind + ' ok=' + $rt.ok + ' msg=[' + $rt.msg +
                  '] - an exception that is swallowed here is exactly how the MakeRectangle leg stayed invisible')
        }

        $rm = Crop-Dr4Rect -SourcePath (Join-Path $scratch 'no-such-source.png') -OutPath $cropPng -X 1 -Y 1 -CropW 10 -CropH 10
        if ((-not $rm.ok) -and $rm.kind -eq 'missing' -and $rm.msg.Length -gt 0) {
            Pass ($Name + ' NO-SOURCE - a missing frame returns kind=missing with a reason (' + $rm.msg + '), it is not a skip')
        } else {
            Fail ($Name + ' NO-SOURCE - missing frame gave kind=' + $rm.kind + ' ok=' + $rm.ok + ' - reading a png that is not there must be a stated failure')
        }

        # 8) ANTI-VACUITY control: the pre-fix expression is replayed here and MUST throw. Without this
        # row the EXEC pass above would only prove that today's helper runs; this proves the gate can
        # still tell a working crop from the broken one, so re-introducing the bad member is a red run.
        $oldTxt = @'
param([string]$P)
$tb = New-Object System.Drawing.Bitmap($P)
$tc = $tb.Clone([System.Drawing.Rectangle]::FromRectangle([System.Drawing.Rectangle]::MakeRectangle(20, 30, 60, 40)), $tb.PixelFormat)
$null = $tc.Width
$tc.Dispose(); $tb.Dispose()
'@
        $threw = $false; $oldMsg = ''
        try { & ([ScriptBlock]::Create($oldTxt)) -P $fullPng } catch { $threw = $true; $oldMsg = ($_.Exception.Message -replace '\s+', ' ') }
        if ($threw) {
            Pass ($Name + ' ANTI-VACUITY - replaying the pre-fix line really does throw: ' +
                  $oldMsg.Substring(0, [Math]::Min(80, $oldMsg.Length)) + ' => this gate has teeth against that exact regression, it is not merely agreeing with the current file')
        } else {
            Fail ($Name + ' ANTI-VACUITY - the pre-fix MakeRectangle shape did NOT throw, so this rule stopped measuring anything and its PASS lines are decoration')
        }

        # Three evidence rows, printed unconditionally (they survive -Quiet): the artefacts this gate
        # really wrote, with size and md5, so a reader can check the green against a file instead of
        # against a sentence.
        W ($Name + ' EVIDENCE-1 source  size=' + $SW + 'x' + $SH + ' md5=' + $md5full + ' bytes=' + (Get-Item -LiteralPath $fullPng).Length + ' path=' + $fullPng)
        W ($Name + ' EVIDENCE-2 requested rect=' + $rx + ',' + $ry + ' ' + $rw + 'x' + $rh + 'px tolerance=+-' + $TolPx + 'px')
        W ($Name + ' EVIDENCE-3 crop    size=' + $cropWmeas + 'x' + $cropHmeas + ' md5=' + $md5crop +
           ' bytes=' + (Get-Item -LiteralPath $cropPng).Length + ' path=' + $cropPng +
           ' (kept on disk, outside the repo: ' + $scratch + ')')
    } catch {
        Fail ($Name + ' ABORT - the crop smoke test threw outside the helper (' + $_.Exception.GetType().Name + ': ' +
              (($_.Exception.Message -replace '\s+', ' ')) + '). It is a FAIL, not a skip: this rule exists because a broken crop used to walk past nobody noticing')
    }
}

# ---------------------------------------------------------------- rule 16: does that STATIC EVEN EXIST?
# Why this exists (T5K, same family as rule 15): rust/tests/dr4-drive.ps1's `inv:` leg carried a
# fifth fallback arm written as
#   [System.Windows.Automation.LegacyIAccessiblePatternIdentifiers]::Pattern  and
#   [System.Windows.Automation.LegacyIAccessiblePattern]
# Measured here by reflecting the type tables: UIAutomationClient exports 57 types (38 of them
# *Pattern*), UIAutomationTypes exports 69 (21 of them *PatternIdentifiers*), WindowsBase 155,
# UIAutomationProvider 27 - and the number of types in ANY of them whose name contains Legacy or
# IAccessible is ZERO. Both literals throw RuntimeException "cannot find type" at CALL time (not
# parse time: the file parses fine), and they sat inside an empty `catch { }`, so the arm never ran,
# never reported, and `INVOKED=` still printed green. That is strictly worse than rule 15's
# MakeRectangle, which at least threw loudly.
#
# The methodological point from the last ticket is the reason this is a RULE and not a grep: a
# text gate cannot see this class at all, because the member name is a plausible word. The only
# thing that grades it is asking the runtime. So this rule takes every System.*-qualified TYPE
# LITERAL out of the AST of each tool script and asks (a) does the type resolve, and (b) when the
# literal is the left side of `::`, does that static member exist on that exact type.
#
# Two AST facts that are easy to get wrong and are measured rather than assumed:
#   * there is NO StaticMemberExpressionAst in PS 5.1 - a static access is a
#     MemberExpressionAst / InvokeMemberExpressionAst whose .Expression is a TypeExpressionAst;
#   * a CAST ([SomeType]$x) is NOT a TypeExpressionAst, it is a TypeConstraintAst. Scanning only
#     TypeExpressionAst finds line 1 of the bug and misses line 2. This rule scans both.
#
# Scope, stated instead of hidden: literals whose name does not start with 'System.' are skipped,
# because they are either PS type accelerators ([Math], [regex], [IntPtr]) or types that only exist
# after the driver's own compiled Add-Type -TypeDefinition body runs (Dr4Native, ShotHarness32,
# Win32Uia, WndEnum). The gate must not compile C# (rule 14 pins that at 0), so it exempts exactly
# the type names declared inside the scanned file's own TypeDefinition bodies - read off the AST of
# that file, not a hand-written allow-list.
#
# SELF-EXCLUSION, and the evidence that it is not a convenience: this gate's own source contains
# rule 15's anti-vacuity payload - the literal broken line `[System.Drawing.Rectangle]::FromRectangle
# ([System.Drawing.Rectangle]::MakeRectangle(...))`. It lives inside a verbatim here-string, so it is
# ONE string token and produces NO TypeExpressionAst/TypeConstraintAst node. The rule therefore
# (a) does not gate this file at all, and (b) separately measures how many System.* literals this
# file's own live AST carries and reports that number, so the exemption cannot be used to hide rot
# here. The needle for the deleted arm is built by concatenation, so this file never contains the
# pattern it is scanning for (repo precedent: $script:LibName, $script:LaunchPat, and the index-78632
# self-hit T4K caught in rule 14).
function Get-LocalTypeDefNames($Ast) {
    $names = @{}
    foreach ($body in (Get-TypeDefinitionBodies $Ast)) {
        $clean = Remove-CStyleComments $body
        foreach ($m in [regex]::Matches($clean, '(?im)^\s*(?:\[[^\]]*\]\s*)*(?:public|internal|private)?\s*(?:static\s+)?(?:sealed\s+|partial\s+)*(?:class|struct|enum)\s+(?<n>[A-Za-z_][A-Za-z0-9_]*)')) {
            $names[$m.Groups['n'].Value] = $true
        }
    }
    return $names
}

# Every System.*-qualified type literal in one parsed script, graded by reflection.
# Returns @{ Sites = n; Resolved = n; MemberChecked = n; Defects = @('file:line STATUS type :: member') }.
function Measure-StaticMembers($Ast, [string]$Tag, $LocalNames) {
    $defects = @()
    $nSites = 0; $nRes = 0; $nMem = 0
    # which type literals are the LEFT SIDE of a static member access, and with what member?
    $staticMembers = @{}
    foreach ($mn in @($Ast.FindAll({ param($n)
        ($n -is [System.Management.Automation.Language.MemberExpressionAst]) -or
        ($n -is [System.Management.Automation.Language.InvokeMemberExpressionAst]) }, $true))) {
        if ($mn.Expression -is [System.Management.Automation.Language.TypeExpressionAst]) {
            $mem = ''
            try { $mem = [string]$mn.Member.Value } catch { $mem = '' }
            if ($mem -eq '') { $mem = (([string]$mn.Member.Extent.Text) -replace '["'']', '') }
            $staticMembers[[string]$mn.Expression.Extent.StartOffset] = $mem
        }
    }
    $nodes = @($Ast.FindAll({ param($n)
        ($n -is [System.Management.Automation.Language.TypeExpressionAst]) -or
        ($n -is [System.Management.Automation.Language.TypeConstraintAst]) }, $true))
    foreach ($n in $nodes) {
        $tname = ''
        try { $tname = [string]$n.TypeName } catch { $tname = '' }
        if ($tname -notmatch '^System\.') { continue }
        $nSites++
        $rt = $null
        foreach ($aa in [AppDomain]::CurrentDomain.GetAssemblies()) {
            try { $x = $aa.GetType($tname); if ($null -ne $x) { $rt = $x; break } } catch { }
        }
        $key = [string]$n.Extent.StartOffset
        $member = ''
        if ($staticMembers.ContainsKey($key)) { $member = [string]$staticMembers[$key] }
        if ($null -eq $rt) {
            $defects += ($Tag + ':' + $n.Extent.StartLineNumber + ' TYPE-UNRESOLVED [' + $tname + ']' +
                         $(if ($member -ne '') { ' ::' + $member } else { ' (cast / -is / -as operand)' }) +
                         ' - no such type is loaded on this machine, so this line can only throw')
            continue
        }
        $nRes++
        if ($member -eq '') { continue }
        $bf = [System.Reflection.BindingFlags]::Public -bor [System.Reflection.BindingFlags]::Static
        if ($member -eq 'new') { $bf = $bf -bor [System.Reflection.BindingFlags]::Instance }
        $hit = $false
        try { if (@($rt.GetMember($member, $bf)).Count -gt 0) { $hit = $true } } catch { }
        if (-not $hit -and $member -eq 'new') { try { if (@($rt.GetConstructors()).Count -gt 0) { $hit = $true } } catch { } }
        if ($hit) { $nMem++; continue }
        $real = @()
        try { $real = @($rt.GetMembers($bf) | ForEach-Object { $_.Name } | Sort-Object -Unique) } catch { }
        $tail = ($real | Select-Object -First 12) -join '/'
        $defects += ($Tag + ':' + $n.Extent.StartLineNumber + ' MEMBER-MISSING [' + $tname + ']::' + $member +
                     ' - that type exists but has no such static member; its real ones are ' + $tail)
    }
    return [pscustomobject]@{ Sites = $nSites; Resolved = $nRes; MemberChecked = $nMem; Defects = @($defects) }
}

function Check-StaticMemberReflection([string]$Dir, [string]$SelfName) {
    $Name = 'STATIC-MEMBERS'
    # Metadata loads only - see the pinned budget in rule 14. No element, no tree, no window.
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    Add-Type -AssemblyName System.Windows.Forms
    $files = @(Get-ChildItem -LiteralPath $Dir -Filter '*.ps1' -File | Sort-Object Name |
               Where-Object { $_.Name -ne $SelfName })
    if ($files.Count -eq 0) {
        Fail ($Name + ' SCAN - no *.ps1 to grade in ' + $Dir + ' (after excluding ' + $SelfName + '), so this rule is measuring nothing')
        return
    }
    $totSites = 0; $totRes = 0; $totMem = 0; $allDefects = @()
    foreach ($f in $files) {
        $rr = Read-Script $f.FullName
        if ($rr.Errors.Count -gt 0) {
            $allDefects += ($f.Name + ' PARSE - ' + $rr.Errors.Count + ' parse error(s): every type literal in this file is ungradeable while it does not parse')
            continue
        }
        $ln = Get-LocalTypeDefNames $rr.Ast
        $m = Measure-StaticMembers $rr.Ast $f.Name $ln
        $totSites += $m.Sites; $totRes += $m.Resolved; $totMem += $m.MemberChecked
        $allDefects += @($m.Defects)
        $line = $Name + ' ' + $f.Name + ' - ' + $m.Sites + ' System.* type literal(s) taken from the AST, ' +
                $m.Resolved + ' resolve against the loaded assemblies (' + $m.MemberChecked +
                ' static-member lookups succeeded), defects=' + $m.Defects.Count +
                $(if ($ln.Count -gt 0) { '; ' + $ln.Count + ' local Add-Type type name(s) exempted from the same file' } else { '' })
        # A per-file line that says PASS while printing defects=2 is the same fake green this ticket exists
        # to kill: the verdict was already carried by the aggregate DEFECTS Fail, but the readable line
        # lied. Same condition, same colour.
        if ($m.Defects.Count -gt 0) {
            Fail ($line + ' :: ' + (($m.Defects | Select-Object -First 3) -join ' || '))
        } else {
            Pass $line
        }
    }
    # the gate's own file: reported, not gated, so the exemption cannot hide rot here.
    $selfPath = Join-Path $Dir $SelfName
    $selfSites = -1; $selfDefects = -1
    if (Test-Path -LiteralPath $selfPath) {
        $sr = Read-Script $selfPath
        $sm = Measure-StaticMembers $sr.Ast $SelfName (Get-LocalTypeDefNames $sr.Ast)
        $selfSites = $sm.Sites; $selfDefects = $sm.Defects.Count
    }

    # --- ANTI-VACUITY: the verifier must reject the two shapes this ticket is about.
    # Written to a temp file OUTSIDE the repo and graded by the very same Measure-StaticMembers.
    $scratch = Join-Path ([System.IO.Path]::GetTempPath()) ('t5k-statics-' + $PID)
    $badPs = Join-Path $scratch 't5k-bad-shapes.ps1'
    # the dead arm, replayed verbatim, plus the rule-15 shape and a member that lives on another type
    $needleA = 'Legacy' + 'IAccessiblePatternIdentifiers'
    $needleB = 'Legacy' + 'IAccessiblePattern'
    $badSrc = '@(' + "`r`n" +
        '  $p = [System.Windows.Automation.' + $needleA + ']::Pattern' + "`r`n" +
        '  [void]([System.Windows.Automation.' + $needleB + ']$p).DoDefaultAction()' + "`r`n" +
        '  $q = [System.Drawing.Rectangle]::MakeRectangle(1, 2, 3, 4)' + "`r`n" +
        '  $s = [System.Drawing.Rectangle]::FromRectangle($q)' + "`r`n" +
        '  $t = [System.Drawing.Rectangle]::FromLTRB(1, 2, 3, 4)' + "`r`n" +
        ')'
    try {
        if (-not (Test-Path -LiteralPath $scratch)) { $null = New-Item -ItemType Directory -Force -Path $scratch }
        if (Test-Path -LiteralPath $badPs) { Remove-Item -LiteralPath $badPs -Force }
        $bytesBad = @([System.Text.Encoding]::UTF8.GetBytes($badSrc))
        [System.IO.File]::WriteAllBytes($badPs, ([byte[]](0xEF, 0xBB, 0xBF) + [byte[]]$bytesBad))
        $br = Read-Script $badPs
        $bm = Measure-StaticMembers $br.Ast 't5k-bad-shapes.ps1' (Get-LocalTypeDefNames $br.Ast)
        # Substring checks, NOT -like: the defect strings carry real type literals in square brackets and
        # in PowerShell -like a bracket range IS a wildcard class, so '*MEMBER-MISSING [System.Drawing.X]::M*'
        # can never match (measured: hits=0 against a defect that contains that exact text). Each entry below
        # is built with its own statement because in PS the comma operator binds tighter than +, which turns
        # `@('a' + 'b', 'c')` into a single glued element (measured: $want.Count = 1) - that is exactly the
        # kind of silent self-nullification this ticket is about, so it is pinned here instead of avoided.
        $want = @()
        $want = $want + (@{ label = ('TYPE-UNRESOLVED ' + $needleA); must = @('TYPE-UNRESOLVED', $needleA) })
        $want = $want + (@{ label = ('TYPE-UNRESOLVED ' + $needleB); must = @('TYPE-UNRESOLVED', ($needleB + ']')) })
        $want = $want + (@{ label = 'MEMBER-MISSING [Rectangle]::MakeRectangle';
                            must = @('MEMBER-MISSING', '[System.Drawing.Rectangle]::MakeRectangle') })
        $want = $want + (@{ label = 'MEMBER-MISSING [Rectangle]::FromRectangle';
                            must = @('MEMBER-MISSING', '[System.Drawing.Rectangle]::FromRectangle') })
        $miss = @()
        foreach ($w in $want) {
            $found = $false
            foreach ($dtext in @($bm.Defects)) {
                $all = $true
                foreach ($frag in @($w.must)) { if (-not ([string]$dtext).Contains([string]$frag)) { $all = $false; break } }
                if ($all) { $found = $true; break }
            }
            if (-not $found) { $miss += [string]$w.label }
        }
        if ($bm.Defects.Count -lt 4) {
            Fail ($Name + ' ANTI-VACUITY - the replay of 4 known-bad shapes produced only ' + $bm.Defects.Count +
                  ' defect(s), it must produce at least 4: ' + (($bm.Defects | Select-Object -First 6) -join ' || '))
        } elseif ($miss.Count -gt 0) {
            Fail ($Name + ' ANTI-VACUITY - the replay ran but these defects were NOT reported: ' + ($miss -join ' ; ') +
                  ' - the verifier agrees with whatever is on disk instead of judging it')
        } else {
            Pass ($Name + ' ANTI-VACUITY - the same verifier, on a file this rule wrote itself, reports all 4 planted shapes: ' +
                  (($bm.Defects | ForEach-Object { (($_ -split ' ')[1]) }) -join ' / ') +
                  ' and still counts the one control that IS real ([Rectangle]::FromLTRB) as a non-defect (' +
                  $bm.Sites + ' sites, ' + $bm.Defects.Count + ' defects)')
        }
    } catch {
        Fail ($Name + ' ANTI-VACUITY ABORT - could not replay the bad shapes (' + $_.Exception.GetType().Name + ': ' +
              (($_.Exception.Message -replace '\s+', ' ')) + '); a static rule with no negative control is decoration')
    }

    # --- needle self-hit measurement: prove this file does not contain the pattern it scans for.
    $selfR = $null
    if (Test-Path -LiteralPath $selfPath) { $selfR = Read-Script $selfPath }
    if ($null -eq $selfR) {
        Fail ($Name + ' SELF-CHECK - ' + $SelfName + ' is not inside the directory it scans, so the self-exclusion cannot be measured')
    } else {
        $hits = [regex]::Matches($selfR.Code, [regex]::Escape($needleA)).Count +
                [regex]::Matches($selfR.Code, [regex]::Escape($needleB)).Count
        if ($hits -gt 0) {
            Fail ($Name + ' SELF-HIT - the needle (' + $needleA + '/' + $needleB + ') appears ' + $hits +
                  ' time(s) in this gate''s own CODE stream: a text-shaped rule built on it would grade its own source, build the needle by concatenation instead')
        } else {
            Pass ($Name + ' SELF-HIT measured: 0 occurrences of the ' + $needleA + '/' + $needleB +
                  ' needle in this gate''s own code stream (both names are assembled by concatenation here, same precedent as $script:LibName)')
        }
        $oldShape = [regex]::Matches($selfR.Code, 'MakeRectangle').Count
        if ($oldShape -gt 0) {
            Pass ($Name + ' SELF-EXCLUSION measured: this gate''s code stream carries ' + $oldShape +
                  ' MakeRectangle token(s) - all of them inside rule 15''s verbatim here-string, which the AST keeps as ONE string token and therefore produces no TypeConstraintAst/TypeExpressionAst node. That is why the gate is exempt from rule 16 (a negative control must not be able to flip the verdict) and it is an AST fact, not a filename convenience')
        } else {
            Pass ($Name + ' SELF-EXCLUSION: 0 MakeRectangle tokens in this gate''s code stream, so the exemption is currently load-bearing only for rule 15''s replay payload')
        }
        Pass ($Name + ' SELF-SCAN reported, not gated: this gate''s own live AST carries ' + $selfSites +
              ' System.* type literal(s) with ' + $selfDefects + ' defect(s) - printed so the exemption cannot hide rot here')
        if ($selfDefects -gt 0) {
            Fail ($Name + ' SELF-SCAN - ' + $SelfName + ' itself has ' + $selfDefects +
                  ' bad static member(s) in its own live AST, so the rule 16 exemption is being used to hide rot in the gate')
        }
    }

    # --- the deleted arm must not come back, and the leg that replaced it must be in the roster.
    $drv = Join-Path $Dir 'dr4-drive.ps1'
    if (Test-Path -LiteralPath $drv) {
        $dr = Read-Script $drv
        $dm = Measure-StaticMembers $dr.Ast 'dr4-drive.ps1' (Get-LocalTypeDefNames $dr.Ast)
        $back = @($dm.Defects | Where-Object { $_ -like ('*' + $needleA + '*') -or $_ -like ('*' + $needleB + '*') })
        if ($back.Count -gt 0) {
            Fail ($Name + ' REGRESSION - the ' + $needleA + ' arm is back in dr4-drive.ps1 (' + ($back -join ' || ') +
                  '); it cannot work on PS 5.1 and inside an empty catch it is a SILENT dead arm')
        } else {
            Pass ($Name + ' REGRESSION - 0 unresolvable UIA type literals left in dr4-drive.ps1: the inv: leg now walks only the ' +
                  '4 arms that really exist (Invoke/Toggle/SelectionItem/ExpandCollapse)')
        }
        $dn = Get-NormCode $dr.Code
        $litNotInvoked = Lit('NOTINVOKED=')
        # Measured from the AST-read roster, not from "the string appears somewhere in the file": a print
        # site alone also contains the quoted prefix, so a Contains() test can never notice the prefix being
        # deleted from $script:Dr4FailureTokens (proved by mutation M3 - rule 10 went red, this pin stayed
        # green, which is precisely the decoration this ticket refuses).
        $drRoster = @(Get-ArrayLiteral $dn $script:ExitContract['dr4-drive.ps1'].roster)
        $inRoster = @($drRoster | Where-Object { $_ -eq 'NOTINVOKED=' }).Count
        if ($dn.Contains('NOTINVOKED=') -and $inRoster -gt 0) {
            Pass ($Name + ' FAIL-CLOSED - dr4-drive.ps1 prints NOTINVOKED= and names it in ' +
                  $script:ExitContract['dr4-drive.ps1'].roster + ' (roster read off the AST: ' + $drRoster.Count +
                  ' prefix(es) in scope), so "no pattern took" is a counted problem instead of a green INVOKED= line')
        } else {
            Fail ($Name + ' FAIL-CLOSED - dr4-drive.ps1 lost the NOTINVOKED= prefix (print site and/or roster entry; inRoster=' +
                  $inRoster + '). Without it a driver with no working arm prints INVOKED= with an empty pattern= and the round stays green - the exact fake green this ticket deletes')
        }
    } else {
        Fail ($Name + ' TARGET - dr4-drive.ps1 is not in the scanned directory, the inv: leg cannot be graded')
    }

    # --- the verdict
    if ($allDefects.Count -gt 0) {
        Fail ($Name + ' DEFECTS - ' + $allDefects.Count + ' static member / type literal(s) in the tool scripts do not exist on this machine:')
        foreach ($d in $allDefects) { W ('       ' + $d) }
        W ('       (a call that cannot bind either throws, or - inside an empty catch - silently never runs; both end the step nobody saw)')
    } else {
        Pass ($Name + ' VERDICT - all ' + $totSites + ' System.*-qualified type literals across ' + $files.Count +
              ' scanned driver/gate scripts resolve, and all ' + $totMem + ' static-member lookups on them exist (' +
              $totRes + ' types resolved, defects=0)')
    }
}

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

    # 6 + 7) the native surface of this file: computed ABI shapes, then module attribution.
    # Both run on the C# that Add-Type actually compiles, comments removed - which is the
    # lesson of DR5: the old gate looked at WORDS, so `DllImport("gdi32.dll") PrintWindow`
    # passed and a whole round of screenshots was worthless.
    $bodies = @(Get-TypeDefinitionBodies $r.Ast)
    if ($bodies.Count -eq 0) {
        Pass ($name + ' NATIVE n/a (no Add-Type -TypeDefinition body to audit)')
    }
    foreach ($b in $bodies) {
        Check-CSharpAbi $name $b
        Check-CSharpDllImports $name $b
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
        # The flag bit alone is NOT the contract (ledger DR5): matching only the WORDS
        # 'PrintWindow' + 'PW_RENDERFULLCONTENT' let a DllImport("gdi32.dll") pass, and
        # PrintWindow is exported by user32, not gdi32 - so the whole capture core was
        # producing untrustworthy frames behind a green gate. Pin the MODULE, per declaring
        # line, over the CODE stream only ($r.Code has comments stripped, so a corrected
        # comment can never satisfy this gate either - that was the same class of miss).
        $pwDecls = @(($r.Code -split "\r?\n") | Where-Object { $_ -match 'DllImport' -and $_ -match 'PrintWindow' })
        $pwUser = @($pwDecls | Where-Object { $_ -match 'DllImport\(\s*[\x27"]user32\.dll[\x27"]' })
        $pwGdi = @($pwDecls | Where-Object { $_ -match 'DllImport\(\s*[\x27"]gdi32\.dll[\x27"]' })
        if ($pwDecls.Count -eq 0) {
            Fail 'shot_harness.ps1 has no DllImport PrintWindow declaration at all - captures would be impossible'
        } elseif ($pwGdi.Count -gt 0) {
            Fail ('shot_harness.ps1 declares PrintWindow against gdi32.dll (' + $pwGdi.Count +
                   ' line(s)) - PrintWindow is a user32 export, so every captured frame is untrustworthy')
        } elseif ($pwUser.Count -ne $pwDecls.Count) {
            Fail ('shot_harness.ps1 PrintWindow DllImport names the wrong module: user32=' + $pwUser.Count +
                   ' of ' + $pwDecls.Count + ' declaring line(s)')
        } else {
            Pass ('shot_harness.ps1 PrintWindow is declared against user32.dll on all ' + $pwUser.Count +
                  ' declaring line(s) (gdi32 = 0, the anti-untrustworthy-frame pin)')
        }
        if ($r.Code -match 'PER_MONITOR_AWARE_V2|DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2') {
            Pass 'shot_harness.ps1 sets PMv2 itself (task #22 DPI trap)'
        } else {
            Fail 'shot_harness.ps1 lost its own DPI-awareness call'
        }
        Check-ShotHarnessAbiContract $r
    }

    # 10-13) the T3K knife: exit-code contract, kernel default, find channels, log caliber.
    # Each of these is a STRUCTURE/NUMBER check on the code stream, see rules 10-13 in the header.
    Check-ExitCodeContract $r $name
    Check-KernelDefault $r $name
    Check-FindChannels $r $name
    Check-LogCaliber $r $name
}

# ---------------------------------------------------------------- main
if ($Dir -eq '') { $Dir = $PSScriptRoot }
if (-not (Test-Path -LiteralPath $Dir)) { Fail ('directory not found: ' + $Dir); W 'SELFTEST-TOOLS RESULT=error'; exit 1 }
$Dir = (Resolve-Path -LiteralPath $Dir).Path
W ('SELFTEST-TOOLS start=' + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss') + ' dir=' + $Dir +
    ' (headless except rules 15 and 16: no exe is started, no window is opened, no screen/PrintWindow capture, no UIA tree walk and no AutomationElement, no SendInput;' +
    ' the one driver function this gate EXECUTES is the crop helper (rule 15) on a bitmap it paints itself (synthetic pixels, not a frame of the app), and rule 16 only asks the runtime whether each System.* type literal and static member in the drivers exists (metadata, no element);' +
    ' the native ABI is computed from the C# source and the' +
    ' module table is proved by READING the PE export tables of the system DLLs, never by loading them)')
W ('SELFTEST-TOOLS host=' + [System.Environment]::OSVersion.Version.ToString() + ' arch=' + $env:PROCESSOR_ARCHITECTURE +
    ' ptr=' + [IntPtr]::Size + ' ps=' + $PSVersionTable.PSVersion.ToString() +
    ' (the 40B INPUT total is an x64 number: pointer=' + [IntPtr]::Size + ')')

W ('SELFTEST-TOOLS VER=' + $script:ToolsSelftestVersion + ' rules=' +
    '1:parse 2:bom/utf8 3:entrypoints 4:no-tmp-deps 5:no-silent-fallback 6:native-abi' +
    ' 7:native-homes(+oracle,PE-read) 8:abi-contract 9:coverage' +
    ' 10:exit-code-contract 11:kernel-default+launch-roster 12:find-channel' +
    ' 13:log-caliber+no-stale-claim 14:coverage-self-declaration' +
    ' 15:crop-smoke-run(headless, executes Crop-Dr4Rect)' +
    ' 16:static-member-reflection(AST + reflection, grades every System.* type literal and ::member)')
$files = @(Get-ChildItem -LiteralPath $Dir -Filter '*.ps1' -File | Sort-Object Name)
if ($files.Count -eq 0) { Fail 'no *.ps1 found in tests/ - the tool library vanished' }
# Rule 14 + rule 15 in the banner: what this green pass count is evidence ABOUT, printed every run.
# Five tokens, pinned by name and value in $script:ScopeTokens; UNCOVERED ones are proved by
# arithmetic in Check-CoverageSelfDecl (0 product paths, 0 compiled Add-Type body, 0 cargo invocation).
# SCOPE-CROP-SYNTH is the token T4K added because rule 15 executes something: it is a COVERED claim
# about ONE crop call on synthetic pixels, and it deliberately does NOT widen SCOPE-CAPTURE.
W ('SCOPE-TOOLING=COVERED ' + $files.Count + ' *.ps1 in ' + $Dir + ' (parse, BOM/UTF-8, entry points, native ABI shapes, DllImport modules, exit-code/kernel/find/log structure)')
W ('SCOPE-PRODUCT-CODE=UNCOVERED the Rust fork sources are never opened by this gate (rule 14 measures 0 product-source path references in its own code stream), so nothing here proves a line of product code is correct')
W ('SCOPE-CAPTURE=UNCOVERED no window is opened and no frame of the app is captured here (rule 14 measures 0 compiled Add-Type body, so PrintWindow/SendInput are not even reachable), so pass=N says nothing about whether the screenshot path produces good pixels of the product - rule 15 runs a crop on synthetic pixels and that is claimed separately as SCOPE-CROP-SYNTH')
W ('SCOPE-CROP-SYNTH=COVERED rule 15 dot-sources Crop-Dr4Rect out of dr4-drive.ps1 and runs it on a 200x120 bitmap this gate paints: the per-element crop call exists, does not throw, and produces the requested rect - nothing more (no app frame, no image quality, no product code)')
W ('SCOPE-PRODUCT-BUILD=UNCOVERED no cargo is invoked (the cargo lane belongs to whoever holds it), so a green gate is not a build verdict')
foreach ($f in $files) {
    W ('--- ' + $f.Name + ' (' + $f.Length + ' B, ' + @([System.IO.File]::ReadAllLines($f.FullName)).Count + ' lines) ---')
    Check-File $f.FullName
}

# ---------------------------------------------------------------- rule 7b: prove the truth table
# The per-file NATIVE-HOME checks above compare a declaration against $script:NativeHome.
# This one compares that table against the machine: it reads the PE export directory of each
# named DLL as DATA (no LoadLibrary, no process, no window) and demands every row resolve and
# every negative control NOT resolve. If the oracle is lying, the red line says so.
Check-NativeOracle

# Rule 11b + 14: the offline launch lock (who may start a process, how many sites, with what
# target) and this gate's own scope declaration. Both read the FILE ON DISK right now, so a
# driver edited after this run cannot hide behind a previous green.
Check-ProcessLaunchRoster $Dir

# Rule 15 (T4K): the crop leg is EXECUTED here, on synthetic pixels, before the gate grades its own
# scope declaration - so SCOPE-CROP-SYNTH=COVERED is backed by a run that already happened in this
# process, not by a sentence.
Check-CropSmokeRun $Dir $script:CropTolPx

# Rule 16 (T5K): every System.* type literal and ::static member in the drivers is asked of the
# RUNTIME, not of a grep. Rule 15 killed one landmine this way; the `inv:` leg's silent
# LegacyIAccessible arm is the same family with the volume turned down to zero (empty catch, no
# throw escapes, still printed green), which is why a text gate can never have found it.
Check-StaticMemberReflection $Dir 'selftest_tools.ps1'

$gate = Join-Path $Dir 'selftest_tools.ps1'
if (Test-Path -LiteralPath $gate) { Check-CoverageSelfDecl (Read-Script $gate) 'selftest_tools.ps1' }
else { Fail 'coverage: selftest_tools.ps1 is not inside the directory it is scanning - the SCOPE banner cannot be audited' }

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
