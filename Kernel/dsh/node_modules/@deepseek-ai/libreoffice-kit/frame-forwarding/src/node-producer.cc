#if defined(_WIN32)
// uv.h includes <windows.h>, whose min and max macros would replace std::min and time_point::max.
#define NOMINMAX
#endif
#include "dsh_frame_forwarding.h"
#include "producer-internal.h"
#include <node_api.h>
#include <uv.h>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <map>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

namespace {
using Clock = std::chrono::steady_clock;
struct Producer;
const napi_type_tag producer_tag{0x466e5cc643ce407bULL, 0xa3b8afcc01dcd6bfULL};
const napi_type_tag surface_tag{0xff1772a406cd44a0ULL, 0x8c25dd9be06f5518ULL};
enum class SurfaceState { acquired, busy, retired };
struct Surface {
  std::weak_ptr<Producer> owner;
  dsh_frame_forwarding_surface* native;
  uint32_t width, height;
  std::atomic<SurfaceState> state{SurfaceState::acquired};
};
/** Producer commands by their JavaScript code; code 5 is a latest-frame submit. */
enum class Kind { acquire = 0, write = 1, submit = 2, discard = 3, send = 4 };
constexpr uint64_t submit_latest_command = 5;
struct Job {
  Kind kind;
  napi_deferred deferred = nullptr;
  napi_ref buffer = nullptr;
  const void* pixels = nullptr;
  size_t stride = 0;
  uint32_t width = 0, height = 0;
  uint64_t frame_id = 0;
  // A submission through dsh_frame_forwarding_submit_latest.
  bool restore_lease = false, latest = false;
  std::string text;
  std::shared_ptr<Surface> surface;
  Clock::time_point deadline;
};
struct Delivery {
  enum Type { ready, result, message, error, closed } type;
  std::unique_ptr<Job> job;
  std::string text;
};
struct Producer : std::enable_shared_from_this<Producer> {
  napi_env env;
  napi_threadsafe_function callback = nullptr;
  napi_deferred ready = nullptr, closed = nullptr;
  napi_async_cleanup_hook_handle cleanup = nullptr;
  uv_async_t done{};
  bool environment_closing = false, main_released = false, delivered = false, closing_handle = false;
  bool worker_released = false, dispatcher_closed = false;
  std::atomic<bool> stopped{false}, ended{false};
  std::thread worker, polling;
  std::mutex mutex, client_mutex, dispatch_mutex;
  std::condition_variable wake;
  dsh_frame_forwarding_event incoming{};
  bool incoming_ready = false, acquisition_ready = true;
  std::deque<std::unique_ptr<Job>> jobs;
  std::vector<std::unique_ptr<Job>> acquisitions;
  std::map<dsh_frame_forwarding_surface*, std::shared_ptr<Surface>> surfaces;
  size_t pending = 0;
  uint32_t max_pending, timeout_ms;
  /** TX credential of the channel this producer connects to. */
  std::string tx;
  dsh_frame_forwarding_client* client = nullptr;

