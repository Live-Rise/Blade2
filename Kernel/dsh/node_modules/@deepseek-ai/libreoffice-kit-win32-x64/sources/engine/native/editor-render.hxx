// Shared-texture and input transport. Document access stays on the VCL thread.
#pragma once

class RenderConnection {
public:
    struct Frame {
        std::chrono::steady_clock::time_point drawingStarted;
        dsh_frame_forwarding_surface* raster = nullptr;
        bool ganesh = false;
#ifdef SK_GRAPHITE
        office_ganesh::Target ganeshTarget(int width, int height) const {
            return { dsh_frame_forwarding_surface_native(raster), dsh_frame_forwarding_surface_shared_handle(raster), width, height };
        }
        dsh::graphite::detail::InteropFrame* graphite = nullptr;
        WGPUTexture texture() const { return graphite ? dsh::graphite::detail::texture(graphite) : nullptr; }
#else
        WGPUTexture texture() const { return nullptr; }
#endif
    };
private:
    dsh_frame_forwarding_client* client = nullptr;
    std::thread reader;
    std::atomic<bool> stopping{false};
    std::mutex mutex;
    std::deque<std::string> incoming;
    size_t incomingBytes = 0;
    std::string failure;
    LibreOfficeKitDocument* document;
    State& state;
    FILE* output;
    bool writable;
    const bool diagnostics = [] { const char* value = std::getenv("OFFICE_GRAPHITE_DIAGNOSTICS"); return value && std::strcmp(value, "1") == 0; }();
    ModificationListener& modifications;
    std::function<bool()> playing;
    uint64_t sequence = 0, frameId = 0, viewportVersion = 0;
    int originX = 0, originY = 0, pixelSize = 1, twipSize = 15;
    double dpr = 1;
    int viewWidth = 1, viewHeight = 1;
    bool visible = true;
    std::set<int> heldKeys;
    int pointerX = 0, pointerY = 0, heldButtons = 0;

