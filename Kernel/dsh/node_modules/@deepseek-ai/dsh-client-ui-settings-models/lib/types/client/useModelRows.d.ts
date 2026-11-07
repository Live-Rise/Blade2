/** Local model-row identities, disclosures, capacity text, and draft ordering. */
import type { DeepSeekModelDraft } from './DeepSeekModelsEditor.tsx';
/** Text fields whose unfinished spelling belongs to one draft row. */
export type CapacityField = 'contextWindow' | 'maxTokens';
interface RowState {
    key: number;
    expanded: boolean;
    editing: Map<CapacityField, string>;
}
/** Drag state and actions for one model row within its editor. */
export interface ModelRowOrder {
    dragging: boolean;
    target: 'before' | 'after' | undefined;
    canDrop: boolean;
    onStart: () => void;
    onEnd: () => void;
    onOver: (half: 'before' | 'after') => void;
    onLeave: () => void;
    onDrop: (half: 'before' | 'after') => void;
    onMove: (offset: -1 | 1) => void;
}
interface ModelRows {
    rows: (RowState & {
        model: DeepSeekModelDraft;
        order: ModelRowOrder;
    })[];
    toggle: (index: number) => void;
    edit: (index: number, field: CapacityField, text: string | undefined) => void;
    forget: (removed: ReadonlySet<number>) => void;
    reset: () => void;
}
/**
 * Keep row state with its model through draft moves and removals.
 * @param models - current draft array, owned by the provider card.
 * @param onChange - replaces only that card's draft array.
 * @param disabled - prevents reordering while the card cannot be edited.
 * @returns row state and local editing and ordering actions.
 */
export declare function useModelRows(models: readonly DeepSeekModelDraft[], onChange: (models: DeepSeekModelDraft[]) => void, disabled: boolean): ModelRows;
export {};
//# sourceMappingURL=useModelRows.d.ts.map