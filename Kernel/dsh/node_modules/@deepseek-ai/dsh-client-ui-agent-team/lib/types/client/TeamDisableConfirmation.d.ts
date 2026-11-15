import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { HostObservable, InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
/** The originating Session remains the target while the dialog is open. */
export interface TeamDisableRequest {
    sessionId: SessionId;
}
/** Confirmation request and Session-bound selection callbacks supplied by the plugin. */
export interface TeamDisableConfirmationInjected {
    hooks: {
        disableRequest: HostObservable<TeamDisableRequest | null>;
    };
    cancelDisable: (sessionId: SessionId) => void;
    disableTeam: (sessionId: SessionId) => Promise<string | null>;
}
/** Root overlay props, independent of the model menu lifetime. */
export type TeamDisableConfirmationProps = PropsRuntime<'shell.overlay'> & InjectFace<TeamDisableConfirmationInjected> & PropsLocale<typeof NS>;
/**
 * Render the requested confirmation outside the model menu.
 * @param props - request observation, command callbacks, and localized text.
 * @returns the confirmation, or nothing when no request is pending.
 */
export declare function TeamDisableConfirmation({ useDisableRequest, ...props }: TeamDisableConfirmationProps): import("react").JSX.Element | null;
//# sourceMappingURL=TeamDisableConfirmation.d.ts.map