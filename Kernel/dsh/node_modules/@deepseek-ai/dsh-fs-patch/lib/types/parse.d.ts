/**
 * Batch parser following OpenAI Codex rust-v0.158.0 parser.rs and streaming_parser.rs.
 * Adapted into a pure TypeScript parser with original-input diagnostics and no environment routing.
 * Copyright 2025 OpenAI. Apache-2.0 portions and modification notice: ../LICENSE.
 */
import type { PatchOperation } from './types.ts';
/** Syntax failure with a one-based line in the original input, including any wrapper. */
export declare class PatchParseError extends Error {
    readonly lineNumber: number;
    readonly reason: string;
    constructor(lineNumber: number, reason: string);
}
/**
 * Parse a complete Codex-style patch without reading or modifying files.
 * @param patch - patch text, optionally wrapped in a literal EOF heredoc.
 * @returns ordered, JSON-serializable operations; an empty patch returns an empty array.
 * @throws PatchParseError for malformed syntax or an unsupported Environment ID header.
 */
export declare function parsePatch(patch: string): PatchOperation[];
//# sourceMappingURL=parse.d.ts.map