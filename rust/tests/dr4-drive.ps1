<#
  dr4-drive.ps1 - DR4 前台验收轮驱动器（只读产品码；本轮唯一允许的写入之一）。

  为什么另起一颗而不是复用 rust/tmp/qa5-drive.ps1：
    1. `qa5-drive.ps1:39` dot-source `rust\tmp\shotlib.ps1`，而 `rust/tests/selftest_tools.ps1`
       的检查 4c 明令禁止（`$script:LibName = 'shot' + 'lib'`）⇒ 本颗只依赖在册核
       `rust/tests/shot_harness.ps1`（PrintWindow PW_RENDERFULLCONTENT + PER_MONITOR_AWARE_V2）。
    2. `qa5-drive` 没有真键盘 / 真指针 / 元素值回读 / 局部裁图 四条腿，而本轮
       #73（`@` 浮层要 typed 输入）、权限预设（要真点真切）、逐态截图（要真 hover）
       全依赖它们。它的 `key:` 走 PostMessage，按成规**不算** hover、也不算真打字。
    3. 默认 `-Exe` 钉在热验收位 `target-mr1`（`qa5-drive:27` 的默认值是 09-26 的陈旧
       `rust/target/debug`，用它跑 = 六项全假 FAIL）。
    4. 默认 `-OutDir` 钉在本轮截图目录 `rust/tests/gui-shots/2026-09-28`（不落 rust/tmp、
       不落 target*，两者都被 .gitignore 吞）。

  步骤语言（-Steps，';' 分隔；每步一行 '--- STEP' 证据）：
    shot:<name>            整窗 PrintWindow -> <OutDir>/<name>.png + md5/unique/mean/rms/bad
    eshot:<key>=<name>     整窗抓完后按元素 rect 裁局部（小面变化的正面证据）
    tree[:cap]             作用域 UIA 树（depth|type|id|name|focusable|rect）
    find:<regex>           只打 id 或 name 命中正则的元素
    count:<regex>          命中元素条数（>20 行那种计数判据）
    inv:<key>              AutomationId 优先、再 Name 全等、再 Name 正则；**只走 4 枚真臂**
                           Invoke / Toggle / SelectionItem.Select / ExpandCollapse.Expand。
                           **无 LegacyIAccessible 兜底**（T5K 现测：PS 5.1 的托管 UIA wrapper 里
                           LegacyIAccessiblePattern(Identifiers) 两枚类型根本不存在，旧兜底臂套
                           空 catch 静默死臂 ⇒ 已删；四臂全空手打 NOTINVOKED= 记问题，不是打绿）
    set:<key>=<text>       ValuePattern.SetValue（受控件会走 TextChanged ⇒ 但 caret 打不真）
    val:<key>              **回读** ValuePattern.Value + Name + rect（无 caret 通道时的采纳判据）
    type:<key>=<text>      SetFocus 后**真 SendInput**（KEYEVENTF_UNICODE）逐字打，含空白/换行不吞
    keyin:<spec>           真 SendInput 单键：esc|ret|up|down|left|right|tab|bksp，可 shift+ / ctrl+ 前缀
    hover:<key>            真 SendInput 指针移到元素中心 + 抖 3px（成规：伪造 hover 不算）
    click:<key>            UIA 定位到元素中心后真 SendInput 左键单击（不是按像素盲点）
    waitlog:<substr>       阻塞直到分叉自持日志含 substr（字面 Ordinal 子串）
    loggrep:<substr>       字面 Ordinal 子串，与 waitlog 同口径；命中行原样打出来（逐字取证）
    logre:<regex>          显式正则那一发（先编译再跑；^ 行锚 = 立刻记问题，见下面日志口径备案）
    sleep:<ms>
    raise                  只抢前台不截图
    close                  CloseMainWindow（永不 taskkill）

  退出码（T3K 洞 #1，这一条是本轮的开跑前提）：
    0 = RESULT=ok problems=0 —— 本轮**没有**任何一条失败证据行，也没有一帧 bad= 非空
    1 = RESULT=FAIL problems=N —— 上面每一条 [PROBLEM n] 行都是一次没成（超时/NOTFOUND/WARN/坏帧/歧义定位…）
    3/4/5 = 开跑前就硬停（exe 不在 / 假内核不在 / 本颗注入核的 INPUT 不是 40B）
  红线（T3K 洞 #3）：默认档**永远**走假内核（-FakeKernelArgs 非空 = 注入 fake_dsh.exe）。
  起真内核（= 分叉自己去解析 %APPDATA%\npm\dsh.cmd 起 node）只有显式 -RealKernel 一条路，
  且那一发必然以退出码 1 收尾。主干与 Kernel\node.exe 一律不许由本驱动器起。
#>
[CmdletBinding()]
param(
    [string]$Exe = 'E:\Syncthing\DshWinUI\rust\target-mr1\debug\blade2-rs.exe',
    [string]$Steps = 'tree',
    [string]$LogPrefix = 'E:\Syncthing\DshWinUI\rust\tmp\dr4-run',
    [string]$OutDir = 'E:\Syncthing\DshWinUI\rust\tests\gui-shots\2026-09-28',
    [string]$TitlePattern = '.',
    [int]$TimeoutSec = 90,
    [int]$WaitSec = 90,
    [int]$Cap = 2000,
    # T3K hole #3 (was: [string]$FakeKernelArgs = '' ⇒ 空串就是默认档 ⇒ 默认档打 KERNEL=real
    # ⇒ 分叉去解析 %APPDATA%\npm\dsh.cmd 起真 node 内核)。默认值现在**必须非空**，即默认走假内核；
    # 真内核要显式 -RealKernel，且那一发永远结尾非零。selftest_tools.ps1 的 KERNEL-DEFAULT 锁钉这条。
    [string]$FakeKernelArgs = '--pace=0',
    [switch]$RealKernel,
    # T3K hole #1：inv:/set:/val:/type:/hover:/click:/eshot: 的定位如果落到正则兜底桶（key 只写半截
    # 就能命中四颗元素），本轮一律记一条 FIND-BY-REGEX 问题 ⇒ 退出码非零。-LooseFind 才降级为纯打印。
    [switch]$LooseFind,
    # 主干同形的“量不到”白名单（';' 分隔的正则）。默认空 = 一律算问题（fail-closed）。
    # 已定案例：windows-reactor 只搬 SetName/SetAutomationId ⇒ 单设 AutomationId 的布局宿主
    # （如 ReferencePalette 根那颗 Border）压根不进树，量不到是应有，不是缺陷 —— 但要不要豁免
    # 由调用方显式写出来，例如 -TolerateAbsent 'NOTFOUND=ReferencePalette\b'。
    [string]$TolerateAbsent = '',
    [string]$RaiseFirst = ''
)

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
# ⚠⚠ 这一整块以前是**三行写死的旧结论**（在册抓图核 md5=47e6b40d…、缺 using System.Text、
#   INPUT 是 48B、TapAlt 恒 0）。DR6P §1-A 实测那三条**全部过期**（#189 已修），注释当证据
#   就是把别人的修复说成缺陷 ⇒ 现在改成「现读现判」：见下面 TOOLCHAIN 那一段，它在每一次运行时
#   重新读 shot_harness.ps1 的 md5 与那两处形状，判据是量出来的，不是抄来的。
#   本颗自带抓图/注入核这件事**不是**因为在册核坏了，而是设计差：本颗还要真键盘 / 真指针 /
#   元素值回读 / 局部裁图四条腿（见文件头 4 条理由），shot_harness 不提供 SendInput 打字。
Add-Type -AssemblyName System.Drawing

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

