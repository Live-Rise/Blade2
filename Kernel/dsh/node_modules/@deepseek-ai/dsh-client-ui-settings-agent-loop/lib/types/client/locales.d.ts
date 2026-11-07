/** Locale bundles for the agent loop's settings page. */
import type { SettingsFormLabels } from '@deepseek-ai/dsh-client-ui-primitives';
/** Locale keys the page renders. */
export type AgentLoopSettingsLocaleKey = 'title' | 'description' | 'maxParallel' | 'maxParallelHint' | 'overridden' | 'reset' | 'readOnly' | 'unavailable' | 'save' | 'saving' | 'saveFailed' | 'invalidNumber';
/** English copy. */
export declare const en: Record<AgentLoopSettingsLocaleKey, string>;
/** Simplified Chinese copy. */
export declare const zh: Record<AgentLoopSettingsLocaleKey, string>;
/**
 * The form frame's copy, read from this page's dictionary.
 * @param t - the page's locale reader.
 * @returns the labels the shared settings form renders.
 */
export declare function formLabels(t: (key: AgentLoopSettingsLocaleKey) => string): SettingsFormLabels;
/** Japanese copy, checked against the English keys. */
export declare const ja: {
    title: string;
    description: string;
    maxParallel: string;
    maxParallelHint: string;
    overridden: string;
    reset: string;
    readOnly: string;
    unavailable: string;
    save: string;
    saving: string;
    saveFailed: string;
    invalidNumber: string;
};
/** Spanish dictionary, checked against the English keys. */
export declare const es: {
    title: string;
    description: string;
    maxParallel: string;
    maxParallelHint: string;
    overridden: string;
    reset: string;
    readOnly: string;
    unavailable: string;
    save: string;
    saving: string;
    saveFailed: string;
    invalidNumber: string;
};
/** French dictionary, checked against the English keys. */
export declare const fr: {
    title: string;
    description: string;
    maxParallel: string;
    maxParallelHint: string;
    overridden: string;
    reset: string;
    readOnly: string;
    unavailable: string;
    save: string;
    saving: string;
    saveFailed: string;
    invalidNumber: string;
};
/** German dictionary, checked against the English keys. */
export declare const de: {
    title: string;
    description: string;
    maxParallel: string;
    maxParallelHint: string;
    overridden: string;
    reset: string;
    readOnly: string;
    unavailable: string;
    save: string;
    saving: string;
    saveFailed: string;
    invalidNumber: string;
};
/** Russian dictionary, checked against the English keys. */
export declare const ru: {
    title: string;
    description: string;
    maxParallel: string;
    maxParallelHint: string;
    overridden: string;
    reset: string;
    readOnly: string;
    unavailable: string;
    save: string;
    saving: string;
    saveFailed: string;
    invalidNumber: string;
};
/** Portuguese dictionary, checked against the English keys. */
export declare const pt: {
    title: string;
    description: string;
    maxParallel: string;
    maxParallelHint: string;
    overridden: string;
    reset: string;
    readOnly: string;
    unavailable: string;
    save: string;
    saving: string;
    saveFailed: string;
    invalidNumber: string;
};
//# sourceMappingURL=locales.d.ts.map