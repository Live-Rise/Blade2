# RUST-PARITY-AUDIT：rust/ 重构 vs C# 主干 功能一致性审计

> **基线**：`WINUI-GUI-FEATURES.md`（主干 C#/XAML 功能真值，不含 rust/）
> **对照**：`rust/`（windows-reactor 重构壳，crate `blade2-rs` 0.8.2）
> **姊妹篇**：`GAP-MATRIX.md` 管「主干 vs 官方 WebUI」；本文只管「rust/ vs 主干」。
> **判定口径**：允许 WinUI XAML vs windows-reactor 的实现差异；**对外可观测行为**（RPC 载荷、文件持久化、进程生命周期、托盘/通知、用户可点路径）必须对齐，否则记缺口。
> **核实原则**：疑点逐条回双方源码验证；盘点漏报/误报在「误报澄清」专节标注 **【纠错】**。
> **审计方式**：双路子代理独立盘点 + 对照复核 + 独立验收交叉；全程只读，未改代码。
> **审计日期**：2026 对齐工作区未提交态（含 `rust/src/main.rs`、`i18n.rs`、`tokens.rs`、`kernel.rs` 在改项）。

---

## 1. 一句话结论

**不能称为「与主干功能完全一致」。** 独立验收 19 项：PASS 1 / PARTIAL 6 / FAIL 12；缺口对照 30 项（P0 7 / P1 19 / P2 4）。rust/ 已对齐内核启动握手、RPC 信封、mux 长驻流与会话主路径；内核生命周期保障、插件 bootstrap、审批闭环、工作区 CRUD、壳本地持久化及宠物/壁纸/托盘/附件/文件面板等整片产品能力均未对齐。

---

## 2. 验收记分卡（19 项独立验收）

