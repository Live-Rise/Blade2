/** Session-owned Agent Teams selection in the model and reasoning menu. */
import type { PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
/** Session selection callbacks supplied by the Client plugin. */
export interface TeamModeToggleInjected {
    /** Save the selection; null means success, otherwise the string describes the failure. */
    selectTeamMode: (enabled: boolean) => Promise<string | null>;
    /** Open a Session-targeted confirmation outside this menu. */
    requestDisableConfirmation: () => void;
}
/** Full props of the model-menu Team switch. */
export type TeamModeToggleProps = PropsRuntime<'conversation.input.model.options'> & PropsLocale<typeof NS> & TeamModeToggleInjected;
/**
 * Render the saved selection without an optimistic update.
 * @param props - Session projection, menu state, localized copy, and selection callback.
 * @returns the Team switch when the capability projection is available.
 */
export declare function TeamModeToggle(props: TeamModeToggleProps): import("react").JSX.Element | null;
//# sourceMappingURL=TeamModeToggle.d.ts.map