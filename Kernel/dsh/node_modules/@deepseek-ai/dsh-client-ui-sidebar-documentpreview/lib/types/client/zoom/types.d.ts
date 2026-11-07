import type { DocumentZoomLabels } from '@deepseek-ai/dsh-client-ui-primitives';
/** Automatic fit or an explicit actual-size scale selected by the reader. */
export type ZoomPreference = {
    readonly kind: 'fit-width';
} | {
    readonly kind: 'fixed';
    readonly scale: number;
};
/** Default preference for a newly opened zoomable preview. */
export declare const FIT_WIDTH: ZoomPreference;
/** Localized copy consumed by the framework-free zoom controls. */
export interface ZoomLabels extends DocumentZoomLabels {
    readonly fitWidth: string;
}
//# sourceMappingURL=types.d.ts.map