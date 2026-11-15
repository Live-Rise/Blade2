#include "transport.h"
#include <algorithm>
#include <cstring>
#include <cstdio>
#include <sstream>
#include <stdexcept>
#include <vector>
#include <chrono>
#if defined(__APPLE__)
#include <CoreFoundation/CoreFoundation.h>
#include <IOSurface/IOSurface.h>
#include <bsm/libbsm.h>
#include <mach/mach.h>
#include <mach/notify.h>
#include <servers/bootstrap.h>
#include <unistd.h>
#else
#define NOMINMAX
#include <windows.h>
#include <sddl.h>
#include <d3d11_4.h>
#include <dxgi1_2.h>
#include <wrl/client.h>
#endif
namespace dsh_frame_forwarding {
Endpoint parse_endpoint(const std::string& value) {
  auto a = value.find('|'), b = value.find('|', a + 1), c = value.find('|', b + 1);
  if (a == std::string::npos || b == std::string::npos || c == std::string::npos || value.substr(0, a) != "1")
    throw std::runtime_error("Unsupported frame-forwarding endpoint");
  auto pid_text = value.substr(a + 1, b - a - 1);
  size_t used = 0;
  auto pid = std::stoul(pid_text, &used);
  Endpoint result{static_cast<uint32_t>(pid), value.substr(b + 1, c - b - 1), value.substr(c + 1)};
  if (used != pid_text.size() || !pid || pid > UINT32_MAX || result.token.size() != 64 || result.name.empty() || result.name.size() > 200)
    throw std::runtime_error("Invalid frame-forwarding endpoint");
  return result;
}
uint64_t parse_connection(const char (&token)[65]) {
  if (!std::memchr(token, 0, sizeof(token)) || !token[0] || token[0] == '0' || std::strlen(token) > 16) return 0;
  uint64_t value = 0;
  for (const char* digit = token; *digit; ++digit) {
    if (*digit < '0' || *digit > '9') return 0;
    value = value * 10 + static_cast<uint64_t>(*digit - '0');
  }
  return value <= 9007199254740991ULL ? value : 0;
}
void Channel::bind_connection(uint64_t id) {
  connection_ = id;
  std::snprintf(connection_text_, sizeof(connection_text_), "%llu", static_cast<unsigned long long>(id));
}
int Channel::receive(Received& out, uint32_t timeout_ms) {
  const auto result = receive_packet(out, timeout_ms);
  if (result != 1 || !connection_ || std::strncmp(out.packet.token, connection_text_, sizeof(out.packet.token)) == 0) return result;
  release_resource(out.resource); out.resource = 0; foreign_ = true; interrupt(); return -1;
}
bool Channel::send(const Packet& packet, uintptr_t resource) {
  if (!connection_) return send_packet(packet, resource);
  Packet stamped = packet;
  std::memcpy(stamped.token, connection_text_, sizeof(stamped.token));
  return send_packet(stamped, resource);
}
#if defined(__APPLE__)
struct MachMessage {
  mach_msg_header_t header;
  mach_msg_body_t body;
  mach_msg_port_descriptor_t port;
  Packet packet;
  mach_msg_audit_trailer_t trailer;
};
/**
 * Receive one protocol message: 1 data, 0 timeout, -1 failure, -2 a message of another shape (destroyed), -3 the
 * watched peer's dead-name notice. Releasing a notification request's send-once right posts a notice that is -2.
 */
int receive_mach(mach_port_t port, uint32_t timeout, MachMessage& msg, uint32_t& pid) {
  auto result = mach_msg(&msg.header, MACH_RCV_MSG | MACH_RCV_TIMEOUT |
      MACH_RCV_TRAILER_TYPE(MACH_MSG_TRAILER_FORMAT_0) | MACH_RCV_TRAILER_ELEMENTS(MACH_RCV_TRAILER_AUDIT),
      0, sizeof(msg), port, timeout, MACH_PORT_NULL);
  if (result == MACH_RCV_TIMED_OUT) return 0;
  if (result != KERN_SUCCESS) return -1;
  if (msg.header.msgh_id == MACH_NOTIFY_DEAD_NAME) { mach_msg_destroy(&msg.header); return -3; }
  if (msg.header.msgh_size != offsetof(MachMessage, trailer) || !(msg.header.msgh_bits & MACH_MSGH_BITS_COMPLEX) ||
      msg.body.msgh_descriptor_count != 1 || msg.port.type != MACH_MSG_PORT_DESCRIPTOR) {
    mach_msg_destroy(&msg.header); return -2;
  }
  auto* trailer = reinterpret_cast<mach_msg_audit_trailer_t*>(reinterpret_cast<char*>(&msg) + msg.header.msgh_size);
  if (trailer->msgh_trailer_size < sizeof(mach_msg_audit_trailer_t)) { mach_msg_destroy(&msg.header); return -2; }
  pid = static_cast<uint32_t>(audit_token_to_pid(trailer->msgh_audit));
  return 1;
}
mach_port_t allocate_port() {
  mach_port_t port = MACH_PORT_NULL;
  if (mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &port) != KERN_SUCCESS) throw std::runtime_error("Cannot allocate frame-forwarding Mach port");
  if (mach_port_insert_right(mach_task_self(), port, port, MACH_MSG_TYPE_MAKE_SEND) != KERN_SUCCESS) {
    mach_port_mod_refs(mach_task_self(), port, MACH_PORT_RIGHT_RECEIVE, -1); throw std::runtime_error("Cannot allocate frame-forwarding Mach port");
  }
  return port;
}
void destroy_port(mach_port_t port) {
  if (!port) return;
  mach_port_deallocate(mach_task_self(), port);
  mach_port_mod_refs(mach_task_self(), port, MACH_PORT_RIGHT_RECEIVE, -1);
}
/**
 * One producer connection. The producer's hello carries its reply port; the receiver answers on it with ready, whose
 * descriptor is the connection port this channel receives on, so every connection owns its ports.
 */
