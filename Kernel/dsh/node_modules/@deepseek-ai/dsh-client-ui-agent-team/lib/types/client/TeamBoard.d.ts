import type { PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import { NS } from './locales.ts';
/** Full props of the task board below the shared subagent catalog. */
export type TeamBoardProps = PropsRuntime<'conversation.session.header.actions.subagent.footer'> & PropsLocale<typeof NS>;
/** Render read-only tasks in source order; the enclosing subagent menu owns scrolling. */
export declare function TeamBoard({ sessionId, useProjection, useSessions, t }: TeamBoardProps): import("react").JSX.Element | null;
//# sourceMappingURL=TeamBoard.d.ts.map