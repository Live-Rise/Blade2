//#region lib/types/viewer-states.js
/**
* The conditions one viewer read can end in, and the answer that carries each.
*
* A page that cannot name why a read failed states one failure for every cause: a link that never
* named a share, a share the owner stopped, an owner whose machine is asleep, and a sign-in that
* expired all reach a reader as the same silence. These are the conditions the Relay may answer a
* viewer with, each paired with the answer that means it and nothing else, so a page, a test, and an
* operator all read the same name for what happened.
*
* @module @deepseek-ai/dsh-experimental-liveshare-protocol
*/
/**
* Whether the page may ask again, or the answer is the peer's verdict, for every condition.
*
* This is the whole vocabulary the protocol layer publishes: HTTP statuses and error codes belong to
* the Relay, and copy keys belong to the client. A page, a test, and an operator read one name per
* condition from here and get the retry decision with it.
*/
const VIEWER_STATE_REACH = {
	"share-not-found": "terminal",
	"share-stopped": "terminal",
	"share-expired": "terminal",
	"owner-offline": "retryable",
	"sign-in-expired": "terminal",
	"access-denied": "terminal",
	"relay-unreachable": "retryable"
};
//#endregion
//#region lib/types/tunnel-protocol.js
/** Relay wire version; Host/worker IPC ships together and is not negotiated by this number. */
const LIVESHARE_PROTOCOL_VERSION = 1;
/**
* Optional behaviors this build offers the Relay, beyond the frames themselves.
*
* A peer that declares none is a build older than this one. Each name states behavior the peer can
* observe on the wire, so a Relay relies only on what it has been told rather than on the release it
* happens to run beside.
*/
const LIVESHARE_CAPABILITIES = [
	"session-mode",
	"fork",
	"acl-subjects",
	"expiry",
	"owner-credential",
	"scoped-rpc-response",
	"unsupported-frame-refusal",
	"share-access-state",
	"actor-credential",
	"direct-connection"
];
/**
* Routing fields an opaque viewer request carries, for logging only.
*
* The Relay and the owner worker both forward the same request body without reading it, and both
* need the same two fields to correlate one read across hops; every other field stays opaque.
* @param body Request body as forwarded.
* @returns The operation name and stream id, or placeholders when the body carries neither.
*/
function viewerRequestFields(body) {
	if (typeof body !== "object" || body === null) return {
		op: "unknown",
		streamId: "-"
	};
	const value = body;
	return {
		op: typeof value.op === "string" ? value.op : "unknown",
		streamId: typeof value.id === "number" ? String(value.id) : "-"
	};
}
/** `WebSocket.OPEN`, restated so the shared protocol package carries no socket dependency. */
const SOCKET_OPEN = 1;
/**
* Send one frame when the socket is open.
* @param socket Owner gateway or viewer socket.
* @param frame Wire frame.
*/
function sendFrame(socket, frame) {
	if (socket.readyState === SOCKET_OPEN) socket.send(JSON.stringify(frame));
}
/**
* Decode one WebSocket data frame without relying on its representation.
* @param data Message bytes supplied by ws.
* @returns UTF-8 text for one complete message.
*/
function rawText(data) {
	if (Array.isArray(data)) return Buffer.concat(data).toString("utf8");
	if (data instanceof ArrayBuffer) return Buffer.from(data).toString("utf8");
	return Buffer.from(data).toString("utf8");
}
/**
* Read the envelope every accepted frame carries from an already-decoded value.
*
* Both directions state the same envelope, so one decoder owns it — a frame's id is what correlates an
* answer with its request, and a second copy of these rules is a second place for that to drift.
* @param value - Decoded JSON value.
* @returns The value with its id, or undefined when the value is not an accepted envelope.
*/
function frameEnvelope(value) {
	if (!isRecord(value) || !Number.isSafeInteger(value.id) || Number(value.id) <= 0) return void 0;
	return {
		value,
		id: Number(value.id)
	};
}
/**
* Decode one raw frame's envelope: the JSON value and the request id every accepted frame carries.
* @param text - Raw socket text.
* @returns The decoded value with its id, or undefined when the text is not an accepted envelope.
*/
function decodeFrameEnvelope(text) {
	let value;
	try {
		value = JSON.parse(text);
	} catch {
		return;
	}
	return frameEnvelope(value);
}
/**
* Parse one gateway-to-Relay frame; unknown or malformed frames are dropped.
* @param text Raw socket text.
* @returns Parsed frame, or undefined when the frame is not accepted.
*/
function parseGatewayFrame(text) {
	const envelope = decodeFrameEnvelope(text);
	if (envelope === void 0) return void 0;
	const { value, id } = envelope;
	if (value.type === "share-rpc-res") {
		if (!Number.isSafeInteger(value.status) || Number(value.status) < 100 || Number(value.status) > 599) return;
		return {
			type: "share-rpc-res",
			id,
			status: Number(value.status),
			...value.body === void 0 ? {} : { body: value.body }
		};
	}
}
/**
* Parse one already-decoded Relay-to-gateway value; unknown or malformed frames are dropped.
* @param value Decoded JSON value.
* @returns Parsed frame, or undefined when the frame is not accepted.
*/
function parseRelayValue(value) {
	const state = parseShareAccessState(value);
	if (state !== void 0) return state;
	const envelope = frameEnvelope(value);
	if (envelope === void 0) return void 0;
	const { value: frame, id } = envelope;
	if (frame.type === "share-rpc") {
		if (frame.body === void 0) return void 0;
		if (!isShareRpcActor(frame.actor)) return void 0;
		return {
			type: "share-rpc",
			id,
			body: frame.body,
			actor: frame.actor
		};
	}
	if (frame.type === "share-rpc-cancel") return {
		type: "share-rpc-cancel",
		id
	};
}
/**
* Parse one Relay-to-gateway frame; unknown or malformed frames are dropped.
* @param text Raw socket text.
* @returns Parsed frame, or undefined when the frame is not accepted.
*/
function parseRelayFrame(text) {
	let value;
	try {
		value = JSON.parse(text);
	} catch (_error) {
		return;
	}
	return parseRelayValue(value);
}
/**
* The refusal this build answers with when it reads a frame it cannot serve.
*
* A frame whose type this build does not know, or a known request whose fields it refuses, otherwise
* leaves the peer waiting for an answer that will never come: the read holds until the peer's own
* budget expires, and a version mismatch reaches the reader as a timeout rather than as a fact.
* Answering states the one thing the peer can act on — this build cannot serve that frame — while a
* frame with no usable envelope states no request id to correlate an answer with, and a frame that
* answers or cancels something is not a request to answer at all.
* @param text Raw socket text the Relay sent.
* @returns The refusal to send, or undefined when this frame is not one to answer.
*/
function unsupportedFrameRefusal(text) {
	let value;
	try {
		value = JSON.parse(text);
	} catch (_error) {
		return;
	}
	if (parseRelayValue(value) !== void 0) return void 0;
	const envelope = frameEnvelope(value);
	if (envelope === void 0) return void 0;
	const type = envelope.value.type;
	if (type === "share-rpc") return {
		type: "share-rpc-res",
		id: envelope.id,
		status: 400,
		body: { error: "invalid-request" }
	};
	if (type === "share-rpc-res" || type === "share-rpc-cancel") return void 0;
	if (typeof type !== "string" || type === "") return void 0;
	return {
		type: "share-rpc-res",
		id: envelope.id,
		status: 501,
		body: { error: "unsupported-frame" }
	};
}
/**
* The share one rejected IPC command names, so its host learns the command was refused.
*
* A command this build does not accept is otherwise dropped in silence: an interactive share then
* fails after its own wait, and a share restored at startup stays offline with nothing said.
* @param value Raw IPC message payload the worker refused.
* @returns The share the command named, or undefined when the payload names none.
*/
function rejectedCommandShareId(value) {
	if (!isRecord(value)) return void 0;
	if (typeof value.shareId === "string" && value.shareId !== "") return value.shareId;
	const share = value.share;
	if (isRecord(share) && typeof share.shareId === "string" && share.shareId !== "") return share.shareId;
}
/**
* Parse one host-to-worker IPC command; unknown or malformed commands are dropped.
*
* The worker and its host run in separate processes, so the message crosses a process boundary and
* every field the worker reads is checked here rather than asserted.
* @param value Raw IPC message payload.
* @returns Parsed command, or undefined when the payload is not an accepted command.
*/
function parseTunnelWorkerCommand(value) {
	if (!isRecord(value)) return void 0;
	switch (value.type) {
		case "shutdown": return { type: "shutdown" };
		case "remove-share": return typeof value.shareId === "string" && value.shareId !== "" ? {
			type: "remove-share",
			shareId: value.shareId
		} : void 0;
		case "add-share": return isRegistrationToken(value.registrationToken) && isTunnelShareSpec(value.share) ? {
			type: "add-share",
			registrationToken: value.registrationToken,
			share: {
				...value.share,
				connection: value.share.connection ?? { mode: "relay" }
			}
		} : void 0;
		case "share-rpc-response":
			if (!isTunnelRequestIdentity(value) || !Number.isSafeInteger(value.status) || Number(value.status) < 100 || Number(value.status) > 599) return void 0;
			return {
				type: "share-rpc-response",
				registrationToken: value.registrationToken,
				token: value.token,
				id: value.id,
				shareId: value.shareId,
				status: Number(value.status),
				...value.body === void 0 ? {} : { body: value.body }
			};
		default: return;
	}
}
/**
* Parse one worker-to-host IPC event; unknown or malformed events are dropped.
* @param value Raw IPC message payload.
* @returns Parsed event, or undefined when the payload is not an accepted event.
*/
function parseTunnelWorkerEvent(value) {
	if (!isRecord(value)) return void 0;
	switch (value.type) {
		case "ready": return { type: "ready" };
		case "share-connected":
		case "share-closed": {
			const type = value.type;
			return isTunnelRegistrationIdentity(value) ? {
				type,
				registrationToken: value.registrationToken,
				shareId: value.shareId
			} : void 0;
		}
		case "share-access-state": {
			const state = parseShareAccessState(value);
			return state !== void 0 && isTunnelRegistrationIdentity(value) ? {
				...state,
				registrationToken: value.registrationToken,
				shareId: value.shareId
			} : void 0;
		}
		case "share-error": return isTunnelRegistrationIdentity(value) && typeof value.message === "string" ? {
			type: "share-error",
			registrationToken: value.registrationToken,
			shareId: value.shareId,
			message: value.message
		} : void 0;
		case "share-rpc-cancel": return isTunnelRequestIdentity(value) ? {
			type: "share-rpc-cancel",
			registrationToken: value.registrationToken,
			token: value.token,
			id: value.id,
			shareId: value.shareId
		} : void 0;
		case "share-rpc-request": return isTunnelRequestIdentity(value) && value.body !== void 0 && isShareRpcActor(value.actor) ? {
			type: "share-rpc-request",
			registrationToken: value.registrationToken,
			token: value.token,
			id: value.id,
			shareId: value.shareId,
			body: value.body,
			actor: value.actor
		} : void 0;
		default: return;
	}
}
/**
* Validate one share spec the owner host asked its worker to carry.
*
* Optional fields are checked against the same rules the frame parsers apply; an expiry must be a
* parseable ISO time, so a registration cannot carry a deadline no reader can enforce.
* @param value Candidate spec.
* @returns True when every required field is present with the declared type.
*/
function isTunnelShareSpec(value) {
	if (!isRecord(value)) return false;
	const mode = value.sessionMode;
	return typeof value.shareId === "string" && value.shareId !== "" && typeof value.sessionId === "string" && typeof value.sessionTitle === "string" && (value.accessMode === "open" || value.accessMode === "selected") && typeof value.relay === "string" && typeof value.gatewayToken === "string" && typeof value.ownerDisplayName === "string" && (value.ownerProviderId === void 0 || typeof value.ownerProviderId === "string") && (value.ownerTenantId === void 0 || typeof value.ownerTenantId === "string") && (value.dshVersion === void 0 || typeof value.dshVersion === "string") && (value.ownerInstanceId === void 0 || typeof value.ownerInstanceId === "string") && (value.allowsFork === void 0 || typeof value.allowsFork === "boolean") && (mode === void 0 || mode === "view" || mode === "write") && (value.connection === void 0 || isRecord(value.connection) && (value.connection.mode === "relay" || value.connection.mode === "direct")) && (value.ownerCredential === void 0 || typeof value.ownerCredential === "string") && Array.isArray(value.allowedSubjectIds) && value.allowedSubjectIds.every((subject) => typeof subject === "string" && subject !== "") && (value.expiresAt === void 0 || typeof value.expiresAt === "string" && Number.isFinite(Date.parse(value.expiresAt)));
}
/**
* Validate the actor the Relay asserts on one forwarded request.
* @param value Candidate actor.
* @returns True when it carries a non-empty provider, tenant, and subject.
*/
function isShareRpcActor(value) {
	if (typeof value !== "object" || value === null) return false;
	const actor = value;
	return typeof actor.providerId === "string" && actor.providerId !== "" && typeof actor.tenantId === "string" && actor.tenantId !== "" && typeof actor.subjectId === "string" && actor.subjectId !== "" && (actor.displayName === void 0 || typeof actor.displayName === "string" && actor.displayName !== "") && (actor.credential === void 0 || typeof actor.credential === "string");
}
function isRegistrationToken(value) {
	return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}
