import type { InjectFace, PropsLocale, PropsRuntime, PropsRenderSlots } from '@deepseek-ai/dsh-client-ui-slots';
import type { AccountSectionInjected } from './AccountSection.tsx';
/** Account launcher composed by the settings shell. */
export type AccountMenuProps = PropsRuntime<'settings.launcher'> & PropsRenderSlots<'settings.launcher.notifications'> & PropsLocale<'settings.account'> & InjectFace<AccountSectionInjected>;
/** The signed-in label stays empty while the profile loads.
 * @param props - sidebar geometry, settings navigation and account operations.
 * @returns account menu launcher.
 */
export declare function AccountMenu({ subscribeSessionExpired, subscribeModelSignInRequired, wide, settingsShortcut, openSettings, openOnboarding, settingsOpen, useAccount, useTheme, signOut, hasRunningAccountTasks, refreshAccount, renderSlot, contactUs, showLogin, start, cancel, avatarFailed, t, }: AccountMenuProps): import("react").JSX.Element;
//# sourceMappingURL=AccountMenu.d.ts.map