/**
 * Model-facing delegation through one configured `ctx.subagents` provider.
 * Provider lifecycle controls tool registration and context-sensitive schema
 * wording. Every delegation starts a managed activation and returns its child
 * id; the subagent runtime owns result delivery and resource cleanup.
 * @module @deepseek-ai/dsh-tool-subagent
 */
import type { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { AgentOptions } from '@deepseek-ai/dsh-agent';
import { type PresetSelectionConfig } from './preset-selection.ts';
export declare const name = "tool-subagent";
export declare const inject: string[];
/** Config: which registered provider this tool delegates to, plus child defaults. */
export interface Config extends PresetSelectionConfig {
    /** The `ctx.subagents` provider name to start runs on (e.g. `spawn`, `acp`). */
    provider: string;
    /**
     * Model-facing tool name (default `subagent`). Each loaded instance must use
     * a distinct name.
     */
    toolName?: string;
    /**
     * Expose optional child `provider`, `model`, and `reasoning_effort` fields.
     * Requires provider `agentOptions` support. Execution enforces the Session
     * policy, or restricts explicit selection to the current parent route.
     */
    modelSelection?: boolean;
    /**
     * Register the shared `list_subagent_models` tool for this scope.
     * Enable on only one instance per scope; defaults to false and does not
     * enable model selection. Discovery is removed when this instance unloads.
     */
    modelDiscovery?: boolean;
    /**
     * Agent options applied to every child; omitted fields use child-loop defaults.
     */
    agentOptions?: AgentOptions;
    /**
     * Maximum child depth: a non-negative safe integer (`0` forbids delegation),
     * or `'provider-managed'` to send no cap. A numeric cap
     * requires the provider's `depthLimit` capability (mount fails loud
     * otherwise). The provider checks the calling agent's current depth at every
     * start; the tool remains model-visible so runtime policy owns rejection.
     * `'provider-managed'` is for an out-of-process provider whose recursion
     * budget belongs to the child runtime or its own deployment. Omission reads
     * the current Host subagent depth setting (default `1`) at each delegation.
     */
    maxDepth?: number | 'provider-managed';
}
export declare const Config: z<Config>;
/**
 * Install one delegation-tool composition.
 * The owning composition shares its registrations across Agents; child startup resolves the selected preset.
 * @param ctx - Context that owns the registrations.
 * @param config - delegation-tool configuration.
 * @returns when the plugin is ready.
 */
export declare function apply(ctx: Context, config: Config): Promise<void>;
//# sourceMappingURL=index.d.ts.map