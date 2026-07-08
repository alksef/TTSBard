//! Sound Panel Storage
//!
//! Хранение привязок звуковой панели в JSON файле в %APPDATA%.
//! Копирование аудиофайлов в папку soundpanel.
//!
//! NOTE: Appearance settings are now stored in windows.json (via WindowsManager)
//! The old soundpanel_appearance.json file is no longer used.

use crate::config::WindowsManager;
use crate::soundpanel::state::{SoundBinding, SoundPanelState, SoundSets};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

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

fn default_clickthrough() -> bool {
    false
}

impl Default for SoundPanelAppearance {
    fn default() -> Self {
        Self {
            opacity: 90,
            bg_color: "#2a2a2a".to_string(),
            clickthrough: false,
        }
    }
}

/// Загрузить привязки из JSON файла (с миграцией из старого формата)
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

    let sets = if let Ok(parsed) = serde_json::from_str::<SoundSets>(&content) {
        info!(
            set_count = parsed.sets.len(),
            "Loaded bindings (new format)"
        );
        parsed
    } else if let Ok(old_bindings) = serde_json::from_str::<Vec<SoundBinding>>(&content) {
        // Миграция из старого формата Vec<SoundBinding> → SoundSets
        warn!(
            count = old_bindings.len(),
            "Migrating bindings from old format (Vec) to SoundSets"
        );
        let id = uuid::Uuid::new_v4().to_string();
        SoundSets {
            active_set_id: id.clone(),
            sets: vec![crate::soundpanel::state::SoundSet {
                id,
                name: "Основной".into(),
                bindings: old_bindings,
            }],
        }
    } else {
        return Err("Failed to parse bindings file: unrecognized format".to_string());
    };

    let bindings = sets
        .find_active()
        .map(|s| s.bindings.clone())
        .unwrap_or_default();

    info!(
        set_count = sets.sets.len(),
        bindings_count = bindings.len(),
        "Loaded bindings"
    );

    state.replace_sets(sets);

    Ok(bindings)
}

/// Сохранить наборы в JSON файл
pub fn save_sets(state: &SoundPanelState) -> Result<(), String> {
    let sets = state.get_sets();

    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let file_path = PathBuf::from(&appdata_path).join(BINDINGS_FILE);

    info!(set_count = sets.sets.len(), active_set_id = %sets.active_set_id, ?file_path, "Saving sets");

    let json = serde_json::to_string_pretty(&sets)
        .map_err(|e| format!("Failed to serialize sets: {}", e))?;

    fs::write(&file_path, json).map_err(|e| format!("Failed to write bindings file: {}", e))?;

    info!("Sets saved successfully");
    Ok(())
}

/// Сохранить привязки в JSON файл (alias для save_sets, обратная совместимость)
#[allow(dead_code)]
pub fn save_bindings(state: &SoundPanelState) -> Result<(), String> {
    save_sets(state)
}

/// Скопировать аудиофайл в папку soundpanel
///
/// Возвращает имя скопированного файла
pub fn copy_sound_file(source_path: &str, appdata_path: &str) -> Result<String, String> {
    let soundpanel_dir = PathBuf::from(appdata_path).join("soundpanel");

    if !soundpanel_dir.exists() {
        fs::create_dir_all(&soundpanel_dir)
            .map_err(|e| format!("Failed to create soundpanel directory: {}", e))?;
        debug!(?soundpanel_dir, "Created soundpanel directory");
    }

    let source = PathBuf::from(source_path);
    let filename = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;

    let dest_path = generate_unique_path(&soundpanel_dir, filename);

    fs::copy(&source, &dest_path).map_err(|e| format!("Failed to copy sound file: {}", e))?;

    let final_filename = dest_path
        .file_name()
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
        fs::remove_file(&file_path).map_err(|e| format!("Failed to delete sound file: {}", e))?;
        debug!(?file_path, "Deleted sound file");
    }

    Ok(())
}

/// Сгенерировать уникальный путь для файла
/// Если файл с таким именем существует, добавляет суффикс _1, _2 и т.д.
fn generate_unique_path(dir: &Path, filename: &str) -> PathBuf {
    let mut path = dir.join(filename);
    let mut counter = 1;

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
pub fn load_appearance(
    state: &SoundPanelState,
    windows_manager: &WindowsManager,
) -> Result<SoundPanelAppearance, String> {
    debug!("Loading appearance from windows.json");

    let opacity = windows_manager.get_soundpanel_opacity();
    let bg_color = windows_manager.get_soundpanel_bg_color();
    let clickthrough = windows_manager.get_soundpanel_clickthrough();

    info!(opacity, bg_color, clickthrough, "Loaded appearance");

    state.set_floating_opacity(opacity);
    state.set_floating_bg_color(bg_color.clone());
    state.set_floating_clickthrough(clickthrough);

    Ok(SoundPanelAppearance {
        opacity,
        bg_color,
        clickthrough,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soundpanel::state::SoundSet;

    #[test]
    fn test_migration_old_vec_to_sound_sets() {
        let old_json = r#"[
            {"key":"A","description":"test a","filename":"a.mp3","original_path":null},
            {"key":"B","description":"test b","filename":"b.mp3","original_path":"D:\\b.mp3"}
        ]"#;

        // Simulate parsing logic
        let content = old_json;
        let sets = if let Ok(parsed) = serde_json::from_str::<SoundSets>(&content) {
            parsed
        } else if let Ok(old_bindings) = serde_json::from_str::<Vec<SoundBinding>>(&content) {
            let id = uuid::Uuid::new_v4().to_string();
            SoundSets {
                active_set_id: id.clone(),
                sets: vec![SoundSet {
                    id,
                    name: "Основной".into(),
                    bindings: old_bindings,
                }],
            }
        } else {
            SoundSets::default()
        };

        assert_eq!(sets.sets.len(), 1);
        assert_eq!(sets.sets[0].name, "Основной");
        assert_eq!(sets.sets[0].bindings.len(), 2);
        assert_eq!(sets.sets[0].bindings[0].key, 'A');
        assert!(!sets.active_set_id.is_empty());
    }

    #[test]
    fn test_new_format_loads_directly() {
        let new_json = r#"{
            "active_set_id": "set1",
            "sets": [
                {"id": "set1", "name": "Основной", "bindings": []},
                {"id": "set2", "name": "Мемы", "bindings": [
                    {"key":"Z","description":"lol","filename":"lol.mp3","original_path":null}
                ]}
            ]
        }"#;

        let sets: SoundSets = serde_json::from_str(new_json).unwrap();
        assert_eq!(sets.sets.len(), 2);
        assert_eq!(sets.active_set_id, "set1");

        let active = sets.find_active().unwrap();
        assert_eq!(active.id, "set1");
        assert_eq!(active.name, "Основной");
    }
}
