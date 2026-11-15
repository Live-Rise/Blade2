/**
 * Lifecycle of account notifications: one foreground card in a stack, one read result in
 * force at a time, and independent display, action-click, and dismissal reports. Account activation
 * and Settings entry replace the polling cycle with an immediate read. Each read
 * settles before the next delay begins; the server owns unconsumed notifications.
 *
 * A displayed event is sent once the foreground card reports a presented frame: the card has
 * a position, the document is visible, and it painted one frame. The card measures
 * no cover, so a card painted under the open Settings overlay reports like any
 * other. Business-action clicks report without waiting or closing the card.
 * Explicit dismissal sends only a dismissed event.
 * Expiry and account cleanup do not report dismissal.
 *
 * One lifecycle covers one signed-in session. Login-progress updates retain the
 * card and active poll. Credential and connection changes end
 * that lifecycle before another begins. Signing out and unloading also clear
 * the cards; a later sign-in shows whatever the server offers.
 * @module @deepseek-ai/dsh-client-ui-account-notifications/src/client/notification-controller
 */
import type { AccountNotification, AccountNotificationBatch, AccountNotificationEvent, AccountNotificationId } from '@deepseek-ai/dsh-deepseek-account/types';
/** Notification content currently available to the sidebar presentation. */
export type AccountNotice = AccountNotification;
/** Collaborators and polling interval of one account notification lifecycle. */
export interface NotificationControllerOptions {
    /** Initial delay for each poll, retained until the server provides a new one. */
    readonly pollIntervalMs: number;
    /** UI language in effect when the controller starts. */
    readonly locale: string;
    /** Read the signed-in account's unconsumed notifications. @returns the batch, or null when no account is signed in. */
    read: (locale: string) => Promise<AccountNotificationBatch | null>;
    /**
     * Query localized content for an existing card without changing consumption.
     * @param id - notification requested by stable identity.
     * @param locale - language captured for this request.
     * @returns content, or null when the account changed; failures reject.
     */
    readContent: (id: AccountNotificationId, locale: string) => Promise<AccountNotification | null>;
    /**
     * Report one notification interaction.
     * @param id - notification the user interacted with.
     * @param event - displayed frame, business-action click, or explicit dismissal.
     * @returns false when the account changed or signed out; a protocol failure rejects.
     */
    reportEvent: (id: AccountNotificationId, event: AccountNotificationEvent) => Promise<boolean>;
    /** @param notices - notices in front-to-back order; empty when none are shown. */
    publish: (notices: readonly AccountNotice[]) => void;
}
/** Commands the account plugin issues to the account notification lifecycle. */
export interface NotificationController {
    /**
     * Start the signed-in lifecycle and restart polling with an immediate read.
     * Re-entry retains cards; the caller ends the lifecycle on credential or connection changes.
     */
    begin(): void;
    /** Stop the lifecycle, drop the displayed notice, and ignore late event failures. */
    end(): void;
    /**
     * Restart polling with an immediate read, retaining cards.
     * @returns after the immediate read settles, without waiting for later polls.
     */
    refresh(): Promise<void>;
    /**
     * Refresh current cards in a new UI language, retaining order and consumption.
     * @param locale - active UI language; an unchanged value issues no request.
     * @returns after content requests settle; failures retain the previous content.
     */
    setLocale(locale: string): Promise<void>;
    /** @param id - notice whose card passed a presented frame while visible. */
    shown(id: AccountNotificationId): void;
    /** @param id - foreground notice whose non-dismiss action was clicked; reports without removing the card. */
    ctaClicked(id: AccountNotificationId): void;
    /** @param id - foreground notice the user explicitly closed; reports only a dismissed event. */
    dismiss(id: AccountNotificationId): void;
}
/**
 * Own the read, display, and interaction reporting lifecycle of account notifications.
 * @param options - collaborators and timings.
 * @returns the controller the account plugin drives from account and settings state.
 */
export declare function createNotificationController(options: NotificationControllerOptions): NotificationController;
//# sourceMappingURL=notification-controller.d.ts.map