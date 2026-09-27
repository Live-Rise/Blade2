# T37 PARTIAL-CLOSE：Cordis 剩余 RPC 补接 + HTML 预览方案定案

> 范围：`MainWindow.Cordis.cs` / `Pages/FilesPanel.Preview.cs` / `Pages/FilesPanel.xaml` / `Blade2.csproj`（路 A）
> 构建：`pwsh -File build.ps1 -Configuration Debug` → **0 error**（warning 为预存项）
> 规则：`rust/`、`Kernel/node_modules` 零改动

---

## 1. Cordis `dynamicCordisRunner/*` 方法表（终态）

对照 `Kernel/dsh/node_modules/@deepseek-ai/dsh-cordis-host-runner/lib/typert.remote-client.js`（12 descriptors）。

| # | 方法 | wire 面 | 桌面调用方 | 终态 | 说明 |
|---|------|---------|-----------|------|------|
| 1 | `inventory` | ✅ | 面板/事件刷新 | **已接** | `RefreshCordisInventoryAsync` |
| 2 | `runHostHalf` | ✅ | 审批卡 / 面板 Run | **已接** | `CordisRunHostHalfAsync` |
| 3 | `resolveRequestRun` | ✅ | 审批四 outcome | **已接** | `CordisResolveRequestRunAsync` |
| 4 | `settleUserRun` | ✅ | 面板 Run 后结算 | **已接** | `CordisSettleUserRunAsync` |
| 5 | `stopFromPanel` | ✅ | 面板 Stop | **已接** | `CordisStopFromPanelAsync` |
| 6 | `undefineFromPanel` | ✅ | 面板 Remove | **已接** | `CordisUndefineFromPanelAsync` |
| 7 | `getClientCode` | ✅ | 面板 Inspect | **已接** | `CordisGetClientCodeAsync` |
| 8 | `syncInspectManifest` | ✅ | `AttachCordisEvents` | **已接** | 传 `[]` 显式声明 Client inspect 目录为空 |
| 9 | `invoke` | ✅ | （无） | **wire 已接 / 无调用方** | Client→Host 方法桥；面板 inspect 对齐官方只用 `getClientCode`，不从面板 invoke |
| 10 | `resolveInspectQuery` | ✅ | （无） | **wire 已接 / 无调用方** | Host 仅接受 `ok:true`（`!ok` → `accepted:false` 不结算）；无真实 Client provider 数据，不可伪造应答。未注册 provider 时 Host 在 `queryClient` 直接抛 `not registered`，不挂 pending |
| 11 | `reportClientGuardFailure` | ✅ | （无） | **wire 已接 / 无调用方** | Client 守卫失败回传；无 Client 半边 = 无失败源 |
| 12 | `reportRenderFailure` | ✅ | （无） | **wire 已接 / 无调用方** | Client 渲染失败回传；无 Client 半边 = 无失败源 |

**计数**：12/12 wire 面已接；**8/12 有桌面调用方**；4/12 为 Client-half 专用（架构限制）。

### 架构限制（写死，禁止造假分支）

桌面壳 = WinUI，**无浏览器 Client 半边运行时**（官方 Client 在 opaque iframe 跑 `dsh-cordis-client-runner`）。

| 限制 | 结论 |
|------|------|
| `client-half-failed` | `resolveRequestRun` / `settleUserRun` 的 `reason:"client-half-failed"` 分支**不可达**——Client 半边从不加载，失败只可能是 `rejected` / `host-half-failed`。已在 `ResolveCordisApprovalAsync` 注释写死；**不伪造该分支**。 |
| `invoke` | Client 半边调用 `run.handlers`。桌面壳无 Client 可发起；官方面板 inspect 亦只读源码。 |
| `resolveInspectQuery` + `syncInspectManifest` | 官方 Client 注册 Service/Event/Builtin/Slots/Theme 五个 inspect provider 并应答 `cordis/inspect-query`。桌面壳无 slots/theme service，无法提供等价 provider。空 `syncInspectManifest([])` = 显式无目录。 |
| `reportClientGuardFailure` / `reportRenderFailure` | Client 失败回传 Host。无 Client = 无失败源。 |

官方 Client inspect providers（对照 `dsh-cordis-client-runner/lib/client.js:4665-4710`）：`Service` / `Event` / `Builtin` / `Slots` / `Theme`——全部依赖浏览器 Client 运行时。

### 按钮文案对齐（REAUDIT F6③）

对齐 `dsh-client-ui-cordis/lib/client.js` `action.*`：

| 官方 key | 官方 zh | 官方 en | 本壳（修后） |
|----------|---------|---------|-------------|
| `action.approve` | 允许 | Allow | ✅ 允许 / Allow |
| `action.approveOnce` | 仅允许此版本 | Allow this version only | ✅ 同 |
| `action.approvePlugin` | **允许此插件的后续版本** | **Allow future versions of this plugin** | ✅ 已从「允许后续版本」改正 |
| `action.decline` | 拒绝 | **Decline** | ✅ ShellEnglish `拒绝` 由 "Reject" 改为 "Decline" |

---

## 2. HTML 预览方案定案（P0-1 最后一块）

### 两条路评估

