//! PicoDJ native side.
//!
//! The UI (ui/index.html) does the DJ work; this side gives it what a browser cannot:
//! - MIDI input from every connected controller (the Pico board over USB, or BLE via a bridge), forwarded as `midi` events,
//!   and MIDI output for LED feedback;
//! - recursive scanning of music folders and direct file reads, without per-file permission prompts.

use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, State};

const AUDIO_EXT: [&str; 9] = ["mp3", "wav", "flac", "m4a", "aac", "ogg", "aif", "aiff", "opus"];

#[derive(Serialize)]
struct FileEntry {
    path: String,
    name: String,
    size: u64,
    folder: String,
}

fn is_audio(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXT.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Every audio file under `path` (sub-folders included).
#[tauri::command]
async fn scan_folder(path: String) -> Result<Vec<FileEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = Path::new(&path);
        if !root.is_dir() {
            return Err(format!("Không tìm thấy thư mục: {path}"));
        }
        let files = walkdir::WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            // skip hidden folders and Android's app-data folder (huge, never music)
            .filter_entry(|e| {
                let n = e.file_name().to_string_lossy();
                e.depth() == 0 || !e.file_type().is_dir() || !(n.starts_with('.') || n == "Android")
            })
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file() && is_audio(e.path()))
            .map(|e| FileEntry {
                path: e.path().to_string_lossy().into_owned(),
                name: e.file_name().to_string_lossy().into_owned(),
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                folder: path.clone(),
            })
            .collect();
        Ok(files)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Whole file as raw bytes (arrives in JS as an ArrayBuffer, no JSON encoding).
#[tauri::command]
async fn read_file(path: String) -> Result<Response, String> {
    tauri::async_runtime::spawn_blocking(move || std::fs::read(&path).map(Response::new).map_err(|e| format!("{path}: {e}")))
        .await
        .map_err(|e| e.to_string())?
}

/// Part of a file (used to read ID3 tags without loading the audio).
#[tauri::command]
fn read_range(path: String, start: u64, len: u64) -> Result<Response, String> {
    let mut f = File::open(&path).map_err(|e| format!("{path}: {e}"))?;
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(len.min(16 << 20) as usize);
    f.take(len).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(Response::new(buf))
}

