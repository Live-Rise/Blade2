<#
  gui_uia.ps1 - drive & inspect a WinUI 3 desktop app through UI Automation (PowerShell 5.1).

  Starts the target exe, waits for its top-level window, dumps the automation tree as
  compact lines, optionally invokes controls BY NAME (no pixel guessing), optionally
  sets text via ValuePattern, and optionally screenshots the window rect.

  Params:
    -Exe <path>            (required) app to start
    -TitlePattern <regex>  top-level window Name filter, default '.'
    -Invoke <names>        element Name(s) to press; '-eq' first, then '-match'
    -WaitFor <regex>       after an action, block until some element Name matches
    -WaitForLog <substr>   like -WaitFor but polls the app's OWN log files instead of the UIA
                           tree: blocks until <log>.rs.txt (first) or <log>.out.txt / .err.txt
                           contains the literal substring. Use it for diagnostics the app no
                           longer renders on screen (the fork dropped its bottom self-test strip
                           on 2026-09-22 - kernel state now prints as `STATUS: ...` lines).
                           CANONICAL MARKER: `STATUS: 内核: 已连接`.
                           <log>.rs.txt is the file the fork writes itself (env BLADE2_RS_LOG,
                           open/append/close per line) and is the reliable one; .out/.err only
                           carry whatever the inherited std handles reached, which a concurrent
                           run with the same -Log prefix can unlink. See -Log below.
                           Requires -Log. -WaitFor keeps its old behaviour untouched; when both
                           are given, -WaitForLog wins because it is the more precise signal.
    -SetText '<name>=<val>'  write into a TextBox via ValuePattern
    -PreInvokeSetText '<name>=<val>'  same, but before -Invoke (type-then-press flows)
    -Out <png>             screenshot the window rect
    -TimeoutSec <n>        window-appear budget, default 30
    -MaxElements <n>       tree / name-scan cap, default 200
    -LookupCap <n>         how many descendants name matching scans, default 800
    -SettleSec <n>         post-action wait budget, default 8
    -Kill                  stop the app at the end (else it is left running; PID= is printed)
    -Log <path>            redirect the started app's stdout/stderr to <path>.out/.err so a
                           Rust panic on the fork side is recoverable as evidence, AND set
                           BLADE2_RS_LOG=<path>.rs.txt so the fork's own STATUS:/DIAG: lines
                           land in a file no handle game can steal

  Output lines (stdout):
    PID=<id>                     started process id (always printed first)
    WINDOW=<title> FOUND-VIA=... / HWND=...
    <depth> | <ControlType> | <Name> | <IsKeyboardFocusable> | <BoundingRectangle>
    INVOKED=<name> / NOTINVOKED=<name> ... / NOTFOUND=<name>
    TREE-CHANGED|TREE-UNCHANGED|WAITFOR-MET|WAITFOR-TIMEOUT ...
    WAITFORLOG-MET=<line>|WAITFORLOG-TIMEOUT=...|WAITFORLOG-SKIPPED=...
    SHOT=<path>                  when -Out is given
    RESULT=ok|error  <detail>

  NOTE on -Invoke: PowerShell 5.1 with -File cannot bind a parameter twice, so pass
  several names in ONE argument separated by comma or semicolon (they are split here).
  NOTE on liveness: RESULT=ok / WINDOW= alone does NOT prove the UI is alive - a stubbed-out
  tree still prints WINDOW=. Require the `elements=<n>` line with n >= 5 (this script itself
  warns with WARN=tree looks empty below that bar), and prefer -WaitForLog over -WaitFor for
  anything the app no longer renders.
  NOTE on the kernel-state marker: the fork publishes `内核: <status>` on the title-bar element's
  UIA Name (a container's Name is never rendered, so it costs zero layout), so the old recipe
  `-WaitFor '内核: 已连接'` keeps working; -WaitForLog 'STATUS: 内核: 已连接' is the preferred,
  exact form.
  NOTE: names may be ASCII or 中文; the app's own UIA Names are read at runtime, so the
  script itself keeps to ASCII literals and only echoes what UIA returns.
  NOTE for callers that spawn this via ProcessStartInfo: `powershell -File` splits its
  arguments with native-argv rules (only "..." groups a token, '...' is literal), so a
  value containing a space must arrive double-quoted or it silently becomes two arguments.

  Examples (Git Bash):
    powershell -NoProfile -ExecutionPolicy Bypass -File rust/tests/gui_uia.ps1 \
      -Exe rust/target/debug/blade2-rs.exe -TitlePattern '^Blade' -Out ./tmp/app.png
    ... -Log ./tmp/app -WaitForLog 'STATUS: 内核: 已连接'   # wait for the kernel over stdout
    ... -Invoke 新建会话                        # the real nav item (mainline NewSessionItem)
    ... -Invoke '新建会话;设置' -SetText '消息输入框=hello' -Kill
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$TitlePattern = '.',
    [string[]]$Invoke = @(),
    [string]$WaitFor = '',
    [string]$WaitForLog = '',
    [string]$SetText = '',
    [string]$PreInvokeSetText = '',
    [string]$Out = '',
    [int]$TimeoutSec = 30,
    [int]$MaxElements = 200,
    [int]$LookupCap = 800,
    [int]$SettleSec = 8,
    [string]$Log = '',
    [switch]$Kill
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

function W([string]$m) { [Console]::Out.WriteLine($m) }

# The app spawns a kernel child process; a bare Stop-Process orphans it and it keeps
# target/debug/*.exe locked, so the next `cargo build` fails with os error 5.
function Stop-ProcTree([int]$id) {
    $kids = @()
    try { $kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$id" -ErrorAction Stop) } catch { }
    foreach ($k in $kids) { Stop-ProcTree ([int]$k.ProcessId) }
    try { Stop-Process -Id $id -Force -ErrorAction Stop } catch { }
}

# Real exit code of the started process. `Start-Process -PassThru` hands back a Process
# object whose ExitCode can come back $null once the native image died (a WinUI 3 startup
# fault is fail-fast, so the cached handle cannot re-query it) - and an empty `code=` on
# that line cost a whole debug round, because "app exited immediately" and "app was killed
# by the harness" looked identical. Fall back to the WER Application-error record, which
# carries the exception code (e.g. 0xC000027B + faulting module) for exactly this case.
function Get-AppExitCode($procObj) {
    $code = $null
    try { $code = $procObj.ExitCode } catch { $code = $null }
    if ($null -ne $code) {
        try {
            $c = [int]$code
            if ($c -ge 0 -and $c -lt 256) { return [string]$c }
            return ('exit=' + $c + ' (0x' + ('{0:X8}' -f $c) + ')' + $(if ($c -lt 0) { ' <-- native fail-fast / crash' } else { '' }))
        } catch { return [string]$code }
    }
    $id = 0
    try { $id = [int]$procObj.Id } catch { $id = 0 }
    try {
        $since = (Get-Date).AddMinutes(-2)
        $ev = @(Get-WinEvent -FilterHashtable @{ LogName = 'Application'; ProviderName = 'Application Error'; StartTime = $since } -ErrorAction Stop |
            Select-Object -First 8)
        foreach ($e in $ev) {
            $m = [string]$e.Message
            if ($id -gt 0 -and $m -notmatch ('(?i)process id[:\s]+0x' + [Convert]::ToString($id, 16) + '\b')) { continue }
            $ec = [regex]::Match($m, '(?i)exception code[:\s]+(0x[0-9a-f]{4,})')
            $fm = [regex]::Match($m, '(?i)faulting module name:\s*([^,]+),')
            if ($ec.Success) {
                return ('unavailable+WER ' + $ec.Groups[1].Value + $(if ($fm.Success) { ' in ' + $fm.Groups[1].Value.Trim() } else { '' }))
            }
        }
    } catch { }
    return 'unavailable'
}

if (-not (Test-Path -LiteralPath $Exe)) { W "RESULT=error exe not found: $Exe"; exit 3 }
$Exe = (Resolve-Path -LiteralPath $Exe).Path

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing

# ---------------------------------------------------------------- Win32 fallback
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public struct Win32Uia {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern void SwitchToThisWindow(IntPtr h, bool altTab);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@
# 本机主屏 200%：不设 PMv2 时 CopyFromScreen 吃的是虚拟化坐标，截出来的是错位/缩放的图。
try { [void][Win32Uia]::SetProcessDpiAwarenessContext([IntPtr](-4)) } catch { }

# CopyFromScreen grabs SCREEN PIXELS, so the target must really be on top when we grab.
# ShowWindow(9)+SetForegroundWindow from a background console process is refused by the
# foreground lock, which is why shots came back with the user's browser over the app.
# HWND_TOPMOST (-1) + SWP_NOSIZE|NOMOVE|NOACTIVATE (0x13) does move the window, and
# SwitchToThisWindow is the one API allowed to break the foreground lock.
function Raise-For-Shot([IntPtr]$h) {
    if ($h -eq [IntPtr]::Zero) { return }
    try { [void][Win32Uia]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x13) } catch { }
    try { [Win32Uia]::SwitchToThisWindow($h, $true) } catch { }
    try { [void][Win32Uia]::ShowWindow($h, 5) } catch { }
    try { [void][Win32Uia]::SetForegroundWindow($h) } catch { }
    Start-Sleep -Milliseconds 400
    try {
        $fg = [Win32Uia]::GetForegroundWindow()
        W ('FOREGROUND=' + $fg.ToInt64() + ' target=' + $h.ToInt64() + ' match=' + [string]($fg -eq $h))
    } catch { }
}
function Drop-After-Shot([IntPtr]$h) {
    if ($h -eq [IntPtr]::Zero) { return }
    try { [void][Win32Uia]::SetWindowPos($h, [IntPtr](-2), 0, 0, 0, 0, 0x13) } catch { }
}

function Find-HwndByTitle([uint32]$procId, [string]$pattern) {
    $found = New-Object System.Collections.Generic.List[object]
    $cb = [Win32Uia+EnumWindowsProc] {
        param($h, $l)
        $vp = [uint32]0
        [void][Win32Uia]::GetWindowThreadProcessId($h, [ref]$vp)
        if ($vp -eq $procId -and [Win32Uia]::IsWindowVisible($h)) {
            $len = [Win32Uia]::GetWindowTextLength($h)
            if ($len -gt 0) {
                $sb = New-Object System.Text.StringBuilder($len + 2)
                [void][Win32Uia]::GetWindowText($h, $sb, $sb.Capacity)
                if ($sb.ToString() -match $pattern) { $found.Add(@{ h = $h; t = $sb.ToString() }) }
            }
        }
        return $true
    }
    [void][Win32Uia]::EnumWindows($cb, [IntPtr]::Zero)
    return $found
}

# ---------------------------------------------------------------- UIA helpers
$AE       = [System.Windows.Automation.AutomationElement]
$TS       = [System.Windows.Automation.TreeScope]
$TRUECOND = [System.Windows.Automation.Condition]::TrueCondition
$WALKER   = [System.Windows.Automation.TreeWalker]::ControlViewWalker

# NOTE: PS 5.1's managed UIA wrapper exposes no settable call time-out on this build, so
# every UIA call below is wrapped in try/catch to avoid aborting the whole run.

function Get-ElInfo($el) {
    if ($null -eq $el) { return $null }
    try { return $el.Current } catch { return $null }
}

function Format-Name([string]$n) {
    if ($null -eq $n) { return '' }
    $n = $n -replace '[\r\n]+', ' \n '
    $n = $n -replace '\s+', ' '
    $n = $n.Trim()
    if ($n.Length -gt 120) { $n = $n.Substring(0, 117) + '...' }
    return $n
}

function Format-Rect($rc) {
    try {
        if ($rc.IsEmpty) { return 'empty' }
        return ('{0},{1} {2}x{3}' -f [int]$rc.X, [int]$rc.Y, [int]$rc.Width, [int]$rc.Height)
    } catch { return '?' }
}

# Dump the control-view tree of $root as compact lines, level by level (shallow nodes
# first, so a long sibling run - e.g. 90 session buttons - cannot push the interesting
# controls past the cap). Prints the lines; returns nothing.
function Dump-Tree($root, [int]$cap) {
    $count = 0
    $truncated = $false
    $queue = New-Object System.Collections.Generic.Queue[object]
    $queue.Enqueue(@{ e = $root; d = 0 })
    while ($queue.Count -gt 0) {
        $cur = $queue.Dequeue()
        $el = $cur.e
        $depth = $cur.d
        $info = Get-ElInfo $el
        if ($null -ne $info) {
            $ct = $info.ControlType.ProgrammaticName -replace '^ControlType\.', ''
            W ('{0} | {1} | {2} | {3} | {4}' -f $depth, $ct, (Format-Name $info.Name), $info.IsKeyboardFocusable, (Format-Rect $info.BoundingRectangle))
        }
        $count++
        if ($count -ge $cap) { $truncated = $true; break }
        try {
            $k = $WALKER.GetFirstChild($el)
            while ($null -ne $k) {
                $queue.Enqueue(@{ e = $k; d = $depth + 1 })
                try { $k = $WALKER.GetNextSibling($k) } catch { $k = $null }
            }
        } catch { }
    }
    if ($truncated) { W ('... capped at ' + $cap + ' elements, ' + $queue.Count + ' more queued (raise -MaxElements)') }
    W ('elements=' + $count)
}

# Descendants of $root (control view), capped.
# NB: AutomationElementCollection must be indexed as $col[$i] (or $col.Item($i)).
# $col.Item[$i] silently yields $null in PowerShell, which would make every lookup fail.
function Get-Descendants($root, [int]$cap) {
    $list = New-Object System.Collections.Generic.List[object]
    try {
        $col = $root.FindAll($TS::Descendants, $TRUECOND)
        $n = [Math]::Min($col.Count, $cap)
        for ($i = 0; $i -lt $n; $i++) {
            $el = $col[$i]
            if ($null -ne $el) { $list.Add($el) }
        }
    } catch { W ('WARN=FindAll(Descendants) failed: ' + $_.Exception.Message) }
    return $list
}

function Find-ByName($root, [string]$needle, [int]$cap) {
    $all = Get-Descendants $root $cap
    if ($all.Count -eq 0) { W 'WARN=name scan saw 0 descendants (tree not ready or UIA query failed)' }
    $exact = @(); $re = @()
    foreach ($el in $all) {
        $info = Get-ElInfo $el
        if ($null -eq $info) { continue }
        $nm = [string]$info.Name
        if ($nm -eq $needle) { $exact += $el; continue }
        try { if ($nm -match $needle) { $re += $el } } catch { }
    }
    if ($exact.Count -gt 0) { return @{ how = 'eq'; els = $exact } }
    if ($re.Count -gt 0) { return @{ how = 'match'; els = $re } }
    return @{ how = 'none'; els = @() }
}

# Snapshot of the visible tree, used to detect that an action actually changed the UI.
# WinUI 3 keeps a TextBox's UIA Name as its placeholder even when it holds text, so the
# ValuePattern value is folded in here too.
function Get-EditValue($el) {
    try {
        $pat = $el.GetCurrentPattern([System.Windows.Automation.ValuePatternIdentifiers]::Pattern)
        if ($null -eq $pat) { return '' }
        return [string]([System.Windows.Automation.ValuePattern]$pat).Current.Value
    } catch { return '' }
}

function Get-Snapshot {
    $sb = New-Object System.Text.StringBuilder
    foreach ($el in (Get-Descendants $win $LookupCap)) {
        $info = Get-ElInfo $el
        if ($null -eq $info) { continue }
        [void]$sb.Append($info.ControlType.ProgrammaticName).Append('#').Append($info.Name)
        if ($info.ControlType.ProgrammaticName -eq 'ControlType.Edit') {
            [void]$sb.Append('=').Append((Get-EditValue $el))
        }
        [void]$sb.Append('|')
    }
    return $sb.ToString()
}

# Wait (at least $minMs, at most $maxMs) until the tree differs from $before and then
# stops changing for ~2s, so async status changes land in the re-dump.
function Wait-TreeChange([string]$before, [int]$minMs, [int]$maxMs) {
    $waited = 0
    $last = ''
    $prev = ''
    $stable = 0
    $quiet = 5
    while ($waited -lt $maxMs) {
        Start-Sleep -Milliseconds 400
        $waited += 400
        $cur = Get-Snapshot
        if ($cur -ne $before) {
            if ($cur -eq $prev) { $stable++ } else { $stable = 0 }
            $prev = $cur
            if ($stable -ge $quiet -and $waited -ge $minMs) {
                return @{ changed = $true; waited = $waited; settled = $true }
            }
        }
        $last = $cur
    }
    return @{ changed = ($last -ne $before); waited = $waited; settled = $false }
}

# First descendant Name matching a regex ('' when none) - lets -WaitFor block on an outcome.
function Get-NameMatch([string]$pattern) {
    foreach ($el in (Get-Descendants $win $LookupCap)) {
        $info = Get-ElInfo $el
        if ($null -eq $info) { continue }
        try { if ([string]$info.Name -match $pattern) { return [string]$info.Name } } catch { }
    }
    return ''
}

# ---------------------------------------------------------------- -WaitForLog helpers
# -Log writes the child's stdout/stderr via Start-Process redirection, so the app still owns
# those handles: open them with FileShare ReadWrite or every poll throws a sharing violation.
# Returns the first line containing the literal substring ('' when not there yet). Whole file
# is re-read each poll on purpose - the line may have been written long before we attached
# (the fork prints `STATUS: 内核: 已连接 …` during startup, seconds before UIA settles).
function Get-LogHit([string]$needle) {
    if ($script:LogPaths -eq $null -or $script:LogPaths.Count -eq 0) { return '' }
    foreach ($path in $script:LogPaths) {
        if (-not (Test-Path -LiteralPath $path)) { continue }
        $text = ''
        try {
            $fs = [System.IO.File]::Open($path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
            try {
                $sr = New-Object System.IO.StreamReader($fs, [System.Text.Encoding]::UTF8)
                try { $text = $sr.ReadToEnd() } finally { $sr.Dispose() }
            } finally { $fs.Dispose() }
        } catch { continue }
        foreach ($line in ($text -split "`r?`n")) {
            if ($line.IndexOf($needle, [System.StringComparison]::Ordinal) -ge 0) { return $line }
        }
    }
    return ''
}

# What each polled file looks like right now, for the timeout message: a bare "0 bytes" was
# useless evidence twice over (the directory entry lags behind the stream while the app holds the
# handle, and the polled path may not even be the file the app writes to). Opens with
# FileShare=ReadWrite like the poller does, so a live writer's stream length is the honest number.
function Format-LogState {
    $bits = @()
    if ($script:LogPaths -eq $null) { return 'no-log-paths' }
    foreach ($path in $script:LogPaths) {
        $leaf = [System.IO.Path]::GetFileName($path)
        if (-not (Test-Path -LiteralPath $path)) { $bits += ($leaf + '=missing'); continue }
        try {
            $fs = [System.IO.File]::Open($path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
            try { $bits += ($leaf + '=stream:' + $fs.Length) } finally { $fs.Dispose() }
        } catch { $bits += ($leaf + '=open:' + $_.Exception.GetType().Name) }
    }
    return ($bits -join ' ')
}

# Current text of every Edit element: '[<Name>]=<Value>' (Name is the placeholder in WinUI 3).
function Get-EditValues {
    $out = @()
    foreach ($el in (Get-Descendants $win $LookupCap)) {
        $info = Get-ElInfo $el
        if ($null -eq $info) { continue }
        if ($info.ControlType.ProgrammaticName -ne 'ControlType.Edit') { continue }
        $out += ('[' + (Format-Name ([string]$info.Name)) + ']=' + (Format-Name (Get-EditValue $el)))
    }
    return $out
}

# Report what happened after an action: block on -WaitFor when given, else on tree settling.
function Wait-Outcome([string]$pre, [int]$budgetMs, [string]$preHit) {
    # -WaitForLog is checked FIRST: it is the replacement for the on-screen self-test strip that
    # used to carry these diagnostics. The -WaitFor path below keeps its old behaviour untouched
    # because other scripts still rely on it.
    if ($WaitForLog -ne '') {
        if ($null -eq $script:LogPaths -or $script:LogPaths.Count -eq 0) {
            W ('WAITFORLOG-SKIPPED=-WaitForLog needs -Log (needle=' + $WaitForLog + ')')
            return
        }
        $preLogHit = Get-LogHit $WaitForLog
        $waited = 0
        $hit = ''
        while ($true) {
            $hit = Get-LogHit $WaitForLog
            if ($hit -ne '') { break }
            if ($waited -ge $budgetMs) { break }
            Start-Sleep -Milliseconds 400
            $waited += 400
        }
        if ($hit -ne '') {
            $note = ''
            if ($preLogHit -ne '' -and $preLogHit -eq $hit) { $note = ' NOTE: already true before the action' }
            W ('WAITFORLOG-MET=' + (Format-Name $hit) + ' waited=' + $waited + 'ms' + $note)
        }
        else { W ('WAITFORLOG-TIMEOUT=' + (Format-Name $WaitForLog) + ' waited=' + $waited + 'ms files=[' + (Format-LogState) + '] (raise -SettleSec)') }
        return
    }
    if ($WaitFor -ne '') {
        $waited = 0
        $hit = ''
        while ($waited -lt $budgetMs) {
            Start-Sleep -Milliseconds 400
            $waited += 400
            $hit = Get-NameMatch $WaitFor
            if ($hit -ne '') { break }
        }
        if ($hit -ne '') {
            $note = ''
            if ($preHit -ne '' -and $preHit -eq $hit) { $note = ' NOTE: already true before the action' }
            W ('WAITFOR-MET=' + (Format-Name $hit) + ' waited=' + $waited + 'ms' + $note)
        }
        else { W ('WAITFOR-TIMEOUT pattern=' + $WaitFor + ' waited=' + $waited + 'ms (raise -SettleSec)') }
        return
    }
    $st = Wait-TreeChange $pre 1200 $budgetMs
    if ($st.changed) { W ('TREE-CHANGED waited=' + $st.waited + 'ms settled=' + $st.settled) }
    else { W ('TREE-UNCHANGED waited=' + $st.waited + 'ms (raise -SettleSec if the action is slower)') }
}

# Compact error text: PS wraps UIA failures in MethodInvocationException, so prefer the
# inner exception (e.g. ElementNotEnabledException) over the noisy wrapper message.
function Format-Err($e) {
    $cur = $e
    if ($null -ne $e.InnerException) { $cur = $e.InnerException }
    return ($cur.GetType().Name + ': ' + (($cur.Message -replace '\s+', ' ')).Trim())
}

function Invoke-Element($el) {
    $tries = @(
        @{ n = 'InvokePattern'; id = [System.Windows.Automation.InvokePatternIdentifiers]::Pattern },
        @{ n = 'TogglePattern'; id = [System.Windows.Automation.TogglePatternIdentifiers]::Pattern },
        @{ n = 'ExpandCollapsePattern'; id = [System.Windows.Automation.ExpandCollapsePatternIdentifiers]::Pattern },
        @{ n = 'SelectionItemPattern'; id = [System.Windows.Automation.SelectionItemPatternIdentifiers]::Pattern }
    )
    foreach ($t in $tries) {
        $pat = $null
        try { $pat = $el.GetCurrentPattern($t.id) } catch { $pat = $null }
        if ($null -eq $pat) { continue }
        try {
            if ($t.n -eq 'InvokePattern') { [void]([System.Windows.Automation.InvokePattern]$pat).Invoke() }
            elseif ($t.n -eq 'TogglePattern') { [void]([System.Windows.Automation.TogglePattern]$pat).Toggle() }
            elseif ($t.n -eq 'ExpandCollapsePattern') { [void]([System.Windows.Automation.ExpandCollapsePattern]$pat).Expand() }
            else { [void]([System.Windows.Automation.SelectionItemPattern]$pat).Select() }
            return @{ ok = $true; via = $t.n; why = '' }
        } catch {
            return @{ ok = $false; via = $t.n; why = (Format-Err $_.Exception) }
        }
    }
    return @{ ok = $false; via = ''; why = 'no invoke/toggle/expand pattern' }
}

# ---------------------------------------------------------------- start + wait
# -Log: keep the app's own stdout/stderr (a Rust panic on the fork side is the only
# evidence of *why* a window vanished mid-run) next to the caller's chosen path.
$pi = $null
$script:LogPaths = @()
if ($Log -ne '') {
    $logFull = $Log
    if (-not [System.IO.Path]::IsPathRooted($logFull)) { $logFull = Join-Path (Get-Location).Path $logFull }
    $logFull = [System.IO.Path]::GetFullPath($logFull)
    $logParent = [System.IO.Path]::GetDirectoryName($logFull)
    if ($logParent -ne '' -and -not (Test-Path -LiteralPath $logParent)) { New-Item -ItemType Directory -Force -Path $logParent | Out-Null }
    $logOut = $logFull + '.out.txt'
    $logErr = $logFull + '.err.txt'
    # The fork's OWN log file (env BLADE2_RS_LOG): it appends+flushes every `STATUS:`/`DIAG:` line
    # with a short-lived handle, so polling it never depends on an inherited stdout handle.
    # Why that matters (2026-09-22, "qa3-app.out.txt is 0 bytes" while the app was healthy):
    # the .out.txt below is a handle WE hand the child - a second run with the same -Log prefix
    # Remove-Item's it, and the still-running app keeps writing to the unlinked old file object.
    # Measured there: directory entry said 1 byte while the stream we could still open had 413.
    $logRs = $logFull + '.rs.txt'
    Remove-Item -LiteralPath $logOut, $logErr, $logRs -Force -ErrorAction SilentlyContinue
    $env:BLADE2_RS_LOG = $logRs
    # poll order: the app's own file first (deterministic), then the redirected stdout/stderr
    # (stdout carries `STATUS:`/`DIAG:` when inheritance works; stderr is where a Rust panic lands).
    $script:LogPaths = @($logRs, $logOut, $logErr)
    $pi = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $logOut -RedirectStandardError $logErr
    W ('LOG=' + $logFull)
} else {
    $pi = Start-Process -FilePath $Exe -PassThru
}
W ('PID=' + $pi.Id)

$deadline = (Get-Date).AddSeconds($TimeoutSec)
$win = $null
$hwnd = [IntPtr]::Zero
$title = ''
$how = ''
while ((Get-Date) -lt $deadline) {
    if ($pi.HasExited) { W ('RESULT=error app exited code=' + (Get-AppExitCode $pi)); exit 4 }
    # (a) UIA: top-level children of the root element
    try {
        $col = $AE::RootElement.FindAll($TS::Children, $TRUECOND)
        for ($i = 0; $i -lt $col.Count; $i++) {
            $el = $col[$i]
            $info = Get-ElInfo $el
            if ($null -eq $info) { continue }
            if ($info.ProcessId -ne $pi.Id) { continue }
            if ([string]$info.Name -match $TitlePattern) {
                $win = $el; $title = [string]$info.Name; $how = 'uia-children'
                try { $hwnd = [IntPtr]$info.NativeWindowHandle } catch { }
                # Re-root on the HWND element so tree depth numbering is identical
                # whichever discovery path won the race.
                if ($hwnd -ne [IntPtr]::Zero) {
                    try { $fh = $AE::FromHandle($hwnd); if ($null -ne $fh) { $win = $fh } } catch { }
                }
                break
            }
        }
    } catch { }
    if ($null -ne $win) { break }
    # (b) Win32 fallback (same approach as tests/gui_probe.ps1)
    $ws = @(Find-HwndByTitle -procId ([uint32]$pi.Id) -pattern $TitlePattern)
    if ($ws.Count -gt 0) {
        $hwnd = $ws[0].h; $title = $ws[0].t
        try { $win = $AE::FromHandle($hwnd) } catch { $win = $null }
        if ($null -ne $win) { $how = 'win32+fromhandle'; break }
        $win = $null
    }
    Start-Sleep -Milliseconds 500
}

if ($null -eq $win) {
    W 'RESULT=error no window matched TitlePattern'
    if ($Kill -and -not $pi.HasExited) { Stop-Process -Id $pi.Id -Force; W 'KILLED' }
    exit 2
}

# The UIA path may not hand back a usable HWND; resolve one via Win32 for ShowWindow/screenshot.
if ($hwnd -eq [IntPtr]::Zero) {
    $ws = @(Find-HwndByTitle -procId ([uint32]$pi.Id) -pattern $TitlePattern)
    if ($ws.Count -gt 0) { $hwnd = $ws[0].h; if ($title -eq '') { $title = $ws[0].t } }
}

try { [void][Win32Uia]::ShowWindow($hwnd, 9) } catch { }
try { [void][Win32Uia]::SetForegroundWindow($hwnd) } catch { }
W ('WINDOW=' + $title)
W ('FOUND-VIA=' + $how)
if ($hwnd -ne [IntPtr]::Zero) { W ('HWND=' + $hwnd.ToInt64()) }

# ---------------------------------------------------------------- settle + dump
# WinUI 3 creates the HWND before the XAML content is realized: wait quietly until the
# subtree is populated, then dump once.
$elements = $null
$settleDeadline = (Get-Date).AddSeconds([Math]::Min($TimeoutSec, 20))
while ((Get-Date) -lt $settleDeadline) {
    Start-Sleep -Milliseconds 400
    $elements = Get-Descendants $win $MaxElements
    if ($elements.Count -ge 5) { break }
}
W ('--- TREE window=[' + (Format-Name $title) + '] ---')
Dump-Tree $win $MaxElements
W '--- END TREE ---'
if ($null -eq $elements -or $elements.Count -lt 5) {
    W 'WARN=tree looks empty; WinUI 3 content may still be virtualizing or names not exposed'
}

# -WaitForLog with NO -Invoke/-SetText still has to gate the run: callers use it as the
# "did the kernel come up" liveness check before the screenshot (that is what the bottom
# self-test strip used to provide on screen). -WaitFor deliberately keeps its old
# action-only behaviour here so existing scripts see no change.
if ($WaitForLog -ne '' -and @($Invoke).Count -eq 0 -and $SetText -eq '' -and $PreInvokeSetText -eq '') {
    Wait-Outcome (Get-Snapshot) ([int]($SettleSec * 1000)) ''
}

# ---------------------------------------------------------------- -SetText helpers
# Writes text through ValuePattern. Used both before -Invoke (-PreInvokeSetText, for flows
# that must type first and then press a button) and after it (-SetText).
function Set-EditText([string]$spec, [string]$label) {
    $idx = $spec.IndexOf('=')
    if ($idx -lt 1) {
        W ('SETTEXT-SKIPPED=' + $label + ' usage: "<element name>=<value>"')
        return
    }
    $tname = $spec.Substring(0, $idx)
    $tval = $spec.Substring($idx + 1)
    $hit = Find-ByName $win $tname $LookupCap
    if ($hit.els.Count -eq 0) {
        W ('NOTFOUND=' + $tname)
        return
    }
    $el = $hit.els[0]
    $pat = $null
    try { $pat = $el.GetCurrentPattern([System.Windows.Automation.ValuePatternIdentifiers]::Pattern) } catch { $pat = $null }
    if ($null -eq $pat) {
        W ('SETTEXT-SKIPPED=' + (Format-Name $tname) + ' no ValuePattern (element may be read-only text)')
        return
    }
    $vp = [System.Windows.Automation.ValuePattern]$pat
    if ($vp.Current.IsReadOnly) {
        W ('SETTEXT-SKIPPED=' + (Format-Name $tname) + ' ValuePattern is read-only')
        return
    }
    try {
        $preT = Get-Snapshot
        $preWaitT = ''
        if ($WaitFor -ne '') { $preWaitT = Get-NameMatch $WaitFor }
        $vp.SetValue($tval)
        W ('SETTEXT=' + (Format-Name $tname) + ' value=' + (Format-Name $tval) + ' when=' + $label)
        Wait-Outcome $preT ([int]($SettleSec * 1000)) $preWaitT
        W ('--- TREE after SetText [' + $tname + '] ---')
        Dump-Tree $win $MaxElements
        W '--- END TREE ---'
        foreach ($v in (Get-EditValues)) { W ('VERIFY-EDIT ' + $v) }
    } catch {
        W ('SETTEXTFAILED=' + (Format-Name $tname) + ' err=' + $_.Exception.Message)
    }
}

if ($PreInvokeSetText -ne '') { Set-EditText $PreInvokeSetText 'pre' }

# ---------------------------------------------------------------- -Invoke
# NB: only the first actionable match is invoked (clicking several matches would fire the
# same command multiple times); extra matches are just reported.
foreach ($nameRaw in @($Invoke)) {
    foreach ($name in ($nameRaw -split '[;,]')) {
        $needle = $name.Trim()
        if ($needle -eq '') { continue }
        $hit = Find-ByName $win $needle $LookupCap
        if ($hit.els.Count -eq 0) {
            W ('NOTFOUND=' + $needle)
            continue
        }
        $pre = Get-Snapshot
        $preWaitT = ''
        if ($WaitFor -ne '') { $preWaitT = Get-NameMatch $WaitFor }
        $done = $false
        $why = ''
        foreach ($el in $hit.els) {
            $r = Invoke-Element $el
            if ($r.ok) {
                W ('INVOKED=' + $needle + ' via=' + $r.via + ' match=' + $hit.how + ' candidates=' + $hit.els.Count)
                $done = $true
                break
            }
            if ($why -eq '') { $why = $r.via + ' ' + $r.why }
        }
        if (-not $done) {
            $info = Get-ElInfo $hit.els[0]
            $en = 'unknown'
            if ($null -ne $info) { $en = [string]$info.IsEnabled }
            W ('NOTINVOKED=' + $needle + ' enabled=' + $en + ' reason=' + $why)
        }
        # Wait >=1.2s for the app to react, then keep polling until the outcome lands so an
        # async status change (内核启动中… -> 已连接/失败: ...) is visible in the re-dump.
        Wait-Outcome $pre ([int]($SettleSec * 1000)) $preWaitT
        W ('--- TREE after invoke [' + $needle + '] ---')
        Dump-Tree $win $MaxElements
        W '--- END TREE ---'
    }
}

# ---------------------------------------------------------------- -SetText name=value
if ($SetText -ne '') { Set-EditText $SetText 'post' }

# ---------------------------------------------------------------- -Out screenshot
if ($Out -ne '') {
    # CopyFromScreen grabs real pixels (PrintWindow returns an all-white frame for WinUI 3),
    # so whatever slid on top of the window would end up inside the PNG - raise it first.
    Raise-For-Shot $hwnd
    $rc = $null
    try {
        $b = $win.Current.BoundingRectangle
        if (-not $b.IsEmpty) { $rc = @{ L = [int]$b.X; T = [int]$b.Y; R = [int]($b.X + $b.Width); B = [int]($b.Y + $b.Height) } }
    } catch { }
    if ($null -eq $rc -and $hwnd -ne [IntPtr]::Zero) {
        $r = New-Object Win32Uia+RECT
        if ([Win32Uia]::GetWindowRect($hwnd, [ref]$r)) { $rc = @{ L = $r.Left; T = $r.Top; R = $r.Right; B = $r.Bottom } }
    }
    if ($null -eq $rc) {
        W 'SHOT-SKIPPED no bounding rectangle available'
    } else {
        try {
            $outFull = $Out
            if (-not [System.IO.Path]::IsPathRooted($outFull)) { $outFull = Join-Path (Get-Location).Path $outFull }
            $outFull = [System.IO.Path]::GetFullPath($outFull)
            $parent = [System.IO.Path]::GetDirectoryName($outFull)
            if ($parent -ne '' -and -not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
            $w = [Math]::Max(1, $rc.R - $rc.L); $h = [Math]::Max(1, $rc.B - $rc.T)
            $bmp = New-Object System.Drawing.Bitmap($w, $h)
            $g = [System.Drawing.Graphics]::FromImage($bmp)
            $g.CopyFromScreen($rc.L, $rc.T, 0, 0, $bmp.Size)
            $g.Dispose()
            $bmp.Save($outFull, [System.Drawing.Imaging.ImageFormat]::Png)
            $bmp.Dispose()
            W ('SHOT=' + $outFull + ' (' + $w + 'x' + $h + ')')
        } catch {
            W ('SHOTFAILED=' + $_.Exception.Message)
        }
    }
    Drop-After-Shot $hwnd
}

W 'RESULT=ok'
if ($Kill) {
    Stop-ProcTree ([int]$pi.Id)
    W 'KILLED'
} else {
    W ('LEFT-RUNNING pid=' + $pi.Id + ' (taskkill //PID ' + $pi.Id + ' //F)')
}
exit 0
