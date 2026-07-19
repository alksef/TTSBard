fn main() {
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
