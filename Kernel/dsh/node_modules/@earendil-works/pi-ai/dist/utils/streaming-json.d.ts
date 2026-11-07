/**
 * Incremental, tolerant parsing of JSON text that arrives in pieces, such as streamed tool-call arguments.
 *
 * `StreamingJsonParser.append()` reads each piece once, so a stream costs time and memory linear in its length, and
 * `snapshot()` returns the value of the text read so far. Snapshots are plain JSON values and never change once
 * returned: the parser copies the containers that are still open (the "open path") into every new snapshot and shares
 * completed containers and strings. The rules for incomplete text follow `parseStreamingJson()`:
 *
 * - An unterminated string shows the characters read so far; an unterminated escape is held back until complete.
 * - An unterminated container shows the members and elements read so far.
 * - A member whose key or value has not started is omitted.
 * - An unterminated literal shows its value once it is a prefix of `true`, `false` or `null`; an unterminated number
 *   shows its value when the digits read so far form a number (an exponent without digits is ignored).
 *
 * Malformed text is read leniently, the way `repairJson()` and `parseStreamingJson()` treat it: raw control characters
 * in strings are kept, an invalid escape such as `\d` stays literal text, missing commas and colons are tolerated,
 * values in invalid positions and unknown words are dropped, and text after the root value is ignored. The parser never
 * throws. Callers that need the exact value of the finished text parse it once, e.g. with `parseToolCallArguments()`.
 *
 * Differences from `parseStreamingJson()` on incomplete text: trailing whitespace inside an unterminated string is
 * kept (it was trimmed), a string that contains a raw control character or an invalid escape stays visible while it
 * streams (it was dropped until the document was complete), and `NaN` and `Infinity` words are dropped.
 *
 * `append()` also returns the changes the piece made, for a writer that keeps its own copy of the value and applies
 * them instead of re-assigning snapshots: `set` when a key, an array item or a finished value appears (a new string is
 * set once with its first text, a container empty), `append` when the piece extended an open string. Unterminated
 * numbers and literals are not reported until they end. A container nested more than 32 levels deep is reported as one
 * `set` of its whole value when it closes, so paths stay short.
 *
 * Cost: copying the open path costs its member and element count. Each `append()` earns copy credit proportional to its
 * length plus a constant; `snapshot()` spends it and returns the previous snapshot while the open path costs more than
 * the credit. So a document that keeps a very large container open (thousands of elements) shows a preview that lags
 * a little behind the text, and no input makes the total work grow faster than the text.
 */
import type { JsonValue, ToolCall } from "../types.ts";
/** A path from the root value: member keys and array indices. */
export type JsonPath = readonly (string | number)[];
/** One change a piece of text made to the value; see the module comment. */
export type JsonChange = {
    readonly kind: "set";
    readonly path: JsonPath;
    readonly value: JsonValue;
} | {
    readonly kind: "append";
    readonly path: JsonPath;
    readonly text: string;
};
/** Incremental, tolerant JSON parser; see the module comment for its rules and costs. */
export declare class StreamingJsonParser {
    #private;
    /**
     * Read the next piece of text; returns the changes it made, in order. Costs time linear in `text.length`.
     */
    append(text: string): readonly JsonChange[];
    /**
     * The value of the text read so far: `undefined` before a value starts, otherwise a plain JSON value that never
     * changes afterwards. Returns the previous snapshot while the text read since did not change the value, or while a
     * fresh copy of the open path is not yet paid for (see the module comment).
     */
    snapshot(): JsonValue | undefined;
}
/**
 * The arguments of one streaming tool call: the raw text, for the one full parse when the call ends, and a
 * `StreamingJsonParser` for the partial arguments in between. Providers keep one per call in their own stream state,
 * never on the content block, so the raw text never reaches partial messages or transcripts.
 */
export declare class ToolCallArgumentsStream {
    #private;
    /** The raw argument text received so far. */
    get text(): string;
    /** Append a piece of argument text; returns the partial arguments, a new object whenever they changed. */
    append(delta: string): ToolCall["arguments"];
    /**
     * Take the provider's complete argument text. When it continues the text so far, returns the missing piece as
     * `delta`; otherwise starts over with `text`. Either way returns the partial arguments of `text`.
     */
    replace(text: string): {
        readonly delta: string | undefined;
        readonly arguments: ToolCall["arguments"];
    };
    /** The final arguments: one full parse of the complete text, which reports malformed arguments. */
    finish(): Pick<ToolCall, "arguments" | "argumentsError">;
}
//# sourceMappingURL=streaming-json.d.ts.map