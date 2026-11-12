#include "log.h"
#include "picodj.h"

#include <atomic>
#include <cstdarg>
#include <cstdio>
#include <mutex>

namespace {
std::mutex g_mutex;
pdj_log_fn g_fn = nullptr;
void* g_user = nullptr;
} // namespace

void pdj_set_log(pdj_log_fn fn, void* user) {
    std::lock_guard<std::mutex> lock(g_mutex);
    g_fn = fn;
    g_user = user;
}

const char* pdj_version(void) {
    return "0.9.0";
}

namespace pdj {
void logf(int level, const char* fmt, ...) {
    char buf[1024];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_fn) {
        g_fn(level, buf, g_user);
    } else {
        static const char* names[] = {"debug", "info", "warn", "ERROR"};
        fprintf(stderr, "[engine %s] %s\n", names[level < 0 || level > 3 ? 1 : level], buf);
    }
}
} // namespace pdj
