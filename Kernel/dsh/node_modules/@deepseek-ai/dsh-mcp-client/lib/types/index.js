/**
 * MCP client bridge plugin: connects to an external MCP server and registers
 * its tools on `ctx.tools` under server-qualified public names
 * (`mcp__<serverName>__<rawName>`). Each plugin instance connects to one MCP
 * server; load multiple instances in `cordis.yml` for multiple servers.
 *
 * Namespace plugin (named exports, no default export). Lifecycle is
 * effect-scoped: disposal disconnects from the server, unregisters all tools,
 * and releases the `serverName` namespace reservation. HMR hot-swaps by
 * disposing the old instance and creating a new one; identical `serverName`
 * reproduces identical public tool names.
 *
 * @module @deepseek-ai/dsh-mcp-client
 */
import z from '@deepseek-ai/schemastery';
import { scopeOf } from '@deepseek-ai/dsh-scope';
import { MAX_TIMER_DELAY_MS } from '@deepseek-ai/dsh-timeout';
import { DEFAULT_MAX_INSTRUCTION_BYTES, RECONNECT_DEFAULTS, resolveReconnectPolicy, startConnection } from "./connection.js";
export { resolveReconnectPolicy } from "./connection.js";
import { registerServerContext } from "./server-context.js";
import { connectionStatusIn, registerStatusCell } from "./status.js";
export { createMcpToolDefinition } from "./tools.js";
export { probeServer } from "./probe.js";
export { assertTemplateSyntax } from "./templates.js";
/** Cordis plugin name used by loader diagnostics. */
export const name = 'mcp-client';
/** Services required by this plugin. */
export const inject = ['tools'];
/** Default timeout for individual MCP tool calls and resource requests (ms). */
const DEFAULT_TOOL_CALL_TIMEOUT_MS = 60_000;
/** Default budget for transport closure and local cleanup (ms). */
const DEFAULT_CLOSE_TIMEOUT_MS = 5_000;
/** Valid `serverName`, kept below the public tool-name budget. */
export const SERVER_NAME_PATTERN = /^[A-Za-z0-9_-]{1,32}$/;
/**
 * Live `serverName` reservations per registration scope. Agent-scoped MCP
 * servers may reuse a namespace in another Agent, while global instances and
 * duplicates inside one Agent remain mutually exclusive.
 */
const activeServerNames = new WeakMap();
/**
 * Read an MCP instance's stable status handle. Reloading the same Fiber keeps
 * the handle; final disposal detaches observers and leaves its last state readable.
 * @param fiber - the plugin Fiber or the handle returned by `ctx.plugin()`.
 * @returns the status handle, or `undefined` before the instance registers one.
 */
export function connectionStatusOf(fiber) {
    return connectionStatusIn(fiber);
}
/**
 * Reconnect-policy section schema, exported so a manager composing mcp-client
 * instances (`dsh-mcp-manager`) validates the same fields, defaults, and
 * bounds without restating them.
 */
