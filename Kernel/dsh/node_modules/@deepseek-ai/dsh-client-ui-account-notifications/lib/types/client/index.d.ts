/** Account notifications driven by the existing account UI stream. */
import type { Context } from '@deepseek-ai/cordis';
export type { AccountNotificationsInjected, AccountNotificationsProps } from './AccountNotifications.tsx';
/** Services used for notification lifecycle and presentation. */
export declare const inject: string[];
/** @param ctx - client plugin context sharing the account owner's existing state stream. */
export declare function apply(ctx: Context): void;
//# sourceMappingURL=index.d.ts.map