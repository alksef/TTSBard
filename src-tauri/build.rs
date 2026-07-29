fn main() {
    configure_windows_common_controls_manifest();

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

    // The manifest above applies to every Rust target, including the unit-test
    // harness. Disable Tauri's otherwise equivalent bin-only manifest to avoid
    // duplicate RT_MANIFEST resources when Cargo also builds the app binary.
    let attributes = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    tauri_build::try_build(attributes).expect("failed to run Tauri build script");
}

fn configure_windows_common_controls_manifest() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // Rust test harnesses do not inherit Tauri's application manifest. Without
    // this dependency Windows loads comctl32 v5, which does not export
    // TaskDialogIndirect, and the harness exits before running the first test.
    let manifest_path = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").expect("OUT_DIR is set for build scripts"),
    )
    .join("common-controls-v6.manifest");
    std::fs::write(
        &manifest_path,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
    )
    .expect("failed to write Windows Common Controls manifest");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
        manifest_path.display()
    );
}