if (-not ('Dr4Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Dr4Native {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {
        public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo;
    }
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {
        public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo;
    }
    // 真 INPUT：type 在 0，联合在 8（x64 对齐）；显式 FieldOffset 免得被自动布局顶到 16。
    [StructLayout(LayoutKind.Explicit)] public struct INPUT {
        [FieldOffset(0)] public uint type;
        [FieldOffset(8)] public KEYBDINPUT ki;
        [FieldOffset(8)] public MOUSEINPUT mi;
    }
    [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint n, INPUT[] p, int cb);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out PPOINT p);
    [StructLayout(LayoutKind.Sequential)] public struct PPOINT { public int x; public int y; }
    [DllImport("user32.dll")] public static extern bool GetSystemMetrics(int n);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc f, IntPtr l);
    public delegate bool EnumWindowsProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);

    public const uint INPUT_KEYBOARD = 1;
    public const uint INPUT_MOUSE = 0;
    public const uint KEYEVENTF_UNICODE = 4;
    public const uint KEYEVENTF_KEYUP = 2;
    public const uint MOUSEEVENTF_MOVE = 1;
    public const uint MOUSEEVENTF_LEFTDOWN = 2;
    public const uint MOUSEEVENTF_LEFTUP = 4;
    public const uint MOUSEEVENTF_ABSOLUTE = 0x8000;
    public const uint MOUSEEVENTF_VIRTUALDESK = 0x4000;
    public const int SM_XVIRTUALSCREEN = 76;
    public const int SM_YVIRTUALSCREEN = 77;
    public const int SM_CXVIRTUALSCREEN = 78;
    public const int SM_CYVIRTUALSCREEN = 79;

    // 一次按键（按下 + 抬起）；返回真注入的事件数（0 = SendInput 拒了，调用方必须喊出来）。
    public static uint TapKey(ushort vk) {
        INPUT[] seq = new INPUT[2];
        seq[0].type = INPUT_KEYBOARD; seq[0].ki.wVk = vk; seq[0].ki.dwFlags = 0;
        seq[1].type = INPUT_KEYBOARD; seq[1].ki.wVk = vk; seq[1].ki.dwFlags = KEYEVENTF_KEYUP;
        return SendInput(2, seq, Marshal.SizeOf(typeof(INPUT)));
    }
    public static uint HoldKey(ushort vk, bool down) {
        INPUT[] seq = new INPUT[1];
        seq[0].type = INPUT_KEYBOARD; seq[0].ki.wVk = vk;
        seq[0].ki.dwFlags = down ? 0u : KEYEVENTF_KEYUP;
        return SendInput(1, seq, Marshal.SizeOf(typeof(INPUT)));
    }
    // 真打字：KEYEVENTF_UNICODE 逐枚码元，走系统输入队列（SendKeys 那种剪贴板路不算）。
    public static uint TypeText(string text) {
        if (string.IsNullOrEmpty(text)) return 0;
        ushort[] units = new ushort[text.Length];
        for (int i = 0; i < text.Length; i++) units[i] = text[i];
        INPUT[] seq = new INPUT[units.Length * 2];
        for (int i = 0; i < units.Length; i++) {
            seq[i * 2].type = INPUT_KEYBOARD;
            seq[i * 2].ki.wScan = units[i]; seq[i * 2].ki.dwFlags = KEYEVENTF_UNICODE;
            seq[i * 2 + 1].type = INPUT_KEYBOARD;
            seq[i * 2 + 1].ki.wScan = units[i]; seq[i * 2 + 1].ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;
        }
        return SendInput((uint)seq.Length, seq, Marshal.SizeOf(typeof(INPUT)));
    }
    // 指针移到虚拟桌面绝对坐标（成规：SetCursorPos 不入队列，测不到 hover）。
    public static uint MoveAbs(int physX, int physY) {
        int vx = 0, vy = 0, vw = 1, vh = 1;
        // GetSystemMetrics 的 bool 原型是占位错签；这里用 GetSystemMetrics32 才拿得到整数度量。
        vx = GetSystemMetricsInt(SM_XVIRTUALSCREEN); vy = GetSystemMetricsInt(SM_YVIRTUALSCREEN);
        vw = GetSystemMetricsInt(SM_CXVIRTUALSCREEN); vh = GetSystemMetricsInt(SM_CYVIRTUALSCREEN);
        if (vw <= 0) vw = 1; if (vh <= 0) vh = 1;
        int nx = (int)((((double)physX - vx) / vw) * 65535.0);
        int ny = (int)((((double)physY - vy) / vh) * 65535.0);
        INPUT[] seq = new INPUT[1];
        seq[0].type = INPUT_MOUSE;
        seq[0].mi.dx = nx; seq[0].mi.dy = ny;
        seq[0].mi.dwFlags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
        return SendInput(1, seq, Marshal.SizeOf(typeof(INPUT)));
    }
    public static uint ClickAbs(int physX, int physY) {
        uint a = MoveAbs(physX, physY);
        INPUT[] seq = new INPUT[2];
        seq[0].type = INPUT_MOUSE; seq[0].mi.dwFlags = MOUSEEVENTF_LEFTDOWN;
        seq[1].type = INPUT_MOUSE; seq[1].mi.dwFlags = MOUSEEVENTF_LEFTUP;
        uint b = SendInput(2, seq, Marshal.SizeOf(typeof(INPUT)));
        return a + b;
    }
    public static int GetSystemMetricsInt(int n) { return GetSystemMetrics32(n); }
    [DllImport("user32.dll", EntryPoint="GetSystemMetrics")] public static extern int GetSystemMetrics32(int n);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint f);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool join);
    [DllImport("user32.dll")] public static extern bool AllowSetForegroundWindow(uint pid);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);

    // 一发 ALT down/up：破前台锁的**唯一**合法钥匙（必须真 SendInput，伪造事件不算）。
    public static uint TapAltReal() {
        INPUT[] seq = new INPUT[2];
        seq[0].type = INPUT_KEYBOARD; seq[0].ki.wVk = 0x12; seq[0].ki.dwFlags = 0;
        seq[1].type = INPUT_KEYBOARD; seq[1].ki.wVk = 0x12; seq[1].ki.dwFlags = KEYEVENTF_KEYUP;
        return SendInput(2, seq, Marshal.SizeOf(typeof(INPUT)));
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
            if (IsIconic(h)) { ShowWindow(h, 9); }
            SetForegroundWindow(h); BringWindowToTop(h);
            TapAltReal();
            SetForegroundWindow(h); BringWindowToTop(h);
        } finally { if (joined) { AttachThreadInput(cur, fgTid, false); } }
    }
    public static RECT VirtualDesk() {
        RECT r = new RECT();
        r.Left = GetSystemMetrics32(SM_XVIRTUALSCREEN); r.Top = GetSystemMetrics32(SM_YVIRTUALSCREEN);
        r.Right = r.Left + GetSystemMetrics32(SM_CXVIRTUALSCREEN); r.Bottom = r.Top + GetSystemMetrics32(SM_CYVIRTUALSCREEN);
        return r;
    }
    public static string FgTitle(IntPtr h) {
        int len = GetWindowTextLength(h);
        if (len <= 0) return "";
        StringBuilder sb = new StringBuilder(len + 2);
        GetWindowText(h, sb, sb.Capacity);
        return sb.ToString();
    }
}
'@
}

# PMv2 自己抢（#22 那坑：不抢则 200% 缩放下每个像素/DIP 坐标对不上）。
# 同一进程只能设一次 ⇒ 返回 $false 也照常往下走（只登记，不致命）。
$script:Dr4HarnessVersion = 'dr4-drive.ps1/3 (T3K knife: honest exit code + fake-kernel default + literal log grep + live toolchain read | local capture core; INPUT=40B; PW flag=2; user32!PrintWindow | T4K: crop leg = Crop-Dr4Rect with a real Rectangle ctor + ESHOT-CROP-FAILED= in the failure roster | T5K: inv: leg walks ONLY the 4 patterns that really exist on PS 5.1 - Invoke/Toggle/SelectionItem/ExpandCollapse - the LegacyIAccessible fallback arm is DELETED because those two types are not in UIAutomationClient/UIAutomationTypes here, and its empty catch made it a silent dead arm; nothing invoked now prints NOTINVOKED= and is counted. Roster gained SHOTFAILED= / ESHOT-EMPTY-RECT= / QASHOT-PRINTWINDOW-FAILED / NOTINVOKED= as real failures and ESHOT-DEGENERATE as the tolerated-skip family, clamp-lost degenerate re-routed to ESHOT-CROP-FAILED=)'
$script:Dr4Dpi = $false
try { $script:Dr4Dpi = [bool]([Dr4Native]::SetProcessDpiAwarenessContext([IntPtr](-4))) } catch { }

