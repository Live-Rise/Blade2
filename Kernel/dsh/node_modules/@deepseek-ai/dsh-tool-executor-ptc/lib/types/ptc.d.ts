/**
 * PTC mode `run_code` transport. Programs call the registry's agent-visible
 * tools through nested executions scheduled under the native concurrency
 * contract; each sub-dispatch is logged for reconstruction, while only the
 * outer curated result enters model history.
 * @module
 */
import { type Context } from '@deepseek-ai/cordis';
import type { PtcRuntime } from '@deepseek-ai/dsh-ptc-runtime';
import { RUN_CODE_NAME } from '@deepseek-ai/dsh-tools';
import type { ToolExecutionPipeline } from '@deepseek-ai/dsh-tools/pipeline';
import type { ToolDefinition } from '@deepseek-ai/dsh-tools';
/** The model-facing name of the PTC mode tool. */
export { RUN_CODE_NAME };
/** Languages with both a transport schema and an SDK renderer. */
export type PtcSdkLanguage = 'typescript' | 'python';
type SupportedPtcRuntime = PtcRuntime & {
    readonly language: PtcSdkLanguage;
};
/**
 * Read the active runtime, rejecting missing providers and unsupported languages.
 * @param ctx - executor context containing the runtime provider.
 * @returns the runtime with a supported SDK language.
 */
export declare function requirePtcRuntime(ctx: Context): SupportedPtcRuntime;
/**
 * Build the reserved `run_code` transport outside registered capabilities.
 * @param ctx - executor context owning policy listeners and active program cleanup.
 * @param executor - shared invocation pipeline for nested calls.
 * @param maxParallel - maximum overlapping concurrency-safe subcalls.
 * @returns the transport definition with runtime-dependent schema text.
 */
export declare function createRunCodeTool(ctx: Context, executor: ToolExecutionPipeline, maxParallel: number): ToolDefinition;
//# sourceMappingURL=ptc.d.ts.map