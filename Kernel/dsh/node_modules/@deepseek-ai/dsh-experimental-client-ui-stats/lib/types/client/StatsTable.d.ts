/**
 * Statistics table for one conversation: per-turn aggregates, expandable step
 * rows, the time-composition bars, and the per-step tool-call accordion.
 */
import { type ReactElement, type ReactNode } from 'react';
import type { ImageAttachmentRef } from '@deepseek-ai/dsh-attachment';
import type { ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
import type { ConvViewProps } from '@deepseek-ai/dsh-client-ui-conversation/client';
import type { InjectFace, PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { StatsSnapshot, LoadOlderResult, StatsTurnMetrics } from './stats-contract.ts';
import { type ContextMode, type StatsTotals } from './stats-aggregate.ts';
import type { StatsTranslate } from './locales.ts';
/** Distance from the scroll top, in pixels, that requests one older history page. */
export declare const LOAD_OLDER_THRESHOLD = 64;
/** Numeric columns in table order. */
export declare const COLUMNS: readonly ["context", "output", "cache", "ttft", "model", "tps", "tool"];
/** Business face injected by the browser plugin. */
export interface StatsViewInjected {
    /** Snapshot sources bound by InjectFace. */
    hooks: {
        /** This target's snapshot; InjectFace exposes it as `useStats`. */
        stats: ObservableSnapshot<StatsSnapshot>;
    };
    /**
     * Load one older history page into the published window.
     * @returns whether the page grew, exhausted history, or must be retried.
     */
    loadOlder: () => Promise<LoadOlderResult>;
    /** Cancel a pending open-window wait when this view unmounts. */
    cancel: () => void;
    /**
     * Resolve one durable prompt image through the shared Conversation image cache.
     * @param attachment - image reference the user message recorded.
     * @returns browser URL owned by the cache for the Session binding's lifetime.
     */
    loadImage: (attachment: ImageAttachmentRef) => Promise<string>;
}
/** Full props of the performance conversation View. */
export type StatsViewProps = ConvViewProps & InjectFace<StatsViewInjected> & PropsLocale<'stats'>;
/** Resolve one durable prompt image to a browser URL. */
type LoadImage = StatsViewInjected['loadImage'];
/** Open/closed default for turns the user never toggled explicitly. */
type ViewMode = 'newest' | 'all' | 'none';
/** Step-list presentation of one turn: newest slice or all steps, plus filters. */
export interface StepViewState {
    readonly mode?: 'tail' | 'all';
    readonly filter?: string | null;
    readonly open?: Readonly<Record<number, boolean>>;
    readonly raw?: Readonly<Record<string, boolean>>;
    readonly wrap?: boolean;
}
/** Component-local table state: turn folds and per-turn step state. */
export interface ViewState {
    readonly mode: ViewMode;
    readonly openTurns: Readonly<Record<number, boolean>>;
    readonly stepViews: Readonly<Record<number, StepViewState>>;
    readonly showTotals: boolean;
    readonly showEvents: boolean;
    readonly showContextDelta: boolean;
    readonly contextMode: ContextMode;
}
/** Fields the time-composition bar reads from a turn or step row. */
interface TimeState {
    readonly running: boolean;
    readonly startTime: number | null;
    readonly wall: number | null;
    readonly ttft: number | null;
    readonly model: number | null;
    readonly tool: number | null;
    /** Non-overlapping union of the paired tool calls; the bar prefers it over `tool`. */
    readonly toolWall: number | null;
}
/** Table state shared by one turn's rendered rows. */
interface TurnRowsOptions {
    readonly turn: StatsTurnMetrics;
    readonly t: StatsTranslate;
    readonly loadImage: LoadImage;
    readonly now: number;
    readonly open: boolean;
    readonly state: StepViewState;
    readonly turnScale: number;
    readonly contextMode: ContextMode;
    readonly showEvents: boolean;
    readonly showContextDelta: boolean;
    readonly contextDelta: number | null;
    readonly onToggle: () => void;
    readonly onToggleDetail: (stepNumber: number) => void;
    readonly onControl: (patch: Partial<StepViewState>) => void;
}
/** Props of the pagination/expand footer button. */
interface TableButtonProps {
    readonly caption: string;
    readonly enabled: boolean;
    readonly onClick: () => void;
}
/** Empty per-turn step state, shared so unexpanded turns allocate nothing. */
export declare const EMPTY_STEP_VIEW: StepViewState;
/** Whether a row is running with no measured wall time yet. */
export declare function isTimeRunning(value: TimeState): boolean;
/** Measured span; a running row falls back to the elapsed time at `now`. */
export declare function totalSpan(value: TimeState, now: number): number | null;
/** Default fold state of a turn the user never toggled. */
export declare function defaultTurnOpen(mode: ViewMode, turnNumber: number, latestTurn: number | null): boolean;
/** Fresh table state: follow the newest turn's open default. */
export declare function createViewState(): ViewState;
/** Props of the totals row. */
interface TotalsRowProps {
    readonly totals: StatsTotals;
    readonly full: boolean;
    readonly t: StatsTranslate;
    readonly now: number;
}
/** Bottom row summing the turns loaded in the current window. */
export declare function TotalsRow({ totals, full, t, now }: TotalsRowProps): ReactElement;
/** All rows of one turn: the clickable header, then the expanded body. */
export declare function turnRows(options: TurnRowsOptions): ReactNode[];
/** Footer button that switches between enabled and disabled styling. */
export declare function TableButton({ caption, enabled, onClick }: TableButtonProps): ReactElement;
export {};
//# sourceMappingURL=StatsTable.d.ts.map