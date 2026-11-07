/**
 * Output capture, result encoding, and host binding proxies over the process channel.
 * @module @deepseek-ai/dsh-ptc-runtime-node/src/bootstrap
 */
import type { HostMethods, ProgramResult, ProgramBootData } from './protocol.ts';
/**
 * A writable stream's `write` slot, as the bootstrap patches it (see
 * {@link captureStreamWrites}). Method-typed so the real
 * `process.stdout`/`process.stderr` (narrower chunk parameters) remain
 * assignable.
 */
export interface PatchableStream {
    write(chunk: unknown, ...rest: unknown[]): boolean;
}
/**
 * Ordered text capture under the shared outer JSON-byte budget, delivered to
 * a sink as each item lands (the real sink sends text to the host eagerly,
 * so captured output survives a mid-run termination). It includes the log
 * array syntax and string escaping in its accounting. Once exhausted it emits
 * the fitting prefix and reports the limit once; the host turns that condition
 * into an explicit `output-limit` run failure.
 */
export declare class LogBuffer {
    private bytes;
    private entries;
    private truncated;
    private readonly sink;
    private readonly onLimit;
    private readonly maxBytes;
    constructor(maxBytes: number, sink: (text: string) => void, onLimit?: () => void);
    /**
     * Emit text to the sink, charging it against the budget (drops + marks once exhausted).
     * @param text - the captured text to deliver.
     */
    push(text: string): void;
    /** Remaining exact JSON-byte budget for the completion value or failure message. */
    remainingOutputBytes(): number;
}
/** The five console methods the shim captures, in the seam's level vocabulary. */
declare const CONSOLE_LEVELS: readonly ["log", "info", "warn", "error", "debug"];
/**
 * A `console` replacement whose five leveled methods render their arguments
 * `util.inspect`-style (matching real console formatting closely enough for
 * a model to recognize its own output) into the buffer. Only these five
 * exist — the program gets a deliberately small console, not Node's full
 * console API.
 * @param logs - the buffer every rendered line is pushed into.
 * @returns the five-method console object handed to the program.
 */
export declare function makeConsoleShim(logs: Pick<LogBuffer, 'push'>): Record<(typeof CONSOLE_LEVELS)[number], (...args: unknown[]) => void>;
/**
 * Redirect a stream's `write` into the log buffer (the program-visible
 * `process.stdout`/`process.stderr` in the Node child), so raw writes land in emission order
 * alongside console output instead of racing down a pipe. It preserves Node's optional callback
 * contract: the callback runs asynchronously after admission, even when the log budget drops
 * the write.
 *
 * @param logs - the buffer captured writes are pushed into.
 * @param stream - the stream whose `write` slot is patched.
 * @returns the restore function (the in-process tests un-patch; the real
 *   child never needs to).
 */
export declare function captureStreamWrites(logs: Pick<LogBuffer, 'push'>, stream: PatchableStream): () => void;
/**
 * Prepare the program's completion value for the run result. Only lossless
 * JSON crosses, and a value that does not fit the remaining combined outer
 * budget reports `output-limit`; the host revalidates hostile traffic and
 * remains authoritative for native pipe writes the program shim cannot observe.
 *
 * @param value - the program's completion value.
 * @param remainingOutputBytes - exact bytes left after captured logs.
 * @param maxOutputBytes - the configured cap named in an overflow diagnostic.
 * @returns the result fragment: `{}` for `undefined`, else a flat wire `{ value }`.
 */
export declare function prepareCompletion(value: unknown, remainingOutputBytes: number, maxOutputBytes?: number): ProgramResult;
/**
 * Prepare a thrown program value, including cross-realm errors, without sending
 * an unbounded stack or string across the process channel.
 * @param error - the value thrown by the program.
 * @param remainingOutputBytes - exact bytes left after captured logs.
 * @param maxOutputBytes - the configured cap named in an overflow diagnostic.
 * @returns a bounded exception or fixed output-limit fragment.
 */
export declare function prepareException(error: unknown, remainingOutputBytes: number, maxOutputBytes?: number): ProgramResult;
/** Constructor type for one program-visible binding rejection class. */
export type BindingErrorConstructor = new (memberName: string, message: string) => Error;
/**
 * Materialize the real error constructor declared by one namespace.
 * @param descriptor - program-global class name and member-name property.
 * @returns the constructor injected into the program and used for rejections.
 */
export declare function makeBindingErrorClass(descriptor: {
    name: string;
    memberNameProperty: string;
}): BindingErrorConstructor;
/**
 * Build null-prototype binding globals with namespace-specific rejection classes.
 * @param data - Namespace declarations injected into the program.
 * @param host - Evaluation-scoped remote binding calls.
 * @param errorClasses - Constructors shared with program globals, resolved per invocation.
 * @returns One namespace object per declaration; arguments and results must be lossless JSON.
 */
export declare function makeNamespaces(data: Pick<ProgramBootData, 'namespaces'>, host: Pick<HostMethods, 'invoke'>, errorClasses: Pick<ReadonlyMap<string, BindingErrorConstructor>, 'get'>): Record<string, unknown>[];
export {};
//# sourceMappingURL=bootstrap.d.ts.map