var __runInitializers = (this && this.__runInitializers) || function (thisArg, initializers, value) {
    var useValue = arguments.length > 2;
    for (var i = 0; i < initializers.length; i++) {
        value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
    }
    return useValue ? value : void 0;
};
var __esDecorate = (this && this.__esDecorate) || function (ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
    function accept(f) { if (f !== void 0 && typeof f !== "function") throw new TypeError("Function expected"); return f; }
    var kind = contextIn.kind, key = kind === "getter" ? "get" : kind === "setter" ? "set" : "value";
    var target = !descriptorIn && ctor ? contextIn["static"] ? ctor : ctor.prototype : null;
    var descriptor = descriptorIn || (target ? Object.getOwnPropertyDescriptor(target, contextIn.name) : {});
    var _, done = false;
    for (var i = decorators.length - 1; i >= 0; i--) {
        var context = {};
        for (var p in contextIn) context[p] = p === "access" ? {} : contextIn[p];
        for (var p in contextIn.access) context.access[p] = contextIn.access[p];
        context.addInitializer = function (f) { if (done) throw new TypeError("Cannot add initializers after decoration has completed"); extraInitializers.push(accept(f || null)); };
        var result = (0, decorators[i])(kind === "accessor" ? { get: descriptor.get, set: descriptor.set } : descriptor[key], context);
        if (kind === "accessor") {
            if (result === void 0) continue;
            if (result === null || typeof result !== "object") throw new TypeError("Object expected");
            if (_ = accept(result.get)) descriptor.get = _;
            if (_ = accept(result.set)) descriptor.set = _;
            if (_ = accept(result.init)) initializers.unshift(_);
        }
        else if (_ = accept(result)) {
            if (kind === "field") initializers.unshift(_);
            else descriptor[key] = _;
        }
    }
    if (target) Object.defineProperty(target, contextIn.name, descriptor);
    done = true;
};
/** Explicit-need official plugin recommendations: an `enable_official_plugin` card offers one plugin, and only a human click enables it. */
import { z } from 'zod';
import { Service } from '@deepseek-ai/cordis';
import { Remote, TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import { isPluginStopping } from '@deepseek-ai/dsh-action-cards';
import { eligible, registerOfficialPluginsSkill } from "./skill.js";
export const name = 'plugin-recommendations';
/** Services the plugin uses; `actionCards` owns the card tool, card state and user actions. */
export const inject = ['pluginManager', 'agents', 'skills', 'actionCards'];
/** Card action this plugin registers. */
const ACTION = 'enable_official_plugin';
const detailsSchema = z.object({ name: z.string().trim().min(1) });
/** Own entries only: browser-supplied ids such as `toString` never reach inherited object members. */
function ownEntry(record, key) {
    return Object.hasOwn(record, key) ? record[key] : undefined;
}
function settled(decision) {
    return { status: 'settled', decision };
}
function refused(reason) {
    return { status: 'refused', reason };
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
            __esDecorate(this, null, _describe_decorators, { kind: "method", name: "describe", static: false, private: false, access: { has: obj => "describe" in obj, get: obj => obj.describe }, metadata: _metadata }, null, _instanceExtraInitializers);
            if (_metadata) Object.defineProperty(this, Symbol.metadata, { enumerable: true, configurable: true, writable: true, value: _metadata });
        }
        static inject = inject;
        /** Card reconciliations and enabling in flight; unloading waits for them. */
        operations = (__runInitializers(this, _instanceExtraInitializers), new Set());
        stopped = Promise.withResolvers();
        closing = false;
        owner;
        constructor(ctx) {
            super(ctx, 'pluginRecommendations');
            this.owner = ctx.fiber;
        }
        [Service.init]() {
            registerOfficialPluginsSkill(this.ctx);
            this.ctx.actionCards.register({
                action: ACTION, skill: 'official-plugins', details: detailsSchema,
                invitation: 'End with at most one short invitation in the user’s language to click Enable below; '
                    + 'do not say the plugin is already on.',
                verbs: { enable: { global: true } },
                openPhases: ['failed', 'unconfirmed'],
                statusGuidance: 'Plugin cards: enabled means the plugin is on now, or after a DSH restart when application is restart-required; '
                    + 'already-enabled means it was on before; failed, or unconfirmed when a reload cut enabling off, can be retried from the card; '
                    + 'dismissed means the user declined it, so do not offer it again in this conversation; enabling authorizes nothing beyond that plugin.',
                // Every action, including Not now, first learns whether an interrupted enable has since landed.
                reconcile: (agent, proposal) => this.tracked(this.confirm(agent, proposal)),
                act: (agent, proposal) => this.tracked(this.enable(agent, proposal)),
            });
            this.ctx.effect(() => async () => {
                this.closing = true;
                this.stopped.resolve(undefined);
                await Promise.allSettled([...this.operations]);
            }, 'plugin-recommendations: drain enabling');
        }
        /**
         * Read the current catalog text for a logged card.
         * @param agent - Exact live root resolved by the Gateway from the Session id.
         * @param callId - The card's `propose_action` call id, never a package supplied by the browser.
         * @returns the plugin's title, description and selection, or a refusal when the card names no available official plugin.
         */
        async describe(agent, callId) {
            if (this.isStopping())
                return { status: 'refused', reason: 'closing' };
            if (!this.ctx.agents.roots().includes(agent))
                return { status: 'refused', reason: 'not-live-root' };
            const proposal = ownEntry(this.ctx.actionCards.view(agent.session).proposals, callId);
            const details = proposal?.action === ACTION ? detailsSchema.safeParse(proposal.details) : undefined;
            if (details?.success !== true)
                return { status: 'refused', reason: 'unknown' };
            const bundle = (await this.ctx.pluginManager.listBundles()).find(item => item.name === details.data.name);
            if (bundle === undefined || !eligible(bundle))
                return { status: 'refused', reason: 'unavailable' };
            return { status: 'available', title: bundle.meta?.title ?? bundle.name,
                description: bundle.meta?.description ?? bundle.description ?? '', enabled: bundle.enabled };
        }
        isStopping() {
            return isPluginStopping(this.owner, this.closing);
        }
        /** Track an operation until it settles, so unloading waits for it. */
        tracked(operation) {
            this.operations.add(operation);
            return operation.finally(() => { this.operations.delete(operation); });
        }
        /** Record `enabled` on an unconfirmed card whose interrupted enable has since turned its plugin on. */
        async confirm(agent, proposal) {
            // Held from the start, like enabling: an unload while the catalog is read still records the outcome.
            const cards = this.ctx.actionCards;
            if (ownEntry(cards.view(agent.session).decisions, proposal.callId)?.phase !== 'unconfirmed')
                return;
            const bundle = (await this.ctx.pluginManager.listBundles()).find(item => item.name === proposal.details.name);
            if (bundle?.enabled === true)
                cards.record(agent, { callId: proposal.callId, action: ACTION, phase: 'enabled' });
        }
        /** Enable the card's plugin once; an enabled card settles unchanged, and a failed or unconfirmed one can be retried. */
        async enable(agent, proposal) {
            // Held from the start: an unload during the profile change still records where this card stands.
            const cards = this.ctx.actionCards;
            const record = (phase, data = {}) => {
                const fields = Object.fromEntries(Object.entries(data).filter(([, value]) => value !== undefined));
                const decision = {
                    callId: proposal.callId, action: ACTION, phase, ...Object.keys(fields).length === 0 ? {} : { data: fields },
                };
                cards.record(agent, decision);
                return settled(decision);
            };
            const previous = ownEntry(cards.view(agent.session).decisions, proposal.callId);
            if (previous !== undefined && previous.phase !== 'failed' && previous.phase !== 'unconfirmed')
                return settled(previous);
            const bundle = (await this.ctx.pluginManager.listBundles()).find(item => item.name === proposal.details.name);
            if (bundle === undefined || !eligible(bundle))
                return refused('unavailable');
            if (this.isStopping())
                return refused('closing');
            if (!this.ctx.agents.roots().includes(agent))
                return refused('not-live-root');
            // An unconfirmed card's plugin that reconciliation still found off and is on now was turned on by its own enable.
            if (bundle.enabled)
                return record(previous?.phase === 'unconfirmed' ? 'enabled' : 'already-enabled');
            let result;
            try {
                // PluginManager owns the accepted profile mutation. HMR may unload this
                // consumer before it finishes, so only our wait for its result is cancelled.
                result = await Promise.race([this.ctx.pluginManager.setBundleEnabled(bundle.name, true), this.stopped.promise]);
            }
            catch (error) {
                // A rejected change reports no management code; the card offers a retry instead of failing the click.
                this.ctx.logger.warn('Enabling %s was rejected: %s', bundle.name, String(error));
                return record('failed', { errorCode: 'operation-error' });
            }
            if (result === undefined) {
                record('unconfirmed');
                return { status: 'unconfirmed', reason: 'closing' };
            }
            return record(result.application === 'failed' ? 'failed' : 'enabled', {
                application: result.application === 'restart-required' ? 'restart-required' : undefined, errorCode: result.error?.code,
            });
        }
    };
})();
export { PluginRecommendations };
/**
 * Register the official-plugins skill, the `enable_official_plugin` card kind and the catalog-text Remote;
 * disposing the plugin stops local follow-up of pending enabling and removes all three.
 * @param ctx - Owning plugin context.
 */
export async function apply(ctx) {
    await ctx.plugin(PluginRecommendations);
}
//# sourceMappingURL=index.js.map