# ---------------------------------------------------------------- T3K 洞 #1: 退出码不许说谎
# 以前这颗脚本末尾是无条件 `RESULT=ok` + `exit 0`（DR6P §1-A 点名「最狠的一条」）：超时、NOTFOUND、
# WARN 全都不改退出码 ⇒ 一整轮可以全错而横幅全绿。现在每一条失败证据都要进累计器，
# 结尾的横幅按累计器的**条数**说话。selftest_tools.ps1 的 EXIT-CODE-CONTRACT 锁钉这套结构。
$script:Dr4Problems = @()
$script:Dr4Tolerated = @()
# 证据前缀名册：以这些前缀开头的行 = 这一步没有成。门禁拿它自己的名册来比这里，少一颗就 FAIL。
$script:Dr4FailureTokens = @(
    'NOTFOUND=', 'SET-NOTFOUND=', 'VAL-NOTFOUND=', 'TYPE-NOTFOUND=', 'HOVER-NOTFOUND=', 'CLICK-NOTFOUND=',
    'ESHOT-NOTFOUND=', 'WAITFORLOG-TIMEOUT=', 'WAITFORLOG-SKIPPED=', 'LOGGREP-NOLOG=', 'TYPE-DEGRADED',
    'STEP-UNKNOWN=', 'STEP-THREW=', 'QASHOT-SKIPPED', 'QASHOT-THREW=', 'ESHOT-SKIPPED', 'SHOT-SKIPPED',
    # T4K：逐元素裁剪（Crop-Dr4Rect）没成 = 这一发没成。以前那一发写的是 Rectangle 上**不存在**的
    # MakeRectangle，每调必抛，整条 eshot: 腿零产出，而名册里没有任何一颗前缀认领它。
    'ESHOT-CROP-FAILED=',
    # T5K：下面四枚都是"打印出去但从前不改退出码"的静默失败（旧名册里没有它们，W() 的分类网
    # 抓不到，于是整发没成也照样绿）。逐条都是真失败，不是跳过。
    #   NOTINVOKED=               inv: 腿四条真臂全空手。旧写法只有 INVOKED= 一行，pattern= 打空字符串
    #                             没人判，等于"没兜底就当成功"（T4K 那条 LegacyIAccessible 兜底臂
    #                             本来就是静默死臂，删掉它之后这条腿必须显式说话）。
    #   SHOTFAILED=               Take-Shot 打完 PrintWindow，png 不在盘上 = 这一帧根本没拍到。
    #                             同仓 gui_uia.ps1 早就把这枚放在名册里并判 critical，dr4 漏了。
    #   ESHOT-EMPTY-RECT=         元素在树里找得到，但 BoundingRectangle 是空的 = 要拍的逐元素
    #                             证据根本不存在。和已经在册的 HOVER-NO-RECT=/CLICK-NO-RECT= 同判据。
    #   QASHOT-PRINTWINDOW-FAILED 抓图核自己报的成因行；同一段里 QASHOT-THREW=/QASHOT-SKIPPED 都在册，
    #                             只有这一枚漏了，而它正是"帧没出"的那一发。
    'NOTINVOKED=', 'SHOTFAILED=', 'ESHOT-EMPTY-RECT=', 'QASHOT-PRINTWINDOW-FAILED',
    # ESHOT-DEGENERATE 也入册，但它是**合法跳过那一族**：元素本来就只有 1~3px 时这是元素的正常态，
    # 不是工具坏。入册 = 它不再可能静默；能被 -TolerateAbsent 显式豁免 = 正常轮不会被它钉成永远红。
    # selftest_tools.ps1 把它钉在 tokens 里但**故意不钉进 critical**（判据见 Take-ElShot 那段注释）。
    'ESHOT-DEGENERATE',
    'KEYIN-UNKNOWN=', 'HOVER-NO-RECT=', 'CLICK-NO-RECT=', 'SET-BADSPEC', 'ESHOT-BADSPEC', 'TYPE-BADSPEC',
    'SET-VALUE-THREW=', 'TYPE-THREW=', 'STDOUT-SAVE-THREW=', 'FG-EXHAUSTED=', 'ALT-TAP-INJECTED=',
    'TREE-THIN=', 'FRAME-BAD=', 'NEEDLE-ANCHOR=', 'FIND-BY-REGEX=', 'AMBIGUOUS-FIND=', 'LOGRE-LINEANCHOR=',
    'REAL-KERNEL=', 'KERNEL-NOFAKE=', 'DR4-ABI=', 'TOOLCHAIN-BAD=', 'PROC-LAUNCH-NOT-EXE='
)
function Write-Dr4Line([string]$m) { [Console]::Out.WriteLine($m) }
function Test-Dr4FailureToken([string]$m) {
    if ($null -eq $m -or $m -eq '') { return '' }
    foreach ($t in $script:Dr4FailureTokens) { if ($m.StartsWith($t, [System.StringComparison]::Ordinal)) { return $t } }
    return ''
}
function Test-Dr4Tolerated([string]$m) {
    # 只有调用方显式给了 -TolerateAbsent 才可能豁免；默认空 = 一颗都不豁免（fail-closed）。
    if ($null -eq $TolerateAbsent -or $TolerateAbsent -eq '') { return $false }
    foreach ($one in ($TolerateAbsent -split ';')) {
        $t = $one.Trim(); if ($t -eq '') { continue }
        try { if ($m -match $t) { return $true } } catch { }
    }
    return $false
}
function Mark-Dr4Problem([string]$kind, [string]$detail) {
    $line = ($kind + $detail)
    if (Test-Dr4Tolerated $line) {
        $script:Dr4Tolerated = @($script:Dr4Tolerated) + $line
        Write-Dr4Line ($line + ' [TOLERATED]')
        return $false
    }
    $script:Dr4Problems = @($script:Dr4Problems) + [pscustomobject]@{ Kind = $kind; Detail = $detail }
    Write-Dr4Line ($line + ' [PROBLEM ' + $script:Dr4Problems.Count + ']')
    return $true
}
function W([string]$m) {
    Write-Dr4Line $m
    # 安全网：任何一条以失败前缀开头的证据行都必须落进累计器（漏一条 = 那一类错永远绿）。
    $tok = Test-Dr4FailureToken $m
    if ($tok -ne '') { [void](Mark-Dr4Problem $tok (ConvertTo-Dr4Detail $m $tok)) }
    # 值级判据（不是前缀）：帧统计里 bad= 只要非空 = 这一帧不可信，SHOT=/ESHOT= 行不许绿着过。
    if ($m -match '^(SHOT|ESHOT)=' ) {
        $bm = [regex]::Match($m, '\bbad=(?<v>\S+)')
        if ($bm.Success) { [void](Mark-Dr4Problem 'FRAME-BAD=' ('bad=' + $bm.Groups['v'].Value + ' line=' + $m.Substring(0, [Math]::Min(90, $m.Length)))) }
    }
}
function ConvertTo-Dr4Detail([string]$m, [string]$tok) {
    if ($m.Length -le $tok.Length) { return '' }
    return ($m.Substring($tok.Length) -replace '\s+', ' ').Trim()
}
function Fmt([string]$s) { if ($null -eq $s) { return '' } return ($s -replace '\s+', ' ') }
function RectStr($b) {
    try { if ($b.IsEmpty) { return 'empty' } return ('x' + [int]$b.X + ' y' + [int]$b.Y + ' w' + [int]$b.Width + ' h' + [int]$b.Height) } catch { return '?' }
}
function Now([string]$tag) { W ($tag + ' ts=' + (Get-Date -Format 'HH:mm:ss.fff')) }

# ---------------------------------------------------------------- T3K 过期账 A：现读现判在册抓图核
# 这一段替掉了原先写死在本文件里的三行旧结论（在册核 md5=47e6b40d… / 缺 using System.Text /
# INPUT=48B）。那三条现在全是过期证据（DR6P §1-A），所以改成每次运行**重新读盘**：
# md5 现算、`using System.Text;` 现查，判据是量出来的。结构尺寸的正解归
# rust/tests/selftest_tools.ps1 的规则 6/7/8（那边是按字段表重算的），本颗不另立一套口径。
$script:Dr4ToolchainMd5 = ''
$shPath = Join-Path $PSScriptRoot 'shot_harness.ps1'
if (-not (Test-Path -LiteralPath $shPath)) {
    [void](Mark-Dr4Problem 'TOOLCHAIN-BAD=' ('shot_harness.ps1 is not next to this script at ' + $shPath))
} else {
    try { $script:Dr4ToolchainMd5 = (Get-FileHash -Algorithm MD5 -LiteralPath $shPath).Hash.ToLower() } catch { }
    $shText = ''
    try { $shText = [System.IO.File]::ReadAllText($shPath) } catch { }
    $shUsesSb = ($shText -match 'StringBuilder')
    $shHasUsingText = ($shText -match "(?m)^\s*using\s+System\.Text\s*;")
    W ('TOOLCHAIN shot_harness.ps1 md5=' + $script:Dr4ToolchainMd5 + ' bytes=' + $shText.Length +
       ' StringBuilder=' + [string]$shUsesSb + ' using-System.Text=' + [string]$shHasUsingText)
    if ($script:Dr4ToolchainMd5 -eq '') { [void](Mark-Dr4Problem 'TOOLCHAIN-BAD=' 'md5 of shot_harness.ps1 unreadable') }
    if ($shUsesSb -and -not $shHasUsingText) {
        [void](Mark-Dr4Problem 'TOOLCHAIN-BAD=' 'uses StringBuilder without "using System.Text;" - defect T-1 is back: Add-Type fails with CS0246, the type never loads and every entry point is missing')
    }
    W 'TOOLCHAIN-VERDICT the shape of that core is judged by selftest_tools.ps1 rules 6/7/8 (ABI-USING/SHAPE/SIZE/OFFSET/UNION/LAYOUT/STAMP/CONTRACT + NATIVE-HOME/ORACLE), not by a comment here'
}
# 本颗自己那套注入核的尺寸是**运行量**出来的，不是注释写的：错了就别开这一轮。
$script:Dr4InputSize = 0
try { $script:Dr4InputSize = [int][System.Runtime.InteropServices.Marshal]::SizeOf([type][Dr4Native+INPUT]) } catch { }
W ('DR4-ABI INPUT=' + $script:Dr4InputSize + 'B ptr=' + [IntPtr]::Size +
   ' (a wrong total makes SendInput inject 0 events and still report success: defect T-6)')
if ($script:Dr4InputSize -ne 40) {
    [void](Mark-Dr4Problem 'DR4-ABI=' ('INPUT measured ' + $script:Dr4InputSize + 'B but x64 INPUT is 40B - a broken injection core injects 0 events and still reports success (defect T-6), so this round would be fabricated evidence'))
    W 'RESULT=error DR4-ABI INPUT is not 40B - refusing to run a foreground round on a broken injection core'
    exit 5
}

# ---------------------------------------------------------------- 自带抓图核（口径逐字照 shot_harness）
$script:Dr4Md5Seen = @{}
function Reset-Dr4Round { $script:Dr4Md5Seen = @{} }

# 阈值与键名一律照在册核 Get-Dr4Stats：rms<6 或 unique<=4 = near-monochrome；
# mean<8 = near-black；mean>247 = near-white。
function Get-Dr4Stats {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Path)
    $out = @{ w = 0; h = 0; unique = -1; mean = -1; rms = -1; bad = 'unreadable'; bytes = 0; md5 = '' }
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

# PrintWindow + PW_RENDERFULLCONTENT(2)：WinUI3 是 DirectComposition 宿主，少这一位 = 全白帧。
# ⚠ 没有 shotlib/shot_harness 那种 flag=0 降级腿（#35 根因）——宁可报失败也不端白帧。
function Invoke-Dr4Shot {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][IntPtr]$Hwnd, [string]$Path, [string]$Tag = 'shot', [scriptblock]$Log)
    if ($Hwnd -eq [IntPtr]::Zero) { W 'QASHOT-SKIPPED zero-hwnd'; return $null }
    $r = New-Object Dr4Native+RECT
    if (-not [Dr4Native]::GetWindowRect($Hwnd, [ref]$r)) { W 'QASHOT-SKIPPED no-window-rect'; return $null }
    $w = [Math]::Max(1, $r.Right - $r.Left); $h = [Math]::Max(1, $r.Bottom - $r.Top)
    if ($w -lt 8 -or $h -lt 8) { W ('QASHOT-SKIPPED degenerate ' + $w + 'x' + $h); return $null }
    $dir = [System.IO.Path]::GetDirectoryName($Path)
    if ($dir -ne '' -and -not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    $bmp = $null; $g = $null; $hdc = [IntPtr]::Zero; $hdcTaken = $false
    try {
        $bmp = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $hdc = $g.GetHdc(); $hdcTaken = $true
        $pw = [bool]([Dr4Native]::PrintWindow($Hwnd, $hdc, [uint32]2))
        $g.ReleaseHdc($hdc); $hdcTaken = $false
        if (-not $pw) { W ('QASHOT-PRINTWINDOW-FAILED hwnd=' + $Hwnd.ToInt64()); return $null }
        $g.Dispose(); $g = $null
        $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
        W ('QASHOT=' + [System.IO.Path]::GetFileName($Path) + ' (' + $w + 'x' + $h + ') flags=2')
        return $Path
    } catch {
        W ('QASHOT-THREW=' + (($_.Exception.Message -replace '\s+', ' ')))
        return $null
    } finally {
        if ($hdcTaken -and $null -ne $g) { try { $g.ReleaseHdc($hdc) } catch { } }
        if ($null -ne $g) { try { $g.Dispose() } catch { } }
        if ($null -ne $bmp) { try { $bmp.Dispose() } catch { } }
    }
}

