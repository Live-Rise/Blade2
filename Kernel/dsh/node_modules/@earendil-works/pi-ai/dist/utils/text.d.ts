import type { ImageContent, SystemMessage, TextContent, ThinkingContent, ToolCall } from "../types.ts";
type Content = TextContent | ImageContent | ThinkingContent | ToolCall;
/** Extract and join text from message content. */
export declare function contentText(content: string | readonly Content[], separator?: string): string;
/** Render a system message as a complete prompt: its content followed by its sections. */
export declare function getSystemMessageText(message: SystemMessage): string;
/**
 * Render a later system message for APIs that accept system messages mid-conversation.
 * Section changes are framed by name so the model can relate them to the leading prompt.
 * This framing is request-time only and may change between versions.
 */
export declare function renderSystemMessageUpdate(message: SystemMessage): string;
/**
 * `text.slice(start, end)` as a string of its own. V8 represents a slice of 13 or more characters as a view that keeps
 * all of `text` alive, so a short excerpt kept from a large text (a 500-character error message cut from a megabyte
 * response body) would keep the whole text alive as long as the excerpt lives. Appending a character and slicing it off
 * again makes V8 copy just the slice.
 */
export declare function ownedSlice(text: string, start: number, end?: number): string;
export {};
//# sourceMappingURL=text.d.ts.map