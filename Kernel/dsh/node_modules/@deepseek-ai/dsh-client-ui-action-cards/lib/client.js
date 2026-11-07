window.__ModuleLoader__.load({
	id: "@deepseek-ai/dsh-client-ui-action-cards",
	factory: (require) => {
		var module = { exports: {} };
		var exports = module.exports;
		Object.defineProperty(exports, Symbol.toStringTag, { value: "Module" });
		let zod = require("zod");
		let react_jsx_runtime = require("react/jsx-runtime");
		let react = require("react");
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
		//#region ../action-cards/lib/typert.remote-client.js
		let _deepseek_ai_dsh_action_cards_actionCards_act_parameter_0$schema$value;
		const _deepseek_ai_dsh_action_cards_actionCards_act_parameter_0$schema = () => _deepseek_ai_dsh_action_cards_actionCards_act_parameter_0$schema$value ??= zod.z.string();
		let _deepseek_ai_dsh_action_cards_actionCards_act_parameter_1$schema$value;
		const _deepseek_ai_dsh_action_cards_actionCards_act_parameter_1$schema = () => _deepseek_ai_dsh_action_cards_actionCards_act_parameter_1$schema$value ??= zod.z.string();
		let _deepseek_ai_dsh_action_cards_actionCards_act_parameter_2$schema$value;
		const _deepseek_ai_dsh_action_cards_actionCards_act_parameter_2$schema = () => _deepseek_ai_dsh_action_cards_actionCards_act_parameter_2$schema$value ??= zod.z.string();
		let _deepseek_ai_dsh_action_cards_actionCards_act_parameter_3$schema$value;
		const _deepseek_ai_dsh_action_cards_actionCards_act_parameter_3$schema = () => _deepseek_ai_dsh_action_cards_actionCards_act_parameter_3$schema$value ??= zod.z.union([zod.z.undefined(), zod.z.record(zod.z.string(), zod.z.json())]);
		let _deepseek_ai_dsh_action_cards_actionCards_act_result$schema$value;
		const _deepseek_ai_dsh_action_cards_actionCards_act_result$schema = () => _deepseek_ai_dsh_action_cards_actionCards_act_result$schema$value ??= zod.z.union([
			zod.z.object({ "status": zod.z.literal("accepted").readonly() }),
			zod.z.object({
				"status": zod.z.literal("settled").readonly(),
				"decision": zod.z.object({
					"callId": zod.z.string().readonly(),
					"action": zod.z.string().readonly(),
					"phase": zod.z.string().readonly(),
					"data": zod.z.union([zod.z.undefined(), zod.z.record(zod.z.string(), zod.z.json()).readonly()]).readonly().optional().describe("Kind-defined fields such as a chosen location or a created package.")
				}).readonly()
			}),
			zod.z.object({
				"status": zod.z.literal("refused").readonly(),
				"reason": zod.z.string().readonly()
			}),
			zod.z.object({
				"status": zod.z.literal("unconfirmed").readonly(),
				"reason": zod.z.string().readonly()
			})
		]);
		let _deepseek_ai_dsh_action_cards_actionCards_act_parameter_0_strict$schema$value;
		const _deepseek_ai_dsh_action_cards_actionCards_act_parameter_0_strict$schema = () => _deepseek_ai_dsh_action_cards_actionCards_act_parameter_0_strict$schema$value ??= zod.z.string();
		let _deepseek_ai_dsh_action_cards_actionCards_act_parameter_1_strict$schema$value;
		const _deepseek_ai_dsh_action_cards_actionCards_act_parameter_1_strict$schema = () => _deepseek_ai_dsh_action_cards_actionCards_act_parameter_1_strict$schema$value ??= zod.z.string();
		let _deepseek_ai_dsh_action_cards_actionCards_act_parameter_2_strict$schema$value;
		const _deepseek_ai_dsh_action_cards_actionCards_act_parameter_2_strict$schema = () => _deepseek_ai_dsh_action_cards_actionCards_act_parameter_2_strict$schema$value ??= zod.z.string();
		let _deepseek_ai_dsh_action_cards_actionCards_act_parameter_3_strict$schema$value;
		const _deepseek_ai_dsh_action_cards_actionCards_act_parameter_3_strict$schema = () => _deepseek_ai_dsh_action_cards_actionCards_act_parameter_3_strict$schema$value ??= zod.z.union([zod.z.undefined(), zod.z.record(zod.z.string(), zod.z.json())]);
		const TYPERT_REMOTE = createTypertRemoteInfo({
			package: "@deepseek-ai/dsh-action-cards",
			methods: { "actionCards/act": {
				service: "actionCards",
				description: "Apply a user action to a logged card. Shared checks run first, then the kind reconciles the card; `dismiss` is recorded here and every other verb goes to the card's kind. `dismiss` also works on an unstarted card whose kind is no longer registered. Concurrent actions on one card share one operation.",
				invocation: { kind: "direct" },
				scope: {
					context: "agent",
					wire: "agentId"
				},
				parameters: [
					{
						name: "agent",
						wire: "agentId",
						description: "live root resolved from the displayed Session.",
						source: "lookup",
						lookup: "agent",
						codec: {
							mode: "strict",
							typeSymbol: "@deepseek-ai/dsh-session/types#SessionId",
							create: _deepseek_ai_dsh_action_cards_actionCards_act_parameter_0$schema,
							jsonSchema: { "type": "string" }
						},
						strictCodec: {
							mode: "strict",
							typeSymbol: "@deepseek-ai/dsh-session/types#SessionId",
							create: _deepseek_ai_dsh_action_cards_actionCards_act_parameter_0_strict$schema,
							jsonSchema: { "type": "string" }
						}
					},
					{
						name: "callId",
						description: "the card's `propose_action` call id.",
						source: "json",
						codec: {
							mode: "strict",
							typeSymbol: "@deepseek-ai/dsh-llm/brand#ToolCallId",
							create: _deepseek_ai_dsh_action_cards_actionCards_act_parameter_1$schema,
							jsonSchema: { "type": "string" }
						},
						strictCodec: {
							mode: "strict",
							typeSymbol: "@deepseek-ai/dsh-llm/brand#ToolCallId",
							create: _deepseek_ai_dsh_action_cards_actionCards_act_parameter_1_strict$schema,
							jsonSchema: { "type": "string" }
						}
					},
					{
						name: "verb",
						description: "`dismiss` or one of the kind's verbs.",
						source: "json",
						codec: {
							mode: "strict",
							typeSymbol: "@deepseek-ai/dsh-action-cards#actionCards/act:verb",
							create: _deepseek_ai_dsh_action_cards_actionCards_act_parameter_2$schema,
							jsonSchema: { "type": "string" }
						},
						strictCodec: {
							mode: "strict",
							typeSymbol: "@deepseek-ai/dsh-action-cards#actionCards/act:verb",
							create: _deepseek_ai_dsh_action_cards_actionCards_act_parameter_2_strict$schema,
							jsonSchema: { "type": "string" }
						}
					},
					{
						name: "payload",
						description: "kind-defined fields such as a chosen location.",
						source: "json",
						acceptsUndefined: true,
						codec: {
							mode: "strict",
							typeSymbol: "@deepseek-ai/dsh-action-cards#actionCards/act:payload",
							create: _deepseek_ai_dsh_action_cards_actionCards_act_parameter_3$schema,
							jsonSchema: {
								"type": "object",
								"properties": {},
								"required": [],
								"additionalProperties": true
							}
						},
						strictCodec: {
							mode: "strict",
							typeSymbol: "@deepseek-ai/dsh-action-cards#actionCards/act:payload",
							create: _deepseek_ai_dsh_action_cards_actionCards_act_parameter_3_strict$schema,
							jsonSchema: {
								"type": "object",
								"properties": {},
								"required": [],
								"additionalProperties": true
							}
						}
					}
				],
				result: {
					mode: "strict",
					typeSymbol: "@deepseek-ai/dsh-action-cards/types#ActionCardResult",
					create: _deepseek_ai_dsh_action_cards_actionCards_act_result$schema,
					description: "acceptance of background work, the decision in effect, or a refusal.",
					jsonSchema: { "oneOf": [
						{
							"type": "object",
							"properties": { "status": {
								"type": "string",
								"const": "accepted"
							} },
							"required": ["status"],
							"additionalProperties": true
						},
						{
							"type": "object",
							"properties": {
								"status": {
									"type": "string",
									"const": "settled"
								},
								"decision": {
									"type": "object",
									"properties": {
										"callId": { "type": "string" },
										"action": { "type": "string" },
										"phase": { "type": "string" },
										"data": {
											"type": "object",
											"properties": {},
											"required": [],
											"additionalProperties": true,
											"description": "Kind-defined fields such as a chosen location or a created package."
										}
									},
									"required": [
										"callId",
										"action",
										"phase"
									],
									"additionalProperties": true
								}
							},
							"required": ["status", "decision"],
							"additionalProperties": true
						},
						{
							"type": "object",
							"properties": {
								"status": {
									"type": "string",
									"const": "refused"
								},
								"reason": { "type": "string" }
							},
							"required": ["status", "reason"],
							"additionalProperties": true
						},
						{
							"type": "object",
							"properties": {
								"status": {
									"type": "string",
									"const": "unconfirmed"
								},
								"reason": { "type": "string" }
							},
							"required": ["status", "reason"],
							"additionalProperties": true
						}
					] }
				},
				sourceLocation: {
					"file": "packages/experimental/action-cards/src/index.ts",
					"line": 338,
					"column": 3
				}
			} }
		}, zod.z);
		//#endregion
		//#region \0dsh-css:/home/runner/work/deepseek-harness/deepseek-harness/packages/experimental/client-ui-action-cards/src/client/Card.module.css.mjs
		const css$1 = ".N10aSW_card{border:.5px solid var(--dsw-alias-border-l2);border-radius:var(--dsw-radius-lg);background:var(--dsw-alias-bg-layer-1);font-family:var(--dsw-font-family);color:var(--dsw-alias-label-primary);margin:16px 0;overflow:hidden}.N10aSW_body{padding:12px 20px 20px}.N10aSW_title{overflow-wrap:anywhere;font-size:16px;font-weight:500;line-height:1.5;display:block}.N10aSW_text{white-space:pre-wrap;overflow-wrap:anywhere;color:var(--dsw-alias-label-secondary);margin:8px 0 0;font-size:13px;line-height:1.7}.N10aSW_toggle{height:auto;min-height:24px;margin:8px 0 0;padding-inline:0;font-size:12px}.N10aSW_status{color:var(--dsw-alias-label-secondary);margin:12px 0 0;font-size:12px;line-height:1.6}.N10aSW_footer{justify-content:flex-end;align-items:center;gap:12px;margin-top:12px;display:flex}";
		const tagId$1 = "@deepseek-ai/dsh-client-ui-action-cards/Card.module.css";
		if (typeof document !== "undefined" && document.querySelector("style[data-plugin-css=" + JSON.stringify(tagId$1) + "]") === null) {
			const tag = document.createElement("style");
			tag.dataset.plugin = "@deepseek-ai/dsh-client-ui-action-cards";
			tag.dataset.pluginCss = tagId$1;
			tag.textContent = css$1;
			document.head.appendChild(tag);
		}
		var Card_module_css_default = {
			"body": "N10aSW_body",
			"card": "N10aSW_card",
			"footer": "N10aSW_footer",
			"status": "N10aSW_status",
			"text": "N10aSW_text",
			"title": "N10aSW_title",
			"toggle": "N10aSW_toggle"
		};
		//#endregion
		//#region lib/types/client/Card.js
		/** The shared card frame below a completed answer: title, folded brief, and the kind's body or a dismiss-only fallback. */
		/** Graphemes drawn about twice as wide as a Latin letter: CJK scripts, CJK punctuation and fullwidth forms. */
		const WIDE_GRAPHEME = /^[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}　-〿＀-｠￠-￦]/u;
		/** Preview budget in Latin-letter widths, about two card lines at desktop width. */
		const FOLD_WIDTH = 260;
		/**
		* Fold omitted text without measuring layout; a wide grapheme counts twice so Chinese and English fold near the
		* same line. The full text is never altered.
		* @param props - text and its localized toggle labels.
		* @returns the paragraph and, when text was omitted, its toggle.
		*/
		function FoldedText({ text, expandLabel, collapseLabel }) {
			const id = (0, react.useId)();
			const [expanded, setExpanded] = (0, react.useState)(false);
			const preview = (0, react.useMemo)(() => {
				const compact = text.replace(/\s+/g, " ").trim();
				const characters = Array.from(new Intl.Segmenter(void 0, { granularity: "grapheme" }).segment(compact), (part) => part.segment);
				let width = 0;
				for (const [index, character] of characters.entries()) {
					width += WIDE_GRAPHEME.test(character) ? 2 : 1;
					if (width > FOLD_WIDTH) return characters.slice(0, index).join("").trimEnd() + "…";
				}
			}, [text]);
			return (0, react_jsx_runtime.jsxs)(react_jsx_runtime.Fragment, { children: [(0, react_jsx_runtime.jsx)("p", {
				id,
				className: Card_module_css_default.text,
				children: expanded || preview === void 0 ? text : preview
			}), preview !== void 0 && (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
				variant: "ghost",
				size: "sm",
				className: Card_module_css_default.toggle,
				"aria-expanded": expanded,
				"aria-controls": id,
				onClick: () => {
					setExpanded((value) => !value);
				},
				children: expanded ? collapseLabel : expandLabel
			})] });
		}
		/** The projection admits at most one card per turn; should several exist, the lowest call id is shown. */
		function proposalOf(view, turn) {
			let found;
			for (const proposal of Object.values(view?.proposals ?? {})) if (proposal.turn === turn && (found === void 0 || proposal.callId < found.callId)) found = proposal;
			return found;
		}
		function decisionOf(view, callId) {
			return view !== void 0 && Object.hasOwn(view.decisions, callId) ? view.decisions[callId] : void 0;
		}
		/**
		* Place the card admitted in a turn after that turn's final answer, once the turn has ended with one.
		* @param props - Turn location, projection reader, registered bodies and Session-bound actions.
		* @returns the card, or null while the turn is open or when it ended without answer text.
		*/
		function ActionCardTail({ turn, useProjection, ...props }) {
			const view = useProjection("actionCards");
			const proposal = proposalOf(view, turn.turn);
			return proposal === void 0 || turn.end === void 0 || turn.data.get("turn-tail")?.closing == null ? null : (0, react_jsx_runtime.jsx)(ActionCard, {
				...props,
				proposal,
				decision: decisionOf(view, proposal.callId)
			});
		}
		/**
		* Render the frame around one card.
		* @param props - the card, its state, registered bodies and actions.
		* @returns the card section.
		*/
		function ActionCard({ proposal, decision, sessionId, act, bind, useBodies, t }) {
			const body = useBodies((bodies) => bodies.get(proposal.action));
			const send = (0, react.useCallback)((verb, payload) => act(proposal.callId, verb, payload), [act, proposal.callId]);
			const summary = (0, react_jsx_runtime.jsxs)(react_jsx_runtime.Fragment, { children: [(0, react_jsx_runtime.jsx)("strong", {
				className: Card_module_css_default.title,
				children: proposal.title
			}), (0, react_jsx_runtime.jsx)(FoldedText, {
				text: proposal.brief,
				expandLabel: t("expandBrief"),
				collapseLabel: t("collapseBrief")
			})] });
			const content = body === void 0 ? (0, react_jsx_runtime.jsx)(FallbackBody, {
				summary,
				decision,
				act: send,
				t
			}) : body.render({
				proposal,
				decision,
				sessionId,
				summary,
				Folded: FoldedText,
				act: send
			}, bind(body.locale));
			return (0, react_jsx_runtime.jsx)("section", {
				className: Card_module_css_default.card,
				"aria-label": proposal.title,
				"data-action-card": proposal.callId,
				"data-action": proposal.action,
				children: content
			});
		}
		/** Phases the fallback can name; other kind phases leave only the summary. */
		const NAMED_PHASES = {
			dismissed: "dismissed",
			replaced: "replaced"
		};
		/** Title, brief and Not now for a card whose kind registered no body in this browser. */
		function FallbackBody({ summary, decision, act, t }) {
			const [pending, setPending] = (0, react.useState)(false);
			const [error, setError] = (0, react.useState)();
			const phase = decision === void 0 || !Object.hasOwn(NAMED_PHASES, decision.phase) ? void 0 : NAMED_PHASES[decision.phase];
			const dismiss = async () => {
				setPending(true);
				setError(void 0);
				try {
					const result = await act("dismiss");
					if (result === void 0) setError("transport");
					else if (result.status === "refused" || result.status === "unconfirmed") setError("unavailable");
				} catch {
					setError("transport");
				} finally {
					setPending(false);
				}
			};
			return (0, react_jsx_runtime.jsxs)("div", {
				className: Card_module_css_default.body,
				children: [
					summary,
					phase !== void 0 && (0, react_jsx_runtime.jsx)("p", {
						role: "status",
						className: Card_module_css_default.status,
						children: t(phase)
					}),
					error !== void 0 && (0, react_jsx_runtime.jsx)("p", {
						role: "alert",
						className: Card_module_css_default.status,
						children: t(error)
					}),
					decision === void 0 && (0, react_jsx_runtime.jsx)("footer", {
						className: Card_module_css_default.footer,
						children: (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
							variant: "ghost",
							size: "sm",
							disabled: pending,
							onClick: () => {
								dismiss();
							},
							children: t("dismiss")
						})
					})
				]
			});
		}
		//#endregion
		//#region \0dsh-css:/home/runner/work/deepseek-harness/deepseek-harness/packages/experimental/client-ui-action-cards/src/client/ProposeRow.module.css.mjs
		const css = "._29hOxG_summary{min-width:0;color:inherit;align-items:center;gap:8px;margin-left:8px;font-size:12px;display:flex}._29hOxG_summary>:first-child{flex-shrink:0}._29hOxG_title{text-overflow:ellipsis;white-space:nowrap;overflow:hidden}._29hOxG_reason{border-radius:var(--dsw-radius-lg);background:var(--dsw-alias-bg-layer-1);color:var(--dsw-alias-label-secondary);white-space:pre-wrap;overflow-wrap:anywhere;margin:8px 0;padding:12px;font-size:12px}._29hOxG_inspect{color:var(--dsw-alias-link);font:inherit;cursor:pointer;background:0 0;border:none;align-self:flex-start;padding:4px 0;font-size:12px}";
		const tagId = "@deepseek-ai/dsh-client-ui-action-cards/ProposeRow.module.css";
		if (typeof document !== "undefined" && document.querySelector("style[data-plugin-css=" + JSON.stringify(tagId) + "]") === null) {
			const tag = document.createElement("style");
			tag.dataset.plugin = "@deepseek-ai/dsh-client-ui-action-cards";
			tag.dataset.pluginCss = tagId;
			tag.textContent = css;
			document.head.appendChild(tag);
		}
		var ProposeRow_module_css_default = {
			"inspect": "_29hOxG_inspect",
			"reason": "_29hOxG_reason",
			"summary": "_29hOxG_summary",
			"title": "_29hOxG_title"
		};
		//#endregion
		//#region lib/types/client/ProposeRow.js
		/** One-line process row for a `propose_action` call: the card title and whether the card was shown. */
		function stateOf(props) {
			if (props.phase === "preparing") return "preparing";
			if (props.phase === "start") return "running";
			if (props.block.error?.code === "interrupted") return "stopped";
			return props.block.isError ? "error" : "ok";
		}
		/**
		* Render a `propose_action` call from its recorded arguments and result. Only a refused call expands, to show
		* why no card followed the answer.
		* @param props - tool call and localized status copy.
		* @returns the status row.
		*/
		function ProposeRow(props) {
			const { block, inspect, t } = props;
			const [expanded, setExpanded] = (0, react.useState)(false);
			const state = stateOf(props);
			const title = block.args.text("title") ?? "";
			const reason = props.phase === "result" && props.block.isError ? props.block.content.flatMap((item) => item.type === "text" ? [item.text] : []).join("\n") : "";
			return (0, react_jsx_runtime.jsx)("div", {
				"data-tool": "propose_action",
				"data-state": state,
				children: (0, react_jsx_runtime.jsxs)(_deepseek_ai_dsh_client_ui_primitives.DisclosureRow, {
					title: t("row.title"),
					icon: (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.IconAppsAddOutlineRegular, { size: 14 }),
					running: state === "preparing" || state === "running",
					open: expanded && reason !== "",
					expandable: reason !== "",
					expandOnRowClick: true,
					keepContentWhenOpen: true,
					onToggle: () => {
						setExpanded((value) => !value);
					},
					collapsedContent: (0, react_jsx_runtime.jsxs)("span", {
						className: ProposeRow_module_css_default.summary,
						children: [(0, react_jsx_runtime.jsx)("span", { children: t(`row.${state}`) }), (0, react_jsx_runtime.jsx)("span", {
							className: ProposeRow_module_css_default.title,
							children: title
						})]
					}),
					children: [(0, react_jsx_runtime.jsx)("pre", {
						className: ProposeRow_module_css_default.reason,
						children: reason
					}), inspect && (0, react_jsx_runtime.jsx)("button", {
						type: "button",
						className: ProposeRow_module_css_default.inspect,
						onClick: inspect,
						children: t("row.inspect")
					})]
				})
			});
		}
		//#endregion
		//#region lib/types/client/registry.js
		/** Card bodies registered by action kind for the shared card frame. */
		/** Card bodies by action; the frame renders a dismiss-only fallback for an action without one. */
		var ActionCardRegistry = class {
			ctx;
			bodies = /* @__PURE__ */ new Map();
			listeners = /* @__PURE__ */ new Set();
			/** Current bodies by action; the snapshot changes on every registration change. */
			source = {
				getSnapshot: () => this.bodies,
				subscribe: (listener) => {
					this.listeners.add(listener);
					return () => {
						this.listeners.delete(listener);
					};
				}
			};
			/** @param ctx - Context whose effects own registered bodies. */
			constructor(ctx) {
				this.ctx = ctx;
			}
			/**
			* Register one kind's body. The caller holds the returned disposer inside its own `ctx.effect`, so the body
			* lives exactly as long as the plugin that contributed it.
			* @param descriptor - the kind's action, locale namespace and body.
			* @returns idempotent disposer.
			* @throws when another body already serves the action.
			*/
			register(descriptor) {
				const { action, locale, Body } = descriptor;
				const body = {
					locale,
					render: (props, t) => (0, react.createElement)(Body, {
						...props,
						t
					})
				};
				const dispose = this.ctx.effect(() => {
					if (this.bodies.has(action)) throw new Error(`action-cards: a body for "${action}" is already registered`);
					this.publish(new Map([...this.bodies, [action, body]]));
					return () => {
						const next = new Map(this.bodies);
						next.delete(action);
						this.publish(next);
					};
				}, `action-cards: body ${action}`);
				return () => {
					dispose();
				};
			}
			publish(bodies) {
				this.bodies = bodies;
				for (const listener of [...this.listeners]) listener();
			}
		};
		//#endregion
		//#region lib/types/client/locales.js
		/** English card frame, fallback and tool-row copy. */
		const en = {
			expandBrief: "Show more",
			collapseBrief: "Show less",
			dismiss: "Not now",
			dismissed: "Dismissed",
			replaced: "Replaced",
			unavailable: "This action is unavailable. Check the plugin in Plugins.",
			transport: "Could not confirm the result. Reconnect and check the card before retrying.",
			"row.title": "Action card",
			"row.preparing": "Preparing a card",
			"row.running": "Proposing",
			"row.ok": "Proposed",
			"row.error": "Not shown",
			"row.stopped": "Interrupted",
			"row.inspect": "Inspect call"
		};
		/** Chinese card frame, fallback and tool-row copy. */
		const zh = {
			expandBrief: "展开",
			collapseBrief: "收起",
			dismiss: "暂不需要",
			dismissed: "已忽略",
			replaced: "已替换",
			unavailable: "暂时无法执行，请到插件页面查看",
			transport: "暂时无法确认结果，请重新连接并查看卡片状态后再重试",
			"row.title": "操作卡片",
			"row.preparing": "正在准备卡片",
			"row.running": "正在提议",
			"row.ok": "已提议",
			"row.error": "未显示",
			"row.stopped": "已中断",
			"row.inspect": "查看调用"
		};
		/** Japanese card frame, fallback and tool-row copy. */
		const ja = {
			expandBrief: "すべて表示",
			collapseBrief: "折りたたむ",
			dismiss: "今はしない",
			dismissed: "却下済み",
			replaced: "置き換え済み",
			unavailable: "この操作は利用できません。プラグイン画面でプラグインを確認してください。",
			transport: "結果を確認できませんでした。再接続してカードを確認してから再試行してください。",
			"row.title": "アクションカード",
			"row.preparing": "カードを準備中",
			"row.running": "提案中",
			"row.ok": "提案済み",
			"row.error": "表示されませんでした",
			"row.stopped": "中断されました",
			"row.inspect": "呼び出しを確認"
		};
		/** Spanish card frame, fallback and tool-row copy. */
		const es = {
			expandBrief: "Mostrar más",
			collapseBrief: "Mostrar menos",
			dismiss: "Ahora no",
			dismissed: "Descartado",
			replaced: "Reemplazado",
			unavailable: "Esta acción no está disponible. Revisa el complemento en Complementos.",
			transport: "No se pudo confirmar el resultado. Vuelve a conectarte y revisa la tarjeta antes de reintentar.",
			"row.title": "Tarjeta de acción",
			"row.preparing": "Preparando una tarjeta",
			"row.running": "Proponiendo",
			"row.ok": "Propuesto",
			"row.error": "No se mostró",
			"row.stopped": "Interrumpido",
			"row.inspect": "Inspeccionar llamada"
		};
		/** French card frame, fallback and tool-row copy. */
		const fr = {
			expandBrief: "Afficher plus",
			collapseBrief: "Afficher moins",
			dismiss: "Pas maintenant",
			dismissed: "Ignoré",
			replaced: "Remplacé",
			unavailable: "Cette action n'est pas disponible. Vérifiez le plugin dans Plugins.",
			transport: "Impossible de confirmer le résultat. Reconnectez-vous et vérifiez la carte avant de réessayer.",
			"row.title": "Carte d'action",
			"row.preparing": "Préparation d'une carte",
			"row.running": "Proposition en cours",
			"row.ok": "Proposé",
			"row.error": "Non affiché",
			"row.stopped": "Interrompu",
			"row.inspect": "Inspecter l'appel"
		};
		/** German card frame, fallback and tool-row copy. */
		const de = {
			expandBrief: "Mehr anzeigen",
			collapseBrief: "Weniger anzeigen",
			dismiss: "Nicht jetzt",
			dismissed: "Verworfen",
			replaced: "Ersetzt",
			unavailable: "Diese Aktion ist nicht verfügbar. Prüfen Sie das Plugin unter Plugins.",
			transport: "Das Ergebnis konnte nicht bestätigt werden. Verbinden Sie sich erneut und prüfen Sie die Karte, bevor Sie es erneut versuchen.",
			"row.title": "Aktionskarte",
			"row.preparing": "Karte wird vorbereitet",
			"row.running": "Wird vorgeschlagen",
			"row.ok": "Vorgeschlagen",
			"row.error": "Nicht angezeigt",
			"row.stopped": "Unterbrochen",
			"row.inspect": "Aufruf prüfen"
		};
		/** Russian card frame, fallback and tool-row copy. */
		const ru = {
			expandBrief: "Показать больше",
			collapseBrief: "Показать меньше",
			dismiss: "Не сейчас",
			dismissed: "Отклонено",
			replaced: "Заменено",
			unavailable: "Это действие недоступно. Проверьте плагин в разделе «Плагины».",
			transport: "Не удалось подтвердить результат. Переподключитесь и проверьте карточку перед повторной попыткой.",
			"row.title": "Карточка действия",
			"row.preparing": "Подготовка карточки",
			"row.running": "Предлагается",
			"row.ok": "Предложено",
			"row.error": "Не показано",
			"row.stopped": "Прервано",
			"row.inspect": "Проверить вызов"
		};
		/** Portuguese card frame, fallback and tool-row copy. */
		const pt = {
			expandBrief: "Mostrar mais",
			collapseBrief: "Mostrar menos",
			dismiss: "Agora não",
			dismissed: "Dispensado",
			replaced: "Substituído",
			unavailable: "Esta ação não está disponível. Verifique o plug-in em Plug-ins.",
			transport: "Não foi possível confirmar o resultado. Reconecte-se e verifique o cartão antes de tentar novamente.",
			"row.title": "Cartão de ação",
			"row.preparing": "Preparando um cartão",
			"row.running": "Propondo",
			"row.ok": "Proposto",
			"row.error": "Não exibido",
			"row.stopped": "Interrompido",
			"row.inspect": "Inspecionar chamada"
		};
		//#endregion
		//#region lib/types/client/mount.js
		/** Registries available before this plugin mounts its Remote. */
		const inject = [
			"slots",
			"locale",
			"remote"
		];
		/**
		* Provide the body registry, then mount the Remote and the UI for the same plugin lifetime.
		* @param ctx - Client plugin context.
		* @param contribution - Generated Remote descriptors supplied by the built entry.
		* @returns disposer removing the UI before the Remote contribution.
		*/
		async function mountActionCards(ctx, contribution) {
			const registry = new ActionCardRegistry(ctx);
			ctx.reflect.provide("actionCards", registry);
			const unmount = await ctx.remote.$mount(contribution);
			const ui = ctx.inject(["remote.actionCards"], (inner) => {
				registerUi(inner, registry);
			});
			try {
				await ui;
			} catch (error) {
				await ui.dispose();
				await unmount();
				throw error;
			}
			return async () => {
				await ui.dispose();
				await unmount();
			};
		}
		function registerUi(ctx, registry) {
			ctx.effect(() => ctx.locale.register("actionCards", {
				en,
				zh,
				ja,
				es,
				fr,
				de,
				ru,
				pt
			}), "action-cards: locale");
			ctx.slots.inject("conversation.chat.turnTail", () => ctx.slots.register({
				name: "conversation.chat.turnTail",
				id: "@deepseek-ai/dsh-client-ui-action-cards",
				locale: "actionCards",
				inject: (sessionId) => ({
					sessionId,
					hooks: { bodies: registry.source },
					bind: (locale) => ctx.locale.bind(locale),
					act: async (callId, verb, payload) => {
						const result = await ctx.remote.actionCards.act(sessionId, callId, verb, payload);
						return result.ok ? result.value : void 0;
					}
				})
			}, ActionCardTail));
			ctx.slots.inject("tool.call.toolview", () => ctx.slots.register({
				name: "tool.call.toolview",
				key: "propose_action",
				locale: "actionCards"
			}, ProposeRow));
		}
		//#endregion
		//#region lib/types/client/index.js
		/**
		* Mount action cards and their Remote.
		* @param ctx - Client plugin context.
		* @returns disposer for every contribution.
		*/
		async function apply(ctx) {
			return await mountActionCards(ctx, TYPERT_REMOTE);
		}
		//#endregion
		exports.apply = apply;
		exports.inject = inject;
		return module.exports;
	}
});

//# sourceMappingURL=client.js.map