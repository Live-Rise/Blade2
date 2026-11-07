import { Service } from "@deepseek-ai/cordis";
import z from "@deepseek-ai/schemastery";
import { SessionSeq } from "@deepseek-ai/dsh-session";
import { z as z$1 } from "zod";
import "@deepseek-ai/dsh-llm";
//#region lib/types/model-selection.js
/** Schema shared by the Host setting and its deployment base. */
const AllowedModelRouteSchema = z.object({
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
//#region lib/types/model-selection-settings.js
/** Host settings owner that records Session authorization before the first model request. */
var SubagentModelSelectionConfig = class extends Service {
	config;
	static inject = ["sessionProjections", "sessions"];
	static Config = z.object({
		enabled: z.boolean().default(false).volatile(),
		allowedModels: z.array(AllowedModelRouteSchema).default([]).volatile()
	});
	constructor(ctx, config) {
		super(ctx, "subagentModelSelection");
		this.config = config;
	}
	async [Service.init]() {
		const ctx = this.ctx;
		const policyReady = ctx.sessionProjections.register(subagentModelSelectionProjectionDefinition);
		ctx.on("agent/created", async ({ agent }) => {
			const policy = await policyReady;
			ctx.fiber.assertActive();
			const target = agent.session;
			if (subagentModelSelectionPolicy(policy, target) !== void 0) return;
			let allowedModels;
			const parentId = target.header.origin === "subagent" ? target.header.parentSession : void 0;
			if (parentId !== void 0) {
				const parent = ctx.sessions.get(parentId);
				if (parent !== void 0) allowedModels = subagentModelSelectionPolicy(policy, parent);
			} else if (target.firstLiveSeq === 0 && target.eventAt(SessionSeq(0))?.type !== "session/end-seed") {
				const current = this.current();
				allowedModels = current.enabled ? current.allowedModels : void 0;
			}
			if (allowedModels !== void 0) target.append("subagent/model-selection-policy", { allowedModels });
		});
		await policyReady;
	}
	/**
	* Read a detached selection preference for the next fresh top-level Session.
	* @returns the enabled state and exact allowed routes.
	*/
	current() {
		const enabled = this.config.enabled.get();
		const allowedModels = this.config.allowedModels.get();
		assertAllowedModelRoutes(allowedModels);
		if (enabled && allowedModels.length === 0) throw new Error("enabled subagent model selection requires at least one allowed model");
		return {
			enabled,
			allowedModels: allowedModels.map((route) => ({ ...route }))
		};
	}
};
const name = "subagent-model-selection-settings";
//#endregion
export { SubagentModelSelectionConfig, SubagentModelSelectionConfig as default, name };
