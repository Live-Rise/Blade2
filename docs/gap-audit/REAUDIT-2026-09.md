# 独立重审报告 REAUDIT-2026-09

> 立场：不采信 `FINAL-ACCEPTANCE.md` / `P0-ACCEPTANCE.md` / 实现代理自评。只采信源码与内核契约（`@deepseek-ai/*` 的 `typert.remote-client.js` / `client.js`）。
> 对照面：WinUI 主干 `MainWindow*.cs` / `Pages/` / `Dsh/`（**不含 `rust/`**）。
> 时间点：2026-09-23，以本次读到的当前源码为准（若并行代理正在改文件，行号可能已漂移）。
> 方法：官方包源码重新抽功能面 → 逐条回代码核验 UI 入口 / RPC 载荷键 / 边界 / i18n / UIA。旧清单仅作线索索引。

---

## 1. 复查结论一句话

**真实静态覆盖率约 84%（对齐约 132 / 部分约 8 / 缺失 0，分母 158）；不能维持「静态一致 / 功能对齐完成」的强结论**——P0 骨架与关键 RPC 载荷基本真实，但至少 3 项被标 PASS 而实际是 PARTIAL/重定义，且 UI 冒烟从未跑过，用户可感知闭环未证。

---

## 2. 假 PASS / 夸大清单

| # | 旧结论 | 复审 | 为何是假 PASS / 夸大 | 证据 |
|---|---|---|---|---|
| F1 | **P1-11 插件设置项补全 = PASS** | **PARTIAL** | 硬编码 ns 仅 4 个（`shell` / `agent-loop` / `subagent-model-selection` / `web-search-deepseek`），其余靠 describe 泛化。官方 `dsh-client-ui-settings-plugins` 有专属卡：webSearchApiKey / BaseUrl / **MaxUses（每请求搜索次数）**、并行工具数、命令超时、单流上限、终端等，WinUI 未做 1:1 字段面。旧报告自己写「有意泛化」却仍打 PASS——这是把降级算成对齐。 | `MainWindow.SettingsExtras.cs:516`；官方 `@deepseek-ai/dsh-client-ui-settings-plugins/lib/client.js:692-730,1593-1602` |
| F2 | **P0-1 六形态 = PASS（HTML 附属清单）** | **PARTIAL**（5/6 真，HTML 重定义） | 官方 documentpreview 是 **DOM 级 HTML 预览**（`HtmlBody` + `readRelated` 注入资源）。WinUI 把 HTML 面重定义为「源码高亮 + 附属清单」，验收口径被改写成实现能力。MD/代码/图/PDF/纯文本五形态真实；HTML 不等价。旧报告披露了降级，但仍计全 PASS 并进覆盖率分子。 | `Pages/FilesPanel.Preview.cs:5,799-822`（源码+附属）；官方 `dsh-client-ui-sidebar-documentpreview/lib/client.js:2314-2468`（DOM 注入） |
| F3 | **覆盖率 88%、对齐 138 / 部分 1 / 缺失 0** | **约 84%** | 分子把多处「有意降级 / 泛化 / 需人工点检」计成全对齐；P1-11、P0-1(HTML)、P1-16(仅会话级停) 应计部分。P2-5 SKIP 计 0.5 合理，但不能把 40/41 缺口都算闭环。 | 本表 F1/F2 + §3 逐项表 |
| F4 | **「可以宣布 Blade² 与 dsh 官方 WebUI 功能静态一致（P0+P1 全绿）」** | **夸大** | UI 冒烟 0 项跑过（旧报告 §5-9 自认）；空态/错误/取消/多实例等边界多数只写了代码路径未验证。`static parity` 可说「代码层大体具备」，不可说「全绿 / 对齐完成」。 | `FINAL-ACCEPTANCE.md:159-181` |
| F5 | **P0-1 验收探针「readAll 对 PDF/图」** | **描述有误** | PDF/图实际走 `workspaceFiles/readBytes`（分段），`readAll` 只在「复制全文且未加载完」时调用。功能在，但旧验收把 RPC 映射写错，削弱了「RPC 真调」的证据可信度。 | `FilesPanel.Preview.cs:82`（readAll 仅 CopyAll）,`:651,693`（readBytes）；官方 `client.js:26941`（readAll 用于 textFace） |
| F6 | **P0-6「Cordis 12 RPC」口径** | **7/12 已接** | typert 实有 12 方法；WinUI 只调 inventory / runHostHalf / resolveRequestRun / settleUserRun / stopFromPanel / undefineFromPanel / getClientCode。缺 invoke / reportClientGuardFailure / reportRenderFailure / resolveInspectQuery / syncInspectManifest（多为 Client 运行时回传，桌面壳可解释）。旧报告在代码注释里承认「面板/审批子集」，但摘要易被读成 12 全接。四 outcome 与 typert **一致**（见 §3）。 | `MainWindow.Cordis.cs:17-19,183-352`；`dsh-cordis-host-runner/lib/typert.remote-client.js`（12 descriptors） |

