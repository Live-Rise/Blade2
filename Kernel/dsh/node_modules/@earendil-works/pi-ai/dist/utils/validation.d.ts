import type { Tool, ToolCall } from "../types.ts";
/**
 * Finds a tool by name and validates the tool call arguments against its JSON Schema
 * @param tools Array of tool definitions
 * @param toolCall The tool call from the LLM
 * @returns The validated arguments
 * @throws Error if tool is not found or validation fails
 */
export declare function validateToolCall(tools: Tool[], toolCall: ToolCall): any;
/**
 * Validates tool call arguments against the tool's JSON Schema (any JSON Schema: pi-schema's builders, TypeBox, plain
 * objects). The arguments are coerced first (pi-schema's `parse()`): models often send `"42"` for a number, a single
 * value for an array, or `null` for an omitted optional property.
 * @param tool The tool definition with its JSON Schema
 * @param toolCall The tool call from the LLM
 * @returns The validated (and potentially coerced) arguments
 * @throws Error with formatted message if validation fails
 */
export declare function validateToolArguments(tool: Tool, toolCall: ToolCall): any;
//# sourceMappingURL=validation.d.ts.map