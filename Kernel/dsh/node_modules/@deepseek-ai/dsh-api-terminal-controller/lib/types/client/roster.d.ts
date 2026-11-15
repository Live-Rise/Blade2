/** One reference-counted terminal roster stream per watched Session. */
import { type ClientRemote } from '@deepseek-ai/dsh-api-gateway/client';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { TerminalRemote } from './model.ts';
import type { WebTerminalInfo } from '../types.ts';
/** Receives each whole roster; a listener added to a live stream first receives its latest roster. */
export type TerminalRosterListener = (terminals: readonly WebTerminalInfo[]) => void;
/**
 * Shares one Host roster stream among a Session's watchers and stops it with the last watcher. A stream that fails
 * while watched reopens for its remaining watchers on the next watch of that Session or on {@link TerminalRosters.resume}.
 */
export declare class TerminalRosters {
    private readonly gateway;
    private readonly remote;
    private readonly received;
    private readonly failed;
    private readonly entries;
    private readonly stopping;
    /**
     * @param gateway - Client Remote carrier captured while the plugin context is current.
     * @param remote - generated terminal namespace.
     * @param received - shared state update applied before listeners run.
     * @param failed - diagnostic sink for listener and stream failures.
     */
    constructor(gateway: Pick<ClientRemote, '$stream'>, remote: Pick<TerminalRemote, 'watch'>, received: (sessionId: SessionId, terminals: readonly WebTerminalInfo[]) => void, failed: (error: unknown) => void);
    /**
     * Watch one Session's terminals.
     * @param sessionId - watched Session.
     * @param listener - called with every roster while watched.
     * @returns the release; the stream stops when its last watcher releases.
     */
    watch(sessionId: SessionId, listener: TerminalRosterListener): () => void;
    /**
     * Reopen the streams that failed while watched, for example after the connection is replaced.
     */
    resume(): void;
    /**
     * Stop every stream.
     * @returns after every stream has closed.
     */
    dispose(): Promise<void>;
    private open;
    private notify;
    private stop;
    private track;
}
//# sourceMappingURL=roster.d.ts.map