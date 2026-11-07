/** Correlate IPC requests by worker-issued tokens and Relay ids by their exact socket. */
import type { TunnelRequestToken } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
/** One request and the exact Relay connection that may receive its response. */
export interface PendingRequest<T> {
    readonly token: TunnelRequestToken;
    readonly shareId: string;
    readonly id: number;
    readonly owner: T;
    readonly since: number;
}
/** Request identities remain distinct when a Relay reconnects and reuses its numeric ids. */
export declare class PendingRpc<T> {
    private readonly tokens;
    private readonly owners;
    /**
     * Register a request with a new IPC token.
     * @param shareId Share carrying the request.
     * @param id Relay id on this socket.
     * @param owner Exact socket that received the request.
     * @returns Registered request; an existing id must be cancelled before reuse on the same socket.
     */
    add(shareId: string, id: number, owner: T): PendingRequest<T>;
    /**
     * Claim an answer only when all IPC correlation fields match.
     * @param answer Token, share and Relay id echoed by the Host.
     * @returns The request removed from pending state, or undefined for a stale or mismatched answer.
     */
    take(answer: {
        readonly token: TunnelRequestToken;
        readonly shareId: string;
        readonly id: number;
    }): PendingRequest<T> | undefined;
    /**
     * Remove one socket-local request.
     * @param owner Socket receiving the cancellation.
     * @param id Relay id on that socket.
     * @returns Removed request for cancellation notification, or undefined if it already settled.
     */
    cancel(owner: T, id: number): PendingRequest<T> | undefined;
    /**
     * Remove all requests on a closed socket.
     * @param owner Socket being disposed.
     * @returns Removed requests for Host cancellation notifications.
     */
    clear(owner: T): readonly PendingRequest<T>[];
}
//# sourceMappingURL=pending-rpc.d.ts.map