import { z } from "zod";
import { Service } from "@deepseek-ai/cordis";
import { Remote, TypertRemoteService } from "@deepseek-ai/dsh-typert-protocol";
import { isPluginStopping } from "@deepseek-ai/dsh-action-cards";
import { readFile } from "node:fs/promises";
import { BUNDLED_SKILL_RANK } from "@deepseek-ai/dsh-skill";
//#region lib/types/skill.js
/** On-demand official-plugin catalog for explicit product-use needs. */
/** Shipped optional plugins only; no arbitrary install target is accepted.
* @param bundle - a profile bundle as the plugin manager reports it.
* @returns whether a card may offer and enable it.
*/
function eligible(bundle) {
	return bundle.official && bundle.optional && bundle.availability !== "missing" && bundle.readOnlyReason === void 0 && bundle.error === void 0;
}
/** The eligible plugins as the skill lists them: name, title, description and selection. */
function catalogOf(bundles) {
	return JSON.stringify(bundles.filter(eligible).map((bundle) => ({
		name: bundle.name,
		title: bundle.meta?.title ?? bundle.name,
		description: bundle.meta?.description ?? bundle.description ?? "",
		enabled: bundle.enabled
	})));
}
/** Register the experiment's skill without adding a model tool or a system-prompt section.
* @param ctx - Owning plugin context with skill and plugin registries.
*/
function registerOfficialPluginsSkill(ctx) {
	const candidate = {
		name: "official-plugins",
		description: "Check DSH’s optional plugins when the user asks how to do or use something in this app, whether it can, or says a capability is missing or should change: tools, input, thinking or title display, session search, terminals, worktrees, agent teams, desktop control, automation, access, approvals, badges or signatures, plugins or plugin sources. Not a request to develop DSH, even in its own repository: load before reading files or source. Not for ordinary tasks, quoted wishes or declined offers.",
		invocation: {
			modelInvocable: true,
			userInvocable: false
		},
		provider: "dsh-official-plugins",
		source: "bundled",
		rank: BUNDLED_SKILL_RANK,
		locator: null
	};
	ctx.skills.registerProvider(() => ({
		name: candidate.provider,
		list: () => Promise.resolve([candidate]),
		async get(_candidate, { signal }) {
			const policy = await readFile(new URL("../skills/official-plugins.md", import.meta.url), {
				encoding: "utf8",
				signal
			});
			const catalog = catalogOf(await ctx.pluginManager.listBundles());
			signal?.throwIfAborted();
			const { rank: _rank, locator: _locator, ...entry } = candidate;
			return {
				...entry,
				content: `${policy}\n## Official plugins\n\n${catalog}`
			};
		}
	}));
}
//#endregion
//#region lib/types/index.js
/** Explicit-need official plugin recommendations: an `enable_official_plugin` card offers one plugin, and only a human click enables it. */
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
const name = "plugin-recommendations";
/** Services the plugin uses; `actionCards` owns the card tool, card state and user actions. */
const inject = [
	"pluginManager",
	"agents",
	"skills",
	"actionCards"
];
/** Card action this plugin registers. */
const ACTION = "enable_official_plugin";
const detailsSchema = z.object({ name: z.string().trim().min(1) });
/** Own entries only: browser-supplied ids such as `toString` never reach inherited object members. */
function ownEntry(record, key) {
	return Object.hasOwn(record, key) ? record[key] : void 0;
}
function settled(decision) {
	return {
		status: "settled",
		decision
	};
}
function refused(reason) {
	return {
		status: "refused",
		reason
	};
}
/** Catalog lookups for card text and the enabling behind the `enable_official_plugin` card. */
let PluginRecommendations = (() => {
	let _classSuper = TypertRemoteService;
	let _instanceExtraInitializers = [];
	let _describe_decorators;
	return class PluginRecommendations extends _classSuper {
		static {
			const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
			_describe_decorators = [Remote];
			__esDecorate(this, null, _describe_decorators, {
				kind: "method",
				name: "describe",
				static: false,
				private: false,
				access: {
					has: (obj) => "describe" in obj,
					get: (obj) => obj.describe
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
		static inject = inject;
		/** Card reconciliations and enabling in flight; unloading waits for them. */
		operations = (__runInitializers(this, _instanceExtraInitializers), /* @__PURE__ */ new Set());
		stopped = Promise.withResolvers();
		closing = false;
		owner;
		constructor(ctx) {
			super(ctx, "pluginRecommendations");
			this.owner = ctx.fiber;
		}
		[Service.init]() {
			registerOfficialPluginsSkill(this.ctx);
			this.ctx.actionCards.register({
				action: ACTION,
				skill: "official-plugins",
				details: detailsSchema,
				invitation: "End with at most one short invitation in the user’s language to click Enable below; do not say the plugin is already on.",
				verbs: { enable: { global: true } },
				openPhases: ["failed", "unconfirmed"],
				statusGuidance: "Plugin cards: enabled means the plugin is on now, or after a DSH restart when application is restart-required; already-enabled means it was on before; failed, or unconfirmed when a reload cut enabling off, can be retried from the card; dismissed means the user declined it, so do not offer it again in this conversation; enabling authorizes nothing beyond that plugin.",
				reconcile: (agent, proposal) => this.tracked(this.confirm(agent, proposal)),
				act: (agent, proposal) => this.tracked(this.enable(agent, proposal))
			});
			this.ctx.effect(() => async () => {
				this.closing = true;
				this.stopped.resolve(void 0);
				await Promise.allSettled([...this.operations]);
			}, "plugin-recommendations: drain enabling");
		}
		/**
		* Read the current catalog text for a logged card.
		* @param agent - Exact live root resolved by the Gateway from the Session id.
		* @param callId - The card's `propose_action` call id, never a package supplied by the browser.
		* @returns the plugin's title, description and selection, or a refusal when the card names no available official plugin.
		*/
		async describe(agent, callId) {
			if (this.isStopping()) return {
				status: "refused",
				reason: "closing"
			};
			if (!this.ctx.agents.roots().includes(agent)) return {
				status: "refused",
				reason: "not-live-root"
			};
			const proposal = ownEntry(this.ctx.actionCards.view(agent.session).proposals, callId);
			const details = proposal?.action === ACTION ? detailsSchema.safeParse(proposal.details) : void 0;
			if (details?.success !== true) return {
				status: "refused",
				reason: "unknown"
			};
			const bundle = (await this.ctx.pluginManager.listBundles()).find((item) => item.name === details.data.name);
			if (bundle === void 0 || !eligible(bundle)) return {
				status: "refused",
				reason: "unavailable"
			};
			return {
				status: "available",
				title: bundle.meta?.title ?? bundle.name,
				description: bundle.meta?.description ?? bundle.description ?? "",
				enabled: bundle.enabled
			};
		}
		isStopping() {
			return isPluginStopping(this.owner, this.closing);
		}
		/** Track an operation until it settles, so unloading waits for it. */
		tracked(operation) {
			this.operations.add(operation);
			return operation.finally(() => {
				this.operations.delete(operation);
			});
		}
		/** Record `enabled` on an unconfirmed card whose interrupted enable has since turned its plugin on. */
		async confirm(agent, proposal) {
			const cards = this.ctx.actionCards;
			if (ownEntry(cards.view(agent.session).decisions, proposal.callId)?.phase !== "unconfirmed") return;
			if ((await this.ctx.pluginManager.listBundles()).find((item) => item.name === proposal.details.name)?.enabled === true) cards.record(agent, {
				callId: proposal.callId,
				action: ACTION,
				phase: "enabled"
			});
		}
		/** Enable the card's plugin once; an enabled card settles unchanged, and a failed or unconfirmed one can be retried. */
		async enable(agent, proposal) {
			const cards = this.ctx.actionCards;
			const record = (phase, data = {}) => {
				const fields = Object.fromEntries(Object.entries(data).filter(([, value]) => value !== void 0));
				const decision = {
					callId: proposal.callId,
					action: ACTION,
					phase,
					...Object.keys(fields).length === 0 ? {} : { data: fields }
				};
				cards.record(agent, decision);
				return settled(decision);
			};
			const previous = ownEntry(cards.view(agent.session).decisions, proposal.callId);
			if (previous !== void 0 && previous.phase !== "failed" && previous.phase !== "unconfirmed") return settled(previous);
			const bundle = (await this.ctx.pluginManager.listBundles()).find((item) => item.name === proposal.details.name);
			if (bundle === void 0 || !eligible(bundle)) return refused("unavailable");
			if (this.isStopping()) return refused("closing");
			if (!this.ctx.agents.roots().includes(agent)) return refused("not-live-root");
			if (bundle.enabled) return record(previous?.phase === "unconfirmed" ? "enabled" : "already-enabled");
			let result;
			try {
				result = await Promise.race([this.ctx.pluginManager.setBundleEnabled(bundle.name, true), this.stopped.promise]);
			} catch (error) {
				this.ctx.logger.warn("Enabling %s was rejected: %s", bundle.name, String(error));
				return record("failed", { errorCode: "operation-error" });
			}
			if (result === void 0) {
				record("unconfirmed");
				return {
					status: "unconfirmed",
					reason: "closing"
				};
			}
			return record(result.application === "failed" ? "failed" : "enabled", {
				application: result.application === "restart-required" ? "restart-required" : void 0,
				errorCode: result.error?.code
			});
		}
	};
})();
/**
* Register the official-plugins skill, the `enable_official_plugin` card kind and the catalog-text Remote;
* disposing the plugin stops local follow-up of pending enabling and removes all three.
* @param ctx - Owning plugin context.
*/
async function apply(ctx) {
	await ctx.plugin(PluginRecommendations);
}
//#endregion
export { PluginRecommendations, apply, inject, name };
