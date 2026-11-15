/**
 * Refuse missing, link-shaped, oversized, and truncated output.
 * @param path - kit output path in the provider-owned scratch directory.
 * @param limit - inclusive byte limit.
 * @param signal - conversion lifetime.
 * @returns complete bytes independent of the scratch file.
 * @throws {OfficeKitError} `invalid-output` for a non-regular or changing file and `output-too-large` above `limit`.
 */
export declare function readOutput(path: string, limit: number, signal: AbortSignal): Promise<Buffer>;
/**
 * Refuse missing, link-shaped, oversized, truncated, and non-PDF output.
 * @param path - kit output path in the provider-owned scratch directory.
 * @param limit - inclusive PDF byte limit.
 * @param signal - conversion lifetime.
 * @returns complete bytes independent of the scratch file.
 */
export declare function readPdf(path: string, limit: number, signal: AbortSignal): Promise<Uint8Array>;
/**
 * Refuse missing, link-shaped, oversized, and non-PNG image output.
 * @param path - kit image path in the provider-owned scratch directory.
 * @param limit - inclusive PNG byte limit.
 * @param signal - rendering lifetime.
 * @returns complete PNG bytes independent of the scratch file.
 */
export declare function readPng(path: string, limit: number, signal: AbortSignal): Promise<Uint8Array>;
//# sourceMappingURL=output.d.ts.map