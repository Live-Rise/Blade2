/** Adapt authorized delivery and changed-file routes to the shared opening control. */
import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import { type OpenTargetReporter } from './OpenTargetButton.tsx';
import type { NS } from './locales.ts';
/**
 * Render file actions without bypassing the owning Session's authorization route.
 * @param props - authenticated route, desktop availability, the native gesture
 * callback, and the announcement seat its settled outcome is reported to.
 * @returns the shared compact control, or null without a desktop.
 */
export declare function FileRouteAction(props: Pick<PropsRuntime<'deliverables.file.actions'>, 'actionUrl' | 'available' | 'pending' | 'onAction'> & PropsLocale<typeof NS> & InjectFace<OpenTargetReporter>): import("react").JSX.Element | null;
//# sourceMappingURL=FileRouteAction.d.ts.map