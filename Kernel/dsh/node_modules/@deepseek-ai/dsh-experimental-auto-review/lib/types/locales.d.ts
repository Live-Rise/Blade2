/** Locale-owned approval text; reviewer reasons remain verbatim in every language. */
/** English approval messages. */
export declare const en: {
    denied: string;
    deniedWithReason: (reason: string) => string;
};
/** Chinese approval messages. */
export declare const zh: {
    denied: string;
    deniedWithReason: (reason: string) => string;
};
/** Japanese approval messages. */
export declare const ja: {
    denied: string;
    deniedWithReason: (reason: string) => string;
};
/** Spanish dictionary, checked against the English keys. */
export declare const es: {
    denied: string;
    deniedWithReason: (reason: string) => string;
};
/** French dictionary, checked against the English keys. */
export declare const fr: {
    denied: string;
    deniedWithReason: (reason: string) => string;
};
/** German dictionary, checked against the English keys. */
export declare const de: {
    denied: string;
    deniedWithReason: (reason: string) => string;
};
/** Russian dictionary, checked against the English keys. */
export declare const ru: {
    denied: string;
    deniedWithReason: (reason: string) => string;
};
/** Portuguese dictionary, checked against the English keys. */
export declare const pt: {
    denied: string;
    deniedWithReason: (reason: string) => string;
};
/**
 * Build all display translations without translating the reviewer's raw reason.
 * @param reason - optional reviewer explanation.
 * @returns text for the Client's localized-text resolver.
 */
export declare function deniedDisplayReason(reason?: string): Record<'en' | 'zh' | 'ja' | 'es' | 'fr' | 'de' | 'ru' | 'pt', string>;
//# sourceMappingURL=locales.d.ts.map