/**
 * The name the owner's own identity carries, shared with surfaces that do not
 * receive the owner status as props.
 *
 * The durable message source names a reader by subject id alone. When that
 * reader is the owner sending through their own viewer page, the owner host
 * already knows the display name; this store lets the attribution line use it
 * without persisting a display name into Session logs.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/owner-display
 */
import type { OwnerShares } from './owner-shares.ts';
/** Display facts one verified owner identity carries. */
export interface OwnerDisplay {
    /** Subject the owner identity belongs to. */
    readonly subjectId?: string;
    /** Verified display name; absent when the identity published none. */
    readonly displayName?: string;
}
/**
 * Record the owner identity the host reported.
 * @param identity - Subject and display name, when the host reported a verified identity.
 */
export declare function rememberOwnerDisplay(identity: OwnerDisplay): void;
/** Forget the name when the face unloads or the identity stops being verified. */
export declare function forgetOwnerDisplay(): void;
/**
 * Read the owner identity the host reports and remember its display name.
 *
 * A host or carrier without the read leaves the store empty; a read that fails
 * or is no longer verified forgets a stale name instead of keeping it.
 * @param shares - Owner share management carrying the identity read.
 * @returns Nothing; the store holds the result.
 */
export declare function rememberOwnerIdentity(shares: OwnerShares): Promise<void>;
/**
 * The owner's display facts for components that cannot receive the owner hook.
 * @returns The current owner display facts.
 */
export declare function useOwnerDisplay(): OwnerDisplay;
//# sourceMappingURL=owner-display.d.ts.map