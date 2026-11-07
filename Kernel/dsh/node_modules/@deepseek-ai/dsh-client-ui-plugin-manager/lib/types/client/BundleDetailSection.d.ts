/** A bundle's long description on its page: the Markdown document the Host reads for it. */
import { type ReactNode } from 'react';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import { type BundleDetailsState } from './detail-store.ts';
type Translate = PropsLocale<'pluginManager'>['t'];
/** What the section reads and calls; the page passes them from its injected face. */
export interface BundleDetailSectionProps {
    readonly name: string;
    readonly version: string | undefined;
    /** Whether the bundle's files are unavailable, so the Host reads its offered version from the npm file CDNs. */
    readonly fromNpm: boolean;
    readonly language: string;
    readonly details: BundleDetailsState;
    readonly t: Translate;
    readonly load: (name: string, version: string | undefined, language: string, fromNpm: boolean) => void;
    readonly retry: (name: string, version: string | undefined, language: string, fromNpm: boolean) => void;
    readonly image: (name: string, document: string, destination: string, fromNpm: boolean) => string | undefined;
}
/**
 * Read and render a bundle's detail document in the active language; relative images load from the Host beside it.
 * @param props - the bundle, the language, the reads, and the page's callbacks.
 * @returns the section, or null when the bundle has no document.
 */
export declare function BundleDetailSection(props: BundleDetailSectionProps): ReactNode;
export {};
//# sourceMappingURL=BundleDetailSection.d.ts.map