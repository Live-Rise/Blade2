/** Whitelisted operation records shared by diagnostic transport and durable readers. */
import { z } from 'zod';
const fields = {
    id: z.uuid().transform(value => value),
    time: z.iso.datetime(),
    reason: z.enum({
        'plugin': 'plugin',
        'bundle': 'bundle',
        'install': 'install',
        'remove': 'remove',
    }),
    target: z.string().max(256).nullable(),
    stage: z.enum({
        'install': 'install',
        'enable': 'enable',
        'remove': 'remove',
    }),
    enabled: z.boolean().optional(),
    version: z.string().max(128).optional(),
};
const operation = z.discriminatedUnion('phase', [
    z.object({ ...fields, phase: z.literal('started') }),
    z.object({ ...fields, phase: z.literal('unknown') }),
    z.object({ ...fields, phase: z.literal('settled'),
        application: z.enum({
            'applied': 'applied',
            'restart-required': 'restart-required',
            'overridden': 'overridden',
            'failed': 'failed',
            'cancelled': 'cancelled',
        }),
        changed: z.boolean(),
        errorCode: z.enum({
            'management-required': 'management-required',
            'unaddressable': 'unaddressable',
            'unknown-plugin': 'unknown-plugin',
            'invalid-spec': 'invalid-spec',
            'already-installed': 'already-installed',
            'ambiguous-install': 'ambiguous-install',
            'not-bundle': 'not-bundle',
            'not-removable': 'not-removable',
            'stop-profile': 'stop-profile',
            'bundle-in-use': 'bundle-in-use',
            'stale-approval': 'stale-approval',
            'incompatible-version': 'incompatible-version',
            'unsupported-platform': 'unsupported-platform',
            'operation-error': 'operation-error',
        }).optional(),
        restartReason: z.enum({
            'no-hmr': 'no-hmr',
            'package-moved': 'package-moved',
            'host-module': 'host-module',
            'unmanaged-plugin': 'unmanaged-plugin',
            'module-comparison': 'module-comparison',
        }).optional(),
    }),
]);
const record = z.object({
    runId: z.uuid().transform(value => value),
    operation,
    transportDropped: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER),
});
/**
 * Validate transport/file data and discard fields outside the diagnostic whitelist.
 * @param value - Untrusted decoded JSON.
 * @returns A bounded record, or undefined for malformed, oversized or unsupported records.
 */
export function parsePluginOperationRecord(value) {
    const parsed = record.safeParse(value);
    return parsed.success ? parsed.data : undefined;
}
//# sourceMappingURL=diagnostic.js.map