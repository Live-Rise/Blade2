#ifndef DSH_FRAME_FORWARDING_H
#define DSH_FRAME_FORWARDING_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
#define DSH_FRAME_FORWARDING_PROTOCOL_VERSION 1
#define DSH_FRAME_FORWARDING_TEXT_CAPACITY 16384
/** Opaque connected producer. One thread may poll while another acquires, writes,
 * submits or discards. Callers must serialize use of the same graphics context.
 */
typedef struct dsh_frame_forwarding_client dsh_frame_forwarding_client;
/** Pool-owned surface. Borrowed resources and imported objects are usable only
 * during the acquisition; never destroy the SDK-owned resource.
 */
typedef struct dsh_frame_forwarding_surface dsh_frame_forwarding_surface;
/** kind: 0 credit/timeout, 1 incoming JSON, -1 closed/error.
 * Terminal events carry diagnostic text when available.
 */
typedef struct dsh_frame_forwarding_event { int kind; char text[DSH_FRAME_FORWARDING_TEXT_CAPACITY]; } dsh_frame_forwarding_event;
/** Connect to a frame-forwarding channel with its TX credential, the opaque text the receiver issued. The credential
 * names no producer process: any process holding a copy may connect, and a parent may hand it to the process it
 * starts. The channel serves one connection at a time. While another producer is connected, connect fails with an
 * error starting "channel-busy". After that producer disconnects, the first authenticated successor waits until the
 * receiver has released every frame and resource of the previous connection; further candidates fail as busy.
 * Without options the wait is unbounded. Receivers that predate transferable credentials, or that do not acknowledge
 * that a producer names only handles of its own process, are refused.
 */
dsh_frame_forwarding_client* dsh_frame_forwarding_connect(const char* tx, char* error, size_t error_size);
/** Connect using the application's ID3D11Device on Windows; NULL selects the default
 * device. The client retains a device reference until close. macOS requires NULL.
 * Serialize the device's immediate context with all SDK writes.
 */
dsh_frame_forwarding_client* dsh_frame_forwarding_connect_with_device(const char* tx, void* device, char* error, size_t error_size);
/** Bounded connection options. cancelled is optional and runs on the connecting
 * thread; its context must remain alive until connect_with_options returns.
 */
typedef struct dsh_frame_forwarding_connect_options {
  void* device;
  uint32_t timeout_ms;
  void* cancellation_context;
  int (*cancelled)(void*);
} dsh_frame_forwarding_connect_options;
/** Connect with a positive timeout and optional cancellation, which also bound waiting
 * for the previous connection to drain. Device ownership and platform rules match connect_with_device.
 */
dsh_frame_forwarding_client* dsh_frame_forwarding_connect_with_options(const char* tx,
  const dsh_frame_forwarding_connect_options* options, char* error, size_t error_size);
/** Acquire BGRA8 premultiplied top-left pixels; NULL means allocation pending or no credit; poll reports terminal allocation errors.
 * When no other surface of the size is free, the surface of an unsent submit_latest frame is reused and that frame is dropped.
 */
dsh_frame_forwarding_surface* dsh_frame_forwarding_acquire(dsh_frame_forwarding_client* client, uint32_t width, uint32_t height);
/** Borrow an IOSurfaceRef (macOS) or ID3D11Texture2D* (Windows) while acquired.
 * Windows external writers acquire/release key 0 around GPU commands (see
 * surface_keyed_mutex); macOS writers complete GPU writes before submit/discard.
 * Release imported objects/views before ending the acquisition.
 */
void* dsh_frame_forwarding_surface_native(dsh_frame_forwarding_surface* surface);
/** Borrow the Windows shared NT handle while this surface is acquired; NULL on macOS.
 * Do not close the borrowed handle. External imports must synchronize with key 0
 * and release access and imported objects before submit/discard.
 */
void* dsh_frame_forwarding_surface_shared_handle(dsh_frame_forwarding_surface* surface);
/** Return the ID3D11Device* borrowed until client close, or NULL on macOS.
 * External GPU imports must use the same adapter as this device.
 */
void* dsh_frame_forwarding_device(dsh_frame_forwarding_client* client);
/** Borrow the acquired Windows surface's IDXGIKeyedMutex, or NULL on macOS.
 * acquire itself never locks. External writers acquire key 0 before issuing GPU
 * commands and release key 0 before submit/discard, without waiting for completion.
 * SDK write_bgra owns this lock; do not lock around that call.
 * Do not release the borrowed COM reference. AcquireSync may block the CPU.
 */
