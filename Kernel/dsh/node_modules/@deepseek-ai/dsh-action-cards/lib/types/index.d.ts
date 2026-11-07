/** Optional action cards below an answer: the `propose_action` tool, card state, user actions and status notices for registered kinds. */
import { FiberState, Service, type Context } from '@deepseek-ai/cordis';
import { z, type ZodType } from 'zod';
import { TypertRemoteService } from '@deepseek-ai/dsh-typert-protocol';
import { HarnessError, type ToolCallId } from '@deepseek-ai/dsh-llm';
import { type Session } from '@deepseek-ai/dsh-session';
import type { Agent } from '@deepseek-ai/dsh-agent';
import type { JsonValue } from '@deepseek-ai/dsh-util-values';
import type { ActionCardDecision, ActionCardFields, ActionCardProposal, ActionCardResult, ActionCardView } from './types.ts';
export type * from './types.ts';
/** Deployment-tunable limits for offering cards. */
export declare const Config: z.ZodPrefault<z.ZodObject<{
    queuedMessageLimit: z.ZodDefault<z.ZodNumber>;
}, z.core.$strip>>;
/** Validated card limits after schema defaults. */
export type Config = z.infer<typeof Config>;
declare module '@deepseek-ai/cordis' {
    interface Context {
        actionCards: ActionCards;
    }
}
/** The fiber fields the stopping check reads. */
export interface PluginFiber {
    readonly uid: number | null;
    readonly state: FiberState;
    readonly parent: {
        readonly fiber: PluginFiber;
    };
}
/**
 * Whether a plugin must refuse new work: its fiber or an ancestor is disposed or unloading, since a parent's disposal
 * begins before the plugin's own effects run, or the plugin's own drain has begun.
 * @param owner - the plugin's fiber.
 * @param draining - whether the plugin's drain effect has started.
 * @returns whether the plugin is stopping.
 */
