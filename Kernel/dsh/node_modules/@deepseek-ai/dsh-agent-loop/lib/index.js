import { Service } from "@deepseek-ai/cordis";
import { randomUUID } from "node:crypto";
import z from "@deepseek-ai/schemastery";
import { brandString } from "@deepseek-ai/dsh-brand";
import { AgentModel, agentEvents, assembleContextFor, modelSelectionProjectionDefinition } from "@deepseek-ai/dsh-agent";
import { AssistantStreamAccumulator, BlockAssembler, LlmAttemptId, LlmError, createAssistantMessage, createDeveloperMessage, createSystemMessage, createUserMessage, errorChain, markAgentLoopRequest } from "@deepseek-ai/dsh-llm";
import { SessionLogOffset, SessionPreparation, ToolCallRecovery, canonicalHeader, headerEquals, interruptedTurnClosers, isReplacementSurfaceEvent } from "@deepseek-ai/dsh-session";
import { SessionPersistenceNotFoundError } from "@deepseek-ai/dsh-session-persistence";
import { assertNever, deepFreeze } from "@deepseek-ai/dsh-util-values";
import { createScope } from "@deepseek-ai/dsh-scope";
import { joinContextSections, renderContextSections, renderPrompt } from "@deepseek-ai/dsh-system-prompt";
import { ToolActivationError, ToolBatchExecutionError } from "@deepseek-ai/dsh-tools";
import { z as z$1 } from "zod";
//#region lib/types/inbox.js
/**
* Driver-owned durable agent inbox projection and command facade.
*
* @module @deepseek-ai/dsh-agent-loop/inbox
*/
/** Wire validation for pending agent input reconstructed from durable inbox splices. */
const inboxProjectionSchema = z$1.object({
	"next-turn": z$1.array(z$1.custom()).readonly(),
	"next-step": z$1.array(z$1.custom()).readonly()
}).readonly();
/** Standard fold that reconstructs pending input and rejects invalid durable splice history. */
const inboxProjectionDefinition = {
	key: "inbox",
	stateSchema: inboxProjectionSchema,
	init: () => ({
		"next-turn": [],
		"next-step": []
	}),
	apply(state, event) {
		if (event.type !== "agent/inbox/spliced") return state;
		const splice = event.data;
		try {
			const inbox = state[splice.target];
			const removedCount = splice.removedCount ?? 0;
			if (!Number.isSafeInteger(splice.start) || splice.start < 0 || splice.start > inbox.length || !Number.isSafeInteger(removedCount) || removedCount < 0 || splice.start + removedCount > inbox.length) throw new Error("invalid inbox splice");
			const next = inbox.toSpliced(splice.start, removedCount, ...splice.inserted);
			const ids = /* @__PURE__ */ new Set();
			for (const message of splice.target === "next-turn" ? [...next, ...state["next-step"]] : [...state["next-turn"], ...next]) {
				if (ids.has(message.id)) throw new Error(`message "${message.id}" is already pending`);
				ids.add(message.id);
			}
			return splice.target === "next-turn" ? {
				"next-turn": next,
				"next-step": state["next-step"]
			} : {
				"next-turn": state["next-turn"],
				"next-step": next
			};
		} catch (error) {
			throw new Error(`invalid persisted inbox splice at session seq ${event.seq}`, { cause: error });
		}
	},
	wire: {
		viewSchema: inboxProjectionSchema,
		view: (state) => state
	},
	stateVersion: 1
};
/**
* Driver-owned durable Inbox implementation used by ReactLoopAgent and focused
* provider tests.
* @param readState - reader for the live Inbox projection state.
* @param session - session whose durable events store pending input.
* @param dispatch - agent-scoped notifications for Inbox lifecycle events.
*/
var ReactLoopInbox = class {
	readState;
	session;
	dispatch;
	constructor(readState, session, dispatch) {
		this.readState = readState;
		this.session = session;
		this.dispatch = dispatch;
	}
	/** Prompts awaiting individual turns. */
	get nextTurn() {
		return this.readState()["next-turn"];
	}
	/** Input awaiting the next step boundary. */
	get nextStep() {
		return this.readState()["next-step"];
	}
	/** Whether either pending-message list contains work. */
	get hasPending() {
		const state = this.readState();
		return state["next-turn"].length > 0 || state["next-step"].length > 0;
	}
	/** Durably cancel all pending input, clearing next-step before next-turn. */
	clear() {
		this.splice("next-step", 0, this.nextStep.length, []);
		this.splice("next-turn", 0, this.nextTurn.length, []);
	}
	/**
	* Remove and return the complete batch proposed for one step.
	* @param target - whether this boundary also consumes one queued turn.
	* @param turn - turn that will own the claimed batch.
	* @returns next-step input followed by the queued turn, when requested.
	*/
	claim(target, turn) {
		const claimed = this.mutate("next-step", 0, this.nextStep.length, [], false);
		if (target === "next-turn") claimed.push(...this.mutate("next-turn", 0, 1, [], false));
		for (const message of claimed) this.dispatch.emit("agent/inbox/claimed", {
			message,
			turn
		});
		return claimed;
	}
	/**
	* Append one message to a pending list.
	* @param target - pending list to extend.
	* @param message - message to append.
	*/
	append(target, message) {
		this.splice(target, this.readState()[target].length, 0, [message]);
	}
	/**
	* Prepend one message to a pending list.
	* @param target - pending list to extend.
	* @param message - message to prepend.
	*/
	prepend(target, message) {
		this.splice(target, 0, 0, [message]);
	}
	/**
	* Replace one pending message in place.
	* @param messageId - identity of the pending message to replace.
	* @param newMessage - replacement message.
	* @returns whether the message was still pending.
	*/
	replace(messageId, newMessage) {
		const location = this.locate(messageId);
		if (location === void 0) return false;
		this.splice(location.target, location.index, 1, [newMessage]);
		return true;
	}
	/**
	* Remove one pending message.
	* @param messageId - identity of the pending message to remove.
	* @returns whether the message was still pending.
	*/
	remove(messageId) {
		const location = this.locate(messageId);
		if (location === void 0) return false;
		this.splice(location.target, location.index, 1, []);
		return true;
	}
	/**
	* Apply standard splice semantics and durably record the normalized result.
	* @param target - pending list to mutate.
	* @param start - splice position.
	* @param deleteCount - maximum number of messages to remove.
	* @param inserted - messages to insert at the resolved position.
	* @returns messages removed by the splice.
	*/
	splice(target, start, deleteCount, inserted) {
		return this.mutate(target, start, deleteCount, inserted, true);
	}
	/** Locate one pending identity across both owned lists. */
	locate(messageId) {
		const state = this.readState();
		for (const target of ["next-turn", "next-step"]) {
			const index = state[target].findIndex((message) => message.id === messageId);
			if (index >= 0) return {
				target,
				index
			};
		}
	}
	/** Commit one normalized mutation and publish its live events. */
	mutate(target, start, deleteCount, inserted, discardRemoved) {
		const state = this.readState();
		const inbox = state[target];
		const truncatedStart = Math.trunc(start);
		const offset = Number.isNaN(truncatedStart) ? 0 : truncatedStart;
		const actualStart = offset < 0 ? Math.max(inbox.length + offset, 0) : Math.min(offset, inbox.length);
		const truncatedDeleteCount = Math.trunc(deleteCount);
		const actualDeleteCount = Math.min(Math.max(Number.isNaN(truncatedDeleteCount) ? 0 : truncatedDeleteCount, 0), inbox.length - actualStart);
		if (actualDeleteCount === 0 && inserted.length === 0) return [];
		const candidate = inbox.toSpliced(actualStart, actualDeleteCount, ...inserted);
		const ids = /* @__PURE__ */ new Set();
		for (const message of target === "next-turn" ? [...candidate, ...state["next-step"]] : [...state["next-turn"], ...candidate]) {
			if (ids.has(message.id)) throw new Error(`message "${message.id}" is already pending`);
			ids.add(message.id);
		}
		const outcome = discardRemoved && actualDeleteCount > 0 ? "canceled" : void 0;
		const splice = {
			target,
			start: actualStart,
			...actualDeleteCount === 0 ? {} : { removedCount: actualDeleteCount },
			inserted,
			...outcome === void 0 ? {} : { outcome }
		};
		const removed = inbox.slice(actualStart, actualStart + actualDeleteCount);
		const event = this.session.append("agent/inbox/spliced", splice);
		if (discardRemoved) for (const message of removed) this.dispatch.emit("agent/inbox/discarded", { message });
		for (const message of event.data.inserted) this.dispatch.emit("agent/inbox/inserted", { message });
		return removed;
	}
};
//#endregion
//#region lib/types/runtime-context.js
/**
* Durable projection state for the two loop-owned surface messages the system
* prompt plugin forms: the system prompt (surface node 0 and any in-history
* replacement) and the dynamic runtime-context snapshot.
* @module @deepseek-ai/dsh-agent-loop/runtime-context
*/
const SOURCE = "runtime-context";
const CLEARED = "Current runtime context: none. Earlier runtime-context snapshots no longer apply.";
function isOwned(message) {
	return message.source.kind === SOURCE;
}
function textOf(message) {
	const [block] = message.content;
	return message.content.length === 1 && block?.type === "text" ? block.text : void 0;
}
/** Committed events from the newest backward; the restore scans stop at the first match. */
function eventsNewestFirst(session) {
	return session.snapshotEvents().toReversed();
}
/**
* Decides how a rendered system prompt reaches the surface without owning the
* commit. The first prompt, even empty, reserves surface node 0.
* A capable continuing series appends changed nonempty text after the
* cached history. An incapable route, a cleared prompt, or a new series whose
* effective text differs instead normalizes the first system node and empties
* later active nodes. A series start whose latest surviving node already holds
* the rendering emits nothing: moving the same text to node 0 would change the
* request prefix without changing the prompt. Dormant empty tails do not supply
* effective text or require repeated replacements.
*/
var SystemPromptProjection = class {
	session;
	constructor(session) {
		this.session = session;
	}
	/** The surviving `system/message` nodes in surface order. */
	systemNodes() {
		const nodes = [];
		for (const seq of this.session.surface.nodes) {
			const event = this.session.eventAt(seq);
			if (event?.type !== "system/message") continue;
			const text = event.data.message.content.length === 0 ? "" : textOf(event.data.message);
			nodes.push({
				seq,
				text
			});
		}
		return nodes;
	}
	/**
	* Reconcile effective text and retained nodes with the prepared route and series.
	* A series start re-baselines node 0 only when the effective text differs; a
	* capable continuing series keeps appended nodes in place to preserve the prefix.
	* @param rendered - the fully rendered system prompt; `''` when none is active.
	* @param input - the route capability and series facts for this step.
	* @returns ordered per-node updates; an empty list means no update is needed.
	*/
	project(rendered, input) {
		const nodes = this.systemNodes();
		const head = nodes[0];
		if (head === void 0) return [{
			message: createSystemMessage(rendered),
			intent: { surfaceOp: "append" }
		}];
		const latest = nodes.findLast((node) => node.text !== "") ?? head;
		if (!input.inHistory || rendered.length === 0 || input.startsSeries && latest.text !== rendered) {
			const updates = nodes.slice(1).filter((node) => node.text !== "").map((node) => this.replace(node.seq, ""));
			if (head.text !== rendered) updates.push(this.replace(head.seq, rendered));
			return updates;
		}
		if (latest.text === rendered) return [];
		return [{
			message: createSystemMessage(rendered),
			intent: { surfaceOp: "append" }
		}];
	}
	replace(seq, text) {
		return {
			message: createSystemMessage(text),
			intent: {
				surfaceOp: {
					op: "replace",
					startSeq: seq,
					endSeq: seq
				},
				sourceEventSeqs: [seq]
			}
		};
	}
};
/** Tracks the last retained runtime-context snapshot without owning its commit. */
var RuntimeContextProjection = class {
	/** `undefined` means no snapshot ever existed; `null` means none is retained. */
	retained;
	/**
	* Restore projection state once, then follow authoritative session events.
	* @param ctx - agent-scoped event context.
	* @param session - session receiving projected messages.
	*/
	constructor(ctx, session) {
		const surface = new Set(session.surface.nodes);
		for (const event of eventsNewestFirst(session)) {
			if (event.type !== "user/message" || !isOwned(event.data)) continue;
			this.retained ??= null;
			if (surface.has(event.seq)) {
				this.retained = {
					seq: event.seq,
					text: textOf(event.data)
				};
				break;
			}
		}
		ctx.on("session/event", (subject, event) => {
			if (subject !== session) return;
			if (event.type === "user/message" && isOwned(event.data)) this.retained = {
				seq: event.seq,
				text: textOf(event.data)
			};
			else if (this.retained && isReplacementSurfaceEvent(event) && event.sourceEventSeqs?.includes(this.retained.seq) === true) this.retained = null;
		});
	}
	/**
	* Create an uncommitted snapshot only when the retained value differs.
	* @param current - fully rendered dynamic context.
	* @param sections - named contributions that formed the current snapshot.
	* @returns a candidate user message, or `undefined` when no update is needed.
	*/
	project(current, sections) {
		if (this.retained === void 0 && current.length === 0) return;
		const snapshot = current.length === 0 ? CLEARED : current;
		if (this.retained?.text === snapshot) return;
		return createUserMessage({
			content: [{
				type: "text",
				text: snapshot
			}],
			source: sections.length === 0 ? { kind: SOURCE } : {
				kind: SOURCE,
				form: "snapshot",
				sections
			}
		});
	}
};
//#endregion
//#region lib/types/assistant-stream.js
/** Process-local assistant attempt framing and durable stream accumulation. */
/** Folds one model attempt into one compact stream plus ordered transient frames. */
var AssistantStreamAttempt = class {
	nextRevision;
	turn;
	step;
	emit;
	accumulator = new AssistantStreamAccumulator();
	assembler = new BlockAssembler();
	index = 0;
	terminal = false;
	/** Attempt identity unique within this Agent lifecycle. */
	attemptId;
	/** Whether this started attempt has emitted its terminal frame. */
	get ended() {
		return this.terminal;
	}
	/**
	* @param sessionId - identity embedded only in the Agent-lifecycle-local attempt id.
	* @param attempt - attached-Session-local attempt counter.
	* @param nextRevision - allocates the next emitted frame revision.
	* @param turn - durable turn owning the request.
	* @param step - durable step owning the request.
	* @param emit - agent-scoped notification publisher.
	*/
	constructor(sessionId, attempt, nextRevision, turn, step, emit) {
		this.nextRevision = nextRevision;
		this.turn = turn;
		this.step = step;
		this.emit = emit;
		this.attemptId = LlmAttemptId(`${sessionId}:${attempt}`);
	}
	/** Publish the opening marker before the first delivered chunk. */
	start() {
		this.emit({
			type: "start",
			attemptId: this.attemptId,
			revision: this.nextRevision(),
			turn: this.turn,
			step: this.step
		});
	}
	/** Snapshot one chunk once, then feed durable compaction, assembly, and live publication. */
	push(chunk) {
		const timed = this.accumulator.push({
			time: Date.now(),
			chunk
		});
		this.assembler.push(timed.chunk);
		this.emit({
			type: "chunk",
			attemptId: this.attemptId,
			revision: this.nextRevision(),
			index: this.index++,
			time: timed.time,
			chunk: timed.chunk
		});
	}
	/**
	* Publish terminal settlement after the matching durable event commits.
	* @param eventType - durable settlement type.
	* @param append - synchronous durable append returning its committed seq.
	*/
	settle(eventType, append) {
		let seq;
		try {
			seq = append();
		} catch (error) {
			this.abandon();
			throw error;
		}
		this.terminal = true;
		this.emit({
			type: "end",
			attemptId: this.attemptId,
			revision: this.nextRevision(),
			index: this.index,
			outcome: {
				kind: "committed",
				eventType,
				seq
			}
		});
	}
	/** Publish abandonment when no durable attempt event can be committed. */
	abandon() {
		this.terminal = true;
		this.emit({
			type: "end",
			attemptId: this.attemptId,
			revision: this.nextRevision(),
			index: this.index,
			outcome: { kind: "abandoned" }
		});
	}
	/** Exact compact stream for the final durable event. */
	get stream() {
		return [...this.accumulator.snapshot()];
	}
	/** Canonical completed-message blocks from the same chunks. */
	blocks() {
		return this.assembler.blocks();
	}
	/** Safe visible prefix when cancellation interrupts the attempt. */
	interruptedBlocks() {
		return this.assembler.interruptedBlocks();
	}
	/** Latest adapter-reported usage in the stream. */
	get usage() {
		return this.assembler.usage;
	}
	/** Terminal reason, defaulting to stop when the stream omitted one. */
	get finish() {
		return this.assembler.finish;
	}
	/** Replay state carried by the terminal finish record. */
	get replayState() {
		return this.assembler.replayState;
	}
};
//#endregion
//#region lib/types/agent.js
/**
* Default Agent driver over queued turns and step-boundary input. Every request
* is derived from the session log.
* @module dsh-agent-loop/agent
*/
/** Remove adapter-derived values before plugins propose the next request config. */
function requestProposal(header) {
	if (header.adapterDefaults === void 0) return header.config;
	const proposal = { ...header.config };
	if (header.adapterDefaults.reasoningEffort === true) delete proposal.reasoningEffort;
	if (header.adapterDefaults.maxTokens === true) delete proposal.maxTokens;
	return proposal;
}
/**
* Read the cause `cancel()` passed when aborting a loop-owned signal, copying
* only the fields `turn/end` records. The live reason stays the caller's
* object, and Node's fetch assigns a `stack` onto it that `Session.append`
* would either log or reject as data JSON cannot hold.
* @param signal - a turn or maintenance signal this loop owns.
* @returns the copied cause, or undefined while the signal is still live.
*/
function abortedCancelCause(signal) {
	if (!signal.aborted) return void 0;
	const cause = signal.reason;
	switch (cause.kind) {
		case "user":
		case "parent":
		case "disposed": return { kind: cause.kind };
		case "hook": return {
			kind: "hook",
			reason: cause.reason
		};
		/* v8 ignore next -- cancel accepts the closed AgentCancelCause union */
		default: return assertNever(cause);
	}
}
/** Drives one session through turn and step boundaries. */
var ReactLoopAgent = class {
	loopCtx;
	id;
	options;
	session;
	model;
	inbox;
	phase;
	activityDone = Promise.resolve();
	/** The agent-scoped registration boundary; the lifecycle owner unwinds it after the driver exits. */
	scope;
	ctx;
	/** Fused dispatcher, built once in the constructor so hot-path dispatches never allocate. */
	dispatch;
	/** Whether this loop instance has appended its initial/resume request anchor. */
	requestHeaderLogged = false;
	/** Surface generation at attachment or the preceding built request. */
	requestSurfaceGeneration;
	runtimeContext;
	/** Process-local revision of assistant frames for this attached Session. */
	assistantStreamRevision = 0;
	assistantAttemptCounter = 0;
	systemPrompt;
	/** Identities fully frozen by this loop; weak references do not retain replaced history. */
	frozenMessages = /* @__PURE__ */ new WeakSet();
	constructor(loopCtx, id, options, session, projections, lastTurn) {
		this.loopCtx = loopCtx;
		this.id = id;
		this.options = options;
		this.session = session;
		this.requestSurfaceGeneration = session.surface.contentGeneration;
		this.dispatch = agentEvents(loopCtx, this);
		this.scope = createScope(loopCtx, this);
		this.ctx = this.scope.ctx;
		this.model = new AgentModel(projections.modelSelection, session, options);
		this.inbox = new ReactLoopInbox(projections.inbox, session, this.dispatch);
		this.phase = {
			kind: "idle",
			lastTurn
		};
		this.runtimeContext = new RuntimeContextProjection(this.ctx, session);
		this.systemPrompt = new SystemPromptProjection(session);
	}
	get status() {
		return this.phase.kind === "idle" || this.phase.kind === "maintenance" ? "idle" : "running";
	}
	/** Commit a phase and publish its externally visible status transition. */
	setPhase(next) {
		const previousStatus = this.status;
		this.phase = next;
		const status = this.status;
		if (status !== previousStatus) this.dispatch.emit("agent/status", { status });
	}
	send(message, target, wakeup) {
		const wakingAfterAbort = wakeup && this.phase.kind !== "idle" && this.phase.abort.signal.aborted;
		const resolvedTarget = wakingAfterAbort ? "next-turn" : target;
		this.inbox.splice(resolvedTarget, Infinity, 0, [message]);
		if (wakeup) this.wakeDriver(wakingAfterAbort);
	}
	followup(input) {
		this.send(input, "next-turn", true);
	}
	steer(input) {
		this.send(input, "next-step", true);
	}
	inject(input) {
		this.send(input, "next-step", false);
	}
	cancel(cause, options = {}) {
		if (!options.keepInbox) {
			this.inbox.clear();
			if (this.phase.kind !== "idle") this.phase.wakeRequested = false;
		}
		if (this.phase.kind !== "idle") this.phase.abort.abort(cause);
	}
	runMaintenance(job) {
		if (this.phase.kind !== "idle") throw new Error(`agent "${this.id}" already has active work`);
		const done = Promise.withResolvers();
		const maintenance = {
			kind: "maintenance",
			abort: new AbortController(),
			lastTurn: this.phase.lastTurn,
			wakeRequested: false
		};
		this.setPhase(maintenance);
		this.activityDone = done.promise;
		return (async () => {
			try {
				return await job(maintenance.abort.signal);
			} finally {
				this.setPhase({
					kind: "idle",
					lastTurn: maintenance.lastTurn
				});
				if (abortedCancelCause(maintenance.abort.signal)?.kind !== "disposed" && maintenance.wakeRequested && this.inbox.hasPending) this.wakeDriver();
				done.resolve();
			}
		})();
	}
	/**
	* Start one driver, or latch its wake behind maintenance or an aborted
	* activity. A wake sent while idle always opens its turn boundary, even
	* when its message was cleared; only a latched replay is suppressed when
	* the queue no longer holds the wake.
	* @param wakeAfterAbort - the {@link send} classification, captured before
	*   the inbox insertion so a reentrant cancel cannot reclassify it.
	*/
	wakeDriver(wakeAfterAbort = false) {
		if (this.phase.kind !== "idle") {
			if (abortedCancelCause(this.phase.abort.signal)?.kind !== "disposed" && (this.phase.kind === "maintenance" || wakeAfterAbort)) this.phase.wakeRequested = true;
			return;
		}
		const driver = Promise.withResolvers();
		this.activityDone = driver.promise;
		this.setPhase({
			kind: "running",
			abort: new AbortController(),
			turn: this.phase.lastTurn,
			step: 0,
			wakeRequested: false
		});
		this.loopCtx.agents.withInitiator(this, () => this.kick()).then(driver.resolve, driver.reject);
	}
	async whenIdle() {
		let activity;
		do
			await (activity = this.activityDone);
		while (activity !== this.activityDone);
	}
	/** Report one failure at its live boundary, then preserve it for driver containment. */
	throwError(error) {
		const turn = this.phase.kind === "running" ? this.phase.turn : this.phase.lastTurn;
		const step = this.phase.kind === "running" ? this.phase.step : 0;
		this.dispatch.emit("agent/error", {
			turn,
			step,
			error
		});
		throw error;
	}
	async kick() {
		try {
			while (await this.turn());
		} catch (_error) {} finally {
			/* v8 ignore next -- kick owns a running phase until this driver boundary */
			if (this.phase.kind === "running") {
				const { turn, wakeRequested } = this.phase;
				this.setPhase({
					kind: "idle",
					lastTurn: turn
				});
				if (wakeRequested && this.inbox.hasPending) this.wakeDriver();
			}
		}
	}
	async preStep(target, position) {
		/* v8 ignore next -- private callers establish the running phase before proposing a step */
		if (this.phase.kind !== "running") throw new Error(`agent "${this.id}": pre-step outside running phase`);
		const signal = this.phase.abort.signal;
		const claimed = this.inbox.claim(target, position.turn);
		const selection = this.model.current();
		if (selection === void 0) throw new Error(`agent "${this.id}" has no provider/model: set AgentOptions.provider and AgentOptions.model or record a model selection`);
		const toolSet = await this.loopCtx.tools.resolve({
			agent: this,
			modelSelection: selection,
			signal
		});
		try {
			const assembly = await this.loopCtx.systemPrompt.assemble({
				...assembleContextFor(this, signal),
				modelSelection: selection,
				toolSet
			});
			signal.throwIfAborted();
			const decision = await this.dispatch.waterfall("agent/pre-step", {
				messages: claimed,
				...position,
				signal,
				modelSelection: selection
			}, () => {
				const sections = renderContextSections(assembly);
				const context = this.runtimeContext.project(joinContextSections(sections), sections);
				return Promise.resolve({
					kind: "enter",
					messages: context === void 0 ? claimed : [...claimed, context]
				});
			});
			signal.throwIfAborted();
			if (decision.kind === "reject") {
				toolSet.dispose();
				return decision;
			}
			return {
				...decision,
				assembly,
				selection,
				toolSet
			};
		} catch (error) {
			toolSet.dispose();
			throw error;
		}
	}
	/** Whether the assembled tool schemas differ from the logged request header's. */
	toolsChanged(tools) {
		const baseline = this.session.requestHeader();
		if (baseline === void 0) return false;
		return !headerEquals(baseline, canonicalHeader({
			...baseline,
			tools: [...tools]
		}));
	}
	/** Open one turn before claiming its first proposed step. */
	async turn() {
		if (this.phase.kind !== "running") this.throwError(/* @__PURE__ */ new Error(`agent "${this.id}": turn without driver reservation`));
		const phase = this.phase;
		const { signal } = phase.abort;
		signal.throwIfAborted();
		const turn = phase.turn + 1;
		try {
			this.session.append("turn/start", { turn });
		} catch (error) {
			this.throwError(error);
		}
		phase.turn = turn;
		let turnEnds = null;
		let target = "next-turn";
		try {
			while (true) {
				signal.throwIfAborted();
				const step = phase.step + 1;
				const decision = await this.preStep(target, {
					turn,
					step
				});
				if (decision.kind === "reject") {
					turnEnds = { kind: "blocked" };
					return false;
				}
				try {
					if (turnEnds && decision.messages.length === 0) break;
					if (phase.step === 0 && decision.messages.length === 0) {
						turnEnds = { kind: "completed" };
						return false;
					}
					signal.throwIfAborted();
					this.session.append("step/start", {
						turn,
						step
					});
					phase.step = step;
					const toolRecovery = new ToolCallRecovery();
					const stopRecovery = this.ctx.on("session/event", (session, event) => {
						if (session === this.session) toolRecovery.observe(event);
					});
					try {
						const stepEnd = await this.step(decision);
						if (turnEnds === null || turnEnds.kind !== "max-tokens") turnEnds = stepEnd;
					} catch (error) {
						try {
							for (const event of toolRecovery.results()) this.session.append("tool/result", event.data, {
								surfaceOp: "append",
								...event.sourceEventSeqs === void 0 ? {} : { sourceEventSeqs: event.sourceEventSeqs }
							});
						} catch (recoveryError) {
							throw new AggregateError([error, recoveryError], "Step failed and its pending tool results could not be recorded", { cause: error });
						}
						throw error;
					} finally {
						stopRecovery();
						this.session.append("step/end", {
							turn,
							step
						});
					}
				} finally {
					decision.toolSet.dispose();
				}
				signal.throwIfAborted();
				if (turnEnds && this.inbox.nextStep.length === 0) {
					await this.dispatch.serial("agent/turn-stopping", {
						turn,
						signal
					});
					signal.throwIfAborted();
				}
				if (turnEnds && this.inbox.nextStep.length === 0) break;
				target = "next-step";
			}
		} catch (error) {
			const cause = abortedCancelCause(signal);
			if (cause !== void 0) {
				turnEnds = {
					kind: "aborted",
					reason: cause
				};
				throw error;
			}
			turnEnds = {
				kind: "error",
				error: error instanceof LlmError ? error.failure : {
					message: errorChain(error),
					code: error instanceof ToolActivationError ? error.code : "UNKNOWN"
				}
			};
			this.throwError(error);
		} finally {
			try {
				this.session.append("turn/end", {
					turn,
					reason: turnEnds
				});
			} catch (error) {
				this.throwError(error);
			}
		}
		if (!this.inbox.hasPending) return false;
		phase.abort = new AbortController();
		phase.wakeRequested = false;
		phase.step = 0;
		return true;
	}
	async step(decision) {
		/* v8 ignore next -- private callers establish the running phase before executing a step */
		if (this.phase.kind !== "running") throw new Error(`agent "${this.id}": step outside running phase`);
		const { turn, step, abort: { signal } } = this.phase;
		signal.throwIfAborted();
		const { assembly } = decision;
		const renderedPrompt = renderPrompt(assembly);
		let firstAttempt = true;
		while (true) {
			const { config, preparedCall } = await this.prepareRequest(turn, step, decision.selection, signal);
			const sections = renderContextSections(this.loopCtx.systemPrompt.refreshContext(assembly, {
				...assembleContextFor(this, signal),
				modelSelection: decision.selection
			}));
			const context = this.runtimeContext.project(joinContextSections(sections), sections);
			signal.throwIfAborted();
			const startsRequestSeries = firstAttempt && decision.startsRequestSeries === true;
			const commits = this.systemPrompt.project(renderedPrompt, {
				inHistory: preparedCall?.systemPromptUpdate === "in-history",
				startsSeries: startsRequestSeries || this.requestSurfaceGeneration !== this.session.surface.contentGeneration || preparedCall?.toolUpdate === void 0 && this.toolsChanged(assembly.tools)
			});
			for (const { message, intent } of commits) this.session.append("system/message", {
				turn,
				step,
				message
			}, intent);
			let contextAdmitted = false;
			if (firstAttempt) for (const message of decision.messages) if (message.source.kind === "runtime-context") {
				if (context !== void 0 && !contextAdmitted) {
					this.session.append("user/message", context, { surfaceOp: "append" });
					contextAdmitted = true;
				}
			} else this.session.append("user/message", message, { surfaceOp: "append" });
			if (context !== void 0 && !contextAdmitted) this.session.append("user/message", context, { surfaceOp: "append" });
			firstAttempt = false;
			const request = this.buildRequest(config, preparedCall, assembly.tools, {
				turn,
				step
			}, startsRequestSeries, signal);
			const live = new AssistantStreamAttempt(this.session.id, ++this.assistantAttemptCounter, () => ++this.assistantStreamRevision, turn, step, (frame) => {
				this.dispatch.emit("agent/assistant-stream", { frame });
			});
			let started = false;
			try {
				const stream = preparedCall?.stream(request) ?? this.loopCtx.llm.stream(request);
				signal.throwIfAborted();
				live.start();
				started = true;
				for await (const chunk of stream) {
					signal.throwIfAborted();
					live.push(chunk);
				}
				signal.throwIfAborted();
			} catch (error) {
				if (!started) throw error;
				try {
					if (signal.aborted) {
						const content = live.interruptedBlocks();
						if (content.length > 0) live.settle("assistant/message", () => this.session.append("assistant/message", {
							turn,
							step,
							message: createAssistantMessage({
								content,
								source: {
									provider: request.provider,
									model: request.model,
									...live.replayState === void 0 ? {} : { replayState: live.replayState }
								}
							}),
							interrupted: true,
							...live.usage === void 0 ? {} : { usage: live.usage },
							stream: live.stream
						}, { surfaceOp: "append" }).seq);
						else live.settle("assistant/attempt", () => this.session.append("assistant/attempt", {
							turn,
							step,
							stream: live.stream
						}).seq);
					} else live.settle("assistant/attempt", () => this.session.append("assistant/attempt", {
						turn,
						step,
						stream: live.stream
					}).seq);
				} catch (settlementError) {
					throw new AggregateError([error, settlementError], "Assistant stream failed and its durable settlement was rejected", { cause: error });
				}
				throw error;
			}
			try {
				const finish = live.finish;
				if (finish.kind === "error" || finish.kind === "aborted") {
					live.settle("assistant/attempt", () => this.session.append("assistant/attempt", {
						turn,
						step,
						stream: live.stream
					}).seq);
					const action = await this.dispatch.waterfall("agent/request-error", {
						turn,
						step,
						provider: request.provider,
						failure: finish.failure,
						retryPolicy: preparedCall?.retryPolicy,
						signal
					}, () => Promise.resolve(void 0));
					signal.throwIfAborted();
					if (action?.kind !== "retry") throw new LlmError(finish.failure.message, finish.failure.code, finish.failure);
					continue;
				}
				const message = createAssistantMessage({
					content: live.blocks(),
					source: {
						provider: request.provider,
						model: request.model,
						...live.replayState !== void 0 ? { replayState: live.replayState } : {}
					}
				});
				live.settle("assistant/message", () => this.session.append("assistant/message", {
					turn,
					step,
					message,
					...live.usage === void 0 ? {} : { usage: live.usage },
					stream: live.stream
				}, { surfaceOp: "append" }).seq);
				if (finish.kind === "max-tokens") return { kind: "max-tokens" };
				const toolCalls = message.content.filter((block) => block.type === "tool-call");
				if (toolCalls.length === 0) return { kind: "completed" };
				const maxParallelToolCalls = this.loopCtx.agentLoop.config.maxParallelToolCalls.get();
				try {
					const result = await this.loopCtx.tools.execute({
						calls: toolCalls,
						signal,
						agent: this,
						log: {
							turn,
							step
						},
						maxParallelToolCalls,
						toolSet: decision.toolSet
					});
					for (const context of result.additionalContexts) this.inbox.splice("next-step", this.inbox.nextStep.length, 0, [context]);
					return result.concluded ? { kind: "completed" } : null;
				} catch (error) {
					if (!(error instanceof ToolBatchExecutionError)) throw error;
					for (const context of error.partial.additionalContexts) this.inbox.splice("next-step", this.inbox.nextStep.length, 0, [context]);
					throw error.cause;
				}
			} catch (error) {
				if (!live.ended) live.abandon();
				throw error;
			}
		}
	}
	/** Resolve request config for the step's selection and bind its adapter before admitting model-visible input. */
	async prepareRequest(turn, step, selection, signal) {
		const { session } = this;
		const maxTokens = this.options.maxTokens;
		const { provider: _provider, model: _model, reasoningEffort: _effort, ...controls } = this.requestHeaderLogged ? requestProposal(session.requestHeader()) : maxTokens === void 0 ? {} : { maxTokens };
		const seedConfig = deepFreeze(structuredClone({
			...controls,
			...selection
		}));
		const proposedConfig = await this.dispatch.waterfall("agent/request", {
			turn,
			step,
			signal
		}, () => Promise.resolve(seedConfig));
		signal.throwIfAborted();
		if (proposedConfig.provider !== selection.provider || proposedConfig.model !== selection.model) throw new Error(`agent "${this.id}": agent/request proposed "${proposedConfig.provider}/${proposedConfig.model}" after step preparation fixed "${selection.provider}/${selection.model}"; use agent.model.select() before step preparation`);
		if (proposedConfig.reasoningEffort !== selection.reasoningEffort) throw new Error(`agent "${this.id}": agent/request proposed reasoning effort "${proposedConfig.reasoningEffort ?? "adapter default"}" after step preparation fixed "${selection.reasoningEffort ?? "adapter default"}"; use agent.model.select() before step preparation`);
		let config;
		let preparedCall;
		try {
			preparedCall = await this.loopCtx.llm.prepareCall(proposedConfig, signal);
			config = preparedCall.config;
		} catch (error) {
			if (!(error instanceof LlmError) || error.code !== "NO_ADAPTER") throw error;
			config = proposedConfig;
		}
		signal.throwIfAborted();
		return {
			config,
			...preparedCall === void 0 ? {} : { preparedCall }
		};
	}
	/** Log the resolved envelope and derive a frozen request from the admitted surface. */
	buildRequest(config, preparedCall, tools, position, startsRequestSeries, signal) {
		const { session } = this;
		const surfaceGeneration = session.surface.contentGeneration;
		const header = canonicalHeader({
			config,
			...preparedCall === void 0 ? {} : { adapterDefaults: preparedCall.adapterDefaults },
			...tools.length > 0 ? { tools } : {}
		});
		const baseline = this.session.requestHeader();
		const startsSeries = startsRequestSeries || this.requestSurfaceGeneration !== surfaceGeneration;
		let headerSeq;
		if (!this.requestHeaderLogged) {
			headerSeq = this.session.append("request/header", {
				header,
				reason: baseline === void 0 ? "initial" : "resume",
				...startsSeries ? { startsSeries: true } : {}
			}).seq;
			this.requestHeaderLogged = true;
		} else if (baseline === void 0 || !headerEquals(baseline, header)) headerSeq = this.session.append("request/header", {
			header,
			reason: "change",
			...startsSeries ? { startsSeries: true } : {}
		}).seq;
		else if (startsSeries) this.session.append("request/header", {
			header,
			reason: "series"
		});
		if (baseline !== void 0 && headerSeq !== void 0) {
			const previousNames = new Set(baseline.tools?.map((tool) => tool.name));
			const currentNames = new Set(tools.map((tool) => tool.name));
			const additions = tools.filter((tool) => !previousNames.has(tool.name)).map((tool) => ({
				type: "tool-addition",
				toolName: tool.name
			}));
			const removals = (baseline.tools ?? []).filter((tool) => !currentNames.has(tool.name)).map((tool) => ({
				type: "tool-removal",
				toolName: tool.name
			}));
			if (additions.length > 0 || removals.length > 0) session.append("developer/message", {
				...position,
				message: createDeveloperMessage({
					source: { kind: "tool-registry" },
					content: [...additions, ...removals]
				}),
				...additions.length > 0 ? { headerSeq } : {}
			}, { surfaceOp: "append" });
		}
		this.requestSurfaceGeneration = surfaceGeneration;
		const contextWindow = preparedCall?.context?.contextWindow;
		const systemPromptUpdate = preparedCall?.systemPromptUpdate;
		const requestContext = {
			provider: config.provider,
			model: config.model,
			...contextWindow === void 0 ? {} : { contextWindow },
			...systemPromptUpdate === void 0 ? {} : { systemPromptUpdate }
		};
		const previousContext = session.requestContext();
		if (previousContext?.provider !== requestContext.provider || previousContext.model !== requestContext.model || previousContext.contextWindow !== requestContext.contextWindow || previousContext.systemPromptUpdate !== requestContext.systemPromptUpdate) session.append("request/context", requestContext);
		signal.throwIfAborted();
		deepFreeze(header);
		const boundaryMessages = session.deriveMessages();
		for (const message of boundaryMessages) {
			if (this.frozenMessages.has(message)) continue;
			deepFreeze(message);
			this.frozenMessages.add(message);
		}
		Object.freeze(boundaryMessages);
		return markAgentLoopRequest(Object.freeze({
			...header.config,
			messages: boundaryMessages,
			toolHistory: session.toolHistory(),
			...header.tools !== void 0 ? { tools: header.tools } : {},
			sessionId: this.session.id,
			signal
		}));
	}
};
//#endregion
//#region lib/types/constants.js
/** Shared agent-loop scheduler defaults.
* @module dsh-agent-loop/constants
*/
/** Default maximum in-flight parallel-safe calls per agent step. */
const DEFAULT_MAX_PARALLEL_TOOL_CALLS = 10;
//#endregion
//#region lib/types/index.js
var __addDisposableResource = function(env, value, async) {
	if (value !== null && value !== void 0) {
		if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
		var dispose, inner;
		if (async) {
			if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
			dispose = value[Symbol.asyncDispose];
		}
		if (dispose === void 0) {
			if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
			dispose = value[Symbol.dispose];
			if (async) inner = dispose;
		}
		if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
/** Fiber states that cannot own or serve a new lifecycle. */
const INACTIVE_STATES = new Set([
	5,
	4,
	3
]);
/** Re-throw one collected failure as itself, or several as one `AggregateError`. */
function throwCollectedFailures(failures, message) {
	if (failures.length === 1) throw failures[0];
	if (failures.length > 1) throw new AggregateError(failures, message);
}
/** Factory-level ownership: live agent teardowns plus config startup work. */
var FactoryOwnership = class {
	fiber;
	accepting = true;
	teardown = new AbortController();
	inactive = Promise.withResolvers();
	liveAgents = /* @__PURE__ */ new Set();
	startupTasks = /* @__PURE__ */ new Set();
	constructor(fiber) {
		this.fiber = fiber;
	}
	/** Aborts (reason: `agent loop is not active` error) when factory teardown begins. */
	get signal() {
		return this.teardown.signal;
	}
	isActive() {
		return this.accepting && !INACTIVE_STATES.has(this.fiber.state);
	}
	/** Track one live agent's shared teardown until it has run. */
	track(dispose) {
		this.liveAgents.add(dispose);
		return () => {
			this.liveAgents.delete(dispose);
		};
	}
	/** Join config startup work that begins before an agent exists. */
	trackStartup(job) {
		this.startupTasks.add(job);
		const forget = () => {
			this.startupTasks.delete(job);
		};
		job.then(forget, forget);
	}
	/** Join one public create/resume continuation; factory dispose awaits its settlement. */
	trackWrapper(job) {
		this.trackStartup(job.then(() => void 0, () => void 0));
	}
	/** Resolve `task`, or stop waiting when factory teardown begins. */
	async waitWhileActive(job) {
		await Promise.race([job, this.inactive.promise]);
	}
	async dispose() {
		this.accepting = false;
		this.teardown.abort(/* @__PURE__ */ new Error("agent loop is not active"));
		this.inactive.resolve();
		const settlements = await Promise.allSettled([...[...this.liveAgents].map((dispose) => dispose()), ...this.startupTasks]);
		const failures = [];
		for (const result of settlements) if (result.status === "rejected") failures.push(result.reason);
		throwCollectedFailures(failures, "agent loop disposal failed");
	}
};
/** Await `operation`, or throw the signal's reason as soon as it aborts. */
async function raceAbort(operation, signal, id) {
	const toAbortError = () => signal.reason instanceof Error ? signal.reason : new Error(`agent "${id}" creation aborted`, { cause: signal.reason });
	if (signal.aborted) throw toAbortError();
	const aborted = Promise.withResolvers();
	const listener = () => {
		aborted.reject(toAbortError());
	};
	signal.addEventListener("abort", listener, { once: true });
	try {
		return await Promise.race([Promise.resolve(operation), aborted.promise]);
	} finally {
		signal.removeEventListener("abort", listener);
	}
}
/** Start an abortable operation and release a value that arrives after cancellation. */
async function raceAbortCall(operation, signal, id, releaseAbandoned) {
	if (signal.aborted) throw signal.reason instanceof Error ? signal.reason : new Error(`agent "${id}" creation aborted`, { cause: signal.reason });
	const pending = Promise.resolve().then(operation);
	try {
		return await raceAbort(pending, signal, id);
	} catch (error) {
		if (signal.aborted && releaseAbandoned !== void 0) pending.then(releaseAbandoned, () => void 0);
		throw error;
	}
}
/** Reject an output-token cap that cannot be represented exactly on the request wire. */
function assertAgentOptions(options) {
	if (options.maxTokens !== void 0 && (!Number.isSafeInteger(options.maxTokens) || options.maxTokens <= 0)) throw new TypeError("agent maxTokens must be a positive safe integer");
}
/**
* Context key a launcher sets before any Loader entry mounts
* (`ctx.provide(CONFIGURED_AGENT_IDENTITIES_KEY, identities)`) to fix
* configured agents' session identities without a config key, so an overlay
* repointing the row's model route cannot drop them.
*/
const CONFIGURED_AGENT_IDENTITIES_KEY = "configuredAgentIdentities";
/**
* Apply launcher-owned identities over the configured agents, replacing both
* identity keys for every entry the launcher named so a config-supplied
* identity can never survive alongside a launcher-supplied one.
* @param agents - the configured agent entries.
* @param identities - launcher identities keyed by configured agent `id`, or `undefined`.
* @returns the entries with launcher-owned identities applied.
*/
function applyLauncherIdentities(agents, identities) {
	if (identities === void 0) return agents;
	return agents.map((agent) => {
		const identity = identities[agent.id];
		if (identity === void 0) return agent;
		const { sessionId: _sessionId, resumeSessionId: _resumeSessionId, ...rest } = agent;
		return identity.resume ? {
			...rest,
			resumeSessionId: identity.id
		} : {
			...rest,
			sessionId: identity.id
		};
	});
}
/** Reject self-contained identity conflicts before any configured agent starts. */
function validateConfiguredAgents(agents) {
	const exactIdentities = /* @__PURE__ */ new Map();
	for (const { id, sessionId, resumeSessionId } of agents) {
		const hasResumeId = resumeSessionId !== void 0 && resumeSessionId !== "";
		if (sessionId !== void 0 && hasResumeId) throw new Error(`agent "${id}": sessionId and resumeSessionId are mutually exclusive`);
		const exactIdentity = hasResumeId ? resumeSessionId : sessionId;
		if (exactIdentity === void 0) continue;
		const firstId = exactIdentities.get(exactIdentity);
		if (firstId !== void 0) throw new Error(`agents "${firstId}" and "${id}" use duplicate exact session identity "${exactIdentity}"`);
		exactIdentities.set(exactIdentity, id);
	}
}
/** Concrete agent factory and driver service. */
var AgentLoop = class extends Service {
	static inject = [
		"agents",
		"sessions",
		"llm",
		"tools",
		"systemPrompt",
		"sessionProjections"
	];
	/** Runtime schema for declarative agents. */
	static Config = z.object({
		maxParallelToolCalls: z.number().step(1).min(1).default(10).volatile(),
		agents: z.array(z.object({
			id: z.string().required(),
			sessionId: z.string().min(1),
			provider: z.string(),
			model: z.string(),
			reasoningEffort: z.string().min(1),
			maxTokens: z.number().step(1).min(1).max(Number.MAX_SAFE_INTEGER),
			cwd: z.string(),
			resumeSessionId: z.string()
		})).default([])
	});
	/** Validated configuration owned by the agent-loop service. */
	config;
	ownership;
	/** Plain holder prevents Cordis from re-tracing the factory's dependency context through a caller shadow. */
	runtime;
	modelProjectionHandle;
	inboxProjectionHandle;
	constructor(ctx, config) {
		super(ctx, "agentLoop");
		this.config = {
			agents: applyLauncherIdentities(config.agents, ctx.get(CONFIGURED_AGENT_IDENTITIES_KEY)),
			maxParallelToolCalls: config.maxParallelToolCalls
		};
		validateConfiguredAgents(this.config.agents);
		this.ownership = new FactoryOwnership(ctx.fiber);
		this.runtime = { ctx };
	}
	async [Service.init]() {
		const ctx = this.runtime.ctx;
		const ownership = this.ownership;
		ctx.effect(() => () => ownership.dispose(), "agentLoop.transactions()");
		this.modelProjectionHandle = await ctx.sessionProjections.register(modelSelectionProjectionDefinition);
		this.inboxProjectionHandle = await ctx.sessionProjections.register(inboxProjectionDefinition);
		ctx.effect(() => ctx.agents.setFactory(this), "agentLoop.setFactory()");
		ctx.systemPrompt.variable("provider", (context) => context.modelSelection?.provider ?? context.agent?.options.provider);
		ctx.systemPrompt.variable("model", (context) => context.modelSelection?.model ?? context.agent?.options.model);
		for (const { id, sessionId, cwd, resumeSessionId, ...options } of this.config.agents) {
			const meta = cwd === void 0 ? {} : { cwd };
			if (resumeSessionId === void 0 || resumeSessionId === "") {
				const configuredId = sessionId ?? brandString(`${id}-session-${randomUUID()}`);
				const persistence = sessionId === void 0 ? void 0 : ctx.get("sessionPersistence");
				if (persistence === void 0) {
					const startup = this.create(configuredId, options, meta).then(() => void 0, (error) => {
						this.reportConfiguredStartupFailure(id, "restore", configuredId, error);
					});
					this.ownership.trackStartup(startup);
				} else {
					const startup = this.restoreOrCreateConfigured(ctx, persistence, configuredId, options, meta).catch((error) => {
						this.reportConfiguredStartupFailure(id, "restore", configuredId, error);
					});
					this.ownership.trackStartup(startup);
				}
				continue;
			}
			ctx.effect(() => {
				return ctx.inject(["sessionPersistence"], (childCtx) => {
					this.resumeWith(ctx, childCtx.sessionPersistence, {
						resumeSessionId,
						agentOptions: options,
						setup: (_agentCtx, agent) => {
							this.applyConfiguredRoute(agent, options);
						}
					}).catch((error) => {
						this.reportConfiguredStartupFailure(id, "resume", resumeSessionId, error);
					});
				}).dispose;
			}, `agentLoop.resume(${id})`);
		}
	}
	applyConfiguredRoute(agent, options) {
		const { provider, model, reasoningEffort } = options;
		if (!provider || !model) return;
		const previous = agent.model.current();
		const selection = {
			provider,
			model,
			...reasoningEffort === void 0 ? {} : { reasoningEffort }
		};
		if (selection.reasoningEffort === void 0 && previous !== void 0 && previous.provider === selection.provider && previous.model === selection.model && previous.reasoningEffort !== void 0) selection.reasoningEffort = previous.reasoningEffort;
		agent.model.select(selection);
	}
	/** Report a contained declarative-start failure to identity-bound consumers. */
	reportConfiguredStartupFailure(configId, action, sessionId, error) {
		if (!this.ownership.isActive()) return;
		this.ctx.logger.warn(`agent "${configId}": config-driven ${action} of "${sessionId}" failed: ${errorChain(error)}`);
		const args = ["agent-loop/config-start-failed", {
			sessionId,
			error
		}];
		for (const callback of this.ctx.events.dispatch("emit", args)) try {
			const returned = callback(...args);
			Promise.resolve(returned).catch((listenerError) => {
				this.ctx.logger.warn(`agent "${configId}": config-start-failed listener rejected: ${errorChain(listenerError)}`);
			});
		} catch (listenerError) {
			this.ctx.logger.warn(`agent "${configId}": config-start-failed listener threw: ${errorChain(listenerError)}`);
		}
	}
	/** Restore a materialized exact config identity on remount, or create it on first use. */
	async restoreOrCreateConfigured(ownerCtx, persistence, sessionId, agentOptions, meta) {
		await this.waitForDrainingConfiguredIdentity(ownerCtx, sessionId);
		if (!this.ownership.isActive()) return;
		try {
			await this.resumeWith(ownerCtx, persistence, {
				resumeSessionId: sessionId,
				agentOptions,
				setup: (_agentCtx, agent) => {
					this.applyConfiguredRoute(agent, agentOptions);
				}
			});
			return;
		} catch (error) {
			if (!this.ownership.isActive()) return;
			if (!(error instanceof SessionPersistenceNotFoundError)) throw error;
		}
		await this.create(sessionId, agentOptions, meta);
	}
	/** Wait for a draining same-id lifecycle to finish registry teardown. */
	async waitForDrainingConfiguredIdentity(ownerCtx, sessionId) {
		if (ownerCtx.agents.get(sessionId) === void 0 && ownerCtx.sessions.get(sessionId) === void 0) return;
		const released = Promise.withResolvers();
		const checkReleased = () => {
			if (ownerCtx.agents.get(sessionId) === void 0 && ownerCtx.sessions.get(sessionId) === void 0) released.resolve();
		};
		const disposeAgentListener = ownerCtx.on("agent/disposed", () => {
			checkReleased();
		});
		const disposeSessionListener = ownerCtx.on("session/disposed", checkReleased);
		try {
			checkReleased();
			await this.ownership.waitWhileActive(released.promise);
		} finally {
			disposeAgentListener();
			disposeSessionListener();
		}
	}
	/**
	* Own initialization and teardown from an acquired Session to a published Agent.
	* Registers factory teardown and caller cancellation before constructing the scope.
	* Failed initialization drains cleanup while retaining stored events and inbox input.
	* @param ownerCtx - caller context that owns the Agent lifecycle.
	* @param options - acquired Session/write handle, loop options, start source, parent Agent, setup, and cancellation.
	* @returns the initialized Agent and its shared disposer.
	*/
	async initializeAgent(ownerCtx, options) {
		const { session, agentOptions, source, stored, setup, signal: callerSignal, parentAgent } = options;
		const id = session.id;
		const loopCtx = this.runtime.ctx;
		const abort = new AbortController();
		const onCallerAbort = () => {
			abort.abort(callerSignal?.reason instanceof Error ? callerSignal.reason : new Error(`agent "${id}" creation aborted`, { cause: callerSignal?.reason }));
		};
		const onFactoryTeardown = () => {
			abort.abort(this.ownership.signal.reason);
		};
		callerSignal?.addEventListener("abort", onCallerAbort, { once: true });
		this.ownership.signal.addEventListener("abort", onFactoryTeardown, { once: true });
		let machine;
		let detachSession;
		let detachAgent;
		let disposing;
		let publication;
		let ready = false;
		const machineReady = Promise.withResolvers();
		const teardown = () => disposing ??= (async () => {
			abort.abort(/* @__PURE__ */ new Error(this.ownership.isActive() ? `agent "${id}" lifecycle disposed` : "agent loop is not active"));
			callerSignal?.removeEventListener("abort", onCallerAbort);
			this.ownership.signal.removeEventListener("abort", onFactoryTeardown);
			const failures = [];
			try {
				if (publication !== void 0) await publication.promise;
				/* v8 ignore next -- Cordis effect teardown waits for synchronous setup before observing the machine slot. */
				if (machine === void 0) await machineReady.promise;
				/* v8 ignore next -- setup failure untracks this disposer before resolving without a machine. */
				if (machine !== void 0) {
					machine.cancel({ kind: "disposed" }, { keepInbox: !ready });
					await machine.whenIdle();
				}
			} catch (error) {
				failures.push(error);
			}
			try {
				await machine?.scope.dispose();
			} catch (error) {
				failures.push(error);
			}
			try {
				await stored?.handle.close();
			} catch (error) {
				failures.push(error);
			}
			try {
				detachAgent?.();
				detachSession?.();
			} finally {
				untrack();
			}
			throwCollectedFailures(failures, `agent "${id}" disposal failed`);
		})();
		let unfollowScope;
		let unfollowOwner;
		const dispose = async () => {
			try {
				await teardown();
			} finally {
				await Promise.all([unfollowOwner(), unfollowScope?.()]);
			}
		};
		const untrack = this.ownership.track(dispose);
		try {
			assertAgentOptions(agentOptions);
			ownerCtx.fiber.assertActive();
			/* v8 ignore next -- unreachable backstop for factory ownership */
			if (!this.ownership.isActive()) throw new Error("agent loop is not active");
			if (callerSignal?.aborted) throw callerSignal.reason instanceof Error ? callerSignal.reason : new Error(`agent "${id}" creation aborted`, { cause: callerSignal.reason });
			const lastTurn = loopCtx.agents.turnBoundaryOf(session).lastTurn;
			const projections = {
				modelSelection: this.modelProjectionHandle.readerFor(session),
				inbox: this.inboxProjectionHandle.readerFor(session)
			};
			unfollowOwner = ownerCtx.effect(function* () {
				const created = machine = new ReactLoopAgent(loopCtx, id, agentOptions, session, projections, lastTurn);
				machineReady.resolve();
				if (!INACTIVE_STATES.has(loopCtx.fiber.state)) unfollowScope = loopCtx.effect(function* () {
					yield created.scope.rawDispose;
					yield () => teardown();
				}, `agentLoop.scope(${id})`);
				yield async () => {
					abort.abort(/* @__PURE__ */ new Error(`agent "${id}" setup aborted: owner disposed during setup`));
					try {
						await teardown();
					} finally {
						await unfollowScope?.();
					}
				};
			}, `agentLoop.lifecycle(${id})`);
		} catch (error) {
			machineReady.resolve();
			untrack();
			callerSignal?.removeEventListener("abort", onCallerAbort);
			this.ownership.signal.removeEventListener("abort", onFactoryTeardown);
			await stored?.handle.close().catch((_closeError) => {});
			throw error;
		}
		const assertLive = () => {
			if (!abort.signal.aborted) return;
			/* v8 ignore next -- unreachable String() arm, see above */
			throw abort.signal.reason instanceof Error ? abort.signal.reason : new Error(String(abort.signal.reason));
		};
		try {
			/* v8 ignore next -- a synchronous effect exhausts the generator before returning */
			if (machine === void 0) throw new Error(`agent "${id}" lifecycle did not construct its driver`);
			const agent = machine;
			assertLive();
			return await agent.runMaintenance(async () => {
				publication = Promise.withResolvers();
				try {
					assertLive();
					await this.appendUnstoredSuffix(stored, session);
					assertLive();
					detachSession = agent.ctx.sessions.enter(session);
					agent.ctx.sessions.announce(session);
					assertLive();
					const setupCommit = await raceAbort(setup?.(agent.ctx, agent), abort.signal, id);
					assertLive();
					setupCommit?.commit();
					assertLive();
					detachAgent = loopCtx.agents.enter(agent, parentAgent);
					await loopCtx.agents.announce(agent, source, abort.signal);
					assertLive();
					ready = true;
					return {
						agent,
						dispose
					};
				} catch (error) {
					agent.cancel({ kind: "disposed" }, { keepInbox: true });
					throw error;
				} finally {
					publication.resolve();
					publication = void 0;
				}
			});
		} catch (error) {
			await dispose().catch((_disposeError) => {});
			throw error;
		}
	}
	/**
	* Create an agent and session under one caller-supplied identity, owned by
	* the accessing fiber. Constructor-driven config calls mint a fresh combined
	* id before entering this boundary. When a persistence backend is mounted,
	* the session's durable identity and any seed are stored before publication.
	* @param id - shared agent/session identity.
	* @param options - concrete loop options.
	* @param meta - optional fresh-session workspace metadata.
	* @returns the published running agent.
	*/
	async create(id, options = {}, meta = {}) {
		const env_1 = {
			stack: [],
			error: void 0,
			hasError: false
		};
		try {
			const preparation = __addDisposableResource(env_1, SessionPreparation.create(this.runtime.ctx.sessions.prepare(id, { meta })), false);
			const stored = await this.createStoredSession(preparation.session);
			return (await this.initializeAgent(this.ctx, {
				session: preparation.session,
				stored,
				agentOptions: options,
				source: "startup"
			})).agent;
		} catch (e_1) {
			env_1.error = e_1;
			env_1.hasError = true;
		} finally {
			__disposeResources(env_1);
		}
	}
	/**
	* Take a fresh session's write ownership when persistence is mounted.
	* The constructor seed is stored before Session store entry. Setup then
	* appends through the live event feed; creation failure retains any events
	* already written to the handle.
	* @param session - the unpublished session to store.
	* @param signal - optional cancellation forwarded to the backend create.
	* @returns the owned handle and stored cursor, or `undefined` without a backend.
	*/
	async createStoredSession(session, signal) {
		const persistence = this.runtime.ctx.get("sessionPersistence");
		if (persistence === void 0) return void 0;
		return {
			handle: await persistence.create(session.header, {
				inheritedEventCount: session.inheritedEventCount,
				...signal === void 0 ? {} : { signal }
			}),
			storedCount: 0
		};
	}
	/**
	* Durably store the session events appended since the last stored cursor.
	* @param stored - the session's owned handle and stored cursor, if any.
	* @param session - the unpublished session whose suffix is stored.
	*/
	async appendUnstoredSuffix(stored, session) {
		if (stored === void 0) return;
		const suffix = session.snapshotEvents(SessionLogOffset(stored.storedCount));
		if (suffix.length > 0) await stored.handle.append(suffix);
		stored.storedCount += suffix.length;
	}
	/**
	* Create an owned agent on a caller-supplied session id.
	* @param ownerCtx - caller context that structurally owns the lifecycle.
	* @param options - identities, optional live parent, session seed/metadata, loop options, setup, and cancellation.
	* @returns the published handle.
	*/
	async createAgent(ownerCtx, options) {
		const preparation = SessionPreparation.create(this.runtime.ctx.sessions.prepare(options.sessionId, {
			...options.seed === void 0 ? {} : { seed: options.seed },
			...options.meta === void 0 ? {} : { meta: options.meta },
			...options.inheritedEventCount === void 0 ? {} : { inheritedEventCount: options.inheritedEventCount }
		}));
		const published = (async () => {
			const env_2 = {
				stack: [],
				error: void 0,
				hasError: false
			};
			try {
				const ownedPreparation = __addDisposableResource(env_2, preparation, false);
				const stored = options.signal === void 0 ? await this.createStoredSession(ownedPreparation.session) : await raceAbortCall(() => this.createStoredSession(ownedPreparation.session, options.signal), options.signal, options.sessionId, (abandoned) => {
					abandoned?.handle.close().catch(() => {});
				});
				return await this.initializeAgent(ownerCtx, {
					session: ownedPreparation.session,
					stored,
					agentOptions: options.agentOptions ?? {},
					setup: options.setup,
					signal: options.signal,
					source: "startup",
					parentAgent: options.parentAgent
				});
			} catch (e_2) {
				env_2.error = e_2;
				env_2.hasError = true;
			} finally {
				__disposeResources(env_2);
			}
		})();
		this.ownership.trackWrapper(published);
		return published;
	}
	/**
	* Resume an owned agent from the configured persistence service.
	* @param ownerCtx - caller context that owns load, setup, and the live lifecycle.
	* @param options - persisted identity, optional live parent, loop options, setup, and cancellation.
	* @returns the published handle.
	*/
	async resume(ownerCtx, options) {
		const persistence = this.runtime.ctx.get("sessionPersistence");
		if (persistence === void 0) throw new Error("cannot resume: session persistence is not configured (load a dsh-session-persistence backend)");
		return this.resumeWith(ownerCtx, persistence, options);
	}
	/** Resume through an explicit persistence handle used by the deferred config path. */
	resumeWith(ownerCtx, persistence, options) {
		const id = options.resumeSessionId;
		const published = (async () => {
			const ownerAbort = new AbortController();
			const unfollowOwner = ownerCtx.effect(() => () => {
				ownerAbort.abort(/* @__PURE__ */ new Error(`agent "${id}" setup aborted: owner disposed during setup`));
			}, `agentLoop.resume-load(${id})`);
			const fused = AbortSignal.any([
				...options.signal === void 0 ? [] : [options.signal],
				ownerAbort.signal,
				this.ownership.signal
			]);
			let handle;
			let stored;
			let preparation;
			try {
				try {
					handle = await raceAbortCall(() => persistence.open(id, "write", { signal: fused }), fused, id, (abandoned) => {
						abandoned.close();
					});
					const coldRead = await handle.read(0, void 0, { signal: fused });
					fused.throwIfAborted();
					const persisted = coldRead.events;
					const closers = interruptedTurnClosers(persisted);
					if (closers.length > 0) await handle.append(closers);
					preparation = SessionPreparation.create(this.runtime.ctx.sessions.prepare(id, {
						seed: [...persisted, ...closers],
						meta: structuredClone(handle.header),
						inheritedEventCount: handle.inheritedEventCount,
						eventState: coldRead.eventState
					}));
					stored = {
						handle,
						storedCount: persisted.length + closers.length
					};
					await this.appendUnstoredSuffix(stored, preparation.session);
				} finally {
					await unfollowOwner();
				}
				ownerCtx.fiber.assertActive();
				if (!this.ownership.isActive()) throw new Error("agent loop is not active");
				const owned = stored;
				handle = void 0;
				return await this.initializeAgent(ownerCtx, {
					session: preparation.session,
					stored: owned,
					agentOptions: options.agentOptions ?? {},
					setup: options.setup,
					signal: options.signal,
					source: "resume",
					parentAgent: options.parentAgent
				});
			} finally {
				preparation?.[Symbol.dispose]();
				await handle?.close().catch(() => {});
			}
		})();
		this.ownership.trackWrapper(published);
		return published;
	}
};
//#endregion
export { AgentLoop, AgentLoop as default, CONFIGURED_AGENT_IDENTITIES_KEY, DEFAULT_MAX_PARALLEL_TOOL_CALLS };