# 前台阶梯：真 SendInput ALT 解锁（成规：不许伪造），三级 try，逐次打 FOREGROUND-BREAK 行。
function Invoke-Dr4Foreground {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][IntPtr]$Hwnd, [string]$Tag = 'fg')
    try { if ([Dr4Native]::GetForegroundWindow() -eq $Hwnd) { return ' fg-match=yes(already-foreground)' } } catch { }
    $rungs = @(
        @{ name = 'SetFG+AltTap'; act = {
                param($h)
                [void][Dr4Native]::SetForegroundWindow($h)
                [void][Dr4Native]::BringWindowToTop($h)
                $inj = [int]([Dr4Native]::TapAltReal())
                if ($inj -eq 0) { W ('FOREGROUND-BREAK ' + $Tag + ' ALT-TAP-INJECTED=0 (SendInput refused)'); [void](Mark-Dr4Problem 'ALT-TAP-INJECTED=' ('tag=' + $Tag + ' SendInput refused the ALT tap, so the foreground lock is still on: any CopyFromScreen frame from here photographs the wrong window')) }
                Start-Sleep -Milliseconds 150
            } },
        @{ name = 'AttachThreadAltActivate'; act = { param($h) [void][Dr4Native]::AltActivate($h); Start-Sleep -Milliseconds 200 } },
        @{ name = 'ShowWindow5'; act = { param($h) [void][Dr4Native]::ShowWindow($h, 5); [void][Dr4Native]::SetForegroundWindow($h); Start-Sleep -Milliseconds 220 } }
    )
    for ($t = 1; $t -le 3; $t++) {
        foreach ($r in $rungs) {
            try { & $r.act $Hwnd } catch { W ('FOREGROUND-BREAK ' + $Tag + ' RUNG-THREW=' + $r.name) }
            try { if ([Dr4Native]::GetForegroundWindow() -eq $Hwnd) {
                W ('FOREGROUND-BREAK ' + $Tag + ' tech=' + $r.name + ' try=' + $t + ' MATCH target=' + $Hwnd.ToInt64())
                return (' fg-match=yes(tech=' + $r.name + ' try=' + $t + ')')
            } } catch { }
        }
    }
    W ('FOREGROUND-BREAK ' + $Tag + ' tech=exhausted target=' + $Hwnd.ToInt64())
    [void](Mark-Dr4Problem 'FG-EXHAUSTED=' ('tag=' + $Tag + ' the whole foreground ladder ran out, so a screen grab taken now is not of this fork (ledger #35/#189)'))
    return ' fg-match=no(ladder-exhausted)'
}

# ---------------------------------------------------------------- launch
if (-not (Test-Path -LiteralPath $Exe)) { W "RESULT=error exe not found: $Exe"; exit 3 }
$logFile = "$LogPrefix.rs.txt"
if (Test-Path -LiteralPath $logFile) { Remove-Item -LiteralPath $logFile -Force }
$env:BLADE2_RS_LOG = $logFile
# T3K 洞 #3：以前 `$FakeKernelArgs = ''` 就是默认档，默认档打 `KERNEL=real` 且不注入
# BLADE2_KERNEL_EXE ⇒ 分叉去解析 %APPDATA%\npm\dsh.cmd 起**真 node 内核**（DR6P §1-A/§3 抬头 4）。
# 现在假内核是唯一默认路，真内核要显式 -RealKernel，而那一发必然以非零退出码收尾。
# selftest_tools.ps1 的 KERNEL-DEFAULT 锁把这颗默认值钉成非空串，并把起进程的位置逐枚点名。
$script:Dr4KernelMode = 'unresolved'
if ($RealKernel) {
    $script:Dr4KernelMode = 'real'
    [void](Mark-Dr4Problem 'REAL-KERNEL=' 'explicit -RealKernel: BLADE2_KERNEL_EXE is NOT injected, so the fork resolves its own kernel (a real node process). Every stub-side expectation of this round is void and the orphan sweep (#68) can touch user node processes. This run can never end green.')
    W 'KERNEL=real (EXPLICIT -RealKernel ONLY - red line, problem-marked)'
} else {
    if ($FakeKernelArgs -eq '') {
        [void](Mark-Dr4Problem 'KERNEL-NOFAKE=' '-FakeKernelArgs was emptied while -RealKernel is off: falling through to that arm used to start the real kernel, so this run is pinned back onto --pace=0 and stays red')
        $FakeKernelArgs = '--pace=0'
    }
    $fk = Join-Path (Split-Path -Parent $Exe) 'fake_dsh.exe'
    if (-not (Test-Path -LiteralPath $fk)) {
        [void](Mark-Dr4Problem 'KERNEL-NOFAKE=' ('fake kernel missing at ' + $fk + ' - the fake arm is the ONLY arm this driver may take'))
        W ("RESULT=error fake kernel missing: " + $fk); exit 3
    }
    $env:BLADE2_KERNEL_EXE = $fk
    $env:BLADE2_KERNEL_ARGS = $FakeKernelArgs
    $script:Dr4KernelMode = 'fake'
    W ('KERNEL=fake exe=' + $fk + ' args=' + $FakeKernelArgs + ' (默认档：真内核只在显式 -RealKernel 时才可能)')
}
if ($script:Dr4KernelMode -eq 'unresolved') { W 'RESULT=error kernel mode unresolved'; exit 3 }
# 离线可证的锁：这里点名「本颗只起这一枚进程」，且它必须是 .exe。
W ('KERNEL-LOCK mode=' + $script:Dr4KernelMode + ' BLADE2_KERNEL_EXE=' +
   $(if ($script:Dr4KernelMode -eq 'fake') { [string]$env:BLADE2_KERNEL_EXE } else { 'NOT-INJECTED(REAL)' }) +
   ' launch-sites=1 purpose=the-window-under-test')
$si = New-Object System.Diagnostics.ProcessStartInfo
$si.FileName = (Resolve-Path -LiteralPath $Exe).Path
if ($si.FileName -notmatch '\.exe$') {
    [void](Mark-Dr4Problem 'PROC-LAUNCH-NOT-EXE=' ($si.FileName + ' - this driver may only start the window under test, never a .cmd wrapper (dsh.cmd/node.exe is the red line)'))
    W 'RESULT=error refusing to launch a non-exe target'; exit 3
}
$si.UseShellExecute = $false
$si.RedirectStandardOutput = $true
$si.RedirectStandardError = $true
$proc = [System.Diagnostics.Process]::Start($si)
W ('PID=' + $proc.Id)
$stdoutJob = $proc.StandardOutput.ReadToEndAsync()
$stderrJob = $proc.StandardError.ReadToEndAsync()

$TRUECOND = [System.Windows.Automation.Condition]::TrueCondition
$auto = [System.Windows.Automation.AutomationElement]
$nameProp = [System.Windows.Automation.AutomationElementIdentifiers]::NameProperty

function Get-TopWindows([int]$forPid) {
    $list = New-Object System.Collections.Generic.List[object]
    $cb = [Dr4Native+EnumWindowsProc] {
        param($h, $l)
        try {
            $p = 0
            [void][Dr4Native]::GetWindowThreadProcessId($h, [ref]$p)
            if ($p -eq [uint32]$forPid -and [Dr4Native]::IsWindowVisible($h)) {
                $r = New-Object Dr4Native+RECT
                [void][Dr4Native]::GetWindowRect($h, [ref]$r)
                $list.Add(@{ h = $h; t = [Dr4Native]::FgTitle($h); w = ($r.Right - $r.Left); ht = ($r.Bottom - $r.Top) }) | Out-Null
            }
        } catch { }
        return $true
    }
    [void][Dr4Native]::EnumWindows($cb, [IntPtr]::Zero)
    return $list
}

$deadline = (Get-Date).AddSeconds($TimeoutSec)
$mainHwnd = [IntPtr]::Zero
$win = $null
while ((Get-Date) -lt $deadline) {
    if ($proc.HasExited) { W ('APP-EXITED-EARLY code=' + $proc.ExitCode); break }
    $ws = @(Get-TopWindows $proc.Id | Where-Object { $_.w -gt 200 -and $_.ht -gt 200 -and $_.t -match $TitlePattern })
    if ($ws.Count -gt 0) {
        $biggest = $ws | Sort-Object { $_.w * $_.ht } -Descending | Select-Object -First 1
        $mainHwnd = $biggest.h
        try { $win = $auto::FromHandle($mainHwnd) } catch { $win = $null }
        if ($null -ne $win) { break }
    }
    Start-Sleep -Milliseconds 300
}
if ($null -eq $win) { W 'RESULT=error no window / no UIA element'; exit 4 }
W ('WINDOW=' + (Fmt $biggest.t) + ' HWND=' + $mainHwnd.ToInt64() + ' size=' + (RectStr $win.Current.BoundingRectangle))

# ---------------------------------------------------------------- scoped scan
function Get-ScopeElements([int]$cap) {
    $all = New-Object System.Collections.Generic.List[object]
    $roots = New-Object System.Collections.Generic.List[object]
    try { $roots.Add($win) | Out-Null } catch { }
    try {
        foreach ($t in (Get-TopWindows $proc.Id)) {
            if ($t.h -eq $mainHwnd) { continue }
            try { $e = $auto::FromHandle($t.h); if ($null -ne $e) { $roots.Add($e) | Out-Null } } catch { }
        }
    } catch { }
    foreach ($r in $roots) {
        try {
            $col = $r.FindAll([System.Windows.Automation.TreeScope]::Descendants, $TRUECOND)
            $n = [Math]::Min($col.Count, $cap)
            for ($i = 0; $i -lt $n; $i++) { try { $e = $col[$i]; if ($null -ne $e) { $all.Add($e) | Out-Null } } catch { } }
        } catch { }
    }
    return $all
}