void* dsh_frame_forwarding_surface_keyed_mutex(dsh_frame_forwarding_surface* surface);
/** Copy CPU BGRA rows; return 1 on success, 0 on failure.
 * Windows acquires key 0 with a 1000 ms timeout and releases it after enqueueing
 * the upload. Input memory may be reused on return; GPU completion is asynchronous.
 */
int dsh_frame_forwarding_write_bgra(dsh_frame_forwarding_surface* surface, const void* pixels, size_t stride);
/** CPU upload with a positive keyed-mutex acquisition timeout. Failure leaves completion
 * uncertain; close the client without discarding or reusing the acquisition.
 */
int dsh_frame_forwarding_write_bgra_with_timeout(dsh_frame_forwarding_surface* surface, const void* pixels, size_t stride, uint32_t timeout_ms);
/** Submit after Windows GPU writes are queued and the writer released key 0;
 * macOS GPU writes must be complete. Release all imported objects/views first.
 * frame_id must be 1..9007199254740991 and unique among outstanding submissions.
 * Return 1 on submission; the surface cannot be reused until it is reacquired.
 * Return 0 on failure; a valid acquisition stays with the caller for retry or discard.
 */
int dsh_frame_forwarding_submit(dsh_frame_forwarding_client* client, dsh_frame_forwarding_surface* surface, uint64_t frame_id, const char* metadata_json);
/** Submit a continuous-stream frame without waiting for receiver credit, under the GPU
 * ownership rules of submit. With N the receiver's frame limit, the frame is sent at once
 * while fewer than max(1, N - 1) submissions await release; otherwise it becomes the client's
 * single queued frame, and an older queued frame returns to the pool unsent. Polling that
 * observes a release sends the queued frame, so the newest complete frame reaches the receiver
 * without another render. Acquisition reuses the queued surface when no other surface of that
 * size is free; with N of at least 2 an allocated stream always obtains a surface. The frame is
 * marked replaceable, so a receiver that stages frames before presentation may drop it once a
 * newer latest frame is ready; submit frames are never dropped.
 * frame_id must be unique among outstanding submissions. Return 1 when sent or queued.
 * Return 0 on failure; a valid acquisition stays with the caller for retry or discard.
 */
int dsh_frame_forwarding_submit_latest(dsh_frame_forwarding_client* client, dsh_frame_forwarding_surface* surface, uint64_t frame_id, const char* metadata_json);
/** Return every idle surface to the receiver's byte budget: available surfaces, the queued submit_latest frame,
 * granted allocations not yet created, and pending allocation requests. Acquired and submitted surfaces stay. A later
 * acquire requests allocations again. Call it when the view stops displaying frames, for example after applying an
 * invisible viewport. On Windows it flushes the D3D11 immediate context of the producer's device so the released
 * textures are destroyed; call it on the thread that uses that context. Return 1 when every deallocation was sent; 0
 * for a closed client, and then poll reports the error.
 */
int dsh_frame_forwarding_trim(dsh_frame_forwarding_client* client);
/** Return an acquired, unsubmitted surface under the same GPU ownership rules as
 * submit. If releasing GPU access failed, terminate the connection instead of
 * discarding. The next writer must acquire key 0 even after a discard.
 */
void dsh_frame_forwarding_discard(dsh_frame_forwarding_client* client, dsh_frame_forwarding_surface* surface);
/** Poll and apply credits; return event->kind. Only one thread may poll at a time.
 * A negative result ends processing; preserve any diagnostic text before closing.
 */
int dsh_frame_forwarding_poll(dsh_frame_forwarding_client* client, uint32_t timeout_ms, dsh_frame_forwarding_event* event);
/** Send application JSON to the receiver; never sends pixel bytes. */
int dsh_frame_forwarding_send(dsh_frame_forwarding_client* client, const char* json);
/** Interrupt polling without freeing client. Join polling thread before close. */
void dsh_frame_forwarding_interrupt(dsh_frame_forwarding_client* client);
/** Close this producer's connection after all its API calls have returned: no further operation is sent, unsubmitted
 * and queued frames are dropped, and the receiver drains the frames it still holds before a successor holding the same
 * TX credential becomes ready. Closing does not revoke other copies of the credential; only the receiver closes the
 * channel. Receiver references stay valid. */
void dsh_frame_forwarding_close(dsh_frame_forwarding_client* client);
#ifdef __cplusplus
}
#endif
#endif
