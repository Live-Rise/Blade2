# T27 UI 冒烟点检报告（UIA）

> 方法：PowerShell + `System.Windows.Automation`（AutomationId 定位，禁止坐标/键鼠注入）  
> 形态：`pwsh -File build.ps1 -Configuration Debug` 后直接跑 `bin/x64/Debug/.../Blade2.exe`（未打包）  
> 探针脚本：`docs/gap-audit/uia-smoke.ps1`、`uia-nav.ps1`、`uia-final.ps1`（只读点检，不进 `rust/`）  
> 环境：console 会话可跑 GUI；内核冷启动不稳定（有时停加载层/`Value does not fall within the expected range`，有时进到工作区与会话）。  
> **口径**：运行时 UIA 点到 = PASS；壳/契约在但依赖内核内容物未触发 = PARTIAL（附静态证据）；完全点不到且无静态闭环 = FAIL。

---

## 汇总

| # | 点检项 | 结论 | 运行时证据 | 静态/修复证据 |
|---|---|---|---|---|
| 1 | 文件预览六形态 + 换行/加载更多/复制/已更新横幅 | **PARTIAL** | `FilesPanelTree` PASS（修复后）；`FileActionMenu` PASS；`PreviewRoot` 等仅在选中文件后创建，无内核读文件时 UIA 不出现 | 六形态 `FilesPanel.Preview.cs:29-71,137-157`；工具条/横幅/加载更多 `FilesPanel.xaml:161-358`；无 RPC 时改为可见错误面（本轮修复） |
| 2 | 工具卡 expander / bash 退出码·信号 / diff / 复制 JSON | **PARTIAL** | 无工具消息流，`ToolCard*` 未实例化 | `MainWindow.ToolCards.cs:207,252,338-389,535-601,911-937`；`ParseExitStatus` `:1197-1211` |
| 3 | 多题导航（上一题/下一题/跳过/放弃整组/去聊天） | **PARTIAL** | 无 ask_user_question 载荷，`Question*` 未挂出 | `MainWindow.UserQuestions.cs:225-325`（五钮 AutomationId + IsEnabled 门槛 `:564-565`） |
| 4 | 权限门闸：勾选前确认禁用；取消不写回 | **PASS** | `RiskAckCheckBox` PASS；`ConfirmDangerFullAccessButton` 初始 `en=False`；勾选后 `en=True`；取消后对话框关闭且 `PermissionButton` 仍为「自动审批」（未写回） | `MainWindow.PermissionConfirm.cs:64-95,166-177` |
| 5 | Cordis 审批四态 / 面板 | **PARTIAL** | 无 dynamicCordis 请求；`CordisPanel`/`CordisApprovalCard`/`CordisRunStop` 未挂出 | 四按钮 `MainWindow.Cordis.cs:380-387`；四 outcome `:475-534`；面板 `:567-674` |
| 6 | 子代理目录/续跑/打断 | **PARTIAL** | `SubagentList` PASS（有会话头时可达）；`SubagentPrompt`/`SubagentInterrupt` 按目录 mode 显隐，无子代理条目时未挂出 | `MainWindow.Subagents.cs:159-196,258-268,457-482,588-659` |
| 7 | 轨迹面板：轮次分段 + 账本行 | **PARTIAL** | 无 journal 事件，`TrajectoryPanel` 未打开 | `MainWindow.Trajectory.cs:190+,359-367,495,655-725`（`TrajectoryTiming`/`TrajectoryLedger`） |
| 8 | 布局：拖拽把手；右栏两格上限 | **PARTIAL** | `LayoutSplitterSidebar`/`LayoutSplitterRight` **PASS**（修复后启动即在）；`FilesPanelTree` PASS；`RightPaneTabStrip`/`RightPaneCell*` 在 dock 重建后仍偶发不进 UIA 树 | 把手 `MainWindow.LayoutColumns.cs:100-130`；两格上限 `:613-618` + 菜单文案「分栏已满（最多两格）」`:495-508` |
| 9 | P2 模型两档说明 / 上次使用 / 恢复默认 / About 内测 | **PARTIAL** | **P2-2 PASS**：模型菜单 HelpText 命中 Flash/Pro 两档全文 + `EffortTierBlurb_*`；**P2-6 文案命中**（欢迎流「内测声明」）；`ModelOption_LastUsed` 无 lastUsed 投影未出现；`RestoreDefaultModelButton` 随设置模型页，本轮设置页 UIA 打开不稳定 | `MainWindow.ModelExtras.cs:24-65,107-145,149-206,210-243` |

**通过率（运行时硬 PASS）**：1/9 全项 PASS（P0-5）；P2-2 子项 PASS；布局把手/FilesPanelTree 壳级 PASS。其余 **PARTIAL**（实现齐、内容物依赖内核/会话流，未能运行时点全）。

---

## 本轮修复（真实问题）

1. **设置页离线点不开**：`ShowSettingsAsync` 在 `_rpc is null` 时直接 return → About/内测声明/恢复默认模型/本地设置全不可达。改为允许进入，RPC 分区自行兜底。  
2. **布局壳挂在内核成功路径**：`InitLayoutColumns` 原在 `RunKernelBootCoreAsync` 成功后才跑 → 内核失败时无拖拽把手。改为 `PostUi(InitLayoutColumns)`（构造中同步调会撞 `0xC000027B`）。  
3. **UIA 点不开设置**：`NavigationViewItem` 无 InvokePattern，Select 到不了 `ItemInvoked`。补 `SelectionChanged` + `SettingsItem.Tapped`（`ToggleSettingsPageFromUi` 400ms 防抖）。  
4. **`FilesPanelTree` AutomationId 丢失**：TreeView UIA peer 暴露成 `ListControl`。id 改挂外层 Grid → UIA `FilesPanelTree` PASS。  
5. **预览无 RPC 静默失败**：`ShowPreviewAsync` 无 `_rpc/_session` 时改为可见错误面。

## 未能运行时点全的项（原因）

- 工具卡 / 多题 / Cordis / 轨迹 / 预览六形态切换：需要内核会话流或 `workspaceFiles/*` 读文件；本机内核冷启动失败或只到加载层时控件树不实例化。  
- `ModelOption_LastUsed`：依赖 `modelSelection.lastUsed` 投影。  
- 右栏分栏两格拒绝：`RightPaneTabStrip` UIA 偶发不暴露，静态契约完整。

## 构建

`pwsh -File build.ps1 -Configuration Debug` → **0 error**（预存 CS 可空/未用警告）。收尾前已 `Stop-Process` 自启的 Blade2。

*探针日志：`docs/gap-audit/uia-smoke-log.txt`；脚本 `uia-smoke.ps1` / `uia-nav.ps1` / `uia-final.ps1`。*
