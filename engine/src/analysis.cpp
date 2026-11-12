// Track analysis: decode → beat tracking → constant beat grid, and key detection.
//
// Beat tracking and key detection use the Queen Mary DSP library (qm-dsp, GPL), set up the same way
// Mixxx does (src/analyzer/plugins/analyzerqueenmarybeats.cpp / analyzerqueenmarykey.cpp).
// The constant-tempo grid fit at the end is ours.

#include "picodj.h"
#include "log.h"

#include <dsp/keydetection/GetKeyMode.h>
#include <dsp/onsets/DetectionFunction.h>
#include <dsp/tempotracking/TempoTrackV2.h>
#include <maths/MathUtilities.h>

#define DR_MP3_IMPLEMENTATION
#define DR_FLAC_IMPLEMENTATION
#define DR_WAV_IMPLEMENTATION
#define DR_MP3_NO_STDIO
#define DR_FLAC_NO_STDIO
#define DR_WAV_NO_STDIO
#include <dr_mp3.h>
#include <dr_flac.h>
#include <dr_wav.h>

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <functional>
#include <memory>
#include <string>
#include <vector>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#endif

namespace {

void set_err(char* err, int len, const std::string& msg) {
    if (err && len > 0) {
        snprintf(err, static_cast<size_t>(len), "%s", msg.c_str());
    }
    PDJ_ERROR("%s", msg.c_str());
}

std::string lower_ext(const char* path) {
    std::string p(path ? path : "");
    const size_t dot = p.find_last_of('.');
    if (dot == std::string::npos) {
        return "";
    }
    std::string e = p.substr(dot + 1);
    for (char& c : e) {
        c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    }
    return e;
}

bool read_file(const char* path, std::vector<unsigned char>& out) {
#ifdef _WIN32
    // UTF-8 → UTF-16 so that Vietnamese file names open
    const int n = MultiByteToWideChar(CP_UTF8, 0, path, -1, nullptr, 0);
    std::wstring w(static_cast<size_t>(n > 0 ? n : 1), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, path, -1, &w[0], n);
    FILE* f = _wfopen(w.c_str(), L"rb");
#else
    FILE* f = fopen(path, "rb");
#endif
    if (!f) {
        return false;
    }
    fseek(f, 0, SEEK_END);
    const long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (size <= 0) {
        fclose(f);
        return false;
    }
    out.resize(static_cast<size_t>(size));
    const size_t got = fread(out.data(), 1, out.size(), f);
    fclose(f);
    return got == out.size();
}

// Mono downmix into overlapping windows (window centred on the first frame), like Mixxx's DownmixAndOverlapHelper.
class Windower {
  public:
    using Callback = std::function<void(double*)>;
    void init(size_t window, size_t step, Callback cb) {
        m_buf.assign(window, 0.0);
        m_window = window;
        m_step = step;
        m_write = window / 2;
        m_cb = std::move(cb);
    }
    void process(const float* stereo, size_t frames) { inner(stereo, frames); }
    void finish() {
        const size_t fill = m_window - m_write;
        inner(nullptr, std::max(fill, m_window / 2 - 1));
    }

  private:
    void inner(const float* in, size_t frames) {
        size_t read = 0;
        while (read < frames) {
            const size_t n = std::min(frames - read, m_window - m_write);
            for (size_t i = 0; i < n; ++i) {
                m_buf[m_write + i] = in ? (in[(read + i) * 2] + in[(read + i) * 2 + 1]) * 0.5 : 0.0;
            }
            m_write += n;
            read += n;
            if (m_write == m_window) {
                m_cb(m_buf.data());
                std::copy(m_buf.begin() + static_cast<long>(m_step), m_buf.end(), m_buf.begin());
                m_write -= m_step;
            }
        }
    }
    std::vector<double> m_buf;
    size_t m_window = 0, m_step = 0, m_write = 0;
    Callback m_cb;
};

const char* const kCamelot[25] = {"",
        // major: C  Db  D   Eb  E   F   F#  G   Ab  A   Bb  B
        "8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B",
        // minor: C  C#  D   Eb  E   F   F#  G   G#  A   Bb  B
        "5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A"};

class Analyzer {
  public:
    explicit Analyzer(int sampleRate)
            : m_sr(sampleRate) {
        // beats — same settings as Mixxx / the QM Vamp beat tracker
        m_step = static_cast<int>(m_sr * 0.01161f);
        m_windowSize = MathUtilities::nextPowerOfTwo(m_sr / 50);
        DFConfig c;
        c.DFType = DF_COMPLEXSD;
        c.stepSize = m_step;
        c.frameLength = m_windowSize;
        c.dbRise = 3;
        c.adaptiveWhitening = false;
        c.whiteningRelaxCoeff = -1;
        c.whiteningFloor = -1;
        m_df = std::make_unique<DetectionFunction>(c);
        m_beatWin.init(static_cast<size_t>(m_windowSize), static_cast<size_t>(m_step), [this](double* w) {
            m_detection.push_back(m_df->processTimeDomain(w));
        });
        // key
        GetKeyMode::Config kc(static_cast<double>(m_sr), 440.0f);
        m_key = std::make_unique<GetKeyMode>(kc);
        m_keyWin.init(static_cast<size_t>(m_key->getBlockSize()), static_cast<size_t>(m_key->getHopSize()), [this](double* w) {
            const int k = m_key->process(w);
            if (k >= 1 && k <= 24) {
                ++m_keyCount[k];
            }
        });
    }

