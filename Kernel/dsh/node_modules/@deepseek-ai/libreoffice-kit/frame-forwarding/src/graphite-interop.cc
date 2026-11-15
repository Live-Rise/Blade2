#include "graphite-interop-internal.hxx"
#include "dawn/native/DawnNative.h"
#include <algorithm>
#include <cstdio>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>
#if defined(_WIN32)
#include "dawn/native/D3DBackend.h"
#include <d3d11.h>
#include <dxgi.h>
#include <wrl/client.h>
#endif

namespace dsh::graphite::detail {
namespace {
void errorText(char* output, size_t size, const char* message) {
  if (output && size) std::snprintf(output, size, "%s", message);
}
struct Context {
  std::unique_ptr<dawn::native::Instance> instance;
  wgpu::Adapter adapter;
  wgpu::Device device;
  wgpu::Queue queue;
  bool failed = false;
};
bool completed(Context& context) {
  struct Result { bool called = false; WGPUQueueWorkDoneStatus status{}; };
  auto result = std::make_shared<Result>();
  WGPUQueueWorkDoneCallbackInfo callback = WGPU_QUEUE_WORK_DONE_CALLBACK_INFO_INIT;
  callback.mode = WGPUCallbackMode_WaitAnyOnly;
  callback.userdata1 = new std::shared_ptr<Result>(result);
  callback.callback = [](WGPUQueueWorkDoneStatus status, WGPUStringView, void* data, void*) {
    std::unique_ptr<std::shared_ptr<Result>> retained(static_cast<std::shared_ptr<Result>*>(data));
    (*retained)->called = true; (*retained)->status = status;
  };
  WGPUFutureWaitInfo wait = WGPU_FUTURE_WAIT_INFO_INIT;
  wait.future = wgpuQueueOnSubmittedWorkDone(context.queue.Get(), callback);
  return wgpuInstanceWaitAny(context.instance->Get(), 1, &wait, UINT64_MAX) == WGPUWaitStatus_Success
    && result->called && result->status == WGPUQueueWorkDoneStatus_Success;
}
#if defined(_WIN32)
LUID adapterId(ID3D11Device* device) {
  Microsoft::WRL::ComPtr<IDXGIDevice> dxgi;
  Microsoft::WRL::ComPtr<IDXGIAdapter> adapter;
  DXGI_ADAPTER_DESC description{};
  if (FAILED(device->QueryInterface(IID_PPV_ARGS(&dxgi))) || FAILED(dxgi->GetAdapter(&adapter))
      || FAILED(adapter->GetDesc(&description))) throw std::runtime_error("Cannot identify display adapter");
  return description.AdapterLuid;
}
void compatible(Context& context, dsh_frame_forwarding_client* client) {
  auto producer = static_cast<ID3D11Device*>(dsh_frame_forwarding_device(client));
  if (!producer) throw std::runtime_error("Display has no D3D11 device");
  auto expected = adapterId(producer);
  DXGI_ADAPTER_DESC description{};
  auto adapter = dawn::native::d3d::GetDXGIAdapter(context.adapter.Get());
  if (FAILED(adapter->GetDesc(&description)) || expected.HighPart != description.AdapterLuid.HighPart
      || expected.LowPart != description.AdapterLuid.LowPart)
    throw std::runtime_error("WebGPU and display adapters differ");
}
#endif
}

struct InteropContext { std::shared_ptr<Context> state; };
struct InteropFrame {
  std::shared_ptr<Context> context;
  dsh_frame_forwarding_client* client = nullptr;
  dsh_frame_forwarding_surface* surface = nullptr;
  wgpu::SharedTextureMemory memory;
  wgpu::Texture texture;
  bool active = false;
};

InteropContext* create_context(const WGPUDeviceDescriptor* requested,
  DawnObjects* objects, char* error, size_t size) {
  try {
    if (!objects) throw std::runtime_error("WebGPU object output is required");
    *objects = {};
    auto state = std::make_shared<Context>();
    const auto timedWait = wgpu::InstanceFeatureName::TimedWaitAny;
    wgpu::InstanceDescriptor instanceDescriptor{};
    instanceDescriptor.requiredFeatureCount = 1; instanceDescriptor.requiredFeatures = &timedWait;
    state->instance = std::make_unique<dawn::native::Instance>(&instanceDescriptor);
    wgpu::RequestAdapterOptions adapterOptions{};
    std::vector<WGPUFeatureName> features;
    if (requested && requested->requiredFeatureCount)
      features.assign(requested->requiredFeatures, requested->requiredFeatures + requested->requiredFeatureCount);
    auto feature = [&](WGPUFeatureName value) {
      if (std::find(features.begin(), features.end(), value) == features.end()) features.push_back(value);
    };
#if defined(__APPLE__)
    adapterOptions.backendType = wgpu::BackendType::Metal;
    feature(WGPUFeatureName_SharedTextureMemoryIOSurface);
    feature(WGPUFeatureName_SharedFenceMTLSharedEvent);
#elif defined(_WIN32)
    Microsoft::WRL::ComPtr<ID3D11Device> selectionDevice;
    if (FAILED(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        nullptr, 0, D3D11_SDK_VERSION, &selectionDevice, nullptr, nullptr)))
      throw std::runtime_error("D3D11 hardware display adapter unavailable");
    dawn::native::d3d::RequestAdapterOptionsLUID luid;
    luid.adapterLUID = adapterId(selectionDevice.Get());
    adapterOptions.nextInChain = &luid;
    adapterOptions.backendType = wgpu::BackendType::D3D12;
    feature(WGPUFeatureName_SharedTextureMemoryDXGISharedHandle);
    feature(WGPUFeatureName_SharedFenceDXGISharedHandle);
#else
#error DSH native WebGPU requires macOS or Windows
#endif
    auto adapters = state->instance->EnumerateAdapters(&adapterOptions);
    if (adapters.empty()) throw std::runtime_error("Compatible hardware WebGPU adapter unavailable");
    state->adapter = wgpu::Adapter(adapters.front().Get());
    // Without dual-source blending, Graphite coverage draws can allocate destination copies.
    if (wgpuAdapterHasFeature(state->adapter.Get(), WGPUFeatureName_DualSourceBlending))
      feature(WGPUFeatureName_DualSourceBlending);
    for (auto value : features) if (!wgpuAdapterHasFeature(state->adapter.Get(), value))
      throw std::runtime_error("WebGPU adapter lacks required shared-texture features");
    WGPUDeviceDescriptor descriptor = WGPU_DEVICE_DESCRIPTOR_INIT;
    if (requested) descriptor = *requested;
    descriptor.requiredFeatureCount = features.size(); descriptor.requiredFeatures = features.data();
    if (!descriptor.uncapturedErrorCallbackInfo.callback) {
      descriptor.uncapturedErrorCallbackInfo.callback = [](WGPUDevice const*, WGPUErrorType,
          WGPUStringView message, void*, void*) {
        std::fprintf(stderr, "WebGPU: %.*s\n", static_cast<int>(message.length), message.data);
      };
    }
    state->device = wgpu::Device::Acquire(adapters.front().CreateDevice(&descriptor));
    if (!state->device) throw std::runtime_error("WebGPU device creation failed");
    state->queue = state->device.GetQueue();
    *objects = {state->instance->Get(), state->adapter.Get(), state->device.Get(), state->queue.Get()};
    return new InteropContext{std::move(state)};
  } catch (const std::exception& exception) { errorText(error, size, exception.what()); return nullptr; }
}

