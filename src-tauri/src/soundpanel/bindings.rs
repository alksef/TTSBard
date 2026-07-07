//! Sound Panel Tauri Commands
//!
//! Tauri команды для взаимодействия между frontend и backend.

use crate::config::{is_valid_hex_color, WindowsManager};
use crate::events::AppEvent;
use crate::soundpanel::audio::play_audio_file;
use crate::soundpanel::intercept::InterceptSettings;
use crate::soundpanel::state::{SoundBinding, SoundPanelState};
use crate::soundpanel::storage::{copy_sound_file, delete_sound_file, save_bindings};
use crate::soundpanel_window::emit_soundpanel_bindings_changed;
use tauri::{AppHandle, Manager, State};
use tracing::{debug, info};

/// Получить все привязки звуковой панели
#[tauri::command]
pub fn sp_get_bindings(state: State<'_, SoundPanelState>) -> Result<Vec<SoundBinding>, String> {
    debug!("Get bindings command");
    Ok(state.get_all_bindings())
}

/// Добавить новую привязку
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

    // Валидация клавиши: только A-Z
    let key_char = key.to_uppercase().chars().next().ok_or("Key is empty")?;

    if !key_char.is_ascii_alphabetic() || !key_char.is_ascii_uppercase() {
        return Err("Key must be A-Z".to_string());
    }

    // Проверка: клавиша уже занята
    if let Some(existing) = state.get_binding(key_char) {
        return Err(format!(
            "Key {} is already bound to '{}'",
            key_char, existing.description
        ));
    }

    // Копировать файл в папку soundpanel
    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let filename = copy_sound_file(&file_path, &appdata_path)?;

    let binding = SoundBinding {
        key: key_char,
        description,
        filename,
        original_path: Some(file_path),
    };

    // Добавить в состояние
    state.add_binding(binding.clone());

    // Сохранить в JSON
    save_bindings(&state)?;

    // Notify soundpanel window to reload bindings
    let _ = emit_soundpanel_bindings_changed(&app_handle);

    info!("Binding added successfully");
    Ok(binding)
}

/// Удалить привязку по клавише
#[tauri::command]
pub fn sp_remove_binding(
    key: String,
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    info!(key, "Remove binding");

    let key_char = key.chars().next().ok_or("Key is empty")?;

    // Получить привязку для удаления файла
    if let Some(binding) = state.get_binding(key_char) {
        // Удалить файл звук
        let appdata_path = state.appdata_path.lock().unwrap().clone();
        let _ = delete_sound_file(&binding.filename, &appdata_path);
    }

    // Удалить из состояния
    state.remove_binding(key_char);

    // Сохранить изменения
    save_bindings(&state)?;

    // Notify soundpanel window to reload bindings
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

    // Проверить существование файла
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
#[tauri::command]
pub fn sp_get_floating_appearance(
    state: State<'_, SoundPanelState>,
) -> Result<(u8, String), String> {
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
    // Сохраняем в windows.json
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
    // Валидация hex цвета
    if !is_valid_hex_color(&color) {
        return Err("Invalid color format. Use #RRGGBB".to_string());
    }
    info!(color, "Setting bg color");
    state.set_floating_bg_color(color.clone());
    // Сохраняем в windows.json
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
    // Сохраняем в windows.json
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

/// Воспроизвести звук по клавише (A-Z) и скрыть панель
#[tauri::command]
pub fn sp_play_binding(key: String, app_handle: AppHandle) -> Result<(), String> {
    let key_char = key.chars().next().ok_or("Key is empty")?;
    if !key_char.is_ascii_uppercase() || key_char < 'A' || key_char > 'Z' {
        return Err("Key must be A-Z".to_string());
    }
    let state = app_handle.state::<SoundPanelState>();
    if let Some(binding) = state.get_binding(key_char) {
        info!(key = %key_char, description = binding.description, "Playing binding");
        state.play_sound(&binding);
        state.emit_event(AppEvent::HideSoundPanelWindow);
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
