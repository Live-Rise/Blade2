# 终版验收 FINAL-ACCEPTANCE（T38 · 终终版）

> 覆盖旧版（T21 88%「可宣布」作废；T29 双口径版以本文为准）。
> 证据链（最新在后）：`REAUDIT-2026-09.md` → `REMAINING-GAPS-CLOSE.md` → `UI-SMOKE.md` → `PARTIAL-CLOSE.md` → `UI-SMOKE-DEEP.md` + `uia-deep-log.txt` → 本次源码行号抽查。
> 构建：`pwsh -File build.ps1 -Configuration Debug` → **0 error / 0 warning**（T38 复核）。
> 对照面：WinUI 主干（**不含 `rust/`、`Kernel/**/node_modules`**）。契约源：`@deepseek-ai/*` 的 `typert.remote-client.js` / `client.js`。

---

## 1. 终终版一句话

**代码层：P0/P1 功能面与 RPC 载荷键已对齐 typert（158 条 ≈87%，缺口账 38/2/1）；运行时已证：内核 ready 后 UIA 硬 PASS 扩大到工具卡真流 + 预览 5 面 + 模型两档/lastUsed + 能力菜单入口 + 壳底座（45/88 探针）；仍降级：HTML 脚本关 ≠ 官方 allow-scripts DOM、加密 PDF 无 password API、Cordis Client 半边架构不可达——不得宣布「与官方完全一致 / 全绿 / 对齐完成」。**

| 层 | 状态 | 含义 |
|---|---|---|
| **代码层** | **成立（有保留）** | P0/P1 骨架、关键 RPC 载荷键、官方 4 插件卡字段面、Cordis 12/12 wire、HTML WebView2 路 A+路 B 均已落地；保留 HTML 脚本关、PDF 加密、Cordis Client-half |
| **运行时已证** | **显著扩大（非 1/9）** | UI-SMOKE-DEEP：内核 `ready` 后真会话流跑通 bash/write；硬证工具卡、预览文本/MD/HTML/图/PDF 壳、模型两档+lastUsed、内测声明、SubagentList、能力菜单五入口、壳底座。日志 `TOTAL=88 PASS=45` |
| **仍降级 / 仍 PARTIAL** | **明确列出** | ① HTML=WebView2 **脚本关**（`IsScriptEnabled=false`），≠ 官方 `sandbox="allow-scripts"` ② 加密 PDF 壳内不可解锁（`Windows.Data.Pdf` 无 password API）③ Cordis 8/12 有调用方 + 4 个 Client-half 专用（架构限制，`client-half-failed` 不可达）④ 多题无 waterfall、轨迹/Cordis Dialog 不进主窗 UIA 树、权限门本轮复核未绿、SubagentPrompt/Interrupt 无条目 |

---

## 2. 覆盖率三口径

公式沿用 GAP-MATRIX：覆盖率 =（对齐 + 0.5×部分）/ 分母；壳增强不进分母。

### 2.1 口径 A：官方 158 条 · 代码层

| 口径 | 对齐 | 部分 | 缺失 | 覆盖率 |
|---|---|---|---|---|
| 旧 FINAL（T21，作废） | 138 | 1 | 0 | ~~88%~~ |
| REAUDIT 纠偏 | ~132 | ~8 | 0 | ~84% |
| T29（T28 收口后） | ~134 | ~6 | 0 | ≈87% |
| **T38 终终版（T37 后）** | **~134** | **~6** | **0** | **≈87%** |

仍计「部分」约 6 条：**HTML 脚本关 DOM**（路 A 已落地但仍不等价）、**PDF 加密解锁**、**Cordis Client 半边**、**Cordis 4 调用方缺失**（wire 已 12/12，调用面 8/12）、**P2-5 匿名 ID**（有意 SKIP）、**插件卡保存模型**（壳防抖 vs 官方草稿，轻度）。

**88% 作废。降级不得进对齐分子。**

### 2.2 口径 B：P0+P1+P2 缺口账（41 条）

| 结论 | 条数 | 明细 |
|---|---|---|
| **PASS（代码层）** | **38** | P0-2/3/4/5/7/8；P1-1…27 全；P2-1/2/3/4/6 |
| **PARTIAL** | **2** | **P0-1**（HTML 脚本关 + PDF 加密）；**P0-6**（四 outcome 形状对；wire 12/12；调用方 8/12 + Client-half 近似） |
| **SKIP（有意）** | **1** | **P2-5** 匿名用户 ID |

