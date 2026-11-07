/** Shared Host wiring for the DeepSeek protocol adapter. */
import type { Context } from '@deepseek-ai/cordis';
import type { DeepSeekAdapterOptions, DeepSeekConnectionOptions } from './types.ts';
import { ModelListingCache, type ListingCacheDependencies } from './listing-cache.ts';
/**
 * Own a route's listing refreshes, stderr diagnostics, and catalog-change notifications.
 * @param ctx - provider plugin lifetime.
 * @param owner - plugin name used in diagnostics.
 * @param provider - route whose catalog changes.
 * @param dependencies - current configuration, credentials, and listing transport.
 * @returns the cache, disposed with the provider after its active work settles.
 */
export declare function createModelListingCache(ctx: Context, owner: string, provider: string, dependencies: Pick<ListingCacheDependencies, 'settings' | 'credential' | 'load'>): ModelListingCache;
/**
 * Register one provider with request-local transport services and live retry policy.
 * @param ctx - provider plugin lifetime with the LLM registry injected.
 * @param provider - exact route owned by this plugin.
 * @param dependencies - provider-owned discovery, credential, and configuration callbacks.
 */
export declare function registerDeepSeekProvider<C extends DeepSeekConnectionOptions>(ctx: Context, provider: string, dependencies: Pick<DeepSeekAdapterOptions<C>, 'options' | 'resolveAuth' | 'providerName' | 'discoverModels' | 'connect' | 'admit'>): void;
//# sourceMappingURL=host.d.ts.map