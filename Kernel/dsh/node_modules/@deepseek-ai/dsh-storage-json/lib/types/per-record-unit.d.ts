/**
 * One opened JSON unit in `per-record` layout: the unit is a directory at
 * `dir`, holding one document per record under `<dir>/<table>/<key>.json`
 * plus `global.json` for the global slot. The directory is the state — this
 * unit holds NO in-memory state of its own: `loadAll` re-reads the tree, each
 * put or delete touches one durable document, and a batch delete renames its
 * documents out of the readable set before unlinking them. The domain layer owns the live
 * in-memory tables (seeded by the open-time `loadAll`) and serializes writes
 * through its write chain. A rejected write leaves domain memory unchanged;
 * a failed deletion restores its staged documents or quarantines the unit
 * with `unknown-state` when restoration cannot be established.
 *
 * Per-record contract: a record document that is malformed or stamped with a
 * version outside the accepted set (the descriptor's current version plus
 * its `compatibleVersions`) reads as an absent record — one bad or stale
 * file never bricks the whole unit, and an unaccepted version stamp discards
 * the record instead of migrating it. Record keys become path segments, so
 * they must be path-safe (`[a-zA-Z0-9_-]+`); an unsafe key rejects at write.
 *
 * Legacy bootstrap: when the new tree has no declared table directory or
 * global document, a whole-unit file `<root>/<name>.json` seeds
 * per-record documents, provided its stored unit version is in the accepted
 * set — a legacy file stamped with any other version is left alone and reads
 * as the empty unit. An existing declared table directory, even when empty,
 * or a declared global document suppresses the bootstrap for the whole unit.
 * The legacy file is never changed or deleted.
 * @module @deepseek-ai/dsh-storage-json/src/per-record-unit
 */
import type { KvUnit, KvUnitDescriptor } from '@deepseek-ai/dsh-storage';
/**
 * Open one `per-record`-layout unit under `root`: the unit directory is
 * `<root>/<name>/`. Loads lazily on the first `loadAll` — this unit holds no
 * state, so opening touches nothing on the medium.
 * @param descriptor - Static identity and shape of the unit.
 * @param root - Absolute backend root directory.
 * @param onClose - Backend callback releasing the unit's open-slot.
 * @returns the opened unit.
 */
export declare function openPerRecordUnit(descriptor: KvUnitDescriptor, root: string, onClose: () => void): Promise<KvUnit>;
/**
 * One opened `per-record`-layout unit. Stateless by design: the directory is
 * the medium and the domain layer owns the live memory. Replacements publish
 * one document; deletions stage and sync their documents before cleanup.
 * Write ordering belongs to the caller (the domain layer's write chain).
 */
export declare class PerRecordJsonUnit implements KvUnit {
    private readonly descriptor;
    private readonly dir;
    private readonly onClose;
    private closed;
    /** Set when a failed batch could not restore its staged documents. */
    private unusable;
    /** In-flight durable writes; close() drains them before releasing the unit. */
    private readonly inFlight;
    constructor(descriptor: KvUnitDescriptor, dir: string, onClose: () => void);
    /** Re-read the tree: the directory is the authoritative state. */
    loadAll(): Promise<{
        tables: Record<string, Record<string, unknown>>;
        global: unknown;
    }>;
    /** Durably replace one record: its own document, atomically. */
    putRecord(table: string, key: string, value: unknown): Promise<void>;
    /** Durably delete one record. Idempotent: a missing key is a no-op. */
    deleteRecord(table: string, key: string): Promise<void>;
    /**
     * Durably delete many records as one batch. Documents are renamed out of the
     * readable set first; only after the whole batch is staged are the staged
     * copies unlinked. A failure while staging restores the documents it moved,
     * so a rejection normally leaves the batch unapplied. If that restoration
     * itself fails, the readable or durable state is unknown: the unit rejects with
     * `unknown-state` and every later operation on it rejects, while `close`
     * still releases it. Staged directory changes are synced before cleanup on
     * POSIX, so a resolved deletion remains absent after a crash. Cleanup residue
     * is not readable (`open` reads `*.json` documents only).
     */
    deleteRecords(table: string, keys: readonly string[]): Promise<void>;
    /**
     * Remove one batch's documents under a single tracked promise.
     * @param dir - Declared table directory.
     * @param keys - Validated, de-duplicated record keys.
     */
    private removeDocuments;
    /**
     * Move one record's document aside as `<key>.json.bak.<YYYYMMDDHHmm>`. The
     * moved file no longer ends in `.json`, so every later read ignores it; the
     * bytes stay on disk for inspection. A same-minute backup of the same
     * key overwrites the previous backup (the newer bytes are the ones worth
     * keeping).
     */
    backupRecord(table: string, key: string): Promise<string>;
    /** Durably replace the global singleton. Only valid when declared. */
    setGlobal(value: unknown): Promise<void>;
    /** Drain in-flight writes and release the unit. Idempotent. */
    close(): Promise<void>;
    private assertOpen;
    /** Resolve a declared table's directory; an undeclared table is a caller bug and throws. */
    private tableDir;
    /** Durably replace one document, creating its parent directory. */
    private writeDocument;
    /** Track one durable write so close() drains it. */
    private tracked;
}
//# sourceMappingURL=per-record-unit.d.ts.map