class MachChannel final : public Channel {
  mach_port_t local_ = MACH_PORT_NULL, remote_ = MACH_PORT_NULL;
  bool server_;
  /** The process this connection's packets must come from: the receiver for a producer, the candidate for a receiver. */
  uint32_t source_pid_;
  std::atomic<bool> stopped_{false};
  std::atomic<uint32_t> peer_{0};
  std::mutex writes_;
  void watch_peer() {
    mach_port_t previous = MACH_PORT_NULL;
    auto result = mach_port_request_notification(mach_task_self(), remote_, MACH_NOTIFY_DEAD_NAME, 0, local_, MACH_MSG_TYPE_MAKE_SEND_ONCE, &previous);
    if (previous) mach_port_deallocate(mach_task_self(), previous);
    if (result != KERN_SUCCESS) stopped_ = true;
  }
  /** Cancel the dead-name request so releasing the right posts no port-deleted notice. */
  void unwatch_peer() {
    mach_port_t previous = MACH_PORT_NULL;
    mach_port_request_notification(mach_task_self(), remote_, MACH_NOTIFY_DEAD_NAME, 0, MACH_PORT_NULL, MACH_MSG_TYPE_MAKE_SEND_ONCE, &previous);
    if (previous) mach_port_deallocate(mach_task_self(), previous);
  }
 public:
  /** Producer side: discover the listener; ready later replaces it with the connection port. */
  explicit MachChannel(const Endpoint& endpoint) : server_(false), source_pid_(endpoint.pid) {
    local_ = allocate_port();
    if (bootstrap_look_up(bootstrap_port, const_cast<char*>(endpoint.name.c_str()), &remote_) != KERN_SUCCESS) {
      destroy_port(local_); throw std::runtime_error("Cannot discover frame-forwarding Mach endpoint");
    }
    watch_peer();
  }
  /** Receiver side: adopt the candidate's reply port and open a connection port for it. */
  MachChannel(mach_port_t reply, uint32_t pid) : remote_(reply), server_(true), source_pid_(pid) {
    try { local_ = allocate_port(); } catch (...) { mach_port_deallocate(mach_task_self(), reply); throw; }
    peer_ = pid; watch_peer();
  }
  ~MachChannel() override {
    if (remote_) { unwatch_peer(); mach_port_deallocate(mach_task_self(), remote_); }
    destroy_port(local_);
  }
  uint32_t peer_pid() const override { return peer_; }
  void interrupt() override { stopped_ = true; }
 protected:
  bool send_packet(const Packet& packet, uintptr_t resource) override {
    std::lock_guard<std::mutex> lock(writes_);
    if (stopped_ || !remote_) return false;
    MachMessage msg{};
    msg.header.msgh_bits = MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, 0) | MACH_MSGH_BITS_COMPLEX;
    msg.header.msgh_size = offsetof(MachMessage, trailer);
    msg.header.msgh_remote_port = remote_;
    msg.header.msgh_id = magic;
    msg.body.msgh_descriptor_count = 1;
    msg.port.type = MACH_MSG_PORT_DESCRIPTOR;
    msg.port.disposition = MACH_MSG_TYPE_COPY_SEND;
    mach_port_t surface_port = MACH_PORT_NULL;
    if ((!server_ && packet.kind == hello) || (server_ && packet.kind == ready)) msg.port.name = local_;
    else if (resource) msg.port.name = surface_port = IOSurfaceCreateMachPort(reinterpret_cast<IOSurfaceRef>(resource));
    msg.packet = packet;
    auto result = mach_msg(&msg.header, MACH_SEND_MSG | MACH_SEND_TIMEOUT, msg.header.msgh_size, 0, MACH_PORT_NULL, 1000, MACH_PORT_NULL);
    if (surface_port) mach_port_deallocate(mach_task_self(), surface_port);
    return result == KERN_SUCCESS;
  }
  int receive_packet(Received& out, uint32_t timeout) override {
    if (stopped_) return -1;
    MachMessage msg{}; uint32_t pid = 0;
    const auto result = receive_mach(local_, timeout, msg, pid);
    if (result == 0 || result == -2) return stopped_ ? -1 : 0;
    if (result < 0) return -1;
    out.pid = pid; out.packet = msg.packet;
    if (pid != source_pid_) { mach_msg_destroy(&msg.header); return -1; }
    if (!server_ && out.packet.kind == ready && msg.port.name) {
      // The connection port replaces the listener for every later packet.
      unwatch_peer(); mach_port_deallocate(mach_task_self(), remote_); remote_ = msg.port.name; msg.port.name = MACH_PORT_NULL; watch_peer();
    } else if (out.packet.kind == frame && msg.port.name) {
      out.resource = reinterpret_cast<uintptr_t>(IOSurfaceLookupFromMachPort(msg.port.name));
    }
    if (msg.port.name) mach_port_deallocate(mach_task_self(), msg.port.name);
    return 1;
  }
};
class MachListener final : public Listener {
  mach_port_t port_ = MACH_PORT_NULL;
  std::string name_;
  std::atomic<bool> stopped_{false};
 public:
  explicit MachListener(const Endpoint& endpoint) : name_(endpoint.name) {
    port_ = allocate_port();
    if (bootstrap_register(bootstrap_port, const_cast<char*>(name_.c_str()), port_) != KERN_SUCCESS) {
      destroy_port(port_); throw std::runtime_error("Cannot register frame-forwarding Mach endpoint");
    }
  }
  ~MachListener() override {
    const auto ignored = bootstrap_register(bootstrap_port, const_cast<char*>(name_.c_str()), MACH_PORT_NULL); (void)ignored;
    destroy_port(port_);
  }
  void interrupt() override { stopped_ = true; }
  int accept(Candidate& candidate, uint32_t timeout) override {
    if (stopped_) return -1;
    MachMessage msg{}; uint32_t pid = 0;
    const auto result = receive_mach(port_, timeout, msg, pid);
    if (result == 0 || result <= -2) return stopped_ ? -1 : 0;
    if (result < 0) return -1;
    if (msg.packet.kind != hello || !msg.port.name || msg.port.disposition != MACH_MSG_TYPE_PORT_SEND) { mach_msg_destroy(&msg.header); return 0; }
    const auto reply = msg.port.name;
    candidate.hello = Received{}; candidate.hello.packet = msg.packet; candidate.hello.pid = pid;
    candidate.channel = std::make_unique<MachChannel>(reply, pid);
    return 1;
  }
};
std::unique_ptr<Listener> listen(Endpoint& endpoint, uint32_t) { return std::make_unique<MachListener>(endpoint); }
std::unique_ptr<Channel> connect_channel(const Endpoint& endpoint, const std::function<bool()>& cancelled) {
  if (cancelled && cancelled()) throw std::runtime_error("Frame-forwarding connection cancelled or timed out");
  return std::make_unique<MachChannel>(endpoint);
}
uint32_t process_id() { return getpid(); }
void clear_validation_device() {}
std::string resource_adapter(uintptr_t) { return "IOSurface"; }
std::string device_adapter(void*) { return "IOSurface"; }
void release_resource(uintptr_t resource) { if (resource) CFRelease(reinterpret_cast<IOSurfaceRef>(resource)); }
uint64_t resource_bytes(uintptr_t resource) { return IOSurfaceGetAllocSize(reinterpret_cast<IOSurfaceRef>(resource)); }
bool resource_dimensions(uintptr_t resource, uint32_t width, uint32_t height, uint32_t stride,
    std::chrono::steady_clock::time_point, std::string& diagnostic, std::string& adapter, bool* keyed_mutex) {
  if (keyed_mutex) *keyed_mutex = false;
  diagnostic = "IOSurface format or dimensions differ"; adapter = "IOSurface";
  auto surface = reinterpret_cast<IOSurfaceRef>(resource);
  return surface && IOSurfaceGetWidth(surface) == width && IOSurfaceGetHeight(surface) == height &&
    IOSurfaceGetBytesPerRow(surface) == stride && IOSurfaceGetBytesPerElement(surface) == 4 && IOSurfaceGetPixelFormat(surface) == 0x42475241;
}
#else
using Microsoft::WRL::ComPtr;
HANDLE create_deadline_timer() {
  // The SDK also compiles with Windows 8 target headers, which omit this flag.
  constexpr DWORD high_resolution_timer = 0x00000002;
  auto timer = CreateWaitableTimerExW(nullptr, nullptr, high_resolution_timer, TIMER_MODIFY_STATE | SYNCHRONIZE);
  if (!timer && GetLastError() == ERROR_INVALID_PARAMETER)
    timer = CreateWaitableTimerExW(nullptr, nullptr, 0, TIMER_MODIFY_STATE | SYNCHRONIZE);
  return timer;
}
struct ReadbackSync {
  ComPtr<ID3D11Fence> fence;
  ComPtr<ID3D11DeviceContext4> signaling;
  ComPtr<ID3D11Query> query;
  HANDLE event = nullptr;
  uint64_t value = 0;
  bool pending = false, fence_submitted = false, event_armed = false;
  ~ReadbackSync() { if (event) CloseHandle(event); }
};
bool gpu_failure(std::string& diagnostic, const char* operation, HRESULT result) {
  std::ostringstream text; text << operation << " HRESULT=0x" << std::hex << static_cast<uint32_t>(result);
  diagnostic = text.str(); return false;
}
bool finish_gpu_submission(ID3D11DeviceContext* context, ReadbackSync& sync, std::string& diagnostic) {
  // The query remains valid when fence submission or notification fails after a copy.
  context->End(sync.query.Get());
  sync.pending = true; sync.fence_submitted = false; sync.event_armed = false;
  HRESULT result = S_OK;
  const char* operation = "Cannot signal GPU completion";
  if (sync.fence) {
    result = sync.signaling->Signal(sync.fence.Get(), ++sync.value);
    if (SUCCEEDED(result)) {
      sync.fence_submitted = true;
      operation = "Cannot arm GPU completion event";
      result = sync.fence->SetEventOnCompletion(sync.value, sync.event);
      sync.event_armed = SUCCEEDED(result);
    }
  }
  context->Flush();
  return SUCCEEDED(result) || gpu_failure(diagnostic, operation, result);
}
// Pending=0, completed=1, device removed=2; other errors retain ownership.
int poll_gpu_submission(ID3D11Device* device, ID3D11DeviceContext* context,
    ReadbackSync& sync, bool retiring, std::string& diagnostic) {
  if (!sync.pending) return 1;
  if (retiring && FAILED(device->GetDeviceRemovedReason())) { sync.pending = false; return 2; }
  if (sync.fence_submitted) {
    const auto completed = sync.fence->GetCompletedValue();
    if (completed == UINT64_MAX) { sync.pending = false; gpu_failure(diagnostic, "GPU device removed", DXGI_ERROR_DEVICE_REMOVED); return 2; }
    if (completed >= sync.value) { sync.pending = false; return 1; }
    return 0;
  }
  const auto result = context->GetData(sync.query.Get(), nullptr, 0, D3D11_ASYNC_GETDATA_DONOTFLUSH);
  if (result == S_OK) { sync.pending = false; return 1; }
  if (FAILED(device->GetDeviceRemovedReason())) { sync.pending = false; gpu_failure(diagnostic, "GPU device removed", DXGI_ERROR_DEVICE_REMOVED); return 2; }
  if (result == S_FALSE) return 0;
  gpu_failure(diagnostic, "Cannot query GPU completion", result); return -1;
}
uintptr_t create_gpu_retry_timer() { return reinterpret_cast<uintptr_t>(create_deadline_timer()); }
thread_local ID3D11Device1* validation_device = nullptr;
void clear_validation_device() {
  if (validation_device) { validation_device->Release(); validation_device = nullptr; }
}
std::string adapter_id(LUID luid) {
  std::ostringstream id; id << std::hex << static_cast<uint32_t>(luid.HighPart) << ':' << luid.LowPart;
  return id.str();
}
std::string device_adapter(void* device) {
  if (!device) return {};
  ComPtr<IDXGIDevice> dxgi; ComPtr<IDXGIAdapter> adapter; DXGI_ADAPTER_DESC desc{};
  if (FAILED(static_cast<ID3D11Device*>(device)->QueryInterface(IID_PPV_ARGS(&dxgi))) ||
      FAILED(dxgi->GetAdapter(&adapter)) || FAILED(adapter->GetDesc(&desc))) return {};
  return adapter_id(desc.AdapterLuid);
}
std::string resource_adapter(uintptr_t resource) {
  ComPtr<IDXGIFactory2> factory; LUID luid{};
  if (FAILED(CreateDXGIFactory1(IID_PPV_ARGS(&factory))) ||
      FAILED(factory->GetSharedResourceAdapterLuid(reinterpret_cast<HANDLE>(resource), &luid))) return {};
  return adapter_id(luid);
}
/** Overlapped I/O wait shared by pipe channels and the listener: 1 completed, 0 timed out, -1 stopped or failed. */
int wait_pipe(HANDLE pipe, OVERLAPPED& operation, HANDLE stop, HANDLE deadline, DWORD timeout, DWORD& count) {
  HANDLE events[] = {operation.hEvent, stop, deadline};
  const bool timed = timeout != 0 && timeout != INFINITE;
  if (timed) {
    LARGE_INTEGER due; due.QuadPart = -static_cast<LONGLONG>(timeout) * 10000;
    if (!SetWaitableTimer(deadline, &due, 0, nullptr, nullptr, FALSE)) {
      CancelIoEx(pipe, &operation);
      return GetOverlappedResult(pipe, &operation, &count, TRUE) ? 1 : -1;
    }
  }
  // A millisecond pipe poll must not round up to the system's coarse clock tick.
  auto result = WaitForMultipleObjects(timed ? 3 : 2, events, FALSE, timed ? INFINITE : timeout);
  if (timed) {
    CancelWaitableTimer(deadline);
    if (result == WAIT_OBJECT_0 + 2) result = WAIT_TIMEOUT;
  }
  if (result != WAIT_OBJECT_0) {
    CancelIoEx(pipe, &operation);
    const bool completed = GetOverlappedResult(pipe, &operation, &count, TRUE) != FALSE;
    // Completion can race the deadline; preserve a successfully received message.
    if (result == WAIT_TIMEOUT && completed) return 1;
    return result == WAIT_TIMEOUT ? 0 : -1;
  }
  return GetOverlappedResult(pipe, &operation, &count, FALSE) ? 1 : -1;
}
/**
 * One connected pipe instance: the producer's client handle, or one instance the listener accepted. A producer sends
 * its own handle values, and the receiving instance duplicates each one from the producer's process.
 */