function El-Line($e, [int]$d) {
    $ct = '?'
    try { $ct = $e.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '' } catch { }
    $aid = ''; $nm = ''; $foc = ''
    try { $aid = [string]$e.Current.AutomationId } catch { }
    try { $nm = [string]$e.Current.Name } catch { }
    try { $foc = [string]$e.Current.IsKeyboardFocusable } catch { }
    return ($d.ToString() + ' | ' + $ct + ' | id=' + (Fmt $aid) + ' | ' + (Fmt $nm) + ' | ' + $foc + ' | ' + (RectStr $e.Current.BoundingRectangle))
}

function Find-El([string]$key) {
    $all = Get-ScopeElements $Cap
    $byId = @(); $byNameEq = @(); $byIdRe = @(); $byNameRe = @()
    foreach ($e in $all) {
        $aid = ''; $nm = ''
        try { $aid = [string]$e.Current.AutomationId } catch { }
        try { $nm = [string]$e.Current.Name } catch { }
        if ($aid -eq $key) { $byId += $e; continue }
        if ($nm -eq $key) { $byNameEq += $e; continue }
        try { if ($aid -match $key) { $byIdRe += $e; continue } } catch { }
        try { if ($nm -match $key) { $byNameRe += $e } } catch { }
    }
    $f = $null
    if ($byId.Count -gt 0) { $f = @{ how = 'id'; els = $byId } }
    elseif ($byNameEq.Count -gt 0) { $f = @{ how = 'name-eq'; els = $byNameEq } }
    elseif ($byIdRe.Count -gt 0) { $f = @{ how = 'id-re'; els = $byIdRe } }
    elseif ($byNameRe.Count -gt 0) { $f = @{ how = 'name-re'; els = $byNameRe } }
    else { return @{ how = 'none'; els = @() } }
    # T3K 洞 #1（DR6P §1-A「find:Permission 一次命中四颗，点错元素仍报 INVOKED」）：
    # 正则兜底桶与「多颗同名」都必须喊出来 —— 命中几颗是**量出来的** els.Count，不是措辞。
    if ($f.how.EndsWith('-re') -and -not $LooseFind) {
        [void](Mark-Dr4Problem 'FIND-BY-REGEX=' ('how=' + $f.how + ' key=' + $key + ' n=' + $f.els.Count +
            ' - a half-written key resolves through the regex fallback and can invoke the wrong element; write the full AutomationId (or pass -LooseFind to make this a printout)'))
    }
    if ($f.els.Count -gt 1) {
        [void](Mark-Dr4Problem 'AMBIGUOUS-FIND=' ('how=' + $f.how + ' key=' + $key + ' n=' + $f.els.Count +
            ' - the driver only ever touches els[0], so the other ' + ($f.els.Count - 1) + ' candidate(s) make this verdict unattributable'))
    }
    return $f
}

# ---------------------------------------------------------------- inv: 的四条真臂（T5K 删掉第五条静默死臂）
# 判断顺序（先复量，再处置，别跳步）：
#  1. 复量（反射枚类型，本机 PS 5.1.26100.9568，全文证据 rust/tmp/t5k-report.md §1）
#     UIAutomationClient 导出类型 57 枚（顶层 41 枚，其中 *Pattern* 38 枚）、
#     UIAutomationTypes 导出类型 69 枚（其中 *PatternIdentifiers* 21 枚）、WindowsBase 155 枚、
#     UIAutomationProvider 27 枚。把这四枚装配的全部类型（含嵌套）拿名字去枚
#     Legacy / IAccessible / MSAA = **0 命中**（整个 AppDomain 只命中 mscorlib 的
#     Security.Policy.ILegacyEvidenceAdapter 那一族，和 UIAutomationClient 的内部类
#     MS.Internal.Automation.Accessible，两枚都不是 Pattern）。
#     ⇒ [System.Windows.Automation.LegacyIAccessiblePatternIdentifiers] 与
#       [System.Windows.Automation.LegacyIAccessiblePattern] 在本机 PowerShell 5.1 **不存在**。
#       单写 [Type]::Pattern 运行时抛 RuntimeException「找不到类型 […]」；旧的那两行套的是**空
#       catch {}** ⇒ 一声不响，inv: 的兜底臂从写下那天起一步都没跑过，而 INVOKED= 照样打绿。
#       （比引用上一张票的结论更硬的一点：枚的时候先用 [Type]::GetType('名, UIAutomationClient')
#        试了一遍，连 InvokePatternIdentifiers 都报 ABSENT —— 那 21 枚 *PatternIdentifiers* 其实在
#        UIAutomationTypes，不在 UIAutomationClient。所以类型存在性只能按 PowerShell 自己的类型
#        字面量解析来量，拿单装配名的 GetType 当尺会造出假阴性。）
#  2. 真能用的等价路径 = 下面这 4 枚，逐枚反射证据（同一次枚表，全部 PRESENT）：
#       InvokePatternIdentifiers / TogglePatternIdentifiers / SelectionItemPatternIdentifiers /
#       ExpandCollapsePatternIdentifiers，同名 Pattern 类 InvokePattern / TogglePattern /
#       SelectionItemPattern / ExpandCollapsePattern 也都在 UIAutomationClient 里。
#       ValuePatternIdentifiers 同样在（走 set:/type:，不算 inv: 的臂）。
#     LegacyIAccessible 的 DoDefaultAction 只剩 COM 那条路（UIAutomationCore 的
#     IUIAutomationElement2::get_LegacyIAccessiblePattern），要走得新写一段 Add-Type
#     -TypeDefinition COM 胶水 —— 本票 §1-4 明令不许为补兜底造胶水，selftest_tools.ps1 的 rule 14
#     也把 -TypeDefinition 钉死在 0，所以**本机结构性没有这条腿**。
#  3. 处置 = 选「删兜底臂 + fail-closed」这一条，不选「换一条臂」：
#     四条真臂全空手时这一发就是没成，由调用方打 **NOTINVOKED=**（已入名册且钉进 critical），
#     **绝不许"没兜底就当成功"**。旧写法在这里只打 INVOKED= + 一个空的 pattern= ，那就是
#     打印即绿；这条腿的红绿从此由累计器说话，不由一行字面说话。
#  4. 口径变更：返回值从字符串改成状态表（ok/via/why/tried），和同仓 gui_uia.ps1 的 Invoke-Element
#     同形制。顺手修掉旧臂序的一处真缺陷：旧写法第 2 臂之后判的是 $did.Count -eq 0，而 Invoke 抛
#     异常时 $did 里已经有一条 'Invoke-threw:…' ⇒ Count=1 ⇒ 第 3/4 臂被跳过。改成"没成就接着试"。
function Invoke-El($e) {
    $tries = @(
        @{ n = 'Invoke';        id = [System.Windows.Automation.InvokePatternIdentifiers]::Pattern;        act = 'Invoke' },
        @{ n = 'Toggle';        id = [System.Windows.Automation.TogglePatternIdentifiers]::Pattern;        act = 'Toggle' },
        @{ n = 'SelectionItem'; id = [System.Windows.Automation.SelectionItemPatternIdentifiers]::Pattern; act = 'Select' },
        @{ n = 'ExpandCollapse'; id = [System.Windows.Automation.ExpandCollapsePatternIdentifiers]::Pattern; act = 'Expand' }
    )
    $tried = @(); $whys = @()
    foreach ($t in $tries) {
        $pat = $null
        try { $pat = $e.GetCurrentPattern($t.id) } catch { $pat = $null }
        if ($null -eq $pat) { $tried += ($t.n + '=absent'); continue }
        try {
            if ($t.act -eq 'Invoke') { [void]([System.Windows.Automation.InvokePattern]$pat).Invoke() }
            elseif ($t.act -eq 'Toggle') { [void]([System.Windows.Automation.TogglePattern]$pat).Toggle() }
            elseif ($t.act -eq 'Select') { [void]([System.Windows.Automation.SelectionItemPattern]$pat).Select() }
            else { [void]([System.Windows.Automation.ExpandCollapsePattern]$pat).Expand() }
            $tried += ($t.n + '=ok')
            return @{ ok = $true; via = $t.n; why = ''; tried = ($tried -join ' ') }
        } catch {
            $tried += ($t.n + '=threw')
            $whys += ($t.n + ':' + $_.Exception.GetType().Name)
        }
    }
    return @{ ok = $false; via = ''; why = (($whys -join ' ') + ' | none of the 4 real patterns took; PS 5.1 has no LegacyIAccessible fallback arm');
             tried = ($tried -join ' ') }
}

# 前台闸：真键盘 / 真指针之前必须先把窗抬起来（否则 SendInput 打到操作者正在看的窗）。
function Ensure-Foreground([string]$tag) {
    $note = [string](Invoke-Dr4Foreground -Hwnd $mainHwnd -Tag ('fg-' + $tag))
    W ('FOREGROUND=' + $tag + ' note=' + $note)
    return $note
}

function Center-Of($e) {
    try {
        $b = $e.Current.BoundingRectangle
        if ($b.IsEmpty) { return $null }
        return @{ x = [int]($b.X + $b.Width / 2); y = [int]($b.Y + $b.Height / 2) }
    } catch { return $null }
}

# ---------------------------------------------------------------- 新腿：值回读
function Read-Val([string]$key) {
    $f = Find-El $key
    if ($f.how -eq 'none') { W ('VAL-NOTFOUND=' + $key); return }
    $e = $f.els[0]
    $v = ''
    try {
        $p = $e.GetCurrentPattern([System.Windows.Automation.ValuePatternIdentifiers]::Pattern)
        if ($null -ne $p) { $v = [string]([System.Windows.Automation.ValuePattern]$p).Value }
    } catch { $v = 'THREW:' + $_.Exception.GetType().Name }
    $sel = ''
    try {
        $p = $e.GetCurrentPattern([System.Windows.Automation.SelectionItemPatternIdentifiers]::Pattern)
        if ($null -ne $p) { $sel = ' selected=' + [string]([System.Windows.Automation.SelectionItemPattern]$p).Current.IsSelected }
    } catch { }
    $tg = ''
    try {
        $p = $e.GetCurrentPattern([System.Windows.Automation.TogglePatternIdentifiers]::Pattern)
        if ($null -ne $p) { $tg = ' toggle=' + [string]([System.Windows.Automation.TogglePattern]$p).Current.ToggleState }
    } catch { }
    W ('VALREAD=' + $key + ' how=' + $f.how + ' n=' + $f.els.Count + ' value=[' + (Fmt $v) + ']' + ' name=[' + (Fmt $e.Current.Name) + '] rect=' + (RectStr $e.Current.BoundingRectangle) + $sel + $tg)
}

