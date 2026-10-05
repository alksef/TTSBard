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

/// Один корень поиска пользовательских моделей (Piper, OCR, RUAccent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelSearchRoot {
    /// Каталог корня: cwd процесса или эффективный корень данных.
    pub path: PathBuf,
    /// True для эффективного корня данных («Данные программы»): единственный
    /// корень, где потребителю разрешено создавать каталоги и управляемые файлы.
    pub managed: bool,
}

/// Упорядоченные корни поиска пользовательских моделей (ROADMAP-117): cwd
/// процесса, затем эффективный корень данных (`storage.data_dir` либо дефолт).
/// Разрешение ничего не создаёт и не меняет cwd процесса. Ошибка получения
/// cwd теряет только первый корень. Повтор одного каталога — включая
/// канонические алиасы существующих путей на Windows — убирается с сохранением
/// признака управляемого корня.
pub(crate) fn model_search_roots() -> Vec<ModelSearchRoot> {
    let cwd = std::env::current_dir();
    if let Err(error) = &cwd {
        tracing::warn!(
            error = %error,
            "Failed to resolve process cwd; model search continues in the data root"
        );
    }
    let data_root = data_root();
    if let Err(error) = &data_root {
        tracing::warn!(
            error = %error,
            "Failed to resolve data root; model search continues in the cwd"
        );
    }
    model_search_roots_from(cwd.ok().as_deref(), data_root.ok())
}

/// Пути корней в том же порядке — для сканеров, которым нужны только пути.
pub(crate) fn model_search_root_paths() -> Vec<PathBuf> {
    model_search_roots()
        .into_iter()
        .map(|root| root.path)
        .collect()
}

/// Чистое ядро [`model_search_roots`] с инъекцией входов: тесты не трогают
/// глобальный cwd процесса. `None` для cwd моделирует ошибку получения и
/// теряет только первый корень.
pub(crate) fn model_search_roots_from(
    cwd: Option<&Path>,
    data_root: Option<PathBuf>,
) -> Vec<ModelSearchRoot> {
    let mut roots = Vec::new();
    if let Some(cwd) = cwd {
        push_model_root(&mut roots, cwd.to_path_buf(), false);
    }
    if let Some(data_root) = data_root {
        push_model_root(&mut roots, data_root, true);
    }
    roots
}

fn push_model_root(roots: &mut Vec<ModelSearchRoot>, path: PathBuf, managed: bool) {
    if let Some(existing) = roots
        .iter_mut()
        .find(|root| same_directory(&root.path, &path))
    {
        existing.managed |= managed;
        return;
    }
    roots.push(ModelSearchRoot { path, managed });
}

/// Обозначают ли два пути один каталог. Для существующих путей сравнение
/// выполняется после canonicalize, что на Windows снимает различия регистра,
/// префикса `\\?\` и коротких имён; для несуществующих остаётся текстовое
/// сравнение — без учёта регистра на Windows и точное на остальных ОС.
fn same_directory(a: &Path, b: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (a.canonicalize(), b.canonicalize()) {
        return same_path_text(&a, &b);
    }
    same_path_text(a, b)
}

fn same_path_text(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
    }
    #[cfg(not(windows))]
    {
        a == b
    }
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

    fn unique_model_roots_test_dir(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-model-roots-test-{}-{}-{}",
            std::process::id(),
            unique,
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn model_roots_order_is_cwd_then_data_root() {
        let cwd = unique_model_roots_test_dir("cwd");
        let data = unique_model_roots_test_dir("data");

        let roots = model_search_roots_from(Some(&cwd), Some(data.clone()));

        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].path, cwd);
        assert!(!roots[0].managed, "cwd is a read-only root");
        assert_eq!(roots[1].path, data);
        assert!(roots[1].managed, "data root is the managed root");

        std::fs::remove_dir_all(&cwd).ok();
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn model_roots_cwd_only_without_data_root() {
        let cwd = unique_model_roots_test_dir("cwd-only");

        let roots = model_search_roots_from(Some(&cwd), None);

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].path, cwd);
        assert!(!roots[0].managed);

        std::fs::remove_dir_all(&cwd).ok();
    }

    #[test]
    fn model_roots_failed_cwd_lookup_keeps_data_root() {
        let data = unique_model_roots_test_dir("data-only");

        let roots = model_search_roots_from(None, Some(data.clone()));

        assert_eq!(roots.len(), 1, "a failed cwd lookup drops only the cwd");
        assert_eq!(roots[0].path, data);
        assert!(roots[0].managed);

        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn model_roots_empty_when_both_lookups_fail() {
        assert!(model_search_roots_from(None, None).is_empty());
    }

    #[test]
    fn model_roots_identical_paths_collapse_to_one_managed_root() {
        let dir = unique_model_roots_test_dir("same");

        let roots = model_search_roots_from(Some(&dir), Some(dir.clone()));

        assert_eq!(roots.len(), 1, "cwd equal to the data root is scanned once");
        assert_eq!(roots[0].path, dir);
        assert!(roots[0].managed, "dedup must keep the managed flag");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn model_roots_canonical_alias_collapse() {
        let dir = unique_model_roots_test_dir("alias");
        let alias = dir.canonicalize().unwrap();

        let roots = model_search_roots_from(Some(&alias), Some(dir.clone()));

        assert_eq!(roots.len(), 1, "canonical alias must be deduplicated");
        assert!(roots[0].managed);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(windows)]
    #[test]
    fn model_roots_windows_case_only_paths_collapse() {
        let dir = unique_model_roots_test_dir("case");
        let upper: PathBuf = dir.to_string_lossy().to_uppercase().into();
        assert_ne!(upper, dir);

        let roots = model_search_roots_from(Some(&upper), Some(dir.clone()));

        assert_eq!(roots.len(), 1, "case-only Windows paths are one directory");
        assert!(roots[0].managed);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn model_roots_dedup_keeps_stored_path_unchanged() {
        let cwd = unique_model_roots_test_dir("stored");
        let data = unique_model_roots_test_dir("stored-data");

        let roots = model_search_roots_from(Some(&cwd), Some(data.clone()));

        assert_eq!(roots[0].path, cwd, "stored paths stay as resolved");
        assert_eq!(roots[1].path, data);

        std::fs::remove_dir_all(&cwd).ok();
        std::fs::remove_dir_all(&data).ok();
    }

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
        let dir = std::env::temp_dir().join(format!("ttsbard-paths-test-{}", std::process::id()));
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
        assert_eq!(
            clean_dir_contents(Path::new("Z:/definitely-missing-ttsbard")),
            0
        );
    }

    #[test]
    fn remove_dir_if_exists_is_false_for_missing() {
        assert!(!remove_dir_if_exists(Path::new(
            "Z:/definitely-missing-ttsbard"
        )));
    }
}
