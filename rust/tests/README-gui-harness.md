# GUI 自测工具链（台账 #143 / #35 / #22）

## 为什么工具不能放 `rust/tmp/`
`.gitignore:11` 忽略了 `rust/tmp/`，Syncthing 会把整个目录清空（已被清过两次）。
放那里的脚本等于不存在，而且失败方式是**静默降级**：`gui_uia.ps1` 一旦取不到抓图 helper，
就退回 `SwitchToThisWindow + ShowWindow(5)` 的 legacy nudge，那玩意儿打不过 Windows 的前台锁，
于是 `CopyFromScreen` 拍到的是操作者当时正在看的窗口 —— 产出一张**看着正常、其实不是分叉**的图。

规则：**`rust/tests/` 放代码，`rust/tmp/` 只放产出**（截图、日志、报告）。
任何 `rust/tests/*.ps1` 都不许再 dot-source `rust/tmp/` 里的 helper。

## 四个脚本各管什么
| 文件 | 职责 | 公开入口 |
| --- | --- | --- |
| `shot_harness.ps1` | 抓图 + 前台 rung 阶梯。PrintWindow `flag=2`(`PW_RENDERFULLCONTENT`) 是**必需的**：WinUI 3 是 DirectComposition/DXGI 宿主，少了这一位拿回来的是全白帧（#35 的根因）。自带 `PER_MONITOR_AWARE_V2`（#22 的 DPI 坑）。 | `Invoke-ShotForeground -Hwnd -Tag -Log [-Capture]` / `Invoke-QaShot` / `Get-QaShotStats` / `Reset-ShotRound` / `Get-ShotNoteTail` |
| `gui_uia.ps1` | UIA 驱动器：按 **Name** 找控件、点、读状态，每步出 `RESULT=` 与 `FOREGROUND=` 证据行。`$PSScriptRoot\shot_harness.ps1` 是唯一 helper 加载路径（`gui_uia.ps1:196`）。 | 各 `-Scenario` 档 |
| `gui_probe.ps1` | 只探不做：列 hwnd / 矩形 / UIA 树，用来在驱动前先确认目标存在。 | — |
| `selftest_tools.ps1` | **离线契约检查**：不启动 GUI，静态核上面三张脚本有没有被改坏（helper 路径是否仍钉在 `$PSScriptRoot`、PrintWindow 那一位有没有丢、PMv2 有没有丢、公开函数名有没有被重命名、两张脚本是否还在同一目录）。改完任何 `.ps1` 先跑它。 | — |

## 判据口径（这些是我定下的，改脚本别把它们放宽）
- **禁止按像素盲点**。必须 UIA 按 `Name`/`AutomationId` 驱动 —— 曾经误点到用户自己的浏览器。
- 截图必须**互异且非单色**：`Get-QaShotStats` 出的 `md5/unique/mean/rms` 要逐张核；md5 相同 = 抓到同一帧 = 假证据。
- `FOREGROUND=` 行的 note 串以**空格开头**（`gui_uia.ps1` 把它直接拼在 `match=<bool>` 后面）。
- 虚线描边 = 真缺口，不是样式；不要拿它当"已经画了"。

## 前台许可
用户说"在忙/不要来前台"时，这一整条链**全部挂起**：不许置顶、不许 SendInput、不许抓图。
前台许可是**按轮**给的，上一轮给过不等于这一轮还能用。挂起期间只做静态/后台活。

## 仍未清的两笔账（要写 git，我无权动）
```
git add rust/tests/contextmeter_host.rs rust/tests/dock_chrome.rs rust/tests/mw2_deadbuttons.rs \
        rust/tests/rt6_turnrail.rs rust/tests/selftest_tools.ps1 rust/tests/sf1_shellfiles.rs \
        rust/tests/shellfiles_wiring.rs rust/tests/shot_harness.ps1 rust/tests/w4_job_diag.rs \
        rust/tests/mainline-snapshot.txt
```
以及 `.gitignore:2` 的 `bin/` 未锚定，会连带忽略 `rust/src/bin/fake_dsh.rs`（要改成 `/bin/`）。
在这两条落地之前，本目录里**一半的工具在 git 眼里不存在**，同步盘清一次就真没了。

## 编码
四张 `.ps1` 全部带 UTF-8 **BOM**（`efbbbf`）—— 没有 BOM 时 PowerShell 5 会按本地代码页读，注释里的中文一坏，整张脚本的字符串判据就跟着坏。新建 `.ps1` 记得补 BOM。