class PipeChannel final : public Channel {
  HANDLE pipe_ = INVALID_HANDLE_VALUE, stop_ = nullptr, deadline_ = nullptr;
  /** The producer process a receiving instance duplicates handles from; null on the producer side. */
  HANDLE producer_ = nullptr;
  std::atomic<uint32_t> peer_{0};
  std::mutex writes_;
  void open_events() {
    stop_ = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    deadline_ = create_deadline_timer();
    if (!stop_ || !deadline_) throw std::runtime_error("Cannot create frame-forwarding I/O wait events");
  }
  void close_handles() {
    if (pipe_ != INVALID_HANDLE_VALUE) CloseHandle(pipe_);
    if (stop_) CloseHandle(stop_);
    if (deadline_) CloseHandle(deadline_);
    if (producer_) CloseHandle(producer_);
    pipe_ = INVALID_HANDLE_VALUE; stop_ = nullptr; deadline_ = nullptr; producer_ = nullptr;
  }
 public:
  /** Producer side: connect to the receiver's pipe and check its process identity. */
  PipeChannel(const Endpoint& endpoint, const std::function<bool()>& cancelled) {
    try {
      open_events();
      while (!WaitNamedPipeA(endpoint.name.c_str(), cancelled ? 100 : 10000)) {
        const auto error = GetLastError();
        if (cancelled && cancelled()) throw std::runtime_error("Frame-forwarding connection cancelled or timed out");
        if (!cancelled || (error != ERROR_SEM_TIMEOUT && error != ERROR_PIPE_BUSY))
          throw std::runtime_error("Frame-forwarding receiver unavailable");
      }
      pipe_ = CreateFileA(endpoint.name.c_str(), GENERIC_READ | GENERIC_WRITE, 0, nullptr, OPEN_EXISTING, FILE_FLAG_OVERLAPPED, nullptr);
      if (pipe_ == INVALID_HANDLE_VALUE) throw std::runtime_error("Frame-forwarding receiver unavailable");
      DWORD mode = PIPE_READMODE_MESSAGE; SetNamedPipeHandleState(pipe_, &mode, nullptr, nullptr);
      ULONG pid = 0;
      if (!GetNamedPipeServerProcessId(pipe_, &pid) || pid != endpoint.pid) throw std::runtime_error("Frame-forwarding receiver identity mismatch");
      peer_ = pid;
    } catch (...) { close_handles(); throw; }
  }
  /** Receiver side: adopt an instance whose client connected, with that client's process for handle duplication. */
  PipeChannel(HANDLE pipe, uint32_t pid, HANDLE producer) : pipe_(pipe), producer_(producer) {
    try { open_events(); } catch (...) { close_handles(); throw; }
    peer_ = pid;
  }
  ~PipeChannel() override { interrupt(); close_handles(); }
  uint32_t peer_pid() const override { return peer_; }
  void interrupt() override { if (stop_) SetEvent(stop_); if (pipe_ != INVALID_HANDLE_VALUE) CancelIoEx(pipe_, nullptr); }
 protected:
  /** The sender keeps a sent handle open until the receiver releases its frame, so the receiver can duplicate it. */
  bool send_packet(const Packet& source, uintptr_t resource) override {
    std::lock_guard<std::mutex> lock(writes_);
    if (WaitForSingleObject(stop_, 0) == WAIT_OBJECT_0) return false;
    Packet packet = source; packet.resource = static_cast<uint64_t>(resource);
    OVERLAPPED operation{}; operation.hEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr); DWORD count = 0;
    bool ok = WriteFile(pipe_, &packet, sizeof(packet), &count, &operation) != FALSE;
    if (!ok && GetLastError() == ERROR_IO_PENDING) ok = wait_pipe(pipe_, operation, stop_, deadline_, INFINITE, count) == 1;
    CloseHandle(operation.hEvent);
    return ok && count == sizeof(packet);
  }
  int receive_packet(Received& out, uint32_t timeout) override {
    if (WaitForSingleObject(stop_, 0) == WAIT_OBJECT_0) return -1;
    OVERLAPPED operation{}; operation.hEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr); DWORD count = 0;
    int result = ReadFile(pipe_, &out.packet, sizeof(Packet), &count, &operation) != FALSE ? 1 : -1;
    if (result < 0 && GetLastError() == ERROR_IO_PENDING) result = wait_pipe(pipe_, operation, stop_, deadline_, timeout, count);
    CloseHandle(operation.hEvent);
    if (result == 0) return WaitForSingleObject(stop_, 0) == WAIT_OBJECT_0 ? -1 : 0;
    if (result < 0 || count != sizeof(Packet)) return -1;
    out.pid = peer_; out.resource = 0;
    // Only a receiving instance takes handles, and only from the producer's own handle table.
    if (producer_ && out.packet.resource) {
      HANDLE local = nullptr;
      if (!DuplicateHandle(producer_, reinterpret_cast<HANDLE>(static_cast<uintptr_t>(out.packet.resource)), GetCurrentProcess(), &local, 0, FALSE,
          DUPLICATE_SAME_ACCESS)) {
        // An exited producer took its handles with it; a live one named a handle it does not own.
        if (WaitForSingleObject(producer_, 0) != WAIT_OBJECT_0) invalid_resource_ = true;
        interrupt(); return -1;
      }
      out.resource = reinterpret_cast<uintptr_t>(local);
    }
    return 1;
  }
};
/** Per-user pipe security; remote clients are rejected by the pipe mode. */
std::string user_pipe_sddl() {
  HANDLE token = nullptr; DWORD size = 0;
  if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) throw std::runtime_error("Cannot inspect receiver identity");
  GetTokenInformation(token, TokenUser, nullptr, 0, &size);
  std::vector<unsigned char> data(size);
  bool ok = GetTokenInformation(token, TokenUser, data.data(), size, &size) != FALSE;
  CloseHandle(token);
  LPSTR sid = nullptr;
  if (!ok || !ConvertSidToStringSidA(reinterpret_cast<TOKEN_USER*>(data.data())->User.Sid, &sid)) throw std::runtime_error("Cannot encode receiver identity");
  std::string sddl = std::string("D:P(A;;GA;;;") + sid + ")"; LocalFree(sid);
  return sddl;
}
/**
 * Pipe instances of one channel name. One instance waits for a client at a time; an instance whose client connected
 * becomes a candidate after its hello arrives within the handshake deadline. The active connection, a waiting
 * successor and a candidate being refused each hold an instance.
 */
