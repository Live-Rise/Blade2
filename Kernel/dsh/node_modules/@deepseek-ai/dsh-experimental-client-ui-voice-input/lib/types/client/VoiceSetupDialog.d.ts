import type { PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { NS } from './locales.ts';
type VoiceSetupDialogProps = PropsLocale<typeof NS> & Pick<PropsRuntime<'plugins.bundle.activation'>, 'onDismiss'> & {
    open: boolean;
    needsInstallation: boolean;
    /** Open the plugin details; the details button is absent while it is undefined. */
    onOpenDetails: PropsRuntime<'plugins.bundle.activation'>['onOpenDetails'] | undefined;
};
/**
 * Guide activation or microphone clicks to the existing plugin details.
 * @param props - visibility, installation need and navigation callbacks.
 * @returns a dismissible prompt that never starts preparation or recording.
 */
export declare function VoiceSetupDialog({ open, needsInstallation, onDismiss, onOpenDetails, t }: VoiceSetupDialogProps): import("react").JSX.Element;
export {};
//# sourceMappingURL=VoiceSetupDialog.d.ts.map