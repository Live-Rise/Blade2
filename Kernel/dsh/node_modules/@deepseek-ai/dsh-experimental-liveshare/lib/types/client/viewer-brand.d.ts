/**
 * The viewer page's brand row: the sidebar's own mark and name seats carry the share.
 *
 * The page renders the normal client UI, so its identity belongs where every DSH page keeps one —
 * the expanded sidebar's brand row — instead of a floating banner. The mark stays the shipped
 * Harness logo; the name seat becomes the way home and states, compactly, who shared this Session
 * and which DSH built it. The Relay's own release stays a detail on that link rather than a second
 * identifier in the page's chrome.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/viewer-brand
 */
import { type ReactNode } from 'react';
import type { Context as ClientContext } from '@deepseek-ai/cordis';
import type { SidebarBrandMarkOwnerProps } from '@deepseek-ai/dsh-client-ui-sidebar/client';
import type { PropsLocale } from '@deepseek-ai/dsh-client-ui-slots';
import { NS } from './locales.ts';
export declare const name = "liveshare-viewer-brand";
/**
 * Registration rank for the viewer's brand seats.
 *
 * `sidebar.brand.mark` and `sidebar.brand.name` are single slots, and a served viewer page mounts the
 * owner's own plugins, so the shipped brand (including `ui-brand-official`, which registers both at
 * the default rank on official builds) is already registered at rank 0. A second registration at that
 * rank fails, so the viewer entry registers below it and shadows the shipped one.
 */
export declare const VIEWER_BRAND_PRIORITY = -10;
/** The slot registry is the only service this plugin reads. */
export declare const inject: readonly string[];
/**
 * Render the mark slot: the shipped Harness logo at the shell's requested size.
 * @param props - Host-supplied mark presentation.
 * @returns the Harness mark.
 */
export declare function ViewerBrandMark({ size }: SidebarBrandMarkOwnerProps): ReactNode;
/**
 * Render the name slot: LiveShare, its owner, and the owner's DSH version, linked to the page's
 * share directory. The Relay's release rides the same link's tooltip so a long identifier stays
 * readable on demand instead of widening the row.
 * @param props - The framework-injected translate seat for this package's copy.
 * @returns the viewer's brand content.
 */
export declare function ViewerBrandName({ t }: PropsLocale<typeof NS>): ReactNode;
/**
 * Fill the sidebar's brand seats with the viewer's own identity.
 * @param ctx - browser context of the viewer's module table.
 */
export declare function apply(ctx: ClientContext): void;
//# sourceMappingURL=viewer-brand.d.ts.map