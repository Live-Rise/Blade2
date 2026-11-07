/** HTTP responses of Host routes that serve images beside detail documents. */
import type { DetailFailureReason } from './types.ts';
/** An image read beside a detail document. */
export interface DetailImage {
    readonly bytes: Uint8Array;
    readonly mediaType: string;
}
/**
 * The response of an image route: the image, 404 when there is none, or the failure's status with its reason as the
 * body. Every response carries `X-Content-Type-Options: nosniff` and a sandbox content security policy, because an SVG
 * opened directly would otherwise run scripts on the authenticated API origin; only an image gets `cacheControl`.
 * @param outcome - the image, undefined when none exists, or why it cannot be read; `invalid` also answers a malformed request.
 * @param cacheControl - the `Cache-Control` value of an image.
 * @returns the response.
 */
export declare function detailImageResponse(outcome: DetailImage | DetailFailureReason | undefined, cacheControl: string): Response;
//# sourceMappingURL=response.d.ts.map