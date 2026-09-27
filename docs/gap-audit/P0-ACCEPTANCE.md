# P0 验收报告（T12）

> 基线：`docs/gap-audit/GAP-MATRIX.md` §6 P0 验收清单  
> 构建：`pwsh -File build.ps1 -Configuration Debug` → **0 error**（142 warning，均为预存 CS 可空/未用 + PRI 对 `Kernel/**/node_modules` 打包噪音，与 P0 无关）  
> 方法：整包 Debug 构建 + 静态代码/探针核对（UI 未启动，凡依赖运行时点选的项标「需人工点检」）

---

## 结论表

| 项 | 结论 | 用户可感知行为落地 | 证据 | 遗留 |
|---|---|---|---|---|
| **P0-1 文档预览** | **PASS**（静态） | 六形态：MD 渲染 / 代码高亮 / 图片 / PDF 翻页 / HTML 附属清单 / 纯文本 | `Pages/FilesPanel.Preview.cs:29-37`（`PreviewFace` 六枚举）、`:54-71`（`FaceOf` 按扩展名切面）、`:78-87` `workspaceFiles/readAll`、`:89-113` `readBytes` 分段、`:115-127` `readRelated`、`:286-310` `RenderCode`+`AppendHighlighted`、`:388` `RenderMarkdown`、`:725-762` PDF 翻页、`:799-822` HTML 附属；UIA 子节点按面切换 `Pages/FilesPanel.xaml:149` `PreviewRoot`、`:272` `PreviewMarkdown`、`:286` `PreviewImage`、`:293` `PreviewPdf`、`:241` `PreviewHtmlRelated`、`:259` `FilesPanelPreviewText` | 需人工点检：PDF 密码输入未做（加密 PDF 走「无法在壳内渲染」降级 + 用系统打开，`FilesPanel.Preview.cs:714-721`）；HTML 刻意不引入 WebView2，只读源码 + 附属清单（`FilesPanel.Preview.cs:5`） |
| **P0-2 预览辅助** | **PASS**（静态） | 换行切换 / 加载更多 / 重新读取 / 文件已更新横幅 / 复制全文·选中 | `FilesPanel.xaml:165-192` 工具条（`PreviewWrapToggle`/`PreviewCopyAll`/`PreviewCopySelection`/`PreviewReload`）、`:196-219` `PreviewChangedBanner`+`PreviewChangedReload`、`:344` `PreviewLoadMore`；`FilesPanel.xaml.cs:573-594` `stat` version 变化弹横幅不自动重载；`FilesPanel.Preview.cs:180-227` 分页加载、`:855` wrap 提示、`:926/:938` 复制 | 需人工点检：wrap 切换后行布局、长文件加载更多 |
| **P0-3 工具树** | **PASS**（静态） | bash 命令+退出码+信号+输出折叠；str-replace 差异展开；参数/结果 JSON 复制 | `MainWindow.ToolCards.cs:106` `BuildToolCard`、`:327-350` bash/pwsh 退出码/信号、`:523-566` `AppendDiffBody`（收起差异/展开其余 N 行）、`:729+` `CollectToolDiffs`（`meta.diffs` 优先）、`:201` `ToolCopyJson`、`:851-857` `ToolCopyResultJson`；UIA `ToolCard`/`ToolCardExitCode`/`ToolCardSignal`/`ToolCardExpand`/`ToolCopyJson`/`ToolCopyResultJson`；装配钩子 `MainWindow.MessageActions.cs:552-553` | 需人工点检：剪贴板写入、diff 展开交互 |
| **P0-4 多题导航** | **PASS**（静态） | 上一题/下一题/跳过本题/放弃整组/去聊天里说/推荐标记/确认执行 | `MainWindow.UserQuestions.cs:225-230` `QuestionAbandonAll`（`ASK_CANCELLED`）、`:259` `QuestionNavPrev`、`:284` `QuestionNavNext`、`:315` `QuestionSkip`、`:322` `QuestionToChat`、`:332-334` `QuestionSubmitButton`、`:439` 推荐标记（尾缀剥离 `:166-176`）、`:663-705` 汇总 `$events/result` answers 全题 id、`:719-780` 放弃整组 `rejected`+`ASK_CANCELLED` | 需人工点检：分页切换、未答不能靠下一题跳过门槛（`:595`） |
| **P0-5 完全权限确认** | **PASS**（静态） | 「确认启用完全权限？」+ 风险勾选后才可确认；取消不写回；不双弹 | `MainWindow.PermissionConfirm.cs:65-95` `RiskAckCheckBox` → `IsPrimaryButtonEnabled` 联动 + `ConfirmDangerFullAccessButton`；`:34-48` `_permissionConfirmShowing` 重入拒绝（防 ContentDialog 双弹）；`:95` 仅 `Primary && checked` 放行；`:166-177` 设置页取消回退且不 `Edit`；`MainWindow.xaml.cs:18051` `/permission danger-full-access` 取消则不发 `commands/execute`；`:16320` 空态默认权限同门闸。文案成对 `MainWindow.xaml.cs:621-627` | 需人工点检：勾选前确认钮禁用、取消后设置回退 |
| **P0-6 Cordis 审批/面板** | **PASS**（静态） | 审批卡允许/仅允许此版本/允许后续版本/拒绝；面板 run/stop | `MainWindow.Cordis.cs:380-387` 四按钮（UIA `CordisApproveAllow/Once/Future/Decline`）、`:475-534` 四 outcome → `runHostHalf`+`resolveRequestRun`；RPC `:326` `dynamicCordisRunner/resolveRequestRun`、`:339` `stopFromPanel`、`:333` `settleUserRun`、`:345` `undefineFromPanel`、`:352` `getClientCode`、`:183` `inventory`；面板 `:567-574` `ShowCordisPanelAsync` / UIA `CordisPanel`；入口 `MainWindow.Capabilities.cs:45` | 需人工点检：审批卡出现时机、run/stop 状态回写 |
| **P0-7 子代理** | **PASS**（静态） | 目录可展开；continuable 续发；父会话打断 | `MainWindow.Subagents.cs:61` `subagents/list`、`:122` `subagents/prompt`（mode=continuable, delivery queue/steer）、`:146` `subagents/interruptByParent`；UI `:259` `ShowSubagentCatalogAsync`、`:364` 目录行（hasChildren 下级）、`:588` 续跑对话框、`:633` 打断确认；会话头钮 `MainWindow.xaml:391-417` `SubagentCatalogButton/PromptButton/InterruptButton`，显隐 `MainWindow.Subagents.cs:162-196` | 需人工点检：parent-unavailable / not-resumable 错误路径 |
| **P0-8 轨迹面板** | **PASS**（静态） | 事件账本 + 计时总览（轮次/步骤/工具/缓存） | `MainWindow.Trajectory.cs:150-175` `TrajectoryObserve`（唯一钩子，seq 去重）、`:190+` `FoldTrajectoryTiming` 按轮分段（steps/toolMs/llm/ttft/tps/缓存）、`:655-791` `ShowCapabilityTrajectoryAsync`（UIA `TrajectoryPanel`/`TrajectoryTiming`/`TrajectoryLedger`）、`:670-678` 类型筛选、`:794` 轮次跳转、状态行「共 N 条事件 · M 轮」`:785-787`；入口 `MainWindow.Capabilities.cs:44` + 首事件自动挂菜单 `:617-651` | 需人工点检：事件条数 ≈ journal 条数（受 4000 条上限 `:168-171` 与 8 会话 LRU `:110-116` 约束） |

