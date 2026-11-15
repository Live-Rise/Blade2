/** Optional Computer Use controls mounted through existing plugin-page and overlay slots. */
import type { Context } from '@deepseek-ai/cordis';
import type { TypertRemoteContribution } from '@deepseek-ai/dsh-typert-protocol';
import { type CuaPermissionLocaleKey } from './locales.ts';
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        /** Native permission controls and application consent. */
        cuaPermissions: CuaPermissionLocaleKey;
    }
}
/** Existing browser registries needed by the optional permission UI. */
export declare const inject: string[];
/**
 * Mount the optional permission Remote and its browser controls.
 * @param ctx Browser plugin context.
 * @param contribution Generated permission Remote definitions.
 * @returns Cleanup for the optional Remote and UI registrations.
 */
export declare function mountCuaPermissions(ctx: Context, contribution: TypertRemoteContribution): Promise<() => Promise<void>>;
//# sourceMappingURL=mount.d.ts.map