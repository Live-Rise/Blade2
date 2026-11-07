/** Display formatting for the Statistics table. */
/** Placeholder for a value the event stream did not record. */
export declare const DASH = "\u2014";
/**
 * Duration: minutes past one minute, seconds past one second, milliseconds below.
 * @param value - duration in milliseconds.
 * @returns formatted duration, or the dash placeholder.
 */
export declare function formatMs(value: number | null): string;
/**
 * Token count with thousands separators, for tooltips.
 * @param value - token count.
 * @returns grouped digits, or the dash placeholder.
 */
export declare function formatTokens(value: number | null): string;
/**
 * Token count in K, switching to M once the value reaches ten million.
 * @param value - token count, or null when unrecorded.
 * @returns the K/M-suffixed count, or the dash placeholder.
 */
export declare function formatTokensShort(value: number | null): string;
/**
 * Signed context delta, using the same K scale as the counts it belongs to.
 * @param value - token delta; null produces an empty string.
 * @returns signed K-suffixed delta.
 */
export declare function formatTokensDelta(value: number | null): string;
/**
 * Character count of tool output, collapsed to K past one thousand.
 * @param value - character count.
 * @returns formatted size, or the dash placeholder.
 */
export declare function formatChars(value: number | null): string;
/**
 * Output speed in tokens per second.
 * @param value - tokens per second.
 * @returns one-decimal speed, or the dash placeholder.
 */
export declare function formatTps(value: number | null): string;
/**
 * Share as a two-decimal percentage.
 * @param value - share in [0, 1].
 * @returns percentage text, or the dash placeholder.
 */
export declare function formatPercent(value: number | null): string;
/**
 * Local wall-clock time of day, so a row lines up with logs.
 * @param value - epoch milliseconds.
 * @returns `HH:MM:SS`, or the dash placeholder.
 */
export declare function formatClock(value: number | null): string;
/**
 * Compact month-day clock for the turn scope cell; tooltips carry the full date.
 * @param value - epoch milliseconds.
 * @returns `MM-DD HH:MM`, or the dash placeholder.
 */
export declare function formatDateTimeShort(value: number | null): string;
/**
 * Full timestamp with milliseconds for tooltips.
 * @param value - epoch milliseconds.
 * @returns `YYYY-MM-DD HH:MM:SS.mmm`, or the dash placeholder.
 */
export declare function formatDateTimeMs(value: number | null): string;
/**
 * Step start timestamp with date and milliseconds for the trigger column.
 * @param value - epoch milliseconds.
 * @returns `MM-DD HH:MM:SS.mmm`, or the dash placeholder.
 */
export declare function formatStepClock(value: number | null): string;
/**
 * Date and time of day: the table shows the clock, tooltips add the date.
 * @param value - epoch milliseconds.
 * @returns `YYYY-MM-DD HH:MM:SS`, or the dash placeholder.
 */
export declare function formatDateTime(value: number | null): string;
/**
 * Bar length on a square-root scale: a linear scale collapses short steps into
 * slivers, and a logarithmic one flattens the differences the column exists to
 * show. The longest member of the comparison group fills the bar.
 * @param total - measured duration.
 * @param scale - longest duration of the comparison group.
 * @returns fill share in [0, 1].
 */
export declare function barShare(total: number, scale: number): number;
//# sourceMappingURL=stats-format.d.ts.map