# ---------------------------------------------------------------- 新腿：真打字 / 真键 / 真指针
function Send-Text([string]$key, [string]$text) {
    Ensure-Foreground 'type' | Out-Null
    $f = Find-El $key
    if ($f.how -eq 'none') { W ('TYPE-NOTFOUND=' + $key); return }
    $e = $f.els[0]
    $via = ''
    try { $e.SetFocus(); $via = 'SetFocus' } catch { $via = 'SetFocus-threw' }
    if ($via -eq 'SetFocus-threw') {
        $c = Center-Of $e
        if ($null -ne $c) { [void]([Dr4Native]::ClickAbs([int]$c.x, [int]$c.y)); Start-Sleep -Milliseconds 250; $via = 'RealClick' }
    }
    Start-Sleep -Milliseconds 250
    $inj = 0
    try { $inj = [int]([Dr4Native]::TypeText($text)) } catch { W ('TYPE-THREW=' + (($_.Exception.Message -replace '\s+', ' '))) }
    W ('TYPED=' + $text.Length + ' chars codepoints-injected=' + $inj + ' focus-via=' + $via + ' key=' + $key)
    if ($inj -lt ($text.Length * 2)) { W 'TYPE-DEGRADED SendInput 注入数 < 期望（这一发不能算真打字）' }
    Start-Sleep -Milliseconds 500
}

function Send-Key([string]$spec) {
    Ensure-Foreground 'keyin' | Out-Null
    $mods = @(); $name = $spec
    while ($true) {
        if ($name -match '^shift\+(.+)$') { $mods += 0x10; $name = $Matches[1]; continue }
        if ($name -match '^ctrl\+(.+)$') { $mods += 0x11; $name = $Matches[1]; continue }
        if ($name -match '^alt\+(.+)$') { $mods += 0x12; $name = $Matches[1]; continue }
        break
    }
    $map = @{ esc = 0x1B; ret = 0x0D; enter = 0x0D; up = 0x26; down = 0x28; left = 0x25; right = 0x27;
              tab = 0x09; bksp = 0x08; space = 0x20; home = 0x24; end = 0x23; del = 0x2E; f6 = 0x75 }
    $vk = 0
    if ($map.ContainsKey($name)) { $vk = $map[$name] }
    elseif ($name -match '^[0-9A-Fa-f]+$') { $vk = [Convert]::ToInt32($name, 16) }
    else { W ('KEYIN-UNKNOWN=' + $spec); return }
    $report = @()
    foreach ($m in $mods) { $report += ('hold-' + $m + '=' + [int][Dr4Native]::HoldKey([uint16]$m, $true)) }
    $report += ('tap-' + $name + '=' + [int][Dr4Native]::TapKey([uint16]$vk))
    foreach ($m in $mods) { $report += ('rel-' + $m + '=' + [int][Dr4Native]::HoldKey([uint16]$m, $false)) }
    W ('KEYIN=' + $spec + ' vk=' + $vk + ' (' + ($report -join ' ') + ')')
    Start-Sleep -Milliseconds 450
}

function Do-Hover([string]$key) {
    Ensure-Foreground 'hover' | Out-Null
    $f = Find-El $key
    if ($f.how -eq 'none') { W ('HOVER-NOTFOUND=' + $key); return }
    $c = Center-Of $f.els[0]
    if ($null -eq $c) { W ('HOVER-NO-RECT=' + $key); return }
    $a = [int]([Dr4Native]::MoveAbs([int]$c.x, [int]$c.y))
    Start-Sleep -Milliseconds 120
    $b = [int]([Dr4Native]::MoveAbs([int]($c.x + 3), [int]$c.y))
    Start-Sleep -Milliseconds 120
    $d = [int]([Dr4Native]::MoveAbs([int]$c.x, [int]$c.y))
    W ('HOVERED=' + $key + ' how=' + $f.how + ' at=' + $c.x + ',' + $c.y + ' injected=' + ($a + $b + $d) + ' (真 SendInput 移动 + 抖 3px)')
    Start-Sleep -Milliseconds 500
}

function Do-Click([string]$key) {
    Ensure-Foreground 'click' | Out-Null
    $f = Find-El $key
    if ($f.how -eq 'none') { W ('CLICK-NOTFOUND=' + $key); return }
    $c = Center-Of $f.els[0]
    if ($null -eq $c) { W ('CLICK-NO-RECT=' + $key); return }
    $n = [int]([Dr4Native]::ClickAbs([int]$c.x, [int]$c.y))
    W ('CLICKED=' + $key + ' how=' + $f.how + ' at=' + $c.x + ',' + $c.y + ' injected=' + $n + ' (UIA 定位后真 SendInput 左键)')
    Start-Sleep -Milliseconds 700
}

# ---------------------------------------------------------------- 截图
function Shot-Name([string]$name) {
    if ($name -notmatch '^dr4-') { $name = 'dr4-' + $name }
    if ($name -notmatch '\.png$') { $name = $name + '.png' }
    return $name
}
function Take-Shot([string]$name) {
    if (-not (Test-Path -LiteralPath $OutDir)) { New-Item -ItemType Directory -Force -Path $OutDir | Out-Null }
    $name = Shot-Name $name
    $path = Join-Path $OutDir $name
    $note = [string](Invoke-Dr4Foreground -Hwnd $mainHwnd -Tag $name)
    W ('FOREGROUND=' + $name + ' note=' + $note + ' hwnd=' + $mainHwnd.ToInt64())
    $null = Invoke-Dr4Shot -Hwnd $mainHwnd -Path $path
    if (Test-Path -LiteralPath $path) {
        $st = Get-Dr4Stats -Path $path
        $md5 = (Get-FileHash -Algorithm MD5 -LiteralPath $path).Hash.ToLower()
        W ('SHOT=' + $name + ' md5=' + $md5 + ' px=' + $st.w + 'x' + $st.h + ' bytes=' + $st.bytes + ' unique=' + $st.unique + ' mean=' + $st.mean + ' rms=' + $st.rms + ' bad=' + [string]$st.bad)
    } else {
        # T5K：这一发没拍到帧 = 真失败，不是跳过。SHOTFAILED= 以前只是一行字面，名册里没有它，
        # 所以整轮可以一张帧都没落地而横幅全绿。同仓 gui_uia.ps1 早就把它在册并判 critical。
        # 带上 hwnd / 落盘路径 / 前台阶梯的 note，让这一行自己说清是哪儿没成。
        W ('SHOTFAILED=' + $name + ' hwnd=' + $mainHwnd.ToInt64() + ' path=' + $path + ' fgnote=' + $note)
    }
}
# ---------------------------------------------------------------- 逐元素裁剪（eshot: 腿唯一的一处裁图）
# T4K 修的那一发（原 dr4-drive.ps1:777）：写法是
#   $bmp.Clone([System.Drawing.Rectangle]::FromRectangle([System.Drawing.Rectangle]::MakeRectangle(...)), $bmp.PixelFormat)
# 现测（rust/tmp/t4k-audit-statics.txt，反射枚成员）System.Drawing.Rectangle 的静态成员是
#   Ceiling Equals FromLTRB Inflate Intersect op_Equality op_Inequality ReferenceEquals Round Truncate Union
# ⇒ MakeRectangle 不存在，外层那发 FromRectangle **也不存在**（FromRectangle 是 RectangleF 的成员，
# Rectangle 只有 FromLTRB），所以这一发每调用必抛 RuntimeException，整条 eshot: 腿一张裁剪图都没出过。
# 正解走真正存在的四元组构造 .ctor(Int32 x, Int32 y, Int32 width, Int32 height)，FromRectangle 套壳一并去掉。
#
# 为什么把它抽成一颗函数：rust/tests/selftest_tools.ps1 要能在**不开窗、不起 exe、不碰 UIA** 的前提下
# 把**同一条代码路**真跑一遍（它执行的是从本文件磁盘原文里取出的这颗函数，不是抄一份），
# 枚不到方法名这种错只有跑一次才会响。
#
# 口径没有动：clamp（[Math]::Max / [Math]::Min 到边框）与 degenerate（<4px）判据逐字保留。
# 本函数不打印、不记问题，只回一个状态表（kind/msg/cw/ch）——日志与累计器归调用方 Take-ElShot，
# 走既有的 W + $script:Dr4FailureTokens + $script:Dr4Problems + $rc 那张网，不另造一套 verdict 机制。
function Crop-Dr4Rect {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$SourcePath,
        [Parameter(Mandatory = $true)][string]$OutPath,
        [Parameter(Mandatory = $true)][int]$X,
        [Parameter(Mandatory = $true)][int]$Y,
        [Parameter(Mandatory = $true)][int]$CropW,
        [Parameter(Mandatory = $true)][int]$CropH
    )
    $res = @{ ok = $false; kind = 'missing'; cw = 0; ch = 0; msg = '' }
    if (-not (Test-Path -LiteralPath $SourcePath)) { $res.msg = 'source frame missing: ' + $SourcePath; return $res }
    $bmp = $null; $crop = $null
    try {
        $bmp = New-Object System.Drawing.Bitmap($SourcePath)
        $cx = [Math]::Max(0, $X); $cy = [Math]::Max(0, $Y)
        $cw = [Math]::Min($CropW, $bmp.Width - $cx); $ch = [Math]::Min($CropH, $bmp.Height - $cy)
        $res.cw = $cw; $res.ch = $ch
        if ($cw -lt 4 -or $ch -lt 4) { $res.kind = 'degenerate'; $res.msg = 'degenerate ' + $cw + 'x' + $ch; return $res }
        $crop = $bmp.Clone([System.Drawing.Rectangle]::new($cx, $cy, $cw, $ch), $bmp.PixelFormat)
        $crop.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
        $res.ok = $true; $res.kind = 'ok'; $res.msg = ''
    } catch {
        $res.kind = 'threw'
        $res.msg = ((('' + $_.Exception.GetType().Name) + ': ' + ($_.Exception.Message -replace '\s+', ' ')))
        # fail-closed：裁图失败绝不降级成"跳过 = 通过"，也不留半帧当证据。
        try { if (Test-Path -LiteralPath $OutPath) { Remove-Item -LiteralPath $OutPath -Force } } catch { }
    } finally {
        if ($null -ne $crop) { $crop.Dispose() }
        if ($null -ne $bmp) { $bmp.Dispose() }
    }
    return $res
}

