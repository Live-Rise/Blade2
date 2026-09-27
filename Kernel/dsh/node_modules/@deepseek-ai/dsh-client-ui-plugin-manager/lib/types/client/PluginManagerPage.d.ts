/**
 * Global plugin management: the Official group's cards for the bundles the
 * installation ships switched off and for the official plugins that register
 * their configuration, the Installed group's cards for the profile's bundles,
 * their row switches, the install dialog with its guide and folded pnpm
 * output, the uninstall confirmation, and the toasts an action's outcome
 * becomes. A bundle's page lists the rows it contributes as the Host runs
 * them; a plugin's configuration renders on its own page through the slots
 * the page declares.
 */
import { type ReactNode } from 'react';
import type { InjectFace, PropsLocale, PropsRenderSlots, PropsRuntime, PropsStore } from '@deepseek-ai/dsh-client-ui-slots';
import type { createNavigationStore } from './navigation-store.ts';
import { type PluginManagerFace } from './manager-store.ts';
/** Full component props assembled by the main slot renderer. */
export type PluginManagerPageProps = PropsRuntime<'main'> & PropsLocale<'pluginManager'> & PropsRenderSlots<'plugins.item' | 'plugins.bundle.config' | 'plugins.row.config' | 'plugins.bundle.activation' | 'plugins.detail.actions' | 'plugins.detail.badge' | 'plugins.detail.section'> & InjectFace<PluginManagerFace> & PropsStore<ReturnType<typeof createNavigationStore>>;
/** Render the plugin manager: the official plugins and installed bundles, their pages, the install dialog, and the confirmation. */
export declare function PluginManagerPage(props: PluginManagerPageProps): ReactNode;
//# sourceMappingURL=PluginManagerPage.d.ts.map