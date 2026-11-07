import { brandString } from "@deepseek-ai/dsh-brand";
import { defineTool } from "@deepseek-ai/dsh-tools";
import { markAdjacentAgentSendMessageTool } from "@deepseek-ai/dsh-subagent/internal";
import { assertNever } from "@deepseek-ai/dsh-util-values";
//#region lib/types/list-agents.js
/**
* The globally named `list_agents` tool: a thin model-facing adapter over
* the continuable projection of `ctx.subagents.listChildren()` and, for the
* `descendants` scope, `ctx.subagents.listDescendants()`. The control plugin owns
* registration and disposal alongside messaging and interruption.
*/
/** Resolve the optional model request into an internal required-scope spec. */
function resolveListAgentsRequest(request) {
	return { scope: request.scope ?? "children" };
}
/** Report turn activity without exposing whether the child is loaded. */
function statusOf(agents, id) {
	return agents.get(id)?.status === "running" ? "running" : "inactive";
}
/** Project one service row into the model-facing entry, or omit a one-shot child. */
function project(agents, entry, position) {
	const at = position === void 0 ? {} : {
		parent: position.parentId,
		depth: position.depth
	};
	if ("kind" in entry && entry.kind === "diagnostic") return {
		kind: "diagnostic",
		id: entry.id,
		reason: entry.reason,
		...at
	};
	if (entry.mode !== "continuable") return void 0;
	return {
		kind: "child",
		id: entry.id,
		label: entry.label,
		status: statusOf(agents, entry.id),
		...at
	};
}
/**
* Register the `list_agents` tool.
* @param ctx - context carrying the tool registry, subagent service, and live Agent registry.
*/
function registerListAgents(ctx) {
	ctx.tools.register(defineTool({
		name: "list_agents",
		description: "List subagents you started, with their ids, labels, and status. running means it is working; inactive means it is not currently working. You will be notified when a subagent finishes; there is no need to keep checking its status. Use send_message to continue the conversation.",
		parameters: { scope: {
			type: "string",
			enum: ["children", "descendants"],
			description: "children (default) lists direct children, which accept send_message in any status. descendants lists the whole tree below you with each entry's parent session id and depth; entries deeper than 1 accept only interrupt_agent."
		} },
		output: {
			schema: {
				type: "array",
				items: { oneOf: [{
					type: "object",
					additionalProperties: false,
					properties: {
						kind: {
							type: "string",
							required: true,
							enum: ["child"]
						},
						id: {
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
							enum: ["running", "inactive"]
						},
						parent: { type: "string" },
						depth: { type: "number" }
					}
				}, {
					type: "object",
					additionalProperties: false,
					properties: {
						kind: {
							type: "string",
							required: true,
							enum: ["diagnostic"]
						},
						id: {
							type: "string",
							required: true
						},
						reason: {
							type: "string",
							required: true,
							enum: [
								"corrupt",
								"unsupported",
								"unavailable"
							]
						},
						parent: { type: "string" },
						depth: { type: "number" }
					}
				}] }
			},
			render: (args, entries) => {
				const request = resolveListAgentsRequest(args);
				return [{
					type: "text",
					text: entries.length === 0 ? "(no subagents)" : entries.map((entry) => {
						const at = request.scope === "descendants" ? ` parent=${String(entry.parent)} depth=${String(entry.depth)}` : "";
						return entry.kind === "child" ? `${entry.id} [${entry.status}]${at} — ${entry.label}` : `${entry.id} [diagnostic: ${entry.reason}]${at}`;
					}).join("\n")
				}];
			}
		},
		async execute(args, exec) {
			const parent = exec.agent;
			if (!parent) throw new Error("list_agents requires a calling agent (exec.agent was undefined)");
			const request = resolveListAgentsRequest(args);
			switch (request.scope) {
				case "children": return (await ctx.subagents.listChildren(parent.id, exec.signal)).map((entry) => project(ctx.agents, entry)).filter((entry) => entry !== void 0);
				case "descendants": return (await ctx.subagents.listDescendants(parent.id, exec.signal)).map((entry) => project(ctx.agents, entry, entry)).filter((entry) => entry !== void 0);
				/* v8 ignore next 2 -- the resolver normalizes the schema-validated closed scope before dispatch. */
				default: return assertNever(request.scope, "list_agents scope");
			}
		}
	}));
}
//#endregion
//#region lib/types/index.js
/**
* Shared messaging, interruption, and child discovery tools.
* They perform no lifecycle routing of their own —
* residency, cold resume, and interrupt authorization belong to the subagent
* service — and they live apart from the provider-bound
* `@deepseek-ai/dsh-tool-subagent` instances so multiple delegation tools share
* one control API.
* @module @deepseek-ai/dsh-tool-subagent-control
*/
const name = "tool-subagent-control";
const inject = [
	"tools",
	"subagents",
	"agents"
];
/**
* Register the shared subagent control and discovery tools.
* @param ctx - context carrying tools, subagents, and live Agents.
*/
function apply(ctx) {
	registerListAgents(ctx);
	ctx.tools.register(markAdjacentAgentSendMessageTool(defineTool({
		name: "send_message",
		description: "Send a message to an agent. A working agent receives it at its next step; an idle agent starts a new turn with it. Returns delivery confirmation, not the agent's answer.",
		parameters: {
			agent_id: {
				type: "string",
				required: true,
				description: "The agent id of your direct continuable child, or your direct parent when you are a resident continuable child."
			},
			message: {
				type: "string",
				required: true,
				description: "The message to deliver to the agent."
			}
		},
		output: {
			schema: {
				type: "object",
				additionalProperties: false,
				properties: { messageId: {
					type: "string",
					required: true
				} }
			},
			render: (args, _value) => [{
				type: "text",
				text: `message delivered to agent ${args.agent_id}`
			}]
		},
		async execute(args, exec) {
			const sender = exec.agent;
			if (!sender) throw new Error("send_message requires a calling agent (exec.agent was undefined)");
			const message = [{
				type: "text",
				text: args.message
			}];
			return { messageId: await ctx.subagents.sendMessage(sender, brandString(args.agent_id), message, { signal: exec.signal }) };
		}
	})));
	ctx.tools.register(defineTool({
		name: "interrupt_agent",
		description: "Ask a subagent to stop its current work. This call returns without waiting for it to stop. You can continue a local direct child's conversation later with send_message. External executions stop permanently and cannot receive follow-ups. Subagents it started will keep running.",
		parameters: { agent_id: {
			type: "string",
			required: true,
			description: "The id of an agent created under you: your direct child or a deeper descendant."
		} },
		output: {
			schema: {
				type: "object",
				additionalProperties: false,
				properties: { accepted: {
					type: "boolean",
					required: true
				} }
			},
			render: (args, _value) => [{
				type: "text",
				text: `interrupt requested for agent ${args.agent_id}`
			}]
		},
		execute(args, exec) {
			const caller = exec.agent;
			if (!caller) throw new Error("interrupt_agent requires a calling agent (exec.agent was undefined)");
			ctx.subagents.interrupt(brandString(args.agent_id), {
				kind: "ancestor",
				agent: caller
			});
			return Promise.resolve({ accepted: true });
		}
	}));
}
//#endregion
export { apply, inject, name };
