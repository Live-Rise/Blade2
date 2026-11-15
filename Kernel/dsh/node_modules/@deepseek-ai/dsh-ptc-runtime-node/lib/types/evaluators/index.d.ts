/** Select one child evaluator without process-lifetime policy. */
import type { PtcProgramFormat } from '@deepseek-ai/dsh-ptc-runtime';
import type { ProgramEvaluator, ProgramImportHandler } from './types.ts';
/**
 * Create the selected evaluator for either a fresh or retained process.
 * @param format - Evaluation syntax selected by the host.
 * @param reportError - Asynchronous diagnostics attributed to the active evaluation.
 * @param importHandler - Runner-local import handling; omission uses Node's loader.
 * @returns Globals, evaluation entry point, and resource cleanup.
 */
export declare function createProgramEvaluator(format: PtcProgramFormat, reportError: (message: string) => void, importHandler?: ProgramImportHandler): ProgramEvaluator;
//# sourceMappingURL=index.d.ts.map