**汇总：8 PASS / 0 FAIL / 0 PARTIAL**（静态口径；UI 交互均需人工点检）

---

## 重点回归风险

| 风险 | 结论 | 证据 |
|---|---|---|
| ShellEnglish / L() 成对 | **已修**。P0-1/2 预览工具条与横幅约 25 条中文键原先缺失英文对（`自动换行`/`取消换行`/`加载更多`/`重新读取`/`复制全文`/`复制选中`/`文件已更新…`/`重新载入`/PDF 页码/HTML 附属/图像解码等）；已补入 `ShellEnglish`。XAML 硬编码横幅改为 `ChangedBannerText`/`ChangedReloadButton` 并进 `ApplyLocale`。P0-3/4/5/7 键位此前已成对（`:1002-1029`、`:741-754`、`:621-627`、`:1755-1784`）。P0-6/8 用 `CordisText`/`TrajText` 域内 en 兜底（有意不进 ShellEnglish 表） | `MainWindow.xaml.cs:1465-1497`（本次补）、`Pages/FilesPanel.xaml.cs:775-805`、`Pages/FilesPanel.xaml:210-219` |
| RenderEventCore 钩子不破坏原气泡 | **通过**。`TrajectoryObserve` 在 track 三兄弟之后旁路折叠，整函数 try/catch 吞异常且不改 bubble；`NoteToolResult` 只写工具卡旁路状态（ResultJson/MetaJson GetRawText 拷贝），随后 `IsToolRunning=false` + `RepaintBubble` 重建同一 `ChatBubble`，不触碰文本/图片气泡 | `MainWindow.xaml.cs:5656-5659`、`:6015-6022`；`MainWindow.ToolCards.cs:58-74`；`MainWindow.Trajectory.cs:150-175` |
| 权限门闸不双弹、取消不写回 | **通过**。`_permissionConfirmShowing` 重入直接 false；取消/异常一律 false；设置页取消回退 `lastAccepted` 且不登记 `Edit`；`/permission` 与空态默认同门闸，确认前不发 `commands/execute` | `MainWindow.PermissionConfirm.cs:34-48,95-104,166-177`；`MainWindow.xaml.cs:16320,18051` |
| FilesPanel 旧 PreviewText 路径 | **已清理**。旧「只调 `workspaceFiles/read` 文本段」路径由 `ShowPreviewAsync` 六形态分派取代；`FilesPanelPreviewText` 仅为文本面 RichEditBox 的 UIA id（与 `PreviewMarkdown`/`PreviewImage`/`PreviewPdf` 平级），无独立旧加载函数残留 | `Pages/FilesPanel.xaml.cs:511-571`；`Pages/FilesPanel.Preview.cs:178-246` |
| `rust/` 与 Kernel node_modules 预存 WIP | **未触碰**。本次仅改 `MainWindow.xaml.cs`（ShellEnglish）、`Pages/FilesPanel.xaml(.cs)`（横幅命名+ApplyLocale）；`rust/`、`Kernel/**` 零改动 | `git status` 变更集 |

