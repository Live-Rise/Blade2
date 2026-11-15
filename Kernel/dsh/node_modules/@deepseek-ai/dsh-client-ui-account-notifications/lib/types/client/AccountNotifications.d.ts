import type { HostObservable, InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { AccountNotice } from './notification-controller.ts';
import type { NotificationKey } from './locales.ts';
/** Notification view and interactions supplied by the notification plugin. */
export interface AccountNotificationsInjected {
    hooks: {
        notices: HostObservable<readonly AccountNotice[]>;
    };
    /** @param id - notification that completed a displayed frame. */
    shown: (id: AccountNotice['id']) => void;
    /** @param id - foreground notification whose non-dismiss action was clicked. */
    ctaClicked: (id: AccountNotice['id']) => void;
    /** @param id - notification explicitly closed by the user. */
    dismiss: (id: AccountNotice['id']) => void;
}
/** Notification component props derived from its parent-owned slot. */
export type AccountNotificationsProps = PropsRuntime<'settings.launcher.notifications'> & InjectFace<AccountNotificationsInjected> & PropsLocale<'account.notifications'>;
/** @param props - account-menu availability and notification content. @returns the anchored notification stack. */
export declare function AccountNotifications({ active, openAccount, useNotices, shown, ctaClicked, dismiss, t }: AccountNotificationsProps): false | import("react").JSX.Element;
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        'account.notifications': NotificationKey;
    }
}
/** @param props - server-ordered notices, localized fallback copy, and display callbacks.
 * @returns a non-modal stack whose foreground card alone can report display or be closed.
 */
export declare function AccountNoticeStack({ notices, closeLabel, onShown, onCtaClicked, onDismiss, onOpenAccount }: {
    notices: readonly AccountNotice[];
    closeLabel: string;
    /** Open the account settings route while retaining the notification. */
    onOpenAccount: () => void;
    /** Called after the foreground card has passed a visible presented frame. */
    onShown: (id: AccountNotice['id']) => void;
    /** Called when the user clicks any non-dismiss action on the foreground card. */
    onCtaClicked: (id: AccountNotice['id']) => void;
    /** Called when the user closes the foreground card. */
    onDismiss: (id: AccountNotice['id']) => void;
}): import("react").JSX.Element;
//# sourceMappingURL=AccountNotifications.d.ts.map