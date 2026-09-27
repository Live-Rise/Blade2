# T28 收尾剩余降级（REMAINING-GAPS-CLOSE）

> 基线：`FINAL-ACCEPTANCE.md` §5 遗留与有意降级清单  
> 范围：插件设置 ns 1:1 / 主题与个性化域 1 条 / HTML 预览 / PDF 密码 / P2-5 匿名 ID  
> 构建：`pwsh -File build.ps1 -Configuration Debug` → **0 error**（warning 为预存 CS 可空/未用 + PRI `Kernel/**/node_modules` 噪音）  
> 规则：中文 + L()/ShellEnglish；`rust/`、`Kernel/node_modules`、`Dsh/` 零改动

---

## 1. 插件设置 ns 1:1

### 做了什么

对照 `@deepseek-ai/dsh-client-ui-settings-plugins` 与 `settings-plugin-inventory` 的分区/文案/控件，把官方 4 张插件卡补齐为等价 UI，并保留泛化兜底：

| 官方卡 | ns | 字段（官方文案） | 落地 |
|---|---|---|---|
| BashCard「终端」 | `shell` | 命令超时（毫秒）/ 单流输出上限（字节） | `AddShellCard` + `MakeOfficialFieldRow` |
| AgentLoopCard「Agent 循环」 | `agent-loop` | 并行工具调用数 | `AddAgentLoopCard` |
| SubagentModelSelectionCard「Subagent」 | `subagent-model-selection` | 允许开关 + Agent 可选择的模型 | `AddSubagentCard` + `AppendSubagentAllowedModels` |
| WebSearchCard「网页搜索」 | `web-search-deepseek` | API Key / 接口地址 / 单次请求最多搜索次数 | `AddWebSearchCard` |

控件对齐（官方 reset/overridden 语义）：

- **「已覆盖」** 胶囊：`IsNsFieldOverridden` 比对 user 层 vs base 层（`MakeOverriddenBadge`）
- **「恢复默认」** 钮：`settings/mutate op=unset` 字段级回退（`MakeFieldResetButton` / `ResetNsFieldAsync`）
- **「请填数字；留空表示使用默认值。」** 官方 invalidNumber 文案贴数值卡
- Subagent 开关说明改为官方 `subagentModelSelectionChoose` 全文（含「推理强度」）
- 网页搜索 API Key 状态改为官方 `webSearchApiKeySet/Unset`（「已配置密钥。」/「未配置密钥；配置之前搜索不可用。」）
- 插件列表 Tab 文案对齐 inventory：「搜索插件」「暂无插件。」「没有匹配的插件。」+ 运行态「运行中/加载中/等待依赖/卸载中/启动失败/未运行」+「已启用/已停用」

**泛化兜底**（保留并显式接入）：`AppendGenericPluginNsFallback` 扫 `settings/describe` 里其余 writable、带 schema.dict 的插件式 ns（排除官方 4 + `llm-*` + 权限/主题/语言/预设域），按字段类型生成 number/bool/string/union 行（`AddGenericPluginNsCard`）。

### 证据

- 官方文案：`Kernel/dsh/node_modules/@deepseek-ai/dsh-client-ui-settings-plugins/lib/client.js:1620-1672`（zh 字典）
- 官方 inventory 文案：`…/dsh-client-ui-settings-plugin-inventory/lib/client.js:559-595`
- 落地：`MainWindow.SettingsExtras.cs`（`IsNsFieldOverridden` / `MakeOfficialFieldRow` / `AppendGenericPluginNsFallback` / `AddGenericPluginNsCard`）；`MainWindow.xaml.cs` `AddShellCard`/`AddAgentLoopCard`/`AddSubagentCard`/`AddWebSearchCard`/`RenderPluginInventoryContentAsync`
- ShellEnglish 成对键：`MainWindow.xaml.cs` T28 注释块

### 仍降级原因

- **保存模型不同**：官方为卡级「保存/放弃修改/未保存」草稿；壳沿用 `Edit()` 防抖自动写回（FINAL-ACCEPTANCE 已标明的壳交互差异，用户仍能完成同一件事）。字段级「恢复默认/已覆盖」已对齐。
- 官方「展开设置/收起设置」折叠：壳为二级导航卡钻取（Windows 设置形态），语义等价。

---

## 2. 主题与个性化域 83% → 对齐

### 做了什么

对照 `@deepseek-ai/dsh-client-ui-theme`（AppearanceRow / FontSizeRow）补齐域 13 那 1 条「官方个性化子能力未逐字对齐」：

- **外观三选方块**：`MakeThemeCubes` 替换原 ComboBox——浅色/深色/跟随系统三个 ToggleButton（图标+文案），单选语义 + aria-pressed 等价（`ThemeCube_{id}` UIA），对齐官方 `themeCube` + `selected` 形态。
- 字号步进器（12–17px、仅会话内容）此前已 1:1（`MakeFontSizeStepper`）。
- 主题 token 体系（`--dsw-*`）在 WinUI 侧为 Theme 资源字典等价（`Theme/Tokens.xaml`），契约由 `check-dsh-contract.ps1` 维持。
- 壳增强（Mica/壁纸/气泡材质等）保留，不进官方分母。

