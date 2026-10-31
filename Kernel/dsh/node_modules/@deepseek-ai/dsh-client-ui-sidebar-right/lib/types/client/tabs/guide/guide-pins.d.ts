/** In-memory pin choices for the guide's tool cards. */
import { type ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
import type { SidebarRightGuideBox } from '../../tab-registry.ts';
/** Tab kind whose card always shows and never enters the customization list. */
export declare const FIXED_GUIDE_KIND = "files";
/**
 * Pin identity of one guide card; the kind survives an extension taking over that kind.
 * @param entry - Guide card from the tab registry.
 * @returns `kind:id`.
 */
export declare function guideEntryId(entry: Pick<SidebarRightGuideBox, 'kind' | 'id'>): string;
/** Unpinned guide card ids and their only mutation. */
export interface GuidePins {
    /** Unpinned `kind:id` card ids; ids of cards that are not registered stay listed. */
    readonly unpinned: ObservableSnapshot<readonly string[]>;
    /**
     * Pin or unpin one guide card; cards of the fixed kind are ignored.
     * @param id - `kind:id` card id.
     * @param pinned - Whether the card is manually pinned.
     */
    readonly setPinned: (id: string, pinned: boolean) => void;
}
/**
 * Create the in-memory guide pin list; the snapshot keeps its reference until the list changes.
 * @returns Unpinned card ids and the pin mutation.
 */
export declare function createGuidePins(): GuidePins;
//# sourceMappingURL=guide-pins.d.ts.map