  void interrupt() {
    { std::lock_guard<std::mutex> lock(mutex); stopped = true; }
    wake.notify_all();
    std::lock_guard<std::mutex> lock(client_mutex);
    if (client) dsh_frame_forwarding_interrupt(client);
  }
  bool emit(Delivery* delivery) {
    while (true) {
      {
        std::lock_guard<std::mutex> lock(dispatch_mutex);
        if (dispatcher_closed || worker_released) { delete delivery; return false; }
        const auto result = napi_call_threadsafe_function(callback, delivery, napi_tsfn_nonblocking);
        if (result == napi_ok) return true;
        if (result != napi_queue_full) {
          // napi_closing already relinquishes this thread's TSFN reference.
          if (result != napi_closing) napi_release_threadsafe_function(callback, napi_tsfn_release);
          worker_released = true; dispatcher_closed = true;
          delete delivery; interrupt(); return false;
        }
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
  }
  void close_dispatcher() {
    std::lock_guard<std::mutex> lock(dispatch_mutex);
    // Node destroys TSFNs during environment cleanup even while native work remains.
    dispatcher_closed = true;
    if (!worker_released) { worker_released = true; napi_release_threadsafe_function(callback, napi_tsfn_release); }
    if (!main_released) { main_released = true; napi_release_threadsafe_function(callback, napi_tsfn_abort); }
  }
  void complete(std::unique_ptr<Job> job, std::string error = {}) {
    auto delivery = new Delivery{Delivery::result, std::move(job), std::move(error)};
    emit(delivery);
  }
  static int cancelled(void* data) { return static_cast<Producer*>(data)->stopped; }
  void process(std::unique_ptr<Job> job) {
    if (Clock::now() >= job->deadline) {
      if (job->surface && (job->kind == Kind::submit || job->kind == Kind::discard))
        job->restore_lease = true;
      complete(std::move(job), "Native producer operation timed out"); return;
    }
    try {
      switch (job->kind) {
        case Kind::acquire:
          acquisitions.push_back(std::move(job)); return;
        case Kind::write:
          if (!dsh_frame_forwarding_write_bgra_with_timeout(job->surface->native, job->pixels, job->stride, timeout_ms))
            throw std::runtime_error("Native producer CPU upload failed or timed out");
          break;
        case Kind::submit:
          if (!(job->latest ? dsh_frame_forwarding_submit_latest : dsh_frame_forwarding_submit)(client, job->surface->native, job->frame_id, job->text.c_str()))
            throw std::runtime_error("Native producer submission failed");
          surfaces.erase(job->surface->native); break;
        case Kind::discard:
          dsh_frame_forwarding_discard(client, job->surface->native); surfaces.erase(job->surface->native);
          acquisition_ready = true; break;
        case Kind::send:
          if (!dsh_frame_forwarding_send(client, job->text.c_str())) throw std::runtime_error("Native producer control send failed");
          break;
      }
      complete(std::move(job));
    } catch (const std::exception& exception) {
      complete(std::move(job), exception.what()); throw;
    }
  }
  void acquire_pending() {
    for (auto item = acquisitions.begin(); item != acquisitions.end();) {
      auto& job = *item;
      if (Clock::now() >= job->deadline) {
        complete(std::move(job), "Native surface acquisition timed out"); item = acquisitions.erase(item); continue;
      }
      ++item;
    }
    // A new size retires idle C SDK allocations, so only one allocation may await a grant.
    while (acquisition_ready && !acquisitions.empty()) {
      auto& job = acquisitions.front();
      auto native = dsh_frame_forwarding_acquire(client, job->width, job->height);
      if (!native) { acquisition_ready = false; break; }
      auto surface = std::make_shared<Surface>();
      surface->owner = shared_from_this(); surface->native = native; surface->width = job->width; surface->height = job->height;
      surfaces.emplace(native, surface); job->surface = surface;
      complete(std::move(job)); acquisitions.erase(acquisitions.begin());
    }
  }
  void poll() {
    while (!stopped) {
      {
        std::unique_lock<std::mutex> lock(mutex);
        wake.wait(lock, [&] { return stopped || !incoming_ready; });
        if (stopped) return;
      }
      dsh_frame_forwarding_event event{};
      bool received = false;
      const auto result = dsh_frame_forwarding_poll_activity(client, 100, &event, &received);
      if (!received && result >= 0) continue;
      std::lock_guard<std::mutex> lock(mutex);
      if (stopped) return;
      incoming = event; incoming_ready = true;
      wake.notify_all();
      if (result < 0) return;
    }
  }
  void wait_for_work() {
    auto deadline = Clock::time_point::max();
    for (const auto& job : acquisitions) deadline = std::min(deadline, job->deadline);
    std::unique_lock<std::mutex> lock(mutex);
    wake.wait_until(lock, deadline, [&] { return stopped || incoming_ready || !jobs.empty(); });
  }
  void run() {
    bool connected = false;
    std::string failure;
    try {
      char error[512]{};
      const dsh_frame_forwarding_connect_options options{nullptr, timeout_ms, this, cancelled};
      auto opened = dsh_frame_forwarding_connect_with_options(tx.c_str(), &options, error, sizeof(error));
      if (!opened) throw std::runtime_error(error[0] ? error : "Native producer connection failed");
      {
        std::lock_guard<std::mutex> lock(client_mutex); client = opened;
        if (stopped) dsh_frame_forwarding_interrupt(client);
      }
      if (stopped) throw std::runtime_error("Native producer closed");
      polling = std::thread([this] { poll(); });
      connected = true; emit(new Delivery{Delivery::ready, nullptr, {}});
      while (!stopped) {
        std::deque<std::unique_ptr<Job>> work;
        dsh_frame_forwarding_event event{};
        {
          std::lock_guard<std::mutex> lock(mutex); work.swap(jobs);
          if (incoming_ready) {
            event = incoming; incoming_ready = false;
            if (event.kind == 0) acquisition_ready = true;
            wake.notify_all();
          }
        }
        if (event.kind == 1) emit(new Delivery{Delivery::message, nullptr, event.text});
        for (auto& job : work) {
          if (stopped) complete(std::move(job), "Native producer closed");
          else {
            try { process(std::move(job)); }
            catch (...) {
              for (auto& remaining : work) if (remaining) complete(std::move(remaining), "Native producer failed");
              throw;
            }
          }
        }
        if (event.kind < 0) {
          if (!stopped) throw std::runtime_error(event.text[0] ? event.text : "Native receiver disconnected");
          break;
        }
        acquire_pending();
        wait_for_work();
      }
    } catch (const std::exception& exception) { failure = exception.what(); }
    interrupt();
    if (polling.joinable()) polling.join();
    if (!connected) emit(new Delivery{Delivery::ready, nullptr, failure.empty() ? "Native producer closed" : failure});
    if (!failure.empty()) emit(new Delivery{Delivery::error, nullptr, failure});
    std::deque<std::unique_ptr<Job>> remaining;
    { std::lock_guard<std::mutex> lock(mutex); remaining.swap(jobs); }
    for (auto& job : remaining) complete(std::move(job), "Native producer closed");
    for (auto& job : acquisitions) complete(std::move(job), "Native producer closed");
    acquisitions.clear();
    for (auto& item : surfaces) item.second->state = SurfaceState::retired;
    surfaces.clear();
    dsh_frame_forwarding_client* closing_client;
    { std::lock_guard<std::mutex> lock(client_mutex); closing_client = client; client = nullptr; }
    if (closing_client) dsh_frame_forwarding_close(closing_client);
    emit(new Delivery{Delivery::closed, nullptr, {}});
    {
      std::lock_guard<std::mutex> lock(dispatch_mutex);
      if (!worker_released) { worker_released = true; napi_release_threadsafe_function(callback, napi_tsfn_release); }
    }
    ended = true; uv_async_send(&done);
  }
};

napi_value undefined(napi_env env) { napi_value value; napi_get_undefined(env, &value); return value; }
napi_value text(napi_env env, const std::string& str) { napi_value value; napi_create_string_utf8(env, str.data(), str.size(), &value); return value; }
napi_value error(napi_env env, const std::string& str) { napi_value value; napi_create_error(env, nullptr, text(env, str), &value); return value; }
void property(napi_env env, napi_value obj, const char* key, napi_value value) { napi_set_named_property(env, obj, key, value); }
void done_closed(uv_handle_t* handle) {
  auto holder = static_cast<std::shared_ptr<Producer>*>(handle->data);
  if ((*holder)->cleanup) napi_remove_async_cleanup_hook((*holder)->cleanup);
  delete holder;
}
void abandon_setup(Producer* self, bool handle_initialized) {
  self->stopped = true; self->ended = true; self->environment_closing = true; self->closing_handle = true;
  if (self->callback) {
    self->close_dispatcher();
  }
  if (handle_initialized) uv_close(reinterpret_cast<uv_handle_t*>(&self->done), done_closed);
  else { delete static_cast<std::shared_ptr<Producer>*>(self->done.data); self->done.data = nullptr; }
}
void finish_on_main(Producer* self) {
  if (!self->ended || (!self->delivered && !self->environment_closing) || self->closing_handle) return;
  self->closing_handle = true;
  self->worker.join();
  if (!self->environment_closing) {
    napi_handle_scope scope;
    if (napi_open_handle_scope(self->env, &scope) == napi_ok) {
      napi_resolve_deferred(self->env, self->closed, undefined(self->env));
      napi_close_handle_scope(self->env, scope);
    }
  }
  if (!self->main_released) { self->main_released = true; napi_release_threadsafe_function(self->callback, napi_tsfn_release); }
  uv_close(reinterpret_cast<uv_handle_t*>(&self->done), done_closed);
}
void dispatch(napi_env env, napi_value callback, void* context, void* data) {
  std::unique_ptr<Delivery> delivery(static_cast<Delivery*>(data));
  if (!env) return;
  auto self = static_cast<Producer*>(context);
  if (delivery->type == Delivery::ready) {
    if (delivery->text.empty()) napi_resolve_deferred(env, self->ready, undefined(env));
    else napi_reject_deferred(env, self->ready, error(env, delivery->text));
    return;
  }
  if (delivery->type == Delivery::result) {
    auto& job = *delivery->job;
    if (job.kind != Kind::discard) { std::lock_guard<std::mutex> lock(self->mutex); --self->pending; }
    if (job.buffer) napi_delete_reference(env, job.buffer);
    if (job.surface && job.kind == Kind::write) {
      auto expected = SurfaceState::busy;
      job.surface->state.compare_exchange_strong(expected, SurfaceState::acquired);
    }
    if (job.restore_lease && !self->stopped) {
      auto expected = SurfaceState::retired;
      job.surface->state.compare_exchange_strong(expected, SurfaceState::acquired);
    }
    if (!delivery->text.empty()) { napi_reject_deferred(env, job.deferred, error(env, delivery->text)); return; }
    napi_value value = undefined(env);
    if (job.kind == Kind::acquire) {
      auto holder = new std::shared_ptr<Surface>(job.surface);
      napi_create_external(env, holder, [](napi_env, void* data, void*) { delete static_cast<std::shared_ptr<Surface>*>(data); }, nullptr, &value);
      napi_type_tag_object(env, value, &surface_tag);
    }
    napi_resolve_deferred(env, job.deferred, value); return;
  }
  napi_value event; napi_create_object(env, &event);
  property(env, event, "type", text(env, delivery->type == Delivery::message ? "message" : delivery->type == Delivery::error ? "error" : "closed"));
  if (!delivery->text.empty()) property(env, event, "text", text(env, delivery->text));
  napi_value result;
  napi_call_function(env, undefined(env), callback, 1, &event, &result);
  bool pending_exception = false;
  napi_is_exception_pending(env, &pending_exception);
  if (pending_exception) {
    napi_value exception; napi_get_and_clear_last_exception(env, &exception);
    const auto diagnostics = std::getenv("DSH_FRAME_FORWARDING_DIAGNOSTICS");
    if (diagnostics && std::strcmp(diagnostics, "1") == 0)
      std::fputs("DSH native producer event callback threw; delivery continues\n", stderr);
  }
  if (delivery->type == Delivery::closed) { self->delivered = true; finish_on_main(self); }
}
bool integer(napi_env env, napi_value value, uint64_t& result, uint64_t min = 0, uint64_t max = 9007199254740991ULL) {
  double number;
  if (napi_get_value_double(env, value, &number) != napi_ok || !std::isfinite(number) || std::floor(number) != number || number < min || number > max) return false;
  result = static_cast<uint64_t>(number); return true;
}
bool string(napi_env env, napi_value value, std::string& result, size_t capacity = DSH_FRAME_FORWARDING_TEXT_CAPACITY) {
  size_t size;
  if (napi_get_value_string_utf8(env, value, nullptr, 0, &size) != napi_ok || size >= capacity) return false;
  std::vector<char> buffer(size + 1);
  napi_get_value_string_utf8(env, value, buffer.data(), buffer.size(), &size);
  result.assign(buffer.data(), size); return result.find('\0') == std::string::npos;
}
napi_value invalid(napi_env env, const char* message) { napi_throw_type_error(env, nullptr, message); return nullptr; }
template<typename T> std::shared_ptr<T> external(napi_env env, napi_value value, const napi_type_tag& tag) {
  bool valid = false; void* data = nullptr;
  if (napi_check_object_type_tag(env, value, &tag, &valid) != napi_ok || !valid || napi_get_value_external(env, value, &data) != napi_ok || !data) return {};
  return *static_cast<std::shared_ptr<T>*>(data);
}
napi_value create_producer(napi_env env, napi_callback_info info) {
  size_t count = 4; napi_value args[4]; napi_get_cb_info(env, info, &count, args, nullptr, nullptr);
  uint64_t max_pending, timeout; napi_valuetype callback_type;
  auto self = std::make_shared<Producer>(); self->env = env;
  if (count != 4 || !string(env, args[0], self->tx, 4096) ||
      !integer(env, args[1], max_pending, 1, 65536) || !integer(env, args[2], timeout, 1, UINT32_MAX) ||
      napi_typeof(env, args[3], &callback_type) != napi_ok || callback_type != napi_function)
    return invalid(env, "Invalid native producer options");
  self->max_pending = static_cast<uint32_t>(max_pending); self->timeout_ms = static_cast<uint32_t>(timeout);
  napi_value result, native, ready, closed; napi_create_object(env, &result);
  napi_create_promise(env, &self->ready, &ready); napi_create_promise(env, &self->closed, &closed);
  auto external_holder = new std::shared_ptr<Producer>(self);
  napi_create_external(env, external_holder, [](napi_env, void* data, void*) { delete static_cast<std::shared_ptr<Producer>*>(data); }, nullptr, &native);
  napi_type_tag_object(env, native, &producer_tag);
  property(env, result, "native", native); property(env, result, "ready", ready); property(env, result, "closed", closed);
  auto callback_holder = new std::shared_ptr<Producer>(self);
  if (napi_create_threadsafe_function(env, args[3], nullptr, text(env, "DSH native producer"), max_pending + 4, 2,
      callback_holder, [](napi_env, void* data, void*) { delete static_cast<std::shared_ptr<Producer>*>(data); }, self.get(), dispatch, &self->callback) != napi_ok) {
    delete callback_holder;
    return invalid(env, "Cannot create native producer dispatcher");
  }
  uv_loop_t* loop;
  if (napi_get_uv_event_loop(env, &loop) != napi_ok) {
    abandon_setup(self.get(), false); return invalid(env, "Cannot find native producer event loop");
  }
  self->done.data = new std::shared_ptr<Producer>(self);
  if (uv_async_init(loop, &self->done, [](uv_async_t* handle) { finish_on_main(static_cast<std::shared_ptr<Producer>*>(handle->data)->get()); }) != 0) {
    abandon_setup(self.get(), false); return invalid(env, "Cannot create native producer completion handle");
  }
  if (napi_add_async_cleanup_hook(env, [](napi_async_cleanup_hook_handle, void* data) {
    auto self = static_cast<Producer*>(data); self->environment_closing = true;
    self->interrupt();
    self->close_dispatcher();
    finish_on_main(self);
  }, self.get(), &self->cleanup) != napi_ok) {
    abandon_setup(self.get(), true); return invalid(env, "Cannot register native producer cleanup");
  }
  try { self->worker = std::thread([self] { self->run(); }); }
  catch (const std::exception& exception) {
    abandon_setup(self.get(), true); return invalid(env, exception.what());
  }
  return result;
}
napi_value queue(napi_env env, const std::shared_ptr<Producer>& self, std::unique_ptr<Job> job, napi_value buffer = nullptr) {
  std::lock_guard<std::mutex> lock(self->mutex);
  if (self->stopped) return invalid(env, "Native producer closed");
  if (job->kind != Kind::discard && self->pending >= self->max_pending) return invalid(env, "Native producer operation queue is full");
  if (job->surface) {
    auto expected = SurfaceState::acquired;
    const auto state = job->kind == Kind::submit || job->kind == Kind::discard ? SurfaceState::retired : SurfaceState::busy;
    if (!job->surface->state.compare_exchange_strong(expected, state)) return invalid(env, "Native surface is busy or already consumed");
  }
  napi_value promise; napi_create_promise(env, &job->deferred, &promise);
  if (buffer) napi_create_reference(env, buffer, 1, &job->buffer);
  job->deadline = Clock::now() + std::chrono::milliseconds(self->timeout_ms);
  if (job->kind != Kind::discard) ++self->pending;
  self->jobs.push_back(std::move(job)); self->wake.notify_all(); return promise;
}
napi_value command(napi_env env, napi_callback_info info) {
  size_t count = 7; napi_value args[7]; napi_get_cb_info(env, info, &count, args, nullptr, nullptr);
  if (count < 2) return invalid(env, "Expected native producer command");
  auto self = external<Producer>(env, args[0], producer_tag); uint64_t kind;
  if (!self || !integer(env, args[1], kind, 0, submit_latest_command)) return invalid(env, "Invalid native producer command");
  // The latest-frame submission shares submit's lease and validation rules.
  auto job = std::make_unique<Job>(); job->latest = kind == submit_latest_command;
  job->kind = job->latest ? Kind::submit : static_cast<Kind>(kind);
  uint64_t width, height, value;
  napi_value buffer = nullptr;
  switch (job->kind) {
    case Kind::acquire:
      if (count < 4 || !integer(env, args[2], width, 1, 16384) || !integer(env, args[3], height, 1, 16384)) return invalid(env, "Invalid native surface dimensions");
      job->width = static_cast<uint32_t>(width); job->height = static_cast<uint32_t>(height);
      break;
    case Kind::write:
    case Kind::submit:
    case Kind::discard:
      if (count < 3 || !(job->surface = external<Surface>(env, args[2], surface_tag)) || job->surface->owner.lock() != self)
        return invalid(env, "Native surface does not belong to this producer");
      if (job->kind == Kind::write) {
        bool valid = false; void* data = nullptr; size_t size = 0;
        if (count != 5 || napi_is_buffer(env, args[3], &valid) != napi_ok || !valid ||
            napi_get_buffer_info(env, args[3], &data, &size) != napi_ok ||
            !integer(env, args[4], value, static_cast<uint64_t>(job->surface->width) * 4, UINT32_MAX) ||
            size < value * (job->surface->height - 1) + static_cast<uint64_t>(job->surface->width) * 4)
          return invalid(env, "CPU pixels require a Buffer with valid stride and byte length");
        job->pixels = data; job->stride = static_cast<size_t>(value); buffer = args[3];
      } else if (job->kind == Kind::submit) {
        if (count != 5 || !integer(env, args[3], job->frame_id, 1) || !string(env, args[4], job->text)) return invalid(env, "Invalid native frame ID or metadata");
      }
      break;
    case Kind::send:
      if (count < 3 || !string(env, args[2], job->text)) return invalid(env, "Native control JSON exceeds the UTF-8 limit");
      break;
  }
  return queue(env, self, std::move(job), buffer);
}
napi_value close_producer(napi_env env, napi_callback_info info) {
  size_t count = 1; napi_value args[1]; napi_get_cb_info(env, info, &count, args, nullptr, nullptr);
  auto self = count == 1 ? external<Producer>(env, args[0], producer_tag) : nullptr;
  if (!self) return invalid(env, "Expected native producer");
  self->interrupt(); return undefined(env);
}
}

/** Register producer commands beside the receiver in the same Node-API binary. */
void register_node_producer(napi_env env, napi_value exports) {
  const napi_property_descriptor methods[] = {
    {"createProducer", nullptr, create_producer, nullptr, nullptr, nullptr, napi_default, nullptr},
    {"producerCommand", nullptr, command, nullptr, nullptr, nullptr, napi_default, nullptr},
    {"closeProducer", nullptr, close_producer, nullptr, nullptr, nullptr, napi_default, nullptr},
  };
  napi_define_properties(env, exports, sizeof(methods) / sizeof(methods[0]), methods);
}
