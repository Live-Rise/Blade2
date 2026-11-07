/** Selected-result subscriptions retain no history-sized Client cache. */
import type { ClientRemote } from '@deepseek-ai/dsh-api-gateway/client';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { JsonValue } from '@deepseek-ai/dsh-util-values';
/** Observe one result until the inspector releases it; undefined denotes unavailable. */
export type WatchToolResult = (seq: number, publish: (value: JsonValue | undefined) => void) => () => void;
/**
 * Bind an ephemeral inspector stream to one Session.
 * @param remote - Generated Remote transport and reconnecting stream owner.
 * @param sessionId - Session inspected by this view.
 * @param report - Diagnostic sink for unavailable transport and teardown failures.
 * @returns Subscription factory; closing a subscription releases its selected value.
 */
export declare function createToolResultWatcher(remote: Pick<ClientRemote, '$stream' | 'toolResults'>, sessionId: SessionId, report: (error: unknown) => void): WatchToolResult;
//# sourceMappingURL=watch-tool-result.d.ts.map