代码层全 PASS 率 = 38/41 ≈ **93%**。

### 2.3 口径 C：运行时 UIA 硬证（UI-SMOKE-DEEP 口径，**禁止再写 1/9**）

| 范围 | 已证 | 说明 |
|---|---|---|
| **UI-SMOKE-DEEP 硬 PASS 类** | **13 类** | 内核 ready；工具卡真流；预览文本/MD/HTML/图/PDF 壳；模型两档；lastUsed；内测声明；SubagentList；壳底座；能力菜单五入口 |
| **探针计数** | **45 PASS / 88 TOTAL**（`uia-deep-log.txt` SUMMARY） | GROUP：tool 5/5、preview 18、shell 9、model 4、files 4、perm 2、settings 1、sub 1、boot 1 |
| 41 缺口账中运行时硬证 | **≥4**（P0-3 工具卡、P0-1 预览 5 面、P2-2 两档、P2-3 lastUsed）+ 壳级/入口级 | P0-5 权限门 **T27 曾硬 PASS，T36 复核未仍绿**（菜单 Toggle） |
| T27 对照 | ~~1/9~~ | **作废**。T36 内核 ready 后覆盖面扩大一个数量级 |

**结论：运行时已证从「薄」升为「主干路径硬证」，但仍不能支撑「功能对齐完成」**——多题 / Dialog 面板 / 权限门复核 / 子代理条目级仍未硬证。

---

## 3. 假 PASS 终态表（REAUDIT 6 条全部闭环）

