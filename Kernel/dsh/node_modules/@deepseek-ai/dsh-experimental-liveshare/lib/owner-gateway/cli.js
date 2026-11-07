import { a as relayRefusal, c as relayEndpoint, i as retryBackoffSettled, r as retryBackoffNext, t as INITIAL_RETRY_BACKOFF } from "../retry-backoff-ccJYtzOj.js";
import { randomUUID } from "node:crypto";
import { LIVESHARE_CAPABILITIES, LIVESHARE_PROTOCOL_VERSION, parseRelayFrame, parseTunnelWorkerCommand, rawText, sendFrame, unsupportedFrameRefusal, viewerRequestFields } from "@deepseek-ai/dsh-experimental-liveshare-protocol";
import { WebSocket } from "ws";
//#region lib/types/owner-gateway/registration-url.js
/**
* The registration handshake one share is published with.
*
* This URL is the one surface an older owner meets a newer Relay on, so what it states about the
* owner — the protocol version it reads frames with, and the capabilities it offers beyond them — is
* the only thing a Relay can compare instead of assuming. Keeping the whole handshake in one function
* is what lets a check pin it: every parameter here is read by a peer this repository does not build.
*/
/**
* Build the WebSocket URL one share registers with.
*
* Absent optionals stay absent rather than being sent empty: the Relay reads a missing `sessionMode`
* as view-only and a missing `allowsFork` as no forking, so an older owner publishes the narrower
* share instead of a share whose fields a peer must interpret.
* @param spec - Share the owner host asked this worker to carry.
* @returns The registration URL, with this build's protocol version and capabilities stated.
*/
function registrationUrl(spec) {
	const relay = relayEndpoint(spec.relay);
	const url = new URL(`${relay.basePath}/__tunnel_gateway__`, relay.origin);
	url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
	url.searchParams.set("shareId", spec.shareId);
	url.searchParams.set("sessionId", spec.sessionId);
	url.searchParams.set("sessionTitle", spec.sessionTitle);
	url.searchParams.set("accessMode", spec.accessMode);
	url.searchParams.set("ownerDisplayName", spec.ownerDisplayName);
	if (spec.dshVersion !== void 0 && spec.dshVersion !== "") url.searchParams.set("dshVersion", spec.dshVersion);
	if (spec.ownerInstanceId !== void 0 && spec.ownerInstanceId !== "") url.searchParams.set("ownerInstanceId", spec.ownerInstanceId);
	url.searchParams.set("protocolVersion", String(LIVESHARE_PROTOCOL_VERSION));
	url.searchParams.set("capabilities", LIVESHARE_CAPABILITIES.join(","));
	if (spec.allowsFork === true) url.searchParams.set("allowsFork", "1");
	if (spec.sessionMode !== void 0) url.searchParams.set("sessionMode", spec.sessionMode);
	if (spec.ownerProviderId !== void 0 && spec.ownerProviderId !== "") url.searchParams.set("ownerProviderId", spec.ownerProviderId);
	if (spec.ownerTenantId !== void 0 && spec.ownerTenantId !== "") url.searchParams.set("ownerTenantId", spec.ownerTenantId);
	for (const subject of spec.allowedSubjectIds) url.searchParams.append("allowedSubjectId", subject);
	if (spec.expiresAt !== void 0 && spec.expiresAt !== "") url.searchParams.set("expiresAt", spec.expiresAt);
	return url;
}
//#endregion
//#region lib/types/owner-gateway/pending-rpc.js
/** Correlate IPC requests by worker-issued tokens and Relay ids by their exact socket. */
/** Request identities remain distinct when a Relay reconnects and reuses its numeric ids. */
var PendingRpc = class {
	tokens = /* @__PURE__ */ new Map();
	owners = /* @__PURE__ */ new Map();
	/**
	* Register a request with a new IPC token.
	* @param shareId Share carrying the request.
	* @param id Relay id on this socket.
	* @param owner Exact socket that received the request.
	* @returns Registered request; an existing id must be cancelled before reuse on the same socket.
	*/
	add(shareId, id, owner) {
		const request = {
			token: randomUUID(),
			shareId,
			id,
			owner,
			since: Date.now()
		};
		let requests = this.owners.get(owner);
		if (requests === void 0) {
			requests = /* @__PURE__ */ new Map();
			this.owners.set(owner, requests);
		}
		if (requests.has(id)) throw new Error("Relay request id is already pending on this socket");
		requests.set(id, request);
		this.tokens.set(request.token, request);
		return request;
	}
	/**
	* Claim an answer only when all IPC correlation fields match.
	* @param answer Token, share and Relay id echoed by the Host.
	* @returns The request removed from pending state, or undefined for a stale or mismatched answer.
	*/
	take(answer) {
		const request = this.tokens.get(answer.token);
		if (request === void 0 || request.shareId !== answer.shareId || request.id !== answer.id) return void 0;
		return this.cancel(request.owner, request.id);
	}
	/**
	* Remove one socket-local request.
	* @param owner Socket receiving the cancellation.
	* @param id Relay id on that socket.
	* @returns Removed request for cancellation notification, or undefined if it already settled.
	*/
	cancel(owner, id) {
		const requests = this.owners.get(owner);
		const request = requests?.get(id);
		if (request === void 0) return void 0;
		this.tokens.delete(request.token);
		requests?.delete(id);
		if (requests?.size === 0) this.owners.delete(owner);
		return request;
	}
	/**
	* Remove all requests on a closed socket.
	* @param owner Socket being disposed.
	* @returns Removed requests for Host cancellation notifications.
	*/
	clear(owner) {
		const requests = [...this.owners.get(owner)?.values() ?? []];
		for (const request of requests) this.tokens.delete(request.token);
		this.owners.delete(owner);
		return requests;
	}
};
//#endregion
//#region lib/types/owner-gateway/worker.js
/** Owner-side gateway worker: one process multiplexes every share over IPC to the owner host. */
const HEARTBEAT_INTERVAL_MS = 25e3;
/**
* Consecutive unanswered heartbeats tolerated before this worker drops its own Relay socket.
*
* One miss means a busy event loop, not a dead connection: terminating there aborts every in-flight
* viewer stream on the share and forces a re-registration, which is the periodic reconnect an owner
* would otherwise never see the cause of.
*/
const GATEWAY_MAX_MISSED_PONGS = 2;
/** IPC tokens identify requests; socket-local Relay ids never select a Host response. */
const pendingRpc = new PendingRpc();
/**
* Run one owner worker that multiplexes every share over IPC. The worker owns
* one outbound Relay socket per share, but one process and one owner DSH.
*/
function runGatewayWorker() {
	const shares = /* @__PURE__ */ new Map();
	notifyWorker({ type: "ready" });
	process.on("message", (value) => {
		const command = parseTunnelWorkerCommand(value);
		if (command === void 0) {
			console.warn("[liveshare] worker refused one owner command");
			return;
		}
		if (command.type === "share-rpc-response") {
			if ((shares.get(command.shareId)?.gateway)?.registrationToken !== command.registrationToken) return;
			const request = pendingRpc.take(command);
			if (request === void 0) {
				console.warn(`[liveshare] worker dropped late read response relayId=${String(command.id)}`);
				return;
			}
			const gateway = request.owner.socket;
			if (gateway.readyState === WebSocket.OPEN) {
				const observed = viewerRequestFields(command.body);
				console.log(`[liveshare] worker read answered share=${request.shareId} relayId=${String(command.id)} op=${observed.op} streamId=${observed.streamId} status=${String(command.status)} elapsedMs=${String(Date.now() - request.since)}`);
				sendFrame(gateway, {
					type: "share-rpc-res",
					id: command.id,
					status: command.status,
					...command.body === void 0 ? {} : { body: command.body }
				});
			} else console.warn(`[liveshare] worker cannot deliver read response share=${request.shareId} relayId=${String(command.id)} — gateway is not connected`);
			return;
		}
		if (command.type === "add-share") addShare(shares, command.share, command.registrationToken);
		else if (command.type === "remove-share") {
			console.warn(`[liveshare] worker dropping one share by owner command share=${command.shareId}`);
			removeShare(shares, command.shareId);
		} else {
			console.warn(`[liveshare] worker shutting down command=${String(command.type)} shares=${String(shares.size)}`);
			for (const shareId of [...shares.keys()]) removeShare(shares, shareId);
			process.exit(0);
		}
	});
	process.on("disconnect", () => {
		console.warn(`[liveshare] worker lost its host connection shares=${String(shares.size)}`);
		for (const shareId of [...shares.keys()]) removeShare(shares, shareId);
		process.exit(0);
	});
}
function addShare(shares, spec, registrationToken) {
	if (shares.get(spec.shareId)?.registrationToken === registrationToken) return;
	removeShare(shares, spec.shareId);
	const state = {
		spec,
		registrationToken,
		accessVersion: void 0,
		gateway: void 0,
		heartbeat: void 0,
		retryTimer: void 0,
		retry: INITIAL_RETRY_BACKOFF,
		openedAt: void 0,
		stopped: false
	};
	shares.set(spec.shareId, state);
	connectShare(shares, state);
}
function removeShare(shares, shareId) {
	const state = shares.get(shareId);
	if (state === void 0) return;
	state.stopped = true;
	if (state.retryTimer !== void 0) clearTimeout(state.retryTimer);
	clearInterval(state.heartbeat);
	if (state.gateway !== void 0) clearPendingRpc(state.gateway);
	state.gateway?.socket.close(1e3, "share removed");
	shares.delete(shareId);
}
function connectShare(shares, state) {
	const gateway = new WebSocket(registrationUrl(state.spec), { headers: {
		authorization: `Bearer ${state.spec.gatewayToken}`,
		...state.spec.ownerCredential === void 0 || state.spec.ownerCredential === "" ? {} : { "x-liveshare-owner-credential": state.spec.ownerCredential }
	} });
	const connection = {
		socket: gateway,
		registrationToken: state.registrationToken
	};
	state.gateway = connection;
	let awaitingPong = false;
	let missedPongs = 0;
	let closeReason = "unknown";
	let handshakeStatus;
	state.heartbeat = setInterval(() => {
		if (gateway.readyState !== WebSocket.OPEN) return;
		if (awaitingPong) {
			missedPongs += 1;
			if (missedPongs >= GATEWAY_MAX_MISSED_PONGS) {
				closeReason = `heartbeat-timeout missedPongs=${String(missedPongs)}`;
				gateway.terminate();
				return;
			}
			console.warn(`[liveshare] worker heartbeat missed share=${state.spec.shareId} missedPongs=${String(missedPongs)}`);
			return;
		}
		awaitingPong = true;
		gateway.ping();
	}, HEARTBEAT_INTERVAL_MS);
	gateway.on("pong", () => {
		awaitingPong = false;
		missedPongs = 0;
	});
	gateway.on("open", () => {
		if (state.stopped || state.gateway !== connection) return;
		state.openedAt = Date.now();
		notifyWorker({
			type: "share-connected",
			registrationToken: state.registrationToken,
			shareId: state.spec.shareId
		});
	});
	gateway.on("message", (data) => {
		awaitingPong = false;
		missedPongs = 0;
		const text = rawText(data);
		const frame = parseRelayFrame(text);
		if (frame !== void 0) {
			if (!state.stopped && state.gateway === connection) handleGatewayFrame(state, connection, frame);
			return;
		}
		const refusal = unsupportedFrameRefusal(text);
		console.warn(`[liveshare] worker refused one relay frame share=${state.spec.shareId} frame=${refusal === void 0 ? "unanswerable" : `status=${String(refusal.status)} relayId=${String(refusal.id)}`}`);
		if (refusal !== void 0 && !state.stopped && state.gateway === connection) sendFrame(gateway, refusal);
	});
	gateway.on("close", (code, socketReason) => {
		clearPendingRpc(connection);
		if (state.gateway !== connection) return;
		clearInterval(state.heartbeat);
		state.gateway = void 0;
		state.accessVersion = void 0;
		if (state.stopped) return;
		console.warn(`[liveshare] worker gateway closed share=${state.spec.shareId} code=${String(code)} reason=${closeReason} socketReason=${socketReason.toString()} retryMs=${String(state.retry.delayMs)}`);
		const refusal = relayRefusal(handshakeStatus);
		if (refusal !== void 0) {
			state.stopped = true;
			shares.delete(state.spec.shareId);
			console.warn(`[liveshare] worker stopped share=${state.spec.shareId} ${refusal}`);
			return;
		}
		if (state.openedAt !== void 0 && Date.now() - state.openedAt >= 3e4) state.retry = retryBackoffSettled();
		state.openedAt = void 0;
		const delayMs = state.retry.delayMs;
		state.retry = retryBackoffNext(state.retry);
		state.retryTimer = setTimeout(() => {
			connectShare(shares, state);
		}, delayMs);
		notifyWorker({
			type: "share-closed",
			registrationToken: state.registrationToken,
			shareId: state.spec.shareId
		});
	});
	gateway.on("error", (error) => {
		if (state.stopped || state.gateway !== connection) return;
		const detail = (handshakeStatus === void 0 ? void 0 : relayRefusal(handshakeStatus) ?? `relay answered registration status=${String(handshakeStatus)}`) ?? (error instanceof Error ? error.message : String(error));
		closeReason = `gateway-error ${detail}`;
		notifyWorker({
			type: "share-error",
			registrationToken: state.registrationToken,
			shareId: state.spec.shareId,
			message: detail
		});
		gateway.terminate();
	});
	gateway.on("unexpected-response", (_request, response) => {
		handshakeStatus = response.statusCode;
		gateway.terminate();
	});
}
function handleGatewayFrame(state, connection, frame) {
	const shareId = state.spec.shareId;
	switch (frame.type) {
		case "share-access-state": {
			if (state.accessVersion !== void 0 && state.accessVersion >= frame.version) return;
			const { expiresAt: _expiresAt, ...retained } = state.spec;
			state.spec = {
				...retained,
				accessMode: frame.accessMode,
				allowedSubjectIds: frame.allowedSubjectIds,
				sessionMode: frame.sessionMode,
				allowsFork: frame.allowsFork === true,
				...frame.expiresAt === void 0 ? {} : { expiresAt: frame.expiresAt }
			};
			state.accessVersion = frame.version;
			notifyWorker({
				...frame,
				registrationToken: state.registrationToken,
				shareId
			});
			return;
		}
		case "share-rpc": {
			const previous = pendingRpc.cancel(connection, frame.id);
			if (previous !== void 0) notifyCancellation(previous);
			const request = pendingRpc.add(shareId, frame.id, connection);
			const observed = viewerRequestFields(frame.body);
			console.log(`[liveshare] worker read received share=${shareId} relayId=${String(frame.id)} op=${observed.op} streamId=${observed.streamId}`);
			notifyWorker({
				type: "share-rpc-request",
				registrationToken: connection.registrationToken,
				token: request.token,
				id: frame.id,
				shareId,
				body: frame.body,
				actor: frame.actor
			});
			return;
		}
		case "share-rpc-cancel": {
			const request = pendingRpc.cancel(connection, frame.id);
			if (request !== void 0) notifyCancellation(request);
		}
	}
}
function notifyWorker(event) {
	if (process.send !== void 0) process.send(event);
}
/** Cancel Host work before forgetting requests on a closed Relay socket. */
function clearPendingRpc(connection) {
	for (const request of pendingRpc.clear(connection)) notifyCancellation(request);
}
function notifyCancellation(request) {
	notifyWorker({
		type: "share-rpc-cancel",
		registrationToken: request.owner.registrationToken,
		token: request.token,
		shareId: request.shareId,
		id: request.id
	});
}
//#endregion
//#region lib/types/owner-gateway/cli.js
/** Entry point for the owner gateway worker, launched by the host over an IPC channel. */
if (process.send === void 0) throw new Error("Liveshare gateway requires the owner worker IPC channel");
runGatewayWorker();
//#endregion
export {};
