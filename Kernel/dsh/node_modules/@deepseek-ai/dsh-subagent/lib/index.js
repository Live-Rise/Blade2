import { Service } from "@deepseek-ai/cordis";
import z from "@deepseek-ai/schemastery";
import { brandString } from "@deepseek-ai/dsh-brand";
import { scopeTarget } from "@deepseek-ai/dsh-scope";
import { ToolArgsError, assertObjectJsonSchema, validateJsonSchemaValue } from "@deepseek-ai/dsh-tools";
import { canonicalClientTimeZone } from "@deepseek-ai/dsh-util-time";
import { Remote, RemoteError, TypertRemoteService, principalOf } from "@deepseek-ai/dsh-typert-protocol";
import { AttachmentError } from "@deepseek-ai/dsh-attachment";
import { z as z$1 } from "zod";
import { HarnessError, ReasoningEffortId, boundContextSummary, contentHasImage, createUserMessage, errorChain, joinAssistantStreamText } from "@deepseek-ai/dsh-llm";
import { foldConsumedWork } from "@deepseek-ai/dsh-agent";
import { randomUUID } from "node:crypto";
import { isAbsolute, resolve } from "node:path";
import { SessionLogOffset, SessionSeq } from "@deepseek-ai/dsh-session";
import { assertNever, snapshotJsonValue } from "@deepseek-ai/dsh-util-values";
import { appendChunkedList, chunkedListSchema, iterateChunkedList } from "@deepseek-ai/dsh-chunked-list";
import { accessSync, constants, statSync } from "node:fs";
//#region lib/types/error.js
/**
* Typed failures shared by subagent service and provider operations.
*
* @module @deepseek-ai/dsh-subagent
*/
/** Typed failure for the subagent seam. */
var SubagentError = class extends HarnessError {
	constructor(message, code, options) {
		super(message, code, options);
		this.name = "SubagentError";
	}
};
//#endregion
//#region lib/types/control.js
/**
* Browser-facing subagent prompt and interrupt request validation plus the
* stable prompt failure codes returned by the Remote surface.
*
* @module @deepseek-ai/dsh-subagent
*/
const SESSION_ID_SCHEMA = z$1.string().min(1);
const CONTROL_ID_SCHEMAS = {
	"subagent.prompt": z$1.object({
		childSessionId: SESSION_ID_SCHEMA,
		delivery: z$1.enum(["queue", "steer"])
	}),
	"subagent.interrupt": z$1.object({ childSessionId: SESSION_ID_SCHEMA })
};
/**
* Apply the subagent payload checks that are stricter than generated
* branded-string codecs.
* @param method - method name carried in the failure message.
* @param payload - decoded control fields to validate.
* @throws {RemoteError} `gateway/bad-request` with the original Zod issues.
*/
function validateControlRequest(method, payload) {
	const parsed = CONTROL_ID_SCHEMAS[method].safeParse(payload);
	if (!parsed.success) throw new RemoteError("gateway/bad-request", `invalid payload for ${method}`, { issues: parsed.error.issues });
}
/**
* Refuse one continuation prompt without exposing provider detail: admission
* failures the caller can act on keep their own code, everything else is
* internal.
* @param error - the thrown value.
* @param childSessionId - the addressed child.
* @param signal - the caller's cancellation.
* @returns Never — the refusal is thrown.
* @throws {RemoteError} always.
*/
function rejectPrompt(error, childSessionId, signal) {
	if (isCancellation(error, signal)) throw new RemoteError("gateway/cancelled", "subagent prompt was cancelled", {}, { cause: error });
	if (error instanceof AttachmentError) throw new RemoteError("subagent/attachment-invalid", error.message, { reason: error.code }, { cause: error });
	if (error instanceof SubagentError) switch (error.code) {
		case "MODEL_DOES_NOT_SUPPORT_IMAGES": throw new RemoteError("subagent/attachment-invalid", error.message, { reason: error.code }, { cause: error });
		case "NOT_RESUMABLE": throw new RemoteError("subagent/not-resumable", "subagent cannot be resumed", { childSessionId }, { cause: error });
		case "UNAUTHORIZED": throw new RemoteError("subagent/unauthorized", "subagent membership in the active tree could not be verified", { childSessionId }, { cause: error });
		case "DRAINING":
		case "ACTIVATION_CLOSING":
		case "ACTIVATION_LIMIT_REACHED":
		case "CONTINUATION_UNAVAILABLE":
		case "PERSISTENCE_UNAVAILABLE": throw new RemoteError("subagent/delivery-unavailable", "subagent follow-up is temporarily unavailable", { childSessionId }, { cause: error });
		default: break;
	}
	throw new RemoteError("gateway/internal", "subagent prompt failed", {}, { cause: error });
}
function isCancellation(error, signal) {
	return signal.aborted || error instanceof SubagentError && error.code === "CANCELLED";
}
//#endregion
//#region lib/types/depth.js
/**
* Delegation-depth accounting: the recursion budget a parent passes to its
* children. Kept apart from the service so composition helpers can read it
* without importing the registry.
*
* @module @deepseek-ai/dsh-subagent/depth
*/
/**
* Read an agent's delegation depth, treating absence as top-level depth zero.
* The persisted session header is authoritative and monotone: runtime
* `AgentOptions.subagentDepth` may DEEPEN the count but can never lower it —
* a resumed child arrives with fresh options, and counting it from zero would
* let it delegate as if it were top-level.
* @param agent - the agent whose header and options carry the depth.
* @returns its non-negative safe-integer depth.
* @throws if the runtime `AgentOptions.subagentDepth` is not a non-negative safe integer.
*/
function delegationDepthOf(agent) {
	const runtime = agent.options.subagentDepth;
	if (runtime !== void 0 && (!Number.isSafeInteger(runtime) || runtime < 0 || Object.is(runtime, -0))) throw new TypeError("agent subagentDepth must be a non-negative safe integer");
	return Math.max(agent.session.header.delegationDepth ?? 0, runtime ?? 0);
}
/**
* Reject a recursion cap that cannot represent an exact delegation depth.
* @param maxDepth - the optional runtime value to validate.
*/
function assertSubagentMaxDepth(maxDepth) {
	if (maxDepth !== void 0 && (typeof maxDepth !== "number" || !Number.isSafeInteger(maxDepth) || maxDepth < 0 || Object.is(maxDepth, -0))) throw new TypeError("subagent maxDepth must be a non-negative safe integer");
}
//#endregion
//#region lib/types/lifecycle.js
/**
* Contained lifecycle notifications for subagents.
*
* The service owns scoped dispatch and listener containment; the manager
* supplies event payloads from each activation.
*
* @module @deepseek-ai/dsh-subagent/lifecycle
*/
/**
* Build the contained lifecycle emitter this seam publishes every edge through.
* Every listener is independently contained: a synchronous throw or a rejected
* returned promise is logged without starving peer listeners, changing the run,
* or — for provider removal, which fires from a disposer — breaking teardown.
* @param ctx - the service's own context, owning dispatch and the logger.
* @param carrier - resolve the scoped dispatch carrier for one subagent tree root.
* @returns the emitter the manager and provider registry publish through.
*/
function createLifecycleEmitter(ctx, carrier) {
	return (name, info, parent) => {
		const dispatchArgs = parent === void 0 ? [name, info] : [
			carrier(parent),
			name,
			info
		];
		for (const callback of ctx.events.dispatch("emit", dispatchArgs)) try {
			const returned = callback(info);
			Promise.resolve(returned).catch((error) => {
				ctx.logger.warn(`subagent: ${name} listener rejected: ${renderThrown(error)}`);
			});
		} catch (error) {
			ctx.logger.warn(`subagent: ${name} listener threw: ${renderThrown(error)}`);
		}
	};
}
/**
* Why this child's epoch ended, for the terminal lifecycle edge and the
* manager's own parent delivery. The child's own log is authoritative:
* teardown succeeding says nothing about whether the model errored, hit its
* token ceiling, or was cancelled, so deriving the reason from disposal would
* report failed work as completed.
*
* {@link foldConsumedWork} supplies both halves the raw turn sequence cannot:
* which turn accounts for the work this epoch consumed, and whether accepted
* work was cancelled after it without any turn opening over it. A recorded
* failure still wins over a cancellation — stopping a child that had already
* failed does not turn its failure into a cancellation.
* @param events - this epoch's own event suffix.
* @returns its terminal stop reason; `completed` only for an epoch that both
*   closed cleanly and had nothing left to run.
*/
function epochStopReason(events) {
	const { end, droppedUnrun } = foldConsumedWork(events);
	switch (end?.data.reason.kind) {
		case "max-tokens": return "max-tokens";
		case "aborted":
		case "interrupted": return "aborted";
		case "error": return "error";
		case "blocked": return "refusal";
		case void 0:
		case "completed": return droppedUnrun ? "aborted" : "completed";
		/* v8 ignore next 4 -- `forked` appears only in constructor seed history, while
		* this function reads an epoch-owned suffix. `TurnEndReason` is merge-extensible,
		* so a backend-added variant cannot be listed; treating an unnameable reason as
		* success would report failed work as completed. */
		default: return "error";
	}
}
/** Render any listener-thrown value without letting coercion escape containment. */
function renderThrown(value) {
	try {
		return value instanceof Error ? `${value.name}: ${value.message}` : String(value);
	} catch {
		return "<unrenderable thrown value>";
	}
}
//#endregion
//#region lib/types/child-agent.js
/**
* Shared in-process child composition: the delegation-depth budget, the
* durable session metadata, the resolved child `AgentOptions`, the delegated
* policy seed, and the scoped setup used by local activations.
*
* @module @deepseek-ai/dsh-subagent/child-agent
*/
/** Thrown when starting a child would exceed the requested depth cap. */
var SubagentDepthError = class extends Error {
	attemptedDepth;
	maxDepth;
	constructor(attemptedDepth, maxDepth) {
		super(`subagent depth ${attemptedDepth} exceeds maxDepth ${maxDepth}`);
		this.attemptedDepth = attemptedDepth;
		this.maxDepth = maxDepth;
		this.name = "SubagentDepthError";
	}
};
/**
* Resolve the child's delegation depth from its parent and enforce an optional
* cap. The persisted parent header is the monotone floor, so a resumed parent
* cannot delegate as if it were top-level.
* @param parent - the delegating parent agent.
* @param maxDepth - optional absolute cap the resolved depth must not exceed.
* @returns the child's non-negative safe-integer depth.
* @throws {SubagentDepthError} when the resolved depth exceeds `maxDepth`.
* @throws {RangeError} when the resolved depth leaves the safe-integer range.
*/
function resolveChildDepth(parent, maxDepth) {
	const childDepth = delegationDepthOf(parent) + 1;
	if (!Number.isSafeInteger(childDepth)) throw new RangeError("subagent child depth exceeds the safe-integer range");
	if (maxDepth !== void 0 && childDepth > maxDepth) throw new SubagentDepthError(childDepth, maxDepth);
	return childDepth;
}
/**
* Resolve the parent values inherited by a child. The latest request header
* owns provider, model, and reasoning effort after request-time selection;
* creation options remain the fallback before the first request and retain
* the configured output-token limit.
* @param parent - delegating parent Agent.
* @returns detached Agent options for child-option merging.
*/
function parentAgentOptionsForDelegation(parent) {
	const requestConfig = parent.session.requestHeader()?.config;
	if (requestConfig === void 0) return { ...parent.options };
	const { provider: _createdProvider, model: _createdModel, reasoningEffort: _createdReasoningEffort, ...createdOptions } = parent.options;
	return {
		...createdOptions,
		provider: requestConfig.provider,
		model: requestConfig.model,
		...requestConfig.reasoningEffort === void 0 ? {} : { reasoningEffort: requestConfig.reasoningEffort }
	};
}
/**
* Resolve the child's `AgentOptions`: the parent's provider/model,
* reasoning-effort, and maxTokens values unless the request overrides them,
* stamped with the child's own delegation depth. Changing the route without
* naming an effort clears the parent's route-owned effort so the selected
* model resolves its own default.
* @param parent - the delegating parent whose route the child inherits.
* @param requested - per-child overrides, if any.
* @param childDepth - the resolved delegation depth to stamp.
* @returns the resolved options for `ctx.agents.create()`.
*/
function resolveChildAgentOptions(parent, requested, childDepth) {
	const parentOptions = parentAgentOptionsForDelegation(parent);
	const parentProvider = parentOptions.provider;
	const parentModel = parentOptions.model;
	const parentReasoningEffort = parentOptions.reasoningEffort;
	const parentMaxTokens = parentOptions.maxTokens;
	const resolved = {
		...parentProvider !== void 0 ? { provider: parentProvider } : {},
		...parentModel !== void 0 ? { model: parentModel } : {},
		...parentReasoningEffort !== void 0 ? { reasoningEffort: parentReasoningEffort } : {},
		...parentMaxTokens !== void 0 ? { maxTokens: parentMaxTokens } : {},
		...requested,
		subagentDepth: childDepth
	};
	if ((resolved.provider !== parentProvider || resolved.model !== parentModel) && requested?.reasoningEffort === void 0) delete resolved.reasoningEffort;
	return resolved;
}
/**
* Build the child session's durable creation metadata: the parent's workspace,
* its direct lineage, coarse product origin, the recursion budget that must
* survive persistence, the seed boundary that separates inherited parent
* history from child work, and the composition the child runs under.
*
* Omitted selection uses the parent's live composition, including any switch
* made while the parent was blank.
* @param parent - the delegating parent agent.
* @param childDepth - the resolved delegation depth to persist.
* @param isSeeded - whether this child inherits a parent-log prefix, including an explicitly empty one.
* @param selectedPreset - independent selection, or omission to inherit the parent.
* @returns the `meta` for `ctx.agents.create()`.
*/
function childSessionMeta(parent, childDepth, isSeeded, selectedPreset) {
	const parentHeader = parent.session.header;
	const agentPreset = selectedPreset ?? parent.ctx.get("agentPresets")?.composedPreset(parent.ctx);
	return {
		...parentHeader.cwd !== void 0 ? { cwd: parentHeader.cwd } : {},
		...agentPreset === void 0 ? {} : { agentPreset },
		parentSession: parentHeader.id,
		isSeeded,
		origin: "subagent",
		delegationDepth: childDepth
	};
}
/**
* Model-facing delegation-scope statement for every in-process child. A
* runtime-context contribution rather than a system-prompt section, so the
* deployment's system prompt stays uniform across parents and children.
*/
const SUBAGENT_DELEGATION_CONTEXT = "You are a delegated subagent: your permission scope was fixed when you were started and cannot be widened from inside this session — operations that require approval are rejected automatically. When the task needs access beyond that scope, do not retry the denied operation; state the limitation in your reply so the delegating agent can handle it.";
/**
* Compose a child before publication and restore historical descriptor overrides.
* Explicit selection mounts the named preset. Omission joins a supplied creator's
* current generation; a restored child without a saved preset uses host composition.
* Registrations belong to the child's scope.
* @param childCtx - the child agent's scoped creation context.
* @param parent - creator for fresh delegation; absent during target-only restoration.
* @param composition - independent preset and any overrides read from a historical descriptor.
* @returns A promise when mounting an independent preset; otherwise completes synchronously.
*/
function applyChildComposition(childCtx, parent, composition) {
	if (composition.agentPreset !== void 0) {
		const presets = childCtx.get("agentPresets");
		if (presets === void 0) throw new SubagentError("selecting a child agent preset requires the agent-presets service", "PRESET_UNAVAILABLE");
		return presets.mount(childCtx, composition.agentPreset).then(() => {
			applyChildOverrides(childCtx, composition);
		});
	}
	if (parent !== void 0) childCtx.get("agentPresets")?.composeFrom(childCtx, parent.ctx);
	applyChildOverrides(childCtx, composition);
}
function applyChildOverrides(childCtx, composition) {
	childCtx.systemPrompt.context({
		name: "subagent:delegation",
		order: childCtx.systemPrompt.getContextOrder("SUBAGENT_DELEGATION"),
		text: SUBAGENT_DELEGATION_CONTEXT
	});
	if (composition.persona !== void 0) childCtx.systemPrompt.section({
		name: "deployment:persona-prefix",
		order: childCtx.systemPrompt.getSectionOrder("DEPLOYMENT_PERSONA_PREFIX"),
		text: composition.persona
	});
	if (composition.toolFilter !== void 0) childCtx.tools.restrict(composition.toolFilter);
}
/**
* Capture the permission state to seed into one delegation. Call synchronously before
* the child start's first await: a later parent switch belongs to the
* parent's future, not to this child. Auto and Full access identities are
* inherited only through the in-process DSH path so either can replace a stale
* same-bundle fork value. Only the parent session's explicit sandbox override
* is captured — never deployment defaults or one-shot grants — and the approval
* policy is pinned to `'never'` regardless of the parent's own policy.
* @param parent - the delegating parent agent.
* @returns the sandbox override (or `undefined` without one) and the approval pin.
*/
function captureDelegatedPolicyOverrides(parent) {
	const preset = parent.ctx.get("permissionPresets")?.current(parent.session);
	return {
		permissionPreset: preset === "auto" || preset === "danger-full-access" ? preset : void 0,
		sandboxMode: parent.ctx.get("sandboxPolicy")?.overrideOf(parent.session),
		approvalPolicy: parent.ctx.get("approval") === void 0 ? void 0 : "never"
	};
}
/**
* Append the captured delegation policy onto the child's own log as
* `source: 'delegation'` events after Session announcement and before Agent registration, so the
* child's effective policy is reconstructable from its log alone. Appends land
* after any fork seed, so fresh policy wins stale seed state; later child
* switches still win over these events.
* @param childSession - the child's announced, writable Session.
* @param overrides - the policy captured at delegation.
*/
function appendDelegatedPolicyOverrides(childSession, overrides) {
	if (overrides.sandboxMode !== void 0) childSession.append("sandbox/mode", {
		mode: overrides.sandboxMode,
		source: "delegation"
	});
	if (overrides.approvalPolicy !== void 0) childSession.append("approval/policy", {
		policy: overrides.approvalPolicy,
		source: "delegation"
	});
	if (overrides.permissionPreset !== void 0) childSession.append("permission/preset", { preset: overrides.permissionPreset });
}
//#endregion
//#region lib/types/continuation-messages.js
/**
* Model-visible messages owned by continuable-subagent orchestration.
*
* @module @deepseek-ai/dsh-subagent/continuation-messages
*/
/** Build durable attribution for one Agent message. */
function agentMessageSource(sender) {
	return {
		kind: "agent-message",
		form: "relay",
		senderSessionId: sender.id
	};
}
/**
* Build the model-visible and durable representation of one Agent message.
* @param sender - exact live Agent that authored the message.
* @param senderName - the sender's model-facing name in its subagent tree.
* @param content - model-visible message blocks supplied by the sender.
* @returns the durable user-message representation delivered to the recipient.
*/
function createAgentMessage(sender, senderName, content) {
	return createUserMessage({
		content: [{
			type: "text",
			text: `Agent ${senderName} sent a message: `
		}, ...content],
		source: agentMessageSource(sender)
	});
}
/**
* Append automatic result-delivery guidance to a parent-delivered child's initial task.
* @param prompt - initial model-visible task blocks.
* @returns task blocks followed by the result-delivery guidance.
*/
function withResultDeliveryGuidance(prompt) {
	return [...prompt, {
		type: "text",
		text: "Your final answer is delivered automatically if your parent is still online. Include the result and relevant evidence in that answer; the parent does not receive your full transcript, tool output, or reasoning. If your last action is a message to your parent, the completion notice omits your final answer; include the result and relevant evidence in that message."
	}];
}
/**
* One line telling a parent that a background child is finished and why, in
* the parent's own task vocabulary.
* @param childName - the model-facing name the parent knows the child by.
* @param stopReason - how the child's last ordinary turn ended.
* @param continuable - whether the child accepts a later message.
* @returns the model-facing opening line of the settlement notice.
*/
function settlementSummary(childName, stopReason, continuable) {
	const subject = `Background subagent ${childName}`;
	switch (stopReason) {
		case "completed": return continuable ? `${subject} finished and will do no further work unless you send it more.` : `${subject} finished. It cannot receive follow-up messages.`;
		case "aborted": return `${subject} was stopped before it finished.`;
		case "max-tokens": return `${subject} ran out of room before it finished.`;
		case "refusal": return `${subject} declined the task.`;
		case "error": return `${subject} failed before it finished.`;
		/* v8 ignore next 4 -- `SubagentResult['stopReason']` is merge-extensible, so this arm
		* needs a backend that adds a variant; an unnameable ending is reported as unfinished
		* rather than silently as success. */
		default: return `${subject} ended abnormally (${String(stopReason)}) before it finished.`;
	}
}
/**
* Build the runtime-owned settlement notice from the child's nonempty closing text.
* @param childId - durable child session id recorded as the sender.
* @param childName - model-facing child name used in the notice text.
* @param terminal - recorded terminal state for the settled Activation.
* @param continuable - whether the child can accept another task.
* @param omitClosingText - replace the closing text when the child's last action
*   was a message to its direct parent.
* @returns the durable user-message representation delivered to the parent.
*/
function createSettlementMessage(childId, childName, terminal, continuable = true, omitClosingText = false) {
	const summary = settlementSummary(childName, terminal.stopReason, continuable);
	const closingText = terminal.output.flatMap((block) => block.type === "text" && block.text.length > 0 ? [block] : []);
	return createUserMessage({
		content: [
			{
				type: "text",
				text: summary
			},
			...omitClosingText ? [{
				type: "text",
				text: "It sent you a message just before finishing, so its closing message is omitted."
			}] : closingText.length === 0 ? [{
				type: "text",
				text: "It left no closing message."
			}] : [{
				type: "text",
				text: "Its closing message:"
			}, ...closingText],
			...terminal.structured !== void 0 ? [{
				type: "text",
				text: `Structured result: ${JSON.stringify(terminal.structured)}`
			}] : [],
			...terminal.diagnostic !== void 0 ? [{
				type: "text",
				text: terminal.diagnostic
			}] : []
		],
		source: {
			kind: "subagent-settled",
			form: "notice",
			summary: boundContextSummary(summary),
			stopReason: terminal.stopReason,
			senderSessionId: childId
		}
	});
}
//#endregion
//#region lib/types/archive-admission.js
/**
* The `subagent` family of the Workspace registry's archive admission: which
* subagent descendants of a Session are still inside a turn, and how they
* stop when the Session is archived with its work. Archived ancestors also
* block later model steps, including when their Sessions have been released.
*
* @module @deepseek-ai/dsh-subagent
*/
/**
* Answer `workspace/session-activity` with the running subagent descendants of
* the asked Session, and `workspace/session-stop` by cancelling each of them
* as their parent would. Reject subsequent steps beneath archived ancestors.
* All listeners live as long as `ctx`'s fiber.
* @param ctx - context carrying the Agent registry.
* @param residentAncestors - verified creation ancestors for an exact managed Agent, absent before publication or for unmanaged Agents.
* @param identityOf - ready service-owned descriptor identity reader; absent or invalid descriptors yield no identity.
*/
function installSubagentArchiveAdmission(ctx, residentAncestors, identityOf) {
	const ancestorsOf = (agent) => residentAncestors(agent) ?? headerAncestors(ctx, agent);
	const runningDescendants = (sessionId) => ctx.agents.list().filter((agent) => agent.status === "running" && ancestorsOf(agent).includes(sessionId));
	ctx.on("workspace/session-activity", async ({ sessionId }, next) => {
		const running = runningDescendants(sessionId);
		const rest = await next();
		if (running.length === 0) return rest;
		return [{
			kind: "subagent",
			items: running.map((child) => describe(child, identityOf(child.session)))
		}, ...rest];
	});
	ctx.on("workspace/session-stop", ({ sessionId }) => {
		for (const child of runningDescendants(sessionId)) try {
			child.cancel({ kind: "parent" });
		} catch (error) {
			ctx.logger.warn(`subagent: cancelling "${child.id}" for an archived Session failed: ${String(error)}`);
		}
	});
	ctx.on("agent/pre-step", ({ agent }, next) => {
		const archived = ctx.get("workspaceRegistry")?.archivedSessionIds;
		return archived !== void 0 && ancestorsOf(agent).some((id) => archived.includes(id)) ? Promise.resolve({ kind: "reject" }) : next();
	});
}
/**
* Header-only ancestry for Agents without a published Activation. Include an
* offline direct creator; ordinary forks stop the chain and cycles terminate.
*/
function headerAncestors(ctx, agent) {
	const ancestors = [];
	const visited = new Set([agent.id]);
	let header = agent.session.header;
	while (header.origin === "subagent" && header.parentSession !== void 0 && !visited.has(header.parentSession)) {
		const parentId = header.parentSession;
		ancestors.push(parentId);
		visited.add(parentId);
		const parent = ctx.agents.get(parentId)?.session;
		if (parent === void 0) break;
		header = parent.header;
	}
	return ancestors;
}
/**
* Report the child's current projected label only when its descriptor belongs
* to the child's own log; inherited, absent, or invalid identities yield the id alone.
*/
function describe(child, identity) {
	const label = identity !== void 0 && child.session.isOwnSeq(identity.seq) ? identity.label : void 0;
	return label === void 0 ? { id: child.id } : {
		id: child.id,
		label
	};
}
//#endregion
//#region lib/types/descriptor.js
/**
* The durable subagent-child descriptor: the versioned, model-hidden
* `subagent/descriptor` session event that identifies every session-backed
* subagent, including historical one-shot records. The manager appends each
* continuable child's descriptor inside its initial turn, preserving the
* declared composition required for cold resume.
*
* The descriptor deliberately snapshots explicit fields rather than the
* merge-extensible `AgentOptions` object: an unrelated extension value cannot
* make continuation fail merely because it is not JSON, and later composition
* inputs require a deliberate {@link SUBAGENT_DESCRIPTOR_VERSION} change. It
* omits `subagentDepth` — cold resume trusts the persisted header's
* `delegationDepth` as the monotone floor — and `outputSchema`, which belongs
* to one activation's result contract rather than durable child composition.
* Per-activation knobs such as `maxTokens` are omitted for the same reason as
* `outputSchema`: they budget one activation. Cold resume requires the exact
* live subagent tree root for authorization but reconstructs child options only from the
* durable descriptor, so it neither restores the prior budget nor inherits
* the parent's current one; the resumed route's defaults apply instead.
*
* @module @deepseek-ai/dsh-subagent/descriptor
*/
/**
* The current descriptor format version, stamped into every appended
* `subagent/descriptor` event and required verbatim by {@link foldSubagentDescriptor}.
* Supporting another composition input is a deliberate version change, never
* an implicit extra field.
*/
const SUBAGENT_DESCRIPTOR_VERSION = 3;
const DESCRIPTOR_BASE_KEYS = [
	"version",
	"mode",
	"provider",
	"label"
];
const ONE_SHOT_DESCRIPTOR_KEYS = new Set(DESCRIPTOR_BASE_KEYS);
const CONTINUABLE_DESCRIPTOR_KEYS = new Set([
	...DESCRIPTOR_BASE_KEYS,
	"agentProvider",
	"agentModel",
	"agentReasoningEffort",
	"persona",
	"toolFilter"
]);
const TOOL_FILTER_KEYS = new Set(["allow", "deny"]);
/** Whether a persisted JSON value is an object record. */
function isRecord(value) {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
/** Reject fields outside one versioned record's declared schema. */
function assertKnownKeys(value, keys, path) {
	const unknown = Object.keys(value).find((key) => !keys.has(key));
	if (unknown !== void 0) throw new Error(`persisted subagent descriptor ${path} has unknown field "${unknown}"`);
}
/** Read one optional string field from a persisted descriptor record. */
function optionalString(value, key) {
	if (!Object.hasOwn(value, key)) return void 0;
	const field = value[key];
	if (typeof field !== "string") throw new Error(`persisted subagent descriptor ${key} must be a string`);
	return field;
}
/** Read one optional string-array field from a persisted tool restriction. */
function optionalStringArray(value, key) {
	if (!Object.hasOwn(value, key)) return void 0;
	const field = value[key];
	if (!Array.isArray(field)) throw new Error(`persisted subagent descriptor toolFilter.${key} must be an array of strings`);
	const items = field;
	if (items.some((item) => typeof item !== "string")) throw new Error(`persisted subagent descriptor toolFilter.${key} must be an array of strings`);
	return items;
}
/** Validate and reconstruct a persisted tool restriction. */
function parseToolFilter(value) {
	if (!isRecord(value)) throw new Error("persisted subagent descriptor toolFilter must be an object");
	assertKnownKeys(value, TOOL_FILTER_KEYS, "toolFilter");
	const allow = optionalStringArray(value, "allow");
	const deny = optionalStringArray(value, "deny");
	if (allow === void 0 && deny === void 0) throw new Error("persisted subagent descriptor toolFilter must declare allow and/or deny");
	return {
		...allow !== void 0 ? { allow } : {},
		...deny !== void 0 ? { deny } : {}
	};
}
/** Validate one persisted descriptor payload for the current runtime. */
function parseSubagentDescriptor(value) {
	if (!isRecord(value)) throw new Error("persisted subagent descriptor payload must be an object");
	const version = value["version"];
	if (typeof version !== "number") throw new Error("persisted subagent descriptor version must be a number");
	if (version !== 3) return void 0;
	const mode = value["mode"];
	if (mode !== "one-shot" && mode !== "continuable") throw new Error("persisted subagent descriptor mode must be \"one-shot\" or \"continuable\"");
	assertKnownKeys(value, mode === "one-shot" ? ONE_SHOT_DESCRIPTOR_KEYS : CONTINUABLE_DESCRIPTOR_KEYS, "payload");
	const provider = value["provider"];
	if (typeof provider !== "string") throw new Error("persisted subagent descriptor provider must be a string");
	if (mode === "one-shot") {
		const label = optionalString(value, "label");
		return {
			version: 3,
			mode,
			provider,
			...label !== void 0 ? { label } : {}
		};
	}
	const label = value["label"];
	if (typeof label !== "string") throw new Error("persisted subagent descriptor label must be a string");
	const agentProvider = optionalString(value, "agentProvider");
	const agentModel = optionalString(value, "agentModel");
	const agentReasoningEffort = optionalString(value, "agentReasoningEffort");
	const persona = optionalString(value, "persona");
	const toolFilter = Object.hasOwn(value, "toolFilter") ? parseToolFilter(value["toolFilter"]) : void 0;
	return {
		version: 3,
		mode,
		provider,
		label,
		...agentProvider !== void 0 ? { agentProvider } : {},
		...agentModel !== void 0 ? { agentModel } : {},
		...agentReasoningEffort !== void 0 ? { agentReasoningEffort } : {},
		...persona !== void 0 ? { persona } : {},
		...toolFilter !== void 0 ? { toolFilter } : {}
	};
}
/**
* Validate and detach a continuable descriptor input.
* @param input - the caller-collected continuable composition fields.
* @returns the versioned, detached continuable descriptor payload.
* @throws when a field is not losslessly JSON-serializable.
*/
function snapshotSubagentDescriptor(input) {
	const snapshot = snapshotJsonValue({
		version: 3,
		mode: input.mode,
		provider: input.provider,
		label: input.label,
		...input.agentProvider !== void 0 ? { agentProvider: input.agentProvider } : {},
		...input.agentModel !== void 0 ? { agentModel: input.agentModel } : {},
		...input.agentReasoningEffort !== void 0 ? { agentReasoningEffort: input.agentReasoningEffort } : {},
		...input.persona !== void 0 ? { persona: input.persona } : {},
		...input.toolFilter !== void 0 ? { toolFilter: input.toolFilter } : {}
	});
	if (snapshot === void 0) throw new Error("subagent descriptor is not losslessly JSON-serializable");
	return snapshot;
}
/**
* Fold a persisted child log to its supported descriptor. The first
* `subagent/descriptor` event is authoritative — the establishing provider
* appends exactly one, so a later same-type event cannot rewrite the declared
* composition.
* @param events - the loaded child session events.
* @returns the descriptor, or `undefined` when the log has none or its
*   version is not {@link SUBAGENT_DESCRIPTOR_VERSION} (the child cannot be
*   classified by this runtime).
* @throws when a current-version persisted payload does not match its complete
*   declared schema.
*/
function foldSubagentDescriptor(events) {
	const event = events.find((candidate) => candidate.type === "subagent/descriptor");
	if (event === void 0) return void 0;
	return parseSubagentDescriptor(event.data);
}
//#endregion
//#region lib/types/names.js
/**
* Model-facing agent names. Each child's name is unique among its creator's
* children and is recorded in the creator's catalog; a member's full name is
* the slash-separated path of names from the root, whose own name is `lead`.
*
* @module @deepseek-ai/dsh-subagent/names
*/
/** The root agent's fixed name; no child may take it. */
const ROOT_AGENT_NAME = "lead";
/** One lower-case segment of at most 32 characters, so a 36-character UUID never matches. */
const NAME_PATTERN = /^[a-z][a-z0-9-]{0,31}$/u;
/**
* Whether a string is a valid child name.
* @param value - candidate name segment.
* @returns true for 1–32 lower-case letters, digits, or hyphens starting with a letter, other than `lead`.
*/
function isAgentName(value) {
	return NAME_PATTERN.test(value) && value !== "lead";
}
/**
* Append a child's name to its creator's path. The root's path is the empty string.
* @param parentPath - the creator's path, or undefined when an ancestor below the root has no name.
* @param name - the child's catalog name, absent on historical entries.
* @returns the child's path, or undefined when any link is unnamed.
*/
function joinAgentPath(parentPath, name) {
	if (parentPath === void 0 || name === void 0) return void 0;
	return parentPath === "" ? name : `${parentPath}/${name}`;
}
/**
* Select the model-facing name of one tree member.
* @param path - the member's path; `''` for the root and undefined for unnamed history.
* @param id - the member's Session id, used when no complete path exists.
* @returns `lead`, the full path, or the Session id.
*/
function agentAddress(path, id) {
	if (path === "") return ROOT_AGENT_NAME;
	return path ?? id;
}
/**
* Remove the last name from a descendant path.
* @param path - a non-empty descendant path.
* @returns the creator's path; `''` when the creator is the root.
*/
function parentAgentPath(path) {
	const separator = path.lastIndexOf("/");
	return separator < 0 ? "" : path.slice(0, separator);
}
/** Names claimed by in-flight creations, keyed by the exact live creator. */
var SiblingNameReservations = class {
	pending = /* @__PURE__ */ new Map();
	/**
	* Claim a name synchronously before startup awaits, so concurrent siblings cannot share it.
	* @param parent - exact live creator whose catalog receives the name.
	* @param committed - names already recorded in the creator's catalog.
	* @param requested - requested name; omission selects the first free `agent-N`.
	* @returns the claimed name and its release.
	* @throws {@link SubagentError} `INVALID_AGENT_NAME` or `DUPLICATE_AGENT_NAME`.
	*/
	reserve(parent, committed, requested) {
		const pending = this.pending.get(parent) ?? /* @__PURE__ */ new Set();
		const taken = (name) => committed.has(name) || pending.has(name);
		let name;
		if (requested === void 0) {
			let ordinal = 1;
			while (taken(`agent-${ordinal}`)) ordinal++;
			name = `agent-${ordinal}`;
		} else {
			if (!isAgentName(requested)) throw new SubagentError(`agent name ${JSON.stringify(requested)} is invalid; use 1-32 lower-case letters, digits, or hyphens starting with a letter, other than "${ROOT_AGENT_NAME}"`, "INVALID_AGENT_NAME");
			if (taken(requested)) throw new SubagentError(`agent name ${JSON.stringify(requested)} is already used by another subagent you created; choose another name`, "DUPLICATE_AGENT_NAME");
			name = requested;
		}
		pending.add(name);
		this.pending.set(parent, pending);
		return {
			name,
			release: () => {
				pending.delete(name);
				if (pending.size === 0 && this.pending.get(parent) === pending) this.pending.delete(parent);
			}
		};
	}
};
const sessionIdSchema = z$1.string();
const oneShotCatalogSchema = z$1.object({
	version: z$1.union([z$1.literal(0), z$1.literal(1)]),
	childId: sessionIdSchema,
	childCreatedAt: z$1.number().int().nonnegative(),
	mode: z$1.literal("one-shot"),
	label: z$1.string().optional()
}).strict();
const continuableCatalogSchema = z$1.object({
	version: z$1.union([z$1.literal(0), z$1.literal(1)]),
	childId: sessionIdSchema,
	childCreatedAt: z$1.number().int().nonnegative(),
	mode: z$1.literal("continuable"),
	label: z$1.string()
}).strict();
const externalCatalogSchema = oneShotCatalogSchema.extend({
	version: z$1.literal(2),
	mode: z$1.literal("external")
});
const unknownCatalogSchema = oneShotCatalogSchema.extend({
	version: z$1.literal(1),
	mode: z$1.literal("unknown")
});
const nameSchema = z$1.string().refine(isAgentName, "invalid agent name");
const namedContinuableCatalogSchema = continuableCatalogSchema.extend({
	version: z$1.literal(3),
	name: nameSchema
});
const namedExternalCatalogSchema = externalCatalogSchema.extend({
	version: z$1.literal(3),
	name: nameSchema
});
const eventDataSchema = z$1.union([
	oneShotCatalogSchema,
	externalCatalogSchema,
	continuableCatalogSchema,
	unknownCatalogSchema,
	namedContinuableCatalogSchema,
	namedExternalCatalogSchema
]);
const viewSchema = z$1.array(z$1.union([
	oneShotCatalogSchema.omit({
		version: true,
		childId: true,
		childCreatedAt: true
	}).extend({
		id: sessionIdSchema,
		createdAt: oneShotCatalogSchema.shape.childCreatedAt
	}),
	externalCatalogSchema.omit({
		version: true,
		childId: true,
		childCreatedAt: true
	}).extend({
		id: sessionIdSchema,
		createdAt: externalCatalogSchema.shape.childCreatedAt,
		name: nameSchema.optional()
	}),
	continuableCatalogSchema.omit({
		version: true,
		childId: true,
		childCreatedAt: true
	}).extend({
		id: sessionIdSchema,
		createdAt: continuableCatalogSchema.shape.childCreatedAt,
		name: nameSchema.optional()
	}),
	unknownCatalogSchema.omit({
		version: true,
		childId: true,
		childCreatedAt: true
	}).extend({
		id: sessionIdSchema,
		createdAt: unknownCatalogSchema.shape.childCreatedAt
	})
]));
const stateSchema = z$1.object({
	inheritedEventCount: z$1.number().int().nonnegative(),
	head: chunkedListSchema(eventDataSchema).optional()
}).strict();
/**
* Materialize complete and unknown-mode child identities from parent catalog events.
* @param state - parent catalog fold state.
* @returns current direct-child rows in parent catalog event order.
*/
function subagentCatalogEntries(state) {
	const entries = [];
	for (const data of iterateChunkedList(state.head)) {
		const { version: _version, childId, childCreatedAt, ...descriptor } = data;
		entries.push({
			id: childId,
			createdAt: childCreatedAt,
			...descriptor
		});
	}
	return entries;
}
/** Parent-owned catalog; version-6 checkpoints retain unnamed facts, and invalid own facts reject restoration. */
const subagentCatalogProjectionDefinition = {
	key: "subagentCatalog",
	stateSchema,
	init: (_header, inheritedEventCount) => ({ inheritedEventCount }),
	apply: (state, event) => {
		if (event.type !== "subagent/catalog" || event.seq < state.inheritedEventCount) return state;
		return {
			...state,
			head: appendChunkedList(state.head, eventDataSchema.parse(event.data))
		};
	},
	stateVersion: 6,
	wire: {
		viewSchema,
		view: subagentCatalogEntries
	}
};
/**
* Append a complete direct-child discovery fact to its parent Session.
* @param parent - durable direct parent receiving the discovery fact.
* @param child - established child's immutable Session metadata.
* @param descriptor - mode-discriminated creation label frozen with the child.
* @param name - the child's name, reserved unique among the parent's children.
*/
function establishCatalogChild(parent, child, descriptor, name) {
	parent.append("subagent/catalog", {
		version: 3,
		childId: child.id,
		childCreatedAt: child.createdAt,
		mode: descriptor.mode,
		label: descriptor.label,
		name
	});
}
/**
* Record an external execution without a local child Session.
* @param parent - durable direct parent receiving the discovery fact.
* @param childId - identity returned by the external provider.
* @param label - display label assigned by the caller.
* @param name - the child's name, reserved unique among the parent's children.
*/
function establishExternalCatalogChild(parent, childId, label, name) {
	parent.append("subagent/catalog", {
		version: 3,
		childId,
		childCreatedAt: Date.now(),
		mode: "external",
		label,
		name
	});
}
/**
* Collect the names recorded in one parent's catalog.
* @param entries - the parent's current catalog rows.
* @returns names of every named child; historical entries contribute none.
*/
function catalogNames(entries) {
	const names = /* @__PURE__ */ new Set();
	for (const entry of entries) if ("name" in entry) names.add(entry.name);
	return names;
}
//#endregion
//#region lib/types/types.js
/**
* The seam's consumer-facing contracts: request, result, and capability types
* for {@link SubagentProvider}, plus the `subagent/start` and `subagent/end`
* payloads that plugins and hosts observe. Internal control interfaces, such
* as the manager host, belong with their implementation.
*
* @module @deepseek-ai/dsh-subagent/types
*/
/**
* Brand a string as a {@link SubagentRunId}.
* @param id - the raw run id.
* @returns the same string, branded.
*/
function SubagentRunId(id) {
	return id;
}
//#endregion
//#region lib/types/structured.js
/**
* Child-scoped structured-output tool, prompt instruction, terminal guard, and authoritative
* result capture for in-process subagents. Each child registers its real schema on its own
* scope, so concurrent runs do not interact and disposal leaves no global residue. The prompt
* contribution is ordinary reconstructed request state.
*
* Capture commits only after the authoritative `tools/result` succeeds; PTC mode capture also
* waits for the enclosing `run_code` result. The terminal result marker and monotonic tool
* guard prevent later calls from reopening a completed structured run.
* @module @deepseek-ai/dsh-subagent/structured
*/
/** The model-facing tool name a structured child must call to finish. */
const STRUCTURED_OUTPUT_TOOL = "structured_output";
/**
* The instruction registered as the child's trailing scoped prompt section:
* the demand travels with the
* tool, as ordinary prompt state of exactly one agent.
*/
const STRUCTURED_OUTPUT_INSTRUCTION = `When you have your final answer, you MUST report it by calling the \`${STRUCTURED_OUTPUT_TOOL}\` tool with arguments matching its parameter schema exactly. Do not finish with a plain text answer: only the tool call counts as your result.`;
/**
* Attach the scoped capture tool, instruction, and enforcement to a child during
* its creation window. Child disposal removes every registration.
* @param childCtx - the child agent's scope context (`setup`'s argument).
* @param schema - the trusted, already-asserted schema subset to enforce (see
*   `assertObjectJsonSchema` in dsh-tools).
* @param canComplete - whether accepted input has been processed.
* @returns the attachment handle (read `captured()` after the child settles).
*/
function attachStructuredRuntime(childCtx, schema, canComplete) {
	/**
	* Validated values staged by the capture tool body, awaiting THEIR OWN
	* authoritative `tools/result` notification. The execution object's identity
	* uniquely identifies a trip through the pipeline: adapter call ids may
	* repeat across steps, but another execution can never reach this Map
	* entry. This is distinct from the opaque `ToolExecutionToken` used to
	* correlate nested transports. The final notification always deletes its own
	* stage, whether the result succeeded or failed.
	*/
	const staged = /* @__PURE__ */ new Map();
	/** Successful nested capture waiting for its enclosing transport to commit. */
	let pending;
	let captured;
	const schemaEntry = {
		name: STRUCTURED_OUTPUT_TOOL,
		description: "Report your final structured result. Call this exactly once, when your answer is complete; the arguments must match this tool's parameter schema exactly.",
		parameters: { ...schema }
	};
	childCtx.tools.register({
		...schemaEntry,
		output: {
			schema: {
				type: "object",
				properties: { recorded: {
					type: "boolean",
					const: true
				} },
				required: ["recorded"],
				additionalProperties: false
			},
			render: () => [{
				type: "text",
				text: "Structured output recorded."
			}]
		},
		execute(args, exec) {
			const violations = validateJsonSchemaValue(schema, args);
			if (violations.length > 0) throw new ToolArgsError(violations);
			if (!canComplete()) throw new Error("Process pending messages before submitting structured output.");
			staged.set(exec, { value: args });
			exec.concludeTurn();
			return Promise.resolve({ recorded: true });
		}
	});
	childCtx.systemPrompt.section({
		name: `tool:${STRUCTURED_OUTPUT_TOOL}`,
		order: childCtx.systemPrompt.getSectionOrder("STRUCTURED_OUTPUT"),
		text: STRUCTURED_OUTPUT_INSTRUCTION
	});
	childCtx.tools.guard((exec) => captured === void 0 && pending === void 0 ? void 0 : `structured output already recorded: the run is complete, so \`${exec.name}\` is not executed`);
	childCtx.on("tools/result", function(exec, result) {
		if (exec.name === "structured_output") {
			const entry = staged.get(exec);
			if (entry === void 0) return;
			staged.delete(exec);
			if (result.isError) return;
			if (exec.parent === void 0) {
				/* v8 ignore else -- sequential agent-loop dispatch lets the guard block every later supported call */
				if (captured === void 0) captured = { value: entry.value };
			} else if (captured === void 0 && pending === void 0) pending = {
				parent: exec.parent,
				value: entry.value
			};
			return;
		}
		if (pending?.parent !== exec.token) return;
		const entry = pending;
		pending = void 0;
		if (result.isError) return;
		/* v8 ignore else -- PTC mode serializes outer executions, so the guard blocks every later supported call */
		if (captured === void 0) captured = { value: entry.value };
	});
	return {
		captured: () => captured,
		acceptsInput: () => staged.size === 0 && pending === void 0 && captured === void 0
	};
}
//#endregion
//#region lib/types/assistant-output.js
/**
* Canonical selection of a child's final assistant output. Backend run results
* and `subagent/end.lastAssistantMessage` apply the same rule: select the last
* non-empty assistant message. An empty-content message records usage only
* when the loop appends it after a max-tokens step with no executable blocks,
* so it does not replace earlier output. If no non-empty message exists,
* select the accumulated assistant text. Selection is independent of the
* run's stop reason.
*
* @module @deepseek-ai/dsh-subagent/assistant-output
*/
/**
* Incremental fold of the selection rule, for backends that observe a child's
* output as it streams: session-event backends {@link push} each event, and
* transports without session events (ACP content chunks) {@link pushText} raw
* text into the same streamed fallback.
*/
var AssistantOutputFold = class {
	message;
	partial = [];
	/**
	* Fold one session event: a non-empty assistant message becomes the
	* candidate final answer, while its embedded stream and any log-only attempt
	* extend the streamed fallback; every other event contributes nothing.
	* @param event - the next observed session event.
	*/
	push(event) {
		if (event.type === "assistant/message") {
			const content = event.data.message.content;
			if (content.length > 0) this.message = content;
		}
		if (event.type === "assistant/message" || event.type === "assistant/attempt") this.pushText(joinAssistantStreamText(event.data.stream));
	}
	/**
	* Extend the streamed fallback with text observed outside session events.
	* @param text - the next streamed text piece (an empty piece is a no-op).
	*/
	pushText(text) {
		if (text.length > 0) this.partial.push(text);
	}
	/**
	* Select the final output folded so far.
	* @returns the last non-empty assistant message, else the accumulated
	*   streamed text, or `undefined` when the child produced neither.
	*/
	collect() {
		if (this.message !== void 0) return this.message;
		const text = this.partial.join("");
		return text.length > 0 ? [{
			type: "text",
			text
		}] : void 0;
	}
};
/**
* Apply the selection rule to one complete child-owned event suffix.
* @param events - the child-owned events (after any seed or epoch boundary).
* @returns the selected output, or `undefined` when the child produced none.
*/
function finalAssistantOutput(events) {
	const fold = new AssistantOutputFold();
	for (const event of events) fold.push(event);
	return fold.collect();
}
//#endregion
//#region lib/types/activation.js
/** Execution-period records, capacity slots, child serialization, and local output capture.
* @module @deepseek-ai/dsh-subagent/activation
*/
/** Process-local capacity shared by every member of a subagent tree. */
var ActivationPool = class {
	slots = /* @__PURE__ */ new Set();
	/**
	* Reserve before reconstruction; release also tolerates unpublished rollback.
	* @param capacity - the maximum simultaneous activations in this pool.
	* @returns an idempotent slot release.
	*/
	reserve(capacity) {
		if (this.slots.size >= capacity) throw new SubagentError(`subagent limit reached (active child limit: ${capacity}); wait for an existing child to finish or complete this work with the current agents`, "ACTIVATION_LIMIT_REACHED");
		const slot = Symbol();
		this.slots.add(slot);
		return () => {
			this.slots.delete(slot);
		};
	}
};
/**
* Read the live Agent retained by local execution.
* @param activation - the managed local or external execution.
* @returns the local Agent, absent for an external run.
*/
function activationAgent(activation) {
	return activation.kind === "local" ? activation.handle.agent : void 0;
}
/**
* Require local residency before accepting further input.
* @param activation - execution targeted by message delivery.
* @returns the local activation.
* @throws when the external backend cannot accept another message.
*/
function requireLocalActivation(activation) {
	switch (activation.kind) {
		case "local": return activation;
		case "external": throw new SubagentError(`subagent "${activation.childId}" does not accept follow-up input`, "NOT_CONTINUABLE");
		/* v8 ignore next 2 -- Both execution kinds are handled above. */
		default: return assertNever(activation);
	}
}
/**
* Retain local failures whose turn ending could not be committed.
* @param activation - exact local Agent and epoch receiving the scoped observers.
* @param wake - notify its settlement watcher when failure changes readiness.
*/
function observeLocalFailure(activation, wake) {
	const child = activation.handle.agent;
	child.ctx.on("agent/error", ({ agent }) => {
		if (agent !== child) return;
		activation.failureAt = child.session.seq;
		wake();
	});
	child.ctx.on("session/event", (session, event) => {
		if (session === child.session && event.type === "turn/end" && activation.failureAt !== void 0 && event.seq >= activation.failureAt) activation.failureAt = void 0;
	});
}
/**
* Capture local output and retain failures without a later committed turn ending.
* @param activation - the settled local execution and any structured result.
* @returns final output, stop reason, and committed structured value.
*/
function captureLocalResult(activation) {
	const events = activation.handle.agent.session.snapshotEvents(activation.outputStart);
	const output = finalAssistantOutput(events) ?? [];
	const stopReason = activation.failureAt === void 0 ? epochStopReason(events) : "error";
	const structured = activation.structured;
	if (structured !== void 0) {
		const captured = structured.captured();
		if (captured !== void 0) return {
			output,
			stopReason,
			structured: captured.value
		};
		if (stopReason === "completed") return {
			output,
			stopReason: "error"
		};
	}
	return {
		output,
		stopReason
	};
}
/** Serialize each durable child's delivery, release, and disposal. */
var ChildLock = class {
	tails = /* @__PURE__ */ new Map();
	/**
	* Run `operation` after every previously queued operation for `childId`.
	* @param childId - the durable child whose operations are linearized.
	* @param operation - the critical section to run in order.
	* @returns the operation's own settlement.
	*/
	run(childId, operation) {
		const result = (this.tails.get(childId) ?? Promise.resolve()).then(operation, operation);
		const tail = result.then(() => void 0, () => void 0);
		this.tails.set(childId, tail);
		tail.then(() => {
			if (this.tails.get(childId) === tail) this.tails.delete(childId);
		});
		return result;
	}
};
//#endregion
//#region lib/types/manager.js
/**
* Owns subagent startup, message admission, cold resume, and execution lifetimes.
* One child lock orders input acceptance and closure; one activation map owns
* capacity, runtime ownership, and resource release across all providers.
*
* @module @deepseek-ai/dsh-subagent/manager
*/
var __addDisposableResource$1 = function(env, value, async) {
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
var __disposeResources$1 = (function(SuppressedError) {
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
/** Render only the safe outer message from execution or teardown failures. */
function failureMessage(error) {
	return error instanceof Error ? error.message : "unknown teardown failure";
}
/** Own subagent requests and the resources of every live activation. */
var SubagentManager = class {
	ctx;
	host;
	maxActiveSubagents;
	/** Child session id → its live Activation. Process-local, never durable. */
	resident = /* @__PURE__ */ new Map();
	/** Root identities retain their pool across child settlement without retaining dead roots. */
	rootPools = /* @__PURE__ */ new WeakMap();
	/** Materializations admitted before drain, tracked through publication or rollback. */
	materializations = /* @__PURE__ */ new Set();
	/** Per-child serializer shared by delivery, release, and disposal. */
	locks = new ChildLock();
	/** Structural Cordis owner of every Activation handle. */
	ownerCtx;
	/** Exact subagent tree roots whose host teardown has closed admission. */
	closingRoots = /* @__PURE__ */ new Set();
	/** Sibling names held by creations that have not committed their catalog entry. */
	names = new SiblingNameReservations();
	draining = false;
	/**
	* Build one manager inside the service's Agent-injected context.
	* @param ctx - context providing Agents, Sessions, and teardown ownership.
	* @param host - descriptor reads and lifecycle publication owned by the service.
	* @param maxActiveSubagents - the current configured child capacity.
	*/
	constructor(ctx, host, maxActiveSubagents) {
		this.ctx = ctx;
		this.host = host;
		this.maxActiveSubagents = maxActiveSubagents;
		const scope = ctx.plugin(function activationOwner() {});
		this.ownerCtx = scope.ctx;
		installSubagentArchiveAdmission(ctx, (agent) => {
			const activation = this.resident.get(agent.id);
			return activation !== void 0 && activationAgent(activation) === agent ? activation.ancestors : void 0;
		}, (session) => host.identityOf(session));
		ctx.on("agent/disposed", ({ agent }) => {
			this.closingRoots.delete(agent);
			const activation = this.resident.get(agent.id);
			if (activation?.kind === "local" && activation.handle.agent === agent) this.close(activation, () => this.finishDisposal(activation, false)).catch((error) => {
				this.reportTeardownFailure(activation, error);
			});
		});
		ctx.effect(function* () {
			yield scope.dispose;
			yield () => this.drain();
		}.bind(this), "subagents.manager()");
	}
	/**
	* Interrupt one live continuable child's current turn under the supplied authority.
	* @param targetSessionId - the durable child session id to interrupt.
	* @param authority - a trusted user operation or exact live ancestor Agent.
	*/
	interrupt(targetSessionId, authority) {
		if (authority.kind === "ancestor") {
			const caller = authority.agent;
			if (this.ctx.agents.get(caller.id) !== caller) throw new SubagentError(`interrupting "${targetSessionId}" requires the exact live ancestor agent`, "UNAUTHORIZED");
			if (caller.id === targetSessionId) throw new SubagentError(`agent "${caller.id}" cannot interrupt itself`, "UNAUTHORIZED");
		}
		const activation = this.resident.get(targetSessionId);
		if (activation === void 0) return;
		if (authority.kind === "ancestor" && (this.rootOf(authority.agent) !== activation.root || !activation.ancestors.includes(authority.agent.id))) throw new SubagentError(`subagent "${targetSessionId}" is not a live descendant of agent "${authority.agent.id}"`, "UNAUTHORIZED");
		if (activation.closing !== void 0) return;
		const kind = authority.kind === "user" ? "user" : "parent";
		if (activation.kind === "local") activation.handle.agent.cancel({ kind }, { keepInbox: true });
		else {
			activation.controller.abort({ kind });
			this.dispose(activation).catch(() => void 0);
		}
	}
	/**
	* Close admission, await every already-admitted materialization through
	* publication or rollback, then dispose every agent in the subagent tree.
	*/
	async drain() {
		this.draining = true;
		for (const pending of this.materializations) pending.controller.abort();
		await Promise.all([...this.materializations].map((materialization) => materialization.settled));
		await this.disposeRoots([...this.resident.values()], "activation(s)");
	}
	/**
	* Join progressing subagent tree work for its root, leaving idle parked inboxes resident.
	* @param root - exact live subagent tree root; ordinary members return false.
	* @returns whether work was joined and the host must recheck root completion.
	*/
	async waitForTreeWork(root) {
		if (this.rootOf(root) !== root) return false;
		let observed = false;
		while (true) {
			const pending = [...this.materializations].filter((item) => item.root === root);
			const children = [...this.resident.values()].filter((item) => item.root === root);
			observed ||= children.some((item) => item.kind === "local" && item.handle.agent.status !== "idle");
			await Promise.all(children.flatMap((item) => item.kind === "local" ? [item.handle.agent.whenIdle()] : []));
			const waits = pending.map((item) => item.settled);
			for (const item of children) {
				if (item.kind === "external" || item.closing !== void 0) {
					waits.push(item.result.promise.catch(() => void 0));
					continue;
				}
				if (item.handle.agent.status !== "idle") {
					waits.push(item.handle.agent.whenIdle());
					continue;
				}
				const inbox = item.handle.agent.inbox;
				if (item.failureAt === void 0 && (inbox.nextTurn.length > 0 || inbox.nextStep.length > 0)) continue;
				waits.push(Promise.race([item.result.promise.catch(() => void 0), item.poke.promise]));
			}
			if (waits.length > 0) {
				observed = true;
				await Promise.all(waits);
				continue;
			}
			if ([...this.materializations].some((item) => item.root === root && !pending.includes(item)) || [...this.resident.values()].some((item) => item.root === root && !children.includes(item))) continue;
			return observed;
		}
	}
	/**
	* Stop every member of the selected subagent trees; ordinary members do not close a subagent tree.
	* @param parents - exact live subagent tree roots whose instances must stop.
	*/
	async drainDescendants(parents) {
		const roots = new Set(parents.filter((root) => this.ctx.agents.get(root.id) === root && this.rootOf(root) === root));
		for (const root of roots) this.closingRoots.add(root);
		const pending = [...this.materializations].filter((item) => roots.has(item.root));
		for (const item of pending) item.controller.abort();
		const targets = [...this.resident.values()].filter((item) => roots.has(item.root));
		const disposal = this.disposeRoots(targets, "scoped activation(s)");
		const [disposed] = await Promise.allSettled([disposal, ...pending.map((item) => item.settled)]);
		if (disposed.status === "rejected") throw disposed.reason;
	}
	/**
	* Start one continuable background child and resolve at initial inbox acceptance.
	* Every earlier failure disposes any created handle and rolls back Activation
	* and its capacity slot without returning either id.
	* @param spec - provider, delegation request, and caller cancellation.
	* @param prepare - creation contribution bound to the checked provider.
	* @returns the durable child id and accepted initial prompt message id.
	*/
	async startLocal(spec, prepare) {
		const parent = spec.parent;
		this.assertAdmitting(parent);
		const ancestry = this.ancestryForChild(parent);
		const reservation = this.reserveName(parent, spec.name);
		const path = joinAgentPath(this.pathOf(parent, ancestry.root), reservation.name);
		try {
			return await this.startLocalNamed(spec, prepare, ancestry, reservation, path);
		} finally {
			reservation.release();
		}
	}
	/** Create one local child under a name reserved before any await. */
	startLocalNamed(spec, prepare, ancestry, reservation, path) {
		const parent = spec.parent;
		return this.trackStartup(ancestry.root, spec.signal, async (signal) => {
			const persistence = spec.delivery === "caller" ? this.ctx.get("sessionPersistence") : this.requirePersistence();
			const childId = spec.childId ?? brandString(randomUUID());
			this.assertChildIdAvailable(childId);
			const childDepth = resolveChildDepth(parent, spec.maxDepth);
			const agentOptions = resolveChildAgentOptions(parent, spec.agentOptions, childDepth);
			const agentProvider = agentOptions.provider;
			const agentModel = agentOptions.model;
			const agentReasoningEffort = agentOptions.reasoningEffort;
			const descriptor = snapshotSubagentDescriptor({
				mode: "continuable",
				provider: spec.provider,
				label: spec.label,
				...agentProvider !== void 0 ? { agentProvider } : {},
				...agentModel !== void 0 ? { agentModel } : {},
				...agentReasoningEffort !== void 0 ? { agentReasoningEffort } : {}
			});
			const delegatedPolicies = captureDelegatedPolicyOverrides(parent);
			const cwd = await this.resolveDirectory(spec, signal);
			const prepared = await prepare({
				sessionId: childId,
				cwd,
				parent,
				signal
			});
			signal.throwIfAborted();
			this.assertAdmitting(parent);
			const inheritedEventCount = SessionLogOffset(prepared.seed?.length ?? 0);
			const seed = prepared.seed;
			let established;
			const messageId = await this.locks.run(childId, async () => {
				signal.throwIfAborted();
				this.assertAdmitting(parent);
				this.assertChildIdAvailable(childId);
				if (spec.childId !== void 0) {
					const persisted = await persistence?.stat(childId, { signal });
					signal.throwIfAborted();
					this.assertAdmitting(parent);
					this.assertChildIdAvailable(childId);
					if (persisted !== void 0) throw new SubagentError(`subagent "${childId}" already exists`, "DUPLICATE_CHILD");
				}
				const activation = await this.materialize({
					kind: "local",
					childId,
					provider: spec.provider,
					...ancestry,
					path,
					create: {
						parent,
						cwd,
						seed,
						meta: childSessionMeta(parent, childDepth, prepared.seed !== void 0, spec.agentPreset),
						inheritedEventCount,
						delegatedPolicies,
						descriptor
					},
					agentOptions,
					composition: { agentPreset: spec.agentPreset },
					signal,
					delivery: spec.delivery,
					outputSchema: spec.outputSchema
				});
				established = activation;
				const childHeader = requireLocalActivation(activation).handle.agent.session.header;
				return await this.submitMaterialized(activation, spec.delivery !== "caller" ? withResultDeliveryGuidance(spec.prompt) : spec.prompt, {
					source: { kind: "user" },
					signal,
					delivery: "queue",
					authorize: () => {
						this.assertAdmitting(parent);
					}
				}, ancestry.root, () => {
					this.assertAdmitting(parent);
					establishCatalogChild(parent.session, childHeader, descriptor, reservation.name);
				});
			});
			return {
				...this.receipt(established),
				childId,
				messageId
			};
		});
	}
	/**
	* Start an external backend under activation ownership.
	* @param spec - task, provider, cancellation, and delivery policy.
	* @param start - execution bound to the checked provider.
	* @returns the accepted activation and its result/disposal capabilities.
	*/
	async startExternal(spec, start) {
		const { delivery, name: requestedName, parent } = spec;
		this.assertAdmitting(parent);
		const ancestry = this.ancestryForChild(parent);
		if (delivery === "caller" && requestedName !== void 0) throw new SubagentError("agent names require a cataloged child; caller-delivered external runs are not cataloged", "UNSUPPORTED_CAPABILITY");
		const reservation = delivery === "parent" ? this.reserveName(parent, requestedName) : void 0;
		const path = joinAgentPath(this.pathOf(parent, ancestry.root), reservation?.name);
		try {
			return await this.startExternalNamed(spec, start, ancestry, reservation, path);
		} finally {
			reservation?.release();
		}
	}
	/** Start one external execution, cataloging parent-delivered runs under their reserved name. */
	startExternalNamed(spec, start, ancestry, reservation, path) {
		const { provider, delivery, childId: _childId, name: _name, ...request } = spec;
		const parent = request.parent;
		return this.trackStartup(ancestry.root, spec.signal, async (signal) => {
			let activation;
			try {
				const cwd = await this.resolveDirectory(spec, signal);
				activation = await this.materialize({
					kind: "external",
					provider,
					...ancestry,
					path,
					signal,
					delivery,
					start: (signal) => start({
						...request,
						cwd,
						signal
					})
				});
				this.assertAdmitting(parent);
				signal.throwIfAborted();
				if (reservation !== void 0) establishExternalCatalogChild(parent.session, activation.childId, spec.label, reservation.name);
				this.accept(activation);
				return this.receipt(activation);
			} catch (error) {
				if (activation !== void 0) try {
					await this.dispose(activation);
				} catch (cleanupError) {
					this.ctx.logger.warn(`subagent "${activation.childId}" admission rollback failed: ${failureMessage(cleanupError)}`);
				}
				throw error;
			}
		});
	}
	/** Capture the explicit or inherited directory while the parent still owns startup. */
	async resolveDirectory(request, signal) {
		const cwd = request.cwd !== void 0 && isAbsolute(request.cwd) ? request.cwd : resolve(await this.ctx.workingDirectory.ensure(request.parent, signal), request.cwd ?? ".");
		signal.throwIfAborted();
		return cwd;
	}
	/**
	* Resolve the live subagent tree root from runtime ownership, independent of creator residency.
	* @param caller - exact live Agent selecting the subagent tree.
	* @returns the live subagent tree root.
	* @throws when the caller has no active runtime owner.
	*/
	rootOf(caller) {
		if (this.ctx.agents.get(caller.id) !== caller) throw new SubagentError("subagent tree access requires the exact live caller", "UNAUTHORIZED");
		const identity = this.host.identityOf(caller.session);
		const delegated = caller.session.header.origin === "subagent" || identity != null && caller.session.isOwnSeq(identity.seq);
		const activation = this.resident.get(caller.id);
		if (activation !== void 0 && activationAgent(activation) === caller && this.ctx.agents.get(activation.root.id) === activation.root) return activation.root;
		if (delegated || activation !== void 0) throw new SubagentError("agent has no active runtime owner", "UNAUTHORIZED");
		return caller;
	}
	/** Capture creation ancestry separately from the root that owns the new instance. */
	ancestryForChild(creator) {
		return {
			root: this.rootOf(creator),
			ancestors: [...this.resident.get(creator.id)?.ancestors ?? [], creator.id]
		};
	}
	/** Claim a sibling name against the creator's committed catalog and in-flight siblings. */
	reserveName(parent, requested) {
		return this.names.reserve(parent, catalogNames(this.host.catalogOf(parent.session)), requested);
	}
	/** Path of an exact live tree member: `''` for the root, undefined for unnamed history. */
	pathOf(member, root) {
		return member === root ? "" : this.resident.get(member.id)?.path;
	}
	/**
	* Report the model-facing names of a live member and its direct creator.
	* @param agent - exact live root or resident tree member.
	* @returns `lead` for the root, otherwise the member's and creator's names or Session ids.
	* @throws when the caller has no active runtime owner.
	*/
	namesOf(agent) {
		const root = this.rootOf(agent);
		if (root === agent) return { self: ROOT_AGENT_NAME };
		const activation = this.resident.get(agent.id);
		const parentId = activation.ancestors.at(-1);
		const parentPath = activation.path === void 0 ? parentId === root.id ? "" : this.resident.get(parentId)?.path : parentAgentPath(activation.path);
		return {
			self: agentAddress(activation.path, agent.id),
			parent: agentAddress(parentPath, parentId)
		};
	}
	/**
	* Deliver to another agent in the subagent tree, restoring only the target when absent.
	* @param sender - exact live root or resident continuable Agent.
	* @param targetId - another member's durable Session id.
	* @param content - model-authored message blocks.
	* @param options - cancellation before inbox acceptance.
	* @returns the accepted inbox message id.
	*/
	async sendMessage(sender, targetId, content, options) {
		options.signal.throwIfAborted();
		const root = this.rootOf(sender);
		if (sender.id === targetId) throw new SubagentError("send_message requires another agent in the subagent tree", "UNAUTHORIZED");
		this.assertAdmitting(sender);
		const message = createAgentMessage(sender, agentAddress(this.pathOf(sender, root), sender.id), content);
		const activation = this.resident.get(sender.id);
		return this.deliverToMember(root, targetId, [...message.content], {
			signal: options.signal,
			delivery: "steer",
			source: message.source,
			authorize: () => {
				this.assertAdmitting(sender);
			},
			onAccepted: () => {
				if (activation?.kind === "local" && activation.ancestors.at(-1) === targetId) activation.omitClosingText = true;
			}
		});
	}
	/** Resolve the live owner through verified creation edges without activating ancestors. */
	async resolveCreationAncestry(source, signal, root) {
		const ancestors = [];
		const names = [];
		const seen = new Set([source.header.id]);
		let { header, projections, inheritedEventCount } = source;
		const pathFromRoot = () => names.reduce(joinAgentPath, "");
		while (true) {
			const env_1 = {
				stack: [],
				error: void 0,
				hasError: false
			};
			try {
				signal.throwIfAborted();
				const parentId = header.parentSession;
				const identity = projections?.values.subagent;
				if (parentId === void 0 || identity == null || identity.seq < inheritedEventCount) break;
				if (seen.has(parentId)) throw new SubagentError("cyclic subagent creation ancestry", "UNAUTHORIZED");
				seen.add(parentId);
				const parent = __addDisposableResource$1(env_1, await this.observeContinuation(parentId, signal), false);
				const entry = parent.projections?.values.subagentCatalog?.find((entry) => entry.id === header.id);
				if (entry === void 0) throw new SubagentError("subagent is absent from its creator catalog", "UNAUTHORIZED");
				ancestors.unshift(parentId);
				names.unshift("name" in entry ? entry.name : void 0);
				if (parentId === root?.id) return {
					root,
					ancestors,
					path: pathFromRoot()
				};
				({header, projections, inheritedEventCount} = parent);
			} catch (e_1) {
				env_1.error = e_1;
				env_1.hasError = true;
			} finally {
				__disposeResources$1(env_1);
			}
		}
		if (root === void 0 && ancestors.length > 0 && header.origin !== "subagent") {
			const owner = this.ctx.agents.get(header.id);
			if (owner === void 0 || this.rootOf(owner) !== owner) throw new SubagentError("subagent tree root is not live", "CONTINUATION_UNAVAILABLE");
			return {
				root: owner,
				ancestors,
				path: pathFromRoot()
			};
		}
		throw new SubagentError(`agent "${source.header.id}" is not a member of this subagent tree`, "UNAUTHORIZED");
	}
	/**
	* Deliver one host-authored prompt through the target's live subagent tree root.
	* Remote callers must be authorized for the target Session before invoking this operation.
	* @param childId - durable continuable subagent session id.
	* @param content - model-visible prompt blocks.
	* @param options - inbox destination, durable attribution, and cancellation before acceptance.
	* @returns the accepted durable message id.
	*/
	async deliverPrompt(childId, content, options) {
		options.signal.throwIfAborted();
		let root = this.resident.get(childId)?.root;
		if (root === void 0) {
			const env_2 = {
				stack: [],
				error: void 0,
				hasError: false
			};
			try {
				const source = __addDisposableResource$1(env_2, await this.observeContinuation(childId, options.signal), false);
				({root} = await this.resolveCreationAncestry(source, options.signal));
			} catch (e_2) {
				env_2.error = e_2;
				env_2.hasError = true;
			} finally {
				__disposeResources$1(env_2);
			}
		}
		return this.deliverToMember(root, childId, content, options);
	}
	/** Admit local input synchronously before any teardown can begin. */
	deliver(activation, message, delivery) {
		if (activation.closing !== void 0) throw new SubagentError("subagent activation is being disposed; the message was not accepted", "ACTIVATION_CLOSING");
		const local = requireLocalActivation(activation);
		if (local.structured?.acceptsInput() === false) throw new SubagentError("subagent is submitting or has submitted its structured result; the message was not accepted", "INPUT_CLOSED");
		if (delivery === "steer") local.handle.agent.steer(message);
		else local.handle.agent.followup(message);
	}
	/**
	* Reject one child identity already owned by a live Agent or Session.
	* @param childId - proposed durable child session id.
	*/
	assertChildIdAvailable(childId) {
		if (this.resident.has(childId) || this.ctx.agents.get(childId) !== void 0 || this.ctx.get("sessions")?.get(childId) !== void 0) throw new SubagentError(`subagent "${childId}" already exists`, "DUPLICATE_CHILD");
	}
	/**
	* Apply activation admission to a resident parent before delivering input.
	* @param parent - exact live Agent receiving the message.
	* @param message - durable user message to deliver.
	* @param delivery - receiving inbox destination.
	*/
	sendWaking(parent, message, delivery) {
		const parentActivation = this.resident.get(parent.id);
		if (parentActivation !== void 0 && activationAgent(parentActivation) === parent) {
			try {
				this.deliver(parentActivation, message, delivery);
			} finally {
				this.wake(parentActivation);
			}
			return;
		}
		if (delivery === "steer") parent.steer(message);
		else parent.followup(message);
	}
	/**
	* Reject new work from closing members or subagent trees.
	* @param agent - exact live caller whose instance and subagent tree must admit work.
	*/
	assertAdmitting(agent) {
		const closing = this.closingTeardownFor(agent);
		if (closing === void 0) {
			if (this.resident.get(agent.id)?.closing !== void 0) throw new SubagentError(`subagent "${agent.id}" is closing`, "ACTIVATION_CLOSING");
			return;
		}
		throw new SubagentError(closing === "manager" ? "subagents are draining; the operation was not admitted" : `subagent tree "${closing.id}" is draining; the operation was not admitted`, "DRAINING");
	}
	/**
	* Create or resume one child Agent and publish its Activation.
	* @param inputs - reconstruction and admission inputs for the residency epoch.
	* @returns the published process-local Activation.
	*/
	materialize(inputs) {
		this.assertAdmitting(inputs.root);
		inputs.signal.throwIfAborted();
		const releaseSlot = this.rootPool(inputs.root).reserve(this.maxActiveSubagents());
		return this.materializeTracked(inputs, releaseSlot).catch((error) => {
			releaseSlot();
			throw error;
		});
	}
	/** Keep subagent tree shutdown and host waits joined to preparation, publication, and rollback. */
	trackStartup(root, signal, start) {
		this.assertAdmitting(root);
		signal.throwIfAborted();
		const settled = Promise.withResolvers();
		const controller = new AbortController();
		const materialization = {
			root,
			settled: settled.promise,
			controller
		};
		this.materializations.add(materialization);
		return start(AbortSignal.any([signal, controller.signal])).catch((error) => {
			if (error instanceof Error && error.name === "AbortError") this.assertAdmitting(root);
			throw error;
		}).finally(() => {
			this.materializations.delete(materialization);
			settled.resolve();
		});
	}
	/**
	* Stop and release one Activation through its memoized close transaction.
	* @param activation - exact residency epoch to close.
	* @returns the shared close transaction.
	*/
	dispose(activation) {
		return this.close(activation, () => this.finishDisposal(activation, true));
	}
	/** Close admission synchronously and share one release, including startup rollback. */
	close(activation, release) {
		if (activation.closing !== void 0) return activation.closing;
		const completion = Promise.withResolvers();
		activation.closing = completion.promise;
		release().then(completion.resolve, completion.reject);
		return completion.promise;
	}
	/** Dispose independent roots and report every branch failure after all settle. */
	async disposeRoots(roots, failureSubject) {
		const reasons = (await Promise.all(roots.map(async (activation) => {
			try {
				await this.dispose(activation);
				return;
			} catch (error) {
				return error;
			}
		}))).filter((failure) => failure !== void 0);
		if (reasons.length > 0) throw new SubagentError(`subagent teardown failed for ${reasons.length} ${failureSubject}: ` + reasons.map((reason) => failureMessage(reason)).join("; "), "ACTIVATION_TEARDOWN_FAILED");
	}
	/** Return the subagent tree teardown that closed admission for this exact Agent. */
	closingTeardownFor(agent) {
		if (this.draining) return "manager";
		const root = this.rootOf(agent);
		return this.closingRoots.has(root) ? root : void 0;
	}
	/** Share one capacity pool across every instance in a subagent tree. */
	rootPool(parent) {
		let pool = this.rootPools.get(parent);
		if (pool === void 0) {
			pool = new ActivationPool();
			this.rootPools.set(parent, pool);
		}
		return pool;
	}
	/** Perform one tracked materialization through publication or rollback. */
	async materializeTracked(inputs, releaseSlot) {
		const { provider, root } = inputs;
		inputs.signal.throwIfAborted();
		let resource;
		if (inputs.kind === "external") {
			const controller = new AbortController();
			const cancelStartup = () => {
				controller.abort(inputs.signal.reason);
			};
			inputs.signal.addEventListener("abort", cancelStartup, { once: true });
			try {
				const run = await inputs.start(controller.signal);
				resource = {
					kind: "external",
					run,
					controller
				};
				run.result.catch(() => void 0);
				try {
					this.assertChildIdAvailable(run.id);
				} catch (error) {
					await run.dispose();
					throw error;
				}
			} finally {
				inputs.signal.removeEventListener("abort", cancelStartup);
			}
		} else {
			const { create, childId } = inputs;
			let structured;
			const setup = async (childCtx, child) => {
				if (inputs.composition.agentPreset !== void 0) await applyChildComposition(childCtx, create?.parent, inputs.composition);
				if (create !== void 0) {
					if (inputs.composition.agentPreset !== void 0) child.session.append("agent-preset/selected", { agentPreset: inputs.composition.agentPreset });
					child.session.append("subagent/descriptor", create.descriptor);
					appendDelegatedPolicyOverrides(child.session, create.delegatedPolicies);
					const workingDirectory = childCtx.get("workingDirectory");
					if (workingDirectory === void 0) throw new Error("local subagents require the working-directory service");
					await workingDirectory.set(child, create.cwd, inputs.signal);
				}
				if (inputs.composition.agentPreset === void 0) await applyChildComposition(childCtx, create?.parent, inputs.composition);
				if (inputs.outputSchema !== void 0) structured = attachStructuredRuntime(childCtx, inputs.outputSchema, () => child.inbox.nextStep.length === 0 && child.inbox.nextTurn.length === 0);
			};
			const handle = create === void 0 ? await this.ownerCtx.agents.resume({
				resumeSessionId: childId,
				agentOptions: inputs.agentOptions,
				signal: inputs.signal,
				setup
			}) : await this.ownerCtx.agents.create({
				sessionId: childId,
				meta: create.meta,
				...create.seed === void 0 ? {} : { seed: create.seed },
				inheritedEventCount: create.inheritedEventCount,
				agentOptions: inputs.agentOptions,
				signal: inputs.signal,
				setup
			});
			resource = {
				kind: "local",
				handle,
				outputStart: handle.agent.session.seq,
				structured,
				failureAt: void 0,
				poke: Promise.withResolvers(),
				omitClosingText: false
			};
		}
		const childId = resource.kind === "local" ? resource.handle.agent.id : resource.run.id;
		const activation = {
			releaseSlot,
			childId,
			runId: SubagentRunId(randomUUID()),
			provider,
			...resource,
			root,
			ancestors: inputs.ancestors,
			path: inputs.path,
			delivery: inputs.delivery ?? "parent",
			result: Promise.withResolvers(),
			closing: void 0,
			accepted: false
		};
		activation.result.promise.catch(() => void 0);
		this.resident.set(childId, activation);
		try {
			this.assertAdmitting(root);
			inputs.signal.throwIfAborted();
			const wakeSettlement = () => {
				this.wake(activation);
			};
			if (activation.kind === "local") {
				observeLocalFailure(activation, wakeSettlement);
				activation.handle.agent.ctx.on("agent/inbox/inserted", wakeSettlement);
				activation.handle.agent.ctx.on("agent/inbox/claimed", () => {
					activation.omitClosingText = false;
					wakeSettlement();
				});
				activation.handle.agent.ctx.on("agent/inbox/discarded", wakeSettlement);
				activation.handle.agent.ctx.on("session/event", (_session, event) => {
					if (event.type === "tool/call") activation.omitClosingText = false;
				});
			}
			this.host.emitLifecycle("subagent/start", this.runInfo(activation), root);
		} catch (error) {
			/* v8 ignore next -- rollback failure must not mask the admission failure
			* that prevented this operation from returning an accepted message id. */
			await this.rollbackUnpublished(activation, error).catch(() => void 0);
			throw error;
		}
		return activation;
	}
	/** Identity shared by one residency epoch's start/end pair, including cold resumes. */
	runInfo(activation) {
		return {
			runId: activation.runId,
			provider: activation.provider,
			parentSessionId: activation.ancestors.at(-1),
			id: activation.childId,
			local: activation.kind === "local"
		};
	}
	/** Record acceptance and begin settlement observation before returning the receipt. */
	accept(activation) {
		activation.accepted = true;
		if (activation.kind === "local") this.watchLocalSettlement(activation);
		else {
			const finish = () => this.close(activation, () => this.finishDisposal(activation, false));
			activation.run.result.then(finish, finish).catch((error) => {
				this.reportTeardownFailure(activation, error);
			});
		}
	}
	/** Release an Activation whose start edge was not published. */
	rollbackUnpublished(activation, error) {
		return this.close(activation, async () => {
			try {
				await (activation.kind === "local" ? activation.handle.dispose() : activation.run.dispose());
			} finally {
				this.resident.delete(activation.childId);
				activation.releaseSlot();
				activation.result.reject(error);
			}
		});
	}
	/** Let a settlement watcher re-check residency after relevant state changes. */
	wake(activation) {
		if (activation.kind === "external") return;
		activation.poke.resolve();
		activation.poke = Promise.withResolvers();
	}
	/** Follow one Activation to natural settlement. */
	watchLocalSettlement(activation) {
		(async () => {
			while (true) {
				const idleObservation = activation.poke;
				await activation.handle.agent.whenIdle();
				if (activation.closing !== void 0) return;
				const readiness = await this.locks.run(activation.childId, () => Promise.resolve(this.settlementState(activation, idleObservation)));
				if (readiness === "closed") return;
				if (readiness === "retry") continue;
				if (readiness === "wait") {
					await idleObservation.promise;
					continue;
				}
				const finalSeq = activation.handle.agent.session.seq;
				await this.flushFinalState(activation);
				const attempt = await this.locks.run(activation.childId, () => {
					const state = this.settlementState(activation, idleObservation);
					if (state !== "ready") return Promise.resolve(state);
					if (activation.handle.agent.session.seq !== finalSeq) return Promise.resolve("retry");
					let done;
					try {
						activation.handle.agent.runMaintenance(() => {
							done = this.close(activation, () => this.finishDisposal(activation, false));
							return Promise.resolve();
						});
					} catch (_error) {
						return Promise.resolve("retry");
					}
					return Promise.resolve({ done });
				});
				if (typeof attempt === "string") {
					if (attempt === "closed") return;
					continue;
				}
				try {
					await attempt.done;
				} catch (error) {
					this.reportTeardownFailure(activation, error);
				}
				return;
			}
		})();
	}
	/** Report disposal failure once after the natural settlement observer finishes. */
	reportTeardownFailure(activation, error) {
		this.ctx.logger.warn(`subagent "${activation.childId}" activation teardown failed: ${failureMessage(error)}`);
	}
	/** Check pending input before closing admission. */
	settlementState(activation, observation) {
		if (activation.closing !== void 0) return "closed";
		if (activation.poke !== observation) return "retry";
		const inbox = activation.handle.agent.inbox;
		if (activation.failureAt === void 0 && (inbox.nextTurn.length > 0 || inbox.nextStep.length > 0)) return "wait";
		return "ready";
	}
	/** Stop and release one instance without affecting its creation descendants. */
	async finishDisposal(activation, stop) {
		this.wake(activation);
		const { childId } = activation;
		const failures = [];
		let result = {
			output: [],
			stopReason: "error"
		};
		let resultFailure;
		let externalDisposal;
		const child = activationAgent(activation);
		const detail = (error) => child === void 0 ? failureMessage(error) : errorChain(error);
		try {
			if (activation.kind === "external") {
				if (stop) {
					activation.controller.abort({ kind: "parent" });
					externalDisposal = Promise.resolve().then(() => activation.run.dispose());
					externalDisposal.catch(() => void 0);
				}
				result = await activation.run.result;
			} else {
				if (stop) {
					activation.handle.agent.cancel({ kind: "parent" });
					await activation.handle.agent.whenIdle();
					await this.flushFinalState(activation);
				}
				result = captureLocalResult(activation);
			}
		} catch (error) {
			resultFailure = { error };
			failures.push(new SubagentError(`subagent "${childId}" activation teardown failed: ${detail(error)}`, "ACTIVATION_TEARDOWN_FAILED", { cause: error }));
		}
		try {
			if (activation.kind === "local") await activation.handle.dispose();
			else await (externalDisposal ?? activation.run.dispose());
		} catch (error) {
			failures.push(new SubagentError(`subagent "${childId}" activation handle disposal failed: ${detail(error)}`, "ACTIVATION_TEARDOWN_FAILED", { cause: error }));
		}
		let failure;
		if (failures.length === 1) failure = failures[0];
		else if (failures.length > 1) failure = new SubagentError(`subagent "${childId}" activation teardown failed at ${failures.length} boundaries: ` + failures.map((item) => detail(item)).join("; "), "ACTIVATION_TEARDOWN_FAILED", { cause: new AggregateError(failures) });
		if (failure !== void 0 && result.stopReason === "completed" && child !== void 0 && (child.inbox.nextStep.length > 0 || child.inbox.nextTurn.length > 0)) result = {
			...result,
			stopReason: "error"
		};
		this.resident.delete(childId);
		activation.releaseSlot();
		const terminal = failure === void 0 ? result : {
			output: [],
			stopReason: "error"
		};
		this.notifySettlement(activation, result);
		this.host.emitLifecycle("subagent/end", {
			...this.runInfo(activation),
			stopReason: terminal.stopReason,
			...terminal.output.length === 0 ? {} : { lastAssistantMessage: terminal.output }
		}, activation.root);
		if (resultFailure === void 0) activation.result.resolve(result);
		else activation.result.reject(resultFailure.error);
		if (failure !== void 0) throw failure;
	}
	/** Deliver the captured execution result to the durable direct parent after cleanup. */
	notifySettlement(activation, terminal) {
		if (!activation.accepted || activation.delivery === "caller") return;
		try {
			const parentId = activation.ancestors.at(-1);
			const parent = this.ctx.agents.get(parentId);
			if (parent === void 0 || parent.ctx.fiber.state !== 2 || this.rootOf(parent) !== activation.root) return;
			const message = createSettlementMessage(activation.childId, agentAddress(activation.path, activation.childId), terminal, activation.kind === "local", activation.kind === "local" && activation.omitClosingText);
			if (this.closingTeardownFor(parent) !== void 0) {
				parent.inject(message);
				return;
			}
			this.sendWaking(parent, message, parent.status === "idle" ? "queue" : "steer");
		} catch (error) {
			this.ctx.logger.warn(`subagent "${activation.childId}" settlement notice was not delivered to its parent: ` + errorChain(error));
		}
	}
	/** Request a best-effort final session flush before closing natural-settlement admission. */
	async flushFinalState(activation) {
		try {
			await activation.handle.agent.ctx.sessions.flush(activation.handle.agent.session);
		} catch (error) {
			this.ctx.logger.warn(`subagent "${activation.childId}" best-effort final session flush failed; the persisted state may be unavailable or stale on resume: ${errorChain(error)}`);
		}
	}
	/** Return capabilities bound to an exact activation instead of a future same-id instance. */
	receipt(activation) {
		return {
			childId: activation.childId,
			name: agentAddress(activation.path, activation.childId),
			result: activation.result.promise,
			dispose: () => this.dispose(activation)
		};
	}
	/** Serialize target restoration and inbox admission; closure is awaited outside the lock. */
	async deliverToMember(root, childId, content, options) {
		while (true) {
			this.assertAdmitting(root);
			options.signal.throwIfAborted();
			const result = await this.locks.run(childId, async () => {
				if (childId === root.id) {
					if (contentHasImage(content)) await this.assertImageCapable(root, options.signal);
					options.signal.throwIfAborted();
					options.authorize?.();
					this.assertAdmitting(root);
					const message = createUserMessage({
						content,
						source: options.source
					});
					this.sendWaking(root, message, options.delivery);
					options.onAccepted?.();
					return { messageId: message.id };
				}
				const activation = this.resident.get(childId);
				if (activation === void 0) {
					const restored = await this.restoreTarget(root, childId, options.signal);
					return { messageId: await this.submitMaterialized(restored, content, options, root) };
				}
				if (activation.root !== root) throw new SubagentError("target belongs to another subagent tree", "UNAUTHORIZED");
				requireLocalActivation(activation);
				if (activation.closing === void 0 && contentHasImage(content)) await this.assertImageCapable(requireLocalActivation(activation).handle.agent, options.signal);
				if (activation.closing !== void 0) return { closing: activation.closing };
				return { messageId: this.submitAdmitted(activation, content, options, root) };
			});
			if ("messageId" in result) return result.messageId;
			await Promise.allSettled([result.closing]);
		}
	}
	/** Read continuation history, preserving cancellation and reporting unavailable storage. */
	async observeContinuation(childId, signal) {
		const query = this.requireSessionQuery();
		try {
			return await query.observeSession(childId, { signal });
		} catch (error) {
			signal.throwIfAborted();
			throw new SubagentError(`subagent "${childId}" is unavailable`, "NOT_RESUMABLE", { cause: error });
		}
	}
	/** Reconstruct only the target under its subagent tree root; the caller owns acceptance or rollback. */
	async restoreTarget(root, childId, signal) {
		return this.trackStartup(root, signal, async (signal) => {
			const env_3 = {
				stack: [],
				error: void 0,
				hasError: false
			};
			try {
				const source = __addDisposableResource$1(env_3, await this.observeContinuation(childId, signal), false);
				this.assertAdmitting(root);
				const descriptor = foldSubagentDescriptor(source.events.slice(source.inheritedEventCount));
				if (descriptor === void 0 || descriptor.mode !== "continuable") throw new SubagentError(`subagent "${childId}" has no supported continuation state and cannot be resumed; choose a different target`, "NOT_RESUMABLE");
				const { ancestors, path } = await this.resolveCreationAncestry(source, signal, root);
				let activation;
				try {
					const projectedPreset = source.projections?.values.agentPreset;
					if (projectedPreset === void 0 && source.events.some((event) => event.type === "agent-preset/selected")) throw new SubagentError(`subagent "${childId}" has no agent preset projection for restoration`, "NOT_RESUMABLE");
					const agentPreset = projectedPreset ?? source.header.agentPreset;
					activation = await this.materialize({
						kind: "local",
						childId,
						provider: descriptor.provider,
						root,
						ancestors,
						path,
						agentOptions: {
							...descriptor.agentProvider !== void 0 ? { provider: descriptor.agentProvider } : {},
							...descriptor.agentModel !== void 0 ? { model: descriptor.agentModel } : {},
							...descriptor.agentReasoningEffort !== void 0 ? { reasoningEffort: ReasoningEffortId(descriptor.agentReasoningEffort) } : {}
						},
						composition: {
							agentPreset: agentPreset ?? void 0,
							persona: descriptor.persona,
							toolFilter: descriptor.toolFilter
						},
						signal
					});
				} catch (error) {
					signal.throwIfAborted();
					if (error instanceof SubagentError) throw error;
					throw new SubagentError(`subagent "${childId}" is unavailable`, "NOT_RESUMABLE", { cause: error });
				}
				return activation;
			} catch (e_3) {
				env_3.error = e_3;
				env_3.hasError = true;
			} finally {
				__disposeResources$1(env_3);
			}
		});
	}
	/** Admit a materialized child, commit its creation fact, and release it on failure. */
	async submitMaterialized(activation, content, options, parent, commit) {
		try {
			if (contentHasImage(content)) {
				await this.assertImageCapable(requireLocalActivation(activation).handle.agent, options.signal);
				if (activation.closing !== void 0) throw new SubagentError(`subagent "${activation.childId}" is closing`, "ACTIVATION_CLOSING");
			}
			const messageId = this.submitAdmitted(activation, content, options, parent);
			commit?.();
			this.accept(activation);
			return messageId;
		} catch (error) {
			try {
				await this.dispose(activation);
			} catch (cleanupError) {
				this.ctx.logger.warn(`subagent continuation: disposal after admission or catalog append failure also failed: ${String(cleanupError)}`);
			}
			throw error;
		}
	}
	/** Build and submit one message across the final synchronous admission cutoff. */
	submitAdmitted(activation, content, options, parent) {
		const message = createUserMessage({
			content,
			source: options.source
		});
		options.signal.throwIfAborted();
		options.authorize?.();
		this.assertAdmitting(parent);
		try {
			this.deliver(activation, message, options.delivery);
		} finally {
			this.wake(activation);
		}
		options.onAccepted?.();
		return message.id;
	}
	/** Refuse image content for a child whose fixed model accepts text only. */
	async assertImageCapable(agent, signal) {
		const { provider, model } = agent.options;
		if (provider === void 0 || model === void 0) return;
		const llm = this.ctx.get("llm");
		/* v8 ignore next -- without an LLM registry, delivery defers to projection. */
		if (llm === void 0) return;
		const info = await llm.resolveModelInfo(provider, model, signal);
		if (info.inputModalities !== void 0 && !info.inputModalities.includes("image")) throw new SubagentError(`Model "${model}" does not support image input.`, "MODEL_DOES_NOT_SUPPORT_IMAGES");
	}
	/** Resolve the persistence service continuable children require, or fail loud. */
	requirePersistence() {
		const persistence = this.ctx.get("sessionPersistence");
		if (persistence === void 0) throw new SubagentError("continuable subagents require session persistence (load a dsh-session-persistence backend)", "PERSISTENCE_UNAVAILABLE");
		return persistence;
	}
	/** Resolve the Session query service used for cold child observations. */
	requireSessionQuery() {
		const query = this.ctx.get("sessionQuery");
		if (query === void 0) throw new SubagentError("continuable subagents require session query (load @deepseek-ai/dsh-session-query)", "CONTINUATION_UNAVAILABLE");
		return query;
	}
};
//#endregion
//#region lib/types/list-children.js
/**
* Direct-child and recursive descendant discovery from parent-owned catalogs.
* Each catalog read releases its Session observation before the next branch.
* @module @deepseek-ai/dsh-subagent
*/
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
/**
* Read one parent's durable catalog through a live-preferred Session observation.
* @param ctx - context carrying the Session query service.
* @param parentSessionId - parent whose direct children are requested.
* @param signal - cancellation forwarded to the Session observation.
* @returns direct-child rows in parent catalog event order.
* @throws {@link SubagentError} when query or catalog projection is unavailable.
*/
async function listChildren(ctx, parentSessionId, signal) {
	const env_1 = {
		stack: [],
		error: void 0,
		hasError: false
	};
	try {
		const query = ctx.get("sessionQuery");
		if (query === void 0) throw new SubagentError("listing subagents requires the sessionQuery service (load @deepseek-ai/dsh-session-query)", "SUBAGENT_CONTROL_QUERY_UNAVAILABLE");
		const entries = __addDisposableResource(env_1, await query.observeSession(parentSessionId, { ...signal === void 0 ? {} : { signal } }), false).projections?.values.subagentCatalog;
		if (entries === void 0) throw new SubagentError("listing subagents requires the registered subagentCatalog projection", "SUBAGENT_CONTROL_PROJECTIONS_UNAVAILABLE");
		return entries;
	} catch (e_1) {
		env_1.error = e_1;
		env_1.hasError = true;
	} finally {
		__disposeResources(env_1);
	}
}
/**
* Walk reachable parent catalogs in stable pre-order without loading Agents.
* @see SubagentRuntime.listDescendants for failure and cancellation semantics.
* @param ctx - context carrying the Session store and query service.
* @param rootSessionId - parent whose catalog starts the traversal.
* @param signal - cancellation checked around each catalog read.
* @returns children and branch diagnostics with catalog parent and depth.
*/
async function listDescendants(ctx, rootSessionId, signal) {
	const sessions = ctx.get("sessions");
	if (sessions === void 0) throw new SubagentError("listing subagents requires the session store (load @deepseek-ai/dsh-session)", "SUBAGENT_CONTROL_SESSION_STORE_UNAVAILABLE");
	const readChildren = async (id) => {
		assertListingNotCancelled(signal);
		let children;
		try {
			children = await listChildren(ctx, id, signal);
		} catch (error) {
			assertListingNotCancelled(signal);
			throw error;
		}
		assertListingNotCancelled(signal);
		return children;
	};
	const stack = (await readChildren(rootSessionId)).map((entry) => ({
		entry,
		parentId: rootSessionId,
		depth: 1,
		parentPath: ""
	})).reverse();
	const visited = new Set([rootSessionId]);
	const result = [];
	for (let position = stack.pop(); position !== void 0; position = stack.pop()) {
		const { entry, parentId, depth, parentPath } = position;
		if (visited.has(entry.id)) continue;
		visited.add(entry.id);
		const path = joinAgentPath(parentPath, "name" in entry ? entry.name : void 0);
		const at = {
			parentId,
			depth,
			...path === void 0 ? {} : { path }
		};
		let children;
		try {
			children = entry.mode === "external" ? [] : await readChildren(entry.id);
		} catch (error) {
			if (error instanceof SubagentError) throw error;
			const code = error instanceof Error && "code" in error ? error.code : void 0;
			result.push({
				kind: "diagnostic",
				id: entry.id,
				...at,
				reason: code === "SESSION_QUERY_CORRUPT_SESSION" || code === "SESSION_QUERY_SOURCE_CONFLICT" ? "corrupt" : "unavailable"
			});
			continue;
		}
		if (entry.mode === "unknown") result.push({
			kind: "diagnostic",
			id: entry.id,
			...at,
			reason: "unsupported"
		});
		else {
			const { createdAt: _createdAt, ...identity } = entry;
			result.push({
				...identity,
				kind: "child",
				...at,
				activity: sessions.get(entry.id) === void 0 ? "inactive" : "running",
				hasChildren: children.length > 0
			});
		}
		for (const child of [...children].reverse()) stack.push({
			entry: child,
			parentId: entry.id,
			depth: depth + 1,
			parentPath: path
		});
	}
	return result;
}
/**
* Resolve a slash-separated name by reading only the catalogs along its path.
* @param ctx - context carrying the Session query service.
* @param rootSessionId - tree root whose catalog holds the first name.
* @param name - candidate full name relative to the root.
* @param signal - cancellation forwarded to each catalog read.
* @returns the named member's Session id, or undefined when the text is not a name in this tree.
* @throws {@link SubagentError} when a catalog dependency is unavailable; Session read failures propagate.
*/
async function resolveAgentPath(ctx, rootSessionId, name, signal) {
	const segments = name.split("/");
	if (!segments.every(isAgentName)) return void 0;
	let current = rootSessionId;
	for (const [index, segment] of segments.entries()) {
		const child = (await listChildren(ctx, current, signal)).find((entry) => "name" in entry && entry.name === segment);
		if (child === void 0) return void 0;
		if (index < segments.length - 1 && child.mode !== "continuable" && child.mode !== "one-shot") return void 0;
		current = child.id;
	}
	return current;
}
/** Stop the complete traversal when its caller cancels. */
function assertListingNotCancelled(signal) {
	if (signal?.aborted) throw new SubagentError("subagent listing was cancelled", "CANCELLED");
}
//#endregion
//#region lib/types/projection.js
/**
* Pure session projections for subagent identity (mode/label) and active-turn
* duration.
*
* @module @deepseek-ai/dsh-subagent/projection
*/
const activeIntervalSchema = z$1.object({
	since: z$1.number().int().nonnegative(),
	through: z$1.number().int().nonnegative()
}).strict();
const projectionSchema = z$1.object({
	settledMs: z$1.number().int().nonnegative(),
	active: activeIntervalSchema.optional(),
	lastTurnCompleted: z$1.boolean().optional()
}).strict().transform(({ settledMs, active, lastTurnCompleted }) => ({
	settledMs,
	...active === void 0 ? {} : { active },
	...lastTurnCompleted === void 0 ? {} : { lastTurnCompleted }
}));
/**
* Fold turn boundaries around the child's own durable descriptor.
*
* A fork seed may contain an ancestor descriptor and completed turns. Every
* descriptor therefore resets the accumulated state; the healthy catalog
* admits only a child with exactly one descriptor in its own suffix, making
* the final reset the child's authoritative timing origin.
*/
const subagentTimingProjectionDefinition = {
	key: "subagentTiming",
	stateSchema: z$1.object({
		settledMs: z$1.number().int().nonnegative(),
		active: activeIntervalSchema.optional(),
		pendingTurnStart: z$1.number().int().nonnegative().optional(),
		descriptorSeen: z$1.boolean(),
		lastTurnCompleted: z$1.boolean().optional()
	}).strict(),
	init: () => ({
		descriptorSeen: false,
		settledMs: 0
	}),
	apply: (state, event) => {
		if (event.type === "turn/start") {
			const { lastTurnCompleted: _closed, ...openState } = state;
			return state.descriptorSeen ? {
				...openState,
				active: {
					since: event.time,
					through: event.time
				}
			} : {
				...openState,
				pendingTurnStart: event.time
			};
		}
		if (event.type === "subagent/descriptor") {
			const activeSince = state.active?.since ?? state.pendingTurnStart;
			return {
				descriptorSeen: true,
				settledMs: 0,
				...activeSince === void 0 ? {} : { active: {
					since: activeSince,
					through: event.time
				} }
			};
		}
		if (event.type === "turn/end") {
			if (!state.descriptorSeen) {
				if (state.pendingTurnStart === void 0) return state;
				const { pendingTurnStart: _closed, ...next } = state;
				return next;
			}
			if (state.active === void 0) return state;
			const { active, ...rest } = state;
			return {
				...rest,
				settledMs: state.settledMs + Math.max(0, event.time - active.since),
				lastTurnCompleted: event.data.reason.kind === "completed"
			};
		}
		if (state.active === void 0) return state;
		return {
			...state,
			active: {
				...state.active,
				through: event.time
			}
		};
	},
	wire: {
		viewSchema: projectionSchema,
		view: (state) => ({
			settledMs: state.settledMs,
			...state.active === void 0 ? {} : { active: state.active },
			...state.lastTurnCompleted === void 0 ? {} : { lastTurnCompleted: state.lastTurnCompleted }
		})
	},
	stateVersion: 3
};
const identityValueSchema = z$1.discriminatedUnion("mode", [z$1.object({
	mode: z$1.literal("one-shot"),
	label: z$1.string().optional(),
	seq: z$1.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER).transform(SessionSeq)
}).strict(), z$1.object({
	mode: z$1.literal("continuable"),
	label: z$1.string(),
	seq: z$1.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER).transform(SessionSeq)
}).strict()]);
const identitySchema = identityValueSchema.nullable();
const identityStateSchema = z$1.object({ identity: identityValueSchema.optional() }).strict();
/** Interpret one `subagent/descriptor` event's identity; no value when the payload cannot be trusted. */
function descriptorIdentity(event) {
	let descriptor;
	try {
		descriptor = foldSubagentDescriptor([event]);
	} catch {
		descriptor = void 0;
	}
	if (descriptor === void 0) return void 0;
	return descriptor.mode === "one-shot" ? {
		mode: "one-shot",
		...descriptor.label !== void 0 ? { label: descriptor.label } : {},
		seq: event.seq
	} : {
		mode: "continuable",
		label: descriptor.label,
		seq: event.seq
	};
}
/**
* Fold the durable mode/label identity from `subagent/descriptor` events,
* last-wins: a fork seed may replay an ancestor's descriptor, and the child's
* own descriptor must override it — the same reset discipline as
* {@link subagentTimingProjectionDefinition}. A malformed or unknown-version
* payload resets to the `null` sentinel instead of throwing, so a fork of a
* healthy ancestor never inherits an identity its own descriptor failed to
* establish — and the reset survives every JSON push frame, so a consumer
* holding the earlier identity replaces it instead of keeping it stale;
* `null` ⟺ no valid descriptor, with the causes deliberately undistinguished.
*/
const subagentIdentityProjectionDefinition = {
	key: "subagent",
	stateSchema: identityStateSchema,
	init: () => ({}),
	apply: (state, event) => {
		if (event.type !== "subagent/descriptor") return state;
		const identity = descriptorIdentity(event);
		return identity === void 0 ? {} : { identity };
	},
	wire: {
		viewSchema: identitySchema,
		view: (state) => state.identity ?? null
	},
	stateVersion: 2
};
//#endregion
//#region lib/types/out-of-process.js
/**
* Provider-side vocabulary for OUT-OF-PROCESS subagent backends — the pieces
* that enforce this seam's own contracts around a child in another process:
* the no-capabilities advertisement, timing-bound validation, child
* working-directory validation, the never-reject result settlement, and the standard
* run-handle publication. Backends compose these with their own wire drivers;
* the process machinery itself (spawn, env scrub, managed-range teardown)
* belongs to the `dsh-subprocess` seam.
*
* @module @deepseek-ai/dsh-subagent/out-of-process
*/
/** Maximum UTF-8 size of {@link SubagentResult.diagnostic}. */
const MAX_SUBAGENT_DIAGNOSTIC_BYTES = 4096;
const DIAGNOSTIC_TRUNCATION_SUFFIX = "\n[diagnostic truncated]";
const utf8Encoder = new TextEncoder();
const utf8Decoder = new TextDecoder();
/**
* Limit provider-authored failure detail without splitting a UTF-8 sequence.
* @param diagnostic - safe diagnostic text produced by the provider.
* @returns the original text, or a visibly truncated value within the limit.
*/
function limitSubagentDiagnostic(diagnostic) {
	const bytes = utf8Encoder.encode(diagnostic);
	if (bytes.byteLength <= MAX_SUBAGENT_DIAGNOSTIC_BYTES) return diagnostic;
	let prefixBytes = MAX_SUBAGENT_DIAGNOSTIC_BYTES - utf8Encoder.encode(DIAGNOSTIC_TRUNCATION_SUFFIX).byteLength;
	while ((bytes[prefixBytes] & 192) === 128) prefixBytes -= 1;
	return utf8Decoder.decode(bytes.subarray(0, prefixBytes)) + DIAGNOSTIC_TRUNCATION_SUFFIX;
}
/** Enforce the byte limit on a provider-returned diagnostic. */
function normalizeSubagentDiagnostic(result) {
	return result.diagnostic === void 0 ? result : {
		...result,
		diagnostic: limitSubagentDiagnostic(result.diagnostic)
	};
}
/**
* The capability advertisement of an out-of-process backend: NONE. A child in
* another process cannot honor parent-enforced start features
* (`agentOptions`/`agentPreset`/`outputSchema`/`maxDepth`), so the service rejects a
* request needing any of them before `start` runs — never accepted-then-ignored.
*/
const NO_START_CAPABILITIES = Object.freeze({
	agentOptions: false,
	agentPreset: false,
	outputSchema: false,
	depthLimit: false
});
/**
* Assert a configured timing bound is a positive finite number (it bounds a
* teardown or shutdown wait; zero, negative, or NaN would skip or wedge it).
* @param prefix - the consuming plugin's diagnostic prefix (e.g. `subagent-acp`).
* @param name - the config field name, for the diagnostic.
* @param value - the configured value.
*/
function assertPositiveFinite(prefix, name, value) {
	if (!Number.isFinite(value) || value <= 0) throw new Error(`${prefix}: ${name} must be a positive finite number`);
}
/**
* Whether `path` names an existing directory the harness can ENTER. The
* search-permission probe matters: `statSync().isDirectory()` is true for a
* mode-600 directory, but a subprocess cwd needs `X_OK` or spawn fails EACCES.
*/
function isEnterableDirectory(path) {
	try {
		if (!statSync(path).isDirectory()) return false;
		accessSync(path, constants.X_OK);
		return true;
	} catch {
		return false;
	}
}
/**
* Assert `cwd` can actually host the child: absolute (it doubles as the
* child's workspace identity, and a relative path would be re-anchored to the
* server process's launch directory) and an existing directory (fail here,
* before the process boundary, instead of as an ambiguous spawn ENOENT).
* @param prefix - the consuming plugin's diagnostic prefix.
* @param label - which source supplied the value, for the diagnostic.
* @param cwd - the candidate working directory.
* @returns `cwd`, validated.
*/
function assertUsableCwd(prefix, label, cwd) {
	if (!isAbsolute(cwd)) throw new Error(`${prefix}: ${label} must be an absolute path: ${cwd}`);
	if (!isEnterableDirectory(cwd)) throw new Error(`${prefix}: ${label} is not an accessible directory: ${cwd}`);
	return cwd;
}
/** Normalize an unknown thrown value to an Error (the catch binding is `unknown`). */
function toError(value) {
	/* v8 ignore next */
	return value instanceof Error ? value : new Error(String(value));
}
/**
* Settle an out-of-process run result under the seam contract: `result` never
* rejects after publication. A normally completed or rejected attempt resolves
* as `aborted` when cancellation already settled locally; another rejection is
* flattened to `stopReason: 'error'` through the contained diagnostic sink.
* Provider-returned diagnostics use the same byte limit. The abort listener is
* removed on every path.
* @param parts - the attempt, output snapshot, cancellation state, sink, and signal wiring.
* @returns the terminal result (never a rejection).
*/
async function settleRunResult(parts) {
	try {
		const result = await parts.attempt();
		return parts.cancelled() ? {
			output: parts.collectOutput(),
			stopReason: "aborted"
		} : normalizeSubagentDiagnostic(result);
	} catch (error) {
		if (parts.cancelled()) return {
			output: parts.collectOutput(),
			stopReason: "aborted"
		};
		try {
			parts.onError?.(toError(error), "error");
		} catch {}
		const collected = parts.collectDiagnostic?.();
		const diagnostic = collected === void 0 ? void 0 : limitSubagentDiagnostic(collected);
		return {
			output: parts.collectOutput(),
			...diagnostic === void 0 ? {} : { diagnostic },
			stopReason: "error"
		};
	} finally {
		parts.signal.removeEventListener("abort", parts.onAbort);
	}
}
/**
* Publish the seam run handle for an out-of-process child. `dispose()` is
* idempotent (one memoized teardown): it removes the abort listener, settles
* local cancellation — there is no assumption the child cooperates — and then
* awaits the backend's teardown to actual exit.
* @param parts - the run identity, result, cancellation wiring, and teardown.
* @returns the external run handle.
*/
function subprocessRunHandle(parts) {
	let disposal;
	return {
		id: parts.id,
		result: parts.result,
		dispose() {
			if (disposal !== void 0) return disposal;
			parts.signal.removeEventListener("abort", parts.onAbort);
			parts.requestCancel();
			disposal = parts.teardown();
			return disposal;
		}
	};
}
//#endregion
//#region lib/types/index.js
var __runInitializers = function(thisArg, initializers, value) {
	var useValue = arguments.length > 2;
	for (var i = 0; i < initializers.length; i++) value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
	return useValue ? value : void 0;
};
var __esDecorate = function(ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
	function accept(f) {
		if (f !== void 0 && typeof f !== "function") throw new TypeError("Function expected");
		return f;
	}
	var kind = contextIn.kind, key = kind === "getter" ? "get" : kind === "setter" ? "set" : "value";
	var target = !descriptorIn && ctor ? contextIn["static"] ? ctor : ctor.prototype : null;
	var descriptor = descriptorIn || (target ? Object.getOwnPropertyDescriptor(target, contextIn.name) : {});
	var _, done = false;
	for (var i = decorators.length - 1; i >= 0; i--) {
		var context = {};
		for (var p in contextIn) context[p] = p === "access" ? {} : contextIn[p];
		for (var p in contextIn.access) context.access[p] = contextIn.access[p];
		context.addInitializer = function(f) {
			if (done) throw new TypeError("Cannot add initializers after decoration has completed");
			extraInitializers.push(accept(f || null));
		};
		var result = (0, decorators[i])(kind === "accessor" ? {
			get: descriptor.get,
			set: descriptor.set
		} : descriptor[key], context);
		if (kind === "accessor") {
			if (result === void 0) continue;
			if (result === null || typeof result !== "object") throw new TypeError("Object expected");
			if (_ = accept(result.get)) descriptor.get = _;
			if (_ = accept(result.set)) descriptor.set = _;
			if (_ = accept(result.init)) initializers.unshift(_);
		} else if (_ = accept(result)) if (kind === "field") initializers.unshift(_);
		else descriptor[key] = _;
	}
	if (target) Object.defineProperty(target, contextIn.name, descriptor);
	done = true;
};
/** Named provider registry with managed activations, durable discovery, and local child messaging. */
let SubagentRuntime = (() => {
	let _classSuper = TypertRemoteService;
	let _instanceExtraInitializers = [];
	let _prompt_decorators;
	let _interruptByUser_decorators;
	return class SubagentRuntime extends _classSuper {
		static {
			const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
			_prompt_decorators = [Remote({
				name: "prompt",
				resources: { session: (request) => request.childSessionId }
			})];
			_interruptByUser_decorators = [Remote({
				name: "interruptByUser",
				resources: { session: (childSessionId) => childSessionId }
			})];
			__esDecorate(this, null, _prompt_decorators, {
				kind: "method",
				name: "prompt",
				static: false,
				private: false,
				access: {
					has: (obj) => "prompt" in obj,
					get: (obj) => obj.prompt
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			__esDecorate(this, null, _interruptByUser_decorators, {
				kind: "method",
				name: "interruptByUser",
				static: false,
				private: false,
				access: {
					has: (obj) => "interruptByUser" in obj,
					get: (obj) => obj.interruptByUser
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			if (_metadata) Object.defineProperty(this, Symbol.metadata, {
				enumerable: true,
				configurable: true,
				writable: true,
				value: _metadata
			});
		}
		config = __runInitializers(this, _instanceExtraInitializers);
		static Config = z.object({
			maxDepth: z.number().step(1).min(0).max(Number.MAX_SAFE_INTEGER).default(1).volatile(),
			maxActiveSubagents: z.number().step(1).min(1).max(Number.MAX_SAFE_INTEGER).default(8).volatile()
		});
		static inject = ["workingDirectory", "sessionProjections"];
		providers = /* @__PURE__ */ new Map();
		manager;
		/**
		* The contained lifecycle-edge publisher. Built here because scoped dispatch
		* keys its carrier by this exact service instance, whose own context filter
		* composes into the carrier.
		*/
		emitLifecycle;
		constructor(ctx, config) {
			super(ctx, "subagents");
			this.config = config;
			this.emitLifecycle = createLifecycleEmitter(this.ctx, (parent) => scopeTarget(this, parent));
		}
		async [Service.init]() {
			const ctx = this.ctx;
			const catalogProjection = await ctx.sessionProjections.register(subagentCatalogProjectionDefinition);
			await ctx.sessionProjections.register(subagentTimingProjectionDefinition);
			const identityProjection = await ctx.sessionProjections.register(subagentIdentityProjectionDefinition);
			if (ctx.fiber.uid === null) return;
			ctx.inject(["agents"], (childCtx) => {
				const manager = new SubagentManager(childCtx, {
					identityOf: (session) => identityProjection.stateOf(session).identity,
					catalogOf: (session) => subagentCatalogProjectionDefinition.wire.view(catalogProjection.stateOf(session)),
					emitLifecycle: this.emitLifecycle
				}, () => this.config.maxActiveSubagents.get());
				this.manager = manager;
				childCtx.effect(() => () => {
					/* v8 ignore else -- one injected binding owns the slot until its fiber disposes. */
					if (this.manager === manager) this.manager = void 0;
				}, "subagents.managerBinding()");
			});
		}
		/**
		* Resolve a delegation tool's depth policy against the current user setting.
		* @param configured - Explicit tool limit, or provider-managed for external delegation.
		* @returns The numeric limit, or undefined when the provider owns depth enforcement.
		*/
		resolveMaxDepth(configured) {
			if (configured === "provider-managed") return void 0;
			if (configured !== void 0) return configured;
			const depth = this.config.maxDepth.get();
			assertSubagentMaxDepth(depth);
			return depth;
		}
		/**
		* Establish a managed child with backend-specific execution capabilities.
		* @param spec - task, backend, and result recipient.
		* @returns identities, execution result, and exact-activation disposal. A reserved
		* childId requires a local backend and guarantees an accepted messageId.
		* @throws PROVIDER_CHANGED if the selected provider is removed or replaced before dispatch.
		*/
		async startActivation(spec) {
			const provider = this.expectProvider(spec.provider);
			if (spec.childId !== void 0 && provider.prepareContinuable === void 0) throw new SubagentError("reserved child ids require a local backend", "UNSUPPORTED_CAPABILITY");
			assertSubagentMaxDepth(spec.maxDepth);
			this.assertCapabilities(provider, spec);
			const manager = this.requireManager();
			if (this.ctx.get("agents")?.get(spec.parent.id) !== spec.parent) throw new SubagentError("subagent creation requires the exact live parent agent", "UNAUTHORIZED");
			if (spec.outputSchema !== void 0) assertObjectJsonSchema(spec.outputSchema);
			const prepare = provider.prepareContinuable?.bind(provider);
			if (prepare !== void 0) return manager.startLocal(spec, (request) => {
				this.assertProviderCurrent(provider);
				return prepare(request);
			});
			const start = provider.start.bind(provider);
			return manager.startExternal(spec, (request) => {
				this.assertProviderCurrent(provider);
				return start(request);
			});
		}
		/**
		* Resolve the root shared by an Agent and its other agents in its subagent tree.
		* @param caller - exact live Agent whose subagent ancestry selects the tree.
		* @returns the live root; ordinary Session forks lead independent trees.
		* @throws when the caller or its runtime owner is unavailable.
		*/
		rootOf(caller) {
			return this.requireManager().rootOf(caller);
		}
		/**
		* Report the model-facing names of a live tree member and its direct creator.
		* The root is `lead`; a descendant is the slash-separated path of names from
		* the root, or its Session id when it or a creator below the root has no name.
		* @param agent - exact live root or resident tree member.
		* @returns the member's name and, for a descendant, its creator's name.
		* @throws when the caller or its runtime owner is unavailable.
		*/
		namesOf(agent) {
			return this.requireManager().namesOf(agent);
		}
		/**
		* Resolve a model-supplied agent reference within the caller's tree. `lead`
		* selects the root and a slash-separated path is looked up through the
		* catalogs along it. Any other text that is not a name in the tree is
		* returned as a Session id, because a single token can be one; a
		* slash-separated address never is, so an unmatched path fails instead of
		* reaching a member lookup as an unknown id. Resolution grants no authority:
		* messaging, interruption, and assignment authorize the result.
		* @param caller - exact live Agent whose tree root anchors the lookup.
		* @param name - `lead`, a full name from the root, or a Session id.
		* @param signal - cancellation forwarded to each catalog read.
		* @returns the referenced Session id.
		* @throws {@link SubagentError} `AGENT_NAME_NOT_FOUND` when a path names no member;
		*   when the caller's root is unavailable; or when a catalog along the path cannot be read.
		*/
		async resolveName(caller, name, signal) {
			const root = this.rootOf(caller);
			if (name === "lead") return root.id;
			const resolved = await resolveAgentPath(this.ctx, root.id, name, signal);
			if (resolved !== void 0) return resolved;
			if (name.includes("/")) throw new SubagentError(`agent name ${JSON.stringify(name)} was not found in this subagent tree; use list_agents to see the names`, "AGENT_NAME_NOT_FOUND");
			return brandString(name);
		}
		/**
		* Join progressing subagent tree work for its root. Idle members with parked input
		* stay resident without delaying host completion. Ordinary members return false.
		* @param root - exact live subagent tree root whose member work is awaited.
		* @returns whether work was joined; hosts recheck root idle after true.
		*/
		async waitForTreeWork(root) {
			return this.manager?.waitForTreeWork(root) ?? false;
		}
		/**
		* Steer one model-authored message to another member of the same subagent tree.
		* A running target receives it at its next step; an idle target starts a turn.
		* A cold target resumes under its runtime subagent tree root without restoring
		* its creation ancestors. Sender identity is rechecked before inbox acceptance; cancellation
		* stops only pre-acceptance work and never retracts an accepted message.
		* @param sender - exact live root or resident continuable Agent originating the message.
		* @param targetId - another root or continuable member's durable Session id.
		* @param content - model-authored content to deliver.
		* @param options - caller cancellation before inbox acceptance.
		* @returns the accepted message's inbox id.
		* @throws when the target is outside the tree, cannot resume, or rejects input.
		*/
		async sendMessage(sender, targetId, content, options) {
			return this.requireManager().sendMessage(sender, targetId, content, options);
		}
		/**
		* Interrupt one live child's current execution under a trusted user
		* operation or an exact live ancestor Agent. Fire-and-return: the cancel
		* signal is issued before this returns, but the target may keep running
		* until it observes the signal. Unclaimed pending inbox work, the Activation,
		* and published descendants are preserved; claimed work is not requeued.
		* Once the interrupted Agent is idle, a waking send resumes the parked FIFO
		* queue. External backends stop their single execution. An absent target
		* is an accepted no-op, as is a manager-less composition, which cannot own a
		* live Activation.
		* @param targetSessionId - the durable child session id to interrupt.
		* @param authority - a trusted user operation or exact live ancestor Agent.
		* @throws {SubagentError} `UNAUTHORIZED` when the authority does not own the
		*   live target.
		*/
		interrupt(targetSessionId, authority) {
			this.manager?.interrupt(targetSessionId, authority);
		}
		/**
		* Close admission and stop all instances in the selected subagent trees, then await
		* pending startup and cleanup. Ordinary members are ignored. The cutoff lasts
		* until each exact root leaves the registry; unrelated subagent trees remain live.
		* @param parents - exact live subagent tree roots entering teardown.
		* @returns once all admitted startup and member cleanup have settled.
		* @throws an aggregate error after all branches settle when any failed.
		*/
		async drainDescendants(parents) {
			const manager = this.manager;
			if (manager === void 0) return;
			await manager.drainDescendants(parents);
		}
		/**
		* Recursively list reachable parent catalogs in stable pre-order, preserving
		* each catalog's event order. Each row carries its catalog parent and depth;
		* external children are leaves; one-shot and unknown-mode children remain
		* traversal nodes. Unknown modes
		* produce unsupported diagnostics. Unreadable child catalogs produce corrupt
		* or unavailable diagnostics and stop only that branch. Root read failures,
		* missing services or projections, and cancellation reject the whole listing.
		* Each catalog is observed once and released before the next read. No Agent
		* is loaded or resumed; Sessions absent from reachable catalogs are omitted.
		* @param rootSessionId - session whose catalog starts descendant discovery.
		* @param signal - cancellation forwarded to and checked around each catalog read.
		* @returns children and branch diagnostics in parent-catalog pre-order.
		* @throws {@link SubagentError} when listing dependencies are unavailable or the caller cancels.
		* @throws SessionQueryError when the root catalog cannot be read.
		*/
		listDescendants(rootSessionId, signal) {
			return listDescendants(this.ctx, rootSessionId, signal);
		}
		/**
		* Deliver one browser-authored message to a continuable child through its
		* live subagent tree root, retaining the caller-minted request identity and
		* validated browser zone on the accepted message. Success identifies the
		* message the child's inbox accepted; later execution is independent of this
		* call. Queue delivery targets a later turn; steer delivery targets the
		* nearest step and retains the Agent loop's best-effort fallback semantics.
		* Image parts are admitted and persisted through the attachment store
		* before delivery, and the child's model must accept image input.
		* Cold resume at capacity rejects with `subagent/delivery-unavailable`.
		* Access is checked on the target Session before attachment admission or restoration.
		* @param request - target id, delivery, minted identity, content, and optional browser zone.
		* @param signal - carrier cancellation, owning the call until inbox acceptance.
		* @returns the accepted message's inbox identity.
		* @throws {RemoteError} `gateway/bad-request`, `subagent/attachment-invalid`,
		*   `subagent/invalid-time-zone`,
		*   `subagent/not-resumable`, `subagent/unauthorized`,
		*   `subagent/delivery-unavailable`, `gateway/cancelled`, or `gateway/internal`.
		*/
		async prompt(request, signal) {
			const { childSessionId, clientTimeZone, delivery } = request;
			validateControlRequest("subagent.prompt", request);
			const canonicalTimeZone = clientTimeZone === void 0 ? void 0 : canonicalClientTimeZone(clientTimeZone);
			if (clientTimeZone !== void 0 && canonicalTimeZone === void 0) throw new RemoteError("subagent/invalid-time-zone", "clientTimeZone must be UTC or a valid IANA Area/Location name", { value: clientTimeZone });
			const principal = principalOf(this.ctx.invocation);
			const source = {
				kind: "user",
				rpcId: request.requestId,
				...canonicalTimeZone === void 0 ? {} : { clientTimeZone: canonicalTimeZone },
				...principal === void 0 ? {} : { principal }
			};
			try {
				let content;
				if (request.content.every((part) => part.type === "text")) content = request.content.map((part) => ({
					type: "text",
					text: part.text
				}));
				else {
					const attachments = this.ctx.get("attachments");
					if (attachments === void 0) throw new Error("subagent image prompt requires an attachment store");
					content = await attachments.admitPromptContent(request.content);
				}
				return { messageId: await this.requireManager().deliverPrompt(childSessionId, content, {
					source,
					signal,
					delivery
				}) };
			} catch (error) {
				return rejectPrompt(error, childSessionId, signal);
			}
		}
		/**
		* Remote face of {@link interrupt}, authorized on the target Session by the Gateway.
		* No catalog, history, persistence, or parent Agent lookup runs.
		* Absent, idle, and already-completed targets are accepted no-ops there.
		* @param childSessionId - durable child session id to interrupt.
		* @returns acknowledgement that the cancel signal was admitted, not that the target is quiescent.
		* @throws {RemoteError} `gateway/bad-request` for an empty id,
		*   otherwise `gateway/internal`.
		*/
		interruptByUser(childSessionId) {
			validateControlRequest("subagent.interrupt", { childSessionId });
			try {
				this.interrupt(childSessionId, { kind: "user" });
			} catch (error) {
				throw new RemoteError("gateway/internal", "subagent interrupt failed", {}, { cause: error });
			}
			return { accepted: true };
		}
		/**
		* Register a provider under its name. Registration is effect-scoped and HMR
		* safe; removing a provider blocks new starts but does not revoke runs that
		* were already returned to their holders. Providers without either execution
		* method are rejected with UNSUPPORTED_CAPABILITY before registration.
		* @param provider - the trusted provider implementation.
		* @returns the exact Cordis effect disposer.
		*/
		registerProvider(provider) {
			const name = provider.name;
			if (provider.start === void 0 && provider.prepareContinuable === void 0) throw new SubagentError(`subagent provider "${name}" must implement start or prepareContinuable`, "UNSUPPORTED_CAPABILITY");
			return this.ctx.effect(function* () {
				if (this.providers.has(name)) throw new SubagentError(`a subagent provider named "${name}" is already registered`, "DUPLICATE_PROVIDER");
				this.providers.set(name, provider);
				yield () => {
					this.providers.delete(name);
					this.emitLifecycle("subagent/provider-removed", name);
				};
				this.ctx.emit("subagent/provider-added", provider);
			}.bind(this), "subagents.registerProvider()");
		}
		/**
		* Look up a provider by name.
		* @param name - the provider name.
		* @returns the provider, or undefined when absent.
		*/
		getProvider(name) {
			return this.providers.get(name);
		}
		/** A pending start must use the exact provider whose capabilities were checked. */
		assertProviderCurrent(provider) {
			if (this.providers.get(provider.name) !== provider) throw new SubagentError(`subagent provider "${provider.name}" changed during startup; retry the delegation`, "PROVIDER_CHANGED");
		}
		/** Look up a provider for dispatch or fail loud. */
		expectProvider(name) {
			const provider = this.providers.get(name);
			if (provider === void 0) throw new SubagentError(`no subagent provider registered for "${name}"`, "NO_PROVIDER");
			return provider;
		}
		/** Resolve the subagent manager or fail loud. */
		requireManager() {
			if (this.manager === void 0) throw new SubagentError("continuable subagents require the agents service", "CONTINUATION_UNAVAILABLE");
			return this.manager;
		}
		/** Reject the first requested capability that the provider lacks. */
		assertCapabilities(provider, request) {
			if (request.agentPreset !== void 0 && (!provider.capabilities.agentPreset || provider.inheritsParentContext)) throw new SubagentError(`subagent provider "${provider.name}" does not support independent agent preset selection`, "UNSUPPORTED_CAPABILITY");
			const needs = [
				{
					when: request.agentOptions !== void 0,
					cap: "agentOptions"
				},
				{
					when: request.outputSchema !== void 0,
					cap: "outputSchema"
				},
				{
					when: request.maxDepth !== void 0,
					cap: "depthLimit"
				}
			];
			for (const { when, cap } of needs) if (when && !provider.capabilities[cap]) throw new SubagentError(`subagent provider "${provider.name}" does not support the "${cap}" capability`, "UNSUPPORTED_CAPABILITY");
		}
	};
})();
//#endregion
export { AssistantOutputFold, NO_START_CAPABILITIES, ROOT_AGENT_NAME, STRUCTURED_OUTPUT_TOOL, SUBAGENT_DESCRIPTOR_VERSION, SubagentDepthError, SubagentError, SubagentRunId, SubagentRuntime, SubagentRuntime as default, appendDelegatedPolicyOverrides, applyChildComposition, assertPositiveFinite, assertSubagentMaxDepth, assertUsableCwd, captureDelegatedPolicyOverrides, childSessionMeta, delegationDepthOf, finalAssistantOutput, foldSubagentDescriptor, parentAgentOptionsForDelegation, resolveChildAgentOptions, resolveChildDepth, settleRunResult, snapshotSubagentDescriptor, subprocessRunHandle };
