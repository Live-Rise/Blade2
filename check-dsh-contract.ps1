# Blade2 与 dsh 内核契约检查工具
# 用途：dsh 内核升级后（npm install -g @deepseek-ai/dsh@xxx），一跑即知壳的全部契约是否断裂。
#
# 壳与内核的接触面（全部经过实测梳理，5 类契约，零文件依赖）：
#   1. 进程契约：cmd /c "%APPDATA%\npm\dsh.cmd" web --no-open --port 0（进程解析器在壳内 ResolveDshLauncher）
#   2. CLI 契约：--no-open / --port 0 必须仍被 web 应用接受
#   3. URL 契约：stdout 必须仍打印 "dsh web: http://127.0.0.1:<port>/?token=..."（壳的 UrlPattern 正则捕获）
#   4. 启动等待契约：stdout URL 在插件树就绪后打印（超时 90s 兜底）
#   5. DOM 契约（壳的桥接脚本与主题注入，升级时最易断）：
#      - 会话列表: [role="treeitem"] + 内部 [class*="title"] / [class*="time"]（dsh-client-ui-workspace 插件）
#      - 侧边栏切换: button[aria-label="打开侧边栏"/"收起侧边栏"]（dsh-client-ui-sidebar 插件 locale key toggle.open/toggle.collapse）
#      - 新建会话: button aria-label 含 "新建会话"（sidebar 插件 session.new.label；workspace 插件 actions.newSession.aria 是 "在"{name}"中新建会话" 模板）
#      - 设置 modal: [class*="VOzbGW_overlay"] / [class*="VOzbGW_close"]（dsh-client-ui-settings-general 插件）
#      - 布局: [class*="sidebarCol"]、[class*="_frame"]、[class*="_centerCol"]、[class*="_rightbarCol"]、[data-slot="sidebar"]（dsh-client-ui-layout 插件）
#      - 主题: body[data-ds-dark-theme] + --dsw-alias-* token 体系（dsh-client-ui-theme 插件）
#      - 侧栏 0 宽裁剪依赖 modal 渲染在 sidebarCol 子树内（display:none 会藏掉设置 modal）
#
# 架构事实（0.1.5-rc.2 实测确认）：页面 DOM 不来自 dsh-web-frontend 静态 bundle，而是服务端
# 把 ~162 个 dsh-client-* 插件的 client.js 注入 HTML（/plugins/??.../client.js）。静态产物里
# treeitem/VOzbGW/sidebarCol 全部为 0 是正常的——契约的真实载体是插件包，本工具因此以运行时
# DOM 断言为准，静态包扫描仅作参考。
#
# 用法：
#   pwsh -File check-dsh-contract.ps1              # 完整检查（CLI + URL + DOM）
#   pwsh -File check-dsh-contract.ps1 -CliOnly      # 只查 CLI/URL 契约（不开浏览器，最快）
#   pwsh -File check-dsh-contract.ps1 -KeepAlive    # 诊断用：检查完不杀 dsh 实例
param(
    [switch]$CliOnly,
    [switch]$KeepAlive
)

$ErrorActionPreference = 'Stop'
$fail = 0
$warn = 0
function Pass([string]$msg) { Write-Host "  [PASS] $msg" -ForegroundColor Green }
function Fail([string]$msg) { Write-Host "  [FAIL] $msg" -ForegroundColor Red; $script:fail++ }
function Warn([string]$msg) { Write-Host "  [WARN] $msg" -ForegroundColor Yellow; $script:warn++ }
function Info([string]$msg) { Write-Host "  [..]   $msg" -ForegroundColor DarkGray }

