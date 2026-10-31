/** Fixed analytics categories for the picker; custom preset ids never leave this package. */
import type { AnalyticsPreset } from '@deepseek-ai/dsh-client-product-analytics/client';
/** Resolve a known selection by preset id without treating an unloaded state as a default.
 * @param id - selected preset id; an empty id means unknown.
 * @returns the built-in category for a shipped id, other for any other id, or undefined until known.
 */
export declare function analyticsMode(id: string): AnalyticsPreset | 'other' | undefined;
//# sourceMappingURL=analytics.d.ts.map