int acquire(InteropContext* context, dsh_frame_forwarding_client* client,
  uint32_t width, uint32_t height, InteropFrame** output, char* error, size_t size) {
  if (output) *output = nullptr;
  dsh_frame_forwarding_surface* acquired = nullptr;
  try {
    if (!context || !client || !output) throw std::runtime_error("WebGPU context, display and frame output are required");
    if (context->state->failed || dawn::native::IsDeviceLost(context->state->device.Get())) throw std::runtime_error("WebGPU display context has failed");
#if defined(_WIN32)
    compatible(*context->state, client);
#endif
    acquired = dsh_frame_forwarding_acquire(client, width, height);
    if (!acquired) return 0;
    auto frame = std::make_unique<InteropFrame>();
    frame->context = context->state; frame->client = client; frame->surface = acquired;
    wgpu::SharedTextureMemoryDescriptor descriptor{};
#if defined(__APPLE__)
    wgpu::SharedTextureMemoryIOSurfaceDescriptor source{};
    source.ioSurface = dsh_frame_forwarding_surface_native(acquired); source.allowStorageBinding = false;
#else
    wgpu::SharedTextureMemoryDXGISharedHandleDescriptor source{};
    source.handle = dsh_frame_forwarding_surface_shared_handle(acquired); source.useKeyedMutex = true;
#endif
    descriptor.nextInChain = &source;
    frame->memory = context->state->device.ImportSharedTextureMemory(&descriptor);
    wgpu::TextureDescriptor texture{};
    texture.size = {width, height, 1}; texture.format = wgpu::TextureFormat::BGRA8Unorm;
    texture.usage = wgpu::TextureUsage::RenderAttachment | wgpu::TextureUsage::TextureBinding
      | wgpu::TextureUsage::CopySrc | wgpu::TextureUsage::CopyDst;
    frame->texture = frame->memory.CreateTexture(&texture);
    wgpu::SharedTextureMemoryBeginAccessDescriptor begin{};
    begin.initialized = false;
    if (frame->memory.BeginAccess(frame->texture, &begin) != wgpu::Status::Success)
      throw std::runtime_error("Cannot begin WebGPU shared-texture access");
    frame->active = true; *output = frame.release(); return 1;
  } catch (const std::exception& exception) {
    // No submitted commands can refer to an acquisition that has not been returned.
    if (acquired) dsh_frame_forwarding_discard(client, acquired);
    errorText(error, size, exception.what()); return -1;
  }
}

