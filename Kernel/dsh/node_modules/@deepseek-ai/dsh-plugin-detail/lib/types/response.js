/** HTTP status of each detail failure. */
const FAILURE_STATUS = {
    'too-large': 413, 'network': 502, 'denied': 403, 'git-unavailable': 503, 'invalid': 400,
};
/**
 * The response of an image route: the image, 404 when there is none, or the failure's status with its reason as the
 * body. Every response carries `X-Content-Type-Options: nosniff` and a sandbox content security policy, because an SVG
 * opened directly would otherwise run scripts on the authenticated API origin; only an image gets `cacheControl`.
 * @param outcome - the image, undefined when none exists, or why it cannot be read; `invalid` also answers a malformed request.
 * @param cacheControl - the `Cache-Control` value of an image.
 * @returns the response.
 */
export function detailImageResponse(outcome, cacheControl) {
    const [status, body, type] = typeof outcome === 'object'
        // A copy into a fresh buffer is a body `Response` accepts whatever buffer the bytes view.
        ? [200, new Uint8Array(outcome.bytes), outcome.mediaType]
        : outcome === undefined ? [404, 'not found', undefined] : [FAILURE_STATUS[outcome], outcome, undefined];
    return new Response(body, {
        status,
        headers: {
            'Content-Type': type ?? 'text/plain; charset=utf-8',
            'Cache-Control': status === 200 ? cacheControl : 'no-store',
            'X-Content-Type-Options': 'nosniff',
            'Content-Security-Policy': "sandbox; default-src 'none'; style-src 'unsafe-inline'",
        },
    });
}
//# sourceMappingURL=response.js.map