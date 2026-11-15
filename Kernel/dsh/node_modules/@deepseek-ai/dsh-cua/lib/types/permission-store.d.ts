import type { Context } from '@deepseek-ai/cordis';
import { z } from 'zod';
import type { CuaApplicationGrant, CuaApplicationId } from './application-types.ts';
declare const application: z.ZodObject<{
    revoked: z.ZodNumber;
    always: z.ZodOptional<z.ZodObject<{
        id: z.ZodPipe<z.ZodString, z.ZodTransform<CuaApplicationId, string>>;
        name: z.ZodString;
        executablePath: z.ZodOptional<z.ZodString>;
    }, z.core.$strip>>;
    sessions: z.ZodRecord<z.ZodString, z.ZodObject<{
        identity: z.ZodObject<{
            id: z.ZodPipe<z.ZodString, z.ZodTransform<CuaApplicationId, string>>;
            name: z.ZodString;
            executablePath: z.ZodOptional<z.ZodString>;
        }, z.core.$strip>;
        decision: z.ZodEnum<{
            allow: "allow";
            deny: "deny";
            ask: "ask";
        }>;
        revision: z.ZodNumber;
    }, z.core.$strip>>;
}, z.core.$strip>;
declare const state: z.ZodObject<{
    migrated: z.ZodBoolean;
    applications: z.ZodRecord<z.ZodString, z.ZodObject<{
        revoked: z.ZodNumber;
        always: z.ZodOptional<z.ZodObject<{
            id: z.ZodPipe<z.ZodString, z.ZodTransform<CuaApplicationId, string>>;
            name: z.ZodString;
            executablePath: z.ZodOptional<z.ZodString>;
        }, z.core.$strip>>;
        sessions: z.ZodRecord<z.ZodString, z.ZodObject<{
            identity: z.ZodObject<{
                id: z.ZodPipe<z.ZodString, z.ZodTransform<CuaApplicationId, string>>;
                name: z.ZodString;
                executablePath: z.ZodOptional<z.ZodString>;
            }, z.core.$strip>;
            decision: z.ZodEnum<{
                allow: "allow";
                deny: "deny";
                ask: "ask";
            }>;
            revision: z.ZodNumber;
        }, z.core.$strip>>;
    }, z.core.$strip>>;
}, z.core.$strip>;
/** One application's grants and revocation generations. */
export type ApplicationPermission = z.infer<typeof application>;
/** Validated authoritative state; an empty migrated store never reimports legacy grants. */
export type PermissionState = z.infer<typeof state>;
/** One locked storage operation, with writes resolving only after durability. */
export interface PermissionTransaction {
    state: PermissionState;
    save: (next: PermissionState) => Promise<void>;
}
/**
 * Open fresh domain state under a profile lock for every decision and native operation.
 * Without a profile, only process-local session decisions are writable. A profile requires domain storage.
 */
export declare class CuaPermissionStore {
    private readonly ctx;
    private readonly legacy;
    private readonly waitMs;
    private readonly directory;
    private memory;
    private readonly pending;
    private tail;
    /**
     * @param ctx Provider context with optional profile and domain storage services.
     * @param legacy Grants imported on the first access to this profile's store.
     * @param waitMs Existing operation budget, also bounding profile-lock contention.
     */
    constructor(ctx: Context, legacy: readonly CuaApplicationGrant[], waitMs: number);
    /** Whether this composition can retain application and session grants across restarts. */
    get persistent(): boolean;
    /**
     * Serialize a read, write, or authorized dispatch against all other users of this profile.
     * @param signal Cancellation while waiting; admitted operations own their cancellation.
     * @param operation Work using fresh state. Native dispatch uses its separate application lease.
     * @returns The operation's result after its domain handle has closed.
     */
    withState<T>(signal: AbortSignal, operation: (transaction: PermissionTransaction) => T | Promise<T>): Promise<T>;
    /**
     * Coordinate native dispatch and whole-application revocation without blocking other applications.
     * @param id Validated native application identity.
     * @param signal Cancellation while waiting for admission.
     * @param operation Authorized dispatch or revocation, after checking fresh state separately.
     * @returns Result after releasing the application lease.
     */
    withApplication<T>(id: CuaApplicationId, signal: AbortSignal, operation: () => Promise<T>): Promise<T>;
    /** Join accepted operations before withdrawing this provider. */
    drain(): Promise<void>;
    private operate;
    private importLegacy;
}
export {};
//# sourceMappingURL=permission-store.d.ts.map