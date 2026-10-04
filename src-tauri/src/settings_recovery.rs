//! Minimal recovery application for a failed settings load (ROADMAP-123).
//!
//! When `SettingsManager::new` fails, the ordinary startup must not continue
//! and the broken file must not be replaced silently. Instead this module
//! boots a reduced Tauri application that owns nothing but the recovery
//! dialog window and a handful of commands. No services, workers, windows, or
//! settings state from the normal startup are created; the user either fixes
//! the original file or explicitly restores defaults, then starts the
//! application again by hand.

use anyhow::Context;
use tauri::Manager;
use tracing::error;

use crate::commands::localization::LocalizationState;
use crate::commands::settings_recovery::RecoveryState;
use crate::config::{normalize_detected_locale, SettingsFailureStage, SettingsLoadFailure};

/// Tauri window label of the recovery dialog (declared in `tauri.conf.json`
/// with `"create": false` and built only by this module).
const RECOVERY_WINDOW_LABEL: &str = "settings-recovery";

/// Run the minimal recovery application for a failed settings load and exit
/// the process when its window closes. Never returns.
pub fn run_recovery_app(error: anyhow::Error) -> ! {
    let failure = error
        .downcast_ref::<SettingsLoadFailure>()
        .cloned()
        .unwrap_or_else(|| {
            // The failure happened before the settings path was known (for
            // example, the config directory itself is unavailable).
            SettingsLoadFailure::unresolved(SettingsFailureStage::Read, error.to_string())
        });

    // The tracing subscriber is not initialized yet at this point (it is
    // configured from the settings we failed to load), so report on stderr.
    error!("Settings recovery dialog started: {}", failure);
    eprintln!("TTSBard: {}", failure);

    let requested_locale =
        normalize_detected_locale(failure.detected_locale.as_deref()).to_string();

    let build = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(RecoveryState::new(failure))
        .invoke_handler(tauri::generate_handler![
            crate::commands::localization::get_localization,
            crate::commands::settings_recovery::settings_recovery_get_diagnostics,
            crate::commands::settings_recovery::settings_recovery_open_settings_file,
            crate::commands::settings_recovery::settings_recovery_restore_defaults,
            crate::commands::settings_recovery::settings_recovery_quit,
        ])
        .setup(move |app| {
            // Localization never depends on settings.json: the catalog comes
            // from bundled resources and config locale packs, and the fallback
            // language is built into the frontend. The detected locale only
            // picks the snapshot language for the dialog.
            let resource_path = app
                .path()
                .resource_dir()
                .ok()
                .map(|dir| dir.join("locales"));
            let config_locales = crate::paths::config_root()
                .ok()
                .map(|dir| dir.join("locales"));
            let catalog = crate::localization::LocaleCatalog::load(
                resource_path.as_deref(),
                config_locales.as_deref(),
            );
            app.manage(LocalizationState::new(catalog, &requested_locale));

            create_recovery_window(app)?;
            Ok(())
        })
        .build(tauri::generate_context!());

    match build {
        Ok(app) => {
            app.run(|_app_handle, _event| {
                // Deliberately minimal: closing the last window (dialog close,
                // Escape, or quit) exits the process without touching the
                // original settings file. There is no coordinated shutdown in
                // recovery mode because no services were started.
            });
        }
        Err(build_error) => {
            error!(error = %build_error, "Failed to run the settings recovery dialog");
            eprintln!(
                "TTSBard: failed to run the settings recovery dialog: {}",
                build_error
            );
            std::process::exit(1);
        }
    }

    std::process::exit(0);
}

/// Build only the recovery dialog window; the windows of the normal startup
/// stay uncreated because their config entries carry `"create": false`.
fn create_recovery_window(app: &tauri::App) -> anyhow::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == RECOVERY_WINDOW_LABEL)
        .cloned()
        .with_context(|| format!("{RECOVERY_WINDOW_LABEL} window config is missing"))?;

    tauri::WebviewWindowBuilder::from_config(app.handle(), &config)
        .with_context(|| format!("failed to prepare the {RECOVERY_WINDOW_LABEL} window"))?
        .build()
        .with_context(|| format!("failed to create the {RECOVERY_WINDOW_LABEL} window"))?;
    Ok(())
}
