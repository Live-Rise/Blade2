import type { ProgramEvaluator, ProgramEvaluatorOptions } from './types.ts';
/**
 * Create a VM context supporting Script completion values and top-level await.
 * @param options - Runner diagnostics and optional import handling.
 * @returns Retained globals, asynchronous evaluation and resource cleanup.
 */
export declare function createVmScriptEvaluator(options: ProgramEvaluatorOptions): ProgramEvaluator;
//# sourceMappingURL=vm-script.d.ts.map