//! Sound Panel Tauri Commands
//!
//! Tauri команды для взаимодействия между frontend и backend.

use crate::soundpanel::state::{SoundPanelState, SoundBinding};
use crate::soundpanel::storage::{save_bindings, copy_sound_file, delete_sound_file, save_appearance};
use crate::soundpanel::audio::play_audio_file;
use tauri::{State, AppHandle, Manager};

/// Получить все привязки звуковой панели
#[tauri::command]
pub fn sp_get_bindings(state: State<'_, SoundPanelState>) -> Result<Vec<SoundBinding>, String> {
    eprintln!("[SOUNDPANEL] Get bindings command");
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
    state: State<'_, SoundPanelState>,
) -> Result<SoundBinding, String> {
    eprintln!("[SOUNDPANEL] Add binding: key={}, desc={}", key, description);

    // Валидация клавиши: только A-Z
    let key_char = key.to_uppercase().chars().next()
        .ok_or("Key is empty")?;

    if !key_char.is_ascii_alphabetic() || !key_char.is_ascii_uppercase() {
        return Err("Key must be A-Z".to_string());
    }

    // Проверка: клавиша уже занята
    if let Some(existing) = state.get_binding(key_char) {
        return Err(format!("Key {} is already bound to '{}'", key_char, existing.description));
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

    eprintln!("[SOUNDPANEL] Binding added successfully");
    Ok(binding)
}

/// Удалить привязку по клавише
#[tauri::command]
pub fn sp_remove_binding(
    key: String,
    state: State<'_, SoundPanelState>,
) -> Result<(), String> {
    eprintln!("[SOUNDPANEL] Remove binding: key={}", key);

    let key_char = key.chars().next()
        .ok_or("Key is empty")?;

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

    eprintln!("[SOUNDPANEL] Binding removed successfully");
    Ok(())
}

/// Тестировать воспроизведение звука
///
/// Воспроизводит указанный файл без создания привязки
#[tauri::command]
pub fn sp_test_sound(file_path: String) -> Result<(), String> {
    eprintln!("[SOUNDPANEL] Test sound: {}", file_path);

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
    Ok(crate::soundpanel::audio::is_supported_audio_format(&filename))
}

/// Получить настройки внешнего вида floating окна звуковой панели
#[tauri::command]
pub fn sp_get_floating_appearance(state: State<'_, SoundPanelState>) -> Result<(u8, String), String> {
    let opacity = state.get_floating_opacity();
    let color = state.get_floating_bg_color();
    Ok((opacity, color))
}

/// Установить прозрачность floating окна звуковой панели
#[tauri::command]
pub fn sp_set_floating_opacity(value: u8, state: State<'_, SoundPanelState>) -> Result<(), String> {
    eprintln!("[SOUNDPANEL] Setting opacity to {}", value);
    state.set_floating_opacity(value);
    // Сохраняем в файл
    save_appearance(&state)?;
    Ok(())
}

/// Установить цвет фона floating окна звуковой панели
#[tauri::command]
pub fn sp_set_floating_bg_color(color: String, state: State<'_, SoundPanelState>) -> Result<(), String> {
    // Валидация hex цвета
    if !color.starts_with('#') || color.len() != 7 {
        return Err("Invalid color format. Use #RRGGBB".to_string());
    }
    eprintln!("[SOUNDPANEL] Setting bg color to {}", color);
    state.set_floating_bg_color(color);
    // Сохраняем в файл
    save_appearance(&state)?;
    Ok(())
}

/// Установить clickthrough для floating окна звуковой панели
#[tauri::command]
pub fn sp_set_floating_clickthrough(enabled: bool, state: State<'_, SoundPanelState>) -> Result<(), String> {
    eprintln!("[SOUNDPANEL] Setting clickthrough to {}", enabled);
    state.set_floating_clickthrough(enabled);
    // Сохраняем в файл
    save_appearance(&state)?;
    Ok(())
}

/// Проверить, включен ли clickthrough для floating окна звуковой панели
#[tauri::command]
pub fn sp_is_floating_clickthrough_enabled(state: State<'_, SoundPanelState>) -> Result<bool, String> {
    Ok(state.is_floating_clickthrough_enabled())
}

/// Установить исключение из записи экрана для звуковой панели
#[tauri::command]
pub fn sp_set_exclude_from_recording(enabled: bool, state: State<'_, SoundPanelState>) -> Result<(), String> {
    eprintln!("[SOUNDPANEL] Setting exclude_from_recording to {}", enabled);
    state.set_exclude_from_recording(enabled);
    // Сохраняем в файл
    save_appearance(&state)?;
    Ok(())
}

/// Проверить, исключено ли окно из записи экрана
#[tauri::command]
pub fn sp_is_exclude_from_recording(state: State<'_, SoundPanelState>) -> Result<bool, String> {
    Ok(state.is_exclude_from_recording())
}

/// Применить исключение из записи к существующему окну звуковой панели
#[tauri::command]
pub fn sp_apply_exclude_recording(app_handle: AppHandle, state: State<'_, SoundPanelState>) -> Result<(), String> {
    if app_handle.get_webview_window("soundpanel").is_some() {
        // Настройка будет применена при следующем показе окна
        let exclude = state.is_exclude_from_recording();
        eprintln!("[SOUNDPANEL] Apply exclude recording called, exclude={}", exclude);
        return Ok(());
    }
    Err("Window not available".to_string())
}
