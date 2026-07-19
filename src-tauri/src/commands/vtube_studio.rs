use crate::config::{SettingsManager, VTubeStudioSettings, VTubeStudioSettingsDto};
use crate::state::AppState;
use tauri::{AppHandle, Manager, State};
use tracing::{debug, info};

#[tauri::command]
pub async fn get_vtube_studio_settings(
    state: State<'_, AppState>,
) -> Result<VTubeStudioSettingsDto, String> {
    let settings = state.vtube_studio.settings.read().await;
    Ok(VTubeStudioSettingsDto {
        enabled: settings.enabled,
        port: settings.port,
    })
}

#[tauri::command]
pub async fn save_vtube_studio_settings(
    enabled: bool,
    port: u16,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    if port < 1024 {
        return Err(format!("Invalid port: {}. Must be 1024-65535.", port));
    }

    info!(enabled, port, "Saving VTube Studio settings");

    let old_enabled;
    let old_port;
    {
        let current = state.vtube_studio.settings.read().await;
        old_enabled = current.enabled;
        old_port = current.port;
    }

    let endpoint_changed = old_port != port || old_enabled != enabled;

    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;

    let token = {
        let s = state.vtube_studio.settings.read().await;
        s.token.clone()
    };

    let persist_settings = VTubeStudioSettings {
        enabled,
        port,
        token: token.clone(),
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
    }

    crate::commands::emit_settings_changed(&app_handle);

    if endpoint_changed {
        state.vtube_studio.disconnect().await;
        info!("VTube Studio connection cleared due to settings change");
    }

    Ok("VTube Studio settings saved".to_string())
}

#[tauri::command]
pub async fn test_vtube_studio_connection(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let (enabled, port, stored_token) = {
        let settings = state.vtube_studio.settings.read().await;
        (settings.enabled, settings.port, settings.token.clone())
    };

    if !enabled {
        return Err(
            "VTube Studio integration is disabled. Enable it in settings first.".to_string(),
        );
    }

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

            Ok(
                "Successfully connected to VTube Studio. A brief test signal was sent to TTSBardTyping; bind this parameter to a model parameter/expression in VTube Studio for visible avatar movement."
                    .to_string(),
            )
        }
        Err(e) => {
            state.vtube_studio.mark_authenticated(false);
            Err(format!("VTube Studio connection failed: {}", e))
        }
    }
}

#[tauri::command]
pub async fn set_vtube_studio_typing(
    typing: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (enabled, port, token) = {
        let settings = state.vtube_studio.settings.read().await;
        (settings.enabled, settings.port, settings.token.clone())
    };

    if !enabled {
        debug!("VTS: set_typing({}) called but disabled — no-op", typing);
        return Ok(());
    }

    let stored_token = match token.as_deref() {
        None | Some("") => {
            debug!("VTS: set_typing({}) called but no token — no-op", typing);
            return Ok(());
        }
        Some(t) => t,
    };

    debug!(typing, "VTS: set_vtube_studio_typing");
    state
        .vtube_studio
        .set_typing(typing, port, stored_token)
        .await?;

    Ok(())
}
