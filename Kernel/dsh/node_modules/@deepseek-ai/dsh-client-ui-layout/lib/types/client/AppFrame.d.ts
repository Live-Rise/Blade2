import type { HostObservable, InjectFace, PropsLocale, PropsRenderSlots, PropsRuntime, PropsStore } from '@deepseek-ai/dsh-client-ui-slots';
import type { createLayoutStore } from './stores.ts';
/** Browser posture supplied by the root registration. */
export interface AppFrameInjected {
    hooks: {
        foldedLandscape: HostObservable<boolean>;
    };
}
/** Full composed props: runtime, child slots, store, locale, and injected posture. */
export type AppFrameProps = PropsRuntime<'root'> & PropsRenderSlots<'sidebar' | 'main' | 'rightbar' | 'shell.bottom' | 'shell.overlay' | 'shell.leading'> & PropsStore<ReturnType<typeof createLayoutStore>> & PropsLocale<'common'> & InjectFace<AppFrameInjected>;
/** The three-column frame (see module doc). */
export declare function AppFrame({ useStore, useSessions, usePanelInfo, useFoldedLandscape, actions, renderSlot, t, }: AppFrameProps): import("react").JSX.Element;
//# sourceMappingURL=AppFrame.d.ts.map