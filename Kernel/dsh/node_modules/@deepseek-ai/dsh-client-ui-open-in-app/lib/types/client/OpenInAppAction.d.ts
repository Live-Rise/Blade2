import type { ObservableSnapshot } from '@deepseek-ai/dsh-client-store';
import type { InjectFace, PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import type { OpenInAppLaunchState } from './controller.ts';
import type { OpenInAppOutcome } from './outcome.ts';
import type { ShortcutCatalogEntry } from '@deepseek-ai/dsh-client-shortcuts/client';
import { NS } from './locales.ts';
import { type OpenTargetReporter } from './OpenTargetButton.tsx';
/** Browser operations and state shared by the Session header and file tree contributions. */
export interface OpenInAppActionInjected extends OpenTargetReporter {
    hooks: {
        openInAppApps: ObservableSnapshot<readonly string[] | null>;
        openInAppChoice: ObservableSnapshot<string>;
        openInAppLaunch: ObservableSnapshot<OpenInAppLaunchState>;
        shortcuts: ObservableSnapshot<readonly ShortcutCatalogEntry[]>;
    };
    launch: (appId: string, path: string) => Promise<OpenInAppOutcome>;
    choose: (appId: string) => void;
    iconUrl: (appId: string) => string;
}
/** Directory path, installed applications, and launch operations for the split button. */
export type OpenInAppActionProps = PropsLocale<typeof NS> & InjectFace<OpenInAppActionInjected> & {
    absolutePath: string;
};
/**
 * Adapt the installed directory catalog to the shared opening control.
 * @param props - displayed directory, installed catalog, launch operations, and the announcement seat.
 * @returns the shared control, or null without an eligible application.
 */
export declare function OpenInAppAction(props: OpenInAppActionProps): React.JSX.Element | null;
//# sourceMappingURL=OpenInAppAction.d.ts.map