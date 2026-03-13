//! Sound Panel Storage
//!
//! Хранение привязок звуковой панели в JSON файле в %APPDATA%.
//! Копирование аудиофайлов в папку soundpanel.
//!
//! NOTE: Appearance settings are now stored in windows.json (via WindowsManager)
//! The old soundpanel_appearance.json file is no longer used.

use std::fs;
use std::path::{Path, PathBuf};
use crate::soundpanel::state::{SoundPanelState, SoundBinding};
use crate::config::WindowsManager;
use serde::{Serialize, Deserialize};
use tracing::{debug, info};

const BINDINGS_FILE: &str = "soundpanel_bindings.json";

/// Настройки внешнего вида звуковой панели (deprecated, use WindowsManager instead)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundPanelAppearance {
    pub opacity: u8,
    pub bg_color: String,
    /// Пропускает ли плавающее окно клики
    #[serde(default = "default_clickthrough")]
    pub clickthrough: bool,
}

fn default_clickthrough() -> bool { false }

impl Default for SoundPanelAppearance {
    fn default() -> Self {
        Self {
            opacity: 90,
            bg_color: "#2a2a2a".to_string(),
            clickthrough: false,
        }
    }
}

/// Загрузить привязки из JSON файла
pub fn load_bindings(state: &SoundPanelState) -> Result<Vec<SoundBinding>, String> {
    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let file_path = PathBuf::from(&appdata_path).join(BINDINGS_FILE);

    debug!(?file_path, "Loading bindings");

    if !file_path.exists() {
        debug!("Bindings file does not exist, starting with empty bindings");
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read bindings file: {}", e))?;

    let bindings: Vec<SoundBinding> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse bindings: {}", e))?;

    info!(count = bindings.len(), "Loaded bindings");

    // Загрузить в состояние
    for binding in &bindings {
        state.add_binding(binding.clone());
    }

    Ok(bindings)
}

/// Сохранить привязки в JSON файл
pub fn save_bindings(state: &SoundPanelState) -> Result<(), String> {
    let bindings = state.get_all_bindings();

    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let file_path = PathBuf::from(&appdata_path).join(BINDINGS_FILE);

    info!(count = bindings.len(), ?file_path, "Saving bindings");

    let json = serde_json::to_string_pretty(&bindings)
        .map_err(|e| format!("Failed to serialize bindings: {}", e))?;

    fs::write(&file_path, json)
        .map_err(|e| format!("Failed to write bindings file: {}", e))?;

    info!("Bindings saved successfully");
    Ok(())
}

/// Скопировать аудиофайл в папку soundpanel
///
/// Возвращает имя скопированного файла
pub fn copy_sound_file(source_path: &str, appdata_path: &str) -> Result<String, String> {
    // Создать папку soundpanel если не существует
    let soundpanel_dir = PathBuf::from(appdata_path).join("soundpanel");

    if !soundpanel_dir.exists() {
        fs::create_dir_all(&soundpanel_dir)
            .map_err(|e| format!("Failed to create soundpanel directory: {}", e))?;
        debug!(?soundpanel_dir, "Created soundpanel directory");
    }

    // Получить имя файла
    let source = PathBuf::from(source_path);
    let filename = source.file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;

    // Уникальное имя если файл существует
    let dest_path = generate_unique_path(&soundpanel_dir, filename);

    // Скопировать файл
    fs::copy(&source, &dest_path)
        .map_err(|e| format!("Failed to copy sound file: {}", e))?;

    let final_filename = dest_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap()
        .to_string();

    debug!(source_path, final_filename, "Copied sound file");

    Ok(final_filename)
}

/// Удалить файл звука из папки soundpanel
pub fn delete_sound_file(filename: &str, appdata_path: &str) -> Result<(), String> {
    let soundpanel_dir = PathBuf::from(appdata_path).join("soundpanel");
    let file_path = soundpanel_dir.join(filename);

    if file_path.exists() {
        fs::remove_file(&file_path)
            .map_err(|e| format!("Failed to delete sound file: {}", e))?;
        debug!(?file_path, "Deleted sound file");
    }

    Ok(())
}

/// Сгенерировать уникальный путь для файла
/// Если файл с таким именем существует, добавляет суффикс _1, _2 и т.д.
fn generate_unique_path(dir: &Path, filename: &str) -> PathBuf {
    let mut path = dir.join(filename);
    let mut counter = 1;

    // Extract stem and extension once to avoid temporary values
    let stem = PathBuf::from(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();

    let ext = PathBuf::from(filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s))
        .unwrap_or_default();

    while path.exists() {
        let new_name = format!("{}_{}{}", stem, counter, ext);
        path = dir.join(&new_name);
        counter += 1;
    }

    path
}

/// Загрузить настройки внешнего вида из windows.json
pub fn load_appearance(state: &SoundPanelState, windows_manager: &WindowsManager) -> Result<SoundPanelAppearance, String> {
    debug!("Loading appearance from windows.json");

    // Load from WindowsManager
    let opacity = windows_manager.get_soundpanel_opacity();
    let bg_color = windows_manager.get_soundpanel_bg_color();
    let clickthrough = windows_manager.get_soundpanel_clickthrough();

    info!(
        opacity,
        bg_color,
        clickthrough,
        "Loaded appearance"
    );

    // Применить к состоянию
    state.set_floating_opacity(opacity);
    state.set_floating_bg_color(bg_color.clone());
    state.set_floating_clickthrough(clickthrough);

    Ok(SoundPanelAppearance {
        opacity,
        bg_color,
        clickthrough,
    })
}

// NOTE: save_appearance and save_appearance_direct functions removed
// Appearance settings are now saved via WindowsManager in the command handlers

#[cfg(test)]
mod tests {
    #[test]
    fn test_generate_unique_path() {
        // Этот тест требует реальной файловой системы
        // В реальном коде можно использовать tempfile crate
    }
}
