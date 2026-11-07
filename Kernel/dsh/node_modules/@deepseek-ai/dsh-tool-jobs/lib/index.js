import z from "@deepseek-ai/schemastery";
import { boundContextSummary, createUserMessage } from "@deepseek-ai/dsh-llm";
import { TextRetainer } from "@deepseek-ai/dsh-output-retention";
import { defineTool } from "@deepseek-ai/dsh-tools";
import { JobId } from "@deepseek-ai/dsh-jobs";
//#region lib/types/render.js
/**
* Model-facing rendering of registry reads: the consuming delta as the
* shell tools have always shown it (stdout, then one marked stderr section),
* the `[status: …]` line, and the public job projection the tool schemas
* expose.
* @module @deepseek-ai/dsh-tool-jobs/render
*/
/**
* The one-line qualifier the model reads beside a status: live progress while
* the job runs, the terminal reason once it settled.
* @param job - the job projection.
* @returns the qualifier, or undefined when the job carries neither.
*/
function jobDetail(job) {
	return job.progress ?? job.detail;
}
/**
* Remove ownership, offsets, and limits from a registry projection.
* @param job - the registry projection.
* @returns the model-safe projection.
*/
function publicJob(job) {
	const detail = jobDetail(job);
	return {
		id: job.id,
		kind: job.kind,
		label: job.label,
		status: job.status,
		...detail !== void 0 ? { detail } : {},
		startedAt: job.startedAt,
		...job.finishedAt !== void 0 ? { finishedAt: job.finishedAt } : {}
	};
}
/**
* Render generic status with optional detail.
* @param snapshot - job state to render.
* @returns a bracketed status line.
*/
function statusLine(snapshot) {
	return snapshot.detail !== void 0 ? `[status: ${snapshot.status}, ${snapshot.detail}]` : `[status: ${snapshot.status}]`;
}
/**
* Render one consuming read for the model: stdout and unlabeled chunks in
* order, then every stderr chunk in one `[stderr]` section, exactly as the
* shell tools render a foreground result. `log` chunks are producer
* narration for observers and never reach the model. Lost bytes — the cursor
* fell behind the ring's retention, or a model-visible chunk carries a
* producer-side gap — end the read with the shell tools' dropped-output
* notice, naming the spill files the job's sources keep.
* @param chunks - the chunks since the model cursor, in offset order.
* @param lossy - whether bytes before `chunks` were evicted unread.
* @param spillPaths - the complete-stream files the job currently advertises (`JobView.output.spillPaths`).
* @returns the delta text, possibly empty.
*/
function renderModelDelta(chunks, lossy, spillPaths) {
	const visible = chunks.filter((chunk) => chunk.channel !== "log");
	const out = visible.filter((chunk) => chunk.channel !== "stderr").map((chunk) => chunk.text).join("");
	const err = visible.filter((chunk) => chunk.channel === "stderr").map((chunk) => chunk.text).join("");
	const separator = out.length > 0 && !out.endsWith("\n") ? "\n" : "";
	const body = out + (err.length > 0 ? `${separator}[stderr]\n${err}` : "");
	if (!lossy && !visible.some((chunk) => chunk.gapBefore === true)) return body;
	const notice = `[some output was dropped from memory; full output: ${spillPaths.length > 0 ? spillPaths.join(", ") : "(unavailable)"}]`;
	return `${body}${body.length > 0 && !body.endsWith("\n") ? "\n" : ""}${notice}`;
}
//#endregion
//#region lib/types/index.js
/**
* Model-facing `job_output`, `job_list`, and `job_kill` tools over
* `ctx.jobs`. Loading the plugin attaches the controller required by
* producers. It also delivers completions the model has not already
* collected to the owning agent: injected into a busy owner's next step, or
* opening a turn on an idle one under the default `wakeup` delivery, unbounded
* unless `maxConsecutiveWakes` caps it per owner. A notice carries the job's
* output and result when it fits `inlineOutputBytes`, and `job_output`
* withdraws a notice still pending.
* @module @deepseek-ai/dsh-tool-jobs
*/
const name = "tool-jobs";
const inject = [
	"tools",
	"jobs",
	"systemPrompt"
];
const Config = z.object({
	waitTimeoutMs: z.number().min(1).default(3e4),
	maxWaitTimeoutMs: z.number().min(1).default(6e5),
	completionDelivery: z.union(["quiet", "wakeup"]).default("wakeup"),
	maxConsecutiveWakes: z.number().min(1),
	inlineOutputBytes: z.natural().default(2048)
});
/** Shared schema for job-control outputs. */
const PUBLIC_JOB_SCHEMA = {
	type: "object",
	additionalProperties: false,
	properties: {
		id: {
			type: "string",
			required: true
		},
		kind: {
			type: "string",
			required: true
		},
		label: {
			type: "string",
			required: true
		},
		status: {
			type: "string",
			required: true,
			enum: [
				"running",
				"stopping",
				"completed",
				"killed",
				"failed"
			]
		},
		detail: { type: "string" },
		startedAt: {
			type: "integer",
			required: true
		},
		finishedAt: { type: "integer" }
	}
};
const encoder = new TextEncoder();
function retainTail(text, maxBytes) {
	const retainer = new TextRetainer({
		kind: "tail",
		maxBytes
	});
	retainer.push(text);
	return retainer.finish().text;
}
function retainHead(text, maxBytes) {
	const retainer = new TextRetainer({
		kind: "head",
		maxBytes
	});
	retainer.push(text);
	return retainer.finish().text;
}
function fitWithSuffix(content, suffix, maxBytes, omitted) {
	const complete = `${content}${suffix}`;
	if (maxBytes === void 0 || encoder.encode(complete).byteLength <= maxBytes) return complete;
	const fixed = `${content.endsWith(omitted.trimStart()) ? "" : omitted}${suffix}`;
	const fixedBytes = encoder.encode(fixed).byteLength;
	if (fixedBytes >= maxBytes) return retainTail(fixed, maxBytes);
	return `${retainTail(content, maxBytes - fixedBytes)}${fixed}`;
}
/**
* One-line account of a settled job for the `notice` form's collapsed row.
* @param job - the settled job.
* @returns its kind, label, and status, bounded like every notice summary.
*/
function completionSummary(job) {
	return boundContextSummary(`${job.kind} ${job.label} ${statusLine(publicJob(job))}`);
}
/**
* The job's label as a notice names it: the first non-empty line, bounded
* like a notice summary. The model wrote the full command and holds the job id.
* @param job - the settled job.
* @returns ` (kind: label) finished [status: …]`.
*/
function noticeDetail(job) {
	const text = job.label.trimStart();
	const newline = text.indexOf("\n");
	const label = boundContextSummary(newline === -1 ? text : `${text.slice(0, newline)}…`);
	return ` (${job.kind}: ${label}) finished ${statusLine(publicJob(job))}`;
}
/**
* The notice that carries the job's output and final result, when the
* complete text fits both `maxBytes` and the producer's cap.
* @param job - the settled job.
* @param output - the consuming read taken at settlement.
* @param maxBytes - the configured inline bound.
* @returns the notice text, or undefined when it does not fit.
*/
function inlineCompletionNotice(job, output, maxBytes) {
	const head = `background job ${job.id}${noticeDetail(job)}`;
	const text = output.length === 0 ? `${head}. No new output.` : `${head}. Output:\n${output.replace(/\n$/, "")}`;
	const bytes = encoder.encode(text).byteLength;
	return bytes <= maxBytes && bytes <= (job.outputLimitBytes ?? Infinity) ? text : void 0;
}
function fitCompletionNotice(job) {
	const prefix = `background job ${job.id}`;
	const detail = noticeDetail(job);
	const action = "\nDone; job_output.";
	const complete = `${prefix}${detail}. Read its output with job_output.`;
	const maxBytes = job.outputLimitBytes;
	if (maxBytes === void 0 || encoder.encode(complete).byteLength <= maxBytes) return complete;
	const omitted = "\n[notice truncated]";
	const fixed = `${prefix}${omitted}${action}`;
	const fixedBytes = encoder.encode(fixed).byteLength;
	if (fixedBytes <= maxBytes) return fixedBytes === maxBytes ? fixed : `${prefix}${retainHead(detail, maxBytes - fixedBytes)}${omitted}${action}`;
	const compact = `${prefix}${action}`;
	if (encoder.encode(compact).byteLength <= maxBytes) return compact;
	const recovery = "background job ended; use job_list for id.";
	if (encoder.encode(recovery).byteLength <= maxBytes) return recovery;
	return retainHead("job_list", maxBytes);
}
function rawSingleText(content) {
	if (content.length !== 1) return void 0;
	const block = content[0];
	if (block?.type !== "text") return void 0;
	return block.text;
}
function boundSingleText(content, maxBytes) {
	const text = rawSingleText(content);
	if (text === void 0) return void 0;
	return [{
		type: "text",
		text: fitWithSuffix(text, "", maxBytes, "\n[result truncated]")
	}];
}
/** The producer's cap for the job a `job_output` or `job_kill` call names, when it is visible to the caller. */
function visibleOutputLimit(ctx, exec) {
	if (exec.name !== "job_output" && exec.name !== "job_kill") return void 0;
	const jobId = exec.arguments?.job_id;
	if (typeof jobId !== "string" || jobId.length === 0) return void 0;
	return ctx.jobs.list(exec.agent?.id).find((job) => job.id === jobId)?.outputLimitBytes;
}
/** Validate the non-empty constraint that ParameterSchemaSpec cannot express. */
function validateJobId(value) {
	if (value.length === 0) throw new Error(`invalid job_id: expected a non-empty string, got ${JSON.stringify(value)}`);
	return JobId(value);
}
/** Pending presentation shared by the three generic job controls. */
function presentJobCall(title, kind, rawInput) {
	return {
		card: "generic",
		title,
		kind,
		...rawInput !== void 0 ? { rawInput } : {}
	};
}
/** The consuming read as the model sees it: the delta, then the result once, then the status line. */
function readBody(read) {
	const delta = renderModelDelta(read.chunks, read.lossy, read.job.output.spillPaths ?? []);
	return {
		text: read.result === void 0 ? delta : `${delta}${delta.length > 0 && !delta.endsWith("\n") ? "\n" : ""}${read.result}`,
		job: publicJob(read.job)
	};
}
function apply(ctx, config) {
	const waitDefault = config.waitTimeoutMs ?? 3e4;
	const waitCap = config.maxWaitTimeoutMs ?? 6e5;
	const delivery = config.completionDelivery ?? "wakeup";
	const wakeBudget = config.maxConsecutiveWakes;
	const inlineBytes = config.inlineOutputBytes ?? 2048;
	const spentWakes = /* @__PURE__ */ new WeakMap();
	if (waitDefault > waitCap) throw new Error(`tool-jobs: waitTimeoutMs (${waitDefault}) exceeds maxWaitTimeoutMs (${waitCap})`);
	if (wakeBudget !== void 0 && !Number.isSafeInteger(wakeBudget)) throw new Error(`tool-jobs: maxConsecutiveWakes (${wakeBudget}) must be a whole number of turns`);
	const pending = /* @__PURE__ */ new Map();
	if (delivery === "wakeup" && wakeBudget !== void 0) ctx.on("agent/inbox/claimed", ({ agent, message }) => {
		if (message.source.kind === "user") spentWakes.delete(agent);
	});
	ctx.on("session/event", (session, event) => {
		if (event.type !== "user/message" || event.data.source.kind !== "tool-jobs") return;
		for (const [id, notice] of pending) {
			if (notice.messageId !== event.data.id || notice.owner !== session.id) continue;
			if (notice.inline || notice.held === void 0) pending.delete(id);
		}
	});
	const outputLimits = /* @__PURE__ */ new WeakMap();
	ctx.on("tools/pre-execute", (exec, next) => {
		const maxBytes = visibleOutputLimit(ctx, exec);
		if (maxBytes !== void 0) outputLimits.set(exec, maxBytes);
		return next();
	}, { prepend: true });
	const finalizeJobContent = (exec, result) => {
		const maxBytes = outputLimits.get(exec) ?? visibleOutputLimit(ctx, exec);
		outputLimits.delete(exec);
		if (maxBytes === void 0) return void 0;
		if (exec.name === "job_output" && !result.isError) {
			const value = result.value;
			const body = value.text.length > 0 ? value.text : "(no new output)";
			const content = body.endsWith("\n") ? body.slice(0, -1) : body;
			const suffix = `\n${statusLine(value.job)}`;
			if (rawSingleText(result.content) === `${content}${suffix}`) return [{
				type: "text",
				text: fitWithSuffix(content, suffix, maxBytes, "\n[output truncated]")
			}];
		}
		return boundSingleText(result.content, maxBytes);
	};
	ctx.jobs.attachController("tool-jobs");
	ctx.systemPrompt.section({
		name: "tool:jobs",
		order: ctx.systemPrompt.getSectionOrder("TOOL_JOBS"),
		text: "Track every background job id you start. You are notified in-session when a job finishes — do not busy-poll or sleep on one; keep working on independent steps and do not duplicate a running job's work. Before giving a final answer, collect every still-relevant job with job_output (set wait: true only when you are genuinely blocked on it), and job_kill jobs that stopped mattering."
	});
	const killedByModel = /* @__PURE__ */ new Set();
	const releasedReads = /* @__PURE__ */ new Map();
	/** A settlement a released read is expected to carry; delivered at once if that read never commits. */
	const suppressedSettlements = /* @__PURE__ */ new Map();
	const beginReleasedRead = (id) => {
		const claim = releasedReads.get(id) ?? { pending: 0 };
		releasedReads.set(id, claim);
		claim.pending += 1;
		return () => {
			claim.pending -= 1;
			if (claim.pending !== 0) return;
			releasedReads.delete(id);
			const pending = suppressedSettlements.get(id);
			if (pending === void 0) return;
			suppressedSettlements.delete(id);
			deliverSettlement(pending);
		};
	};
	/**
	* Deliver one settlement that no caller already collected.
	* @param event - the settlement to announce.
	*/
	const deliverSettlement = (event) => {
		if (event.cause === "teardown" || event.job.owner === void 0) return;
		const owner = ctx.get("agents")?.get(event.job.owner);
		if (owner === void 0) return;
		const held = inlineBytes > 0 && event.job.output.total <= inlineBytes ? readBody(ctx.jobs.read(event.job.id, event.job.owner)).text : void 0;
		const inline = held === void 0 ? void 0 : inlineCompletionNotice(event.job, held, inlineBytes);
		const message = createUserMessage({
			content: [{
				type: "text",
				text: inline ?? fitCompletionNotice(event.job)
			}],
			source: {
				kind: "tool-jobs",
				form: "notice",
				summary: completionSummary(event.job)
			}
		});
		pending.set(event.job.id, {
			owner: event.job.owner,
			messageId: message.id,
			inline: inline !== void 0,
			...held === void 0 ? {} : { held }
		});
		if (delivery === "wakeup" && owner.status === "idle") {
			if (wakeBudget === void 0) {
				owner.followup(message);
				return;
			}
			const spent = spentWakes.get(owner) ?? 0;
			if (spent < wakeBudget) {
				spentWakes.set(owner, spent + 1);
				owner.followup(message);
				return;
			}
		}
		owner.inject(message);
	};
	ctx.jobs.events.subscribe({ owners: "scope" }, (event) => {
		if (event.type === "removed") {
			killedByModel.delete(event.job.id);
			pending.delete(event.job.id);
			return;
		}
		if (event.type !== "settled") return;
		if (killedByModel.delete(event.job.id) || event.awaited) return;
		if (releasedReads.has(event.job.id)) {
			suppressedSettlements.set(event.job.id, event);
			return;
		}
		deliverSettlement(event);
	});
	ctx.tools.register(defineTool({
		name: "job_output",
		description: "Read a background job: output since the previous read for stream jobs, or the result of a finished final-output job.",
		parameters: {
			job_id: {
				type: "string",
				required: true,
				description: "Job id returned by the tool that started the background work."
			},
			wait: {
				type: "boolean",
				description: "Block until the job finishes or the timeout expires; a timed-out wait leaves the job running. Defaults to false."
			},
			timeout_ms: {
				type: "number",
				description: "Max wait in milliseconds with wait: true. Defaults to and is capped by configuration."
			}
		},
		finalizeContent: finalizeJobContent,
		output: {
			schema: {
				type: "object",
				additionalProperties: false,
				properties: {
					text: {
						type: "string",
						required: true
					},
					job: {
						...PUBLIC_JOB_SCHEMA,
						required: true
					}
				}
			},
			render: (_args, value) => {
				const body = value.text.length > 0 ? value.text : "(no new output)";
				return [{
					type: "text",
					text: `${body}${body.endsWith("\n") ? "" : "\n"}${statusLine(value.job)}`
				}];
			}
		},
		async execute(args, exec) {
			const id = validateJobId(args.job_id);
			const jobs = ctx.jobs;
			const agent = exec.agent;
			let endReleasedRead;
			let releasedBy;
			try {
				if (args.wait === true) {
					const input = new AbortController();
					const checkInput = () => {
						if (agent === void 0 || !agent.inbox.nextStep.some((message) => message.source.kind === "user")) return;
						if (endReleasedRead === void 0) {
							endReleasedRead = beginReleasedRead(id);
							releasedBy = "user steered";
						}
						input.abort();
					};
					const unsubscribe = agent === void 0 ? () => {} : ctx.on("agent/inbox/inserted", ({ agent: subject }) => {
						if (subject === agent) checkInput();
					});
					try {
						checkInput();
						await jobs.wait(id, Math.min(args.timeout_ms ?? waitDefault, waitCap), agent?.id, AbortSignal.any([exec.signal, input.signal]));
					} catch (error) {
						if (!input.signal.aborted) throw error;
						if (exec.signal.aborted) throw new Error("wait aborted");
					} finally {
						unsubscribe();
					}
				}
				const body = readBody(jobs.read(id, exec.agent?.id));
				if (releasedBy !== void 0) suppressedSettlements.delete(id);
				const notice = pending.get(id);
				if (notice !== void 0) {
					pending.delete(id);
					ctx.get("agents")?.get(notice.owner)?.inbox.remove(notice.messageId);
					body.text = `${notice.held ?? ""}${body.text}`;
				}
				if (releasedBy !== void 0 && (body.job.status === "running" || body.job.status === "stopping")) body.job.detail = body.job.detail === void 0 ? releasedBy : `${body.job.detail}; ${releasedBy}`;
				return body;
			} finally {
				endReleasedRead?.();
			}
		},
		presentCall: (args) => presentJobCall(`Read output from background job ${args.job_id}`, "read", args.job_id)
	}));
	ctx.tools.register(defineTool({
		name: "job_list",
		description: "List your background jobs (running and finished) with their ids, kinds, and statuses.",
		parameters: {},
		output: {
			schema: {
				type: "array",
				items: PUBLIC_JOB_SCHEMA
			},
			render: (_args, jobs) => [{
				type: "text",
				text: jobs.length === 0 ? "(no background jobs)" : jobs.map((t) => `${t.id} [${t.kind}] ${t.status} — ${t.label}`).join("\n")
			}]
		},
		execute(_args, exec) {
			const jobs = ctx.jobs.list(exec.agent?.id);
			return Promise.resolve(jobs.map(publicJob));
		},
		presentCall: () => presentJobCall("List background jobs", "read")
	}));
	ctx.tools.register(defineTool({
		name: "job_kill",
		description: "Request cancellation of a running background job.",
		parameters: {
			job_id: {
				type: "string",
				required: true,
				description: "Job id returned by the tool that started the background work."
			},
			reason: {
				type: "string",
				description: "Optional short reason, recorded in the log and forwarded to the job."
			}
		},
		finalizeContent: finalizeJobContent,
		output: {
			schema: {
				type: "object",
				additionalProperties: false,
				properties: {
					outcome: {
						type: "string",
						required: true,
						enum: ["cancellation-requested", "already-finished"]
					},
					job: {
						...PUBLIC_JOB_SCHEMA,
						required: true
					}
				}
			},
			render: (_args, value) => [{
				type: "text",
				text: value.outcome === "already-finished" ? `job ${value.job.id} had already finished ${statusLine(value.job)}` : `requested cancellation of job ${value.job.id}`
			}]
		},
		execute(args, exec) {
			const id = validateJobId(args.job_id);
			const jobs = ctx.jobs;
			const result = jobs.kill(id, exec.agent?.id, args.reason);
			if (result === "requested") killedByModel.add(id);
			const job = publicJob(jobs.get(id, exec.agent?.id));
			return Promise.resolve({
				outcome: result === "already-finished" ? "already-finished" : "cancellation-requested",
				job
			});
		},
		presentCall: (args) => presentJobCall(`Kill background job ${args.job_id}`, "execute", args.job_id)
	}));
}
//#endregion
export { Config, apply, inject, name };
