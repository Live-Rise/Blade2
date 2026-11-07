import { Context } from '@deepseek-ai/cordis';
import type { PtcRunSpec, PtcRunResult, PtcBindingNamespace } from '@deepseek-ai/dsh-ptc-runtime';
import type { SandboxExecutionPolicy } from '@deepseek-ai/dsh-sandbox';
import type { PolicyProcesses } from './policy-processes.ts';
import type { ResolvedConfig } from './config.ts';
import type { ExecutionMode } from './execution.ts';
/** One managed process; all mutable program state belongs to this process alone. */
export declare class ProgramWorker {
    private readonly ctx;
    private readonly config;
    readonly policy: SandboxExecutionPolicy;
    private readonly mode;
    private readonly retainsState;
    private readonly unregister;
    private confined;
    private handle?;
    private channel?;
    private active;
    /** Admission remains reserved until the result includes completed process cleanup. */
    private running;
    private cleanupPending;
    private ready;
    private stderr;
    private controlFailure;
    private stopped;
    private starting?;
    private exitOutcome?;
    private readonly controller;
    closed: boolean;
    /** Whether termination succeeded and this process no longer needs cleanup retries. */
    get cleanupComplete(): boolean;
    /**
     * Own one process under a fixed file policy.
     * @param ctx - Provider execution services.
     * @param config - Validated resource limits and launch selection.
     * @param policy - Authority retained for the lifetime of this process.
     * @param processes - Session policy invalidation and cleanup coordination.
     * @param mode - Evaluator syntax and TypeScript erasure.
     * @param retainsState - Whether this process belongs to an owner and survives completed evaluations.
     */
    constructor(ctx: Context, config: ResolvedConfig, policy: SandboxExecutionPolicy, processes: PolicyProcesses, mode: ExecutionMode, retainsState: boolean);
    /** Reject reuse until the previous evaluation and required cleanup settle. */
    assertIdle(): void;
    /**
     * Evaluate a program, retaining this process only for an enabled state owner.
     * @param spec - Resolved program inputs and cancellation.
     * @param bindings - Current host binding implementations.
     * @returns Output after any required process cleanup.
     */
    run(spec: PtcRunSpec, bindings: Map<string, PtcBindingNamespace>): Promise<PtcRunResult>;
    private output;
    private message;
    private abortMessage;
    private evaluate;
    /** Cancel active work and await managed process termination and output drainage. */
    dispose(): Promise<void>;
    private stop;
    private fail;
    private dispatch;
    private withControlFailure;
    private exitMessage;
    private start;
}
//# sourceMappingURL=worker.d.ts.map