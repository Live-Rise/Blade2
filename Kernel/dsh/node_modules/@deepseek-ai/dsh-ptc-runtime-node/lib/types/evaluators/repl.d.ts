import type { ProgramEvaluator, ProgramEvaluatorOptions } from './types.ts';
/**
 * Create a REPL whose globals survive sequential evaluations until close.
 * @param options - Runner diagnostics and optional synchronous import handling.
 * @returns Retained globals, asynchronous evaluation and REPL cleanup.
 */
export declare function createReplEvaluator(options: ProgramEvaluatorOptions): ProgramEvaluator;
//# sourceMappingURL=repl.d.ts.map