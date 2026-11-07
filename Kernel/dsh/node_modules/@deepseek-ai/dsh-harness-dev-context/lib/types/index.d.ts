/**
 * @deepseek-ai/dsh-harness-dev-context — model context for an agent that can
 * inspect and extend the DeepSeek Harness serving its Web GUI. It registers
 * path- and URL-free prompt sections and states the current implementation
 * checkout and Web GUI URL as runtime context, so a process started from
 * another checkout, port, or installation leaves the system prompt unchanged.
 * A Session whose admitted prompt holds the paragraphs that embedded those
 * values keeps them verbatim, so resuming it never rewrites its logged prompt.
 * @module @deepseek-ai/dsh-harness-dev-context
 */
import type { Context } from '@deepseek-ai/cordis';
/** Stable Cordis plugin name. */
export declare const name = "harness-dev-context";
/** Prompt registries, the projection registry, and the Web GUI whose URL the model learns. */
export declare const inject: string[];
/**
 * Register the two sections, their runtime-context values, and the
 * projection that keeps a Session's admitted paragraphs.
 * @param ctx - plugin context with every {@link inject} service.
 * @returns after the projection reader and prompt registrations are ready.
 */
export declare function apply(ctx: Context): Promise<void>;
//# sourceMappingURL=index.d.ts.map