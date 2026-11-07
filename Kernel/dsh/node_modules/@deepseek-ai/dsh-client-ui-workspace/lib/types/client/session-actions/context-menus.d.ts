/** Structured Session actions over the workspace owner's existing callbacks. */
import type { Context } from '@deepseek-ai/cordis';
import type { ArchiveSessionInjected, ForkSessionInjected, PinSessionInjected, RenameSessionInjected, SessionIdInjected } from '../contract/slots.ts';
type Actions = Omit<PinSessionInjected & ArchiveSessionInjected, 'hooks'> & ForkSessionInjected & RenameSessionInjected & SessionIdInjected;
/**
 * Register built-in actions while retaining the legacy Slot for external components.
 * @param ctx - workspace plugin context owning menus, access, membership, and locale.
 * @param actions - shared row and dialog callbacks; execution always uses the captured Session.
 */
export declare function registerSessionMenus(ctx: Context, actions: Actions): void;
export {};
//# sourceMappingURL=context-menus.d.ts.map