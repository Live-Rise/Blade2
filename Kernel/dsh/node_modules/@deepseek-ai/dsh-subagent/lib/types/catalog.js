/**
 * Parent-owned durable subagent catalog events and their chunked projection.
 *
 * @module @deepseek-ai/dsh-subagent/catalog
 */
import { z } from 'zod';
import { appendChunkedList, chunkedListSchema, iterateChunkedList } from '@deepseek-ai/dsh-chunked-list';
import { isAgentName } from "./names.js";
/** Catalog payload version emitted by local and external child creation. */
export const SUBAGENT_CATALOG_VERSION = 3;
const sessionIdSchema = z.string();
const oneShotCatalogSchema = z.object({
    version: z.union([z.literal(0), z.literal(1)]),
    childId: sessionIdSchema,
    childCreatedAt: z.number().int().nonnegative(),
    mode: z.literal('one-shot'),
    label: z.string().optional(),
}).strict();
const continuableCatalogSchema = z.object({
    version: z.union([z.literal(0), z.literal(1)]),
    childId: sessionIdSchema,
    childCreatedAt: z.number().int().nonnegative(),
    mode: z.literal('continuable'),
    label: z.string(),
}).strict();
const externalCatalogSchema = oneShotCatalogSchema.extend({ version: z.literal(2), mode: z.literal('external') });
const unknownCatalogSchema = oneShotCatalogSchema.extend({ version: z.literal(1), mode: z.literal('unknown') });
const nameSchema = z.string().refine(isAgentName, 'invalid agent name');
const namedContinuableCatalogSchema = continuableCatalogSchema.extend({ version: z.literal(3), name: nameSchema });
const namedExternalCatalogSchema = externalCatalogSchema.extend({ version: z.literal(3), name: nameSchema });
const eventDataSchema = z.union([
    oneShotCatalogSchema,
    externalCatalogSchema,
    continuableCatalogSchema,
    unknownCatalogSchema,
    namedContinuableCatalogSchema,
    namedExternalCatalogSchema,
]);
const viewSchema = z.array(z.union([
    oneShotCatalogSchema.omit({ version: true, childId: true, childCreatedAt: true }).extend({
        id: sessionIdSchema,
        createdAt: oneShotCatalogSchema.shape.childCreatedAt,
    }),
    externalCatalogSchema.omit({ version: true, childId: true, childCreatedAt: true }).extend({
        id: sessionIdSchema,
        createdAt: externalCatalogSchema.shape.childCreatedAt,
        name: nameSchema.optional(),
    }),
    continuableCatalogSchema.omit({ version: true, childId: true, childCreatedAt: true }).extend({
        id: sessionIdSchema,
        createdAt: continuableCatalogSchema.shape.childCreatedAt,
        name: nameSchema.optional(),
    }),
    unknownCatalogSchema.omit({ version: true, childId: true, childCreatedAt: true }).extend({
        id: sessionIdSchema,
        createdAt: unknownCatalogSchema.shape.childCreatedAt,
    }),
]));
const stateSchema = z.object({
    inheritedEventCount: z.number().int().nonnegative(),
    head: chunkedListSchema(eventDataSchema).optional(),
}).strict();
/**
 * Materialize complete and unknown-mode child identities from parent catalog events.
 * @param state - parent catalog fold state.
 * @returns current direct-child rows in parent catalog event order.
 */
function subagentCatalogEntries(state) {
    const entries = [];
    for (const data of iterateChunkedList(state.head)) {
        const { version: _version, childId, childCreatedAt, ...descriptor } = data;
        entries.push({ id: childId, createdAt: childCreatedAt, ...descriptor });
    }
    return entries;
}
/** Parent-owned catalog; version-6 checkpoints retain unnamed facts, and invalid own facts reject restoration. */
export const subagentCatalogProjectionDefinition = {
    key: 'subagentCatalog',
    stateSchema,
    init: (_header, inheritedEventCount) => ({ inheritedEventCount }),
    apply: (state, event) => {
        if (event.type !== 'subagent/catalog' || event.seq < state.inheritedEventCount)
            return state;
        return { ...state, head: appendChunkedList(state.head, eventDataSchema.parse(event.data)) };
    },
    stateVersion: 6,
    wire: { viewSchema, view: subagentCatalogEntries },
};
/**
 * Append a complete direct-child discovery fact to its parent Session.
 * @param parent - durable direct parent receiving the discovery fact.
 * @param child - established child's immutable Session metadata.
 * @param descriptor - mode-discriminated creation label frozen with the child.
 * @param name - the child's name, reserved unique among the parent's children.
 */
export function establishCatalogChild(parent, child, descriptor, name) {
    parent.append('subagent/catalog', {
        version: SUBAGENT_CATALOG_VERSION,
        childId: child.id,
        childCreatedAt: child.createdAt,
        mode: descriptor.mode,
        label: descriptor.label,
        name,
    });
}
/**
 * Record an external execution without a local child Session.
 * @param parent - durable direct parent receiving the discovery fact.
 * @param childId - identity returned by the external provider.
 * @param label - display label assigned by the caller.
 * @param name - the child's name, reserved unique among the parent's children.
 */
export function establishExternalCatalogChild(parent, childId, label, name) {
    parent.append('subagent/catalog', {
        version: SUBAGENT_CATALOG_VERSION,
        childId,
        childCreatedAt: Date.now(),
        mode: 'external',
        label,
        name,
    });
}
/**
 * Collect the names recorded in one parent's catalog.
 * @param entries - the parent's current catalog rows.
 * @returns names of every named child; historical entries contribute none.
 */
export function catalogNames(entries) {
    const names = new Set();
    for (const entry of entries) {
        if ('name' in entry)
            names.add(entry.name);
    }
    return names;
}
//# sourceMappingURL=catalog.js.map