| # | 旧假 PASS / 夸大 | **终态** | 闭环说明 |
|---|---|---|---|
| **F1** | P1-11 插件设置 = PASS（字段面不全） | **已修 → 关闭** | T28 落地官方 4 卡 1:1：`shell.timeoutMs/maxOutputBytes`、`agent-loop.maxParallelToolCalls`、`subagent-model-selection`、`web-search-deepseek` apiKey/baseURL/**maxUses**；overridden/reset 语义对齐。残留轻度：保存模型 = 壳 `Edit()` 防抖（非官方卡级草稿）。证据：`MainWindow.xaml.cs:12633-12765`；`MainWindow.SettingsExtras.cs:520-602` |
| **F2** | P0-1 HTML 六形态 = PASS（实为源码+附属） | **仍 PARTIAL（已如实降级）** | T37 路 A：WebView2 DOM（钉 **1.0.2651.64**）+ 路 B 兜底。`IsScriptEnabled=false` 等安全边界写死。**脚本关 ≠ 官方 `sandbox="allow-scripts"`** → 维持 PARTIAL，不再冒充 PASS。证据：`Pages/FilesPanel.Preview.cs:979-1013`；`Pages/FilesPanel.xaml:210,288`；`Blade2.csproj:44` |
| **F3** | 覆盖率 88%（138/1/0） | **已纠正 → 关闭** | 三口径见 §2：158 条 **≈87%**；缺口账 **38/2/1**；运行时 **45/88 探针 + 13 类硬 PASS**。旧 88% 与「1/9」均作废 |
| **F4** | 「静态一致 / 对齐完成 / P0+P1 全绿」 | **已纠正 → 关闭** | 只说「代码层功能面具备 + 主干运行时硬证」。2 项 PARTIAL + Dialog/多题未证，**不能**说全绿 |
| **F5** | 探针写「readAll 对 PDF/图」 | **已纠正（文档）→ 关闭** | 正确映射：图/PDF 走 `workspaceFiles/readBytes`；`readAll` 仅 CopyAll 未加载完路径；HTML 附属走 `readRelated`。证据：`FilesPanel.Preview.cs:89-130,1244`；`FilesPanel.xaml.cs:690-701` |
| **F6** | 「Cordis 12 RPC」易读成 12 全接 | **部分关闭 → 维持 PARTIAL** | T37：**12/12 wire 面已接**；**8/12 有桌面调用方**（inventory / runHostHalf / resolveRequestRun / settleUserRun / stopFromPanel / undefineFromPanel / getClientCode / syncInspectManifest）；**4/12 Client-half 专用无调用方**（invoke / resolveInspectQuery / reportClientGuardFailure / reportRenderFailure）。四 outcome 与 typert 一致；`client-half-failed` 不可达，以 `ok:true` 近似，**禁止伪造**。证据：`MainWindow.Cordis.cs:17-39,319-419,551-603` |

**清理结果：6/6 已闭环说明——F1/F3/F4/F5 关闭；F2/F6 如实维持 PARTIAL，不再假 PASS。**

---

## 4. P0 / P1 / P2 终态

### P0（8）

| # | 功能 | 终态 | 证据（当前行号） | 备注 |
|---|---|---|---|---|
| P0-1 | 六形态预览 | **PARTIAL** | `FilesPanel.Preview.cs:36-85`（枚举/FaceOf）`:89-130`（readAll/readBytes/readRelated）`:715` PDF `:849` HTML `:979-1013` WebView2 路 A `:1133,1160` 大纲/附属；分派 `FilesPanel.xaml.cs:660-701` | 5/6 真；**HTML=WebView2 路 A 脚本关** + 路 B 兜底，≠ 官方 DOM；PDF 加密无 password API |
| P0-2 | 预览辅助 | **PASS**（运行时子项已证） | `FilesPanel.xaml` 工具条/横幅；`Preview.cs:148-176`（ApplyPreviewFace）`:180+` 分页 | UIA：`PreviewWrapToggle`/`PreviewCopyAll`/`PreviewReload` 硬 PASS |
| P0-3 | 工具卡 bash/diff/JSON | **PASS**（**运行时硬证**） | `ToolCards.cs:106,300,338-388,536+,743,929-934`；`MessageActions.cs:592` | 真会话流：`ToolCard`/`Expand`/`CopyJson`/`ExitCode`/`Status`/`Workdir`/`Path`/`DiffStat`/`ResultSummary` |
| P0-4 | 多题导航 + ASK_CANCELLED | **PASS**（代码）/ **PARTIAL**（运行时） | `UserQuestions.cs:225-334,643-780` | 载荷与官方 `ASK_CANCELLED` 一致；运行时无 `user-questions/request` waterfall |
| P0-5 | 权限门闸 | **PASS**（代码）/ **复核 PARTIAL** | `PermissionConfirm.cs:64-95,166-177` | T27 硬 PASS；T36 菜单 `Toggle≠Click`，`RiskAckCheckBox` 未挂出 |
| P0-6 | Cordis 审批/面板 | **PARTIAL** | `Cordis.cs:17-39`（12/8/4）`:319-419`（RPC 面）`:456-459`（四钮）`:551-603`（四 outcome + ok:true 近似） | **wire 12/12；调用方 8/12；4 Client-half 架构限制** |
| P0-7 | subagents/* | **PASS**（代码）/ 目录运行时硬证 | `Subagents.cs:61,122-151`；UI `:268,459,482` | wire 三键逐键一致；`SubagentPrompt`/`Interrupt` 无条目不挂出 |
| P0-8 | 轨迹 journal | **PASS**（代码）/ Dialog PARTIAL | `Trajectory.cs:150,360`；`MainWindow.xaml.cs:5993` | 真消费 follow+page；`TrajectoryPanel` 不进主窗 UIA 树 |

### P1（27）

**PASS × 27**（代码层）。重点行号：P1-11 `MainWindow.xaml.cs:12633-12765` + `SettingsExtras.cs:520-602`；P1-10 `Skills.cs:267` + `ToolCards.cs:467-468`；P1-14 `Capabilities.cs` goals/clear；P1-16 `SessionState.cs:459`（会话级停，官方亦只读）；P1-25/26 `LayoutColumns.cs:372`（`RightPaneHost.Children.Clear()` 防崩）+ 两格上限。

### P2（6）

| # | 功能 | 终态 | 备注 |
|---|---|---|---|
| P2-1 | 轨迹 Goal 轮 | **PASS** | `Trajectory.cs:215,360` |
| P2-2 | 模型/推理两档 | **PASS**（**运行时硬证**） | `ModelExtras.cs:24-65`；HelpText 命中 Flash/Pro 全文 |
| P2-3 | lastUsed/next | **PASS**（**运行时硬证**） | `ModelExtras.cs:69-145`；`ModelOption_LastUsed` 出现 |
| P2-4 | 恢复默认模型 | **PASS**（代码）/ 设置页 PARTIAL | `ModelExtras.cs:150`；`RestoreDefaultModelButton` 本轮 `SettingsHost` 未暴露 |
| P2-5 | 匿名用户 ID | **SKIP（有意）** | 隐私口径 |
| P2-6 | 内测声明 | **PASS**（**运行时硬证**） | `ModelExtras.cs:213`；`About.cs:104`；名称命中 |

---

## 5. 两轮 UI 冒烟合并摘要

| 轮次 | 形态 | 结果 |
|---|---|---|
| **T27 `UI-SMOKE.md`** | 内核冷启动常失败 | 九大点检 **1/9 全项 PASS**（P0-5）；修 5 bug（设置离线、布局挂成功路径、NavigationView 无 Invoke、FilesPanelTree id、预览静默失败） |
| **T36 `UI-SMOKE-DEEP.md`** | 内核 **ready** + 真 bash/write 流 | **硬 PASS 扩大**：13 类 / 45 探针；修 5 bug（RightPaneDock Clear、WebView2 钉 1.0.2651.64、EnsureCapabilityUiEntries、Array.Empty、菜单 TryEnqueue） |

### 合并后硬 PASS（运行时 UIA）

内核 ready · 工具卡真流（9 个 id）· 预览文本/MD/HTML/图/PDF 壳 · 模型两档 + lastUsed · 内测声明 · SubagentList · 壳底座（ModelButton/NewSessionItem/InputBox/SendButton/ChatList/SettingsItem/PermissionButton/ComposerAddButton/LayoutSplitterSidebar/FilesPanelTree/RightPaneTabStrip/RightPaneCell0/FileActionMenu）· 能力菜单（Cordis/Trajectory/Skills/Schedules/Todos MenuItem）。

### 仍 PARTIAL 及根因（三类）

| 根因类 | 项 | 说明 |
|---|---|---|
| **waterfall 缺事件** | 多题 ask_user（P0-4 运行时） | 真实 prompt 后 90s 内无 `user-questions/request` → `QuestionPanel` 未实例化。**内容物/模型侧未触发，非壳缺口** |
| **Dialog UIA 树** | 轨迹面板、Cordis 面板（P0-8/P0-6 运行时） | `TrajectoryMenuItem`/`CordisMenuItem` Invoke OK，但 ContentDialog **不进主窗 UIA 树**（`ShowAsync` 未完成或 peer 不暴露）。数据链静态闭环 |
| **菜单 Toggle** | 权限门复核（P0-5） | `RadioMenuFlyoutItem` 仅 TogglePattern，**Toggle ≠ Click** → 确认框未弹、`RiskAckCheckBox` 未挂出。T27 曾硬 PASS，本轮未仍绿 |
| 其它 | SubagentPrompt/Interrupt；RestoreDefaultModelButton；PreviewRoot id；RightPaneCell1 | 无条目数据 / SettingsHost 未暴露 / Panel 类 peer 丢 id / 需手动分栏 |

**两轮合计 10 个真 bug 已修**（T27×5 + T36×5），均有源码落点。

---

## 6. 有意 / 架构降级最终清单

| # | 项 | 性质 | 说明 | 可逆？ |
|---|---|---|---|---|
| 1 | **HTML 脚本关** | 有意安全降级 | WebView2 路 A DOM 已落地，但 `IsScriptEnabled=false` + CSP `script-src 'none'` + 剥 script/外链；官方为 `sandbox="allow-scripts"`。路 B 源码+大纲+附属兜底保留 | 需脚本沙箱才可能 1:1 |
| 2 | **加密 PDF 不可壳内解锁** | 平台限制 | `Windows.Data.Pdf` 无 password/decrypt API；`LooksLikeEncryptedPdf` + 系统打开 + 复制路径 | 需第三方 PDF 库 |
| 3 | **Cordis Client 半边近似** | 架构限制 | 无浏览器 Client 运行时；`client-half-failed` 不可达；Host 起后 `ok:true` 结算 | 需嵌 JS 运行时 |
| 4 | **Cordis 4 方法无调用方** | 架构限制 | invoke / resolveInspectQuery / reportClientGuardFailure / reportRenderFailure = Client-half 专用；wire 已接，**禁止伪造调用** | 同上 |
| 5 | **P2-5 匿名用户 ID** | 有意跳过 | 隐私/设备指纹 | 任务口径不做 |
| 6 | **插件设置保存模型** | 壳交互差异 | 自动防抖写回 vs 官方卡级草稿 | 可改草稿制 |
| 7 | **插件设置折叠形态** | 壳交互差异 | 二级导航钻取 vs 官方展开/收起 | 语义等价 |
| 8 | **jobs 无 job 级停止** | 口径对齐 | 官方亦只读；WinUI 多给会话级 `session/cancel` | — |
| 9 | **Todo / 预设 yml / Schedules 只读** | 口径对齐 | 写路径 = 模型工具 | — |
| 10 | **Dialog / 多题运行时未硬证** | 验收边界 | 见 §5 三类根因 | 需人工点 Dialog / 造 ask_user |

---

## 7. 对外宣布口径（诚实版）

### 可以说

> Blade² WinUI 主干在**代码层**具备 dsh 官方 WebUI 的 P0/P1 功能面：文档预览（HTML 已走 WebView2 DOM，脚本关）、工具卡、多题审批、权限门闸、Cordis 审批/面板（12/12 wire）、子代理、轨迹账本、插件设置 4 卡、布局契约均已落地；关键 RPC 载荷键与官方 typert **逐键一致**。Debug 构建 **0 error**。内核 ready 后 UI 冒烟已硬证：**真会话工具卡流、预览 5 面、模型两档与 lastUsed、能力菜单入口**（45/88 UIA 探针）。

### 必须同时说

> **不是**「与官方 WebUI 完全一致」。仍保留：HTML **脚本关**（≠ 官方 allow-scripts）、加密 PDF 不可壳内解锁、Cordis Client 半边近似（8/12 调用方 + 4 个 Client-half 架构限制）、匿名用户 ID 不做。多题 waterfall、轨迹/Cordis ContentDialog、权限门复核**尚未运行时硬证**。官方 158 条代码层 **≈87%**，缺口账 **38 PASS / 2 PARTIAL / 1 SKIP**。

### 不要说

- ❌「静态一致 / 功能对齐完成 / P0+P1 全绿」
- ❌「覆盖率 88%」或「运行时 1/9」
- ❌「HTML 六形态与官方 DOM 一致」（脚本关）
- ❌「Cordis 12 RPC 全有调用方」（实为 8/12）
- ❌「运行时已验证全部功能」

### 一句话对外

> **代码层功能面已对齐（≈87%，RPC 键与 typert 一致），主干运行时已 UIA 硬证；3 处平台/安全/架构有意降级 + 多题与 Dialog 面板待补测，不宣称完全一致。**

### 若还要抬「运行时已证」，下一步该干什么

1. **人工点 Dialog**：Trajectory / Cordis 的 ContentDialog——UIA 探针看不到主窗树内的 Dialog peer，需人工确认面板真开、或把面板改挂主窗可视树（非独立 XAML Root）。
2. **造 ask_user 会话**：让模型触发 `user-questions/request` waterfall（多题 ≥2），再点上一题/下一题/跳过/放弃整组。
3. **权限门复核**：绕开 `RadioMenuFlyoutItem` Toggle 限制（改 Click 路径或直接 Invoke 确认框），重证 `RiskAckCheckBox` 勾选门槛 + 取消不写回。
4. **子代理条目**：准备 continuable 子代理数据，点 `SubagentPrompt` / `SubagentInterrupt`。
5. **设置模型页**：暴露 `SettingsHost` 后点 `RestoreDefaultModelButton`。
6. **右栏第二格**：手动分栏验 `RightPaneCell1` + 「分栏已满」拒绝文案。

---

## 附：T38 源码行号抽查

| 抽查点 | 当前行号 | 结论 |
|---|---|---|
| Cordis 12/8/4 + 禁伪造 | `MainWindow.Cordis.cs:17-39,374-419` | ✅ 与 PARTIAL-CLOSE 一致 |
| WebView2 钉 1.0.2651.64 | `Blade2.csproj:44` | ✅ |
| HTML 路 A 脚本关 | `FilesPanel.Preview.cs:979-1013`；`FilesPanel.xaml:210,288` | ✅ |
| PDF 无 password + 降级 UX | `FilesPanel.Preview.cs:715,744,838` | ✅ |
| RightPaneDock Clear | `MainWindow.LayoutColumns.cs:372` | ✅ |
| EnsureCapabilityUiEntries | `MainWindow.Capabilities.cs:56`；挂接 `MainWindow.xaml.cs:2623,4529` | ✅ |
| 菜单 TryEnqueue | `Capabilities.cs:69`；`Trajectory.cs:624` | ✅ |
| Array.Empty inspect | `Cordis.cs:109` | ✅ |
| ASK_CANCELLED | `UserQuestions.cs:225,643,719,780` | ✅ |
| 插件 4 卡 + maxUses | `MainWindow.xaml.cs:12633-12765` | ✅ |
| 构建 | `build.ps1 -Configuration Debug` | **0 error / 0 warning** |

*生成说明：T38 单代理，只整合证据 + 行号抽查 + 本报告，未改业务功能。`rust/` 与 `Kernel/**/node_modules` 零改动。旧版 T21/T29 结论以本文为准。*
