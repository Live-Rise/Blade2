/**
 * Model-facing `schedule_create`, `schedule_list`, `schedule_view`, and
 * `schedule_edit` tools over the Host `ctx.schedule` service. Mounting the
 * plugin registers them in the mounting scope, so a preset decides which
 * agents receive them. Tasks belong to the Host, not to a Session: the tools
 * read and change any task by id, and only the delivery target names a Session.
 * @module @deepseek-ai/dsh-tool-schedule
 */
import type { Context } from '@deepseek-ai/cordis';
/** Plugin name registered with the Loader. */
export declare const name = "tool-schedule";
/** Host services this plugin consumes. The Schedule service is resolved per
 * scope, so a preset that declares this plugin stays inert while the shipped
 * composition keeps that service off. */
export declare const inject: string[];
/**
 * Register the four Schedule tools and shared guidance in the mounting scope.
 *
 * Each call acts for the Agent that dispatched it, so a mount outside an
 * agent scope still registers the tools but every call without a caller Agent
 * is refused as an internal failure.
 * @param ctx - Context owning the tool and system-prompt registries and the Host Schedule service.
 */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=index.d.ts.map