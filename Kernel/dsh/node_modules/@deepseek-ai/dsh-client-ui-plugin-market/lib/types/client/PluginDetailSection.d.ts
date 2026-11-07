/** A market entry's long description: the Markdown document the Host reads for one listed version. */
import { type ReactNode } from 'react';
import type { InjectFace, PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import { type DetailFace, type DetailRequest } from './market-store.ts';
/** Props of the detail section: the entry version, the dictionary, and the injected detail reads. */
export type PluginDetailSectionProps = {
    readonly request: DetailRequest;
} & PropsLocale<'pluginMarket'> & InjectFace<DetailFace>;
/**
 * Read and render an entry version's detail document in the active language. Relative images load from the Host beside
 * the document.
 * @param props - the entry version, the dictionary, and the injected detail reads.
 * @returns the section, or null when the version has no document.
 */
export declare function PluginDetailSection(props: PluginDetailSectionProps): ReactNode;
//# sourceMappingURL=PluginDetailSection.d.ts.map