    void feed(const float* stereo, size_t frames) {
        pdj::Timer t;
        m_beatWin.process(stereo, frames);
        m_msBeats += t.ms();
        pdj::Timer t2;
        m_keyWin.process(stereo, frames);
        m_msKey += t2.ms();
        m_frames += static_cast<long long>(frames);
    }

    void finish(pdj_analysis* out) {
        pdj::Timer t;
        m_beatWin.finish();
        std::vector<double> beats = track_beats();
        fit_grid(beats, out);
        m_msBeats += t.ms();

        pdj::Timer t2;
        m_keyWin.finish();
        long long total = 0;
        int best = 0;
        for (int k = 1; k <= 24; ++k) {
            total += m_keyCount[k];
            if (m_keyCount[k] > m_keyCount[best] || best == 0) {
                best = m_keyCount[k] > 0 ? k : best;
            }
        }
        out->key = best;
        out->key_strength = total > 0 && best ? static_cast<double>(m_keyCount[best]) / static_cast<double>(total) : 0.0;
        snprintf(out->camelot, sizeof out->camelot, "%s", kCamelot[best]);
        m_msKey += t2.ms();

        out->duration = static_cast<double>(m_frames) / m_sr;
        out->sample_rate = m_sr;
        out->ms_beats = m_msBeats;
        out->ms_key = m_msKey;
    }

  private:
    // beat positions in seconds
    std::vector<double> track_beats() {
        int n = static_cast<int>(m_detection.size());
        while (n > 0 && m_detection[static_cast<size_t>(n - 1)] <= 0.0) {
            --n;
        }
        std::vector<double> beats;
        if (n < 8) {
            return beats;
        }
        // the first 2 values may be noise detected as an onset (same as the Vamp plugin and Mixxx)
        std::vector<double> df(m_detection.begin() + 2, m_detection.begin() + n);
        std::vector<double> period(df.size(), 0.0), tempi, raw;
        TempoTrackV2 tt(static_cast<float>(m_sr), m_step);
        tt.calculateBeatPeriod(df, period, tempi);
        tt.calculateBeats(df, period, raw);
        beats.reserve(raw.size());
        for (double b : raw) {
            // + half a step: the beat lies between two detection frames
            beats.push_back((b * m_step + m_step / 2.0) / m_sr);
        }
        return beats;
    }

    // least-squares line t = a + p*k over the selected points
    static bool fit_line(const std::vector<double>& t, const std::vector<long>& k, const std::vector<char>& use, double& a, double& p) {
        double n = 0, sk = 0, st = 0, skk = 0, skt = 0;
        for (size_t i = 0; i < t.size(); ++i) {
            if (!use[i]) {
                continue;
            }
            const double kk = static_cast<double>(k[i]);
            n += 1;
            sk += kk;
            st += t[i];
            skk += kk * kk;
            skt += kk * t[i];
        }
        const double den = n * skk - sk * sk;
        if (n < 4 || std::fabs(den) < 1e-9) {
            return false;
        }
        p = (n * skt - sk * st) / den;
        a = (st - p * sk) / n;
        return true;
    }

