/** Authenticated raw-byte upload route registered on the Connection fetch registry. */
import type { FileUploads } from './index.ts';
/**
 * Handle one authenticated raw-byte upload. A `mediaType` query parameter
 * stages the body as a prompt image; without it the body is stored verbatim.
 * @param service - Host upload service receiving streamed bytes.
 * @param request - authenticated HTTP request from Connection.
 * @returns JSON result using HTTP status 200 after request validation.
 */
export declare function handleFileUploadHttp(service: FileUploads, request: Request): Promise<Response>;
//# sourceMappingURL=http-route.d.ts.map