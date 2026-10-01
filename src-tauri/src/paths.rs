//! Центральное разрешение каталогов данных приложения (ROADMAP-117).
//!
//! Каждая категория данных получает каталог по своей семантике:
//! - config (Roaming): настройки, история, шаблоны — [`config_root`];
//! - cache (Local): регенерируемый кеш и модели — [`local_root`];
//! - transient: короткоживущие файлы — [`temp_root`].
//!
//! Функции разрешения чистые и не создают каталогов на диске: создание
//! остаётся ответственностью потребителя. Исключение — функции очистки,
//! которые сами помечены как одноразовые стартовые.

use anyhow::Context;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const APP_DIR_NAME: &str = "ttsbard";

/// Пользовательский override корня данных программы (`storage.data_dir`).
/// Глобальное состояние — как и остальные владельцы путей: hot-path резолверы
/// не имеют доступа к SettingsManager.
static DATA_ROOT_OVERRIDE: OnceLock<parking_lot::RwLock<Option<PathBuf>>> = OnceLock::new();

fn data_root_override() -> &'static parking_lot::RwLock<Option<PathBuf>> {
    DATA_ROOT_OVERRIDE.get_or_init(|| parking_lot::RwLock::new(None))
}

/// Roaming-корень данных: `%APPDATA%\ttsbard` (Windows), XDG config (Linux),
/// `~/Library/Application Support/ttsbard` (macOS).
pub(crate) fn config_root() -> anyhow::Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("Failed to resolve config directory")?
        .join(APP_DIR_NAME))
}

/// Дефолтный корень данных программы: `%LOCALAPPDATA%\ttsbard` (Windows),
/// XDG cache (Linux), `~/Library/Caches/ttsbard` (macOS). Не зависит от
/// пользовательского override и используется для сброса (`reset`).
pub(crate) fn default_data_root() -> anyhow::Result<PathBuf> {
    Ok(dirs::cache_dir()
        .context("Failed to resolve local data directory")?
        .join(APP_DIR_NAME))
}

/// Эффективный корень данных программы (пользовательский override либо дефолт).
/// Каталог не создаётся.
pub(crate) fn data_root() -> anyhow::Result<PathBuf> {
    if let Some(dir) = data_root_override().read().clone() {
        Ok(dir)
    } else {
        default_data_root()
    }
}

/// Используется ли дефолтный корень данных (без пользовательского override).
pub(crate) fn data_root_is_default() -> bool {
    data_root_override().read().is_none()
}

/// Publish an already validated and created root after its setting was saved.
pub(crate) fn publish_data_root(custom: Option<PathBuf>) {
    *data_root_override().write() = custom;
}

/// Применить пользовательский корень данных из настроек. `None` сбрасывает
/// override; невалидный (несоздаваемый) путь игнорируется с warning — запуск не
/// блокируется, используется дефолт.
pub(crate) fn init_data_root(custom: Option<&str>) {
    let Some(custom) = custom.map(str::trim).filter(|s| !s.is_empty()) else {
        *data_root_override().write() = None;
        return;
    };
    let dir = PathBuf::from(custom);
    match std::fs::create_dir_all(&dir) {
        Ok(()) => *data_root_override().write() = Some(dir),
        Err(e) => {
            tracing::warn!(
                dir = %crate::secret_log::safe_path_for_log(&dir),
                error = %e,
                "Configured data root unavailable; falling back to default"
            );
            *data_root_override().write() = None;
        }
    }
}

/// Local-корень регенерируемых данных: эффективный корень (override либо
/// дефолт). Каталог не создаётся.
pub(crate) fn local_root() -> anyhow::Result<PathBuf> {
    data_root()
}

/// Корень транзиентных файлов: `%TEMP%\ttsbard`. Каталог не создаётся.
pub(crate) fn temp_root() -> PathBuf {
    std::env::temp_dir().join(APP_DIR_NAME)
}

