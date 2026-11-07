import type { SessionFormatEvent } from '@deepseek-ai/dsh-session-format';
/**
 * Validate nested released payload semantics for one known event.
 * @param event - known event with exact top-level members.
 * @param version - source or current payload generation.
 */
export declare function assertReleasedPayloadSemantics(event: SessionFormatEvent, version: number): void;
/** `subagent/descriptor` versions that format v0 logs contain. */
export declare const SUBAGENT_DESCRIPTOR_VERSIONS: readonly number[];
//# sourceMappingURL=payload-validation.d.ts.map