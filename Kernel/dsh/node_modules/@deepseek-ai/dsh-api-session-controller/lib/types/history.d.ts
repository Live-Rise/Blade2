/** Shared Session history pagination, address validation and wire conversion. */
import type { SessionEvent, SessionHeader, SessionId, SessionLogOffset as SessionLogOffsetType, SessionSeqCursor } from '@deepseek-ai/dsh-session';
import type { SessionObservation } from '@deepseek-ai/dsh-session-query';
import type { SessionAddress, SessionEventEntry, SessionHistoryRecord, SessionPageRequest } from './types.ts';
/** Message budget applied when a history request omits `maxMessages`. */
export declare const DEFAULT_MAX_MESSAGES = 50;
/**
 * Reject an invalid message budget or turn window.
 * @param request - history window fields.
 */
export declare function validateHistoryWindow(request: Pick<SessionPageRequest, 'maxMessages' | 'turnWindow'>): void;
/**
 * Select the Session whose log an address names.
 * @param address - ordinary Session or direct-subagent address.
 * @returns the Session whose log the address reads.
 */
export declare function addressId(address: SessionAddress): SessionId;
/**
 * Reject an address that does not match the observed Session.
 * @param address - ordinary Session or direct-subagent address.
 * @param header - observed Session header.
 * @param inheritedEventCount - observed fork-inherited event count.
 * @param projections - observed projections; subagent addresses require them.
 */
export declare function validateAddress(address: SessionAddress, header: SessionHeader, inheritedEventCount: SessionLogOffsetType, projections: SessionObservation['projections']): void;
/**
 * Reject a missing Session using its address-specific error.
 * @param address - ordinary Session or direct-subagent address.
 * @returns never; this operation always throws.
 * @throws RemoteError `session/not-found` or `subagent/not-found`.
 */
export declare function rejectNotFound(address: SessionAddress): never;
/**
 * Cut one message-aligned backwards page from a contiguous event prefix.
 * @param events - contiguous events starting at seq 0.
 * @param beforeSeq - exclusive upper offset of an older page.
 * @param maxMessages - message budget.
 * @param throughSeq - inclusive log cut.
 * @param turnWindow - optional turn-boundary stop rule.
 * @returns the page events and whether older events remain.
 */
export declare function paginate(events: readonly SessionEvent[], beforeSeq: SessionLogOffsetType | undefined, maxMessages: number, throughSeq: SessionSeqCursor, turnWindow?: SessionPageRequest['turnWindow']): {
    readonly events: SessionEvent[];
    readonly hasMore: boolean;
};
/**
 * Convert a committed event to its wire entry.
 * @param event - committed Session event.
 * @returns the wire event entry.
 */
export declare function entryFor(event: SessionEvent): SessionEventEntry;
/**
 * Encode one bounded logical page without changing its pagination cut.
 * @param events - page events.
 * @returns the wire records.
 */
export declare function pageRecords(events: readonly SessionEvent[]): SessionHistoryRecord[];
//# sourceMappingURL=history.d.ts.map