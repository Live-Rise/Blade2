import type { Fiber } from '@deepseek-ai/cordis';
import type { McpCloseResult, McpConnectionStatus } from './types.ts';
/**
 * Connection status and cleanup results keyed by the owning Cordis Fiber.
 * The cell survives reloads of that Fiber and remains readable after final
 * disposal, which detaches existing observers.
 *
 * @module
 */
/** Read-and-observe handle over one instance's {@link McpConnectionStatus}. */
export interface McpConnectionStatusHandle {
    /**
     * The current status.
     * @returns the latest status the supervisor published.
     */
    current(): McpConnectionStatus;
    /**
     * The latest generation's cleanup result, independent of connection status.
     * @returns unconfirmed while a generation is open, or its recorded cleanup result.
     */
    closure(): McpCloseResult;
    /**
     * Observe status transitions. Callbacks run synchronously in publish order;
     * a throwing callback is contained and logged without affecting the
     * supervisor or other callbacks.
     * @param callback - invoked with each published status.
     * @returns the disposer removing this observer.
     */
    onChange(callback: (status: McpConnectionStatus) => void | Promise<void>): () => void;
}
/** Writable status cell retained across supervisors on the same Fiber. */
export declare class McpStatusCell implements McpConnectionStatusHandle {
    private warn;
    private value;
    private closeResult;
    closure(): McpCloseResult;
    /** Mark the next generation's closure as unconfirmed. */
    opened(): void;
    /**
     * Record cleanup without publishing a connection-status transition.
     * @param result - independently observed transport and local cleanup outcome.
     */
    recordClosure(result: McpCloseResult): void;
    /** Remove existing observers without discarding the readable status or cleanup result. */
    detach(): void;
    private readonly listeners;
    /**
     * @param warn - contained-listener diagnostic sink (the owning plugin's logger).
     */
    constructor(warn: (message: string, error: unknown) => void);
    /**
     * Associate observer diagnostics with the Fiber's current supervisor.
     * @param warn - contained-listener diagnostic sink.
     */
    setWarningSink(warn: (message: string, error: unknown) => void): void;
    /**
     * The current status.
     * @returns the latest published status.
     */
    current(): McpConnectionStatus;
    /**
     * Observe status transitions; see {@link McpConnectionStatusHandle.onChange}.
     * @param callback - invoked with each published status.
     * @returns the disposer removing this observer.
     */
    onChange(callback: (status: McpConnectionStatus) => void | Promise<void>): () => void;
    /**
     * Publish the next status and notify observers. A throwing observer is
     * contained so a broken consumer cannot wedge the supervisor's transition.
     * @param next - the status to publish.
     */
    update(next: McpConnectionStatus): void;
}
/**
 * Create or reuse a Fiber's status cell without discarding prior cleanup results.
 * @param fiber - the actual plugin Fiber owning the supervisor.
 * @param warn - diagnostic sink for observer failures.
 * @returns the stable writable cell for this Fiber.
 */
export declare function registerStatusCell(fiber: Fiber, warn: (message: string, error: unknown) => void): McpStatusCell;
/**
 * Look up status through an actual Fiber or the handle returned by `ctx.plugin()`.
 * @param fiber - plugin Fiber or its thenable handle.
 * @returns the retained cell, or `undefined` before the MCP instance registers one.
 */
export declare function connectionStatusIn(fiber: Fiber): McpConnectionStatusHandle | undefined;
//# sourceMappingURL=status.d.ts.map