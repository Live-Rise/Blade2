/** Locale bundles for the shell executor's settings page. */
import type { SettingsFormLabels } from '@deepseek-ai/dsh-client-ui-primitives';
/** Locale keys the page renders. */
export type ShellSettingsLocaleKey = 'title' | 'description' | 'timeoutMs' | 'timeoutMsHint' | 'maxOutputBytes' | 'maxOutputBytesHint' | 'overridden' | 'reset' | 'readOnly' | 'unavailable' | 'save' | 'saving' | 'saveFailed' | 'invalidNumber';
/** English copy. */
export declare const en: Record<ShellSettingsLocaleKey, string>;
/** Simplified Chinese copy. */
export declare const zh: Record<ShellSettingsLocaleKey, string>;
/**
 * The form frame's copy, read from this page's dictionary.
 * @param t - the page's locale reader.
 * @returns the labels the shared settings form renders.
 */
export declare function formLabels(t: (key: ShellSettingsLocaleKey) => string): SettingsFormLabels;
/** Japanese copy, checked against the English keys. */
export declare const ja: {
    title: string;
    description: string;
    timeoutMs: string;
    timeoutMsHint: string;
    maxOutputBytes: string;
    maxOutputBytesHint: string;
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
    timeoutMs: string;
    timeoutMsHint: string;
    maxOutputBytes: string;
    maxOutputBytesHint: string;
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
    timeoutMs: string;
    timeoutMsHint: string;
    maxOutputBytes: string;
    maxOutputBytesHint: string;
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
    timeoutMs: string;
    timeoutMsHint: string;
    maxOutputBytes: string;
    maxOutputBytesHint: string;
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
    timeoutMs: string;
    timeoutMsHint: string;
    maxOutputBytes: string;
    maxOutputBytesHint: string;
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
    timeoutMs: string;
    timeoutMsHint: string;
    maxOutputBytes: string;
    maxOutputBytesHint: string;
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