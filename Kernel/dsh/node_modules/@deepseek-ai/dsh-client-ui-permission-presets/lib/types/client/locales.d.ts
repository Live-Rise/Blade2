/** `settings.permission` namespace dictionaries (the Permission row's copy). */
/** Locale namespace shared by both current-session permission pickers. */
export declare const PERMISSION_ACCESS_NS = "permission.access";
/** Simplified Chinese dictionary (the key-set source of truth). */
export declare const zh: {
    title: string;
    description: string;
    loading: string;
    unavailable: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
};
/** The settings.permission namespace key union. */
export type PermissionSettingsKey = keyof typeof zh;
/** English dictionary, checked complete against the zh key set. */
export declare const en: {
    title: string;
    description: string;
    loading: string;
    unavailable: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
};
/** Simplified Chinese dictionary for the current-session popup gate. */
export declare const accessZh: {
    mode: string;
    close: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
    'auto.label': string;
    'auto.badge': string;
    'auto.description': string;
    'auto.confirm.title': string;
    'auto.confirm.description': string;
    'auto.confirm.acknowledge': string;
    'auto.confirm.enable': string;
};
/** Current-session popup-gate key union. */
export type PermissionAccessKey = keyof typeof accessZh;
/** English dictionary for the current-session popup gate. */
export declare const accessEn: {
    mode: string;
    close: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
    'auto.label': string;
    'auto.badge': string;
    'auto.description': string;
    'auto.confirm.title': string;
    'auto.confirm.description': string;
    'auto.confirm.acknowledge': string;
    'auto.confirm.enable': string;
};
/** Japanese copy, checked against the English keys. */
export declare const ja: {
    title: string;
    description: string;
    loading: string;
    unavailable: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
};
/** Spanish dictionary, checked against the English keys. */
export declare const es: {
    title: string;
    description: string;
    loading: string;
    unavailable: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
};
/** French dictionary, checked against the English keys. */
export declare const fr: {
    title: string;
    description: string;
    loading: string;
    unavailable: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
};
/** German dictionary, checked against the English keys. */
export declare const de: {
    title: string;
    description: string;
    loading: string;
    unavailable: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
};
/** Russian dictionary, checked against the English keys. */
export declare const ru: {
    title: string;
    description: string;
    loading: string;
    unavailable: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
};
/** Portuguese dictionary, checked against the English keys. */
export declare const pt: {
    title: string;
    description: string;
    loading: string;
    unavailable: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
};
/** Japanese copy, checked against the English keys. */
export declare const accessJa: {
    mode: string;
    close: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
    'auto.label': string;
    'auto.badge': string;
    'auto.description': string;
    'auto.confirm.title': string;
    'auto.confirm.description': string;
    'auto.confirm.acknowledge': string;
    'auto.confirm.enable': string;
};
/** Spanish dictionary, checked against the English keys. */
export declare const accessEs: {
    mode: string;
    close: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
    'auto.label': string;
    'auto.badge': string;
    'auto.description': string;
    'auto.confirm.title': string;
    'auto.confirm.description': string;
    'auto.confirm.acknowledge': string;
    'auto.confirm.enable': string;
};
/** French dictionary, checked against the English keys. */
export declare const accessFr: {
    mode: string;
    close: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
    'auto.label': string;
    'auto.badge': string;
    'auto.description': string;
    'auto.confirm.title': string;
    'auto.confirm.description': string;
    'auto.confirm.acknowledge': string;
    'auto.confirm.enable': string;
};
/** German dictionary, checked against the English keys. */
export declare const accessDe: {
    mode: string;
    close: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
    'auto.label': string;
    'auto.badge': string;
    'auto.description': string;
    'auto.confirm.title': string;
    'auto.confirm.description': string;
    'auto.confirm.acknowledge': string;
    'auto.confirm.enable': string;
};
/** Russian dictionary, checked against the English keys. */
export declare const accessRu: {
    mode: string;
    close: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
    'auto.label': string;
    'auto.badge': string;
    'auto.description': string;
    'auto.confirm.title': string;
    'auto.confirm.description': string;
    'auto.confirm.acknowledge': string;
    'auto.confirm.enable': string;
};
/** Portuguese dictionary, checked against the English keys. */
export declare const accessPt: {
    mode: string;
    close: string;
    'preset.readOnly': string;
    'preset.workspaceWrite': string;
    'preset.fullAccess': string;
    'confirm.title': string;
    'confirm.description': string;
    'confirm.acknowledge': string;
    'confirm.cancel': string;
    'confirm.enable': string;
    'auto.label': string;
    'auto.badge': string;
    'auto.description': string;
    'auto.confirm.title': string;
    'auto.confirm.description': string;
    'auto.confirm.acknowledge': string;
    'auto.confirm.enable': string;
};
//# sourceMappingURL=locales.d.ts.map