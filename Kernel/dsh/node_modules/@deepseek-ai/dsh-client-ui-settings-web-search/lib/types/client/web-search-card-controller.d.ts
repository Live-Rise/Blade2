/**
 * The web search and fetch page's staged form over three settings namespaces:
 * `web`, whose `searchProvider` and `fetchProvider` the two provider menus
 * write, `web-deepseek-api` for the DeepSeek Search and Fetch APIs, and
 * `web-search-deepseek` for legacy Messages search.
 *
 * A selection equal to the deployment's composed value clears the override
 * instead of storing a copy of it, and while the deployment composes no
 * provider for a capability its menu also offers automatic selection, which
 * clears the override. The menus offer the providers the Host reports through
 * `web/listProviders`, the official ones first in a fixed order and the rest
 * in registration order, plus a selected id the Host no longer registers. A
 * failed read is reported on the page with a retry.
 *
 * The DeepSeek Search and Fetch APIs take no credential values here: each
 * has a billing menu, written to `searchBilling` or `fetchBilling`, that
 * chooses which DeepSeek credential it sends. Their own endpoint fields are
 * YAML-only: the page reads them to report that they override billing and
 * never writes them. Legacy Messages search keeps its key control.
 *
 * The legacy key's literal never rides a response, so the page learns only
 * whether one is configured and writes it through the credentials domain,
 * addressed by the reference `web-search-deepseek.apiKeyEnv` names. A key
 * draft belongs to the reference its control addressed when it was typed: it
 * is dropped when that reference changes or the page stops showing the
 * control, and a save writes it under that reference. The key is still staged
 * with the rest of the form, so one save covers everything the page shows.
 */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import type { WebProviderInfo } from '@deepseek-ai/dsh-web/types';
import { type SnapshotStore } from '@deepseek-ai/dsh-client-store';
import { type SettingsFieldState, type SettingsFormActions, type SettingsFormShell, type SettingsFormScope } from '@deepseek-ai/dsh-client-ui-primitives';
/**
 * Namespaces this page edits. Spelled here rather than imported: a client
 * package must not depend on a Host package.
 */
