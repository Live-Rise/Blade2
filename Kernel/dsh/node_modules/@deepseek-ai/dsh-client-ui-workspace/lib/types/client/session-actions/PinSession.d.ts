import type { PinSessionInjected, SessionRowActionProps } from '../contract/slots.ts';
/**
 * Hover button (order 200), also visible without hover when pinned; absent on
 * archived rows and while the Session's access snapshot excludes that toggle.
 * Tooltip forwards the class to the button. Keep that button directly inside
 * the display-contents slot anchor so hoverless rows can retain the Unpin action.
 * @param props - owner share, the access Hook, and the pin share.
 * @returns the button, or null for an archived or refused Session.
 */
export declare function PinSessionRowButton(props: SessionRowActionProps<PinSessionInjected>): import("react").JSX.Element | null;
//# sourceMappingURL=PinSession.d.ts.map