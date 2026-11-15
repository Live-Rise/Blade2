import type { ProgramImportHandler } from '../evaluators/types.ts';
/** Module hooks and an export bridge owned by one native evaluator. */
export declare class ModuleHooksImportLoader {
    private readonly identity;
    private readonly bridgeName;
    private readonly bridgeKey;
    private readonly hooks;
    private readonly programs;
    private readonly replPrograms;
    private readonly scope;
    private readonly sources;
    private readonly bindings;
    private modules;
    private sequence;
    private closed;
    /**
     * Install synchronous hooks for this evaluator's programs only.
     * @param handler - Runner-local decisions; returning a Promise rejects the import.
     */
    constructor(handler: ProgramImportHandler);
    /**
     * Register an AsyncFunction import parent owned by this evaluator.
     * @param filename - Absolute program filename, captured before its evaluation.
     * @returns Unique program URL for the native compiler and constructor source URL.
     */
    forFile(filename: string): string;
    /**
     * Attribute Node REPL imports to this evaluator, including asynchronous callbacks.
     * @param filename - Absolute program filename for the evaluation's working directory.
     * @param evaluate - Start the REPL evaluation while its import scope is active.
     */
    runRepl(filename: string, evaluate: () => void): void;
    /** Deregister hooks and remove the export bridge; Node retains loaded ESM namespaces until process exit. */
    close(): void;
    private moduleUrl;
}
//# sourceMappingURL=module-hooks.d.ts.map