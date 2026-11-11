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
        // our own virtual ports and the Linux loopback
        name.contains("Midi Through") || name.starts_with("PicoDJ")
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
            let out_names: Vec<String> = probe_out.ports().iter().filter_map(|p| probe_out.port_name(p).ok()).filter(|n| !skip(n)).collect();
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
            midi::midi_connect_all,
            midi::midi_send
        ])
        .run(tauri::generate_context!())
        .expect("error while running PicoDJ");
}
