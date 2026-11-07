/** Canonical identities for change requests. */
import type { ChangeRequestKey, ChangeRequestRef } from './types.ts';
/**
 * Build the canonical identity of a change request. Host, owner, and name
 * compare case-insensitively, as they do on the hosts this vocabulary serves.
 * @param ref - change request to identify.
 * @returns `<forge>:<host>/<owner>/<name>#<number>` in lowercase.
 */
export declare function changeRequestKey(ref: ChangeRequestRef): ChangeRequestKey;
//# sourceMappingURL=keys.d.ts.map