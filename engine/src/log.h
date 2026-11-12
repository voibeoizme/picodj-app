// Engine logging: PDJ_LOG(level, "printf format", …). Goes to the callback set with pdj_set_log (the app writes it to picodj.log).
#pragma once
#include <chrono>

namespace pdj {
void logf(int level, const char* fmt, ...)
#if defined(__GNUC__)
        __attribute__((format(printf, 2, 3)))
#endif
        ;
struct Timer {
    std::chrono::steady_clock::time_point t0 = std::chrono::steady_clock::now();
    double ms() const { return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count(); }
};
} // namespace pdj

#define PDJ_DEBUG(...) ::pdj::logf(0, __VA_ARGS__)
#define PDJ_INFO(...) ::pdj::logf(1, __VA_ARGS__)
#define PDJ_WARN(...) ::pdj::logf(2, __VA_ARGS__)
#define PDJ_ERROR(...) ::pdj::logf(3, __VA_ARGS__)
