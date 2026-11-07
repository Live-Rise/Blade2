/** Remote contribution and slot registrations of the forge panel. */
import type { Context } from '@deepseek-ai/cordis';
import type { TypertRemoteContribution } from '@deepseek-ai/dsh-typert-protocol';
/** Services required before the Remote contribution mounts. */
export declare const inject: string[];
/** Registry id of the forge tab type, also its slot key. */
export declare const TAB_ID = "@deepseek-ai/dsh-experimental-client-ui-forge";
/** Kind of the forge tab. */
export declare const TAB_KIND = "forge-panel";
/**
 * Mount the forge panel Remote, then the UI that calls it; the UI is withdrawn before the Remote.
 * @param ctx - client plugin lifetime.
 * @param contribution - generated forge panel Remote definitions.
 * @returns the disposer that removes both contributions.
 */
export declare function mountForgePanel(ctx: Context, contribution: TypertRemoteContribution): Promise<() => Promise<void>>;
//# sourceMappingURL=mount.d.ts.map