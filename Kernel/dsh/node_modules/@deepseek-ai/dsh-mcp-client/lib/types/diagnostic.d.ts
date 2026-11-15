/**
 * Connection diagnostics retain cause chains. Configurations that resolve
 * secrets replace every known secret value before text leaves the instance.
 */
/** Local failure whose message contains no expanded configuration or upstream text. */
export declare class McpLocalError extends Error {
}
/**
 * Replace every occurrence of known secret values, longest first so a secret
 * that contains another is replaced whole.
 * @param text - diagnostic or upstream text.
 * @param secrets - non-empty secret values known to the instance.
 * @returns the text with each occurrence replaced by `[redacted]`.
 */
export declare function redactSecrets(text: string, secrets: Iterable<string>): string;
/**
 * Flatten an error and its `cause` chain into `outer, cause: inner, …`.
 * @param error - the caught value; non-Errors stringify as-is.
 * @returns the joined diagnostic line.
 */
export declare function describeError(error: unknown): string;
//# sourceMappingURL=diagnostic.d.ts.map