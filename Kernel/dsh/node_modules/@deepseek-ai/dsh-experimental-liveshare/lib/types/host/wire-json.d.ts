/**
 * JSON payload helpers shared by the owner's HTTP adapters and Relay reads.
 * @module @deepseek-ai/dsh-experimental-liveshare/src/host/wire-json
 */
/**
 * Answer one JSON payload with the content type the owner routes use.
 * @param value - Payload to serialize.
 * @param status - HTTP status to answer with.
 * @returns The response carrying the serialized payload.
 */
export declare function json(value: unknown, status: number): Response;
/**
 * Whether one value is a plain record, the only object shape wire payloads may take.
 * @param value - Value read from a wire payload.
 * @returns Whether the value is a non-array object.
 */
export declare function isRecord(value: unknown): value is Record<string, unknown>;
/**
 * Read one JSON request body.
 * @param request - Owner request that may carry JSON.
 * @returns Parsed body, or undefined when the body is absent or malformed.
 */
export declare function readJsonBody(request: Request): Promise<unknown>;
//# sourceMappingURL=wire-json.d.ts.map