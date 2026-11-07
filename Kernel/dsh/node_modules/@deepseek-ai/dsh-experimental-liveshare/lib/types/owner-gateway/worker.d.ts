/** Owner-side gateway worker: one process multiplexes every share over IPC to the owner host. */
/**
 * Run one owner worker that multiplexes every share over IPC. The worker owns
 * one outbound Relay socket per share, but one process and one owner DSH.
 */
export declare function runGatewayWorker(): void;
//# sourceMappingURL=worker.d.ts.map