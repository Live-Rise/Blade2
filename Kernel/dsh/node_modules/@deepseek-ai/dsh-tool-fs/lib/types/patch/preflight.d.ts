/** Read-only simulation of patch effects; no internal read establishes model observation. */
import type { Context } from '@deepseek-ai/cordis';
import type { SandboxExecutionPolicy } from '@deepseek-ai/dsh-sandbox';
import type { ToolExecution } from '@deepseek-ai/dsh-tools';
import type { FsSandboxController } from '../sandbox.ts';
import type { PatchRefusal, PlannedOperation } from './types.ts';
/**
 * Collect every independent preflight refusal while simulating only admitted operations.
 * Sandbox containment is the provider's decision at commit, as for write and edit.
 * @param ctx - filesystem service owner; reads never emit observations.
 * @param sandbox - model-facing denial mapper.
 * @param exec - observation actor and cancellation signal.
 * @param policy - resolved confinement policy used to label refusals.
 * @param operations - complete resolved operation plan.
 * @returns refused operations in patch order; any refusal forbids the commit phase.
 */
export declare function preflightPatch(ctx: Context, sandbox: FsSandboxController, exec: ToolExecution, policy: SandboxExecutionPolicy | undefined, operations: readonly PlannedOperation[]): Promise<PatchRefusal[]>;
//# sourceMappingURL=preflight.d.ts.map