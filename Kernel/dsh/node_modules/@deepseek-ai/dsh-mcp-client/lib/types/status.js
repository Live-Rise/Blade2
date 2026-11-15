/** Writable status cell retained across supervisors on the same Fiber. */
export class McpStatusCell {
    warn;
    value = { state: 'connecting', attempt: 0 };
    closeResult = { closeConfirmed: true };
    closure() { return this.closeResult; }
    /** Mark the next generation's closure as unconfirmed. */
    opened() { this.closeResult = { closeConfirmed: false }; }
    /**
     * Record cleanup without publishing a connection-status transition.
     * @param result - independently observed transport and local cleanup outcome.
     */
    recordClosure(result) { this.closeResult = result; }
    /** Remove existing observers without discarding the readable status or cleanup result. */
    detach() { this.listeners.clear(); }
    // Stored with an unknown return: dispatch inspects the returned value for
    // a promise to contain, which a void-typed call expression cannot express.
    listeners = new Set();
    /**
     * @param warn - contained-listener diagnostic sink (the owning plugin's logger).
     */
    constructor(warn) {
        this.warn = warn;
    }
    /**
     * Associate observer diagnostics with the Fiber's current supervisor.
     * @param warn - contained-listener diagnostic sink.
     */
    setWarningSink(warn) { this.warn = warn; }
    /**
     * The current status.
     * @returns the latest published status.
     */
    current() {
        return this.value;
    }
    /**
     * Observe status transitions; see {@link McpConnectionStatusHandle.onChange}.
     * @param callback - invoked with each published status.
     * @returns the disposer removing this observer.
     */
    onChange(callback) {
        this.listeners.add(callback);
        return () => void this.listeners.delete(callback);
    }
    /**
     * Publish the next status and notify observers. A throwing observer is
     * contained so a broken consumer cannot wedge the supervisor's transition.
     * @param next - the status to publish.
     */
    update(next) {
        this.value = next;
        for (const listener of [...this.listeners]) {
            try {
                // An async observer's rejection must land in the same containment as
                // a sync throw; the callback type admits it explicitly.
                const returned = listener(next);
                if (typeof returned?.then === 'function') {
                    void Promise.resolve(returned).catch((error) => {
                        this.warn('an onChange status observer failed', error);
                    });
                }
            }
            catch (error) {
                this.warn('an onChange status observer failed', error);
            }
        }
    }
}
const cellsByFiber = new WeakMap();
/**
 * Create or reuse a Fiber's status cell without discarding prior cleanup results.
 * @param fiber - the actual plugin Fiber owning the supervisor.
 * @param warn - diagnostic sink for observer failures.
 * @returns the stable writable cell for this Fiber.
 */
export function registerStatusCell(fiber, warn) {
    let cell = cellsByFiber.get(fiber);
    if (cell === undefined) {
        cell = new McpStatusCell(warn);
        cellsByFiber.set(fiber, cell);
    }
    else
        cell.setWarningSink(warn);
    return cell;
}
/**
 * Look up status through an actual Fiber or the handle returned by `ctx.plugin()`.
 * @param fiber - plugin Fiber or its thenable handle.
 * @returns the retained cell, or `undefined` before the MCP instance registers one.
 */
export function connectionStatusIn(fiber) {
    return cellsByFiber.get(fiber.ctx.fiber);
}
//# sourceMappingURL=status.js.map