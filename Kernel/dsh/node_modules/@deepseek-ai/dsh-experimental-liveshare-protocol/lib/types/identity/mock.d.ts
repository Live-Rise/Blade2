import type { IdentityActor, IdentityAuthorizationRequest, IdentityExchangeRequest, IdentityProvider } from './types.ts';
/** One fixed local identity used by tests and loopback demos. */
export interface MockIdentityPersona {
    readonly id: string;
    readonly tenantId?: string;
    readonly displayName?: string;
    /** Avatar a provider would publish; absent stands for a provider that issues none. */
    readonly avatarUrl?: string;
}
/** Deterministic provider for tests; not an authentication boundary. */
export declare class MockIdentityProvider implements IdentityProvider {
    readonly id: string;
    private readonly personas;
    readonly scopes: readonly string[];
    constructor(id: string, personas: readonly MockIdentityPersona[]);
    /** Redirect back to the Relay callback with the selected fixed persona code. */
    authorizationUrl(request: IdentityAuthorizationRequest): URL;
    /** Resolve a fixed persona code into a provider-neutral actor. */
    exchange(request: IdentityExchangeRequest): Promise<IdentityActor>;
}
//# sourceMappingURL=mock.d.ts.map