// Builds the C++ engine (../engine) into the app, then the Tauri glue.
use std::path::Path;

fn main() {
    let eng = Path::new("../engine");
    let qm = eng.join("third_party/qm-dsp");
    let includes = [eng.join("src"), qm.clone(), qm.join("include"), qm.join("ext/kissfft"), eng.join("third_party/dr_libs")];
    let android = std::env::var("CARGO_CFG_TARGET_OS").map(|o| o == "android").unwrap_or(false);

    // kissfft is C
    let mut c = cc::Build::new();
    c.warnings(false).define("kiss_fft_scalar", "double").define("_USE_MATH_DEFINES", None);
    for i in &includes {
        c.include(i);
    }
    c.file(qm.join("ext/kissfft/kiss_fft.c")).file(qm.join("ext/kissfft/tools/kiss_fftr.c"));
    c.compile("picodj_kissfft");

    // the engine and qm-dsp are C++17; the list of sources is shared with engine/build-cli.sh
    let mut cpp = cc::Build::new();
    cpp.cpp(true).std("c++17").warnings(false).define("kiss_fft_scalar", "double").define("_USE_MATH_DEFINES", None);
    cpp.flag_if_supported("/utf-8").flag_if_supported("/EHsc");
    if android {
        // no separate libc++_shared.so to ship in the APK
        cpp.cpp_link_stdlib("c++_static");
    }
    for i in &includes {
        cpp.include(i);
    }
    let list = std::fs::read_to_string(eng.join("sources.txt")).expect("engine/sources.txt");
    for f in list.lines().map(str::trim).filter(|l| !l.is_empty()) {
        cpp.file(eng.join(f));
    }
    cpp.compile("picodj_engine");

    println!("cargo:rerun-if-changed=../engine/src");
    println!("cargo:rerun-if-changed=../engine/sources.txt");
    println!("cargo:rerun-if-changed=../engine/third_party");

    tauri_build::build()
}
