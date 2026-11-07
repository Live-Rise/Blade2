/**
 * Transform top-level await using Node's retained-declaration and boxed-result convention.
 * @param src - Type-stripped JavaScript ending in a newline, with any hashbang removed.
 * @returns Transformed source yielding undefined or { value }, or null when native errors apply.
 * @throws SyntaxError for invalid syntax or unsupported top-level disposable bindings.
 */
export declare function processTopLevelAwait(src: string): string | null;
//# sourceMappingURL=vm-script-await.d.ts.map