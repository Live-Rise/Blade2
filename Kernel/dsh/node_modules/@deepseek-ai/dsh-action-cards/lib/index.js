import { Service } from "@deepseek-ai/cordis";
import { z } from "zod";
import { Remote, TypertRemoteService } from "@deepseek-ai/dsh-typert-protocol";
import { HarnessError, ToolCallId, createUserMessage } from "@deepseek-ai/dsh-llm";
import { defineTool } from "@deepseek-ai/dsh-tools";
import { SessionLogOffset, SessionSeq, appendPluginRecord, pluginRecordOf } from "@deepseek-ai/dsh-session";
//#region lib/types/projection.js
/** Recover card proposals from logged `propose_action` calls and decisions from plugin records. */
/** Model-facing name of the card tool. */
const PROPOSE_ACTION = "propose_action";
/** Kind-defined JSON object of a proposal or decision. */
const fieldsSchema = z.record(z.string(), z.json());
/** Arguments of a `propose_action` call; blank titles and briefs create no card. */
const argumentsSchema = z.object({
	action: z.string().min(1),
	title: z.string().trim().min(1),
	brief: z.string().trim().min(1),
	details: fieldsSchema
});
const proposalSchema = z.object({
	callId: z.string().min(1).transform(ToolCallId),
	turn: z.number().int().nonnegative(),
	action: z.string().min(1),
	title: z.string().min(1),
	brief: z.string().min(1),
	details: fieldsSchema
}).strict();
/** Recorded decision of one card. */
const decisionSchema = z.object({
	callId: z.string().min(1).transform(ToolCallId),
	action: z.string().min(1),
	phase: z.string().min(1),
	data: fieldsSchema.optional()
}).strict();
const viewSchema = z.object({
	proposals: z.record(z.string(), proposalSchema),
	decisions: z.record(z.string(), decisionSchema)
});
/**
* Full-Session fold. A successful `propose_action` result admits the call as a card, at most one per turn;
* the first admitted call of a turn wins. A newer card of any kind replaces every unstarted earlier card.
* Events inherited by a forked Session create no proposals or decisions.
*/
const actionCardProjection = {
	key: "actionCards",
	stateVersion: 2,
	stateSchema: z.object({
		/** Fork-inherited prefix length; inherited events create no proposals or decisions. */
		inheritedEventCount: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER).transform(SessionLogOffset),
		view: viewSchema,
		/** `propose_action` calls of the open turn still waiting for their result. */
		calls: z.record(z.string(), proposalSchema.omit({ callId: true })),
		/** Turn of the latest admitted proposal; later calls of that turn create no card. */
		admittedTurn: z.number().int().nullable(),
		/** Decisions not yet reported to the model, by call id, with the seq of their record. */
		pendingStatus: z.record(z.string(), z.number()),
		/** User messages still queued for later turns, folded from inbox splices; the card tool compares it with its limit. */
		queued: z.number().int().nonnegative()
	}),
	init: (_header, inheritedEventCount) => ({
		inheritedEventCount,
		view: {
			proposals: {},
			decisions: {}
		},
		calls: {},
		admittedTurn: null,
		pendingStatus: {},
		queued: 0
	}),
	apply: (state, event) => {
		if (event.seq < state.inheritedEventCount) return state;
		switch (event.type) {
			case "turn/start": return Object.keys(state.calls).length === 0 ? state : {
				...state,
				calls: {}
			};
			case "agent/inbox/spliced":
				if (event.data.target !== "next-turn") return state;
				return {
					...state,
					queued: Math.max(0, state.queued - (event.data.removedCount ?? 0) + event.data.inserted.length)
				};
			case "user/message": {
				if (event.data.source.kind !== "action-cards") return state;
				const through = event.data.source.through;
				return {
					...state,
					pendingStatus: Object.fromEntries(Object.entries(state.pendingStatus).filter(([, seq]) => seq > through))
				};
			}
			case "tool/call": {
				if (event.data.name !== "propose_action") return state;
				let raw;
				try {
					raw = JSON.parse(event.data.arguments);
				} catch {
					return state;
				}
				const parsed = argumentsSchema.safeParse(raw);
				if (!parsed.success) return state;
				return {
					...state,
					calls: {
						...state.calls,
						[event.data.callId]: {
							turn: event.data.turn,
							...parsed.data
						}
					}
				};
			}
			case "tool/result": {
				const callId = event.data.message.toolCallId;
				const call = Object.hasOwn(state.calls, callId) ? state.calls[callId] : void 0;
				if (call === void 0) return state;
				const calls = Object.fromEntries(Object.entries(state.calls).filter(([id]) => id !== callId));
				if (event.data.message.isError === true || state.admittedTurn === call.turn) return {
					...state,
					calls
				};
				const decisions = { ...state.view.decisions };
				for (const earlier of Object.values(state.view.proposals)) if (decisions[earlier.callId] === void 0) decisions[earlier.callId] = {
					callId: earlier.callId,
					action: earlier.action,
					phase: "replaced"
				};
				return {
					...state,
					calls,
					admittedTurn: call.turn,
					view: {
						proposals: {
							...state.view.proposals,
							[callId]: {
								callId,
								...call
							}
						},
						decisions
					}
				};
			}
			default: {
				const record = pluginRecordOf(event);
				if (record?.type !== "plugin:action-cards/decision") return state;
				const decision = decisionSchema.safeParse(record.data);
				if (!decision.success) return state;
				return {
					...state,
					view: {
						...state.view,
						decisions: {
							...state.view.decisions,
							[decision.data.callId]: decision.data
						}
					},
					pendingStatus: {
						...state.pendingStatus,
						[decision.data.callId]: event.seq
					}
				};
			}
		}
	},
	wire: {
		summary: false,
		viewSchema,
		view: (state) => state.view
	}
};
//#endregion
//#region lib/types/index.js
/** Optional action cards below an answer: the `propose_action` tool, card state, user actions and status notices for registered kinds. */
var __runInitializers = function(thisArg, initializers, value) {
	var useValue = arguments.length > 2;
	for (var i = 0; i < initializers.length; i++) value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
	return useValue ? value : void 0;
};
var __esDecorate = function(ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
	function accept(f) {
		if (f !== void 0 && typeof f !== "function") throw new TypeError("Function expected");
		return f;
	}
	var kind = contextIn.kind, key = kind === "getter" ? "get" : kind === "setter" ? "set" : "value";
	var target = !descriptorIn && ctor ? contextIn["static"] ? ctor : ctor.prototype : null;
	var descriptor = descriptorIn || (target ? Object.getOwnPropertyDescriptor(target, contextIn.name) : {});
	var _, done = false;
	for (var i = decorators.length - 1; i >= 0; i--) {
		var context = {};
		for (var p in contextIn) context[p] = p === "access" ? {} : contextIn[p];
		for (var p in contextIn.access) context.access[p] = contextIn.access[p];
		context.addInitializer = function(f) {
			if (done) throw new TypeError("Cannot add initializers after decoration has completed");
			extraInitializers.push(accept(f || null));
		};
		var result = (0, decorators[i])(kind === "accessor" ? {
			get: descriptor.get,
			set: descriptor.set
		} : descriptor[key], context);
		if (kind === "accessor") {
			if (result === void 0) continue;
			if (result === null || typeof result !== "object") throw new TypeError("Object expected");
			if (_ = accept(result.get)) descriptor.get = _;
			if (_ = accept(result.set)) descriptor.set = _;
			if (_ = accept(result.init)) initializers.unshift(_);
		} else if (_ = accept(result)) if (kind === "field") initializers.unshift(_);
		else descriptor[key] = _;
	}
	if (target) Object.defineProperty(target, contextIn.name, descriptor);
	done = true;
};
/** Deployment-tunable limits for offering cards. */
const Config = z.object({ 
/** Queued next-turn user messages at or above which `propose_action` refuses, so no card is offered. */
queuedMessageLimit: z.number().int().min(1).default(2) }).prefault({});
/** The user action every card offers; the shared service records it. */
const DISMISS = "dismiss";
/** Phases the projection derives from the log; no decision may record them. */
const DERIVED_PHASES = ["replaced"];
/** Phases after which no user action changes the card. */
const FINAL_PHASES = ["dismissed", ...DERIVED_PHASES];
/** Tool whose successful loads activate `propose_action`; its `name` argument names the skill. */
const SKILL_TOOL = "skill";
const skillArguments = z.object({ name: z.string() });
const STATUS_PREAMBLE = "Latest status of action cards from user actions and background work. These are status facts, not a request to start or retry work. Use these receipts instead of the earlier proposal when describing progress.";
const STATUS_CLOSING = "Continue with the user’s current request.";
/**
* Whether a plugin must refuse new work: its fiber or an ancestor is disposed or unloading, since a parent's disposal
* begins before the plugin's own effects run, or the plugin's own drain has begun.
* @param owner - the plugin's fiber.
* @param draining - whether the plugin's drain effect has started.
* @returns whether the plugin is stopping.
*/
function isPluginStopping(owner, draining) {
	for (let fiber = owner;; fiber = fiber.parent.fiber) {
		if (fiber.uid === null || fiber.state === 5) return true;
		if (fiber.parent.fiber === fiber) return draining;
	}
}
/** A refused `propose_action` call; the message tells the model what to do instead. */
var ActionCardError = class extends HarnessError {};
/** Own entries only: browser-supplied ids such as `toString` never reach inherited object members. */
function ownEntry(record, key) {
	return Object.hasOwn(record, key) ? record[key] : void 0;
}
function refused(reason) {
	return {
		status: "refused",
		reason
	};
}
function issuesOf(error) {
	return "Invalid details: " + error.issues.map((issue) => `${["details", ...issue.path.map(String)].join(".")}: ${issue.message}`).join("; ");
}
/** Card tool, kind registry, Session projection, user actions and status notices. */
let ActionCards = (() => {
	let _classSuper = TypertRemoteService;
	let _instanceExtraInitializers = [];
	let _act_decorators;
	return class ActionCards extends _classSuper {
		static {
			const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
			_act_decorators = [Remote];
			__esDecorate(this, null, _act_decorators, {
				kind: "method",
				name: "act",
				static: false,
				private: false,
				access: {
					has: (obj) => "act" in obj,
					get: (obj) => obj.act
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			if (_metadata) Object.defineProperty(this, Symbol.metadata, {
				enumerable: true,
				configurable: true,
				writable: true,
				value: _metadata
			});
		}
		config = __runInitializers(this, _instanceExtraInitializers);
		static inject = [
			"tools",
			"agents",
			"sessionProjections",
			"remoteAccess"
		];
		static Config = Config;
		projection;
		kinds = /* @__PURE__ */ new Map();
		/**
		* Actions whose kind unregistered. Cordis disposes a plugin's effects concurrently, so a kind's unload drain can
		* still record the outcome of its in-flight work after its registration is gone.
		*/
		retired = /* @__PURE__ */ new Set();
		/** Skills of registered kinds each live agent has loaded; `propose_action` is declared while one is still registered. */
		loaded = /* @__PURE__ */ new Map();
		/** Kind skills each agent loaded in its latest turn. */
		turnLoads = /* @__PURE__ */ new Map();
		/** The call accepted in each agent's latest turn, until its result fails. */
		accepted = /* @__PURE__ */ new Map();
		actions = /* @__PURE__ */ new Map();
		closing = false;
		owner;
		/**
		* @param ctx - Owning plugin context.
		* @param config - Validated card limits; direct composition without a config gets the schema defaults.
		*/
		constructor(ctx, config = Config.parse({})) {
			super(ctx, "actionCards");
			this.config = config;
			this.owner = ctx.fiber;
		}
		async [Service.init]() {
			this.projection = await this.ctx.sessionProjections.register(actionCardProjection);
			this.ctx.tools.register(defineTool({
				name: PROPOSE_ACTION,
				description: "Show the user one optional action card under this answer. Call only as the skill that names the action instructs.",
				parameters: {
					action: {
						type: "string",
						required: true,
						description: "Action name from the skill."
					},
					title: {
						type: "string",
						required: true,
						description: "Card title."
					},
					brief: {
						type: "string",
						required: true,
						description: "What the action does, in the user's language."
					},
					details: {
						type: "object",
						additionalProperties: true,
						required: true,
						description: "Fields the skill lists for this action."
					}
				},
				output: {
					schema: {
						type: "object",
						additionalProperties: false,
						properties: {
							action: {
								type: "string",
								required: true
							},
							title: {
								type: "string",
								required: true
							},
							invitation: {
								type: "string",
								required: true
							}
						}
					},
					render: (_args, value) => [{
						type: "text",
						text: `Card shown below your answer. Do not restate it. ${value.invitation}`
					}]
				},
				execute: (args, exec) => Promise.resolve(this.propose(args, exec))
			}), { when: ({ agent }) => this.declares(agent) });
			this.ctx.on("tools/result", (exec, result) => {
				const agent = exec.agent;
				if (agent === void 0) return;
				if (exec.name === "propose_action") {
					if (result.isError && this.accepted.get(agent)?.callId === exec.callId) this.accepted.delete(agent);
					return;
				}
				if (exec.parent !== void 0 || !this.ctx.agents.roots().includes(agent)) return;
				const skill = exec.name === SKILL_TOOL && !result.isError ? skillArguments.safeParse(exec.arguments) : void 0;
				if (skill?.success !== true || ![...this.kinds.values()].some((kind) => kind.skill === skill.data.name)) return;
				const turn = this.ctx.agents.turnBoundaryOf(agent.session).lastTurn;
				const loads = this.turnLoads.get(agent);
				if (loads?.turn === turn) loads.skills.add(skill.data.name);
				else this.turnLoads.set(agent, {
					turn,
					skills: new Set([skill.data.name])
				});
				this.loaded.set(agent, new Set([...this.loaded.get(agent) ?? [], skill.data.name]));
			});
			this.ctx.on("agent/disposed", ({ agent }) => {
				this.loaded.delete(agent);
				this.turnLoads.delete(agent);
				this.accepted.delete(agent);
			});
			this.ctx.on("agent/pre-step", async ({ agent, signal }, next) => {
				const decision = await next();
				if (decision.kind === "reject" || signal.aborted || this.isStopping()) return decision;
				const state = this.projection.stateOf(agent.session);
				const pending = Object.values(state.pendingStatus);
				if (pending.length === 0) return decision;
				const receipts = Object.values(state.view.decisions).filter((receipt) => Object.hasOwn(state.pendingStatus, receipt.callId));
				const guidance = new Set(receipts.flatMap((receipt) => this.kinds.get(receipt.action)?.statusGuidance ?? []));
				const updates = receipts.map((receipt) => {
					const fields = this.kinds.get(receipt.action)?.statusFields;
					const data = receipt.data === void 0 || fields === void 0 ? receipt.data : Object.fromEntries(Object.entries(receipt.data).filter(([key]) => fields.includes(key)));
					return {
						action: receipt.action,
						callId: receipt.callId,
						title: ownEntry(state.view.proposals, receipt.callId)?.title,
						phase: receipt.phase,
						...data === void 0 ? {} : { data }
					};
				});
				return {
					...decision,
					messages: [...decision.messages, createUserMessage({
						source: {
							kind: "action-cards",
							through: SessionSeq(Math.max(...pending)),
							form: "notice",
							summary: "Action card status updated"
						},
						content: [{
							type: "text",
							text: [
								STATUS_PREAMBLE,
								...guidance,
								STATUS_CLOSING
							].join(" ") + "\n" + JSON.stringify(updates)
						}]
					})]
				};
			}, { prepend: true });
			this.ctx.effect(() => async () => {
				this.closing = true;
				await Promise.allSettled([...this.actions.values()]);
			}, "action-cards: drain user actions");
		}
		/**
		* Register one action kind for the caller's lifetime.
		* @param kind - the kind's skill, details schema, verbs and handler.
		* @returns disposer removing the kind; actions on its cards are then unavailable.
		* @throws when another kind already registered the same action.
		*/
		register(kind) {
			const typed = (proposal) => {
				const details = kind.details.safeParse(proposal.details);
				return details.success ? {
					...proposal,
					details: details.data
				} : void 0;
			};
			const reconcile = kind.reconcile?.bind(kind);
			const entry = {
				skill: kind.skill,
				invitation: kind.invitation,
				verbs: kind.verbs,
				openPhases: kind.openPhases ?? [],
				statusGuidance: kind.statusGuidance,
				statusFields: kind.statusFields,
				detailsIssues: (details) => {
					const parsed = kind.details.safeParse(details);
					return parsed.success ? void 0 : issuesOf(parsed.error);
				},
				dismissal: (proposal, payload) => {
					const card = typed(proposal);
					return card === void 0 ? void 0 : kind.dismissal?.(card, payload);
				},
				reconcile: reconcile === void 0 ? void 0 : async (agent, proposal) => {
					const card = typed(proposal);
					if (card !== void 0) await reconcile(agent, card);
				},
				act: (agent, proposal, verb, payload) => {
					const card = typed(proposal);
					return card === void 0 ? Promise.resolve(refused("unavailable")) : kind.act(agent, card, verb, payload);
				}
			};
			const dispose = this.ctx.effect(() => {
				if (this.kinds.has(kind.action)) throw new Error(`action-cards: action "${kind.action}" is already registered`);
				this.kinds.set(kind.action, entry);
				return () => {
					this.kinds.delete(kind.action);
					this.retired.add(kind.action);
				};
			}, `action-cards: kind ${kind.action}`);
			return () => {
				dispose();
			};
		}
		/**
		* Record a card decision in the Session log; the next model request reports it.
		* @param agent - agent whose Session owns the card.
		* @param decision - the card's new state.
		* @throws when no kind ever registered the decision's action with this service, or its phase is one the projection derives.
		*/
		record(agent, decision) {
			if (!this.kinds.has(decision.action) && !this.retired.has(decision.action)) throw new Error(`action-cards: cannot record a decision for unregistered action "${decision.action}"`);
			if (DERIVED_PHASES.includes(decision.phase)) throw new Error(`action-cards: phase "${decision.phase}" is derived from the log and cannot be recorded`);
			this.append(agent, decision);
		}
		/**
		* Read the cards of one Session.
		* @param session - a Session whose projections are prepared.
		* @returns proposals and decisions by call id.
		*/
		view(session) {
			return this.projection.stateOf(session).view;
		}
		/**
		* Apply a user action to a logged card. Shared checks run first, then the kind reconciles the card; `dismiss` is
		* recorded here and every other verb goes to the card's kind. `dismiss` also works on an unstarted card whose kind
		* is no longer registered. Concurrent actions on one card share one operation.
		* @param agent - live root resolved from the displayed Session.
		* @param callId - the card's `propose_action` call id.
		* @param verb - `dismiss` or one of the kind's verbs.
		* @param payload - kind-defined fields such as a chosen location.
		* @returns acceptance of background work, the decision in effect, or a refusal.
		*/
		act(agent, callId, verb, payload) {
			if (this.isStopping()) return Promise.resolve(refused("closing"));
			if (!this.ctx.agents.roots().includes(agent)) return Promise.resolve(refused("not-live-root"));
			const proposal = ownEntry(this.projection.stateOf(agent.session).view.proposals, callId);
			if (proposal === void 0) return Promise.resolve(refused("unknown"));
			const kind = this.kinds.get(proposal.action);
			if (verb === DISMISS) return this.coalesce(agent, callId, () => this.dismiss(agent, kind, proposal, payload));
			const policy = kind === void 0 ? void 0 : ownEntry(kind.verbs, verb);
			if (kind === void 0 || policy === void 0) return Promise.resolve(refused("unavailable"));
			if (policy.global === true && !this.ctx.remoteAccess.allows({ kind: "global" })) return Promise.resolve(refused("access-denied"));
			return this.coalesce(agent, callId, () => this.perform(agent, kind, proposal, verb, policy, payload));
		}
		/** Share one operation per card, run under the Agent's initiator, until it settles. */
		coalesce(agent, callId, run) {
			const key = JSON.stringify([agent.id, callId]);
			const existing = this.actions.get(key);
			if (existing !== void 0) return existing;
			const operation = this.ctx.agents.withInitiator(agent, run).finally(() => {
				this.actions.delete(key);
			});
			this.actions.set(key, operation);
			return operation;
		}
		/**
		* Await the kind's reconciliation. A kind that unloads meanwhile still handles the action and refuses it itself.
		* @returns a refusal when reconciliation failed or the root closed meanwhile; undefined to continue.
		*/
		async reconciled(agent, kind, proposal, verb) {
			if (kind.reconcile === void 0) return void 0;
			try {
				await kind.reconcile(agent, proposal);
			} catch (error) {
				this.ctx.logger.warn("Cannot reconcile action card %s before %s: %s", proposal.callId, verb, String(error));
				return refused("unavailable");
			}
			return this.ctx.agents.roots().includes(agent) ? void 0 : refused("not-live-root");
		}
		async dismiss(agent, kind, proposal, payload) {
			const refusal = kind === void 0 ? void 0 : await this.reconciled(agent, kind, proposal, DISMISS);
			if (refusal !== void 0) return refusal;
			const decision = ownEntry(this.projection.stateOf(agent.session).view.decisions, proposal.callId);
			if (decision !== void 0 && (FINAL_PHASES.includes(decision.phase) || kind?.openPhases.includes(decision.phase) !== true)) return {
				status: "settled",
				decision
			};
			const data = kind?.dismissal(proposal, payload);
			const dismissed = {
				callId: proposal.callId,
				action: proposal.action,
				phase: "dismissed",
				...data === void 0 ? {} : { data }
			};
			this.append(agent, dismissed);
			return {
				status: "settled",
				decision: dismissed
			};
		}
		async perform(agent, kind, proposal, verb, policy, payload) {
			const refusal = await this.reconciled(agent, kind, proposal, verb);
			if (refusal !== void 0) return refusal;
			const decision = ownEntry(this.projection.stateOf(agent.session).view.decisions, proposal.callId);
			if (decision !== void 0 && FINAL_PHASES.includes(decision.phase)) return {
				status: "settled",
				decision
			};
			if (policy.writes === true && this.ctx.get("sandboxPolicy")?.resolve({ session: agent.session }).mode === "read-only") return refused("read-only");
			return await kind.act(agent, proposal, verb, payload);
		}
		append(agent, decision) {
			appendPluginRecord(agent.session, "plugin:action-cards/decision", decision);
		}
		/** Whether `propose_action` is declared for this agent's next step. */
		declares(agent) {
			const skills = this.loaded.get(agent);
			return skills !== void 0 && [...this.kinds.values()].some((kind) => skills.has(kind.skill));
		}
		propose(args, exec) {
			const agent = exec.agent;
			if (agent === void 0 || exec.parent !== void 0) throw new ActionCardError("Call propose_action directly, not from code.", "ACTION_CARD_NESTED");
			if (!this.ctx.agents.roots().includes(agent)) throw new ActionCardError("Cards are offered in the main conversation only.", "ACTION_CARD_ROOT");
			if (this.projection.stateOf(agent.session).queued >= this.config.queuedMessageLimit) throw new ActionCardError("The user has queued more messages; show no card and answer briefly.", "ACTION_CARD_QUEUED");
			const kind = this.kinds.get(args.action);
			if (kind === void 0) throw new ActionCardError(`Unknown action "${args.action}". Registered actions: ${[...this.kinds.keys()].join(", ") || "none"}.`, "ACTION_CARD_UNKNOWN");
			const turn = this.ctx.agents.turnBoundaryOf(agent.session).lastTurn;
			const loads = this.turnLoads.get(agent);
			if (loads?.turn !== turn || !loads.skills.has(kind.skill)) throw new ActionCardError(`Load skill ${kind.skill} first, then call propose_action.`, "ACTION_CARD_SKILL");
			if (args.title.trim() === "" || args.brief.trim() === "") throw new ActionCardError("Write a title and a brief.", "ACTION_CARD_BLANK");
			const issues = kind.detailsIssues(args.details);
			if (issues !== void 0) throw new ActionCardError(issues, "ACTION_CARD_DETAILS");
			if (this.accepted.get(agent)?.turn === turn || Object.values(this.view(agent.session).proposals).some((card) => card.turn === turn)) throw new ActionCardError("One card per turn; the earlier card stands.", "ACTION_CARD_LIMIT");
			this.accepted.set(agent, {
				turn,
				callId: exec.callId
			});
			return {
				action: args.action,
				title: args.title.trim(),
				invitation: kind.invitation
			};
		}
		isStopping() {
			return isPluginStopping(this.owner, this.closing);
		}
	};
})();
//#endregion
export { ActionCardError, ActionCards, ActionCards as default, Config, isPluginStopping };