    // One constant tempo for the whole track (what DJ software shows): robust line through the beats.
    void fit_grid(const std::vector<double>& beats, pdj_analysis* out) {
        out->n_beats = static_cast<int>(beats.size());
        out->bpm = 0;
        out->first_beat = 0;
        out->confidence = 0;
        if (beats.size() < 16) {
            return;
        }
        std::vector<double> d;
        d.reserve(beats.size());
        for (size_t i = 1; i < beats.size(); ++i) {
            d.push_back(beats[i] - beats[i - 1]);
        }
        std::vector<double> sorted = d;
        std::nth_element(sorted.begin(), sorted.begin() + static_cast<long>(sorted.size() / 2), sorted.end());
        const double med = sorted[sorted.size() / 2];
        if (med <= 0.05) {
            return;
        }
        // beat numbers (a longer gap = skipped beats)
        std::vector<long> k(beats.size(), 0);
        for (size_t i = 1; i < beats.size(); ++i) {
            k[i] = k[i - 1] + std::max(1L, std::lround(d[i - 1] / med));
        }
        // The tracker wanders at the intro/outro and in breakdowns. Start from the steadiest stretch of the track
        // (most intervals within 2 % of the median in a 64-beat window), then grow the set of beats that agree with its line.
        const size_t win = std::min<size_t>(64, d.size());
        size_t bestStart = 0;
        int bestScore = -1;
        int score = 0;
        auto steady = [&](size_t i) { return std::fabs(d[i] - med) < med * 0.02 ? 1 : 0; };
        for (size_t i = 0; i < d.size(); ++i) {
            score += steady(i);
            if (i >= win) {
                score -= steady(i - win);
            }
            if (i + 1 >= win && score > bestScore) {
                bestScore = score;
                bestStart = i + 1 - win;
            }
        }
        std::vector<char> use(beats.size(), 0);
        for (size_t i = bestStart; i <= bestStart + win && i < beats.size(); ++i) {
            use[i] = 1;
        }
        double a = 0, p = med;
        if (!fit_line(beats, k, use, a, p)) {
            return;
        }
        const double tol = 0.030; // 30 ms
        int inliers = 0;
        for (int it = 0; it < 12; ++it) {
            int count = 0;
            for (size_t i = 0; i < beats.size(); ++i) {
                use[i] = std::fabs(beats[i] - (a + p * static_cast<double>(k[i]))) < tol ? 1 : 0;
                count += use[i];
            }
            double a2, p2;
            if (!fit_line(beats, k, use, a2, p2)) {
                break;
            }
            a = a2;
            p = p2;
            if (count == inliers) {
                break;
            }
            inliers = count;
        }
        if (p <= 0.05) {
            return;
        }
        double bpm = 60.0 / p;
        // Music made on a computer has a whole-number tempo: snap when the rounded tempo fits the beats just as well.
        const double rounded = std::round(bpm);
        if (std::fabs(bpm - rounded) < 0.08 && rounded > 0) {
            const double pr = 60.0 / rounded;
            double sa = 0, n = 0;
            for (size_t i = 0; i < beats.size(); ++i) {
                if (use[i]) {
                    sa += beats[i] - pr * static_cast<double>(k[i]);
                    n += 1;
                }
            }
            if (n > 0) {
                const double ar = sa / n;
                double e0 = 0, e1 = 0;
                for (size_t i = 0; i < beats.size(); ++i) {
                    if (use[i]) {
                        const double r0 = beats[i] - (a + p * static_cast<double>(k[i]));
                        const double r1 = beats[i] - (ar + pr * static_cast<double>(k[i]));
                        e0 += r0 * r0;
                        e1 += r1 * r1;
                    }
                }
                const double rms0 = std::sqrt(e0 / n), rms1 = std::sqrt(e1 / n);
                if (rms1 < rms0 * 1.5 + 0.004) {
                    PDJ_DEBUG("bpm %.3f snapped to %.0f (rms %.1f → %.1f ms)", bpm, rounded, rms0 * 1000, rms1 * 1000);
                    p = pr;
                    a = ar;
                    bpm = rounded;
                }
            }
        }
        // keep the tempo in the usual DJ range
        while (bpm < 70.0) {
            bpm *= 2;
            p /= 2;
        }
        while (bpm >= 180.0) {
            bpm /= 2;
            p *= 2;
        }
        double first = std::fmod(a, p);
        if (first < 0) {
            first += p;
        }
        out->bpm = bpm;
        // the detection function reacts a few ms before the audible attack (measured with click tracks: 7–9 ms)
        first += 0.008;
        if (first >= p) {
            first -= p;
        }
        out->first_beat = first;
        out->confidence = static_cast<double>(inliers) / static_cast<double>(beats.size());
    }

    int m_sr;
    int m_step = 0, m_windowSize = 0;
    std::unique_ptr<DetectionFunction> m_df;
    std::unique_ptr<GetKeyMode> m_key;
    Windower m_beatWin, m_keyWin;
    std::vector<double> m_detection;
    long long m_keyCount[25] = {0};
    long long m_frames = 0;
    double m_msBeats = 0, m_msKey = 0;
};

// to interleaved stereo
void to_stereo(const float* in, size_t frames, unsigned channels, std::vector<float>& out) {
    out.resize(frames * 2);
    for (size_t i = 0; i < frames; ++i) {
        const float l = in[i * channels];
        out[i * 2] = l;
        out[i * 2 + 1] = channels > 1 ? in[i * channels + 1] : l;
    }
}

} // namespace

extern "C" int pdj_can_decode(const char* path) {
    const std::string e = lower_ext(path);
    return e == "mp3" || e == "wav" || e == "flac" || e == "aif" || e == "aiff" ? 1 : 0;
}

