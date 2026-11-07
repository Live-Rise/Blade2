/**
 * Single replay-aware token-meter service for request and surface pressure.
 *
 * @module @deepseek-ai/dsh-token-meter
 */
import { Service } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import { deepFreeze } from '@deepseek-ai/dsh-util-values';
import { canonicalHeader, headerEquals, isSurfaceEvent, SessionLogOffset, SessionSeq, } from '@deepseek-ai/dsh-session';
import { contextBreakdownProjectionDefinition } from "./breakdown-projection.js";
import { contextPressureProjectionDefinition, tokenUsageProjectionDefinition } from "./usage-projection.js";
import { estimateMessage, estimateToolsTokens } from "./estimate.js";
import { advanceMeasurement, measurementProjectionDefinition } from "./measurement-projection.js";
import { commitSurfaceTokens, planSurfaceTokens } from "./surface-fold.js";
import { priceSurface } from "./route-pricing.js";
/** Sum disjoint provider usage buckets without double-counting reasoning output. */
function usageTokens(usage) {
    return usage.inputTokens
        + (usage.cacheReadTokens ?? 0)
        + (usage.cacheWriteTokens ?? 0)
        + usage.outputTokens;
}
/** Compare optional envelopes so a headerless estimate can track later surface deltas. */
function optionalHeaderEquals(left, right) {
    if (left === undefined || right === undefined)
        return left === right;
    return headerEquals(left, right);
}
/** Reject stale or misspelled keys before defaults can hide them. */
function validateConfigKeys(config) {
    for (const key of Object.keys(config)) {
        throw new Error(`TokenMeterConfig: unknown key "${key}" (no settings are supported)`);
    }
}
/** Replay owner for one service-wide estimator and isolated per-session folds. */
export class TokenMeter extends Service {
    // Schemastery preserves untrusted loader keys on an empty object schema;
    // the public type excludes settings while validateConfigKeys rejects them.
    static Config = z.object({});
    static inject = ['sessionProjections'];
    states = new WeakMap();
    measurementProjectionHandle;
    contextPressureProjectionHandle;
    constructor(ctx, config = {}) {
        super(ctx, 'tokenMeter');
        validateConfigKeys(config);
    }
    async [Service.init]() {
        const ctx = this.ctx;
        this.measurementProjectionHandle = await ctx.sessionProjections.register(measurementProjectionDefinition);
        await ctx.sessionProjections.register(tokenUsageProjectionDefinition);
        this.contextPressureProjectionHandle = await ctx.sessionProjections.register(contextPressureProjectionDefinition);
        await ctx.sessionProjections.register(contextBreakdownProjectionDefinition);
        // Readers catch up independently, while eager observation bounds ordinary
        // read latency without creating state for sessions no consumer has read.
        ctx.on('session/event', (session) => {
            if (this.states.has(session))
                this._sync(session);
        });
    }
    /**
     * Read the latest logged provider prompt size and route capacity.
     * @param session - Session whose committed usage and request capacity are read.
     * @returns context pressure; fields remain absent until their source events are recorded.
     */
    contextPressure(session) {
        const { pressureTokens, contextWindow } = this.contextPressureProjectionHandle.stateOf(session);
        return {
            ...pressureTokens === undefined ? {} : { pressureTokens },
            ...contextWindow === undefined ? {} : { contextWindow },
        };
    }
    /**
     * Measure current request pressure and surface through the durable tail.
     *
     * The effective envelope's routed provider/model selects the request-image
     * pricing every node is priced under: a route whose adapter declares image
     * pricing charges each retained image its visual tokens plus its
     * model-visible text, while other routes keep the fixed heuristic. Provider
     * usage is reused only when the latest successful call's canonical request
     * envelope matches `requestHeader` and its total is no lower than that
     * call's full route-priced anchor; otherwise the complete envelope and
     * surface are repriced. The anchor includes all surface nodes immediately
     * before the assistant message, including inputs admitted after step/start.
     *
     * `requestHeader` replaces the latest logged envelope for pressure and node
     * pricing; the node set always describes the current session surface. Every
     * call clones those positional nodes, so measurement is O(surface).
     *
     * @param session - session to replay through its current durable tail.
     * @param requestHeader - optional effective request envelope replacing the latest logged header.
     * @returns a detached deeply immutable pressure and surface measurement.
     */
    measure(session, requestHeader) {
        const { prices: state, measurement } = this._sync(session);
        const header = requestHeader === undefined
            ? this._cachedHeader(state, measurement.headerSeq)
            : canonicalHeader(requestHeader);
        const pricing = this._routeImagePricing(header);
        const fileText = this._fileRequestText();
        const surface = priceSurface(state.surface, pricing, fileText);
        const anchor = measurement.anchor;
        let baseline;
        let surfaceDeltaTokens;
        if (anchor !== null && optionalHeaderEquals(this._cachedHeader(state, anchor.headerSeq), header)) {
            // Matching headers share one route, so the anchored snapshot reprices
            // under the same pricing as the current surface and the signed delta
            // compares like with like.
            const anchorSurfaceTokens = priceSurface(state.anchorNodes, pricing, fileText).surfaceTokens
                + anchor.assistantTokens;
            const estimatedAnchorTokens = estimateToolsTokens(header) + anchorSurfaceTokens;
            const usage = anchor.usage;
            // Signed heuristic deltas remain conservative only from an anchor
            // that is at least as large as the matching full heuristic price.
            baseline = usage !== null && usageTokens(usage) >= estimatedAnchorTokens
                ? { kind: 'usage', tokens: usageTokens(usage), usage }
                : { kind: 'estimated', tokens: estimatedAnchorTokens };
            surfaceDeltaTokens = surface.surfaceTokens - anchorSurfaceTokens;
        }
        else if (header === undefined && surface.surfaceTokens === 0) {
            baseline = { kind: 'none', tokens: 0 };
            surfaceDeltaTokens = 0;
        }
        else {
            baseline = {
                kind: 'estimated',
                tokens: estimateToolsTokens(header) + surface.surfaceTokens,
            };
            surfaceDeltaTokens = 0;
        }
        return deepFreeze(structuredClone({
            logRevision: state.consumedEvents,
            baseline,
            surfaceDeltaTokens,
            totalTokens: Math.max(0, baseline.tokens + surfaceDeltaTokens),
            surfaceTokens: surface.surfaceTokens,
            nodes: surface.nodes,
        }));
    }
    /** Resolve the routed model's image pricing, when the llm service and route declare one. */
    _routeImagePricing(header) {
        const config = header?.config;
        if (config === undefined)
            return undefined;
        return this.ctx.get('llm')?.imageRequestPricing(config.provider, config.model);
    }
    /** Resolve request-time file projection when an LLM service is mounted. */
    _fileRequestText() {
        const llm = this.ctx.get('llm');
        return llm === undefined ? undefined : ref => llm.fileRequestText(ref);
    }
    /**
     * Heuristically price one model-visible message (instance face of the pure
     * `estimateMessage` export from `estimate.ts`).
     * @param message - message to price without mutation.
     * @returns content and role-framing tokens under the fixed service heuristic.
     */
    estimateMessage(message) {
        return estimateMessage(message);
    }
    /** Resolve one of the two header payloads retained by the rebuildable price cache. */
    _cachedHeader(state, seq) {
        if (seq === null)
            return undefined;
        if (state.header?.seq === seq)
            return state.header.value;
        return state.anchorHeader?.value;
    }
    /** Catch rebuildable prices up to the current durable tail. */
    _sync(session, useProjection = true) {
        let state = this.states.get(session);
        if (state === undefined) {
            state = {
                consumedEvents: SessionLogOffset(0),
                surface: [],
                header: undefined,
                anchorHeader: undefined,
                anchorNodes: [],
                pendingMeasurement: measurementProjectionDefinition.init(),
            };
            this.states.set(session, state);
        }
        const projected = useProjection ? this.measurementProjectionHandle?.stateOf(session) : undefined;
        while (state.consumedEvents < session.seq) {
            // Cold price caches may replay resident history.
            // oxlint-disable-next-line typescript/no-non-null-assertion, typescript/no-deprecated
            const event = session.eventAt(SessionSeq(state.consumedEvents));
            const next = projected !== undefined
                ? undefined
                : measurementProjectionDefinition.apply(state.pendingMeasurement, event);
            const measurement = next ?? projected;
            if (measurement?.invalidSeq === event.seq) {
                // Match the original assistant validation order before committing any prices.
                if (event.type === 'assistant/message')
                    planSurfaceTokens(state.surface, event);
                advanceMeasurement(measurement, event);
            }
            this._foldEvent(state, event);
            if (next !== undefined)
                state.pendingMeasurement = next;
            state.consumedEvents = SessionLogOffset(state.consumedEvents + 1);
        }
        if (projected !== undefined && (projected.headerSeq !== (state.header?.seq ?? null)
            || (projected.anchor !== null && projected.anchor.headerSeq !== (state.anchorHeader?.seq ?? null)))) {
            // A checkpoint reference must match the original journal. Discard the
            // rebuildable cache and use the same calculation from the beginning.
            this.states.delete(session);
            return this._sync(session, false);
        }
        // An initialized registration supplies `projected`; earlier reads keep the same pure fold locally.
        return { prices: state, measurement: projected ?? state.pendingMeasurement };
    }
    /** Update only discardable positional prices and the pre-assistant view. */
    _foldEvent(state, event) {
        if (event.type === 'request/header')
            state.header = { seq: event.seq, value: canonicalHeader(event.data.header) };
        if (event.type === 'image/offload') {
            const offloaded = new Map(event.data.targets.map(target => [target.seq, new Set(target.imageIndexes)]));
            state.surface = state.surface.map((node) => {
                const indexes = offloaded.get(node.seq);
                if (indexes === undefined)
                    return node;
                return {
                    ...node,
                    images: node.images.map((image, index) => (indexes.has(index) ? { ...image, offloaded: true } : image)),
                };
            });
        }
        const plan = isSurfaceEvent(event) ? planSurfaceTokens(state.surface, event) : undefined;
        if (event.type === 'assistant/message') {
            state.anchorNodes = [...state.surface];
            state.anchorHeader = state.header;
        }
        if (plan !== undefined)
            commitSurfaceTokens(state.surface, plan);
    }
}
export default TokenMeter;
//# sourceMappingURL=index.js.map