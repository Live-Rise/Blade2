<#
  shot_harness.ps1 - git-tracked screenshot / foreground helper for the WinUI 3 fork.

  WHY THIS FILE LIVES IN rust/tests/ (ledger #143)
    rust/tests/gui_uia.ps1:189 dot-sources  <repo>/rust/tmp/shotlib.ps1  and calls
    Invoke-ShotForeground at :219. rust/tmp/ is listed in .gitignore:11, so a sync pass
    wipes it and every later run silently degrades to gui_uia.ps1's "legacy nudge"
    (SwitchToThisWindow + ShowWindow(5)), which cannot beat the Windows foreground lock
    from a background console parent: CopyFromScreen then photographs whatever the
    operator was looking at instead of this fork. A reusable tool must not live in a
    declared-ignorable directory. This file is that tool, tracked by git.

    It is a DROP-IN for tmp/shotlib.ps1: same public names and parameter names as the
    copies consumed today by gui_uia.ps1:219 and tmp/qa5-drive.ps1:286-291
    (Invoke-ShotForeground / Invoke-QaShot / Get-QaShotStats). Nothing here is invented;
    the interface was read off those consumers.

    STATUS 2026-09-25 (ledger #143, agent TL2): gui_uia.ps1 dot-sources THIS file only.
    The old <repo>/rust/tmp/shotlib.ps1 is RETIRED - every mention of it below is history,
    kept so a reader can tell why the interface looks the way it does. No script under
    rust/tests/ may load a helper out of rust/tmp/ again: that directory is gitignored
    (.gitignore:11) and has been wiped twice. rust/tmp remains legal as an OUTPUT directory
    (screenshots, logs), never as a source of code.

  PUBLIC API (consumed by gui_uia.ps1 - do not rename)
    Invoke-ShotForeground -Hwnd <IntPtr> -Tag <string> -Log <scriptblock(param $m)>
        -> NOTE STRING. gui_uia.ps1:235 concatenates it straight after 'match=<bool>' on
           its FOREGROUND= evidence line, so it MUST start with a space
           (e.g. " fg-match=yes(tech=TechAltForeground try=1)"). '' only on total failure.
           Rung ladder + 'FOREGROUND-BREAK <tag> tech=<rung> try=<n> MATCH target=<hwnd>'
           log lines keep the original shape (gui_uia.ps1:180-183 documents it).
           -Capture makes it ALSO grab a PNG whose path is derived from -Tag, and appends
           ' shot=<file> px=WxH md5=<8hex> unique=N mean=M rms=R' (+ ' BAD=<why>') to the
           note. Without -Capture nothing is written, which is what gui_uia.ps1 wants.

    Invoke-QaShot -Hwnd <IntPtr> (-Path <png> | -Tag <tag> [-OutRoot <dir>])
                  [-Client] [-Log <sb>]
        -> resolved png path, or $null. gdi32!PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT=2).
           The +2 bit is mandatory: a WinUI 3 window is a DirectComposition/DXGI host and
           without it PrintWindow hands back an all-white frame - that is task #35's root
           cause and the reason gui_uia.ps1:801 gave up on PrintWindow entirely.

    Get-QaShotStats -Path <png>
        -> hashtable {w,h,unique,mean,rms,gray,bad,bytes,md5} . Keys are read verbatim by
           tmp/qa5-drive.ps1:292, so their names and thresholds are frozen.

    Reset-ShotRound            -> forgets seen md5s; starts a new admissibility round.
    $script:ShotHarnessVersion -> 'shot_harness.ps1/1' (prove which copy is loaded).

  ADMISSIBILITY RULES THIS LIBRARY SELF-PROVES (zero tolerance for wasted frames)
    1) file exists and is > 0 bytes                 -> bad='missing' / 'zero-byte'
    2) not near-monochrome (rms>=6 and unique>4)    -> bad='near-monochrome'
       also 'near-black' (mean<8) / 'near-white' (mean>247) - a stale or blank frame
    3) md5 must be NEW within the round; a repeat is a frozen frame, so it is logged as
       'SHOT-MD5-DUP <file> same-as=<earlier tag>' and returned as ' md5dup=<earlier>'
    Every one of these is reported through -Log, never thrown: a helper that aborts the
    parent run destroys the UIA evidence collected before it.

  DPI (task #22's trap)
    PER_MONITOR_AWARE_V2 is set HERE, from our own P/Invoke, before any geometry call.
    tmp/shotlib.ps1:115 borrows the [Win32Uia] type that only exists while gui_uia.ps1 is
    the host, so dot-sourcing it standalone leaves the process DPI-virtualised and every
    pixel/logical coordinate pair disagrees on this 200%-scaled panel.
    SetProcessDpiAwarenessContext may be called only once per process: gui_uia.ps1:171
    already did it on that path, so a FALSE return here is expected and harmless.

  SELF-TEST (does NOT start the app under test - no blade2-rs.exe, no fake_dsh.exe)
    powershell -NoProfile -ExecutionPolicy Bypass -File rust\tests\shot_harness.ps1 -SelfTest
    Photobombs a harmless already-existing window (our own console, else the foreground
    window, else Program Manager), re-reads md5 twice to prove stability, then feeds a
    synthetic pure-black frame through the stats to prove the variance gate catches it.
#>
[CmdletBinding()]
param(
    [switch]$SelfTest,
    [int]$SelfTestRounds = 2
)

$ErrorActionPreference = 'Stop'
$script:ShotHarnessVersion = 'shot_harness.ps1/1'

# ---------------------------------------------------------------- native surface
# Own type name (ShotHarness32) + load guard: this file may be dot-sourced into a process
# that already loaded tmp/shotlib.ps1's ShotLib32, and Add-Type of a duplicate type name is
# a hard error that would take the parent harness down.
if (-not ('ShotHarness32' -as [type])) {
    Add-Type -AssemblyName System.Drawing
    Add-Type -TypeDefinition @'
using System;
using System.Drawing;
using System.Runtime.InteropServices;
public static class ShotHarness32 {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT {
        public uint type; public ulong padding; public INPUTUNION u;
    }
    [StructLayout(LayoutKind.Explicit)] public struct INPUTUNION {
        [FieldOffset(0)] public KEYBDINPUT ki;
        [FieldOffset(0)] public MOUSEINPUT mi;
    }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {
        public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo;
    }
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {
        public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; IntPtr dwExtraInfo_unused;
    }
    [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint n, INPUT[] p, int cb);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint f);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr FindWindow(string cls, string name);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool join);
    [DllImport("user32.dll")] public static extern bool AllowSetForegroundWindow(uint pid);
    [DllImport("user32.dll")] public static extern bool LockSetForegroundWindow(uint v);
    [DllImport("gdi32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("shcore.dll")] public static extern int SetProcessDpiAwareness(int a);

    public const uint VK_MENU = 0x12;
    public const uint KEYEVENTF_KEYUP = 2;
    public const uint PW_CLIENTONLY = 1;
    public const uint PW_RENDERFULLCONTENT = 2;
    public const int DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = -4;

    // One ALT down/up via SendInput: the documented way to release the foreground lock
    // without disturbing the target. Returns the number of events actually injected.
    public static uint TapAlt() {
        INPUT[] seq = new INPUT[2];
        seq[0].type = 1;
        seq[0].u.ki.wVk = VK_MENU; seq[0].u.ki.dwFlags = 0;
        seq[1].type = 1;
        seq[1].u.ki.wVk = VK_MENU; seq[1].u.ki.dwFlags = KEYEVENTF_KEYUP;
        return SendInput((uint)seq.Length, seq, Marshal.SizeOf(typeof(INPUT)));
    }
    public static void AltActivate(IntPtr h) {
        uint fgPid = 0, myPid = 0;
        IntPtr fg = GetForegroundWindow();
        uint fgTid = GetWindowThreadProcessId(fg, out fgPid);
        uint myTid = GetWindowThreadProcessId(h, out myPid);
        uint cur = GetCurrentThreadId();
        bool joined = false;
        try {
            if (fgTid != 0 && fgTid != cur) { joined = AttachThreadInput(cur, fgTid, true); }
            if (joined) { AllowSetForegroundWindow(myPid == 0 ? 0xFFFFFFFFu : myPid); }
            if (IsIconic(h)) { ShowWindow(h, 9); }              // SW_RESTORE
            SetForegroundWindow(h);
            BringWindowToTop(h);
            TapAlt();
            SetForegroundWindow(h);
            BringWindowToTop(h);
        } finally {
            if (joined) { AttachThreadInput(cur, fgTid, false); }
        }
    }
}
'@
}

# PMv2 first, before any GetWindowRect / bitmap sizing. Every variant is guarded: a second
# call returns FALSE, shcore's shim throws on Win10+, and SetProcessDPIAware is only a
# system-WiDP promise - best effort, in that order, never fatal.
$script:ShotDpiSet = $false
try { $script:ShotDpiSet = [bool]([ShotHarness32]::SetProcessDpiAwarenessContext([IntPtr]([ShotHarness32]::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2))) } catch { }
if (-not $script:ShotDpiSet) {
    try { $script:ShotDpiSet = ([ShotHarness32]::SetProcessDpiAwareness(2) -eq 0) } catch { }
}
if (-not $script:ShotDpiSet) {
    try { $script:ShotDpiSet = [bool]([ShotHarness32]::SetProcessDPIAware()) } catch { }
}

$script:ShotMd5Seen = @{}

function Write-ShotLog([scriptblock]$Log, [string]$m) {
    if ($null -ne $Log) { try { & $Log $m } catch { } }
}
function Reset-ShotRound { $script:ShotMd5Seen = @{} }

function Get-ShotWindowTitle([IntPtr]$h) {
    try {
        $len = [ShotHarness32]::GetWindowTextLength($h)
        if ($len -le 0) { return '' }
        $sb = New-Object System.Text.StringBuilder($len + 2)
        [void][ShotHarness32]::GetWindowText($h, $sb, $sb.Capacity)
        return $sb.ToString()
    } catch { return '' }
}

# -Tag decides the file name. rust/tmp/shots/ is the default so evidence never dirties git
# (rust/tmp/ is ignored), while -OutRoot lets a caller aim at a tracked dir instead.
function Get-ShotPathFromTag([string]$Tag, [string]$OutRoot) {
    if ([string]::IsNullOrWhiteSpace($OutRoot)) {
        $OutRoot = Join-Path (Split-Path -Parent $PSScriptRoot) 'tmp\shots'
    }
    $safe = (($Tag -replace '[^A-Za-z0-9._-]+', '-') -replace '^-+|-$+', '')
    if ($safe -eq '') { $safe = 'shot' }
    if ($safe -notmatch '\.png$') { $safe = $safe + '.png' }
    return (Join-Path $OutRoot $safe)
}

# ---------------------------------------------------------------- Get-QaShotStats
function Get-QaShotStats {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Path)
    $out = @{ w = 0; h = 0; unique = -1; mean = -1; rms = -1; gray = $true; bad = 'unreadable'; bytes = 0; md5 = '' }
    if (-not (Test-Path -LiteralPath $Path)) { $out.bad = 'missing'; return $out }
    try { $out.bytes = [long](Get-Item -LiteralPath $Path).Length } catch { }
    if ($out.bytes -eq 0) { $out.bad = 'zero-byte'; return $out }
    try { $out.md5 = (Get-FileHash -Algorithm MD5 -LiteralPath $Path).Hash.ToLower() } catch { }
    $bmp = $null
    try {
        $bmp = New-Object System.Drawing.Bitmap($Path)
        $w = $bmp.Width; $h = $bmp.Height
        $out.w = $w; $out.h = $h
        $seen = New-Object 'System.Collections.Generic.HashSet[uint32]'
        $sum = 0.0; $sumsq = 0.0; $n = 0
        $stepX = [Math]::Max(1, [int]($w / 200)); $stepY = [Math]::Max(1, [int]($h / 200))
        for ($y = 0; $y -lt $h; $y += $stepY) {
            for ($x = 0; $x -lt $w; $x += $stepX) {
                $c = $bmp.GetPixel($x, $y)
                [void]$seen.Add([uint32](($c.R -shl 16) -bor ($c.G -shl 8) -bor $c.B))
                $lum = (0.299 * $c.R) + (0.587 * $c.G) + (0.114 * $c.B)
                $sum += $lum; $sumsq += ($lum * $lum); $n++
            }
        }
        if ($n -eq 0) { $out.bad = 'no-samples'; return $out }
        $mean = $sum / $n
        $var = ($sumsq / $n) - ($mean * $mean)
        if ($var -lt 0) { $var = 0 }
        $out.mean = [Math]::Round($mean, 1)
        $out.rms = [Math]::Round([Math]::Sqrt($var), 1)
        $out.unique = $seen.Count
        $out.gray = ($seen.Count -le 24)
        if ($out.rms -lt 6 -or $seen.Count -le 4) { $out.bad = 'near-monochrome' }
        elseif ($out.mean -lt 8) { $out.bad = 'near-black' }
        elseif ($out.mean -gt 247) { $out.bad = 'near-white' }
        else { $out.bad = '' }
        return $out
    } catch {
        $out.bad = 'throw:' + (($_.Exception.Message -replace '\s+', ' '))
        return $out
    } finally {
        if ($null -ne $bmp) { try { $bmp.Dispose() } catch { } }
    }
}

