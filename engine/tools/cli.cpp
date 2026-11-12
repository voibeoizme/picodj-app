// picodj-cli — runs the engine from a terminal, without the app. For testing and bug reports.
//
//   picodj-cli analyze <file>...        BPM, first beat, key for each file (one line of JSON per file)
//   picodj-cli selftest                 analyses generated click tracks with known tempo; exit code 1 on a wrong result

#include "../src/picodj.h"

#include <cmath>
#include <cstdio>
#include <cstring>
#include <vector>

static int analyze(int argc, char** argv) {
    int bad = 0;
    for (int i = 2; i < argc; ++i) {
        pdj_analysis a;
        char err[256] = "";
        const int rc = pdj_analyze_file(argv[i], &a, err, sizeof err);
        if (rc != 0) {
            printf("{\"file\":\"%s\",\"error\":\"%s\"}\n", argv[i], err);
            ++bad;
            continue;
        }
        printf("{\"file\":\"%s\",\"bpm\":%.3f,\"first_beat\":%.4f,\"confidence\":%.2f,\"key\":\"%s\",\"key_strength\":%.2f,\"duration\":%.2f,"
               "\"ms\":{\"decode\":%.0f,\"beats\":%.0f,\"key\":%.0f}}\n",
                argv[i], a.bpm, a.first_beat, a.confidence, a.camelot, a.key_strength, a.duration, a.ms_decode, a.ms_beats, a.ms_key);
    }
    return bad ? 1 : 0;
}

// kick-like click on every beat + a quieter hat on the off-beats, with a chord so that key detection has something to do
static std::vector<float> click_track(double bpm, double first, double seconds, int sr) {
    const long long n = static_cast<long long>(seconds * sr);
    std::vector<float> s(static_cast<size_t>(n) * 2, 0.0f);
    const double per = 60.0 / bpm;
    const double notes[3] = {220.0, 261.63, 329.63}; // A minor
    for (long long i = 0; i < n; ++i) {
        const double t = static_cast<double>(i) / sr;
        double v = 0;
        for (double f : notes) {
            v += 0.05 * std::sin(2 * M_PI * f * t);
        }
        const double ph = std::fmod(t - first + per * 1000, per);
        if (t >= first && ph < 0.12) {
            v += 0.8 * std::exp(-ph * 40) * std::sin(2 * M_PI * (55 + 80 * std::exp(-ph * 30)) * ph);
        }
        const double ph2 = std::fmod(t - first + per * 1000.5, per);
        if (t >= first && ph2 < 0.03) {
            v += 0.15 * std::exp(-ph2 * 200) * ((i * 1103515245LL + 12345) % 2000 / 1000.0 - 1.0);
        }
        s[static_cast<size_t>(i) * 2] = s[static_cast<size_t>(i) * 2 + 1] = static_cast<float>(v);
    }
    return s;
}

static int selftest() {
    struct Case { double bpm, first; } cases[] = {{128, 0.10}, {120, 0.25}, {174, 0.05}, {90, 0.30}, {125.5, 0.20}, {140, 0.00}};
    int bad = 0;
    for (const Case& c : cases) {
        const int sr = 44100;
        const std::vector<float> s = click_track(c.bpm, c.first, 90, sr);
        pdj_analysis a;
        char err[256] = "";
        if (pdj_analyze_pcm(s.data(), static_cast<long long>(s.size() / 2), sr, &a, err, sizeof err) != 0) {
            printf("FAIL %.1f BPM: %s\n", c.bpm, err);
            ++bad;
            continue;
        }
        const double per = 60.0 / c.bpm;
        double off = std::fmod(a.first_beat - c.first, per);
        if (off > per / 2) off -= per;
        if (off < -per / 2) off += per;
        // half/double tempo is a matter of taste (174 vs 87), not an error
        const bool tempo = std::fabs(a.bpm - c.bpm) < 0.06 || std::fabs(a.bpm * 2 - c.bpm) < 0.12 || std::fabs(a.bpm / 2 - c.bpm) < 0.06;
        const bool ok = tempo && std::fabs(off) < 0.006;
        printf("%s  want %.2f BPM first %.3f | got %.3f BPM first %.4f (offset %+.1f ms) conf %.2f key %s\n", ok ? "ok  " : "FAIL", c.bpm, c.first,
                a.bpm, a.first_beat, off * 1000, a.confidence, a.camelot);
        bad += ok ? 0 : 1;
    }
    printf(bad ? "SELFTEST FAILED (%d)\n" : "SELFTEST OK\n", bad);
    return bad ? 1 : 0;
}

int main(int argc, char** argv) {
    if (argc >= 3 && !strcmp(argv[1], "analyze")) {
        return analyze(argc, argv);
    }
    if (argc >= 2 && !strcmp(argv[1], "selftest")) {
        return selftest();
    }
    printf("PicoDJ engine %s\nusage: picodj-cli analyze <file>... | selftest\n", pdj_version());
    return 2;
}
