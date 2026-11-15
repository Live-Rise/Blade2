#ifndef DSH_GRAPHITE_INTEROP_INTERNAL_HXX
#define DSH_GRAPHITE_INTEROP_INTERNAL_HXX
/* Internal Graphite texture adapter; not part of the public frame-forwarding SDK. */
#include "dsh_frame_forwarding.h"
#include <webgpu/webgpu.h>
namespace dsh::graphite::detail {
struct InteropContext;
struct InteropFrame;
/** Borrowed native Dawn objects. Release producer-owned references before destroying
 * the context; the SDK owns these references. No object crosses a process boundary. */
struct DawnObjects {
  WGPUInstance instance;
  WGPUAdapter adapter;
  WGPUDevice device;
  WGPUQueue queue;
};
/** Create a hardware Metal/D3D12 context independently of a display connection.
 * descriptor may be NULL. Required shared-memory features are added by the SDK.
 * Caller callbacks retain their ordinary WebGPU semantics. Returns NULL on error. */
InteropContext* create_context(const WGPUDeviceDescriptor* descriptor,
  DawnObjects* objects, char* error, size_t error_size);
/** Borrow an exclusive BGRA8 premultiplied display texture. Returns 1 when ready,
 * 0 while credits are pending, -1 on error. Poll client through dsh_frame_forwarding_poll;
 * this adapter never consumes application control messages. Serialize context calls.
 * client must outlive every frame acquired from it. */
int acquire(InteropContext* context, dsh_frame_forwarding_client* client,
  uint32_t width, uint32_t height, InteropFrame** frame, char* error, size_t error_size);
/** Borrow the current texture; NULL after publish/discard. Supports RenderAttachment,
 * TextureBinding, CopySrc and CopyDst. Retained texture/views expire at handoff. */
WGPUTexture texture(InteropFrame* frame);
/** Publish submitted GPU writes, consuming texture access even on failure.
 * Both platforms wait for submitted GPU work to complete. Submit all recordings
 * and release renderer wrappers before calling. Dawn queue submission may block
 * while acquiring a texture still used by another device.
 * Returns 1 on success, 0 on failure; release_frame remains required. */
int publish(InteropFrame* frame, uint64_t frame_id,
  const char* metadata_json, char* error, size_t error_size);
/** Publish like publish through dsh_frame_forwarding_submit_latest: the frame is
 * sent or queued without waiting for receiver credit. */
int publish_latest(InteropFrame* frame, uint64_t frame_id,
  const char* metadata_json, char* error, size_t error_size);
/** Cancel shared access using the same synchronization as publish.
 * Returns 1 on success, 0 on failure. Failed handoff or device loss quarantines the
 * allocation until the client is closed. */
int discard(InteropFrame* frame, char* error, size_t error_size);
/** Dispose the lease; an active lease is cancelled. No context/client call may race. */
void release_frame(InteropFrame* frame);
/** Release SDK context ownership. Outstanding frames retain the device; consumers'
 * imported images are owned by the existing DSH release protocol. */
void destroy_context(InteropContext* context);
}
#endif
