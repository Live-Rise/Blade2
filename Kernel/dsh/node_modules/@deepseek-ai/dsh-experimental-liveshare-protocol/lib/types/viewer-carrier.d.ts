/**
 * Viewer page carrier: the script the Relay places at the top of the owner's served index document.
 *
 * It installs the page's Connection transport over the Relay read channel and names the share the
 * page presents. It performs no Liveshare business: grants, page status, and refusal handling belong
 * to the Liveshare client plugin, which reads them through ordinary reads. The carrier only converts
 * Connection calls into `call` requests, Gateway streams into `stream-open`/`stream-next`/
 * `stream-cancel` polls, and marks each failure as a retryable carrier loss or a terminal refusal.
 *
 * It also forwards the page-origin owner routes the Relay cannot answer, so app code that calls `fetch`
 * directly reaches the owner instead of the Relay's tail. That set is explicit and read-only; every
 * other request keeps the page's own `fetch`.
 *
 * {@link installViewerCarrier} runs in the browser from its own source text, so it must reference no
 * module binding: every constant and helper it needs is declared inside its body.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare-protocol
 */
/** Page global naming the share one viewer page presents; the Liveshare client plugin reads it. */
export declare const LIVESHARE_VIEWER_GLOBAL = "__DSH_LIVESHARE_VIEWER__";
/** Share one served viewer page presents. */
export interface ViewerPageTarget {
    /** Public Relay base path every read is posted under; empty at the Relay root. */
    readonly basePath: string;
    readonly shareId: string;
    /** Session the share presents; the page opens it once the session list carries it. */
    readonly sessionId: string;
}
/** Carrier inputs: the page target plus the page's own read budget. */
export interface ViewerCarrierOptions extends ViewerPageTarget {
    /** Longest wait for one Relay answer before the read counts as lost. */
    readonly readTimeoutMs: number;
}
/**
 * Install the viewer globals in the current page. Runs before the shell entry so the Connection
 * plugin finds `__DSH_TRANSPORT__` when it installs.
 * @param options - Page target and read budget.
 */
export declare function installViewerCarrier(options: ViewerCarrierOptions): void;
/**
 * Build the classic inline script the Relay places first in the served viewer document.
 * @param options - Page target and read budget.
 * @returns Script text: {@link installViewerCarrier}'s own source applied to `options`.
 */
export declare function viewerCarrierScript(options: ViewerCarrierOptions): string;
//# sourceMappingURL=viewer-carrier.d.ts.map