# md5 uniqueness inside a round: a byte-identical repeat means the compositor never
# repainted, i.e. the frame is worthless as evidence even though it is a valid PNG.
function Resolve-ShotRound([string]$Path, [string]$Md5, [scriptblock]$Log) {
    if ($Md5 -eq '') { return '' }
    if ($script:ShotMd5Seen.ContainsKey($Md5)) {
        $earlier = [string]$script:ShotMd5Seen[$Md5]
        Write-ShotLog $Log ('SHOT-MD5-DUP ' + [System.IO.Path]::GetFileName($Path) + ' same-as=' + $earlier)
        return $earlier
    }
    $script:ShotMd5Seen[$Md5] = [System.IO.Path]::GetFileName($Path)
    return ''
}

# ---------------------------------------------------------------- Invoke-QaShot
function Invoke-QaShot {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][IntPtr]$Hwnd,
        [string]$Path = '',
        [string]$Tag = 'shot',
        [string]$OutRoot = '',
        [switch]$Client,
        [scriptblock]$Log
    )
    if ($Hwnd -eq [IntPtr]::Zero) { Write-ShotLog $Log 'QASHOT-SKIPPED zero-hwnd'; return $null }
    if ($Path -eq '') { $Path = Get-ShotPathFromTag $Tag $OutRoot }

    $r = New-Object ShotHarness32+RECT
    $ok = $false
    try {
        if ($Client) { $ok = [ShotHarness32]::GetClientRect($Hwnd, [ref]$r) }
        else { $ok = [ShotHarness32]::GetWindowRect($Hwnd, [ref]$r) }
    } catch { $ok = $false }
    if (-not $ok) { Write-ShotLog $Log ('QASHOT-SKIPPED no-window-rect hwnd=' + $Hwnd.ToInt64()); return $null }
    $w = [Math]::Max(1, $r.Right - $r.Left); $h = [Math]::Max(1, $r.Bottom - $r.Top)
    if ($w -lt 8 -or $h -lt 8) { Write-ShotLog $Log ('QASHOT-SKIPPED degenerate ' + $w + 'x' + $h); return $null }

    try {
        $dir = [System.IO.Path]::GetDirectoryName($Path)
        if ($dir -ne '' -and -not (Test-Path -LiteralPath $dir)) {
            New-Item -ItemType Directory -Force -Path $dir | Out-Null
        }
    } catch { Write-ShotLog $Log ('QASHOT-MKDIR-THREW=' + (($_.Exception.Message -replace '\s+', ' '))) }

    $bmp = $null; $g = $null; $hdc = [IntPtr]::Zero; $hdcTaken = $false
    try {
        $bmp = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $hdc = $g.GetHdc(); $hdcTaken = $true
        $flags = [uint32]([ShotHarness32]::PW_RENDERFULLCONTENT)
        if ($Client) { $flags = [uint32]($flags -bor [ShotHarness32]::PW_CLIENTONLY) }
        $pw = $false
        try { $pw = [bool][ShotHarness32]::PrintWindow($Hwnd, $hdc, $flags) } catch { }
        if (-not $pw) {
            # host refused the +2 bit: retry bare so at least the non-composited chrome
            # lands, and say out loud that the frame may be blank.
            Write-ShotLog $Log ('QASHOT-FLAG2-REFUSED hwnd=' + $Hwnd.ToInt64() + ' retrying flags=0')
            try { $pw = [bool][ShotHarness32]::PrintWindow($Hwnd, $hdc, 0) } catch { }
        }
        $g.ReleaseHdc($hdc); $hdcTaken = $false
        if (-not $pw) { Write-ShotLog $Log ('QASHOT-PRINTWINDOW-FAILED hwnd=' + $Hwnd.ToInt64()); return $null }
        $g.Dispose(); $g = $null
        $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
        Write-ShotLog $Log ('QASHOT=' + $Path + ' (' + $w + 'x' + $h + ') flags=' + $flags)
        return $Path
    } catch {
        Write-ShotLog $Log ('QASHOT-THREW=' + (($_.Exception.Message -replace '\s+', ' ')))
        return $null
    } finally {
        if ($hdcTaken -and $null -ne $g) { try { $g.ReleaseHdc($hdc) } catch { } }
        if ($null -ne $g) { try { $g.Dispose() } catch { } }
        if ($null -ne $bmp) { try { $bmp.Dispose() } catch { } }
    }
}

