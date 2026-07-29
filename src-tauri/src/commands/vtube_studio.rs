use crate::config::{
    SettingsManager, VTubeStudioSettings, VTubeStudioSettingsDto, VTubeStudioTypingAction,
    VTubeStudioTypingMode, VtsHotkeyInfoDto,
};
use crate::events::VTubeStudioConnectionStatus;
use crate::state::AppState;
use crate::vtube_studio::{SceneItemRecord, VTubeStudioItemStatus};
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{debug, info};

pub const VTS_STATUS_CHANGED_EVENT: &str = "vtube-studio-status-changed";
pub const VTS_ITEM_STATUS_CHANGED_EVENT: &str = "vtube-studio-item-status-changed";

fn emit_vts_status(app_handle: &AppHandle, status: &VTubeStudioConnectionStatus) {
    let _ = app_handle.emit(VTS_STATUS_CHANGED_EVENT, status);
}

fn emit_vts_item_status(app_handle: &AppHandle, status: &VTubeStudioItemStatus) {
    let _ = app_handle.emit(VTS_ITEM_STATUS_CHANGED_EVENT, status);
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
        let item_status = state.vtube_studio.get_item_status();
        emit_vts_item_status(&app_handle, &item_status);
        info!("VTube Studio connection cleared due to port change");
    }

    Ok("Настройки подключения VTube Studio сохранены".to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri IPC exposes these fields as named arguments.
pub async fn save_vtube_studio_typing_action(
    output_mode: String,
    parameter_name: String,
    start_hotkey_id: String,
    stop_hotkey_id: String,
    start_hotkey_name: String,
    stop_hotkey_name: String,
    item_file_name: Option<String>,
    item_type: Option<String>,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let mode = match output_mode.as_str() {
        "Event" => VTubeStudioTypingMode::Event,
        "Hotkeys" => VTubeStudioTypingMode::Hotkeys,
        "Item" => VTubeStudioTypingMode::Item,
        other => {
            return Err(format!(
                "Invalid output mode: '{}'. Must be 'Event', 'Hotkeys', or 'Item'.",
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
        VTubeStudioTypingMode::Item => {
            let file_name = match &item_file_name {
                Some(f) if !f.is_empty() => f.clone(),
                _ => {
                    return Err(
                        "itemFileName is required and must be non-empty for Item mode.".to_string(),
                    );
                }
            };
            let item_type_val = match &item_type {
                Some(t) if !t.is_empty() => t.clone(),
                _ => {
                    return Err(
                        "itemType is required and must be non-empty for Item mode.".to_string()
                    );
                }
            };

            let records = state.vtube_studio.list_scene_items().await.map_err(|e| {
                let status = state.vtube_studio.get_connection_status();
                emit_vts_status(&app_handle, &status);
                format!(
                    "Cannot validate item selection: {}. Make sure VTube Studio is connected.",
                    e
                )
            })?;

            validate_item_selection(&records, &file_name, &item_type_val)?;
        }
    }

    let (existing_file_name, existing_item_type, enabled, port, token, start_on_boot) = {
        let s = state.vtube_studio.settings.read().await;
        (
            s.typing_action.item_file_name.clone(),
            s.typing_action.item_type.clone(),
            s.enabled,
            s.port,
            s.token.clone(),
            s.start_on_boot,
        )
    };

    let (resolved_file_name, resolved_item_type) = resolve_item_metadata(
        &mode,
        &existing_file_name,
        &existing_item_type,
        item_file_name,
        item_type,
    );

    let typing_action = VTubeStudioTypingAction {
        output_mode: mode,
        parameter_name: trimmed_parameter_name,
        start_hotkey_id: trimmed_start,
        stop_hotkey_id: trimmed_stop,
        start_hotkey_name: start_hotkey_name.trim().to_string(),
        stop_hotkey_name: stop_hotkey_name.trim().to_string(),
        item_file_name: resolved_file_name,
        item_type: resolved_item_type,
    };

    info!(?typing_action, "Saving VTube Studio typing action");

    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;

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

    let is_item_mode = typing_action.output_mode == VTubeStudioTypingMode::Item;

    {
        let mut s = state.vtube_studio.settings.write().await;
        s.typing_action = typing_action;
    }

    crate::commands::emit_settings_changed(&app_handle);

    let item_status = state.vtube_studio.refresh_item_action().await;
    emit_vts_item_status(&app_handle, &item_status);

    if is_item_mode {
        if matches!(item_status, VTubeStudioItemStatus::Ready { .. }) {
            Ok("Действие при наборе сохранено".to_string())
        } else {
            Err(format!(
                "Item action saved but item is not ready: {:?}. Check that the item exists in the scene.",
                item_status
            ))
        }
    } else {
        Ok("Действие при наборе сохранено".to_string())
    }
}

#[tauri::command]
pub async fn get_vtube_studio_scene_items(
    state: State<'_, AppState>,
) -> Result<Vec<SceneItemRecord>, String> {
    state.vtube_studio.list_scene_items().await
}

#[tauri::command]
pub async fn get_vtube_studio_item_status(
    state: State<'_, AppState>,
) -> Result<VTubeStudioItemStatus, String> {
    Ok(state.vtube_studio.get_item_status())
}

#[tauri::command]
pub async fn refresh_vtube_studio_item(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<VTubeStudioItemStatus, String> {
    let status = state.vtube_studio.refresh_item_action().await;
    emit_vts_item_status(&app_handle, &status);
    info!(?status, "VTube Studio item action refreshed");
    Ok(status)
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
        .inspect_err(|_| {
            let status = state.vtube_studio.get_connection_status();
            emit_vts_status(&app_handle, &status);
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

            let item_status = state.vtube_studio.get_item_status();
            emit_vts_item_status(&app_handle, &item_status);

            Ok("Successfully connected to VTube Studio.".to_string())
        }
        Err(e) => {
            state.vtube_studio.mark_authenticated(false);
            let item_status = state.vtube_studio.get_item_status();
            emit_vts_item_status(&app_handle, &item_status);
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
    let item_status = state.vtube_studio.get_item_status();
    emit_vts_item_status(&app_handle, &item_status);

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
    let item_status = state.vtube_studio.get_item_status();
    emit_vts_item_status(&app_handle, &item_status);
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
    let item_status = state.vtube_studio.get_item_status();
    emit_vts_item_status(&app_handle, &item_status);

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
    if !(100..=5000).contains(&timeout_ms) {
        return Err("Таймаут должен быть от 100 до 5000 мс".to_string());
    }
    if !(1..=10).contains(&repeat_count) {
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

    let mode = state
        .vtube_studio
        .settings
        .read()
        .await
        .typing_action
        .output_mode
        .clone();

    if mode == VTubeStudioTypingMode::Item {
        if typing && !state.vtube_studio.is_desired_running() {
            debug!("VTS: Item set_typing(true) ignored — desired_running is false");
            return Ok(());
        }
        state.vtube_studio.record_item_desired(typing);
        let svc = state.vtube_studio.clone();
        let app_clone = app_handle.clone();
        tokio::spawn(async move {
            let status = svc.run_item_sync().await;
            emit_vts_item_status(&app_clone, &status);
        });
        return Ok(());
    }

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

pub fn validate_item_selection(
    records: &[SceneItemRecord],
    file_name: &str,
    item_type: &str,
) -> Result<(), String> {
    if file_name.is_empty() {
        return Err("Item file name must be non-empty.".to_string());
    }
    if item_type.is_empty() {
        return Err("Item type must be non-empty.".to_string());
    }

    let matching: Vec<&SceneItemRecord> = records
        .iter()
        .filter(|r| r.file_name == file_name)
        .collect();

    if matching.is_empty() {
        return Err(format!(
            "No scene item with file name '{}' found. Add the item to the scene and try again.",
            file_name
        ));
    }

    if matching.len() > 1 {
        return Err(format!(
            "Multiple scene item types found for file name '{}' ({} types). Remove duplicates from the scene to use a specific item.",
            file_name,
            matching.len()
        ));
    }

    let record = matching[0];

    if record.duplicate_count > 1 {
        return Err(format!(
            "Multiple instances ({}) of item '{}' (type '{}') in scene. Remove duplicate instances to use this item.",
            record.duplicate_count, file_name, record.item_type
        ));
    }

    if !record.supported {
        return Err(format!(
            "Item '{}' has unsupported type '{}'. Use a PNG, JPG, GIF, or AnimationFolder item instead.",
            file_name, record.item_type
        ));
    }

    if record.item_type != item_type {
        return Err(format!(
            "Type mismatch: item '{}' has type '{}' but '{}' was selected. Refresh the item list and try again.",
            file_name, record.item_type, item_type
        ));
    }

    Ok(())
}

fn resolve_item_metadata(
    mode: &VTubeStudioTypingMode,
    existing_file_name: &str,
    existing_item_type: &str,
    supplied_file_name: Option<String>,
    supplied_item_type: Option<String>,
) -> (String, String) {
    match mode {
        VTubeStudioTypingMode::Event | VTubeStudioTypingMode::Hotkeys => {
            let file_name = supplied_file_name.unwrap_or_else(|| existing_file_name.to_string());
            let item_type = supplied_item_type.unwrap_or_else(|| existing_item_type.to_string());
            (file_name, item_type)
        }
        VTubeStudioTypingMode::Item => (
            supplied_file_name.unwrap_or_default(),
            supplied_item_type.unwrap_or_default(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(
        file_name: &str,
        item_type: &str,
        supported: bool,
        duplicate_count: u32,
    ) -> SceneItemRecord {
        SceneItemRecord {
            file_name: file_name.to_string(),
            item_type: item_type.to_string(),
            supported,
            duplicate_count,
        }
    }

    #[test]
    fn exact_case_sensitive_one_supported_succeeds() {
        let records = vec![make_record("icon.png", "PNG", true, 1)];
        let result = validate_item_selection(&records, "icon.png", "PNG");
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn wrong_case_fails() {
        let records = vec![make_record("icon.png", "PNG", true, 1)];
        let result = validate_item_selection(&records, "ICON.PNG", "PNG");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ICON.PNG"));
    }

    #[test]
    fn missing_filename_fails() {
        let records = vec![make_record("icon.png", "PNG", true, 1)];
        let result = validate_item_selection(&records, "ghost.png", "PNG");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("ghost.png"),
            "expected filename in error: {}",
            err
        );
        assert!(
            err.contains("Add the item to the scene"),
            "expected actionable message: {}",
            err
        );
        assert!(
            !err.contains("instanceID"),
            "error must not contain instanceID: {}",
            err
        );
        assert!(
            !err.contains("token"),
            "error must not contain token: {}",
            err
        );
    }

    #[test]
    fn duplicate_count_gt_1_fails() {
        let records = vec![make_record("dup.png", "PNG", true, 3)];
        let result = validate_item_selection(&records, "dup.png", "PNG");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("3"),
            "expected duplicate count in error: {}",
            err
        );
        assert!(
            err.contains("Remove duplicate instances"),
            "expected actionable message: {}",
            err
        );
    }

    #[test]
    fn two_catalog_records_same_filename_different_type_fails_ambiguous() {
        let records = vec![
            make_record("logo.png", "PNG", true, 1),
            make_record("logo.png", "GIF", true, 1),
        ];
        let result = validate_item_selection(&records, "logo.png", "PNG");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Multiple scene item types"),
            "expected ambiguous message: {}",
            err
        );
        assert!(
            err.contains("logo.png"),
            "expected filename in error: {}",
            err
        );
    }

    #[test]
    fn unsupported_fails() {
        let records = vec![make_record("model.moc3", "Live2D", false, 1)];
        let result = validate_item_selection(&records, "model.moc3", "Live2D");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("unsupported type"),
            "expected unsupported message: {}",
            err
        );
        assert!(err.contains("Live2D"), "expected type in error: {}", err);
    }

    #[test]
    fn type_mismatch_fails() {
        let records = vec![make_record("banner.jpg", "JPG", true, 1)];
        let result = validate_item_selection(&records, "banner.jpg", "PNG");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Type mismatch"),
            "expected type mismatch: {}",
            err
        );
        assert!(
            err.contains("JPG"),
            "expected actual type in error: {}",
            err
        );
    }

    #[test]
    fn unicode_filename_is_preserved() {
        let records = vec![make_record("кот 😺.png", "PNG", true, 1)];
        let result = validate_item_selection(&records, "кот 😺.png", "PNG");
        assert!(
            result.is_ok(),
            "expected Ok for unicode filename, got {:?}",
            result
        );
    }

    #[test]
    fn exact_whitespace_filename_is_preserved() {
        let records = vec![make_record("  my item  .png", "PNG", true, 1)];
        let result = validate_item_selection(&records, "  my item  .png", "PNG");
        assert!(
            result.is_ok(),
            "expected Ok for whitespace filename, got {:?}",
            result
        );
    }

    #[test]
    fn error_text_contains_next_action_and_no_instance_id_or_token() {
        let records = vec![make_record("icon.png", "PNG", false, 1)];
        let result = validate_item_selection(&records, "icon.png", "PNG");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            !err.contains("instanceID"),
            "error must not contain instanceID: {}",
            err
        );
        assert!(
            !err.contains("token"),
            "error must not contain token: {}",
            err
        );
        assert!(
            err.contains("unsupported type") || err.contains("Use a"),
            "error should suggest a next action: {}",
            err
        );
    }

    #[test]
    fn empty_file_name_fails() {
        let records = vec![make_record("pic.png", "PNG", true, 1)];
        let result = validate_item_selection(&records, "", "PNG");
        assert!(result.is_err());
    }

    #[test]
    fn empty_item_type_fails() {
        let records = vec![make_record("pic.png", "PNG", true, 1)];
        let result = validate_item_selection(&records, "pic.png", "");
        assert!(result.is_err());
    }

    #[test]
    fn event_missing_args_preserves_mixed_case_unicode() {
        let existing = ("МикСケース 😺.png".to_string(), "PNG".to_string());
        let result = resolve_item_metadata(
            &VTubeStudioTypingMode::Event,
            &existing.0,
            &existing.1,
            None,
            None,
        );
        assert_eq!(result, existing);
    }

    #[test]
    fn hotkeys_missing_args_preserves_metadata() {
        let existing = ("   spaced  .png".to_string(), "GIF".to_string());
        let result = resolve_item_metadata(
            &VTubeStudioTypingMode::Hotkeys,
            &existing.0,
            &existing.1,
            None,
            None,
        );
        assert_eq!(result, existing);
    }

    #[test]
    fn event_explicit_optional_replaces_exactly_including_whitespace() {
        let result = resolve_item_metadata(
            &VTubeStudioTypingMode::Event,
            "old.png",
            "PNG",
            Some("  new name  .jpg".to_string()),
            Some("JPG".to_string()),
        );
        assert_eq!(result.0, "  new name  .jpg");
        assert_eq!(result.1, "JPG");
    }

    #[test]
    fn hotkeys_explicit_optional_replaces_exactly() {
        let result = resolve_item_metadata(
            &VTubeStudioTypingMode::Hotkeys,
            "old.png",
            "PNG",
            Some("exact.gif".to_string()),
            Some("GIF".to_string()),
        );
        assert_eq!(result.0, "exact.gif");
        assert_eq!(result.1, "GIF");
    }

    #[test]
    fn item_supplied_validated_values_unchanged() {
        let result = resolve_item_metadata(
            &VTubeStudioTypingMode::Item,
            "old.png",
            "PNG",
            Some("アイテム.png".to_string()),
            Some("PNG".to_string()),
        );
        assert_eq!(result.0, "アイテム.png");
        assert_eq!(result.1, "PNG");
    }

    #[test]
    fn item_missing_args_falls_back_to_default() {
        let result =
            resolve_item_metadata(&VTubeStudioTypingMode::Item, "old.png", "PNG", None, None);
        assert_eq!(result.0, "");
        assert_eq!(result.1, "");
    }

    #[test]
    fn event_only_type_supplied_preserves_existing_filename() {
        let result = resolve_item_metadata(
            &VTubeStudioTypingMode::Event,
            "existing.png",
            "PNG",
            None,
            Some("GIF".to_string()),
        );
        assert_eq!(result.0, "existing.png");
        assert_eq!(result.1, "GIF");
    }

    #[test]
    fn hotkeys_only_filename_supplied_preserves_existing_type() {
        let result = resolve_item_metadata(
            &VTubeStudioTypingMode::Hotkeys,
            "old.gif",
            "GIF",
            Some("new.png".to_string()),
            None,
        );
        assert_eq!(result.0, "new.png");
        assert_eq!(result.1, "GIF");
    }
}
