/** Owner credential kept beside the owner state file, bound to one Relay address. */
import type { OwnerIdentityState } from '../owner-identity.ts';
/** One stored credential and the Relay address it was issued for. */
export interface StoredOwnerCredential {
    /** Relay base URL, normalized, the credential is bound to. */
    readonly relay: string;
    readonly credential: string;
    /** ISO expiry decoded from the credential payload. */
    readonly expiresAt?: string;
    readonly displayName?: string;
    readonly subjectId?: string;
    /** Provider-issued avatar URL for display; absent when the credential carries none. */
    readonly avatarUrl?: string;
}
/**
 * Read the stored credential.
 * @param file Credential file path.
 * @returns Stored credential, or undefined when the file is absent or unreadable.
 */
export declare function loadOwnerCredential(file: string): StoredOwnerCredential | undefined;
/**
 * Persist one credential with owner-only permissions.
 * @param file Credential file path.
 * @param value Credential and the Relay address it belongs to.
 */
export declare function saveOwnerCredential(file: string, value: StoredOwnerCredential): void;
/**
 * Read the identity facts out of a credential without verifying its signature.
 *
 * The Relay is the only party that can verify a credential, so these facts exist to report status
 * and expiry to the local browser; they never authorize anything by themselves.
 * @param credential Credential token exactly as stored.
 * @returns Decoded payload fields, or undefined when the token is not a signed payload.
 */
export declare function decodeOwnerCredential(credential: string): {
    readonly expiresAt?: string;
    readonly displayName?: string;
    readonly subjectId?: string;
    readonly avatarUrl?: string;
} | undefined;
/**
 * Read the horizon one Relay reported for a credential it accepted.
 *
 * The signed expiry inside a credential is the horizon the Relay wrote when it minted the token, and
 * the login behind it can be renewed past that instant. A response carrying the horizon the Relay
 * currently grants is therefore the authority, and the signed value is only its floor.
 * @param reported Value of the Relay's `ownerCredentialExpiresAt` response field.
 * @returns ISO time the Relay reported, or undefined when it reported none this host can read.
 */
export declare function reportedOwnerCredentialExpiry(reported: unknown): string | undefined;
/**
 * Report the credential state for one Relay address.
 * @param stored Credential currently on disk, if any.
 * @param relay Relay base URL this host is about to use.
 * @param now Current epoch milliseconds.
 * @returns Status of the credential for that address.
 */
export declare function ownerCredentialState(stored: StoredOwnerCredential | undefined, relay: string, now: number): OwnerIdentityState;
/**
 * Delete the stored credential.
 * @param file Credential file path.
 */
export declare function clearOwnerCredential(file: string): void;
//# sourceMappingURL=owner-credential-file.d.ts.map