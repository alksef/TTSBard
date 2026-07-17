//! Sound Panel Tauri Commands
//!
//! Tauri команды для взаимодействия между frontend и backend.

use crate::commands::window::resolve_main_appearance;
use crate::config::{is_valid_hex_color, SettingsManager, WindowsManager};
use crate::events::AppEvent;
use crate::soundpanel::audio::play_audio_file;
use crate::soundpanel::intercept::InterceptSettings;
use crate::soundpanel::state::{SoundBinding, SoundPanelState, SoundSet, SoundSets};
use crate::soundpanel::storage::{copy_sound_file, delete_sound_file, save_sets};
use crate::soundpanel_window::emit_soundpanel_bindings_changed;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{debug, info};

/// Получить все привязки звуковой панели (активный набор)
#[tauri::command]
pub fn sp_get_bindings(state: State<'_, SoundPanelState>) -> Result<Vec<SoundBinding>, String> {
    debug!("Get bindings command");
    Ok(state.get_all_bindings())
}

/// Добавить новую привязку в активный набор
///
/// # Аргументы
/// * `key` - Клавиша (A-Z)
/// * `description` - Описание звука
/// * `file_path` - Путь к исходному аудиофайлу
#[tauri::command]
pub fn sp_add_binding(
    key: String,
    description: String,
    file_path: String,
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
) -> Result<SoundBinding, String> {
    info!(key, description, "Add binding");

    let key_char = key.to_uppercase().chars().next().ok_or("Key is empty")?;

    if !key_char.is_ascii_alphabetic() || !key_char.is_ascii_uppercase() {
        return Err("Key must be A-Z".to_string());
    }

    if let Some(existing) = state.get_binding(key_char) {
        return Err(format!(
            "Key {} is already bound to '{}'",
            key_char, existing.description
        ));
    }

    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let filename = copy_sound_file(&file_path, &appdata_path)?;

    let binding = SoundBinding {
        key: key_char,
        description,
        filename,
        original_path: Some(file_path),
    };

    state.add_binding(binding.clone());
    save_sets(&state)?;

    let _ = emit_soundpanel_bindings_changed(&app_handle);

    info!("Binding added successfully");
    Ok(binding)
}

/// Удалить привязку по клавише из активного набора
#[tauri::command]
pub fn sp_remove_binding(
    key: String,
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    info!(key, "Remove binding");

    let key_char = key.chars().next().ok_or("Key is empty")?;

    if let Some(binding) = state.get_binding(key_char) {
        let appdata_path = state.appdata_path.lock().unwrap().clone();
        let _ = delete_sound_file(&binding.filename, &appdata_path);
    }

    state.remove_binding(key_char);
    save_sets(&state)?;

    let _ = emit_soundpanel_bindings_changed(&app_handle);

    info!("Binding removed successfully");
    Ok(())
}

/// Тестировать воспроизведение звука
///
/// Воспроизводит указанный файл без создания привязки
#[tauri::command]
pub fn sp_test_sound(file_path: String) -> Result<(), String> {
    info!(file_path, "Test sound");

    if !std::path::Path::new(&file_path).exists() {
        return Err("File not found".to_string());
    }

    play_audio_file(&file_path);
    Ok(())
}

/// Проверить, поддерживается ли формат файла
#[tauri::command]
pub fn sp_is_supported_format(filename: String) -> Result<bool, String> {
    Ok(crate::soundpanel::audio::is_supported_audio_format(
        &filename,
    ))
}

/// Получить настройки внешнего вида floating окна звуковой панели
///
/// Если `appearance_source == "main"`, оформление наследуется от главного окна
/// (с учётом активной темы, когда собственный цвет главного окна выключен).
#[tauri::command]
pub fn sp_get_floating_appearance(
    state: State<'_, SoundPanelState>,
    windows_manager: State<'_, WindowsManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(u8, String), String> {
    if windows_manager.get_soundpanel_appearance_source() == "main" {
        return Ok(resolve_main_appearance(&windows_manager, &settings_manager));
    }
    let opacity = state.get_floating_opacity();
    let color = state.get_floating_bg_color();
    Ok((opacity, color))
}

/// Установить прозрачность floating окна звуковой панели
#[tauri::command]
pub fn sp_set_floating_opacity(
    value: u8,
    state: State<'_, SoundPanelState>,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(value, "Setting opacity");
    state.set_floating_opacity(value);
    windows_manager
        .set_soundpanel_opacity(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    Ok(())
}

/// Установить цвет фона floating окна звуковой панели
#[tauri::command]
pub fn sp_set_floating_bg_color(
    color: String,
    state: State<'_, SoundPanelState>,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    if !is_valid_hex_color(&color) {
        return Err("Invalid color format. Use #RRGGBB".to_string());
    }
    info!(color, "Setting bg color");
    state.set_floating_bg_color(color.clone());
    windows_manager
        .set_soundpanel_bg_color(color)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    Ok(())
}

