import { RUN_CODE_NAME, ToolNotFoundError, projectToolSchema } from '@deepseek-ai/dsh-tools';
import { ToolExecutionPipeline, executeToolBatch } from '@deepseek-ai/dsh-tools/pipeline';
import { createRunCodeTool, requirePtcRuntime } from "./ptc.js";
import { renderToolsSdk } from "./ts-types.js";
import { renderToolsSdkPy } from "./py-types.js";
import { Config } from "./config.js";
const PTC_ONLY_INSTRUCTION = `\`${RUN_CODE_NAME}\` is the only tool you can call directly — a tool call naming any other tool fails. Reach every tool the SDK declares below from inside the program.`;
const SDK_RENDERERS = {
    typescript: renderToolsSdk,
    python: renderToolsSdkPy,
};
/** Runs programs whose nested tool calls use the shared guarded invocation pipeline. */
export class PtcExecutor {
    ctx;
    maxParallel;
    sections;
    pipeline;
    maxParallelSubCalls;
    ptcTransport;
    constructor(ctx, config = {}, maxParallel = () => 10) {
        this.ctx = ctx;
        this.maxParallel = maxParallel;
        this.maxParallelSubCalls = Config(config).maxParallelSubCalls;
        const tools = ctx.tools;
        this.sections = [
            { name: 'tools:ptc-only', order: ctx.systemPrompt.getSectionOrder('PTC_ONLY'), text: PTC_ONLY_INSTRUCTION, interpolate: false },
            { name: 'tools:sdk', order: ctx.systemPrompt.getSectionOrder('TOOLS_SDK'), text: tools => this.renderSdk(tools), interpolate: false },
        ];
        this.pipeline = new ToolExecutionPipeline(ctx, {
            get: (name, scope, toolSet) => name === RUN_CODE_NAME ? this.requirePtcTransport() : tools.executable(name, scope, toolSet),
            reject: (name, nested) => !nested && name !== RUN_CODE_NAME
                ? new ToolNotFoundError(name, `only \`${RUN_CODE_NAME}\` is callable directly — call \`${name}\` from inside a \`${RUN_CODE_NAME}\` program instead`)
                : undefined,
        });
    }
    describe(_tools) {
        return { schemas: [projectToolSchema(this.requirePtcTransport(), false)], knownNames: [RUN_CODE_NAME] };
    }
    renderSdk(tools) {
        const runtime = requirePtcRuntime(this.ctx);
        const render = SDK_RENDERERS[runtime.language];
        return render(tools.map(definition => ({
            ...projectToolSchema(definition, false),
            output: definition.output.schema,
        })), runtime.programFormat);
    }
    execute(input) {
        return executeToolBatch(input, this.pipeline, this.maxParallel);
    }
    /** Keep the reserved transport outside registered capabilities and their restrictions. */
    requirePtcTransport() {
        this.ptcTransport ??= createRunCodeTool(this.ctx, this.pipeline, this.maxParallelSubCalls);
        return this.ptcTransport;
    }
}
export { Config } from "./config.js";
export { CodeRunFailedError } from "./errors.js";
export { jsonSchemaToTs, renderToolsSdk } from "./ts-types.js";
export { jsonSchemaToPy, renderToolsSdkPy } from "./py-types.js";
/** Cordis plugin name. */
export const name = 'ptc-tool-executor';
/** Activate the executor only while its registry, prompt service, and runtime are available. */
export const inject = ['tools', 'systemPrompt', 'ptcRuntime'];
/**
 * Select PTC execution in the mounting scope.
 * @param ctx - context owning this executor's selection and lifetime.
 * @param config - program subcall concurrency limits.
 */
export function apply(ctx, config) {
    ctx.tools.setExecutor(new PtcExecutor(ctx, config));
}
//# sourceMappingURL=index.js.map