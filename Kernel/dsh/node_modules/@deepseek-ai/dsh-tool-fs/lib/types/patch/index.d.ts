/** Internal patch executor; tool registration and persisted result metadata are owned by its caller. */
import type { Context } from '@deepseek-ai/cordis';
import type { ToolExecution } from '@deepseek-ai/dsh-tools';
import type { FsSandboxController } from '../sandbox.ts';
import type { PatchExecutionReport, PatchExecutionRequest } from './types.ts';
export type { PatchExecutionReport, PatchExecutionRequest, PatchFailure, PatchFailureCode, PatchFileChange, PatchOperationOutcome, PatchOperationRecord, PatchRefusal } from './types.ts';
/**
 * Preflight a whole patch before sequential guarded mutations; business failures become records.
 * @param ctx - mounted filesystem and observation-policy events.
 * @param sandbox - denial mapper shared with existing filesystem tools.
 * @param exec - session cwd, observation actor, and cancellation signal.
 * @param request - raw patch and policy already resolved by the caller.
 * @returns the syntax failure, the preflight refusals, or ordered commit outcomes.
 * @throws unexpected programming defects without starting further operations.
 */
export declare function executePatch(ctx: Context, sandbox: FsSandboxController, exec: ToolExecution, request: PatchExecutionRequest): Promise<PatchExecutionReport>;
//# sourceMappingURL=index.d.ts.map