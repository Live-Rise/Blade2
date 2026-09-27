import type { FileSystem } from '@deepseek-ai/dsh-fs';
/** Deployment-owned Node executable and optional preinstalled built bootstrap. */
export interface LaunchConfig {
    /** Executable in the subprocess world; defaults to the current Node executable. */
    nodeExecutable?: string;
    /** Absolute preinstalled built bootstrap in the execution world. */
    bootstrapPath?: string;
}
/**
 * Select explicit arguments without inheriting host loader or inspector flags.
 * @param fs - Filesystem mapping host bootstrap assets into the process world.
 * @param config - Optional preinstalled built bootstrap.
 * @param maxMessageBytes - Validated frame and queued-write limit.
 * @returns Arguments following the resolved Node executable.
 */
export declare function bootstrapArgs(fs: FileSystem, config: LaunchConfig, maxMessageBytes: number): string[];
//# sourceMappingURL=launch.d.ts.map