/** Sequential provider commits with truthful partial results and uncertainty checks. */
import type { Context } from '@deepseek-ai/cordis';
import type { SandboxExecutionPolicy } from '@deepseek-ai/dsh-sandbox';
import type { ToolExecution } from '@deepseek-ai/dsh-tools';
import type { FsSandboxController } from '../sandbox.ts';
import type { PatchExecutionReport, PlannedOperation } from './types.ts';
/**
 * Commit a fully admitted plan; the first failure stops later operations without rollback.
 * @param ctx - provider and observation dispatcher.
 * @param sandbox - existing model-facing denial mapper.
 * @param exec - session actor and cancellation signal.
 * @param policy - request's resolved confinement policy.
 * @param operations - operations that all passed preflight.
 * @returns known changes, failed or unknown effects, and skipped successors in patch order.
 */
export declare function commitPatch(ctx: Context, sandbox: FsSandboxController, exec: ToolExecution, policy: SandboxExecutionPolicy | undefined, operations: readonly PlannedOperation[]): Promise<PatchExecutionReport>;
//# sourceMappingURL=execute.d.ts.map