/** Program execution failures reported by PTC. @module */
import { HarnessError } from '@deepseek-ai/dsh-llm';
/**
 * Host-only abort reason for an exhausted tool-call allowance; the excess call is never dispatched.
 * The executor recognizes this class at settlement and reports CODE_RUN_FAILED to the model.
 */
export declare class ToolCallLimitExceededError extends Error {
    /**
     * Describe the rejected call and how to correct the program.
     * @param limit - Timeout-derived maximum tool-call attempts for one run_code invocation.
     * @param callNumber - One-based attempt rejected without invoking the tool.
     * @param callsPerMinute - Attempts granted per minute of run_code timeout.
     */
    constructor(limit: number, callNumber: number, callsPerMinute: number);
}
/**
 * Thrown by `run_code` when the program run itself failed — a program
 * exception, a budget expiry, a runtime-local abort, or substrate death. Caller
 * cancellation uses the shared `AbortError` / `ABORTED` tool outcome. Extends
 * {@link HarnessError} (`code: 'CODE_RUN_FAILED'`); the registry's execution
 * pipeline converts it into a structured `isError` result whose text carries
 * the failure kind plus the captured logs, so the model can self-correct.
 */
export declare class CodeRunFailedError extends HarnessError {
    constructor(message: string);
}
/** A host reset is unsupported or failed; cleanup failures require an explicit retry. */
export declare class CodeResetFailedError extends HarnessError {
    constructor(message: string);
}
//# sourceMappingURL=errors.d.ts.map