class PipeListener final : public Listener {
  std::string name_, sddl_;
  uint32_t handshake_ms_;
  HANDLE stop_ = nullptr, deadline_ = nullptr, instance_ = INVALID_HANDLE_VALUE;
  OVERLAPPED connect_{};
  bool first_ = true, connecting_ = false;
  static constexpr DWORD instances = 4;
  void open_instance() {
    PSECURITY_DESCRIPTOR descriptor = nullptr;
    if (!ConvertStringSecurityDescriptorToSecurityDescriptorA(sddl_.c_str(), SDDL_REVISION_1, &descriptor, nullptr)) throw std::runtime_error("Cannot restrict frame-forwarding pipe");
    SECURITY_ATTRIBUTES security{sizeof(security), descriptor, FALSE};
    const DWORD open_mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | (first_ ? FILE_FLAG_FIRST_PIPE_INSTANCE : 0);
    instance_ = CreateNamedPipeA(name_.c_str(), open_mode, PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
        instances, sizeof(Packet) * 4, sizeof(Packet) * 4, 0, &security);
    LocalFree(descriptor);
    if (instance_ == INVALID_HANDLE_VALUE) throw std::runtime_error("Cannot open private frame-forwarding pipe");
    first_ = false;
  }
  void close_handles() {
    if (instance_ != INVALID_HANDLE_VALUE) { CancelIoEx(instance_, nullptr); CloseHandle(instance_); instance_ = INVALID_HANDLE_VALUE; }
    if (connect_.hEvent) { CloseHandle(connect_.hEvent); connect_.hEvent = nullptr; }
    if (stop_) { CloseHandle(stop_); stop_ = nullptr; }
    if (deadline_) { CloseHandle(deadline_); deadline_ = nullptr; }
  }
 public:
  PipeListener(const Endpoint& endpoint, uint32_t handshake_ms) : name_(endpoint.name), handshake_ms_(handshake_ms) {
    try {
      sddl_ = user_pipe_sddl();
      stop_ = CreateEventW(nullptr, TRUE, FALSE, nullptr);
      deadline_ = create_deadline_timer();
      connect_.hEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr);
      if (!stop_ || !deadline_ || !connect_.hEvent) throw std::runtime_error("Cannot create frame-forwarding I/O wait events");
      open_instance();
    } catch (...) { close_handles(); throw; }
  }
  ~PipeListener() override { close_handles(); }
  void interrupt() override { SetEvent(stop_); if (instance_ != INVALID_HANDLE_VALUE) CancelIoEx(instance_, nullptr); }
  int accept(Candidate& candidate, uint32_t timeout) override {
    if (WaitForSingleObject(stop_, 0) == WAIT_OBJECT_0) return -1;
    if (instance_ == INVALID_HANDLE_VALUE) open_instance();
    DWORD count = 0;
    if (!connecting_) {
      ResetEvent(connect_.hEvent);
      const bool connected = ConnectNamedPipe(instance_, &connect_) != FALSE;
      const auto error = GetLastError();
      if (!connected && error == ERROR_IO_PENDING) connecting_ = true;
      else if (!connected && error != ERROR_PIPE_CONNECTED) { CloseHandle(instance_); instance_ = INVALID_HANDLE_VALUE; return 0; }
    }
    if (connecting_) {
      HANDLE events[] = {connect_.hEvent, stop_};
      const auto waited = WaitForMultipleObjects(2, events, FALSE, timeout);
      if (waited == WAIT_TIMEOUT) return 0;
      if (waited != WAIT_OBJECT_0) return -1;
      connecting_ = false;
      if (!GetOverlappedResult(instance_, &connect_, &count, FALSE)) { CloseHandle(instance_); instance_ = INVALID_HANDLE_VALUE; return 0; }
    }
    // The connected instance belongs to its candidate; the next accept opens another one.
    HANDLE pipe = instance_; instance_ = INVALID_HANDLE_VALUE;
    ULONG pid = 0;
    if (!GetNamedPipeClientProcessId(pipe, &pid)) { CloseHandle(pipe); return 0; }
    HANDLE producer = OpenProcess(PROCESS_DUP_HANDLE | SYNCHRONIZE, FALSE, pid);
    if (!producer) { CloseHandle(pipe); return 0; }
    auto channel = std::make_unique<PipeChannel>(pipe, static_cast<uint32_t>(pid), producer);
    Received hello;
    if (channel->receive(hello, handshake_ms_) != 1 || hello.resource) { release_resource(hello.resource); return 0; }
    candidate.hello = hello; candidate.channel = std::move(channel);
    return 1;
  }
};
std::unique_ptr<Listener> listen(Endpoint& endpoint, uint32_t handshake_ms) { return std::make_unique<PipeListener>(endpoint, handshake_ms); }
std::unique_ptr<Channel> connect_channel(const Endpoint& endpoint, const std::function<bool()>& cancelled) { return std::make_unique<PipeChannel>(endpoint, cancelled); }
uint32_t process_id() { return GetCurrentProcessId(); }
void release_resource(uintptr_t resource) { if (resource) CloseHandle(reinterpret_cast<HANDLE>(resource)); }
uint64_t resource_bytes(uintptr_t) { return 0; }
bool source_ready(ID3D11Texture2D* texture, std::chrono::steady_clock::time_point deadline, std::string& diagnostic);
/**
 * Open a producer frame on the validation device of its adapter and check its description. The texture stays open
 * in `texture`, also when the description is invalid. While the producer renders, a keyed-mutex frame released
 * without taking key 0, or opened a second time, stays charged to this process's GPU memory after release; callers
 * take key 0 on this open and never open the frame again.
 */
