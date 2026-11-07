//#region lib/types/host/relay-url.js
/**
* Normalize a public Relay base URL by removing query, hash, and trailing slashes.
* @param value Public Relay URL or base URL.
* @returns Canonical base URL without credentials, query, hash, or trailing slash.
*/
function normalizeRelayUrl(value) {
	if (value === "") throw new Error("liveshare relayUrl must not be empty");
	let url;
	try {
		url = new URL(value);
	} catch {
		throw new Error("liveshare relayUrl must be an absolute URL");
	}
	if (url.protocol !== "http:" && url.protocol !== "https:") throw new Error("liveshare relayUrl must use http or https");
	if (url.username !== "" || url.password !== "") throw new Error("liveshare relayUrl must not include credentials");
	url.search = "";
	url.hash = "";
	url.pathname = url.pathname.replace(/\/+$/u, "");
	return url.href.replace(/\/$/u, "");
}
/**
* Split one Relay base URL into its origin and base path.
* @param relay Public Relay URL or base URL.
* @returns Validated origin and trailing-slash-free base path.
*/
function relayEndpoint(relay) {
	const url = new URL(normalizeRelayUrl(relay));
	return {
		origin: url.origin,
		basePath: url.pathname.replace(/\/+$/u, "")
	};
}
//#endregion
//#region lib/types/owner-gateway/relay-refusal.js
/**
* When a Relay handshake response ends one registration for good.
*
* The Relay answers a gateway handshake it will not establish with an HTTP status instead of
* upgrading. A credential the Relay does not accept (401), one it forbids for this share (403), or a
* share it no longer serves (410), or an unsupported protocol (426) ends this registration.
* Reconnecting with the same token and protocol reaches the same answer, so the share stays offline and the status becomes its diagnostic
* instead of a silent retry. Every other failure — a Relay that is down (5xx), a rate limit, a
* timeout, a refused socket — is transient and keeps the ordinary reconnect schedule.
*/
/** The Relay handshake statuses that end a registration for good. */
const RELAY_REFUSAL_STATUSES = [
	401,
	403,
	410,
	426
];
/**
* Read one handshake status as a decision about the registration it carried.
* @param status HTTP status the Relay answered the gateway handshake with, if it answered.
* @returns The refusal to report, or undefined when the failure is worth retrying.
*/
function relayRefusal(status) {
	if (status !== void 0 && RELAY_REFUSAL_STATUSES.includes(status)) return `relay refused registration status=${String(status)}`;
}
/** The one form every permanent refusal reaches a Host as; nothing else composes it. */
const RELAY_REFUSAL_MESSAGE = /^relay refused registration status=(\d+)$/u;
/**
* Read one reported registration failure as the Relay status that decided it.
*
* The gateway reports a permanent refusal exactly as `relayRefusal` composes it, and every other
* failure it reports — a status it did not refuse with, a dropped socket, a timeout — is worth
* another attempt. A Host reading the message can therefore tell the owner to act on this address
* instead of promising a retry that reaches the same answer, and can say which status decided it.
* @param message - Failure the gateway reported for one registration.
* @returns The refusing status, or undefined when the failure is worth retrying.
*/
function relayRefusalStatus(message) {
	const match = RELAY_REFUSAL_MESSAGE.exec(message);
	if (match === null) return void 0;
	const status = Number(match[1]);
	return RELAY_REFUSAL_STATUSES.includes(status) ? status : void 0;
}
//#endregion
//#region lib/types/owner-gateway/retry-backoff.js
/**
* Delay schedule for one connection that keeps failing.
*
* The owner host's worker supervisor and the gateway's per-share Relay socket both reconnect rather
* than give up, and both treat a connection that stayed up long enough as healthy, so the failure
* after one starts the delays over instead of continuing the previous run's growth. Extracted from
* the socket lifecycle so the schedule is deterministic and testable apart from spawning processes.
*/
/** Schedule of a connection that has not (yet) proven itself stable. */
const INITIAL_RETRY_BACKOFF = {
	delayMs: 500,
	attempt: 0
};
/** Ceiling for the doubling delay: a failing peer is retried, never hammered. */
const RETRY_BACKOFF_MAX_MS = 1e4;
/**
* A connection that stays up this long is healthy, so the next failure starts the delays over.
* A peer that replaces the connection right after the handshake keeps the previous backoff instead
* of retrying every 500 ms.
*/
const RETRY_STABLE_MS = 3e4;
/**
* Advance the schedule after one failed attempt.
* @param state Backoff of the attempt that just failed.
* @returns Schedule the retry starts from: the delay to wait and the state after that attempt.
*/
function retryBackoffNext(state) {
	return {
		delayMs: Math.min(state.delayMs * 2, RETRY_BACKOFF_MAX_MS),
		attempt: state.attempt + 1
	};
}
/**
* Schedule for a connection that has proven itself stable.
* @returns The schedule's first entry, so the next failure is retried promptly.
*/
function retryBackoffSettled() {
	return INITIAL_RETRY_BACKOFF;
}
//#endregion
export { relayRefusal as a, relayEndpoint as c, retryBackoffSettled as i, RETRY_STABLE_MS as n, relayRefusalStatus as o, retryBackoffNext as r, normalizeRelayUrl as s, INITIAL_RETRY_BACKOFF as t };
