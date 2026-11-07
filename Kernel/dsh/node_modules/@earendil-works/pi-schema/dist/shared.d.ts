/**
 * The regular expression of a `pattern` source, shared by source. Patterns are ECMAScript regular expressions with
 * the `u` flag; a source that is invalid with it (some older schemas escape characters `u` rejects) compiles
 * without it.
 */
export declare function sharedRegex(source: string): RegExp;
/** The number of cached regular expressions (for tests). */
export declare function sharedRegexCount(): number;
//# sourceMappingURL=shared.d.ts.map