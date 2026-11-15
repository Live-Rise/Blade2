/** Source locators and converter-owned identities for shared PDF reuse. */
import { type Branded } from '@deepseek-ai/dsh-brand';
/** Authorized execution scope and canonical source path, encoded by the consumer. */
export type OfficeSourceKey = Branded<'OfficeSourceKey'>;
/**
 * Label an authorized source locator for pre-read deduplication.
 * @param key - unambiguous encoding of authorization scope, execution world, and canonical path.
 * @returns branded source locator; source authorization remains the caller's responsibility.
 */
export declare function OfficeSourceKey(key: string): OfficeSourceKey;
/** One provider lifetime, including its engine, rendering settings, and font configuration. */
export type OfficeKitGeneration = Branded<'OfficeKitGeneration'>;
/**
 * Label a provider lifetime.
 * @param value - unique generation created by the provider.
 * @returns branded converter generation.
 */
export declare function OfficeKitGeneration(value: string): OfficeKitGeneration;
/** Provider generation and source-content digest; consumers must not parse it. */
export type OfficeKitKey = Branded<'OfficeKitKey'>;
/**
 * Label a converter-owned content identity.
 * @param value - generation and content identity created by the provider.
 * @returns branded conversion identity.
 */
export declare function OfficeKitKey(value: string): OfficeKitKey;
//# sourceMappingURL=identity.d.ts.map