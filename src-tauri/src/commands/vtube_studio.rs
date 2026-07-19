use crate::config::{
    SettingsManager, VTubeStudioSettings, VTubeStudioSettingsDto, VTubeStudioTypingAction,
    VTubeStudioTypingMode, VtsHotkeyInfoDto,
};
use crate::events::VTubeStudioConnectionStatus;
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{debug, info};

pub const VTS_STATUS_CHANGED_EVENT: &str = "vtube-studio-status-changed";

fn emit_vts_status(app_handle: &AppHandle, status: &VTubeStudioConnectionStatus) {
    let _ = app_handle.emit(VTS_STATUS_CHANGED_EVENT, status);
}

#[tauri::command]
pub async fn get_vtube_studio_settings(
    state: State<'_, AppState>,
) -> Result<VTubeStudioSettingsDto, String> {
    let settings = state.vtube_studio.settings.read().await;
    Ok(VTubeStudioSettingsDto::from(settings.clone()))
}

#[tauri::command]
pub async fn save_vtube_studio_settings(
    enabled: bool,
    port: u16,
    start_on_boot: bool,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    if port < 1024 {
        return Err(format!("Invalid port: {}. Must be 1024-65535.", port));
    }

    info!(enabled, port, start_on_boot, "Saving VTube Studio settings");

    let old_port;
    {
        let current = state.vtube_studio.settings.read().await;
        old_port = current.port;
    }

    let endpoint_changed = old_port != port;

    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;

    let (token, typing_action) = {
        let s = state.vtube_studio.settings.read().await;
        (s.token.clone(), s.typing_action.clone())
    };

    let persist_settings = VTubeStudioSettings {
        enabled,
        port,
        token: token.clone(),
        start_on_boot,
        typing_action,
    };

    let mgr = settings_manager.inner().clone();
    crate::commands::persist_blocking(&mgr, move |mgr| {
        mgr.set_vtube_studio_settings(&persist_settings)
    })
    .await?;

    {
        let mut s = state.vtube_studio.settings.write().await;
        s.enabled = enabled;
        s.port = port;
        s.start_on_boot = start_on_boot;
    }

    crate::commands::emit_settings_changed(&app_handle);

    if endpoint_changed {
        state.vtube_studio.disconnect().await;
        let status = state.vtube_studio.get_connection_status();
        emit_vts_status(&app_handle, &status);
        info!("VTube Studio connection cleared due to port change");
    }

    Ok("Настройки подключения VTube Studio сохранены".to_string())
}

#[tauri::command]
pub async fn save_vtube_studio_typing_action(
    output_mode: String,
    parameter_name: String,
    start_hotkey_id: String,
    stop_hotkey_id: String,
    start_hotkey_name: String,
    stop_hotkey_name: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let mode = match output_mode.as_str() {
        "Event" => VTubeStudioTypingMode::Event,
        "Hotkeys" => VTubeStudioTypingMode::Hotkeys,
        other => {
            return Err(format!(
                "Invalid output mode: '{}'. Must be 'Event' or 'Hotkeys'.",
                other
            ));
        }
    };

    let trimmed_parameter_name = parameter_name.trim().to_string();
    let trimmed_start = start_hotkey_id.trim().to_string();
    let trimmed_stop = stop_hotkey_id.trim().to_string();

    match mode {
        VTubeStudioTypingMode::Event => {
            if trimmed_parameter_name.is_empty() {
                return Err("Parameter name must be non-empty.".to_string());
            }
        }
        VTubeStudioTypingMode::Hotkeys => {
            if trimmed_start.is_empty() {
                return Err("Start hotkey ID must be non-empty for Hotkeys mode.".to_string());
            }
            if trimmed_stop.is_empty() {
                return Err("Stop hotkey ID must be non-empty for Hotkeys mode.".to_string());
            }
        }
    }

    let typing_action = VTubeStudioTypingAction {
        output_mode: mode,
        parameter_name: trimmed_parameter_name,
        start_hotkey_id: trimmed_start,
        stop_hotkey_id: trimmed_stop,
        start_hotkey_name: start_hotkey_name.trim().to_string(),
        stop_hotkey_name: stop_hotkey_name.trim().to_string(),
    };

    info!(?typing_action, "Saving VTube Studio typing action");

    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;

    let (enabled, port, token, start_on_boot) = {
        let s = state.vtube_studio.settings.read().await;
        (s.enabled, s.port, s.token.clone(), s.start_on_boot)
    };

    let persist_settings = VTubeStudioSettings {
        enabled,
        port,
        token,
        start_on_boot,
        typing_action: typing_action.clone(),
    };

    let mgr = settings_manager.inner().clone();
    crate::commands::persist_blocking(&mgr, move |mgr| {
        mgr.set_vtube_studio_settings(&persist_settings)
    })
    .await?;

    {
        let mut s = state.vtube_studio.settings.write().await;
        s.typing_action = typing_action;
    }

    crate::commands::emit_settings_changed(&app_handle);

    Ok("Действие при наборе сохранено".to_string())
}

