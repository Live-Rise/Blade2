import type { Duplex } from 'node:stream';
import type { PtcProgramFormat } from '@deepseek-ai/dsh-ptc-runtime';
import type { PatchableStream } from './bootstrap.ts';
/** Process-owned environment, output and directory control consumed by the child. */
export interface ProgramProcess {
    env: NodeJS.ProcessEnv;
    stdout: PatchableStream;
    stderr: PatchableStream;
    exitCode?: string | number | null | undefined;
    chdir(directory: string): void;
}
/**
 * Serve sequential evaluations until the host closes the control channel.
 * @param stream - Inherited, already-adopted control endpoint.
 * @param maxMessageBytes - Maximum control message and queued output bytes.
 * @param processState - Child environment, output, directory control and exit status.
 * @param defaultFormat - Syntax used by installed bootstraps when the host omits it.
 * @returns After control closure and evaluator cleanup.
 */
export declare function runNodeMain(stream: Duplex, maxMessageBytes: number, processState: ProgramProcess, defaultFormat?: PtcProgramFormat): Promise<void>;
//# sourceMappingURL=process.d.ts.map