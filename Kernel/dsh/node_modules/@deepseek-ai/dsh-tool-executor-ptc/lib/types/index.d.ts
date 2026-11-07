/** PTC execution plugin for the mounting Agent, preset, or deployment scope. @module */
import type { Context } from '@deepseek-ai/cordis';
import type { ToolDefinition, ToolExecutor, ToolModelInput, ToolPromptSection, ToolBatchInput, ToolBatchResult } from '@deepseek-ai/dsh-tools';
import { Config } from './config.ts';
/** Runs programs whose nested tool calls use the shared guarded invocation pipeline. */
export declare class PtcExecutor implements ToolExecutor {
    private readonly ctx;
    private readonly maxParallel;
    readonly sections: readonly ToolPromptSection[];
    private readonly pipeline;
    private readonly maxParallelSubCalls;
    private ptcTransport;
    constructor(ctx: Context, config?: Partial<Config>, maxParallel?: () => number);
    describe(_tools: readonly ToolDefinition[]): ToolModelInput;
    private renderSdk;
    execute(input: ToolBatchInput): Promise<ToolBatchResult>;
    /** Keep the reserved transport outside registered capabilities and their restrictions. */
    private requirePtcTransport;
}
export { Config } from './config.ts';
export { CodeRunFailedError } from './errors.ts';
export type { PtcDispatchLog } from './runtime-types.ts';
export type { PtcDispatchStartEventData, PtcDispatchEventData } from './types.ts';
export { jsonSchemaToTs, renderToolsSdk, type ToolSdkSchema } from './ts-types.ts';
export { jsonSchemaToPy, renderToolsSdkPy } from './py-types.ts';
/** Cordis plugin name. */
export declare const name = "ptc-tool-executor";
/** Activate the executor only while its registry, prompt service, and runtime are available. */
export declare const inject: string[];
/**
 * Select PTC execution in the mounting scope.
 * @param ctx - context owning this executor's selection and lifetime.
 * @param config - program subcall concurrency limits.
 */
export declare function apply(ctx: Context, config: Config): void;
//# sourceMappingURL=index.d.ts.map