#[tauri::command]
fn read_text(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
fn file_exists(path: String) -> bool {
    Path::new(&path).is_file()
}

/// The app's own music folder (files dragged into the app are kept here).
fn app_music(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let d = app.path().app_data_dir().map_err(|e| e.to_string())?.join("Music");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

#[tauri::command]
fn music_dir(app: AppHandle) -> Result<String, String> {
    Ok(app_music(&app)?.to_string_lossy().into_owned())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Saves a dropped audio file (raw bytes in the request body, name in the `x-name` header) into the app's music folder.
#[tauri::command]
async fn import_file(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<String, String> {
    let tauri::ipc::InvokeBody::Raw(data) = request.body() else { return Err("thiếu dữ liệu file".into()) };
    let raw = request.headers().get("x-name").and_then(|v| v.to_str().ok()).ok_or("thiếu tên file")?;
    let name = percent_decode(raw);
    let name = Path::new(&name).file_name().and_then(|n| n.to_str()).ok_or("tên file không hợp lệ")?.to_string();
    if !is_audio(Path::new(&name)) {
        return Err(format!("không phải file nhạc: {name}"));
    }
    let dest = app_music(&app)?.join(&name);
    let same = std::fs::metadata(&dest).map(|m| m.len() == data.len() as u64).unwrap_or(false);
    if !same {
        let bytes = data.clone();
        let d2 = dest.clone();
        tauri::async_runtime::spawn_blocking(move || std::fs::write(&d2, bytes)).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    }
    Ok(dest.to_string_lossy().into_owned())
}

// ───────────────────────── log file ─────────────────────────
// Everything worth knowing for a bug report goes to <app data>/picodj.log: engine messages, JS errors, button presses.
mod logfile {
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LOG: OnceLock<Mutex<(PathBuf, Option<File>)>> = OnceLock::new();

    pub fn init(dir: PathBuf) {
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("picodj.log");
        // keep the previous session, start fresh when the file gets big
        if std::fs::metadata(&path).map(|m| m.len() > 1_500_000).unwrap_or(false) {
            let _ = std::fs::rename(&path, dir.join("picodj.old.log"));
        }
        let file = OpenOptions::new().create(true).append(true).open(&path).ok();
        let _ = LOG.set(Mutex::new((path, file)));
    }

    pub fn path() -> String {
        LOG.get().map(|l| l.lock().unwrap().0.to_string_lossy().into_owned()).unwrap_or_default()
    }

    pub fn write(src: &str, msg: &str) {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
        let line = format!("{h:02}:{m:02}:{s:02}Z [{src}] {msg}\n");
        if let Some(l) = LOG.get() {
            if let Some(f) = l.lock().unwrap().1.as_mut() {
                let _ = f.write_all(line.as_bytes());
                return;
            }
        }
        eprint!("{line}");
    }

    /// The last `max` bytes of the log.
    pub fn tail(max: usize) -> String {
        let p = path();
        let data = std::fs::read(&p).unwrap_or_default();
        let from = data.len().saturating_sub(max);
        String::from_utf8_lossy(&data[from..]).into_owned()
    }
}

#[tauri::command]
fn log_write(msg: String) {
    logfile::write("ui", &msg);
}

#[tauri::command]
fn log_tail() -> String {
    logfile::tail(60_000)
}

#[tauri::command]
fn log_path() -> String {
    logfile::path()
}

// ───────────────────────── C++ engine ─────────────────────────
mod engine {
    use serde::Serialize;
    use std::ffi::{c_char, c_int, c_void, CStr, CString};

    #[repr(C)]
    struct PdjAnalysis {
        duration: f64,
        sample_rate: c_int,
        channels: c_int,
        bpm: f64,
        first_beat: f64,
        confidence: f64,
        n_beats: c_int,
        key: c_int,
        camelot: [c_char; 4],
        key_strength: f64,
        ms_decode: f64,
        ms_beats: f64,
        ms_key: f64,
    }

    extern "C" {
        fn pdj_analyze_file(path: *const c_char, out: *mut PdjAnalysis, err: *mut c_char, err_len: c_int) -> c_int;
        fn pdj_can_decode(path: *const c_char) -> c_int;
        fn pdj_version() -> *const c_char;
        fn pdj_set_log(f: Option<extern "C" fn(c_int, *const c_char, *mut c_void)>, user: *mut c_void);
    }

    extern "C" fn on_log(level: c_int, msg: *const c_char, _user: *mut c_void) {
        if msg.is_null() {
            return;
        }
        let text = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
        let src = match level {
            0 => "engine debug",
            2 => "engine warn",
            3 => "engine ERROR",
            _ => "engine",
        };
        super::logfile::write(src, &text);
    }

    pub fn init() {
        unsafe { pdj_set_log(Some(on_log), std::ptr::null_mut()) };
        super::logfile::write("app", &format!("PicoDJ {} · engine {} · {} {}", env!("CARGO_PKG_VERSION"), version(), std::env::consts::OS, std::env::consts::ARCH));
    }

    pub fn version() -> String {
        unsafe { CStr::from_ptr(pdj_version()) }.to_string_lossy().into_owned()
    }

    #[derive(Serialize)]
    pub struct Analysis {
        pub bpm: f64,
        pub first: f64,
        pub confidence: f64,
        pub key: String,
        pub key_strength: f64,
        pub duration: f64,
        pub ms: f64,
    }

    pub fn can_decode(path: &str) -> bool {
        CString::new(path).map(|c| unsafe { pdj_can_decode(c.as_ptr()) } != 0).unwrap_or(false)
    }

    pub fn analyze(path: &str) -> Result<Analysis, String> {
        let c = CString::new(path).map_err(|e| e.to_string())?;
        let mut out: PdjAnalysis = unsafe { std::mem::zeroed() };
        let mut err = [0 as c_char; 256];
        let rc = unsafe { pdj_analyze_file(c.as_ptr(), &mut out, err.as_mut_ptr(), err.len() as c_int) };
        if rc != 0 {
            return Err(unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned());
        }
        Ok(Analysis {
            bpm: out.bpm,
            first: out.first_beat,
            confidence: out.confidence,
            key: unsafe { CStr::from_ptr(out.camelot.as_ptr()) }.to_string_lossy().into_owned(),
            key_strength: out.key_strength,
            duration: out.duration,
            ms: out.ms_decode + out.ms_beats + out.ms_key,
        })
    }
}

/// BPM, beat grid and key of one file, computed by the C++ engine on a background thread.
#[tauri::command]
async fn analyze_file(path: String) -> Result<engine::Analysis, String> {
    tauri::async_runtime::spawn_blocking(move || engine::analyze(&path)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn engine_can_decode(path: String) -> bool {
    engine::can_decode(&path)
}

#[tauri::command]
fn engine_version() -> String {
    engine::version()
}

// ───────────────────────── MIDI ─────────────────────────
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod midi {
    use super::*;
    use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct Midi {
        inputs: Mutex<Vec<(String, MidiInputConnection<()>)>>,
        outputs: Mutex<Vec<(String, MidiOutputConnection)>>,
    }

    #[derive(Serialize, Clone)]
    struct MidiMsg {
        port: String,
        data: Vec<u8>,
    }

    fn skip(name: &str) -> bool {
        // our own virtual ports, the Linux loopback, and software synths (Windows' "Microsoft GS Wavetable Synth"
        // turns every LED message into a piano note)
        let l = name.to_lowercase();
        name.contains("Midi Through") || name.starts_with("PicoDJ") || l.contains("wavetable") || l.contains("microsoft gs")
            || l.contains("fluid") || l.contains("timidity") || l.contains("synth")
    }

    /// "Pico DJ 1" / "MIDIOUT2 (Pico DJ)" → "picodj"
    fn norm(name: &str) -> String {
        let n = name.to_lowercase();
        let n = n.trim_start_matches("midiout").trim_start_matches("midiin");
        let s: String = n.chars().filter(|c| c.is_alphanumeric()).collect();
        s.trim_start_matches(|c: char| c.is_ascii_digit()).trim_end_matches(|c: char| c.is_ascii_digit()).to_string()
    }

    /// Opens every MIDI input/output that is not open yet and closes the ones that were unplugged.
    /// Safe to call repeatedly (the UI calls it every few seconds for hot-plugging).
    #[tauri::command]
    pub fn midi_connect_all(app: AppHandle, state: State<'_, Midi>) -> Result<Vec<String>, String> {
        let probe = MidiInput::new("PicoDJ probe").map_err(|e| e.to_string())?;
        let names: Vec<String> = probe.ports().iter().filter_map(|p| probe.port_name(p).ok()).filter(|n| !skip(n)).collect();
        let mut inputs = state.inputs.lock().unwrap();
        inputs.retain(|(n, _)| names.contains(n));
        for name in &names {
            if inputs.iter().any(|(n, _)| n == name) {
                continue;
            }
            let mut input = MidiInput::new("PicoDJ").map_err(|e| e.to_string())?;
            input.ignore(Ignore::All); // sysex, clock, active sensing
            let ports = input.ports();
            let Some(port) = ports.iter().find(|p| input.port_name(p).ok().as_deref() == Some(name.as_str())) else { continue };
            let (app2, nm) = (app.clone(), name.clone());
            match input.connect(port, "picodj-in", move |_ts, msg, _| {
                let _ = app2.emit("midi", MidiMsg { port: nm.clone(), data: msg.to_vec() });
            }, ()) {
                Ok(c) => inputs.push((name.clone(), c)),
                Err(e) => eprintln!("MIDI in {name}: {e}"),
            }
        }
        // outputs (LED feedback) — same device names as the inputs
        if let Ok(probe_out) = MidiOutput::new("PicoDJ probe") {
            // LED feedback only goes to the controllers we receive from (never to a synth or another app)
            let ins: Vec<String> = names.iter().map(|n| norm(n)).filter(|n| !n.is_empty()).collect();
            let out_names: Vec<String> = probe_out
                .ports()
                .iter()
                .filter_map(|p| probe_out.port_name(p).ok())
                .filter(|n| !skip(n))
                .filter(|n| {
                    let o = norm(n);
                    !o.is_empty() && ins.iter().any(|i| o.contains(i.as_str()) || i.contains(o.as_str()))
                })
                .collect();
            let mut outputs = state.outputs.lock().unwrap();
            outputs.retain(|(n, _)| out_names.contains(n));
            for name in &out_names {
                if outputs.iter().any(|(n, _)| n == name) {
                    continue;
                }
                let Ok(out) = MidiOutput::new("PicoDJ") else { continue };
                let ports = out.ports();
                let Some(port) = ports.iter().find(|p| out.port_name(p).ok().as_deref() == Some(name.as_str())) else { continue };
                if let Ok(c) = out.connect(port, "picodj-out") {
                    outputs.push((name.clone(), c));
                }
            }
        }
        Ok(inputs.iter().map(|(n, _)| n.clone()).collect())
    }

    /// Sends a short MIDI message to every connected output (LEDs on the controller).
    #[tauri::command]
    pub fn midi_send(state: State<'_, Midi>, data: Vec<u8>) {
        for (_, c) in state.outputs.lock().unwrap().iter_mut() {
            let _ = c.send(&data);
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
mod midi {
    use super::*;
    #[derive(Default)]
    pub struct Midi;
    #[tauri::command]
    pub fn midi_connect_all(_app: AppHandle, _state: State<'_, Midi>) -> Result<Vec<String>, String> {
        Ok(vec![])
    }
    #[tauri::command]
    pub fn midi_send(_state: State<'_, Midi>, _data: Vec<u8>) {}
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(midi::Midi::default())
        .setup(|app| {
            if let Ok(dir) = app.path().app_data_dir() {
                logfile::init(dir);
            }
            engine::init();
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_title(&format!("PicoDJ {}", app.package_info().version));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_folder,
            read_file,
            read_range,
            read_text,
            file_exists,
            music_dir,
            log_write,
            log_tail,
            log_path,
            analyze_file,
            engine_can_decode,
            engine_version,
            import_file,
            midi::midi_connect_all,
            midi::midi_send
        ])
        .run(tauri::generate_context!())
        .expect("error while running PicoDJ");
}
