import type { ProgramEvaluator } from './types.ts';
/**
 * Create a REPL whose globals survive sequential evaluations until close.
 * @param reportError - REPL diagnostics attributed to the active evaluation.
 * @returns Retained globals, asynchronous evaluation and REPL cleanup.
 */
export declare function createReplEvaluator(reportError: (message: string) => void): ProgramEvaluator;
//# sourceMappingURL=repl.d.ts.map