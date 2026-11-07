/** Localized durations and relative times shown on change request cards. */
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
type Translate = PropsLocale<typeof NS>['t'];
/**
 * Format an elapsed time as hours and minutes, minutes and seconds, or seconds.
 * @param ms - elapsed milliseconds.
 * @param t - forge panel copy.
 * @returns the localized duration.
 */
export declare function formatDuration(ms: number, t: Translate): string;
/**
 * Format how long ago an ISO 8601 time was.
 * @param iso - the time.
 * @param now - current epoch milliseconds.
 * @param t - forge panel copy.
 * @returns the localized relative time, or undefined when `iso` does not parse.
 */
export declare function formatAgo(iso: string, now: number, t: Translate): string | undefined;
export {};
//# sourceMappingURL=format.d.ts.map