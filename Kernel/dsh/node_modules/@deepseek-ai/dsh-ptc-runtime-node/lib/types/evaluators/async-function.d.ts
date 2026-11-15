import type { ProgramEvaluator, ProgramEvaluatorOptions } from './types.ts';
/**
 * Create an evaluator whose function-local declarations end with each call.
 * @param options - Runner diagnostics and optional synchronous import handling.
 * @returns Binding globals, asynchronous evaluation and resource cleanup.
 */
export declare function createAsyncFunctionEvaluator(options: ProgramEvaluatorOptions): ProgramEvaluator;
//# sourceMappingURL=async-function.d.ts.map