# ---------------------------------------------------------------- Invoke-ShotForeground
function Invoke-ShotForeground {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][IntPtr]$Hwnd,
        [string]$Tag = 'shot',
        [scriptblock]$Log,
        [switch]$Capture,
        [switch]$Client,
        [string]$OutRoot = '',
        [switch]$NoAltTap
    )
    if ($Hwnd -eq [IntPtr]::Zero) { return ' fg-match=zero-hwnd' }
    try {
        if ([ShotHarness32]::GetForegroundWindow() -eq $Hwnd) {
            $tail = ''
            if ($Capture) { $tail = Get-ShotNoteTail $Hwnd $Tag $Log $Client $OutRoot }
            return (' fg-match=yes(already-foreground)' + $tail)
        }
    } catch { }

    # Rung ladder, cheapest first. TapAlt is the foreground-lock breaker; a SendInput that
    # returns 0 injected events is a DEGRADATION we log, never an exception we propagate.
    $rungs = @(
        @{ name = 'TechAltForeground'; act = {
                param($h)
                [void][ShotHarness32]::SetForegroundWindow($h)
                [void][ShotHarness32]::BringWindowToTop($h)
                if (-not $NoAltTap) {
                    $inj = 0
                    try { $inj = [int][ShotHarness32]::TapAlt() } catch { Write-ShotLog $Log ('FOREGROUND-BREAK ' + $Tag + ' ALT-TAP-THREW=' + (($_.Exception.Message -replace '\s+', ' '))) }
                    if ($inj -eq 0) { Write-ShotLog $Log ('FOREGROUND-BREAK ' + $Tag + ' ALT-TAP-INJECTED=0 (SendInput refused; continuing with plain activate)') }
                }
                Start-Sleep -Milliseconds 120
            } },
        @{ name = 'TechAltActivate'; act = {
                param($h) [ShotHarness32]::AltActivate($h); Start-Sleep -Milliseconds 160
            } },
        @{ name = 'NudgeActivate'; act = {
                param($h)
                [void][ShotHarness32]::ShowWindow($h, 5)
                [void][ShotHarness32]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x0013)
                Start-Sleep -Milliseconds 200
            } }
    )
    for ($try = 1; $try -le 3; $try++) {
        foreach ($r in $rungs) {
            try { & $r.act $Hwnd }
            catch { Write-ShotLog $Log ('FOREGROUND-BREAK ' + $Tag + ' RUNG-THREW=' + $r.name + ' ' + (($_.Exception.Message -replace '\s+', ' '))) }
            $fg = [IntPtr]::Zero
            try { $fg = [ShotHarness32]::GetForegroundWindow() } catch { }
            if ($fg -eq $Hwnd) {
                Write-ShotLog $Log ('FOREGROUND-BREAK ' + $Tag + ' tech=' + $r.name + ' try=' + $try + ' MATCH target=' + $Hwnd.ToInt64())
                $tail = ''
                if ($Capture) { $tail = Get-ShotNoteTail $Hwnd $Tag $Log $Client $OutRoot }
                return (' fg-match=yes(tech=' + $r.name + ' try=' + $try + ')' + $tail)
            }
        }
    }
    $fgl = 0
    try { $fgl = [ShotHarness32]::GetForegroundWindow().ToInt64() } catch { }
    Write-ShotLog $Log ('FOREGROUND-BREAK ' + $Tag + ' tech=exhausted target=' + $Hwnd.ToInt64() + ' foreground-still=' + $fgl)
    $tail = ''
    if ($Capture) { $tail = Get-ShotNoteTail $Hwnd $Tag $Log $Client $OutRoot }
    return (' fg-match=no(ladder-exhausted)' + $tail)
}

