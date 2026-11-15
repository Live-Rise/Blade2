/** A displayed Session's header watches its terminals and opens the ones its Agent starts. */
import { type ReactNode } from 'react';
import type { PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
/** The plugin owns tab placement; the component only ties the watch to the displayed Session. */
export interface AgentTerminalOpenerInjected {
    /** @returns the release of this Session's terminal watch. */
    readonly watchAgentTerminals: () => () => void;
}
/**
 * Watch Agent terminals while the Session header is displayed, so its sidebar store accepts new tabs.
 * @param props - Session header lifetime and the watch owned by the plugin.
 * @returns nothing; the effect is the whole component.
 */
export declare function AgentTerminalOpener({ watchAgentTerminals }: PropsRuntime<'conversation.session.header.actions'> & AgentTerminalOpenerInjected): ReactNode;
//# sourceMappingURL=AgentTerminalOpener.d.ts.map