/**
 * Reading one share's owner-submitted plugin endpoint selection against the live offers.
 *
 * The owner panel submits what the owner ticked; the plugins' own declarations decide what may be
 * ticked at all. A submission is therefore accepted only where the plugin still offers every named
 * endpoint under the kind it was named with, and a name nothing offers is refused by name instead of
 * being stored as a selection no decision can ever expose.
 *
 * The result is the value the share record carries: absent when the owner exposed nothing, so a share
 * written before the selection existed keeps meaning "none", and an explicit empty submission clears
 * the field rather than storing an empty list.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/viewer-capability-selection
 */
import type { ViewerCapabilityGroup } from './viewer-declarations.ts';
/** One endpoint kind a submission may name. */
type CapabilityKind = 'readable' | 'writable';
/**
 * Why one submission was refused, as the owner panel and the tests read it.
 *
 * The codes are stable and narrow: a malformed body, a plugin the live offers do not name, or an
 * endpoint that plugin does not offer under the kind it was submitted with. The last two carry what
 * they refused, so a refusal names the offending plugin or endpoint instead of only that one is.
 */
export type ViewerCapabilityRefusal = {
    readonly code: 'invalid-capabilities';
} | {
    readonly code: 'unknown-plugin';
    readonly plugin: string;
} | {
    readonly code: 'endpoint-not-offered';
    readonly plugin: string;
    readonly kind: CapabilityKind;
    readonly endpoint: string;
};
/** The accepted selection, or the refusal that replaces it. */
export type ViewerCapabilitySelection = {
    readonly ok: true;
    /**
     * The endpoints this share exposes, grouped by plugin in offer order, or undefined when the
     * submission exposed none. Undefined is what the share record omits: no field means no exposure.
     */
    readonly selection: readonly ViewerCapabilityGroup[] | undefined;
} | {
    readonly ok: false;
    readonly refusal: ViewerCapabilityRefusal;
};
/**
 * Accept one submission against the endpoints plugins currently offer.
 *
 * A plugin group that names no endpoint is dropped, because it exposes nothing; a submission that
 * names nothing at all resolves to an absent selection, which is the field the share record omits.
 * @param offered - Endpoints plugins currently offer, grouped by declaring plugin.
 * @param input - Submitted `capabilities` value, of any type.
 * @returns The selection to store, or the refusal that replaces it.
 */
export declare function readViewerCapabilitySelection(offered: readonly ViewerCapabilityGroup[], input: unknown): ViewerCapabilitySelection;
export {};
//# sourceMappingURL=viewer-capability-selection.d.ts.map