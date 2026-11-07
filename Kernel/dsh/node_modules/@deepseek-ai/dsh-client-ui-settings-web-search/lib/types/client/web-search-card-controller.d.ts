/**
 * The web-search page's staged form over three settings namespaces: `web`,
 * whose provider selection the Search API switch writes, `web-search-deepseek`
 * for Messages search, and `web-deepseek-api` for the Search and Fetch APIs.
 *
 * The switch selects `deepseek-search-api` and `deepseek-fetch-api` together, or
 * Messages search with local HTTP fetch. A selection equal to the deployment's
 * composed value clears the override instead of storing a copy of it.
 *
 * The key is the one control that does not live in a section: its literal
 * never rides a response, so the page learns only whether one is configured
 * and writes it through the credentials domain, addressed by the reference the
 * selected API's section names. It is still staged with the rest of the form,
 * so one save covers everything the page shows.
 */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
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
/** The web service's provider selection. */
export interface WebProviderSelection {
    /** Search provider id. */
    searchProvider?: string;
    /** Fetch provider id. */
    fetchProvider?: string;
}
/** The Messages search fields this page edits. */
export interface WebSearchSettings {
    /** Credential reference naming the environment key. */
    apiKeyEnv?: string;
    /** Messages endpoint; blank inherits the provider default. */
    baseURL?: string;
    /** Maximum Messages server-tool uses. */
    maxUses?: number;
}
/** The Search and Fetch API fields this page edits. */
export interface WebApiSettings {
    /** Credential reference naming the environment key. */
    apiKeyEnv?: string;
    /** Search and Fetch API base with its path prefix; blank inherits the provider default. */
    baseURL?: string;
    /** Search API results per search. */
    count?: number;
}
/** What the web-search page renders. */
export interface WebSearchCardState extends SettingsFormShell {
    /** Whether the Host serves both the selection and the Search / Fetch API section. */
    apiSwitchAvailable: boolean;
    /** Invalid edits or failed saves that switching APIs must keep visible. */
    apiSwitchBlocked: boolean;
    /** Staged switch position: true selects the Search and Fetch APIs. */
    useSearchApi: boolean;
    /** Endpoint of the selected API. */
    baseURL: SettingsFieldState;
    /** Maximum Messages server-tool uses. */
    maxUses: SettingsFieldState;
    /** Search API results per search. */
    count: SettingsFieldState;
    /** The staged credential, which starts blank on every load. */
    apiKey: SettingsFieldState;
    /** Whether the Host reports a credential configured for the referenced key. */
    apiKeyConfigured: boolean;
    /** Whether the credentials domain accepts a write for it; false disables the control. */
    apiKeyWritable: boolean;
}
/** The registration-side face the web-search page's slot entry injects. */
export interface WebSearchCardFace extends SettingsFormActions {
    hooks: {
        /** Page snapshot bound by the renderer as useWebSearchCard. */
        webSearchCard: SnapshotStore<WebSearchCardState>;
    };
    /** Stage the API switch: true selects the Search and Fetch APIs, false Messages search with local fetch. */
    setSearchApi: (enabled: boolean) => void;
}
/** Settings scopes for the namespaces the page edits. */
export interface WebSearchCardScopes {
    /** The web service's provider selection. */
    web: SettingsFormScope<WebProviderSelection>;
    /** Messages search settings. */
    messages: SettingsFormScope<WebSearchSettings>;
    /** Search and Fetch API settings. */
    api: SettingsFormScope<WebApiSettings>;
}
/** Bridges the three settings scopes and the credentials domain onto the page. */
export declare class WebSearchCardController {
    private readonly scopes;
    private readonly ctx;
    private readonly selection;
    private readonly messages;
    private readonly api;
    private readonly store;
    private readonly unsubscribe;
    private credential;
    private credentialReadQueued;
    private credentialReadSequence;
    private disposed;
    /**
     * @param scopes - the bound settings scopes for the page's namespaces.
     * @param ctx - the page plugin's context, whose `remote.credentials` namespace
     * answers for the credential the selected section references.
     */
    constructor(scopes: WebSearchCardScopes, ctx: ClientContext);
    /** Whether the Host serves both sections the switch needs. */
    private switchAvailable;
    /** The staged switch position, read from the staged search selection. */
    private searchApi;
    private projection;
    /** The reference the selected API's section names, or the providers' default. */
    private ref;
    /** Clear the old reference's answer and invalidate its pending reads. */
    private resetCredentialReference;
    /** Combine settings publications, including connection-reset refreshes, into one credential read. */
    private queueCredentialRead;
    /**
     * Ask the credentials domain about the reference the selected section currently names.
     *
     * Only the latest read for the selected reference publishes its answer.
     * A write's readback retains its own result even if another read starts.
     * @returns the answer for this read, or undefined after failure, disposal, or a reference change.
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
     * Build the face the page's slot registration injects. `baseURL` edits the
     * selected API's endpoint; `maxUses` and `count` edit their own sections.
     * @returns the page's snapshot and its form actions.
     */
    inject(): WebSearchCardFace;
    /**
     * Stage both provider selections for one switch position. A target equal to
     * the composed value clears the override so the deployment default keeps
     * applying.
     * @param enabled - true for the Search and Fetch APIs.
     */
    private select;
    /**
     * Save the provider sections before the selection, so a switch takes effect
     * only after the endpoint it uses has landed. A refused section stops the
     * selection from saving and keeps every remaining draft for correction.
     * @returns settlement after every attempted write.
     */
    private save;
    /**
     * Write the staged key, then re-read whether the Host now holds one.
     * @param value - the staged credential literal.
     * @returns whether the Host reports a configured credential afterwards.
     */
    private writeKey;
    /** Release subscriptions and prevent queued reads or late answers from publishing. */
    dispose(): void;
}
//# sourceMappingURL=web-search-card-controller.d.ts.map