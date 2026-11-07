import type { SnapshotStore } from '@deepseek-ai/dsh-client-store';
import type { ActionCardBodyProps } from '@deepseek-ai/dsh-client-ui-action-cards/client';
/** Body props: the frame's card props plus this browser's cost-notice preference. */
export type PageCardProps = ActionCardBodyProps<'creationRecommendations'> & {
    readonly infoDismissed: SnapshotStore<boolean>;
};
/**
 * Display creation, persistent installation, reversible removal and restoration below the frame's summary.
 * @param props - card, state, actions, folded-text component and notice preference.
 * @returns the card body.
 */
export declare function PageCardBody({ proposal, decision, summary, Folded, act, infoDismissed, t }: PageCardProps): import("react").JSX.Element;
//# sourceMappingURL=Card.d.ts.map