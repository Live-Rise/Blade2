import { HarnessError } from '@deepseek-ai/dsh-llm';
import type { InferValue } from '@deepseek-ai/dsh-tools';
import type { PatchExecutionReport } from './patch/types.ts';
/** Strict canonical output; full file texts feed rendering and are excluded from presentation metadata. */
export declare const patchOutputSchema: {
    readonly type: "object";
    readonly additionalProperties: false;
    readonly properties: {
        readonly phase: {
            readonly type: "string";
            readonly const: "commit";
            readonly required: true;
        };
        readonly cancelled: {
            readonly type: "boolean";
            readonly required: true;
        };
        readonly operations: {
            readonly type: "array";
            readonly required: true;
            readonly items: {
                readonly type: "object";
                readonly additionalProperties: false;
                readonly properties: {
                    readonly index: {
                        readonly type: "integer";
                        readonly required: true;
                    };
                    readonly kind: {
                        readonly type: "string";
                        readonly enum: readonly ["add", "update", "delete", "move"];
                        readonly required: true;
                    };
                    readonly path: {
                        readonly type: "string";
                        readonly required: true;
                    };
                    readonly destination: {
                        readonly type: "string";
                    };
                    readonly patchLine: {
                        readonly type: "integer";
                        readonly required: true;
                    };
                    readonly status: {
                        readonly type: "string";
                        readonly enum: readonly ["completed", "failed", "unknown", "skipped"];
                        readonly required: true;
                    };
                    readonly code: {
                        readonly type: "string";
                    };
                    readonly message: {
                        readonly type: "string";
                    };
                    readonly hunkLine: {
                        readonly type: "integer";
                    };
                    readonly changes: {
                        readonly type: "array";
                        readonly items: {
                            readonly type: "object";
                            readonly additionalProperties: false;
                            readonly properties: {
                                readonly path: {
                                    readonly type: "string";
                                    readonly required: true;
                                };
                                readonly before: {
                                    readonly required: true;
                                    readonly oneOf: readonly [{
                                        readonly type: "string";
                                    }, {
                                        readonly type: "null";
                                    }];
                                };
                                readonly after: {
                                    readonly required: true;
                                    readonly oneOf: readonly [{
                                        readonly type: "string";
                                    }, {
                                        readonly type: "null";
                                    }];
                                };
                            };
                        };
                    };
                };
            };
        };
        readonly affectedPaths: {
            readonly type: "array";
            readonly items: {
                readonly type: "string";
            };
            readonly required: true;
        };
    };
};
/** Validated commit value shared by rendering and presentation. */
export type PatchToolValue = InferValue<typeof patchOutputSchema>;
/** Patch syntax and symbolic-link refusals retain their own stable codes. */
export declare class PatchToolError extends HarnessError {
}
/**
 * Convert a report into a commit value, or reject while no disk change exists.
 * A commit whose first operation failed without any change, such as a provider sandbox
 * denial, rejects like a preflight refusal so the error code reaches the same retry paths.
 * @param report - executor report.
 * @returns a flat, lossless JSON commit value, including partial failures.
 * @throws FsError or PatchToolError for syntax, preflight, or first-operation refusals.
 */
export declare function patchToolValue(report: PatchExecutionReport): PatchToolValue;
/**
 * Identify a stopped commit, including cancellation before the next operation.
 * @param value - canonical patch result.
 * @returns whether the patch did not finish every operation.
 */
export declare function patchStopped(value: PatchToolValue): boolean;
/**
 * Render completed paths and actionable stopped-operation diagnostics.
 * @param value - canonical patch result; paths already use the executor's display spelling.
 * @returns concise model-visible text without whole-file contents.
 */
export declare function renderPatch(value: PatchToolValue): string;
/**
 * Derive hunks only from known completed effects, including a partial move destination.
 * Creations store no diff, as for `write`; a removal without a bounded basis stores none either.
 * @param value - canonical commit value containing the provider's bounded baselines.
 * @returns serializable diff hunks and an explicit stopped flag; no whole-file change records.
 */
export declare function patchPresentationMeta(value: PatchToolValue): {
    diffs: {
        path: string;
        oldText: string | null;
        newText: string;
    }[];
    stopped: boolean;
};
//# sourceMappingURL=apply-patch-output.d.ts.map