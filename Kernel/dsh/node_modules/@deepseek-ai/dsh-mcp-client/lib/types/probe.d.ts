/**
 * One-shot MCP server probe: connect, read the server identity, list tools,
 * close. Nothing registers on `ctx.tools`, no `serverName` is reserved, and
 * nothing persists — the probe answers a configuration surface's
 * "test this server" without touching the live instance set. Expected
 * failures return a stage-classified value rather than throwing, so a wire
 * caller renders the outcome without exception mapping.
 *
 * @module
 */
import type { Context } from '@deepseek-ai/cordis';
import type { McpProbeResult } from './types.ts';
import type { Config } from './index.ts';
/** Options for one {@link probeServer} call. */
export interface McpProbeOptions {
    /** Budget for the whole probe (connect + listing) in milliseconds; default 10000. */
    timeoutMs?: number;
    /** Maximum listing pages before returning a truncated result; default 16. */
    maxPages?: number;
    /** Maximum raw tool names retained in the result; default 512. */
    maxToolNames?: number;
    /**
     * Abort the in-flight probe; settles as a failure at the current stage after
     * bounded generation cleanup.
     */
    signal?: AbortSignal;
}
/**
 * Probe a server without registering tools or reserving its namespace.
 * @param ctx - context providing template sources when expansion is enabled.
 * @param config - transport configuration; its startup budget does not bound the probe.
 * @param options - whole-probe budget, listing caps, and cancellation signal.
 * @returns the connect/list outcome and independent cleanup result after bounded closure.
 * @throws for invalid options, malformed templates, or a missing required credentials provider.
 */
export declare function probeServer(ctx: Context, config: Config, options?: McpProbeOptions): Promise<McpProbeResult>;
//# sourceMappingURL=probe.d.ts.map