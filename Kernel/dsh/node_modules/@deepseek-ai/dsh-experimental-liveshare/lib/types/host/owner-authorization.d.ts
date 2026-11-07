/**
 * The host's half of one owner-authorization: a transaction the browser starts here, a single-use code the
 * Relay issues on its own origin, and the verifier only this host holds.
 *
 * The owner authorizes one DSH instance to be reported by one Relay address. The long-term credential
 * never travels through the browser: the Relay issues a code to the page it served, the page reports that
 * code back to the exact origin this transaction bound, and this host redeems it over TLS with the PKCE
 * verifier that never left the process. Every binding — Relay, instance, origin, state — is checked again
 * at redemption, and the transaction is consumed by exactly one completion.
 */
/** Why one owner-authorization attempt was refused. */
export type OwnerAuthorizationRefusal = 
/** No transaction has this id, or it was already completed. */
'unknown-transaction'
/** The transaction outlived its deadline. */
 | 'expired'
/** The request did not come from the page that began the transaction. */
 | 'origin-mismatch'
/** The address in force is not the one this transaction authorized. */
 | 'relay-changed'
/** This host's instance is not the one the transaction was minted for. */
 | 'instance-changed';
/** One transaction the host began and is waiting to complete. */
export interface OwnerAuthorizationTransaction {
    /**
     * Single-use identifier the Relay echoes back through the browser.
     *
     * It is also the transaction's key, so the browser carries exactly one opaque value: the state the page
     * compares against, the value the Relay binds its code to, and the handle this host completes with.
     */
    readonly state: string;
    /** PKCE verifier, held only by this host; the Relay is given its S256 challenge. */
    readonly verifier: string;
    readonly challenge: string;
    /** Relay address this transaction authorizes, normalized. */
    readonly relay: string;
    /** Host instance the credential will be minted for. */
    readonly instanceKey: string;
    /** Exact page origin allowed to report the code back. */
    readonly origin: string;
    /** Epoch milliseconds after which the transaction is refused. */
    readonly expiresAt: number;
}
/** What one completed transaction hands the redemption step. */
export interface CompletedOwnerAuthorization {
    readonly relay: string;
    readonly instanceKey: string;
    readonly verifier: string;
}
/** Outcome of completing a transaction: the verifier and its bindings, or a stable refusal. */
export type OwnerAuthorizationOutcome = {
    readonly ok: true;
    readonly completed: CompletedOwnerAuthorization;
} | {
    readonly ok: false;
    readonly code: OwnerAuthorizationRefusal;
};
/**
 * The transactions this host has begun and not yet completed.
 *
 * Held in memory and keyed by id: a restart invalidates pending authorizations, which the owner retries
 * from the browser rather than the host resurrecting one it can no longer vouch for.
 */
export declare class OwnerAuthorizations {
    private readonly options;
    private readonly pending;
    /**
     * @param options - Clock and lifetime overrides for tests; production uses the defaults.
     */
    constructor(options?: {
        readonly ttlMs?: number;
        readonly now?: () => number;
    });
    /** The clock this store judges expiry against. */
    private now;
    /**
     * Begin one authorization for a Relay address, a host instance, and the page that asked.
     *
     * The verifier stays here and only its S256 challenge is published, so a code intercepted in transit is
     * useless without this process.
     * @param input - Relay address, host instance, and the exact page origin that began it.
     * @returns The transaction, whose id and state the browser carries and whose challenge the Relay gets.
     */
    begin(input: {
        readonly relay: string;
        readonly instanceKey: string;
        readonly origin: string;
    }): OwnerAuthorizationTransaction;
    /**
     * Complete one authorization, checking every binding the browser could have altered.
     *
     * The transaction is consumed only by a completion that passes: a mismatched state, origin, relay, or
     * instance leaves it pending so the owner can retry the same attempt.
     * @param input - The state and code the page reported, the page's own origin, and the Relay address and
     *   host instance in force right now.
     * @returns The verifier with its bindings, or the stable code naming the refusal.
     */
    complete(input: {
        readonly state: string;
        readonly origin: string | undefined;
        readonly relay: string;
        readonly instanceKey: string;
    }): OwnerAuthorizationOutcome;
    /**
     * Drop one pending authorization the browser abandoned.
     *
     * A cancelled attempt leaves no credential and nothing to redeem: the verifier is gone, so a code
     * minted for it afterwards can no longer be exchanged.
     * @param state - The transaction the page reported as cancelled.
     * @returns True when this host was still waiting for that transaction.
     */
    cancel(state: string): boolean;
    /** Drop expired transactions so a long-lived host keeps only what can still complete. */
    private sweep;
}
/**
 * The URL a browser opens, on the Relay's own origin, to sign in and then confirm this authorization.
 * @param input - Relay address, transaction values, and the page origin the Relay may report back to.
 * @returns The absolute authorization URL.
 */
export declare function ownerAuthorizeUrl(input: {
    readonly relay: string;
    readonly state: string;
    readonly challenge: string;
    readonly instanceKey: string;
    readonly origin: string;
}): string;
//# sourceMappingURL=owner-authorization.d.ts.map