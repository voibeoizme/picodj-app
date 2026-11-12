/* PicoDJ engine — C API (called from the Rust shell and from the command-line test tool).
 *
 * The engine is plain C++17 with no UI or platform code, so the same sources build for
 * Windows, Android and the Linux test tool. Everything here is safe to call from any thread.
 */
#ifndef PICODJ_H
#define PICODJ_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct pdj_analysis {
    double duration;     /* seconds */
    int sample_rate;     /* Hz */
    int channels;
    double bpm;          /* 0 = not found */
    double first_beat;   /* seconds, position of the first beat of the constant grid (0 <= first_beat < one beat) */
    double confidence;   /* 0…1: share of detected beats that sit on the constant grid */
    int n_beats;         /* beats found by the tracker */
    int key;             /* 0 = none, 1…12 = C…B major, 13…24 = C…B minor (chromatic, like Mixxx) */
    char camelot[4];     /* "8A", "11B", "" */
    double key_strength; /* 0…1: share of the track spent in that key */
    double ms_decode;    /* timings, for the debug log */
    double ms_beats;
    double ms_key;
} pdj_analysis;

/* Decodes the file (mp3 / wav / aiff / flac) and finds BPM, beat grid and key.
 * `path` is UTF-8. Returns 0 on success; otherwise a negative code and a message in `err`. */
int pdj_analyze_file(const char* path, pdj_analysis* out, char* err, int err_len);

/* Same analysis on audio already in memory: interleaved stereo float, `frames` frames. */
int pdj_analyze_pcm(const float* stereo, long long frames, int sample_rate, pdj_analysis* out, char* err, int err_len);

/* 1 if pdj_analyze_file can decode this file type (decided by extension). */
int pdj_can_decode(const char* path);

const char* pdj_version(void);

/* Debug log: every engine message goes to this callback (level: 0 debug, 1 info, 2 warning, 3 error).
 * Without a callback messages go to stderr. */
typedef void (*pdj_log_fn)(int level, const char* msg, void* user);
void pdj_set_log(pdj_log_fn fn, void* user);

#ifdef __cplusplus
}
#endif
#endif
