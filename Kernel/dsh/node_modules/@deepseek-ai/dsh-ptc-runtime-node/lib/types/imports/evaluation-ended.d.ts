/** Import cancellation belongs to an evaluation, not to the imported module's cached failure. */
export declare class ImportEvaluationEndedError extends Error {
    /** Whether a later evaluation can reuse this process after the cancelled import. */
    readonly retainsState: boolean;
    constructor(retainsState: boolean, pending?: boolean);
}
//# sourceMappingURL=evaluation-ended.d.ts.map