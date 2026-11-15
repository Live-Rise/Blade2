// Office callbacks scoped to synchronous document loading.
#pragma once
#ifndef LOK_USE_UNSTABLE_API
#define LOK_USE_UNSTABLE_API
#endif
#include <LibreOfficeKit/LibreOfficeKit.h>
#include <LibreOfficeKit/LibreOfficeKitEnums.h>
#include <string>

namespace dsh {
// The helper owns this office callback only while loading one document.
class LoadDiagnostics {
    LibreOfficeKit* office_;
    void (*diagnostic_)(int, const char*);
    std::string error_;
    static void receive(int type, const char* payload, void* data)
    {
        auto& self = *static_cast<LoadDiagnostics*>(data);
        if (type == LOK_CALLBACK_ERROR && payload) self.error_ = payload;
        if (self.diagnostic_) self.diagnostic_(type, payload ? payload : "");
    }
public:
    explicit LoadDiagnostics(LibreOfficeKit* office, void (*diagnostic)(int, const char*) = nullptr)
        : office_(office), diagnostic_(diagnostic)
    {
        office_->pClass->registerCallback(office_, receive, this);
    }
    ~LoadDiagnostics() { office_->pClass->registerCallback(office_, nullptr, nullptr); }
    LoadDiagnostics(const LoadDiagnostics&) = delete;
    LoadDiagnostics& operator=(const LoadDiagnostics&) = delete;
    const std::string& error() const { return error_; }
};

}
