import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { DesktopChannelSource } from './desktop-channel-source.ts';
import type { DesktopUpdateChannel } from '../types.ts';
/** Framework-bound shell state and explicit selection action. */
export interface DesktopChannelRowInjected {
    hooks: {
        channel: DesktopChannelSource['store'];
    };
    select(channel: DesktopUpdateChannel): Promise<void>;
}
/**
 * Render the Desktop-only channel control without depending on a Host settings service.
 * @param props - Shell observation, selection action and localized copy.
 * @returns The General settings row.
 */
export declare function DesktopChannelRow({ useChannel, select, t }: PropsRuntime<'settings.general.item'> & PropsLocale<'settings'> & InjectFace<DesktopChannelRowInjected>): import("react").JSX.Element;
/** Shell-owned operation feedback that survives closing Settings. */
export interface DesktopChannelToastInjected {
    hooks: {
        channel: DesktopChannelSource['store'];
    };
    dismiss(sequence: number): void;
    openConfiguration(): void;
}
/**
 * Render the latest channel save result outside Settings.
 * @param props - Shared operation state, dismissal and localized copy.
 * @returns A transient toast, or nothing.
 */
export declare function DesktopChannelToast({ useChannel, dismiss, openConfiguration, t }: PropsRuntime<'shell.overlay'> & PropsLocale<'settings'> & InjectFace<DesktopChannelToastInjected>): import("react").JSX.Element | null;
//# sourceMappingURL=DesktopChannelRow.d.ts.map