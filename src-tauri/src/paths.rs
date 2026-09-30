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

const APP_DIR_NAME: &str = "ttsbard";

/// Roaming-корень данных: `%APPDATA%\ttsbard` (Windows), XDG config (Linux),
/// `~/Library/Application Support/ttsbard` (macOS).
pub(crate) fn config_root() -> anyhow::Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("Failed to resolve config directory")?
        .join(APP_DIR_NAME))
}

/// Local-корень регенерируемых данных: `%LOCALAPPDATA%\ttsbard` (Windows),
/// XDG cache (Linux), `~/Library/Caches/ttsbard` (macOS).
pub(crate) fn local_root() -> anyhow::Result<PathBuf> {
    Ok(dirs::cache_dir()
        .context("Failed to resolve local data directory")?
        .join(APP_DIR_NAME))
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
