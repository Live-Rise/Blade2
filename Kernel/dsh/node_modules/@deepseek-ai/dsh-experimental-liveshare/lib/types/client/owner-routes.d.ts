/**
 * Owner endpoints the share surfaces still call over HTTP.
 *
 * Share management moved to the `liveshareShares` Remote. The sign-in round trip stays a
 * same-origin route because the browser opens the Relay's authorization page and the Host redeems
 * the code it hands back, which no Remote method mediates.
 *
 * The served document carries the application's own base, so a browser call that names a route from
 * the root leaves a mounted deployment behind. Every entry here is the path the owner host registers
 * with its leading slash stripped; `verify-client-route-resolution` keeps the two forms apart.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/owner-routes
 */
/** Same-origin owner endpoints the share sign-in calls. */
export declare const OWNER_ROUTES: {
    readonly ownerLoginCancel: "api/liveshare.ownerLoginCancel";
    readonly ownerLoginRedeem: "api/liveshare.ownerLoginRedeem";
    readonly ownerLoginStart: "api/liveshare.ownerLoginStart";
};
//# sourceMappingURL=owner-routes.d.ts.map