# The capture verdict folded into the FOREGROUND= note: short, single-spaced, greppable.
function Get-ShotNoteTail([IntPtr]$Hwnd, [string]$Tag, [scriptblock]$Log, [switch]$Client, [string]$OutRoot) {
    $path = Invoke-QaShot -Hwnd $Hwnd -Tag $Tag -OutRoot $OutRoot -Log $Log
    if ([string]::IsNullOrEmpty($path)) { return ' shot=FAILED' }
    $st = Get-QaShotStats -Path $path
    $dup = Resolve-ShotRound $path ([string]$st.md5) $Log
    $name = [System.IO.Path]::GetFileName($path)
    $md5s = [string]$st.md5
    if ($md5s.Length -gt 8) { $md5s = $md5s.Substring(0, 8) }
    $tail = ' shot=' + $name + ' px=' + $st.w + 'x' + $st.h + ' bytes=' + $st.bytes +
            ' md5=' + $md5s +
            ' unique=' + $st.unique + ' mean=' + $st.mean + ' rms=' + $st.rms
    if ([string]$st.bad -ne '') { $tail += ' BAD=' + $st.bad }
    if ($dup -ne '') { $tail += ' md5dup=' + $dup }
    Write-ShotLog $Log ('SHOTCHECK=' + $tail.Trim())
    return $tail
}