export declare function isPluginStopping(owner: PluginFiber, draining: boolean): boolean;
/** A refused `propose_action` call; the message tells the model what to do instead. */
export declare class ActionCardError extends HarnessError {
}
/** Shared checks one user action of a kind requires before the kind handles it. */
export interface ActionCardVerb {
    /** The connection must hold global Remote access, as for profile changes. */
    readonly global?: true;
    /** Refused while the Session sandbox mode is read-only. */
    readonly writes?: true;
}
/** A proposal whose details passed its kind's schema. */
export type ActionCardProposalOf<Details> = Omit<ActionCardProposal, 'details'> & {
    readonly details: Details;
};
/** Host half of one action kind: the skill that proposes it, its fields and its user actions. */
export interface ActionCardKind<Details> {
    /** Value of the `action` argument. */
    readonly action: string;
    /** Skill whose instructions propose this action; a load in the current turn admits the call. */
    readonly skill: string;
    /**
     * Model-facing closing guidance that the result of an accepted call gives: one or two sentences on how to invite
     * the user to the card, in the user's language. Refused calls never carry it, so the model invites the user only to a
     * card that exists.
     */
    readonly invitation: string;
    /** Validates `details` when the model proposes and before each user action. */
    readonly details: ZodType<Details>;
    /** User actions besides `dismiss`, with the shared checks each requires; other verbs are refused as unavailable. */
    readonly verbs: Readonly<Record<string, ActionCardVerb>>;
    /** Recorded phases that still offer the action; dismissal is recorded for these and for an unstarted card. */
    readonly openPhases?: readonly string[];
    /** Sentences the status notice adds when it reports this kind's decisions. */
    readonly statusGuidance?: string;
    /** Decision data fields the status notice reports; omitted reports every field. */
    readonly statusFields?: readonly string[];
    /**
     * Fields recorded with a dismissal.
     * @param proposal - the dismissed card.
     * @param payload - fields the user's client sent.
     * @returns decision data, or undefined to record none.
     */
    dismissal?(proposal: ActionCardProposalOf<Details>, payload: ActionCardFields | undefined): ActionCardFields | undefined;
    /**
     * Bring the card's recorded decision up to date with state outside the Session, such as an installation that
     * finished while the Session was closed, recording each change with {@link ActionCards.record}. The service awaits it
     * before every user action, including `dismiss`, then reads the card's decision again, so a card that is no longer
     * open settles instead of being dismissed.
     * @param agent - live root of the card's Session.
     * @param proposal - the card with validated details.
     * @throws when that state cannot be read; the action is then refused as unavailable.
     */
    reconcile?(agent: Agent, proposal: ActionCardProposalOf<Details>): Promise<void>;
    /**
     * Handle one user action other than `dismiss` after the shared checks passed. The handler records each new
     * decision with {@link ActionCards.record}.
     * @param agent - live root of the card's Session.
     * @param proposal - the card with validated details.
     * @param verb - one of {@link verbs}.
     * @param payload - fields the user's client sent; the kind validates them.
     * @returns the action's result.
     */
    act(agent: Agent, proposal: ActionCardProposalOf<Details>, verb: string, payload: ActionCardFields | undefined): Promise<ActionCardResult>;
}
/** Card tool, kind registry, Session projection, user actions and status notices. */
export declare class ActionCards extends TypertRemoteService {
    private readonly config;
    static inject: string[];
    static Config: z.ZodPrefault<z.ZodObject<{
        queuedMessageLimit: z.ZodDefault<z.ZodNumber>;
    }, z.core.$strip>>;
    private projection;
    private readonly kinds;
    /**
     * Actions whose kind unregistered. Cordis disposes a plugin's effects concurrently, so a kind's unload drain can
     * still record the outcome of its in-flight work after its registration is gone.
     */
    private readonly retired;
    /** Skills of registered kinds each live agent has loaded; `propose_action` is declared while one is still registered. */
    private readonly loaded;
    /** Kind skills each agent loaded in its latest turn. */
    private readonly turnLoads;
    /** The call accepted in each agent's latest turn, until its result fails. */
    private readonly accepted;
    private readonly actions;
    private closing;
    private readonly owner;
    /**
     * @param ctx - Owning plugin context.
     * @param config - Validated card limits; direct composition without a config gets the schema defaults.
     */
    constructor(ctx: Context, config?: Config);
    protected [Service.init](): Promise<void>;
    /**
     * Register one action kind for the caller's lifetime.
     * @param kind - the kind's skill, details schema, verbs and handler.
     * @returns disposer removing the kind; actions on its cards are then unavailable.
     * @throws when another kind already registered the same action.
     */
    register<Details>(kind: ActionCardKind<Details>): () => void;
    /**
     * Record a card decision in the Session log; the next model request reports it.
     * @param agent - agent whose Session owns the card.
     * @param decision - the card's new state.
     * @throws when no kind ever registered the decision's action with this service, or its phase is one the projection derives.
     */
    record(agent: Agent, decision: ActionCardDecision): void;
    /**
     * Read the cards of one Session.
     * @param session - a Session whose projections are prepared.
     * @returns proposals and decisions by call id.
     */
    view(session: Session): ActionCardView;
    /**
     * Apply a user action to a logged card. Shared checks run first, then the kind reconciles the card; `dismiss` is
     * recorded here and every other verb goes to the card's kind. `dismiss` also works on an unstarted card whose kind
     * is no longer registered. Concurrent actions on one card share one operation.
     * @param agent - live root resolved from the displayed Session.
     * @param callId - the card's `propose_action` call id.
     * @param verb - `dismiss` or one of the kind's verbs.
     * @param payload - kind-defined fields such as a chosen location.
     * @returns acceptance of background work, the decision in effect, or a refusal.
     */
    act(agent: Agent, callId: ToolCallId, verb: string, payload?: Record<string, JsonValue>): Promise<ActionCardResult>;
    /** Share one operation per card, run under the Agent's initiator, until it settles. */
    private coalesce;
    /**
     * Await the kind's reconciliation. A kind that unloads meanwhile still handles the action and refuses it itself.
     * @returns a refusal when reconciliation failed or the root closed meanwhile; undefined to continue.
     */
    private reconciled;
    private dismiss;
    private perform;
    private append;
    /** Whether `propose_action` is declared for this agent's next step. */
    private declares;
    private propose;
    private isStopping;
}
export default ActionCards;
//# sourceMappingURL=index.d.ts.map