**假 PASS / 夸大计数：6 条**（其中 F1/F2 直接影响覆盖率分子，F4 是结论级夸大）。

---

## 3. 逐项复核表

> 图例：**PASS** = UI 入口 + 真 RPC/真渲染路径 + 键名对齐；**PARTIAL** = 有入口但子能力/边界不闭环；**FAIL** = 无实现或死代码。旧结论列引自 FINAL-ACCEPTANCE / P0-ACCEPTANCE。

### P0（8 项）

| 官方条目 | 旧结论 | **复审结论** | 证据 file:line | 备注 |
|---|---|---|---|---|
| P0-1 六形态预览 | PASS | **PARTIAL** | `Pages/FilesPanel.Preview.cs:29-37`（六枚举）`:54-71`（FaceOf）`:78-127`（readAll/readBytes/readRelated）`;:651` 图=`readBytes` `:693` PDF=`readBytes` `:810` HTML附属=`readRelated` `:913` CopyAll=`readAll`；分派 `FilesPanel.xaml.cs:681-703` | MD/代码/图/PDF/纯文本真；**HTML=源码+附属，非官方 DOM**（F2）；PDF 密码降级（已披露）；readAll 非预览主路径（F5）。RPC 载荷键 `workspaceFileScopeId`/`path`/`range`/`relativePath` 与 typert `wire` **完全一致**（`dsh-api-workspace-files/lib/typert.remote-client.js:207-322`） |
| P0-2 预览辅助 | PASS | **PASS** | `FilesPanel.xaml:165-219,344` 工具条/横幅/加载更多；`FilesPanel.xaml.cs:573-594,714-731`（stat version→横幅不自动重载）；`FilesPanel.Preview.cs:180-227`（分页）`:882-901`（LoadMore）`:903-945`（复制全文/选中） | wrap / 加载更多 / 重读 / 变更横幅 / 复制均在渲染路径上 |
| P0-3 工具卡 bash/diff/JSON | PASS | **PASS** | **渲染路径**：`MessageActions.cs:592` → `BuildToolCard`（`ToolCards.cs:106`）→ `BuildToolCardBody:300`；bash 退出码/信号/折叠 `:338-388`；diff `:536+` + `CollectToolDiffs:743`；JSON 复制菜单 `:929-934`（参数/结果/紧凑/属性路径）；result 落盘 `MainWindow.xaml.cs:6247` `NoteToolResult` | **不是死代码**。UIA：`ToolCardExitCode`/`ToolCardSignal`/`ToolCopyJson`/`ToolCopyResultJson` |
| P0-4 多题导航 + ASK_CANCELLED | PASS | **PASS** | `UserQuestions.cs:225-334`（上一题/下一题/跳过/提交/去聊天）`:225-230` 放弃整组 `:663-717` 汇总 answers 全题 id `:719-780` `rejected`+`ASK_CANCELLED`；推荐标记 `:439` | 载荷与官方 `pending.cancel()` → `questionError(..., "ASK_CANCELLED")` 一致（`dsh-client-ui-user-questions/lib/client.js:139,500`） |
| P0-5 权限门闸 | PASS | **PASS** | `PermissionConfirm.cs:42-104`（`RiskAckCheckBox` 勾选前禁用确认钮；`ConfirmDangerFullAccessButton`）`:34-48` 防双弹 `:166-177` 设置回退不写回；入口三处全覆盖：设置默认 `PermissionConfirm.cs:164-166`、设置 mutate `MainWindow.xaml.cs:16642`、`/permission` 命令 `:18396`（取消则不发 `commands/execute`） | 官方文案「我已了解风险，并愿意继续」逐字对齐（`dsh-client-ui-permission-presets/lib/client.js:24`） |
| P0-6 Cordis 审批/面板 | PASS | **PARTIAL** | 四按钮 `Cordis.cs:380-388`；四 outcome `:475-538`（allow/once→`runHostHalf(future=false)`→`resolve{ok:true}`；future→`runHostHalf(future=true)`；decline→`resolve{ok:false,reason:"rejected"}`；失败→`host-half-failed`）；RPC `:183,305,326,333,339,345,352` | **四 outcome 与 typert 一致**（`typert.remote-client.js:107-118` 的 ok/rejected/host-half-failed/client-half-failed）。但：① 12 RPC 只接 7（F6）② Client 半边桌面壳无法加载，以 `ok:true` 近似结算（`:531-537`）③ 官方面板第三钮文案是「允许此插件的后续版本」，WinUI 简写「允许后续版本」 |
| P0-7 subagents/* | PASS | **PASS** | `Subagents.cs:61` `list{parentSessionId}` `:122-133` `prompt{request:{requestId,parentSessionId,childSessionId,mode:"continuable",delivery,content[]}}` `:146-151` `interruptByParent{childSessionId,parentSessionId,mode:"continuable"}`；UI `:259` 目录 `:588` 续跑 `:633` 打断 | 载荷键与 `dsh-subagent/lib/typert.remote-client.js:65-155` 的 `wire` **逐键一致** |
| P0-8 轨迹 journal | PASS | **PASS** | **真消费 journal**：`MainWindow.xaml.cs:5916` `RenderEventCore` → `TrajectoryObserve`（`Trajectory.cs:150`）；数据源 = `session/follow` 增量（`DshRpcClient.cs:359-376`）+ `session/page` 回放；`:360` `ApplyGoalRound`；面板 `:655+` | seq 去重防回放翻倍；4000 条上限 / 8 会话 LRU 为资源约束非遗漏 |

### P1（27 项，仅列复审≠旧结论或重点）

| 官方条目 | 旧结论 | **复审结论** | 证据 | 备注 |
|---|---|---|---|---|
| P1-1 Details 面 | PASS | **PASS** | `MessageDetails.cs:1423+`；装载 `MessageActions.cs:93-94` | |
| P1-2 计划评审 | PASS | **PASS** | `UserQuestions.cs:177,199`（`id=="plan-review"` 专属表头）；状态 `SessionState.cs:231-233,331`「计划待审」 | 走 user-questions 通道，与官方 `dsh-plan-mode` intent 一致 |
| P1-3 计划待审状态 | PASS | **PASS** | `SessionState.cs:222-233` 优先级 approval>plan-review>question；`MainWindow.xaml.cs:16422-16438` | |
| P1-4 跨会话召回 | PASS | **PASS** | `MessageActions.cs:252-285` `AppendReferenceChips`；`MessageDetails.cs:236-298` | |
| P1-5 截断/继续 | PASS | **PASS** | `MessageDetails.cs:947-959`（文案+「发送继续」+`MaxTokensContinueButton`）`SendContinuePrompt:554` | |
| P1-6 模型重试 | PASS | **PASS** | `MessageDetails.cs:560-670`（倒计时/取消重试/延迟）`CancelModelRetry:673` | |
| P1-7 压缩摘要 | PASS | **PASS** | `MessageDetails.cs:682+`「上下文已压缩」可点开 `:1031-1058` | |
| P1-8 创造模式 | PASS | **PASS** | `SettingsExtras.cs:85+` 向导；`agentPresets/select` `:126` | |
| P1-9 预设 yml 查看 | PASS | **PASS** | `SettingsExtras.cs:25-80` `agentPresets/read` 只读 | 官方亦为查看 |
| P1-10 Skill chip | PASS | **PASS** | `Skills.cs:267-299` `MakeSkillReferenceChip`→`ShowSkillExplanationAsync:305`；**工具行挂接真** `ToolCards.cs:467-468` | |
| P1-11 插件设置 | PASS | **PARTIAL** | `SettingsExtras.cs:516` 四 ns + 泛化渲染 | **假 PASS（F1）** |
| P1-12 Onboarding | PASS | **PASS** | `Onboarding.cs:30-144,150+` | |
| P1-13 目标指令提示 | PASS | **PASS** | `SessionState.cs:371+`；`MainWindow.xaml.cs` edit/pause/resume/clear | |
| P1-14 `goals/clear` | PASS | **PASS** | `Capabilities.cs:130-131`（clear 门闸）`:183-192`（`args={agentId,ref:{id,revision}}`）`:194-198`（清后 GoalBar 收起）`:223` 按钮 | 载荷键 `agentId`+`ref` 与 `dsh-goal/lib/typert.remote-client.js:129-158` **一致** |
| P1-15 活动定时任务标记 | PASS | **PASS** | `MainWindow.xaml.cs:57` `HasActiveSchedule`；`:3466` 列表标记 | |
| P1-16 jobs 停止 | PASS | **PASS**（口径对齐） | `SessionState.cs:452-533` `StopJobViaSessionCancelAsync`→`session/cancel`；UI 已标明会话级 | **官方 jobs 也是只读**（`dsh-client-ui-jobs/lib/client.js` 仅 status 文案，无 stop 钮）。WinUI 反而多给了会话级停止。旧 GAP 从「正在停止」文案推断官方可停，属推断过头；现行口径正确 |
| P1-17 Todo 面板 | PASS | **PASS** | `Todos.cs:20+`；投影 `SessionState.cs:56-102` | 写路径=模型 `todo_write`，与官方一致 |
| P1-18 workflow-run | PASS | **PASS** | `MessageDetails.cs:740+`；`ToolCards.cs:325-328,517` 成员展开 | |
| P1-19 bash 行细节 | PASS | **PASS** | `ToolCards.cs:338-388` | 同 P0-3 渲染路径 |
| P1-20 文件编辑 diff | PASS | **PASS** | `ToolCards.cs:536-601`；hunks `:743+` | |
| P1-21 文件操作菜单 | PASS | **PASS** | `LayoutColumns.cs:958+`；`FilesPanel.xaml.cs:520+` | |
| P1-22 settings applies | PASS | **PASS** | `SettingsExtras.cs:147-224` | |
| P1-23 连接状态/重连 | PASS | **PASS** | `SettingsExtras.cs:224+`；`MainWindow.xaml.cs:5035` | |
| P1-24 欢迎/版本公告 | PASS | **PASS** | `Onboarding.cs:21-22` `WelcomeNoticeVersion` | |
| P1-25 三栏拖拽 | PASS | **PASS** | `LayoutColumns.cs:28-35` 常量 264/420/280/300/0.7/400 与官方 `dsh-client-ui-layout/lib/client.js` clamp **同值** | |
| P1-26 右栏分栏两格 | PASS | **PASS** | `LayoutColumns.cs:486-508`（满两格禁用+文案「分栏已满（最多两格）」）`:613-618` 短路；新标签页/全屏/移到这里 `:332-393` | 与官方 `sidebar-right` `dockPaneIds>=2` 一致 |
| P1-27 JSON 复制变体 | PASS | **PASS** | `ToolCards.cs:929-934`（参数/紧凑/结果/属性路径）；`MessageDetails.cs:1564-1565` | |

### P2（6 项）

| 官方条目 | 旧结论 | **复审结论** | 证据 | 备注 |
|---|---|---|---|---|
| P2-1 轨迹 Goal 轮 | PASS | **PASS** | `Trajectory.cs:215,360-367` `ApplyGoalRound`→「目标 · Round {n}」 | |
| P2-2 推理/模型两档说明 | PASS | **PASS** | `ModelExtras.cs:24-65` Flash/Pro 文案；`MainWindow.xaml.cs:13517-13522` `EffortTierBlurb` | 与官方 `option.deepseekV4Flash/Pro.description` 对应 |
| P2-3 lastUsed/next 模型记忆 | PASS | **PASS** | `ModelExtras.cs:67-145` `CaptureModelLastUsed`/`RestoreLastUsedModelAsync`；菜单标记 `MainWindow.xaml.cs:13386-13388`；投影 `:13645-13673` | |
| P2-4 恢复默认模型 | PASS | **PASS** | `ModelExtras.cs:150-206` `MakeRestoreDefaultModelCard`；挂接 `MainWindow.xaml.cs:10740` | |
| P2-5 匿名用户 ID | SKIP | **SKIP**（维持） | 隐私相关，任务口径可不做 | 不计缺口，计 0.5 |
| P2-6 内测声明 | PASS | **PASS** | `ModelExtras.cs:212-234` `AppendAboutBrandNotices`；`About.cs:104` | |

### 壳增强（不计分母，核对未被误算进「对齐」）

| 项 | 复审 | 备注 |
|---|---|---|
| 宠物 / 壁纸 / 托盘 / RunStats / ContextMeter / TurnRail / 灯箱 / 使用统计 / KernelBoot / UpdateCheck / 背景皮肤 / 窗口材质 / 气泡材质 / AGENTS.md / 撤回编辑 / 撤回本轮 / 消息时钟 / 等待动画 / ThemeProbe / 28 语 i18n / 记忆·电脑·浏览器控制页 | **未混入官方对齐分子** | FINAL-ACCEPTANCE §4 明确「壳增强不进分母」，本次抽查 GAP-MATRIX §4 清单与覆盖率算法，**未发现被误算进 158 分母**。TurnRail/RunStats 也未被拿来顶替 P0-8（P0-8 走 TrajectoryObserve） |

---

## 4. 真实缺口清单

### P0

| # | 缺口 | 性质 | 说明 |
|---|---|---|---|
| P0-G1 | HTML 专用预览非 DOM | **降级被计全对齐** | 源码高亮 + readRelated 附属清单 ≠ 官方 HtmlBody DOM 渲染。若要求「用户可感知一致」，此项未闭环 |
| P0-G2 | PDF 密码输入 | 有意降级（可接受） | `Windows.Data.Pdf` 无解锁 API；占位 + 用系统打开 |
| P0-G3 | Cordis Client 半边 | **近似结算** | 桌面壳无浏览器 Client 运行时，`resolveRequestRun` 在 Host 起来后直接 `ok:true`（`Cordis.cs:531-537`）。四 outcome 形状对，但 client-half-failed 分支实际到不了 |
| P0-G4 | Cordis 12 RPC 只接 7 | 部分 | 缺 invoke / report*Failure / resolveInspectQuery / syncInspectManifest（inspect/回传类，优先级低） |

### P1

| # | 缺口 | 性质 |
|---|---|---|
| P1-G1 | **插件设置未全量对齐** | 真缺口：webSearchMaxUses、并行工具数、命令超时、单流上限等官方专属字段/文案未 1:1 |
| P1-G2 | jobs 无 job 级停止 | 口径对齐（官方也只读）；若用户期望「只停这一个 job」则仍缺 |
| P1-G3 | UI 冒烟 0 项 | 空态/错误/取消/多实例/只读路径全部「写了但没跑」 |

### P2

| # | 缺口 | 性质 |
|---|---|---|
| P2-G1 | 匿名用户 ID | 有意跳过（维持） |
| P2-G2 | 模型记忆/恢复默认的运行时投影到达 | 静态闭环，依赖 `session/control` modelSelection 帧实测 |

---

## 5. 与旧 FINAL-ACCEPTANCE 的差异说明

| 维度 | 旧 FINAL | 本复审 | 差异原因 |
|---|---|---|---|
| 静态覆盖率 | **88%**（138/1/0） | **约 84%**（约 132/8/0） | F1/F2 等 PARTIAL 不应计全对齐；公式同为 (A+0.5P)/158 |
| P0 | 8/8 PASS | **6 PASS + 2 PARTIAL**（P0-1 HTML、P0-6 Client 近似） | 回源码后 HTML 是重定义；Cordis client-half 不可达 |
| P1 | 27/27 PASS | **26 PASS + 1 PARTIAL**（P1-11） | 插件设置泛化 ≠ 官方全量 |
| P2 | 5 PASS + 1 SKIP | **同**（5 PASS + 1 SKIP） | 一致 |
| 假 PASS 数 | 0（自称） | **6 条夸大/假 PASS** | 见 §2 |
| 最终判定 | 「静态一致——可宣布功能对齐完成」 | **不维持**。可说「P0/P1 代码层大体具备、RPC 载荷键与 typert 对齐」；不可说「对齐完成 / 全绿」 | UI 冒烟未做 + 至少 2 项把降级算对齐 |
| 壳增强 | 23 项不进分母 | **确认未被误算** | 一致 |
| jobs 停止口径 | 会话级 + 标明 | **维持**（且官方亦只读，WinUI 略超） | 旧 GAP 曾推断官方可停，现行表述正确 |
| goals/clear / subagents/* / readAll·readBytes·readRelated 载荷 | 称已接通 | **核实通过**，wire 键逐键对齐 typert | 这部分旧报告没吹牛 |

**一句话差异**：旧报告的 **RPC 接线与 P0 骨架是真的**，但 **覆盖率分子和「对齐完成」结论被美化**；把「有意降级/泛化」记成 PASS，再用 88% 支撑「静态一致」，这是本次重审要打掉的核心问题。

---

## 6. 附：RPC 载荷键核对摘要（真调证据）

| RPC | WinUI 载荷 | typert wire | 结论 |
|---|---|---|---|
| `workspaceFiles/readAll` | `{workspaceFileScopeId, path}` | `workspaceFileScopeId`, `path` | 对齐 |
| `workspaceFiles/readBytes` | `{workspaceFileScopeId, path, range:{offset,length}}` | 同 | 对齐 |
| `workspaceFiles/readRelated` | `{workspaceFileScopeId, path, relativePath}` | 同 | 对齐 |
| `subagents/list` | `{parentSessionId}` | `parentSessionId` | 对齐 |
| `subagents/prompt` | `{request:{requestId,parentSessionId,childSessionId,mode,delivery,content}}` | `request` 整包 | 对齐 |
| `subagents/interruptByParent` | `{childSessionId,parentSessionId,mode:"continuable"}` | 三键同名 | 对齐 |
| `goals/clear` | `{agentId, ref:{id,revision}}` | `agentId`+`ref` | 对齐 |
| `dynamicCordisRunner/resolveRequestRun` | `{requestId, resolution}` | 二参数 | 对齐；resolution 四形状与 typert union 一致 |
| `dynamicCordisRunner/runHostHalf` | `{agentId,pluginId,packageId,mode,requestId,approveFutureVersions}` | 六参数 | 对齐 |

---

*生成说明：本报告为独立重审，未修改业务代码（源码未发现必须小修的明确 bug：权限门闸、RPC 键名、ASK_CANCELLED、jobs 停止口径均正确）。`rust/` 与 `Kernel/**/node_modules` 零改动、零写入。*
