/** Register the card frame, the tool row, their copy and the body registry. */
import type { Context } from '@deepseek-ai/cordis';
import type { TypertRemoteContribution } from '@deepseek-ai/dsh-typert-protocol';
import { ActionCardRegistry } from './registry.ts';
import { type ActionCardKey } from './locales.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        actionCards: ActionCardKey;
    }
}
declare module '@deepseek-ai/cordis' {
    interface Context {
        /** Card bodies by action kind; kinds register theirs inside their own `ctx.effect`. */
        actionCards: ActionCardRegistry;
    }
}
/** Registries available before this plugin mounts its Remote. */
export declare const inject: string[];
/**
 * Provide the body registry, then mount the Remote and the UI for the same plugin lifetime.
 * @param ctx - Client plugin context.
 * @param contribution - Generated Remote descriptors supplied by the built entry.
 * @returns disposer removing the UI before the Remote contribution.
 */
export declare function mountActionCards(ctx: Context, contribution: TypertRemoteContribution): Promise<() => Promise<void>>;
//# sourceMappingURL=mount.d.ts.map