WGPUTexture texture(InteropFrame* frame) {
  return frame && frame->active ? frame->texture.Get() : nullptr;
}
namespace {
bool finish(InteropFrame* frame, char* error, size_t size) {
  if (!frame || !frame->active) { errorText(error, size, "WebGPU frame is not acquired"); return false; }
  frame->active = false;
  wgpu::SharedTextureMemoryEndAccessState access{};
  const bool ended = frame->memory.EndAccess(frame->texture, &access) == wgpu::Status::Success;
  const bool done = completed(*frame->context);
  frame->texture.Destroy(); frame->texture = nullptr; frame->memory = nullptr;
  if (!ended || !done || dawn::native::IsDeviceLost(frame->context->device.Get())) {
    frame->context->failed = true;
    // The native allocation remains acquired until client close; uncertainty cannot return credit.
    frame->surface = nullptr;
    errorText(error, size, "WebGPU shared-texture handoff failed"); return false;
  }
  return true;
}
}
namespace {
int submitFrame(InteropFrame* frame, uint64_t frameId, const char* metadata, char* error, size_t size,
  int (*submit)(dsh_frame_forwarding_client*, dsh_frame_forwarding_surface*, uint64_t, const char*)) {
  if (!finish(frame, error, size)) return 0;
  auto* surface = frame->surface; frame->surface = nullptr;
  if (submit(frame->client, surface, frameId, metadata)) return 1;
  dsh_frame_forwarding_discard(frame->client, surface);
  errorText(error, size, "Display rejected WebGPU frame"); return 0;
}
}
int publish(InteropFrame* frame, uint64_t frameId,
  const char* metadata, char* error, size_t size) {
  return submitFrame(frame, frameId, metadata, error, size, dsh_frame_forwarding_submit);
}
int publish_latest(InteropFrame* frame, uint64_t frameId,
  const char* metadata, char* error, size_t size) {
  return submitFrame(frame, frameId, metadata, error, size, dsh_frame_forwarding_submit_latest);
}
int discard(InteropFrame* frame, char* error, size_t size) {
  if (!finish(frame, error, size)) return 0;
  dsh_frame_forwarding_discard(frame->client, frame->surface); frame->surface = nullptr; return 1;
}
void release_frame(InteropFrame* frame) {
  if (frame && frame->active) discard(frame, nullptr, 0);
  delete frame;
}
void destroy_context(InteropContext* context) { delete context; }

} // namespace dsh::graphite::detail
