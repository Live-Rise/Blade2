/** Runtime template resolution and secret redaction for one MCP instance. */
import type { Context } from '@deepseek-ai/cordis';
import type { Config } from './index.ts';
/** Resolves raw configuration through the current credential provider and redacts every value it resolved. */
export declare class McpTemplateBinding {
    private readonly ctx;
    private readonly config;
    /** Enabled credential references from raw configuration, including currently unconfigured references. */
    readonly refs: ReadonlySet<string>;
    /** Resolved values and their URL-encoded forms, retained for the instance lifetime. */
    private readonly secrets;
    constructor(ctx: Context, config: Config);
    /**
     * Replace every value this binding resolved, including earlier rotations.
     * @param text - diagnostic or upstream text.
     * @returns the text with resolved values replaced by `[redacted]`.
     */
    redact(text: string): string;
    /**
     * Format a failure, naming the operation and redacting resolved values when templates are enabled.
     * @param error - caught operation failure.
     * @param operation - local operation name for the diagnostic.
     * @returns diagnostic text safe for the configured template mode.
     */
    diagnostic(error: unknown, operation: string): string;
    /**
     * Resolve transport strings without changing the plugin's raw configuration.
     * @param signal - cancellation checked before and after template resolution.
     * @returns resolved transport configuration, or the raw configuration when expansion is disabled.
     * @throws when a credential or the credentials provider is unavailable.
     */
    resolve(signal: AbortSignal): Promise<Config>;
}
//# sourceMappingURL=credentials.d.ts.map