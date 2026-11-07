/**
 * Pure text updates following OpenAI Codex rust-v0.158.0 NormalizeToLf behavior.
 * Adapted from seek_sequence.rs and file_update.rs with structured match failures.
 * Copyright 2025 OpenAI. Apache-2.0 portions and modification notice: ../LICENSE.
 */
import type { ApplyHunksResult, PatchHunk } from './types.ts';
/**
 * Apply parsed hunks to LF-normalized text without I/O or input mutation.
 * All searches use the original text; a failed search returns no partial text.
 * Successful nonempty output ends in LF, including initially unterminated text.
 * @param before - text normalized by the caller; CR characters receive no line-ending handling.
 * @param hunks - ordered hunks from parsePatch; EOF searches follow NormalizeToLf tail semantics.
 * @returns new text or the first unmatched hunk, without throwing for a match failure.
 */
export declare function applyHunks(before: string, hunks: readonly PatchHunk[]): ApplyHunksResult;
//# sourceMappingURL=apply.d.ts.map