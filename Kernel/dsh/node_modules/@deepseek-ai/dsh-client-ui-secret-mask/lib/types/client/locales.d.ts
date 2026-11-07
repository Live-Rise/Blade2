/** `secretMask` dictionaries for the per-credential reveal control. */
/** Dictionary namespace owned by this plugin. */
export declare const NS = "secretMask";
/** Simplified Chinese dictionary (the key-set source of truth). */
export declare const zh: {
    'secret.reveal': string;
    'secret.hide': string;
};
/** The secretMask namespace key union. */
export type SecretMaskKey = keyof typeof zh;
declare module '@deepseek-ai/dsh-client-ui-slots' {
    interface LocaleNamespaceMap {
        /** Accessible names of the control that reveals one masked credential. */
        secretMask: SecretMaskKey;
    }
}
/** English dictionary, checked complete against the zh key set. */
export declare const en: {
    'secret.reveal': string;
    'secret.hide': string;
};
/** Japanese copy, checked against the English keys. */
export declare const ja: {
    'secret.reveal': string;
    'secret.hide': string;
};
/** Spanish dictionary, checked against the English keys. */
export declare const es: {
    'secret.reveal': string;
    'secret.hide': string;
};
/** French dictionary, checked against the English keys. */
export declare const fr: {
    'secret.reveal': string;
    'secret.hide': string;
};
/** German dictionary, checked against the English keys. */
export declare const de: {
    'secret.reveal': string;
    'secret.hide': string;
};
/** Russian dictionary, checked against the English keys. */
export declare const ru: {
    'secret.reveal': string;
    'secret.hide': string;
};
/** Portuguese dictionary, checked against the English keys. */
export declare const pt: {
    'secret.reveal': string;
    'secret.hide': string;
};
//# sourceMappingURL=locales.d.ts.map