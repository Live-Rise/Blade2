import type { Branded } from '@deepseek-ai/dsh-brand';
import type { PluginOperation } from './types.ts';
/** Identifies the process lifetime which observed management operations. */
export type PluginOperationRunId = Branded<'PluginOperationRunId'>;
/** One diagnostic event; omissions and unknown outcomes must not be interpreted as successful operations. */
export interface PluginOperationRecord {
    readonly runId: PluginOperationRunId;
    readonly operation: PluginOperation;
    readonly transportDropped: number;
}
/**
 * Validate transport/file data and discard fields outside the diagnostic whitelist.
 * @param value - Untrusted decoded JSON.
 * @returns A bounded record, or undefined for malformed, oversized or unsupported records.
 */
export declare function parsePluginOperationRecord(value: unknown): PluginOperationRecord | undefined;
//# sourceMappingURL=diagnostic.d.ts.map