import type { StoredSpan } from "./index.ts";
/**
 * Render stored spans as a chronological forest, treating absent parents as roots. Lengths of time are milliseconds to
 * the microsecond. Control characters in ids, names, attribute keys and error messages are escaped (attribute values
 * are JSON).
 */
export declare function renderTrace(spans: readonly StoredSpan[]): string;
//# sourceMappingURL=render.d.ts.map