| # | 能力 | 结果 | 一句话 |
|---|------|------|--------|
| 1 | 内核进程契约 | PARTIAL | 启动/URL/DSH_HOME/PATH 对齐；杀树与 npm 回退缺 |
| 2 | 稳定性契约 | FAIL | JobObject / 孤儿清扫 / 写租约 零匹配 |
| 3 | 插件 bootstrap | FAIL | 9 包安装 + cordis.patch.yml 全链路缺失 |
| 4 | RPC 信封与 mux 帧 | PARTIAL | 信封/streamId/双层 ok 一致；无 upload 回执 |
| 5 | 会话主路径 | PARTIAL | list/create/prompt/follow/cancel/fork 有；search/page/updateQueue/selectModel/upload 缺 |
| 6 | 工作区 | FAIL | 仅 workspace/follow；CRUD + workspaceFiles + fileReferences + directoryPicker 全无 |
| 7 | 审批与提问闭环 | FAIL | approval/request、user-questions/request、$events/result 零匹配 |
| 8 | 命令 list/execute | PASS | 四态回执对齐 |
| 9 | 投影 | PARTIAL | plan/permissions/turnOutline/sessionStats 有；schedule/goal 缺 |
| 10 | 反馈 | PARTIAL | messageFeedback/* 有；sessionFeedback/record 缺 |
| 11 | 宠物全链路 | FAIL | 仅占位 UI，自陈无 `/api/pet/*` |
| 12 | 壁纸全链路 | FAIL | 皮肤 UI 不落盘；shell-skin.* / dsh-wallpaper 缺 |
| 13 | 壳集成 | FAIL | 托盘开关 UI 有、无实现；无 toast / MSIX |
| 14 | shell.json / AGENTS.md | FAIL | 自陈「分叉不落盘」；重启丢偏好 |
| 15 | i18n 30 语 | PARTIAL | zh/en 内置 + 外部 JSON；无 LocalSettings 语言覆盖 |
| 16 | 更新检查 | FAIL | 「检查更新」死按钮（不挂 on_click） |
| 17 | 文件面板/技能/灯箱/@ | FAIL | 文件面板仅 i18n 串；技能不扫盘；灯箱/@ 未移植 |
| 18 | 异常兜底 | FAIL | 无 `%TEMP%\blade2_unhandled.txt` |
| 19 | 隐性不一致 | FAIL | 7 处行为偏差（见 §6） |

---

## 3. 缺口矩阵（30 项）

### P0 阻断一致性（核心用户路径，共 7 项）

| # | 功能名 | C# 主干行为 | rust 现状 | 证据 | 判定 | 工作量 |
|---|---|---|---|---|---|---|
| P0-1 | JobObject 杀进程树 | `KILL_ON_JOB_CLOSE`，壳退出带走内核整树；`Kill(entireProcessTree:true)` | 无 JobObject；仅 `child.kill()` | C# `Dsh/DshKernelHost.cs:22,174,183,204,439`；rust `src/kernel.rs:1216,1385` | **缺失** | M |
| P0-2 | 孤儿清扫 + 写租约 | `SweepOrphanKernels` 清残留 node；`SessionAlreadyOwnedError` 判定后杀残留再发 | 全源零匹配 | C# `DshKernelHost.cs:236-275`；rust 无 | **缺失** | M |
| P0-3 | 插件 bootstrap | `pnpm` 装 9 包 + 写 `profiles/web/cordis.patch.yml`（approval-gate/pet/wallpaper/memory/browser/computer） | 只扫状态、不安装；默认插件卡空壳 | C# `Dsh/DshPluginBootstrap.cs:53-68,75-82,137,601-735`；rust `src/main.rs:223,7014-7018` | **缺失** | L |
| P0-4 | 审批/提问闭环 | `$events` waterfall `approval/request` + `user-questions/request` → `$events/result` 解挂 | `$events` 只认 `api-session/status`；无 waterfall/回传 | C# `Dsh/DshRpcClient.cs:17,42,612-682,712`、`MainWindow.xaml.cs:2598-2607`；rust `src/kernel.rs:500-504` | **缺失** | M |
| P0-5 | 工作区 CRUD | `workspace/{create,rename,delete,insertBefore,insertSessionBefore,archiveSession}` | 仅 `workspace/follow` | C# `MainWindow.xaml.cs:3450-3888`；rust 全源无 | **缺失** | M |
| P0-6 | 壳本地持久化 | `shell.json`（托盘/材质/通知）+ `shell-skin.*`（皮肤/透明度）+ `AGENTS.md` | 显式注释「分叉不落盘」，重启丢 | C# `MainWindow.TraySettings.cs:62`、`MainWindow.xaml.cs:8405-8426`、`Personalization.cs:26`；rust `main.rs:1326,1941,2221` | **缺失** | M |
| P0-7 | 内核启动回退/搬迁 | `dsh.cmd` 回退；`Code2→Blade2` 数据搬迁 | 仅 `Kernel/node.exe`；无搬迁 | C# `DshKernelHost.cs:45-61`、`MainWindow.xaml.cs:8387-8399`；rust `kernel.rs:109-135` | **缺失** | S |

### P1 重要功能缺口（共 19 项）

| # | 功能名 | C# 主干行为 | rust 现状 | 证据 | 判定 | 工作量 |
|---|---|---|---|---|---|---|
| P1-1 | 附件上传/回读 | `session/uploadFileBinary`→receiptId（四形态回执）；`session/attachment` 历史图 | 无上传；receiptId 仅类型字段 | C# `DshRpcClient.cs:138-169`、`MainWindow.xaml.cs:6438-6459,6859-6874`；rust `kernel.rs:1087` | **缺失** | M |
| P1-2 | 文件面板 | `workspaceFiles/{list,stat,read,changes}` + 文本预览截断 | 无 RPC/UI | C# `Pages/FilesPanel.xaml.cs:54-57,242-638`；rust 无 | **缺失** | L |
| P1-3 | @ 引用 | `fileReferences/list` + `sessionReferenceResolver/candidates` | 显式「那棵分叉没有」 | C# `MainWindow.xaml.cs:17749-17837`；rust `main.rs:1728,4315` | **缺失** | M |
| P1-4 | 目录选择器 | `directoryPicker/{pick,list,createDirectory}` | 无 | C# `MainWindow.xaml.cs:3506-3761`；rust 仅 i18n | **缺失** | M |
| P1-5 | 会话检索/队列/反馈/路径探测 | `session/search`、`session/updateQueue`、`sessionFeedback/record`、`canOpenWorkspacePath` | 全无 RPC | C# `MainWindow.xaml.cs:13592,16626,16910,3817`；rust 无调用 | **缺失** | M |
| P1-6 | goals | `goals/get` + GoalBar + create/pause/resume/complete | 无 RPC/UI | C# `MainWindow.Capabilities.cs:137,186`、`xaml.cs:16083`；rust 无 | **缺失** | M |
| P1-7 | 宠物全链路 | `/api/pet/*` + 精灵分层窗 + petdex/codex-pets 安装 + `pet-window.json` | 显式「没有 `/api/pet/*` 通道」，仅占位设置页 | C# `Dsh/DshPetClient.cs:16-222`、`MainWindow.Pet.cs`、`DshPetStore.cs`；rust `main.rs:7583-7587` | **缺失** | L |
| P1-8 | 壁纸/皮肤落盘 | `shell-skin.{img,video,we,opacity,videopause}` + WE 工坊 + `dsh-wallpaper/*` | 仅皮肤设置 UI；自陈不落盘 | C# `MainWindow.Wallpaper.cs:23-54`、`Dsh/DshWallpaperClient.cs`；rust 无 `shell-skin` | **缺失** | L |
| P1-9 | 托盘 + toast | `Shell_NotifyIcon` + AppNotification/COM `action=update` + 关闭进托盘 | 开关 UI 有、不挂 on_click；无 toast | C# `MainWindow.ShellIntegration.cs:11,125,319`；rust `main.rs:8209-8247,7112-7113` | **缺失** | M |
| P1-10 | 更新检查 + MSIX | GitHub releases 静默检查 + 每 tag 一次 + toast + About 下载 msix | 「检查更新」死按钮 | C# `MainWindow.UpdateCheck.cs:9-35`、`About.cs:230-265`；rust `main.rs:7537,7112-7113` | **缺失** | M |
| P1-11 | credentials / llm 设置 | `credentials/{describe,set,unset}`、`llm/{listConfigurableProviders,listProviders,discoverModels}` | UI 显式「没有这两条端点」；协议在 fake_dsh 有测 | C# `xaml.cs:10652-11891`；rust `main.rs:7767-7769` | **缺失（UI）** | M |
| P1-12 | agentPresets 真接 | list/copy/deletePreset/select + 打开目录 | 注释「分叉拿不到」 | C# `xaml.cs:12808-13083`；rust `main.rs:7744` | **缺失** | S |
| P1-13 | settings 真写 | `settings/{describe,mutate,update,replace,openSettingsDocument}` | 「只存在内存里」「未接入」 | C# `xaml.cs:10468-10516,11873`；rust `main.rs:1942,8262,8275` | **行为不一致** | M |
| P1-14 | 技能真扫盘 | 5 根目录扫 `SKILL.md`（project.dsh/.agents → preset → user） | 「分叉不扫盘」→ 空态 | C# `MainWindow.Skills.cs:19-390`；rust `main.rs:7713` | **缺失** | M |
| P1-15 | open-in-app | `GET /open-in-app/apps` + `POST /open-in-app/open` | 无 | C# `xaml.cs:7972-8066`；rust 无 | **缺失** | S |
| P1-16 | TurnRail UI | 右侧轮次轨 + 悬停预览 + 点击跳转 | **有** turnOutline 解析、**无** UI 轨 | C# `MainWindow.TurnRail.cs:14-74`；rust `kernel.rs:622-698,1031-1034` | **部分** | M |
| P1-17 | 使用统计图表 | journal 聚合 + 热力/趋势/模型占比 | 仅 KPI 卡壳 + 空态 / 硬编码 0 | C# `MainWindow.RunStats.cs`、`xaml.cs:13871-15003`；rust `main.rs:7804-7812,7897` | **部分** | L |
| P1-18 | 图片灯箱 | `ShowImageLightbox` Esc/背板/焦点还原 | 无 | C# `MainWindow.ImageLightbox.cs:23-98`；rust 无 | **缺失** | S |
| P1-19 | 撤回本轮修改 | tool/result 突变流水 + 文件恢复 | 仅按钮样式注释 | C# `MainWindow.MessageActions.cs:1064-1282`；rust `main.rs:8455` | **缺失** | L |

### P2 边角（共 4 项）

| # | 功能名 | C# 主干行为 | rust 现状 | 证据 | 判定 | 工作量 |
|---|---|---|---|---|---|---|
| P2-1 | 未处理异常落盘 | `%TEMP%\blade2_unhandled.txt` | 无（仅 hook 内 catch_unwind） | C# `App.xaml.cs:18-36`；rust `keys.rs:372` | **缺失** | S |
| P2-2 | Layout/Reconnect 探针 | 独立 probe 通道 + 测试产物 JSON | 仅测试侧脚本 | C# `MainWindow.LayoutProbe.cs`、`ReconnectProbe.cs` | **缺失** | S |
| P2-3 | selectModel / modelCatalog | 发送前补 `session/selectModel`；模型目录 | 显式「整步跳过」 | C# `xaml.cs:7188,2553`；rust `main.rs:2292-2295` | **缺失** | S |
| P2-4 | 发送者/品牌迁移 | `Code2→Blade2` + `shell-toast.sender` 迁移 | 无 | C# `MainWindow.ShellIntegration.cs:334-422` | **缺失** | S |

---

## 4. OK 已对齐（简表）

| 能力 | 证据 |
|---|---|
| 内核定位/启动：`Kernel\node.exe` + `web --no-open --port 0` + `dsh web:` 90s 握手 + `DSH_HOME=%LOCALAPPDATA%\Blade2` + `PATH` 前置 `Kernel\bin` | `rust/src/kernel.rs:14-16,109-137` |
| RPC 信封 `{type:client-request,rpcId,method,payload:{args}}` + `/api/remote.mux` 字符串 streamId | `rust/src/kernel.rs:1283-1297`、`mux.rs:206-217` |
| 长驻流：`session/follow`(+assistantStream) / `workspace/follow` / `session/control` / `$events` | `rust/src/main.rs:3443-3449`、`main.rs:947` |
| 自动重连退避 0.5s→30s + `reseed_after_reconnect` | `rust/src/main.rs:150-164,1844-1889,3494-3551,4201-4308` |
| 会话：`session/{create,list,page,prompt,cancel,rename,fork,openWorkspacePath}` | `rust/src/main.rs:2310-2658`、`kernel.rs:1327-1381` |
| 命令：`commands/{list,execute}` 四态回执 | `rust/src/kernel.rs:1048-1139,1342-1373` |
| 反馈：`messageFeedback/{list,put,delete}`（双层 ok + ifVersion） | `rust/src/main.rs:2670-2879` |
| 交付物：`POST /api/present.open` open/reveal | `rust/src/kernel.rs` post_route |
| 投影：`plan` / `permissions` / `turnOutline` / `sessionStats` / `title` | `rust/src/kernel.rs:624-736,698` |
| 主题 tokens / Palette / 窗口材质 4 档 / 气泡材质 3 档 | `rust/src/theme.rs`、`tokens.rs` |
| i18n：zh/en 内置 + `Assets/i18n/<locale>.json` 回落 | `rust/src/i18n.rs:1166-1209` |
| 键盘钩子 / 剪贴板 CF_UNICODETEXT | `rust/src/keys.rs`、`clipboard.rs` |
| 契约测试：`fake_dsh` + `tests/ipc.rs`（测 rust 客户端↔伪内核，**不替代**主干功能） | `rust/src/bin/fake_dsh.rs`、`rust/tests/ipc.rs` |

---

## 5. 误报澄清【纠错】

| # | 盘点初判 | 核实结论 | 证据 |
|---|---|---|---|
| 1 | StreamsReset 自动重连「未见」 | **误报**。rust 有完整退避重连 + `reseed_after_reconnect` 重拉台账 | `rust/src/main.rs:150-164,1844-1889,3494-3551` |
| 2 | TurnRail 整块缺失 | **纠为「部分」**。有 turnOutline 投影解析，缺 UI 轨条 | `rust/src/kernel.rs:622-698` |
| 3 | 使用统计「无面板」 | **纠为「部分」**。有 KPI 卡壳，缺 journal 聚合与图表 | `rust/src/main.rs:7805-7812` |
| 4 | session/search「有搜索 UI 没接内核」 | **纠表述**。无 RPC 调用，仅 i18n 文案 + 本地标题过滤 `self.search` | `rust/src/main.rs` search 字段 |
| 5 | 「检查更新按钮 disabled」 | **纠为「死按钮」**。正常绘制但 `settings_button` 不挂 `on_click` | `rust/src/main.rs:7112-7113` |
| 6 | credentials/\* 完全无 | **纠分层**。`fake_dsh`/`tests/ipc.rs` 有完整契约测试，缺的是 UI 接线 | `rust/tests/ipc.rs:2450-2816` |
| 7 | messageFeedback / commands 为缺口 | **误报**。已接 list/put/delete 与 list/execute | `rust/src/main.rs:2670-2879`、`kernel.rs:1342-1373` |

---

## 6. 隐性不一致（实现有但行为/契约不同）

| # | 偏差点 | C# | rust | 影响 |
|---|---|---|---|---|
| 1 | 退出杀树 | `Kill(entireProcessTree:true)` + JobObject | 仅 `child.kill()` | 孙进程不回收 |
| 2 | `session/list` 参数键 | `{_request:{}}` 形状待真内核核 | rust 同为 `{_request:{}}`（`kernel.rs:1338`）键名带下划线，**疑契约漂移** | 真内核联调优先验证 |
| 3 | 发送模式 | busyEnter 分支 queue/steer | 恒 `mode=queue`（`main.rs:2289-2291`） | 繁忙插话语义丢失 |
| 4 | selectModel 前置 | 每发补 `session/selectModel` | 整步跳过（`main.rs:2292-2293`） | 模型/effort 不随发送对齐 |
| 5 | 内核版本缓存 | 每次渲染重读 | OnceLock（`kernel.rs:76-80`） | 升级后 About 可能显示旧值 |
| 6 | 上传回执 | 四形态兼容 | 无上传，receiptId 仅类型字段 | 附件链路不可用 |
| 7 | DSH_HOME 覆盖口 | 参数注入 | `BLADE2_DSH_HOME` | 配置面不同 |

---

## 7. 补齐路线图（按依赖排序）

### 共用基础设施（先做）

| ID | 任务 | 解锁 | 工作量 |
|---|---|---|---|
| I-1 | 进程生命周期：JobObject `KILL_ON_JOB_CLOSE` + `Kill(entireProcessTree)` + SweepOrphanKernels + `SessionAlreadyOwnedError` 清租约重试 | P0-1/2 | M |
| I-2 | 插件 bootstrap 全链路（pnpm 装 9 包 + cordis.patch.yml + 记忆 jsonl + session-query patch + auto-approve + disabled 热切） | P0-3 及宠物/壁纸/记忆/浏览器电脑控制 | L |
| I-3 | 壳持久化层：`shell.json` / `shell-skin.*` / `AGENTS.md` 读写 | P0-6、材质/托盘/皮肤/个性化 | M |
| I-4 | `$events` waterfall + `$events/result` 回传通道 | P0-4 审批/提问 | M |

### P0 任务

| ID | 任务 | 依赖 | 工作量 |
|---|---|---|---|
| P0-1/2 | 进程树回收 + 孤儿清扫 + 写租约 | I-1 | M |
| P0-3 | 插件 bootstrap | I-2 | L |
| P0-4 | 审批/提问闭环 | I-4 | M |
| P0-5 | workspace CRUD 六方法 | 无 | M |
| P0-6 | shell.json / shell-skin / AGENTS.md 落盘 | I-3 | M |
| P0-7 | npm `dsh.cmd` 回退 + Code2→Blade2 搬迁 | 无 | S |

### P1 任务

| ID | 任务 | 依赖 | 工作量 |
|---|---|---|---|
| P1-1 | uploadFileBinary + image/file block + session/attachment + updateQueue | 无 | M |
| P1-2 | 文件面板 workspaceFiles/* | 无 | L |
| P1-3/4 | @ 引用 + directoryPicker | 无 | M |
| P1-5 | session/search + sessionFeedback/record + canOpenWorkspacePath | 无 | M |
| P1-6 | goals/* + GoalBar | 无 | M |
| P1-7 | 宠物 `/api/pet/*` + 精灵窗 + 安装器 | I-2 | L |
| P1-8 | 壁纸 shell-skin + dsh-wallpaper + WE | I-2, I-3 | L |
| P1-9 | 托盘 Shell_NotifyIcon + toast COM + action=update | 无 | M |
| P1-10 | GitHub 更新检查 + remindedUpdateTag + msix | P1-9 toast | M |
| P1-11/12/13 | credentials / llm / agentPresets / settings 真写接线 | 无 | M |
| P1-14 | 技能 5 根扫盘 | 无 | M |
| P1-15 | open-in-app | 无 | S |
| P1-16 | TurnRail UI 轨 | 无（投影已有） | M |
| P1-17 | 使用统计 journal 聚合 | 无 | L |
| P1-18 | 图片灯箱 | 无 | S |
| P1-19 | 撤回本轮修改 | 无 | L |

### P2 任务

| ID | 任务 | 工作量 |
|---|---|---|
| P2-1 | `blade2_unhandled.txt` 兜底 | S |
| P2-2 | Layout/Reconnect 探针 | S |
| P2-3 | selectModel 前置 + modelCatalog | S |
| P2-4 | Code2→Blade2 / toast sender 迁移 | S |

### 契约漂移优先核验（真内核联调）

1. `session/list` 参数键 `_request`（`rust/src/kernel.rs:1338`）
2. prompt busyEnter → mode=queue|steer
3. 发送前 `session/selectModel` 是否必须

---

## 8. 验收结论

**不通过。** 「功能完全一致」不成立：12 项硬缺失 + 7 处隐性不一致 + 30 项缺口矩阵未清。

rust/ 当前是「内核协议 + 会话主路径 + 设置壳」的骨架。若要冲一致，优先 I-1 进程生命周期、I-2 插件 bootstrap、I-4 审批闭环、I-3 持久化四条共用基础设施——它们是 P0 乘数，缺了会连带 P1 整片失效。

> **审计轨迹**：双路 explore 盘点 → general 对照复核（含 7 条纠错）→ general 独立验收（19 项记分卡）交叉一致后落盘。全程只读。
