import type { InjectFace, PropsLocale, PropsRuntime } from '@deepseek-ai/dsh-client-ui-slots';
import type { ConfigForm } from '@deepseek-ai/dsh-client-ui-settings/client';
import type { PermissionFace } from './controller.ts';
import type { NS } from './locales.ts';
/** Renderer-bound permission observation and localized controls. */
export type PermissionProps = InjectFace<PermissionFace> & PropsLocale<typeof NS>;
/** Permission guidance only navigates to the existing bundle details. */
export type PermissionSetupProps = PermissionProps & Pick<PropsRuntime<'plugins.bundle.activation'>, 'onDismiss' | 'onOpenDetails'>;
type DefaultPresetProps = PermissionProps & InjectFace<{
    hooks: {
        cuaDefaultPreset: ConfigForm<{
            default: string;
            selectedDefault?: string;
        }>;
    };
    setPtcDefault: () => Promise<boolean>;
}>;
type PersistentPtcProps = PermissionProps & {
    setPersistentPtc: () => Promise<boolean>;
};
/** The detail page owns explicit preset and execution-mode settings. */
export type PermissionPanelProps = PermissionProps & DefaultPresetProps & PersistentPtcProps;
/** @param props Host observation and explicit user actions. @returns Permission controls without automatic system prompts. */
export declare function PermissionPanel(props: PermissionPanelProps): import("react").JSX.Element;
/**
 * @param props Permission observation and activation navigation callbacks.
 * @returns Setup guidance when system permissions need attention.
 */
export declare function PermissionSetup({ t, refresh, useCuaPermissions, onDismiss, onOpenDetails }: PermissionSetupProps): import("react").JSX.Element;
/** @param props Shared operation outcome, independent of page lifetime. @returns The existing global toast presentation. */
export declare function PermissionToast(props: PermissionProps): import("react").JSX.Element | null;
export {};
//# sourceMappingURL=PermissionPanel.d.ts.map