function Take-ElShot([string]$key, [string]$name) {
    $f = Find-El $key
    if ($f.how -eq 'none') { W ('ESHOT-NOTFOUND=' + $key); return }
    $b = $f.els[0].Current.BoundingRectangle
    if ($b.IsEmpty) {
        # T5K：找得到元素但没有几何 = 要拍的这张图不存在，这一发没成（在册 + critical）。
        # 不是"元素本来就该没矩形"的合法跳过：那种情况该由 ESHOT-NOTFOUND= 那条路说话。
        W ('ESHOT-EMPTY-RECT=' + $key + ' how=' + $f.how + ' n=' + $f.els.Count +
           ' name=' + (Fmt $f.els[0].Current.Name) + ' id=' + (Fmt $f.els[0].Current.AutomationId)); return
    }
    $full = Join-Path $OutDir ((Shot-Name ($name + '-full')) )
    Take-Shot ($name + '-full') | Out-Null
    if (-not (Test-Path -LiteralPath $full)) { W 'ESHOT-SKIPPED no full frame'; return }
    $wr = [Dr4Native+RECT]::new()
    [void][Dr4Native]::GetWindowRect($mainHwnd, [ref]$wr)
    $x = [int]($b.X - $wr.Left); $y = [int]($b.Y - $wr.Top)
    $w = [int]$b.Width; $h = [int]$b.Height
    $name2 = Shot-Name $name
    $out = Join-Path $OutDir $name2
    # clamp + degenerate + 裁剪 + 存盘全在 Crop-Dr4Rect 里（判据逐字照旧，见那颗函数的注释）。
    $cr = Crop-Dr4Rect -SourcePath $full -OutPath $out -X $x -Y $y -CropW $w -CropH $h
    if ($cr.kind -eq 'degenerate') {
        # T5K 判档：degenerate 有两种，只有一种是元素的正常态，两色不能混打一枚前缀。
        #   req 本身就 <4px   ⇒ 元素只有 1~3px（分隔条、已收起的箭头），裁不出有意义的图。
        #                      这是**元素的属性**，不是工具坏 ⇒ ESHOT-DEGENERATE：入册（所以不可能
        #                      静默），但 selftest 故意不把它钉进 critical，并且允许按既有
        #                      -TolerateAbsent 那一族显式豁免 ⇒ 正常轮不会被它钉成永远红。
        #   req >=4px 而 clamp 后 <4px ⇒ 元素矩形落在窗口框外 / 坐标算错，是我的几何坏 ⇒
        #                      走 ESHOT-CROP-FAILED=（在册 + 已在 critical，T4K 钉的），
        #                      绝不许它冒充"小元素"混过去。
        if ($w -lt 4 -or $h -lt 4) {
            W ('ESHOT-DEGENERATE ' + $cr.cw + 'x' + $cr.ch + ' key=' + $key + ' req=' + $w + 'x' + $h +
               ' out=' + $name2 + ' the element itself is below the 4px crop floor, this is the tolerated-skip' +
               ' family: opt in with -TolerateAbsent ^ESHOT-DEGENERATE only when this element is meant to be that small')
        } else {
            W ('ESHOT-CROP-FAILED=' + $key + ' kind=clamp-lost req=' + $w + 'x' + $h + ' @' + $x + ',' + $y +
               ' clamped=' + $cr.cw + 'x' + $cr.ch + ' out=' + $name2 +
               ' the requested rect is >=4px but the clamp threw the crop away, so the element geometry is wrong')
        }
        return
    }
    if (-not $cr.ok) {
        # 带失败前缀的一发：W 会把它经 Test-Dr4FailureToken 记进 $script:Dr4Problems ⇒
        # 结尾 RESULT=FAIL problems=N + exit 1。"裁图失败"在这里**不是**跳过，是一发没成。
        W ('ESHOT-CROP-FAILED=' + $key + ' kind=' + $cr.kind + ' req=' + $w + 'x' + $h +
           ' @' + $x + ',' + $y + ' clamped=' + $cr.cw + 'x' + $cr.ch + ' out=' + $name2 + ' ' + $cr.msg)
        return
    }
    $st = Get-Dr4Stats -Path $out
    $md5 = (Get-FileHash -Algorithm MD5 -LiteralPath $out).Hash.ToLower()
    W ('ESHOT=' + $name2 + ' key=' + $key + ' how=' + $f.how + ' crop=' + $cr.cw + 'x' + $cr.ch + ' md5=' + $md5 + ' unique=' + $st.unique + ' mean=' + $st.mean + ' rms=' + $st.rms + ' bad=' + [string]$st.bad)
}

# ---------------------------------------------------------------- 日志两把尺（T3K 洞 #3 附带 + 过期账 B）
# DR6P §1-A 点名：同一颗脚本里 `waitlog:` 用字面 Contains、`loggrep:` 用 `-match`（正则），
# 于是 `-match '[SCROLL]'` 把方括号吃成字符类、`(` `+` 吃成量词 ⇒ 一整批必假 FAIL。
# 现在两把尺同口径（字面 Ordinal 子串），要正则请显式走 `logre:`，且它跑之前先过一遍编译。
# 过期账 B（DR6P §5-3）：分叉的 push_log 无条件加 `DIAG: ` 前缀，而调用点自己就写 `DIAG: …`
# ⇒ 实盘行首是 `DIAG: DIAG: `。锚行首的 needle **必假 FAIL**，所以这条口径不写成措辞、写成闸：
# 以 `^` 开头的 needle 一落地就记问题并立刻停等，不再白烧一整把超时。
$script:Dr4LogPrefixCaliber = 'DIAG: DIAG: (push_log prefixes every DIAG line; 18 call sites already write "DIAG: " themselves) => use the INNER substring; a line-start anchor is ALWAYS a false FAIL'
function Test-Dr4AnchoredNeedle([string]$needle) {
    if ($null -eq $needle) { return $false }
    return ($needle.StartsWith('^') -or $needle.StartsWith('\A'))
}
function Show-Dr4LogHits([object[]]$hits, [string]$mode, [string]$pat) {
    W ('LOGGREP=' + $pat + ' mode=' + $mode + ' hits=' + $hits.Count)
    $shown = 0
    foreach ($l in $hits) { W ('  | ' + $l); $shown++; if ($shown -ge 40) { W '  ... capped at 40'; break } }
}
function Get-Dr4LogLines {
    if (-not (Test-Path -LiteralPath $logFile)) { return $null }
    try { return @([System.IO.File]::ReadAllLines($logFile)) } catch { return $null }
}
function Wait-Log([string]$needle, [int]$sec) {
    if (Test-Dr4AnchoredNeedle $needle) {
        [void](Mark-Dr4Problem 'NEEDLE-ANCHOR=' ($needle + ' - waitlog is a literal substring match, so a leading ^ can never hit anything (and every DIAG line is double-prefixed). Not waiting.'))
        return $false
    }
    $d = (Get-Date).AddSeconds($sec)
    while ((Get-Date) -lt $d) {
        if (Test-Path -LiteralPath $logFile) {
            $c = ''
            try { $c = [System.IO.File]::ReadAllText($logFile) } catch { }
            if ($c.Contains($needle)) {
                $line = ($c -split "`r?`n" | Where-Object { $_.IndexOf($needle, [System.StringComparison]::Ordinal) -ge 0 } | Select-Object -Last 1)
                W ('WAITFORLOG-MET=' + (Fmt $line)); return $true
            }
        }
        Start-Sleep -Milliseconds 250
    }
    W ('WAITFORLOG-TIMEOUT=' + $needle)
    $dbl = 0
    $ls = Get-Dr4LogLines
    if ($null -ne $ls) { $dbl = @($ls | Where-Object { $_.StartsWith('DIAG: DIAG:') }).Count }
    W ('LOGPREFIX-CALIBER double-DIAG-lines=' + $dbl + ' note=' + $script:Dr4LogPrefixCaliber)
    return $false
}

function Close-App() {
    W 'CLOSE-begin (CloseMainWindow, never taskkill)'
    try {
        $p = $win.GetCurrentPattern([System.Windows.Automation.WindowPatternIdentifiers]::Pattern)
        if ($null -ne $p) { [void]([System.Windows.Automation.WindowPattern]$p).Close(); W 'CLOSE-VIA=UIA WindowPattern' }
    } catch { W ('CLOSE-UIA-THREW=' + $_.Exception.GetType().Name) }
    try { if (-not $proc.HasExited) { [void]$proc.CloseMainWindow(); W 'CLOSE-VIA=Process.CloseMainWindow' } } catch { }
    $d = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $d -and -not $proc.HasExited) { Start-Sleep -Milliseconds 300 }
    W ('APP-EXITED=' + [string]$proc.HasExited)
}

# ---------------------------------------------------------------- step loop
W ('LOGFILE=' + $logFile)
W ('OUTDIR=' + $OutDir)
W ('HARNESS=' + $script:Dr4HarnessVersion)
Reset-Dr4Round
if ($RaiseFirst -ne '') { Wait-Log $RaiseFirst 40 | Out-Null }

