/** Returns true when a provider error message says the request's strict tools exceed the provider's limits. */
export declare function isStrictToolLimitErrorMessage(message: string): boolean;
/**
 * Records that a session's strict tools exceeded Claude's limits; does nothing without a session. Keeps at most
 * `MAX_SESSIONS_OVER_STRICT_TOOL_LIMITS` sessions.
 *
 * Entries are normally freed by `cleanupSessionResources(sessionId)` when a session ends. The cap bounds memory for
 * library users that never call it; a dropped session just pays one more rejection.
 */
export declare function markStrictToolLimitsExceeded(sessionId: string | undefined): void;
/** Returns true when an earlier request of this session had its strict tools rejected for exceeding Claude's limits. */
export declare function hasExceededStrictToolLimits(sessionId: string | undefined): boolean;
//# sourceMappingURL=strict-tool-limits.d.ts.map