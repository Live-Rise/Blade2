/** Adapt runner-local import decisions to Node's VM module callbacks. */
import * as vm from 'node:vm';
import type { ProgramImportHandler } from '../evaluators/types.ts';
/** Modules belong to one evaluator; each compiled program retains its own resolution filename. */
export declare class VmModuleImportLoader {
    private modules;
    private files;
    private readonly handler;
    private readonly context;
    private closed;
    private readonly nativeImport;
    /**
     * Own module instances for one evaluator's runner-local handler.
     * @param handler - Runner-local import decisions.
     * @param context - Script context receiving the imported modules.
     */
    constructor(handler: ProgramImportHandler, context: vm.Context);
    /**
     * Bind import resolution to the filename used to compile a program.
     * @param filename - Absolute program filename, captured before its evaluation.
     * @returns Callback implementing the runner's import handler.
     */
    forFile(filename: string): vm.DynamicModuleLoader<object>;
    /** Reject further handler calls and release cached modules; imported values remain usable. */
    close(): void;
    private assertOpen;
    private callback;
    private request;
    private file;
    private sourceModule;
    private link;
    private dependencies;
    private module;
}
//# sourceMappingURL=vm-modules.d.ts.map