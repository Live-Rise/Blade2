import type { CuaApi, CuaDispatch, CuaObservationSink } from './types.ts';
export type * from './types.ts';
/**
 * Create app/window objects using the caller's authorized dispatcher.
 * @param dispatch - Current tool authority; rejects propagate without retries.
 * @param sink - Optional text/image output; emit false suppresses this sink only.
 * @returns Objects independent of variable storage and evaluator lifecycle.
 */
export declare function createCuaApi(dispatch: CuaDispatch, sink?: CuaObservationSink): CuaApi;
//# sourceMappingURL=api.d.ts.map