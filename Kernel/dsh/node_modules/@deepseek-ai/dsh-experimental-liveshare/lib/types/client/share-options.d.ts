/**
 * Choice lists the share dialog's create form and stored-share edit panel both present.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/share-options
 */
import type { SegmentedControlOption } from '@deepseek-ai/dsh-client-ui-primitives';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import { type ViewerSessionMode } from './viewer-capabilities.ts';
/** Translate seat the dialog's choice lists read their labels through. */
type Translate = PropsLocale<typeof NS>['t'];
/**
 * The two session modes, in the order the host stores and reports them.
 * @param t - Localized copy.
 * @returns One option per session mode.
 */
export declare function sessionModeOptions(t: Translate): readonly SegmentedControlOption<ViewerSessionMode>[];
export {};
//# sourceMappingURL=share-options.d.ts.map