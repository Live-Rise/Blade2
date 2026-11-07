/**
 * Session row icons after the time label: a branch glyph while the Session
 * works in the worktree the panel created for it, and the state glyph of its
 * most urgent change request. Glyphs of open change requests stay in the
 * row's subdued color with a dot for their checks; merged and closed ones keep
 * their state color.
 */
import type { ReactNode } from 'react';
import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { ChecksRollup } from '@deepseek-ai/dsh-experimental-forge/types';
import type { ForgePanelKey, NS } from './locales.ts';
import { type SessionListChangeRequest, type SessionListInjected } from './session-list.ts';
type Translate = PropsLocale<typeof NS>['t'];
/** Copy of each checks outcome a row and its hover card name. */
export declare const CHECKS_LABEL: Record<Exclude<ChecksRollup, 'none'>, ForgePanelKey>;
/**
 * The checks outcome a row shows for a change request: none for a merged,
 * closed, or not yet polled one.
 * @param entry - the change request.
 * @returns the outcome.
 */
export declare function rowChecks(entry: SessionListChangeRequest): ChecksRollup;
/**
 * Name a change request's state and checks, as one line.
 * @param entry - the change request.
 * @param t - forge panel copy.
 * @returns the line.
 */
export declare function stateLine(entry: SessionListChangeRequest, t: Translate): string;
/** Full props of the row icons. */
export type SessionRowForgeProps = PropsRuntime<'sidebar.session.row.trailing'> & PropsLocale<typeof NS> & InjectFace<SessionListInjected>;
/**
 * Render a Session row's forge icons.
 * @param props - the row's Session, the shared hooks, and copy.
 * @returns the icons, or nothing for a Session without a worktree or change requests.
 */
export declare function SessionRowForge({ sessionId, useSessions, useStates, t }: SessionRowForgeProps): ReactNode;
export {};
//# sourceMappingURL=SessionRowForge.d.ts.map