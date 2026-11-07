/**
 * The viewer composer's model seat: the Session's model and reasoning effort, stated and not selectable.
 *
 * The served page renders the owner's Session, so the model it runs on and the reasoning effort it
 * asks for are facts about that Session rather than choices for the reader. This seat fills the
 * composer's own model position — where a DSH user looks for it — with that model's name, or its id
 * before the catalog arrives, and with the effort the next request would use. It renders through
 * {@link ViewerSeat}, whose `readOnly` switch is what keeps it a label.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/viewer-model
 */
import { type ReactNode } from 'react';
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import { NS } from './locales.ts';
export declare const name = "liveshare-viewer-model";
/** Composer slot registry, the Session list, and the Remote face the model catalog comes from. */
export declare const inject: readonly string[];
/** One exact model route, as the Session's own selection fold states it. */
interface StatedModel {
    readonly provider: string;
    readonly model: string;
    /** Reasoning effort the next request would use; absent when the Session states none. */
    readonly reasoningEffort?: string;
}
/** One effort level an adapter advertises for an exact model route. */
interface CatalogEffort {
    readonly id: string;
    readonly name: string;
}
/** Selectable reasoning metadata one catalog entry carries for its route. */
interface CatalogReasoning {
    readonly efforts: readonly CatalogEffort[];
    readonly defaultEffort?: string;
}
/** One provider's advertised models, read only to prefer a display name over the model id. */
interface CatalogGroup {
    readonly id: string;
    readonly models: readonly {
        readonly id: string;
        readonly name: string;
        readonly reasoning?: CatalogReasoning;
    }[];
}
/** The observation face the Session store exposes for one projection key. */
interface ObservedFold {
    subscribe(listener: () => void): () => void;
    getSnapshot(): unknown;
}
/** The Host's advisory catalog: the default route plus the display names to label it with. */
interface Catalog {
    readonly default: StatedModel;
    readonly groups: readonly CatalogGroup[];
}
/** Everything the seat reads: the Session's fold and the Host catalog. */
export interface ViewerModelSeatProps {
    /** Whether the composer's own owner disabled the seat; a stated model stays a label either way. */
    readonly locked: boolean;
    /** The Session's durable model-selection fold, absent until the Session is bound. */
    readonly selection: ObservedFold | undefined;
    /** Read the Host's advisory catalog, or undefined when it cannot be read. */
    readonly catalog: () => Promise<Catalog | undefined>;
}
/**
 * Render the model and reasoning effort this Session runs on.
 * @param props - the Session's selection fold, the catalog reader, and the framework-injected
 *   translate seat.
 * @returns the seat, or nothing until the Session states a model.
 */
export declare function ViewerModelSeat({ selection, catalog, t }: ViewerModelSeatProps & PropsLocale<typeof NS>): ReactNode;
/**
 * Fill the composer's model seat with the Session's model.
 * @param ctx - the viewer page's root context.
 */
export declare function apply(ctx: ClientContext): void;
export {};
//# sourceMappingURL=viewer-model.d.ts.map