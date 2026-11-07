/**
 * Conversation Definitions producing the performance target's nodes: one step
 * context per `step/start`, one prompt context per user message, one end marker
 * per turn, and one context per Code-Mode root call for its inner dispatches.
 *
 * Every callback is a pure function of the events it receives: turn and step
 * identity comes from the engine's match location, never from module state, so
 * one Definition set serves every Session binding and history paging
 * (`replaceWindow`/`prepend`) cannot misattribute events.
 */
import type { Context } from '@deepseek-ai/cordis';
/** Target name shared by every Definition and the view builder. */
export declare const TARGET = "stats";
/**
 * Register the six Definitions. Each registration rides the caller's fiber
 * effect, so unloading the plugin removes the nodes with it.
 * @param ctx - client root context.
 */
export declare function registerStatsDefinitions(ctx: Context): void;
//# sourceMappingURL=stats-definition.d.ts.map