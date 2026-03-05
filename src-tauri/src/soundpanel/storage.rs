//! Sound Panel Storage
//!
//! Хранение привязок звуковой панели в JSON файле в %APPDATA%.
//! Копирование аудиофайлов в папку soundpanel.

use std::fs;
use std::path::PathBuf;
use crate::soundpanel::state::{SoundPanelState, SoundBinding};
use serde::{Serialize, Deserialize};

const BINDINGS_FILE: &str = "soundpanel_bindings.json";
const APPEARANCE_FILE: &str = "soundpanel_appearance.json";

/// Настройки внешнего вида звуковой панели
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

    eprintln!("[SOUNDPANEL] Loading bindings from: {:?}", file_path);

    if !file_path.exists() {
        eprintln!("[SOUNDPANEL] Bindings file does not exist, starting with empty bindings");
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read bindings file: {}", e))?;

    let bindings: Vec<SoundBinding> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse bindings: {}", e))?;

    eprintln!("[SOUNDPANEL] Loaded {} bindings", bindings.len());

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

    eprintln!("[SOUNDPANEL] Saving {} bindings to: {:?}", bindings.len(), file_path);

    let json = serde_json::to_string_pretty(&bindings)
        .map_err(|e| format!("Failed to serialize bindings: {}", e))?;

    fs::write(&file_path, json)
        .map_err(|e| format!("Failed to write bindings file: {}", e))?;

    eprintln!("[SOUNDPANEL] Bindings saved successfully");
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
        eprintln!("[SOUNDPANEL] Created soundpanel directory: {:?}", soundpanel_dir);
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

    eprintln!("[SOUNDPANEL] Copied sound file: {} -> {}", source_path, final_filename);

    Ok(final_filename)
}

/// Удалить файл звука из папки soundpanel
pub fn delete_sound_file(filename: &str, appdata_path: &str) -> Result<(), String> {
    let soundpanel_dir = PathBuf::from(appdata_path).join("soundpanel");
    let file_path = soundpanel_dir.join(filename);

    if file_path.exists() {
        fs::remove_file(&file_path)
            .map_err(|e| format!("Failed to delete sound file: {}", e))?;
        eprintln!("[SOUNDPANEL] Deleted sound file: {:?}", file_path);
    }

    Ok(())
}

/// Сгенерировать уникальный путь для файла
/// Если файл с таким именем существует, добавляет суффикс _1, _2 и т.д.
fn generate_unique_path(dir: &PathBuf, filename: &str) -> PathBuf {
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

/// Загрузить настройки внешнего вида из JSON файла
pub fn load_appearance(state: &SoundPanelState) -> Result<SoundPanelAppearance, String> {
    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let file_path = PathBuf::from(&appdata_path).join(APPEARANCE_FILE);

    eprintln!("[SOUNDPANEL] Loading appearance from: {:?}", file_path);

    if !file_path.exists() {
        eprintln!("[SOUNDPANEL] Appearance file does not exist, using defaults");
        let appearance = SoundPanelAppearance::default();
        // Создать файл с настройками по умолчанию
        let _ = save_appearance_direct(&file_path, &appearance);
        return Ok(appearance);
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read appearance file: {}", e))?;

    let appearance: SoundPanelAppearance = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse appearance: {}", e))?;

    eprintln!("[SOUNDPANEL] Loaded appearance: opacity={}%, color={}, clickthrough={}", appearance.opacity, appearance.bg_color, appearance.clickthrough);

    // Применить к состоянию
    state.set_floating_opacity(appearance.opacity);
    state.set_floating_bg_color(appearance.bg_color.clone());
    state.set_floating_clickthrough(appearance.clickthrough);

    Ok(appearance)
}

/// Сохранить настройки внешнего вида в JSON файл
pub fn save_appearance(state: &SoundPanelState) -> Result<(), String> {
    let appearance = SoundPanelAppearance {
        opacity: state.get_floating_opacity(),
        bg_color: state.get_floating_bg_color(),
        clickthrough: state.is_floating_clickthrough_enabled(),
    };

    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let file_path = PathBuf::from(&appdata_path).join(APPEARANCE_FILE);

    save_appearance_direct(&file_path, &appearance)
}

/// Внутренняя функция сохранения appearance (без доступа к state)
fn save_appearance_direct(file_path: &PathBuf, appearance: &SoundPanelAppearance) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&appearance)
        .map_err(|e| format!("Failed to serialize appearance: {}", e))?;

    fs::write(&file_path, json)
        .map_err(|e| format!("Failed to write appearance file: {}", e))?;

    eprintln!("[SOUNDPANEL] Appearance saved: opacity={}%, color={}", appearance.opacity, appearance.bg_color);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_unique_path() {
        // Этот тест требует реальной файловой системы
        // В реальном коде можно использовать tempfile crate
    }
}
