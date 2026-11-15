import { OfficeKitKey, type OfficeKitGeneration } from './identity.ts';
import type { OfficeConversionFormat, OfficeImage, OfficeKitPriority, OfficeKitSource, OfficeRecalculationFormat } from './types.ts';
import type { Config } from './index.ts';
/** Engine operation applied to one admitted source; its JSON form distinguishes cached outputs. */
export type OfficeOperation = {
    readonly kind: 'pdf';
} | {
    readonly kind: 'images';
    readonly pages?: readonly number[];
    readonly sheet?: string;
    readonly range?: string;
    readonly dpi?: number;
    readonly maxImages: number;
    readonly maxPixels: number;
    readonly maxDimension: number;
    readonly maxBytes: number;
} | {
    readonly kind: 'convert';
    readonly format: OfficeConversionFormat;
    readonly sheet?: string;
} | {
    readonly kind: 'recalculate';
    readonly format: OfficeRecalculationFormat;
};
/** Complete engine output for one operation kind. */
export type OfficeOperationOutput = {
    readonly kind: 'pdf';
    readonly pdf: Uint8Array;
    readonly missingFonts: string[];
} | {
    readonly kind: 'images';
    readonly images: readonly OfficeImage[];
    readonly pageCount: number;
    readonly dpi: number;
    readonly missingFonts: string[];
} | {
    readonly kind: 'file';
    readonly data: Uint8Array;
    readonly pageCount?: number;
    readonly missingFonts: string[];
};
/** Output kind produced by each operation. */
export type OutputOf<O extends OfficeOperation> = Extract<OfficeOperationOutput, {
    kind: O['kind'] extends 'convert' | 'recalculate' ? 'file' : O['kind'];
}>;
/** One admitted operation on an authorized source. */
export interface QueueRequest<O extends OfficeOperation = OfficeOperation> {
    readonly extension: string;
    readonly priority: OfficeKitPriority;
    readonly source: OfficeKitSource;
    readonly operation: O;
}
/** Caller-owned output with its converter-owned identity. */
export type QueueResult<O extends OfficeOperation = OfficeOperation> = OutputOf<O> & {
    readonly cacheKey: OfficeKitKey;
    readonly generation: OfficeKitGeneration;
};
type Convert = (bytes: Uint8Array, request: Pick<QueueRequest, 'extension' | 'operation'>, signal: AbortSignal) => Promise<OfficeOperationOutput>;
/** One converter generation owns every queued source, conversion, reader, and retained PDF. */
export declare class ConversionQueue {
    private readonly config;
    private readonly generation;
    private readonly convert;
    private readonly ready;
    private readonly aliases;
    private readonly sources;
    private readonly digests;
    private readonly queue;
    private readonly tasks;
    private readonly jobs;
    private cachedBytes;
    private sourceBytes;
    private running;
    private background;
    private readers;
    private disposed;
    /**
     * @param config - validated queue, source, reader, and completed-result limits.
     * @param generation - provider lifetime; prevents reuse after engine or font replacement.
     * @param convert - executes one admitted conversion and settles after scratch cleanup.
     */
    constructor(config: Config, generation: OfficeKitGeneration, convert: Convert);
    /**
     * Admit metadata before loading source bytes and share conversion across authorized readers.
     * @param request - already-authorized metadata and deferred bounded source read.
     * @param signal - this reader's cancellation; the final reader cancels shared work.
     * @returns independent output bytes; busy or canceled readers reject without releasing active engine capacity early.
     */
    read<O extends OfficeOperation>(request: QueueRequest<O>, signal?: AbortSignal): Promise<QueueResult<O>>;
    private readStored;
    /** Cancel all readers and wait until actual reads, conversions, and scratch cleanup finish. */
    dispose(): Promise<void>;
    private busy;
    private unavailable;
    private copy;
    private release;
    private cancel;
    private fail;
    private reservation;
    private drain;
    private execute;
    private finish;
    private retain;
}
export {};
//# sourceMappingURL=queue.d.ts.map