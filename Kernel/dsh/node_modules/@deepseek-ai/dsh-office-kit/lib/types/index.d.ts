import { Context } from '@deepseek-ai/cordis';
import z from '@deepseek-ai/schemastery';
import type { WorkspaceFileScope } from '@deepseek-ai/dsh-api-workspace-files';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import { OfficeKitGeneration } from './identity.ts';
import type { OfficeConversionRequest, OfficeFileResult, OfficeImagesRequest, OfficeImagesResult, OfficeKitRequest, OfficeKitResult, OfficeKitPriority, OfficeRecalculationRequest, RenderedDocumentBytes } from './types.ts';
export * from './errors.ts';
export * from './identity.ts';
export * from './types.ts';
declare module '@deepseek-ai/cordis' {
    interface Context {
        /** Shared Office rendering, conversion, recalculation, and authorized workspace-file previews. */
        officeKit: OfficeKit;
    }
}
/** Provider concurrency and kit rendering/font configuration. */
export interface Config {
    /** Maximum simultaneous conversions; queued callers remain cancellable. */
    maxConcurrentConversions: number;
    /** Maximum metadata-only jobs awaiting source admission. */
    maxQueuedJobs: number;
    /** Maximum outstanding conversion readers. */
    maxReaders: number;
    /** Maximum reserved bytes across admitted source reads and conversions. */
    maxSourceBytes: number;
    /** Maximum concurrent background jobs; zero refuses speculative work. */
    maxBackgroundConversions: number;
    /** Maximum retained content-addressed outputs. */
    maxCachedEntries: number;
    /** Maximum retained output bytes. */
    maxCachedBytes: number;
    /** Maximum retained source-version aliases to cached content. */
    maxSourceEntries: number;
    /** Absolute directory for converted and recalculated files, with one subdirectory per Session. */
    outputDirectory: string;
    /** Milliseconds a converted or recalculated file is kept. */
    outputRetentionMs: number;
    /** Maximum bytes across retained converted and recalculated files; the oldest leave first. */
    maxOutputStoreBytes: number;
    /** Conversion deadline in milliseconds; excludes the DSH queue. */
    timeoutMs: number;
    /** Maximum authorized source bytes. */
    maxInputBytes: number;
    /** Maximum complete PDF, converted, or recalculated file bytes. */
    maxOutputBytes: number;
    /** Image resolution in DPI of PDF exports; `renderImages()` takes its resolution from each request's `dpi`. */
    maxImageResolution: number;
    /** Maximum OOXML ZIP entries. */
    maxArchiveEntries: number;
    /** Maximum total declared uncompressed OOXML bytes. */
    maxUncompressedBytes: number;
    /** Absolute font roots; omission uses the kit's platform defaults. */
    fontDirectories?: string[];
    /** Ordered font-family preference groups; omission retains the kit defaults. */
    fontFallbacks?: string[][];
    /** Maximum physical font files indexed by each converter. */
    maxFontFiles: number;
    /** Maximum individual font-file bytes. */
    maxFontFileBytes: number;
    /** Maximum original font bytes loaded for a conversion. */
    maxLoadedFontBytes: number;
}
/** Deployment defaults resolved before provider construction. */
export declare const Config: z<Partial<Config>, Config>;
/** A provider lifetime owns all converters, queued calls, and temporary files. */
export declare class OfficeKit extends TypertRemoteService {
    private readonly config;
    static Config: z<Partial<Config>, Config>;
    /** Changes whenever engine, font, or conversion configuration is replaced. */
    readonly generation: OfficeKitGeneration;
    /** Absolute Host directory holding converted and recalculated files; consumers check that their execution world reads it. */
    readonly outputDirectory: string;
    private readonly remoteLifetime;
    private readonly remoteRequests;
    private readonly slots;
    private readonly queue;
    private readonly outputs;
    private readonly options;
    private loadedKit?;
    private readonly reportedKitFailures;
    /**
     * @param ctx - owning Host context.
     * @param config - resolved rendering, font, and concurrency limits.
     */
    constructor(ctx: Context, config: Config);
    /**
     * Convert Office bytes to PDF without modifying the source or writing Session events.
     * @param request - authorized metadata and deferred bounded source read.
     * @param signal - caller cancellation; provider disposal also stops active work.
     * @returns caller-owned PDF bytes after conversion and scratch cleanup settle; canceled readers reject independently.
     * @throws {OfficeKitError} Invalid input, unusable output, or engine failure; cancellation rejects with its reason.
     */
    convert(request: OfficeKitRequest, signal?: AbortSignal): Promise<OfficeKitResult>;
    /**
     * Render selected pages, slides, or worksheet areas of one document as PNG images. Requests share the admission
     * queue, concurrency, source-byte reservation, and cache with previews, and run at foreground priority.
     * @param request - authorized metadata, deferred bounded source read, selection, and complete-result limits.
     * @param signal - caller cancellation; provider disposal also stops active work.
     * @returns caller-owned images in document order whose count, per-image size, and total bytes satisfy the limits.
     * @throws {OfficeKitError} `invalid-request` for a selection the engine rejects, `output-too-large` when the
     * selection exceeds a limit, and the other conversion codes; cancellation rejects with its reason.
     */
    renderImages(request: OfficeImagesRequest, signal?: AbortSignal): Promise<OfficeImagesResult>;
    /**
     * Convert one document to another format at foreground priority, sharing admission, concurrency, and cache with
     * previews, and save the result in the provider's output store.
     * @param request - authorized metadata, deferred bounded source read, target format, and output owner and name.
     * @param signal - caller cancellation; provider disposal also stops active work.
     * @returns the saved file's absolute Host path and size; the file is deleted after `outputRetentionMs`, when older
     * outputs need `maxOutputStoreBytes`, when the Session whose id is `output.owner` is disposed, or when the provider is disposed.
     * @throws {OfficeKitError} `invalid-request` for a format or sheet the engine rejects and the other conversion
     * codes; cancellation rejects with its reason.
     */
    convertTo(request: OfficeConversionRequest, signal?: AbortSignal): Promise<OfficeFileResult>;
    /**
     * Recalculate every formula of one workbook at foreground priority and save formulas with refreshed cached results, sharing admission,
     * concurrency, and cache with previews, and save the workbook in the provider's output store. This does not validate
     * formulas or business data.
     * @param request - authorized workbook metadata, deferred bounded source read, output format, and output owner and name.
     * @param signal - caller cancellation; provider disposal also stops active work.
     * @returns the saved workbook's absolute Host path and size, retained like {@link convertTo} outputs.
     * @throws {OfficeKitError} Invalid input, unusable output, or engine failure; cancellation rejects with its reason.
     */
    recalculate(request: OfficeRecalculationRequest, signal?: AbortSignal): Promise<OfficeFileResult>;
    private saveFile;
    /**
     * Read and convert one Office file using the Session's ordinary filesystem authorization.
     * @param workspaceFileScope - Session header lookup shared with workspaceFiles.
     * @param path - absolute or workspace-relative Office path.
     * @param priority - foreground preview or speculative background work.
     * @param signal - Remote cancellation; disposal also cancels outstanding reads and conversions.
     * @returns complete PDF bytes with original source identity and missing font families.
     */
    render(workspaceFileScope: WorkspaceFileScope, path: string, priority: OfficeKitPriority, signal: AbortSignal): Promise<RenderedDocumentBytes>;
    /**
     * Read the current rendering generation before reusing a Client PDF.
     * @param signal - Remote caller cancellation.
     * @returns provider lifetime, replaced with rendering, font, or engine configuration.
     */
    getGeneration(signal: AbortSignal): OfficeKitGeneration;
    private renderFile;
    /**
     * Locate and import Kit for a converter. A successful import is reused for the provider lifetime; a missing or
     * unloadable Kit rejects as `unavailable` without being retained, so the next converter request locates Kit again.
     */
    private loadKit;
    private importKit;
    /** Classify a Kit failure as `unavailable`, logging each distinct message and cause once per provider lifetime. */
    private unavailableKit;
    private runOperation;
    /** Run one engine operation in a private scratch directory and read its complete, bounded output. */
    private runEngine;
    private readFile;
    private renderPng;
}
export default OfficeKit;
//# sourceMappingURL=index.d.ts.map