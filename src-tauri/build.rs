fn main() {
    // Enable console in release builds on Windows
    #[cfg(target_os = "windows")]
    {
        if std::env::var("PROFILE").unwrap_or_default() == "release" {
            println!("cargo:rustc-link-arg=/SUBSYSTEM:CONSOLE");
        }
    }

    tauri_build::build()
}