function isTunnelRegistrationIdentity(value) {
	return isRegistrationToken(value.registrationToken) && typeof value.shareId === "string" && value.shareId !== "";
}
/** Validate request correlation fields at the IPC parser. */
function isTunnelRequestIdentity(value) {
	return isTunnelRegistrationIdentity(value) && typeof value.token === "string" && value.token !== "" && Number.isSafeInteger(value.id) && Number(value.id) > 0;
}
/**
* Validate a committed Relay authority snapshot at wire, IPC, or authenticated HTTP ingress.
* @param value - Untrusted snapshot fields.
* @returns Owned snapshot fields, or undefined when any authority field is invalid.
*/
function parseShareAccessState(value) {
	if (!isRecord(value) || value.type !== "share-access-state" || !Number.isSafeInteger(value.version) || Number(value.version) < 0 || value.accessMode !== "open" && value.accessMode !== "selected" || !isSubjectIds(value.allowedSubjectIds) || value.sessionMode !== "view" && value.sessionMode !== "write" || value.allowsFork !== void 0 && typeof value.allowsFork !== "boolean" || value.expiresAt !== void 0 && (typeof value.expiresAt !== "string" || !Number.isFinite(Date.parse(value.expiresAt)))) return;
	return {
		type: "share-access-state",
		version: Number(value.version),
		accessMode: value.accessMode,
		allowedSubjectIds: [...value.allowedSubjectIds],
		sessionMode: value.sessionMode,
		...value.allowsFork === void 0 ? {} : { allowsFork: value.allowsFork },
		...value.expiresAt === void 0 ? {} : { expiresAt: value.expiresAt }
	};
}
function isSubjectIds(value) {
	return Array.isArray(value) && value.every((id) => typeof id === "string" && id !== "");
}
function isRecord(value) {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
//#endregion
//#region lib/types/identity/keys.js
/** Canonical ACL key for one identity triple, and the normalization every adapter applies first. */
/**
* Build the canonical identity key stored in share ACLs.
*
* The caller normalizes the identity first: {@link normalizeIdentity} fills the documented tenant
* placeholder, while an unnormalized empty tenant renders an empty segment that
* {@link identityFromKey} cannot parse.
* @param identity Provider, tenant, and subject triple.
* @returns Percent-encoded stable key with no user-controlled formatting.
*/
function identityKeyOf(identity) {
	return [
		encodeURIComponent(identity.providerId),
		encodeURIComponent(identity.tenantId),
		encodeURIComponent(identity.subjectId)
	].join(":");
}
/**
* Parse a canonical identity key created by {@link identityKeyOf}.
* @param key Percent-encoded provider, tenant, and subject key.
* @returns Identity triple, or undefined when the key is not canonical.
*/
function identityFromKey(key) {
	const parts = key.split(":");
	if (parts.length !== 3) return void 0;
	try {
		const [providerId = "", tenantId = "", subjectId = ""] = parts.map((part) => decodeURIComponent(part));
		if (providerId === "" || tenantId === "" || subjectId === "") return void 0;
		return {
			providerId,
			tenantId,
			subjectId
		};
	} catch {
		return;
	}
}
/**
* Tenant id recorded for a provider that issues no tenant of its own.
*
* An account adapter whose provider issues no tenant must reject an empty tenant instead of using
* this placeholder; see {@link requireIdentityTenant}.
*/
const IDENTITY_TENANT_PLACEHOLDER = "default";
/**
* Normalize one adapter-produced identity before Relay state stores it.
* @param identity Provider-produced triple.
* @param tenantFallback Tenant id used when the provider issued none.
* @returns Normalized triple, or undefined when the provider or subject id is empty.
*/
function normalizeIdentity(identity, tenantFallback = IDENTITY_TENANT_PLACEHOLDER) {
	if (identity.providerId === "" || identity.subjectId === "") return void 0;
	if (identity.tenantId !== "") return identity;
	if (tenantFallback === "") return void 0;
	return {
		...identity,
		tenantId: tenantFallback
	};
}
/**
* Validate one adapter-produced identity whose provider must issue a tenant id.
* @param identity Provider-produced triple.
* @returns The same triple, or undefined when the provider, tenant, or subject id is empty.
*/
function requireIdentityTenant(identity) {
	return identity.providerId === "" || identity.tenantId === "" || identity.subjectId === "" ? void 0 : identity;
}
//#endregion
//#region lib/types/viewer-carrier.js
/**
* Viewer page carrier: the script the Relay places at the top of the owner's served index document.
*
* It installs the page's Connection transport over the Relay read channel and names the share the
* page presents. It performs no Liveshare business: grants, page status, and refusal handling belong
* to the Liveshare client plugin, which reads them through ordinary reads. The carrier only converts
* Connection calls into `call` requests, Gateway streams into `stream-open`/`stream-next`/
* `stream-cancel` polls, and marks each failure as a retryable carrier loss or a terminal refusal.
*
* It also forwards the page-origin owner routes the Relay cannot answer, so app code that calls `fetch`
* directly reaches the owner instead of the Relay's tail. That set is explicit and read-only; every
* other request keeps the page's own `fetch`.
*
* {@link installViewerCarrier} runs in the browser from its own source text, so it must reference no
* module binding: every constant and helper it needs is declared inside its body.
*
* @module @deepseek-ai/dsh-experimental-liveshare-protocol
*/
/** Page global naming the share one viewer page presents; the Liveshare client plugin reads it. */
const LIVESHARE_VIEWER_GLOBAL = "__DSH_LIVESHARE_VIEWER__";
/**
* Install the viewer globals in the current page. Runs before the shell entry so the Connection
* plugin finds `__DSH_TRANSPORT__` when it installs.
* @param options - Page target and read budget.
*/
function installViewerCarrier(options) {
	const carrierStatus = new Set([
		408,
		429,
		502,
		503,
		504
	]);
	const carrierCodes = new Set(["stream-not-found", "gateway/cancelled"]);
	const codeShaped = /^[a-z][a-z0-9-]*(?:\/[a-z0-9-]+)*$/u;
	const target = options.shareId === "" ? `${options.basePath}/__share/rpc` : `${options.basePath}/__share/rpc?shareId=${encodeURIComponent(options.shareId)}`;
	let nextStreamId = (crypto.getRandomValues(new Uint32Array(1))[0] ?? 0) % 2147483392 + 1;
	const isRecord = (value) => typeof value === "object" && value !== null && !Array.isArray(value);
	const carrier = (message) => {
		const error = new Error(message);
		error.dshRemoteStreamFailure = { kind: "carrier" };
		return error;
	};
	const refusal = (answer) => {
		if (!isRecord(answer.body)) return {};
		const value = answer.body.error;
		const nested = isRecord(value);
		const code = nested ? value.code : value;
		const message = nested ? value.message : answer.body.message;
		return {
			...typeof code === "string" && codeShaped.test(code) ? { code } : {},
			...typeof message === "string" ? { message } : {}
		};
	};
	const remote = (answer, subject) => {
		const { code = "liveshare/read-refused", message = `${subject} refused (HTTP ${String(answer.status)})` } = refusal(answer);
		const error = new Error(message);
		error.name = code;
		error.dshRemoteStreamFailure = {
			kind: "remote",
			code,
			details: { status: answer.status }
		};
		return error;
	};
	const post = async (request, subject, signal) => {
		const timeout = AbortSignal.timeout(options.readTimeoutMs);
		let response;
		try {
			response = await fetch(target, {
				method: "POST",
				credentials: "same-origin",
				headers: { "content-type": "application/json" },
				body: JSON.stringify(request),
				signal: signal === void 0 ? timeout : AbortSignal.any([signal, timeout])
			});
		} catch (error) {
			if (signal?.aborted === true) throw error;
			throw carrier(`${subject}: Relay unreachable`);
		}
		const body = await response.json().catch(() => void 0);
		return {
			status: response.status,
			body
		};
	};
	const readable = (answer, subject) => {
		if (answer.status === 200) {
			if (isRecord(answer.body)) return answer.body;
			throw carrier(`${subject}: Relay answered without a body`);
		}
		const code = refusal(answer).code;
		if (answer.status !== 401 && answer.status !== 403 && (carrierStatus.has(answer.status) || code !== void 0 && carrierCodes.has(code))) throw carrier(`${subject}: ${code ?? `HTTP ${String(answer.status)}`}`);
		throw remote(answer, subject);
	};
	const failure = (frame, subject) => {
		const value = frame.failure;
		if (!isRecord(value)) return void 0;
		const code = typeof value.code === "string" ? value.code : "liveshare/read-refused";
		const message = typeof value.message === "string" ? value.message : subject;
		if (carrierCodes.has(code)) return carrier(message);
		const error = new Error(message);
		error.name = code;
		error.dshRemoteStreamFailure = {
			kind: "remote",
			code,
			details: isRecord(value.details) ? value.details : {}
		};
		return error;
	};
	/**
	* Page-origin owner routes the Relay serves no answer for, forwarded instead of left to the Relay.
	*
	* Each is a read the shell makes for itself and the Relay cannot serve: the owner route exists on the
	* owner's own server, so a direct `fetch` from this page would leave the Relay origin and 404 at its
	* tail. Only read-only routes belong here: a route that acts on the owner's machine stays out.
	*/
	const forwardedRoutes = ["/open-in-app/apps", "/api/present.host"];
	/**
	* Offset at which `route` starts a path segment run, ignoring a Relay base path in front of it.
	* @param pathname - Resolved request pathname.
	* @param route - Owner route to find.
	* @returns The offset, or undefined when the route appears only inside a longer segment.
	*/
	const segmentOffset = (pathname, route) => {
		let from = 0;
		for (;;) {
			const at = pathname.indexOf(route, from);
			if (at === -1) return void 0;
			const next = pathname[at + route.length];
			if (next === void 0 || next === "/") return at;
			from = at + 1;
		}
	};
	/**
	* Owner path one page request forwards, matched at path-segment boundaries because the page URL carries
	* the Relay's own base path in front of it. Prefix-neighbouring paths such as `/api/present.hostile` or
	* `/x/open-in-app/apps-foo` are not this route.
	* @param pathname - Resolved request pathname (query and hash already excluded).
	* @param method - Request method, already upper-cased.
	* @returns The owner path to post, or undefined when this request is not one of the forwarded routes.
	*/
	const forwardedRoute = (pathname, method) => {
		if (method !== "GET") return void 0;
		for (const route of forwardedRoutes) {
			const at = segmentOffset(pathname, route);
			if (at !== void 0) return pathname.slice(at);
		}
	};
	/**
	* Read one forwarded owner route and rebuild its answer as an ordinary HTTP response.
	* @param path - Owner path to post.
	* @param init - The page request's own init, for its signal.
	* @returns The owner's status and body, JSON-encoded.
	*/
	const forwardRoute = async (path, init) => {
		const answer = await post({
			op: "call",
			path,
			body: typeof init?.body === "string" ? JSON.parse(init.body) : {}
		}, path, init?.signal ?? void 0);
		return new Response(JSON.stringify(answer.body ?? null), {
			status: answer.status,
			headers: { "content-type": "application/json" }
		});
	};
	const pageFetch = globalThis.fetch;
	const carrierFetch = async (input, init) => {
		let pathname;
		let method = "GET";
		try {
			const page = globalThis;
			const pageOrigin = page.location?.origin;
			const target = new URL(input instanceof Request ? input.url : String(input), page.document?.baseURI);
			pathname = pageOrigin !== void 0 && target.origin === pageOrigin ? target.pathname : void 0;
			method = (init?.method ?? (input instanceof Request ? input.method : "GET")).toUpperCase();
		} catch {
			pathname = void 0;
		}
		const path = pathname === void 0 ? void 0 : forwardedRoute(pathname, method);
		return path === void 0 ? await pageFetch.call(globalThis, input, init) : await forwardRoute(path, init);
	};
	globalThis.fetch = carrierFetch;
	const fetchCall = async (input, init) => {
		const pageBase = globalThis.document?.baseURI;
		const pathname = new URL(input instanceof Request ? input.url : input, pageBase).pathname;
		const marker = pathname.lastIndexOf("/api/");
		const path = marker === -1 ? pathname : pathname.slice(marker);
		const body = typeof init.body === "string" ? JSON.parse(init.body) : void 0;
		const answer = await post({
			op: "call",
			path,
			body
		}, path, init.signal ?? void 0);
		if (answer.status === 200 && isRecord(answer.body)) return Response.json(answer.body);
		const rpcId = isRecord(body) && typeof body.rpcId === "string" ? body.rpcId : "";
		const { code = "liveshare/relay-error", message = `Relay answered HTTP ${String(answer.status)}` } = refusal(answer);
		return Response.json({
			type: "server-response",
			rpcId,
			result: {
				ok: false,
				error: {
					code,
					message,
					details: { status: answer.status }
				}
			}
		});
	};
	const openStream = (endpoint, payload, signal) => ({ async *[Symbol.asyncIterator]() {
		const id = nextStreamId++;
		signal.throwIfAborted();
		const cancel = () => {
			post({
				op: "stream-cancel",
				id
			}, endpoint).catch(() => void 0);
		};
		let opened;
		try {
			opened = await post({
				op: "stream-open",
				id,
				endpoint,
				payload
			}, endpoint, signal);
		} catch (error) {
			if (signal.aborted) cancel();
			throw error;
		}
		if (signal.aborted) {
			cancel();
			signal.throwIfAborted();
		}
		let openedBody;
		try {
			openedBody = readable(opened, endpoint);
		} catch (error) {
			if (opened.status === 200) cancel();
			throw error;
		}
		const refusedOpen = failure(openedBody, endpoint);
		if (refusedOpen !== void 0) throw refusedOpen;
		signal.addEventListener("abort", cancel, { once: true });
		try {
			while (!signal.aborted) {
				const frame = readable(await post({
					op: "stream-next",
					id
				}, endpoint, signal), endpoint);
				signal.throwIfAborted();
				const failed = failure(frame, endpoint);
				if (failed !== void 0) throw failed;
				if (frame.end === true) return;
				if (frame.pending === true) continue;
				if (Array.isArray(frame.values)) {
					for (const value of frame.values) {
						signal.throwIfAborted();
						yield value;
					}
					continue;
				}
				yield frame.value;
			}
		} finally {
			signal.removeEventListener("abort", cancel);
			if (!signal.aborted) cancel();
		}
	} });
	const page = globalThis;
	page.__DSH_LIVESHARE_VIEWER__ = {
		basePath: options.basePath,
		shareId: options.shareId,
		sessionId: options.sessionId
	};
	page.__DSH_TRANSPORT__ = {
		fetch: fetchCall,
		openStream
	};
}
/**
* Build the classic inline script the Relay places first in the served viewer document.
* @param options - Page target and read budget.
* @returns Script text: {@link installViewerCarrier}'s own source applied to `options`.
*/
function viewerCarrierScript(options) {
	const argument = JSON.stringify(options).replace(/</g, "\\u003c");
	return `(${installViewerCarrier.toString()})(${argument});`;
}
//#endregion
export { IDENTITY_TENANT_PLACEHOLDER, LIVESHARE_CAPABILITIES, LIVESHARE_PROTOCOL_VERSION, LIVESHARE_VIEWER_GLOBAL, VIEWER_STATE_REACH, identityFromKey, identityKeyOf, installViewerCarrier, normalizeIdentity, parseGatewayFrame, parseRelayFrame, parseShareAccessState, parseTunnelWorkerCommand, parseTunnelWorkerEvent, rawText, rejectedCommandShareId, requireIdentityTenant, sendFrame, unsupportedFrameRefusal, viewerCarrierScript, viewerRequestFields };