/// Установить clickthrough для floating окна звуковой панели
#[tauri::command]
pub fn sp_set_floating_clickthrough(
    enabled: bool,
    state: State<'_, SoundPanelState>,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(enabled, "Setting clickthrough");
    state.set_floating_clickthrough(enabled);
    windows_manager
        .set_soundpanel_clickthrough(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    Ok(())
}

/// Проверить, включен ли clickthrough для floating окна звуковой панели
#[tauri::command]
pub fn sp_is_floating_clickthrough_enabled(
    state: State<'_, SoundPanelState>,
) -> Result<bool, String> {
    Ok(state.is_floating_clickthrough_enabled())
}

/// Проверить, оставлять ли окно видимым после воспроизведения звука
#[tauri::command]
pub fn sp_get_stay_visible(state: State<'_, SoundPanelState>) -> Result<bool, String> {
    Ok(state.get_stay_visible())
}

/// Установить, оставлять ли окно видимым после воспроизведения звука
#[tauri::command]
pub fn sp_set_stay_visible(
    enabled: bool,
    state: State<'_, SoundPanelState>,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(enabled, "Setting stay_visible");
    state.set_stay_visible(enabled);
    windows_manager
        .set_soundpanel_stay_visible(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    Ok(())
}

/// Установить, скрывать ли панель при потере фокуса
#[tauri::command]
pub fn sp_set_hide_on_blur(
    enabled: bool,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(enabled, "Setting hide_on_blur");
    windows_manager
        .set_soundpanel_hide_on_blur(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    Ok(())
}

/// Воспроизвести звук по клавише (A-Z) и скрыть панель (если stay_visible выключен)
#[tauri::command]
pub fn sp_play_binding(key: String, app_handle: AppHandle) -> Result<(), String> {
    let key_char = key.chars().next().ok_or("Key is empty")?;
    if !key_char.is_ascii_uppercase() {
        return Err("Key must be A-Z".to_string());
    }
    let state = app_handle.state::<SoundPanelState>();
    if let Some(binding) = state.get_binding(key_char) {
        info!(key = %key_char, description = binding.description, "Playing binding");
        state.play_sound(&binding);
        if !state.get_stay_visible() {
            state.emit_event(AppEvent::HideSoundPanelWindow);
        }
        Ok(())
    } else {
        Err(format!("No binding for key {}", key_char))
    }
}

/// Получить настройки перехвата
#[tauri::command]
pub fn get_intercept_settings(state: State<'_, SoundPanelState>) -> InterceptSettings {
    state.get_intercept()
}

/// Включить/выключить перехват
#[tauri::command]
pub fn set_intercept_enabled(
    enabled: bool,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    state.set_intercept_enabled(enabled);
    Ok(())
}

/// Установить биндинг перехвата
#[tauri::command]
pub fn set_intercept_binding(
    key: String,
    action: String,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    state.set_intercept_binding(key, action);
    Ok(())
}

/// Очистить биндинг перехвата
#[tauri::command]
pub fn clear_intercept_binding(
    key: String,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    state.clear_intercept_binding(key);
    Ok(())
}

// ---- Set management commands ----

/// Получить все наборы звуков
#[tauri::command]
pub fn sp_get_sets(state: State<'_, SoundPanelState>) -> Result<SoundSets, String> {
    Ok(state.get_sets())
}

/// Получить активный набор
#[tauri::command]
pub fn sp_get_active_set(state: State<'_, SoundPanelState>) -> Result<SoundSet, String> {
    Ok(state.get_active_set())
}

/// Сменить активный набор
#[tauri::command]
pub fn sp_set_active_set(
    id: String,
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    info!(set_id = %id, "Setting active set");
    state.set_active_set(&id);
    save_sets(&state)?;

    let _ = emit_soundpanel_bindings_changed(&app_handle);
    let _ = app_handle.emit("soundpanel-active-set-changed", &id);

    Ok(())
}

/// Создать новый набор
#[tauri::command]
pub fn sp_add_set(
    name: String,
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
) -> Result<SoundSet, String> {
    info!(name, "Adding set");
    let set = state.add_set(&name)?;
    save_sets(&state)?;

    let _ = emit_soundpanel_bindings_changed(&app_handle);
    let _ = app_handle.emit("soundpanel-active-set-changed", &set.id);

    Ok(set)
}

/// Переименовать набор
#[tauri::command]
pub fn sp_rename_set(
    id: String,
    name: String,
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    info!(set_id = %id, name, "Renaming set");
    state.rename_set(&id, &name)?;
    save_sets(&state)?;

    let _ = emit_soundpanel_bindings_changed(&app_handle);

    Ok(())
}

/// Удалить набор
#[tauri::command]
pub fn sp_remove_set(
    id: String,
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    info!(set_id = %id, "Removing set");
    state.remove_set(&id)?;
    save_sets(&state)?;

    let _ = emit_soundpanel_bindings_changed(&app_handle);
    let _ = app_handle.emit("soundpanel-active-set-changed", "");

    Ok(())
}
