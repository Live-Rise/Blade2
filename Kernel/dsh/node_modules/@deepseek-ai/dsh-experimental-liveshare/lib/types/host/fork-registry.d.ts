/**
 * Durable record of the child sessions a fork created, under either share tier.
 *
 * The registry is what makes a fork isolatable: it knows which share and which viewer produced a
 * child, so a viewer's list shows only its own children, a revoked share stops its children, and a
 * child's prompt can be refused for a reason the Host can name. It also holds the ceiling the child
 * runs under, and refuses every prompt until that ceiling is installed and verified.
 *
 * One DSH home can be served by several Host processes, so every change is committed as a
 * read-modify-write under the cross-process writer lock of `@deepseek-ai/dsh-atomic-write`: the file
 * is re-read inside the lock, this process's own changes are applied to what it holds, and the result
 * replaces the file in one atomic step. A change therefore cannot be dropped by a concurrent write
 * from another process, and an entry another process changed is never restored from this process's
 * older copy.
 */
import type { ForkCeiling } from './fork-policy.ts';
/** Trusted creator identity of one child session. */
export interface ForkCreator {
    readonly providerId: string;
    readonly tenantId: string;
    readonly subjectId: string;
}
/** One recorded child session and where it came from. */
export interface ForkRecord {
    readonly childSessionId: string;
    /** Share that authorized the fork. */
    readonly shareId: string;
    /** Session the child was forked from; the source is never modified. */
    readonly sourceSessionId: string;
    /** Completed prefix the child started from. */
    readonly boundary: number;
    readonly creator: ForkCreator;
    readonly createdAt: string;
    /** Set when the source share stopped, expired, or was revoked; the child is kept, not deleted. */
    readonly revokedAt?: string;
}
/** Why one child session may not run a prompt. */
export type ForkPromptRefusal = 'fork-unknown-child' | 'fork-viewer-mismatch' | 'fork-source-revoked' | 'fork-ceiling-uninstalled'
/** Preparing a cold child through the owner's service failed, so nothing was admitted. */
 | 'fork-prepare-failed';
/** Whether one child may accept a prompt. */
export type ForkPromptDecision = {
    readonly ok: true;
} | {
    readonly ok: false;
    readonly code: ForkPromptRefusal;
    readonly message: string;
};
/** One logical fork request this host already served, so a retry resolves to its child. */
export interface ForkOperation {
    /** Identity of the logical request: trusted viewer, share, source, and the caller's operation id. */
    readonly key: string;
    /** Child this request produced; empty while the request is only reserved. */
    readonly childSessionId: string;
    /** Boundary the request named, which a retry of the same identity must repeat. */
    readonly boundary: number;
    /**
     * Whether the child was left ready to be driven.
     *
     * A request whose child was created but could not be given read-only limits is remembered so a retry
     * does not fork again, and this flag keeps that record from ever reading as a success.
     */
    readonly ready: boolean;
    /** The owner's own answer, replayed verbatim so a retry reports the child it already created. */
    readonly status: number;
    readonly body: unknown;
}
/**
 * Child-session registry with owner-only persistence and a default-deny prompt gate.
 *
 * Every mutation resolves once the file holds it and rejects when it could not be written, so a caller
 * that must not continue on an unwritten record — a reserved request, a withdrawn share — can await it
 * and stop. The registry's own reads stay in memory: the file is read at construction and each write
 * re-reads it, but an entry another process adds is not merged into this instance's maps.
 */
