/** Projection that folds shell results and binding records into one Session's change request bindings, and shows clients the bindings. */
import { z } from 'zod';
import type { SessionEvent } from '@deepseek-ai/dsh-session';
import type { BindingState, ForgeChangeRequestsState } from './types.ts';
/** Plugin record type of binding actions. */
export declare const BINDING_RECORD = "plugin:forge-session/binding";
/**
 * The `forgeChangeRequests` unit. A `bash` or `pwsh` call whose command is
 * `gh pr create`, `gh pr view`, `gh pr edit`, `gh pr checkout`, or
 * `git push` stays pending until its first result. A successful result binds
 * every pull request URL a `gh pr create` result prints; for the other `gh pr`
 * commands it binds the URL the command names and the URL the output prints
 * for its own change request; and it records the branches it reports as
 * pushed. Failed tool executions bind nothing; a nonzero exit status does not
 * matter, because the URL or ref line is the evidence. `turn/end` drops calls
 * that never received a result. Binding records bind, restore, and hide URLs.
 * Clients, including the Session summary, see the bindings; the array keeps
 * its reference while only pending calls and pushes change.
 */
export declare const forgeChangeRequestsProjection: {
    key: "forgeChangeRequests";
    stateVersion: number;
    stateSchema: z.ZodType<ForgeChangeRequestsState, unknown, z.core.$ZodTypeInternals<ForgeChangeRequestsState, unknown>>;
    init: () => ForgeChangeRequestsState;
    apply: typeof apply;
    wire: {
        viewSchema: z.ZodType<readonly BindingState[], unknown, z.core.$ZodTypeInternals<readonly BindingState[], unknown>>;
        view: (state: NoInfer<ForgeChangeRequestsState>) => readonly BindingState[];
    };
};
declare function apply(state: ForgeChangeRequestsState, event: SessionEvent): ForgeChangeRequestsState;
export {};
//# sourceMappingURL=projection.d.ts.map