foreach ($raw in ($Steps -split ';')) {
    $s = $raw.Trim()
    if ($s -eq '') { continue }
    $i = $s.IndexOf(':')
    if ($i -lt 0) { $verb = $s; $arg = '' } else { $verb = $s.Substring(0, $i); $arg = $s.Substring($i + 1) }
    W ('--- STEP ' + $s)
    try {
    switch ($verb) {
        'shot' { Take-Shot $arg }
        'eshot' {
            $k = $arg.IndexOf('=')
            if ($k -lt 0) { W 'ESHOT-BADSPEC use eshot:<key>=<name>' } else { Take-ElShot $arg.Substring(0, $k) $arg.Substring($k + 1) }
        }
        'tree' {
            $capv = $Cap
            if ($arg -match '^\d+$') { $capv = [int]$arg }
            $all = Get-ScopeElements $capv
            $n = 0
            foreach ($e in $all) { W (El-Line $e 1); $n++ }
            W ('elements=' + $n)
            if ($n -lt 5) {
                [void](Mark-Dr4Problem 'TREE-THIN=' ($n + ' elements - below the liveness floor this tool chain itself documents (gui_uia.ps1 uses the same >=5 bar). find/count/inv verdicts on a tree this empty mean nothing'))
            }
        }
        'find' {
            $all = Get-ScopeElements $Cap
            $n = 0
            foreach ($e in $all) {
                $aid = ''; $nm = ''
                try { $aid = [string]$e.Current.AutomationId } catch { }
                try { $nm = [string]$e.Current.Name } catch { }
                if ($aid -match $arg -or $nm -match $arg) {
                    W (El-Line $e 1); $n++
                    if ($n -ge 60) { W '... find capped at 60'; break }
                }
            }
            W ('find-matches=' + $n)
        }
        'count' {
            $all = Get-ScopeElements $Cap
            $n = 0; $names = @()
            foreach ($e in $all) {
                $aid = ''; $nm = ''
                try { $aid = [string]$e.Current.AutomationId } catch { }
                try { $nm = [string]$e.Current.Name } catch { }
                if ($aid -match $arg -or $nm -match $arg) { $n++; if ($names.Count -lt 30) { $names += ('[' + (Fmt $nm) + ']') } }
            }
            W ('count-matches=' + $n + ' first=' + ($names -join ' '))
        }
        'inv' {
            $f = Find-El $arg
            if ($f.how -eq 'none') { W ('NOTFOUND=' + $arg) }
            else {
                $r = Invoke-El $f.els[0]
                if ($r.ok) {
                    W ('INVOKED=' + $arg + ' how=' + $f.how + ' pattern=' + $r.via + ' tried=' + $r.tried + ' n=' + $f.els.Count + ' rect=' + (RectStr $f.els[0].Current.BoundingRectangle) + ' name=' + (Fmt $f.els[0].Current.Name) + ' id=' + (Fmt $f.els[0].Current.AutomationId))
                } else {
                    # T5K fail-closed：四条真臂全空手 = 这一发没成，不是"跳过"。
                    # 旧写法这里无条件打 INVOKED= ，pattern= 后面是空字符串 ⇒ 打印即绿。
                    # NOTINVOKED= 已在 $script:Dr4Problems 的名册里且在 critical 里，W() 会把这条
                    # 经 Test-Dr4FailureToken 记进累计器，结尾 RESULT=FAIL + exit 1。
                    W ('NOTINVOKED=' + $arg + ' how=' + $f.how + ' tried=' + $r.tried + ' why=' + $r.why +
                       ' n=' + $f.els.Count + ' rect=' + (RectStr $f.els[0].Current.BoundingRectangle) +
                       ' name=' + (Fmt $f.els[0].Current.Name) + ' id=' + (Fmt $f.els[0].Current.AutomationId))
                }
                Start-Sleep -Milliseconds 900
            }
        }
        'set' {
            $k = $arg.IndexOf('=')
            if ($k -lt 0) { W 'SET-BADSPEC use set:<key>=<text>' }
            else {
                $key = $arg.Substring(0, $k); $text = $arg.Substring($k + 1)
                $f = Find-El $key
                if ($f.how -eq 'none') { W ('SET-NOTFOUND=' + $key) }
                else {
                    $e = $f.els[0]
                    try {
                        $p = $e.GetCurrentPattern([System.Windows.Automation.ValuePatternIdentifiers]::Pattern)
                        if ($null -ne $p) { [void]([System.Windows.Automation.ValuePattern]$p).SetValue($text); W ('SET-VIA=ValuePattern how=' + $f.how) }
                    } catch { W ('SET-VALUE-THREW=' + (($_.Exception.Message -replace '\s+', ' '))) }
                    Start-Sleep -Milliseconds 600
                }
            }
        }
        'val' { Read-Val $arg }
        'type' {
            $k = $arg.IndexOf('=')
            if ($k -lt 0) { W 'TYPE-BADSPEC use type:<key>=<text>' } else { Send-Text $arg.Substring(0, $k) $arg.Substring($k + 1) }
        }
        'keyin' { Send-Key $arg }
        'hover' { Do-Hover $arg }
        'click' { Do-Click $arg }
        'waitlog' { Wait-Log $arg $WaitSec | Out-Null }
        'loggrep' {
            $ls = Get-Dr4LogLines
            if ($null -eq $ls) {
                [void](Mark-Dr4Problem 'LOGGREP-NOLOG=' ($logFile + ' - grepping a log that is not there proves nothing about absence'))
            } else {
                # 字面 Ordinal 子串，与 waitlog 同口径（[SCROLL] 不再被当字符类）。
                $hits = @($ls | Where-Object { $_.IndexOf($arg, [System.StringComparison]::Ordinal) -ge 0 })
                Show-Dr4LogHits $hits 'literal' $arg
            }
        }
        'logre' {
            $ls = Get-Dr4LogLines
            if ($null -eq $ls) {
                [void](Mark-Dr4Problem 'LOGGREP-NOLOG=' ($logFile + ' - grepping a log that is not there proves nothing about absence'))
            } else {
                $re = $null
                try { $re = New-Object System.Text.RegularExpressions.Regex $arg } catch {
                    [void](Mark-Dr4Problem 'LOGRE-BADREGEX=' ($arg + ' does not compile: ' + (($_.Exception.Message -replace '\s+', ' '))))
                }
                if ($null -ne $re) {
                    if ($arg.StartsWith('^')) {
                        [void](Mark-Dr4Problem 'LOGRE-LINEANCHOR=' ($arg + ' anchors at line start, and every DIAG line really starts "DIAG: DIAG: " in this fork => guaranteed false FAIL. Match the inner substring instead.'))
                    }
                    Show-Dr4LogHits @($ls | Where-Object { $re.IsMatch($_) }) 'regex' $arg
                }
            }
        }
        'sleep' { Start-Sleep -Milliseconds ([int]$arg) }
        'raise' { Ensure-Foreground 'manual' | Out-Null }
        'close' { Close-App }
        default { W ('STEP-UNKNOWN=' + $s) }
    }
    } catch { W ('STEP-THREW=' + $s + ' ' + (($_.Exception.Message -replace '\s+', ' '))) }
}

# ---------------------------------------------------------------- teardown
$stillAlive = $false
try { $stillAlive = -not $proc.HasExited } catch { }
if ($stillAlive) { Close-App }
Start-Sleep -Milliseconds 800
$kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($proc.Id)" -ErrorAction SilentlyContinue)
foreach ($k in $kids) { W ('ORPHAN-CHILD=' + $k.Name + ' pid=' + $k.ProcessId) }
if ($kids.Count -gt 0) {
    [void](Mark-Dr4Problem 'ORPHAN-CHILD=' ($kids.Count + ' child(ren) survived the close: ' + ((@($kids | ForEach-Object { [string]$_.Name + '@' + [string]$_.ProcessId })) -join ' ')))
}
$leftRs = @(Get-Process blade2-rs -ErrorAction SilentlyContinue)
$leftFake = @(Get-Process fake_dsh -ErrorAction SilentlyContinue)
W ('LEFTOVER blade2-rs=' + $leftRs.Count + ' fake_dsh=' + $leftFake.Count)
if (($leftRs.Count + $leftFake.Count) -gt 0) {
    [void](Mark-Dr4Problem 'LEFTOVER-NOT-CLOSED=' ('blade2-rs=' + $leftRs.Count + ' fake_dsh=' + $leftFake.Count + ' - still running after Close-App, so the next round would start on a dirty machine'))
}
try {
    if (-not $stdoutJob.IsCompleted) { [void]$stdoutJob.Wait(3000) }
    $so = $stdoutJob.Result
    Set-Content -LiteralPath "$LogPrefix.stdout.txt" -Value $so -Encoding UTF8
    W ('STDOUT-SAVED=' + "$LogPrefix.stdout.txt" + ' chars=' + $so.Length)
} catch { W ('STDOUT-SAVE-THREW=' + $_.Exception.GetType().Name) }

# ---------------------------------------------------------------- verdict (T3K 洞 #1)
# 这一把闸是本轮的立命之处：以前这里无条件 `RESULT=ok` + `exit 0`，超时和 NOTFOUND 都不改码。
# 母本同款硬停 = shot_harness.ps1 的 `if ($abiFails -gt 0) { … SELFTEST-RESULT=FAIL … exit 1 }`，
# 区别只在计数来源：那边问 marshaler，这边问**这一轮真正打印过的失败证据行**。
$procKinds = @($script:Dr4Problems | ForEach-Object { [string]$_.Kind } | Sort-Object -Unique)
W ('PROBLEMS=' + $script:Dr4Problems.Count + ' tolerated=' + $script:Dr4Tolerated.Count +
   ' kernel=' + $script:Dr4KernelMode + ' kinds=[' + ($procKinds -join ' ') + ']')
W ('VERDICT-CALIBER RESULT=ok here only means "this driver saw no failure line and no bad frame"; ' +
   'it never means the fork is correct, and it is silent about anything this round did not step on')
$rc = 0
if ($script:Dr4Problems.Count -gt 0) { $rc = 1 }
if ($rc -gt 0) {
    W ('RESULT=FAIL problems=' + $script:Dr4Problems.Count + ' (each [PROBLEM n] line above is one step that did not succeed)')
    exit $rc
}
W ('RESULT=ok problems=0 tolerated=' + $script:Dr4Tolerated.Count)
exit 0
