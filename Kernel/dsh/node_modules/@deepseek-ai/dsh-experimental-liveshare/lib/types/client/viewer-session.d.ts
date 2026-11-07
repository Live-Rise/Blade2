/**
 * Select the one Session a viewer page presents.
 *
 * The viewer page presents exactly one shared Session, so it opens that Session as soon as the
 * session list carries it instead of leaving the empty "new session" home page a signed-in viewer
 * cannot act on.
 *
 * @module @deepseek-ai/dsh-experimental-liveshare/src/client/viewer-session
 */
import type { Context as ClientContext } from '@deepseek-ai/cordis';
/**
 * Open the shared Session once the session list knows it.
 *
 * Selecting a Session is the Workspace navigation action's to perform, so this page asks
 * `uiWorkspace` for the same selection the sidebar's own Session rows ask for; the session list
 * says when the Host has published the Session this page presents.
 * @param ctx - browser context of the viewer's module table.
 */
export declare function openSharedSession(ctx: ClientContext): void;
//# sourceMappingURL=viewer-session.d.ts.map