/**
 * Owner identity and credential state for the share dialog.
 *
 * The owner host is the authority: it verifies the login, holds the credential, renews it while the
 * authorization lasts, and refuses a share it cannot attribute to a verified identity. The client
 * only states what the host reported, and states nothing it did not report — an unreadable or
 * absent answer is `unknown`, never a guess from a display name.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/owner-identity
 */
/**
 * What the host reports about the owner identity this browser will share under.
 *
 * `verified` may create shares, `expired` and `unverified` must verify first, `relay-changed` means
 * the status on hand belongs to another Relay address and cannot be reused, `relay-unsupported`
 * means the address in force cannot verify identities at all, and `unknown` means the host reported
 * nothing the client can read.
 */
export type OwnerIdentityState = 'verified' | 'expired' | 'unverified' | 'relay-changed' | 'relay-unsupported' | 'unknown';
/** Owner identity and credential facts the dialog presents. */
export interface OwnerIdentityStatus {
    readonly state: OwnerIdentityState;
    /** Relay address the reported status was read for; a status never applies to another address. */
    readonly relay: string;
    /** Verified display name, when the identity carried one. */
    readonly displayName?: string;
    /** Avatar the identity provider published for this subject, when it published one. */
    readonly avatarUrl?: string;
    /** Subject the verified identity belongs to; the label of last resort when no name exists. */
    readonly subjectId?: string;
    /** ISO time the owner credential stops being valid. */
    readonly expiresAt?: string;
    /** True when the host renewed the credential without asking the owner to sign in again. */
    readonly renewed?: boolean;
}
/** The one status the client keeps when the host reported none it can read. */
export declare const UNKNOWN_OWNER_IDENTITY: OwnerIdentityStatus;
/**
 * Read the host's identity answer for one Relay address.
 *
 * The host names the state directly; a state this build does not know, or an answer for another
 * Relay, is reported as such instead of being treated as verified or unverified.
 * @param value - Decoded `identity` answer from the owner Remote, or undefined when unreadable.
 * @param relay - Relay address the dialog is addressing.
 * @returns The status the dialog presents.
 */
export declare function readOwnerIdentity(value: unknown, relay: string): OwnerIdentityStatus;
/**
 * Whether this status lets the dialog create a share.
 *
 * Only a verified identity does. An unreadable status refuses too: a host that cannot state what it
 * verified must not be assumed to refuse on its own, and the page is not the place to find out. The
 * host and the Relay refuse independently in every case; this decision only keeps the page from
 * offering an action it cannot attribute.
 * @param status - Status the dialog presents.
 * @returns True when the create action may run.
 */
export declare function mayCreateShare(status: OwnerIdentityStatus): boolean;
/**
 * The address a login round trip may return to.
 *
 * The page may have been opened with an owner startup token or another one-time hand-off in its own
 * query. Passing that address onward verbatim would hand the secret to the Relay and to whatever the
 * callback navigates to, so every credential-looking parameter is dropped and the fragment, which a
 * single-page app may also use for one-time state, never leaves this page.
 * @param current - The page's own address.
 * @returns The return address without credentials or fragment, or the current address when unparsable.
 */
export declare function loginReturnUrl(current: string): string;
/**
 * The Relay entry that verifies an owner, asked to return to a sanitized copy of this address.
 *
 * The owner host and the Relay own what that entry does — the client only names it in one place, so
 * confirming the real path (and whether it delivers the host credential rather than a viewer login)
 * is a one-line change here.
 * @param relay - Relay address in force.
 * @param current - The page's own address.
 * @returns The absolute verification entry with its return address.
 */
export declare function ownerLoginHref(relay: string, current: string): string;
//# sourceMappingURL=owner-identity.d.ts.map