window.__ModuleLoader__.load({
	id: "@deepseek-ai/dsh-client-ui-plugin-recommendations",
	factory: (require) => {
		var module = { exports: {} };
		var exports = module.exports;
		Object.defineProperty(exports, Symbol.toStringTag, { value: "Module" });
		let zod = require("zod");
		let react = require("react");
		let _deepseek_ai_dsh_client_store = require("@deepseek-ai/dsh-client-store");
		let react_jsx_runtime = require("react/jsx-runtime");
		let _deepseek_ai_dsh_client_ui_primitives = require("@deepseek-ai/dsh-client-ui-primitives");
		//#region ../../typert/protocol/src/remote-info.ts
		/**
		* Retain one generated definition per endpoint without constructing parsers or projections.
		* The class instance builds its shared descriptor/method index on first access;
		* input schemas and JSON Schema projections are cached only when requested.
		* @param spec - compiler-produced method facts and Zod factories.
		* @param zod - the generated artifact's Zod runtime for named-input parsers.
		* @returns a lazy, mountable info instance with no registration or transport effects.
		*/
		function createTypertRemoteInfo(spec, zod) {
			return new RemoteInfo(spec, zod);
		}
		var RemoteInfo = class {
			spec;
			zod;
			index;
			constructor(spec, zod) {
				this.spec = spec;
				this.zod = zod;
			}
			get package() {
				return this.spec.package;
			}
			get descriptors() {
				return this.resolve().descriptors;
			}
			get methods() {
				return this.resolve().methods;
			}
			resolve() {
				if (this.index !== void 0) return this.index;
				const zod = this.zod;
				const descriptors = [];
				const methods = Object.create(null);
				for (const [endpoint, definition] of Object.entries(this.spec.methods)) {
					const separator = endpoint.indexOf("/");
					const namespace = endpoint.slice(0, separator);
					const method = endpoint.slice(separator + 1);
					const parameters = definition.parameters.map((parameter) => ({
						...parameter,
						wire: parameter.wire ?? parameter.name
					}));
					const descriptor = {
						...definition,
						id: `${this.spec.package}#${endpoint}`,
						namespace,
						method,
						parameters,
						result: definition.result.description === void 0 ? definition.result : new ResultValue(definition.result)
					};
					if (definition.invocation.kind === "direct") {
						const inputs = [];
						methods[endpoint] = Object.assign(descriptor, {
							input(options) {
								const strict = options?.strict === true;
								return inputs[strict ? 1 : 0] ??= new NamedInput(parameters, strict, zod);
							},
							invoke(remote, input, signal) {
								const named = input;
								const values = parameters.map((parameter) => named[parameter.wire]);
								if (definition.cancellation !== void 0) values.push(signal);
								const receiver = Reflect.get(remote, namespace);
								return Reflect.get(receiver, method).apply(receiver, values);
							}
						});
					}
					descriptors.push(descriptor);
				}
				return this.index = {
					descriptors,
					methods
				};
			}
		};
		function optional(parameter) {
			return parameter.source === "json" && parameter.acceptsUndefined === true;
		}
		var NamedInput = class {
			parameters;
			strict;
			zod;
			schema;
			projection;
			constructor(parameters, strict, zod) {
				this.parameters = parameters;
				this.strict = strict;
				this.zod = zod;
			}
			codec(parameter) {
				return this.strict ? parameter.strictCodec : parameter.codec;
			}
			create = () => this.schema ??= this.zod.strictObject(Object.fromEntries(this.parameters.map((parameter) => {
				const value = this.codec(parameter).create();
				return [parameter.wire, optional(parameter) ? value.optional() : value.nonoptional()];
			})));
			get jsonSchema() {
				return this.project().jsonSchema;
			}
			get jsonSchemaError() {
				return this.project().jsonSchemaError;
			}
			project() {
				if (this.projection !== void 0) return this.projection;
				const unavailable = this.parameters.find((parameter) => this.codec(parameter).jsonSchema === void 0);
				if (unavailable !== void 0) return this.projection = { jsonSchemaError: this.codec(unavailable).jsonSchemaError ?? `No JSON Schema for parameter ${unavailable.wire}` };
				return this.projection = { jsonSchema: {
					type: "object",
					properties: Object.fromEntries(this.parameters.map((parameter) => [parameter.wire, {
						...this.codec(parameter).jsonSchema,
						...parameter.description === void 0 ? {} : { description: parameter.description }
					}])),
					required: this.parameters.filter((parameter) => !optional(parameter)).map((parameter) => parameter.wire),
					additionalProperties: false
				} };
			}
		};
		var ResultValue = class {
			spec;
			projection;
			constructor(spec) {
				this.spec = spec;
			}
			get mode() {
				return this.spec.mode;
			}
			get typeSymbol() {
				return this.spec.typeSymbol;
			}
			get create() {
				return this.spec.create;
			}
			get decode() {
				return this.spec.decode;
			}
			get encode() {
				return this.spec.encode;
			}
			get description() {
				return this.spec.description;
			}
			get jsonSchemaError() {
				return this.spec.jsonSchemaError;
			}
			get jsonSchema() {
				if (this.projection === void 0) {
					const schema = this.spec.jsonSchema;
					this.projection = { value: schema === void 0 ? void 0 : {
						...schema,
						description: this.spec.description
					} };
				}
				return this.projection.value;
			}
		};
		//#endregion
		//#region ../plugin-recommendations/lib/typert.remote-client.js
		let _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_0$schema$value;
		const _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_0$schema = () => _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_0$schema$value ??= zod.z.string();
		let _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_1$schema$value;
		const _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_1$schema = () => _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_1$schema$value ??= zod.z.string();
		let _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_result$schema$value;
		const _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_result$schema = () => _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_result$schema$value ??= zod.z.union([zod.z.object({
			"status": zod.z.literal("available").readonly(),
			"title": zod.z.union([zod.z.string(), zod.z.intersection(zod.z.object({ "en": zod.z.string().readonly() }), zod.z.record(zod.z.string(), zod.z.string()).readonly())]).readonly(),
			"description": zod.z.union([zod.z.string(), zod.z.intersection(zod.z.object({ "en": zod.z.string().readonly() }), zod.z.record(zod.z.string(), zod.z.string()).readonly())]).readonly(),
			"enabled": zod.z.boolean().readonly()
		}), zod.z.object({
			"status": zod.z.literal("refused").readonly(),
			"reason": zod.z.union([
				zod.z.literal("unknown"),
				zod.z.literal("unavailable"),
				zod.z.literal("not-live-root"),
				zod.z.literal("closing")
			]).readonly()
		})]);
		let _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_0_strict$schema$value;
		const _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_0_strict$schema = () => _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_0_strict$schema$value ??= zod.z.string();
		let _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_1_strict$schema$value;
		const _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_1_strict$schema = () => _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_1_strict$schema$value ??= zod.z.string();
		const TYPERT_REMOTE = createTypertRemoteInfo({
			package: "@deepseek-ai/dsh-plugin-recommendations",
			methods: { "pluginRecommendations/describe": {
				service: "pluginRecommendations",
				description: "Read the current catalog text for a logged card.",
				invocation: { kind: "direct" },
				scope: {
					context: "agent",
					wire: "agentId"
				},
				parameters: [{
					name: "agent",
					wire: "agentId",
					description: "Exact live root resolved by the Gateway from the Session id.",
					source: "lookup",
					lookup: "agent",
					codec: {
						mode: "strict",
						typeSymbol: "@deepseek-ai/dsh-session/types#SessionId",
						create: _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_0$schema,
						jsonSchema: { "type": "string" }
					},
					strictCodec: {
						mode: "strict",
						typeSymbol: "@deepseek-ai/dsh-session/types#SessionId",
						create: _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_0_strict$schema,
						jsonSchema: { "type": "string" }
					}
				}, {
					name: "callId",
					description: "The card's `propose_action` call id, never a package supplied by the browser.",
					source: "json",
					codec: {
						mode: "strict",
						typeSymbol: "@deepseek-ai/dsh-llm/brand#ToolCallId",
						create: _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_1$schema,
						jsonSchema: { "type": "string" }
					},
					strictCodec: {
						mode: "strict",
						typeSymbol: "@deepseek-ai/dsh-llm/brand#ToolCallId",
						create: _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_parameter_1_strict$schema,
						jsonSchema: { "type": "string" }
					}
				}],
				result: {
					mode: "strict",
					typeSymbol: "@deepseek-ai/dsh-plugin-recommendations/types#RecommendationDetails",
					create: _deepseek_ai_dsh_plugin_recommendations_pluginRecommendations_describe_result$schema,
					description: "the plugin's title, description and selection, or a refusal when the card names no available official plugin.",
					jsonSchemaError: "typert tool schema: selected dictionaries require string keys and unconstrained JSON values"
				},
				sourceLocation: {
					"file": "packages/experimental/plugin-recommendations/src/index.ts",
					"line": 88,
					"column": 9
				}
			} }
		}, zod.z);
		//#endregion
		//#region \0dsh-css:/home/runner/work/deepseek-harness/deepseek-harness/packages/experimental/client-ui-plugin-recommendations/src/client/Card.module.css.mjs
		const css = ".KaT4gq_header{background:var(--dsw-alias-bg-module-platform);color:var(--dsw-alias-label-secondary);justify-content:space-between;align-items:center;gap:12px;padding:10px 20px;font-size:12px;line-height:1.5;display:flex}.KaT4gq_badge{font-size:12px}.KaT4gq_body{padding:20px}.KaT4gq_details{box-sizing:border-box;border-radius:var(--dsw-radius-sm);width:calc(100% + 16px);color:inherit;font:inherit;text-align:start;cursor:pointer;background:0 0;border:0;margin:-8px;padding:8px;transition:background-color .12s;display:block}@media (hover:hover){.KaT4gq_details:hover{background:var(--dsw-alias-interactive-bg-hover)}}.KaT4gq_details:focus-visible{background:var(--dsw-alias-interactive-bg-hover);box-shadow:inset 0 0 0 2px var(--dsw-focus-ring-color,var(--dsw-alias-state-business-primary));outline:none}.KaT4gq_title{overflow-wrap:anywhere;font-size:16px;font-weight:500;line-height:1.5;display:block}.KaT4gq_description{color:var(--dsw-alias-label-secondary);overflow-wrap:anywhere;margin:8px 0 0;font-size:13px;line-height:1.7;display:block}.KaT4gq_placeholder{vertical-align:middle;border-radius:var(--dsw-radius-xs);background:var(--dsw-alias-bg-skeleton);width:50%;height:.8em;display:inline-block}.KaT4gq_footer{border-top:.5px solid var(--dsw-alias-border-l1);flex-wrap:wrap;justify-content:space-between;align-items:center;gap:12px;margin-top:16px;padding-top:16px;display:flex}.KaT4gq_scope{color:var(--dsw-alias-label-secondary);font-size:12px;line-height:1.5}.KaT4gq_actions{align-items:center;gap:8px;margin-left:auto;display:flex}.KaT4gq_status{border-top:.5px solid var(--dsw-alias-border-l1);color:var(--dsw-alias-label-secondary);margin:16px 0 0;padding-top:12px;font-size:12px;line-height:1.6}";
		const tagId = "@deepseek-ai/dsh-client-ui-plugin-recommendations/Card.module.css";
		if (typeof document !== "undefined" && document.querySelector("style[data-plugin-css=" + JSON.stringify(tagId) + "]") === null) {
			const tag = document.createElement("style");
			tag.dataset.plugin = "@deepseek-ai/dsh-client-ui-plugin-recommendations";
			tag.dataset.pluginCss = tagId;
			tag.textContent = css;
			document.head.appendChild(tag);
		}
		var Card_module_css_default = {
			"actions": "KaT4gq_actions",
			"badge": "KaT4gq_badge",
			"body": "KaT4gq_body",
			"description": "KaT4gq_description",
			"details": "KaT4gq_details",
			"footer": "KaT4gq_footer",
			"header": "KaT4gq_header",
			"placeholder": "KaT4gq_placeholder",
			"scope": "KaT4gq_scope",
			"status": "KaT4gq_status",
			"title": "KaT4gq_title"
		};
		//#endregion
		//#region lib/types/client/Card.js
		/** Body of an `enable_official_plugin` card: the plugin's catalog text, its details link, the user's choice and its outcome. */
		const detailsSchema = zod.z.object({ name: zod.z.string() });
		/** Recorded decision data the body reads; see `PluginData` for its meaning. */
		const dataSchema = zod.z.object({
			application: zod.z.literal("restart-required").optional(),
			errorCode: zod.z.string().optional()
		});
		/** Phases that still offer Enable and Not now; any other recorded phase is settled. */
		const OPEN_PHASES = ["failed", "unconfirmed"];
		/** Refusal reasons with their own copy; any other reason reads as unavailable. */
		const REASON_KEYS = [
			"unknown",
			"unavailable",
			"access-denied",
			"not-live-root",
			"closing"
		];
		function reasonKey(reason) {
			return REASON_KEYS.find((key) => key === reason) ?? "unavailable";
		}
		/** Status copy of a recorded phase; `restart-required` refines enabled. */
		function statusKey(phase, data) {
			switch (phase) {
				case "dismissed":
				case "replaced":
				case "already-enabled":
				case "failed":
				case "unconfirmed": return phase;
				case "enabled": return data.application === "restart-required" ? "restart-required" : "enabled";
				default: return;
			}
		}
		/**
		* Display one plugin card: its current catalog title and description, the user's choice and its outcome.
		* Until the plugin's first catalog lookup returns, a placeholder line and the disabled actions hold the final layout.
		* A refused or lost lookup leaves Not now and a retry of the lookup, but not Enable.
		* @param props - card, state, actions and plugin-lifetime services.
		* @returns the card body.
		*/
		function PluginCardBody({ proposal, decision, sessionId, summary, act, catalog, describe, openDetails, resolveText, t }) {
			const details = detailsSchema.safeParse(proposal.details);
			const name = details.success ? details.data.name : void 0;
			const cached = (0, react.useSyncExternalStore)((0, react.useCallback)((listener) => catalog.subscribe(listener), [catalog]), () => {
				const entries = catalog.getSnapshot();
				return name !== void 0 && Object.hasOwn(entries, name) ? entries[name] : void 0;
			});
			const [lookup, setLookup] = (0, react.useState)();
			const [attempt, setAttempt] = (0, react.useState)(0);
			const [pending, setPending] = (0, react.useState)();
			const [error, setError] = (0, react.useState)();
			const descriptionId = (0, react.useId)();
			const phase = decision?.phase;
			(0, react.useEffect)(() => {
				if (name === void 0) return void 0;
				let current = true;
				describe(sessionId, proposal.callId).then((value) => {
					if (!current) return;
					if (value?.status === "available") catalog.set({
						...catalog.getSnapshot(),
						[name]: value
					});
					setLookup(value ?? null);
				}, () => {
					if (current) setLookup(null);
				});
				return () => {
					current = false;
				};
			}, [
				catalog,
				describe,
				sessionId,
				proposal.callId,
				name,
				phase,
				attempt
			]);
			if (name === void 0) return (0, react_jsx_runtime.jsx)("div", {
				className: Card_module_css_default.body,
				children: summary
			});
			const current = lookup === null || lookup?.status === "refused" ? lookup : cached ?? lookup;
			const loading = current === void 0;
			const available = current?.status === "available" ? current : void 0;
			const parsed = dataSchema.safeParse(decision?.data ?? {});
			const data = parsed.success ? parsed.data : {};
			const settled = phase !== void 0 && !OPEN_PHASES.includes(phase);
			const status = phase === void 0 ? available?.enabled ? "already-enabled" : void 0 : statusKey(phase, data);
			const offered = !settled && available?.enabled !== true;
			const enable = offered && (available !== void 0 || loading);
			const lookupProblem = settled || current === void 0 || current?.status === "available" ? void 0 : current === null ? "lookup-failed" : current.reason;
			const failure = phase === "failed" && data.errorCode !== void 0 ? data.errorCode === "incompatible-version" ? "version-error" : "management-error" : void 0;
			const title = available === void 0 ? proposal.title : resolveText(available.title);
			const run = async (verb) => {
				setPending(verb);
				setError(void 0);
				try {
					const result = await act(verb);
					if (result === void 0) setError("transport");
					else if (result.status === "refused" || result.status === "unconfirmed") setError(reasonKey(result.reason));
				} catch {
					setError("transport");
				} finally {
					setPending(void 0);
				}
			};
			const busy = pending !== void 0;
			return (0, react_jsx_runtime.jsxs)(react_jsx_runtime.Fragment, { children: [(0, react_jsx_runtime.jsxs)("header", {
				className: Card_module_css_default.header,
				children: [(0, react_jsx_runtime.jsx)("span", { children: t("heading") }), (0, react_jsx_runtime.jsx)("span", {
					className: Card_module_css_default.badge,
					children: t("official")
				})]
			}), (0, react_jsx_runtime.jsxs)("div", {
				className: Card_module_css_default.body,
				"aria-busy": loading || void 0,
				children: [
					(0, react_jsx_runtime.jsxs)("button", {
						type: "button",
						className: Card_module_css_default.details,
						onClick: () => {
							openDetails(name);
						},
						"aria-label": t("detailsFor", { title }),
						"aria-describedby": available === void 0 ? void 0 : descriptionId,
						children: [(0, react_jsx_runtime.jsx)("strong", {
							className: Card_module_css_default.title,
							children: title
						}), (loading || available !== void 0) && (0, react_jsx_runtime.jsx)("span", {
							id: descriptionId,
							className: Card_module_css_default.description,
							children: available === void 0 ? (0, react_jsx_runtime.jsx)("span", { className: Card_module_css_default.placeholder }) : resolveText(available.description)
						})]
					}),
					status !== void 0 && (0, react_jsx_runtime.jsx)("p", {
						role: "status",
						className: Card_module_css_default.status,
						children: t(status)
					}),
					failure !== void 0 && (0, react_jsx_runtime.jsx)("p", {
						role: "alert",
						className: Card_module_css_default.status,
						children: t(failure)
					}),
					lookupProblem !== void 0 && (0, react_jsx_runtime.jsx)("p", {
						role: "alert",
						className: Card_module_css_default.status,
						children: t(lookupProblem)
					}),
					offered && error !== void 0 && (0, react_jsx_runtime.jsx)("p", {
						role: "alert",
						className: Card_module_css_default.status,
						children: t(error)
					}),
					offered && (0, react_jsx_runtime.jsxs)("footer", {
						className: Card_module_css_default.footer,
						children: [(0, react_jsx_runtime.jsx)("span", {
							className: Card_module_css_default.scope,
							children: t("scope")
						}), (0, react_jsx_runtime.jsxs)("div", {
							className: Card_module_css_default.actions,
							children: [
								lookupProblem !== void 0 && (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
									variant: "ghost",
									size: "sm",
									disabled: busy,
									onClick: () => {
										setLookup(void 0);
										setAttempt((value) => value + 1);
									},
									children: t("retryLookup")
								}),
								(0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
									variant: "ghost",
									size: "sm",
									disabled: busy || loading,
									onClick: () => {
										run("dismiss");
									},
									children: t(pending === "dismiss" ? "working" : "dismiss")
								}),
								enable && (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
									variant: "primary",
									size: "sm",
									disabled: busy || loading,
									onClick: () => {
										run("enable");
									},
									children: t(pending === "enable" ? "working" : phase === void 0 ? "enable" : "retry")
								})
							]
						})]
					})
				]
			})] });
		}
		//#endregion
		//#region lib/types/client/locales.js
		/** Typed product copy for recommendation cards. */
		const zh = {
			heading: "推荐插件",
			official: "官方",
			detailsFor: "查看详情：{title}",
			"version-error": "插件与当前 DSH 版本不兼容，请更新 DSH 后重试。",
			"management-error": "无法启用插件，请打开详情检查错误后重试。",
			enable: "启用",
			dismiss: "暂不需要",
			working: "正在处理…",
			scope: "启用后可在所有会话中使用",
			enabled: "已启用",
			"already-enabled": "已在插件页开启，可查看详情了解运行状态",
			dismissed: "已忽略",
			replaced: "已替换",
			"restart-required": "已保存，重启 DSH 后生效",
			failed: "启用失败。可重试或在插件页检查。",
			retry: "重试启用",
			unconfirmed: "无法确认插件是否已启用，可重试确认。",
			"lookup-failed": "无法读取插件信息，请重试。",
			retryLookup: "重试",
			unavailable: "这个插件当前无法启用，请在插件页检查。",
			unknown: "找不到这条推荐，请重新打开会话。",
			"access-denied": "没有修改此配置的权限。",
			"not-live-root": "请在原会话中操作这条推荐。",
			closing: "服务正在重载，请稍后重试。",
			transport: "无法确认操作结果，请刷新后重试。"
		};
		/** English card copy. */
		const en = {
			heading: "Recommended plugin",
			official: "Official",
			detailsFor: "View details: {title}",
			"version-error": "This plugin is incompatible with this DSH version. Update DSH and retry.",
			"management-error": "Could not enable the plugin. Open its details to check the error and retry.",
			enable: "Enable",
			dismiss: "Not now",
			working: "Working…",
			scope: "Available in all conversations when enabled",
			enabled: "Enabled",
			"already-enabled": "Already selected in Plugins. Open details to check its runtime status.",
			dismissed: "Dismissed",
			replaced: "Replaced",
			"restart-required": "Saved. Restart DSH to apply.",
			failed: "Could not enable. Retry or check Plugins.",
			retry: "Retry enabling",
			unconfirmed: "Could not confirm whether the plugin was enabled. Retry to check.",
			"lookup-failed": "Could not load this plugin’s details. Try again.",
			retryLookup: "Try again",
			unavailable: "This plugin cannot be enabled now. Check Plugins.",
			unknown: "This recommendation is unavailable. Reopen the conversation.",
			"access-denied": "You do not have permission to change this profile.",
			"not-live-root": "Use this recommendation in its original conversation.",
			closing: "The service is reloading. Try again shortly.",
			transport: "Could not confirm the result. Refresh and try again."
		};
		/** Japanese card copy. */
		const ja = {
			heading: "おすすめのプラグイン",
			official: "公式",
			detailsFor: "詳細を表示：{title}",
			"version-error": "このプラグインは現在の DSH バージョンと互換性がありません。DSH を更新してから再試行してください。",
			"management-error": "プラグインを有効にできませんでした。詳細を開いてエラーを確認し、再試行してください。",
			enable: "有効にする",
			dismiss: "今はしない",
			working: "処理中…",
			scope: "有効にするとすべての会話で使えます",
			enabled: "有効化済み",
			"already-enabled": "プラグイン画面で有効になっています。詳細で動作状況を確認できます。",
			dismissed: "却下済み",
			replaced: "置き換え済み",
			"restart-required": "保存しました。適用するには DSH を再起動してください。",
			failed: "有効にできませんでした。再試行するか、プラグイン画面を確認してください。",
			retry: "再度有効にする",
			unconfirmed: "プラグインが有効になったか確認できませんでした。再試行して確認してください。",
			"lookup-failed": "プラグインの情報を読み込めませんでした。もう一度お試しください。",
			retryLookup: "再試行",
			unavailable: "このプラグインは現在有効にできません。プラグイン画面を確認してください。",
			unknown: "この推薦は利用できません。会話を開き直してください。",
			"access-denied": "このプロファイルを変更する権限がありません。",
			"not-live-root": "元の会話でこの推薦を操作してください。",
			closing: "サービスを再読み込み中です。しばらくしてから再試行してください。",
			transport: "結果を確認できませんでした。更新してから再試行してください。"
		};
		/** Spanish card copy. */
		const es = {
			heading: "Complemento recomendado",
			official: "Oficial",
			detailsFor: "Ver detalles: {title}",
			"version-error": "Este complemento no es compatible con esta versión de DSH. Actualiza DSH y vuelve a intentarlo.",
			"management-error": "No se pudo habilitar el complemento. Abre sus detalles para revisar el error y vuelve a intentarlo.",
			enable: "Habilitar",
			dismiss: "Ahora no",
			working: "Procesando…",
			scope: "Disponible en todas las conversaciones una vez habilitado",
			enabled: "Habilitado",
			"already-enabled": "Ya está seleccionado en Complementos. Abre los detalles para ver su estado de ejecución.",
			dismissed: "Descartado",
			replaced: "Reemplazado",
			"restart-required": "Guardado. Reinicia DSH para aplicarlo.",
			failed: "No se pudo habilitar. Reintenta o revisa Complementos.",
			retry: "Reintentar habilitación",
			unconfirmed: "No se pudo confirmar si el complemento se habilitó. Reintenta para comprobarlo.",
			"lookup-failed": "No se pudieron cargar los detalles de este complemento. Inténtalo de nuevo.",
			retryLookup: "Intentar de nuevo",
			unavailable: "Este complemento no se puede habilitar ahora. Revisa Complementos.",
			unknown: "Esta recomendación no está disponible. Vuelve a abrir la conversación.",
			"access-denied": "No tienes permiso para cambiar este perfil.",
			"not-live-root": "Usa esta recomendación en su conversación original.",
			closing: "El servicio se está recargando. Inténtalo de nuevo en breve.",
			transport: "No se pudo confirmar el resultado. Actualiza e inténtalo de nuevo."
		};
		/** French card copy. */
		const fr = {
			heading: "Plugin recommandé",
			official: "Officiel",
			detailsFor: "Voir les détails : {title}",
			"version-error": "Ce plugin n’est pas compatible avec cette version de DSH. Mettez à jour DSH et réessayez.",
			"management-error": "Impossible d’activer le plugin. Ouvrez ses détails pour vérifier l’erreur, puis réessayez.",
			enable: "Activer",
			dismiss: "Pas maintenant",
			working: "Traitement…",
			scope: "Disponible dans toutes les conversations une fois activé",
			enabled: "Activé",
			"already-enabled": "Déjà sélectionné dans Plugins. Ouvrez les détails pour vérifier son état d’exécution.",
			dismissed: "Ignoré",
			replaced: "Remplacé",
			"restart-required": "Enregistré. Redémarrez DSH pour l’appliquer.",
			failed: "Impossible d’activer. Réessayez ou vérifiez Plugins.",
			retry: "Réessayer l’activation",
			unconfirmed: "Impossible de confirmer si le plugin a été activé. Réessayez pour vérifier.",
			"lookup-failed": "Impossible de charger les détails de ce plugin. Réessayez.",
			retryLookup: "Réessayer",
			unavailable: "Ce plugin ne peut pas être activé pour le moment. Vérifiez Plugins.",
			unknown: "Cette recommandation n’est pas disponible. Rouvrez la conversation.",
			"access-denied": "Vous n’avez pas l’autorisation de modifier ce profil.",
			"not-live-root": "Utilisez cette recommandation dans sa conversation d’origine.",
			closing: "Le service se recharge. Réessayez dans un instant.",
			transport: "Impossible de confirmer le résultat. Actualisez et réessayez."
		};
		/** German card copy. */
		const de = {
			heading: "Empfohlenes Plugin",
			official: "Offiziell",
			detailsFor: "Details anzeigen: {title}",
			"version-error": "Dieses Plugin ist mit dieser DSH-Version nicht kompatibel. Aktualisieren Sie DSH und versuchen Sie es erneut.",
			"management-error": "Das Plugin konnte nicht aktiviert werden. Öffnen Sie die Details, um den Fehler zu prüfen, und versuchen Sie es erneut.",
			enable: "Aktivieren",
			dismiss: "Nicht jetzt",
			working: "Wird verarbeitet…",
			scope: "Nach dem Aktivieren in allen Gesprächen verfügbar",
			enabled: "Aktiviert",
			"already-enabled": "Bereits unter Plugins ausgewählt. Öffnen Sie die Details, um den Laufzeitstatus zu prüfen.",
			dismissed: "Verworfen",
			replaced: "Ersetzt",
			"restart-required": "Gespeichert. Starten Sie DSH neu, um die Änderung anzuwenden.",
			failed: "Aktivierung fehlgeschlagen. Versuchen Sie es erneut oder prüfen Sie Plugins.",
			retry: "Aktivierung wiederholen",
			unconfirmed: "Es konnte nicht bestätigt werden, ob das Plugin aktiviert wurde. Versuchen Sie es erneut, um es zu prüfen.",
			"lookup-failed": "Die Details dieses Plugins konnten nicht geladen werden. Versuchen Sie es erneut.",
			retryLookup: "Erneut versuchen",
			unavailable: "Dieses Plugin kann derzeit nicht aktiviert werden. Prüfen Sie Plugins.",
			unknown: "Diese Empfehlung ist nicht verfügbar. Öffnen Sie das Gespräch erneut.",
			"access-denied": "Sie haben keine Berechtigung, dieses Profil zu ändern.",
			"not-live-root": "Verwenden Sie diese Empfehlung in ihrem ursprünglichen Gespräch.",
			closing: "Der Dienst wird neu geladen. Versuchen Sie es gleich erneut.",
			transport: "Das Ergebnis konnte nicht bestätigt werden. Aktualisieren Sie die Seite und versuchen Sie es erneut."
		};
		/** Russian card copy. */
		const ru = {
			heading: "Рекомендуемый плагин",
			official: "Официальный",
			detailsFor: "Подробнее: {title}",
			"version-error": "Этот плагин несовместим с текущей версией DSH. Обновите DSH и повторите попытку.",
			"management-error": "Не удалось включить плагин. Откройте его подробности, чтобы проверить ошибку, и повторите попытку.",
			enable: "Включить",
			dismiss: "Не сейчас",
			working: "Обработка…",
			scope: "После включения доступен во всех разговорах",
			enabled: "Включено",
			"already-enabled": "Уже выбран в разделе «Плагины». Откройте подробности, чтобы проверить состояние работы.",
			dismissed: "Отклонено",
			replaced: "Заменено",
			"restart-required": "Сохранено. Перезапустите DSH, чтобы применить изменение.",
			failed: "Не удалось включить. Повторите попытку или проверьте раздел «Плагины».",
			retry: "Повторить включение",
			unconfirmed: "Не удалось подтвердить, включен ли плагин. Повторите попытку, чтобы проверить.",
			"lookup-failed": "Не удалось загрузить сведения об этом плагине. Повторите попытку.",
			retryLookup: "Повторить",
			unavailable: "Этот плагин сейчас нельзя включить. Проверьте раздел «Плагины».",
			unknown: "Эта рекомендация недоступна. Откройте разговор заново.",
			"access-denied": "У вас нет разрешения на изменение этого профиля.",
			"not-live-root": "Используйте эту рекомендацию в исходном разговоре.",
			closing: "Сервис перезагружается. Повторите попытку чуть позже.",
			transport: "Не удалось подтвердить результат. Обновите страницу и повторите попытку."
		};
		/** Portuguese card copy. */
		const pt = {
			heading: "Plug-in recomendado",
			official: "Oficial",
			detailsFor: "Ver detalhes: {title}",
			"version-error": "Este plug-in não é compatível com esta versão do DSH. Atualize o DSH e tente novamente.",
			"management-error": "Não foi possível ativar o plug-in. Abra os detalhes para verificar o erro e tente novamente.",
			enable: "Ativar",
			dismiss: "Agora não",
			working: "Processando…",
			scope: "Disponível em todas as conversas após a ativação",
			enabled: "Ativado",
			"already-enabled": "Já selecionado em Plug-ins. Abra os detalhes para verificar o status de execução.",
			dismissed: "Dispensado",
			replaced: "Substituído",
			"restart-required": "Salvo. Reinicie o DSH para aplicar.",
			failed: "Não foi possível ativar. Tente novamente ou verifique Plug-ins.",
			retry: "Tentar ativar novamente",
			unconfirmed: "Não foi possível confirmar se o plug-in foi ativado. Tente novamente para verificar.",
			"lookup-failed": "Não foi possível carregar os detalhes deste plug-in. Tente novamente.",
			retryLookup: "Tentar novamente",
			unavailable: "Este plug-in não pode ser ativado agora. Verifique Plug-ins.",
			unknown: "Esta recomendação não está disponível. Reabra a conversa.",
			"access-denied": "Você não tem permissão para alterar este perfil.",
			"not-live-root": "Use esta recomendação na conversa original.",
			closing: "O serviço está sendo recarregado. Tente novamente em instantes.",
			transport: "Não foi possível confirmar o resultado. Atualize e tente novamente."
		};
		//#endregion
		//#region lib/types/client/mount.js
		/** Register the official plugin card body, its catalog-text Remote and its localized copy. */
		/** Services the body needs; the shared card plugin provides `actionCards`. */
		const inject = [
			"locale",
			"remote",
			"actionCards",
			"pluginNavigation"
		];
		/** Mount the catalog-text Remote, then register the body for the same plugin lifetime.
		* @param ctx - Client plugin context.
		* @param contribution - Generated Remote descriptors supplied by the built entry.
		* @returns disposer removing the body before the Remote contribution.
		*/
		async function mountRecommendations(ctx, contribution) {
			const unmount = await ctx.remote.$mount(contribution);
			const body = ctx.inject(["remote.pluginRecommendations"], registerBody);
			try {
				await body;
			} catch (error) {
				await body.dispose();
				await unmount();
				throw error;
			}
			return async () => {
				await body.dispose();
				await unmount();
			};
		}
		function registerBody(ctx) {
			const services = {
				catalog: (0, _deepseek_ai_dsh_client_store.createSnapshotStore)({}),
				describe: async (sessionId, callId) => {
					const result = await ctx.remote.pluginRecommendations.describe(sessionId, callId);
					return result.ok ? result.value : void 0;
				},
				openDetails: (packageName) => {
					ctx.pluginNavigation.openBundle(packageName);
				},
				resolveText: (text) => ctx.locale.resolveText(text)
			};
			ctx.effect(() => ctx.locale.register("pluginRecommendations", {
				en,
				zh,
				ja,
				es,
				fr,
				de,
				ru,
				pt
			}), "plugin-recommendations: locale");
			ctx.effect(() => ctx.actionCards.register({
				action: "enable_official_plugin",
				locale: "pluginRecommendations",
				Body: (props) => (0, react.createElement)(PluginCardBody, {
					...props,
					...services
				})
			}), "plugin-recommendations: card body");
		}
		//#endregion
		//#region lib/types/client/index.js
		/** Mount recommendation UI and its optional Remote.
		* @param ctx - Client plugin context.
		* @returns disposer for both contributions.
		*/
		async function apply(ctx) {
			return await mountRecommendations(ctx, TYPERT_REMOTE);
		}
		//#endregion
		exports.apply = apply;
		exports.inject = inject;
		return module.exports;
	}
});

//# sourceMappingURL=client.js.map