# 安全清理：只杀"本脚本自己启动的进程"（按父进程链递归），绝不按命令行模式匹配杀进程
# ——用户可能同时开着壳（Blade2）或独立 dsh web 实例，模式匹配会误杀。
function Stop-ChildTree([int]$parentId) {
    $kids = Get-CimInstance -ClassName Win32_Process -ErrorAction SilentlyContinue |
        Where-Object { $_.ParentProcessId -eq $parentId }
    foreach ($k in $kids) {
        Stop-ChildTree $k.ProcessId
        Stop-Process -Id $k.ProcessId -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "`n=== Blade2 与 dsh 内核契约检查 ===" -ForegroundColor Cyan

# ---------- 0. 内核版本 ----------
$dshCmd = Join-Path $env:APPDATA 'npm\dsh.cmd'
if (-not (Test-Path $dshCmd)) { Write-Host "[ABORT] 未找到 $dshCmd — npm 全局未装 dsh" -ForegroundColor Red; exit 1 }
$dshVer = (& cmd /c "`"$dshCmd`" --version" 2>$null | Select-Object -First 1)
Info "dsh 内核版本: $dshVer"

# ---------- 1+2+3+4. CLI/URL 契约：真实启动一次 ----------
Write-Host "`n--- 1/5 进程与 CLI 契约：启动 dsh web --no-open --port 0 ---"
$psi = [System.Diagnostics.ProcessStartInfo]::new()
$psi.FileName = "$env:ComSpec"
$psi.Arguments = "/c `"`"$dshCmd`" web --no-open --port 0`""
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true
$p = [System.Diagnostics.Process]::Start($psi)
$url = $null
$sw = [System.Diagnostics.Stopwatch]::StartNew()
while ($sw.Elapsed.TotalSeconds -lt 90 -and -not $url) {
    $line = $p.StandardOutput.ReadLine()  # 挂起直到下一行或 EOF
    if ($null -eq $line) { break }
    if ($line -match '^dsh web: (http\S+)') { $url = $Matches[1] }
}
if ($url) { Pass "stdout URL 契约: 'dsh web: <url>' 保留（$([math]::Round($sw.Elapsed.TotalSeconds,1))s 内打印）" }
else { Fail "stdout URL 契约断裂：90s 内未见 'dsh web: http…' 输出 — 壳将永远停在加载层或回退 3080" }

if ($url -and $url -match '^http://127\.0\.0\.1:(\d+)/\?token=') {
    Pass "URL 形态契约: 127.0.0.1 + token 查询参数（壳 WebView 直接导航该 URL）"
} elseif ($url) {
    Fail "URL 形态变化: $url — 检查 token 换取机制是否仍走 ?token= 查询参数"
}

if ($url) {
    try {
        $r = Invoke-WebRequest $url -UseBasicParsing -TimeoutSec 10 -SkipHttpErrorCheck
        if ($r.StatusCode -eq 200) { Pass "带 token 访问根路径 => 200（页面可加载）" }
        else { Fail "带 token 访问根路径 => $($r.StatusCode)" }
    } catch { Fail "带 token 访问异常: $($_.Exception.Message)" }
}

if (-not $url) {
    Write-Host "`nURL 未捕获，跳过 DOM 检查。" -ForegroundColor Yellow
    if (-not $KeepAlive) { try { if (-not $p.HasExited) { Stop-ChildTree $p.Id; Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue } } catch {} }
    exit 1
}

if ($CliOnly) {
    Write-Host "`nCliOnly 模式：跳过 DOM 检查。" -ForegroundColor Yellow
    if (-not $KeepAlive) { try { if (-not $p.HasExited) { Stop-ChildTree $p.Id; Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue } } catch {} }
    Write-Host "`n结果: $fail FAIL / $warn WARN" -ForegroundColor $(if ($fail) { 'Red' } else { 'Green' })
    exit $(if ($fail) { 1 } else { 0 })
}

# ---------- 5. DOM 契约：headless Edge + CDP ----------
Write-Host "`n--- 5/5 DOM 契约：headless 浏览器加载页面 ---"
$port = 9323
$edge = @("$env:ProgramFiles (x86)\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
$probeDir = Join-Path $env:TEMP 'dsh-contract-probe'
$edgeProfile = Join-Path $probeDir 'edge-profile'
New-Item -ItemType Directory -Force $probeDir, $edgeProfile | Out-Null

# ws 库就位（与探针脚本同目录）
if (-not (Test-Path (Join-Path $probeDir 'node_modules\ws'))) {
    Push-Location $probeDir
    if (-not (Test-Path (Join-Path $probeDir 'package.json'))) { npm init -y *> $null }
    npm install ws --no-audit --no-fund --silent 2>$null
    Pop-Location
}

$edgeProc = Start-Process -FilePath $edge -ArgumentList @('--headless=new', "--remote-debugging-port=$port", "--user-data-dir=$edgeProfile", '--no-first-run', '--no-default-browser-check', '--window-size=1400,900', 'about:blank') -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 3
$wsUrl = $null
try {
    $targets = Invoke-RestMethod "http://127.0.0.1:$port/json" -TimeoutSec 5
    $blank = ($targets | Where-Object { $_.type -eq 'page' -and $_.url -eq 'about:blank' })[0]
    $wsUrl = $blank.webSocketDebuggerUrl
} catch { Fail "CDP 无法连接 headless Edge: $($_.Exception.Message)" }

$probeJs = Join-Path $probeDir 'contract-probe.mjs'
if ($wsUrl) {
    # 探针：加载页面 → 等 12s → 逐契约断言
    @'
import { writeFileSync } from "node:fs";
const wsUrl = process.argv[2];
const url = process.argv[3];
const outPath = process.argv[4];
const { WebSocket } = await import("ws");
const ws = new WebSocket(wsUrl, { maxPayload: 64 * 1024 * 1024 });
let id = 0;
const pending = new Map();
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const i = ++id;
  pending.set(i, { resolve, reject });
  ws.send(JSON.stringify({ id: i, method, params }));
});
ws.on("message", (raw) => {
  const m = JSON.parse(raw);
  if (m.id && pending.has(m.id)) {
    const p = pending.get(m.id); pending.delete(m.id);
    m.error ? p.reject(new Error(JSON.stringify(m.error))) : p.resolve(m.result);
  }
});
await new Promise((r) => ws.once("open", r));
await send("Page.enable");
await send("Page.navigate", { url });
await new Promise((r) => setTimeout(r, 12000));
const r = await send("Runtime.evaluate", { returnByValue: true, expression: `(() => {
  const q = (s) => document.querySelectorAll(s).length;
  const labels = [...new Set([...document.querySelectorAll("[aria-label]")].map(e => e.getAttribute("aria-label")))];
  const btn = (frag) => [...document.querySelectorAll("button")].filter(b => (b.getAttribute("aria-label") || "").includes(frag)).length;
  return {
    treeitem: q('[role="treeitem"]'),
    titleInTreeitem: q('[role="treeitem"] [class*="title"]'),
    timeInTreeitem: q('[role="treeitem"] [class*="time"]'),
    sidebarCol: q('[class*="sidebarCol"]'),
    frameClass: q('[class*="_frame"]'),
    centerCol: q('[class*="_centerCol"], [class*="centerCol"]'),
    rightbarCol: q('[class*="_rightbarCol"], [class*="rightbarCol"]'),
    dataSlotSidebar: q('[data-slot="sidebar"]'),
    vozbOverlay: q('[class*="VOzbGW_overlay"]'),
    vozbClose: q('[class*="VOzbGW_close"]'),
    btnOpenSidebar: btn("打开侧边栏"),
    btnCollapseSidebar: btn("收起侧边栏"),
    btnNewSession: btn("新建会话"),
    btnSettings: btn("设置"),
    darkAttr: document.body.hasAttribute("data-ds-dark-theme"),
    dswAliasCount: [...document.body.attributes].length && document.styleSheets.length,
    pluginLinks: [...document.querySelectorAll("script[src*='/plugins/'], link[href*='/plugins/']")].length,
    ariaSample: labels.slice(0, 30),
  };
})()` });
writeFileSync(outPath, JSON.stringify(r.result.value, null, 2));
console.log("DOM OK");
ws.close();
process.exit(0);
'@ | Set-Content $probeJs -Encoding UTF8

    $snapshotPath = Join-Path $probeDir 'dom-snapshot.json'
    Push-Location $probeDir
    $probeOut = node $probeJs $wsUrl $url $snapshotPath 2>&1
    Pop-Location
    if (Test-Path $snapshotPath) {
        $snap = Get-Content $snapshotPath -Raw | ConvertFrom-Json
        Pass "页面加载成功（服务端注入 $($snap.pluginLinks) 个插件 link）"

        # 桥接脚本契约（MainWindow.xaml.cs BridgeScript）
        if ($snap.treeitem -gt 0) { Pass "会话列表契约: [role=treeitem] x $($snap.treeitem)" }
        else { Fail "会话列表契约断裂: [role=treeitem] = 0 — 壳会话同步/搜索将失效（聊天不受影响）" }
        if ($snap.titleInTreeitem -gt 0) { Pass "会话标题契约: treeitem 内 [class*=title] 存在" }
        else { Fail "会话标题契约断裂: treeitem 内无 [class*=title] — 标题提取退化为整行文本" }
        if ($snap.timeInTreeitem -gt 0) { Pass "会话时间契约: [class*=time] 存在" }
        else { Warn "会话时间契约: treeitem 内无 [class*=time]（次要，时间列可能改名）" }
        if ($snap.btnNewSession -gt 0) { Pass "新建会话按钮契约: aria-label 含 '新建会话'（x $($snap.btnNewSession)）" }
        else { Fail "新建会话按钮断裂: aria-label 文案变了（找 $($snap.ariaSample -join ' | ') 里的新建按钮）— 壳的新建会话按钮失效" }
        if ($snap.btnOpenSidebar + $snap.btnCollapseSidebar -gt 0) { Pass "侧边栏切换契约: 打开/收起按钮任一存在 (open=$($snap.btnOpenSidebar) collapse=$($snap.btnCollapseSidebar))" }
        else { Fail "侧边栏切换断裂: 无 aria-label 含'打开/收起侧边栏'的按钮 — 桥接 ensureExpanded 失效，rail 态下会话列表可能读不到" }
        if ($snap.btnSettings -gt 0) { Pass "设置按钮契约: aria-label 含'设置'" }
        else { Fail "设置按钮断裂: aria-label 文案变了 — 壳的原生设置按钮失效" }

        # 设置 modal 契约（VOzbGW 类是 modal 生命周期标记）
        if ($snap.vozbOverlay -ge 0 -and $snap.vozbClose -ge 0) { Info "VOzbGW 类存在性: overlay=$($snap.vozbOverlay) close=$($snap.vozbClose)（overlay 隐藏态为 0 正常；打开态需 >0，此处仅静态存在性检查在源码层完成）" }

        # 布局契约（主题注入 CSS 的选择器目标 + 桥接 enforceLayout）
        if ($snap.sidebarCol -gt 0) { Pass "布局契约: [class*=sidebarCol] 存在" }
        else { Fail "布局断裂: 无 sidebarCol — 侧栏 0 宽裁剪 CSS 失效，dsh 自带 rail 会露出" }
        if ($snap.frameClass -gt 0 -and $snap.centerCol -gt 0) { Pass "布局契约: _frame / centerCol 存在（grid 轨道归零 + 圆角内容面有效）" }
        else { Fail "布局断裂: _frame=$($snap.frameClass) centerCol=$($snap.centerCol) — 内容面布局与圆角失效" }
        if ($snap.dataSlotSidebar -gt 0) { Pass "布局契约: [data-slot=sidebar] 存在（boot 等待探测有效）" }
        else { Fail "布局断裂: 无 [data-slot=sidebar] — 桥接 bootTimer 等待条件失效，侧栏展开逻辑不触发" }

        # 主题契约
        if ($snap.darkAttr -ne $null) { Info "body[data-ds-dark-theme] 属性存在性: $($snap.darkAttr)（壳的 BuildWinUIThemeScript 会主动设置它）" }
    } else {
        Fail "DOM 探针失败: $probeOut"
    }
}

# VOzbGW 源码层契约（modal 类名在 settings-general 插件源码中）
Write-Host "`n--- 5b/5 设置 modal 类名（源码层）---"
$sg = "C:\Users\Admin\.dsh\profiles\node_modules\@deepseek-ai\dsh-client-ui-settings-general\lib\client.js"
$sgAlt = "$env:APPDATA\npm\node_modules\@deepseek-ai\dsh\node_modules\@deepseek-ai\dsh-client-ui-settings-general\lib\client.js"
$sgSrc = if (Test-Path $sg) { $sg } elseif (Test-Path $sgAlt) { $sgAlt } else { $null }
if ($sgSrc) {
    $t = [IO.File]::ReadAllText($sgSrc)
    if ($t.Contains('VOzbGW_overlay') -and $t.Contains('VOzbGW_close')) { Pass "设置 modal 契约: VOzbGW_overlay/close 类仍在 settings-general 插件源码" }
    else { Fail "设置 modal 契约断裂: settings-general 插件中无 VOzbGW 类 — 壳的设置开关需按新类名适配（aria-haspopup=dialog 的 trigger 仍是定位锚点）" }
    if ($t.Contains('aria-haspopup')) { Pass "设置 trigger 契约: aria-haspopup=dialog 按钮仍在" } else { Fail "设置 trigger 契约断裂: 无 aria-haspopup 按钮" }
} else {
    Warn "settings-general 插件源码未找到（$sg | $sgAlt）— 跳过源码层检查"
}

# ---------- 清理 ----------
Write-Host "`n--- 清理探针进程 ---"
if (-not $KeepAlive) {
    try { if ($edgeProc -and -not $edgeProc.HasExited) { Stop-ChildTree $edgeProc.Id; Stop-Process -Id $edgeProc.Id -Force -ErrorAction SilentlyContinue } } catch {}
    try {
        if (-not $p.HasExited) { Stop-ChildTree $p.Id; Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
    } catch {}
    Pass "探针进程已清理（仅本脚本启动的实例）"
} else {
    Info "KeepAlive: dsh 实例保留（PID $($p.Id)），URL: $url"
}

Write-Host "`n=== 结果: $fail FAIL / $warn WARN ===" -ForegroundColor $(if ($fail) { 'Red' } else { 'Green' })
Write-Host "FAIL>0 时壳需要适配；会话类契约断裂只影响侧栏同步（聊天区不受影响）。"
exit $(if ($fail) { 1 } else { 0 })