extern "C" int pdj_analyze_pcm(const float* stereo, long long frames, int sample_rate, pdj_analysis* out, char* err, int err_len) {
    if (!stereo || !out || frames <= 0 || sample_rate < 8000) {
        set_err(err, err_len, "analyze_pcm: bad arguments");
        return -1;
    }
    std::memset(out, 0, sizeof *out);
    try {
        Analyzer an(sample_rate);
        const long long chunk = 1 << 15;
        for (long long i = 0; i < frames; i += chunk) {
            an.feed(stereo + i * 2, static_cast<size_t>(std::min(chunk, frames - i)));
        }
        an.finish(out);
        out->channels = 2;
    } catch (const std::exception& e) {
        set_err(err, err_len, std::string("analyze_pcm: ") + e.what());
        return -4;
    }
    return 0;
}

extern "C" int pdj_analyze_file(const char* path, pdj_analysis* out, char* err, int err_len) {
    if (!path || !out) {
        set_err(err, err_len, "analyze_file: bad arguments");
        return -1;
    }
    std::memset(out, 0, sizeof *out);
    const std::string ext = lower_ext(path);
    if (!pdj_can_decode(path)) {
        set_err(err, err_len, "unsupported file type: " + ext);
        return -2;
    }
    pdj::Timer total;
    std::vector<unsigned char> bytes;
    if (!read_file(path, bytes)) {
        set_err(err, err_len, std::string("cannot read file: ") + path);
        return -3;
    }
    try {
        const size_t chunk = 1 << 14;
        std::vector<float> in, st;
        std::unique_ptr<Analyzer> an;
        unsigned channels = 0, sr = 0;
        pdj::Timer dec;
        double msDecode = 0;
        auto run = [&](const std::function<size_t(float*)>& readFrames) {
            in.resize(chunk * channels);
            an = std::make_unique<Analyzer>(static_cast<int>(sr));
            for (;;) {
                pdj::Timer t;
                const size_t got = readFrames(in.data());
                msDecode += t.ms();
                if (got == 0) {
                    break;
                }
                to_stereo(in.data(), got, channels, st);
                an->feed(st.data(), got);
            }
        };
        // trust the content, not the name (a .mp3 that is really a WAV is common)
        std::string kind = ext;
        if (bytes.size() >= 12) {
            const char* m = reinterpret_cast<const char*>(bytes.data());
            if (!std::memcmp(m, "RIFF", 4) || !std::memcmp(m, "FORM", 4) || !std::memcmp(m, "RF64", 4)) {
                kind = "wav";
            } else if (!std::memcmp(m, "fLaC", 4)) {
                kind = "flac";
            } else if (!std::memcmp(m, "ID3", 3) || (bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0)) {
                kind = "mp3";
            }
        }
        if (kind == "mp3") {
            drmp3 mp3;
            if (!drmp3_init_memory(&mp3, bytes.data(), bytes.size(), nullptr)) {
                set_err(err, err_len, "not a valid mp3 file");
                return -3;
            }
            channels = mp3.channels;
            sr = mp3.sampleRate;
            run([&](float* b) { return static_cast<size_t>(drmp3_read_pcm_frames_f32(&mp3, chunk, b)); });
            drmp3_uninit(&mp3);
        } else if (kind == "flac") {
            drflac* fl = drflac_open_memory(bytes.data(), bytes.size(), nullptr);
            if (!fl) {
                set_err(err, err_len, "not a valid flac file");
                return -3;
            }
            channels = fl->channels;
            sr = fl->sampleRate;
            run([&](float* b) { return static_cast<size_t>(drflac_read_pcm_frames_f32(fl, chunk, b)); });
            drflac_close(fl);
        } else {
            drwav wav;
            if (!drwav_init_memory(&wav, bytes.data(), bytes.size(), nullptr)) {
                set_err(err, err_len, "not a valid wav/aiff file");
                return -3;
            }
            channels = wav.channels;
            sr = wav.sampleRate;
            run([&](float* b) { return static_cast<size_t>(drwav_read_pcm_frames_f32(&wav, chunk, b)); });
            drwav_uninit(&wav);
        }
        an->finish(out);
        out->channels = static_cast<int>(channels);
        out->ms_decode = msDecode;
        PDJ_INFO("analyze %.0f ms (decode %.0f, beats %.0f, key %.0f) %.1fs %dHz → %.2f BPM first %.3fs conf %.2f beats %d key %s (%.2f) : %s",
                total.ms(), out->ms_decode, out->ms_beats, out->ms_key, out->duration, out->sample_rate, out->bpm, out->first_beat,
                out->confidence, out->n_beats, out->camelot, out->key_strength, path);
    } catch (const std::exception& e) {
        set_err(err, err_len, std::string("analysis failed: ") + e.what());
        return -4;
    }
    return 0;
}