export const ReconnectSchema = z.object({
    enabled: z.boolean().default(RECONNECT_DEFAULTS.enabled),
    initialDelayMs: z.number().min(1).max(MAX_TIMER_DELAY_MS).default(RECONNECT_DEFAULTS.initialDelayMs),
    maxDelayMs: z.number().min(1).max(MAX_TIMER_DELAY_MS).default(RECONNECT_DEFAULTS.maxDelayMs),
    maxAttempts: z.number().step(1).min(1).max(Number.MAX_SAFE_INTEGER).default(RECONNECT_DEFAULTS.maxAttempts),
});
/* jscpd:ignore-start */
// The catalog generator statically walks this literal and rejects spreads and
// computed keys, so the branches restate their shared rows by construction.
export const Config = z.union([
    z.object({
        transport: z.const('stdio'),
        serverName: z.string().required().pattern(SERVER_NAME_PATTERN),
        command: z.string().required(),
        args: z.array(String).default([]),
        env: z.dict(String).default({}),
        cwd: z.string().default(''),
        toolCallTimeoutMs: z.number().default(DEFAULT_TOOL_CALL_TIMEOUT_MS),
        startupTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS),
        closeTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS).default(DEFAULT_CLOSE_TIMEOUT_MS),
        expandTemplates: z.boolean().default(false),
        failOnStartupError: z.boolean().default(false),
        maxInstructionBytes: z.number().step(1).min(1).default(DEFAULT_MAX_INSTRUCTION_BYTES),
        reconnect: ReconnectSchema,
    }),
    z.object({
        transport: z.const('streamable-http'),
        serverName: z.string().required().pattern(SERVER_NAME_PATTERN),
        url: z.string().required(),
        headers: z.dict(String).default({}),
        toolCallTimeoutMs: z.number().default(DEFAULT_TOOL_CALL_TIMEOUT_MS),
        startupTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS),
        closeTimeoutMs: z.number().min(1).max(MAX_TIMER_DELAY_MS).default(DEFAULT_CLOSE_TIMEOUT_MS),
        expandTemplates: z.boolean().default(false),
        failOnStartupError: z.boolean().default(false),
        maxInstructionBytes: z.number().step(1).min(1).default(DEFAULT_MAX_INSTRUCTION_BYTES),
        reconnect: ReconnectSchema,
    }),
]);
/* jscpd:ignore-end */
// ---- Plugin apply ----
/**
 * Connect one MCP server and publish its initial tool generation before activation.
 * This entry remains explicitly `async`: Cordis treats a prototype-bearing
 * ordinary function as a constructor, whose returned Promise is not startup work.
 * @param ctx - plugin context carrying the tool registry.
 * @param config - resolved transport and server namespace configuration.
 * @returns startup readiness after connection and initial tool discovery settle.
 */
export async function apply(ctx, config) {
    // Fail loud at load: reconnect misconfiguration (including programmatic
    // construction that bypassed Schemastery) rejects THIS instance before any
    // effect registers.
    const reconnect = resolveReconnectPolicy(config.reconnect, `mcp-client(${config.serverName}): reconnect`);
    // Namespace reservations are scope-local; status belongs to the actual
    // Fiber and survives its reloads and final disposal.
    let status;
    ctx.effect(() => {
        const owner = scopeOf(ctx) ?? ctx.root;
        let names = activeServerNames.get(owner);
        if (!names) {
            names = new Set();
            activeServerNames.set(owner, names);
        }
        if (names.has(config.serverName)) {
            throw new Error(`mcp-client: serverName "${config.serverName}" is already in use by another mcp-client instance — pick a unique serverName in cordis.yml`);
        }
        names.add(config.serverName);
        status = registerStatusCell(ctx.fiber, (message, error) => {
            ctx.logger.warn(`mcp-client(${config.serverName}): ${message}`);
            if (!config.expandTemplates)
                ctx.logger.warn(error);
        });
        return () => {
            if (ctx.fiber.uid === null)
                status.detach();
            names.delete(config.serverName);
        };
    }, 'mcp-client.serverName');
    // The supervisor owns the client/transport generations, the reconnect
    // loop, and the live tool registrations; disposal stops reconnection,
    // quiesces in-flight work, and unregisters the current generation.
    if (!status.closure().closeConfirmed || status.closure().cleanupError !== undefined) {
        throw new Error(`mcp-client(${config.serverName}): previous connection cleanup is incomplete; restart the Host`);
    }
    const connection = startConnection(ctx, config, reconnect, status);
    let stopping;
    const dispose = () => stopping ??= connection.dispose();
    // Cordis announces unload before awaiting an unfinished apply(). Closing
    // the transport here releases startup requests that are still awaiting a reply.
    // oxlint-disable-next-line typescript/no-misused-promises -- Cordis contains observer failures; the effect also awaits this promise.
    ctx.on('internal/plugin', (fiber) => {
        if (fiber !== ctx.fiber || fiber.uid !== null)
            return;
        return dispose();
    }, { global: true });
    ctx.effect(() => dispose, 'mcp-client.connection');
    registerServerContext(ctx, config.serverName, connection);
    // Block plugin activation on the initial connection + tool discovery so
    // Cordis consumers observe the tools immediately after the fiber activates.
    // When failOnStartupError is true, a failed initial attempt rejects the
    // fiber (Cordis rolls it back); otherwise the error is logged and the
    // supervisor applies its reconnect or credential-wait policy.
    const outcome = await connection.ready;
    if (outcome.error !== undefined && config.failOnStartupError) {
        throw new Error(`mcp-client(${config.serverName}): initial connection or tool synchronization failed`, { cause: outcome.error });
    }
}
//# sourceMappingURL=index.js.map