### 证据

- 官方：`dsh-client-ui-theme/lib/client.js` `AppearanceRow`（CUBES light/dark/system）、`"appearance.title":"外观"`、`"fontSize.title":"字号大小"`、`"fontSize.description":"仅影响会话内容的字号"`、Schema `fontSize` 12–17
- 落地：`MainWindow.xaml.cs` `MakeThemeCubes` + `RenderGeneralSection` 外观卡；`MainWindow.Personalization.cs` 壳增强分区不动

### 仍降级原因

无（该条已对齐）。壳侧 Mica/壁纸等仍为增强项，不计入官方覆盖率。

---

## 3. HTML 预览（无 WebView2）

### 做了什么

在**不引入 WebView2** 前提下逼近官方：

1. **完整源码**：`LoadTextPageAsync` 整文件源码 + 代码高亮 + 加载更多（已有）
2. **附属资源列表可打开**：`DiscoverHtmlRelated`（script/link/img 相对路径）→ `readRelated` 读大小 → **可点开按钮**（`HtmlRelatedList`，点条目在预览中打开该附属文件）
3. **基础标签结构大纲**：`BuildHtmlOutline` 解析 title / h1 计数 / 链接计数 / 图片计数 → `HtmlOutlineSummary` 一行展示

### 证据

- `Pages/FilesPanel.Preview.cs`：`LoadHtmlFaceAsync` / `BuildHtmlOutline` / `ResolveHtmlRelatedPath` / `DiscoverHtmlRelated`
- `Pages/FilesPanel.xaml`：`HtmlOutlineSummary` / `HtmlRelatedSummary` / `HtmlRelatedList`
- **WebView2 评估结论**：`Blade2.csproj` 未引用 WebView2 包；WindowsAppSDK 传递依赖里虽有 `Microsoft.Web.WebView2.Core.Projection` 投影程序集，但运行时仍需 WebView2 Runtime，且 DOM 预览会执行页面脚本（安全边界）。**保持无 WebView2**，与官方 DOM 级预览不等价——这是有意的安全/部署降级。

### 仍降级原因

- 非官方 DOM 实时预览（无 WebView2，有意）
- 大纲为正则基础解析，非完整 DOM 树

---

## 4. PDF 密码（降级 UX）

### 做了什么

`Windows.Data.Pdf.PdfDocument.LoadFromStreamAsync` **无 password 参数**，无法解锁加密 PDF。改进降级 UX：

- **明确「加密 PDF」**：`LooksLikeEncryptedPdf` 扫尾部 64KB 的 `/Encrypt` 启发式；命中时 `PdfPageLabel` 显示「加密 PDF」，否则「无法在壳内渲染」
- **说明文案**：「Windows.Data.Pdf 无解锁 API，壳内无法解密渲染。可用「用系统打开」查看，或复制路径后用其它工具解锁。」
- **一键系统打开**：`PdfOpenButton`（已有）
- **复制路径**：`PdfCopyPathButton` + `OnPdfCopyPathClick`（绝对路径优先，否则工作区相对路径）

### 证据

- `Pages/FilesPanel.Preview.cs` `LoadPdfFaceAsync` catch 分支 / `LooksLikeEncryptedPdf` / `OnPdfCopyPathClick`
- `Pages/FilesPanel.xaml` `PdfCopyPathButton`

### 仍降级原因

- 平台 API 无解锁入口（查证 `Windows.Data.Pdf` 公开面：无 password/decrypt API）；未发现可行解锁 API，故不接第三方 PDF 库（避免新依赖）。加密 PDF 仍不能壳内渲染。

---

## 5. P2-5 匿名用户 ID

**仍跳过（有意）**。遥测用匿名 ID 管理涉及隐私与设备指纹，任务口径明确可不做。不引入 `dsh-anonymous-user-id` 等价物。

---

## 6. 汇总

| # | 项 | 结论 | 用户可感知变化 |
|---|---|---|---|
| 1 | 插件设置 ns 1:1 | **完成**（4 卡官方文案/控件 + 泛化兜底） | 与官方同文案；字段可「恢复默认」；附加 ns 可配 |
| 2 | 主题与个性化 1 条 | **完成**（外观三选方块） | 与官方同交互形态 |
| 3 | HTML 预览 | **完成（无 WebView2）** | 源码 + 大纲 + 附属可点开 |
| 4 | PDF 密码 | **降级 UX 完成** | 明确「加密 PDF」+ 系统打开 + 复制路径 |
| 5 | P2-5 匿名 ID | **有意跳过** | — |

**构建**：`pwsh -File build.ps1 -Configuration Debug` → 0 error。

*生成说明：T28 单代理完成；对照源 `@deepseek-ai/dsh-client-ui-settings-plugins` / `settings-plugin-inventory` / `dsh-client-ui-theme` 的 zh 字典与控件结构；`rust/` 与 `Kernel/**/node_modules` 零改动。*
