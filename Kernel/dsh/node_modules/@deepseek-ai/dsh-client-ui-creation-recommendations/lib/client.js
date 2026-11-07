window.__ModuleLoader__.load({
	id: "@deepseek-ai/dsh-client-ui-creation-recommendations",
	factory: (require) => {
		var module = { exports: {} };
		var exports = module.exports;
		Object.defineProperty(exports, Symbol.toStringTag, { value: "Module" });
		let react = require("react");
		let react_jsx_runtime = require("react/jsx-runtime");
		let zod = require("zod");
		let _deepseek_ai_dsh_client_ui_primitives = require("@deepseek-ai/dsh-client-ui-primitives");
		let _deepseek_ai_dsh_client_store = require("@deepseek-ai/dsh-client-store");
		//#region \0dsh-css:/home/runner/work/deepseek-harness/deepseek-harness/packages/experimental/client-ui-creation-recommendations/src/client/Card.module.css.mjs
		const css = ".EJ_5_q_header{color:var(--dsw-alias-label-secondary);flex-wrap:wrap;justify-content:space-between;align-items:center;gap:8px 12px;padding:12px 20px 0;font-size:12px;line-height:1.5;display:flex}.EJ_5_q_heading{align-items:center;gap:4px;display:inline-flex}.EJ_5_q_infoButton{width:28px;color:var(--dsw-alias-label-secondary);padding:0}.EJ_5_q_info{border-radius:var(--dsw-radius-md);background:var(--dsw-alias-bg-module-platform);overflow-wrap:anywhere;margin:12px 20px 0;padding:12px;font-size:13px;line-height:1.7}.EJ_5_q_info p{margin:0}.EJ_5_q_info p+p{margin-top:4px}.EJ_5_q_info strong{font-weight:500}.EJ_5_q_phase{color:var(--dsw-alias-label-primary)}.EJ_5_q_body{padding:12px 20px 20px}.EJ_5_q_content{margin-top:12px}.EJ_5_q_contentLabel{color:var(--dsw-alias-label-secondary);font-size:12px;line-height:1.6;display:block}.EJ_5_q_description{border-top:.5px solid var(--dsw-alias-border-l1);color:var(--dsw-alias-label-secondary);overflow-wrap:anywhere;margin:16px 0 0;padding-top:12px;font-size:12px;line-height:1.6}.EJ_5_q_footer{flex-wrap:wrap;justify-content:space-between;align-items:center;gap:12px;margin-top:12px;display:flex}.EJ_5_q_scope{color:var(--dsw-alias-label-secondary);font-size:12px;line-height:1.5}.EJ_5_q_actions{align-items:center;gap:8px;margin-left:auto;display:flex}.EJ_5_q_status{color:var(--dsw-alias-label-secondary);margin:12px 0 0;font-size:12px;line-height:1.6}.EJ_5_q_controls{border-top:.5px solid var(--dsw-alias-border-l1);flex-wrap:wrap;align-items:center;gap:12px;margin-top:16px;padding-top:16px;display:flex}.EJ_5_q_controls .EJ_5_q_footer{margin:0 0 0 auto}.EJ_5_q_locationRow{align-items:center;gap:8px;min-width:0;display:flex}.EJ_5_q_locationLabel{color:var(--dsw-alias-label-secondary);font-size:12px;line-height:1.6}.EJ_5_q_locationButton{gap:8px;font-size:13px}.EJ_5_q_locationMenu{width:264px;max-width:calc(100vw - 24px)}.EJ_5_q_locationOption{white-space:normal;flex-direction:column;gap:2px;line-height:1.5;display:flex}.EJ_5_q_locationOption>span+span{color:var(--dsw-alias-label-secondary);font-size:12px}.EJ_5_q_customLocation{flex-direction:column;flex:1 0 100%;gap:6px;min-width:0;display:flex}.EJ_5_q_customLocation textarea{box-sizing:border-box;resize:vertical;border:.5px solid var(--dsw-alias-border-l4);border-radius:var(--dsw-radius-md);background:var(--dsw-alias-bg-layer-1);width:100%;min-height:72px;color:var(--dsw-alias-label-primary);font:inherit;padding:8px 10px;font-size:13px;line-height:1.7}.EJ_5_q_customLocation textarea:focus-visible{outline:1px solid var(--dsw-alias-state-business-primary);outline-offset:1px}.EJ_5_q_customLocation textarea::placeholder{color:var(--dsw-alias-label-secondary)}";
		const tagId = "@deepseek-ai/dsh-client-ui-creation-recommendations/Card.module.css";
		if (typeof document !== "undefined" && document.querySelector("style[data-plugin-css=" + JSON.stringify(tagId) + "]") === null) {
			const tag = document.createElement("style");
			tag.dataset.plugin = "@deepseek-ai/dsh-client-ui-creation-recommendations";
			tag.dataset.pluginCss = tagId;
			tag.textContent = css;
			document.head.appendChild(tag);
		}
		var Card_module_css_default = {
			"actions": "EJ_5_q_actions",
			"body": "EJ_5_q_body",
			"content": "EJ_5_q_content",
			"contentLabel": "EJ_5_q_contentLabel",
			"controls": "EJ_5_q_controls",
			"customLocation": "EJ_5_q_customLocation",
			"description": "EJ_5_q_description",
			"footer": "EJ_5_q_footer",
			"header": "EJ_5_q_header",
			"heading": "EJ_5_q_heading",
			"info": "EJ_5_q_info",
			"infoButton": "EJ_5_q_infoButton",
			"locationButton": "EJ_5_q_locationButton",
			"locationLabel": "EJ_5_q_locationLabel",
			"locationMenu": "EJ_5_q_locationMenu",
			"locationOption": "EJ_5_q_locationOption",
			"locationRow": "EJ_5_q_locationRow",
			"phase": "EJ_5_q_phase",
			"scope": "EJ_5_q_scope",
			"status": "EJ_5_q_status"
		};
		//#endregion
		//#region lib/types/client/Card.js
		/** Body of a `create_personalized_page` card: location choice, creation progress, retention, undo and restore. */
		const slotSchema = zod.z.enum([
			"sidebar-right",
			"page",
			"sidebar-left"
		]);
		const detailsSchema = zod.z.object({
			slot: slotSchema,
			initialContent: zod.z.string().optional()
		});
		const dataSchema = zod.z.object({
			slot: slotSchema,
			customLocation: zod.z.string().optional(),
			bundleName: zod.z.string().optional(),
			application: zod.z.enum([
				"applied",
				"restart-required",
				"overridden",
				"failed",
				"cancelled"
			]).optional()
		});
		/** Phases with their own status copy. */
		const PHASE_KEYS = [
			"dismissed",
			"replaced",
			"creating",
			"installing",
			"cancelled",
			"installed",
			"kept",
			"removed",
			"failed",
			"interrupted",
			"unconfirmed"
		];
		/** Refusal reasons with their own copy; any other reason reads as unavailable. */
		const REASON_KEYS = [
			"unknown",
			"access-denied",
			"not-live-root",
			"closing",
			"unavailable",
			"invalid-location",
			"read-only",
			"source-missing"
		];
		/** Application outcomes have their own copy; the phase copy of failed and cancelled describes creation instead. */
		const APPLICATION_KEYS = {
			"restart-required": "restart-required",
			overridden: "overridden",
			failed: "applicationFailed",
			cancelled: "applicationCancelled"
		};
		function keyOf(keys, value) {
			return keys.find((key) => key === value);
		}
		function LocationPicker({ value, disabled, onChange, t }) {
			const [open, setOpen] = (0, react.useState)(false);
			return (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Menu, {
				open: open && !disabled,
				portal: true,
				autoFocus: true,
				compact: true,
				listClassName: Card_module_css_default.locationMenu,
				selectedId: value,
				onClose: () => {
					setOpen(false);
				},
				onSelect: (id) => {
					onChange(id);
					setOpen(false);
				},
				items: [
					"sidebar-right",
					"page",
					"sidebar-left",
					"custom"
				].map((id) => ({
					id,
					label: (0, react_jsx_runtime.jsxs)("span", {
						className: Card_module_css_default.locationOption,
						children: [(0, react_jsx_runtime.jsx)("span", { children: t(id) }), (0, react_jsx_runtime.jsx)("span", { children: t(`${id}Hint`) })]
					})
				})),
				anchor: (0, react_jsx_runtime.jsxs)(_deepseek_ai_dsh_client_ui_primitives.Button, {
					variant: "outline",
					size: "sm",
					disabled,
					className: Card_module_css_default.locationButton,
					"aria-label": `${t("location")}: ${t(value)}`,
					"aria-haspopup": "menu",
					"aria-expanded": open && !disabled,
					onClick: () => {
						setOpen((current) => !current);
					},
					children: [t(value), (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.IconChevronDownOutlineRegular, { size: 14 })]
				})
			});
		}
		/**
		* Display creation, persistent installation, reversible removal and restoration below the frame's summary.
		* @param props - card, state, actions, folded-text component and notice preference.
		* @returns the card body.
		*/
		function PageCardBody({ proposal, decision, summary, Folded, act, infoDismissed, t }) {
			const details = detailsSchema.safeParse(proposal.details);
			const parsed = decision === void 0 ? void 0 : dataSchema.safeParse(decision.data);
			const data = parsed?.success === true ? parsed.data : void 0;
			const phase = decision === void 0 ? void 0 : keyOf(PHASE_KEYS, decision.phase);
			const [selected, setSelected] = (0, react.useState)();
			const [custom, setCustom] = (0, react.useState)();
			const [info, setInfo] = (0, react.useState)();
			const noticeDismissed = (0, react.useSyncExternalStore)((0, react.useCallback)((listener) => infoDismissed.subscribe(listener), [infoDismissed]), () => infoDismissed.getSnapshot());
			const infoId = (0, react.useId)();
			const customId = (0, react.useId)();
			const [pending, setPending] = (0, react.useState)();
			const [error, setError] = (0, react.useState)();
			if (!details.success) return (0, react_jsx_runtime.jsx)("div", {
				className: Card_module_css_default.body,
				children: summary
			});
			const busy = pending !== void 0;
			const slot = selected !== void 0 && selected !== "custom" ? selected : data?.slot ?? details.data.slot;
			const location = selected ?? (data?.customLocation === void 0 ? slot : "custom");
			const customLocation = custom ?? data?.customLocation ?? "";
			const offer = decision === void 0 || phase === "cancelled" || phase === "failed" || phase === "interrupted" || phase === "unconfirmed";
			const infoOpen = info ?? (offer && !noticeDismissed);
			const installed = phase === "installed" || phase === "kept";
			const run = async (verb) => {
				setPending(verb);
				setError(void 0);
				if (verb === "create" || verb === "dismiss") {
					infoDismissed.set(true);
					setInfo(false);
				}
				const placement = {
					slot,
					...location === "custom" ? { customLocation: customLocation.trim() } : {}
				};
				try {
					const result = await act(verb, verb === "create" || verb === "dismiss" ? placement : void 0);
					if (result === void 0) setError("transport");
					else if (result.status === "refused" || result.status === "unconfirmed") setError(keyOf(REASON_KEYS, result.reason) ?? "unavailable");
				} catch {
					setError("transport");
				} finally {
					setPending(void 0);
				}
			};
			return (0, react_jsx_runtime.jsxs)(react_jsx_runtime.Fragment, { children: [
				(0, react_jsx_runtime.jsxs)("header", {
					className: Card_module_css_default.header,
					children: [(0, react_jsx_runtime.jsxs)("span", {
						className: Card_module_css_default.heading,
						children: [t("heading"), (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Tooltip, {
							label: t("creationInfo"),
							side: "top",
							portal: true,
							disabled: infoOpen,
							children: (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
								variant: "ghost",
								size: "sm",
								className: Card_module_css_default.infoButton,
								"aria-label": t("creationInfo"),
								"aria-expanded": infoOpen,
								"aria-controls": infoId,
								onClick: () => {
									setInfo(!infoOpen);
									if (infoOpen) infoDismissed.set(true);
								},
								children: (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.IconWarningTriangleOutlineRegular, { size: 14 })
							})
						})]
					}), phase !== void 0 && (0, react_jsx_runtime.jsx)("span", {
						role: "status",
						className: Card_module_css_default.phase,
						"data-phase": phase,
						children: t(phase)
					})]
				}),
				infoOpen && (0, react_jsx_runtime.jsxs)("div", {
					id: infoId,
					role: "note",
					className: Card_module_css_default.info,
					children: [
						(0, react_jsx_runtime.jsx)("p", { children: t("creationMethod") }),
						(0, react_jsx_runtime.jsx)("p", { children: (0, react_jsx_runtime.jsx)("strong", { children: t("creationCost") }) }),
						(0, react_jsx_runtime.jsx)("p", { children: t("creationBackground") }),
						data?.bundleName !== void 0 && (0, react_jsx_runtime.jsx)("p", { children: t("sourceRetained") })
					]
				}),
				(0, react_jsx_runtime.jsxs)("div", {
					className: Card_module_css_default.body,
					children: [
						summary,
						details.data.initialContent !== void 0 && (0, react_jsx_runtime.jsxs)("div", {
							className: Card_module_css_default.content,
							children: [(0, react_jsx_runtime.jsx)("span", {
								className: Card_module_css_default.contentLabel,
								children: t("initialContent")
							}), (0, react_jsx_runtime.jsx)(Folded, {
								text: details.data.initialContent,
								expandLabel: t("expandContent"),
								collapseLabel: t("collapseContent")
							})]
						}),
						data?.application !== void 0 && data.application !== "applied" && phase !== "failed" && (0, react_jsx_runtime.jsx)("p", {
							role: "status",
							className: Card_module_css_default.status,
							children: t(APPLICATION_KEYS[data.application])
						}),
						error !== void 0 && (0, react_jsx_runtime.jsx)("p", {
							role: "alert",
							className: Card_module_css_default.status,
							children: t(error)
						}),
						offer ? (0, react_jsx_runtime.jsxs)("div", {
							className: Card_module_css_default.controls,
							children: [
								(0, react_jsx_runtime.jsxs)("div", {
									className: Card_module_css_default.locationRow,
									children: [(0, react_jsx_runtime.jsx)("span", {
										className: Card_module_css_default.locationLabel,
										children: t("location")
									}), (0, react_jsx_runtime.jsx)(LocationPicker, {
										value: location,
										disabled: busy,
										onChange: setSelected,
										t
									})]
								}),
								location === "custom" && (0, react_jsx_runtime.jsxs)("div", {
									className: Card_module_css_default.customLocation,
									children: [(0, react_jsx_runtime.jsx)("label", {
										htmlFor: customId,
										className: Card_module_css_default.locationLabel,
										children: t("customLabel")
									}), (0, react_jsx_runtime.jsx)("textarea", {
										id: customId,
										value: customLocation,
										rows: 2,
										maxLength: 1e3,
										disabled: busy,
										placeholder: t("customPlaceholder"),
										onChange: (event) => {
											setCustom(event.target.value);
										}
									})]
								}),
								(0, react_jsx_runtime.jsx)("footer", {
									className: Card_module_css_default.footer,
									children: (0, react_jsx_runtime.jsxs)("div", {
										className: Card_module_css_default.actions,
										children: [(0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
											variant: "ghost",
											size: "sm",
											disabled: busy,
											onClick: () => {
												run("dismiss");
											},
											children: t("dismiss")
										}), (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
											variant: "primary",
											size: "sm",
											disabled: busy || location === "custom" && customLocation.trim() === "",
											onClick: () => {
												run("create");
											},
											children: t(pending === "create" ? "starting" : decision === void 0 ? "create" : "retry")
										})]
									})
								})
							]
						}) : (0, react_jsx_runtime.jsxs)("p", {
							className: Card_module_css_default.description,
							children: [
								t("location"),
								" · ",
								data?.customLocation ?? t(data?.slot ?? details.data.slot)
							]
						}),
						phase === "creating" && (0, react_jsx_runtime.jsx)("footer", {
							className: Card_module_css_default.footer,
							children: (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
								variant: "ghost",
								size: "sm",
								disabled: busy,
								onClick: () => {
									run("cancel");
								},
								children: t(pending === "cancel" ? "cancelling" : "cancel")
							})
						}),
						installed && (0, react_jsx_runtime.jsxs)("footer", {
							className: Card_module_css_default.footer,
							children: [(0, react_jsx_runtime.jsx)("span", {
								className: Card_module_css_default.scope,
								children: t("scope")
							}), (0, react_jsx_runtime.jsxs)("div", {
								className: Card_module_css_default.actions,
								children: [(0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
									variant: "ghost",
									size: "sm",
									disabled: busy,
									onClick: () => {
										run("undo");
									},
									children: t("undo")
								}), phase !== "kept" && (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
									variant: "primary",
									size: "sm",
									disabled: busy,
									onClick: () => {
										run("keep");
									},
									children: t("keep")
								})]
							})]
						}),
						phase === "removed" && (0, react_jsx_runtime.jsx)("footer", {
							className: Card_module_css_default.footer,
							children: (0, react_jsx_runtime.jsx)("div", {
								className: Card_module_css_default.actions,
								children: (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Button, {
									variant: "primary",
									size: "sm",
									disabled: busy,
									onClick: () => {
										run("restore");
									},
									children: t(pending === "restore" ? "restoring" : "restore")
								})
							})
						})
					]
				})
			] });
		}
		//#endregion
		//#region lib/types/client/locales.js
		/** English creation and recovery copy. */
		const en = {
			heading: "Personalized pages",
			location: "Add to",
			creationInfo: "How creation works and token usage",
			creationMethod: "Creator mode builds a new page you can remove at any time.",
			creationCost: "Creation may take a while and use a large number of tokens.",
			creationBackground: "Click Create to start in the background. You can keep chatting here.",
			initialContent: "Starts with",
			expandContent: "View all content",
			collapseContent: "Collapse content",
			"sidebar-right": "Right sidebar",
			page: "Session tab",
			"sidebar-left": "Global page",
			custom: "Write your own",
			"sidebar-rightHint": "View alongside this conversation",
			pageHint: "Open beside Chat and Trajectory",
			"sidebar-leftHint": "Open from the left sidebar",
			customHint: "Describe where you want it",
			customLabel: "Where should it appear?",
			customPlaceholder: "For example, a left sidebar entry that opens a full-page calendar",
			create: "Create",
			starting: "Starting…",
			dismiss: "Not now",
			retry: "Create again",
			keep: "Keep",
			undo: "Undo",
			restore: "Restore",
			restoring: "Restoring…",
			scope: "You can remove it at any time",
			dismissed: "Dismissed",
			replaced: "Replaced",
			creating: "Creating in the background",
			cancel: "Cancel creation",
			cancelling: "Cancelling…",
			installing: "Installing…",
			sourceRetained: "Undo keeps the source files in the original workspace’s .dsh-created-plugins/ folder.",
			installed: "Installed",
			kept: "Kept",
			removed: "Removed",
			failed: "Creation failed. Try again.",
			interrupted: "Creation interrupted. Try again.",
			unconfirmed: "Result unconfirmed. Retry to check.",
			"invalid-location": "Choose a location or describe it in up to 1,000 characters.",
			"restart-required": "The change is saved. Restart DSH to apply it.",
			overridden: "Another configuration overrides this change.",
			cancelled: "Cancelled",
			applicationFailed: "The last change did not apply. Check Plugins.",
			applicationCancelled: "The last change was cancelled.",
			transport: "Could not confirm the result. Reconnect and check the card before retrying.",
			unknown: "This recommendation is no longer available.",
			unavailable: "This action is unavailable. Check the plugin in Plugins.",
			"access-denied": "This connection cannot change plugins.",
			"not-live-root": "Open the original conversation to use this card.",
			closing: "The creation plugin is stopping. Try again after it reloads.",
			"read-only": "This Session is read-only. Switch the permission preset to create pages.",
			"source-missing": "The page files are gone. Ask for a new proposal."
		};
		/** Chinese creation and recovery copy. */
		const zh = {
			heading: "个性化页面",
			location: "添加到",
			creationInfo: "创建方式与 token 消耗",
			creationMethod: "用创造模式制作新页面，可随时移除",
			creationCost: "创建可能耗时较长，并消耗大量 token",
			creationBackground: "点击「创建」后在后台运行，你可以继续聊天",
			initialContent: "预填内容",
			expandContent: "展开内容",
			collapseContent: "收起内容",
			"sidebar-right": "右侧栏",
			page: "会话 Tab",
			"sidebar-left": "全局页面",
			custom: "自行撰写",
			"sidebar-rightHint": "在当前会话旁查看",
			pageHint: "与「对话」「轨迹」并列",
			"sidebar-leftHint": "从左侧入口打开",
			customHint: "描述你希望放在哪里",
			customLabel: "你希望放在哪里？",
			customPlaceholder: "例如：在左侧加一个入口，打开全屏日历",
			create: "创建",
			starting: "开始创建…",
			dismiss: "暂不需要",
			retry: "重新创建",
			keep: "保留",
			undo: "撤销",
			restore: "恢复",
			restoring: "恢复中…",
			scope: "可随时移除",
			dismissed: "已忽略",
			replaced: "已替换",
			creating: "正在后台创建",
			cancel: "取消创建",
			cancelling: "正在取消…",
			installing: "正在安装…",
			sourceRetained: "撤销后，源码仍保留在原工作区的 .dsh-created-plugins/ 文件夹中",
			installed: "已安装",
			kept: "已保留",
			removed: "已移除",
			failed: "创建失败，可重试",
			interrupted: "创建中断，可重试",
			unconfirmed: "结果待确认，请重试",
			"invalid-location": "请选择位置，或填写不超过 1,000 字的位置说明",
			"restart-required": "变更已保存，重启 DSH 后生效",
			overridden: "其他配置覆盖了本次变更",
			cancelled: "已取消",
			applicationFailed: "上一步更改未生效，请到插件页查看",
			applicationCancelled: "上一步更改已取消",
			transport: "暂时无法确认结果，请重新连接并查看卡片状态后再重试",
			unknown: "这条推荐已不可用",
			unavailable: "暂时无法执行，请到插件页面查看",
			"access-denied": "当前连接没有修改插件的权限",
			"not-live-root": "请打开原会话后使用这张卡片",
			closing: "创建插件正在停止，请等待重新加载后再试",
			"read-only": "当前会话为只读模式，创建页面需要切换权限预设",
			"source-missing": "页面文件已不在，需要重新提案"
		};
		/** Japanese creation and recovery copy. */
		const ja = {
			heading: "パーソナルページ",
			location: "追加先",
			creationInfo: "作成の仕組みとトークン消費",
			creationMethod: "クリエイターモードで新しいページを作成します。いつでも削除できます。",
			creationCost: "作成には時間がかかり、大量のトークンを消費することがあります。",
			creationBackground: "「作成」をクリックするとバックグラウンドで開始します。ここで会話を続けられます。",
			initialContent: "初期内容",
			expandContent: "内容をすべて表示",
			collapseContent: "内容を折りたたむ",
			"sidebar-right": "右サイドバー",
			page: "セッションタブ",
			"sidebar-left": "グローバルページ",
			custom: "自分で指定",
			"sidebar-rightHint": "この会話の横に表示",
			pageHint: "チャットと軌跡の隣に開く",
			"sidebar-leftHint": "左サイドバーから開く",
			customHint: "配置したい場所を説明",
			customLabel: "どこに表示しますか？",
			customPlaceholder: "例：左サイドバーに全画面カレンダーを開く項目を追加",
			create: "作成",
			starting: "開始しています…",
			dismiss: "今はしない",
			retry: "再作成",
			keep: "保持",
			undo: "元に戻す",
			restore: "復元",
			restoring: "復元中…",
			scope: "いつでも削除できます",
			dismissed: "却下済み",
			replaced: "置き換え済み",
			creating: "バックグラウンドで作成中",
			cancel: "作成をキャンセル",
			cancelling: "キャンセルしています…",
			installing: "インストール中…",
			sourceRetained: "元に戻しても、ソースファイルは元のワークスペースの .dsh-created-plugins/ フォルダーに残ります。",
			installed: "インストール済み",
			kept: "保持済み",
			removed: "削除済み",
			failed: "作成に失敗しました。再試行してください。",
			interrupted: "作成が中断されました。再試行してください。",
			unconfirmed: "結果を確認できません。再試行して確認してください。",
			"invalid-location": "場所を選択するか、1,000文字以内で説明してください。",
			"restart-required": "変更は保存されました。適用するにはDSHを再起動してください。",
			overridden: "別の設定がこの変更を上書きしています。",
			cancelled: "キャンセル済み",
			applicationFailed: "前回の変更は適用されませんでした。プラグイン画面で確認してください。",
			applicationCancelled: "前回の変更はキャンセルされました。",
			transport: "結果を確認できませんでした。再接続してカードを確認してから再試行してください。",
			unknown: "この提案は利用できなくなりました。",
			unavailable: "この操作は利用できません。プラグイン画面でプラグインを確認してください。",
			"access-denied": "この接続ではプラグインを変更できません。",
			"not-live-root": "このカードを使うには元の会話を開いてください。",
			closing: "作成プラグインが停止中です。再読み込み後に再試行してください。",
			"read-only": "このセッションは読み取り専用です。ページを作成するには権限プリセットを切り替えてください。",
			"source-missing": "ページのファイルが見つかりません。新しい提案を依頼してください。"
		};
		/** Spanish creation and recovery copy. */
		const es = {
			heading: "Páginas personalizadas",
			location: "Añadir a",
			creationInfo: "Cómo funciona la creación y el uso de tokens",
			creationMethod: "El modo creador crea una página nueva que puedes eliminar en cualquier momento.",
			creationCost: "La creación puede tardar un rato y usar una gran cantidad de tokens.",
			creationBackground: "Haz clic en Crear para empezar en segundo plano. Puedes seguir conversando aquí.",
			initialContent: "Comienza con",
			expandContent: "Ver todo el contenido",
			collapseContent: "Contraer contenido",
			"sidebar-right": "Barra lateral derecha",
			page: "Pestaña de la sesión",
			"sidebar-left": "Página global",
			custom: "Describir otra ubicación",
			"sidebar-rightHint": "Ver junto a esta conversación",
			pageHint: "Abrir junto a Chat y Trayectoria",
			"sidebar-leftHint": "Abrir desde la barra lateral izquierda",
			customHint: "Describe dónde la quieres",
			customLabel: "¿Dónde debería aparecer?",
			customPlaceholder: "Por ejemplo, una entrada en la barra lateral izquierda que abra un calendario a pantalla completa",
			create: "Crear",
			starting: "Iniciando…",
			dismiss: "Ahora no",
			retry: "Crear de nuevo",
			keep: "Conservar",
			undo: "Deshacer",
			restore: "Restaurar",
			restoring: "Restaurando…",
			scope: "Puedes eliminarla en cualquier momento",
			dismissed: "Descartado",
			replaced: "Reemplazado",
			creating: "Creando en segundo plano",
			cancel: "Cancelar creación",
			cancelling: "Cancelando…",
			installing: "Instalando…",
			sourceRetained: "Deshacer conserva los archivos fuente en la carpeta .dsh-created-plugins/ del espacio de trabajo original.",
			installed: "Instalado",
			kept: "Conservado",
			removed: "Eliminado",
			failed: "La creación falló. Inténtalo de nuevo.",
			interrupted: "La creación se interrumpió. Inténtalo de nuevo.",
			unconfirmed: "Resultado sin confirmar. Reintenta para comprobarlo.",
			"invalid-location": "Elige una ubicación o descríbela en un máximo de 1000 caracteres.",
			"restart-required": "El cambio se guardó. Reinicia DSH para aplicarlo.",
			overridden: "Otra configuración anula este cambio.",
			cancelled: "Cancelado",
			applicationFailed: "El último cambio no se aplicó. Revisa Complementos.",
			applicationCancelled: "El último cambio se canceló.",
			transport: "No se pudo confirmar el resultado. Vuelve a conectarte y revisa la tarjeta antes de reintentar.",
			unknown: "Esta recomendación ya no está disponible.",
			unavailable: "Esta acción no está disponible. Revisa el complemento en Complementos.",
			"access-denied": "Esta conexión no puede modificar complementos.",
			"not-live-root": "Abre la conversación original para usar esta tarjeta.",
			closing: "El complemento de creación se está deteniendo. Inténtalo de nuevo cuando se recargue.",
			"read-only": "Esta sesión es de solo lectura. Cambia el ajuste preestablecido de permisos para crear páginas.",
			"source-missing": "Los archivos de la página ya no existen. Pide una nueva propuesta."
		};
		/** French creation and recovery copy. */
		const fr = {
			heading: "Pages personnalisées",
			location: "Ajouter à",
			creationInfo: "Fonctionnement de la création et consommation de jetons",
			creationMethod: "Le mode Créateur crée une nouvelle page que vous pouvez supprimer à tout moment.",
			creationCost: "La création peut prendre du temps et consommer un grand nombre de jetons.",
			creationBackground: "Cliquez sur Créer pour démarrer en arrière-plan. Vous pouvez continuer à discuter ici.",
			initialContent: "Commence avec",
			expandContent: "Afficher tout le contenu",
			collapseContent: "Réduire le contenu",
			"sidebar-right": "Barre latérale droite",
			page: "Onglet de la session",
			"sidebar-left": "Page globale",
			custom: "Décrire un autre emplacement",
			"sidebar-rightHint": "Afficher à côté de cette conversation",
			pageHint: "Ouvrir à côté de Discussion et Trajectoire",
			"sidebar-leftHint": "Ouvrir depuis la barre latérale gauche",
			customHint: "Décrivez où vous la voulez",
			customLabel: "Où doit-elle apparaître ?",
			customPlaceholder: "Par exemple, une entrée dans la barre latérale gauche qui ouvre un calendrier en pleine page",
			create: "Créer",
			starting: "Démarrage…",
			dismiss: "Pas maintenant",
			retry: "Créer à nouveau",
			keep: "Conserver",
			undo: "Annuler",
			restore: "Restaurer",
			restoring: "Restauration…",
			scope: "Vous pouvez la supprimer à tout moment",
			dismissed: "Ignoré",
			replaced: "Remplacé",
			creating: "Création en arrière-plan",
			cancel: "Annuler la création",
			cancelling: "Annulation…",
			installing: "Installation…",
			sourceRetained: "L’annulation conserve les fichiers source dans le dossier .dsh-created-plugins/ de l’espace de travail d’origine.",
			installed: "Installé",
			kept: "Conservé",
			removed: "Supprimé",
			failed: "La création a échoué. Réessayez.",
			interrupted: "La création a été interrompue. Réessayez.",
			unconfirmed: "Résultat non confirmé. Réessayez pour vérifier.",
			"invalid-location": "Choisissez un emplacement ou décrivez-le en 1 000 caractères maximum.",
			"restart-required": "La modification est enregistrée. Redémarrez DSH pour l’appliquer.",
			overridden: "Une autre configuration remplace cette modification.",
			cancelled: "Annulé",
			applicationFailed: "La dernière modification n’a pas été appliquée. Vérifiez Plugins.",
			applicationCancelled: "La dernière modification a été annulée.",
			transport: "Impossible de confirmer le résultat. Reconnectez-vous et vérifiez la carte avant de réessayer.",
			unknown: "Cette recommandation n’est plus disponible.",
			unavailable: "Cette action n’est pas disponible. Vérifiez le plugin dans Plugins.",
			"access-denied": "Cette connexion ne peut pas modifier les plugins.",
			"not-live-root": "Ouvrez la conversation d’origine pour utiliser cette carte.",
			closing: "Le plugin de création s’arrête. Réessayez après son rechargement.",
			"read-only": "Cette session est en lecture seule. Changez le préréglage d’autorisation pour créer des pages.",
			"source-missing": "Les fichiers de la page ont disparu. Demandez une nouvelle proposition."
		};
		/** German creation and recovery copy. */
		const de = {
			heading: "Personalisierte Seiten",
			location: "Hinzufügen zu",
			creationInfo: "So funktionieren die Erstellung und der Token-Verbrauch",
			creationMethod: "Der Erstellermodus erstellt eine neue Seite, die Sie jederzeit entfernen können.",
			creationCost: "Die Erstellung kann eine Weile dauern und sehr viele Token verbrauchen.",
			creationBackground: "Klicken Sie auf Erstellen, um im Hintergrund zu starten. Sie können hier weiter chatten.",
			initialContent: "Beginnt mit",
			expandContent: "Gesamten Inhalt anzeigen",
			collapseContent: "Inhalt einklappen",
			"sidebar-right": "Rechte Seitenleiste",
			page: "Sitzungs-Tab",
			"sidebar-left": "Globale Seite",
			custom: "Anderen Ort beschreiben",
			"sidebar-rightHint": "Neben diesem Gespräch anzeigen",
			pageHint: "Neben Chat und Ausführungsverlauf öffnen",
			"sidebar-leftHint": "Über die linke Seitenleiste öffnen",
			customHint: "Beschreiben Sie, wo sie erscheinen soll",
			customLabel: "Wo soll sie erscheinen?",
			customPlaceholder: "Zum Beispiel ein Eintrag in der linken Seitenleiste, der einen ganzseitigen Kalender öffnet",
			create: "Erstellen",
			starting: "Wird gestartet…",
			dismiss: "Nicht jetzt",
			retry: "Erneut erstellen",
			keep: "Behalten",
			undo: "Rückgängig machen",
			restore: "Wiederherstellen",
			restoring: "Wird wiederhergestellt…",
			scope: "Sie können sie jederzeit entfernen",
			dismissed: "Verworfen",
			replaced: "Ersetzt",
			creating: "Wird im Hintergrund erstellt",
			cancel: "Erstellung abbrechen",
			cancelling: "Wird abgebrochen…",
			installing: "Wird installiert…",
			sourceRetained: "Beim Rückgängigmachen bleiben die Quelldateien im Ordner .dsh-created-plugins/ des ursprünglichen Arbeitsbereichs erhalten.",
			installed: "Installiert",
			kept: "Behalten",
			removed: "Entfernt",
			failed: "Die Erstellung ist fehlgeschlagen. Versuchen Sie es erneut.",
			interrupted: "Die Erstellung wurde unterbrochen. Versuchen Sie es erneut.",
			unconfirmed: "Ergebnis nicht bestätigt. Versuchen Sie es erneut, um es zu prüfen.",
			"invalid-location": "Wählen Sie einen Ort oder beschreiben Sie ihn in höchstens 1.000 Zeichen.",
			"restart-required": "Die Änderung ist gespeichert. Starten Sie DSH neu, um sie anzuwenden.",
			overridden: "Eine andere Konfiguration überschreibt diese Änderung.",
			cancelled: "Abgebrochen",
			applicationFailed: "Die letzte Änderung wurde nicht angewendet. Prüfen Sie Plugins.",
			applicationCancelled: "Die letzte Änderung wurde abgebrochen.",
			transport: "Das Ergebnis konnte nicht bestätigt werden. Verbinden Sie sich erneut und prüfen Sie die Karte, bevor Sie es erneut versuchen.",
			unknown: "Diese Empfehlung ist nicht mehr verfügbar.",
			unavailable: "Diese Aktion ist nicht verfügbar. Prüfen Sie das Plugin unter Plugins.",
			"access-denied": "Diese Verbindung kann keine Plugins ändern.",
			"not-live-root": "Öffnen Sie das ursprüngliche Gespräch, um diese Karte zu verwenden.",
			closing: "Das Erstellungs-Plugin wird beendet. Versuchen Sie es nach dem Neuladen erneut.",
			"read-only": "Diese Sitzung ist schreibgeschützt. Wechseln Sie die Berechtigungsvoreinstellung, um Seiten zu erstellen.",
			"source-missing": "Die Seitendateien sind nicht mehr vorhanden. Fordern Sie einen neuen Vorschlag an."
		};
		/** Russian creation and recovery copy. */
		const ru = {
			heading: "Персональные страницы",
			location: "Добавить в",
			creationInfo: "Как работает создание и расход токенов",
			creationMethod: "Режим автора создает новую страницу, которую можно удалить в любой момент.",
			creationCost: "Создание может занять некоторое время и израсходовать большое количество токенов.",
			creationBackground: "Нажмите «Создать», чтобы начать в фоновом режиме. Вы можете продолжать общение здесь.",
			initialContent: "Начальное содержимое",
			expandContent: "Показать все содержимое",
			collapseContent: "Свернуть содержимое",
			"sidebar-right": "Правая боковая панель",
			page: "Вкладка сеанса",
			"sidebar-left": "Глобальная страница",
			custom: "Описать другое место",
			"sidebar-rightHint": "Просмотр рядом с этим разговором",
			pageHint: "Открыть рядом с «Чатом» и «Траекторией»",
			"sidebar-leftHint": "Открыть с левой боковой панели",
			customHint: "Опишите, где ее разместить",
			customLabel: "Где она должна появиться?",
			customPlaceholder: "Например, пункт на левой боковой панели, открывающий календарь на всю страницу",
			create: "Создать",
			starting: "Запуск…",
			dismiss: "Не сейчас",
			retry: "Создать снова",
			keep: "Оставить",
			undo: "Отменить",
			restore: "Восстановить",
			restoring: "Восстановление…",
			scope: "Ее можно удалить в любой момент",
			dismissed: "Отклонено",
			replaced: "Заменено",
			creating: "Создается в фоновом режиме",
			cancel: "Отменить создание",
			cancelling: "Отмена…",
			installing: "Установка…",
			sourceRetained: "При отмене исходные файлы сохраняются в папке .dsh-created-plugins/ исходного рабочего пространства.",
			installed: "Установлено",
			kept: "Сохранено",
			removed: "Удалено",
			failed: "Не удалось создать. Попробуйте еще раз.",
			interrupted: "Создание прервано. Попробуйте еще раз.",
			unconfirmed: "Результат не подтвержден. Повторите попытку, чтобы проверить.",
			"invalid-location": "Выберите место или опишите его не более чем в 1000 символах.",
			"restart-required": "Изменение сохранено. Перезапустите DSH, чтобы применить его.",
			overridden: "Другая конфигурация переопределяет это изменение.",
			cancelled: "Отменено",
			applicationFailed: "Последнее изменение не применено. Проверьте раздел «Плагины».",
			applicationCancelled: "Последнее изменение отменено.",
			transport: "Не удалось подтвердить результат. Переподключитесь и проверьте карточку перед повторной попыткой.",
			unknown: "Эта рекомендация больше недоступна.",
			unavailable: "Это действие недоступно. Проверьте плагин в разделе «Плагины».",
			"access-denied": "Это подключение не может изменять плагины.",
			"not-live-root": "Откройте исходный разговор, чтобы использовать эту карточку.",
			closing: "Плагин создания останавливается. Повторите попытку после его перезагрузки.",
			"read-only": "Этот сеанс доступен только для чтения. Смените настройки разрешений, чтобы создавать страницы.",
			"source-missing": "Файлы страницы удалены. Запросите новое предложение."
		};
		/** Portuguese creation and recovery copy. */
		const pt = {
			heading: "Páginas personalizadas",
			location: "Adicionar a",
			creationInfo: "Como funciona a criação e o uso de tokens",
			creationMethod: "O modo criador cria uma nova página que você pode remover a qualquer momento.",
			creationCost: "A criação pode levar algum tempo e usar uma grande quantidade de tokens.",
			creationBackground: "Clique em Criar para começar em segundo plano. Você pode continuar conversando aqui.",
			initialContent: "Começa com",
			expandContent: "Ver todo o conteúdo",
			collapseContent: "Recolher conteúdo",
			"sidebar-right": "Barra lateral direita",
			page: "Guia da sessão",
			"sidebar-left": "Página global",
			custom: "Descrever outro local",
			"sidebar-rightHint": "Ver ao lado desta conversa",
			pageHint: "Abrir ao lado de Chat e Trajetória",
			"sidebar-leftHint": "Abrir pela barra lateral esquerda",
			customHint: "Descreva onde você a quer",
			customLabel: "Onde ela deve aparecer?",
			customPlaceholder: "Por exemplo, uma entrada na barra lateral esquerda que abra um calendário em página inteira",
			create: "Criar",
			starting: "Iniciando…",
			dismiss: "Agora não",
			retry: "Criar novamente",
			keep: "Manter",
			undo: "Desfazer",
			restore: "Restaurar",
			restoring: "Restaurando…",
			scope: "Você pode removê-la a qualquer momento",
			dismissed: "Dispensado",
			replaced: "Substituído",
			creating: "Criando em segundo plano",
			cancel: "Cancelar criação",
			cancelling: "Cancelando…",
			installing: "Instalando…",
			sourceRetained: "Desfazer mantém os arquivos de origem na pasta .dsh-created-plugins/ do espaço de trabalho original.",
			installed: "Instalado",
			kept: "Mantido",
			removed: "Removido",
			failed: "A criação falhou. Tente novamente.",
			interrupted: "A criação foi interrompida. Tente novamente.",
			unconfirmed: "Resultado não confirmado. Tente novamente para verificar.",
			"invalid-location": "Escolha um local ou descreva-o em até 1.000 caracteres.",
			"restart-required": "A alteração foi salva. Reinicie o DSH para aplicá-la.",
			overridden: "Outra configuração substitui esta alteração.",
			cancelled: "Cancelado",
			applicationFailed: "A última alteração não foi aplicada. Verifique Plug-ins.",
			applicationCancelled: "A última alteração foi cancelada.",
			transport: "Não foi possível confirmar o resultado. Reconecte-se e verifique o cartão antes de tentar novamente.",
			unknown: "Esta recomendação não está mais disponível.",
			unavailable: "Esta ação não está disponível. Verifique o plug-in em Plug-ins.",
			"access-denied": "Esta conexão não pode alterar plug-ins.",
			"not-live-root": "Abra a conversa original para usar este cartão.",
			closing: "O plug-in de criação está parando. Tente novamente depois que ele for recarregado.",
			"read-only": "Esta sessão é somente leitura. Mude a predefinição de permissão para criar páginas.",
			"source-missing": "Os arquivos da página não existem mais. Peça uma nova proposta."
		};
		//#endregion
		//#region lib/types/client/info-store.js
		/** Remember the dismissed creation notice across sessions in this browser.
		* @returns a plugin-owned preference; unavailable storage only disables persistence.
		*/
		function createCreationInfoStore() {
			return (0, _deepseek_ai_dsh_client_store.createSnapshotStore)(false, { persist: { name: "dsh.creation-recommendations.info-dismissed" } });
		}
		//#endregion
		//#region lib/types/client/mount.js
		/** Register the personalized page card body and its localized copy. */
		/** Services the body registration needs; the shared card plugin provides `actionCards`. */
		const inject = ["locale", "actionCards"];
		/**
		* Register the `create_personalized_page` body for the plugin lifetime.
		* @param ctx - Client plugin context.
		*/
		function apply(ctx) {
			const infoDismissed = createCreationInfoStore();
			ctx.effect(() => ctx.locale.register("creationRecommendations", {
				en,
				zh,
				ja,
				es,
				fr,
				de,
				ru,
				pt
			}), "creation-recommendations: locale");
			ctx.effect(() => ctx.actionCards.register({
				action: "create_personalized_page",
				locale: "creationRecommendations",
				Body: (props) => (0, react.createElement)(PageCardBody, {
					...props,
					infoDismissed
				})
			}), "creation-recommendations: card body");
		}
		//#endregion
		exports.apply = apply;
		exports.inject = inject;
		return module.exports;
	}
});

//# sourceMappingURL=client.js.map