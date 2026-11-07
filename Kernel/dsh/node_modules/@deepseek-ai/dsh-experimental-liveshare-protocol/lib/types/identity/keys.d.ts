/** Canonical ACL key for one identity triple, and the normalization every adapter applies first. */
import type { IdentityTriple } from './types.ts';
/**
 * Build the canonical identity key stored in share ACLs.
 *
 * The caller normalizes the identity first: {@link normalizeIdentity} fills the documented tenant
 * placeholder, while an unnormalized empty tenant renders an empty segment that
 * {@link identityFromKey} cannot parse.
 * @param identity Provider, tenant, and subject triple.
 * @returns Percent-encoded stable key with no user-controlled formatting.
 */
export declare function identityKeyOf(identity: IdentityTriple): string;
/**
 * Parse a canonical identity key created by {@link identityKeyOf}.
 * @param key Percent-encoded provider, tenant, and subject key.
 * @returns Identity triple, or undefined when the key is not canonical.
 */
export declare function identityFromKey(key: string): IdentityTriple | undefined;
/**
 * Tenant id recorded for a provider that issues no tenant of its own.
 *
 * An account adapter whose provider issues no tenant must reject an empty tenant instead of using
 * this placeholder; see {@link requireIdentityTenant}.
 */
export declare const IDENTITY_TENANT_PLACEHOLDER = "default";
/**
 * Normalize one adapter-produced identity before Relay state stores it.
 * @param identity Provider-produced triple.
 * @param tenantFallback Tenant id used when the provider issued none.
 * @returns Normalized triple, or undefined when the provider or subject id is empty.
 */
export declare function normalizeIdentity<T extends IdentityTriple>(identity: T, tenantFallback?: string): T | undefined;
/**
 * Validate one adapter-produced identity whose provider must issue a tenant id.
 * @param identity Provider-produced triple.
 * @returns The same triple, or undefined when the provider, tenant, or subject id is empty.
 */
export declare function requireIdentityTenant<T extends IdentityTriple>(identity: T): T | undefined;
//# sourceMappingURL=keys.d.ts.map