# ---------------------------------------------------------------- -SelfTest
# Offline proof: no app under test is started. Only already-existing windows are shot.
if ($SelfTest) {
    $ErrorActionPreference = 'Continue'
    function TW([string]$m) { [Console]::Out.WriteLine($m) }
    $log = { param($m) TW ('  | ' + $m) }

    TW ('SELFTEST start=' + (Get-Date -Format 'HH:mm:ss') + ' harness=' + $script:ShotHarnessVersion +
        ' dpi-pmv2-called-ok=' + [string]$script:ShotDpiSet + ' version=' + [System.Environment]::OSVersion.Version)

    $h = [IntPtr]::Zero
    $via = ''
    try {
        $p = [System.Diagnostics.Process]::GetCurrentProcess()
        if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $h = $p.MainWindowHandle; $via = 'own-console' }
    } catch { }
    if ($h -eq [IntPtr]::Zero) {
        try { $h = [ShotHarness32]::GetForegroundWindow(); $via = 'foreground-window' } catch { }
    }
    if ($h -eq [IntPtr]::Zero) {
        try { $h = [ShotHarness32]::FindWindow('Progman', 'Program Manager'); $via = 'progman' } catch { }
    }
    $title = Get-ShotWindowTitle $h
    TW ('WINDOW hwnd=' + $h.ToInt64() + ' via=' + $via + ' title=[' + $title + ']')
    if ($h -eq [IntPtr]::Zero) { TW 'SELFTEST-RESULT=FAIL no host window to photobomb'; exit 1 }

    Reset-ShotRound
    $out = @()
    for ($i = 1; $i -le $SelfTestRounds; $i++) {
        $note = [string](Invoke-ShotForeground -Hwnd $h -Tag ('selftest-' + $i) -Log $log -Capture)
        TW ('NOTE' + $i + '=' + $note)
        $out += $note
    }
    $paths = @($out | Select-String -Pattern 'shot=(\S+?\.png)' -AllMatches | ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value })
    foreach ($f in $paths) {
        $full = Join-Path (Join-Path (Split-Path -Parent $PSScriptRoot) 'tmp\shots') $f
        if (-not (Test-Path -LiteralPath $full)) { $full = (Get-ChildItem -Path (Join-Path (Split-Path -Parent $PSScriptRoot) 'tmp\shots') -Filter $f -ErrorAction SilentlyContinue | Select-Object -First 1).FullName }
        $a = (Get-FileHash -Algorithm MD5 -LiteralPath $full).Hash.ToLower()
        Start-Sleep -Milliseconds 150
        $b = (Get-FileHash -Algorithm MD5 -LiteralPath $full).Hash.ToLower()
        TW ('REREAD ' + $f + ' md5=' + $a + ' stable=' + [string]($a -eq $b) + ' bytes=' + (Get-Item -LiteralPath $full).Length)
    }

    # The gate itself must reject a synthetic dead frame.
    $black = Join-Path (Join-Path (Split-Path -Parent $PSScriptRoot) 'tmp\shots') 'selftest-black.png'
    $d = [System.IO.Path]::GetDirectoryName($black)
    if (-not (Test-Path -LiteralPath $d)) { New-Item -ItemType Directory -Force -Path $d | Out-Null }
    $bmp = New-Object System.Drawing.Bitmap(120, 80, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $bmp.Save($black, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    $sb = Get-QaShotStats -Path $black
    TW ('GATE black-frame -> px=' + $sb.w + 'x' + $sb.h + ' unique=' + $sb.unique + ' mean=' + $sb.mean + ' rms=' + $sb.rms + ' bad=' + [string]$sb.bad)

    $realOk = ($paths.Count -ge 1)
    $gateOk = ([string]$sb.bad -ne '')
    $distinct = $true
    if ($out.Count -ge 2) {
        $m1 = ([regex]'md5=([0-9a-f]{8})').Match($out[0]); $m2 = ([regex]'md5=([0-9a-f]{8})').Match($out[1])
        if ($m1.Success -and $m2.Success) { $distinct = ($m1.Value -ne $m2.Value) }
    }
    TW ('SELFTEST-RESULT real-shot=' + [string]$realOk + ' black-frame-rejected=' + [string]$gateOk +
        ' rounds-distinct=' + [string]$distinct + ' (same-static-window may legitimately repeat md5)')
    TW ('SELFTEST end=' + (Get-Date -Format 'HH:mm:ss'))
    exit 0
}
