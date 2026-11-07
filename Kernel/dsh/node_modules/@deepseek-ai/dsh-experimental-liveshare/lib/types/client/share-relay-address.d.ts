/**
 * Relay address state for the owner share dialog.
 *
 * The owner host is the authority on the address in force: it names the saved address, stores a new
 * one, and answers the connectivity and permission checks the dialog's ordered flow runs. This hook
 * keeps those facts and the owner's draft; the step guide itself belongs to the dialog.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/share-relay-address
 */
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { SessionId } from '@deepseek-ai/dsh-session/types';
import type { NS } from './locales.ts';
import type { OwnerShares } from './owner-shares.ts';
/** What the connectivity step has established for the address in force; credentials play no part. */
export type RelayConnectivity = 'idle' | 'testing' | 'ok' | 'unreachable';
/**
 * What the permission step has established for this host at that address.
 *
 * `blocked` is the Relay's refusal of this host's credential, which the owner acts on by verifying
 * again; `warn` is a failure the Relay did not decide, which keeps the gateway's retry.
 */
export type RelayPermission = 'idle' | 'checking' | 'ok' | 'blocked' | 'warn';
/**
 * The identity facts saving an address needs: whether the owner holds a verified credential at the
 * address in force, and how to re-read that status once the host stored another address.
 */
export interface RelaySaveIdentity {
    /** Whether the host reported a verified identity for the address in force. */
    readonly verified: boolean;
    /** Re-read the identity status for the address the save put in force. */
    readonly refresh: () => void;
}
/** Inputs the share dialog supplies to its Relay address state. */
export interface ShareRelayAddressOptions {
    /** Owner share management the dialog addresses. */
    readonly shares: OwnerShares;
    /** Session whose publish permission the permission step checks. */
    readonly sessionId: SessionId;
    /** Address the mounted share host reported as saved; empty while none is saved. */
    readonly defaultRelay: string;
    /** Localized copy. */
    readonly t: PropsLocale<typeof NS>['t'];
}
/** Relay address state the dialog's first step and its two checks present. */
export interface ShareRelayAddress {
    /** Address the owner is editing. */
    readonly draft: string;
    /** Whether a save is in flight. */
    readonly busy: boolean;
    /** Why the last save did not land, already localized. */
    readonly error: string;
    /** What the last save put in force, already localized. */
    readonly notice: string;
    /** Set when a save replaced an address that already held a verified identity. */
    readonly reverify: boolean;
    /** Address the host itself named as in force, before the header action's earlier probe. */
    readonly effectiveRelay: string | undefined;
    /** Whether the host's own read named the address in force. */
    readonly named: boolean;
    /** Address this dialog presents as in force. */
    readonly effective: string;
    /** What the connectivity step has established for that address. */
    readonly connectivity: RelayConnectivity;
    /** The Host's own connectivity text, printed unchanged when it stated one. */
    readonly connectivityError: string;
    /** What the permission step has established for that address. */
    readonly permission: RelayPermission;
    /** The Host's own permission text, printed unchanged when it stated one. */
    readonly permissionError: string;
    /** Whether either step's check is in flight. */
    readonly testing: boolean;
    /** Whether the permission step is settled enough to create. */
    readonly permissionPassed: boolean;
    /**
     * Whether the field holds an address of the owner's own that is not the one in force.
     *
     * A typed address equal to the one in force is not a change, whatever its source; an empty field
     * is not a change either, because it is the state before any host answer.
     * @returns True only when saving is what puts the field's address in force.
     */
    readonly draftUnsaved: () => boolean;
    /** Record an edit to the address field, which the host's own answer must no longer replace. */
    readonly editDraft: (value: string) => void;
    /** Clear or set the note that a saved address needs verifying again. */
    readonly setReverify: (value: boolean) => void;
    /** Clear the permission result that belonged to a credential a sign-in replaced. */
    readonly resetPermission: () => void;
    /**
     * Ask whether the address in force answers at all.
     *
     * This step presents no credential: a server that refuses an anonymous request has still answered,
     * so only a transport failure makes the address unreachable.
     * @returns True when the address answered.
     */
    readonly testConnectivity: () => Promise<boolean>;
    /**
     * Ask whether this host may publish a share at the address in force.
     *
     * The Host decides the severity: a refusal blocks creation until the owner verifies again, while a
     * failure the Relay did not decide keeps the gateway's retry and only warns.
     * @returns True when creation may proceed.
     */
    readonly testPermission: () => Promise<boolean>;
    /**
     * Save the address the owner typed and read the host's answer back.
     * @param relayUrl - Address to put in force.
     * @param identity - Whether the owner was verified at the previous address, and how to re-read it.
     */
    readonly save: (relayUrl: string, identity: RelaySaveIdentity) => Promise<void>;
}
/**
 * Read, save, and check the Relay address the owner shares through.
 * @param options - Owner share management, the Session, the address prefill, and localized copy.
 * @returns The address in force, the owner's draft, and both checks.
 */
export declare function useShareRelayAddress(options: ShareRelayAddressOptions): ShareRelayAddress;
//# sourceMappingURL=share-relay-address.d.ts.map