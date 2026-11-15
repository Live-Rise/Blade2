import { hostname } from "node:os";
import { isAbsolute, resolve } from "node:path";
import { Service } from "@deepseek-ai/cordis";
import z from "@deepseek-ai/schemastery";
import { AssistantStreamAccumulator, ReasoningEffortId, createUserMessage, errorChain, freezeMessage } from "@deepseek-ai/dsh-llm";
import { canOpenNativePath, nativeFileApplications, nativeFileManager, openNativeAssociatedPath, openNativeFileApplication, revealNativePath } from "@deepseek-ai/dsh-native-command";
import { Remote, RemoteError, TypertRemoteService, principalOf, remoteErrorOf } from "@deepseek-ai/dsh-typert-protocol";
import { canonicalClientTimeZone } from "@deepseek-ai/dsh-util-time";
import { assertNever } from "@deepseek-ai/dsh-util-values";
import { randomUUID } from "node:crypto";
import { mkdir } from "node:fs/promises";
import { installModelSelection } from "@deepseek-ai/dsh-agent";
import { brandNumber, brandString } from "@deepseek-ai/dsh-brand";
import { SessionQueryError } from "@deepseek-ai/dsh-session-query";
import { z as z$1 } from "zod";
import { credentialRef } from "@deepseek-ai/dsh-credentials";
import { AttachmentError } from "@deepseek-ai/dsh-attachment";
import { buildForkSeed } from "@deepseek-ai/dsh-session/fork";
import { SessionLogOffset, SessionSeq, isAppendSurfaceEvent } from "@deepseek-ai/dsh-session";
import { SessionTitleInvalidError } from "@deepseek-ai/dsh-session-title";
import { Deque } from "@deepseek-ai/dsh-deque";
import { MAX_TIMER_DELAY_MS, deadline, timeoutOf } from "@deepseek-ai/dsh-timeout";
import { performance } from "node:perf_hooks";
import { scheduler } from "node:timers/promises";
import { isUserInvocable } from "@deepseek-ai/dsh-skill";
import { FsError } from "@deepseek-ai/dsh-fs";
import mime from "mime-types";
//#region lib/types/agent.js
/** Agent activation, composition, and initial model route owned by API Session. */
var __addDisposableResource$6 = function(env, value, async) {
	if (value !== null && value !== void 0) {
		if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
		var dispose, inner;
		if (async) {
			if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
			dispose = value[Symbol.asyncDispose];
		}
		if (dispose === void 0) {
			if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
			dispose = value[Symbol.dispose];
			if (async) inner = dispose;
		}
		if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources$6 = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
/** Cold Session identity absent from persistence. */
var ApiSessionNotFound = class extends Error {};
/** Session identity whose lifecycle belongs to subagent routing. */
var ApiSessionSubagentOwnership = class extends Error {
	sessionId;
	/** @param sessionId - identity reserved to subagent routing. */
	constructor(sessionId) {
		super(`session "${sessionId}" is a subagent session; use subagent delivery`);
		this.sessionId = sessionId;
	}
};
/** Explicit-id creation attempted to adopt a Session under another cwd. */
var ApiSessionCwdConflict = class extends Error {
	sessionId;
	requestedCwd;
	existingCwd;
	constructor(sessionId, requestedCwd, existingCwd) {
		super(existingCwd === void 0 ? `session "${sessionId}" records no cwd and cannot be adopted for "${requestedCwd}"` : `session "${sessionId}" belongs to "${existingCwd}", not "${requestedCwd}"`);
		this.sessionId = sessionId;
		this.requestedCwd = requestedCwd;
		this.existingCwd = existingCwd;
	}
};
/** Reuse of a Session named an Agent preset while the deployment composes no preset roster. */
var ApiSessionPresetConflict = class extends Error {
	sessionId;
	requestedPreset;
	constructor(sessionId, requestedPreset) {
		super(`the deployment composes no agent preset roster, so session "${sessionId}" cannot run preset "${requestedPreset}"`);
		this.sessionId = sessionId;
		this.requestedPreset = requestedPreset;
	}
};
/** A blank Session chosen for reuse started a turn before this Host acquired its writer. */
var ApiSessionStartedSinceObserved = class extends Error {};
/**
* Test whether generic Session routing must leave an identity to subagent routing.
* @param session - Attached or live Session whose origin is tested.
* @returns whether subagent routing owns the Session identity.
*/
function hasApiSessionSubagentOwner(session) {
	return session.header.origin === "subagent";
}
/**
* Build the stable caller-facing subagent ownership rejection.
* @param sessionId - Session identity owned by subagent routing.
* @returns a stable Session-domain failure.
*/
function apiSessionSubagentOwnershipError(sessionId) {
	return new RemoteError("session/agent-busy", `session "${sessionId}" is owned by subagent routing`, { reason: "use subagent delivery for this child session" });
}
/** Owns every operation that may create, resume, or configure a Web Agent. */
var ApiSessionAgentController = class {
	ctx;
	resumes = /* @__PURE__ */ new Map();
	claims = /* @__PURE__ */ new Map();
	imageAdmissionChains = /* @__PURE__ */ new WeakMap();
	/** @param ctx - Host context carrying Agent, model, persistence, and Typert services. */
	constructor(ctx) {
		this.ctx = ctx;
	}
	/**
	* Configure Session and Agent lookups for Typert.
	*/
	initialize() {
		const ctx = this.ctx;
		ctx.typert.lookups.configure("agent", async (sessionId) => {
			const found = await this.resolveAgent(sessionId);
			if ("error" in found) throw found.error;
			return found.agent;
		});
		ctx.typert.lookups.configure("session", async (sessionId) => {
			const found = await this.resolveAgent(sessionId);
			if ("error" in found) throw found.error;
			return found.agent.session;
		});
		ctx.typert.contexts.configureHost("agent", async (sessionId) => {
			const found = await this.resolveAgent(sessionId);
			if ("error" in found) throw found.error;
			return found.agent.ctx;
		});
	}
	/**
	* Resolve or resume one ordinary Session, deduplicating concurrent resumes.
	* @param sessionId - ordinary Session identity.
	* @returns the live Agent or a stable Session-domain failure.
	*/
	async resolveAgent(sessionId) {
		return this.resolve(sessionId);
	}
	/**
	* Resolve one ordinary Session from an already-retained exact observation.
	* @param observation - Host-owned observation whose preparation stays pinned through setup.
	* @returns the live Agent or a stable Session-domain failure.
	*/
	async resolveObservedAgent(observation) {
		return this.resolve(observation.header.id, observation);
	}
	async resolve(sessionId, observation) {
		const live = this.liveAgent(sessionId);
		if (live !== void 0) return live;
		const attached = this.ctx.sessions.get(sessionId);
		if (attached !== void 0 && hasApiSessionSubagentOwner(attached)) return { error: apiSessionSubagentOwnershipError(sessionId) };
		let resume = this.resumes.get(sessionId);
		if (resume === void 0) {
			resume = this.resume(sessionId, observation).finally(() => {
				this.resumes.delete(sessionId);
			});
			this.resumes.set(sessionId, resume);
		}
		try {
			const agent = await resume;
			return this.liveAgent(sessionId) ?? { agent };
		} catch (error) {
			if (error instanceof ApiSessionNotFound) return { error: new RemoteError("session/not-found", error.message, { sessionId }) };
			if (error instanceof ApiSessionSubagentOwnership) return { error: apiSessionSubagentOwnershipError(error.sessionId) };
			const raced = this.liveAgent(sessionId);
			if (raced !== void 0) return raced;
			const racedSession = this.ctx.sessions.get(sessionId);
			if (racedSession !== void 0 && hasApiSessionSubagentOwner(racedSession)) return { error: apiSessionSubagentOwnershipError(sessionId) };
			if (error instanceof Error && error.name === "SessionAlreadyOwnedError") return { error: new RemoteError("session/writer-held", error.message, { sessionId }) };
			return { error: new RemoteError("gateway/internal", `resume failed for session "${sessionId}": ${String(error)}`, {}) };
		}
	}
	/**
	* Create a Session for a new conversation, reusing the named blank Session
	* while it is still reusable. A reused Session runs the requested preset, or
	* the configured default when the request names none; the preset it recorded
	* is not resolved, and a change is logged as `agent-preset/selected` before
	* the Agent is published. A named Session that has started a turn, fails the
	* caller's placement rule, or is written by another owner stays unchanged,
	* and a new Session is created instead. Requests naming one identity run in
	* arrival order, including their preset resolution, and check the named
	* Session's directory and ownership before resolving the preset.
	* @param request - optional Session to reuse, directory, preset, and placement rule.
	* @returns the reused or created Session and the preset it ran when this request finished.
	* @throws ApiSessionCwdConflict when the named Session belongs to another directory.
	* @throws ApiSessionSubagentOwnership when subagent routing owns the named Session.
	* @throws RemoteError `agent-preset/not-found` or `agent-preset/invalid` for the requested or default preset.
	* @throws ApiSessionPresetConflict when no roster is composed and the request names a preset for a reused Session.
	*/
	async createSession(request) {
		const named = request.sessionId;
		if (named === void 0) return this.created(await this.createMinted(request.cwd, await this.composeAgent(request.agentPreset)));
		return this.inOrder(named, async () => this.created(await this.claim(named, request)));
	}
	/**
	* Read the current Agent preset from the Session projection.
	* @param session - live Session whose projection state is available.
	* @returns the current preset, or undefined without a preset service or recorded selection.
	*/
	presetForSession(session) {
		return this.ctx.get("agentPresets")?.presetForSession(session) ?? void 0;
	}
	/**
	* Serialize image admission and model selection for one Agent.
	* @param agent - live Agent that owns the serialization chain.
	* @param operation - asynchronous operation admitted after prior work settles.
	* @returns the operation result or rejection.
	*/
	serializeImageAdmission(agent, operation) {
		const result = (this.imageAdmissionChains.get(agent) ?? Promise.resolve()).then(operation);
		this.imageAdmissionChains.set(agent, result.then(() => void 0, () => void 0));
		return result;
	}
	/**
	* Resolve the preset id and pre-publication Agent setup for a create or resume.
	* @param presetId - requested preset or the configured default when omitted.
	* @returns the resolved preset identity and Agent setup callback.
	*/
	async composeAgent(presetId) {
		const presets = this.ctx.get("agentPresets");
		if (presets === void 0) return { setup: (agentCtx) => {
			installModelSelection(agentCtx);
		} };
		const resolvedId = (await presets.resolve(presetId)).id;
		return {
			agentPreset: resolvedId,
			setup: async (agentCtx) => {
				installModelSelection(agentCtx);
				await presets.mount(agentCtx, resolvedId);
			}
		};
	}
	liveAgent(sessionId) {
		const agent = this.ctx.agents.get(sessionId);
		if (agent === void 0) return void 0;
		return hasApiSessionSubagentOwner(agent.session) ? { error: apiSessionSubagentOwnershipError(sessionId) } : { agent };
	}
	async resume(sessionId, supplied) {
		if (supplied !== void 0) return this.resumeObserved(sessionId, supplied);
		try {
			const env_1 = {
				stack: [],
				error: void 0,
				hasError: false
			};
			try {
				const observation = __addDisposableResource$6(env_1, await this.ctx.sessionQuery.observeSession(sessionId), false);
				return await this.resumeObserved(sessionId, observation);
			} catch (e_1) {
				env_1.error = e_1;
				env_1.hasError = true;
			} finally {
				__disposeResources$6(env_1);
			}
		} catch (error) {
			if (error instanceof SessionQueryError && error.code === "SESSION_QUERY_SESSION_NOT_FOUND") throw new ApiSessionNotFound(`session "${sessionId}" not found`);
			throw error;
		}
	}
	async resumeObserved(sessionId, observation) {
		if (observation.header.id !== sessionId || observation.header.cwd === void 0) throw new ApiSessionNotFound(`session "${sessionId}" not found`);
		if (hasApiSessionSubagentOwner({ header: observation.header })) throw new ApiSessionSubagentOwnership(sessionId);
		const composition = await this.composeAgent(this.presetForObservation(observation));
		const published = this.ctx.sessions.get(sessionId);
		if (published !== void 0 && hasApiSessionSubagentOwner(published)) throw new ApiSessionSubagentOwnership(sessionId);
		return (await this.ctx.agents.resume({
			resumeSessionId: sessionId,
			agentOptions: this.agentOptions(),
			setup: composition.setup
		})).agent;
	}
	async inOrder(sessionId, operation) {
		const result = (this.claims.get(sessionId) ?? Promise.resolve()).then(operation);
		const settled = result.then(() => void 0, () => void 0);
		this.claims.set(sessionId, settled);
		try {
			return await result;
		} finally {
			if (this.claims.get(sessionId) === settled) this.claims.delete(sessionId);
		}
	}
	/** @returns the reused Session, a new Session under `sessionId` when none is stored, or a new Session under a minted identity. */
	async claim(sessionId, request) {
		const attached = this.ctx.sessions.get(sessionId);
		const live = this.ctx.agents.get(sessionId);
		if (attached !== void 0 && hasApiSessionSubagentOwner(attached)) throw new ApiSessionSubagentOwnership(sessionId);
		if (live !== void 0) {
			if (live.session.header.cwd !== request.cwd) throw new ApiSessionCwdConflict(live.id, request.cwd, live.session.header.cwd);
			const composition = await this.composeAgent(request.agentPreset);
			return await this.reuseLive(live, request, composition) ?? this.createMinted(request.cwd, composition);
		}
		let observation;
		try {
			observation = await this.ctx.sessionQuery.observeSession(sessionId);
		} catch (error) {
			if (!(error instanceof SessionQueryError) || error.code !== "SESSION_QUERY_SESSION_NOT_FOUND") throw error;
			return this.createAgent(sessionId, request.cwd, await this.composeAgent(request.agentPreset));
		}
		let reusable;
		{
			const env_2 = {
				stack: [],
				error: void 0,
				hasError: false
			};
			try {
				const observed = __addDisposableResource$6(env_2, observation, false);
				if (hasApiSessionSubagentOwner({ header: observed.header })) throw new ApiSessionSubagentOwnership(sessionId);
				if (observed.header.cwd !== request.cwd) throw new ApiSessionCwdConflict(sessionId, request.cwd, observed.header.cwd);
				reusable = request.reusable(sessionId) && !observed.events.some((event) => event.type === "turn/start");
			} catch (e_2) {
				env_2.error = e_2;
				env_2.hasError = true;
			} finally {
				__disposeResources$6(env_2);
			}
		}
		const composition = await this.composeAgent(request.agentPreset);
		if (!reusable) return this.createMinted(request.cwd, composition);
		this.assertPresetApplicable(sessionId, request.agentPreset);
		try {
			return (await this.ctx.agents.resume({
				resumeSessionId: sessionId,
				agentOptions: this.agentOptions(),
				setup: this.reuseSetup(composition)
			})).agent;
		} catch (error) {
			if (error instanceof ApiSessionStartedSinceObserved) return this.createMinted(request.cwd, composition);
			if (!(error instanceof Error) || error.name !== "SessionAlreadyOwnedError") throw error;
			return this.ctx.agents.get(sessionId) === void 0 ? this.createMinted(request.cwd, composition) : this.claim(sessionId, request);
		}
	}
	/** @returns the live blank Session under the requested preset, or undefined when it cannot be reused. */
	async reuseLive(live, request, composition) {
		if (!request.reusable(live.id) || !this.isBlank(live.session)) return void 0;
		this.assertPresetApplicable(live.id, request.agentPreset);
		const current = this.presetForSession(live.session);
		const presets = this.ctx.get("agentPresets");
		const target = composition.agentPreset;
		if (presets === void 0 || target === void 0 || target === current) return live;
		try {
			await presets.select(live, target);
		} catch (error) {
			if (remoteErrorOf(error)?.code !== "agent-preset/locked") throw error;
			return;
		}
		return live;
	}
	/** Setup that re-checks blankness under the acquired writer, then records a preset change before publication. */
	reuseSetup(composition) {
		return async (agentCtx, agent) => {
			if (!this.isBlank(agent.session)) throw new ApiSessionStartedSinceObserved();
			const commit = await composition.setup(agentCtx, agent);
			const target = composition.agentPreset;
			if (target !== void 0 && this.presetForSession(agent.session) !== target) agent.session.append("agent-preset/selected", { agentPreset: target });
			return commit;
		};
	}
	/** Whether no turn has started, by the predicate the preset registry's `select` enforces. */
	isBlank(session) {
		const boundary = this.ctx.agents.turnBoundaryOf(session);
		return boundary.openTurnStartSeq === null && boundary.lastTurn === 0;
	}
	/** Without a roster, reuse cannot apply a named preset. */
	assertPresetApplicable(sessionId, requested) {
		if (requested === void 0 || this.ctx.get("agentPresets") !== void 0) return;
		throw new ApiSessionPresetConflict(sessionId, requested);
	}
	created(agent) {
		return {
			agent,
			agentPreset: this.presetForSession(agent.session)
		};
	}
	createMinted(cwd, composition) {
		return this.createAgent(brandString(`session-${randomUUID()}`), cwd, composition);
	}
	async createAgent(sessionId, cwd, composition) {
		try {
			await mkdir(cwd, { recursive: true });
		} catch (error) {
			throw new Error(`failed to ensure project directory "${cwd}": ${String(error)}`, { cause: error });
		}
		return (await this.ctx.agents.create({
			sessionId,
			agentOptions: this.agentOptions(),
			meta: {
				cwd,
				...composition.agentPreset === void 0 ? {} : { agentPreset: composition.agentPreset }
			},
			setup: composition.setup
		})).agent;
	}
	agentOptions() {
		const defaults = this.ctx.agentDefaultModel;
		return {
			get provider() {
				return defaults.currentSelection().provider;
			},
			get model() {
				return defaults.currentSelection().model;
			},
			get reasoningEffort() {
				return defaults.currentSelection().reasoningEffort;
			}
		};
	}
	/**
	* Read the current Agent preset from an all-projections observation.
	* @param observation - exact Session observation carrying its projection snapshot.
	* @returns the current preset, or undefined when the capability is absent.
	*/
	presetForObservation(observation) {
		if (observation.projections === void 0) throw new Error("api-session: Agent activation requires a projected Session observation");
		return observation.projections.values.agentPreset ?? void 0;
	}
};
//#endregion
//#region lib/types/continuation.js
const CONTINUABLE_FAILURES = new Set([
	"RATE_LIMIT",
	"SERVER",
	"TIMEOUT",
	"TRANSPORT",
	"EMPTY_RESPONSE",
	"STREAM_CLOSED"
]);
const endingSchema = z$1.number().int().nonnegative().transform(brandNumber).nullable();
const stateSchema = z$1.object({
	admitted: z$1.boolean(),
	failedAttempt: z$1.boolean(),
	ending: endingSchema
});
const EMPTY = {
	admitted: false,
	failedAttempt: false,
	ending: null
};
/** Eligibility requires admitted human work and a failed settlement in the last request step. */
const continuationProjectionDefinition = {
	key: "conversationContinuation",
	stateVersion: 4,
	stateSchema,
	init: () => EMPTY,
	apply: (state, event) => {
		switch (event.type) {
			case "turn/start": return EMPTY;
			case "step/start":
			case "assistant/message": return state.failedAttempt ? {
				...state,
				failedAttempt: false
			} : state;
			case "user/message": return !state.admitted && event.surfaceOp === "append" && (event.data.source.kind === "user" && event.data.content.length > 0 || event.data.source.kind === "retry" && event.data.content.length === 0) ? {
				...state,
				admitted: true
			} : state;
			case "assistant/attempt": return state.failedAttempt ? state : {
				...state,
				failedAttempt: true
			};
			case "turn/end": {
				const ending = event.data.reason.kind === "error" && CONTINUABLE_FAILURES.has(event.data.reason.error.code) && state.admitted && state.failedAttempt ? event.seq : null;
				return ending === null ? EMPTY : {
					...EMPTY,
					ending
				};
			}
			default: return state;
		}
	},
	wire: {
		viewSchema: endingSchema,
		view: (state) => state.ending
	}
};
//#endregion
//#region lib/types/catalog.js
/** Shared projection of the live LLM registry into the browser model catalog. */
/**
* Build the browser model catalog without requiring a Session.
* @param ctx - Host context carrying the live LLM registry.
* @param defaultSelection - deployment default used before a Session selects a model.
* @returns successful non-empty provider groups and isolated provider failures.
*/
async function buildModelCatalog(ctx, defaultSelection = ctx.agentDefaultModel.currentSelection()) {
	const providers = ctx.llm.listProviders();
	const catalog = await Promise.all(providers.map(async (provider) => {
		try {
			const models = await ctx.llm.listModels(provider.id);
			const entries = await Promise.all(models.map(async (model) => {
				const resolved = await ctx.llm.resolveModelInfo(provider.id, model.id);
				const reasoning = resolved.reasoning === void 0 ? void 0 : {
					efforts: resolved.reasoning.efforts.map((effort) => ({
						id: effort.id,
						name: effort.name,
						...effort.description === void 0 ? {} : { description: effort.description }
					})),
					...resolved.reasoning.defaultEffort === void 0 ? {} : { defaultEffort: resolved.reasoning.defaultEffort }
				};
				return {
					id: model.id,
					name: model.name,
					...model.description === void 0 ? {} : { description: model.description },
					...reasoning === void 0 ? {} : { reasoning },
					...model.badge === void 0 ? {} : { badge: model.badge }
				};
			}));
			return {
				kind: "group",
				group: {
					id: provider.id,
					name: provider.name,
					models: entries
				}
			};
		} catch (error) {
			return {
				kind: "failure",
				failure: {
					id: provider.id,
					name: provider.name,
					message: error instanceof Error ? error.message : String(error)
				}
			};
		}
	}));
	const groups = catalog.flatMap((item) => item.kind === "group" ? [item.group] : []).filter((group) => group.models.length > 0);
	return {
		default: { ...defaultSelection },
		routableProviders: groups.map((group) => group.id),
		groups,
		failures: catalog.flatMap((item) => item.kind === "failure" ? [item.failure] : [])
	};
}
/**
* Check a GUI selection against the current available provider catalog.
* @param ctx - Host LLM registry.
* @param selection - stored or explicitly requested selection.
* @returns whether the exact model is currently advertised as available.
*/
async function modelAvailable(ctx, selection) {
	if (!ctx.llm.listProviders().some((provider) => provider.id === selection.provider)) return false;
	let models;
	try {
		models = await ctx.llm.listModels(selection.provider);
	} catch (error) {
		throw new RemoteError("session/model-unavailable", error instanceof Error ? error.message : String(error), {
			provider: selection.provider,
			model: selection.model
		});
	}
	return models.some((model) => model.id === selection.model);
}
/**
* Check configured provider API-key references independently of model availability.
* @param ctx - Host registry, settings, and credential services.
* @returns whether any API-key provider has a configured credential.
*/
async function hasProviderApiKey(ctx) {
	const settings = ctx.get("settings");
	const credentials = ctx.get("credentials");
	if (settings === void 0 || credentials === void 0) throw new RemoteError("session/provider-credentials-unavailable", "provider credentials are unavailable", {});
	const namespaces = settings.describe({ redactSecrets: true });
	for (const provider of ctx.llm.listConfigurableProviders()) {
		if (provider.provider === "deepseek-account") continue;
		let profile = namespaces.find((namespace) => namespace.ns === provider.settingsNs)?.value;
		for (const key of provider.settingsPath) profile = typeof profile === "object" && profile !== null ? Reflect.get(profile, key) : void 0;
		if (typeof profile !== "object" || profile === null) continue;
		const ref = Reflect.get(profile, "apiKeyEnv");
		if (typeof ref === "string" && ref.length > 0 && (await credentials.describe(credentialRef(ref))).configured) return true;
	}
	return false;
}
//#endregion
//#region lib/types/commands.js
/** Session commands whose activation policy is explicit at each Remote method. */
var __addDisposableResource$5 = function(env, value, async) {
	if (value !== null && value !== void 0) {
		if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
		var dispose, inner;
		if (async) {
			if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
			dispose = value[Symbol.asyncDispose];
		}
		if (dispose === void 0) {
			if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
			dispose = value[Symbol.dispose];
			if (async) inner = dispose;
		}
		if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources$5 = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
/**
* Whether prompt content carries non-whitespace text or an attachment.
* @param content - submitted or edited prompt parts.
* @returns whether any part supplies ordinary prompt content.
*/
function hasPromptContent(content) {
	return content.some((part) => part.type !== "text" || part.text.trim().length > 0);
}
/**
* Resolve the omitted-`atSeq` default to the latest completed-turn prefix,
* including standalone events before the next turn begins.
*/
function latestCompletedPrefixBoundary(events) {
	const lastTurnEnd = events.findLast((event) => event.type === "turn/end");
	if (lastTurnEnd === void 0) return void 0;
	let boundary = lastTurnEnd.seq;
	for (const next of events.slice(boundary + 1)) {
		if (next.type === "turn/start" || next.type === "user/message" && next.surfaceOp === "append" || next.type === "agent/inbox/spliced") break;
		boundary = next.seq;
	}
	return boundary;
}
/** Implements Session business commands delegated by the Session Controller Remote service. */
var SessionCommandController = class {
	ctx;
	agents;
	defaultCwd;
	/** First-insertion facts cached per exact Session; the log remains the recovery source. */
	promptReceipts = /* @__PURE__ */ new WeakMap();
	continuation;
	/** Register complete-history empty-prompt eligibility. @returns when command admission is ready. */
	async initialize() {
		this.continuation = await this.ctx.sessionProjections.register(continuationProjectionDefinition);
	}
	/**
	* @param ctx - Host context carrying Agent, model, attachment, title, and Workspace services.
	* @param agents - sole owner of create, resume, and Agent composition.
	* @param defaultCwd - project directory used when create names neither a Workspace nor a cwd.
	*/
	constructor(ctx, agents, defaultCwd) {
		this.ctx = ctx;
		this.agents = agents;
		this.defaultCwd = defaultCwd;
	}
	/**
	* Create one ordinary Session for a new conversation, reusing the named
	* blank Session while it is an unarchived member of the requested Workspace.
	* @param request - optional blank Session to reuse, location, and Agent preset.
	* @returns the created or reused Session identity and its preset when configured.
	*/
	async create(request) {
		if (request.workspaceId !== void 0 && request.cwd !== void 0) throw new RemoteError("gateway/bad-request", "session.create accepts workspaceId or cwd, not both", {});
		let workspace;
		if (request.workspaceId !== void 0) {
			workspace = this.ctx.workspaceRegistry.get(request.workspaceId);
			if (workspace === void 0) throw new RemoteError("workspace/not-found", `workspace "${request.workspaceId}" not found`, { workspaceId: request.workspaceId });
		}
		const cwd = workspace?.path ?? request.cwd ?? this.defaultCwd;
		let created;
		try {
			created = await this.agents.createSession({
				...request.sessionId === void 0 ? {} : { sessionId: request.sessionId },
				cwd,
				...request.agentPreset === void 0 ? {} : { agentPreset: request.agentPreset },
				reusable: (sessionId) => !this.ctx.workspaceRegistry.archivedSessionIds.includes(sessionId) && (workspace === void 0 || workspace.sessionIds.includes(sessionId))
			});
		} catch (error) {
			this.rejectCreation(error);
		}
		const sessionId = created.agent.id;
		if (workspace !== void 0) try {
			await workspace.attachSession(sessionId);
		} catch (error) {
			throw new RemoteError("session/workspace-attach-failed", `session "${sessionId}" was created but could not attach to workspace "${workspace.id}": ${String(error)}`, {
				sessionId,
				workspaceId: workspace.id
			});
		}
		return {
			sessionId,
			...created.agentPreset === void 0 ? {} : { agentPreset: created.agentPreset }
		};
	}
	/**
	* Validate and record one Session model selection; save the default in the background.
	* @param request - Session identity and requested model selection.
	* @returns the normalized selection recorded for the Session, without waiting for default persistence.
	*/
	async selectModel(request) {
		const agent = await this.resolveAgent(request.sessionId);
		return this.agents.serializeImageAdmission(agent, async () => {
			try {
				await this.requireModel(request);
				const selected = {
					provider: request.provider,
					model: request.model,
					...request.reasoningEffort === void 0 ? {} : { reasoningEffort: ReasoningEffortId(request.reasoningEffort) }
				};
				await this.ctx.llm.resolveCallConfig(selected);
				agent.model.select(selected);
				this.ctx.agentDefaultModel.saveSelection(selected).catch((error) => {
					this.ctx.logger.warn(`session-controller: model selection changed for the Session but the default was not saved: ${String(error)}`);
				});
				return { selected: { ...selected } };
			} catch (error) {
				if (remoteErrorOf(error) !== void 0) throw error;
				throw new RemoteError("session/model-unavailable", error instanceof Error ? error.message : String(error), {
					provider: request.provider,
					model: request.model
				});
			}
		});
	}
	/**
	* Normalize and append a user-owned Session title.
	* @param request - Session identity and proposed title.
	* @returns the accepted title and durable event sequence.
	*/
	async rename(request) {
		const agent = await this.resolveAgent(request.sessionId);
		const titles = this.ctx.get("sessionTitle");
		if (titles === void 0) throw new RemoteError("gateway/internal", "renaming is unavailable: this deployment mounts no session-title service", {});
		try {
			const accepted = titles.rename(agent.session, request.title);
			return {
				title: accepted.title,
				seq: accepted.eventSeq
			};
		} catch (error) {
			if (error instanceof SessionTitleInvalidError) throw new RemoteError("session/title-invalid", error.message, { sessionId: request.sessionId });
			throw new RemoteError("gateway/internal", `failed to rename session "${request.sessionId}": ${String(error)}`, {});
		}
	}
	/**
	* Create a new ordinary Session from an exact event prefix. An explicit
	* `atSeq` is the inclusive cut; an omitted value selects the latest
	* completed-turn prefix. An open cut receives synthetic fork closers.
	* @param request - source Session and optional exact event boundary.
	* @returns the new Session identity.
	*/
	async fork(request) {
		const env_1 = {
			stack: [],
			error: void 0,
			hasError: false
		};
		try {
			let atSeq;
			try {
				atSeq = request.atSeq === void 0 ? void 0 : SessionSeq(request.atSeq);
			} catch {
				throw new RemoteError("gateway/bad-request", "atSeq must be a non-negative safe integer", {});
			}
			let observed;
			try {
				observed = await this.ctx.sessionQuery.observeSession(request.sessionId);
			} catch (error) {
				if (error instanceof SessionQueryError && error.code === "SESSION_QUERY_SESSION_NOT_FOUND") throw new RemoteError("session/not-found", `session "${request.sessionId}" not found`, { sessionId: request.sessionId });
				throw new RemoteError("gateway/internal", `fork source unavailable for session "${request.sessionId}": ${String(error)}`, {});
			}
			const source = __addDisposableResource$5(env_1, observed, false);
			const boundary = atSeq ?? latestCompletedPrefixBoundary(source.events);
			if (boundary === void 0 || source.events[boundary]?.seq !== boundary) throw new RemoteError("session/fork-unavailable", request.atSeq === void 0 ? `session "${request.sessionId}" has no completed turn to fork from` : `event ${String(request.atSeq)} does not exist in session "${request.sessionId}" (last seq: ${String(source.events.at(-1)?.seq ?? "none")})`, { sessionId: request.sessionId });
			const seed = buildForkSeed(source.events, boundary);
			let workspace;
			try {
				workspace = await this.forkWorkspace(source.header);
			} catch (error) {
				throw new RemoteError("gateway/internal", `failed to resolve fork workspace for session "${request.sessionId}": ${String(error)}`, {});
			}
			const childId = brandString(`session-${randomUUID()}`);
			const composition = await this.agents.composeAgent(this.agents.presetForObservation(source));
			try {
				const { provider, model } = this.ctx.agentDefaultModel.currentSelection();
				await this.ctx.agents.create({
					sessionId: childId,
					seed,
					inheritedEventCount: SessionLogOffset(boundary + 1),
					meta: {
						...source.header.cwd === void 0 ? {} : { cwd: source.header.cwd },
						isSeeded: true,
						...composition.agentPreset === void 0 ? {} : { agentPreset: composition.agentPreset }
					},
					agentOptions: {
						provider,
						model
					},
					setup: composition.setup
				});
			} catch (error) {
				throw new RemoteError("gateway/internal", `failed to fork session "${request.sessionId}": ${String(error)}`, {});
			}
			if (workspace !== void 0) try {
				await workspace.attachSession(childId);
			} catch (error) {
				throw new RemoteError("session/workspace-attach-failed", `session "${childId}" was forked but could not attach to workspace "${workspace.id}": ${String(error)}`, {
					sessionId: childId,
					workspaceId: workspace.id
				});
			}
			return { sessionId: childId };
		} catch (e_1) {
			env_1.error = e_1;
			env_1.hasError = true;
		} finally {
			__disposeResources$5(env_1);
		}
	}
	/**
	* Admit one prompt after Agent and attachment validation; the Remote method
	* has already validated the content and built the durable message source.
	* @param request - Session identity, prompt content, and delivery mode.
	* @param source - caller metadata recorded on the accepted ordinary or retry user message.
	* @returns acknowledgement that the Agent accepted the prompt.
	*/
	async prompt(request, source) {
		const agent = await this.resolveAgent(request.sessionId);
		if (this.hasPromptRequest(agent.session, request.requestId)) return { accepted: true };
		if (request.content.length === 0) try {
			return await agent.runMaintenance(() => {
				const inbox = agent.inbox;
				if (request.mode !== "queue" || this.continuation.stateOf(agent.session).ending === null || inbox.nextTurn.length > 0 || inbox.nextStep.some((message) => message.source.kind === "user")) throw new RemoteError("session/agent-busy", "generation continuation is unavailable", { reason: "NO_CONTINUABLE_FAILURE_OR_PENDING_INPUT" });
				agent.followup(createUserMessage({
					content: [],
					source
				}));
				return Promise.resolve({ accepted: true });
			});
		} catch (error) {
			if (remoteErrorOf(error) !== void 0) throw error;
			throw new RemoteError("session/agent-busy", "generation continuation was not accepted", { reason: String(error) });
		}
		const admission = resolvePromptFileReceipts(request.content, (receiptId) => this.ctx.fileUploads.resolve(agent, receiptId));
		const hasImage = admission.content.some((part) => part.type === "image");
		const admit = async () => {
			try {
				const env_2 = {
					stack: [],
					error: void 0,
					hasError: false
				};
				try {
					if (this.hasPromptRequest(agent.session, request.requestId)) return { accepted: true };
					if (hasImage) {
						const current = agent.model.current();
						if (current !== void 0) {
							const model = await this.ctx.llm.resolveModelInfo(current.provider, current.model);
							if (model.inputModalities !== void 0 && !model.inputModalities.includes("image")) throw new RemoteError("session/attachment-invalid", `Model "${current.model}" does not support image input.`, { reason: "MODEL_DOES_NOT_SUPPORT_IMAGES" });
						}
					}
					const content = await this.ctx.attachments.admitPromptContent(admission.content);
					if (this.hasPromptRequest(agent.session, request.requestId)) return { accepted: true };
					const message = createUserMessage({
						content,
						source
					});
					if (this.ctx.agents.get(agent.id) !== agent) throw new RemoteError("session/not-found", `session "${agent.id}" was disposed during prompt admission`, { sessionId: agent.id });
					const binding = __addDisposableResource$5(env_2, this.ctx.fileUploads.bindPrompt(agent, admission.receiptIds, request.requestId), false);
					if (request.mode === "steer") agent.steer(message);
					else agent.followup(message);
					binding.commit();
				} catch (e_2) {
					env_2.error = e_2;
					env_2.hasError = true;
				} finally {
					__disposeResources$5(env_2);
				}
			} catch (error) {
				if (this.hasPromptRequest(agent.session, request.requestId)) return { accepted: true };
				if (remoteErrorOf(error) !== void 0) throw error;
				if (error instanceof AttachmentError) throw new RemoteError("session/attachment-invalid", error.message, { reason: error.code });
				throw new RemoteError("session/agent-busy", "prompt rejected", { reason: String(error) });
			}
			return { accepted: true };
		};
		return hasImage ? this.agents.serializeImageAdmission(agent, admit) : admit();
	}
	/** Recover first input acceptance, including claimed or removed messages, from unseen log entries. */
	hasPromptRequest(session, requestId) {
		let receipts = this.promptReceipts.get(session);
		if (receipts === void 0) {
			receipts = {
				offset: SessionLogOffset(0),
				messages: /* @__PURE__ */ new Set(),
				requests: /* @__PURE__ */ new Set()
			};
			this.promptReceipts.set(session, receipts);
		}
		if (receipts.offset !== session.seq) {
			for (const event of session.snapshotEvents(receipts.offset)) {
				if (event.type !== "agent/inbox/spliced" && event.type !== "user/message") continue;
				const messages = event.type === "agent/inbox/spliced" ? event.data.inserted : [event.data];
				for (const message of messages) {
					if (receipts.messages.has(message.id)) continue;
					receipts.messages.add(message.id);
					if (event.seq < session.inheritedEventCount) continue;
					const source = message.source;
					if ((source.kind === "user" || source.kind === "retry") && "rpcId" in source) receipts.requests.add(source.rpcId);
				}
			}
			receipts.offset = session.seq;
		}
		return receipts.requests.has(requestId);
	}
	async requireModel(selection) {
		if (!await modelAvailable(this.ctx, selection)) throw new RemoteError("session/model-unavailable", "Select an available model before sending a message.", {
			provider: selection.provider,
			model: selection.model
		});
	}
	/**
	* Read one durable image using the supplied reference without consulting Session history.
	* @param request - image reference and gateway permission scope; Session existence is not checked.
	* @returns the durable attachment reference and base64-encoded bytes.
	*/
	async attachment(request) {
		try {
			const stored = await this.ctx.attachments.readImage(request.attachment);
			return {
				attachment: stored.ref,
				data: Buffer.from(stored.data).toString("base64")
			};
		} catch (error) {
			if (error instanceof AttachmentError) throw new RemoteError("session/attachment-invalid", error.message, { reason: error.code });
			throw new RemoteError("gateway/internal", "Unable to read image attachment.", {});
		}
	}
	/**
	* Mutate one pending Inbox occurrence, restoring an ordinary cold Agent when needed.
	* @param request - Session, queue item, and requested mutation.
	* @returns acknowledgement that the queue mutation was applied.
	*/
	async updateQueue(request) {
		if (request.action.kind === "edit") {
			if (request.action.content.some((block) => block.type !== "text")) throw new RemoteError("session/attachment-invalid", "queue edits accept text content only", { reason: "QUEUE_EDIT_NON_TEXT" });
			if (!hasPromptContent(request.action.content)) throw new RemoteError("gateway/bad-request", "queue edit content must include non-whitespace text", {});
		}
		let agent = this.ctx.agents.get(request.sessionId);
		if (agent === void 0) {
			const found = await this.agents.resolveAgent(request.sessionId);
			if ("error" in found) {
				if (found.error.code !== "session/not-found") throw found.error;
				throw new RemoteError("session/queue-item-not-found", "queued item is no longer pending", { itemId: request.itemId });
			}
			agent = found.agent;
		}
		if (hasApiSessionSubagentOwner(agent.session)) {
			const identity = this.ctx.sessionProjections.snapshot(agent.session, ["subagent"]).values.subagent;
			if (identity?.mode !== "continuable" || !agent.session.isOwnSeq(identity.seq)) throw apiSessionSubagentOwnershipError(request.sessionId);
		}
		const nextTurn = agent.inbox.nextTurn.find((message) => message.id === request.itemId);
		const nextStep = agent.inbox.nextStep.find((message) => message.id === request.itemId);
		const located = nextTurn === void 0 ? nextStep === void 0 ? void 0 : {
			target: "next-step",
			message: nextStep
		} : {
			target: "next-turn",
			message: nextTurn
		};
		if (located === void 0) throw new RemoteError("session/queue-item-not-found", "queued item is no longer pending", { itemId: request.itemId });
		const { target, message } = located;
		if (request.action.kind === "steer" && (target !== "next-turn" || agent.status !== "running")) throw new RemoteError("session/steer-unavailable", "current turn no longer accepts steering", { itemId: request.itemId });
		switch (request.action.kind) {
			case "edit":
				agent.inbox.replace(request.itemId, freezeMessage({
					...message,
					content: [...request.action.content]
				}));
				break;
			case "remove": {
				agent.inbox.remove(request.itemId);
				const source = message.source;
				if (source.kind === "user" && "rpcId" in source) this.ctx.fileUploads.retirePrompt(agent, source.rpcId);
				break;
			}
			case "steer":
				agent.inbox.remove(request.itemId);
				agent.steer(message);
				break;
			/* v8 ignore next 2 -- closed-union exhaustiveness guard */
			default: assertNever(request.action, "queue action");
		}
		return { accepted: true };
	}
	/**
	* Cancel one live ordinary Agent while retaining pending inbox work.
	* @param request - Session whose active Agent turn is cancelled.
	* @returns acknowledgement that cancellation was requested.
	*/
	cancel(request) {
		const agent = this.ctx.agents.get(request.sessionId);
		if (agent === void 0) throw new RemoteError("session/not-found", `session "${request.sessionId}" not found (not attached)`, { sessionId: request.sessionId });
		if (hasApiSessionSubagentOwner(agent.session)) throw apiSessionSubagentOwnershipError(request.sessionId);
		agent.cancel({ kind: "user" }, { keepInbox: true });
		return { accepted: true };
	}
	async resolveAgent(sessionId) {
		const found = await this.agents.resolveAgent(sessionId);
		if ("error" in found) throw found.error;
		return found.agent;
	}
	rejectCreation(error) {
		if (remoteErrorOf(error) !== void 0) throw error;
		if (error instanceof ApiSessionPresetConflict) throw new RemoteError("agent-preset/conflict", error.message, {
			sessionId: error.sessionId,
			requestedPreset: error.requestedPreset
		});
		if (error instanceof ApiSessionCwdConflict) throw new RemoteError("session/conflict", error.message, {
			sessionId: error.sessionId,
			requestedCwd: error.requestedCwd,
			...error.existingCwd === void 0 ? {} : { existingCwd: error.existingCwd }
		});
		if (error instanceof ApiSessionSubagentOwnership) throw apiSessionSubagentOwnershipError(error.sessionId);
		throw new RemoteError("gateway/internal", `failed to create session: ${String(error)}`, {});
	}
	async forkWorkspace(source) {
		const workspaces = this.ctx.workspaceRegistry.list();
		const direct = workspaces.find((workspace) => workspace.sessionIds.includes(source.id));
		if (direct !== void 0 || source.origin !== "subagent") return direct;
		const lineage = await this.ctx.sessionQuery.traceSession(source.id);
		for (const ancestor of lineage.ancestors) {
			const workspace = workspaces.find((candidate) => candidate.sessionIds.includes(ancestor.header.id));
			if (workspace !== void 0) return workspace;
		}
	}
};
/** Replace each file part with the file or image its staged receipt names. */
function resolvePromptFileReceipts(content, staged) {
	const receiptIds = /* @__PURE__ */ new Set();
	return {
		content: content.map((part) => {
			if (part.type !== "file") return part;
			const attachment = staged(part.receiptId);
			if (attachment === void 0) throw new RemoteError("session/attachment-invalid", "File was not uploaded for this session.", { reason: "FILE_NOT_STAGED" });
			receiptIds.add(part.receiptId);
			return attachment;
		}),
		receiptIds: [...receiptIds]
	};
}
//#endregion
//#region lib/types/control.js
/** Live Session projection state with reconnect baselines. */
/** Owns the Host-wide Session control stream. */
var SessionControlController = class {
	ctx;
	streams = /* @__PURE__ */ new Set();
	/** @param ctx - Host context carrying live Agent and projection services. */
	constructor(ctx) {
		this.ctx = ctx;
		ctx.sessionProjections.onEventChanged((session, changes, seq) => {
			this.broadcast({
				type: "projection",
				sessionId: session.id,
				seq,
				changes
			});
		});
		ctx.effect(() => () => {
			for (const stream of this.streams) stream.end();
			this.streams.clear();
		}, "session-controller.control");
	}
	/**
	* Open one generation of Host-wide live control state.
	* @param signal - Remote stream cancellation.
	* @returns one complete baseline followed by live replacement frames.
	*/
	async *control(signal) {
		signal.throwIfAborted();
		const queue = new ControlQueue();
		this.streams.add(queue);
		try {
			yield {
				type: "baseline",
				value: this.baseline()
			};
			yield* queue.iterate(signal);
		} finally {
			this.streams.delete(queue);
			queue.end();
		}
	}
	baseline() {
		const sessions = this.ctx.sessions.list();
		return { projections: this.projectionBaseline(sessions) };
	}
	projectionBaseline(sessions) {
		const blocks = Object.create(null);
		for (const session of sessions) {
			const snapshot = this.ctx.sessionProjections.snapshot(session);
			blocks[session.id] = {
				asOfSeq: snapshot.asOfSeq,
				values: snapshot.values
			};
		}
		return blocks;
	}
	broadcast(frame) {
		for (const stream of this.streams) stream.push(frame);
	}
};
var ControlQueue = class {
	buffer = new Deque();
	wake;
	done = false;
	push(frame) {
		if (this.done) return;
		this.buffer.pushBack(frame);
		const wake = this.wake;
		this.wake = void 0;
		wake?.();
	}
	end() {
		if (this.done) return;
		this.done = true;
		const wake = this.wake;
		this.wake = void 0;
		wake?.();
	}
	async *iterate(signal) {
		const onAbort = () => {
			this.end();
		};
		signal.addEventListener("abort", onAbort, { once: true });
		try {
			while (!this.done && !signal.aborted) {
				const frame = this.buffer.popFront();
				if (frame !== void 0) {
					yield frame;
					continue;
				}
				await new Promise((resolve) => {
					this.wake = resolve;
				});
			}
			while (this.buffer.size > 0 && !signal.aborted) yield this.buffer.popFront();
		} finally {
			signal.removeEventListener("abort", onAbort);
			this.end();
		}
	}
};
const MESSAGE_TYPES$1 = new Set(["user/message", "assistant/message"]);
/**
* Reject an invalid message budget or turn window.
* @param request - history window fields.
*/
function validateHistoryWindow(request) {
	if (request.maxMessages !== void 0 && (!Number.isSafeInteger(request.maxMessages) || request.maxMessages <= 0)) throw new RemoteError("gateway/bad-request", "maxMessages must be a positive safe integer", {});
	const window = request.turnWindow;
	if (window !== void 0) {
		if (!Number.isSafeInteger(window.minMessages) || window.minMessages <= 0 || window.minMessages > (request.maxMessages ?? 50)) throw new RemoteError("gateway/bad-request", "turnWindow.minMessages must be a positive safe integer no greater than maxMessages", {});
		if (!Number.isSafeInteger(window.minTurns) || window.minTurns <= 0) throw new RemoteError("gateway/bad-request", "turnWindow.minTurns must be a positive safe integer", {});
	}
}
/**
* Select the Session whose log an address names.
* @param address - ordinary Session or direct-subagent address.
* @returns the Session whose log the address reads.
*/
function addressId(address) {
	return address.kind === "session" ? address.sessionId : address.childSessionId;
}
/**
* Reject an address that does not match the observed Session.
* @param address - ordinary Session or direct-subagent address.
* @param header - observed Session header.
* @param inheritedEventCount - observed fork-inherited event count.
* @param projections - observed projections; subagent addresses require them.
*/
function validateAddress(address, header, inheritedEventCount, projections) {
	if (address.kind === "session") {
		if (header.origin === "subagent") throw new RemoteError("session/agent-busy", "subagent Sessions require their durable parent address", { reason: "use subagent delivery for this child session" });
		return;
	}
	if (header.origin !== "subagent" || header.parentSession !== address.parentSessionId) throw new RemoteError("subagent/unauthorized", "subagent does not belong to the supplied parent", { childSessionId: address.childSessionId });
	const identity = projections?.values.subagent;
	if (identity === null) throw new RemoteError("subagent/catalog-diagnostic", "subagent descriptor is corrupt", {
		parentSessionId: address.parentSessionId,
		childSessionId: address.childSessionId,
		reason: "corrupt"
	});
	if (identity === void 0 || identity.seq < inheritedEventCount) throw new RemoteError("subagent/catalog-diagnostic", "subagent descriptor is unavailable", {
		parentSessionId: address.parentSessionId,
		childSessionId: address.childSessionId,
		reason: "unsupported"
	});
	if (address.mode !== "unknown" && identity.mode !== address.mode) throw new RemoteError("subagent/unauthorized", "subagent mode does not match the supplied address", { childSessionId: address.childSessionId });
}
/**
* Reject a missing Session using its address-specific error.
* @param address - ordinary Session or direct-subagent address.
* @returns never; this operation always throws.
* @throws RemoteError `session/not-found` or `subagent/not-found`.
*/
function rejectNotFound(address) {
	if (address.kind === "session") throw new RemoteError("session/not-found", `session "${address.sessionId}" not found`, { sessionId: address.sessionId });
	throw new RemoteError("subagent/not-found", "subagent is unavailable", {
		parentSessionId: address.parentSessionId,
		childSessionId: address.childSessionId
	});
}
/**
* Cut one message-aligned backwards page from a contiguous event prefix.
* @param events - contiguous events starting at seq 0.
* @param beforeSeq - exclusive upper offset of an older page.
* @param maxMessages - message budget.
* @param throughSeq - inclusive log cut.
* @param turnWindow - optional turn-boundary stop rule.
* @returns the page events and whether older events remain.
*/
function paginate(events, beforeSeq, maxMessages, throughSeq, turnWindow) {
	const end = SessionLogOffset(Math.min(throughSeq + 1, beforeSeq ?? throughSeq + 1));
	let count = 0;
	let turns = 0;
	let cut = SessionLogOffset(0);
	for (let index = end - 1; index >= 0; index--) {
		const event = events[index];
		if (turnWindow !== void 0 && event.type === "turn/start") {
			turns++;
			if (count >= turnWindow.minMessages && turns >= turnWindow.minTurns) {
				cut = SessionLogOffset(index);
				break;
			}
		}
		if (!MESSAGE_TYPES$1.has(event.type) || !isAppendSurfaceEvent(event)) continue;
		count++;
		const sources = event.sourceEventSeqs;
		let groupStart = event.seq;
		if (sources !== void 0) {
			for (const source of sources) if (source < groupStart) groupStart = source;
		}
		if (count >= maxMessages) {
			cut = SessionLogOffset(groupStart);
			break;
		}
	}
	return {
		events: events.slice(cut, end),
		hasMore: cut > 0
	};
}
/**
* Convert a committed event to its wire entry.
* @param event - committed Session event.
* @returns the wire event entry.
*/
function entryFor(event) {
	return {
		type: "event",
		event
	};
}
/**
* Encode one bounded logical page without changing its pagination cut.
* @param events - page events.
* @returns the wire records.
*/
function pageRecords(events) {
	return events.map(entryFor);
}
//#endregion
//#region lib/types/history/listener.js
/** Passive per-Session streams; subscriptions never acquire Session observations. */
/** A single subscription buffers before either consumption mode starts. */
var EventListener = class {
	buffer;
	disposes = [];
	firstReceivedSeq;
	nextExpectedSeq;
	/**
	* @param ctx - context owning the event registrations.
	* @param sessionId - Session whose notifications are emitted.
	* @param signal - cancellation that wakes consumers.
	* @param queue - optional caller-owned buffer; only an internally created buffer is disposed here.
	*/
	constructor(ctx, sessionId, signal, queue) {
		signal.throwIfAborted();
		const buffer = queue ?? new ListenerBuffer(ctx, signal);
		this.buffer = queue === void 0 ? buffer : void 0;
		const input = buffer.createInput((item) => this.transform(item));
		const target = sessionId;
		try {
			this.disposes.push(ctx.on("session/event", (session, event) => {
				if (session.id !== target) return;
				buffer.capture(() => {
					this.recordFirst(event.seq);
					input.append({
						type: "event",
						session,
						event
					});
				});
			}, { global: true }));
			this.disposes.push(ctx.on("session/created", (session) => {
				if (session.id !== target) return;
				buffer.capture(() => {
					if (session.firstLiveSeq < session.seq) this.recordFirst(session.firstLiveSeq);
					input.prepend({
						type: "created",
						session,
						end: session.seq
					});
				});
			}, { global: true }));
			this.buffer?.own(() => {
				this[Symbol.dispose]();
			});
		} catch (error) {
			this[Symbol.dispose]();
			throw error;
		}
	}
	/** Consume notifications without reading history.
	* @returns event frames from the first received event, without loading a baseline. */
	events() {
		return this.iterate();
	}
	/** Set the inclusive opening cut for buffered history notifications.
	* @param cursor - snapshot cut set before consuming the queue. */
	setCursor(cursor) {
		this.nextExpectedSeq = cursor + 1;
	}
	*transform(item) {
		if (this.disposes.length === 0) return;
		this.nextExpectedSeq ??= this.firstReceivedSeq;
		const result = eventItem(item, this.nextExpectedSeq);
		this.nextExpectedSeq = result.next;
		for (const event of result.events) {
			if (this.disposes.length === 0) return;
			yield entryFor(event);
		}
	}
	async *iterate() {
		if (this.buffer === void 0) throw new Error("Consume the externally owned history buffer directly");
		try {
			yield* this.buffer.items();
		} finally {
			this[Symbol.dispose]();
		}
	}
	recordFirst(seq) {
		this.firstReceivedSeq = this.firstReceivedSeq === void 0 ? seq : Math.min(this.firstReceivedSeq, seq);
	}
	[Symbol.dispose]() {
		for (const dispose of this.disposes.splice(0).reverse()) dispose();
		this.buffer?.[Symbol.dispose]();
	}
};
/**
* Select the uncovered events represented by one notification.
* @param item - one buffered notification.
* @param next - next required seq, or undefined before the first event.
* @returns uncovered events and the next required seq without acquiring an observation.
*/
function eventItem(item, next) {
	if (item.type === "event") {
		if (next !== void 0 && item.event.seq < next) return {
			events: [],
			next
		};
		if (next !== void 0 && item.event.seq !== next) throw new RemoteError("gateway/internal", `session event stream skipped seq ${String(next)}`, {});
		return {
			events: [item.event],
			next: item.event.seq + 1
		};
	}
	const start = next ?? item.session.firstLiveSeq;
	if (start >= item.end) return {
		events: [],
		next
	};
	const events = item.session.snapshotEvents(SessionLogOffset(start), SessionLogOffset(item.end));
	for (const event of events) {
		if (next !== void 0 && event.seq !== next) throw new RemoteError("gateway/internal", `session event stream skipped seq ${String(next)}`, {});
		next = event.seq + 1;
	}
	return {
		events,
		next
	};
}
/** Projection changes of one Session, one change per committed event that changed a projection. */
var ProjectionListener = class {
	buffer;
	/**
	* @param ctx - context owning projection notifications.
	* @param sessionId - Session whose notifications are emitted.
	* @param signal - cancellation that removes registrations and wakes consumers.
	*/
	constructor(ctx, sessionId, signal) {
		this.buffer = new ListenerBuffer(ctx, signal);
		const input = this.buffer.createInput((item) => [item]);
		const target = sessionId;
		try {
			this.buffer.own(ctx.sessionProjections.onEventChanged((session, changes, seq) => {
				if (session.id !== target) return;
				this.buffer.capture(() => {
					const change = {
						type: "projection",
						seq,
						changes
					};
					input.append({
						session,
						change
					});
				});
			}));
		} catch (error) {
			this.buffer[Symbol.dispose]();
			throw error;
		}
	}
	/** Consume per-event projection changes.
	* @returns validated change frames, one per committed event; projection seqs need not be contiguous. */
	async *changes() {
		try {
			for await (const { change } of this.buffer.items()) yield change;
		} finally {
			this[Symbol.dispose]();
		}
	}
	[Symbol.dispose]() {
		this.buffer[Symbol.dispose]();
	}
};
/** Owns buffering, callback failure and registration cleanup for one consumer. */
var ListenerBuffer = class {
	values = new Deque();
	transformIds = new Deque();
	transforms = [];
	cleanup = [];
	wake;
	closed = false;
	failure;
	consumed = false;
	constructor(ctx, signal) {
		signal.throwIfAborted();
		const close = () => {
			this[Symbol.dispose]();
		};
		signal.addEventListener("abort", close, { once: true });
		this.own(() => {
			signal.removeEventListener("abort", close);
		});
		try {
			const dispose = ctx.effect(() => close, "session-controller.listener-buffer");
			this.own(() => {
				dispose();
			});
		} catch (error) {
			this[Symbol.dispose]();
			throw error;
		}
	}
	/**
	* Register cleanup, or run it immediately when already closed.
	* @param dispose - synchronous cleanup.
	*/
	own(dispose) {
		if (this.closed) dispose();
		else this.cleanup.push(dispose);
	}
	/**
	* Contain producer failure and wake the consumer with that failure.
	* @param action - synchronous producer operation.
	*/
	capture(action) {
		if (this.closed) return;
		try {
			action();
		} catch (error) {
			this.failure = { error };
			this[Symbol.dispose]();
		}
	}
	/**
	* Register a dequeue transform and obtain its write-only input.
	* @param transform - conversion bound once and evaluated when each input is dequeued.
	* @returns a write-only input sharing this buffer's storage and lifetime.
	* @throws when this buffer is already closed.
	*/
	createInput(transform) {
		if (this.closed) throw new Error("Cannot register a transform on a closed listener buffer");
		const transformId = this.transforms.length;
		this.transforms.push((value) => transform(value));
		return {
			append: (item) => {
				if (this.closed) return;
				this.values.pushBack(item);
				this.transformIds.pushBack(transformId);
				this.notify();
			},
			prepend: (item) => {
				if (this.closed) return;
				this.values.pushFront(item);
				this.transformIds.pushFront(transformId);
				this.notify();
			}
		};
	}
	/**
	* Consume queued values once, releasing the buffer when iteration ends.
	* @returns transformed output values.
	*/
	async *items() {
		if (this.consumed) throw new RemoteError("gateway/internal", "Session listener already has a consumer", {});
		this.consumed = true;
		try {
			for (;;) {
				if (this.failure !== void 0) throw this.failure.error;
				if (this.closed) return;
				if (this.transformIds.size === 0) {
					await new Promise((resolve) => {
						this.wake = resolve;
					});
					continue;
				}
				const transformId = this.transformIds.popFront();
				const value = this.values.popFront();
				for (const item of this.transforms[transformId](value)) {
					this.throwIfFailed();
					if (this.isClosed()) return;
					yield item;
				}
			}
		} finally {
			this[Symbol.dispose]();
		}
	}
	throwIfFailed() {
		if (this.failure !== void 0) throw this.failure.error;
	}
	/** @returns whether disposal ended this buffer while a transform was yielding. */
	isClosed() {
		return this.closed;
	}
	notify() {
		const wake = this.wake;
		this.wake = void 0;
		wake?.();
	}
	[Symbol.dispose]() {
		if (this.closed) return;
		this.closed = true;
		this.values.clear();
		this.transformIds.clear();
		this.transforms.length = 0;
		this.notify();
		for (const dispose of this.cleanup.splice(0).reverse()) dispose();
	}
};
//#endregion
//#region lib/types/assistant-stream.js
/** Process-local assistant state retained for reconnecting Web followers. */
const EMPTY_BASELINE = { revision: 0 };
/**
* Folds dense Agent frames and materializes one shared immutable reconnect
* baseline per accepted revision.
*/
var SessionAssistantStreamAccumulator = class {
	activeAttempt;
	revision = 0;
	snapshotValue = EMPTY_BASELINE;
	dirty = false;
	/**
	* Fold one trusted frame from the current attached Agent lifecycle.
	* @param frame - next dense process-local Assistant frame.
	* @param durableCursor - last committed Session seq when this frame was observed.
	*/
	accept(frame, durableCursor) {
		if (frame.type === "start" && frame.revision === 1 && this.revision !== 0) {
			this.activeAttempt = void 0;
			this.revision = 0;
		}
		if (frame.revision !== this.revision + 1) {
			this.activeAttempt = void 0;
			this.revision = frame.revision;
			this.dirty = true;
			return;
		}
		this.revision = frame.revision;
		switch (frame.type) {
			case "start":
				this.activeAttempt = {
					attemptId: frame.attemptId,
					startedAfterSeq: durableCursor,
					turn: frame.turn,
					step: frame.step,
					stream: new AssistantStreamAccumulator(),
					nextIndex: 0
				};
				break;
			case "chunk": {
				const attempt = this.activeAttempt;
				if (attempt === void 0 || attempt.attemptId !== frame.attemptId || frame.index !== attempt.nextIndex) {
					this.activeAttempt = void 0;
					break;
				}
				attempt.stream.push({
					time: frame.time,
					chunk: frame.chunk
				});
				attempt.nextIndex += 1;
				break;
			}
			case "end":
				this.activeAttempt = void 0;
				break;
		}
		this.dirty = true;
	}
	/**
	* Read the cached reconnect baseline, materializing it after a state change.
	* @returns the identity-stable baseline for the latest accepted revision.
	*/
	snapshot() {
		if (!this.dirty) return this.snapshotValue;
		this.snapshotValue = {
			revision: this.revision,
			...this.activeAttempt === void 0 ? {} : { activeAttempt: {
				attemptId: this.activeAttempt.attemptId,
				startedAfterSeq: this.activeAttempt.startedAfterSeq,
				turn: this.activeAttempt.turn,
				step: this.activeAttempt.step,
				nextIndex: this.activeAttempt.nextIndex,
				stream: this.activeAttempt.stream.snapshot()
			} }
		};
		this.dirty = false;
		return this.snapshotValue;
	}
};
//#endregion
//#region lib/types/history/assistant-stream.js
/** Assistant stream baselines and per-connection subscriptions. */
/** One accumulator per live Session, shared by every subscription bound to it. */
var SessionAssistantStreamsListener = class {
	ctx;
	streams = /* @__PURE__ */ new Map();
	/** @param ctx - service context owning the accumulator listeners. */
	constructor(ctx) {
		this.ctx = ctx;
		ctx.on("agent/assistant-stream", ({ agent, frame }) => {
			let stream = this.streams.get(agent.session.id);
			if (stream === void 0) {
				stream = new SessionAssistantStreamAccumulator();
				this.streams.set(agent.session.id, stream);
			}
			stream.accept(frame, agent.session.seq === 0 ? -1 : SessionSeq(agent.session.seq - 1));
		}, { global: true });
		ctx.on("agent/disposed", ({ agent }) => {
			this.streams.delete(agent.session.id);
		}, { global: true });
		ctx.effect(() => () => {
			this.streams.clear();
		}, "session-controller.assistant-streams");
	}
	/**
	* Bind one connection to a Session assistant stream.
	* @param sessionId - Session to subscribe.
	* @param signal - cancellation owned by the caller.
	* @param queue - caller-owned raw notification buffer.
	* @returns a listener the caller disposes.
	*/
	bind(sessionId, signal, queue) {
		return new AssistantStreamListener(this.ctx, sessionId, signal, () => {
			return this.streams.get(sessionId)?.snapshot() ?? { revision: 0 };
		}, queue);
	}
};
/** Writes assistant notifications into the connection's shared queue. */
var AssistantStreamListener = class {
	_snapshot;
	disposes = [];
	ordinal = 0;
	cut = 0;
	constructor(ctx, sessionId, signal, _snapshot, output) {
		this._snapshot = _snapshot;
		signal.throwIfAborted();
		const input = output.createInput((item) => this.transform(item));
		try {
			this.disposes.push(ctx.on("agent/assistant-stream", ({ agent, frame }) => {
				if (agent.session.id !== sessionId) return;
				output.capture(() => {
					const wire = frame.type === "start" ? {
						...frame,
						startedAfterSeq: agent.session.seq === 0 ? -1 : SessionSeq(agent.session.seq - 1)
					} : frame.type === "chunk" ? {
						...frame,
						chunk: frame.chunk
					} : frame;
					input.append({
						type: "assistant-stream",
						session: agent.session,
						frame: wire,
						ordinal: ++this.ordinal
					});
				});
			}, { global: true }));
		} catch (error) {
			this[Symbol.dispose]();
			throw error;
		}
	}
	/** @returns the baseline and local arrival cut captured synchronously together. */
	snapshot() {
		return {
			baseline: this._snapshot(),
			ordinal: this.ordinal
		};
	}
	/** @param ordinal - local snapshot cut set before queue consumption. */
	setOrdinalCut(ordinal) {
		this.cut = ordinal;
	}
	*transform(item) {
		if (this.disposes.length === 0 || item.ordinal <= this.cut) return;
		yield {
			type: "assistant-stream",
			frame: item.frame
		};
	}
	[Symbol.dispose]() {
		for (const dispose of this.disposes.splice(0).reverse()) dispose();
	}
};
//#endregion
//#region lib/types/history/controller.js
/** Independent Session snapshots and passive event subscriptions. */
var __addDisposableResource$4 = function(env, value, async) {
	if (value !== null && value !== void 0) {
		if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
		var dispose, inner;
		if (async) {
			if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
			dispose = value[Symbol.asyncDispose];
		}
		if (dispose === void 0) {
			if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
			dispose = value[Symbol.dispose];
			if (async) inner = dispose;
		}
		if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources$4 = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
/** Session snapshots do not share lifetime with passive subscriptions. */
var HistoryController = class {
	ctx;
	/** Reconnect state for this controller's assistant subscriptions. */
	streamsListener;
	/** @param ctx - Host context carrying Session reads and projections. */
	constructor(ctx) {
		this.ctx = ctx;
		this.streamsListener = new SessionAssistantStreamsListener(this.ctx);
	}
	/**
	* Read a passive page of Session events at an exact cut.
	* @param request - Session identity and optional cut and page window.
	* @param signal - read cancellation.
	* @returns a page at the supplied cut, or the observed log end when omitted.
	*/
	async events(request, signal) {
		const spec = resolveHistoryRequest(request);
		try {
			return await this.usingObservation({
				sessionId: addressId(request.address),
				projection: request.address.kind === "subagent"
			}, signal, (observation) => {
				validateObservationAddress(request.address, observation);
				return history(observation, spec);
			});
		} catch (error) {
			if (remoteErrorOf(error)?.code === "session/not-found") rejectNotFound(request.address);
			throw error;
		}
	}
	/**
	* Subscribe to Session events without reading or activating the Session.
	* @param request - Session address to subscribe.
	* @param signal - stream cancellation.
	* @returns subscription readiness followed by event frames; readiness does not establish existence.
	*/
	async *observeEvent(request, signal) {
		const env_1 = {
			stack: [],
			error: void 0,
			hasError: false
		};
		try {
			const listener = __addDisposableResource$4(env_1, new EventListener(this.ctx, addressId(request.address), signal), false);
			yield { type: "ready" };
			yield* listener.events();
		} catch (e_1) {
			env_1.error = e_1;
			env_1.hasError = true;
		} finally {
			__disposeResources$4(env_1);
		}
	}
	/**
	* Read exact projections or migration cache hints without activating the Session.
	* @param request - Session identity whose exact projection baseline is required.
	* @param signal - read cancellation.
	* @returns exact sequenced values, migration cache hints, or null when absent.
	*/
	async projections(request, signal) {
		const { sessionId } = request;
		if (sessionId.length === 0) throw new RemoteError("gateway/bad-request", "sessionId must not be empty", {});
		try {
			signal.throwIfAborted();
			const live = this.ctx.sessions.get(sessionId);
			if (live !== void 0) {
				const snapshot = this.ctx.sessionProjections.snapshot(live);
				return {
					kind: "sequenced",
					asOfSeq: snapshot.asOfSeq,
					values: snapshot.values
				};
			} else {
				const stored = await this.ctx.get("sessionPersistence")?.stat(sessionId, { signal });
				signal.throwIfAborted();
				if (stored?.formatStatus === "migration-required") return {
					kind: "migration-required",
					values: { ...(this.ctx.get("sessionProjectionCache")?.cachedSnapshot(stored.header))?.values }
				};
				return await this.usingObservation({
					sessionId: request.sessionId,
					projection: true
				}, signal, (observation) => ({
					kind: "sequenced",
					...projections(observation)
				}));
			}
		} catch (error) {
			const code = typeof error === "object" && error !== null && "code" in error ? error.code : void 0;
			if (code === "SESSION_QUERY_SESSION_NOT_FOUND" || code === "session/not-found") return null;
			if (signal.aborted || code === "SESSION_QUERY_ABORTED") throw new RemoteError("gateway/cancelled", "Session projection read was cancelled", {}, { cause: error });
			if (remoteErrorOf(error) !== void 0) throw error;
			throw new RemoteError("gateway/internal", "Session projection read failed", {}, { cause: error });
		}
	}
	/**
	* Subscribe to projection changes without reading a baseline.
	* @param request - Session identity to subscribe without reading a baseline.
	* @param signal - stream cancellation.
	* @returns readiness followed by one change frame per committed event, listing every key it changed.
	*/
	async *observeProjections(request, signal) {
		const env_2 = {
			stack: [],
			error: void 0,
			hasError: false
		};
		try {
			const listener = __addDisposableResource$4(env_2, new ProjectionListener(this.ctx, request.sessionId, signal), false);
			yield { type: "ready" };
			yield* listener.changes();
		} catch (e_2) {
			env_2.error = e_2;
			env_2.hasError = true;
		} finally {
			__disposeResources$4(env_2);
		}
	}
	/**
	* Read bounded committed assistant messages through a stopping cursor.
	* @param sessionId - Session whose committed assistant messages are requested.
	* @param afterSeq - exclusive lower bound.
	* @param throughSeq - inclusive stopping-state cursor.
	* @param maxMessages - positive assistant record limit.
	* @param signal - caller cancellation, independent of a completed wait deadline.
	* @returns newest-first assistant messages and whether further matching records were omitted.
	*/
	async assistantOutput(sessionId, afterSeq, throughSeq, maxMessages, signal) {
		return this.usingObservation({
			sessionId,
			projection: false
		}, signal, (observation) => {
			if (throughSeq > observation.cursor) throw new RemoteError("gateway/internal", "Session history no longer covers the wait cursor", {});
			const records = [];
			const events = observation.events;
			for (let index = throughSeq; index > afterSeq; index--) {
				const event = events[index];
				if (event.type !== "assistant/message" || !isAppendSurfaceEvent(event)) continue;
				if (records.length === maxMessages) return {
					records,
					truncated: true
				};
				records.push(entryFor(event));
			}
			return {
				records,
				truncated: false
			};
		});
	}
	async usingObservation({ sessionId, projection }, signal, callback) {
		const env_3 = {
			stack: [],
			error: void 0,
			hasError: false
		};
		try {
			let observation;
			try {
				observation = await this.ctx.sessionQuery.observeSession(sessionId, {
					signal,
					projectionMode: projection ? "all" : "none"
				});
			} catch (error) {
				const code = error?.code;
				if (code === "SESSION_QUERY_SESSION_NOT_FOUND") throw new RemoteError("session/not-found", `session "${sessionId}" not found`, { sessionId });
				if (code === "SESSION_QUERY_ABORTED") throw new RemoteError("gateway/cancelled", error instanceof Error ? error.message : "Session read was cancelled", {}, { cause: error });
				throw error;
			}
			try {
				signal.throwIfAborted();
			} catch (error) {
				observation[Symbol.dispose]();
				throw error;
			}
			if (callback === void 0) return observation;
			return await callback(__addDisposableResource$4(env_3, observation, false));
		} catch (e_3) {
			env_3.error = e_3;
			env_3.hasError = true;
		} finally {
			__disposeResources$4(env_3);
		}
	}
};
/**
* Validate an Address against the exact read used by a Controller.
* @param address - caller-supplied Session address.
* @param observation - borrowed read; subagent addresses require projections.
*/
function validateObservationAddress(address, observation) {
	if (observation.header.cwd === void 0) rejectNotFound(address);
	validateAddress(address, observation.header, observation.inheritedEventCount, observation.projections);
}
/**
* Convert a borrowed observation to one page without acquiring or retaining it.
* @param observation - exact read cut owned by the caller.
* @param request - validated page window and optional inclusive cut.
* @returns a chronological page at the requested cut.
*/
function history(observation, request) {
	const cursor = request.throughSeq === void 0 ? observation.cursor : request.throughSeq === -1 ? -1 : SessionSeq(request.throughSeq);
	if (cursor > observation.cursor) throw new RemoteError("gateway/bad-request", `session history cut ${String(cursor)} exceeds ${String(observation.cursor)}`, {});
	const page = paginate(observation.events, request.beforeSeq === void 0 ? void 0 : SessionLogOffset(request.beforeSeq), request.maxMessages, cursor, request.turnWindow);
	return {
		cursor,
		records: pageRecords(page.events),
		hasMore: page.hasMore
	};
}
/**
* Convert the projection baseline without acquiring or retaining its observation.
* @param observation - projected read cut owned by the caller.
* @returns the exact baseline; missing projection support fails explicitly.
*/
function projections(observation) {
	const snapshot = observation.projections;
	if (snapshot === void 0) throw new RemoteError("session/projections-unavailable", "Session projections are unavailable", {});
	return {
		asOfSeq: snapshot.asOfSeq,
		values: snapshot.values
	};
}
/**
* Validate and resolve the history page window.
* @param request - external history window.
* @returns the validated request with an explicit message budget.
*/
function resolveHistoryRequest(request) {
	if (request.throughSeq !== void 0 && (!Number.isSafeInteger(request.throughSeq) || request.throughSeq < -1 || Object.is(request.throughSeq, -0))) throw new RemoteError("gateway/bad-request", "throughSeq must be an integer greater than or equal to -1", {});
	if (request.beforeSeq !== void 0 && (!Number.isSafeInteger(request.beforeSeq) || request.beforeSeq < 0 || Object.is(request.beforeSeq, -0))) throw new RemoteError("gateway/bad-request", "beforeSeq must be a non-negative safe integer", {});
	validateHistoryWindow(request);
	return {
		...request,
		maxMessages: request.maxMessages ?? 50
	};
}
//#endregion
//#region lib/types/state/listener.js
/** Coalesced Session state observations with Host-local revisions. */
/** Coalesces Session/Agent invalidations with local revisions; pending events are sampled on reads. */
var StateListener = class {
	readState;
	disposes = [];
	cancellation = new AbortController();
	signal;
	revision = 0;
	resolver = createDeferredSignal();
	closed = false;
	/**
	* @param ctx - lifecycle notification owner.
	* @param target - Session to observe.
	* @param signal - caller cancellation.
	* @param readState - passive complete-state reader.
	*/
	constructor(ctx, target, signal, readState) {
		this.readState = readState;
		signal.throwIfAborted();
		this.signal = AbortSignal.any([signal, this.cancellation.signal]);
		const notify = (id) => {
			if (id !== target || this.closed) return;
			this.revision++;
			const resolver = this.resolver;
			this.resolver = createDeferredSignal();
			resolver.resolve();
		};
		const close = () => {
			this[Symbol.dispose]();
		};
		signal.addEventListener("abort", close, { once: true });
		this.disposes.push(() => {
			signal.removeEventListener("abort", close);
		});
		try {
			this.disposes.push(ctx.on("session/event", (session) => {
				notify(session.id);
			}, { global: true }));
			this.disposes.push(ctx.on("session/created", (session) => {
				notify(session.id);
			}, { global: true }));
			this.disposes.push(ctx.on("session/disposed", (session) => {
				notify(session.id);
			}, { global: true }));
			this.disposes.push(ctx.on("agent/status", ({ agent }) => {
				notify(agent.id);
			}, { global: true }));
			this.disposes.push(ctx.on("agent/created", ({ agent }) => {
				notify(agent.id);
			}, { global: true }));
			this.disposes.push(ctx.on("agent/disposed", ({ agent }) => {
				notify(agent.id);
			}, { global: true }));
			const dispose = ctx.effect(() => close, "session-controller.state-listener");
			this.disposes.push(() => {
				dispose();
			});
		} catch (error) {
			this[Symbol.dispose]();
			throw error;
		}
	}
	/**
	* Check whether a completed read still represents the latest invalidation.
	* @param revision - revision captured with a completed read.
	* @returns whether no intervening invalidation occurred.
	*/
	isCurrent(revision) {
		return !this.closed && revision === this.revision;
	}
	/**
	* Read coalesced state replacements with optional periodic refresh.
	* @param refreshMs - optional idle recheck interval; omitted for event-only observation.
	* @returns complete state replacements with internal consistency revisions.
	*/
	async *observeStates(refreshMs) {
		let lastRevision = -1;
		try {
			while (!this.closed) {
				if (lastRevision === this.revision) if (refreshMs === void 0) await this.resolver.promise;
				else {
					let timer;
					try {
						await Promise.race([this.resolver.promise, new Promise((resolve) => {
							timer = setTimeout(resolve, refreshMs);
						})]);
					} finally {
						clearTimeout(timer);
					}
					if (this.isCurrent(lastRevision)) {
						this.revision++;
						const resolver = this.resolver;
						this.resolver = createDeferredSignal();
						resolver.resolve();
					}
				}
				if (!this.isCurrent(this.revision)) return;
				lastRevision = this.revision;
				const state = await this.readState(this.signal);
				if (this.isCurrent(lastRevision)) yield {
					state,
					revision: lastRevision
				};
			}
		} finally {
			this[Symbol.dispose]();
		}
	}
	[Symbol.dispose]() {
		if (this.closed) return;
		this.closed = true;
		this.cancellation.abort();
		this.resolver.resolve();
		for (const dispose of this.disposes.splice(0).reverse()) dispose();
	}
};
function createDeferredSignal() {
	let resolve;
	return {
		promise: new Promise((settle) => {
			resolve = settle;
		}),
		resolve
	};
}
//#endregion
//#region lib/types/state/controller.js
/** Passive Session state, coalesced observations and quiescence waits. */
var __addDisposableResource$3 = function(env, value, async) {
	if (value !== null && value !== void 0) {
		if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
		var dispose, inner;
		if (async) {
			if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
			dispose = value[Symbol.asyncDispose];
		}
		if (dispose === void 0) {
			if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
			dispose = value[Symbol.dispose];
			if (async) inner = dispose;
		}
		if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources$3 = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
const DEFAULT_WAIT_TIMEOUT_MS = 3e4;
/** Reads and waits without activating or cancelling Session execution. */
var StateController = class {
	ctx;
	historyController;
	/**
	* @param ctx - live Session and pending Remote event services.
	* @param historyController - passive read entry used for cold state and its bounded output.
	*/
	constructor(ctx, historyController) {
		this.ctx = ctx;
		this.historyController = historyController;
	}
	/**
	* Read passive execution and pending-input state.
	* @param request - Session identity.
	* @param signal - read cancellation.
	* @returns current passive state.
	*/
	async state(request, signal) {
		return this.readState(request.sessionId, signal);
	}
	/**
	* Observe complete passive state replacements.
	* @param request - Session identity.
	* @param signal - observation cancellation.
	* @returns complete state replacements, starting with the current state.
	*/
	async *observeState(request, signal) {
		const env_1 = {
			stack: [],
			error: void 0,
			hasError: false
		};
		try {
			const listener = __addDisposableResource$3(env_1, this.listen(request.sessionId, signal), false);
			for await (const { state } of listener.observeStates()) yield state;
		} catch (e_1) {
			env_1.error = e_1;
			env_1.hasError = true;
		} finally {
			__disposeResources$3(env_1);
		}
	}
	/**
	* Wait for quiescence, pending input, or the deadline without cancelling execution.
	* @param request - Session identity, timeout and assistant output window.
	* @param signal - cancellation of this wait only, never of Agent execution.
	* @returns the stopping reason and last complete state; no state before timeout is an error.
	*/
	async wait(request, signal) {
		const env_2 = {
			stack: [],
			error: void 0,
			hasError: false
		};
		try {
			const timeoutMs = request.timeoutMs ?? DEFAULT_WAIT_TIMEOUT_MS;
			const afterSeq = request.afterSeq ?? -1;
			const maxMessages = request.maxMessages ?? 50;
			if (!Number.isSafeInteger(afterSeq) || afterSeq < -1 || Object.is(afterSeq, -0)) throw new RemoteError("gateway/bad-request", "afterSeq must be an integer greater than or equal to -1", {});
			if (!Number.isSafeInteger(maxMessages) || maxMessages <= 0) throw new RemoteError("gateway/bad-request", "maxMessages must be a positive safe integer", {});
			if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0 || timeoutMs > MAX_TIMER_DELAY_MS) throw new RemoteError("gateway/bad-request", "timeoutMs must be a positive supported timer delay", {});
			const timer = __addDisposableResource$3(env_2, deadline(signal, timeoutMs, "session/wait-timeout"), false);
			const listener = __addDisposableResource$3(env_2, this.listen(request.sessionId, timer.signal), false);
			const sessionId = request.sessionId;
			const statesGenerator = listener.observeStates(1e3);
			let nextStatePromise = statesGenerator.next();
			let currentState;
			let reason = "timeout";
			try {
				for (;;) {
					const observedState = await nextStatePromise;
					if (observedState.done) {
						timer.signal.throwIfAborted();
						throw new RemoteError("session/observation-ended", "Session observation ended before a stopping state", {});
					}
					nextStatePromise = statesGenerator.next();
					nextStatePromise.catch(() => void 0);
					if (!listener.isCurrent(observedState.value.revision)) continue;
					currentState = observedState.value.state;
					const action = computeWaitAction(currentState);
					if (action === "continue") continue;
					if (action === "input-required" || action === "idle") {
						reason = action;
						break;
					}
					const agent = this.ctx.agents.get(sessionId);
					if (agent === void 0 || this.ctx.sessions.get(sessionId) !== agent.session) continue;
					if (await waitForIdleOrStateChange(agent, nextStatePromise) === "state-changed") continue;
					if (!listener.isCurrent(observedState.value.revision) || this.ctx.agents.get(sessionId) !== agent || this.ctx.sessions.get(sessionId) !== agent.session) continue;
					reason = "idle";
					break;
				}
			} catch (error) {
				if (signal.aborted) throw signal.reason;
				if (!expired(timer.signal, error)) throw error;
			} finally {
				listener[Symbol.dispose]();
				await nextStatePromise.catch(() => void 0);
				await statesGenerator.return(void 0);
			}
			signal.throwIfAborted();
			if (currentState === void 0) throw new RemoteError("session/wait-timeout", "No Session state arrived before the wait deadline", {});
			const output = await this.historyController.assistantOutput(sessionId, afterSeq, currentState.cursor, maxMessages, signal);
			return {
				reason,
				state: currentState,
				output
			};
		} catch (e_2) {
			env_2.error = e_2;
			env_2.hasError = true;
		} finally {
			__disposeResources$3(env_2);
		}
	}
	listen(sessionId, signal) {
		return new StateListener(this.ctx, sessionId, signal, (readSignal) => this.readState(sessionId, readSignal));
	}
	/**
	* @param sessionId - Session to sample.
	* @param signal - read cancellation.
	* @param retried - whether a live source already redirected this read once.
	* @returns one passive state sample.
	*/
	async readState(sessionId, signal, retried = false) {
		signal.throwIfAborted();
		const agent = this.ctx.agents.get(sessionId);
		if (agent !== void 0) {
			if (agent.session.header.cwd === void 0) rejectNotFound({
				kind: "session",
				sessionId
			});
			return {
				cursor: agent.session.seq - 1,
				active: true,
				running: agent.status === "running",
				queue: queueItems(agent),
				pendingInput: this.pendingInput(agent.session.id)
			};
		}
		const state = await this.historyController.usingObservation({
			sessionId,
			projection: false
		}, signal, (observation) => {
			if (!retried && observation.source === "live") return void 0;
			if (observation.header.cwd === void 0) rejectNotFound({
				kind: "session",
				sessionId
			});
			return {
				cursor: observation.cursor,
				active: false,
				running: false,
				queue: null,
				pendingInput: this.pendingInput(observation.header.id)
			};
		});
		signal.throwIfAborted();
		return state ?? this.readState(sessionId, signal, true);
	}
	pendingInput(sessionId) {
		const kinds = /* @__PURE__ */ new Set();
		for (const { event, agentId } of this.ctx.typertGateway.getPendingRemoteEvents()) {
			if (String(agentId) !== sessionId) continue;
			kinds.add(event === "approval/request" ? "approval" : event === "user-questions/request" ? "question" : "other");
		}
		return [...kinds];
	}
};
/**
* @param signal - wait deadline signal.
* @param error - failure raised by the in-flight read.
* @returns whether the deadline expired and cancelled that read.
*/
function expired(signal, error) {
	const reason = timeoutOf(signal, "session/wait-timeout");
	if (reason === void 0) return false;
	let current = error;
	for (let depth = 0; depth < 4 && current instanceof Error; depth++) {
		if (current === reason || current.cause === reason) return true;
		current = current.cause;
	}
	return error === reason;
}
/** Choose the next wait action; check-idle requires the caller's whenIdle and identity checks. */
function computeWaitAction(state) {
	if (state.pendingInput.length > 0) return "input-required";
	if (state.running) return "continue";
	if (!state.active) return "idle";
	return "check-idle";
}
/** Wait for either driver quiescence or the already-armed next state; preserve rejections from both. */
function waitForIdleOrStateChange(agent, nextStatePromise) {
	return Promise.race([agent.whenIdle().then(() => "idle"), nextStatePromise.then(() => "state-changed")]);
}
function queueItems(agent) {
	const item = (message, placement) => ({
		id: message.id,
		placement,
		...message.source.kind === "user" && "rpcId" in message.source ? { rpcId: message.source.rpcId } : {},
		content: message.content
	});
	return [...agent.inbox.nextTurn.map((message) => item(message, "queued")), ...agent.inbox.nextStep.map((message) => item(message, message.source.kind === "user" ? "steering" : "context"))];
}
//#endregion
//#region lib/types/web/controller.js
/** Web opening aggregation over one observation and a shared transforming buffer. */
var __addDisposableResource$2 = function(env, value, async) {
	if (value !== null && value !== void 0) {
		if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
		var dispose, inner;
		if (async) {
			if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
			dispose = value[Symbol.asyncDispose];
		}
		if (dispose === void 0) {
			if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
			dispose = value[Symbol.dispose];
			if (async) inner = dispose;
		}
		if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources$2 = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
/** Combines opening data and consumes transformed listener output from one queue. */
var WebController = class {
	ctx;
	historyController;
	promote;
	/**
	* @param ctx - Host context carrying Session queries and identity projections.
	* @param historyController - passive reading and listener construction.
	* @param promote - background activation requested before an opening snapshot is delivered.
	*/
	constructor(ctx, historyController, promote) {
		this.ctx = ctx;
		this.historyController = historyController;
		this.promote = promote;
	}
	/**
	* Open a history snapshot and follow uncovered history and assistant notifications.
	* @param request - Session identity, page budget, assistant output and optional activation (default false).
	* @param signal - cancellation for opening and streaming.
	* @returns an opening snapshot followed by uncovered queued increments.
	*/
	async *follow(request, signal) {
		const env_1 = {
			stack: [],
			error: void 0,
			hasError: false
		};
		try {
			const sessionId = addressId(request.address);
			const spec = resolveHistoryRequest(request);
			const queue = __addDisposableResource$2(env_1, new ListenerBuffer(this.ctx, signal), false);
			const listener = __addDisposableResource$2(env_1, new EventListener(this.ctx, sessionId, signal, queue), false);
			const streams = this.historyController.streamsListener;
			const assistantListener = __addDisposableResource$2(env_1, request.assistantStream === true ? streams.bind(sessionId, signal, queue) : void 0, false);
			const observation = __addDisposableResource$2(env_1, await this.historyController.usingObservation({
				sessionId,
				projection: true
			}, signal).catch((error) => {
				if (remoteErrorOf(error)?.code === "session/not-found") rejectNotFound(request.address);
				throw error;
			}), false);
			validateObservationAddress(request.address, observation);
			let opening = {
				type: "snapshot",
				header: { ...observation.header },
				projections: projections(observation),
				...history(observation, spec)
			};
			listener.setCursor(opening.cursor);
			if (assistantListener !== void 0) {
				const { baseline: assistantStream, ordinal } = assistantListener.snapshot();
				opening = {
					...opening,
					assistantStream
				};
				assistantListener.setOrdinalCut(ordinal);
			}
			if (request.activate === true && !signal.aborted && observation.header.origin !== "subagent" && observation.source === "prepared") {
				const promotion = observation.retain();
				observation[Symbol.dispose]();
				try {
					this.promote(promotion);
				} catch (error) {
					promotion[Symbol.dispose]();
					throw error;
				}
			} else observation[Symbol.dispose]();
			yield opening;
			yield* queue.items();
		} catch (e_1) {
			env_1.error = e_1;
			env_1.hasError = true;
		} finally {
			__disposeResources$2(env_1);
		}
	}
};
//#endregion
//#region lib/types/file-references.js
/** Session Controller adapter for Agent-scoped file-reference discovery. */
var __runInitializers$2 = function(thisArg, initializers, value) {
	var useValue = arguments.length > 2;
	for (var i = 0; i < initializers.length; i++) value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
	return useValue ? value : void 0;
};
var __esDecorate$2 = function(ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
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
/** Host Remote adapter over the composed file-reference provider. */
let SessionFileReferences = (() => {
	let _classSuper = TypertRemoteService;
	let _instanceExtraInitializers = [];
	let _list_decorators;
	return class SessionFileReferences extends _classSuper {
		static {
			const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
			_list_decorators = [Remote];
			__esDecorate$2(this, null, _list_decorators, {
				kind: "method",
				name: "list",
				static: false,
				private: false,
				access: {
					has: (obj) => "list" in obj,
					get: (obj) => obj.list
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
		static inject = ["fileReferences", "typert"];
		/** @param ctx - Host context carrying the selected file-reference provider. */
		constructor(ctx) {
			super(ctx, "sessionFileReferences", { namespace: "fileReferences" });
			__runInitializers$2(this, _instanceExtraInitializers);
		}
		/**
		* List file and directory candidates for one Agent's working directory.
		* @param agent - target Agent resolved from the Session identity on the wire.
		* @param query - path text following `@` or `@"`.
		* @param signal - caller cancellation.
		* @returns deterministic path-only candidates from the composed provider.
		*/
		list(agent, query, signal) {
			return this.ctx.fileReferences.list(agent, query, signal);
		}
	};
})();
//#endregion
//#region lib/types/list.js
/** Cold-safe Session list and search projection. */
const SEARCH_PROVIDER_CALL_LIMIT = 100;
const SESSION_SEARCH_QUERY_MAX_CHARS = 500;
const MESSAGE_TYPES = new Set(["user/message", "assistant/message"]);
const sessionListMetadataSchema = z$1.object({
	blank: z$1.boolean(),
	lastPromptAt: z$1.number().nullable()
});
const imageLimitsSchema = z$1.object({
	maxImageBytes: z$1.number().int().positive(),
	maxImagesPerMessage: z$1.number().int().positive(),
	maxMessageImageBytes: z$1.number().int().positive(),
	maxImagePixels: z$1.number().int().positive(),
	maxImageDimension: z$1.number().int().positive(),
	mediaTypes: z$1.array(z$1.string())
});
/**
* Advance the Session-list metadata projection by one committed event.
* @param state - metadata before the event.
* @param event - next committed Session event.
* @returns the original or advanced metadata value.
*/
function applySessionListMetadata(state, event) {
	const blank = state.blank && event.type !== "turn/start";
	const lastPromptAt = event.type === "user/message" && event.data.source.kind === "user" ? event.time : state.lastPromptAt;
	return blank === state.blank && lastPromptAt === state.lastPromptAt ? state : {
		blank,
		lastPromptAt
	};
}
/**
* Return the longest prefix containing at most `maximum` Unicode code points.
* @param value - source text.
* @param maximum - maximum number of Unicode code points.
* @returns the source text or its longest allowed prefix.
*/
function truncateUnicodeCodePoints(value, maximum) {
	let count = 0;
	let end = 0;
	for (const codePoint of value) {
		if (count === maximum) return value.slice(0, end);
		count++;
		end += codePoint.length;
	}
	return value;
}
/** Owns list projection registration, bounded cold summaries, and authorized search. */
var ApiSessionList = class {
	ctx;
	workSliceMs;
	listMetadata;
	/**
	* @param ctx - Host context carrying Session, query, persistence, and projection services.
	* @param workSliceMs - Resolved positive integral list-work budget in milliseconds.
	*/
	constructor(ctx, workSliceMs) {
		this.ctx = ctx;
		this.workSliceMs = workSliceMs;
	}
	/**
	* Acquire projection readers before exposing dependent operations.
	* @returns when all required registrations are ready.
	*/
	async initialize() {
		this.listMetadata = await this.ctx.sessionProjections.register({
			key: "sessionListMetadata",
			stateSchema: sessionListMetadataSchema,
			init: () => ({
				blank: true,
				lastPromptAt: null
			}),
			apply: applySessionListMetadata,
			wire: {
				viewSchema: sessionListMetadataSchema,
				view: (state) => state
			},
			stateVersion: 1
		});
		await this.ctx.sessionProjections.register({
			key: "imageLimits",
			stateSchema: z$1.null(),
			init: () => null,
			apply: (state) => state,
			wire: {
				viewSchema: imageLimitsSchema,
				view: () => this.ctx.attachments.imageLimits,
				summary: false
			},
			stateVersion: 1
		});
	}
	/**
	* Build one current attached-Session summary.
	* Blankness and recency come from the `sessionListMetadata` handle; projection
	* hints carry every summary key computed through `snapshot`. The first call
	* for a Session computes these projections over its whole in-memory log, and
	* later calls advance them incrementally. The handle read and the hint with
	* the same key come from one cell; the handle read does not depend on any
	* other unit's view.
	* @param session - attached Session to summarize.
	* @returns current list metadata and available projections.
	*/
	summaryFor(session) {
		const metadata = this.listMetadata.stateOf(session);
		const projections = this.projectionsFor(session.header, session);
		return {
			sessionId: session.id,
			updatedAt: updatedAt(session.header, metadata),
			agentAvailable: this.ctx.agents.get(session.id)?.session === session,
			formatStatus: "current",
			running: this.ctx.agents.get(session.id)?.status === "running",
			blank: metadata.blank,
			...listFields(session.header),
			...projections === void 0 ? {} : { projections }
		};
	}
	/**
	* Read every visible attached and persisted Session without activating an Agent.
	* @param signal - optional cancellation for persistence reads and summary generation.
	* @returns visible Session summaries ordered by activity.
	*/
	async list(signal) {
		signal?.throwIfAborted();
		const records = await this.ctx.sessionQuery.listSessions(signal);
		signal?.throwIfAborted();
		const items = [];
		const cold = [];
		let yieldDeadline = performance.now() + this.workSliceMs;
		for (const record of records) {
			signal?.throwIfAborted();
			const live = this.ctx.sessions.get(record.header.id);
			if (live !== void 0) items.push(this.summaryFor(live));
			else if (record.header.cwd !== void 0) cold.push(record);
			if (performance.now() >= yieldDeadline) {
				await scheduler.yield();
				signal?.throwIfAborted();
				yieldDeadline = performance.now() + this.workSliceMs;
			}
		}
		for (const record of cold) {
			signal?.throwIfAborted();
			items.push(this.summarizeCold(record));
			if (performance.now() >= yieldDeadline) {
				await scheduler.yield();
				signal?.throwIfAborted();
				yieldDeadline = performance.now() + this.workSliceMs;
			}
		}
		signal?.throwIfAborted();
		items.sort((left, right) => right.updatedAt - left.updatedAt);
		return items;
	}
	summarizeCold({ header, formatStatus }) {
		const projections = this.projectionsFor(header, void 0);
		const metadata = projections?.values.sessionListMetadata;
		return {
			sessionId: header.id,
			updatedAt: updatedAt(header, metadata),
			agentAvailable: false,
			...formatStatus === void 0 ? {} : { formatStatus },
			running: false,
			blank: metadata?.blank ?? false,
			...listFields(header),
			...projections === void 0 ? {} : { projections }
		};
	}
	/**
	* Search current visible message content without activating any matching Session.
	* @param query - literal message-content query.
	* @param signal - cancellation for list and search reads.
	* @returns authorized bounded Session search results.
	*/
	async search(query, signal) {
		const normalizedQuery = normalizeSearchQuery(query);
		signal.throwIfAborted();
		const provider = this.ctx.get("sessionQuery");
		if (provider === void 0) throw new RemoteError("gateway/internal", "session search is unavailable: this deployment does not mount @deepseek-ai/dsh-session-query", {});
		try {
			const visible = await provider.listSessions(signal);
			signal.throwIfAborted();
			const visibleIds = new Set(visible.filter((record) => record.header.cwd !== void 0).map((record) => record.header.id));
			if (visibleIds.size === 0) return {
				items: [],
				hasMore: false
			};
			const authorized = [];
			const acceptedIds = /* @__PURE__ */ new Set();
			const seenCursors = /* @__PURE__ */ new Set();
			let cursor;
			let providerCalls = 0;
			let pageLimit = 20;
			while (authorized.length <= 20) {
				signal.throwIfAborted();
				if (providerCalls >= SEARCH_PROVIDER_CALL_LIMIT) throw new Error(`session search provider exceeded the ${SEARCH_PROVIDER_CALL_LIMIT}-call work budget`);
				providerCalls++;
				const requestedCursor = cursor;
				const requestedLimit = pageLimit;
				let page;
				try {
					page = await provider.searchSessions({
						query: normalizedQuery,
						eventFilters: [{
							kind: "type",
							values: ["user/message", "assistant/message"]
						}, {
							kind: "surface",
							values: ["current"]
						}],
						limit: requestedLimit,
						...requestedCursor === void 0 ? {} : { cursor: requestedCursor }
					}, { signal });
					signal.throwIfAborted();
				} catch (error) {
					signal.throwIfAborted();
					if (requestedCursor === void 0 && error instanceof SessionQueryError && error.code === "SESSION_QUERY_INVALID_LIMIT" && requestedLimit > 1) {
						pageLimit = Math.max(1, Math.floor(requestedLimit / 2));
						continue;
					}
					if (requestedCursor !== void 0 && error instanceof SessionQueryError && error.code === "SESSION_QUERY_STALE_CURSOR") {
						authorized.length = 0;
						acceptedIds.clear();
						seenCursors.clear();
						cursor = void 0;
						continue;
					}
					throw error;
				}
				if (page.items.length > requestedLimit) throw new Error(`session search provider returned ${String(page.items.length)} items; maximum is ${String(requestedLimit)}`);
				for (const hit of page.items) {
					if (authorized.length > 20) continue;
					if (!visibleIds.has(hit.header.id) || hit.bestMatch.sessionId !== hit.header.id || hit.bestMatch.surface !== "current" || !MESSAGE_TYPES.has(hit.bestMatch.type) || acceptedIds.has(hit.header.id)) continue;
					acceptedIds.add(hit.header.id);
					authorized.push({
						sessionId: hit.header.id,
						snippet: truncateUnicodeCodePoints(hit.bestMatch.snippet, 240)
					});
				}
				if (page.nextCursor !== void 0) {
					if (seenCursors.has(page.nextCursor)) throw new Error("session search provider repeated a continuation cursor");
					seenCursors.add(page.nextCursor);
				}
				if (authorized.length > 20 || page.nextCursor === void 0) break;
				cursor = page.nextCursor;
			}
			return {
				items: authorized.slice(0, 20),
				hasMore: authorized.length > 20
			};
		} catch (error) {
			signal.throwIfAborted();
			if (error instanceof SessionQueryError && error.code === "SESSION_QUERY_ABORTED") throw new RemoteError("gateway/cancelled", "session search was aborted", {});
			throw new RemoteError("gateway/internal", `session search failed: ${String(error)}`, {});
		}
	}
	projectionsFor(header, session) {
		try {
			const keys = this.ctx.sessionProjections.summaryKeys();
			if (session !== void 0) return hintsOf("sequenced", this.ctx.sessionProjections.snapshot(session, keys));
			const cache = this.ctx.get("sessionProjectionCache");
			return hintsOf("cached", cache?.cachedSnapshot(header, keys) ?? cache?.cachedPredecessorTitle(header));
		} catch (error) {
			this.ctx.logger.warn(`api-session.list: projection column for "${header.id}" failed; serving the row without it: ${String(error)}`);
			return;
		}
	}
};
/**
* Wrap one summary projection block as Session-list hints of the named sequence space.
* The block is the summary key set's values, not every value the source holds.
* @param kind - which sequence space the block's watermark belongs to.
* @param block - the block, or `undefined` when no source served one.
* @returns the hints, or `undefined` when the block is absent or carries no value.
*/
function hintsOf(kind, block) {
	if (block === void 0 || Object.keys(block.values).length === 0) return void 0;
	return {
		kind,
		asOfSeq: block.asOfSeq,
		values: block.values
	};
}
function normalizeSearchQuery(query) {
	const normalized = query.trim();
	if (normalized.length === 0) throw new RemoteError("gateway/bad-request", "session search query must not be empty", {});
	if (normalized.length > SESSION_SEARCH_QUERY_MAX_CHARS) throw new RemoteError("gateway/bad-request", `session search query must contain at most ${SESSION_SEARCH_QUERY_MAX_CHARS} UTF-16 code units`, {});
	if (normalized.includes("\0")) throw new RemoteError("gateway/bad-request", "session search query must not contain NUL", {});
	return normalized;
}
function updatedAt(header, metadata) {
	return Math.max(header.createdAt, metadata?.lastPromptAt ?? 0);
}
function listFields(header) {
	return {
		...header.parentSession === void 0 ? {} : { parentSessionId: header.parentSession },
		...header.origin === void 0 ? {} : { origin: header.origin },
		...header.cwd === void 0 ? {} : { cwd: header.cwd }
	};
}
//#endregion
//#region lib/types/skill-catalog.js
/** Session-addressed, cold-readable skill catalog Remote. */
var __runInitializers$1 = function(thisArg, initializers, value) {
	var useValue = arguments.length > 2;
	for (var i = 0; i < initializers.length; i++) value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
	return useValue ? value : void 0;
};
var __esDecorate$1 = function(ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
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
var __addDisposableResource$1 = function(env, value, async) {
	if (value !== null && value !== void 0) {
		if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
		var dispose, inner;
		if (async) {
			if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
			dispose = value[Symbol.asyncDispose];
		}
		if (dispose === void 0) {
			if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
			dispose = value[Symbol.dispose];
			if (async) inner = dispose;
		}
		if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources$1 = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
/** Host service backing `ctx.remote.skills` without activating a cold Agent. */
let SessionSkillCatalog = (() => {
	let _classSuper = TypertRemoteService;
	let _instanceExtraInitializers = [];
	let _list_decorators;
	return class SessionSkillCatalog extends _classSuper {
		static {
			const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
			_list_decorators = [Remote];
			__esDecorate$1(this, null, _list_decorators, {
				kind: "method",
				name: "list",
				static: false,
				private: false,
				access: {
					has: (obj) => "list" in obj,
					get: (obj) => obj.list
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
		static inject = [
			"agents",
			"sessionQuery",
			"typert",
			"workingDirectory"
		];
		/** @param ctx - Host context carrying Session reads and optional skill/preset services. */
		constructor(ctx) {
			super(ctx, "sessionSkillCatalog", { namespace: "skills" });
			__runInitializers$1(this, _instanceExtraInitializers);
		}
		/**
		* List the user-invocable skills visible to one Session composition.
		* @param request - Session identity whose cwd and preset select the catalog view.
		* @param signal - caller lifetime carried by the Remote transport; admitted catalog reads retain their existing completion semantics.
		* @returns user-invocable skill metadata without loading skill bodies.
		* @throws RemoteError when the Session cannot be inspected or no registry can serve it.
		*/
		async list(request, signal) {
			const env_1 = {
				stack: [],
				error: void 0,
				hasError: false
			};
			try {
				const { sessionId } = request;
				let cwd;
				let agentPreset;
				try {
					const env_2 = {
						stack: [],
						error: void 0,
						hasError: false
					};
					try {
						const observation = __addDisposableResource$1(env_2, await this.ctx.sessionQuery.observeSession(sessionId), false);
						if (observation.projections === void 0) throw new Error("skill catalog requires a projected Session observation");
						cwd = observation.projections.values.workingDirectory ?? observation.header.cwd ?? this.ctx.workingDirectory.defaultDirectory;
						agentPreset = observation.projections.values.agentPreset ?? void 0;
						const live = this.ctx.agents.get(sessionId);
						if (live !== void 0) cwd = await this.ctx.workingDirectory.ensure(live, signal);
					} catch (e_1) {
						env_2.error = e_1;
						env_2.hasError = true;
					} finally {
						__disposeResources$1(env_2);
					}
				} catch (error) {
					if (error instanceof SessionQueryError && error.code === "SESSION_QUERY_SESSION_NOT_FOUND") throw new RemoteError("session/not-found", `session "${sessionId}" not found`, { sessionId });
					throw new RemoteError("gateway/internal", `session "${sessionId}" could not be inspected: ${String(error)}`, {});
				}
				const live = this.ctx.agents.get(sessionId);
				const presets = this.ctx.get("agentPresets");
				const skillRegistry = (live === void 0 ? void 0 : presets?.serviceFor(live, "skills")) ?? this.ctx.get("skills");
				if (skillRegistry === void 0) throw new RemoteError("gateway/internal", "skill registry is absent: neither this session's agent preset nor the host composition mounts @deepseek-ai/dsh-skill", {});
				const lease = __addDisposableResource$1(env_1, live === void 0 ? await this.scopeFor(agentPreset) : void 0, true);
				const scope = live ?? lease?.key;
				try {
					return { skills: (await skillRegistry.list({
						cwd,
						scope
					})).filter(isUserInvocable).map((skill) => ({
						name: skill.name,
						...skill.path === void 0 ? {} : { path: skill.path },
						description: skill.description,
						...skill.whenToUse === void 0 ? {} : { whenToUse: skill.whenToUse },
						modelInvocable: skill.invocation.modelInvocable
					})) };
				} catch (error) {
					throw new RemoteError("gateway/internal", `skill listing failed: ${String(error)}`, {});
				}
			} catch (e_2) {
				env_1.error = e_2;
				env_1.hasError = true;
			} finally {
				const result_1 = __disposeResources$1(env_1);
				if (result_1) await result_1;
			}
		}
		/** Resolve a live or standing preset scope without creating an Agent. */
		async scopeFor(agentPreset) {
			const presets = this.ctx.get("agentPresets");
			if (presets === void 0) return void 0;
			try {
				return await presets.acquireScope(agentPreset);
			} catch {
				return;
			}
		}
	};
})();
//#endregion
//#region lib/types/media-references.js
/**
* Authenticated GET/HEAD /api/file reads bounded file responses through
* the composed filesystem provider. Paths and MIME types do not restrict access;
* the connection service authenticates requests before this handler. A request
* without the current `v` version is redirected, uncached, to the versioned URL,
* whose response is cacheable indefinitely.
* @module @deepseek-ai/dsh-api-session-controller/media-references
*/
const BASE_HEADERS = {
	"X-Content-Type-Options": "nosniff",
	"Content-Security-Policy": "sandbox; default-src 'none'"
};
const NO_STORE = {
	...BASE_HEADERS,
	"Cache-Control": "private, no-store"
};
const VERSIONED = {
	...BASE_HEADERS,
	"Cache-Control": "private, max-age=31536000, immutable"
};
async function serveFile(request, fs, maxBytes) {
	const fail = (status, text) => new Response(request.method === "HEAD" ? null : text, {
		status,
		headers: NO_STORE
	});
	const params = new URL(request.url).searchParams;
	const path = params.get("path");
	if (path === null || path.length === 0) return fail(400, "missing path");
	if (path.includes("\0") || !isAbsolute(path)) return fail(400, "absolute path required");
	try {
		const target = await fs.resolve(path, { signal: request.signal });
		const info = await fs.stat(target, request.signal);
		if (info === void 0) return fail(404, "not found");
		if (info.type !== "file") return fail(403, "not a regular file");
		if (info.size !== void 0 && info.size > maxBytes) return fail(413, "file exceeds byte limit");
		if (params.get("v") !== info.version) {
			const location = `?${new URLSearchParams({
				path,
				v: info.version
			}).toString()}`;
			return new Response(null, {
				status: 302,
				headers: {
					...NO_STORE,
					Location: location
				}
			});
		}
		const headers = {
			...VERSIONED,
			"Content-Type": mime.lookup(target.displayPath) || "application/octet-stream"
		};
		if (request.method === "HEAD") {
			if (info.size !== void 0) headers["Content-Length"] = String(info.size);
			return new Response(null, { headers });
		}
		const bytes = await fs.readBytes(target, request.signal, maxBytes);
		headers["Content-Length"] = String(bytes.byteLength);
		return new Response(bytes.slice(), { headers });
	} catch (error) {
		if (!(error instanceof FsError)) throw error;
		return fail({
			FS_NOT_FOUND: 404,
			FS_NOT_REGULAR_FILE: 403,
			FS_PERMISSION_DENIED: 403,
			FS_SANDBOX_DENIED: 403,
			FS_TOO_LARGE: 413,
			FS_ABORTED: 499
		}[error.code] ?? 500, error.code);
	}
}
/**
* File-display contribution. The connection service supplies authentication;
* `ctx.fs` supplies the execution world's paths, reads, and access policy.
*/
const SessionMediaReferences = {
	inject: [
		"connection",
		"fs",
		"attachments"
	],
	apply(ctx) {
		const maxBytes = ctx.attachments.imageLimits.maxImageBytes;
		ctx.effect(() => ctx.connection.fetch.register({
			path: "/api/file",
			methods: ["GET", "HEAD"],
			requestBody: "buffered",
			fetch: (request) => serveFile(request, ctx.fs, maxBytes)
		}), "session-controller: /api/file");
	}
};
//#endregion
//#region lib/types/archived-session-gate.js
/**
* The controller's admission gate for archived Sessions: an archived
* Session, or a subagent descendant of one, must not run a model step until
* it is restored. The work a Session still runs is reported and stopped by
* its owners — the Agent registry (`turn`), the job registry seam (`job`),
* the Subagent runtime (`subagent`), and the Schedule plugin (`schedule`) —
* through the Workspace registry's archive-admission events. The Subagent
* runtime also checks its retained ancestry before each step, so a released
* intermediate Session cannot hide an archived ancestor.
*/
/**
* The gate as a plugin for `ctx.plugin(...)`: it loads once the Agent
* registry, Session store, and Workspace registry are available and unwinds
* with the plugin's fiber. A late waking delivery to an archived Session — a
* subagent settlement, a queued follow-up — proposes a step the gate rejects,
* which the loop ends as `blocked` without a request; unarchiving lifts the
* gate for the whole lineage.
*/
const ArchivedSessionGate = {
	name: "archived-session-gate",
	inject: [
		"agents",
		"sessions",
		"workspaceRegistry"
	],
	apply(ctx) {
		ctx.on("agent/pre-step", (payload, next) => underArchivedSession(ctx, payload.agent) ? Promise.resolve({ kind: "reject" }) : next());
	}
};
/**
* Whether the Agent's Session, or a Session above it in its subagent lineage,
* is archived. This header-only check follows the durable fields through
* subagent-origin Sessions only: a fork of an archived Session is an
* independent conversation. The Subagent runtime separately checks managed
* ancestry when an intermediate Session is unavailable here.
* @param ctx - Host context.
* @param agent - the Agent proposing a step.
* @returns whether an archived Session owns the step.
*/
function underArchivedSession(ctx, agent) {
	const archived = ctx.workspaceRegistry.archivedSessionIds;
	let header = agent.session.header;
	const visited = /* @__PURE__ */ new Set();
	while (!visited.has(header.id)) {
		if (archived.includes(header.id)) return true;
		visited.add(header.id);
		if (header.origin !== "subagent" || header.parentSession === void 0) return false;
		const parent = ctx.sessions.get(header.parentSession);
		if (parent === void 0) return archived.includes(header.parentSession);
		header = parent.header;
	}
	return false;
}
//#endregion
//#region lib/types/index.js
/** Session Remote owner: cold reads, explicit Agent commands, and live control state. */
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
var __addDisposableResource = function(env, value, async) {
	if (value !== null && value !== void 0) {
		if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
		var dispose, inner;
		if (async) {
			if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
			dispose = value[Symbol.asyncDispose];
		}
		if (dispose === void 0) {
			if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
			dispose = value[Symbol.dispose];
			if (async) inner = dispose;
		}
		if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
/**
* Extract the Session resource from an ID or a history address.
*
* Registers as the `session` kind default, so it applies to every Remote method of
* every namespace: a top-level `sessionId` parameter, otherwise the first object
* parameter carrying `sessionId` or an `address`. A new request type whose first
* object parameter has an unrelated `address` field would be read as an address.
* @param input - validated wire arguments and their ordered parameter descriptions.
* @returns the named Session identity, otherwise the first object's Session identity or history address.
* @throws Error when the request object supplies both sessionId and address.
*/
function resolveSessionResource({ args, parameters }) {
	const sessionParameter = parameters.find((parameter) => parameter.name === "sessionId" || parameter.wire === "sessionId");
	if (sessionParameter !== void 0) return args[sessionParameter.wire];
	for (const parameter of parameters) {
		const value = args[parameter.wire];
		if (typeof value !== "object" || value === null || Array.isArray(value)) continue;
		const request = value;
		const address = request.address;
		if (request.sessionId !== void 0 && address !== void 0) throw new Error("Session resource request must not contain both sessionId and address");
		return request.sessionId ?? (address === void 0 ? void 0 : address.kind === "session" ? address.sessionId : address.childSessionId);
	}
}
/** Host service backing the generated `ctx.remote.session` namespace. */
let SessionController = (() => {
	let _classSuper = TypertRemoteService;
	let _instanceExtraInitializers = [];
	let _list_decorators;
	let _search_decorators;
	let _create_decorators;
	let _selectModel_decorators;
	let _initializeDefaultModel_decorators;
	let _modelCatalog_decorators;
	let _canOpenWorkspacePath_decorators;
	let _openWorkspacePath_decorators;
	let _workspacePathApplications_decorators;
	let _rename_decorators;
	let _fork_decorators;
	let _prompt_decorators;
	let _attachment_decorators;
	let _updateQueue_decorators;
	let _cancel_decorators;
	let _page_decorators;
	let _events_decorators;
	let _observeEvent_decorators;
	let _observeProjections_decorators;
	let _state_decorators;
	let _observeState_decorators;
	let _wait_decorators;
	let _follow_decorators;
	let _projections_decorators;
	let _control_decorators;
	return class SessionController extends _classSuper {
		static {
			const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
			_list_decorators = [Remote("list")];
			_search_decorators = [Remote("search")];
			_create_decorators = [Remote("create")];
			_selectModel_decorators = [Remote("selectModel")];
			_initializeDefaultModel_decorators = [Remote];
			_modelCatalog_decorators = [Remote("modelCatalog")];
			_canOpenWorkspacePath_decorators = [Remote];
			_openWorkspacePath_decorators = [Remote("openWorkspacePath")];
			_workspacePathApplications_decorators = [Remote("workspacePathApplications")];
			_rename_decorators = [Remote("rename")];
			_fork_decorators = [Remote("fork")];
			_prompt_decorators = [Remote("prompt")];
			_attachment_decorators = [Remote("attachment")];
			_updateQueue_decorators = [Remote("updateQueue")];
			_cancel_decorators = [Remote("cancel")];
			_page_decorators = [Remote("page")];
			_events_decorators = [Remote];
			_observeEvent_decorators = [Remote({ mode: "stream" })];
			_observeProjections_decorators = [Remote({ mode: "stream" })];
			_state_decorators = [Remote];
			_observeState_decorators = [Remote({ mode: "stream" })];
			_wait_decorators = [Remote];
			_follow_decorators = [Remote({ mode: "stream" })];
			_projections_decorators = [Remote("projections")];
			_control_decorators = [Remote({ mode: "stream" })];
			__esDecorate(this, null, _list_decorators, {
				kind: "method",
				name: "list",
				static: false,
				private: false,
				access: {
					has: (obj) => "list" in obj,
					get: (obj) => obj.list
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _search_decorators, {
				kind: "method",
				name: "search",
				static: false,
				private: false,
				access: {
					has: (obj) => "search" in obj,
					get: (obj) => obj.search
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _create_decorators, {
				kind: "method",
				name: "create",
				static: false,
				private: false,
				access: {
					has: (obj) => "create" in obj,
					get: (obj) => obj.create
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _selectModel_decorators, {
				kind: "method",
				name: "selectModel",
				static: false,
				private: false,
				access: {
					has: (obj) => "selectModel" in obj,
					get: (obj) => obj.selectModel
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _initializeDefaultModel_decorators, {
				kind: "method",
				name: "initializeDefaultModel",
				static: false,
				private: false,
				access: {
					has: (obj) => "initializeDefaultModel" in obj,
					get: (obj) => obj.initializeDefaultModel
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _modelCatalog_decorators, {
				kind: "method",
				name: "modelCatalog",
				static: false,
				private: false,
				access: {
					has: (obj) => "modelCatalog" in obj,
					get: (obj) => obj.modelCatalog
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _canOpenWorkspacePath_decorators, {
				kind: "method",
				name: "canOpenWorkspacePath",
				static: false,
				private: false,
				access: {
					has: (obj) => "canOpenWorkspacePath" in obj,
					get: (obj) => obj.canOpenWorkspacePath
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _openWorkspacePath_decorators, {
				kind: "method",
				name: "openWorkspacePath",
				static: false,
				private: false,
				access: {
					has: (obj) => "openWorkspacePath" in obj,
					get: (obj) => obj.openWorkspacePath
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _workspacePathApplications_decorators, {
				kind: "method",
				name: "workspacePathApplications",
				static: false,
				private: false,
				access: {
					has: (obj) => "workspacePathApplications" in obj,
					get: (obj) => obj.workspacePathApplications
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _rename_decorators, {
				kind: "method",
				name: "rename",
				static: false,
				private: false,
				access: {
					has: (obj) => "rename" in obj,
					get: (obj) => obj.rename
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _fork_decorators, {
				kind: "method",
				name: "fork",
				static: false,
				private: false,
				access: {
					has: (obj) => "fork" in obj,
					get: (obj) => obj.fork
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _prompt_decorators, {
				kind: "method",
				name: "prompt",
				static: false,
				private: false,
				access: {
					has: (obj) => "prompt" in obj,
					get: (obj) => obj.prompt
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _attachment_decorators, {
				kind: "method",
				name: "attachment",
				static: false,
				private: false,
				access: {
					has: (obj) => "attachment" in obj,
					get: (obj) => obj.attachment
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _updateQueue_decorators, {
				kind: "method",
				name: "updateQueue",
				static: false,
				private: false,
				access: {
					has: (obj) => "updateQueue" in obj,
					get: (obj) => obj.updateQueue
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _cancel_decorators, {
				kind: "method",
				name: "cancel",
				static: false,
				private: false,
				access: {
					has: (obj) => "cancel" in obj,
					get: (obj) => obj.cancel
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _page_decorators, {
				kind: "method",
				name: "page",
				static: false,
				private: false,
				access: {
					has: (obj) => "page" in obj,
					get: (obj) => obj.page
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _events_decorators, {
				kind: "method",
				name: "events",
				static: false,
				private: false,
				access: {
					has: (obj) => "events" in obj,
					get: (obj) => obj.events
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _observeEvent_decorators, {
				kind: "method",
				name: "observeEvent",
				static: false,
				private: false,
				access: {
					has: (obj) => "observeEvent" in obj,
					get: (obj) => obj.observeEvent
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _observeProjections_decorators, {
				kind: "method",
				name: "observeProjections",
				static: false,
				private: false,
				access: {
					has: (obj) => "observeProjections" in obj,
					get: (obj) => obj.observeProjections
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _state_decorators, {
				kind: "method",
				name: "state",
				static: false,
				private: false,
				access: {
					has: (obj) => "state" in obj,
					get: (obj) => obj.state
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _observeState_decorators, {
				kind: "method",
				name: "observeState",
				static: false,
				private: false,
				access: {
					has: (obj) => "observeState" in obj,
					get: (obj) => obj.observeState
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _wait_decorators, {
				kind: "method",
				name: "wait",
				static: false,
				private: false,
				access: {
					has: (obj) => "wait" in obj,
					get: (obj) => obj.wait
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _follow_decorators, {
				kind: "method",
				name: "follow",
				static: false,
				private: false,
				access: {
					has: (obj) => "follow" in obj,
					get: (obj) => obj.follow
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _projections_decorators, {
				kind: "method",
				name: "projections",
				static: false,
				private: false,
				access: {
					has: (obj) => "projections" in obj,
					get: (obj) => obj.projections
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _control_decorators, {
				kind: "method",
				name: "control",
				static: false,
				private: false,
				access: {
					has: (obj) => "control" in obj,
					get: (obj) => obj.control
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
		static inject = [
			"agentDefaultModel",
			"agents",
			"attachments",
			"fileUploads",
			"fs",
			"llm",
			"remoteAccess",
			"sessions",
			"sessionProjections",
			"sessionQuery",
			"typert",
			"typertGateway",
			"workspaceRegistry"
		];
		static Config = z.object({
			nativeOpen: z.boolean(),
			listWorkSliceMs: z.natural().min(1).default(16)
		});
		historyController = __runInitializers(this, _instanceExtraInitializers);
		stateController;
		webController;
		agents;
		commands;
		controlState;
		listState;
		openPath;
		fileApplications;
		openFileApplication;
		revealPath;
		canOpenPath;
		listWorkSliceMs;
		promotions = /* @__PURE__ */ new Set();
		/**
		* @param ctx - Host context containing the Session capability assembly.
		* @param config - native-opener and list-scheduling deployment policy.
		* @param internals - host integrations replaceable by direct unit tests.
		*/
		constructor(ctx, config, internals = {}) {
			super(ctx, "sessionController", { namespace: "session" });
			this.listWorkSliceMs = SessionController.Config(config).listWorkSliceMs;
			this.fileApplications = internals.fileApplications ?? nativeFileApplications;
			this.openFileApplication = internals.openFileApplication ?? openNativeFileApplication;
			this.openPath = internals.openPath ?? openNativeAssociatedPath;
			this.revealPath = internals.revealPath ?? revealNativePath;
			this.canOpenPath = internals.canOpenPath ?? (() => config.nativeOpen ?? (internals.openPath !== void 0 || canOpenNativePath()));
		}
		async [Service.init]() {
			const ctx = this.ctx;
			ctx.typert.resources.register("session", { resolve: resolveSessionResource });
			this.agents = new ApiSessionAgentController(ctx);
			this.agents.initialize();
			this.commands = new SessionCommandController(ctx, this.agents, process.cwd());
			ctx.effect(() => ctx.fileUploads.registerAgentResolver(async (sessionId) => {
				const result = await this.agents.resolveAgent(sessionId);
				if ("error" in result) throw result.error;
				return result.agent;
			}), "session-controller: file-upload Agent resolver");
			this.controlState = new SessionControlController(ctx);
			ctx.effect(() => async () => {
				await Promise.allSettled([...this.promotions]);
			}, "session-controller.promotions");
			this.historyController = new HistoryController(ctx);
			this.stateController = new StateController(ctx, this.historyController);
			this.webController = new WebController(ctx, this.historyController, (observation) => {
				this.promote(observation);
			});
			this.listState = new ApiSessionList(ctx, this.listWorkSliceMs);
			await this.commands.initialize();
			ctx.fiber.assertActive();
			await this.listState.initialize();
			ctx.fiber.assertActive();
			ctx.plugin(SessionFileReferences);
			ctx.plugin(SessionMediaReferences);
			ctx.plugin(SessionSkillCatalog);
			ctx.plugin(ArchivedSessionGate);
			ctx.on("session/created", (session) => {
				ctx.emit("api-session/added", this.listState.summaryFor(session));
			});
			ctx.on("session/disposed", (session) => {
				ctx.emit("api-session/removed", session.id);
			});
			const publishAgentAvailability = ({ agent }) => {
				if (ctx.sessions.get(agent.id) === agent.session) ctx.emit("api-session/added", this.listState.summaryFor(agent.session));
			};
			ctx.on("agent/created", publishAgentAvailability);
			ctx.on("agent/disposed", publishAgentAvailability);
			ctx.on("agent/status", ({ agent, status }) => {
				ctx.emit("api-session/status", agent.id, status === "running");
			});
			ctx.on("agent/error", ({ agent, error }) => {
				ctx.emit("api-session/error", agent.id, errorChain(error));
			});
			ctx.on("session/event", (session, event) => {
				if (event.type !== "user/message" || event.data.source.kind !== "user") return;
				ctx.emit("api-session/activity", session.id, event.time);
			});
		}
		/**
		* Request background activation through the existing Session Agent controller.
		* @param observation - owned lease disposed by the background task; callers retain their own lease separately.
		*/
		promote(observation) {
			const sessionId = observation.header.id;
			const task = (async () => {
				const env_1 = {
					stack: [],
					error: void 0,
					hasError: false
				};
				try {
					const ownedObservation = __addDisposableResource(env_1, observation, false);
					const result = await this.agents.resolveObservedAgent(ownedObservation);
					if ("error" in result) this.ctx.emit("api-session/error", sessionId, result.error.message);
				} catch (e_1) {
					env_1.error = e_1;
					env_1.hasError = true;
				} finally {
					__disposeResources(env_1);
				}
			})().catch((error) => {
				this.ctx.logger.error(`session-controller: background activation for "${sessionId}" failed: ${errorChain(error)}`);
			});
			this.promotions.add(task);
			task.finally(() => {
				this.promotions.delete(task);
			});
		}
		/**
		* Resolve or resume one ordinary Session for another Host API domain.
		* @param sessionId - Session identity whose Agent owns the operation.
		* @returns the live Agent or the stable Session-domain failure.
		*/
		resolveAgent(sessionId) {
			return this.agents.resolveAgent(sessionId);
		}
		/**
		* Read all visible Session rows without resuming an Agent.
		* @param _request - reserved empty list request.
		* @param signal - cancellation for persistence reads and summary generation.
		* @returns visible Session summaries ordered by activity.
		*/
		async list(_request, signal) {
			return { items: (await this.listState.list(signal)).filter((item) => this.ctx.remoteAccess.allows({
				kind: "session",
				id: item.sessionId
			})) };
		}
		/**
		* Search visible Session content without resuming an Agent. A non-owner
		* receives only the hits whose Session `allows()` admits; `hasMore`
		* describes the bounded result before that filtering.
		* @param request - literal message-content query.
		* @param signal - cancellation for list and search reads.
		* @returns authorized bounded Session search results.
		*/
		async search(request, signal) {
			const result = await this.listState.search(request.query, signal);
			return {
				...result,
				items: result.items.filter((item) => this.ctx.remoteAccess.allows({
					kind: "session",
					id: item.sessionId
				}))
			};
		}
		/**
		* Create one ordinary Session for a new conversation, reusing the named blank Session while it is reusable.
		* @param request - optional blank Session to reuse, location, and Agent preset.
		* @returns the created or reused Session identity and its preset when configured.
		*/
		create(request) {
			return this.commands.create(request);
		}
		/**
		* Select one Session-local model after explicitly resuming the Session; save the default in the background.
		* @param request - Session identity and requested model selection.
		* @returns the normalized selection installed for the Session, without waiting for default persistence.
		*/
		selectModel(request) {
			return this.commands.selectModel(request);
		}
		/**
		* Select the first available account model after login when no provider API key is configured.
		* @returns after saving the first available model or retaining the existing default.
		*/
		async initializeDefaultModel() {
			const provider = "deepseek-account";
			if (await hasProviderApiKey(this.ctx)) return;
			const model = (await buildModelCatalog(this.ctx)).groups.find((group) => group.id === provider)?.models[0];
			if (model === void 0) throw new RemoteError("session/provider-models-unavailable", `provider "${provider}" has no available models`, { provider });
			const selection = {
				provider,
				model: model.id,
				...model.reasoning?.defaultEffort === void 0 ? {} : { reasoningEffort: ReasoningEffortId(model.reasoning.defaultEffort) }
			};
			await this.ctx.agentDefaultModel.saveSelection(selection);
		}
		/**
		* Describe every currently routable model for Host-generation selectors.
		* @returns provider-grouped models, the deployment default, and isolated provider failures.
		*/
		modelCatalog() {
			return buildModelCatalog(this.ctx);
		}
		/**
		* Report whether this deployment can hand a Session workspace path to a native desktop.
		* @returns true when the matching open operation is available.
		*/
		canOpenWorkspacePath() {
			return this.canOpenPath();
		}
		/**
		* Describe the serving desktop for authenticated file-action routes.
		* @returns Host name, configured availability, and platform-specific file-manager behavior.
		*/
		workspaceDesktop() {
			const fileManager = nativeFileManager();
			return {
				name: hostname(),
				available: fileManager !== null && this.canOpenPath(),
				fileManager
			};
		}
		/**
		* Verify one path through the composed filesystem and open it on the Host desktop.
		* @param request - path after best-effort Session workspace resolution.
		* @param signal - caller lifetime; abort terminates the native command.
		* @returns confirmation after the native opener accepts the path.
		* @throws RemoteError when the request is invalid, has no verified Host mapping, is cancelled, or the opener fails.
		*/
		async openWorkspacePath(request, signal) {
			try {
				const path = await this.verifyDesktopPath(request.path, signal);
				if (request.action === "reveal") await this.revealPath(path, signal);
				else if (request.application !== void 0) await this.openFileApplication(path, request.application, signal);
				else await this.openPath(path, signal);
				return { opened: true };
			} catch (error) {
				if (signal.aborted) throw new RemoteError("gateway/cancelled", "path open was aborted", {});
				if (error instanceof RemoteError) throw error;
				throw new RemoteError("gateway/internal", "path open failed", {}, { cause: error });
			}
		}
		/**
		* Query current file handlers on the serving desktop without activating an Agent.
		* @param request - file path in Host filesystem syntax.
		* @param signal - caller lifetime, propagated to filesystem and desktop queries.
		* @returns OS application names, icons, and default selection; empty when desktop opening is unavailable.
		* @throws RemoteError when the path is invalid, the query is cancelled, or native discovery fails.
		*/
		async workspacePathApplications(request, signal) {
			if (!this.canOpenPath()) return [];
			try {
				const path = await this.verifyDesktopPath(request.path, signal);
				return await this.fileApplications(path, signal);
			} catch (error) {
				if (signal.aborted) throw new RemoteError("gateway/cancelled", "application query was aborted", {});
				if (error instanceof RemoteError) throw error;
				throw new RemoteError("gateway/internal", "file application query failed", {}, { cause: error });
			}
		}
		async verifyDesktopPath(path, signal) {
			if (path.length === 0) throw new RemoteError("gateway/bad-request", "A non-empty file path is required", {});
			signal.throwIfAborted();
			const hostPath = resolve(path);
			const { fs } = this.ctx;
			const mapped = fs.processPathFromHostPath(hostPath);
			if (mapped === void 0 || fs.processPath(await fs.resolve(mapped, { signal })) !== hostPath) throw new RemoteError("gateway/bad-request", "Path has no verified Host path", {});
			signal.throwIfAborted();
			return hostPath;
		}
		/**
		* Rename one Session after explicitly resuming it.
		* @param request - Session identity and proposed title.
		* @returns the accepted title and durable event sequence.
		*/
		rename(request) {
			return this.commands.rename(request);
		}
		/**
		* Fork one cold-readable exact event prefix into a new Session. An omitted
		* boundary selects the latest completed-turn prefix; an open cut receives
		* synthetic fork closers.
		* @param request - source Session, optional exact inclusive event boundary, and migration preflight choice.
		* @returns the new Session identity.
		*/
		async fork(request) {
			if (request.allowMigration === false && this.ctx.sessions.get(request.sessionId) === void 0) {
				if ((await this.ctx.get("sessionPersistence")?.stat(request.sessionId))?.formatStatus === "migration-required" && this.ctx.sessions.get(request.sessionId) === void 0) throw new RemoteError("session/migration-required", "Open the source Session to complete migration before forking", { sessionId: request.sessionId });
			}
			return this.commands.fork(request);
		}
		/**
		* Admit one prompt after explicitly resuming its Session.
		* @param request - Session identity, prompt content, source metadata, and delivery mode.
		* @param signal - caller cancellation before prompt admission begins.
		* @returns acknowledgement that the Agent accepted the prompt.
		*/
		async prompt(request, signal) {
			signal.throwIfAborted();
			if (request.content.length > 0 && !hasPromptContent(request.content)) throw new RemoteError("gateway/bad-request", "prompt content must include non-whitespace text or an attachment", {});
			const clientTimeZone = request.clientTimeZone === void 0 ? void 0 : canonicalClientTimeZone(request.clientTimeZone);
			if (request.clientTimeZone !== void 0 && clientTimeZone === void 0) throw new RemoteError("session/invalid-time-zone", "clientTimeZone must be UTC or a valid IANA Area/Location name", { value: request.clientTimeZone });
			const principal = principalOf(this.ctx.invocation);
			const source = {
				kind: request.content.length === 0 ? "retry" : "user",
				rpcId: request.requestId,
				...clientTimeZone === void 0 ? {} : { clientTimeZone },
				...principal === void 0 ? {} : { principal }
			};
			return this.commands.prompt(request, source);
		}
		/**
		* Read one image from the attachment store after gateway authorization.
		* @param request - image reference and gateway permission scope; Session existence is not checked.
		* @returns the durable attachment reference and base64-encoded bytes.
		*/
		attachment(request) {
			return this.commands.attachment(request);
		}
		/**
		* Mutate one still-pending queue occurrence, resuming a cold Agent first.
		* @param request - Session, queue item, and requested mutation.
		* @returns acknowledgement that the queue mutation was applied.
		*/
		updateQueue(request) {
			return this.commands.updateQueue(request);
		}
		/**
		* Cancel one active Agent turn without dropping its pending inbox.
		* @param request - Session whose active Agent turn is cancelled.
		* @returns acknowledgement that cancellation was requested.
		*/
		cancel(request) {
			return this.commands.cancel(request);
		}
		/**
		* Read one cold-safe, message-aligned Session history page.
		* @deprecated Use `events`; this entry point remains for existing Web callers.
		* @param request - durable address, optional fixed cut, backward cursor, and page budget.
		* @param signal - cancellation for persistence reads.
		* @returns one chronological page with its inclusive cursor; an omitted `throughSeq` selects the observed log end.
		*/
		page(request, signal) {
			return this.historyController.events(request, signal);
		}
		/**
		* Read one cold-safe, message-aligned Session history page.
		* @param request - durable address, optional fixed cut, backward cursor, and page budget.
		* @param signal - cancellation for persistence reads.
		* @returns one chronological page with its inclusive cursor.
		*/
		events(request, signal) {
			return this.historyController.events(request, signal);
		}
		/**
		* Subscribe to Session events without reading a snapshot.
		* @param request - Session address.
		* @param signal - subscription cancellation.
		* @returns readiness and subsequent events, without a snapshot.
		*/
		observeEvent(request, signal) {
			return this.historyController.observeEvent(request, signal);
		}
		/**
		* Subscribe to Session projection changes.
		* @param request - target Session.
		* @param signal - subscription cancellation.
		* @returns readiness followed by one projection change frame per committed event.
		*/
		observeProjections(request, signal) {
			return this.historyController.observeProjections(request, signal);
		}
		/**
		* Read passive Session state.
		* @param request - target Session.
		* @param signal - read cancellation.
		* @returns current passive state.
		*/
		state(request, signal) {
			return this.stateController.state(request, signal);
		}
		/**
		* Subscribe to complete Session state replacements.
		* @param request - target Session.
		* @param signal - subscription cancellation.
		* @returns complete passive state replacements.
		*/
		observeState(request, signal) {
			return this.stateController.observeState(request, signal);
		}
		/**
		* Wait for Session quiescence or pending input with bounded assistant output.
		* @param request - Session, wait bounds and output window.
		* @param signal - caller cancellation.
		* @returns stopping state and bounded assistant output.
		*/
		wait(request, signal) {
			return this.stateController.wait(request, signal);
		}
		/**
		* Follow one Session log from its opening or resume cursor.
		* @param request - durable address and last committed sequence already held by the caller.
		* @param signal - cancellation owned by the Remote stream carrier.
		* @returns a complete opening snapshot followed by gap-free durable event
		*   frames and optional cursorless assistant-stream frames.
		*/
		follow(request, signal) {
			return this.webController.follow(request, signal);
		}
		/**
		* Read exact projections, returning cached hints only when migration is required.
		* @param request - Session whose current values are required.
		* @param signal - cancellation for the Session read.
		* @returns a sequenced baseline, cached hints when migration is deferred, or null when absent.
		*/
		projections(request, signal) {
			return this.historyController.projections(request, signal);
		}
		/**
		* Stream a complete live-control baseline followed by replacement frames.
		* @param signal - cancellation owned by the Remote stream carrier.
		* @returns one complete baseline followed by live replacement frames.
		*/
		async *control(signal) {
			for await (const frame of this.controlState.control(signal)) switch (frame.type) {
				case "baseline": {
					const projections = Object.fromEntries(Object.entries(frame.value.projections).filter(([sessionId]) => this.ctx.remoteAccess.allows({
						kind: "session",
						id: sessionId
					})));
					yield {
						type: "baseline",
						value: {
							...frame.value,
							projections
						}
					};
					break;
				}
				case "projection":
					if (this.ctx.remoteAccess.allows({
						kind: "session",
						id: frame.sessionId
					})) yield frame;
					break;
				/* v8 ignore next -- closed control-frame union. */
				default: assertNever(frame);
			}
		}
	};
})();
//#endregion
export { ApiSessionNotFound, SessionController, SessionController as default, SessionFileReferences, SessionSkillCatalog };
