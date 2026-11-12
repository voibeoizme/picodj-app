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
            import_file,
            midi::midi_connect_all,
            midi::midi_send
        ])
        .run(tauri::generate_context!())
        .expect("error while running PicoDJ");
}