#[tauri::command]
pub async fn get_vtube_studio_current_model_hotkeys(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<Vec<VtsHotkeyInfoDto>, String> {
    info!("Requesting VTS current model hotkeys");

    let hotkeys = state
        .vtube_studio
        .get_current_model_hotkeys()
        .await
        .map_err(|e| {
            let status = state.vtube_studio.get_connection_status();
            emit_vts_status(&app_handle, &status);
            e
        })?;

    let dtos: Vec<VtsHotkeyInfoDto> = hotkeys
        .into_iter()
        .map(|h| VtsHotkeyInfoDto {
            hotkey_id: h.hotkey_id,
            name: h.name,
            hotkey_type: h.hotkey_type,
            description: h.description,
        })
        .collect();

    Ok(dtos)
}

#[tauri::command]
pub async fn test_vtube_studio_connection(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let (port, stored_token) = {
        let settings = state.vtube_studio.settings.read().await;
        (settings.port, settings.token.clone())
    };

    info!(
        port,
        has_token = stored_token.is_some(),
        "Testing VTube Studio connection"
    );

    let result = state
        .vtube_studio
        .test_connection(port, stored_token.as_deref())
        .await;

    match result {
        Ok(new_token) => {
            state.vtube_studio.mark_authenticated(true);
            if let Some(ref tok) = new_token {
                info!("Persisting new VTS authentication token");
                let mut s = state.vtube_studio.settings.write().await;
                s.token = Some(tok.clone());
                drop(s);

                let settings_manager = app_handle
                    .try_state::<SettingsManager>()
                    .ok_or_else(|| "SettingsManager not available".to_string())?;
                let mgr = settings_manager.inner().clone();
                let tok_clone = tok.clone();
                crate::commands::persist_blocking(&mgr, move |m| {
                    let mut vts = m.get_vtube_studio_settings();
                    vts.token = Some(tok_clone);
                    m.set_vtube_studio_settings(&vts)
                })
                .await?;
            }

            Ok("Successfully connected to VTube Studio.".to_string())
        }
        Err(e) => {
            state.vtube_studio.mark_authenticated(false);
            Err(format!("VTube Studio connection failed: {}", e))
        }
    }
}

#[tauri::command]
pub async fn connect_vtube_studio(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let (port, stored_token) = {
        let settings = state.vtube_studio.settings.read().await;
        (settings.port, settings.token.clone())
    };

    info!(
        port,
        has_token = stored_token.is_some(),
        "Connect VTube Studio"
    );

    let result = state
        .vtube_studio
        .connect(port, stored_token.as_deref())
        .await;

    let status = state.vtube_studio.get_connection_status();
    emit_vts_status(&app_handle, &status);

    match result {
        Ok(new_token) => {
            if let Some(ref tok) = new_token {
                info!("Persisting new VTS authentication token");
                let mut s = state.vtube_studio.settings.write().await;
                s.token = Some(tok.clone());
                drop(s);

                let settings_manager = app_handle
                    .try_state::<SettingsManager>()
                    .ok_or_else(|| "SettingsManager not available".to_string())?;
                let mgr = settings_manager.inner().clone();
                let tok_clone = tok.clone();
                crate::commands::persist_blocking(&mgr, move |m| {
                    let mut vts = m.get_vtube_studio_settings();
                    vts.token = Some(tok_clone);
                    m.set_vtube_studio_settings(&vts)
                })
                .await?;
            }
            Ok("Подключено к VTube Studio".to_string())
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn disconnect_vtube_studio(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    info!("Disconnect VTube Studio");
    state.vtube_studio.disconnect().await;
    let status = state.vtube_studio.get_connection_status();
    emit_vts_status(&app_handle, &status);
    Ok("Disconnected from VTube Studio".to_string())
}

#[tauri::command]
pub async fn restart_vtube_studio(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    info!("Restart VTube Studio");

    state.vtube_studio.disconnect().await;

    let (port, stored_token) = {
        let settings = state.vtube_studio.settings.read().await;
        (settings.port, settings.token.clone())
    };

    let result = state
        .vtube_studio
        .connect(port, stored_token.as_deref())
        .await;

    let status = state.vtube_studio.get_connection_status();
    emit_vts_status(&app_handle, &status);

    match result {
        Ok(new_token) => {
            if let Some(ref tok) = new_token {
                info!("Persisting new VTS authentication token");
                let mut s = state.vtube_studio.settings.write().await;
                s.token = Some(tok.clone());
                drop(s);

                let settings_manager = app_handle
                    .try_state::<SettingsManager>()
                    .ok_or_else(|| "SettingsManager not available".to_string())?;
                let mgr = settings_manager.inner().clone();
                let tok_clone = tok.clone();
                crate::commands::persist_blocking(&mgr, move |m| {
                    let mut vts = m.get_vtube_studio_settings();
                    vts.token = Some(tok_clone);
                    m.set_vtube_studio_settings(&vts)
                })
                .await?;
            }
            Ok("Restarted VTube Studio".to_string())
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn get_vtube_studio_status(
    state: State<'_, AppState>,
) -> Result<VTubeStudioConnectionStatus, String> {
    Ok(state.vtube_studio.get_connection_status())
}

#[tauri::command]
pub async fn test_vtube_studio_typing(
    timeout_ms: u64,
    repeat_count: u64,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    if timeout_ms < 100 || timeout_ms > 5000 {
        return Err("Таймаут должен быть от 100 до 5000 мс".to_string());
    }
    if repeat_count < 1 || repeat_count > 10 {
        return Err("Повторы должны быть от 1 до 10".to_string());
    }

    info!(timeout_ms, repeat_count, "Testing VTube Studio typing");

    let result = state
        .vtube_studio
        .test_typing_action(timeout_ms, repeat_count)
        .await;

    match result {
        Ok(msg) => Ok(msg),
        Err(e) => {
            let status = state.vtube_studio.get_connection_status();
            emit_vts_status(&app_handle, &status);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn set_vtube_studio_typing(
    typing: bool,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (port, token) = {
        let settings = state.vtube_studio.settings.read().await;
        (settings.port, settings.token.clone())
    };

    let stored_token = match token.as_deref() {
        None | Some("") => {
            debug!("VTS: set_typing({}) called but no token — no-op", typing);
            return Ok(());
        }
        Some(t) => t,
    };

    if typing && !state.vtube_studio.is_desired_running() {
        debug!("VTS: set_typing(true) ignored — desired_running is false");
        return Ok(());
    }

    let status_before = state.vtube_studio.get_connection_status();

    debug!(typing, "VTS: set_vtube_studio_typing");
    let result = state
        .vtube_studio
        .set_typing(typing, port, stored_token)
        .await;

    let status_after = state.vtube_studio.get_connection_status();
    if status_before != status_after {
        emit_vts_status(&app_handle, &status_after);
    }

    result
}
