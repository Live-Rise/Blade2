import type { Duplex } from 'node:stream';
import { type BirpcReturn } from 'birpc';
/**
 * Connect typed methods over one control endpoint; execution owns all deadlines.
 * @param stream - Owned control endpoint; `$close()` destroys it and rejects outstanding remote calls.
 * @param maxMessageBytes - Maximum incoming JSON line and queued outgoing bytes.
 * @param functions - Only these own methods may be called by the peer.
 * @param eventNames - Remote methods sent as notifications without a response.
 * @param failure - Receives the first I/O error, peer closure or malformed-message failure after the endpoint closes.
 * @returns Typed remote calls and birpc lifetime controls.
 */
export declare function createProcessRpc<Remote extends object, Local extends object>(stream: Duplex, maxMessageBytes: number, functions: Local, eventNames: (keyof Remote)[], failure: (error: Error, kind: 'io' | 'closed' | 'protocol') => void): BirpcReturn<Remote, Local>;
//# sourceMappingURL=rpc.d.ts.map