export declare const WEB_SEARCH_NS = "web-search-deepseek";
/** Namespace of the DeepSeek Search and Fetch API providers. */
export declare const WEB_API_NS = "web-deepseek-api";
/** Namespace of the web service's provider selection. */
export declare const WEB_NS = "web";
/** Provider ids of the official providers whose settings this page edits. */
export declare const PROVIDER_IDS: {
    readonly apiSearch: "deepseek-search-api";
    readonly legacySearch: "deepseek-official";
    readonly localFetch: "http";
    readonly apiFetch: "deepseek-fetch-api";
};
/** Settings-section fields the face edits as text. */
export type WebSectionField = 'count' | 'messagesBaseURL' | 'maxUses';
/** The legacy key control. */
export type WebApiKeyField = 'legacyApiKey';
/** Controls the face edits as text: the key controls and the settings-section fields. */
export type WebSearchCardField = WebApiKeyField | WebSectionField;
/** The web service's provider selection. */
export interface WebProviderSelection {
    /** Search provider id. */
    searchProvider?: string;
    /** Fetch provider id. */
    fetchProvider?: string;
}
/** The Messages search fields this page edits. */
export interface WebLegacySettings {
    /** Credential reference naming the environment key. */
    apiKeyEnv?: string;
    /** Messages endpoint; blank inherits the provider default. */
    baseURL?: string;
    /** Maximum Messages server-tool uses. */
    maxUses?: number;
}
/** Credential a DeepSeek Search or Fetch API capability sends, as `web-deepseek-api` names it. */
export type WebBilling = 'auto' | 'account' | 'api-key';
/** The Search and Fetch API fields this page edits. */
export interface WebApiSettings {
    /** Search API results per search. */
    count?: number;
    /** Credential the Search API sends. */
    searchBilling?: WebBilling;
    /** Credential the Fetch API sends. */
    fetchBilling?: WebBilling;
    /** Search API base set in YAML; read only, and it overrides {@link searchBilling}. */
    readonly searchBaseURL?: string;
    /** Search API credential reference set in YAML; read only, and it overrides {@link searchBilling}. */
    readonly searchApiKeyEnv?: string;
    /** Fetch API base set in YAML; read only, and it overrides {@link fetchBilling}. */
    readonly fetchBaseURL?: string;
    /** Fetch API credential reference set in YAML; read only, and it overrides {@link fetchBilling}. */
    readonly fetchApiKeyEnv?: string;
}
/** One capability's billing menu. */
export interface WebBillingState {
    /** The staged billing; a section that carries none reads as `auto`. */
    value: WebBilling;
    /** Whether saving would leave a user-layer entry for this field. */
    overridden: boolean;
    /**
     * Names of the capability's non-empty own endpoint fields, base URL first. While any is set, the provider ignores
     * billing and the menu is locked.
     */
    advancedOverride: readonly string[];
}
/** One entry of a provider menu. */
export interface WebProviderOption {
    /** Provider id, written into the selection. */
    id: string;
    /** Title the provider registered with, if any. */
    title?: string;
    /** Description the provider registered with, if any; the page shows it under the menu while the provider is selected. */
    description?: WebProviderInfo['description'];
    /** True for a selected id the Host does not currently register. */
    missing: boolean;
}
/** What the Search section shows for its selected provider. */
export type WebSearchKind = 'deepseek' | 'legacy' | 'plugin' | 'none';
/** What the Fetch section shows for its selected provider. */
export type WebFetchKind = 'local' | 'deepseek' | 'plugin' | 'none';
/** The legacy key control. */
export interface WebApiKeyState extends SettingsFieldState {
    /** Whether the Host reports a credential configured for the referenced key. */
    configured: boolean;
    /** Whether the credentials domain accepts a write for it; false disables the control. */
    writable: boolean;
    /** The credential reference the control writes: `web-search-deepseek.apiKeyEnv`, or its default. */
    ref: string;
}
/** One provider section: its menu and the controls its selection shows. */
export interface WebProviderSectionState<K extends string> {
    /** Id of the selected provider, or the empty string when the selection names none. */
    selected: string;
    /** Menu entries: the registered providers, plus a selected id that is not registered. */
    options: readonly WebProviderOption[];
    /** Which provider-specific controls the section shows. */
    kind: K;
    /** Whether the menu offers automatic selection, because the deployment composes no provider for this capability. */
    auto: boolean;
}
/** What the web search and fetch page renders. */
export interface WebSearchCardState extends SettingsFormShell {
    /** Invalid edits or failed saves that changing providers must keep visible. */
    selectionBlocked: boolean;
    /**
     * Whether the latest provider list read failed. The menus keep the last list
     * read, or before any list arrives only the selected ids, so they may be
     * incomplete.
     */
    providersFailed: boolean;
    /** The Search section. */
    search: WebProviderSectionState<WebSearchKind>;
    /** The Fetch section. */
    fetch: WebProviderSectionState<WebFetchKind>;
    /** Search API results per search, shown in the Search section while it selects the DeepSeek Search API. */
    count: SettingsFieldState;
    /** Search API billing, present only while the Search section selects the DeepSeek Search API. */
    searchBilling: WebBillingState | undefined;
    /** Fetch API billing, present only while the Fetch section selects the DeepSeek Fetch API. */
    fetchBilling: WebBillingState | undefined;
    /** Legacy Messages endpoint. */
    messagesBaseURL: SettingsFieldState;
    /** Legacy server-side search limit. */
    maxUses: SettingsFieldState;
    /** Legacy key control, present only while the legacy provider is selected. */
    legacyKey: WebApiKeyState | undefined;
}
/** The registration-side face the page's slot entry injects. */
export interface WebSearchCardFace extends Pick<SettingsFormActions, 'save' | 'discard'> {
    hooks: {
        /** Page snapshot bound by the renderer as useWebSearchCard. */
        webSearchCard: SnapshotStore<WebSearchCardState>;
    };
    /** Stage draft text for one control. */
    edit: (field: WebSearchCardField, text: string) => void;
    /** Stage a clear for one section field. */
    resetField: (field: WebSectionField) => void;
    /** Stage one provider selection; the empty id stages automatic selection. */
    selectProvider: (field: keyof WebProviderSelection, id: string) => void;
    /** Stage one capability's billing; the composed billing clears the override. */
    selectBilling: (capability: WebBillingCapability, billing: WebBilling) => void;
    /** Re-read the registered providers after a failed read. */
    retryProviders: () => void;
    /** Resolve a provider description to the active language. */
    resolveText: (text: NonNullable<WebProviderOption['description']>) => string;
}
/** A capability whose DeepSeek API has a billing menu. */
type WebBillingCapability = 'search' | 'fetch';
/** Settings scopes for the namespaces the page edits. */
export interface WebSearchCardScopes {
    /** The web service's provider selection. */
    web: SettingsFormScope<WebProviderSelection>;
    /** Search and Fetch API settings. */
    api: SettingsFormScope<WebApiSettings>;
    /** Legacy Messages search settings. */
    legacy: SettingsFormScope<WebLegacySettings>;
}
/** Bridges the settings scopes, the provider lists, and the credentials domain onto the page. */
export declare class WebSearchCardController {
    private readonly scopes;
    private readonly ctx;
    private readonly selection;
    private readonly api;
    private readonly legacy;
    private readonly store;
    private readonly unsubscribe;
    private credential;
    private credentialReadQueued;
    private credentialReadSequence;
    /** Reference the key draft was typed for; read only while that draft is non-empty. */
    private draftRef;
    /** The draft reference the running or last selection save writes under. */
    private savingRef;
    private droppingDraft;
    private providers;
    private providersFailed;
    private providerReadSequence;
    private selectionServed;
    private disposed;
    /**
     * @param scopes - the bound settings scopes for the page's namespaces.
     * @param ctx - the page plugin's context, whose `remote.credentials` namespace
     * answers for the credential the legacy key control references, and whose
     * `remote.web` namespace lists the registered providers.
     */
    constructor(scopes: WebSearchCardScopes, ctx: ClientContext);
    /**
     * Republish after a form or scope change. A staged selection can hide the
     * key control without any scope publishing; a scope change can rename the
     * reference itself.
     */
    private publish;
    /** The provider id a selection field stages. */
    private selected;
    /**
     * Menu entries for one capability: the registered official providers in
     * {@link MENU_ORDER}, then every other registered provider in Host order,
     * and the selected id when the Host does not register it. Before the list
     * arrives, the menu holds only the selected id.
     */
    private options;
    /** Whether the Host reports `id` as registered, or has not answered yet. */
    private registered;
    private searchKind;
    private fetchKind;
    /** The reference the key control addresses, from the legacy settings or the default. */
    private ref;
    /** Whether the page shows the key control. */
    private keyShown;
    /**
     * Drop the key draft once the page no longer shows the control or the
     * control addresses another reference, so a draft is never written under
     * another reference or under one no control shows.
     */
    private dropStaleKeyDraft;
    /**
     * Stage a key draft for the reference the control addresses now.
     * @param text - the draft text.
     */
    private editKey;
    private projection;
    /** One capability's billing as its menu renders it. */
    private billing;
    /**
     * Re-read the registered providers. Only the latest read publishes; a failed
     * read keeps the previous list and marks the page's provider list as failed
     * until a later read succeeds.
     */
    refreshProviders(): void;
    private readProviders;
    /** Clear the old reference's answer and invalidate its pending reads. */
    private resetCredentialReference;
    /** Combine settings publications, including connection-reset refreshes, into one credential read. */
    private queueCredentialRead;
    /**
     * Ask the credentials domain about the reference the key control currently names.
     *
     * Only the latest read publishes its answer, and only while its reference
     * is unchanged. A write's readback retains its own result even if
     * another read starts or the reference changes.
     * @param written - a reference a save just wrote, asked about in the same read.
     * @returns the answer by reference, or undefined after failure or disposal.
     */
    private readCredential;
    /**
     * Re-read after the Host reports a change to the reference this page watches.
     *
     * A key can be written from somewhere else — the Models page addresses the
     * same reference — and no settings section changes when it is, so without
     * this the badge keeps reporting a state the Host already replaced.
     * @param ref - the reference the Host reports as changed.
     */
    refreshCredential(ref: string): void;
    /**
     * Build the face the page's slot registration injects.
     * @returns the page's snapshot and its form actions.
     */
    inject(): WebSearchCardFace;
    /**
     * Stage one provider selection. Automatic selection and a target equal to
     * the composed value clear the override, so the deployment default keeps
     * applying.
     * @param field - the selection field.
     * @param id - the provider id to select, or the empty string for automatic selection.
     */
    private select;
    /**
     * Stage one capability's billing. A billing equal to the composed one, which
     * is `auto` when the deployment composes none, clears the override.
     * @param capability - the capability whose billing changes.
     * @param billing - the billing to stage.
     */
    private selectBilling;
    /**
     * Save the API section before the selection, so a selection takes effect
     * only after the settings it uses have landed. A refused section stops the
     * selection from saving and keeps every remaining draft for correction. The
     * key draft's reference is fixed when the selection save plans its writes,
     * so a provider change during the save does not redirect a key the save
     * already planned.
     * @returns settlement after every attempted write.
     */
    private save;
    /**
     * Write the staged key under the reference its draft belonged to when the
     * save started, then re-read whether the Host now holds it.
     * @param value - the staged credential literal.
     * @returns whether the Host reports a configured credential afterwards.
     */
    private writeKey;
    /** Release subscriptions and prevent queued reads or late answers from publishing. */
    dispose(): void;
}
export {};
//# sourceMappingURL=web-search-card-controller.d.ts.map