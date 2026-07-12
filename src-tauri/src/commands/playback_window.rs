//! Playback Control Window Tauri Commands
//!
//! Tauri commands for playback window appearance settings (opacity, bg_color).

use crate::commands::window::resolve_main_appearance;
use crate::config::{is_valid_hex_color, SettingsManager, WindowsManager};
use crate::playback_window::update_playback_appearance;
use tauri::{AppHandle, State};
use tracing::info;

/// Get playback window appearance (opacity, bg_color)
///
/// If `appearance_source == "main"`, the appearance is inherited from the main
/// window settings (resolving the active theme when no custom color is set).
#[tauri::command]
pub fn pc_get_appearance(
    windows_manager: State<'_, WindowsManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(u8, String), String> {
    if windows_manager.get_playback_appearance_source() == "main" {
        return Ok(resolve_main_appearance(&windows_manager, &settings_manager));
    }
    let opacity = windows_manager.get_playback_opacity();
    let color = windows_manager.get_playback_bg_color();
    Ok((opacity, color))
}

/// Set playback window opacity
#[tauri::command]
pub fn pc_set_opacity(
    value: u8,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(value, "Setting playback opacity");
    windows_manager
        .set_playback_opacity(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    let _ = update_playback_appearance(&app_handle);
    Ok(())
}

/// Set playback window background color
#[tauri::command]
pub fn pc_set_bg_color(
    color: String,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    if !is_valid_hex_color(&color) {
        return Err("Invalid color format. Use #RRGGBB".to_string());
    }
    info!(color, "Setting playback bg color");
    windows_manager
        .set_playback_bg_color(color)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    let _ = update_playback_appearance(&app_handle);
    Ok(())
}
