/**
 * The owner's own Relay sign-in, driven from the page that is about to share.
 *
 * The owner host is the authority here as it is for every other identity fact: it mints one
 * short-lived transaction bound to the Relay address, the Relay instance, and this page's origin, and
 * it keeps that transaction's verifier to itself. This page opens the Relay's sign-in in a popup,
 * accepts the one-time code that popup hands back by `postMessage` — only from the origin the host
 * named, only from that popup, and only for the state the host minted — and submits the code with its
 * state to its own host. The host redeems them against the Relay and stores the credential; the page
 * reads the result back the way it reads any other identity answer.
 *
 * The page holds no credential, reads no cookie, and writes nothing to the URL, to storage, or to a
 * log: the code lives from the popup's message to the host's answer and nowhere else.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/owner-login
 */
import { type OwnerIdentityStatus } from './owner-identity.ts';
/** Message type the Relay's sign-in page hands its opener. */
export declare const OWNER_LOGIN_CODE_TYPE = "liveshare-auth-code";
/** Message type this page answers with, so that page closes instead of navigating on. */
export declare const OWNER_LOGIN_ACK_TYPE = "liveshare-auth-ack";
/** What one sign-in attempt is doing, as the dialog states it. */
export type OwnerLoginPhase = 'opening' | 'waiting' | 'verifying' | 'verified' | 'failed';
/** Why one attempt ended without a verified identity, as the host named it. */
export type OwnerLoginFailure = 'blocked' | 'cancelled' | 'expired' | 'mismatch' | 'replayed' | 'unavailable' | 'failed';
/** One host-minted transaction: where to sign in, what it is bound to, and until when. */
export interface OwnerLoginTicket {
    readonly authorizeUrl: string;
    readonly state: string;
    /**
     * Exact origin the Relay's own sign-in page posts from, as the host binds it.
     *
     * The origin the host answered *this page* for is not this: trusting that back would accept a
     * message from whatever page happens to be at this address, and a ticket that names no sender
     * origin leaves the attempt unable to verify any message at all.
     */
    readonly messageOrigin: string;
    /** UTC deadline this transaction stops being redeemable at, when the host stated one. */
    readonly expiresAt?: string;
}
/** One attempt's answer: the value the host named, or the reason it could not. */
export type OwnerLoginResult<T> = {
    readonly ok: true;
    readonly value: T;
} | {
    readonly ok: false;
    readonly reason: OwnerLoginFailure;
};
/** The same-origin host face one attempt submits to. */
export interface OwnerLoginHost {
    start(): Promise<OwnerLoginResult<OwnerLoginTicket>>;
    redeem(code: string, state: string): Promise<OwnerLoginResult<OwnerIdentityStatus>>;
    cancel(state: string): Promise<void>;
}
/**
 * Minimal popup window one sign-in attempt reserves and drives.
 *
 * The flow reads \`closed\`, navigates \`location.href\`, calls \`close\`, and posts the acknowledgement, so
 * a page's \`open\` need not return the whole browser \`Window\`.
 */
export interface OwnerLoginPopup {
    /** Whether the sign-in page closed the window. */
    closed: boolean;
    /** Address the flow navigates to the sign-in URL. */
    location: {
        href: string;
    };
    /** Close the reserved window. */
    close(): void;
    /** Post the acknowledgement the sign-in page waits for. */
    postMessage(message: unknown, targetOrigin: string): void;
}
/**
 * The browser page one sign-in attempt drives: the popup it reserves, the messages it listens for,
 * and the timer it polls with.
 */
export interface OwnerLoginPage {
    /** Open the Relay's sign-in; null when the browser refused a popup for this gesture. */
    open(url: string, name: string, features: string): OwnerLoginPopup | null;
    addEventListener(type: 'message', listener: (event: MessageEvent) => void): void;
    removeEventListener(type: 'message', listener: (event: MessageEvent) => void): void;
    setInterval(handler: () => void, ms: number): number;
    clearInterval(id: number): void;
}
/**
 * Read one host answer's failure code.
 * @param value - Decoded answer body, absent when the answer could not be read.
 * @returns The reason this page states, or undefined when the answer named none it knows.
 */
export declare function readOwnerLoginFailure(value: unknown): OwnerLoginFailure | undefined;
/**
 * Read the one-time code a sign-in popup handed this page, or nothing when the message is not it.
 *
 * An attempt has exactly one legitimate sender, and this is where that is enforced: the origin the
 * host bound the transaction to, the popup the host's ticket opened, the message type this page
 * expects, and the state that transaction minted. A message from another window, another origin,
 * another state, or one that is not even shaped like a code is dropped without an answer and without
 * a trace, so no other page can hand this one a code it would submit.
 * @param event - Message event this page received.
 * @param expected - Origin, popup, and state the host bound this attempt to.
 * @returns The code and state to submit, or undefined when the message is not this attempt's.
 */
export declare function readOwnerLoginCode(event: {
    readonly origin: string;
    readonly source: unknown;
    readonly data: unknown;
}, expected: {
    readonly origin: string;
    readonly source: unknown;
    readonly state: string;
}): {
    readonly code: string;
    readonly state: string;
} | undefined;
/**
 * Build the same-origin host face one attempt submits to.
 *
 * Every call goes to this page's own origin, so no credential, code, or address the host holds
 * crosses to another party from here.
 * @param relay - Relay address in force, so the identity answer is read for that address.
 * @param request - Fetch-like function to post with; defaults to this page's own `fetch`.
 * @returns The host face one attempt uses.
 */
export declare function hostOwnerLogin(relay: string, request?: (input: string, init?: RequestInit) => Promise<Response>): OwnerLoginHost;
/**
 * Start one sign-in attempt: reserve its window, ask the host for a transaction, and take the
 * one-time code the sign-in page hands back.
 *
 * The window is reserved inside the gesture that asked for it, because a window opened after an
 * await is refused by many browsers and this flow has no safe whole-page completion to fall back on:
 * a page that navigated away would leave nobody listening for the code. A blocked window is a named
 * failure, and every way an attempt can end — the popup closing, the transaction expiring, the owner
 * cancelling, a ticket that arrives after the attempt was cancelled — drops the transaction on the
 * host instead of leaving a code redeemable behind it.
 * @param options - Host face, page, phase reporter, and the clock the deadline is read with.
 * @returns A handle that stops waiting for this attempt.
 */
export declare function beginOwnerLogin(options: {
    readonly host: OwnerLoginHost;
    readonly page: OwnerLoginPage;
    readonly report: (phase: OwnerLoginPhase, failure?: OwnerLoginFailure) => void;
    readonly now?: () => number;
}): {
    cancel(): void;
};
//# sourceMappingURL=owner-login.d.ts.map