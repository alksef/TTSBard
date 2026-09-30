//! Пользовательские пути хранения (ROADMAP-117): каталог аудио-кеша и
//! открытие папки локальных данных.

use tauri::{AppHandle, State};

use crate::config::SettingsManager;

/// Эффективный каталог аудио-кеша и признак дефолтного пути.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioCacheDirInfo {
    pub path: String,
    pub is_default: bool,
}

/// Текущий каталог аудио-кеша (дефолт `%LOCALAPPDATA%\ttsbard\audio_cache`
/// или пользовательский путь).
#[tauri::command]
pub async fn storage_get_audio_cache_info() -> Result<AudioCacheDirInfo, String> {
    tokio::task::spawn_blocking(move || {
        let path = crate::history::cache_dir_path().map_err(|e| e.to_string())?;
        Ok(AudioCacheDirInfo {
            path: path.to_string_lossy().into_owned(),
            is_default: crate::history::audio_cache_dir_is_default(),
        })
    })
    .await
    .map_err(|e| format!("Операция была прервана: {}", e))?
}

/// Задать каталог аудио-кеша (`None` — вернуть дефолт). Существующие файлы
/// переносятся до записи настройки; при недоступном пути остаётся прежнее
/// подтверждённое состояние.
#[tauri::command]
pub async fn storage_set_audio_cache_dir(
    path: Option<String>,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let normalized = path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    let manager = settings_manager.inner().clone();

    let apply: Result<(), String> = tokio::task::spawn_blocking(move || {
        let previous_dir = crate::history::cache_dir_path().map_err(|e| e.to_string())?;

        let target_dir = match &normalized {
            Some(p) => {
                let dir = std::path::PathBuf::from(p);
                std::fs::create_dir_all(&dir)
                    .map_err(|e| format!("Не удалось создать каталог кеша: {}", e))?;
                dir
            }
            None => crate::paths::local_root()
                .map_err(|e| e.to_string())?
                .join("audio_cache"),
        };

        if target_dir != previous_dir {
            crate::history::migrate_cache_dir(&previous_dir, &target_dir);
        }

        // Настройка пишется только после успешного применения переноса;
        // провал записи возвращает прежнее подтверждённое состояние.
        let mut settings = manager.load().map_err(|e| e.to_string())?;
        settings.storage.audio_cache_dir = normalized;
        manager.save(&settings).map_err(|e| e.to_string())?;

        crate::history::init_audio_cache_dir(settings.storage.audio_cache_dir.as_deref());
        Ok(())
    })
    .await
    .map_err(|e| format!("Операция была прервана: {}", e))?;

    apply?;

    crate::commands::emit_settings_changed(&app_handle);
    Ok(())
}

/// Open the local data folder (%LOCALAPPDATA%/ttsbard) in the OS file manager.
#[tauri::command]
pub async fn open_local_data_folder() -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let app_dir = crate::paths::local_root()
            .map_err(|_| "Не удалось определить каталог локальных данных приложения".to_string())?;

        std::fs::create_dir_all(&app_dir)
            .map_err(|e| format!("Не удалось создать папку приложения: {}", e))?;

        let app_dir = app_dir
            .canonicalize()
            .map_err(|e| format!("Некорректный путь к папке приложения: {}", e))?;

        let path = app_dir
            .to_str()
            .ok_or("Некорректный путь к папке приложения")?;

        super::open_in_file_manager(path)
    })
    .await
    .map_err(|e| format!("Операция открытия папки была прервана: {}", e))?
}
