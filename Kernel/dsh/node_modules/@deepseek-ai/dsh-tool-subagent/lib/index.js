import z from "@deepseek-ai/schemastery";
import { defineTool } from "@deepseek-ai/dsh-tools";
import { assertSubagentMaxDepth, delegationDepthOf, parentAgentOptionsForDelegation } from "@deepseek-ai/dsh-subagent";
import { ReasoningEffortId } from "@deepseek-ai/dsh-llm";
import { z as z$1 } from "zod";
z.object({
	provider: z.string().min(1).required(),
	model: z.string().min(1).required()
});
/**
* Stable identity for one provider/model pair.
* @param route - Exact provider/model route.
* @returns Opaque key for equality checks.
*/
function modelRouteKey(route) {
	return `${route.provider}\0${route.model}`;
}
/**
* Reject malformed or duplicate route policy entries at a durable or configuration boundary.
* @param routes - Candidate exact routes to validate.
* @returns an assertion that the candidate is a validated exact-route array.
*/
function assertAllowedModelRoutes(routes) {
	if (!Array.isArray(routes)) throw new Error("subagent model selection requires an array of routes");
	const seen = /* @__PURE__ */ new Set();
	const candidates = routes;
	for (const candidate of candidates) {
		if (typeof candidate !== "object" || candidate === null || Array.isArray(candidate) || !("provider" in candidate) || typeof candidate.provider !== "string" || !("model" in candidate) || typeof candidate.model !== "string" || candidate.provider.length === 0 || candidate.model.length === 0) throw new Error("subagent model selection requires non-empty provider and model ids");
		const route = {
			provider: candidate.provider,
			model: candidate.model
		};
		const key = modelRouteKey(route);
		if (seen.has(key)) throw new Error(`subagent model selection repeats route "${route.provider}/${route.model}"`);
		seen.add(key);
	}
}
/**
* Whether a call explicitly selects any child LLM value.
* @param request - Model-facing route fields from the tool call.
* @returns Whether at least one route or effort field is present.
*/
function hasDelegationModelRequest(request) {
	return request.provider !== void 0 || request.model !== void 0 || request.reasoning_effort !== void 0;
}
/** Reject an empty model-facing route value at the tool JSON boundary. */
function assertNonEmpty(value, field) {
	if (value !== void 0 && value.length === 0) throw new Error(`child LLM \`${field}\` must be non-empty`);
}
/**
* Merge model-supplied selection fields over configured child defaults.
* Provider and model form one route and must be supplied together. Changing
* that route without an effort clears the configured route-owned effort.
* @param parentOptions - Current parent values that supply missing child values.
* @param configured - Tool-instance child defaults.
* @param request - Model-facing route override.
* @param enabled - Whether this tool instance permits model-facing selection.
* @returns Child Agent options, preserving omission when no layer contributes one.
*/
function requestedAgentOptions(parentOptions, configured, request, enabled) {
	if (!hasDelegationModelRequest(request)) return configured;
	if (!enabled) throw new Error("child model selection is disabled for this tool instance");
	assertNonEmpty(request.provider, "provider");
	assertNonEmpty(request.model, "model");
	assertNonEmpty(request.reasoning_effort, "reasoning_effort");
	if (request.provider === void 0 !== (request.model === void 0)) throw new Error("child LLM `provider` and `model` must be supplied together");
	const baselineProvider = configured?.provider ?? parentOptions.provider;
	const baselineModel = configured?.model ?? parentOptions.model;
	const routeChanged = request.provider !== void 0 && (request.provider !== baselineProvider || request.model !== baselineModel);
	const { reasoningEffort: _configuredReasoningEffort, ...configuredWithoutReasoning } = configured ?? {};
	return {
		...routeChanged && request.reasoning_effort === void 0 ? configuredWithoutReasoning : configured,
		...request.provider === void 0 ? {} : {
			provider: request.provider,
			model: request.model
		},
		...request.reasoning_effort === void 0 ? {} : { reasoningEffort: ReasoningEffortId(request.reasoning_effort) }
	};
}
/**
* Enforce a settings-owned route list at the operation that creates the child.
* Pure inheritance remains outside this policy because no model-facing choice
* occurred; any explicit route or effort field must resolve to an allowed route.
* @param policy - Session allowlist; omission permits only the current parent route.
* @param parentOptions - Current parent values that supply missing child values.
* @param requested - Effective child options after request/config merging.
* @param request - Model-facing selection fields from the tool call.
*/
function assertAllowedModelSelection(policy, parentOptions, requested, request) {
	if (!hasDelegationModelRequest(request)) return;
	const provider = requested?.provider ?? parentOptions.provider;
	const model = requested?.model ?? parentOptions.model;
	if (provider === void 0 || model === void 0) throw new Error("cannot select child LLM values without an effective provider and model");
	if (policy === void 0 ? provider === parentOptions.provider && model === parentOptions.model : policy.routes.some((route) => route.provider === provider && route.model === model)) return;
	throw new Error(`child LLM route "${provider}/${model}" is not allowed for this Session` + (policy === void 0 ? "; without a recorded model-selection policy, explicit selection (including reasoning_effort) is limited to the parent route" : ""));
}
/**
* Whether configured Agent options require route validation before delegation.
* @param options - Tool-instance child defaults.
* @returns Whether configured provider, model, or effort values must be resolved.
*/
function hasConfiguredLlmSelection(options) {
	return options?.provider !== void 0 || options?.model !== void 0 || options?.reasoningEffort !== void 0;
}
/**
* Resolve an effective child route through its live adapter before the child is
* created. The LLM runtime owns provider lookup, exact-model metadata, effort
* validation, and adapter defaults.
* @param llm - Live LLM runtime.
* @param parentOptions - Current parent values whose compatible fields the child inherits.
* @param requested - Per-child options after request/config merging.
* @param signal - Tool-call cancellation signal.
* @param inheritParentReasoningEffort - Whether an omitted effort may inherit from the parent route.
*/
async function preflightChildLlmRoute(llm, parentOptions, requested, signal, inheritParentReasoningEffort = true) {
	const provider = requested?.provider ?? parentOptions.provider;
	const model = requested?.model ?? parentOptions.model;
	if (provider === void 0 || model === void 0) throw new Error("cannot select child LLM values without an effective provider and model");
	const routeChanged = provider !== parentOptions.provider || model !== parentOptions.model;
	const reasoningEffort = requested?.reasoningEffort ?? (inheritParentReasoningEffort && !routeChanged ? parentOptions.reasoningEffort : void 0);
	await llm.resolveCallConfig({
		provider,
		model,
		...reasoningEffort === void 0 ? {} : { reasoningEffort }
	}, signal);
}
/**
* Host-only projection of the durable model-selection policy.
* Each independently mounted consumer registers this shared definition; the registry reference-counts it.
*/
const subagentModelSelectionProjectionDefinition = {
	key: "subagentModelSelectionPolicy",
	stateVersion: 1,
	stateSchema: z$1.array(z$1.object({
		provider: z$1.string().min(1),
		model: z$1.string().min(1)
	}).strict()).min(1).nullable(),
	init: () => null,
	apply: (policy, event) => {
		if (policy !== null || event.type !== "subagent/model-selection-policy") return policy;
		const { allowedModels } = event.data;
		assertAllowedModelRoutes(allowedModels);
		if (allowedModels.length === 0) throw new Error("subagent/model-selection-policy requires at least one route");
		return allowedModels;
	}
};
/**
* Read the exact route list authorized for a Session.
* @param subagentModelSelectionProjectionHandle - registered policy state reader.
* @param session - session whose durable decision is read.
* @returns a detached route list, or undefined to restrict selection to the parent route.
*/
function subagentModelSelectionPolicy(subagentModelSelectionProjectionHandle, session) {
	return subagentModelSelectionProjectionHandle.stateOf(session)?.map((route) => ({ ...route }));
}
//#endregion
//#region lib/types/list-models.js
/** Model-facing discovery of LLM routes available to child Agents. */
/** Resolve one registered provider with a model-correctable diagnostic. */
function registeredProvider(llm, policy, providerId) {
	const providers = llm.listProviders();
	const provider = providers.find((candidate) => candidate.id === providerId);
	if (provider !== void 0) return provider;
	const available = providers.filter((candidate) => policy.routes.some((route) => route.provider === candidate.id)).map((candidate) => candidate.id).join(", ") || "(none)";
	throw new Error(`LLM provider "${providerId}" is not registered; available providers: ${available}`);
}
/** Render one advertised or resolved model. */
function modelLine(provider, model) {
	return `${provider}/${model.id} — ${model.name}${model.description === void 0 ? "" : `: ${model.description}`}`;
}
/** Read the requested provider, allowed models, or exact-model efforts. */
async function listSubagentModels(ctx, policyHandle, request, exec) {
	if (exec.agent === void 0) throw new Error("list_subagent_models requires a calling agent");
	const routes = subagentModelSelectionPolicy(policyHandle, exec.agent.session);
	let policy = routes === void 0 ? void 0 : { routes };
	const selectionEnabled = policy !== void 0;
	if (policy === void 0) {
		const { provider, model } = parentAgentOptionsForDelegation(exec.agent);
		if (provider === void 0 || model === void 0) throw new Error("cannot discover child LLM routes without a parent provider and model");
		policy = { routes: [{
			provider,
			model
		}] };
	}
	const { signal } = exec;
	const llm = ctx.get("llm");
	if (llm === void 0) throw new Error("cannot discover child LLM routes because the `llm` service is unavailable");
	if (request.model !== void 0 && request.provider === void 0) throw new Error("`model` requires `provider`");
	if (request.provider === void 0) {
		const providers = llm.listProviders().filter((provider) => policy.routes.some((route) => route.provider === provider.id));
		return providers.length === 0 ? "(no LLM providers)" : providers.map((provider) => `${provider.id} — ${provider.name}`).join("\n");
	}
	if (request.provider.length === 0) throw new Error("`provider` must be non-empty");
	const allowedRoutes = policy.routes.filter((route) => route.provider === request.provider);
	const [firstRoute] = allowedRoutes;
	if (firstRoute === void 0) throw new Error(`LLM provider "${request.provider}" is not allowed for this Session`);
	const provider = registeredProvider(llm, policy, request.provider);
	if (request.model === void 0) {
		const models = selectionEnabled ? (await llm.listModels(provider.id)).filter((model) => allowedRoutes.some((route) => route.model === model.id)) : [await llm.resolveModelInfo(provider.id, firstRoute.model, signal)];
		signal.throwIfAborted();
		return models.length === 0 ? `(no advertised models for ${provider.id})` : models.map((model) => modelLine(provider.id, model)).join("\n");
	}
	if (request.model.length === 0) throw new Error("`model` must be non-empty");
	if (!allowedRoutes.some((route) => route.model === request.model)) throw new Error(`child LLM route "${provider.id}/${request.model}" is not allowed for this Session`);
	const model = await llm.resolveModelInfo(provider.id, request.model, signal);
	const efforts = model.reasoning?.efforts.map((effort) => `${effort.id}${model.reasoning?.defaultEffort === effort.id ? " (default)" : ""} — ${effort.name}` + (effort.description === void 0 ? "" : `: ${effort.description}`)).join("\n") || "(no advertised reasoning efforts)";
	return `${modelLine(provider.id, model)}\nReasoning efforts:\n${efforts}`;
}
/**
* Register discovery of the calling Session’s authorized child models.
* @param ctx - Context carrying the tool registry and LLM service.
* @param policyHandle - retained reader for the Session’s authorized child routes.
*/
function registerListSubagentModels(ctx, policyHandle) {
	ctx.tools.register(defineTool({
		name: "list_subagent_models",
		description: "Inspect subagent models without changing the current Agent. No arguments: providers; `provider`: allowed models; `provider` + `model`: that model and its reasoning efforts. Inspect options here before selecting a child provider and model; use returned ids for delegation.",
		parameters: {
			provider: {
				type: "string",
				description: "Registered LLM provider id."
			},
			model: {
				type: "string",
				description: "Exact model id; requires provider."
			}
		},
		output: {
			schema: { type: "string" },
			render: (_args, result) => [{
				type: "text",
				text: result
			}]
		},
		execute(args, exec) {
			return listSubagentModels(ctx, policyHandle, args, exec);
		}
	}));
}
//#endregion
//#region lib/types/preset-selection.js
/** Validates configured choices and resolves model input against the allowlist. */
var SubagentPresetSelection = class {
	config;
	constructor(config) {
		this.config = config;
		const { agentPreset, allowedAgentPresets } = config;
		if (allowedAgentPresets === void 0) return;
		if (new Set(allowedAgentPresets).size !== allowedAgentPresets.length) throw new Error("tool-subagent: allowedAgentPresets contains duplicate preset ids");
		if (agentPreset !== void 0 && !allowedAgentPresets.includes(agentPreset)) throw new Error("tool-subagent: agentPreset must be included in allowedAgentPresets");
	}
	/** Whether this tool requires the preset service and an independently composing provider. */
	get configured() {
		return this.config.agentPreset !== void 0 || this.config.allowedAgentPresets !== void 0;
	}
	/** Preset ids advertised in the model parameter. */
	get choices() {
		return this.config.allowedAgentPresets;
	}
	/** Model-visible selection and default guidance. */
	get description() {
		return `Agent preset for this child. ${this.config.agentPreset === void 0 ? "Omit to inherit the parent agent's current preset." : `Omit to use ${this.config.agentPreset}.`}`;
	}
	/**
	* Reject independent selection for providers that inherit conversation history or lack support.
	* @param provider - Currently registered delegation provider.
	*/
	assertProvider(provider) {
		if (this.configured && (!provider.capabilities.agentPreset || provider.inheritsParentContext)) throw new Error(`tool-subagent: provider "${provider.name}" does not support independent agent preset selection`);
	}
	/**
	* Validate model input and apply the deployment default.
	* @param request - Model-supplied tool arguments.
	* @returns Selected preset id, or omission to inherit the parent.
	*/
	resolve(request) {
		const id = request.agent_preset;
		if (id === void 0) return this.config.agentPreset;
		if (this.choices === void 0) throw new Error("agent_preset selection is disabled for this tool instance");
		if (typeof id !== "string" || !this.choices.includes(id)) throw new Error(`agent_preset must be one of: ${this.choices.join(", ")}`);
		return id;
	}
};
//#endregion
//#region lib/types/index.js
/**
* Model-facing delegation through one configured `ctx.subagents` provider.
* Provider lifecycle controls tool registration and context-sensitive schema
* wording. Every delegation starts a managed activation and returns its child
* id; the subagent runtime owns result delivery and resource cleanup.
* @module @deepseek-ai/dsh-tool-subagent
*/
const name = "tool-subagent";
const inject = [
	"tools",
	"subagents",
	"systemPrompt",
	"sessionProjections"
];
const delegationTools = /* @__PURE__ */ new Set();
const Config = z.object({
	provider: z.string().required(),
	toolName: z.string().default("subagent"),
	modelSelection: z.boolean().default(false),
	modelDiscovery: z.boolean().default(false),
	agentOptions: z.object({
		provider: z.string(),
		model: z.string(),
		reasoningEffort: z.string().min(1),
		maxTokens: z.number().step(1).min(1).max(Number.MAX_SAFE_INTEGER)
	}).default(void 0),
	agentPreset: z.string().min(1),
	allowedAgentPresets: z.array(z.string().min(1)).min(1).default(void 0),
	maxDepth: z.union([z.natural().max(Number.MAX_SAFE_INTEGER), z.const("provider-managed")])
});
/**
* Model-facing wording from the provider's conversation-history descriptor
* ({@link SubagentProvider.inheritsParentContext}).
* A fresh child needs a standalone prompt; a forked child already sees the
* conversation's completed turns — telling the model to restate everything
* (or, worse, that the child "does not see this conversation") would be false
* for a fork.
* @param inheritsConversation - whether the child's conversation is seeded
*   with the parent's completed turns; this says nothing about tool, service,
*   scope, or authority inheritance.
* @returns the tool `description` and the `prompt` parameter description.
*/
function providerWording(inheritsConversation) {
	if (inheritsConversation) return {
		description: "Delegate a task to a subagent that inherits this conversation: a child agent seeded with all completed turns so far (it does not see the current in-flight turn). Use this when the subtask builds on this conversation's context — a follow-up analysis, a review, a continuation — without consuming this conversation's context for the work itself. You receive its result, not its intermediate steps.",
		promptDescription: "The task for the subagent. It already sees this conversation's completed turns, so build on them freely and state only what is new."
	};
	return {
		description: "Delegate a self-contained task to a subagent (a separate agent that works in its own context) to offload focused, independent work — research, a scoped implementation, an analysis — so it does not consume this conversation's context. The subagent returns its result, not its intermediate steps.",
		promptDescription: "The complete, self-contained task for the subagent. It does not share this conversation's context, so include everything it needs."
	};
}
/**
* Install one delegation-tool composition.
* The owning composition shares its registrations across Agents; child startup resolves the selected preset.
* @param ctx - Context that owns the registrations.
* @param config - delegation-tool configuration.
* @returns when the plugin is ready.
*/
async function apply(ctx, config) {
	for (const field of ["persona", "toolFilter"]) if (Object.hasOwn(config, field)) throw new Error(`tool-subagent: ${field} is no longer supported; configure persona and tool plugins in an Agent Preset and select it with agentPreset (spawn only)`);
	const presets = new SubagentPresetSelection(config);
	if (presets.configured && ctx.get("agentPresets") === void 0) throw new Error("tool-subagent: agent preset selection requires the agent-presets service");
	if (config.maxDepth !== "provider-managed") assertSubagentMaxDepth(config.maxDepth);
	const toolName = config.toolName ?? "subagent";
	const modelSelectionEnabled = config.modelSelection === true;
	const subagentModelSelectionProjectionHandle = await ctx.sessionProjections.register(subagentModelSelectionProjectionDefinition);
	const assertSubagentProviderConfiguration = (subagentProvider) => {
		presets.assertProvider(subagentProvider);
		if (ctx.subagents.resolveMaxDepth(config.maxDepth) !== void 0 && !subagentProvider.capabilities.depthLimit) throw new Error(`tool-subagent: provider "${subagentProvider.name}" cannot enforce maxDepth (no depthLimit capability) — set maxDepth: 'provider-managed' to leave the recursion budget to the provider`);
		if (config.agentOptions !== void 0 && !subagentProvider.capabilities.agentOptions) throw new Error(`tool-subagent: provider "${subagentProvider.name}" does not support child agentOptions`);
		if (modelSelectionEnabled && !subagentProvider.capabilities.agentOptions) throw new Error(`tool-subagent: provider "${subagentProvider.name}" does not support child model selection`);
	};
	ctx.on("subagent/provider-added", (subagentProvider) => {
		if (subagentProvider.name === config.provider) assertSubagentProviderConfiguration(subagentProvider);
	});
	const initialProvider = ctx.subagents.getProvider(config.provider);
	if (initialProvider !== void 0) assertSubagentProviderConfiguration(initialProvider);
	if (config.modelDiscovery === true) registerListSubagentModels(ctx, subagentModelSelectionProjectionHandle);
	let mounted;
	const mount = (subagentProvider) => {
		assertSubagentProviderConfiguration(subagentProvider);
		const wording = providerWording(subagentProvider.inheritsParentContext);
		const providerRouteDefaults = subagentProvider.agentRouteDefaults;
		const selectionDescription = providerRouteDefaults !== void 0 ? " Supply `provider` and `model` together. Omitted values use child defaults, falling back to this backend's route defaults. When the effective route changes, an omitted `reasoning_effort` uses the model's default." : " Supply `provider` and `model` together. Omitted values use child defaults, falling back to compatible parent values. When the effective route changes, an omitted `reasoning_effort` uses the model's default.";
		const definition = defineTool({
			name: toolName,
			description: wording.description + " This tool starts an independently managed subagent and immediately returns its id. The runtime notifies you when it finishes." + (subagentProvider.prepareContinuable !== void 0 ? " The child reports results with `send_message`; use `send_message` to steer it while running or continue its conversation after it finishes." : " The completion notice includes its final answer. This backend does not accept follow-up messages.") + (modelSelectionEnabled ? selectionDescription : "") + (modelSelectionEnabled && subagentProvider.inheritsParentContext ? " Changing the route can prevent provider-side reuse of the inherited conversation prefix." : ""),
			parameters: {
				cwd: {
					type: "string",
					description: "Initial child working directory. Relative paths use your current directory; omitted inherits it. Later directory changes in either agent are independent."
				},
				description: {
					type: "string",
					required: true,
					description: "A short (3-5 word) description of the delegated task, for display."
				},
				prompt: {
					type: "string",
					required: true,
					description: wording.promptDescription
				},
				...presets.choices === void 0 ? {} : { agent_preset: {
					type: "string",
					enum: [...presets.choices],
					description: presets.description
				} },
				...modelSelectionEnabled ? {
					provider: {
						type: "string",
						description: "Child LLM provider id."
					},
					model: {
						type: "string",
						description: "Model id within that provider."
					},
					reasoning_effort: {
						type: "string",
						description: "Model-specific reasoning effort id."
					}
				} : {}
			},
			output: {
				schema: {
					type: "object",
					additionalProperties: false,
					properties: {
						kind: {
							type: "string",
							required: true,
							const: "activation"
						},
						subagentId: {
							type: "string",
							required: true
						}
					}
				},
				render: (_args, value) => [{
					type: "text",
					text: `started subagent ${value.subagentId}`
				}]
			},
			isConcurrencySafe: () => true,
			async execute(args, exec) {
				const parent = exec.agent;
				if (!parent) throw new Error("subagent tool requires a calling agent (exec.agent was undefined)");
				const routes = subagentModelSelectionPolicy(subagentModelSelectionProjectionHandle, parent.session);
				const modelSelectionPolicy = routes === void 0 ? void 0 : { routes };
				const agentPreset = presets.resolve(args);
				const modelRequest = args;
				const parentOptions = parentAgentOptionsForDelegation(parent);
				const requiresRoutePreflight = hasDelegationModelRequest(modelRequest) || hasConfiguredLlmSelection(config.agentOptions);
				const requestedChildAgentOptions = requestedAgentOptions(parentOptions, requiresRoutePreflight && providerRouteDefaults !== void 0 ? {
					...providerRouteDefaults,
					...config.agentOptions
				} : config.agentOptions, modelRequest, modelSelectionEnabled);
				assertAllowedModelSelection(modelSelectionPolicy, parentOptions, requestedChildAgentOptions, modelRequest);
				if (requiresRoutePreflight) {
					const llm = ctx.get("llm");
					if (llm === void 0) throw new Error("cannot resolve the selected child LLM route because the `llm` service is unavailable");
					await preflightChildLlmRoute(llm, parentOptions, requestedChildAgentOptions, exec.signal, providerRouteDefaults === void 0);
					if (ctx.subagents.getProvider(config.provider) !== subagentProvider) throw new Error(`subagent provider "${config.provider}" changed while resolving the child LLM route; retry the delegation`);
				}
				exec.signal.throwIfAborted();
				const maxDepth = ctx.subagents.resolveMaxDepth(config.maxDepth);
				const request = {
					...args.cwd === void 0 ? {} : { cwd: args.cwd },
					prompt: [{
						type: "text",
						text: args.prompt
					}],
					parent,
					...agentPreset === void 0 ? {} : { agentPreset },
					...requestedChildAgentOptions !== void 0 ? { agentOptions: requestedChildAgentOptions } : {},
					...maxDepth !== void 0 ? { maxDepth } : {}
				};
				return {
					kind: "activation",
					subagentId: (await ctx.subagents.startActivation({
						provider: config.provider,
						label: args.description,
						request,
						signal: exec.signal,
						delivery: "parent"
					})).childId
				};
			}
		});
		mounted = {
			subagentProvider,
			definition,
			disposeTool: ctx.effect(() => {
				const unregister = ctx.tools.register(definition);
				delegationTools.add(definition);
				return () => {
					delegationTools.delete(definition);
					unregister();
				};
			})
		};
	};
	ctx.on("subagent/provider-added", (subagentProvider) => {
		if (subagentProvider.name === config.provider && mounted === void 0) mount(subagentProvider);
	});
	ctx.on("subagent/provider-removed", (name) => {
		if (name !== config.provider || mounted === void 0) return;
		mounted.disposeTool();
		mounted = void 0;
	});
	const present = ctx.subagents.getProvider(config.provider);
	if (present !== void 0) mount(present);
	else ctx.logger.info(`subagent provider "${config.provider}" not registered yet; the "${config.toolName ?? "subagent"}" tool will register when it appears`);
	ctx.systemPrompt.section({
		name: `tool:${toolName}`,
		order: ctx.systemPrompt.getSectionOrder("TOOL_SUBAGENT"),
		text: (context) => {
			if (mounted === void 0) return "";
			const visible = [...delegationTools].filter((tool) => ctx.tools.get(tool.name, context.scope) === tool).map((tool) => tool.name).sort();
			if (visible[0] !== toolName) return "";
			return `Start independent delegations with ${visible.map((name) => `\`${name}\``).join(" or ")} together in one assistant message and continue useful work while they run. When only waiting for their results remains, end your turn instead of sleeping or polling; the next result starts a new turn.`;
		}
	});
	ctx.systemPrompt.context({
		name: `tool:${toolName}:depth`,
		order: ctx.systemPrompt.getContextOrder("SUBAGENT_DELEGATION"),
		text: (context) => {
			if (context.agent === void 0 || mounted === void 0 || ctx.tools.get(toolName, context.scope) !== mounted.definition) return "";
			const maxDepth = ctx.subagents.resolveMaxDepth(config.maxDepth);
			if (maxDepth === void 0) return "";
			const depth = delegationDepthOf(context.agent);
			if (depth < maxDepth) return "";
			return `Delegation depth for \`${toolName}\`: current ${depth}, maximum ${maxDepth}, remaining levels 0. Do not call \`${toolName}\` at this depth; carry out the work directly.`;
		}
	});
}
//#endregion
export { Config, apply, inject, name };
