//! Sound Panel Tauri Commands
//!
//! Tauri команды для взаимодействия между frontend и backend.

use crate::commands::window::resolve_main_appearance;
use crate::config::{is_valid_hex_color, SettingsManager, WindowsManager};
use crate::soundpanel::audio::play_audio_file;
use crate::soundpanel::intercept::InterceptSettings;
use crate::soundpanel::state::{SoundBinding, SoundPanelState, SoundSet, SoundSets};
use crate::soundpanel::storage::{copy_sound_file, delete_sound_file, save_sets};
use crate::soundpanel_window::{
    emit_soundpanel_bindings_changed, hide_soundpanel_window, restore_soundpanel_foreground,
    restore_soundpanel_foreground_retaining_target, update_soundpanel_appearance,
};
use crate::state::AppState;
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

    if !(key_char.is_ascii_uppercase() || key_char.is_ascii_digit()) {
        return Err("Key must be A–Z or 0–9".to_string());
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

/// Обновить привязку: описание и (опционально) заменить файл.
///
/// * `key` - Клавиша A–Z или 0–9
/// * `description` - Новое описание звука
/// * `file_path` - Путь к новому аудиофайлу; `None` — оставить текущий файл
#[tauri::command]
pub fn sp_update_binding(
    key: String,
    description: String,
    file_path: Option<String>,
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
) -> Result<SoundBinding, String> {
    info!(key, description, "Update binding");

    let key_char = key.to_uppercase().chars().next().ok_or("Key is empty")?;

    let mut binding = state
        .get_binding(key_char)
        .ok_or_else(|| format!("Key {} is not bound", key_char))?;

    let appdata_path = state.appdata_path.lock().unwrap().clone();

    if let Some(file_path) = file_path.filter(|p| !p.is_empty()) {
        let new_filename = copy_sound_file(&file_path, &appdata_path)?;
        let _ = delete_sound_file(&binding.filename, &appdata_path);
        binding.filename = new_filename;
        binding.original_path = Some(file_path);
    }

    binding.description = description;

    state.add_binding(binding.clone());
    save_sets(&state)?;

    let _ = emit_soundpanel_bindings_changed(&app_handle);

    info!("Binding updated successfully");
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

/// Проверить, включён ли stay_visible
#[tauri::command]
pub fn sp_get_stay_visible(state: State<'_, SoundPanelState>) -> Result<bool, String> {
    Ok(state.get_stay_visible())
}

/// Установить stay_visible
///
/// Сначала сохраняет настройку на диск, затем обновляет runtime-состояние
/// и отправляет событие `soundpanel-appearance-update` для обновления
/// открытой панели.
#[tauri::command]
pub fn sp_set_stay_visible(
    enabled: bool,
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(enabled, "Setting stay_visible");
    windows_manager
        .set_soundpanel_stay_visible(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    state.set_stay_visible(enabled);
    let _ = update_soundpanel_appearance(&app_handle);
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

/// Установить транзитный флаг config-режима (не сохраняется на диск).
///
/// Пока активен config-режим, blur-скрытие панели подавляется (например,
/// открытие native file picker не должно скрывать панель).
#[tauri::command]
pub fn sp_set_config_mode(enabled: bool, state: State<'_, SoundPanelState>) -> Result<(), String> {
    info!(enabled, "Setting config_mode");
    state.set_config_mode(enabled);
    Ok(())
}

/// Обработка Escape: следует той же политике видимости, что и активация
/// звука, но без воспроизведения.
///
/// - `stay_visible == false`: `hide → restore`
/// - `stay_visible == true`:  `restore` (панель остаётся видимой)
#[tauri::command]
pub fn sp_escape_soundpanel(
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    let stay_visible = state.get_stay_visible();
    let app_state = app_handle.state::<AppState>();
    handoff_soundpanel_ordered(
        stay_visible,
        || {
            hide_soundpanel_window(&app_handle, &app_state)
                .map_err(|e| format!("Failed to hide window: {}", e))
        },
        || {
            if stay_visible {
                restore_soundpanel_foreground_retaining_target(&app_state)
            } else {
                restore_soundpanel_foreground(&app_state)
            }
        },
    )
}

/// Воспроизвести звук по клавише (A-Z).
///
/// При выключенном `stay_visible` сначала синхронно скрывается окно, затем
/// предпринимается попытка вернуть фокус сохранённому внешнему окну, и только
/// после этого начинается воспроизведение. Ошибка скрытия останавливает
/// выполнение; ошибка восстановления фокуса возвращается, но не подавляет
/// воспроизведение.
///
/// При включённом `stay_visible` панель остаётся видимой, фокус возвращается,
/// и воспроизведение продолжается.
#[tauri::command]
pub fn sp_play_binding(key: String, app_handle: AppHandle) -> Result<(), String> {
    let key_char = key.chars().next().ok_or("Key is empty")?;
    if !(key_char.is_ascii_uppercase() || key_char.is_ascii_digit()) {
        return Err("Key must be A–Z or 0–9".to_string());
    }
    let state = app_handle.state::<SoundPanelState>();
    let binding = state
        .get_binding(key_char)
        .ok_or_else(|| format!("No binding for key {}", key_char))?;
    let stay_visible = state.get_stay_visible();
    info!(key = %key_char, description = binding.description, stay_visible, "Playing binding");

    let app_state = app_handle.state::<AppState>();
    play_binding_ordered(
        stay_visible,
        || {
            hide_soundpanel_window(&app_handle, &app_state)
                .map_err(|e| format!("Failed to hide window: {}", e))
        },
        || {
            if stay_visible {
                restore_soundpanel_foreground_retaining_target(&app_state)
            } else {
                restore_soundpanel_foreground(&app_state)
            }
        },
        || state.play_sound(&binding),
    )
}

/// Единственный владелец последовательности «скрыть → восстановить фокус → воспроизвести».
///
/// При `stay_visible == true` скрытие пропускается. Восстановление фокуса
/// всегда происходит до воспроизведения. Ошибка скрытия останавливает
/// выполнение только в unchecked-режиме. Ошибка восстановления фокуса
/// возвращается, но не подавляет воспроизведение в обоих режимах.
fn play_binding_ordered<H, R, F>(
    stay_visible: bool,
    hide: H,
    restore: R,
    play: F,
) -> Result<(), String>
where
    H: FnOnce() -> Result<(), String>,
    R: FnOnce() -> Result<(), String>,
    F: FnOnce(),
{
    if !stay_visible {
        hide()?;
    }
    let restore_result = restore();
    play();
    restore_result
}

/// Выполнить оконную часть Escape без воспроизведения.
fn handoff_soundpanel_ordered<H, R>(stay_visible: bool, hide: H, restore: R) -> Result<(), String>
where
    H: FnOnce() -> Result<(), String>,
    R: FnOnce() -> Result<(), String>,
{
    if !stay_visible {
        hide()?;
    }
    restore()
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
    state.set_intercept_enabled(enabled)
}

/// Установить биндинг перехвата
#[tauri::command]
pub fn set_intercept_binding(
    key: String,
    action: String,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    state.set_intercept_binding(key, action)
}

/// Очистить биндинг перехвата
#[tauri::command]
pub fn clear_intercept_binding(
    key: String,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    state.clear_intercept_binding(key)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn play_with_recording(
        stay_visible: bool,
        hide_result: Result<(), String>,
        restore_result: Result<(), String>,
    ) -> (Result<(), String>, Vec<&'static str>) {
        let order = RefCell::new(Vec::new());
        let result = play_binding_ordered(
            stay_visible,
            || {
                order.borrow_mut().push("hide");
                hide_result.clone()
            },
            || {
                order.borrow_mut().push("restore");
                restore_result.clone()
            },
            || {
                order.borrow_mut().push("play");
            },
        );
        (result, order.into_inner())
    }

    #[test]
    fn unchecked_hide_and_restore_complete_before_play() {
        let (result, order) = play_with_recording(false, Ok(()), Ok(()));
        assert!(result.is_ok());
        assert_eq!(order, vec!["hide", "restore", "play"]);
    }

    #[test]
    fn unchecked_hide_failure_prevents_restore_and_playback() {
        let (result, order) = play_with_recording(false, Err("hide failed".to_string()), Ok(()));
        assert!(result.is_err());
        assert_eq!(order, vec!["hide"]);
    }

    #[test]
    fn unchecked_restore_failure_still_permits_playback_and_reports_failure() {
        let (result, order) = play_with_recording(false, Ok(()), Err("restore failed".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("restore failed"));
        assert_eq!(order, vec!["hide", "restore", "play"]);
    }

    #[test]
    fn checked_skips_hide_and_orders_restore_then_play() {
        let (result, order) = play_with_recording(true, Ok(()), Ok(()));
        assert!(result.is_ok());
        assert_eq!(order, vec!["restore", "play"]);
    }

    #[test]
    fn checked_restore_failure_still_permits_playback_and_reports_failure() {
        let (result, order) = play_with_recording(true, Ok(()), Err("restore failed".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("restore failed"));
        assert_eq!(order, vec!["restore", "play"]);
    }

    fn record_escape(
        stay_visible: bool,
        hide_result: Result<(), String>,
        restore_result: Result<(), String>,
    ) -> (Result<(), String>, Vec<&'static str>) {
        let order = RefCell::new(Vec::new());
        let result = handoff_soundpanel_ordered(
            stay_visible,
            || {
                order.borrow_mut().push("hide");
                hide_result.clone()
            },
            || {
                order.borrow_mut().push("restore");
                restore_result.clone()
            },
        );
        (result, order.into_inner())
    }

    #[test]
    fn escape_unchecked_orders_hide_then_restore() {
        let (result, order) = record_escape(false, Ok(()), Ok(()));
        assert!(result.is_ok());
        assert_eq!(order, vec!["hide", "restore"]);
    }

    #[test]
    fn escape_checked_orders_only_restore() {
        let (result, order) = record_escape(true, Ok(()), Ok(()));
        assert!(result.is_ok());
        assert_eq!(order, vec!["restore"]);
    }

    #[test]
    fn escape_unchecked_hide_failure_prevents_restore() {
        let (result, order) = record_escape(false, Err("hide failed".to_string()), Ok(()));
        assert!(result.is_err());
        assert_eq!(order, vec!["hide"]);
    }
}