export declare class ForkRegistry {
    private readonly file;
    private readonly records;
    /** Logical requests already served, keyed by identity; a retry resolves to the recorded child. */
    private readonly operations;
    /** Changes this process made that the file does not hold yet, keyed as the document keys them. */
    private readonly pendingForks;
    private readonly pendingOperations;
    /**
     * Shares this process withdrew and has not recorded a child for since.
     *
     * Only a repeated withdrawal of the same share is suppressed: the first batch already withdrew every
     * entry the file held, and a child recorded afterwards is a new child, not one the withdrawal named.
     * The marker is dropped when a new child is recorded, so withdrawing the share again (after its fork
     * switch was turned back on) withdraws that child too.
     */
    private readonly withdrawnShares;
    /** Withdrawals this process still has to commit, keyed by share; cleared once a batch carries them. */
    private readonly pendingRevocations;
    /** Runs this process's batches one at a time, in the order the changes were made. */
    private chain;
    /**
     * Ceiling enforced for each child that actually has one installed.
     *
     * Enforcement is per child, not per host: a guard registers on one child's own scoped context, so a
     * successful install for one child says nothing about its siblings, and a child restored from the
     * registry file after a restart has no live guard until one is registered again. A single host-wide
     * flag would let the first child to install open every other child's prompt.
     */
    private readonly enforced;
    /**
     * @param file JSON file this registry reads at construction and rewrites on every change.
     */
    constructor(file: string);
    /**
     * The child one logical fork request already produced.
     * @param key - identity of that request.
     * @returns The recorded operation, or undefined when this host never served it.
     */
    operationOf(key: string): ForkOperation | undefined;
    /**
     * Record one served fork under its logical identity, so a retry resolves to the same child.
     * @param operation - The identity, its boundary, and the owner's answer to replay.
     * @returns A promise that rejects when the record could not be written.
     */
    recordOperation(operation: ForkOperation): Promise<void>;
    /**
     * Bind one logical request to the boundary it named, before any child exists.
     *
     * A durable binding is what makes a later request of the same identity naming another boundary a
     * conflict rather than a second fork, even when the process that reserved it never finished creating
     * the child. A reservation is replaced by {@link recordOperation} once the child exists.
     * @param key - Identity of the logical request.
     * @param boundary - Normalized boundary that request selected.
     * @returns A promise that rejects when the reservation could not be written.
     */
    reserveOperation(key: string, boundary: number): Promise<void>;
    /**
     * Record that one child's own execution limit is installed.
     *
     * The caller registers that child's scoped guard first and calls this only after it returns, so the
     * record states a fact about the child rather than an intention about the host.
     * @param childSessionId Child whose limit was installed.
     * @param ceiling Ceiling its guard enforces.
     * @returns False when this host never recorded that child, so a failed install is distinguishable.
     */
    installEnforcement(childSessionId: string, ceiling: ForkCeiling): boolean;
    /**
     * Record that one child's guard is gone, so its prompts are refused again.
     * @param childSessionId Child whose guard was disposed.
     */
    releaseEnforcement(childSessionId: string): void;
    /**
     * Whether one child's own execution limit is installed.
     * @param childSessionId Child to check.
     * @returns True only while that child has a live guard.
     */
    enforcementInstalled(childSessionId: string): boolean;
    /**
     * Record one child session, replacing an identical previous record.
     * @param record Child, its source, its creator, and the boundary it started from.
     * @returns A promise that rejects when the record could not be written.
     */
    record(record: ForkRecord): Promise<void>;
    /**
     * Find one child session.
     * @param childSessionId Child session to look up.
     * @returns The record, or undefined when this Host never created that child.
     */
    find(childSessionId: string): ForkRecord | undefined;
    /**
     * Children one viewer created, which is what the viewer's own list may show.
     * @param creator Trusted viewer identity the Relay asserted.
     * @returns That viewer's children, newest last.
     */
    listForCreator(creator: ForkCreator): readonly ForkRecord[];
    /**
     * Children one share produced, for the source owner's view.
     * @param shareId Source share.
     * @returns Every child of that share, whoever created it.
     */
    listForShare(shareId: string): readonly ForkRecord[];
    /**
     * Mark every child of one share revoked without deleting it, so history and audit survive.
     * @param shareId Source share that stopped, expired, or lost authorization.
     * @param now ISO time of the revocation.
     * @returns A promise that rejects when the withdrawal could not be written.
     */
    revokeShare(shareId: string, now: string): Promise<void>;
    /**
     * Decide whether one child session may run a prompt.
     * @param childSessionId Child session the prompt targets.
     * @param viewer Trusted viewer identity the Relay asserted.
     * @returns Allowed only for that child's creator, with the source live and the ceiling installed.
     */
    allowsPrompt(childSessionId: string, viewer: ForkCreator): ForkPromptDecision;
    /**
     * Ceiling installed for one child, for the caller that has to apply it.
     * @param childSessionId Child whose ceiling is asked for.
     * @returns That child's ceiling, or undefined while its prompts are refused.
     */
    enforcedCeiling(childSessionId: string): ForkCeiling | undefined;
    /**
     * Read the registry at construction.
     *
     * A file that cannot be read or understood reads as empty here rather than failing the Host's start;
     * the write path refuses that same file, so nothing replaces content it could not read.
     * @returns The entries the file holds, or nothing.
     */
    private load;
    /**
     * Commit every change this process made since the previous batch.
     *
     * Batches run one at a time in call order, so a later change can never be undone by an earlier one.
     * @returns A promise that rejects when the file could not be written, keeping that batch pending for
     * the next change to retry.
     */
    private persist;
    /**
     * Apply one batch to the file, holding the cross-process writer lock for the whole cycle.
     *
     * The file is re-read inside the lock and the batch is applied to what it holds, so an entry no batch
     * touched survives whatever another process wrote since this instance last read, and an entry this
     * process changed is written on top of that other write instead of replacing it with an older copy.
     * @param delta - This process's changes.
     * @returns A promise that resolves once the replace has committed.
     */
    private commit;
    /**
     * Put a batch that failed to commit back among the pending changes, unless a later change already
     * replaced those entries, so the next mutation writes them again instead of dropping them.
     * @param delta - The batch whose commit failed.
     */
    private retain;
}
//# sourceMappingURL=fork-registry.d.ts.map