//! Logging commands for managing application logging settings

use crate::config::{SettingsManager, LoggingSettings};
use tauri::State;
use std::collections::HashMap;

const VALID_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

/// Validate log level string
fn validate_log_level(level: &str) -> Result<(), String> {
    if VALID_LOG_LEVELS.contains(&level) {
        Ok(())
    } else {
        Err(format!(
            "Invalid log level '{}'. Valid values: {}",
            level,
            VALID_LOG_LEVELS.join(", ")
        ))
    }
}

/// Validate module log levels
pub fn validate_module_levels(levels: &HashMap<String, String>) -> Result<(), String> {
    for level in levels.values() {
        // Validate log level value
        validate_log_level(level)?;

        // Note: Module name format validation is intentionally lenient.
        // tracing crate accepts various module formats, including:
        // - "crate::module::submodule" (standard Rust path)
        // - "ttsbard" (top-level crate name)
        // - "crate" (bare crate name for dependencies)
        // This allows maximum flexibility for per-module log filtering.
    }
    Ok(())
}

/// Get logging settings
#[tauri::command]
pub fn get_logging_settings(
    settings_manager: State<'_, SettingsManager>
) -> Result<LoggingSettings, String> {
    Ok(settings_manager.get_logging_settings())
}

/// Save logging settings
#[tauri::command]
pub fn save_logging_settings(
    enabled: bool,
    level: String,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    // Validate log level before updating (no clone needed - pass as &str)
    validate_log_level(&level)?;

    // Atomically update logging settings
    settings_manager.update_logging(|logging| {
        logging.enabled = enabled;
        logging.level = level;
    }).map_err(|e| e.to_string())
}
