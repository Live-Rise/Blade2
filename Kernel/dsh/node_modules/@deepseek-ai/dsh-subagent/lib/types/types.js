/**
 * The seam's consumer-facing contracts: request, result, and capability types
 * for {@link SubagentProvider}, plus the `subagent/start` and `subagent/end`
 * payloads that plugins and hosts observe. Internal control interfaces, such
 * as the manager host, belong with their implementation.
 *
 * @module @deepseek-ai/dsh-subagent/types
 */
/**
 * Brand a string as a {@link SubagentRunId}.
 * @param id - the raw run id.
 * @returns the same string, branded.
 */
export function SubagentRunId(id) {
    return id;
}
//# sourceMappingURL=types.js.map