bool open_frame(uintptr_t resource, uint32_t width, uint32_t height, uint32_t stride, std::string& diagnostic,
    std::string& adapter, bool* keyed_mutex, ComPtr<ID3D11Texture2D>& texture) {
  const auto failed = [&](const char* operation, HRESULT result) {
    std::ostringstream text; text << operation << " HRESULT=0x" << std::hex << static_cast<uint32_t>(result);
    if (!adapter.empty()) text << " adapter=" << adapter;
    diagnostic = text.str(); return false;
  };
  adapter = resource_adapter(resource);
  if (adapter.empty()) return failed("Cannot identify shared texture adapter", E_INVALIDARG);
  if (validation_device && device_adapter(validation_device) != adapter) clear_validation_device();
  if (!validation_device) {
    ComPtr<IDXGIFactory1> factory; ComPtr<IDXGIAdapter1> selected;
    auto result = CreateDXGIFactory1(IID_PPV_ARGS(&factory));
    if (FAILED(result)) return failed("Cannot enumerate shared texture adapters", result);
    for (UINT index = 0; ; ++index) {
      ComPtr<IDXGIAdapter1> candidate;
      result = factory->EnumAdapters1(index, &candidate);
      if (result == DXGI_ERROR_NOT_FOUND) break;
      if (FAILED(result)) return failed("Cannot enumerate shared texture adapters", result);
      DXGI_ADAPTER_DESC1 desc{};
      if (SUCCEEDED(candidate->GetDesc1(&desc)) && adapter_id(desc.AdapterLuid) == adapter) { selected = candidate; break; }
    }
    if (!selected) return failed("Shared texture adapter is unavailable", DXGI_ERROR_NOT_FOUND);
    ComPtr<ID3D11Device> device; ComPtr<ID3D11Device1> device1;
    result = D3D11CreateDevice(selected.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr, D3D11_CREATE_DEVICE_BGRA_SUPPORT, nullptr, 0, D3D11_SDK_VERSION, &device, nullptr, nullptr);
    if (FAILED(result)) return failed("D3D11 hardware receiver unavailable", result);
    result = device.As(&device1);
    if (FAILED(result)) return failed("D3D11.1 receiver unavailable", result);
    validation_device = device1.Detach();
  }
  auto result = validation_device->OpenSharedResource1(reinterpret_cast<HANDLE>(resource), IID_PPV_ARGS(&texture));
  if (FAILED(result)) return failed("Shared texture is incompatible with receiver GPU", result);
  D3D11_TEXTURE2D_DESC desc{}; texture->GetDesc(&desc);
  if (keyed_mutex) *keyed_mutex = (desc.MiscFlags & D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX) != 0;
  const bool valid = desc.Width == width && desc.Height == height && desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM &&
    stride == width * 4 && desc.MipLevels == 1 && desc.ArraySize == 1 && desc.SampleDesc.Count == 1 && (desc.MiscFlags & D3D11_RESOURCE_MISC_SHARED_NTHANDLE);
  if (!valid) diagnostic = "Shared D3D11 texture format, dimensions or sharing flags differ; adapter=" + adapter;
  return valid;
}
bool resource_dimensions(uintptr_t resource, uint32_t width, uint32_t height, uint32_t stride,
    std::chrono::steady_clock::time_point deadline, std::string& diagnostic, std::string& adapter, bool* keyed_mutex) {
  ComPtr<ID3D11Texture2D> texture;
  bool valid = open_frame(resource, width, height, stride, diagnostic, adapter, keyed_mutex, texture);
  if (!texture) return false;
  std::string synchronization;
  if (!source_ready(texture.Get(), deadline, synchronization) && valid) { diagnostic = synchronization; valid = false; }
  texture.Reset();
  // This context submits no GPU work; Flush retires released validation textures.
  ComPtr<ID3D11DeviceContext> reclamation;
  validation_device->GetImmediateContext(&reclamation);
  reclamation->Flush();
  return valid;
}
#include "relay-transport.inc"
#endif
uintptr_t receive_resource(Received& received) { auto resource = received.resource; received.resource = 0; return resource; }
}