| | 路 A：WebView2 DOM | 路 B：源码+大纲+附属 |
|---|---|---|
| 与官方 DOM 视觉 | **接近**（静态布局/CSS） | 不等价 |
| 脚本 | **关**（`IsScriptEnabled=false`） | 不执行 |
| 官方对照 | `HtmlBody` = `sandbox="allow-scripts"` opaque iframe | — |
| 安全边界 | 可控（见下） | 无新攻击面 |
| 依赖 | WebView2 Runtime（Win10/11 Evergreen 通常自带） | 无 |
| csproj | 需 `Microsoft.Web.WebView2` | 不动 |

### 选定：**路 A（WebView2 DOM 默认）+ 路 B 兜底**

理由：
1. 用户目标「与官方完全一致」——路 A 是唯一能给出 DOM 级渲染的路径；`Blade2.csproj` / WindowsAppSDK 1.6 允许引入 WebView2。
2. 安全边界清楚：仅渲染受信工作区本地 HTML，脚本关、禁外链、CSP 收紧、资源自包含。
3. WebView2 Runtime 缺失 / 初始化失败时**自动回落路 B**，并可手动「源码」切换——部署韧性不降。

### 实现（路 A）

- 包：`Microsoft.Web.WebView2` 1.0.2903.40（`Blade2.csproj`）
- 控件：`Pages/FilesPanel.xaml` `HtmlWebView` + `HtmlModeToggle`（渲染/源码）
- 逻辑：`Pages/FilesPanel.Preview.cs` `TryShowHtmlDomAsync` / `EnsureHtmlWebViewAsync` / `BuildSelfContainedHtmlAsync`

**安全边界（写进代码注释）**：

| 措施 | 值 |
|------|-----|
| `IsScriptEnabled` | **false**（官方 sandbox allow-scripts；壳关脚本） |
| `AreDefaultScriptDialogsEnabled` | false |
| `IsWebMessageEnabled` | false |
| 右键 / DevTools / 状态栏 / 缩放 / 内置错误页 / Autofill | 全关 |
| `NavigationStarting` | 只放行 `about:` / `data:`，其余 `Cancel` |
| `NewWindowRequested` | 一律 `Handled=true` |
| 内容 | 自包含：剥离 `<script>` 与外部 URL；本地 CSS 内联；图片转 `data:`；CSP `default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src 'none'` |
| 附属读取 | 仅工作区相对路径（`ResolveHtmlRelatedPath` 拒绝 `..` 越界） |

**残余风险（已知、已收）**：
1. **JS 驱动页面静态化**——官方 iframe 跑脚本，壳不跑；动态交互/SPA 不等价 → FINAL 口径 **PARTIAL**。
2. **WebView2 Runtime 依赖**——缺失时回落路 B，用户仍可看源码/大纲/附属。
3. **CSS `url()` 残留**——已剥离带协议的 src/href；极端 CSS 逃逸被 CSP `default-src 'none'` 挡住。
4. **NavigateToString 长度上限**——超大 HTML 可能被截断；失败回落路 B。

### 路 B（兜底 / 可切换）保留

源码高亮 + `BuildHtmlOutline` 大纲 + `readRelated` 附属可点开（原 T28 能力，未删）。

### FINAL 口径

> HTML：**路 A DOM 渲染已落地（脚本关）**，静态布局接近官方；**不等价官方 `sandbox="allow-scripts"` DOM**（JS 交互缺失）→ 标 **PARTIAL**。

---

## 3. PDF 加密结论

| 项 | 结论 |
|----|------|
| `Windows.Data.Pdf.PdfDocument.LoadFromStreamAsync` | **无 password / decrypt 参数**（WinRT 公开面已核） |
| 解锁 API | **无**。`PdfDocument` 无 `Unlock` / `Password` / `LoadFromStreamAsync(password)` 等入口 |
| 处置 | **维持**「加密 PDF」降级 UX：`LooksLikeEncryptedPdf`（尾部 64KB `/Encrypt` 启发式）+ 系统打开 + 复制路径 |
| 第三方库 | 不引入（避免新依赖；任务口径允许维持降级） |

**仍 PARTIAL**：加密 PDF 壳内不能解密渲染（平台无 API，有意降级）。

---

## 4. 仍 PARTIAL 项汇总

| # | 项 | 原因 | 能否闭环 |
|---|-----|------|---------|
| 1 | HTML ≠ 官方 DOM（脚本关） | 安全边界：壳不执行页面脚本 | 需引入脚本沙箱才可能；当前有意 |
| 2 | 加密 PDF 壳内渲染 | `Windows.Data.Pdf` 无 password API | 平台限制 |
| 3 | Cordis `client-half-failed` | 无浏览器 Client 半边 | 需嵌 WebView/iframe Client 运行时 |
| 4 | Cordis invoke / resolveInspectQuery / report* 无调用方 | 同上（Client-half 专用） | 同上 |
| 5 | Cordis Client inspect providers（Service/Event/Builtin/Slots/Theme） | 无 Client 运行时 | 同上 |

---

*生成说明：T37 单代理完成。对照源 `@deepseek-ai/dsh-cordis-host-runner/lib/typert.remote-client.js`（12 方法）、`dsh-client-ui-cordis/lib/client.js`（按钮文案）、`dsh-cordis-client-runner/lib/client.js`（Client 回传/inspect）、`dsh-client-ui-sidebar-documentpreview/lib/client.js`（HtmlBody DOM）。`rust/` 与 `Kernel/**/node_modules` 零改动。*
