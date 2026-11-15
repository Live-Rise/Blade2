/** Source-safe task-board browser registration. */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import { type TeamKey } from './locales.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        /** Shared task-board copy. */
        'agent-team': TeamKey;
    }
}
/** Required browser services for slots and localized copy. */
export declare const inject: string[];
/**
 * Register the task-board dictionaries and subagent catalog sections.
 * The panel reads the root Session’s `agentTeam` projection through standard Session hooks.
 * @param ctx - Client Context carrying the injected locale and slot services.
 */
export declare function registerAgentTeamUi(ctx: ClientContext): void;
//# sourceMappingURL=mount.d.ts.map