/// Удалить содержимое каталога поэлементно; вернуть число удалённых записей.
/// Отдельная функция для тестируемости: `clean_temp_root` вызывает её
/// для [`temp_root`].
fn clean_dir_contents(dir: &Path) -> usize {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let result = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match result {
            Ok(()) => removed += 1,
            Err(e) => tracing::warn!(
                path = %crate::secret_log::safe_path_for_log(&path),
                error = %e,
                "Failed to clean transient file"
            ),
        }
    }
    removed
}

/// Стартовая зачистка транзиентных файлов прошлой сессии. Вызывать только
/// после захвата single-instance: у файлов в `temp_root` нет живых владельцев.
#[cfg(windows)]
pub(crate) fn clean_temp_root() {
    let root = temp_root();
    let removed = clean_dir_contents(&root);
    if removed > 0 {
        tracing::info!(
            count = removed,
            dir = %crate::secret_log::safe_path_for_log(&root),
            "Cleaned transient files from previous session"
        );
    }
}

/// Удалить каталог по точному пути, если он существует. Best-effort.
fn remove_dir_if_exists(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    match std::fs::remove_dir_all(dir) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(
                dir = %crate::secret_log::safe_path_for_log(dir),
                error = %e,
                "Failed to remove legacy directory"
            );
            false
        }
    }
}

/// Одноразовая очистка legacy-каталога временных файлов Telegram в Roaming
/// (точный путь `<config_root>/temp`, до ROADMAP-117 там жили скачивания).
#[cfg(windows)]
pub(crate) fn remove_legacy_roaming_temp() {
    let Ok(root) = config_root() else {
        return;
    };
    let legacy = root.join("temp");
    if remove_dir_if_exists(&legacy) {
        tracing::info!(
            dir = %crate::secret_log::safe_path_for_log(&legacy),
            "Removed legacy roaming temp directory"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_root_ends_with_app_dir() {
        let root = config_root().expect("config dir must resolve on CI hosts");
        assert!(root.ends_with(APP_DIR_NAME));
    }

    #[test]
    fn local_root_ends_with_app_dir() {
        let root = local_root().expect("cache dir must resolve on CI hosts");
        assert!(root.ends_with(APP_DIR_NAME));
    }

    #[test]
    fn default_data_root_ends_with_app_dir() {
        let root = default_data_root().expect("cache dir must resolve on CI hosts");
        assert!(root.ends_with(APP_DIR_NAME));
    }

    #[test]
    fn data_root_matches_default_without_override() {
        let effective = data_root().expect("data root must resolve");
        let default = default_data_root().expect("default root must resolve");
        assert_eq!(effective, default);
    }

    #[test]
    fn data_root_is_default_without_override() {
        assert!(data_root_is_default());
    }

    #[test]
    fn init_data_root_none_is_idempotent_reset() {
        init_data_root(None);
        assert!(data_root_is_default());
        init_data_root(Some(""));
        assert!(data_root_is_default());
        init_data_root(Some("   "));
        assert!(data_root_is_default());
    }

    #[test]
    fn temp_root_ends_with_app_dir() {
        assert!(temp_root().ends_with(APP_DIR_NAME));
    }

    #[test]
    fn clean_dir_contents_removes_files_and_subdirs() {
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-paths-test-{}",
            std::process::id()
        ));
        let subdir = dir.join("nested");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(dir.join("a.tmp"), b"x").unwrap();
        std::fs::write(subdir.join("b.tmp"), b"y").unwrap();

        assert_eq!(clean_dir_contents(&dir), 2);
        assert!(dir.exists());
        assert!(std::fs::read_dir(&dir).unwrap().next().is_none());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn clean_dir_contents_missing_dir_is_zero() {
        assert_eq!(clean_dir_contents(Path::new("Z:/definitely-missing-ttsbard")), 0);
    }

    #[test]
    fn remove_dir_if_exists_is_false_for_missing() {
        assert!(!remove_dir_if_exists(Path::new("Z:/definitely-missing-ttsbard")));
    }
}
