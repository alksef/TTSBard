fn main() {
    // espeak-rs-sys v0.2.0 unconditionally emits msvcrtd for Windows debug
    // builds, while its CMake-built Release library uses the shared CRT. That
    // creates a second debug heap and can trigger _CrtIsValidHeapPointer at
    // runtime. Keep the link on the shared CRT used by the native libraries.
    if cfg!(all(windows, debug_assertions)) {
        println!("cargo:rustc-link-arg=/NODEFAULTLIB:msvcrtd.lib");
    }

    // Compile the Signalsmith Stretch C++ bridge with MSVC-compatible settings.
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("src/signalsmith/bridge.cpp")
        .include("vendor/signalsmith-stretch")
        .include("vendor")
        .flag_if_supported("/std:c++14")
        .flag_if_supported("/EHsc")
        .opt_level(2);
    build.compile("signalsmith_bridge");

    tauri_build::build()
}
