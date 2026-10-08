use tauri::{AppHandle, Manager, State};
use tracing::info;

use crate::config::{SettingsManager, VrchatSettings, VrchatSettingsDto};
use crate::state::AppState;
use crate::vrchat::service::resolve_vrchat_destination;

/// Retrieves the current VRChat settings.
#[tauri::command]
pub async fn get_vrchat_settings(state: State<'_, AppState>) -> Result<VrchatSettingsDto, String> {
    let current = state.vrchat.settings.read().clone();
    Ok(current.into())
}

/// Validates and saves VRChat settings to storage and runtime.
#[tauri::command]
pub async fn save_vrchat_settings(
    settings: VrchatSettingsDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let host = settings.host.trim().to_string();
    let port = settings.port;

    let destination = resolve_vrchat_destination(&host, port)?;

    info!(
        enabled = settings.enabled,
        start_on_boot = settings.start_on_boot,
        host = %host,
        port,
        "Saving VRChat settings"
    );

    let new_settings = VrchatSettings {
        enabled: settings.enabled,
        start_on_boot: settings.start_on_boot,
        host,
        port,
    };

    // 1. Persist to storage file first
    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;

    let persist_settings = new_settings.clone();
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_vrchat_settings(&persist_settings)
    })
    .await?;

    // 2. Apply to runtime service (handles typing false if disabling while typing)
    state
        .vrchat
        .apply_settings_resolved(new_settings, destination)
        .await;

    // 3. Emit settings-changed event
    super::emit_settings_changed(&app_handle);

    Ok("VRChat settings saved".to_string())
}

/// Updates the VRChat chatbox typing indicator.
#[tauri::command]
pub async fn set_vrchat_typing(typing: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.vrchat.set_typing(typing).await
}

/// Sends text to the VRChat chatbox.
#[tauri::command]
pub async fn send_vrchat_text(text: String, state: State<'_, AppState>) -> Result<bool, String> {
    state.vrchat.send_text(&text).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dto_conversion() {
        let dto = VrchatSettingsDto {
            enabled: true,
            start_on_boot: true,
            host: "127.0.0.1".to_string(),
            port: 9000,
        };
        let model: VrchatSettings = dto.clone().into();
        assert!(model.enabled);
        assert!(model.start_on_boot);
        assert_eq!(model.host, "127.0.0.1");
        assert_eq!(model.port, 9000);

        let round_trip: VrchatSettingsDto = model.into();
        assert_eq!(dto, round_trip);
    }
}
