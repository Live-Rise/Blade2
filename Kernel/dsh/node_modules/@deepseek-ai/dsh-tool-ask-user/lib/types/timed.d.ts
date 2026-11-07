/**
 * Opt-in timed `ask_user_question` tool definition.
 */
import type { Context } from '@deepseek-ai/cordis';
/** Model-facing meaning of a skipped answer item, shared by the blocking and timed descriptions. */
export declare const skippedAnswerGuidance: string;
/**
 * Register the opt-in timed tool definition.
 * @param ctx - Agent-scoped context receiving the tool.
 * @param timeout - Default foreground wait in seconds.
 */
export declare function registerTimedAskUser(ctx: Context, timeout?: number): void;
//# sourceMappingURL=timed.d.ts.map