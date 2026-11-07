/** Strict async-function-body evaluation with host bindings as parameters. */
import type { ProgramEvaluator } from './types.ts';
/**
 * Create an evaluator whose function-local declarations end with each call.
 * @returns Binding globals, asynchronous evaluation and resource cleanup.
 */
export declare function createAsyncFunctionEvaluator(): ProgramEvaluator;
//# sourceMappingURL=async-function.d.ts.map