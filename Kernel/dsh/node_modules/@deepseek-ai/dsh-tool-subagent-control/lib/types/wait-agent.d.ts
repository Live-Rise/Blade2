/** Input waiting owned by the subagent control plugin. */
import type { Context } from '@deepseek-ai/cordis';
/**
 * Register one inbox wait tool and cancel its waits when the plugin unloads.
 * @param ctx - plugin context owning the tool, event listeners, and wait lifetime.
 */
export declare function registerWaitAgent(ctx: Context): void;
//# sourceMappingURL=wait-agent.d.ts.map