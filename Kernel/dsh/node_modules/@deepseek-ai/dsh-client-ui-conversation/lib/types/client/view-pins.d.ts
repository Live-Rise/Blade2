/** In-memory pin choices for the Conversation View tabs. */
import { type ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
/** View whose tab always shows and never enters the customization list. */
export declare const FIXED_VIEW_ID = "chat";
/** Unpinned View ids and their only mutation. */
export interface ViewPins {
    /** Unpinned View ids; ids of Views that are not registered stay listed. */
    readonly unpinned: ObservableSnapshot<readonly string[]>;
    /**
     * Pin or unpin one View tab; the fixed View is ignored. A pinned tab still
     * enters More when the header's width budget cannot hold it.
     * @param id - View id.
     * @param pinned - Whether the tab is manually pinned.
     */
    readonly setPinned: (id: string, pinned: boolean) => void;
}
/**
 * Create the in-memory View pin list; the snapshot keeps its reference until the list changes.
 * @returns Unpinned View ids and the pin mutation.
 */
export declare function createViewPins(): ViewPins;
//# sourceMappingURL=view-pins.d.ts.map