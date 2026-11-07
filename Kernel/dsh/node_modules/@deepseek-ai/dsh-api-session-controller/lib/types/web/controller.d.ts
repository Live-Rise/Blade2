/** Web opening aggregation over one observation and a shared transforming buffer. */
import type { Context } from '@deepseek-ai/cordis';
import type { SessionObservation } from '@deepseek-ai/dsh-session-query';
import type { HistoryController } from '../history/controller.ts';
import type { SessionFollowFrame, SessionFollowRequest } from '../types.ts';
/** Combines opening data and consumes transformed listener output from one queue. */
export declare class WebController {
    private readonly ctx;
    private readonly historyController;
    private readonly promote;
    /**
     * @param ctx - Host context carrying Session queries and identity projections.
     * @param historyController - passive reading and listener construction.
     * @param promote - background activation requested before an opening snapshot is delivered.
     */
    constructor(ctx: Context, historyController: HistoryController, promote: (observation: SessionObservation) => void);
    /**
     * Open a history snapshot and follow uncovered history and assistant notifications.
     * @param request - Session identity, page budget, assistant output and optional activation (default false).
     * @param signal - cancellation for opening and streaming.
     * @returns an opening snapshot followed by uncovered queued increments.
     */
    follow(request: SessionFollowRequest, signal: AbortSignal): AsyncIterable<SessionFollowFrame>;
}
//# sourceMappingURL=controller.d.ts.map