---

## 本次集成修复

1. **ShellEnglish 缺键**（P0-1/2）：补约 25 条中文→英文（预览工具条、换行、加载更多、重新读取、复制全文/选中、文件已更新横幅、重新载入、PDF 页码/系统打开、HTML 附属、图像解码失败、空文件等）。`MainWindow.xaml.cs:1465-1497`
2. **XAML 硬编码横幅**（P0-2）：`文件已更新，当前显示为旧内容。` / `重新载入` 命名为 `ChangedBannerText`/`ChangedReloadButton`，纳入 `ApplyLocale`。`Pages/FilesPanel.xaml:210-219`、`Pages/FilesPanel.xaml.cs:796-799`

无重复方法、缺 using、XAML 名不匹配等多文件冲突（整包 0 error 一次通过；二次编译复核仍 0 error）。

---

## 遗留（不硬拗）

1. **PDF 密码输入**：加密/损坏 PDF 走降级占位 +「用系统打开」，无密码对话框（`Windows.Data.Pdf` 无解锁 API）。验收探针「翻页」已满足。
2. **HTML 专用预览**：刻意无 WebView2（项目未引用），为源码高亮 + `readRelated` 附属清单，非官方 DOM 预览。脚本执行安全边界有意为之。
3. **UI 人工点检清单**：六形态切换、wrap/加载更多/横幅、diff 展开与 JSON 复制剪贴板、多题分页与放弃整组回执、权限勾选门闸、Cordis 四态审批、子代理续跑/打断、轨迹账本条数 —— 均静态落地，需一次人工冒烟。
4. **警告**：142 条为预存 CS 可空/未用 + PRI `Kernel/**/node_modules` 打包噪音，非本任务引入。

---

## 宣布口径

**可以宣布「P0 对齐完成」（静态验收 8/8 PASS）**，前提是接受「UI 冒烟待人工点检 + PDF 密码/HTML WebView2 为有意降级」两条遗留。
