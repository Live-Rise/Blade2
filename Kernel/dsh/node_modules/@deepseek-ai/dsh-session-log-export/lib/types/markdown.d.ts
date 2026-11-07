/**
 * Pure backend converter for one complete committed Session log. The Session
 * Markdown copy Host route uses it, and the package entry exports it for
 * callers that already hold the logical events. The raw log is authoritative:
 * compaction surface replacements never remove earlier messages, and
 * fork-inherited history appears once. Reasoning, system and developer
 * messages, injected non-human context, request headers, and runtime records
 * are omitted.
 * @module
 */
import type { SessionEvent } from '@deepseek-ai/dsh-session';
/**
 * Render one complete committed Session log as Markdown. This is a pure backend
 * utility: it needs no mounted plugin, reads no storage, and does not mutate the
 * supplied events. The latest `session/title` event supplies the heading; the
 * rest of the transcript is derived only from the supplied logical events.
 * @param events - complete committed logical event snapshot in seq order, including any inherited prefix.
 * @param sourcePath - optional backend-resolved path to the selected Session artifact; emitted verbatim in a fenced text block.
 * @returns Markdown ending in exactly one newline; an empty log returns the heading and optional source metadata only.
 */
export declare function formatSessionMarkdown(events: readonly SessionEvent[], sourcePath?: string): string;
//# sourceMappingURL=markdown.d.ts.map