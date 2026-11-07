/**
 * Owner identity and Relay sign-in state for the share dialog.
 *
 * The owner host is the authority: it verifies the login, holds the credential, renews it while the
 * authorization lasts, and refuses a share it cannot attribute to a verified identity. This hook only
 * states what the host reported, and states nothing it did not report — an unreadable or absent answer
 * is `unknown`, never a guess from a display name.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/owner-identity-login
 */
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import { type OwnerIdentityStatus } from './owner-identity.ts';
import { type OwnerLoginFailure, type OwnerLoginPhase } from './owner-login.ts';
import type { OwnerShares } from './owner-shares.ts';
/** One sign-in attempt's state, or nothing while no attempt was started. */
export interface OwnerLoginAttempt {
    readonly phase: OwnerLoginPhase;
    readonly failure?: OwnerLoginFailure;
}
/** Inputs the share dialog supplies to its owner identity state. */
export interface OwnerIdentityLoginOptions {
    /** Owner share management the dialog addresses. */
    readonly shares: OwnerShares;
    /** Address in force, as the host named it; the identity status is keyed on this address. */
    readonly relay: string | undefined;
    /** Localized copy. */
    readonly t: PropsLocale<typeof NS>['t'];
    /** Clear the states a finished sign-in replaced, before the status is re-read. */
    readonly onVerified: () => void;
}
/** Owner identity and sign-in state the dialog's address row and status line present. */
export interface OwnerIdentityLogin {
    /** The status the host reported for the address in force. */
    readonly status: OwnerIdentityStatus;
    /** The one attempt this dialog started, or nothing while none was started. */
    readonly login: OwnerLoginAttempt | undefined;
    /** Whether a started attempt is still in flight. */
    readonly busy: boolean;
    /** Whether the host reported an identity this page must verify before it may create a share. */
    readonly needsVerification: boolean;
    /** Re-read the identity status for the address in force. */
    readonly refresh: () => void;
    /** Start the one sign-in attempt this dialog runs, at the address in force. */
    readonly begin: () => void;
    /**
     * Name the verified identity row shows: the host's name, or a label derived from the subject.
     * @returns The name, or an empty string when the host reported a subject-less identity.
     */
    readonly name: () => string;
    /** What an identity row that is not verified states instead of a name, and what to do about it. */
    readonly stateLine: () => string;
    /**
     * Credential detail a verified row keeps as a tooltip rather than a stacked line.
     * @returns The expiry line, with the renewal note when the host reported one.
     */
    readonly tooltip: () => string | undefined;
    /**
     * Copy for the phase of the attempt in flight, and the reason a failed one states.
     * @param phase - Phase the attempt reported.
     * @param failure - Reason it ended without a verified identity, when it stated one.
     * @returns The line the status area states for that attempt.
     */
    readonly line: (phase: OwnerLoginPhase, failure: OwnerLoginFailure | undefined) => string;
}
/**
 * Read the owner identity the host reports and drive its one Relay sign-in attempt.
 * @param options - Owner share management, the address in force, localized copy, and the verified hook.
 * @returns The identity status, the attempt, and the copy its row states.
 */
export declare function useOwnerIdentityLogin(options: OwnerIdentityLoginOptions): OwnerIdentityLogin;
//# sourceMappingURL=owner-identity-login.d.ts.map