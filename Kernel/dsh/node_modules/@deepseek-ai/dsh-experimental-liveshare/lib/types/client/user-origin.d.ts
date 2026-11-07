/**
 * The attribution line above a user message a shared-session reader sent.
 *
 * The durable `user/message` source records the share-viewer principal for a
 * caller other than the owner. Chat's `conversation.chat.user.before` chain seat
 * elects that source above the official user bubble; this entry supplies only
 * the label and leaves the bubble, actions, and text interaction to Chat. The
 * seat is dispatched for `user` rows alone, so steering keeps its untouched
 * official DOM. Chat's seat owner props carry the durable source unparsed; this
 * module is the only place that recognizes the share-viewer principal and owns
 * the copy.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/user-origin
 */
import type { ReactNode } from 'react';
import type { MatchedShare, PropsLocale, PropsRuntime, SlotMap } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
/** The durable author a shared message names: its subject and, when recorded, display name. */
export interface ShareViewerAuthor {
    readonly subjectId: string;
    readonly displayName?: string;
}
/**
 * The Relay-asserted author behind a durable message source.
 *
 * Session data is persisted, so every field the attribution relies on is
 * checked; an owner, legacy, or malformed source names no author.
 * @param source - Recorded `user/message` source.
 * @returns The share-viewer author, or undefined for every other author.
 */
export declare function shareViewerAuthor(source: unknown): ShareViewerAuthor | undefined;
/**
 * The Relay-asserted subject behind a durable message source.
 * @param source - Recorded `user/message` source.
 * @returns The share-viewer subject, or undefined for every other author.
 */
export declare function shareViewerSubject(source: unknown): string | undefined;
/** Props of the shared-session origin entry above a user message. */
export type SharedSessionOriginProps = PropsRuntime<'conversation.chat.user.before'> & PropsLocale<typeof NS> & MatchedShare<SlotMap['conversation.chat.user.before'], ShareViewerAuthor>;
/**
 * Render the reader attribution elected for a durable share-viewer message.
 * @param props - Chat's elected chain match and this package's translator.
 * @returns The subject line.
 */
export declare function SharedSessionOrigin({ matched: author, t }: SharedSessionOriginProps): ReactNode;
//# sourceMappingURL=user-origin.d.ts.map