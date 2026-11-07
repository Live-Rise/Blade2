/**
 * The viewer composer's permission seat: which tier this share carries, stated and not selectable.
 *
 * A reader cannot change what the owner granted, so the seat states the share's own answer — the
 * tier the Relay reported for this share — and says in the page that the reader cannot change it.
 * It renders through {@link ViewerSeat}, whose `readOnly` switch is what keeps it a label; the
 * shipped permission control is shadowed here, because the owner's own preset for the Session is
 * not this reader's permission and offering it would offer a change nothing may make.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/viewer-permission
 */
import { type ReactNode } from 'react';
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import { NS } from './locales.ts';
import { type ViewerCapabilities } from './viewer-capabilities.ts';
export declare const name = "liveshare-viewer-permission";
/** Composer slot registry this seat occupies. */
export declare const inject: readonly string[];
/** The observation face this page's capability store exposes. */
interface CapabilityFace {
    subscribe(listener: () => void): () => void;
    getSnapshot(): ViewerCapabilities;
}
/** Everything the seat reads: what the share's own answer granted this page. */
export interface ViewerPermissionSeatProps {
    /** Whether the composer's own owner disabled the seat; a stated tier stays a label either way. */
    readonly locked: boolean;
    /** The one capability set this page holds for the share it presents. */
    readonly capabilities: CapabilityFace;
}
/**
 * Render the permission this share carries.
 * @param props - the share's own capability answer and the framework-injected translate seat.
 * @returns The seat as a label.
 */
export declare function ViewerPermissionSeat({ capabilities, t }: ViewerPermissionSeatProps & PropsLocale<typeof NS>): ReactNode;
/**
 * Fill the composer's permission seat with the share's own tier.
 * @param ctx - the viewer page's root context.
 */
export declare function apply(ctx: ClientContext): void;
export {};
//# sourceMappingURL=viewer-permission.d.ts.map