/**
 * The registration handshake one share is published with.
 *
 * This URL is the one surface an older owner meets a newer Relay on, so what it states about the
 * owner — the protocol version it reads frames with, and the capabilities it offers beyond them — is
 * the only thing a Relay can compare instead of assuming. Keeping the whole handshake in one function
 * is what lets a check pin it: every parameter here is read by a peer this repository does not build.
 */
import { type TunnelShareSpec } from '@deepseek-ai/dsh-experimental-liveshare-protocol';
/**
 * Build the WebSocket URL one share registers with.
 *
 * Absent optionals stay absent rather than being sent empty: the Relay reads a missing `sessionMode`
 * as view-only and a missing `allowsFork` as no forking, so an older owner publishes the narrower
 * share instead of a share whose fields a peer must interpret.
 * @param spec - Share the owner host asked this worker to carry.
 * @returns The registration URL, with this build's protocol version and capabilities stated.
 */
export declare function registrationUrl(spec: TunnelShareSpec): URL;
//# sourceMappingURL=registration-url.d.ts.map