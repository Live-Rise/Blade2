/**
 * The forge section of a Session row's hover card: the worktree branch while
 * the Session works in the worktree the panel created for it, and its change
 * requests, most urgent first.
 */
import type { ReactNode } from 'react';
import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
import { type SessionListInjected } from './session-list.ts';
/** Change requests the hover card lists before naming how many more there are. */
export declare const HOVER_CHANGE_REQUEST_LIMIT = 3;
/** Full props of the hover section. */
export type SessionHoverForgeProps = PropsRuntime<'sidebar.session.row.hover'> & PropsLocale<typeof NS> & InjectFace<SessionListInjected>;
/**
 * Render the forge section of a Session's hover card.
 * @param props - the row's Session, the shared hooks, and copy.
 * @returns the section, or null for a Session without a worktree or change requests.
 */
export declare function SessionHoverForge({ sessionId, useSessions, useStates, t }: SessionHoverForgeProps): ReactNode;
//# sourceMappingURL=SessionHoverForge.d.ts.map