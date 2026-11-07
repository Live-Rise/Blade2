/** Browser-zone facts from committed or proposed user-rpc messages and model-facing policy text. */
import type { UserMessage } from '@deepseek-ai/dsh-llm';
/** Browser-zone facts for the messages selected while preparing a request. */
export type BrowserTimeZoneContext = {
    readonly kind: 'resolved';
    readonly timeZone: string;
} | {
    readonly kind: 'mixed';
    readonly timeZones: readonly string[];
} | {
    readonly kind: 'missing';
};
/**
 * Read the raw zone reported by a user-rpc source; validation belongs to request preparation.
 * @param message - user message; only user sources with a string rpcId and clientTimeZone contribute.
 * @returns the reported zone, or undefined when no user-rpc zone was reported.
 */
export declare function reportedBrowserTimeZone(message: UserMessage): string | undefined;
/**
 * Derive the unique, mixed, or missing browser zone for the selected request messages.
 * @param messages - proposed messages, or entered plus proposed messages selected by the legacy reader.
 * @param enteredTimeZones - reported zones from committed messages, before the proposed messages.
 * @returns Sorted, duplicate-free browser-zone facts.
 * @throws TypeError when a user-rpc source carries an invalid or noncanonical zone.
 */
export declare function deriveBrowserTimeZoneContext(messages: readonly UserMessage[], enteredTimeZones?: readonly string[]): BrowserTimeZoneContext;
/**
 * Render the model instruction for one browser-zone context.
 * @param context - browser-zone facts selected for this request.
 * @returns One durable policy line.
 */
export declare function renderBrowserTimeZoneContext(context: BrowserTimeZoneContext): string;
//# sourceMappingURL=request-zone.d.ts.map