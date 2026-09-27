# T36 内核会话驱动深度 UI 冒烟（UIA）

> 方法：PowerShell + `System.Windows.Automation`，AutomationId 定位，禁止坐标/键鼠。
> 探针：`docs/gap-audit/uia-deep.ps1`、`uia-focused.ps1`。日志：`uia-deep-log.txt`。
> 形态：`build.ps1 -Configuration Debug` 后直跑未打包 `Blade2.exe`。数据家 `%LOCALAPPDATA%\Blade2`（凭据已在）。
> **口径**：运行时 UIA 硬点到 = PASS；壳/入口在但内容物/对话框未挂出 = PARTIAL（附原因）；点不到且无旁证 = FAIL。
> 本轮内核 **冷启动 ready**（KernelBootPanel 撤下），并真实跑了 bash/write 工具流。

---

## 硬 PASS（运行时 UIA）

| 项 | 证据 |
|---|---|
| **内核引导就绪** | `KERNEL-BOOT state=ready`（此前常停加载层） |
| **工具卡** | 真会话流：`ToolCard` / `ToolCardExpand`（Toggle 展开 OK）/ `ToolCopyJson` / `ToolCardExitCode`（退出码 0）/ `ToolCardStatus`（执行中）/ `ToolCardWorkdir` / `ToolCardPath` / `ToolCardDiffStat` / `ToolCardResultSummary` |
| **预览 · 文本** | `sample.cs`/`sample.txt` → `FilesPanelPreviewText`（正文命中）+ `PreviewWrapToggle` + `PreviewCopyAll` |
| **预览 · Markdown** | `sample.md` → `PreviewCopyAll` + `FilesPanelPreviewMeta`（65 B · 6 行）+ `MarkdownScroll` |
| **预览 · HTML** | `sample.html` → `PreviewHtmlOutline`（DOM 预览 · 标题：fixture，脚本已禁用） |
| **预览 · 图** | `sample.png` → `PreviewImage` |
| **预览 · PDF 壳** | `sample.pdf` → `PreviewPdfPageLabel`（无法在壳内渲染）+ `PreviewPdfOpen` + `PreviewPdfCopyPath`（降级面完整） |
| **模型两档说明** | Model 菜单 HelpText 命中 Flash/Pro 全文 + `EffortTierBlurb_*` |
| **模型记忆 lastUsed** | `ModelOption_LastUsed`（上次使用：step-5-preview）出现 |
| **内测声明** | 名称命中「内测声明」 |
| **子代理目录** | `SubagentList` 存在可点 |
| **壳底座** | ModelButton / NewSessionItem / InputBox / SendButton / ChatList / SettingsItem / PermissionButton / ComposerAddButton / LayoutSplitterSidebar / FilesPanelTree / RightPaneTabStrip / RightPaneCell0 / FileActionMenu |
| **能力菜单入口** | `CordisMenuItem` / `TrajectoryMenuItem` / `SkillsMenuItem` / `SchedulesMenuItem` / `TodosMenuItem` 均挂出（本轮修复前 Cordis 不可达） |

## PARTIAL（原因）

| 项 | 原因 |
|---|---|
| 多题 ask_user | 发送真实 prompt 后 90s 内无 `user-questions/request` waterfall → `QuestionPanel` 等未实例化 |
| 轨迹面板 | `TrajectoryMenuItem` Invoke OK，但 `TrajectoryPanel`/`Timing`/`Ledger` 未进 UIA 树（ContentDialog 对 UIA 不暴露或 ShowAsync 未完成）；数据链 `TrajectoryObserve` 静态闭环 |
| Cordis 面板 | 同上：`CordisMenuItem` 入口 PASS，`CordisPanel` 未挂出；无 dynamicCordis 请求 |
| 权限门闸 | `PermissionButton` 菜单四预设挂出；`RiskAckCheckBox` 本轮未出现（`RadioMenuFlyoutItem` UIA 仅 TogglePattern，Toggle 不触发 Click→确认框）。T27 曾硬 PASS，本轮 **复核未仍绿** |
| 子代理续跑/打断 | `SubagentList` PASS；`SubagentPrompt`/`Interrupt` 无子代理条目不挂出 |
| 恢复默认模型 | `RestoreDefaultModelButton` 在设置模型页；本轮设置分区 UIA 未扫到（`SettingsHost` 未暴露） |
| 预览根容器 id | `PreviewRoot`/`PreviewToolbar` 的 AutomationId 在 peer 上丢失（Grid/StackPanel）；六形态内容面/工具条按钮仍硬 PASS |
| 右栏分栏第二格 | `RightPaneCell1` 需手动分栏，本轮未点 |

**探针侧**：`PreviewRoot` 等 Panel 类 AutomationId 不进 UIA peer；`MenuFlyoutItem` 对 UIA 常无 InvokePattern（Toggle≠Click）。

## 本轮修复（真 bug）

1. **`InitRightPaneDock` 启动崩溃**：把仍在 `RightPaneHost` 里的 `FilesPanelView` 直接塞进 `ContentControl.Content` → WinUI `ArgumentException: Value does not fall within the expected range`（`blade2_unhandled.txt`）。先 `RightPaneHost.Children.Clear()` 再挂 Content。
2. **WebView2 1.0.2903.40 起不来**：`XamlTypeInfo.InitTypeTables` → `TypeLoadException: GetVirtualMethodTableInfoForKey`（CsWinRT 投影）。钉 **1.0.2651.64**（WinAppSDK 1.6 基线，NU1605 禁止更低）。
3. **Cordis/技能/计划运行时不可达**：`BuildCapabilityMenu` 从未挂接到任何按钮。补 `EnsureCapabilityUiEntries()` → ComposerAddFlyout（`CordisMenuItem`/`SkillsMenuItem`/`SchedulesMenuItem`）。
4. **`SyncCordisInspectManifestAsync([])`** 编译不过（collection expression→object）→ `Array.Empty<object>()`。
5. **菜单→ContentDialog 静默失败**：MenuFlyout 关闭动画期间 `ShowAsync` 会丢；Click 改 `DispatcherQueue.TryEnqueue` 再开面板。

## 构建与收尾

`pwsh -File build.ps1 -Configuration Debug` → **0 error**（预存 CS 警告 + PRI 资源限定符警告）。自启 Blade2 已 `Stop-Process`。

*日志：`docs/gap-audit/uia-deep-log.txt`；脚本 `uia-deep.ps1` / `uia-focused.ps1`。*
