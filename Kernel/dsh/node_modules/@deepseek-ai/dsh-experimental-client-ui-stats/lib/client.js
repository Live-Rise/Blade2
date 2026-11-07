window.__ModuleLoader__.load({
	id: "@deepseek-ai/dsh-experimental-client-ui-stats",
	factory: (require) => {
		var module = { exports: {} };
		var exports = module.exports;
		Object.defineProperty(exports, Symbol.toStringTag, { value: "Module" });
		let react_jsx_runtime = require("react/jsx-runtime");
		let react = require("react");
		let _deepseek_ai_dsh_client_ui_primitives = require("@deepseek-ai/dsh-client-ui-primitives");
		//#region lib/types/client/locales.js
		/** `stats` namespace dictionaries for the Statistics table. */
		/** Dictionary namespace owned by this plugin. */
		const NS = "stats";
		/** Simplified Chinese dictionary (the key-set source of truth). */
		const zh = {
			"view.stats": "统计",
			"column.scope": "轮次 / 步骤",
			"column.scope.title": "轮次行是该轮的合计；点一行展开或折叠它的每一步",
			"column.prompt": "触发",
			"column.prompt.title": "这一轮是由什么触发的；人写的提示词显示正文，自动触发显示来源标签",
			"trigger.agent-message": "智能体消息",
			"trigger.team-message": "团队消息",
			"trigger.schedule": "定时触发",
			"trigger.webhook": "Webhook",
			"trigger.tool-jobs": "后台任务",
			"trigger.cordis-host-runner": "插件触发",
			"trigger.subagent-settled": "子代理完成",
			"trigger.goal": "目标轮次",
			"trigger.context": "上下文唤醒",
			"trigger.other": "自动触发",
			"column.did": "执行内容",
			"column.did.title": "该步调用的工具与次数；没有工具调用的步显示回答正文的摘要",
			"column.context": "上下文",
			"column.context.title": "该步 usage 的输入词数 + 缓存读 + 缓存写",
			"column.output": "输出",
			"column.output.title": "该步 usage 的输出词数",
			"column.cache": "缓存命中",
			"column.cache.title": "缓存读 ÷（输入 + 缓存读 + 缓存写）：提示词里命中缓存的比例",
			"cache.detail": "缓存读 {read} · 输入 {input} · 缓存写 {write}",
			"column.ttft": "首 token",
			"column.ttft.title": "步开始到第一个输出片段（TTFT）",
			"column.model": "模型用时",
			"column.model.title": "步开始到收到完整回复（含首 token 延迟）",
			"column.tps": "输出速度",
			"column.tps.title": "输出词数 ÷ 生成秒数（TPS，tok/s）",
			"column.tool": "工具用时",
			"column.tool.title": "该步内每次工具调用到拿到结果的耗时之和；还没有结果的调用不计",
			"column.time": "总用时",
			"column.time.title": "总用时拆分：浅蓝 首 token、蓝 生成、橙 工具调用、灰 其余。条长按平方根刻度，步只和同一轮的步比，轮只和轮比",
			"turn.label": "第 {turn} 轮",
			"step.label": "第 {step} 步",
			"step.toggle.aria": "展开或折叠第 {step} 步的工具详情",
			"step.partial": "起止未知",
			"step.started.title": "步骤开始于 {time}",
			"compaction.label": "压缩",
			"compaction.running": "压缩中…",
			"compaction.shadowed": "压缩 {tokens} tokens 历史",
			"compaction.summary": "摘要 {chars} 字符",
			"compaction.title": "压缩调用单独展示，不并入轮次的步骤合计",
			"event.show": "显示事件行（含压缩）",
			"context.delta.show": "显示上下文增量",
			"context.delta.title": "相对同轮上一步的提示词增量；负值表示上下文减少（如压缩）",
			"event.request-header": "请求头",
			"event.system-prompt": "系统提示词",
			"event.tool-set": "工具集变更",
			"event.context-prune": "上下文裁剪",
			"event.request-context": "路由能力",
			"event.model-retry": "模型重试",
			"event.model-attempt": "模型尝试",
			"event.context-refresh": "上下文刷新",
			"event.approval": "审批",
			"event.context.detail": "{provider} / {model} · 提示词模式 {mode}",
			"event.retry.detail": "第 {retry} 次重试 · 等待 {delay}",
			"event.attempt.detail": "未提交消息的模型尝试",
			"event.refresh.detail": "{source} · {chars} 字符",
			"event.approval.detail": "{tool} · {status}",
			"event.source.runtime": "运行时上下文",
			"event.source.time": "时间上下文",
			"event.approval.asked": "等待审批",
			"event.approval.allowed": "允许一次",
			"event.approval.rejected": "拒绝",
			"event.approval.cancelled": "取消",
			"event.approval.unavailable": "不可用",
			"event.title": "非步骤事件行：解释请求、提示词、工具集、重试、上下文刷新、审批和裁剪变化，不计入轮次的步骤合计",
			"event.reason.initial": "初始",
			"event.reason.resume": "恢复",
			"event.reason.change": "变更",
			"event.reason.series": "新序列",
			"event.header.detail": "{reason} · {tools} 个工具",
			"event.series": "新序列",
			"event.system.append": "追加",
			"event.system.replace": "替换",
			"event.system.clear": "清空",
			"event.system.detail": "{action} · {chars} 字符",
			"event.tools.added": "新增 {tools}",
			"event.tools.removed": "移除 {tools}",
			"event.prune.detail": "裁剪 {tokens} tokens",
			"step.partial.title": "这一步的开始与结束事件不在当前事件窗口里（页签重连或窗口重建时被截断），首 token、模型用时、总用时都没有起点可以相减；重新加载页签会从完整窗口重新装配",
			"state.running": "进行中",
			"state.interrupted": "中断",
			"state.cacheMiss": "缓存未命中",
			"state.cacheMiss.title": "这一步的提示词几乎没命中缓存：进程重启、会话恢复后的第一次请求都是这样，输入按全价计",
			"state.aborted": "未完成",
			"state.aborted.title": "该步已经结束，但没有收到完整回复（多为被取消或失败）",
			"unknown": "未记录",
			"partial": "还有 {count} 次工具调用没有拿到结果，这里是已知耗时的下限",
			"empty": "当前窗口里还没有可以测量的步骤。",
			"loadOlder": "加载更早",
			"loadOlder.loading": "正在加载更早的轮次…",
			"loadOlder.done": "已经是最早的记录",
			"loadOlder.failed": "加载失败，重试",
			"summary": "{turns} 轮 · {steps} 步",
			"table.showTotals": "显示合计行",
			"contextMode.label": "上下文口径",
			"contextMode.latest": "最新",
			"contextMode.latest.title": "轮次行显示最后一步的提示词大小（最终上下文）；合计行始终为累计值",
			"contextMode.cumulative": "累计",
			"contextMode.cumulative.title": "轮次行显示该轮各步提示词大小之和（累计处理量）；合计行始终为累计值",
			"totals.full": "全量合计",
			"totals.full.title": "整段会话合计；总用时为模型用时 + 工具用时（不含跨轮空隙）",
			"totals.window": "本窗口合计",
			"totals.window.title": "当前已加载窗口的合计；上下文为各步累计值",
			"expand.all": "展开全部",
			"collapse.all": "折叠全部",
			"turn.expand": "展开",
			"turn.collapse": "折叠",
			"turn.toggle.title": "展开或折叠这一轮的每一步",
			"turn.toggle.aria": "展开或折叠第 {turn} 轮",
			"did.answer": "回答",
			"did.working": "生成中",
			"did.aborted": "中断",
			"did.more": "其他 {count} 项",
			"did.rest": "还有 {count} 次调用",
			"did.title": "共调用 {calls} 次：{tools}",
			"did.retry": "重试 {count} 次",
			"did.lines": "{lines} 行",
			"did.lines.unit": "行",
			"did.chars": "{chars} 字符",
			"did.output": "→ {chars} 字符",
			"did.total": "本步工具输出合计 {chars} 字符，会进入下一步的上下文",
			"detail.show": "详情",
			"detail.hide": "收起",
			"detail.title": "点这一行展开或收起工具详情",
			"detail.calls": "{count} 次工具调用",
			"detail.answer": "回答",
			"detail.answer.truncated": "回答过长，仅展示前 16,000 字符",
			"detail.code.title": "代码块",
			"detail.code.wrap": "自动换行",
			"detail.code.unwrap": "不换行",
			"detail.footnotes": "脚注",
			"detail.copy": "复制",
			"detail.copy.title": "复制这段正文",
			"detail.copied": "已复制",
			"detail.copyFailed": "复制失败",
			"detail.chars": "{chars} 字符",
			"detail.lines": "{lines} 行",
			"detail.file": "文件",
			"detail.diff": "改动对比",
			"detail.emptyBody": "这条调用没有留下正文",
			"detail.hidden": "还有 {count} 次调用没有列出（一步最多列 {limit} 次）",
			"detail.args": "原始参数",
			"detail.wrap": "换行",
			"detail.wrap.title": "长行换行显示，不出现横向滚动",
			"detail.raw.title": "查看原始 JSON 参数",
			"detail.lineCap": "只显示前 {shown} 行，共 {total} 行",
			"detail.truncated": "内容过长，这里只保留了前 {chars} 字符",
			"tool.failed": "失败 {count}",
			"tool.failed.title": "该步有 {count} 次工具调用返回错误结果",
			"bar.ttft": "首 token {value}",
			"bar.gen": "生成 {value}",
			"bar.tool": "工具调用 {value}",
			"bar.rest": "其余 {value}",
			"bar.wall": "总用时 {value}",
			"bar.running": "还没结束，这是已用时间",
			"bar.gap": "距上一步 {value}",
			"bar.start": "开始于 {clock}",
			"turn.started": "这一轮开始于 {time}",
			"turn.images": "{count} 张图",
			"imagePreview.hint": "悬停或聚焦预览这一轮的图片，点击查看大图",
			"imagePreview.dialog": "图片预览",
			"imagePreview.open": "查看第 {index} 张图（共 {count} 张）",
			"imagePreview.alt": "第 {index} 张图",
			"imagePreview.loading": "图片加载中…",
			"imagePreview.failed": "图片加载失败",
			"imagePreview.retry": "重试",
			"imagePreview.close": "关闭图片预览",
			"steps.total": "共 {total} 步",
			"steps.tail": "只看最新 {limit} 步",
			"steps.all": "显示全部 {total} 步",
			"steps.filter": "只看 {tool}",
			"steps.clear": "取消筛选",
			"steps.matched": "{count} 步用到 {tool}"
		};
		/** English dictionary, checked complete against the Chinese source of truth. */
		const en = {
			"view.stats": "Statistics",
			"column.scope": "Turn / step",
			"column.scope.title": "A turn row holds the turn total; click a row to expand or collapse its steps",
			"column.prompt": "Trigger",
			"column.prompt.title": "What opened this turn: a human prompt shows its text, automation shows a source badge",
			"trigger.agent-message": "Agent message",
			"trigger.team-message": "Team message",
			"trigger.schedule": "Scheduled",
			"trigger.webhook": "Webhook",
			"trigger.tool-jobs": "Background job",
			"trigger.cordis-host-runner": "Plugin run",
			"trigger.subagent-settled": "Subagent finished",
			"trigger.goal": "Goal round",
			"trigger.context": "Context wake",
			"trigger.other": "Automated turn",
			"column.did": "Execution",
			"column.did.title": "Tools this step called and how often; a step without tool calls previews its answer text",
			"column.context": "Context",
			"column.context.title": "usage input tokens + cache read + cache write",
			"column.output": "Output",
			"column.output.title": "usage output tokens of the step",
			"column.cache": "Cache hit",
			"column.cache.title": "cache read ÷ (input + cache read + cache write): the share of the prompt served from cache",
			"cache.detail": "cache read {read} · input {input} · cache write {write}",
			"column.ttft": "First token",
			"column.ttft.title": "step start to the first output fragment (TTFT)",
			"column.model": "Model time",
			"column.model.title": "step start to the settled reply (includes the first-token delay)",
			"column.tps": "Output speed",
			"column.tps.title": "output tokens ÷ generation seconds (TPS, tok/s)",
			"column.tool": "Tool time",
			"column.tool.title": "sum of every tool call up to its result inside the step; calls without a result are excluded",
			"column.time": "Total time",
			"column.time.title": "total split: light blue first token, blue generation, orange tool calls, gray everything else. Bar length uses a square-root scale; steps compare only with steps of the same turn, turns with turns",
			"turn.label": "Turn {turn}",
			"step.label": "Step {step}",
			"step.toggle.aria": "Expand or collapse the tool details of step {step}",
			"step.partial": "start unknown",
			"step.started.title": "Step started at {time}",
			"compaction.label": "Compaction",
			"compaction.running": "Compacting…",
			"compaction.shadowed": "Pruned {tokens} tokens of history",
			"compaction.summary": "Summary {chars} chars",
			"compaction.title": "Compaction runs are shown separately and excluded from the turn's step totals",
			"event.show": "Show event rows (incl. compaction)",
			"context.delta.show": "Show context delta",
			"context.delta.title": "Prompt delta against the previous measured step in the turn; negative means the context shrank (for example after compaction)",
			"event.request-header": "Request header",
			"event.system-prompt": "System prompt",
			"event.tool-set": "Tool set change",
			"event.context-prune": "Context prune",
			"event.request-context": "Request context",
			"event.model-retry": "Model retry",
			"event.model-attempt": "Model attempt",
			"event.context-refresh": "Context refresh",
			"event.approval": "Approval",
			"event.context.detail": "{provider} / {model} · prompt mode {mode}",
			"event.retry.detail": "retry {retry} · wait {delay}",
			"event.attempt.detail": "Model attempt without a committed message",
			"event.refresh.detail": "{source} · {chars} chars",
			"event.approval.detail": "{tool} · {status}",
			"event.source.runtime": "runtime context",
			"event.source.time": "time context",
			"event.approval.asked": "waiting",
			"event.approval.allowed": "allowed once",
			"event.approval.rejected": "rejected",
			"event.approval.cancelled": "cancelled",
			"event.approval.unavailable": "unavailable",
			"event.title": "Non-step event rows: request, prompt, tool-set, retry, context-refresh, approval, and prune changes excluded from the turn step totals",
			"event.reason.initial": "initial",
			"event.reason.resume": "resume",
			"event.reason.change": "change",
			"event.reason.series": "new series",
			"event.header.detail": "{reason} · {tools} tools",
			"event.series": "new series",
			"event.system.append": "append",
			"event.system.replace": "replace",
			"event.system.clear": "clear",
			"event.system.detail": "{action} · {chars} chars",
			"event.tools.added": "added {tools}",
			"event.tools.removed": "removed {tools}",
			"event.prune.detail": "pruned {tokens} tokens",
			"step.partial.title": "This step's start and end events are not in the current event window (trimmed while the tab reconnected or the window was rebuilt), so first-token, model time and total have no start to subtract from; reloading the tab reassembles from the full window",
			"state.running": "running",
			"state.interrupted": "interrupted",
			"state.cacheMiss": "cache miss",
			"state.cacheMiss.title": "The prompt barely hit the cache: usually the first request after a restart or a session resume, paid at full input price",
			"state.aborted": "unsettled",
			"state.aborted.title": "The step closed without a settled reply (usually cancelled or failed)",
			"unknown": "not recorded",
			"partial": "{count} tool calls have no result yet; this is the lower bound of known time",
			"empty": "No measurable step in the current window.",
			"loadOlder": "Load earlier",
			"loadOlder.loading": "Loading earlier turns…",
			"loadOlder.done": "Beginning of history reached",
			"loadOlder.failed": "Load failed, retry",
			"summary": "{turns} turns · {steps} steps",
			"table.showTotals": "Show total row",
			"contextMode.label": "Context basis",
			"contextMode.latest": "Latest",
			"contextMode.latest.title": "Turn rows show the last measured step's prompt size (final context); the total row stays cumulative",
			"contextMode.cumulative": "Cumulative",
			"contextMode.cumulative.title": "Turn rows show the sum of prompt sizes across the turn's steps (cumulative processing); the total row stays cumulative",
			"totals.full": "Whole-session total",
			"totals.full.title": "Totals for the complete session; total time is model time + tool time (no inter-turn gaps)",
			"totals.window": "Window total",
			"totals.window.title": "Totals for the turns loaded in this window; the context cell is cumulative across steps",
			"expand.all": "Expand all",
			"collapse.all": "Collapse all",
			"turn.expand": "Show",
			"turn.collapse": "Hide",
			"turn.toggle.title": "Expand or collapse the steps of this turn",
			"turn.toggle.aria": "Expand or collapse turn {turn}",
			"did.answer": "answer",
			"did.working": "generating",
			"did.aborted": "no answer",
			"did.more": "{count} more",
			"did.rest": "{count} more calls",
			"did.title": "{calls} calls: {tools}",
			"did.retry": "retried {count}×",
			"did.lines": "{lines} lines",
			"did.lines.unit": "lines",
			"did.chars": "{chars} chars",
			"did.output": "→ {chars} chars",
			"did.total": "{chars} characters of tool output in this step; they become part of the next prompt",
			"detail.show": "details",
			"detail.hide": "hide",
			"detail.title": "Click this row to expand or collapse the tool details",
			"detail.calls": "{count} tool calls",
			"detail.answer": "Answer",
			"detail.answer.truncated": "Answer is long; only the first 16,000 chars are kept",
			"detail.code.title": "Code block",
			"detail.code.wrap": "Wrap lines",
			"detail.code.unwrap": "Do not wrap",
			"detail.footnotes": "Footnotes",
			"detail.copy": "Copy",
			"detail.copy.title": "Copy this text",
			"detail.copied": "Copied",
			"detail.copyFailed": "Copy failed",
			"detail.chars": "{chars} chars",
			"detail.lines": "{lines} lines",
			"detail.file": "file",
			"detail.diff": "change",
			"detail.emptyBody": "This call left no text",
			"detail.hidden": "{count} more calls are not listed (at most {limit} per step)",
			"detail.args": "raw arguments",
			"detail.wrap": "Wrap",
			"detail.wrap.title": "Wrap long lines instead of scrolling sideways",
			"detail.raw.title": "Show the raw JSON arguments",
			"detail.lineCap": "Showing the first {shown} of {total} lines",
			"detail.truncated": "Too long: only the first {chars} characters are kept",
			"tool.failed": "{count} failed",
			"tool.failed.title": "{count} tool calls in this step returned an error result",
			"bar.ttft": "first token {value}",
			"bar.gen": "generation {value}",
			"bar.tool": "tool calls {value}",
			"bar.rest": "other {value}",
			"bar.wall": "total {value}",
			"bar.running": "not finished yet: this is elapsed time",
			"bar.gap": "gap since previous step {value}",
			"bar.start": "started {clock}",
			"turn.started": "this turn started at {time}",
			"turn.images": "{count} images",
			"imagePreview.hint": "Hover or focus to preview this turn's images; click to open the full image",
			"imagePreview.dialog": "Image preview",
			"imagePreview.open": "View image {index} of {count}",
			"imagePreview.alt": "Image {index}",
			"imagePreview.loading": "Loading image…",
			"imagePreview.failed": "Image failed to load",
			"imagePreview.retry": "Retry",
			"imagePreview.close": "Close image preview",
			"steps.total": "{total} steps",
			"steps.tail": "newest {limit} only",
			"steps.all": "Show all {total} steps",
			"steps.filter": "Only {tool}",
			"steps.clear": "Clear filter",
			"steps.matched": "{count} steps used {tool}"
		};
		/** Japanese dictionary, checked complete against the Chinese source of truth. */
		const ja = {
			"view.stats": "統計",
			"column.scope": "ターン / ステップ",
			"column.scope.title": "ターン行はそのターンの合計です。行をクリックすると各ステップを展開または折りたたみます",
			"column.prompt": "トリガー",
			"column.prompt.title": "このターンを開始したものを表示します。人間のプロンプトは本文、自動トリガーはタグを表示します",
			"trigger.agent-message": "エージェントメッセージ",
			"trigger.team-message": "チームメッセージ",
			"trigger.schedule": "スケジュール実行",
			"trigger.webhook": "Webhook",
			"trigger.tool-jobs": "バックグラウンドジョブ",
			"trigger.cordis-host-runner": "プラグイン実行",
			"trigger.subagent-settled": "サブエージェント完了",
			"trigger.goal": "ゴールラウンド",
			"trigger.context": "コンテキストによる再開",
			"trigger.other": "自動ターン",
			"column.did": "実行内容",
			"column.did.title": "このステップが呼び出したツールと回数です。ツール呼び出しのないステップはモデルの回答、または未完了です",
			"column.context": "コンテキスト",
			"column.context.title": "そのステップの usage の入力トークン + キャッシュ読み込み + キャッシュ書き込み",
			"column.output": "出力",
			"column.output.title": "そのステップの usage の出力トークン数",
			"column.cache": "キャッシュヒット",
			"column.cache.title": "キャッシュ読み込み ÷（入力 + キャッシュ読み込み + キャッシュ書き込み）。プロンプトのうちキャッシュから返された割合です",
			"cache.detail": "キャッシュ読み込み {read} · 入力 {input} · キャッシュ書き込み {write}",
			"column.ttft": "最初のトークン",
			"column.ttft.title": "ステップ開始から最初の出力断片まで（TTFT）",
			"column.model": "モデル時間",
			"column.model.title": "ステップ開始から応答完了まで（最初のトークン遅延を含む）",
			"column.tps": "出力速度",
			"column.tps.title": "出力トークン ÷ 生成秒数（TPS、tok/s）",
			"column.tool": "ツール時間",
			"column.tool.title": "ステップ内の各ツール呼び出しから結果受信までの時間の合計。結果のない呼び出しは含みません",
			"column.time": "合計時間",
			"column.time.title": "合計の内訳。水色は最初のトークン、青は生成、オレンジはツール呼び出し、グレーはそれ以外。バー長は平方根スケールで、ステップは同じターンのステップとのみ、ターンはターン同士で比較します",
			"turn.label": "ターン {turn}",
			"step.label": "ステップ {step}",
			"step.toggle.aria": "ステップ {step} のツール詳細を展開または折りたたみます",
			"step.partial": "開始時刻不明",
			"step.started.title": "ステップ開始 {time}",
			"compaction.label": "圧縮",
			"compaction.running": "圧縮中…",
			"compaction.shadowed": "履歴 {tokens} tokens を圧縮",
			"compaction.summary": "要約 {chars} 文字",
			"compaction.title": "圧縮呼び出しはターンのステップ合計とは別に表示されます",
			"event.show": "イベント行を表示（圧縮を含む）",
			"context.delta.show": "コンテキスト増分を表示",
			"context.delta.title": "同じターンの前ステップとのプロンプト増分。負の値はコンテキストの減少（圧縮など）を表します",
			"event.request-header": "リクエストヘッダー",
			"event.system-prompt": "システムプロンプト",
			"event.tool-set": "ツールセット変更",
			"event.context-prune": "コンテキスト削除",
			"event.request-context": "ルーティング能力",
			"event.model-retry": "モデル再試行",
			"event.model-attempt": "モデル試行",
			"event.context-refresh": "コンテキスト更新",
			"event.approval": "承認",
			"event.context.detail": "{provider} / {model} · プロンプトモード {mode}",
			"event.retry.detail": "再試行 {retry} · 待機 {delay}",
			"event.attempt.detail": "メッセージを確定しなかったモデル試行",
			"event.refresh.detail": "{source} · {chars} 文字",
			"event.approval.detail": "{tool} · {status}",
			"event.source.runtime": "ランタイムコンテキスト",
			"event.source.time": "時間コンテキスト",
			"event.approval.asked": "承認待ち",
			"event.approval.allowed": "1 回許可",
			"event.approval.rejected": "拒否",
			"event.approval.cancelled": "キャンセル",
			"event.approval.unavailable": "利用不可",
			"event.title": "非ステップのイベント行：リクエスト・プロンプト・ツールセット・再試行・コンテキスト更新・承認・削除の変化を説明し、ターンのステップ合計には含めません",
			"event.reason.initial": "初期",
			"event.reason.resume": "再開",
			"event.reason.change": "変更",
			"event.reason.series": "新シリーズ",
			"event.header.detail": "{reason} · {tools} ツール",
			"event.series": "新シリーズ",
			"event.system.append": "追加",
			"event.system.replace": "置換",
			"event.system.clear": "クリア",
			"event.system.detail": "{action} · {chars} 文字",
			"event.tools.added": "追加 {tools}",
			"event.tools.removed": "削除 {tools}",
			"event.prune.detail": "{tokens} tokens を削除",
			"step.partial.title": "このステップの開始・終了イベントが現在のイベントウィンドウにありません（タブ再接続やウィンドウ再構築で切り詰められました）。最初のトークン、モデル時間、合計時間には差し引く開始時刻がありません。タブを再読み込みすると完全なウィンドウから再構築されます",
			"state.running": "実行中",
			"state.interrupted": "中断",
			"state.cacheMiss": "キャッシュミス",
			"state.cacheMiss.title": "このステップのプロンプトはほとんどキャッシュにヒットしていません。再起動後やセッション再開後の最初のリクエストでは通常こうなり、入力は正規料金で課金されます",
			"state.aborted": "未完了",
			"state.aborted.title": "ステップは終了しましたが完全な応答を受信していません（多くはキャンセルか失敗）",
			"unknown": "未記録",
			"partial": "結果のないツール呼び出しが {count} 件あります。これは既知時間の下限です",
			"empty": "現在のウィンドウには計測可能なステップがありません。",
			"loadOlder": "以前を読み込む",
			"loadOlder.loading": "以前のターンを読み込み中…",
			"loadOlder.done": "履歴の先頭に到達しました",
			"loadOlder.failed": "読み込みに失敗しました。再試行",
			"summary": "{turns} ターン · {steps} ステップ",
			"table.showTotals": "合計行を表示",
			"contextMode.label": "コンテキスト基準",
			"contextMode.latest": "最新",
			"contextMode.latest.title": "ターン行は最後に測定したステップのプロンプトサイズ（最終コンテキスト）。合計行は常に累計",
			"contextMode.cumulative": "累計",
			"contextMode.cumulative.title": "ターン行はターン内の各ステップのプロンプトサイズの合計（累計処理量）。合計行は常に累計",
			"totals.full": "全体合計",
			"totals.full.title": "セッション全体の合計。合計時間はモデル時間 + ツール時間（ターン間の空白を含みません）",
			"totals.window": "ウィンドウ合計",
			"totals.window.title": "読み込み済みターンの合計。コンテキストは各ステップの累計",
			"expand.all": "すべて展開",
			"collapse.all": "すべて折りたたむ",
			"turn.expand": "展開",
			"turn.collapse": "折りたたむ",
			"turn.toggle.title": "このターンの各ステップを展開または折りたたみます",
			"turn.toggle.aria": "ターン {turn} を展開または折りたたみます",
			"did.answer": "回答",
			"did.working": "生成中",
			"did.aborted": "中断",
			"did.more": "他 {count} 件",
			"did.rest": "さらに {count} 件の呼び出し",
			"did.title": "{calls} 回の呼び出し: {tools}",
			"did.retry": "{count} 回再試行",
			"did.lines": "{lines} 行",
			"did.lines.unit": "行",
			"did.chars": "{chars} 文字",
			"did.output": "→ {chars} 文字",
			"did.total": "このステップのツール出力は合計 {chars} 文字で、次のプロンプトの一部になります",
			"detail.show": "詳細",
			"detail.hide": "折りたたむ",
			"detail.title": "この行をクリックしてツール詳細を展開または折りたたみます",
			"detail.calls": "{count} 回のツール呼び出し",
			"detail.answer": "回答",
			"detail.answer.truncated": "回答が長いため、先頭 16,000 文字のみ保持しています",
			"detail.code.title": "コードブロック",
			"detail.code.wrap": "折り返す",
			"detail.code.unwrap": "折り返さない",
			"detail.footnotes": "脚注",
			"detail.copy": "コピー",
			"detail.copy.title": "この本文をコピー",
			"detail.copied": "コピーしました",
			"detail.copyFailed": "コピーに失敗しました",
			"detail.chars": "{chars} 文字",
			"detail.lines": "{lines} 行",
			"detail.file": "ファイル",
			"detail.diff": "差分",
			"detail.emptyBody": "この呼び出しには本文がありません",
			"detail.hidden": "未表示の呼び出しが {count} 件あります（1 ステップあたり最大 {limit} 件）",
			"detail.args": "生の引数",
			"detail.wrap": "折り返し",
			"detail.wrap.title": "長い行を折り返し、横スクロールを出しません",
			"detail.raw.title": "生の JSON 引数を表示",
			"detail.lineCap": "全 {total} 行のうち先頭 {shown} 行を表示",
			"detail.truncated": "長すぎるため、先頭 {chars} 文字のみ保持しています",
			"tool.failed": "{count} 失敗",
			"tool.failed.title": "このステップで {count} 件のツール呼び出しがエラー結果を返しました",
			"bar.ttft": "最初のトークン {value}",
			"bar.gen": "生成 {value}",
			"bar.tool": "ツール呼び出し {value}",
			"bar.rest": "その他 {value}",
			"bar.wall": "合計 {value}",
			"bar.running": "未完了のため、これは経過時間です",
			"bar.gap": "前のステップから {value}",
			"bar.start": "{clock} に開始",
			"turn.started": "このターンは {time} に開始",
			"turn.images": "{count} 枚の画像",
			"imagePreview.hint": "ホバーまたはフォーカスでこのターンの画像をプレビューし、クリックで原寸表示します",
			"imagePreview.dialog": "画像プレビュー",
			"imagePreview.open": "画像 {index} / {count} を表示",
			"imagePreview.alt": "画像 {index}",
			"imagePreview.loading": "画像を読み込み中…",
			"imagePreview.failed": "画像を読み込めませんでした",
			"imagePreview.retry": "再試行",
			"imagePreview.close": "画像プレビューを閉じる",
			"steps.total": "全 {total} ステップ",
			"steps.tail": "最新 {limit} ステップのみ",
			"steps.all": "全 {total} ステップを表示",
			"steps.filter": "{tool} のみ",
			"steps.clear": "絞り込みを解除",
			"steps.matched": "{count} ステップが {tool} を使用"
		};
		/** Spanish dictionary. */
		const es = {
			"view.stats": "Estadísticas",
			"column.scope": "Turno / paso",
			"column.scope.title": "La fila de un turno contiene su total; haz clic en una fila para expandir o contraer sus pasos",
			"column.prompt": "Disparador",
			"column.prompt.title": "Qué abrió este turno: un prompt humano muestra su texto y una automatización muestra una etiqueta de origen",
			"trigger.agent-message": "Mensaje del agente",
			"trigger.team-message": "Mensaje del equipo",
			"trigger.schedule": "Programado",
			"trigger.webhook": "Webhook",
			"trigger.tool-jobs": "Tarea en segundo plano",
			"trigger.cordis-host-runner": "Ejecución de plugin",
			"trigger.subagent-settled": "Subagente finalizado",
			"trigger.goal": "Ronda de objetivo",
			"trigger.context": "Reactivación por contexto",
			"trigger.other": "Turno automatizado",
			"column.did": "Ejecución",
			"column.did.title": "Herramientas que llamó este paso y con qué frecuencia; un paso sin llamadas a herramientas muestra un extracto del texto de su respuesta",
			"column.context": "Contexto",
			"column.context.title": "tokens de entrada de usage + lectura de caché + escritura de caché",
			"column.output": "Salida",
			"column.output.title": "tokens de salida de usage en el paso",
			"column.cache": "Acierto de caché",
			"column.cache.title": "lectura de caché ÷ (entrada + lectura de caché + escritura de caché): la proporción del prompt servida desde la caché",
			"cache.detail": "lectura de caché {read} · entrada {input} · escritura de caché {write}",
			"column.ttft": "Primer token",
			"column.ttft.title": "inicio del paso hasta el primer fragmento de salida (TTFT)",
			"column.model": "Tiempo del modelo",
			"column.model.title": "inicio del paso hasta la respuesta completa (incluye la latencia del primer token)",
			"column.tps": "Velocidad de salida",
			"column.tps.title": "tokens de salida ÷ segundos de generación (TPS, tok/s)",
			"column.tool": "Tiempo de herramientas",
			"column.tool.title": "suma del tiempo de cada llamada a herramienta hasta su resultado dentro del paso; las llamadas sin resultado no cuentan",
			"column.time": "Tiempo total",
			"column.time.title": "desglose del total: azul claro el primer token, azul la generación, naranja las llamadas a herramientas y gris el resto. La longitud de la barra usa una escala de raíz cuadrada; los pasos solo se comparan con pasos del mismo turno y los turnos entre sí",
			"turn.label": "Turno {turn}",
			"step.label": "Paso {step}",
			"step.toggle.aria": "Expandir o contraer los detalles de herramientas del paso {step}",
			"step.partial": "inicio desconocido",
			"step.started.title": "El paso comenzó a las {time}",
			"compaction.label": "Compactación",
			"compaction.running": "Compactando…",
			"compaction.shadowed": "Se recortaron {tokens} tokens del historial",
			"compaction.summary": "Resumen de {chars} caracteres",
			"compaction.title": "Las ejecuciones de compactación se muestran por separado y quedan fuera de los totales de pasos del turno",
			"event.show": "Mostrar filas de eventos (incl. compactación)",
			"context.delta.show": "Mostrar delta de contexto",
			"context.delta.title": "Delta del prompt respecto al paso medido anterior del turno; un valor negativo indica que el contexto se redujo (por ejemplo, tras una compactación)",
			"event.request-header": "Encabezado de la solicitud",
			"event.system-prompt": "Prompt del sistema",
			"event.tool-set": "Cambio del conjunto de herramientas",
			"event.context-prune": "Recorte de contexto",
			"event.request-context": "Contexto de la solicitud",
			"event.model-retry": "Reintento de modelo",
			"event.model-attempt": "Intento de modelo",
			"event.context-refresh": "Actualización de contexto",
			"event.approval": "Aprobación",
			"event.context.detail": "{provider} / {model} · modo de prompt {mode}",
			"event.retry.detail": "reintento {retry} · espera {delay}",
			"event.attempt.detail": "Intento de modelo sin mensaje confirmado",
			"event.refresh.detail": "{source} · {chars} caracteres",
			"event.approval.detail": "{tool} · {status}",
			"event.source.runtime": "contexto de ejecución",
			"event.source.time": "contexto de tiempo",
			"event.approval.asked": "en espera",
			"event.approval.allowed": "permitido una vez",
			"event.approval.rejected": "rechazado",
			"event.approval.cancelled": "cancelado",
			"event.approval.unavailable": "no disponible",
			"event.title": "Filas de eventos que no son pasos: cambios de solicitud, prompt, conjunto de herramientas, reintento, actualización de contexto, aprobación y recorte, que quedan fuera de los totales de pasos del turno",
			"event.reason.initial": "inicial",
			"event.reason.resume": "reanudación",
			"event.reason.change": "cambio",
			"event.reason.series": "nueva serie",
			"event.header.detail": "{reason} · {tools} herramientas",
			"event.series": "nueva serie",
			"event.system.append": "añadido",
			"event.system.replace": "reemplazado",
			"event.system.clear": "vaciado",
			"event.system.detail": "{action} · {chars} caracteres",
			"event.tools.added": "añadidas {tools}",
			"event.tools.removed": "eliminadas {tools}",
			"event.prune.detail": "recortados {tokens} tokens",
			"step.partial.title": "Los eventos de inicio y fin de este paso no están en la ventana de eventos actual (se recortaron al reconectar la pestaña o reconstruir la ventana), así que no hay un inicio desde el que calcular el primer token, el tiempo de modelo ni el tiempo total; al recargar la pestaña se reconstruye desde la ventana completa",
			"state.running": "en curso",
			"state.interrupted": "interrumpido",
			"state.cacheMiss": "fallo de caché",
			"state.cacheMiss.title": "El prompt apenas acertó en la caché: normalmente es la primera solicitud tras un reinicio o una reanudación de sesión, y su entrada se cobra a precio completo",
			"state.aborted": "sin concluir",
			"state.aborted.title": "El paso terminó sin una respuesta completa (normalmente cancelado o fallido)",
			"unknown": "sin registro",
			"partial": "{count} llamadas a herramientas aún no tienen resultado; este es el límite inferior del tiempo conocido",
			"empty": "No hay pasos medibles en la ventana actual.",
			"loadOlder": "Cargar anteriores",
			"loadOlder.loading": "Cargando turnos anteriores…",
			"loadOlder.done": "Se alcanzó el inicio del historial",
			"loadOlder.failed": "Error al cargar, reintentar",
			"summary": "{turns} turnos · {steps} pasos",
			"table.showTotals": "Mostrar fila de totales",
			"contextMode.label": "Base del contexto",
			"contextMode.latest": "Último",
			"contextMode.latest.title": "Las filas de cada turno muestran el tamaño de prompt del último paso medido (contexto final); la fila de totales sigue siendo acumulada",
			"contextMode.cumulative": "Acumulado",
			"contextMode.cumulative.title": "Las filas de cada turno muestran la suma de los tamaños de prompt de los pasos del turno (procesamiento acumulado); la fila de totales sigue siendo acumulada",
			"totals.full": "Total de toda la sesión",
			"totals.full.title": "Totales de la sesión completa; el tiempo total es tiempo de modelo + tiempo de herramientas (sin huecos entre turnos)",
			"totals.window": "Total de la ventana",
			"totals.window.title": "Totales de los turnos cargados en esta ventana; la celda de contexto es acumulada entre pasos",
			"expand.all": "Expandir todo",
			"collapse.all": "Contraer todo",
			"turn.expand": "Mostrar",
			"turn.collapse": "Ocultar",
			"turn.toggle.title": "Expandir o contraer los pasos de este turno",
			"turn.toggle.aria": "Expandir o contraer el turno {turn}",
			"did.answer": "respuesta",
			"did.working": "generando",
			"did.aborted": "sin respuesta",
			"did.more": "{count} más",
			"did.rest": "{count} llamadas más",
			"did.title": "{calls} llamadas: {tools}",
			"did.retry": "reintentado {count}×",
			"did.lines": "{lines} líneas",
			"did.lines.unit": "líneas",
			"did.chars": "{chars} caracteres",
			"did.output": "→ {chars} caracteres",
			"did.total": "{chars} caracteres de salida de herramientas en este paso; pasan a formar parte del siguiente prompt",
			"detail.show": "detalles",
			"detail.hide": "ocultar",
			"detail.title": "Haz clic en esta fila para expandir o contraer los detalles de herramientas",
			"detail.calls": "{count} llamadas a herramientas",
			"detail.answer": "Respuesta",
			"detail.answer.truncated": "La respuesta es larga; solo se conservan los primeros 16.000 caracteres",
			"detail.code.title": "Bloque de código",
			"detail.code.wrap": "Ajustar líneas",
			"detail.code.unwrap": "No ajustar",
			"detail.footnotes": "Notas al pie",
			"detail.copy": "Copiar",
			"detail.copy.title": "Copiar este texto",
			"detail.copied": "Copiado",
			"detail.copyFailed": "Error al copiar",
			"detail.chars": "{chars} caracteres",
			"detail.lines": "{lines} líneas",
			"detail.file": "archivo",
			"detail.diff": "cambio",
			"detail.emptyBody": "Esta llamada no dejó texto",
			"detail.hidden": "Hay {count} llamadas más sin listar (máximo {limit} por paso)",
			"detail.args": "argumentos originales",
			"detail.wrap": "Ajustar",
			"detail.wrap.title": "Ajusta las líneas largas en lugar de desplazarte en horizontal",
			"detail.raw.title": "Mostrar los argumentos JSON originales",
			"detail.lineCap": "Mostrando las primeras {shown} de {total} líneas",
			"detail.truncated": "Demasiado largo: solo se conservan los primeros {chars} caracteres",
			"tool.failed": "{count} con error",
			"tool.failed.title": "{count} llamadas a herramientas de este paso devolvieron un error",
			"bar.ttft": "primer token {value}",
			"bar.gen": "generación {value}",
			"bar.tool": "llamadas a herramientas {value}",
			"bar.rest": "resto {value}",
			"bar.wall": "total {value}",
			"bar.running": "aún sin terminar: este es el tiempo transcurrido",
			"bar.gap": "intervalo desde el paso anterior {value}",
			"bar.start": "inicio {clock}",
			"turn.started": "este turno comenzó a las {time}",
			"turn.images": "{count} imágenes",
			"imagePreview.hint": "Pasa el cursor o enfoca para previsualizar las imágenes de este turno; haz clic para abrir la imagen completa",
			"imagePreview.dialog": "Vista previa de imagen",
			"imagePreview.open": "Ver imagen {index} de {count}",
			"imagePreview.alt": "Imagen {index}",
			"imagePreview.loading": "Cargando imagen…",
			"imagePreview.failed": "No se pudo cargar la imagen",
			"imagePreview.retry": "Reintentar",
			"imagePreview.close": "Cerrar vista previa de imagen",
			"steps.total": "{total} pasos",
			"steps.tail": "solo los {limit} más recientes",
			"steps.all": "Mostrar los {total} pasos",
			"steps.filter": "Solo {tool}",
			"steps.clear": "Quitar filtro",
			"steps.matched": "{count} pasos usaron {tool}"
		};
		/** French dictionary. */
		const fr = {
			"view.stats": "Statistiques",
			"column.scope": "Tour / étape",
			"column.scope.title": "Une ligne de tour contient le total du tour\xA0; cliquez sur une ligne pour développer ou réduire ses étapes",
			"column.prompt": "Déclencheur",
			"column.prompt.title": "Ce qui a ouvert ce tour\xA0: une invite humaine affiche son texte, une automatisation affiche un badge de source",
			"trigger.agent-message": "Message de l'agent",
			"trigger.team-message": "Message d'équipe",
			"trigger.schedule": "Planifié",
			"trigger.webhook": "Webhook",
			"trigger.tool-jobs": "Tâche en arrière-plan",
			"trigger.cordis-host-runner": "Exécution de plugin",
			"trigger.subagent-settled": "Sous-agent terminé",
			"trigger.goal": "Tour d'objectif",
			"trigger.context": "Réveil du contexte",
			"trigger.other": "Tour automatique",
			"column.did": "Exécution",
			"column.did.title": "Outils appelés par cette étape et leur nombre\xA0; une étape sans appel d'outil affiche un aperçu du texte de sa réponse",
			"column.context": "Contexte",
			"column.context.title": "jetons d'entrée usage + lecture du cache + écriture du cache",
			"column.output": "Sortie",
			"column.output.title": "jetons de sortie usage de l'étape",
			"column.cache": "Taux de cache",
			"column.cache.title": "lecture du cache ÷ (entrée + lecture du cache + écriture du cache)\xA0: part de l'invite servie depuis le cache",
			"cache.detail": "lecture du cache {read} · entrée {input} · écriture du cache {write}",
			"column.ttft": "Premier jeton",
			"column.ttft.title": "début de l'étape jusqu'au premier fragment de sortie (TTFT)",
			"column.model": "Temps du modèle",
			"column.model.title": "début de l'étape jusqu'à la réponse finalisée (inclut le délai du premier jeton)",
			"column.tps": "Vitesse de sortie",
			"column.tps.title": "jetons de sortie ÷ secondes de génération (TPS, tok/s)",
			"column.tool": "Temps des outils",
			"column.tool.title": "somme des durées de chaque appel d'outil de l'étape jusqu'à son résultat\xA0; les appels sans résultat sont exclus",
			"column.time": "Temps total",
			"column.time.title": "répartition du total\xA0: bleu clair premier jeton, bleu génération, orange appels d'outils, gris tout le reste. La longueur de la barre suit une échelle de racine carrée\xA0; les étapes ne se comparent qu'aux étapes du même tour, les tours entre eux",
			"turn.label": "Tour {turn}",
			"step.label": "Étape {step}",
			"step.toggle.aria": "Développer ou réduire les détails des outils de l'étape {step}",
			"step.partial": "début inconnu",
			"step.started.title": "Étape commencée à {time}",
			"compaction.label": "Compactage",
			"compaction.running": "Compactage…",
			"compaction.shadowed": "{tokens} jetons d'historique retirés",
			"compaction.summary": "Résumé de {chars} caractères",
			"compaction.title": "Les exécutions de compactage sont affichées séparément et exclues des totaux d'étapes du tour",
			"event.show": "Afficher les lignes d'événements (y compris le compactage)",
			"context.delta.show": "Afficher le delta de contexte",
			"context.delta.title": "Écart de l'invite par rapport à l'étape mesurée précédente du tour\xA0; une valeur négative signifie que le contexte a diminué (par exemple après un compactage)",
			"event.request-header": "En-tête de demande",
			"event.system-prompt": "Invite système",
			"event.tool-set": "Changement de jeu d'outils",
			"event.context-prune": "Élagage du contexte",
			"event.request-context": "Contexte de la demande",
			"event.model-retry": "Nouvelle tentative du modèle",
			"event.model-attempt": "Tentative du modèle",
			"event.context-refresh": "Rafraîchissement du contexte",
			"event.approval": "Approbation",
			"event.context.detail": "{provider} / {model} · mode d'invite {mode}",
			"event.retry.detail": "réessai {retry} · attente {delay}",
			"event.attempt.detail": "Tentative du modèle sans message validé",
			"event.refresh.detail": "{source} · {chars} caractères",
			"event.approval.detail": "{tool} · {status}",
			"event.source.runtime": "contexte d'exécution",
			"event.source.time": "contexte temporel",
			"event.approval.asked": "en attente",
			"event.approval.allowed": "autorisé une fois",
			"event.approval.rejected": "refusé",
			"event.approval.cancelled": "annulé",
			"event.approval.unavailable": "indisponible",
			"event.title": "Lignes d'événements hors étapes (demande, invite, jeu d'outils, réessai, rafraîchissement du contexte, approbation et élagage), exclues des totaux d'étapes du tour",
			"event.reason.initial": "initial",
			"event.reason.resume": "reprise",
			"event.reason.change": "changement",
			"event.reason.series": "nouvelle série",
			"event.header.detail": "{reason} · {tools} outils",
			"event.series": "nouvelle série",
			"event.system.append": "ajout",
			"event.system.replace": "remplacement",
			"event.system.clear": "effacement",
			"event.system.detail": "{action} · {chars} caractères",
			"event.tools.added": "ajout de {tools}",
			"event.tools.removed": "retrait de {tools}",
			"event.prune.detail": "{tokens} jetons élagués",
			"step.partial.title": "Les événements de début et de fin de cette étape ne figurent pas dans la fenêtre d'événements actuelle (tronquée lors de la reconnexion de l'onglet ou de la reconstruction de la fenêtre)\xA0; le premier jeton, le temps du modèle et le temps total n'ont donc pas de début à soustraire. Rechargez l'onglet pour reconstituer la fenêtre complète.",
			"state.running": "en cours",
			"state.interrupted": "interrompu",
			"state.cacheMiss": "échec de cache",
			"state.cacheMiss.title": "L'invite n'a pratiquement pas été servie par le cache\xA0: généralement la première requête après un redémarrage ou une reprise de session, facturée au plein tarif des jetons d'entrée",
			"state.aborted": "non finalisé",
			"state.aborted.title": "L'étape s'est terminée sans réponse finalisée (généralement annulée ou en échec)",
			"unknown": "non enregistré",
			"partial": "{count} appels d'outils n'ont pas encore de résultat\xA0; il s'agit de la borne inférieure du temps connu",
			"empty": "Aucune étape mesurable dans la fenêtre actuelle.",
			"loadOlder": "Charger plus tôt",
			"loadOlder.loading": "Chargement des tours précédents…",
			"loadOlder.done": "Début de l'historique atteint",
			"loadOlder.failed": "Échec du chargement, réessayer",
			"summary": "{turns} tours · {steps} étapes",
			"table.showTotals": "Afficher la ligne de total",
			"contextMode.label": "Base du contexte",
			"contextMode.latest": "Dernier",
			"contextMode.latest.title": "Les lignes de tour affichent la taille de l'invite de la dernière étape mesurée (contexte final)\xA0; la ligne de total reste cumulative",
			"contextMode.cumulative": "Cumulatif",
			"contextMode.cumulative.title": "Les lignes de tour affichent la somme des tailles d'invite des étapes du tour (traitement cumulé)\xA0; la ligne de total reste cumulative",
			"totals.full": "Total de la session",
			"totals.full.title": "Totaux de la session complète\xA0; le temps total est le temps du modèle + le temps des outils (sans les écarts entre tours)",
			"totals.window": "Total de la fenêtre",
			"totals.window.title": "Totaux des tours chargés dans cette fenêtre\xA0; la cellule de contexte est cumulative sur les étapes",
			"expand.all": "Tout développer",
			"collapse.all": "Tout réduire",
			"turn.expand": "Afficher",
			"turn.collapse": "Masquer",
			"turn.toggle.title": "Développer ou réduire les étapes de ce tour",
			"turn.toggle.aria": "Développer ou réduire le tour {turn}",
			"did.answer": "réponse",
			"did.working": "génération",
			"did.aborted": "sans réponse",
			"did.more": "{count} de plus",
			"did.rest": "{count} appels de plus",
			"did.title": "{calls} appels\xA0: {tools}",
			"did.retry": "réessayé {count}×",
			"did.lines": "{lines} lignes",
			"did.lines.unit": "lignes",
			"did.chars": "{chars} caractères",
			"did.output": "→ {chars} caractères",
			"did.total": "{chars} caractères de sortie d'outil dans cette étape\xA0; ils feront partie de la prochaine invite",
			"detail.show": "détails",
			"detail.hide": "masquer",
			"detail.title": "Cliquez sur cette ligne pour développer ou réduire les détails des outils",
			"detail.calls": "{count} appels d'outils",
			"detail.answer": "Réponse",
			"detail.answer.truncated": "La réponse est longue\xA0; seuls les 16\xA0000 premiers caractères sont conservés",
			"detail.code.title": "Bloc de code",
			"detail.code.wrap": "Activer le retour à la ligne",
			"detail.code.unwrap": "Désactiver le retour à la ligne",
			"detail.footnotes": "Notes de bas de page",
			"detail.copy": "Copier",
			"detail.copy.title": "Copier ce texte",
			"detail.copied": "Copié",
			"detail.copyFailed": "Échec de la copie",
			"detail.chars": "{chars} caractères",
			"detail.lines": "{lines} lignes",
			"detail.file": "fichier",
			"detail.diff": "modification",
			"detail.emptyBody": "Cet appel n'a laissé aucun texte",
			"detail.hidden": "{count} autres appels ne sont pas listés ({limit} au maximum par étape)",
			"detail.args": "arguments bruts",
			"detail.wrap": "Retour à la ligne",
			"detail.wrap.title": "Afficher les longues lignes sur plusieurs lignes au lieu de défiler horizontalement",
			"detail.raw.title": "Afficher les arguments JSON bruts",
			"detail.lineCap": "Affichage de {shown} lignes sur {total}",
			"detail.truncated": "Trop long\xA0: seuls les {chars} premiers caractères sont conservés",
			"tool.failed": "{count} en échec",
			"tool.failed.title": "{count} appels d'outils de cette étape ont renvoyé un résultat d'erreur",
			"bar.ttft": "premier jeton {value}",
			"bar.gen": "génération {value}",
			"bar.tool": "appels d'outils {value}",
			"bar.rest": "autre {value}",
			"bar.wall": "total {value}",
			"bar.running": "pas encore terminé\xA0: il s'agit du temps écoulé",
			"bar.gap": "écart depuis l'étape précédente {value}",
			"bar.start": "commencé à {clock}",
			"turn.started": "ce tour a commencé à {time}",
			"turn.images": "{count} images",
			"imagePreview.hint": "Survolez ou placez le focus pour prévisualiser les images de ce tour\xA0; cliquez pour ouvrir l'image en grand",
			"imagePreview.dialog": "Aperçu de l'image",
			"imagePreview.open": "Voir l'image {index} sur {count}",
			"imagePreview.alt": "Image {index}",
			"imagePreview.loading": "Chargement de l'image…",
			"imagePreview.failed": "Échec du chargement de l'image",
			"imagePreview.retry": "Réessayer",
			"imagePreview.close": "Fermer l'aperçu de l'image",
			"steps.total": "{total} étapes",
			"steps.tail": "seulement les {limit} plus récentes",
			"steps.all": "Afficher les {total} étapes",
			"steps.filter": "Uniquement {tool}",
			"steps.clear": "Effacer le filtre",
			"steps.matched": "{count} étapes ont utilisé {tool}"
		};
		/** German dictionary. */
		const de = {
			"view.stats": "Statistik",
			"column.scope": "Runde / Schritt",
			"column.scope.title": "Eine Rundenzeile enthält die Summe der Runde; Zeile anklicken, um ihre Schritte ein- oder auszuklappen",
			"column.prompt": "Auslöser",
			"column.prompt.title": "Was diese Runde ausgelöst hat: ein menschlicher Prompt zeigt seinen Text, eine Automatisierung zeigt ein Quellen-Badge",
			"trigger.agent-message": "Agentennachricht",
			"trigger.team-message": "Teamnachricht",
			"trigger.schedule": "Geplant",
			"trigger.webhook": "Webhook",
			"trigger.tool-jobs": "Hintergrundjob",
			"trigger.cordis-host-runner": "Plugin-Lauf",
			"trigger.subagent-settled": "Subagent beendet",
			"trigger.goal": "Zielrunde",
			"trigger.context": "Kontext-Reaktivierung",
			"trigger.other": "Automatische Runde",
			"column.did": "Ausführung",
			"column.did.title": "Welche Werkzeuge dieser Schritt wie oft aufgerufen hat; ein Schritt ohne Werkzeugaufrufe zeigt eine Vorschau seines Antworttexts",
			"column.context": "Kontext",
			"column.context.title": "Usage-Eingabetoken + Cache-Lesen + Cache-Schreiben",
			"column.output": "Ausgabe",
			"column.output.title": "Usage-Ausgabetoken des Schritts",
			"column.cache": "Cache-Treffer",
			"column.cache.title": "Cache-Lesen ÷ (Eingabe + Cache-Lesen + Cache-Schreiben): der Anteil des Prompts, der aus dem Cache bedient wurde",
			"cache.detail": "Cache-Lesen {read} · Eingabe {input} · Cache-Schreiben {write}",
			"column.ttft": "Erstes Token",
			"column.ttft.title": "Vom Schrittbeginn bis zum ersten Ausgabefragment (TTFT)",
			"column.model": "Modellzeit",
			"column.model.title": "Vom Schrittbeginn bis zur abgeschlossenen Antwort (inklusive Verzögerung bis zum ersten Token)",
			"column.tps": "Ausgabetempo",
			"column.tps.title": "Ausgabetoken ÷ Generierungssekunden (TPS, tok/s)",
			"column.tool": "Werkzeugzeit",
			"column.tool.title": "Summe der Dauer jedes Werkzeugaufrufs im Schritt bis zu seinem Ergebnis; Aufrufe ohne Ergebnis zählen nicht",
			"column.time": "Gesamtzeit",
			"column.time.title": "Aufteilung der Gesamtzeit: hellblau erstes Token, blau Generierung, orange Werkzeugaufrufe, grau alles Übrige. Die Balkenlänge folgt einer Quadratwurzel-Skala; Schritte werden nur mit Schritten derselben Runde verglichen, Runden mit Runden",
			"turn.label": "Runde {turn}",
			"step.label": "Schritt {step}",
			"step.toggle.aria": "Werkzeugdetails von Schritt {step} ein- oder ausklappen",
			"step.partial": "Beginn unbekannt",
			"step.started.title": "Schritt begann um {time}",
			"compaction.label": "Komprimierung",
			"compaction.running": "Komprimierung läuft…",
			"compaction.shadowed": "Verlauf um {tokens} Token gekürzt",
			"compaction.summary": "Zusammenfassung {chars} Zeichen",
			"compaction.title": "Komprimierungsläufe werden separat angezeigt und nicht in die Schrittsummen der Runde eingerechnet",
			"event.show": "Ereigniszeilen anzeigen (inkl. Komprimierung)",
			"context.delta.show": "Kontextdelta anzeigen",
			"context.delta.title": "Prompt-Delta gegenüber dem vorigen gemessenen Schritt der Runde; negativ bedeutet, dass der Kontext geschrumpft ist (zum Beispiel nach einer Komprimierung)",
			"event.request-header": "Anfrage-Header",
			"event.system-prompt": "System-Prompt",
			"event.tool-set": "Werkzeugset-Änderung",
			"event.context-prune": "Kontextkürzung",
			"event.request-context": "Anfragekontext",
			"event.model-retry": "Modellwiederholung",
			"event.model-attempt": "Modellversuch",
			"event.context-refresh": "Kontextaktualisierung",
			"event.approval": "Genehmigung",
			"event.context.detail": "{provider} / {model} · Prompt-Modus {mode}",
			"event.retry.detail": "Wiederholung {retry} · Wartezeit {delay}",
			"event.attempt.detail": "Modellversuch ohne festgeschriebene Nachricht",
			"event.refresh.detail": "{source} · {chars} Zeichen",
			"event.approval.detail": "{tool} · {status}",
			"event.source.runtime": "Laufzeitkontext",
			"event.source.time": "Zeitkontext",
			"event.approval.asked": "Wartet",
			"event.approval.allowed": "Einmal zugelassen",
			"event.approval.rejected": "Abgelehnt",
			"event.approval.cancelled": "Abgebrochen",
			"event.approval.unavailable": "Nicht verfügbar",
			"event.title": "Ereigniszeilen außerhalb der Schritte: Änderungen an Anfrage, Prompt, Werkzeugset, Wiederholungen, Kontextaktualisierung, Genehmigungen und Kürzungen, die nicht in die Schrittsummen der Runde einfließen",
			"event.reason.initial": "Initial",
			"event.reason.resume": "Fortsetzung",
			"event.reason.change": "Änderung",
			"event.reason.series": "Neue Serie",
			"event.header.detail": "{reason} · {tools} Werkzeuge",
			"event.series": "Neue Serie",
			"event.system.append": "Anhängen",
			"event.system.replace": "Ersetzen",
			"event.system.clear": "Leeren",
			"event.system.detail": "{action} · {chars} Zeichen",
			"event.tools.added": "{tools} hinzugefügt",
			"event.tools.removed": "{tools} entfernt",
			"event.prune.detail": "{tokens} Token gekürzt",
			"step.partial.title": "Die Start- und Endereignisse dieses Schritts liegen nicht im aktuellen Ereignisfenster (beim Wiederverbinden des Tabs oder beim Neuaufbau des Fensters abgeschnitten). Daher fehlt für erstes Token, Modellzeit und Gesamtzeit der Startpunkt zum Abziehen; beim Neuladen des Tabs wird alles aus dem vollständigen Fenster neu zusammengesetzt",
			"state.running": "Läuft",
			"state.interrupted": "Unterbrochen",
			"state.cacheMiss": "Kein Cache-Treffer",
			"state.cacheMiss.title": "Der Prompt hat den Cache kaum getroffen: meist die erste Anfrage nach einem Neustart oder einer Sitzungsfortsetzung, zum vollen Eingabepreis abgerechnet",
			"state.aborted": "Nicht abgeschlossen",
			"state.aborted.title": "Der Schritt endete ohne abgeschlossene Antwort (meist abgebrochen oder fehlgeschlagen)",
			"unknown": "Nicht aufgezeichnet",
			"partial": "{count} Werkzeugaufrufe haben noch kein Ergebnis; das ist die Untergrenze der bekannten Zeit",
			"empty": "Im aktuellen Fenster gibt es keinen messbaren Schritt.",
			"loadOlder": "Frühere Runden laden",
			"loadOlder.loading": "Frühere Runden werden geladen…",
			"loadOlder.done": "Anfang des Verlaufs erreicht",
			"loadOlder.failed": "Laden fehlgeschlagen, erneut versuchen",
			"summary": "{turns} Runden · {steps} Schritte",
			"table.showTotals": "Summenzeile anzeigen",
			"contextMode.label": "Kontextbasis",
			"contextMode.latest": "Aktuell",
			"contextMode.latest.title": "Rundenzeilen zeigen die Prompt-Größe des letzten gemessenen Schritts (endgültiger Kontext); die Summenzeile bleibt kumulativ",
			"contextMode.cumulative": "Kumulativ",
			"contextMode.cumulative.title": "Rundenzeilen zeigen die Summe der Prompt-Größen über die Schritte der Runde (kumulierte Verarbeitung); die Summenzeile bleibt kumulativ",
			"totals.full": "Gesamtsumme der Sitzung",
			"totals.full.title": "Summen für die gesamte Sitzung; Gesamtzeit ist Modellzeit + Werkzeugzeit (ohne Lücken zwischen Runden)",
			"totals.window": "Summe des Fensters",
			"totals.window.title": "Summen für die in diesem Fenster geladenen Runden; die Kontextzelle ist über die Schritte kumuliert",
			"expand.all": "Alle ausklappen",
			"collapse.all": "Alle einklappen",
			"turn.expand": "Anzeigen",
			"turn.collapse": "Ausblenden",
			"turn.toggle.title": "Die Schritte dieser Runde ein- oder ausklappen",
			"turn.toggle.aria": "Runde {turn} ein- oder ausklappen",
			"did.answer": "Antwort",
			"did.working": "Wird generiert",
			"did.aborted": "Keine Antwort",
			"did.more": "{count} weitere",
			"did.rest": "{count} weitere Aufrufe",
			"did.title": "{calls} Aufrufe: {tools}",
			"did.retry": "{count}× wiederholt",
			"did.lines": "{lines} Zeilen",
			"did.lines.unit": "Zeilen",
			"did.chars": "{chars} Zeichen",
			"did.output": "→ {chars} Zeichen",
			"did.total": "{chars} Zeichen Werkzeugausgabe in diesem Schritt; sie fließen in den nächsten Prompt ein",
			"detail.show": "Details",
			"detail.hide": "Ausblenden",
			"detail.title": "Diese Zeile anklicken, um die Werkzeugdetails ein- oder auszublenden",
			"detail.calls": "{count} Werkzeugaufrufe",
			"detail.answer": "Antwort",
			"detail.answer.truncated": "Die Antwort ist lang; nur die ersten 16.000 Zeichen werden behalten",
			"detail.code.title": "Codeblock",
			"detail.code.wrap": "Zeilen umbrechen",
			"detail.code.unwrap": "Nicht umbrechen",
			"detail.footnotes": "Fußnoten",
			"detail.copy": "Kopieren",
			"detail.copy.title": "Diesen Text kopieren",
			"detail.copied": "Kopiert",
			"detail.copyFailed": "Kopieren fehlgeschlagen",
			"detail.chars": "{chars} Zeichen",
			"detail.lines": "{lines} Zeilen",
			"detail.file": "Datei",
			"detail.diff": "Änderung",
			"detail.emptyBody": "Dieser Aufruf hat keinen Text hinterlassen",
			"detail.hidden": "{count} weitere Aufrufe werden nicht aufgeführt (höchstens {limit} pro Schritt)",
			"detail.args": "Rohargumente",
			"detail.wrap": "Umbrechen",
			"detail.wrap.title": "Lange Zeilen umbrechen statt seitlich zu scrollen",
			"detail.raw.title": "Die rohen JSON-Argumente anzeigen",
			"detail.lineCap": "Die ersten {shown} von {total} Zeilen werden angezeigt",
			"detail.truncated": "Zu lang: nur die ersten {chars} Zeichen werden behalten",
			"tool.failed": "{count} fehlgeschlagen",
			"tool.failed.title": "{count} Werkzeugaufrufe in diesem Schritt haben ein Fehlerergebnis zurückgegeben",
			"bar.ttft": "Erstes Token {value}",
			"bar.gen": "Generierung {value}",
			"bar.tool": "Werkzeugaufrufe {value}",
			"bar.rest": "Sonstiges {value}",
			"bar.wall": "Gesamt {value}",
			"bar.running": "Noch nicht beendet: das ist die verstrichene Zeit",
			"bar.gap": "Lücke seit dem vorigen Schritt {value}",
			"bar.start": "Gestartet {clock}",
			"turn.started": "Diese Runde begann um {time}",
			"turn.images": "{count} Bilder",
			"imagePreview.hint": "Mit dem Mauszeiger auf die Zeile zeigen oder sie fokussieren, um die Bilder dieser Runde als Vorschau zu sehen; klicken, um das vollständige Bild zu öffnen",
			"imagePreview.dialog": "Bildvorschau",
			"imagePreview.open": "Bild {index} von {count} anzeigen",
			"imagePreview.alt": "Bild {index}",
			"imagePreview.loading": "Bild wird geladen…",
			"imagePreview.failed": "Bild konnte nicht geladen werden",
			"imagePreview.retry": "Erneut versuchen",
			"imagePreview.close": "Bildvorschau schließen",
			"steps.total": "{total} Schritte",
			"steps.tail": "Nur die neuesten {limit} anzeigen",
			"steps.all": "Alle {total} Schritte anzeigen",
			"steps.filter": "Nur {tool}",
			"steps.clear": "Filter löschen",
			"steps.matched": "{count} Schritte haben {tool} verwendet"
		};
		/** Russian dictionary. */
		const ru = {
			"view.stats": "Статистика",
			"column.scope": "Ход / шаг",
			"column.scope.title": "В строке хода — итог по этому ходу; нажмите на строку, чтобы развернуть или свернуть шаги",
			"column.prompt": "Триггер",
			"column.prompt.title": "Что запустило этот ход: у промпта человека показывается текст, у автоматического запуска — метка источника",
			"trigger.agent-message": "Сообщение агента",
			"trigger.team-message": "Сообщение команды",
			"trigger.schedule": "По расписанию",
			"trigger.webhook": "Webhook",
			"trigger.tool-jobs": "Фоновая задача",
			"trigger.cordis-host-runner": "Запуск плагина",
			"trigger.subagent-settled": "Субагент завершён",
			"trigger.goal": "Раунд цели",
			"trigger.context": "Пробуждение по контексту",
			"trigger.other": "Автоматический ход",
			"column.did": "Выполнение",
			"column.did.title": "Инструменты, вызванные на этом шаге, и число вызовов; шаг без вызовов инструментов показывает начало ответа",
			"column.context": "Контекст",
			"column.context.title": "входные токены usage + чтение кэша + запись кэша",
			"column.output": "Вывод",
			"column.output.title": "выходные токены usage за шаг",
			"column.cache": "Попадание в кэш",
			"column.cache.title": "чтение кэша ÷ (вход + чтение кэша + запись кэша): доля промпта, отданная из кэша",
			"cache.detail": "чтение кэша {read} · вход {input} · запись кэша {write}",
			"column.ttft": "Первый токен",
			"column.ttft.title": "от начала шага до первого фрагмента вывода (TTFT)",
			"column.model": "Время модели",
			"column.model.title": "от начала шага до готового ответа (включая задержку первого токена)",
			"column.tps": "Скорость вывода",
			"column.tps.title": "выходные токены ÷ секунды генерации (TPS, tok/s)",
			"column.tool": "Время инструментов",
			"column.tool.title": "сумма времени от каждого вызова инструмента до его результата внутри шага; вызовы без результата не учитываются",
			"column.time": "Общее время",
			"column.time.title": "разбивка общего времени: голубой — первый токен, синий — генерация, оранжевый — вызовы инструментов, серый — остальное. Длина полосы в масштабе квадратного корня; шаги сравниваются только с шагами своего хода, ходы — с ходами",
			"turn.label": "Ход {turn}",
			"step.label": "Шаг {step}",
			"step.toggle.aria": "Развернуть или свернуть подробности инструментов шага {step}",
			"step.partial": "начало неизвестно",
			"step.started.title": "Шаг начался в {time}",
			"compaction.label": "Сжатие",
			"compaction.running": "Сжатие…",
			"compaction.shadowed": "Сжато {tokens} токенов истории",
			"compaction.summary": "Сводка {chars} символов",
			"compaction.title": "Вызовы сжатия показываются отдельно и не входят в итоги шагов хода",
			"event.show": "Показывать строки событий (включая сжатие)",
			"context.delta.show": "Показывать прирост контекста",
			"context.delta.title": "Прирост промпта относительно предыдущего измеренного шага в этом ходе; отрицательное значение означает, что контекст уменьшился (например, после сжатия)",
			"event.request-header": "Заголовок запроса",
			"event.system-prompt": "Системный промпт",
			"event.tool-set": "Изменение набора инструментов",
			"event.context-prune": "Обрезка контекста",
			"event.request-context": "Контекст запроса",
			"event.model-retry": "Повтор запроса к модели",
			"event.model-attempt": "Попытка запроса к модели",
			"event.context-refresh": "Обновление контекста",
			"event.approval": "Одобрение",
			"event.context.detail": "{provider} / {model} · режим промпта {mode}",
			"event.retry.detail": "повтор {retry} · ожидание {delay}",
			"event.attempt.detail": "Попытка запроса к модели без зафиксированного сообщения",
			"event.refresh.detail": "{source} · {chars} символов",
			"event.approval.detail": "{tool} · {status}",
			"event.source.runtime": "контекст среды выполнения",
			"event.source.time": "временной контекст",
			"event.approval.asked": "ожидание",
			"event.approval.allowed": "разрешено один раз",
			"event.approval.rejected": "отклонено",
			"event.approval.cancelled": "отменено",
			"event.approval.unavailable": "недоступно",
			"event.title": "Строки событий вне шагов: изменения запроса, промпта, набора инструментов, повторов, обновления контекста, одобрений и обрезки — не входят в итоги шагов хода",
			"event.reason.initial": "начальный",
			"event.reason.resume": "возобновление",
			"event.reason.change": "изменение",
			"event.reason.series": "новая серия",
			"event.header.detail": "{reason} · инструментов: {tools}",
			"event.series": "новая серия",
			"event.system.append": "дополнение",
			"event.system.replace": "замена",
			"event.system.clear": "очистка",
			"event.system.detail": "{action} · {chars} символов",
			"event.tools.added": "добавлено {tools}",
			"event.tools.removed": "удалено {tools}",
			"event.prune.detail": "обрезано {tokens} токенов",
			"step.partial.title": "События начала и конца этого шага отсутствуют в текущем окне событий (обрезаны при переподключении вкладки или пересборке окна), поэтому для первого токена, времени модели и общего времени нет точки отсчёта; перезагрузка вкладки соберёт данные заново из полного окна",
			"state.running": "выполняется",
			"state.interrupted": "прервано",
			"state.cacheMiss": "промах кэша",
			"state.cacheMiss.title": "Промпт этого шага почти не попал в кэш: так обычно бывает у первого запроса после перезапуска или возобновления сеанса; вход тарифицируется по полной цене",
			"state.aborted": "не завершено",
			"state.aborted.title": "Шаг завершился без готового ответа (обычно отменён или завершился ошибкой)",
			"unknown": "не записано",
			"partial": "Вызовов без результата: {count}; это нижняя граница известного времени",
			"empty": "В текущем окне нет шагов, которые можно измерить.",
			"loadOlder": "Загрузить более ранние",
			"loadOlder.loading": "Загрузка более ранних ходов…",
			"loadOlder.done": "Достигнуто начало истории",
			"loadOlder.failed": "Не удалось загрузить, повторите",
			"summary": "Ходов: {turns} · шагов: {steps}",
			"table.showTotals": "Показывать строку итогов",
			"contextMode.label": "Учёт контекста",
			"contextMode.latest": "Последний",
			"contextMode.latest.title": "Строки ходов показывают размер промпта последнего измеренного шага (итоговый контекст); строка итогов всегда накопительная",
			"contextMode.cumulative": "Накопительный",
			"contextMode.cumulative.title": "Строки ходов показывают сумму размеров промптов шагов хода (накопительная обработка); строка итогов всегда накопительная",
			"totals.full": "Итог всего сеанса",
			"totals.full.title": "Итоги по всему сеансу; общее время — время модели + время инструментов (без промежутков между ходами)",
			"totals.window": "Итог окна",
			"totals.window.title": "Итоги по ходам, загруженным в это окно; контекст — накопительно по шагам",
			"expand.all": "Развернуть все",
			"collapse.all": "Свернуть все",
			"turn.expand": "Показать",
			"turn.collapse": "Скрыть",
			"turn.toggle.title": "Развернуть или свернуть шаги этого хода",
			"turn.toggle.aria": "Развернуть или свернуть ход {turn}",
			"did.answer": "ответ",
			"did.working": "генерация",
			"did.aborted": "без ответа",
			"did.more": "ещё {count}",
			"did.rest": "и ещё {count}",
			"did.title": "Вызовов: {calls} · {tools}",
			"did.retry": "повторов: {count}",
			"did.lines": "{lines} стр.",
			"did.lines.unit": "стр.",
			"did.chars": "{chars} симв.",
			"did.output": "→ {chars} симв.",
			"did.total": "Вывод инструментов на этом шаге — {chars} символов; он войдёт в следующий промпт",
			"detail.show": "подробности",
			"detail.hide": "скрыть",
			"detail.title": "Нажмите на строку, чтобы развернуть или свернуть подробности инструментов",
			"detail.calls": "вызовов инструментов: {count}",
			"detail.answer": "Ответ",
			"detail.answer.truncated": "Ответ слишком длинный; сохранены только первые 16 000 символов",
			"detail.code.title": "Блок кода",
			"detail.code.wrap": "Переносить строки",
			"detail.code.unwrap": "Не переносить",
			"detail.footnotes": "Сноски",
			"detail.copy": "Копировать",
			"detail.copy.title": "Скопировать этот текст",
			"detail.copied": "Скопировано",
			"detail.copyFailed": "Не удалось скопировать",
			"detail.chars": "{chars} симв.",
			"detail.lines": "{lines} стр.",
			"detail.file": "файл",
			"detail.diff": "изменение",
			"detail.emptyBody": "У этого вызова нет текста",
			"detail.hidden": "Не показано вызовов: {count} (не более {limit} на шаг)",
			"detail.args": "исходные аргументы",
			"detail.wrap": "Перенос",
			"detail.wrap.title": "Переносить длинные строки вместо горизонтальной прокрутки",
			"detail.raw.title": "Показать исходные аргументы JSON",
			"detail.lineCap": "Показаны первые {shown} из {total} строк",
			"detail.truncated": "Слишком длинно: сохранены только первые {chars} символов",
			"tool.failed": "ошибок: {count}",
			"tool.failed.title": "На этом шаге вызовов инструментов с ошибкой: {count}",
			"bar.ttft": "первый токен {value}",
			"bar.gen": "генерация {value}",
			"bar.tool": "вызовы инструментов {value}",
			"bar.rest": "остальное {value}",
			"bar.wall": "всего {value}",
			"bar.running": "ещё не завершено: это прошедшее время",
			"bar.gap": "от предыдущего шага {value}",
			"bar.start": "начало в {clock}",
			"turn.started": "этот ход начался в {time}",
			"turn.images": "{count} фото",
			"imagePreview.hint": "Наведите курсор или перейдите фокусом, чтобы посмотреть изображения этого хода; нажмите, чтобы открыть изображение целиком",
			"imagePreview.dialog": "Просмотр изображения",
			"imagePreview.open": "Открыть изображение {index} из {count}",
			"imagePreview.alt": "Изображение {index}",
			"imagePreview.loading": "Загрузка изображения…",
			"imagePreview.failed": "Не удалось загрузить изображение",
			"imagePreview.retry": "Повторить",
			"imagePreview.close": "Закрыть просмотр изображения",
			"steps.total": "Шагов: {total}",
			"steps.tail": "Только последние шаги ({limit})",
			"steps.all": "Показать все шаги ({total})",
			"steps.filter": "Только {tool}",
			"steps.clear": "Сбросить фильтр",
			"steps.matched": "Шагов с {tool}: {count}"
		};
		/** Portuguese dictionary. */
		const pt = {
			"view.stats": "Estatísticas",
			"column.scope": "Turno / etapa",
			"column.scope.title": "A linha do turno mostra o total do turno; clique em uma linha para expandir ou recolher suas etapas",
			"column.prompt": "Gatilho",
			"column.prompt.title": "O que abriu este turno: um prompt humano mostra o texto; uma automação mostra uma etiqueta de origem",
			"trigger.agent-message": "Mensagem do agente",
			"trigger.team-message": "Mensagem da equipe",
			"trigger.schedule": "Agendado",
			"trigger.webhook": "Webhook",
			"trigger.tool-jobs": "Tarefa em segundo plano",
			"trigger.cordis-host-runner": "Execução de plugin",
			"trigger.subagent-settled": "Subagente concluído",
			"trigger.goal": "Rodada de objetivo",
			"trigger.context": "Reativação de contexto",
			"trigger.other": "Turno automático",
			"column.did": "Execução",
			"column.did.title": "Ferramentas chamadas nesta etapa e com que frequência; uma etapa sem chamadas de ferramenta mostra uma prévia do texto da resposta",
			"column.context": "Contexto",
			"column.context.title": "Tokens de entrada do uso + leitura de cache + gravação de cache",
			"column.output": "Saída",
			"column.output.title": "Tokens de saída do uso da etapa",
			"column.cache": "Acerto de cache",
			"column.cache.title": "Leitura de cache ÷ (entrada + leitura de cache + gravação de cache): a parte do prompt atendida pelo cache",
			"cache.detail": "Leitura de cache {read} · entrada {input} · gravação de cache {write}",
			"column.ttft": "Primeiro token",
			"column.ttft.title": "Início da etapa até o primeiro fragmento de saída (TTFT)",
			"column.model": "Tempo do modelo",
			"column.model.title": "Início da etapa até a resposta concluída (inclui o atraso do primeiro token)",
			"column.tps": "Velocidade de saída",
			"column.tps.title": "Tokens de saída ÷ segundos de geração (TPS, tok/s)",
			"column.tool": "Tempo de ferramenta",
			"column.tool.title": "Soma de cada chamada de ferramenta até o resultado, dentro da etapa; chamadas sem resultado ficam de fora",
			"column.time": "Tempo total",
			"column.time.title": "Divisão do total: azul-claro primeiro token, azul geração, laranja chamadas de ferramenta, cinza todo o resto. O comprimento da barra usa escala de raiz quadrada; etapas só se comparam com etapas do mesmo turno, turnos com turnos",
			"turn.label": "Turno {turn}",
			"step.label": "Etapa {step}",
			"step.toggle.aria": "Expandir ou recolher os detalhes de ferramentas da etapa {step}",
			"step.partial": "início desconhecido",
			"step.started.title": "Etapa iniciada às {time}",
			"compaction.label": "Compactação",
			"compaction.running": "Compactando…",
			"compaction.shadowed": "{tokens} tokens de histórico podados",
			"compaction.summary": "Resumo de {chars} caracteres",
			"compaction.title": "As execuções de compactação aparecem separadas e ficam fora dos totais das etapas do turno",
			"event.show": "Mostrar linhas de evento (incl. compactação)",
			"context.delta.show": "Mostrar delta de contexto",
			"context.delta.title": "Delta do prompt em relação à etapa medida anterior no turno; negativo significa que o contexto diminuiu (por exemplo, após a compactação)",
			"event.request-header": "Cabeçalho da solicitação",
			"event.system-prompt": "Prompt do sistema",
			"event.tool-set": "Mudança no conjunto de ferramentas",
			"event.context-prune": "Poda de contexto",
			"event.request-context": "Contexto da solicitação",
			"event.model-retry": "Nova tentativa do modelo",
			"event.model-attempt": "Tentativa do modelo",
			"event.context-refresh": "Atualização de contexto",
			"event.approval": "Aprovação",
			"event.context.detail": "{provider} / {model} · modo de prompt {mode}",
			"event.retry.detail": "nova tentativa {retry} · espera {delay}",
			"event.attempt.detail": "Tentativa do modelo sem mensagem confirmada",
			"event.refresh.detail": "{source} · {chars} caracteres",
			"event.approval.detail": "{tool} · {status}",
			"event.source.runtime": "Contexto de execução",
			"event.source.time": "Contexto de tempo",
			"event.approval.asked": "Aguardando",
			"event.approval.allowed": "Permitido uma vez",
			"event.approval.rejected": "Rejeitado",
			"event.approval.cancelled": "Cancelado",
			"event.approval.unavailable": "Indisponível",
			"event.title": "Linhas de evento que não são etapas: mudanças de solicitação, prompt, conjunto de ferramentas, nova tentativa, atualização de contexto, aprovação e poda são excluídas dos totais das etapas do turno",
			"event.reason.initial": "Inicial",
			"event.reason.resume": "Retomada",
			"event.reason.change": "Mudança",
			"event.reason.series": "Nova série",
			"event.header.detail": "{reason} · {tools} ferramentas",
			"event.series": "Nova série",
			"event.system.append": "Acréscimo",
			"event.system.replace": "Substituição",
			"event.system.clear": "Limpeza",
			"event.system.detail": "{action} · {chars} caracteres",
			"event.tools.added": "adicionadas: {tools}",
			"event.tools.removed": "removidas: {tools}",
			"event.prune.detail": "{tokens} tokens podados",
			"step.partial.title": "Os eventos de início e fim desta etapa não estão na janela de eventos atual (foram cortados quando a aba se reconectou ou a janela foi reconstruída), então o primeiro token, o tempo do modelo e o total não têm um início para subtrair; recarregar a aba remonta os dados a partir da janela completa",
			"state.running": "Em execução",
			"state.interrupted": "Interrompido",
			"state.cacheMiss": "Falha de cache",
			"state.cacheMiss.title": "O prompt quase não usou o cache: normalmente é a primeira solicitação após uma reinicialização ou retomada de sessão, paga pelo preço integral de entrada",
			"state.aborted": "Não concluído",
			"state.aborted.title": "A etapa terminou sem uma resposta concluída (geralmente cancelada ou com falha)",
			"unknown": "não registrado",
			"partial": "{count} chamadas de ferramenta ainda estão sem resultado; este é o limite inferior do tempo conhecido",
			"empty": "Nenhuma etapa mensurável na janela atual.",
			"loadOlder": "Carregar anteriores",
			"loadOlder.loading": "Carregando turnos anteriores…",
			"loadOlder.done": "Início do histórico alcançado",
			"loadOlder.failed": "Falha ao carregar, tentar novamente",
			"summary": "{turns} turnos · {steps} etapas",
			"table.showTotals": "Mostrar linha de total",
			"contextMode.label": "Base do contexto",
			"contextMode.latest": "Mais recente",
			"contextMode.latest.title": "As linhas de turno mostram o tamanho do prompt da última etapa medida (contexto final); a linha de total continua cumulativa",
			"contextMode.cumulative": "Cumulativo",
			"contextMode.cumulative.title": "As linhas de turno mostram a soma dos tamanhos de prompt das etapas do turno (processamento cumulativo); a linha de total continua cumulativa",
			"totals.full": "Total da sessão inteira",
			"totals.full.title": "Totais da sessão completa; o tempo total é tempo do modelo + tempo de ferramenta (sem lacunas entre turnos)",
			"totals.window": "Total da janela",
			"totals.window.title": "Totais dos turnos carregados nesta janela; a célula de contexto é cumulativa entre as etapas",
			"expand.all": "Expandir tudo",
			"collapse.all": "Recolher tudo",
			"turn.expand": "Mostrar",
			"turn.collapse": "Ocultar",
			"turn.toggle.title": "Expandir ou recolher as etapas deste turno",
			"turn.toggle.aria": "Expandir ou recolher o turno {turn}",
			"did.answer": "resposta",
			"did.working": "gerando",
			"did.aborted": "sem resposta",
			"did.more": "mais {count}",
			"did.rest": "mais {count} chamadas",
			"did.title": "{calls} chamadas: {tools}",
			"did.retry": "repetida {count}×",
			"did.lines": "{lines} linhas",
			"did.lines.unit": "linhas",
			"did.chars": "{chars} caracteres",
			"did.output": "→ {chars} caracteres",
			"did.total": "{chars} caracteres de saída de ferramentas nesta etapa; eles passam a fazer parte do próximo prompt",
			"detail.show": "detalhes",
			"detail.hide": "ocultar",
			"detail.title": "Clique nesta linha para expandir ou recolher os detalhes de ferramentas",
			"detail.calls": "{count} chamadas de ferramenta",
			"detail.answer": "Resposta",
			"detail.answer.truncated": "A resposta é longa; apenas os primeiros 16.000 caracteres são mantidos",
			"detail.code.title": "Bloco de código",
			"detail.code.wrap": "Quebrar linhas",
			"detail.code.unwrap": "Não quebrar",
			"detail.footnotes": "Notas de rodapé",
			"detail.copy": "Copiar",
			"detail.copy.title": "Copiar este texto",
			"detail.copied": "Copiado",
			"detail.copyFailed": "Falha ao copiar",
			"detail.chars": "{chars} caracteres",
			"detail.lines": "{lines} linhas",
			"detail.file": "arquivo",
			"detail.diff": "alteração",
			"detail.emptyBody": "Esta chamada não deixou texto",
			"detail.hidden": "mais {count} chamadas não são listadas (no máximo {limit} por etapa)",
			"detail.args": "argumentos brutos",
			"detail.wrap": "Quebrar",
			"detail.wrap.title": "Quebrar linhas longas em vez de rolar para o lado",
			"detail.raw.title": "Mostrar os argumentos JSON brutos",
			"detail.lineCap": "Mostrando as primeiras {shown} de {total} linhas",
			"detail.truncated": "Muito longo: apenas os primeiros {chars} caracteres são mantidos",
			"tool.failed": "{count} com falha",
			"tool.failed.title": "{count} chamadas de ferramenta nesta etapa retornaram erro",
			"bar.ttft": "primeiro token {value}",
			"bar.gen": "geração {value}",
			"bar.tool": "chamadas de ferramenta {value}",
			"bar.rest": "outros {value}",
			"bar.wall": "total {value}",
			"bar.running": "ainda não terminou: este é o tempo decorrido",
			"bar.gap": "intervalo desde a etapa anterior {value}",
			"bar.start": "iniciado às {clock}",
			"turn.started": "este turno começou às {time}",
			"turn.images": "{count} imagens",
			"imagePreview.hint": "Passe o mouse ou foque para pré-visualizar as imagens deste turno; clique para abrir a imagem completa",
			"imagePreview.dialog": "Pré-visualização de imagem",
			"imagePreview.open": "Ver imagem {index} de {count}",
			"imagePreview.alt": "Imagem {index}",
			"imagePreview.loading": "Carregando imagem…",
			"imagePreview.failed": "Falha ao carregar a imagem",
			"imagePreview.retry": "Tentar novamente",
			"imagePreview.close": "Fechar pré-visualização de imagem",
			"steps.total": "{total} etapas",
			"steps.tail": "apenas as {limit} mais recentes",
			"steps.all": "Mostrar todas as {total} etapas",
			"steps.filter": "Apenas {tool}",
			"steps.clear": "Limpar filtro",
			"steps.matched": "{count} etapas usaram {tool}"
		};
		//#endregion
		//#region ../../util/values/src/partial-json.ts
		/**
		* Lazily scanned view of one JSON object's top-level fields, built from text
		* that may still be streaming or from an already parsed object. Nothing is
		* scanned until a reader asks; the view remembers every question it answered
		* and reports changed answers when the owner refreshes for publication.
		* Used for model tool-call arguments: a row reads the fields it
		* cares about at whatever granularity it displays, at every stage of the call.
		* @module @deepseek-ai/dsh-util-values/src/partial-json
		*/
		const SIMPLE_ESCAPES = {
			"\"": "\"",
			"\\": "\\",
			"/": "/",
			b: "\b",
			f: "\f",
			n: "\n",
			r: "\r",
			t: "	"
		};
		const CONTENT_ESCAPE = /[\\\u0000-\u001f]/u;
		function isWhitespace(c) {
			return c === " " || c === "\n" || c === "\r" || c === "	";
		}
		function isHex(c) {
			return c >= "0" && c <= "9" || c >= "a" && c <= "f" || c >= "A" && c <= "F";
		}
		(class PartialArguments {
			/** The view of a call with no arguments available. */
			static EMPTY = PartialArguments.fromObject({});
			/**
			* View finished argument text without scanning it until a reader asks.
			* @param text - the complete argument JSON text.
			* @returns a sealed view.
			*/
			static fromText(text) {
				const view = new PartialArguments();
				view.append(text);
				view.sealed = true;
				return view;
			}
			/**
			* View an already parsed argument payload, such as a PTC dispatch object.
			* @param value - the parsed argument value.
			* @returns a sealed view; a non-object payload has no fields.
			*/
			static fromObject(value) {
				const view = new PartialArguments();
				view.object = typeof value === "object" && value !== null && !Array.isArray(value) ? value : {};
				view.sealed = true;
				return view;
			}
			/**
			* The source: text so far or a parsed object, plus whether it can still grow.
			* These are the only enumerable fields, so two views over the same source
			* compare equal structurally however far each has been read.
			*/
			chunks = [];
			object;
			sealed = false;
			#ends = [];
			#size = 0;
			#consumed = 0;
			#mode = "root";
			#escape = false;
			#keyStart = 0;
			#keyEscaped = false;
			#key = "";
			#current = null;
			#nestedEnds = [];
			#nestedInString = false;
			#invalidAt;
			#invalidValue = false;
			#entries = /* @__PURE__ */ new Map();
			#order = [];
			#reads = /* @__PURE__ */ new Map();
			/** Whether this view rejects further appends; does not scan text or register reads. */
			get isSealed() {
				return this.sealed;
			}
			/** Whether indexing or a content read found invalid JSON; unread value contents are not validated. */
			get invalid() {
				this.scan();
				return this.#mode === "invalid" || this.#invalidValue;
			}
			/**
			* Retain streamed argument text without scanning or comparing observed answers.
			* @param fragment - the text following every fragment appended before.
			*/
			append(fragment) {
				if (this.sealed) throw new Error("PartialArguments: cannot append to a sealed view");
				if (fragment.length === 0) return;
				this.chunks.push(fragment);
				this.#size += fragment.length;
				this.#ends.push(this.#size);
			}
			/**
			* Reconcile a streamed prefix with authoritative complete text without joining the fragments.
			* @param text - the final argument text, which replaces missing or conflicting deltas.
			* @returns this view sealed with its caches retained when every character matches; otherwise a new sealed view.
			*/
			settle(text) {
				if (this.object !== void 0 || text.length !== this.#size) return PartialArguments.fromText(text);
				let offset = 0;
				for (const chunk of this.chunks) {
					if (!text.startsWith(chunk, offset)) return PartialArguments.fromText(text);
					offset += chunk.length;
				}
				this.chunks = text.length === 0 ? [] : [text];
				this.#ends = text.length === 0 ? [] : [text.length];
				this.sealed = true;
				return this;
			}
			/**
			* Compare observed answers and advance their publication baseline. Unread views remain unscanned.
			* @returns whether any observed answer changed since its first read or the preceding refresh.
			*/
			refresh() {
				if (this.#reads.size === 0) return false;
				this.scan();
				let changed = false;
				let completions = false;
				for (const read of this.#reads.values()) {
					if (read.completion) {
						completions = true;
						continue;
					}
					changed = this.refreshRead(read) || changed;
				}
				if (completions) {
					for (const read of this.#reads.values()) if (read.completion) changed = this.refreshRead(read) || changed;
				}
				if (this.sealed) this.#reads.clear();
				return changed;
			}
			refreshRead(read) {
				const now = read.answer();
				if (Object.is(now, read.last)) return false;
				read.last = now;
				return true;
			}
			/**
			* Check whether no further fields can arrive.
			* @returns whether the outer object closed, indexing failed, or the view is sealed; unread values are not validated.
			*/
			closed() {
				return this.remember("closed", "", () => this.closedNow());
			}
			/**
			* List discovered fields in first-appearance order.
			* @returns top-level keys seen so far, in first-appearance order.
			*/
			keys() {
				return this.remember("keys", "", () => this.keysNow(), (keys) => keys.length);
			}
			/**
			* Check whether a top-level field has appeared.
			* @param key - argument name.
			* @returns whether the field has appeared (a string opened or another value began).
			*/
			has(key) {
				return this.remember("has", key, () => this.hasNow(key));
			}
			/**
			* Check whether a field's closing delimiter has arrived, without validating its contents.
			* @param key - argument name.
			* @returns whether its delimiter arrived and no content reader has reported an error for this value.
			*/
			complete(key) {
				return this.remember("complete", key, () => this.completeNow(key));
			}
			/**
			* Read string length without materializing its text.
			* @param key - argument name.
			* @param options - change granularity for a streaming string.
			* @returns decoded UTF-16 length of the string field so far; undefined when absent or not a string.
			*/
			stringLength(key, options) {
				const step = Math.max(1, Math.floor(options?.step ?? 1));
				const offset = options?.offset ?? 0;
				return this.remember(`length:${step}:${offset}`, key, () => this.lengthNow(key), (length) => length === void 0 ? void 0 : Math.ceil((length + offset) / step));
			}
			/**
			* Check a string against a decoded UTF-16 length limit without materializing it.
			* @param key - argument name.
			* @param maxLength - decoded UTF-16 limit, floored to at least zero.
			* @returns whether the string is longer than the limit; false when absent or not a string.
			*/
			stringExceeds(key, maxLength) {
				const limit = Math.max(0, Math.floor(maxLength));
				return this.remember(`exceeds:${limit}`, key, () => (this.lengthNow(key, limit + 1) ?? 0) > limit);
			}
			/**
			* Read a decoded string, including a streaming prefix.
			* @param key - argument name.
			* @returns the string field's decoded text so far; undefined when absent or not a string.
			*/
			text(key) {
				return this.remember("text", key, () => this.textNow(key));
			}
			/**
			* Read at most the first decoded UTF-16 units of a string.
			* @param key - argument name.
			* @param maxLength - maximum decoded UTF-16 length, floored to at least one.
			* @returns the bounded string prefix; undefined when absent or not a string.
			*/
			textPrefix(key, maxLength) {
				const limit = Math.max(1, Math.floor(maxLength));
				return this.remember(`prefix:${limit}`, key, () => this.textPrefixNow(key, limit));
			}
			/**
			* Read a completed non-string argument.
			* @param key - argument name.
			* @returns the parsed non-string value once it closed; undefined while open, absent, or a string.
			*/
			value(key) {
				return this.remember("value", key, () => this.valueNow(key));
			}
			/** Answer a question and, on a streaming view, remember it for change detection. */
			remember(kind, key, read, comparison) {
				this.scan();
				const result = read();
				if (!this.sealed) {
					const id = `${kind}/${key}`;
					if (!this.#reads.has(id)) this.#reads.set(id, {
						completion: kind === "complete",
						answer: comparison === void 0 ? read : () => comparison(read()),
						last: comparison === void 0 ? result : comparison(result)
					});
				}
				return result;
			}
			closedNow() {
				return this.sealed || this.#mode === "closed" || this.#mode === "invalid";
			}
			keysNow() {
				return this.object === void 0 ? this.#order : Object.keys(this.object);
			}
			hasNow(key) {
				return this.object === void 0 ? this.#entries.has(key) : Object.hasOwn(this.object, key);
			}
			completeNow(key) {
				if (this.object !== void 0) return Object.hasOwn(this.object, key);
				const entry = this.#entries.get(key);
				return entry !== void 0 && entry.end >= 0 && (entry.kind === "string" ? entry.invalidAt === void 0 : !entry.invalid);
			}
			lengthNow(key, limit = Number.POSITIVE_INFINITY) {
				if (this.object !== void 0) {
					const field = Object.hasOwn(this.object, key) ? this.object[key] : void 0;
					return typeof field === "string" ? field.length : void 0;
				}
				const entry = this.#entries.get(key);
				if (entry?.kind !== "string") return void 0;
				if (entry.text !== void 0 && entry.text.at === entry.end) return entry.text.length;
				const read = entry.length ??= {
					at: entry.start,
					length: 0,
					text: ""
				};
				this.readString(entry, read, limit, false);
				return read.length;
			}
			textNow(key) {
				if (this.object !== void 0) {
					const field = Object.hasOwn(this.object, key) ? this.object[key] : void 0;
					return typeof field === "string" ? field : void 0;
				}
				const entry = this.#entries.get(key);
				if (entry?.kind !== "string") return void 0;
				if (entry.text === void 0 && entry.end >= 0 && entry.needsDecoding && entry.invalidAt === void 0) {
					let text;
					try {
						text = JSON.parse(`"${this.slice(entry.start, entry.end)}"`);
					} catch (_error) {}
					if (text !== void 0) entry.text = {
						at: entry.end,
						length: text.length,
						text
					};
				}
				const read = entry.text ??= {
					at: entry.start,
					length: 0,
					text: ""
				};
				this.readString(entry, read, Number.POSITIVE_INFINITY, true);
				return read.text;
			}
			textPrefixNow(key, maxLength) {
				if (this.object !== void 0) {
					const field = Object.hasOwn(this.object, key) ? this.object[key] : void 0;
					return typeof field === "string" ? field.slice(0, maxLength) : void 0;
				}
				const entry = this.#entries.get(key);
				if (entry?.kind !== "string") return void 0;
				const prefixes = entry.prefixes ??= /* @__PURE__ */ new Map();
				let read = prefixes.get(maxLength);
				if (read === void 0) {
					read = {
						at: entry.start,
						length: 0,
						text: ""
					};
					prefixes.set(maxLength, read);
				}
				this.readString(entry, read, maxLength, true);
				return read.text;
			}
			valueNow(key) {
				if (this.object !== void 0) {
					if (!Object.hasOwn(this.object, key)) return void 0;
					const field = this.object[key];
					return typeof field === "string" ? void 0 : field;
				}
				const entry = this.#entries.get(key);
				if (entry?.kind !== "value" || entry.end < 0 || entry.invalid) return void 0;
				if (entry.parsed === void 0) try {
					entry.parsed = JSON.parse(this.slice(entry.start, entry.end));
				} catch (_error) {
					entry.invalid = true;
					this.#invalidValue = true;
				}
				return entry.parsed;
			}
			chunkAt(at) {
				let low = 0;
				let high = this.#ends.length;
				while (low < high) {
					const mid = low + high >>> 1;
					if (this.#ends[mid] <= at) low = mid + 1;
					else high = mid;
				}
				return low;
			}
			/** Materialize only a requested range, never the cumulative source. */
			slice(start, end) {
				if (start >= end) return "";
				const first = this.chunkAt(start);
				const last = this.chunkAt(end - 1);
				const base = first === 0 ? 0 : this.#ends[first - 1];
				if (first === last) return this.chunks[first].slice(start - base, end - base);
				const parts = [this.chunks[first].slice(start - base)];
				for (let i = first + 1; i < last; i++) parts.push(this.chunks[i]);
				parts.push(this.chunks[last].slice(0, end - this.#ends[last - 1]));
				return parts.join("");
			}
			readString(entry, read, limit, materialize) {
				const end = Math.min(entry.end < 0 ? this.#consumed : entry.end, entry.invalidAt ?? Number.POSITIVE_INFINITY, this.#invalidAt ?? Number.POSITIVE_INFINITY);
				if (!entry.needsDecoding) {
					const length = Math.min(end - read.at, limit - read.length);
					if (length <= 0) return;
					if (materialize) read.text += this.slice(read.at, read.at + length);
					read.at += length;
					read.length += length;
					return;
				}
				let chunkIndex = this.chunkAt(read.at);
				while (read.at < end && read.length < limit) {
					const base = chunkIndex === 0 ? 0 : this.#ends[chunkIndex - 1];
					const chunk = this.chunks[chunkIndex];
					const remaining = chunk.slice(read.at - base, Math.min(chunk.length, end - base));
					const boundary = remaining.search(CONTENT_ESCAPE);
					const length = Math.min(boundary < 0 ? remaining.length : boundary, limit - read.length);
					if (length > 0) {
						if (materialize) read.text += remaining.slice(0, length);
						read.at += length;
						read.length += length;
						if (read.at === base + chunk.length) chunkIndex++;
						continue;
					}
					const type = remaining.length > 1 ? remaining[1] : read.at + 1 < end ? this.chunks[chunkIndex + 1][0] : void 0;
					let decoded;
					let width = 2;
					if (remaining[0] === "\\" && type === void 0 && entry.end < 0) return;
					if (remaining[0] === "\\" && type === "u") {
						const hex = this.slice(read.at + 2, Math.min(end, read.at + 6));
						let valid = true;
						for (let i = 0; i < hex.length; i++) if (!isHex(hex[i])) valid = false;
						if (valid) {
							if (hex.length < 4 && entry.end < 0) return;
							if (hex.length === 4) decoded = String.fromCharCode(Number.parseInt(hex, 16));
						}
						width = 6;
					} else if (remaining[0] === "\\" && type !== void 0) decoded = SIMPLE_ESCAPES[type];
					if (decoded === void 0) {
						entry.invalidAt = read.at;
						this.#invalidValue = true;
						return;
					}
					if (materialize) read.text += decoded;
					read.length++;
					read.at += width;
					while (chunkIndex < this.chunks.length && read.at >= this.#ends[chunkIndex]) chunkIndex++;
				}
			}
			/** Locate new field ranges without decoding or parsing their contents. */
			scan() {
				if (this.object !== void 0 || this.#consumed === this.#size) return;
				for (let i = this.chunkAt(this.#consumed); i < this.chunks.length && this.#invalidAt === void 0; i++) {
					const pending = this.chunks[i];
					const base = i === 0 ? 0 : this.#ends[i - 1];
					for (let index = this.#consumed - base; index < pending.length && this.#mode !== "invalid"; index++) {
						if (this.#mode === "string" || this.#mode === "nested" && this.#nestedInString) {
							const end = this.stringBoundary(pending, index);
							this.#consumed += end - index;
							index = end;
							if (index === pending.length) break;
						}
						this.step(pending[index], this.#consumed);
						this.#consumed++;
					}
				}
			}
			/** Only raw quotes and their preceding backslash runs can terminate a string. */
			stringBoundary(fragment, start) {
				let at = start;
				while (true) {
					const quote = fragment.indexOf("\"", at);
					const end = quote < 0 ? fragment.length : quote;
					if (this.#mode === "string") {
						const entry = this.#current;
						if (!entry.needsDecoding && CONTENT_ESCAPE.test(fragment.slice(at, end))) entry.needsDecoding = true;
					}
					let slashStart = end;
					while (slashStart > at && fragment[slashStart - 1] === "\\") slashStart--;
					const escaped = (end - slashStart) % 2 === 1 !== (slashStart === at && this.#escape);
					this.#escape = quote < 0 && escaped;
					if (quote < 0 || !escaped) return end;
					at = quote + 1;
				}
			}
			step(c, at) {
				switch (this.#mode) {
					case "root":
						if (isWhitespace(c)) return;
						if (c === "{") {
							this.#mode = "key-or-end";
							return;
						}
						this.fail();
						return;
					case "key-or-end":
						if (isWhitespace(c)) return;
						if (c === "}") {
							this.#mode = "closed";
							return;
						}
						if (c === "\"") {
							this.beginKey(at);
							return;
						}
						this.fail();
						return;
					case "key-only":
						if (isWhitespace(c)) return;
						if (c === "\"") {
							this.beginKey(at);
							return;
						}
						this.fail();
						return;
					case "key":
						this.stepKey(c, at);
						return;
					case "colon":
						if (isWhitespace(c)) return;
						if (c === ":") {
							this.#mode = "value";
							return;
						}
						this.fail();
						return;
					case "value":
						this.beginValue(c, at);
						return;
					case "string": {
						const entry = this.#current;
						entry.end = at;
						this.#current = null;
						this.#mode = "comma-or-end";
						return;
					}
					case "scalar":
						this.stepScalar(c, at);
						return;
					case "nested":
						this.stepNested(c, at);
						return;
					case "comma-or-end":
						if (isWhitespace(c)) return;
						if (c === ",") {
							this.#mode = "key-only";
							return;
						}
						if (c === "}") {
							this.#mode = "closed";
							return;
						}
						this.fail();
						return;
					case "closed":
						if (isWhitespace(c)) return;
						this.fail();
						return;
					/* v8 ignore next 2 -- scan() stops stepping once the view is invalid. */
					case "invalid": return;
					/* v8 ignore next 2 -- Every scanner mode has a handler above. */
					default: assertNever(this.#mode);
				}
			}
			fail() {
				this.#invalidAt = this.#consumed;
				this.#mode = "invalid";
				this.#current = null;
			}
			beginKey(at) {
				this.#mode = "key";
				this.#keyStart = at + 1;
				this.#keyEscaped = false;
				this.#escape = false;
			}
			stepKey(c, at) {
				if (c < " ") {
					this.fail();
					return;
				}
				if (this.#escape) {
					this.#escape = false;
					return;
				}
				if (c === "\\") {
					this.#escape = true;
					this.#keyEscaped = true;
					return;
				}
				if (c !== "\"") return;
				const raw = this.slice(this.#keyStart, at);
				if (this.#keyEscaped) try {
					this.#key = JSON.parse(`"${raw}"`);
				} catch (_error) {
					this.fail();
					return;
				}
				else this.#key = raw;
				this.#mode = "colon";
			}
			open(entry) {
				if (!this.#entries.has(this.#key)) this.#order.push(this.#key);
				this.#entries.set(this.#key, entry);
				this.#current = entry;
			}
			beginValue(c, at) {
				if (isWhitespace(c)) return;
				if (c === "\"") {
					this.open({
						kind: "string",
						start: at + 1,
						end: -1,
						needsDecoding: false,
						invalidAt: void 0,
						length: void 0,
						text: void 0,
						prefixes: void 0
					});
					this.#escape = false;
					this.#mode = "string";
					return;
				}
				if (c === "}" || c === "," || c === ":" || c === "]") {
					this.fail();
					return;
				}
				this.open({
					kind: "value",
					start: at,
					end: -1,
					parsed: void 0,
					invalid: false
				});
				if (c === "{" || c === "[") {
					this.#mode = "nested";
					this.#nestedEnds = [c === "{" ? "}" : "]"];
					this.#nestedInString = false;
					this.#escape = false;
					return;
				}
				this.#mode = "scalar";
			}
			stepScalar(c, at) {
				if (c !== "," && c !== "}" && !isWhitespace(c)) return;
				this.closeValue(at);
				this.#mode = c === "," ? "key-only" : c === "}" ? "closed" : "comma-or-end";
			}
			stepNested(c, at) {
				if (this.#nestedInString) {
					this.#nestedInString = false;
					return;
				}
				if (c === "\"") {
					this.#nestedInString = true;
					return;
				}
				if (c === "{" || c === "[") {
					this.#nestedEnds.push(c === "{" ? "}" : "]");
					return;
				}
				if (c === "}" || c === "]") {
					if (this.#nestedEnds.pop() !== c) {
						this.fail();
						return;
					}
					if (this.#nestedEnds.length === 0) {
						this.closeValue(at + 1);
						this.#mode = "comma-or-end";
					}
				}
			}
			closeValue(end) {
				const entry = this.#current;
				entry.end = end;
				this.#current = null;
			}
		});
		//#endregion
		//#region ../../util/values/src/index.ts
		/**
		* Mark an unreachable closed-union branch.
		* @param value - impossible value; an unhandled typed variant fails at the call site.
		* @param context - optional switch-site label included in the failure message.
		* @returns never; a runtime value that escaped its type always throws.
		*/
		function assertNever(value, context) {
			const rendered = JSON.stringify(value) ?? String(value);
			throw new Error(`unreachable variant${context ? ` in ${context}` : ""}: ${rendered}`);
		}
		//#endregion
		//#region ../../llm/llm/src/assistant-stream.ts
		/**
		* Whether one chunk carries the model's first output token for latency measurement.
		* @param chunk - any stream chunk.
		* @returns true for a non-empty text, reasoning, or Tool-call arguments fragment and for
		*   every name-bearing Tool-call delta; false for block, usage, and finish chunks.
		*/
		function isTokenDelta$1(chunk) {
			switch (chunk.type) {
				case "text-delta":
				case "reasoning-delta": return chunk.text !== "";
				case "tool-call-delta": return chunk.argumentsDelta !== "" || chunk.name !== void 0;
				default: return false;
			}
		}
		function firstRunMemberTime(run, predicate) {
			const fragments = run.type === "tool-call-chunks" ? run.args : run.texts;
			let time = run.time0;
			for (let index = 0; index < fragments.length; index += 1) {
				if (index > 0) time += run.dt[index - 1];
				if (predicate(fragments[index])) return time;
			}
		}
		/**
		* Time of the first member of one packed run that {@link isTokenDelta} accepts: a
		* name-bearing Tool-call run starts at its first member, otherwise the first non-empty fragment.
		* Stops scanning at that member.
		* @param run - one packed delta run.
		* @returns the member's reconstructed time, or undefined when no member qualifies.
		*/
		function runFirstTokenTime(run) {
			if (run.type === "tool-call-chunks" && run.name !== void 0) return run.time0;
			return firstRunMemberTime(run, (fragment) => fragment !== "");
		}
		/**
		* Time of the first token in one compact stream per {@link isTokenDelta}, read from the
		* records themselves and stopping at the first qualifying member.
		* @param stream - compact records from one durable Assistant settlement.
		* @returns the first token's time, or undefined when the stream carries no token.
		*/
		function assistantStreamFirstTokenTime(stream) {
			for (const record of stream) {
				const time = record.type === "chunk" ? isTokenDelta$1(record.chunk) ? record.time : void 0 : runFirstTokenTime(record);
				if (time !== void 0) return time;
			}
		}
		//#endregion
		//#region lib/types/client/stats-usage.js
		/** Token accounting helpers shared by the Statistics table's step and turn levels. */
		/**
		* Whether a value is a finite number.
		* @param value - candidate value.
		* @returns true when the value is a finite number.
		*/
		function isNumber$1(value) {
			return typeof value === "number" && Number.isFinite(value);
		}
		/** Whether one usage object reported any token bucket. */
		function reported(usage) {
			return usage !== null && (usage.inputTokens !== void 0 || usage.outputTokens !== void 0 || usage.cacheReadTokens !== void 0 || usage.cacheWriteTokens !== void 0);
		}
		/**
		* Prompt length: input, cache read, and cache write do not overlap.
		* @param usage - usage reported by one assistant message.
		* @returns prompt token count, or null when nothing was reported.
		*/
		function contextTokens(usage) {
			if (!reported(usage)) return null;
			return (usage.inputTokens ?? 0) + (usage.cacheReadTokens ?? 0) + (usage.cacheWriteTokens ?? 0);
		}
		/**
		* Output tokens reported by one usage object.
		* @param usage - usage reported by one assistant message.
		* @returns output token count, or null when unreported.
		*/
		function outputTokens(usage) {
			return reported(usage) ? usage.outputTokens ?? null : null;
		}
		/**
		* Cache-read tokens reported by one usage object.
		* @param usage - usage reported by one assistant message.
		* @returns cache-read token count, or null when unreported.
		*/
		function cacheReadTokens(usage) {
			return usage?.cacheReadTokens ?? null;
		}
		/**
		* Cache-write tokens reported by one usage object.
		* @param usage - usage reported by one assistant message.
		* @returns cache-write token count, or null when unreported.
		*/
		function cacheWriteTokens(usage) {
			return usage?.cacheWriteTokens ?? null;
		}
		/**
		* Share of the prompt served from cache; null when nothing was reported.
		* @param usage - usage reported by one assistant message.
		* @returns cache hit rate in [0, 1], or null when it cannot be derived.
		*/
		function cacheHitRate(usage) {
			const context = contextTokens(usage);
			if (context === null || context <= 0) return null;
			const read = cacheReadTokens(usage);
			if (read === null) return null;
			return Math.min(1, read / context);
		}
		//#endregion
		//#region lib/types/client/stats-fold.js
		/** Event folding and per-step metric derivation for the Statistics table. */
		/** Result text kept per call for the accordion panel. */
		const TEXT_LIMIT = 16e3;
		/** Total result text kept per step; calls past the budget keep only their size. */
		const STEP_TEXT_BUDGET = 48e3;
		/** Before/after text kept per edit call. */
		const EDIT_TEXT_LIMIT = 8e3;
		/** Raw arguments kept per call. */
		const ARGS_LIMIT = 2e3;
		/** Clamp text to one line and a maximum length. */
		function clip(text, max) {
			if (typeof text !== "string") return null;
			const value = text.trim().replace(/\s+/g, " ");
			if (value === "") return null;
			return value.length <= max ? value : `${value.slice(0, max - 1)}…`;
		}
		/** Keep the last two path segments; a full path adds nothing to a table cell. */
		function shortPath(value) {
			if (value === null || value === "") return null;
			const parts = value.split(/[\\/]/).filter((part) => part !== "");
			return parts.length <= 2 ? value : parts.slice(-2).join("/");
		}
		/** Parse tool arguments into a field map, accepting structured Code-Mode values. */
		function argumentsOf(raw) {
			if (raw !== null && typeof raw === "object" && !Array.isArray(raw)) return raw;
			if (typeof raw !== "string" || raw === "") return null;
			try {
				const parsed = JSON.parse(raw);
				return parsed !== null && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : null;
			} catch {
				return null;
			}
		}
		/** Raw arguments for the inspector, bounded and stringified when structured. */
		function rawArgs(raw) {
			if (typeof raw === "string") return boundedText(raw, ARGS_LIMIT).text;
			if (raw === null || raw === void 0) return null;
			try {
				return boundedText(JSON.stringify(raw), ARGS_LIMIT).text;
			} catch {
				return null;
			}
		}
		/** Keep the first `limit` characters, flagging whether anything was cut. */
		function boundedText(text, limit) {
			if (text === "") return {
				text: null,
				truncated: false
			};
			if (text.length <= limit) return {
				text,
				truncated: false
			};
			return {
				text: text.slice(0, limit),
				truncated: true
			};
		}
		/** Line count; an empty string counts as zero lines. */
		function countLines(text) {
			if (text === "") return 0;
			let lines = 1;
			for (let index = 0; index < text.length; index += 1) if (text.charCodeAt(index) === 10) lines += 1;
			return text.charCodeAt(text.length - 1) === 10 ? lines - 1 : lines;
		}
		/** Read a record entry only when the record owns it, so model ids never resolve inherited members. */
		function ownValue(record, key) {
			return record !== null && Object.hasOwn(record, key) ? record[key] : void 0;
		}
		/** In-bounds typed-array read: the surrounding loops own the index bound. */
		function cell(values, index) {
			return values[index];
		}
		/** Longest common subsequence length, one rolling row; fine below 400 lines a side. */
		function lcsLength(left, right) {
			const width = right.length + 1;
			let previous = new Uint16Array(width);
			let current = new Uint16Array(width);
			for (let row = 1; row <= left.length; row += 1) {
				for (let column = 1; column <= right.length; column += 1) {
					const up = cell(previous, column);
					const leftCell = cell(current, column - 1);
					current[column] = left[row - 1] === right[column - 1] ? cell(previous, column - 1) + 1 : Math.max(up, leftCell);
				}
				const swap = previous;
				previous = current;
				current = swap;
				current.fill(0);
			}
			return cell(previous, right.length);
		}
		/** Lines actually added and removed by one replacement. */
		function diffLines(before, after) {
			const left = before === "" ? [] : before.split("\n");
			const right = after === "" ? [] : after.split("\n");
			if (left.length === 0) return {
				added: right.length,
				removed: 0
			};
			if (right.length === 0) return {
				added: 0,
				removed: left.length
			};
			if (left.length > 400 || right.length > 400) return {
				added: Math.max(0, right.length - left.length),
				removed: Math.max(0, left.length - right.length)
			};
			const common = lcsLength(left, right);
			return {
				added: right.length - common,
				removed: left.length - common
			};
		}
		/** Line delta of an edit or write call, or null when the tool has no such measure. */
		function editDelta(name, raw) {
			if (name !== "edit" && name !== "write" && name !== "apply_patch") return null;
			const args = argumentsOf(raw);
			if (args === null) return null;
			const pick = (...keys) => {
				for (const key of keys) {
					const value = args[key];
					if (typeof value === "string") return value;
				}
				return null;
			};
			if (name === "write") {
				const content = pick("content", "text", "file_text");
				if (content === null) return null;
				return {
					added: countLines(content),
					removed: 0,
					before: null,
					after: boundedText(content, EDIT_TEXT_LIMIT).text
				};
			}
			const beforeText = pick("old_string", "old_str", "old");
			const afterText = pick("new_string", "new_str", "new");
			if (beforeText === null && afterText === null) return null;
			return {
				...diffLines(beforeText ?? "", afterText ?? ""),
				before: boundedText(beforeText ?? "", EDIT_TEXT_LIMIT).text,
				after: boundedText(afterText ?? "", EDIT_TEXT_LIMIT).text
			};
		}
		/** Short call description: file, pattern, command, or the first argument worth naming. */
		function toolHint(name, raw) {
			const args = argumentsOf(raw);
			if (args === null) return null;
			const pick = (...keys) => {
				for (const key of keys) {
					const value = args[key];
					if (typeof value === "string" && value.trim() !== "") return value.trim();
				}
				return null;
			};
			switch (name) {
				case "read":
				case "write":
				case "edit": return clip(shortPath(pick("file_path", "path", "file")), 34);
				case "grep":
				case "glob": {
					const pattern = pick("pattern");
					if (pattern === null) return null;
					const include = pick("include");
					return clip(include === null ? pattern : `${pattern} (${include})`, 34);
				}
				case "pwsh":
				case "shell":
				case "bash": return clip((pick("description") ?? pick("command") ?? "").split("\n")[0], 30);
				case "web_fetch": {
					const url = pick("url");
					return url === null ? null : clip(url.replace(/^https?:\/\//, ""), 34);
				}
				case "web_search": return clip(pick("query"), 30);
				case "run_code":
				case "RunCode":
				case "runCode": return clip(pick("description"), 34);
				default: return clip(pick("description", "query", "path", "file_path", "provider", "method", "name"), 30);
			}
		}
		/**
		* Whether one tool name is the Code-Mode runner; its real tools run inside it.
		* @param name - tool name, or null for a nameless call.
		* @returns true when the name is the Code-Mode runner.
		*/
		function isCodeMode(name) {
			return name !== null && /^run_?code$/i.test(name);
		}
		/** Arguments-derived StatsCall fields shared by a native call and one Code-Mode dispatch. */
		function callArguments(name, raw) {
			const delta = editDelta(name, raw);
			return {
				hint: toolHint(name, raw),
				added: delta === null ? null : delta.added,
				removed: delta === null ? null : delta.removed,
				before: delta === null ? null : delta.before,
				after: delta === null ? null : delta.after,
				args: rawArgs(raw)
			};
		}
		/** Whether one stream chunk counts as the first model token. */
		function isTokenDelta(chunk) {
			switch (chunk.type) {
				case "text-delta":
				case "reasoning-delta": return chunk.text !== "";
				case "tool-call-delta": return chunk.argumentsDelta !== "" || chunk.name !== void 0;
				default: return false;
			}
		}
		const TEAM_MESSAGE_PREFIX = /^Team message from ([^:]+):/u;
		const AGENT_MESSAGE_PREFIX = /^Agent (\S+) sent a message:/u;
		const OPAQUE_SESSION_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu;
		/**
		* Sender agent name of a teammate/agent notification, parsed from its body prefix.
		* @param message - waking user message stamped with its producer source.
		* @returns the readable member name, or null when the frame omits it or carries an opaque session id.
		*/
		function triggerSenderOf(message) {
			const kind = message.source.kind;
			if (kind !== "agent-message" && kind !== "team-message") return null;
			let text = "";
			for (const block of message.content) if (block.type === "text") text += block.text;
			const team = TEAM_MESSAGE_PREFIX.exec(text);
			/* v8 ignore next -- a matched capture group is always present. */
			if (team !== null) return team[1]?.trim() ?? null;
			const id = AGENT_MESSAGE_PREFIX.exec(text)?.[1] ?? null;
			return id !== null && OPAQUE_SESSION_ID.test(id) ? null : id;
		}
		/** Answer characters kept for the one-line Did preview. */
		const ANSWER_LIMIT = 400;
		/** Answer characters kept for the expandable body. */
		const ANSWER_BODY_LIMIT = 16e3;
		/** Trimmed answer text, its bounded full body, and whether the body was clipped. */
		function answerOf(message) {
			const parts = [];
			for (const block of message.content) if (block.type === "text") parts.push(block.text);
			const text = parts.join("\n").trim();
			if (text === "") return null;
			return {
				preview: text.length > ANSWER_LIMIT ? text.slice(0, ANSWER_LIMIT) : text,
				body: text.length > ANSWER_BODY_LIMIT ? text.slice(0, ANSWER_BODY_LIMIT) : text,
				truncated: text.length > ANSWER_BODY_LIMIT
			};
		}
		/**
		* Initial state of one step or one turn-level marker.
		* @param turn - turn number.
		* @param step - step number, or the marker's pseudo step.
		* @param mark - marker kind; steps pass null.
		* @returns the empty state.
		*/
		function newState(turn, step, mark = null) {
			return {
				turn,
				step,
				startSeq: null,
				startTime: null,
				endTime: null,
				ended: false,
				firstTokenTime: null,
				messageTime: null,
				usage: null,
				interrupted: false,
				toolPairs: 0,
				toolMs: 0,
				toolOpen: 0,
				toolMissing: 0,
				pending: null,
				tools: null,
				calls: null,
				starts: null,
				retries: 0,
				toolErrors: 0,
				mark,
				prompt: null,
				images: [],
				closed: false,
				closeKind: null,
				trigger: null,
				triggerSender: null,
				answer: null,
				answerBody: null,
				answerTruncated: false
			};
		}
		/** Result text: the text blocks of the tool message joined by newlines. */
		function resultText(content) {
			const parts = [];
			for (const block of content) if (block.type === "text") parts.push(block.text);
			return parts.length === 0 ? null : parts.join("\n");
		}
		/** Size, line count, and bounded body of one tool result. */
		function resultInfo(content) {
			const text = resultText(content);
			if (text === null) return {
				chars: 0,
				lines: 0,
				text: null,
				truncated: false
			};
			let lines = 0;
			for (let index = 0; index < text.length; index += 1) if (text.charCodeAt(index) === 10) lines += 1;
			if (text.charCodeAt(text.length - 1) !== 10) lines += 1;
			const body = boundedText(text, TEXT_LIMIT);
			return {
				chars: text.length,
				lines,
				text: body.text,
				truncated: body.truncated
			};
		}
		/**
		* User question: only a human-typed `user` source is the text the user wrote.
		* @param message - one user-role message from the Session log.
		* @returns the one-line question and the durable images it attached.
		*/
		function promptOf(message) {
			const parts = [];
			const images = [];
			for (const block of message.content) {
				if (block.type === "text") parts.push(block.text);
				if (block.type === "image") images.push(block.attachment);
			}
			return {
				text: clip(parts.join(" "), 400),
				images
			};
		}
		/** Add one call of `name` to the tally, preserving first-call order for ties. */
		function bumpTool(tools, name) {
			const next = { ...tools ?? {} };
			next[name] = (next[name] ?? 0) + 1;
			return next;
		}
		/**
		* Fold one session event into the step state. Events other than `step/start`
		* create a state when the window starts mid-step, so context and throughput
		* still show while start-dependent metrics stay unrecorded.
		* @param state - current state, or undefined before the first foldable event.
		* @param event - one durable or client-only session event.
		* @returns the next state, or undefined when the event needs a start it lacks.
		*/
		function foldEvent(state, event) {
			switch (event.type) {
				case "step/start": return {
					...newState(event.data.turn, event.data.step),
					startSeq: event.seq,
					startTime: event.time
				};
				case "assistant/live-chunk": {
					const next = state ?? newState(event.data.turn, event.data.step);
					const chunk = event.data.chunk;
					if (chunk.type === "usage") return {
						...next,
						usage: chunk.usage
					};
					if (next.firstTokenTime === null && isTokenDelta(chunk)) return {
						...next,
						firstTokenTime: event.time
					};
					return next;
				}
				case "assistant/attempt":
				case "assistant/message": {
					let next = state ?? newState(event.data.turn, event.data.step);
					if (next.firstTokenTime === null) {
						const fromStream = assistantStreamFirstTokenTime(event.data.stream);
						if (isNumber$1(fromStream)) next = {
							...next,
							firstTokenTime: fromStream
						};
					}
					if (event.type === "assistant/attempt") return next;
					const answer = answerOf(event.data.message);
					return {
						...next,
						messageTime: next.messageTime ?? event.time,
						usage: event.data.usage ?? next.usage,
						interrupted: event.data.interrupted === true,
						...answer === null ? {} : {
							answer: answer.preview,
							answerBody: answer.body,
							answerTruncated: answer.truncated
						}
					};
				}
				case "tool/call": {
					const next = state ?? newState(event.data.turn, event.data.step);
					const callId = event.data.callId;
					const pending = { ...next.pending ?? {} };
					const known = Object.hasOwn(pending, callId);
					pending[callId] = event.time;
					const name = event.data.name;
					const calls = { ...next.calls ?? {} };
					if (!Object.hasOwn(calls, callId)) calls[callId] = {
						callId,
						seq: event.seq,
						name,
						...callArguments(name, event.data.arguments),
						chars: null,
						lines: null,
						error: false,
						inner: false,
						text: null,
						truncated: false,
						ms: null,
						startedAt: event.time,
						endedAt: null
					};
					return {
						...next,
						pending,
						toolOpen: known ? next.toolOpen : next.toolOpen + 1,
						tools: bumpTool(next.tools, name),
						calls
					};
				}
				case "llm/retry": return {
					...state ?? newState(event.data.turn, event.data.step),
					retries: (state?.retries ?? 0) + 1
				};
				/**
				* Inner Code-Mode call: the name and arguments are the real tool's, the
				* content is the real result. It does not count toward the outer call's
				* tool time; its own dispatch span is measured instead.
				*/
				case "tool/ptc-dispatch-start": {
					if (state === void 0) return state;
					const subCallId = event.data.subCallId;
					return {
						...state,
						starts: {
							...state.starts ?? {},
							[subCallId]: event.time
						}
					};
				}
				case "tool/ptc-dispatch": {
					const next = state;
					if (next === void 0) return next;
					const name = event.data.name;
					const info = resultInfo(event.data.content);
					const callKey = event.data.subCallId;
					const startedAt = ownValue(next.starts, callKey);
					const calls = { ...next.calls ?? {} };
					calls[callKey] = {
						callId: callKey,
						seq: event.seq,
						name,
						...callArguments(name, event.data.arguments),
						chars: info.chars,
						lines: info.lines,
						text: info.text,
						truncated: info.truncated,
						error: event.data.isError,
						inner: true,
						ms: isNumber$1(startedAt) ? Math.max(0, event.time - startedAt) : null,
						startedAt: isNumber$1(startedAt) ? startedAt : null,
						endedAt: event.time
					};
					return {
						...next,
						calls,
						tools: bumpTool(next.tools, name),
						toolErrors: event.data.isError ? next.toolErrors + 1 : next.toolErrors
					};
				}
				case "tool/result": {
					const next = state ?? newState(event.data.turn, event.data.step);
					const message = event.data.message;
					const callId = message.source.callId;
					const failed = message.isError === true;
					const before = ownValue(next.calls, callId);
					const calls = { ...next.calls ?? {} };
					if (before !== void 0) calls[callId] = {
						...before,
						...resultInfo(message.content),
						error: failed || before.error,
						/* v8 ignore next -- a native call record always carries the start time tool/call recorded. */
						ms: isNumber$1(before.startedAt) ? Math.max(0, event.time - before.startedAt) : before.ms,
						endedAt: event.time
					};
					const withError = failed ? {
						...next,
						toolErrors: next.toolErrors + 1
					} : next;
					const dispatched = ownValue(next.pending, callId);
					if (!isNumber$1(dispatched)) return {
						...withError,
						calls,
						toolMissing: next.toolMissing + 1
					};
					const pending = {};
					/* v8 ignore next -- a numeric pending start implies pending exists; the empty fallback is unreachable. */
					for (const [key, value] of Object.entries(next.pending ?? {})) if (key !== callId) pending[key] = value;
					return {
						...withError,
						calls,
						pending,
						toolOpen: Math.max(0, next.toolOpen - 1),
						toolPairs: next.toolPairs + 1,
						toolMs: next.toolMs + Math.max(0, event.time - dispatched)
					};
				}
				case "step/end": return {
					...state ?? newState(event.data.turn, event.data.step),
					ended: true,
					endTime: event.time
				};
				default: return state;
			}
		}
		/** Calls of one step in call order, bounded per call and per step. */
		function callList(calls) {
			if (calls === null) return [];
			const list = Object.values(calls).sort((left, right) => left.seq - right.seq).slice(0, 12);
			let used = 0;
			return list.map((call) => {
				const size = call.text === null ? 0 : call.text.length;
				if (size === 0) return call;
				if (used + size <= STEP_TEXT_BUDGET) {
					used += size;
					return call;
				}
				return {
					...call,
					text: null,
					truncated: true
				};
			});
		}
		/** Tool tallies by descending count; ties keep first-call order. */
		function sortedTools(tools) {
			if (tools === null) return [];
			return Object.entries(tools).map(([name, count]) => ({
				name,
				count
			})).sort((left, right) => right.count - left.count);
		}
		/**
		* Wall span covered by calls that both started and settled; overlaps count once.
		* @param calls - calls with their paired start and end times.
		* @returns the span in milliseconds, or null when no call is paired.
		*/
		function toolSpanUnion(calls) {
			const intervals = calls.filter((call) => isNumber$1(call.startedAt) && isNumber$1(call.endedAt)).map((call) => [call.startedAt, call.endedAt]).sort((left, right) => left[0] - right[0]);
			if (intervals.length === 0) return null;
			let total = 0;
			let [start, end] = intervals[0];
			for (const [nextStart, nextEnd] of intervals.slice(1)) if (nextStart > end) {
				total += end - start;
				start = nextStart;
				end = nextEnd;
			} else if (nextEnd > end) end = nextEnd;
			return total + end - start;
		}
		/** Tool result characters one step pushed into the next prompt. */
		function outputChars(calls) {
			let total = 0;
			for (const call of calls) if (isNumber$1(call.chars)) total += call.chars;
			return total;
		}
		/**
		* Measured metrics of one step.
		* @param state - folded step state.
		* @returns the row values the table renders.
		*/
		function stepMetrics(state) {
			const start = state.startTime;
			const first = state.firstTokenTime;
			const message = state.messageTime;
			const context = contextTokens(state.usage);
			const output = outputTokens(state.usage);
			const ttft = isNumber$1(start) && isNumber$1(first) ? Math.max(0, first - start) : null;
			const model = isNumber$1(start) && isNumber$1(message) ? Math.max(0, message - start) : null;
			const decode = isNumber$1(first) && isNumber$1(message) ? Math.max(0, message - first) : null;
			const tps = isNumber$1(decode) && decode > 0 && isNumber$1(output) && output > 0 ? output / (decode / 1e3) : null;
			const settled = isNumber$1(message);
			const calls = callList(state.calls);
			const allCalls = state.calls === null ? [] : Object.values(state.calls);
			return {
				turn: state.turn,
				step: state.step,
				startTime: isNumber$1(start) ? start : null,
				endTime: isNumber$1(state.endTime) ? state.endTime : null,
				context,
				ttft,
				model,
				decode,
				tps,
				output,
				tool: state.toolPairs > 0 ? state.toolMs : null,
				toolWall: toolSpanUnion(allCalls),
				tools: sortedTools(state.tools),
				calls,
				toolOutput: outputChars(calls),
				retries: state.retries,
				toolErrors: state.toolErrors,
				answer: state.answer,
				answerBody: state.answerBody,
				answerTruncated: state.answerTruncated,
				cacheRead: cacheReadTokens(state.usage),
				cacheWrite: cacheWriteTokens(state.usage),
				cacheHit: cacheHitRate(state.usage),
				wall: isNumber$1(start) && isNumber$1(state.endTime) ? Math.max(0, state.endTime - start) : null,
				toolPending: state.toolOpen + state.toolMissing,
				running: !state.ended,
				interrupted: state.interrupted,
				aborted: state.ended && !settled,
				mark: state.mark,
				prompt: state.prompt,
				images: state.images,
				closed: state.closed,
				closeKind: state.closeKind
			};
		}
		/**
		* State rebuilt from the matches a window still holds when its state is absent.
		* Event order is preserved, so `step/start` and `step/end` restore the
		* boundaries whenever they survived.
		* @param matches - accepted matches in ascending sequence order.
		* @returns the folded state, or undefined when nothing was foldable.
		*/
		function fallbackState(matches) {
			let state;
			for (const match of matches) state = foldEvent(state, match.event);
			return state;
		}
		//#endregion
		//#region lib/types/client/stats-compaction.js
		/** Compaction transaction state and its presentation as a synthetic step row. */
		/**
		* Compaction id an event names.
		* @param event - event seen by the Definition match.
		* @returns the nonempty compaction id, or null for events outside a transaction.
		*/
		function compactionEventId(event) {
			if (event.type !== "compaction/start" && event.type !== "compaction/summary" && event.type !== "compaction/end") return null;
			const value = event.data.compactionId;
			return typeof value === "string" && value !== "" ? value : null;
		}
		/** Text characters of a content-block list, for the summary size note. */
		function textChars(blocks) {
			let chars = 0;
			for (const block of blocks) if (block.type === "text" && typeof block.text === "string") chars += block.text.length;
			return chars;
		}
		/**
		* Start state for one `compaction/start`.
		* @param event - the start event.
		* @returns state that later summary and end events refine.
		*/
		function newCompactionState(event) {
			/* v8 ignore next 3 -- the Definition only calls start for a compaction/start match. */
			if (event.type !== "compaction/start") throw new Error("stats-compaction start requires compaction/start");
			return {
				compactionId: event.data.compactionId,
				turn: event.data.turn,
				startSeq: event.seq,
				startTime: event.time,
				summarySeq: null,
				endSeq: null,
				endTime: null,
				error: null,
				provider: null,
				model: null,
				usage: null,
				shadowedTokens: null,
				summaryChars: null,
				checkpointSeq: null
			};
		}
		/**
		* Fold the summary or end of a compaction into its state.
		* @param state - state opened by compaction/start.
		* @param event - a matching compaction event.
		* @returns state after the event, or the same state for unrelated events.
		*/
		function foldCompaction(state, event) {
			if (event.type === "compaction/summary") return {
				...state,
				summarySeq: event.seq,
				provider: event.data.provider,
				model: event.data.model,
				usage: event.data.usage ?? null,
				shadowedTokens: event.data.shadowedTokenCount,
				summaryChars: textChars(event.data.summary)
			};
			/* v8 ignore next -- a summary returned above; only compaction/end reaches this branch. */
			if (event.type === "compaction/end") return {
				...state,
				turn: state.turn ?? event.data.turn,
				endSeq: event.seq,
				endTime: event.time,
				error: event.data.error ?? null
			};
			/* v8 ignore next -- the Definition routes only summary and end updates here. */
			return state;
		}
		/**
		* View metrics of one compaction transaction.
		* @param state - folded compaction state.
		* @returns the row values; unavailable measurements stay null.
		*/
		function compactionMetrics(state) {
			const usage = state.usage;
			const endTime = state.endTime;
			return {
				compactionId: state.compactionId,
				turn: state.turn,
				startTime: state.startTime,
				endTime,
				wall: isNumber$1(endTime) ? Math.max(0, endTime - state.startTime) : null,
				running: state.endSeq === null,
				context: contextTokens(usage),
				output: outputTokens(usage),
				cacheHit: cacheHitRate(usage),
				cacheRead: cacheReadTokens(usage),
				cacheWrite: cacheWriteTokens(usage),
				provider: state.provider,
				model: state.model,
				shadowedTokens: state.shadowedTokens,
				summaryChars: state.summaryChars,
				error: state.error
			};
		}
		//#endregion
		//#region lib/types/client/stats-event.js
		/** Non-step event rows: request admission, prompt/tool changes, and model-free prunes. */
		/**
		* Payload of an event whose type is known to the caller.
		* @param event - event seen by the Definition match.
		* @param type - event discriminant already narrowed by the caller.
		* @returns the typed event payload.
		*/
		function dataOf(event, type) {
			return event.data;
		}
		/**
		* Turn and step a Location resolves.
		* @param location - engine Location of a match.
		* @returns the resolved turn and step, or null when the window lost them.
		*/
		function locationTurnStep(location) {
			if (location === void 0) return null;
			if (location.kind === "turn") return {
				turn: location.turn.turn,
				step: null
			};
			if (location.kind === "step") return {
				turn: location.turn.turn,
				step: location.step.step
			};
			return null;
		}
		/** Text characters of a message content list. */
		function contentChars(content) {
			let chars = 0;
			for (const block of content) if (block.type === "text" && typeof block.text === "string") chars += block.text.length;
			return chars;
		}
		/** Tool names added or removed by one developer message. */
		function toolChanges(content) {
			const added = [];
			const removed = [];
			for (const block of content) {
				if (typeof block.toolName !== "string") continue;
				if (block.type === "tool-addition") added.push(block.toolName);
				else if (block.type === "tool-removal") removed.push(block.toolName);
			}
			return {
				added,
				removed
			};
		}
		/**
		* Normalize one non-step event into row values.
		* @param event - a session event seen by the Definition match.
		* @param location - engine Location of the event, for turn and step.
		* @returns row metrics, or null for events the table does not explain.
		*/
		function eventMetrics(event, location) {
			const located = locationTurnStep(location);
			const base = {
				turn: located?.turn ?? null,
				step: located?.step ?? null,
				time: event.time,
				reason: null,
				series: false,
				provider: null,
				model: null,
				toolCount: null,
				surfaceOp: null,
				chars: null,
				added: [],
				removed: [],
				shadowedTokens: null,
				status: null,
				retry: null,
				delayMs: null,
				error: null,
				label: null
			};
			if (event.type === "request/header") {
				const header = event.data.header;
				return {
					...base,
					kind: "request-header",
					reason: event.data.reason,
					series: event.data.startsSeries === true,
					provider: header.config.provider,
					model: header.config.model,
					toolCount: header.tools?.length ?? null
				};
			}
			if (event.type === "system/message") {
				const chars = contentChars(event.data.message.content);
				const action = event.surfaceOp === "append" ? "append" : chars === 0 ? "clear" : "replace";
				return {
					...base,
					kind: "system-prompt",
					surfaceOp: action,
					chars
				};
			}
			if (event.type === "developer/message") {
				if (event.data.message.source.kind !== "tool-registry") return null;
				const changes = toolChanges(event.data.message.content);
				if (changes.added.length === 0 && changes.removed.length === 0) return null;
				return {
					...base,
					kind: "tool-set",
					...changes
				};
			}
			if (event.type === "compaction/prune") return {
				...base,
				kind: "context-prune",
				shadowedTokens: event.data.shadowedTokenCount
			};
			const eventType = event.type;
			if (eventType === "request/context") {
				const data = dataOf(event, "request/context");
				return {
					...base,
					kind: "request-context",
					provider: data.provider,
					model: data.model,
					status: data.systemPromptUpdate ?? null,
					chars: data.contextWindow ?? null
				};
			}
			if (eventType === "llm/retry") {
				const data = dataOf(event, "llm/retry");
				return {
					...base,
					kind: "model-retry",
					turn: data.turn,
					step: data.step,
					provider: data.provider,
					status: data.mode,
					retry: data.retry,
					delayMs: data.delayMs,
					error: data.failure.message
				};
			}
			if (eventType === "assistant/attempt") {
				const data = dataOf(event, "assistant/attempt");
				return {
					...base,
					kind: "model-attempt",
					turn: data.turn,
					step: data.step,
					status: "attempt"
				};
			}
			if (eventType === "user/message") {
				const data = dataOf(event, "user/message");
				const kind = data.source.kind;
				if (kind !== "runtime-context" && kind !== "time-context") return null;
				return {
					...base,
					kind: "context-refresh",
					status: kind,
					chars: contentChars(data.content)
				};
			}
			if (eventType === "approval/asked") {
				const data = dataOf(event, "approval/asked");
				return {
					...base,
					kind: "approval",
					status: "asked",
					label: data.toolName,
					error: data.reason ?? null
				};
			}
			if (eventType === "approval/decided") {
				const data = dataOf(event, "approval/decided");
				return {
					...base,
					kind: "approval",
					status: data.outcome
				};
			}
			return null;
		}
		//#endregion
		//#region lib/types/client/stats-definition.js
		/**
		* Conversation Definitions producing the performance target's nodes: one step
		* context per `step/start`, one prompt context per user message, one end marker
		* per turn, and one context per Code-Mode root call for its inner dispatches.
		*
		* Every callback is a pure function of the events it receives: turn and step
		* identity comes from the engine's match location, never from module state, so
		* one Definition set serves every Session binding and history paging
		* (`replaceWindow`/`prepend`) cannot misattribute events.
		*/
		/** Target name shared by every Definition and the view builder. */
		const TARGET = "stats";
		/** Definition kind of one measured step. */
		const STEP_KIND = "stats-step";
		/** Definition kind of one user prompt. */
		const PROMPT_KIND = "stats-prompt";
		/** Definition kind of a turn's end marker. */
		const END_KIND = "stats-turn-end";
		/** Definition kind of one Code-Mode root call's inner dispatches. */
		const CODE_MODE_KIND = "stats-code-mode";
		/** Definition kind of one compaction transaction. */
		const COMPACTION_KIND = "stats-compaction";
		/** Definition kind of one non-step event row. */
		const EVENT_KIND = "stats-event";
		/** Marker ids never collide with a step number. */
		const PROMPT_STEP = 0;
		const END_STEP = 1e5;
		/** Location a published node carries: the start match's, else the newest match's. */
		function nodeLocation(context) {
			return context.start?.location ?? context.matches.at(-1)?.location ?? { kind: "unresolved" };
		}
		/** Wrap one contribution in the target's node envelope. */
		function statsNode(context, anchorSeq, data) {
			return {
				key: context.key,
				kind: context.kind,
				id: context.id,
				target: TARGET,
				anchorSeq,
				location: nodeLocation(context),
				data
			};
		}
		/** State a step-like Context folds from the matches a trimmed window still holds. */
		function stepState(context) {
			return context.state ?? fallbackState(context.matches);
		}
		/** One measured step per `step/start`, updated by its stream, tools, and end. */
		const stepDefinition = {
			kind: STEP_KIND,
			target: TARGET,
			match: (event) => {
				switch (event.type) {
					case "step/start": return {
						id: `${event.data.turn}:${event.data.step}`,
						role: "start"
					};
					case "step/end":
					case "assistant/live-chunk":
					case "assistant/attempt":
					case "assistant/message":
					case "llm/retry":
					case "tool/call":
					case "tool/result": return {
						id: `${event.data.turn}:${event.data.step}`,
						role: "update"
					};
					default: return null;
				}
			},
			start: (_context, match) => {
				if (match.event.type !== "step/start") throw new Error("stats-step start requires step/start");
				return {
					...newState(match.event.data.turn, match.event.data.step),
					startSeq: match.event.seq,
					startTime: match.event.time
				};
			},
			/* v8 ignore next -- the engine calls update only after start adopted a state, so folding never returns undefined here. */
			update: (context, match) => foldEvent(context.state, match.event) ?? context.state,
			publication: (match) => {
				if (match.event.type === "step/start") return "none";
				if (match.event.type === "assistant/live-chunk" || match.event.type === "assistant/attempt") return "none";
				return "immediate";
			},
			buildViewNode: (context) => {
				const state = stepState(context);
				if (state === void 0) return null;
				return statsNode(context, state.startSeq ?? context.start?.event.seq ?? 0, {
					kind: "step",
					metrics: stepMetrics(state)
				});
			}
		};
		/** The trigger of one turn: one Context per `user/message`, classified by source. */
		const promptDefinition = {
			kind: PROMPT_KIND,
			target: TARGET,
			match: (event) => event.type === "user/message" ? {
				id: `${event.seq}`,
				role: "start"
			} : null,
			start: (_context, match) => {
				if (match.event.type !== "user/message") throw new Error("stats-prompt start requires user/message");
				const located = locationTurnStep(match.location);
				const prompt = promptOf(match.event.data);
				return {
					...newState(located?.turn ?? 0, PROMPT_STEP, "prompt"),
					prompt: prompt.text,
					images: prompt.images,
					trigger: match.event.data.source.kind,
					triggerSender: triggerSenderOf(match.event.data)
				};
			},
			update: (context) => context.state,
			publication: () => "immediate",
			buildViewNode: (context) => {
				const state = context.state;
				const located = locationTurnStep(nodeLocation(context));
				if (state === void 0 || located === null) return null;
				return statsNode(context, context.start?.event.seq ?? 0, {
					kind: "prompt",
					turn: located.turn,
					prompt: state.prompt,
					images: state.images,
					source: state.trigger ?? "other",
					sender: state.triggerSender
				});
			}
		};
		/** Closes a turn, including an interrupted tail whose last step never ended. */
		const endDefinition = {
			kind: END_KIND,
			target: TARGET,
			match: (event) => event.type === "turn/end" ? {
				id: `${event.data.turn}:${END_STEP}`,
				role: "start"
			} : null,
			start: (_context, match) => {
				if (match.event.type !== "turn/end") throw new Error("stats-turn-end start requires turn/end");
				return {
					...newState(match.event.data.turn, END_STEP, "end"),
					ended: true,
					endTime: match.event.time,
					closed: true,
					closeKind: match.event.data.reason.kind
				};
			},
			update: (context) => context.state,
			buildViewNode: (context) => {
				const state = context.state;
				if (state === void 0) return null;
				return statsNode(context, context.start?.event.seq ?? 0, {
					kind: "end",
					turn: state.turn,
					closeKind: state.closeKind,
					endTime: state.endTime
				});
			}
		};
		/**
		* One Context per Code-Mode root call folds its inner dispatches; the builder
		* merges them into the owning step by the root call id. The events carry no
		* turn or step, so the Context takes both from the engine's match location.
		*/
		const codeModeDefinition = {
			kind: CODE_MODE_KIND,
			target: TARGET,
			match: (event) => {
				if (event.type !== "tool/ptc-dispatch-start" && event.type !== "tool/ptc-dispatch") return null;
				return {
					id: `code:${event.data.rootCallId}`,
					role: event.type === "tool/ptc-dispatch-start" ? "start" : "update"
				};
			},
			start: (_context, match) => {
				if (match.event.type !== "tool/ptc-dispatch-start") throw new Error("stats-code-mode start requires tool/ptc-dispatch-start");
				const located = locationTurnStep(match.location);
				const base = located?.step === null || located === null ? newState(0, 0) : newState(located.turn, located.step);
				/* v8 ignore next -- folding a defined state always returns a state. */
				return foldEvent(base, match.event) ?? base;
			},
			/* v8 ignore next -- the engine calls update only after start adopted a state, so folding never returns undefined here. */
			update: (context, match) => foldEvent(context.state, match.event) ?? context.state,
			buildViewNode: (context) => {
				const state = stepState(context);
				const located = locationTurnStep(nodeLocation(context));
				if (state === void 0 || located?.step === null || located === null) return null;
				const calls = stepMetrics(state).calls.filter((call) => !isCodeMode(call.name)).slice(0, 12);
				return statsNode(context, context.start?.event.seq ?? 0, {
					kind: "inner",
					turn: located.turn,
					step: located.step,
					calls
				});
			}
		};
		/** One synthetic row per compaction transaction, including its summary model call. */
		const compactionDefinition = {
			kind: COMPACTION_KIND,
			target: TARGET,
			match: (event) => {
				const id = compactionEventId(event);
				if (id === null) return null;
				return {
					id,
					role: event.type === "compaction/start" ? "start" : "update"
				};
			},
			start: (_context, match) => {
				if (match.event.type !== "compaction/start") throw new Error("stats-compaction start requires compaction/start");
				return newCompactionState(match.event);
			},
			update: (context, match) => foldCompaction(context.state, match.event),
			publication: () => "immediate",
			buildViewNode: (context) => {
				const state = context.state;
				if (state === void 0) return null;
				return {
					key: context.key,
					kind: context.kind,
					id: context.id,
					target: TARGET,
					anchorSeq: state.startSeq,
					/* v8 ignore next -- a live start match always carries a Location. */
					location: context.start?.location ?? { kind: "unresolved" },
					data: {
						kind: "compaction",
						metrics: compactionMetrics(state)
					}
				};
			}
		};
		/** One row per non-step event that explains request, prompt, tool, or prune changes. */
		const eventDefinition = {
			kind: EVENT_KIND,
			target: TARGET,
			match: (event) => eventMetrics(event, void 0) === null ? null : {
				id: `${event.type}:${event.seq}`,
				role: "start"
			},
			start: (_context, match) => {
				const metrics = eventMetrics(match.event, match.location);
				/* v8 ignore next 2 -- match accepted only events this normalizer handles. */
				if (metrics === null) throw new Error("stats-event start requires a supported event");
				return metrics;
			},
			/* v8 ignore next -- event rows are created by their start match and never updated. */
			update: (context) => context.state,
			buildViewNode: (context) => {
				const state = context.state;
				const start = context.start;
				/* v8 ignore next 2 -- a published node always retains its start match and state. */
				if (state === void 0 || start === void 0) return null;
				return {
					key: context.key,
					kind: context.kind,
					id: context.id,
					target: TARGET,
					anchorSeq: start.event.seq,
					location: start.location,
					data: {
						kind: "event",
						metrics: state
					}
				};
			}
		};
		/**
		* Register the six Definitions. Each registration rides the caller's fiber
		* effect, so unloading the plugin removes the nodes with it.
		* @param ctx - client root context.
		*/
		function registerStatsDefinitions(ctx) {
			ctx.uiConversation.events.register(stepDefinition);
			ctx.uiConversation.events.register(promptDefinition);
			ctx.uiConversation.events.register(endDefinition);
			ctx.uiConversation.events.register(codeModeDefinition);
			ctx.uiConversation.events.register(compactionDefinition);
			ctx.uiConversation.events.register(eventDefinition);
		}
		//#endregion
		//#region lib/types/client/stats-aggregate.js
		/** Turn aggregation over the step metrics a window holds. */
		/** Empty image list shared by turns whose trigger attached no image. */
		const NO_IMAGES = [];
		/** Empty compaction list shared by turns without one. */
		const NO_COMPACTIONS = [];
		/** Empty event-row list shared by turns without one. */
		const NO_EVENTS = [];
		/** Human prompts outrank automation context; ties keep the first message. */
		function triggerRank(source) {
			switch (source) {
				case "user": return 4;
				case "team-message": return 3;
				case "agent-message": return 2;
				case "subagent-settled": return 2;
				case "goal": return 1;
				case "schedule":
				case "webhook":
				case "tool-jobs":
				case "cordis-host-runner": return 1;
				default: return 0;
			}
		}
		/** Context measured last in the turn: steps without usage keep the previous value. */
		function lastContext(steps) {
			let context = null;
			for (const step of steps) if (isNumber$1(step.context)) context = step.context;
			return context;
		}
		/** Token-weighted cache level of the turn, over the steps that reported both read and prompt tokens. */
		function turnCache(steps) {
			let read = 0;
			let write = 0;
			let context = 0;
			let measured = false;
			for (const step of steps) {
				if (!isNumber$1(step.cacheRead) || !isNumber$1(step.context)) continue;
				read += step.cacheRead;
				write += step.cacheWrite ?? 0;
				context += step.context;
				measured = true;
			}
			return {
				rate: measured && context > 0 ? Math.min(1, read / context) : null,
				read: measured ? read : null,
				write: measured ? write : null,
				context: measured ? context : null
			};
		}
		/** Tool tallies by descending count; ties keep call order. */
		function tallyCalls(calls) {
			const counts = /* @__PURE__ */ new Map();
			for (const call of calls) {
				if (call.name === null) continue;
				counts.set(call.name, (counts.get(call.name) ?? 0) + 1);
			}
			return [...counts.entries()].map(([name, count]) => ({
				name,
				count
			})).sort((left, right) => right.count - left.count);
		}
		/**
		* Merge the Code-Mode dispatches of one step into its row. The runner call
		* disappears in favor of the real tools, so the row shows what the code called
		* and the characters those results pushed into the next prompt.
		* @param step - step row folded from native events only.
		* @param inner - the step's Code-Mode contributions.
		* @returns the merged row, or the original when no dispatch was recorded.
		*/
		function mergeInnerCalls(step, inner) {
			if (inner.length === 0) return step;
			const innerCalls = inner.flatMap((contribution) => contribution.calls);
			const calls = [...step.calls.filter((call) => !isCodeMode(call.name)), ...innerCalls].sort((left, right) => left.seq - right.seq).slice(0, 12);
			const chars = calls.reduce((sum, call) => sum + (call.chars ?? 0), 0);
			return {
				...step,
				calls,
				tools: tallyCalls(calls),
				toolOutput: chars,
				toolWall: toolSpanUnion(calls),
				toolErrors: step.toolErrors + innerCalls.filter((call) => call.error).length
			};
		}
		/**
		* Aggregate one turn's steps into the row values the table shows.
		* @param turn - turn number.
		* @param steps - the turn's step metrics in step order.
		* @returns the turn row.
		*/
		function aggregateTurn(turn, steps) {
			let ttftSum = 0;
			let ttftUnknown = 0;
			let modelSum = 0;
			let modelUnknown = 0;
			let decodeSum = 0;
			let decodeUnknown = 0;
			let outputSum = 0;
			let outputUnknown = 0;
			let toolSum = 0;
			let toolPairable = 0;
			let toolPending = 0;
			let toolWallSum = 0;
			let toolWallMeasured = false;
			const tools = /* @__PURE__ */ new Map();
			for (const step of steps) {
				for (const item of step.tools) tools.set(item.name, (tools.get(item.name) ?? 0) + item.count);
				if (isNumber$1(step.ttft)) ttftSum += step.ttft;
				else ttftUnknown += 1;
				if (isNumber$1(step.model)) modelSum += step.model;
				else modelUnknown += 1;
				if (isNumber$1(step.decode)) decodeSum += step.decode;
				else decodeUnknown += 1;
				if (isNumber$1(step.output)) outputSum += step.output;
				else outputUnknown += 1;
				if (isNumber$1(step.tool)) {
					toolSum += step.tool;
					toolPairable += 1;
				}
				if (isNumber$1(step.toolWall)) {
					toolWallSum += step.toolWall;
					toolWallMeasured = true;
				}
				toolPending += step.toolPending;
			}
			const first = steps[0];
			const last = steps[steps.length - 1];
			const wall = first !== void 0 && last !== void 0 && !last.running && isNumber$1(first.startTime) && isNumber$1(last.endTime) ? Math.max(0, last.endTime - first.startTime) : null;
			let answer = null;
			let retries = 0;
			let toolErrors = 0;
			let toolOutput = 0;
			let lastStepWall = null;
			for (const step of steps) {
				if (step.answer !== null) answer = step.answer;
				retries += step.retries;
				toolErrors += step.toolErrors;
				if (isNumber$1(step.toolOutput)) toolOutput += step.toolOutput;
				if (isNumber$1(step.wall)) lastStepWall = step.wall;
			}
			const cache = turnCache(steps);
			const all = steps.length;
			const toolCounts = [...tools.entries()].map(([name, count]) => ({
				name,
				count
			})).sort((left, right) => right.count - left.count);
			return {
				turn,
				steps,
				startTime: first !== void 0 && isNumber$1(first.startTime) ? first.startTime : null,
				context: lastContext(steps),
				ttft: ttftUnknown === all ? null : ttftSum,
				ttftPending: ttftUnknown === all ? 0 : ttftUnknown,
				model: modelUnknown === all ? null : modelSum,
				modelPending: modelUnknown,
				decode: decodeUnknown === all ? null : decodeSum,
				tps: decodeSum > 0 && outputSum > 0 ? outputSum / (decodeSum / 1e3) : null,
				output: outputUnknown === all ? null : outputSum,
				tool: toolPairable === 0 ? null : toolSum,
				toolWall: toolWallMeasured ? toolWallSum : null,
				toolPending: toolPairable === 0 ? 0 : toolPending,
				tools: toolCounts,
				cacheHit: cache.rate,
				cacheRead: cache.read,
				cacheWrite: cache.write,
				cacheContext: cache.context,
				retries,
				toolErrors,
				toolOutput,
				lastStepWall,
				running: last !== void 0 && last.running,
				interrupted: steps.some((step) => step.interrupted),
				aborted: steps.some((step) => step.aborted),
				wall,
				compactions: [],
				events: [],
				prompt: null,
				promptImages: NO_IMAGES,
				promptSource: null,
				promptSender: null,
				answer
			};
		}
		/** A step with no measurement at all: the empty step an interruption appends. */
		function isEmptyStep(step) {
			return step.context === null && step.ttft === null && step.model === null && step.tool === null && step.toolPending === 0 && step.tools.length === 0 && (step.wall === null || step.wall === 0);
		}
		/**
		* Group published nodes into ordered turns, closing each turn with its markers.
		* Code-Mode contributions merge into the step that dispatched them, identified
		* by the root call id the node carries.
		* @param nodes - every node the target builder currently holds.
		* @returns turn rows ordered oldest first.
		*/
		function buildTurns(nodes) {
			const grouped = /* @__PURE__ */ new Map();
			const closedAt = /* @__PURE__ */ new Map();
			const closedKind = /* @__PURE__ */ new Map();
			const prompts = /* @__PURE__ */ new Map();
			const inner = /* @__PURE__ */ new Map();
			const compactions = /* @__PURE__ */ new Map();
			const events = /* @__PURE__ */ new Map();
			for (const node of nodes) {
				const data = node.data;
				if (data.kind === "prompt") {
					const previous = prompts.get(data.turn);
					const rank = triggerRank(data.source);
					if (previous === void 0 || rank > triggerRank(previous.source) || rank === triggerRank(previous.source) && previous.text === "" && (data.prompt ?? "") !== "") prompts.set(data.turn, {
						text: data.prompt ?? "",
						images: data.images,
						source: data.source,
						sender: data.sender
					});
					continue;
				}
				if (data.kind === "end") {
					if (isNumber$1(data.endTime)) closedAt.set(data.turn, data.endTime);
					if (data.closeKind !== null) closedKind.set(data.turn, data.closeKind);
					continue;
				}
				if (data.kind === "event") {
					if (!isNumber$1(data.metrics.turn)) continue;
					const list = events.get(data.metrics.turn);
					if (list === void 0) events.set(data.metrics.turn, [data.metrics]);
					else list.push(data.metrics);
					continue;
				}
				if (data.kind === "compaction") {
					if (!isNumber$1(data.metrics.turn)) continue;
					const list = compactions.get(data.metrics.turn);
					if (list === void 0) compactions.set(data.metrics.turn, [data.metrics]);
					else list.push(data.metrics);
					continue;
				}
				if (data.kind === "inner") {
					const key = `${data.turn}:${data.step}`;
					const list = inner.get(key);
					if (list === void 0) inner.set(key, [data]);
					else list.push(data);
					continue;
				}
				const metrics = data.metrics;
				if (!isNumber$1(metrics.turn)) continue;
				const bucket = grouped.get(metrics.turn);
				if (bucket === void 0) grouped.set(metrics.turn, [metrics]);
				else bucket.push(metrics);
			}
			const turns = [];
			const order = [...grouped.keys()].sort((left, right) => left - right);
			order.forEach((turn, index) => {
				const bucket = grouped.get(turn);
				/* v8 ignore next -- turn came from the map's own keys, so the bucket always exists. */
				if (bucket === void 0) return;
				const steps = bucket.slice().sort((left, right) => left.step - right.step);
				if (!(prompts.has(turn) || closedAt.has(turn) || steps.some((step) => step.startTime !== null || step.mark === "end"))) return;
				for (let at = 0; at < steps.length - 1; at += 1) {
					const step = steps[at];
					if (step !== void 0 && step.running) steps[at] = {
						...step,
						running: false
					};
				}
				const at = closedAt.get(turn);
				const tail = steps[steps.length - 1];
				if (at !== void 0 && steps.length > 1 && tail !== void 0 && isEmptyStep(tail)) steps.pop();
				const last = steps[steps.length - 1];
				if (at !== void 0 && last !== void 0 && last.endTime === null) steps[steps.length - 1] = {
					...last,
					endTime: at,
					wall: isNumber$1(last.startTime) ? Math.max(0, at - last.startTime) : null,
					running: false,
					aborted: true
				};
				const aggregate = aggregateTurn(turn, steps.map((step) => mergeInnerCalls(step, inner.get(`${turn}:${step.step}`) ?? [])));
				const prompt = prompts.get(turn);
				turns.push({
					...aggregate,
					compactions: (compactions.get(turn) ?? NO_COMPACTIONS).slice().sort((left, right) => left.startTime - right.startTime),
					events: (events.get(turn) ?? NO_EVENTS).slice().sort((left, right) => left.time - right.time),
					running: index < order.length - 1 || at !== void 0 ? false : aggregate.running,
					interrupted: closedKind.get(turn) === "interrupted" ? true : aggregate.interrupted,
					prompt: prompt === void 0 || prompt.text === "" ? null : prompt.text,
					promptImages: prompt === void 0 ? NO_IMAGES : prompt.images,
					promptSource: prompt === void 0 ? null : prompt.source,
					promptSender: prompt === void 0 ? null : prompt.sender
				});
			});
			return turns;
		}
		/**
		* Context of one turn under the selected basis.
		* @param turn - aggregated turn.
		* @param mode - latest final prompt size, or the sum of its step prompt sizes.
		* @returns the context tokens, or null when no step measured one.
		*/
		function turnContext(turn, mode) {
			if (mode === "latest") return turn.context;
			let sum = 0;
			let measured = false;
			for (const step of turn.steps) {
				if (!isNumber$1(step.context)) continue;
				sum += step.context;
				measured = true;
			}
			return measured ? sum : null;
		}
		/**
		* Aggregate the loaded turns into one totals row.
		* @param turns - aggregated turns in display order.
		* @returns summed metrics; the context column is always the cumulative step context.
		*/
		function aggregateTotals(turns) {
			let stepCount = 0;
			let outputSum = 0;
			let outputMeasured = false;
			let read = 0;
			let write = 0;
			let contextSum = 0;
			let cacheMeasured = false;
			let ttftSum = 0;
			let ttftMeasured = false;
			let ttftPending = 0;
			let modelSum = 0;
			let modelMeasured = false;
			let modelPending = 0;
			let decodeSum = 0;
			let toolSum = 0;
			let toolMeasured = false;
			let toolPending = 0;
			let toolWallSum = 0;
			let toolWallMeasured = false;
			let wallSum = 0;
			let wallMeasured = false;
			for (const turn of turns) {
				stepCount += turn.steps.length;
				if (isNumber$1(turn.output)) {
					outputSum += turn.output;
					outputMeasured = true;
				}
				if (isNumber$1(turn.cacheRead) && isNumber$1(turn.cacheContext)) {
					read += turn.cacheRead;
					write += turn.cacheWrite ?? 0;
					contextSum += turn.cacheContext;
					cacheMeasured = true;
				}
				if (isNumber$1(turn.ttft)) {
					ttftSum += turn.ttft;
					ttftMeasured = true;
				}
				ttftPending += turn.ttftPending;
				if (isNumber$1(turn.model)) {
					modelSum += turn.model;
					modelMeasured = true;
				}
				modelPending += turn.modelPending;
				if (isNumber$1(turn.decode)) decodeSum += turn.decode;
				if (isNumber$1(turn.tool)) {
					toolSum += turn.tool;
					toolMeasured = true;
				}
				toolPending += turn.toolPending;
				if (isNumber$1(turn.toolWall)) {
					toolWallSum += turn.toolWall;
					toolWallMeasured = true;
				}
				if (isNumber$1(turn.wall)) {
					wallSum += turn.wall;
					wallMeasured = true;
				}
			}
			let cumulative = 0;
			let cumulativeMeasured = false;
			for (const turn of turns) for (const step of turn.steps) {
				if (!isNumber$1(step.context)) continue;
				cumulative += step.context;
				cumulativeMeasured = true;
			}
			return {
				turnCount: turns.length,
				stepCount,
				context: cumulativeMeasured ? cumulative : null,
				output: outputMeasured ? outputSum : null,
				cacheHit: cacheMeasured && contextSum > 0 ? Math.min(1, read / contextSum) : null,
				cacheRead: cacheMeasured ? read : null,
				cacheWrite: cacheMeasured ? write : null,
				cacheContext: cacheMeasured ? contextSum : null,
				ttft: ttftMeasured ? ttftSum : null,
				ttftPending: ttftMeasured ? ttftPending : 0,
				model: modelMeasured ? modelSum : null,
				modelPending,
				tps: outputMeasured && decodeSum > 0 && outputSum > 0 ? outputSum / (decodeSum / 1e3) : null,
				tool: toolMeasured ? toolSum : null,
				toolPending,
				toolWall: toolWallMeasured ? toolWallSum : null,
				wall: wallMeasured ? wallSum : null
			};
		}
		/** Stable snapshot used before a Session assembled any performance node. */
		const EMPTY_STATS_SNAPSHOT = {
			turns: [],
			stepCount: 0
		};
		/** Incremental builder over the target's materialized nodes. */
		var StatsSnapshotBuilder = class {
			nodes = /* @__PURE__ */ new Map();
			current = EMPTY_STATS_SNAPSHOT;
			empty = EMPTY_STATS_SNAPSHOT;
			replace(input) {
				this.nodes.clear();
				for (const node of input.nodes) this.nodes.set(node.key, node);
				return this.snapshot();
			}
			apply(input) {
				if (input.upserts.length === 0) return this.current;
				for (const node of input.upserts) this.nodes.set(node.key, node);
				return this.snapshot();
			}
			snapshot() {
				const turns = buildTurns(this.nodes.values());
				this.current = {
					turns,
					stepCount: turns.reduce((sum, turn) => sum + turn.steps.length, 0)
				};
				return this.current;
			}
		};
		/** The `stats` target every Definition publishes into. */
		const statsViewDefinition = {
			target: TARGET,
			create: () => new StatsSnapshotBuilder()
		};
		/**
		* Register the target builder. The registration rides the caller's fiber
		* effect, so unloading the plugin drops the target with it.
		* @param ctx - client root context.
		*/
		function registerStatsConversationView(ctx) {
			ctx.uiConversation.views.register(statsViewDefinition);
		}
		//#endregion
		//#region lib/types/client/stats-totals.js
		/** Whole-session totals from the host projections that survive client paging. */
		/**
		* Map the whole-session stats and token-usage projections onto the totals row.
		* @param stats - whole-log turn/step and wall-time projection.
		* @param usage - whole-log prompt and output token projection.
		* @returns totals over the complete session, or null while either projection is absent.
		*/
		function totalsFromProjections(stats, usage) {
			if (stats === void 0 || usage === void 0) return null;
			const context = usage.uncachedInputTokens + usage.cacheReadTokens + usage.cacheWriteTokens;
			const active = stats.steps === 0 ? null : stats.llmMs + stats.toolMs;
			return {
				turnCount: stats.turns,
				stepCount: stats.steps,
				context: stats.steps === 0 ? null : context,
				output: stats.steps === 0 ? null : usage.outputTokens,
				cacheHit: context > 0 ? Math.min(1, usage.cacheReadTokens / context) : null,
				cacheRead: stats.steps === 0 ? null : usage.cacheReadTokens,
				cacheWrite: stats.steps === 0 ? null : usage.cacheWriteTokens,
				cacheContext: stats.steps === 0 ? null : context,
				ttft: stats.ttftSteps === 0 ? null : stats.ttftMs,
				ttftPending: 0,
				model: stats.steps === 0 ? null : stats.llmMs,
				modelPending: 0,
				tps: stats.steps > 0 && stats.decodeMs > 0 ? stats.decodeTokens / (stats.decodeMs / 1e3) : null,
				tool: stats.steps === 0 ? null : stats.toolMs,
				toolPending: 0,
				toolWall: stats.steps === 0 ? null : stats.toolMs,
				wall: active
			};
		}
		//#endregion
		//#region \0dsh-css:/home/runner/work/deepseek-harness/deepseek-harness/packages/experimental/client-ui-stats/src/client/stats.module.css.mjs
		const css = ".OiOZ0a_root{box-sizing:border-box;height:100%;min-height:0;padding-bottom:calc(var(--dsh-composer-height,152px) + 8px);color:var(--dsw-alias-label-primary);font:var(--dsw-font-xxs-12);flex-direction:column;display:flex;overflow:hidden}.OiOZ0a_scroll{box-sizing:border-box;flex:1;height:100%;min-height:0;padding:0 20px 16px;overflow-y:auto}.OiOZ0a_scroll>:first-child{margin-top:12px}.OiOZ0a_empty{color:var(--dsw-alias-label-secondary);padding:24px 0}.OiOZ0a_table{table-layout:fixed;border-collapse:collapse;border-spacing:0;width:100%;font:var(--dsw-font-xxs-12);font-variant-numeric:tabular-nums}.OiOZ0a_table col:first-child{width:13%}.OiOZ0a_table col:nth-child(2){width:15%}.OiOZ0a_table col:nth-child(3){width:13.5%}.OiOZ0a_table col:nth-child(4),.OiOZ0a_table col:nth-child(5){width:5.5%}.OiOZ0a_table col:nth-child(6){width:6.5%}.OiOZ0a_table col:nth-child(7),.OiOZ0a_table col:nth-child(8){width:6%}.OiOZ0a_table col:nth-child(9){width:7.5%}.OiOZ0a_table col:nth-child(10){width:6%}.OiOZ0a_table col:nth-child(11){width:15.5%}.OiOZ0a_thLeft,.OiOZ0a_th{z-index:2;box-sizing:border-box;border-bottom:.5px solid var(--dsw-alias-border-l2);background:var(--dsw-alias-bg-layer-1);color:var(--dsw-alias-label-tertiary);font:var(--dsw-font-xxs-12);vertical-align:bottom;white-space:normal;text-align:left;background-clip:padding-box;padding:4px 8px;font-weight:500;line-height:1.25;position:sticky;top:0}.OiOZ0a_turnRow{cursor:pointer}.OiOZ0a_turnCell,.OiOZ0a_turnMetric,.OiOZ0a_turnDid,.OiOZ0a_promptCellTurn{text-overflow:ellipsis;border-top:.5px solid var(--dsw-alias-border-l2);border-bottom:.5px solid var(--dsw-alias-border-l1);white-space:nowrap;padding:6px 8px;overflow:hidden}.OiOZ0a_turnCell{white-space:normal;font-weight:600;overflow:hidden}.OiOZ0a_turnMetric{text-align:left;font-weight:600}.OiOZ0a_turnDid{white-space:nowrap}.OiOZ0a_stepCount{color:var(--dsw-alias-label-secondary);font-weight:400}.OiOZ0a_toggle{border-bottom:.5px dotted var(--dsw-alias-border-l2);color:var(--dsw-alias-label-secondary);font:var(--dsw-font-xxxs-11);font-weight:400}.OiOZ0a_rowToggle{color:inherit;cursor:pointer;font:inherit;text-align:left;background:0 0;border:0;padding:0}.OiOZ0a_rowToggle:focus-visible{outline:1px solid var(--dsw-alias-state-business-primary);outline-offset:2px}.OiOZ0a_older{justify-content:center;padding:2px 0 10px;display:flex}.OiOZ0a_olderStatus{color:var(--dsw-alias-label-secondary);font:var(--dsw-font-xxs-12)}.OiOZ0a_turnClock{color:var(--dsw-alias-label-tertiary);font-weight:400}.OiOZ0a_stepRowClickable{cursor:pointer}.OiOZ0a_stepRowError{background:color-mix(in srgb, var(--dsw-alias-state-error-primary) 8%, transparent)}.OiOZ0a_stepCell{box-sizing:border-box;border-bottom:.5px solid var(--dsw-alias-border-l1);height:30px;box-shadow:inset 2px 0 0 var(--dsw-alias-border-l1);color:var(--dsw-alias-label-secondary);white-space:nowrap;padding:0 8px 0 22px}.OiOZ0a_stepMetric,.OiOZ0a_stepDid,.OiOZ0a_promptCell{text-overflow:ellipsis;box-sizing:border-box;border-bottom:.5px solid var(--dsw-alias-border-l1);white-space:nowrap;height:30px;padding:0 8px;overflow:hidden}.OiOZ0a_stepMetric{text-align:left}.OiOZ0a_compactionRow td,.OiOZ0a_eventRow td{border-bottom:.5px solid var(--dsw-alias-border-l1);background:var(--dsw-alias-bg-layer-2)}.OiOZ0a_compactionTag{border:.5px solid var(--dsw-alias-border-l2);color:var(--dsw-alias-label-secondary);font:var(--dsw-font-xxxs-11);border-radius:4px;margin-right:6px;padding:0 4px;display:inline-block}.OiOZ0a_rowTagWhen{vertical-align:middle;flex-wrap:wrap;align-items:center;gap:2px 6px;max-width:100%;display:inline-flex}.OiOZ0a_rowTagWhen .OiOZ0a_compactionTag{overflow-wrap:anywhere;white-space:normal;max-width:100%;margin-right:0;line-height:1.2}.OiOZ0a_rowTagWhen .OiOZ0a_stepClock{overflow-wrap:anywhere;max-width:100%}.OiOZ0a_eventRow .OiOZ0a_stepCell,.OiOZ0a_compactionRow .OiOZ0a_stepCell{white-space:normal;height:auto;min-height:30px;padding-top:4px;padding-bottom:4px;overflow:hidden}.OiOZ0a_eventRow .OiOZ0a_rowTagWhen,.OiOZ0a_compactionRow .OiOZ0a_rowTagWhen{max-width:100%}.OiOZ0a_stepWhen{min-height:30px;color:var(--dsw-alias-label-secondary);font-variant-numeric:tabular-nums;vertical-align:middle;align-items:center;gap:6px;display:inline-flex}.OiOZ0a_stepClock{color:var(--dsw-alias-label-primary)}.OiOZ0a_stepGap{color:var(--dsw-alias-label-tertiary);font:var(--dsw-font-xxxs-11)}.OiOZ0a_stateRunning{color:var(--dsw-alias-state-business-primary);margin-left:6px;font-weight:400}.OiOZ0a_stateWarn{color:var(--dsw-alias-state-warn-primary);margin-left:6px;font-weight:400}.OiOZ0a_stateMuted{color:var(--dsw-alias-label-secondary);margin-left:6px;font-weight:400}.OiOZ0a_partialHint{color:var(--dsw-alias-state-warn-primary);font:var(--dsw-font-xxxs-11);font-weight:400}.OiOZ0a_detailHint{color:var(--dsw-alias-label-tertiary);font:var(--dsw-font-xxxs-11);font-weight:400}.OiOZ0a_promptText{max-width:260px;color:var(--dsw-alias-label-primary);text-overflow:ellipsis;vertical-align:bottom;white-space:nowrap;display:inline-block;overflow:hidden}.OiOZ0a_promptClip{align-items:center;gap:4px;min-width:0;display:flex}.OiOZ0a_promptClip .OiOZ0a_promptText{flex:1;min-width:0;max-width:none}.OiOZ0a_triggerPill{text-overflow:ellipsis;border:.5px solid var(--dsw-alias-border-l2);max-width:70%;color:var(--dsw-alias-label-secondary);font:var(--dsw-font-xxxs-11);border-radius:4px;flex:none;margin-right:4px;padding:0 4px;overflow:hidden}.OiOZ0a_imagePill{cursor:pointer;background:0 0;margin-right:0}.OiOZ0a_promptText+.OiOZ0a_imagePill{margin-left:4px}.OiOZ0a_imagePill[data-state=failed]{color:var(--dsw-alias-state-error-primary)}@media (hover:hover){.OiOZ0a_imagePill:hover{background:var(--dsw-alias-bg-layer-2);color:var(--dsw-alias-label-primary)}}.OiOZ0a_imagePill:focus-visible{outline:1px solid var(--dsw-alias-state-business-primary);outline-offset:1px}.OiOZ0a_imagePreview{flex-wrap:wrap;gap:6px;max-width:284px;display:flex}.OiOZ0a_imageThumb{box-sizing:border-box;border:.5px solid var(--dsw-alias-border-l2);background:var(--dsw-alias-bg-layer-2);cursor:pointer;border-radius:4px;width:76px;height:76px;padding:0;overflow:hidden}.OiOZ0a_imageThumb img{object-fit:cover;width:100%;height:100%;display:block}.OiOZ0a_imagePreview[data-single=true] .OiOZ0a_imageThumb{width:100%;height:180px}.OiOZ0a_imagePreview[data-single=true] .OiOZ0a_imageThumb img{object-fit:contain}.OiOZ0a_imageThumb:focus-visible{outline:1px solid var(--dsw-alias-state-business-primary);outline-offset:1px}@media (hover:hover){.OiOZ0a_imageThumb:hover{border-color:var(--dsw-alias-state-business-primary)}}.OiOZ0a_imageThumbStatus{box-sizing:border-box;border:.5px dashed var(--dsw-alias-border-l2);width:76px;min-height:76px;color:var(--dsw-alias-label-secondary);font:var(--dsw-font-xxxs-11);text-align:center;border-radius:4px;flex-direction:column;justify-content:center;align-items:center;gap:4px;padding:4px;display:inline-flex}.OiOZ0a_imagePreview[data-single=true] .OiOZ0a_imageThumbStatus{width:100%}.OiOZ0a_imageThumbStatus[data-state=failed]{color:var(--dsw-alias-state-error-primary)}.OiOZ0a_imageThumbStatus .OiOZ0a_chip{margin-right:0}.OiOZ0a_answerPreview{color:var(--dsw-alias-label-tertiary);font-weight:400}.OiOZ0a_didClip{text-overflow:ellipsis;vertical-align:bottom;white-space:nowrap;max-width:380px;display:inline-block;overflow:hidden}.OiOZ0a_toolName{color:var(--stats-tool-color,var(--dsw-alias-label-secondary));font-weight:500}.OiOZ0a_didHint{color:var(--dsw-alias-label-secondary)}.OiOZ0a_didSep{color:var(--dsw-alias-label-caption);margin:0 4px}.OiOZ0a_didStat{color:var(--dsw-alias-label-primary)}.OiOZ0a_added{color:var(--dsw-alias-state-success-primary)}.OiOZ0a_removed{color:var(--dsw-alias-state-error-primary)}.OiOZ0a_countBadge{border:.5px solid var(--dsw-alias-border-l2);background:var(--dsw-alias-bg-layer-2);height:19px;color:var(--dsw-alias-label-primary);vertical-align:bottom;border-radius:2px;align-items:center;margin:0 3px;padding:0 5px;font-size:10px;font-weight:650;line-height:14px;display:inline-flex}.OiOZ0a_failPill{background:var(--dsw-alias-state-error-primary);height:19px;color:var(--dsw-alias-bg-base);vertical-align:bottom;border-radius:2px;align-items:center;margin-right:6px;padding:0 6px;font-size:10px;font-weight:650;line-height:14px;display:inline-flex}.OiOZ0a_pills{flex-wrap:wrap;align-items:center;gap:6px;max-width:380px;display:flex}.OiOZ0a_pill{border:.5px solid var(--dsw-alias-border-l2);background:var(--dsw-alias-bg-layer-2);height:19px;color:var(--dsw-alias-label-primary);border-radius:2px;align-items:center;gap:5px;padding:0 6px;font-size:10px;font-weight:650;display:inline-flex}.OiOZ0a_pillMuted{color:var(--dsw-alias-label-secondary)}.OiOZ0a_dot{corner-shape:round;background:var(--stats-tool-color,var(--dsw-alias-label-secondary));border-radius:50%;width:6px;height:6px;display:inline-block}.OiOZ0a_mix{background:var(--dsw-alias-bg-layer-2);border-radius:2px;max-width:380px;height:4px;margin-top:5px;display:flex;overflow:hidden}.OiOZ0a_mixSegment{background:var(--stats-tool-color,var(--dsw-alias-label-secondary));display:block}.OiOZ0a_tone0{--stats-tool-color:var(--dsw-alias-state-business-primary)}.OiOZ0a_tone1{--stats-tool-color:var(--dsw-alias-state-success-primary)}.OiOZ0a_tone2{--stats-tool-color:var(--dsw-alias-state-warn-primary)}.OiOZ0a_tone3{--stats-tool-color:var(--dsw-alias-state-error-secondary)}.OiOZ0a_tone4{--stats-tool-color:var(--dsw-alias-brand-primary-new-colorprimary-new-color)}.OiOZ0a_tone5{--stats-tool-color:var(--dsw-alias-state-success-secondary)}.OiOZ0a_tone6{--stats-tool-color:var(--dsw-alias-state-warn-label)}.OiOZ0a_tone7{--stats-tool-color:var(--dsw-alias-label-caption)}.OiOZ0a_num{font-variant-numeric:tabular-nums;text-align:left;min-width:62px;display:inline-block}.OiOZ0a_numDelta{min-width:62px;font:var(--dsw-font-xxxs-11);font-variant-numeric:tabular-nums;text-align:left;display:inline-block}.OiOZ0a_deltaUp{color:var(--dsw-alias-state-success-primary)}.OiOZ0a_deltaDown{color:var(--dsw-alias-state-error-primary)}.OiOZ0a_timeWrap{align-items:center;gap:8px;width:100%;display:flex}.OiOZ0a_barTurn{box-sizing:border-box;border:.5px solid var(--dsw-alias-border-l2);background:var(--dsw-alias-bg-layer-2);vertical-align:middle;border-radius:2px;flex:1;min-width:40px;height:14px;display:inline-block;overflow:hidden}.OiOZ0a_barStep{background:var(--dsw-alias-bg-layer-2);vertical-align:middle;border-radius:2px;flex:1;min-width:32px;height:6px;display:inline-block;overflow:hidden}.OiOZ0a_barFill{height:100%;display:flex}.OiOZ0a_barSegment{height:100%;display:block}.OiOZ0a_barTtft{background:var(--dsw-alias-state-business-primary);opacity:.5}.OiOZ0a_barDecode{background:var(--dsw-alias-state-business-primary)}.OiOZ0a_barTool{background:var(--dsw-alias-state-warn-primary)}.OiOZ0a_barRest{background:var(--dsw-alias-label-caption)}.OiOZ0a_timeValue{font-variant-numeric:tabular-nums;text-align:left;min-width:58px}.OiOZ0a_timeRunning{min-width:58px;color:var(--dsw-alias-state-business-primary);font-variant-numeric:tabular-nums;text-align:left}.OiOZ0a_moreCell{color:var(--dsw-alias-label-secondary);font:var(--dsw-font-xxxs-11);white-space:normal;padding:2px 8px 8px;line-height:1.9}.OiOZ0a_moreText{margin-right:10px}.OiOZ0a_chip,.OiOZ0a_chipOn{border:.5px solid var(--dsw-alias-border-l2);background:var(--dsw-alias-bg-layer-2);height:19px;color:var(--dsw-alias-label-primary);cursor:pointer;border-radius:2px;align-items:center;gap:5px;margin-right:8px;padding:0 6px;font-size:10px;font-weight:650;display:inline-flex}.OiOZ0a_chipOn{border-color:var(--dsw-alias-state-business-primary)}.OiOZ0a_chip:focus-visible,.OiOZ0a_chipOn:focus-visible{outline:1px solid var(--dsw-alias-state-business-primary);outline-offset:1px}.OiOZ0a_detailCell{border-bottom:.5px solid var(--dsw-alias-border-l1);box-shadow:inset 2px 0 0 var(--dsw-alias-border-l1);font:var(--dsw-font-xs-13);padding:6px 8px 12px 22px}.OiOZ0a_answerSection{margin-bottom:10px}.OiOZ0a_answerBody{background:var(--dsw-alias-bg-layer-2);max-height:420px;color:var(--dsw-alias-label-primary);word-break:break-word;border-radius:6px;padding:8px;overflow:auto}.OiOZ0a_detailHead{align-items:center;gap:6px;margin:2px 0 8px;display:flex}.OiOZ0a_detailTitle{font:var(--dsw-font-xs-strong-13)}.OiOZ0a_detailActions{align-items:center;gap:6px;margin-left:auto;display:flex}.OiOZ0a_block{border:.5px solid var(--dsw-alias-border-l2);border-radius:6px;margin-bottom:10px;overflow:hidden}.OiOZ0a_blockHead{background:var(--dsw-alias-bg-layer-2);height:26px;font:var(--dsw-font-xxs-12);align-items:center;gap:6px;padding:0 8px;display:flex}.OiOZ0a_blockHint{min-width:0;color:var(--dsw-alias-label-secondary);text-overflow:ellipsis;white-space:nowrap;overflow:hidden}.OiOZ0a_blockMeta{color:var(--dsw-alias-label-tertiary);font:var(--dsw-font-xxxs-11);white-space:nowrap}.OiOZ0a_blockSub{color:var(--dsw-alias-label-tertiary);font:var(--dsw-font-xxxs-11);padding:4px 8px 0}.OiOZ0a_blockEmpty{color:var(--dsw-alias-label-secondary);font:var(--dsw-font-xxxs-11);padding:6px 8px;display:block}.OiOZ0a_copy{border:.5px solid var(--dsw-alias-border-l2);height:18px;color:var(--dsw-alias-label-secondary);cursor:pointer;font:var(--dsw-font-xxxs-11);background:0 0;border-radius:4px;margin-left:auto;padding:0 8px}@media (hover:hover){.OiOZ0a_copy:hover{color:var(--dsw-alias-label-primary)}}.OiOZ0a_copy:focus-visible{outline:1px solid var(--dsw-alias-state-business-primary);outline-offset:1px}.OiOZ0a_code{box-sizing:border-box;--dsh-scrollbar-thumb:var(--dsw-alias-scrollbar-bg-l2);--dsh-scrollbar-thumb-hover:var(--dsw-alias-scrollbar-hover-l2);max-height:320px;font:12px/18px var(--ds-font-family-code);tab-size:2;white-space:pre;margin:0;padding:6px 8px;overflow:auto}.OiOZ0a_codeWrap{white-space:pre-wrap;overflow-wrap:anywhere}.OiOZ0a_codeLine,.OiOZ0a_diffAdd,.OiOZ0a_diffDel{white-space:pre;display:block}.OiOZ0a_codeWrap .OiOZ0a_codeLine,.OiOZ0a_codeWrap .OiOZ0a_diffAdd,.OiOZ0a_codeWrap .OiOZ0a_diffDel{white-space:pre-wrap;overflow-wrap:anywhere}.OiOZ0a_diffAdd{color:var(--dsw-alias-state-success-primary)}.OiOZ0a_diffDel{color:var(--dsw-alias-state-error-primary)}.OiOZ0a_toolbar{border-top:.5px solid var(--dsw-alias-border-l1);font:var(--dsw-font-xxs-12);align-items:center;gap:8px;margin:8px 20px 0;padding:8px 0;display:flex}.OiOZ0a_summary{color:var(--dsw-alias-label-secondary);margin-right:auto}.OiOZ0a_controls{flex-wrap:wrap;align-items:center;gap:6px 8px;display:flex}.OiOZ0a_controlLabel{color:var(--dsw-alias-label-secondary);white-space:nowrap}.OiOZ0a_contextMode{flex:none}.OiOZ0a_totalsRow td{border-top:.5px solid var(--dsw-alias-border-l2);background:var(--dsw-alias-bg-layer-1);font-weight:600}.OiOZ0a_totalsLabel{color:var(--dsw-alias-label-primary);font-weight:600}.OiOZ0a_totalsMeta{color:var(--dsw-alias-label-secondary);font-weight:400}.OiOZ0a_button{border:.5px solid var(--dsw-alias-border-l1);background:var(--dsw-alias-bg-layer-2);color:var(--dsw-alias-label-primary);cursor:pointer;border-radius:6px;padding:4px 10px;font-size:12px}@media (hover:hover){.OiOZ0a_button:hover{background:var(--dsw-alias-interactive-bg-hover)}}.OiOZ0a_button:focus-visible{outline:1px solid var(--dsw-alias-state-business-primary);outline-offset:1px}.OiOZ0a_buttonOff{border:.5px solid var(--dsw-alias-border-l1);color:var(--dsw-alias-label-secondary);cursor:default;background:0 0;border-radius:6px;padding:4px 10px;font-size:12px}";
		const tagId = "@deepseek-ai/dsh-experimental-client-ui-stats/stats.module.css";
		if (typeof document !== "undefined" && document.querySelector("style[data-plugin-css=" + JSON.stringify(tagId) + "]") === null) {
			const tag = document.createElement("style");
			tag.dataset.plugin = "@deepseek-ai/dsh-experimental-client-ui-stats";
			tag.dataset.pluginCss = tagId;
			tag.textContent = css;
			document.head.appendChild(tag);
		}
		var stats_module_css_default = {
			"added": "OiOZ0a_added",
			"answerBody": "OiOZ0a_answerBody",
			"answerPreview": "OiOZ0a_answerPreview",
			"answerSection": "OiOZ0a_answerSection",
			"barDecode": "OiOZ0a_barDecode",
			"barFill": "OiOZ0a_barFill",
			"barRest": "OiOZ0a_barRest",
			"barSegment": "OiOZ0a_barSegment",
			"barStep": "OiOZ0a_barStep",
			"barTool": "OiOZ0a_barTool",
			"barTtft": "OiOZ0a_barTtft",
			"barTurn": "OiOZ0a_barTurn",
			"block": "OiOZ0a_block",
			"blockEmpty": "OiOZ0a_blockEmpty",
			"blockHead": "OiOZ0a_blockHead",
			"blockHint": "OiOZ0a_blockHint",
			"blockMeta": "OiOZ0a_blockMeta",
			"blockSub": "OiOZ0a_blockSub",
			"button": "OiOZ0a_button",
			"buttonOff": "OiOZ0a_buttonOff",
			"chip": "OiOZ0a_chip",
			"chipOn": "OiOZ0a_chipOn",
			"code": "OiOZ0a_code",
			"codeLine": "OiOZ0a_codeLine",
			"codeWrap": "OiOZ0a_codeWrap",
			"compactionRow": "OiOZ0a_compactionRow",
			"compactionTag": "OiOZ0a_compactionTag",
			"contextMode": "OiOZ0a_contextMode",
			"controlLabel": "OiOZ0a_controlLabel",
			"controls": "OiOZ0a_controls",
			"copy": "OiOZ0a_copy",
			"countBadge": "OiOZ0a_countBadge",
			"deltaDown": "OiOZ0a_deltaDown",
			"deltaUp": "OiOZ0a_deltaUp",
			"detailActions": "OiOZ0a_detailActions",
			"detailCell": "OiOZ0a_detailCell",
			"detailHead": "OiOZ0a_detailHead",
			"detailHint": "OiOZ0a_detailHint",
			"detailTitle": "OiOZ0a_detailTitle",
			"didClip": "OiOZ0a_didClip",
			"didHint": "OiOZ0a_didHint",
			"didSep": "OiOZ0a_didSep",
			"didStat": "OiOZ0a_didStat",
			"diffAdd": "OiOZ0a_diffAdd",
			"diffDel": "OiOZ0a_diffDel",
			"dot": "OiOZ0a_dot",
			"empty": "OiOZ0a_empty",
			"eventRow": "OiOZ0a_eventRow",
			"failPill": "OiOZ0a_failPill",
			"imagePill": "OiOZ0a_imagePill",
			"imagePreview": "OiOZ0a_imagePreview",
			"imageThumb": "OiOZ0a_imageThumb",
			"imageThumbStatus": "OiOZ0a_imageThumbStatus",
			"mix": "OiOZ0a_mix",
			"mixSegment": "OiOZ0a_mixSegment",
			"moreCell": "OiOZ0a_moreCell",
			"moreText": "OiOZ0a_moreText",
			"num": "OiOZ0a_num",
			"numDelta": "OiOZ0a_numDelta",
			"older": "OiOZ0a_older",
			"olderStatus": "OiOZ0a_olderStatus",
			"partialHint": "OiOZ0a_partialHint",
			"pill": "OiOZ0a_pill",
			"pillMuted": "OiOZ0a_pillMuted",
			"pills": "OiOZ0a_pills",
			"promptCell": "OiOZ0a_promptCell",
			"promptCellTurn": "OiOZ0a_promptCellTurn",
			"promptClip": "OiOZ0a_promptClip",
			"promptText": "OiOZ0a_promptText",
			"removed": "OiOZ0a_removed",
			"root": "OiOZ0a_root",
			"rowTagWhen": "OiOZ0a_rowTagWhen",
			"rowToggle": "OiOZ0a_rowToggle",
			"scroll": "OiOZ0a_scroll",
			"stateMuted": "OiOZ0a_stateMuted",
			"stateRunning": "OiOZ0a_stateRunning",
			"stateWarn": "OiOZ0a_stateWarn",
			"stepCell": "OiOZ0a_stepCell",
			"stepClock": "OiOZ0a_stepClock",
			"stepCount": "OiOZ0a_stepCount",
			"stepDid": "OiOZ0a_stepDid",
			"stepGap": "OiOZ0a_stepGap",
			"stepMetric": "OiOZ0a_stepMetric",
			"stepRowClickable": "OiOZ0a_stepRowClickable",
			"stepRowError": "OiOZ0a_stepRowError",
			"stepWhen": "OiOZ0a_stepWhen",
			"summary": "OiOZ0a_summary",
			"table": "OiOZ0a_table",
			"th": "OiOZ0a_th",
			"thLeft": "OiOZ0a_thLeft",
			"timeRunning": "OiOZ0a_timeRunning",
			"timeValue": "OiOZ0a_timeValue",
			"timeWrap": "OiOZ0a_timeWrap",
			"toggle": "OiOZ0a_toggle",
			"tone0": "OiOZ0a_tone0",
			"tone1": "OiOZ0a_tone1",
			"tone2": "OiOZ0a_tone2",
			"tone3": "OiOZ0a_tone3",
			"tone4": "OiOZ0a_tone4",
			"tone5": "OiOZ0a_tone5",
			"tone6": "OiOZ0a_tone6",
			"tone7": "OiOZ0a_tone7",
			"toolName": "OiOZ0a_toolName",
			"toolbar": "OiOZ0a_toolbar",
			"totalsLabel": "OiOZ0a_totalsLabel",
			"totalsMeta": "OiOZ0a_totalsMeta",
			"totalsRow": "OiOZ0a_totalsRow",
			"triggerPill": "OiOZ0a_triggerPill",
			"turnCell": "OiOZ0a_turnCell",
			"turnClock": "OiOZ0a_turnClock",
			"turnDid": "OiOZ0a_turnDid",
			"turnMetric": "OiOZ0a_turnMetric",
			"turnRow": "OiOZ0a_turnRow"
		};
		/** Whether a value is a finite number. */
		function isNumber(value) {
			return typeof value === "number" && Number.isFinite(value);
		}
		/**
		* Duration: minutes past one minute, seconds past one second, milliseconds below.
		* @param value - duration in milliseconds.
		* @returns formatted duration, or the dash placeholder.
		*/
		function formatMs(value) {
			if (!isNumber(value)) return "—";
			if (value >= 6e4) return `${(value / 6e4).toFixed(1)} min`;
			return value >= 1e3 ? `${(value / 1e3).toFixed(2)} s` : `${Math.round(value)} ms`;
		}
		/**
		* Token count with thousands separators, for tooltips.
		* @param value - token count.
		* @returns grouped digits, or the dash placeholder.
		*/
		function formatTokens(value) {
			if (!isNumber(value)) return "—";
			return String(Math.round(value)).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
		}
		/**
		* Token count in K, switching to M once the value reaches ten million.
		* @param value - token count, or null when unrecorded.
		* @returns the K/M-suffixed count, or the dash placeholder.
		*/
		function formatTokensShort(value) {
			if (!isNumber(value)) return "—";
			if (value >= 1e7) return `${(value / 1e6).toFixed(1)}M`;
			return value >= 1e3 ? `${(value / 1e3).toFixed(1)}K` : `${(value / 1e3).toFixed(2)}K`;
		}
		/**
		* Signed context delta, using the same K scale as the counts it belongs to.
		* @param value - token delta; null produces an empty string.
		* @returns signed K-suffixed delta.
		*/
		function formatTokensDelta(value) {
			if (!isNumber(value)) return "";
			return `${value >= 0 ? "+" : "−"}${formatTokensShort(Math.abs(value))}`;
		}
		/**
		* Character count of tool output, collapsed to K past one thousand.
		* @param value - character count.
		* @returns formatted size, or the dash placeholder.
		*/
		function formatChars(value) {
			if (!isNumber(value) || value <= 0) return "—";
			return value >= 1e3 ? `${(value / 1e3).toFixed(1)}K` : String(Math.round(value));
		}
		/**
		* Output speed in tokens per second.
		* @param value - tokens per second.
		* @returns one-decimal speed, or the dash placeholder.
		*/
		function formatTps(value) {
			return isNumber(value) ? `${value.toFixed(1)} tok/s` : "—";
		}
		/**
		* Share as a two-decimal percentage.
		* @param value - share in [0, 1].
		* @returns percentage text, or the dash placeholder.
		*/
		function formatPercent(value) {
			return isNumber(value) ? `${(value * 100).toFixed(2)}%` : "—";
		}
		/**
		* Local wall-clock time of day, so a row lines up with logs.
		* @param value - epoch milliseconds.
		* @returns `HH:MM:SS`, or the dash placeholder.
		*/
		function formatClock(value) {
			if (!isNumber(value)) return "—";
			const date = new Date(value);
			const pad = (part) => String(part).padStart(2, "0");
			return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
		}
		/**
		* Compact month-day clock for the turn scope cell; tooltips carry the full date.
		* @param value - epoch milliseconds.
		* @returns `MM-DD HH:MM`, or the dash placeholder.
		*/
		function formatDateTimeShort(value) {
			if (!isNumber(value)) return "—";
			const date = new Date(value);
			const pad = (part) => String(part).padStart(2, "0");
			return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
		}
		/** Local timestamp: full date or month-day, seconds, and optional milliseconds. */
		function formatStamp(value, fullDate, millis) {
			if (!isNumber(value)) return "—";
			const date = new Date(value);
			const pad = (part, width = 2) => String(part).padStart(width, "0");
			return `${fullDate ? `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}` : `${pad(date.getMonth() + 1)}-${pad(date.getDate())}`} ${`${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`}${millis ? `.${pad(date.getMilliseconds(), 3)}` : ""}`;
		}
		/**
		* Full timestamp with milliseconds for tooltips.
		* @param value - epoch milliseconds.
		* @returns `YYYY-MM-DD HH:MM:SS.mmm`, or the dash placeholder.
		*/
		function formatDateTimeMs(value) {
			return formatStamp(value, true, true);
		}
		/**
		* Step start timestamp with date and milliseconds for the trigger column.
		* @param value - epoch milliseconds.
		* @returns `MM-DD HH:MM:SS.mmm`, or the dash placeholder.
		*/
		function formatStepClock(value) {
			return formatStamp(value, false, true);
		}
		/**
		* Date and time of day: the table shows the clock, tooltips add the date.
		* @param value - epoch milliseconds.
		* @returns `YYYY-MM-DD HH:MM:SS`, or the dash placeholder.
		*/
		function formatDateTime(value) {
			return formatStamp(value, true, false);
		}
		/**
		* Bar length on a square-root scale: a linear scale collapses short steps into
		* slivers, and a logarithmic one flattens the differences the column exists to
		* show. The longest member of the comparison group fills the bar.
		* @param total - measured duration.
		* @param scale - longest duration of the comparison group.
		* @returns fill share in [0, 1].
		*/
		function barShare(total, scale) {
			if (!isNumber(scale) || scale <= 0) return 1;
			if (total <= 0) return 0;
			return Math.min(1, Math.sqrt(total / scale));
		}
		//#endregion
		//#region lib/types/client/StatsTable.js
		/**
		* Statistics table for one conversation: per-turn aggregates, expandable step
		* rows, the time-composition bars, and the per-step tool-call accordion.
		*/
		/** Steps one turn renders before the "show all" control appears. */
		const STEP_LIMIT = 24;
		/** Ignore sub-second idles when showing the gap since the previous step. */
		const STEP_GAP_MIN_MS = 1e3;
		/** How long the clipboard button reports its outcome, in milliseconds. */
		const COPY_FEEDBACK_MS = 1200;
		/** Lines one code block renders before the line-cap notice. */
		const CODE_LINE_LIMIT = 400;
		/** Line count above which a diff falls back to whole-side removal and addition. */
		const DIFF_LINE_LIMIT = 300;
		/** Hover dwell before the prompt image preview opens. */
		const IMAGE_PREVIEW_DELAY_MS = 200;
		/** Numeric columns in table order. */
		const COLUMNS = [
			"context",
			"output",
			"cache",
			"ttft",
			"model",
			"tps",
			"tool"
		];
		/** Column count of the scope, prompt, did, numeric, and time columns. */
		const TOTAL_COLUMNS = COLUMNS.length + 4;
		/** Fixed palette indexes for tools that keep one tone across sessions. */
		const TOOL_TONES = {
			read: 0,
			glob: 1,
			grep: 1,
			edit: 2,
			write: 2,
			apply_patch: 2,
			pwsh: 4,
			shell: 4,
			bash: 4,
			web_fetch: 5,
			web_search: 5,
			ask_user_question: 3,
			todo_write: 7,
			subagent: 5,
			subagent_fork: 5
		};
		/** Tone classes, one per palette index; each sets `--stats-tool-color`. */
		const TONE_CLASSES = [
			stats_module_css_default.tone0,
			stats_module_css_default.tone1,
			stats_module_css_default.tone2,
			stats_module_css_default.tone3,
			stats_module_css_default.tone4,
			stats_module_css_default.tone5,
			stats_module_css_default.tone6,
			stats_module_css_default.tone7
		];
		/**
		* Join CSS module classes into one `className`, dropping absent entries: the
		* module's index-signature declaration types every lookup as possibly absent
		* under `noUncheckedIndexedAccess`.
		* @param names - class names in declaration order.
		* @returns the space-joined class list.
		*/
		function cx(...names) {
			return names.filter((name) => name !== void 0).join(" ");
		}
		/** Empty per-turn step state, shared so unexpanded turns allocate nothing. */
		const EMPTY_STEP_VIEW = {};
		/** Index of the tool tone for one call name; unknown names hash to a stable tone. */
		function toneIndex(name) {
			if (name === null) return 7;
			const fixed = TOOL_TONES[name];
			if (fixed !== void 0) return fixed;
			let hash = 0;
			for (let index = 0; index < name.length; index += 1) hash = (hash * 31 + name.charCodeAt(index)) % 9973;
			return hash % TONE_CLASSES.length;
		}
		/** CSS class carrying the tool tone for one call name. */
		function toneClass(name) {
			return TONE_CLASSES[toneIndex(name)];
		}
		/** Whether a row is running with no measured wall time yet. */
		function isTimeRunning(value) {
			return value.running && !isNumber$1(value.wall) && isNumber$1(value.startTime);
		}
		/** Measured span; a running row falls back to the elapsed time at `now`. */
		function totalSpan(value, now) {
			if (isNumber$1(value.wall)) return value.wall;
			if (isNumber$1(value.startTime) && value.running) return Math.max(0, now - value.startTime);
			return null;
		}
		/** Default fold state of a turn the user never toggled. */
		function defaultTurnOpen(mode, turnNumber, latestTurn) {
			if (mode === "all") return true;
			if (mode === "none") return false;
			return turnNumber === latestTurn;
		}
		/** Fresh table state: follow the newest turn's open default. */
		function createViewState() {
			return {
				mode: "newest",
				openTurns: {},
				stepViews: {},
				showTotals: false,
				showEvents: false,
				showContextDelta: false,
				contextMode: "latest"
			};
		}
		/** Read an array element whose index the surrounding bounds already prove. */
		function at(lines, index) {
			return lines[index];
		}
		/** Split a text block into lines; an empty string has no lines. */
		function splitLines(text) {
			return text === "" ? [] : text.split("\n");
		}
		/** Map a plain text block to code lines. */
		function textLines(text) {
			return splitLines(text).map((line) => ({
				sign: " ",
				text: line
			}));
		}
		/**
		* Readable argument body: parse and indent JSON by two spaces, or keep the
		* text verbatim when the model produced arguments this build cannot parse.
		* @param args - raw tool arguments as recorded.
		* @returns the body the raw view displays and copies.
		*/
		function prettyArguments(args) {
			try {
				const parsed = JSON.parse(args);
				return JSON.stringify(parsed, null, 2);
			} catch {
				return args;
			}
		}
		/**
		* Line-level diff: removal/addition rows for the parts that differ, common
		* lines kept in place. Above the size cap the two sides are shown whole.
		*/
		function lineDiff(before, after) {
			const left = splitLines(before ?? "");
			const right = splitLines(after ?? "");
			if (left.length > DIFF_LINE_LIMIT || right.length > DIFF_LINE_LIMIT) return [...left.map((text) => ({
				sign: "−",
				text
			})), ...right.map((text) => ({
				sign: "+",
				text
			}))];
			const width = right.length + 1;
			const table = /* @__PURE__ */ new Map();
			let previousRow = 0;
			for (let row = 1; row <= left.length; row += 1) {
				let leftCell = 0;
				for (let column = 1; column <= right.length; column += 1) {
					const up = table.get(previousRow + column) ?? 0;
					const diagonal = table.get(previousRow + column - 1) ?? 0;
					const value = at(left, row - 1) === at(right, column - 1) ? diagonal + 1 : Math.max(up, leftCell);
					table.set(row * width + column, value);
					leftCell = value;
				}
				previousRow = row * width;
			}
			const rows = [];
			let row = left.length;
			let column = right.length;
			while (row > 0 && column > 0) {
				const leftLine = at(left, row - 1);
				const rightLine = at(right, column - 1);
				if (leftLine === rightLine) {
					rows.push({
						sign: " ",
						text: leftLine
					});
					row -= 1;
					column -= 1;
				} else if ((table.get((row - 1) * width + column) ?? 0) >= (table.get(row * width + column - 1) ?? 0)) {
					rows.push({
						sign: "−",
						text: leftLine
					});
					row -= 1;
				} else {
					rows.push({
						sign: "+",
						text: rightLine
					});
					column -= 1;
				}
			}
			while (row > 0) {
				rows.push({
					sign: "−",
					text: at(left, row - 1)
				});
				row -= 1;
			}
			while (column > 0) {
				rows.push({
					sign: "+",
					text: at(right, column - 1)
				});
				column -= 1;
			}
			return rows.reverse();
		}
		/** Unwrap the `<path>`/`<content>` envelope a `read` result carries. */
		function fileContentOf(text) {
			if (text === null) return null;
			const open = text.indexOf("<content>");
			const close = text.lastIndexOf("</content>");
			if (open < 0 || close < 0 || close < open) return null;
			const pathMatch = /<path>([\s\S]*?)<\/path>/.exec(text);
			const body = text.slice(open + 9, close).replace(/^\n/, "");
			let path = null;
			if (pathMatch !== null)
 /* v8 ignore next -- a matched `<path>` group is non-optional in the regexp, so the fallback is unreachable. */
			path = (pathMatch[1] ?? "").trim();
			return {
				path,
				body
			};
		}
		/** CSS class of one code line by its diff sign. */
		function lineClass(sign) {
			if (sign === "+") return stats_module_css_default.diffAdd;
			if (sign === "−") return stats_module_css_default.diffDel;
			return stats_module_css_default.codeLine;
		}
		/** Code block: monospaced lines, capped at CODE_LINE_LIMIT rows. */
		function codeBlock(lines, t, wrap) {
			const shown = lines.length > CODE_LINE_LIMIT ? lines.slice(0, CODE_LINE_LIMIT) : lines;
			return (0, react_jsx_runtime.jsxs)("pre", {
				className: wrap ? cx(stats_module_css_default.code, stats_module_css_default.codeWrap) : stats_module_css_default.code,
				children: [shown.map((line, index) => (0, react_jsx_runtime.jsx)("span", {
					className: lineClass(line.sign),
					children: line.sign === " " ? line.text : `${line.sign} ${line.text}`
				}, index)), shown.length < lines.length ? (0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.blockEmpty,
					children: t("detail.lineCap", {
						shown: formatTokens(shown.length),
						total: formatTokens(lines.length)
					})
				}) : null]
			});
		}
		/** Body of one tool call: raw arguments, a line diff, a file, or result text. */
		function callBody(call, t, wrap, raw) {
			if (raw && call.args !== null) return (0, react_jsx_runtime.jsxs)(react.Fragment, { children: [(0, react_jsx_runtime.jsx)("div", {
				className: stats_module_css_default.blockSub,
				children: t("detail.args")
			}), codeBlock(textLines(prettyArguments(call.args)), t, true)] });
			if (call.name === "edit" || call.name === "write" || call.name === "apply_patch") {
				const rows = lineDiff(call.before, call.after);
				if (rows.length > 0) return (0, react_jsx_runtime.jsxs)(react.Fragment, { children: [(0, react_jsx_runtime.jsx)("div", {
					className: stats_module_css_default.blockSub,
					children: t("detail.diff")
				}), codeBlock(rows, t, wrap)] });
			}
			const file = call.name === "read" ? fileContentOf(call.text) : null;
			if (file !== null) {
				const lines = file.body.split("\n").map((line) => ({
					sign: " ",
					text: line.replace(/^\s*\d+:\s?/, "")
				}));
				return (0, react_jsx_runtime.jsxs)(react.Fragment, { children: [(0, react_jsx_runtime.jsxs)("div", {
					className: stats_module_css_default.blockSub,
					children: [file.path ?? t("detail.file"), ` · ${t("detail.lines", { lines: formatTokens(lines.length) })}`]
				}), codeBlock(lines, t, wrap)] });
			}
			if (call.text === null) return (0, react_jsx_runtime.jsx)("div", {
				className: stats_module_css_default.blockEmpty,
				children: t("detail.emptyBody")
			});
			return (0, react_jsx_runtime.jsxs)(react.Fragment, { children: [call.truncated ? (0, react_jsx_runtime.jsx)("div", {
				className: stats_module_css_default.blockSub,
				children: t("detail.truncated", { chars: formatTokens(call.text.length) })
			}) : null, codeBlock(textLines(call.text), t, wrap)] });
		}
		/** Clipboard button: copies the displayed body and reports the outcome for a moment. */
		function CopyButton({ clipboard, clipboardValue, t }) {
			const [status, setStatus] = (0, react.useState)("idle");
			(0, react.useEffect)(() => {
				if (status === "idle") return void 0;
				const timer = setTimeout(() => {
					setStatus("idle");
				}, COPY_FEEDBACK_MS);
				return () => {
					clearTimeout(timer);
				};
			}, [status]);
			const label = status === "copied" ? t("detail.copied") : status === "failed" ? t("detail.copyFailed") : t("detail.copy");
			return (0, react_jsx_runtime.jsx)("button", {
				type: "button",
				className: stats_module_css_default.copy,
				title: t("detail.copy.title"),
				onClick: () => {
					try {
						clipboard.writeText(clipboardValue).then(() => {
							setStatus("copied");
						}, () => {
							setStatus("failed");
						});
					} catch {
						setStatus("failed");
					}
				},
				children: label
			});
		}
		/**
		* Host Clipboard API, or undefined where the app cannot copy.
		* lib.dom declares the member non-optional, but insecure contexts and jsdom omit it.
		* @returns the clipboard, or undefined when the host exposes none.
		*/
		function clipboardOf() {
			return navigator.clipboard;
		}
		/** One tool call's accordion block: head metadata plus the selected body. */
		function CallBlock({ call, t, wrap, raw, onToggleRaw }) {
			const meta = [];
			if (isNumber$1(call.chars)) meta.push(t("detail.chars", { chars: formatTokens(call.chars) }));
			if (isNumber$1(call.lines)) meta.push(t("detail.lines", { lines: formatTokens(call.lines) }));
			if (isNumber$1(call.ms)) meta.push(formatMs(call.ms));
			const fallback = [call.before ?? "", call.after ?? ""].filter((part) => part !== "").join("\n");
			const clipboardValue = raw && call.args !== null ? prettyArguments(call.args) : call.text ?? fallback;
			const clipboard = clipboardOf();
			return (0, react_jsx_runtime.jsxs)("div", {
				className: stats_module_css_default.block,
				children: [(0, react_jsx_runtime.jsxs)("div", {
					className: stats_module_css_default.blockHead,
					children: [
						(0, react_jsx_runtime.jsx)("span", {
							className: cx(stats_module_css_default.toolName, toneClass(call.name)),
							children: call.name ?? "?"
						}),
						call.hint === null ? null : (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.blockHint,
							children: ` ${call.hint}`
						}),
						meta.length === 0 ? null : (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.blockMeta,
							children: ` · ${meta.join(" · ")}`
						}),
						call.error ? (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.failPill,
							title: t("tool.failed.title", { count: 1 }),
							children: t("tool.failed", { count: 1 })
						}) : null,
						call.args === null ? null : (0, react_jsx_runtime.jsx)("button", {
							type: "button",
							className: raw ? stats_module_css_default.chipOn : stats_module_css_default.chip,
							title: t("detail.raw.title"),
							"aria-pressed": raw,
							onClick: (event) => {
								event.stopPropagation();
								onToggleRaw();
							},
							children: "{}"
						}),
						clipboardValue === "" || clipboard === void 0 ? null : (0, react_jsx_runtime.jsx)(CopyButton, {
							clipboard,
							clipboardValue,
							t
						})
					]
				}), callBody(call, t, wrap, raw)]
			});
		}
		/** Scope markers: running/interrupted/unsettled, retries, and cache misses. */
		function scopeMarkers(value, t) {
			return (0, react_jsx_runtime.jsxs)(react.Fragment, { children: [
				value.running ? (0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.stateRunning,
					children: t("state.running")
				}) : null,
				!value.running && value.interrupted ? (0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.stateWarn,
					children: t("state.interrupted")
				}) : null,
				!value.running && !value.interrupted && value.aborted ? (0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.stateMuted,
					title: t("state.aborted.title"),
					children: t("state.aborted")
				}) : null,
				value.retries > 0 ? (0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.stateWarn,
					children: t("did.retry", { count: value.retries })
				}) : null,
				isNumber$1(value.cacheHit) && value.cacheHit < .5 ? (0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.stateWarn,
					title: t("state.cacheMiss.title"),
					children: t("state.cacheMiss")
				}) : null
			] });
		}
		/** Tool pill with its tone dot and call count. */
		function ToolPill({ item, t }) {
			return (0, react_jsx_runtime.jsxs)("span", {
				className: stats_module_css_default.pill,
				title: t("did.title", {
					calls: item.count,
					tools: item.name
				}),
				children: [(0, react_jsx_runtime.jsx)("span", { className: cx(stats_module_css_default.dot, toneClass(item.name)) }), `${item.name} ×${item.count}`]
			});
		}
		/** Turn tool composition bar: one growing segment per tool, ordered by count. */
		function MixBar({ tools }) {
			return (0, react_jsx_runtime.jsx)("div", {
				className: stats_module_css_default.mix,
				children: tools.map((item) => (0, react_jsx_runtime.jsx)("span", {
					className: cx(stats_module_css_default.mixSegment, toneClass(item.name)),
					style: { flexGrow: item.count }
				}, item.name))
			});
		}
		/** Tool output summary shown after a call group in the did column. */
		function toolStat(name, calls, t) {
			if (name === "read") {
				const lines = calls.reduce((sum, call) => sum + (isNumber$1(call.lines) ? call.lines : 0), 0);
				if (lines <= 0) return null;
				return (0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.didStat,
					children: ` ${t("did.lines", { lines: formatTokens(lines) })}`
				});
			}
			if (name === "edit" || name === "write" || name === "apply_patch") {
				const added = calls.reduce((sum, call) => sum + (isNumber$1(call.added) ? call.added : 0), 0);
				const removed = calls.reduce((sum, call) => sum + (isNumber$1(call.removed) ? call.removed : 0), 0);
				if (added === 0 && removed === 0) return null;
				return (0, react_jsx_runtime.jsxs)("span", {
					className: stats_module_css_default.didStat,
					children: [
						" ",
						added > 0 ? (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.added,
							children: `+${formatTokens(added)}${removed > 0 ? "" : ` ${t("did.lines.unit")}`}`
						}) : null,
						added > 0 && removed > 0 ? " " : null,
						removed > 0 ? (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.removed,
							children: `−${formatTokens(removed)}${added > 0 ? "" : ` ${t("did.lines.unit")}`}`
						}) : null
					]
				});
			}
			const chars = calls.reduce((sum, call) => sum + (isNumber$1(call.chars) ? call.chars : 0), 0);
			return chars > 0 ? (0, react_jsx_runtime.jsx)("span", {
				className: stats_module_css_default.didStat,
				children: ` ${t("did.chars", { chars: formatChars(chars) })}`
			}) : null;
		}
		/** Group calls by tool name and target so `×2` means one target twice. */
		function callGroups(calls) {
			const groups = /* @__PURE__ */ new Map();
			for (const call of calls) {
				const key = `${call.name ?? ""}\u0000${call.hint ?? ""}`;
				const group = groups.get(key);
				if (group === void 0) groups.set(key, {
					key,
					name: call.name,
					hint: call.hint,
					calls: [call]
				});
				else group.calls.push(call);
			}
			return [...groups.values()];
		}
		/** Gap between one step's start and the previous step's end. */
		function stepGaps(turn) {
			const gaps = /* @__PURE__ */ new Map();
			let previousEnd = null;
			for (const step of turn.steps) {
				const startTime = step.startTime;
				if (isNumber$1(previousEnd) && isNumber$1(startTime) && startTime > previousEnd) gaps.set(step.step, startTime - previousEnd);
				if (isNumber$1(step.endTime)) previousEnd = step.endTime;
			}
			return gaps;
		}
		/** Context change of each step against the previous measured step in its turn. */
		function contextDeltas(turn) {
			const deltas = /* @__PURE__ */ new Map();
			let previous = null;
			for (const step of turn.steps) {
				if (!isNumber$1(step.context)) continue;
				if (isNumber$1(previous) && step.context !== previous) deltas.set(step.step, step.context - previous);
				previous = step.context;
			}
			return deltas;
		}
		/** Step slice shown for one turn: filtered pool, then the newest STEP_LIMIT. */
		function stepsOf(turn, state) {
			const filter = state.filter ?? null;
			const pool = filter === null ? turn.steps : turn.steps.filter((step) => step.tools.some((item) => item.name === filter));
			return {
				pool,
				shown: state.mode === "all" ? pool : pool.slice(-24)
			};
		}
		/** One table cell holding a formatted number plus the tooltip for its source. */
		function metricCell(value, format, t, pending) {
			if (!isNumber$1(value)) return {
				text: "—",
				title: t("unknown")
			};
			const text = format(value);
			if (pending === void 0 || pending <= 0) return {
				text,
				title: ""
			};
			return {
				text: `≥ ${text}`,
				title: t("partial", { count: pending })
			};
		}
		/** Reading of one numeric column; turn and step rows share it. */
		const COLUMN_READ = {
			context: (value) => value.context,
			output: (value) => value.output,
			cache: (value) => value.cacheHit,
			ttft: (value) => value.ttft,
			model: (value) => value.model,
			tps: (value) => value.tps,
			tool: (value) => value.tool
		};
		/** Formatting of one numeric column. */
		const COLUMN_FORMAT = {
			context: formatTokensShort,
			output: formatTokensShort,
			cache: formatPercent,
			ttft: formatMs,
			model: formatMs,
			tps: formatTps,
			tool: formatMs
		};
		/** Unsettled calls that make a measured duration a lower bound. */
		const COLUMN_PARTIAL = {
			ttft: (value) => value.ttftPending,
			model: (value) => value.modelPending,
			tool: (value) => value.toolPending
		};
		/** Source breakdown of a cell that has one, replacing the formula tooltip. */
		const COLUMN_DETAIL = { cache: (value, t) => {
			if (!isNumber$1(value.cacheHit)) return "";
			if (!isNumber$1(value.cacheRead) && !isNumber$1(value.cacheWrite)) return t("column.cache.title");
			const read = value.cacheRead ?? 0;
			const write = value.cacheWrite ?? 0;
			const context = isNumber$1(value.cacheContext) ? value.cacheContext : isNumber$1(value.context) ? value.context : read + write;
			const input = Math.max(0, context - read - write);
			return t("cache.detail", {
				read: formatTokens(read),
				input: formatTokens(input),
				write: formatTokens(write)
			});
		} };
		/** All numeric cells of one row, in table order. */
		function metricsCells(value, t) {
			return COLUMNS.map((name) => {
				const partial = COLUMN_PARTIAL[name];
				const cell = metricCell(COLUMN_READ[name](value), COLUMN_FORMAT[name], t, partial === void 0 ? void 0 : partial(value));
				const detail = COLUMN_DETAIL[name];
				if (detail === void 0) return {
					name,
					...cell
				};
				const title = detail(value, t);
				return {
					name,
					text: cell.text,
					title: title === "" ? cell.title : title
				};
			});
		}
		/** Numeric cells after the context column. */
		function MetricCells({ cells, className }) {
			return (0, react_jsx_runtime.jsx)(react.Fragment, { children: cells.slice(1).map((cell) => (0, react_jsx_runtime.jsx)("td", {
				className,
				title: cell.title === "" ? void 0 : cell.title,
				children: cell.text
			}, cell.name)) });
		}
		/** Context column: the last measured prompt size of the row. */
		function ContextCell({ value, className, delta = null }) {
			return (0, react_jsx_runtime.jsxs)("td", {
				className,
				children: [(0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.num,
					children: formatTokensShort(value)
				}), isNumber$1(delta) && delta !== 0 ? (0, react_jsx_runtime.jsx)("span", {
					className: cx(stats_module_css_default.numDelta, delta > 0 ? stats_module_css_default.deltaUp : stats_module_css_default.deltaDown),
					children: formatTokensDelta(delta)
				}) : null]
			});
		}
		/** Time-composition cell: four stacked segments plus the measured total. */
		function TimeCell({ value, scale, gap, className, trackClass, now, t }) {
			const total = totalSpan(value, now);
			if (total === null || total <= 0) return (0, react_jsx_runtime.jsx)("td", {
				className,
				title: t("unknown"),
				children: (0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.timeValue,
					children: "—"
				})
			});
			const running = isTimeRunning(value);
			const ttft = isNumber$1(value.ttft) ? Math.min(value.ttft, total) : 0;
			const model = isNumber$1(value.model) ? Math.min(value.model, total) : ttft;
			const toolSpan = isNumber$1(value.toolWall) ? value.toolWall : value.tool;
			const tool = isNumber$1(toolSpan) ? Math.min(toolSpan, total) : 0;
			const decode = Math.max(0, model - ttft);
			const rest = Math.max(0, total - model - tool);
			const share = barShare(total, scale);
			const segments = [];
			if (ttft > 0) segments.push((0, react_jsx_runtime.jsx)("span", {
				className: cx(stats_module_css_default.barSegment, stats_module_css_default.barTtft),
				style: { flexGrow: ttft }
			}, "ttft"));
			if (decode > 0) segments.push((0, react_jsx_runtime.jsx)("span", {
				className: cx(stats_module_css_default.barSegment, stats_module_css_default.barDecode),
				style: { flexGrow: decode }
			}, "decode"));
			if (tool > 0) segments.push((0, react_jsx_runtime.jsx)("span", {
				className: cx(stats_module_css_default.barSegment, stats_module_css_default.barTool),
				style: { flexGrow: tool }
			}, "tool"));
			if (rest > 0) segments.push((0, react_jsx_runtime.jsx)("span", {
				className: cx(stats_module_css_default.barSegment, stats_module_css_default.barRest),
				style: { flexGrow: rest }
			}, "rest"));
			const parts = [
				t("bar.ttft", { value: formatMs(ttft) }),
				t("bar.gen", { value: formatMs(decode) }),
				t("bar.tool", { value: formatMs(tool) }),
				t("bar.rest", { value: formatMs(rest) }),
				t("bar.wall", { value: formatMs(total) })
			];
			if (isNumber$1(value.startTime)) parts.unshift(t("bar.start", { clock: formatClock(value.startTime) }));
			if (gap !== void 0 && gap > 0) parts.push(t("bar.gap", { value: formatMs(gap) }));
			if (running) parts.push(t("bar.running"));
			return (0, react_jsx_runtime.jsx)("td", {
				className,
				title: parts.join(" · "),
				children: (0, react_jsx_runtime.jsxs)("div", {
					className: stats_module_css_default.timeWrap,
					children: [(0, react_jsx_runtime.jsx)("div", {
						className: trackClass,
						children: (0, react_jsx_runtime.jsx)("div", {
							className: stats_module_css_default.barFill,
							style: { width: `${Math.round(share * 100)}%` },
							children: segments
						})
					}), (0, react_jsx_runtime.jsx)("span", {
						className: running ? stats_module_css_default.timeRunning : stats_module_css_default.timeValue,
						children: running ? `≥ ${formatMs(total)}` : formatMs(total)
					})]
				})
			});
		}
		/** Bottom row summing the turns loaded in the current window. */
		function TotalsRow({ totals, full, t, now }) {
			const cells = metricsCells(totals, t);
			const value = {
				running: false,
				startTime: null,
				wall: totals.wall,
				ttft: totals.ttft,
				model: totals.model,
				tool: totals.tool,
				toolWall: totals.toolWall
			};
			return (0, react_jsx_runtime.jsxs)("tr", {
				className: stats_module_css_default.totalsRow,
				title: full ? t("totals.full.title") : t("totals.window.title"),
				children: [
					(0, react_jsx_runtime.jsxs)("td", {
						colSpan: 2,
						className: stats_module_css_default.totalsLabel,
						children: [(0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.totalsTitle,
							children: full ? t("totals.full") : t("totals.window")
						}), (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.totalsMeta,
							children: ` · ${t("summary", {
								turns: totals.turnCount,
								steps: totals.stepCount
							})}`
						})]
					}),
					(0, react_jsx_runtime.jsx)("td", { className: stats_module_css_default.turnMetric }),
					(0, react_jsx_runtime.jsx)(ContextCell, {
						value: totals.context,
						className: stats_module_css_default.turnMetric
					}),
					(0, react_jsx_runtime.jsx)(MetricCells, {
						cells,
						className: stats_module_css_default.turnMetric
					}),
					(0, react_jsx_runtime.jsx)(TimeCell, {
						value,
						scale: totals.wall ?? 0,
						gap: void 0,
						className: stats_module_css_default.turnMetric,
						trackClass: stats_module_css_default.barTurn,
						now,
						t
					})
				]
			});
		}
		/** Display name of one prompt image: its recorded name, otherwise its position. */
		function imageName(attachment, index, count, t) {
			return attachment.name ?? t("imagePreview.alt", {
				index,
				count
			});
		}
		/** One prompt-image thumbnail: loads while the preview is open and opens the full image on click. */
		function PromptImageThumb({ attachment, index, count, loadImage, t, onOpen }) {
			const [image, setImage] = (0, react.useState)({ status: "loading" });
			const [attempt, setAttempt] = (0, react.useState)(0);
			(0, react.useEffect)(() => {
				let live = true;
				loadImage(attachment).then((src) => {
					if (live) setImage({
						status: "ready",
						src
					});
				}, () => {
					if (live) setImage({ status: "failed" });
				});
				return () => {
					live = false;
				};
			}, [
				attachment,
				loadImage,
				attempt
			]);
			if (image.status === "ready") {
				const name = imageName(attachment, index, count, t);
				return (0, react_jsx_runtime.jsx)("button", {
					type: "button",
					className: stats_module_css_default.imageThumb,
					"aria-label": t("imagePreview.open", {
						index,
						count
					}),
					title: t("imagePreview.open", {
						index,
						count
					}),
					onClick: (event) => {
						event.stopPropagation();
						onOpen({
							src: image.src,
							alt: name
						});
					},
					children: (0, react_jsx_runtime.jsx)("img", {
						src: image.src,
						alt: name
					})
				});
			}
			return (0, react_jsx_runtime.jsxs)("span", {
				className: stats_module_css_default.imageThumbStatus,
				"data-state": image.status,
				children: [(0, react_jsx_runtime.jsx)("span", { children: image.status === "loading" ? t("imagePreview.loading") : t("imagePreview.failed") }), image.status === "failed" ? (0, react_jsx_runtime.jsx)("button", {
					type: "button",
					className: stats_module_css_default.chip,
					onClick: (event) => {
						event.stopPropagation();
						setImage({ status: "loading" });
						setAttempt((value) => value + 1);
					},
					children: t("imagePreview.retry")
				}) : null]
			});
		}
		/**
		* Image-count pill for one turn's prompt: hovering or focusing it previews the
		* images, and clicking it loads the first one full size. URLs come from the
		* shared Conversation image cache, which owns their lifetime per Session.
		*/
		function ImagePromptPill({ first, images, loadImage, t }) {
			const [lightbox, setLightbox] = (0, react.useState)(null);
			const [status, setStatus] = (0, react.useState)("idle");
			const count = images.length;
			const closeLightbox = (0, react.useCallback)(() => {
				setLightbox(null);
			}, []);
			const label = status === "idle" ? t("turn.images", { count }) : status === "loading" ? t("imagePreview.loading") : t("imagePreview.failed");
			const preview = (0, react_jsx_runtime.jsx)("div", {
				className: stats_module_css_default.imagePreview,
				role: "group",
				"aria-label": t("imagePreview.dialog"),
				"data-single": count === 1 || void 0,
				children: images.map((attachment, offset) => (0, react_jsx_runtime.jsx)(PromptImageThumb, {
					attachment,
					index: offset + 1,
					count,
					loadImage,
					t,
					onOpen: setLightbox
				}, `${attachment.attachmentId}:${offset}`))
			});
			return (0, react_jsx_runtime.jsxs)(react.Fragment, { children: [(0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.HoverCard, {
				inline: true,
				openDelayMs: IMAGE_PREVIEW_DELAY_MS,
				anchor: (0, react_jsx_runtime.jsx)("button", {
					type: "button",
					className: cx(stats_module_css_default.triggerPill, stats_module_css_default.imagePill),
					"data-state": status,
					title: t("imagePreview.hint"),
					onClick: (event) => {
						event.stopPropagation();
						setStatus("loading");
						loadImage(first).then((src) => {
							setStatus("idle");
							setLightbox({
								src,
								alt: imageName(first, 1, count, t)
							});
						}, () => {
							setStatus("failed");
						});
					},
					children: label
				}),
				content: preview
			}), lightbox === null ? null : (0, react_jsx_runtime.jsx)("span", {
				onClick: (event) => {
					event.stopPropagation();
				},
				children: (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.ImageLightbox, {
					src: lightbox.src,
					alt: lightbox.alt,
					labels: {
						dialog: t("imagePreview.dialog"),
						close: t("imagePreview.close")
					},
					onClose: closeLightbox
				})
			})] });
		}
		/** Prompt column: the turn's user question, clipped with the full text in the title. */
		function PromptCell({ turn, t, className, loadImage }) {
			const images = turn.promptImages;
			const first = images[0];
			const pill = first === void 0 ? null : (0, react_jsx_runtime.jsx)(ImagePromptPill, {
				first,
				images,
				loadImage,
				t
			});
			const source = turn.promptSource;
			if (source !== null && source !== "user") {
				const raw = turn.prompt ?? "";
				const base = triggerLabelOf(raw, source, t);
				const sender = turn.promptSender;
				const label = sender === null ? base : `${base} · ${sender}`;
				const text = triggerBodyOf(raw);
				return (0, react_jsx_runtime.jsx)("td", {
					className,
					title: raw === "" ? label : raw,
					children: (0, react_jsx_runtime.jsxs)("span", {
						className: stats_module_css_default.promptClip,
						children: [
							(0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.triggerPill,
								children: label
							}),
							pill,
							text === "" ? null : (0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.promptText,
								children: text
							})
						]
					})
				});
			}
			if (turn.prompt === null || turn.prompt === "") {
				if (first === void 0) return (0, react_jsx_runtime.jsx)("td", { className });
				return (0, react_jsx_runtime.jsx)("td", {
					className,
					title: t("turn.images", { count: images.length }),
					children: pill
				});
			}
			return (0, react_jsx_runtime.jsxs)("td", {
				className,
				title: turn.prompt,
				children: [(0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.promptText,
					children: turn.prompt
				}), pill]
			});
		}
		const TEAM_FRAME = /^Team message from /u;
		const AGENT_FRAME = /^Agent \S+ sent a message:/u;
		const SUBAGENT_FRAME = /^Background subagent \S+ finished/u;
		/** Label of the concrete wake frame, falling back to the producer source kind. */
		function triggerLabelOf(text, source, t) {
			if (TEAM_FRAME.test(text)) return t("trigger.team-message");
			if (source === "subagent-settled" && SUBAGENT_FRAME.test(text)) return t("trigger.subagent-settled");
			if (source === "agent-message" && AGENT_FRAME.test(text)) return t("trigger.agent-message");
			return triggerLabel(source, t);
		}
		/** Remove the transport framing so only the message body is previewed. */
		function triggerBodyOf(text) {
			return text.replace(/^Team message from [^:]+:\s*/u, "").replace(/^Agent \S+ sent a message:\s*/u, "").replace(/^Background subagent \S+ finished\s*/u, "");
		}
		/** Localized label for one automated turn trigger. */
		function triggerLabel(source, t) {
			switch (source) {
				case "agent-message": return t("trigger.agent-message");
				case "team-message": return t("trigger.team-message");
				case "subagent-settled": return t("trigger.subagent-settled");
				case "schedule": return t("trigger.schedule");
				case "webhook": return t("trigger.webhook");
				case "tool-jobs": return t("trigger.tool-jobs");
				case "cordis-host-runner": return t("trigger.cordis-host-runner");
				case "goal": return t("trigger.goal");
				case "runtime-context":
				case "time-context": return t("trigger.context");
				default: return t("trigger.other");
			}
		}
		/** Step did column: tool calls grouped by name and target, with output sizes. */
		/** One-line answer preview appended beside the answer label. */
		function answerPreview(answer) {
			if (answer === null) return null;
			const line = answer.split("\n").find((part) => part.trim() !== "") ?? "";
			if (line.trim() === "") return null;
			return (0, react_jsx_runtime.jsx)("span", {
				className: stats_module_css_default.answerPreview,
				title: answer,
				children: ` · ${line.trim()}`
			});
		}
		function stepDidCell(step, t) {
			const nodes = [];
			if (step.toolErrors > 0) nodes.push((0, react_jsx_runtime.jsx)("span", {
				className: stats_module_css_default.failPill,
				title: t("tool.failed.title", { count: step.toolErrors }),
				children: t("tool.failed", { count: step.toolErrors })
			}, "failed"));
			if (step.tools.length === 0) {
				const label = step.running ? t("did.working") : step.aborted ? t("did.aborted") : t("did.answer");
				nodes.push((0, react_jsx_runtime.jsx)("span", { children: label }, "text"));
				nodes.push((0, react_jsx_runtime.jsx)(react.Fragment, { children: answerPreview(step.answer) }, "answer"));
				return (0, react_jsx_runtime.jsx)("td", {
					className: stats_module_css_default.stepDid,
					children: (0, react_jsx_runtime.jsx)("span", {
						className: stats_module_css_default.didClip,
						children: nodes
					})
				});
			}
			const shown = callGroups(step.calls).slice(0, 3);
			let shownCalls = 0;
			shown.forEach((group, index) => {
				shownCalls += group.calls.length;
				if (index > 0) nodes.push((0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.didSep,
					children: "·"
				}, `sep:${group.key}`));
				nodes.push((0, react_jsx_runtime.jsx)("span", {
					className: cx(stats_module_css_default.toolName, toneClass(group.name)),
					children: group.name ?? "?"
				}, `name:${group.key}`));
				if (group.calls.length > 1) nodes.push((0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.countBadge,
					children: `×${group.calls.length}`
				}, `count:${group.key}`));
				if (group.hint !== null) nodes.push((0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.didHint,
					children: ` ${group.hint}`
				}, `hint:${group.key}`));
				if (group.name !== null) nodes.push((0, react_jsx_runtime.jsx)(react.Fragment, { children: toolStat(group.name, group.calls, t) }, `stat:${group.key}`));
			});
			const totalCalls = step.tools.reduce((sum, item) => sum + item.count, 0);
			if (totalCalls > shownCalls) nodes.push((0, react_jsx_runtime.jsx)("span", {
				className: stats_module_css_default.didHint,
				children: ` ${t("did.rest", { count: totalCalls - shownCalls })}`
			}, "more"));
			return (0, react_jsx_runtime.jsx)("td", {
				className: stats_module_css_default.stepDid,
				children: (0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.didClip,
					children: nodes
				})
			});
		}
		/** Turn did column: tool pills, composition bar, and total tool output. */
		function turnDidCell(turn, t) {
			if (turn.tools.length === 0) {
				const label = turn.running ? t("did.working") : turn.aborted ? t("did.aborted") : t("did.answer");
				return (0, react_jsx_runtime.jsx)("td", {
					className: stats_module_css_default.turnDid,
					title: turn.answer ?? void 0,
					children: (0, react_jsx_runtime.jsxs)("span", {
						className: stats_module_css_default.didClip,
						children: [label, answerPreview(turn.answer)]
					})
				});
			}
			const totalCalls = turn.tools.reduce((sum, item) => sum + item.count, 0);
			const shown = turn.tools.slice(0, 3);
			const title = t("did.title", {
				calls: totalCalls,
				tools: turn.tools.map((item) => `${item.name} ×${item.count}`).join(" · ")
			});
			return (0, react_jsx_runtime.jsxs)("td", {
				className: stats_module_css_default.turnDid,
				title,
				children: [(0, react_jsx_runtime.jsxs)("div", {
					className: stats_module_css_default.pills,
					children: [
						turn.toolErrors > 0 ? (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.failPill,
							title: t("tool.failed.title", { count: turn.toolErrors }),
							children: t("tool.failed", { count: turn.toolErrors })
						}) : null,
						shown.map((item) => (0, react_jsx_runtime.jsx)(ToolPill, {
							item,
							t
						}, item.name)),
						turn.tools.length > shown.length ? (0, react_jsx_runtime.jsx)("span", {
							className: cx(stats_module_css_default.pill, stats_module_css_default.pillMuted),
							children: t("did.more", { count: turn.tools.length - shown.length })
						}) : null,
						turn.toolOutput > 0 ? (0, react_jsx_runtime.jsx)("span", {
							className: cx(stats_module_css_default.pill, stats_module_css_default.pillMuted),
							title: t("did.total", { chars: formatTokens(turn.toolOutput) }),
							children: t("did.output", { chars: formatChars(turn.toolOutput) })
						}) : null
					]
				}), (0, react_jsx_runtime.jsx)(MixBar, { tools: turn.tools })]
			});
		}
		/** One step's detail row: every listed call as an accordion block. */
		/** Full model answer rendered with the shared Chat Markdown primitive. */
		function AnswerBody({ text, t }) {
			const labels = (0, react.useMemo)(() => ({
				code: {
					copyLabel: t("detail.copy"),
					copiedLabel: t("detail.copied"),
					toolbarLabels: {
						codeLabel: t("detail.code.title"),
						wrapLabel: t("detail.code.wrap"),
						unwrapLabel: t("detail.code.unwrap")
					}
				},
				footnotes: t("detail.footnotes")
			}), [t]);
			return (0, react_jsx_runtime.jsx)("div", {
				className: stats_module_css_default.answerBody,
				children: (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.MarkdownText, {
					text,
					labels,
					variant: "compact"
				})
			});
		}
		function stepDetailRow(step, t, state, onControl) {
			const calls = step.calls;
			const total = calls.reduce((sum, call) => sum + (isNumber$1(call.chars) ? call.chars : 0), 0);
			const wrap = state.wrap ?? false;
			const raw = state.raw ?? {};
			const allCalls = step.tools.length > 0 ? step.tools.reduce((sum, item) => sum + item.count, 0) : calls.length;
			const hidden = Math.max(0, allCalls - calls.length);
			const answerBody = step.answerBody ?? null;
			const hasCalls = calls.length > 0 || hidden > 0;
			return (0, react_jsx_runtime.jsx)("tr", { children: (0, react_jsx_runtime.jsxs)("td", {
				colSpan: TOTAL_COLUMNS,
				className: stats_module_css_default.detailCell,
				children: [answerBody === null ? null : (0, react_jsx_runtime.jsxs)("section", {
					className: stats_module_css_default.answerSection,
					children: [
						(0, react_jsx_runtime.jsxs)("div", {
							className: stats_module_css_default.detailHead,
							children: [(0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.detailTitle,
								children: t("detail.answer")
							}), (0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.blockMeta,
								children: ` · ${t("detail.chars", { chars: formatTokens(answerBody.length) })}`
							})]
						}),
						(0, react_jsx_runtime.jsx)(AnswerBody, {
							text: answerBody,
							t
						}),
						step.answerTruncated ? (0, react_jsx_runtime.jsx)("div", {
							className: stats_module_css_default.blockEmpty,
							children: t("detail.answer.truncated")
						}) : null
					]
				}), hasCalls ? (0, react_jsx_runtime.jsxs)(react_jsx_runtime.Fragment, { children: [
					(0, react_jsx_runtime.jsxs)("div", {
						className: stats_module_css_default.detailHead,
						children: [
							(0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.detailTitle,
								children: t("detail.calls", { count: allCalls })
							}),
							total > 0 ? (0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.blockMeta,
								children: ` · ${t("detail.chars", { chars: formatTokens(total) })}`
							}) : null,
							(0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.detailActions,
								children: (0, react_jsx_runtime.jsx)("button", {
									type: "button",
									className: wrap ? stats_module_css_default.chipOn : stats_module_css_default.chip,
									title: t("detail.wrap.title"),
									"aria-pressed": wrap,
									onClick: () => {
										onControl({ wrap: !wrap });
									},
									children: t("detail.wrap")
								})
							})
						]
					}),
					calls.map((call) => (0, react_jsx_runtime.jsx)(CallBlock, {
						call,
						t,
						wrap,
						raw: raw[call.callId] === true,
						onToggleRaw: () => {
							onControl({ raw: {
								...raw,
								[call.callId]: raw[call.callId] !== true
							} });
						}
					}, call.callId)),
					hidden > 0 ? (0, react_jsx_runtime.jsx)("div", {
						className: stats_module_css_default.blockEmpty,
						children: t("detail.hidden", {
							count: hidden,
							limit: 12
						})
					}) : null
				] }) : null]
			}) }, `detail:${step.turn}:${step.step}`);
		}
		/** Step-list controls of a long turn: tool filters and tail/all switching. */
		function moreRows(turn, t, state, onControl) {
			const total = turn.steps.length;
			const filter = state.filter ?? null;
			if (total <= STEP_LIMIT && filter === null) return null;
			const { pool, shown } = stepsOf(turn, state);
			const parts = [(0, react_jsx_runtime.jsx)("span", {
				className: stats_module_css_default.moreText,
				children: t("steps.total", { total })
			}, "total")];
			turn.tools.forEach((item) => {
				parts.push((0, react_jsx_runtime.jsxs)("button", {
					type: "button",
					className: filter === item.name ? stats_module_css_default.chipOn : stats_module_css_default.chip,
					title: t("steps.filter", { tool: item.name }),
					"aria-pressed": filter === item.name,
					onClick: () => {
						onControl({ filter: filter === item.name ? null : item.name });
					},
					children: [(0, react_jsx_runtime.jsx)("span", { className: cx(stats_module_css_default.dot, toneClass(item.name)) }), `${item.name} ×${item.count}`]
				}, `tool:${item.name}`));
			});
			if (filter !== null) {
				parts.push((0, react_jsx_runtime.jsx)("span", {
					className: stats_module_css_default.moreText,
					children: t("steps.matched", {
						count: pool.length,
						tool: filter
					})
				}, "matched"));
				parts.push((0, react_jsx_runtime.jsx)("button", {
					type: "button",
					className: stats_module_css_default.button,
					onClick: () => {
						onControl({ filter: null });
					},
					children: t("steps.clear")
				}, "clear"));
			}
			if (state.mode === "all" && pool.length > STEP_LIMIT) parts.push((0, react_jsx_runtime.jsx)("button", {
				type: "button",
				className: stats_module_css_default.button,
				onClick: () => {
					onControl({ mode: "tail" });
				},
				children: t("steps.tail", { limit: STEP_LIMIT })
			}, "tail"));
			else if (shown.length < pool.length) parts.push((0, react_jsx_runtime.jsx)("button", {
				type: "button",
				className: stats_module_css_default.button,
				onClick: () => {
					onControl({ mode: "all" });
				},
				children: t("steps.all", { total: pool.length })
			}, "all"));
			return (0, react_jsx_runtime.jsx)("tr", { children: (0, react_jsx_runtime.jsx)("td", {
				colSpan: TOTAL_COLUMNS,
				className: stats_module_css_default.moreCell,
				children: parts
			}) }, `more:${turn.turn}`);
		}
		/** Localized label of one non-step event kind. */
		function eventLabel(kind, t) {
			switch (kind) {
				case "request-header": return t("event.request-header");
				case "system-prompt": return t("event.system-prompt");
				case "tool-set": return t("event.tool-set");
				case "context-prune": return t("event.context-prune");
				case "request-context": return t("event.request-context");
				case "model-retry": return t("event.model-retry");
				case "model-attempt": return t("event.model-attempt");
				case "context-refresh": return t("event.context-refresh");
				case "approval": return t("event.approval");
				/* v8 ignore next -- StatsEventKind is exhaustive. */
				default: return "";
			}
		}
		/** Localized request-header reason. */
		function eventReason(reason, t) {
			switch (reason) {
				case "initial": return t("event.reason.initial");
				case "resume": return t("event.reason.resume");
				case "change": return t("event.reason.change");
				case "series": return t("event.reason.series");
				default: return reason ?? "";
			}
		}
		/** Localized system-message operation. */
		function eventAction(action, t) {
			switch (action) {
				case "append": return t("event.system.append");
				case "replace": return t("event.system.replace");
				case "clear": return t("event.system.clear");
				/* v8 ignore next -- surfaceOp is null or one of the three actions. */
				default: return "";
			}
		}
		/** Localized Context refresh source. */
		function eventSource(source, t) {
			switch (source) {
				case "runtime-context": return t("event.source.runtime");
				case "time-context": return t("event.source.time");
				default: return source ?? "";
			}
		}
		/** Localized approval state or outcome. */
		function eventApproval(status, t) {
			switch (status) {
				case "asked": return t("event.approval.asked");
				case "allowed-once": return t("event.approval.allowed");
				case "rejected": return t("event.approval.rejected");
				case "cancelled": return t("event.approval.cancelled");
				case "unavailable": return t("event.approval.unavailable");
				default: return status ?? "";
			}
		}
		/** One-line explanation of a non-step event. */
		function eventDetail(event, t) {
			switch (event.kind) {
				case "request-header": {
					const detail = t("event.header.detail", {
						reason: eventReason(event.reason, t),
						tools: String(event.toolCount ?? 0)
					});
					return event.series ? `${detail} · ${t("event.series")}` : detail;
				}
				case "system-prompt": return t("event.system.detail", {
					action: eventAction(event.surfaceOp, t),
					chars: formatChars(event.chars)
				});
				case "tool-set": {
					const parts = [];
					if (event.added.length > 0) parts.push(t("event.tools.added", { tools: event.added.join(", ") }));
					if (event.removed.length > 0) parts.push(t("event.tools.removed", { tools: event.removed.join(", ") }));
					return parts.join(" · ");
				}
				case "context-prune": return t("event.prune.detail", { tokens: formatTokens(event.shadowedTokens) });
				case "request-context": return t("event.context.detail", {
					provider: event.provider ?? "",
					model: event.model ?? "",
					mode: event.status ?? ""
				});
				case "model-retry": return t("event.retry.detail", {
					retry: String(event.retry ?? 0),
					delay: formatMs(event.delayMs)
				});
				case "model-attempt": return t("event.attempt.detail");
				case "context-refresh": return t("event.refresh.detail", {
					source: eventSource(event.status, t),
					chars: formatChars(event.chars)
				});
				case "approval": return t("event.approval.detail", {
					status: eventApproval(event.status, t),
					tool: event.label ?? ""
				});
			}
		}
		/** One synthetic non-step event row. */
		function eventRow(event, t) {
			const metrics = {
				context: null,
				output: null,
				cacheHit: null,
				ttft: null,
				model: null,
				tps: null,
				tool: null,
				cacheRead: null,
				cacheWrite: null
			};
			const detail = eventDetail(event, t);
			const side = event.provider !== null && event.model !== null ? `${event.provider} / ${event.model}` : event.kind === "system-prompt" ? eventAction(event.surfaceOp, t) : event.kind === "tool-set" ? t("event.tool-set") : event.kind === "context-refresh" ? eventSource(event.status, t) : event.kind === "approval" ? eventApproval(event.status, t) : "";
			const title = event.error === null ? t("event.title") : `${t("event.title")}\n${event.error}`;
			return (0, react_jsx_runtime.jsxs)("tr", {
				className: stats_module_css_default.eventRow,
				title,
				children: [
					(0, react_jsx_runtime.jsx)("td", {
						className: stats_module_css_default.stepCell,
						children: (0, react_jsx_runtime.jsxs)("span", {
							className: stats_module_css_default.rowTagWhen,
							children: [(0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.compactionTag,
								children: eventLabel(event.kind, t)
							}), (0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.stepClock,
								children: formatClock(event.time)
							})]
						})
					}),
					(0, react_jsx_runtime.jsx)("td", {
						className: stats_module_css_default.promptCell,
						children: (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.stepWhen,
							children: side
						})
					}),
					(0, react_jsx_runtime.jsx)("td", {
						className: stats_module_css_default.stepDid,
						children: detail
					}),
					(0, react_jsx_runtime.jsx)(ContextCell, {
						value: null,
						className: stats_module_css_default.stepMetric
					}),
					(0, react_jsx_runtime.jsx)(MetricCells, {
						cells: metricsCells(metrics, t),
						className: stats_module_css_default.stepMetric
					}),
					(0, react_jsx_runtime.jsx)("td", {
						className: stats_module_css_default.stepMetric,
						title: t("event.title"),
						children: "—"
					})
				]
			});
		}
		/**
		* One synthetic compaction row: the summary model call's cost, shown between
		* the steps it ran between but excluded from the turn's step totals.
		* @param compaction - folded compaction metrics.
		* @param t - dictionary.
		* @param now - current time for a running row.
		* @param scale - step bar scale, widened to this call's wall.
		* @returns the row.
		*/
		function compactionRow(compaction, t, now, scale) {
			const cells = metricsCells({
				context: compaction.context,
				output: compaction.output,
				cacheHit: compaction.cacheHit,
				ttft: null,
				model: null,
				tps: null,
				tool: null,
				cacheRead: compaction.cacheRead,
				cacheWrite: compaction.cacheWrite
			}, t);
			const value = {
				running: compaction.running,
				startTime: compaction.startTime,
				wall: compaction.wall,
				ttft: null,
				model: null,
				tool: null,
				toolWall: null
			};
			const range = `${formatClock(compaction.startTime)}${compaction.endTime === null ? "" : `–${formatClock(compaction.endTime)}`}`;
			const parts = [];
			if (compaction.shadowedTokens !== null) parts.push(t("compaction.shadowed", { tokens: formatTokens(compaction.shadowedTokens) }));
			if (compaction.summaryChars !== null) parts.push(t("compaction.summary", { chars: formatChars(compaction.summaryChars) }));
			if (parts.length === 0) parts.push(t("compaction.running"));
			const model = compaction.provider !== null && compaction.model !== null ? `${compaction.provider} / ${compaction.model}` : t("compaction.running");
			return (0, react_jsx_runtime.jsxs)("tr", {
				className: stats_module_css_default.compactionRow,
				title: t("compaction.title"),
				children: [
					(0, react_jsx_runtime.jsx)("td", {
						className: stats_module_css_default.stepCell,
						children: (0, react_jsx_runtime.jsxs)("span", {
							className: stats_module_css_default.rowTagWhen,
							children: [(0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.compactionTag,
								children: t("compaction.label")
							}), (0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.stepClock,
								children: range
							})]
						})
					}),
					(0, react_jsx_runtime.jsx)("td", {
						className: cx(stats_module_css_default.promptCell, stats_module_css_default.stepWhen),
						title: model,
						children: model
					}),
					(0, react_jsx_runtime.jsx)("td", {
						className: stats_module_css_default.stepDid,
						children: parts.join(" · ")
					}),
					(0, react_jsx_runtime.jsx)(ContextCell, {
						value: compaction.context,
						className: stats_module_css_default.stepMetric
					}),
					(0, react_jsx_runtime.jsx)(MetricCells, {
						cells,
						className: stats_module_css_default.stepMetric
					}),
					(0, react_jsx_runtime.jsx)(TimeCell, {
						value,
						scale: Math.max(scale, compaction.wall ?? 0),
						gap: void 0,
						className: stats_module_css_default.stepMetric,
						trackClass: stats_module_css_default.barStep,
						now,
						t
					})
				]
			});
		}
		/** Rendered rows of one turn: header, step-list controls, and step rows. */
		function stepRows(options) {
			const { turn, t, now, state, showEvents, showContextDelta, onToggleDetail, onControl } = options;
			const gaps = stepGaps(turn);
			const deltas = showContextDelta ? contextDeltas(turn) : null;
			const open = state.open ?? {};
			const stepScale = turn.steps.reduce((max, step) => {
				const span = totalSpan(step, now);
				return span !== null && span > max ? span : max;
			}, 0);
			const { shown } = stepsOf(turn, state);
			const compactions = showEvents ? turn.compactions : [];
			const events = showEvents ? turn.events : [];
			let nextCompaction = 0;
			let nextEvent = 0;
			const rows = [];
			const emitCompactionsUntil = (startTime) => {
				while (nextCompaction < compactions.length) {
					const compaction = compactions[nextCompaction];
					if (compaction === void 0 || compaction.startTime > (startTime ?? Number.POSITIVE_INFINITY)) break;
					rows.push(compactionRow(compaction, t, now, stepScale));
					nextCompaction += 1;
				}
			};
			const emitEventsUntil = (startTime) => {
				while (nextEvent < events.length) {
					const event = events[nextEvent];
					if (event === void 0 || event.time > (startTime ?? Number.POSITIVE_INFINITY)) break;
					rows.push(eventRow(event, t));
					nextEvent += 1;
				}
			};
			for (const step of shown) {
				emitCompactionsUntil(step.startTime);
				emitEventsUntil(step.startTime);
				const cells = metricsCells(step, t);
				const gap = gaps.get(step.step);
				const visibleGap = gap !== void 0 && gap >= STEP_GAP_MIN_MS ? gap : void 0;
				const expanded = open[step.step] === true;
				const hasAnswer = step.answerBody !== null;
				const hasDetail = step.calls.length > 0 || hasAnswer;
				const rowClass = [];
				if (hasDetail) rowClass.push(stats_module_css_default.stepRowClickable);
				if (step.toolErrors > 0) rowClass.push(stats_module_css_default.stepRowError);
				rows.push((0, react_jsx_runtime.jsxs)("tr", {
					className: cx(...rowClass),
					onClick: hasDetail ? () => {
						onToggleDetail(step.step);
					} : void 0,
					title: hasDetail ? t("detail.title") : void 0,
					children: [
						(0, react_jsx_runtime.jsxs)("td", {
							className: stats_module_css_default.stepCell,
							children: [
								hasDetail ? (0, react_jsx_runtime.jsx)("button", {
									type: "button",
									className: stats_module_css_default.rowToggle,
									"aria-expanded": expanded,
									"aria-label": t("step.toggle.aria", { step: step.step }),
									title: t("detail.title"),
									onClick: (event) => {
										event.stopPropagation();
										onToggleDetail(step.step);
									},
									children: t("step.label", { step: step.step })
								}) : t("step.label", { step: step.step }),
								scopeMarkers(step, t),
								isNumber$1(step.startTime) ? null : (0, react_jsx_runtime.jsx)("span", {
									className: stats_module_css_default.partialHint,
									title: t("step.partial.title"),
									children: ` · ${t("step.partial")}`
								}),
								hasDetail ? (0, react_jsx_runtime.jsx)("span", {
									className: stats_module_css_default.detailHint,
									children: ` · ${expanded ? t("detail.hide") : t("detail.show")}`
								}) : null
							]
						}),
						(0, react_jsx_runtime.jsx)("td", {
							className: stats_module_css_default.promptCell,
							title: isNumber$1(step.startTime) ? [t("step.started.title", { time: formatDateTimeMs(step.startTime) }), ...visibleGap === void 0 ? [] : [t("bar.gap", { value: formatMs(visibleGap) })]].join("\n") : void 0,
							children: (0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.stepWhen,
								children: isNumber$1(step.startTime) ? (0, react_jsx_runtime.jsxs)(react_jsx_runtime.Fragment, { children: [(0, react_jsx_runtime.jsx)("span", {
									className: stats_module_css_default.stepClock,
									children: formatStepClock(step.startTime)
								}), visibleGap === void 0 ? null : (0, react_jsx_runtime.jsx)("span", {
									className: stats_module_css_default.stepGap,
									children: t("bar.gap", { value: formatMs(visibleGap) })
								})] }) : null
							})
						}),
						stepDidCell(step, t),
						(0, react_jsx_runtime.jsx)(ContextCell, {
							value: step.context,
							className: stats_module_css_default.stepMetric,
							delta: deltas === null ? null : deltas.get(step.step) ?? null
						}),
						(0, react_jsx_runtime.jsx)(MetricCells, {
							cells,
							className: stats_module_css_default.stepMetric
						}),
						(0, react_jsx_runtime.jsx)(TimeCell, {
							value: step,
							scale: stepScale,
							gap: gaps.get(step.step),
							className: stats_module_css_default.stepMetric,
							trackClass: stats_module_css_default.barStep,
							now,
							t
						})
					]
				}, `step:${turn.turn}:${step.step}`));
				if (expanded) rows.push(stepDetailRow(step, t, state, onControl));
			}
			emitCompactionsUntil(null);
			emitEventsUntil(null);
			return rows;
		}
		/** All rows of one turn: the clickable header, then the expanded body. */
		function turnRows(options) {
			const { turn, t, loadImage, now, open, state, turnScale, contextMode, showEvents, showContextDelta, contextDelta, onToggle, onToggleDetail, onControl } = options;
			const cells = metricsCells(turn, t);
			const rows = [(0, react_jsx_runtime.jsxs)("tr", {
				"data-turn": turn.turn,
				className: stats_module_css_default.turnRow,
				onClick: onToggle,
				title: t("turn.toggle.title"),
				children: [
					(0, react_jsx_runtime.jsxs)("td", {
						className: stats_module_css_default.turnCell,
						children: [
							(0, react_jsx_runtime.jsx)("button", {
								type: "button",
								className: stats_module_css_default.rowToggle,
								"aria-expanded": open,
								"aria-label": t("turn.toggle.aria", { turn: turn.turn }),
								title: t("turn.toggle.title"),
								onClick: (event) => {
									event.stopPropagation();
									onToggle();
								},
								children: t("turn.label", { turn: turn.turn })
							}),
							isNumber$1(turn.startTime) ? (0, react_jsx_runtime.jsx)("span", {
								className: stats_module_css_default.turnClock,
								title: t("turn.started", { time: formatDateTime(turn.startTime) }),
								children: ` · ${formatDateTimeShort(turn.startTime)}`
							}) : null,
							scopeMarkers(turn, t)
						]
					}),
					(0, react_jsx_runtime.jsx)(PromptCell, {
						turn,
						t,
						className: stats_module_css_default.promptCellTurn,
						loadImage
					}),
					turnDidCell(turn, t),
					(0, react_jsx_runtime.jsx)(ContextCell, {
						value: turnContext(turn, contextMode),
						className: stats_module_css_default.turnMetric,
						delta: contextDelta
					}),
					(0, react_jsx_runtime.jsx)(MetricCells, {
						cells,
						className: stats_module_css_default.turnMetric
					}),
					(0, react_jsx_runtime.jsx)(TimeCell, {
						value: turn,
						scale: turnScale,
						gap: void 0,
						className: stats_module_css_default.turnMetric,
						trackClass: stats_module_css_default.barTurn,
						now,
						t
					})
				]
			}, `turn:${turn.turn}`)];
			if (!open) return rows;
			const controls = moreRows(turn, t, state, onControl);
			if (controls !== null) rows.push(controls);
			rows.push(...stepRows({
				turn,
				t,
				now,
				state,
				showEvents,
				showContextDelta,
				onToggleDetail,
				onControl
			}));
			return rows;
		}
		/** Footer button that switches between enabled and disabled styling. */
		function TableButton({ caption, enabled, onClick }) {
			return (0, react_jsx_runtime.jsx)("button", {
				type: "button",
				className: enabled ? stats_module_css_default.button : stats_module_css_default.buttonOff,
				disabled: !enabled,
				onClick: enabled ? onClick : void 0,
				children: caption
			});
		}
		//#endregion
		//#region lib/types/client/StatsView.js
		/** Statistics view entry: scrolling table, paging state, and footer controls. */
		/**
		* Render the Statistics table of the current conversation view target. The
		* table owns the viewport scroll: reaching the top asks the injected loader
		* for one older page while the already published turns stay in place.
		* @param props - composed conversation View props: the stats snapshot hook, the older-page loader, and the locale seat.
		* @returns the scrolling table, its empty state, and the expand/collapse footer.
		*/
		function StatsView(props) {
			const { useStats, t, loadOlder, loadImage, useProjection, cancel } = props;
			const snapshot = useStats((value) => value);
			const [view, setView] = (0, react.useState)(createViewState);
			const [now, setNow] = (0, react.useState)(() => Date.now());
			const [loading, setLoading] = (0, react.useState)(false);
			const [exhausted, setExhausted] = (0, react.useState)(false);
			const [loadFailed, setLoadFailed] = (0, react.useState)(false);
			const [visibleTurns, setVisibleTurns] = (0, react.useState)(() => /* @__PURE__ */ new Set());
			const scrollRef = (0, react.useRef)(null);
			const loadingRef = (0, react.useRef)(false);
			const pendingPrepend = (0, react.useRef)(null);
			const initialized = (0, react.useRef)(false);
			const autoLoaded = (0, react.useRef)(false);
			const turns = snapshot.turns;
			const firstTurn = turns[0]?.turn ?? null;
			const latestTurn = turns.at(-1)?.turn ?? null;
			const isTurnOpen = (turnNumber) => view.openTurns[turnNumber] ?? defaultTurnOpen(view.mode, turnNumber, latestTurn);
			const allOpen = turns.length > 0 && turns.every((turn) => isTurnOpen(turn.turn));
			const hasRunning = turns.some((turn) => isTimeRunning(turn) || turn.steps.some((step) => isTimeRunning(step)));
			(0, react.useEffect)(() => {
				if (!hasRunning) return void 0;
				setNow(Date.now());
				const timer = setInterval(() => {
					setNow(Date.now());
				}, 1e3);
				return () => {
					clearInterval(timer);
				};
			}, [hasRunning]);
			(0, react.useEffect)(() => () => {
				cancel();
			}, [cancel]);
			const projectionStats = useProjection("sessionStats");
			const projectionUsage = useProjection("tokenUsage");
			const fullTotals = view.showTotals ? totalsFromProjections(projectionStats, projectionUsage) : null;
			const totals = fullTotals ?? (view.showTotals && turns.length > 0 ? aggregateTotals(turns) : null);
			const turnDeltas = /* @__PURE__ */ new Map();
			if (view.showContextDelta) {
				let previous = null;
				for (const turn of turns) {
					const value = turnContext(turn, view.contextMode);
					if (isNumber$1(previous) && isNumber$1(value) && value !== previous) turnDeltas.set(turn.turn, value - previous);
					if (isNumber$1(value)) previous = value;
				}
			}
			const turnScale = (visibleTurns.size === 0 ? turns : turns.filter((turn) => visibleTurns.has(turn.turn))).reduce((max, turn) => {
				const span = totalSpan(turn, now);
				return span !== null && span > max ? span : max;
			}, 0);
			const toggleTurn = (turnNumber) => {
				setView((current) => {
					const open = current.openTurns[turnNumber] ?? defaultTurnOpen(current.mode, turnNumber, latestTurn);
					return {
						...current,
						openTurns: {
							...current.openTurns,
							[turnNumber]: !open
						}
					};
				});
			};
			const toggleAllTurns = () => {
				setView((current) => ({
					...current,
					openTurns: {},
					mode: allOpen ? "none" : "all"
				}));
			};
			const toggleStepDetail = (turnNumber, stepNumber) => {
				setView((current) => {
					const state = current.stepViews[turnNumber] ?? EMPTY_STEP_VIEW;
					const open = { ...state.open ?? {} };
					open[stepNumber] = open[stepNumber] !== true;
					return {
						...current,
						stepViews: {
							...current.stepViews,
							[turnNumber]: {
								...state,
								open
							}
						}
					};
				});
			};
			const patchStepView = (turnNumber, patch) => {
				setView((current) => {
					const state = current.stepViews[turnNumber] ?? EMPTY_STEP_VIEW;
					return {
						...current,
						stepViews: {
							...current.stepViews,
							[turnNumber]: {
								...state,
								...patch
							}
						}
					};
				});
			};
			const requestOlder = async () => {
				if (loadingRef.current || exhausted || turns.length === 0) return;
				const element = scrollRef.current;
				/* v8 ignore next -- the scrollport mounts with the first render, so a request never runs without it. */
				if (element === null) return;
				loadingRef.current = true;
				setLoading(true);
				setLoadFailed(false);
				const before = {
					height: element.scrollHeight,
					top: element.scrollTop,
					firstTurn
				};
				let result;
				try {
					result = await loadOlder();
				} catch {
					loadingRef.current = false;
					setLoading(false);
					setLoadFailed(true);
					return;
				}
				if (result === "grew") pendingPrepend.current = before;
				else if (result === "exhausted") {
					pendingPrepend.current = null;
					setExhausted(true);
				} else pendingPrepend.current = null;
				loadingRef.current = false;
				setLoading(false);
			};
			(0, react.useLayoutEffect)(() => {
				const element = scrollRef.current;
				/* v8 ignore next -- the scrollport mounts with the first render. */
				if (element === null) return;
				if (!initialized.current && turns.length > 0) {
					initialized.current = true;
					element.scrollTop = element.scrollHeight;
					if (!autoLoaded.current && element.clientHeight > 0) {
						autoLoaded.current = true;
						requestOlder();
					}
					return;
				}
				const pending = pendingPrepend.current;
				if (pending === null || firstTurn === pending.firstTurn) return;
				pendingPrepend.current = null;
				element.scrollTop = pending.top + Math.max(0, element.scrollHeight - pending.height);
			});
			(0, react.useEffect)(() => {
				const element = scrollRef.current;
				/* v8 ignore next -- the scrollport mounts with the first render. */
				if (element === null || typeof IntersectionObserver === "undefined") return void 0;
				const visible = /* @__PURE__ */ new Set();
				const observer = new IntersectionObserver((entries) => {
					let changed = false;
					for (const entry of entries) {
						const turn = Number(entry.target.dataset.turn);
						const had = visible.has(turn);
						if (entry.isIntersecting) visible.add(turn);
						else visible.delete(turn);
						if (had !== visible.has(turn)) changed = true;
					}
					if (changed) setVisibleTurns(new Set(visible));
				}, { root: element });
				for (const row of element.querySelectorAll("tr[data-turn]")) observer.observe(row);
				return () => {
					observer.disconnect();
				};
			}, [turns.length]);
			const handleScroll = (event) => {
				if (event.currentTarget.scrollTop > 64) return;
				requestOlder();
			};
			return (0, react_jsx_runtime.jsxs)("div", {
				className: stats_module_css_default.root,
				"data-conversation-composer-overlay": "",
				children: [(0, react_jsx_runtime.jsxs)("div", {
					ref: scrollRef,
					className: stats_module_css_default.scroll,
					role: "region",
					"aria-label": t("view.stats"),
					tabIndex: 0,
					onScroll: handleScroll,
					children: [turns.length === 0 ? null : (0, react_jsx_runtime.jsx)("div", {
						className: stats_module_css_default.older,
						children: loading ? (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.olderStatus,
							role: "status",
							children: t("loadOlder.loading")
						}) : exhausted ? (0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.olderStatus,
							children: t("loadOlder.done")
						}) : loadFailed ? (0, react_jsx_runtime.jsx)("button", {
							type: "button",
							className: stats_module_css_default.button,
							onClick: () => {
								requestOlder();
							},
							children: t("loadOlder.failed")
						}) : (0, react_jsx_runtime.jsx)("button", {
							type: "button",
							className: stats_module_css_default.button,
							onClick: () => {
								requestOlder();
							},
							children: t("loadOlder")
						})
					}), turns.length === 0 ? (0, react_jsx_runtime.jsx)("div", {
						className: stats_module_css_default.empty,
						children: t("empty")
					}) : (0, react_jsx_runtime.jsxs)("table", {
						className: stats_module_css_default.table,
						"aria-label": t("view.stats"),
						children: [
							(0, react_jsx_runtime.jsxs)("colgroup", { children: [
								(0, react_jsx_runtime.jsx)("col", {}),
								(0, react_jsx_runtime.jsx)("col", {}),
								(0, react_jsx_runtime.jsx)("col", {}),
								COLUMNS.map((name) => (0, react_jsx_runtime.jsx)("col", {}, name)),
								(0, react_jsx_runtime.jsx)("col", {})
							] }),
							(0, react_jsx_runtime.jsx)("thead", { children: (0, react_jsx_runtime.jsxs)("tr", { children: [
								(0, react_jsx_runtime.jsx)("th", {
									className: stats_module_css_default.thLeft,
									title: t("column.scope.title"),
									children: t("column.scope")
								}),
								(0, react_jsx_runtime.jsx)("th", {
									className: stats_module_css_default.thLeft,
									title: t("column.prompt.title"),
									children: t("column.prompt")
								}),
								(0, react_jsx_runtime.jsx)("th", {
									className: stats_module_css_default.thLeft,
									title: t("column.did.title"),
									children: t("column.did")
								}),
								COLUMNS.map((name) => (0, react_jsx_runtime.jsx)("th", {
									className: stats_module_css_default.th,
									title: t(`column.${name}.title`),
									children: t(`column.${name}`)
								}, name)),
								(0, react_jsx_runtime.jsx)("th", {
									className: stats_module_css_default.th,
									title: t("column.time.title"),
									children: t("column.time")
								})
							] }) }),
							(0, react_jsx_runtime.jsx)("tbody", { children: turns.flatMap((turn) => turnRows({
								turn,
								t,
								loadImage,
								now,
								open: isTurnOpen(turn.turn),
								state: view.stepViews[turn.turn] ?? EMPTY_STEP_VIEW,
								turnScale,
								contextMode: view.contextMode,
								showEvents: view.showEvents,
								showContextDelta: view.showContextDelta,
								contextDelta: view.showContextDelta ? turnDeltas.get(turn.turn) ?? null : null,
								onToggle: () => {
									toggleTurn(turn.turn);
								},
								onToggleDetail: (stepNumber) => {
									toggleStepDetail(turn.turn, stepNumber);
								},
								onControl: (patch) => {
									patchStepView(turn.turn, patch);
								}
							})) }),
							totals === null ? null : (0, react_jsx_runtime.jsx)("tfoot", { children: (0, react_jsx_runtime.jsx)(TotalsRow, {
								totals,
								full: fullTotals !== null,
								t,
								now
							}) })
						]
					})]
				}), (0, react_jsx_runtime.jsxs)("div", {
					className: stats_module_css_default.toolbar,
					children: [
						(0, react_jsx_runtime.jsx)("span", {
							className: stats_module_css_default.summary,
							children: t("summary", {
								turns: turns.length,
								steps: snapshot.stepCount
							})
						}),
						(0, react_jsx_runtime.jsxs)("div", {
							className: stats_module_css_default.controls,
							children: [
								(0, react_jsx_runtime.jsx)("span", {
									className: stats_module_css_default.controlLabel,
									children: t("table.showTotals")
								}),
								(0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Switch, {
									checked: view.showTotals,
									onChange: (next) => {
										setView((previous) => ({
											...previous,
											showTotals: next
										}));
									},
									label: t("table.showTotals")
								}),
								(0, react_jsx_runtime.jsx)("span", {
									className: stats_module_css_default.controlLabel,
									children: t("event.show")
								}),
								(0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Switch, {
									checked: view.showEvents,
									onChange: (next) => {
										setView((previous) => ({
											...previous,
											showEvents: next
										}));
									},
									label: t("event.show")
								}),
								(0, react_jsx_runtime.jsx)("span", {
									className: stats_module_css_default.controlLabel,
									children: t("context.delta.show")
								}),
								(0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Switch, {
									checked: view.showContextDelta,
									onChange: (next) => {
										setView((previous) => ({
											...previous,
											showContextDelta: next
										}));
									},
									label: t("context.delta.show"),
									title: t("context.delta.title")
								}),
								(0, react_jsx_runtime.jsx)("span", {
									className: stats_module_css_default.controlLabel,
									children: t("contextMode.label")
								}),
								(0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.SegmentedControl, {
									id: "stats-context-mode",
									value: view.contextMode,
									options: [{
										value: "latest",
										label: t("contextMode.latest"),
										title: t("contextMode.latest.title")
									}, {
										value: "cumulative",
										label: t("contextMode.cumulative"),
										title: t("contextMode.cumulative.title")
									}],
									onChange: (next) => {
										setView((previous) => ({
											...previous,
											contextMode: next
										}));
									},
									label: t("contextMode.label"),
									className: stats_module_css_default.contextMode
								})
							]
						}),
						(0, react_jsx_runtime.jsx)(TableButton, {
							caption: allOpen ? t("collapse.all") : t("expand.all"),
							enabled: turns.length > 0,
							onClick: toggleAllTurns
						})
					]
				})]
			});
		}
		//#endregion
		//#region lib/types/client/index.js
		/**
		* Browser performance plugin contributing one entry to the conversation view
		* slot: per-turn and per-step context, timing, cache, and tool-cost columns.
		*/
		/** Required services: the conversation slot and view registries, Session paging, and the locale service. */
		const inject = [
			"slots",
			"sessions",
			"uiConversation",
			"locale"
		];
		/** Resolve once the Session window has opened or failed, or the caller cancels. */
		async function whenOpened(session, signal) {
			const settled = () => {
				const state = session.getSnapshot().openState;
				return state === "open" || state === "error";
			};
			if (settled() || signal.aborted) return;
			await new Promise((resolve) => {
				const subscription = {};
				const stop = () => {
					signal.removeEventListener("abort", stop);
					subscription.cleanup?.();
					resolve();
				};
				subscription.cleanup = session.subscribe(() => {
					if (settled()) stop();
				});
				signal.addEventListener("abort", stop, { once: true });
				if (settled() || signal.aborted) stop();
			});
		}
		/** Snapshot hook name; `InjectFace` turns it into the component's `useStats` prop. */
		const HOOK = "stats";
		/**
		* Client plugin body: register the locale dictionaries, the event Definitions,
		* the view target, and the tab itself. Every registration rides the caller's
		* fiber effect, so unloading the plugin removes the tab with it.
		* @param ctx - client root context.
		*/
		function apply(ctx) {
			const sources = /* @__PURE__ */ new WeakMap();
			const pages = /* @__PURE__ */ new WeakMap();
			ctx.effect(() => ctx.locale.register(NS, {
				zh,
				en,
				ja,
				de,
				es,
				fr,
				pt,
				ru
			}), "client-ui-stats: dictionaries");
			const t = ctx.locale.bind(NS);
			registerStatsDefinitions(ctx);
			registerStatsConversationView(ctx);
			ctx.slots.inject("conversation.view", () => ctx.slots.register({
				name: "conversation.view",
				id: "stats",
				order: 40,
				locale: NS,
				label: () => t("view.stats"),
				inject: (sessionId) => {
					const binding = ctx.sessions.binding(sessionId);
					const session = binding?.session;
					if (binding === void 0 || session === void 0) throw new Error(`client-ui-stats: session "${sessionId}" is unavailable`);
					const target = ctx.uiConversation.binding(sessionId).target("stats");
					const lifecycle = new AbortController();
					let source = sources.get(binding);
					if (source === void 0) {
						source = {
							getSnapshot: () => target.getSnapshot() ?? EMPTY_STATS_SNAPSHOT,
							subscribe: (listener) => target.subscribe(listener)
						};
						sources.set(binding, source);
					}
					return {
						hooks: { [HOOK]: source },
						cancel: () => {
							lifecycle.abort();
						},
						loadOlder: () => {
							const pending = pages.get(binding);
							if (pending !== void 0) return pending;
							const promise = (async () => {
								const before = source.getSnapshot();
								await whenOpened(session, lifecycle.signal);
								if (lifecycle.signal.aborted) return "retry";
								if (session.getSnapshot().loadingOlder) return "retry";
								await session.loadOlder();
								if (source.getSnapshot() !== before) return "grew";
								return session.getSnapshot().hasMore ? "retry" : "exhausted";
							})().finally(() => {
								pages.delete(binding);
							});
							pages.set(binding, promise);
							return promise;
						},
						loadImage: (attachment) => ctx.uiConversation.imageUrl(sessionId, attachment)
					};
				}
			}, StatsView));
		}
		//#endregion
		exports.apply = apply;
		exports.inject = inject;
		return module.exports;
	}
});

//# sourceMappingURL=client.js.map