    void emit(tools::JsonWriter& json)
    {
        const OString message = json.finishAndGetAsOString();
        if (!dsh_frame_forwarding_send(client, message.getStr())) throw std::runtime_error("Frame-forwarding control channel closed");
    }
    void clearInput()
    {
        for (const int key : heldKeys) document->pClass->postKeyEvent(document, 1, 0, key);
        heldKeys.clear();
        if (heldButtons) document->pClass->postMouseEvent(document, 1, pointerX, pointerY, 1, heldButtons, 0);
        heldButtons = 0;
        document->pClass->postWindowExtTextInputEvent(document, 0, 2, "");
    }
    int coordinate(const boost::property_tree::ptree& event, bool xAxis)
    {
        const double x = event.get<double>("x"), y = event.get<double>("y");
        double result;
        if (auto transform = event.get_child_optional("coordinateTransform")) {
            const auto& matrix = *transform;
            result = xAxis ? matrix.get<double>("a") * x + matrix.get<double>("c") * y + matrix.get<double>("e")
                : matrix.get<double>("b") * x + matrix.get<double>("d") * y + matrix.get<double>("f");
        } else result = (xAxis ? originX : originY) + (xAxis ? x : y) / dpr * twipSize / pixelSize;
        if (!std::isfinite(result) || result < 0 || result > INT_MAX) throw std::runtime_error("Native input coordinate outside document");
        return static_cast<int>(std::lround(result));
    }
    struct InputDeviceLost : std::runtime_error {
        explicit InputDeviceLost(const std::string& message) : std::runtime_error(message) {}
    };
    void requireInputDevice() const
    {
#ifdef SK_GRAPHITE
        if (office_ganesh::initialized() && !office_ganesh::healthy())
            throw InputDeviceLost("Ganesh device lost; input is frozen until CPU retry");
        if (graphiteContext && !office_graphite::healthy()) {
            std::string message = "Graphite device lost; input is frozen until CPU retry";
            if (const auto cause = office_graphite::failure(); !cause.empty()) message += ": " + cause;
            throw InputDeviceLost(message);
        }
#endif
    }
    void apply(const boost::property_tree::ptree& event)
    {
        requireInputDevice();
        const auto kind = event.get<std::string>("type");
        if (playing() && kind != "focus") throw std::runtime_error("Stop the slideshow before editing or changing views");
        if (kind == "focus") { if (!event.get<bool>("focused")) clearInput(); }
        else if (kind == "pointer") {
            pointerX = coordinate(event, true); pointerY = coordinate(event, false);
            const auto action = event.get<std::string>("action");
            const int type = action == "down" ? 0 : action == "up" ? 1 : action == "move" ? 2 : -1;
            if (type < 0) throw std::runtime_error("Unknown native pointer action");
            const int buttons = bounded(event, "buttons", 0, 7);
            int lokButtons = (buttons & 1) | ((buttons & 2) << 1) | ((buttons & 4) >> 1);
            if (type != 2) {
                const int button = bounded(event, "button", 0, 2);
                lokButtons |= button == 0 ? 1 : button == 1 ? 2 : 4;
            }
            document->pClass->postMouseEvent(document, type, pointerX, pointerY, event.get<int>("clickCount", 1), lokButtons,
                bounded(event, "modifiers", 0, 65535));
            heldButtons = type == 1 ? ((buttons & 1) | ((buttons & 2) << 1) | ((buttons & 4) >> 1)) : lokButtons;
        } else if (kind == "key") {
            static const std::map<std::string, int> keys {{"ArrowDown",1024},{"ArrowUp",1025},{"ArrowLeft",1026},{"ArrowRight",1027},
                {"Home",1028},{"End",1029},{"PageUp",1030},{"PageDown",1031},{"Enter",1280},{"Escape",1281},{"Tab",1282},{"Backspace",1283},{"Delete",1286}};
            const auto key = event.get<std::string>("key");
            const auto found = keys.find(key);
            if (found == keys.end()) throw std::runtime_error("Unsupported native Office key; use text input for characters");
            const int code = found->second | bounded(event, "modifiers", 0, 65535);
            const auto action = event.get<std::string>("action");
            if (action != "down" && action != "up") throw std::runtime_error("Unknown native key action");
            if (!writable && !(found->second >= 1024 && found->second <= 1031) && found->second != 1281 && found->second != 1282)
                throw std::runtime_error("Document is read-only");
            document->pClass->postKeyEvent(document, action == "down" ? 0 : 1, 0, code);
            if (action == "down") heldKeys.insert(code); else heldKeys.erase(code);
        } else if (kind == "text") {
            if (!writable) throw std::runtime_error("Document is read-only");
            const auto text = event.get<std::string>("text");
            document->pClass->postWindowExtTextInputEvent(document, 0, 0, text.c_str());
            document->pClass->postWindowExtTextInputEvent(document, 0, 2, "");
        } else if (kind == "composition") {
            if (!writable) throw std::runtime_error("Document is read-only");
            const auto action = event.get<std::string>("action"), text = event.get<std::string>("text");
            if (action != "start" && action != "update" && action != "end") throw std::runtime_error("Unknown composition action");
            document->pClass->postWindowExtTextInputEvent(document, 0, action == "end" ? 2 : 0, text.c_str());
        } else if (kind == "wheel") {
            // Office scrolls cached tiles in Chromium; native visibility arrives through viewport.
            const auto dx = event.get<double>("deltaX"), dy = event.get<double>("deltaY");
            if (!std::isfinite(dx) || !std::isfinite(dy)) throw std::runtime_error("Invalid native wheel delta");
        } else throw std::runtime_error("Unsupported native input event");
        settle(state, document, output);
    }
public:
    /** Connect with a channel's TX credential; waiting for a previous producer to drain is bounded by timeoutMs. */
    RenderConnection(const std::string& tx, uint32_t timeoutMs, LibreOfficeKitDocument* doc, State& queue, FILE* stream, bool canWrite, ModificationListener& listener, std::function<bool()> playback)
        : document(doc), state(queue), output(stream), writable(canWrite), modifications(listener), playing(std::move(playback))
    {
        char error[256] = {};
        void* device = nullptr;
#ifdef SK_GRAPHITE
        if (std::strcmp(dsh_frame_forwarding_backend(), "ganesh") == 0) device = office_ganesh::producerDevice();
#endif
        dsh_frame_forwarding_connect_options options{};
        options.device = device; options.timeout_ms = timeoutMs;
        client = dsh_frame_forwarding_connect_with_options(tx.c_str(), &options, error, sizeof(error));
        if (!client) throw std::runtime_error(error);
        reader = std::thread([this] {
            while (!stopping) {
                dsh_frame_forwarding_event event{};
                const int result = dsh_frame_forwarding_poll(client, 100, &event);
                {
                    std::lock_guard lock(mutex);
                    if (result < 0) { if (!stopping) failure = event.text[0] ? event.text : "Frame-forwarding producer disconnected"; break; }
                    if (event.kind == 1) {
                        const auto length = std::strlen(event.text);
                        if (incomingBytes + length > MaxQueuedBytes) { failure = "Native input queue exceeded its byte limit"; break; }
                        incomingBytes += length; incoming.emplace_back(event.text);
                    }
                }
                state.changed.notify_all();
            }
        });
    }
    ~RenderConnection()
    {
        stopping = true; dsh_frame_forwarding_interrupt(client);
        if (reader.joinable()) reader.join();
        dsh_frame_forwarding_close(client);
        try { clearInput(); } catch (const std::exception&) { /* Document teardown also clears native input state. */ }
    }
    void discard(Frame* frame)
    {
#ifdef SK_GRAPHITE
        if (frame->graphite) dsh::graphite::detail::release_frame(frame->graphite);
        else
#endif
        dsh_frame_forwarding_discard(client, frame->raster);
        delete frame;
    }
    void viewport(int x, int y, int pixels, int twips) { originX = x; originY = y; pixelSize = pixels; twipSize = twips; }
    void pump()
    {
        document->pClass->setView(document, state.view);
        struct ModificationScope {
            ModificationListener& listener; bool previous;
            explicit ModificationScope(ModificationListener& value) : listener(value), previous(value.suppressed) { listener.suppressed = false; }
            ~ModificationScope() { listener.suppressed = previous; }
        } recording(modifications);
        for (;;) {
            std::string message;
            {
                std::lock_guard lock(mutex);
                if (!failure.empty()) throw std::runtime_error(failure);
                if (incoming.empty()) return;
                message = std::move(incoming.front()); incoming.pop_front(); incomingBytes -= message.size();
            }
            boost::property_tree::ptree value;
            std::istringstream input(message); boost::property_tree::read_json(input, value);
            const auto type = value.get<std::string>("type");
            try {
                if (type == "viewport") {
                    const auto& view = value.get_child("viewport");
                    const auto version = view.get<uint64_t>("version");
                    const auto scale = view.get<double>("deviceScaleFactor");
                    if (version <= viewportVersion || !std::isfinite(scale) || scale <= 0 || scale > 16)
                        throw std::runtime_error("Invalid native viewport version or scale");
                    viewWidth = bounded(view, "width", 1, 65536); viewHeight = bounded(view, "height", 1, 65536);
                    visible = view.get<bool>("visible");
                    if (!visible) clearInput();
                    viewportVersion = version; dpr = scale;
                    tools::JsonWriter ack; ack.put("type", "viewport-applied"); ack.put("version", static_cast<int64_t>(version));
                    ack.put("requestId", value.get<std::string>("requestId")); emit(ack);
                    // A hidden view returns its idle surfaces to the window budget; the next visible frame allocates
                    // again. A trim that cannot reach the receiver records the failure, which polling reports.
                    if (!visible) dsh_frame_forwarding_trim(client);
                } else if (type == "input" || type == "cancel-input") {
                    requireInputDevice();
                    const auto next = value.get<uint64_t>("sequence");
                    if (next != sequence + 1 || next > 9007199254740991ULL) throw std::runtime_error("Native input sequence is not contiguous");
                    if (type == "input") {
                        if (value.get<uint64_t>("viewportVersion") != viewportVersion) throw std::runtime_error("Native input viewport is stale");
                        apply(value.get_child("event"));
                    } else { clearInput(); settle(state, document, output); }
                    sequence = next;
                    tools::JsonWriter ack; ack.put("type", "input-applied"); ack.put("sequence", static_cast<int64_t>(sequence)); emit(ack);
                } else throw std::runtime_error("Unsupported native producer control message");
            } catch (const InputDeviceLost& error) {
                // The health check runs before input mutation; sequence counts only settled operations.
                tools::JsonWriter rejected;
                rejected.put("type", "input-rejected");
                rejected.put("sequence", value.get<int64_t>("sequence", 0));
                rejected.put("reason", "device-lost");
                rejected.put("appliedSequence", static_cast<int64_t>(sequence));
                rejected.put("error", error.what()); emit(rejected);
            } catch (const std::exception& error) {
                tools::JsonWriter rejected;
                rejected.put("type", type == "viewport" ? "viewport-rejected" : "input-rejected");
                if (type == "viewport") { rejected.put("version", value.get<int64_t>("viewport.version", 0)); rejected.put("requestId", value.get<std::string>("requestId", "")); }
                else rejected.put("sequence", value.get<int64_t>("sequence", 0));
                rejected.put("error", error.what()); emit(rejected);
                std::lock_guard lock(mutex); failure = error.what(); throw;
            }
        }
    }
    void barrier(uint64_t required)
    {
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        do {
            pump(); if (sequence >= required) return;
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Native input barrier timed out");
            SolarMutexGuard guard; Application::Reschedule(true);
            std::unique_lock lock(state.mutex); state.changed.wait_for(lock, std::chrono::milliseconds(5));
        } while (true);
    }
    uint64_t generation() const { return viewportVersion; }
    int width() const { return viewWidth; }
    int height() const { return viewHeight; }
    bool isVisible() const { return visible; }
    Frame* tryAcquire(int width, int height)
    {
        auto frame = std::make_unique<Frame>();
#ifdef SK_GRAPHITE
        frame->ganesh = !graphiteContext && std::strcmp(dsh_frame_forwarding_backend(), "ganesh") == 0;
        if (graphiteContext) {
            char error[512]{};
            const auto result = dsh::graphite::detail::acquire(graphiteContext, client, width, height, &frame->graphite, error, sizeof(error));
            if (result < 0) throw std::runtime_error(error);
            if (result == 0) return nullptr;
        } else
#endif
        {
            frame->raster = dsh_frame_forwarding_acquire(client, width, height);
            if (!frame->raster) return nullptr;
        }
        if (diagnostics) frame->drawingStarted = std::chrono::steady_clock::now();
        return frame.release();
    }
    void streamFailed(const std::string& error)
    {
        tools::JsonWriter message; message.put("type", "error"); message.put("message", "PPT stream rendering failed: " + error); emit(message);
    }
    Frame* acquire(int width, int height)
    {
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        for (;;) {
            pump();
            if (auto surface = tryAcquire(width, height)) return surface;
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Native texture credit timed out");
            SolarMutexGuard guard; Application::Reschedule(true);
            std::unique_lock lock(state.mutex); state.changed.wait_for(lock, std::chrono::milliseconds(5));
        }
    }
    // A latest-frame submission never waits for receiver credit; the SDK keeps only the newest unsent frame.
    void submit(Frame* surface, std::vector<unsigned char>& pixels, int width, int height,
        const boost::property_tree::ptree& metadata, bool latest = false)
    {
        const auto drawingFinished = diagnostics ? std::chrono::steady_clock::now() : std::chrono::steady_clock::time_point{};
        const auto generation = metadata.get<uint64_t>("generation");
        if (generation != viewportVersion) throw std::runtime_error("Native frame viewport is stale");
        tools::JsonWriter json;
        json.put("requestId", metadata.get<std::string>("requestId")); json.put("generation", static_cast<int64_t>(generation));
        json.put("revision", static_cast<int64_t>(sequence)); json.put("inputSequence", static_cast<int64_t>(sequence));
        json.put("width", width); json.put("height", height);
        if (auto tile = metadata.get_optional<std::string>("tileKey")) json.put("tileKey", *tile);
        const auto text = json.finishAndGetAsOString();
        const auto publishStarted = diagnostics ? std::chrono::steady_clock::now() : std::chrono::steady_clock::time_point{};
#ifdef SK_GRAPHITE
        if (surface->graphite) {
            char error[512]{};
            if (!(latest ? dsh::graphite::detail::publish_latest : dsh::graphite::detail::publish)(surface->graphite, ++frameId, text.getStr(), error, sizeof(error)))
                throw std::runtime_error(error);
            dsh::graphite::detail::release_frame(surface->graphite);
        } else
#endif
        if ((!surface->ganesh && !dsh_frame_forwarding_write_bgra(surface->raster, pixels.data(), static_cast<size_t>(width) * 4))
            || !(latest ? dsh_frame_forwarding_submit_latest : dsh_frame_forwarding_submit)(client, surface->raster, ++frameId, text.getStr()))
            throw std::runtime_error("Native shared frame submission failed");
        if (diagnostics) {
            const auto published = std::chrono::steady_clock::now();
            tools::JsonWriter timing;
            timing.put("frameId", static_cast<int64_t>(frameId));
            timing.put("requestId", metadata.get<std::string>("requestId"));
            timing.put("inputSequence", static_cast<int64_t>(sequence));
            timing.put("generation", static_cast<int64_t>(generation));
            timing.put("width", width); timing.put("height", height);
            timing.put("backend", surface->ganesh ? "ganesh" : surface->raster ? "raster" : "graphite");
            timing.put("drawSubmitCpuMs", std::chrono::duration<double, std::milli>(drawingFinished - surface->drawingStarted).count());
            timing.put("publishWaitMs", std::chrono::duration<double, std::milli>(published - publishStarted).count());
            std::fprintf(stderr, "officeGraphiteFrame %s\n", timing.finishAndGetAsOString().getStr());
#ifdef SK_GRAPHITE
            if (surface->graphite) office_graphite::report();
#endif
        }
        delete surface;
        pixels.clear();
    }
};
