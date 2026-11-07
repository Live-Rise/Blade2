/** Native execution plugin for the mounting Agent, preset, or deployment scope. @module */
import type { Context } from '@deepseek-ai/cordis';
import type { ToolDefinition, ToolExecutor, ToolModelInput, ToolBatchInput, ToolBatchResult } from '@deepseek-ai/dsh-tools';
/** Executes visible capabilities directly and owns their complete batch settlement. */
export declare class NativeToolExecutor implements ToolExecutor {
    private readonly maxParallel;
    private readonly pipeline;
    constructor(ctx: Context, maxParallel?: () => number);
    describe(tools: readonly ToolDefinition[]): ToolModelInput;
    execute(input: ToolBatchInput): Promise<ToolBatchResult>;
}
/** Cordis plugin name. */
export declare const name = "native-tool-executor";
/** Tool registration and scoped dispatch. */
export declare const inject: string[];
/**
 * Select native execution in the mounting scope.
 * @param ctx - context owning this executor's selection and lifetime.
 */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=index.d.ts.map