/** Patch parsing and target identity shared by preflight and sequential commits. */
import type { Context } from '@deepseek-ai/cordis';
import { FsError } from '@deepseek-ai/dsh-fs';
import type { FsTarget, FsVersion, FsWriteIntent } from '@deepseek-ai/dsh-fs';
import type { SandboxExecutionPolicy } from '@deepseek-ai/dsh-sandbox';
import type { ToolExecution } from '@deepseek-ai/dsh-tools';
import type { FsSandboxController } from '../sandbox.ts';
import type { PatchExecutionRequest, PatchFailure, PatchPlan, PlannedOperation } from './types.ts';
/**
 * Resolve every operation without dropping repeated aliases or splitting moves.
 * @param ctx - filesystem service owner.
 * @param sandbox - filesystem diagnostic mapper.
 * @param exec - session cwd and execution signal.
 * @param request - raw patch and resolved per-call policy.
 * @returns resolved operations sharing one target per canonical key, plus resolution refusals.
 */
export declare function planPatch(ctx: Context, sandbox: FsSandboxController, exec: ToolExecution, request: PatchExecutionRequest): Promise<PatchPlan>;
/**
 * Obtain a source guard without recording an observation.
 * @param ctx - intent dispatcher.
 * @param exec - opaque observation actor.
 * @param target - resolved source.
 * @param kind - deletion or edit-family source policy.
 * @returns the observed version, or undefined for a bare provider.
 */
export declare function sourceIntent(ctx: Context, exec: ToolExecution, target: FsTarget, kind: 'delete' | 'edit'): Promise<{
    version: FsVersion;
} | undefined>;
/**
 * Obtain a write guard without recording an observation.
 * @param ctx - intent dispatcher.
 * @param exec - opaque observation actor.
 * @param target - resolved write destination.
 * @returns the observed write intent, or undefined for a bare provider.
 */
export declare function destinationIntent(ctx: Context, exec: ToolExecution, target: FsTarget): Promise<FsWriteIntent | undefined>;
/**
 * Preserve filesystem codes while applying the existing model-facing error policy.
 * @param sandbox - shared denial mapper.
 * @param policy - policy supplied by the request.
 * @param error - expected filesystem failure.
 * @param path - display path whose operation failed.
 * @returns the remediated code and message; unexpected mapper defects reject.
 */
export declare function patchFailure(sandbox: FsSandboxController, policy: SandboxExecutionPolicy | undefined, error: FsError, path: string): PatchFailure;
/**
 * List all targets the operation can mutate, with destination first for a move.
 * @param operation - resolved operation.
 * @returns targets whose versions the uncertainty comparison records.
 */
export declare function mutationTargets(operation: PlannedOperation): FsTarget[];
//# sourceMappingURL=plan.d.ts.map