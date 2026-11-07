import type { Context } from '@deepseek-ai/cordis';
import type { PtcProgramFormat, PtcRunRequest, PtcRunResult, PtcRunSpec } from '@deepseek-ai/dsh-ptc-runtime';
import type { ResolvedConfig } from './config.ts';
import type { PolicyProcesses } from './policy-processes.ts';
/** Syntax, state retention and model guidance selected during provider construction. */
export interface ExecutionMode {
    /** Child evaluator selected independently of process lifetime. */
    readonly format: PtcProgramFormat;
    /** Whether caller-owned objects retain processes between runs. */
    readonly retainsState: boolean;
    /** Model-facing syntax, output and recovery rules. */
    readonly executionInstructions: string;
    /**
     * Erase TypeScript in the selected syntax without executing the program.
     * @param program - Model-written TypeScript source.
     * @returns JavaScript accepted by the selected evaluator.
     */
    stripTypes(program: string): string;
}
/**
 * Select program semantics and create their shared process owner.
 * @param ctx - Provider lifecycle and execution services.
 * @param config - Validated configuration with loader defaults.
 * @param processes - Policy invalidation and process cleanup coordination.
 * @returns Mode selection and temporary or retained execution through one process implementation.
 */
export declare function createExecution(ctx: Context, config: ResolvedConfig, processes: PolicyProcesses): NodeExecution;
/** Temporary processes and optional object-owned reuse share one process implementation. */
declare class NodeExecution {
    private readonly ctx;
    private readonly config;
    private readonly processes;
    readonly mode: ExecutionMode;
    private readonly owners;
    private readonly workers;
    /**
     * Own process instances and optional owner lookup for one provider.
     * @param ctx - Provider lifecycle and execution services.
     * @param config - Validated limits and selected executable.
     * @param processes - Provider-owned policy invalidation and cleanup.
     * @param mode - Program syntax and TypeScript erasure.
     */
    constructor(ctx: Context, config: ResolvedConfig, processes: PolicyProcesses, mode: ExecutionMode);
    /** Stop every owned process, retaining failed cleanup for a later retry. */
    dispose(): Promise<void>;
    /**
     * Resolve the common file policy, directory and deadline defaults.
     * @param request - Program, bindings, owner, directory and elapsed budget.
     * @returns Resolved inputs with the selected file policy.
     */
    resolve(request: PtcRunRequest): PtcRunSpec;
    /**
     * Evaluate in a temporary process or reuse an enabled state owner's process.
     * @param spec - Resolved request; concurrent calls for one owner are rejected.
     * @returns Captured logs, JSON completion, or a structured execution failure.
     */
    run(spec: PtcRunSpec): Promise<PtcRunResult>;
    /**
     * Release retained state and await its process cleanup.
     * @param owner - Exact object supplied to run.
     * @returns Completion after the owned process and output streams stop.
     */
    release(owner: object): Promise<void>;
}
export {};
//# sourceMappingURL=execution.d.ts.map