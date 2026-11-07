/**
 * Choices of each changes tab: the local scope, the side-by-side view, line
 * wrapping, and the navigation it last applied. One bucket per tab, so two
 * views in one Session keep their own choices; the bucket ends with the tab
 * record's signal.
 */
import { type EngineStoreHandle } from '@deepseek-ai/dsh-client-store';
import type { ForgeLocalScope } from '../types.ts';
/** One changes tab's choices. */
export interface ChangesTabState {
    /** Local changes shown; a change request view ignores it. */
    scope: ForgeLocalScope['kind'];
    /** Whether deletions and additions show in two columns. */
    split: boolean;
    /** Whether long lines wrap instead of being cut at the column edge. */
    wrap: boolean;
    /** The navigation revision last applied, whose file the view scrolled to. */
    navigated: number;
}
/** Every changes tab's choices, keyed by tab id. */
export interface ChangesState {
    byTab: Record<string, ChangesTabState>;
}
/** The changes store's write set; every action names the tab it writes. */
type ChangesActions = {
    navigated: (draft: ChangesState, tabId: string, revision: number) => void;
    scoped: (draft: ChangesState, tabId: string, scope: ForgeLocalScope['kind']) => void;
    toggledSplit: (draft: ChangesState, tabId: string) => void;
    toggledWrap: (draft: ChangesState, tabId: string) => void;
    forget: (draft: ChangesState, tabId: string) => void;
};
/**
 * Declare the changes tab's store; the registration declares it as an
 * exclusive store, so the framework mints one instance per Session.
 * @returns the store handle to declare on the registration.
 */
export declare function createChangesStore(): EngineStoreHandle<ChangesState, ChangesActions>;
export {};
//# sourceMappingURL=changes-store.d.ts.map