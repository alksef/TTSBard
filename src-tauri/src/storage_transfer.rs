//! Безопасный перенос данных программы между корнями данных.
//!
//! Перенос копирует только управляемые каталоги `audio_cache` и `models`
//! (рекурсивно, кусками для точного прогресса внутри крупных моделей),
//! сохраняя неизвестные данные источника нетронутыми. Исходные файлы
//! удаляются лишь после успешного копирования и сохранения настроек и только
//! если их содержимое не изменилось с момента снимка.
//!
//! Валидация путей (абсолютность, вложенность/совпадение, config root,
//! symlink/junction/reparse) выполняется заново при каждой передаче:
//! preflight (`build_transfer_plan`) является advisory.

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use tracing::{info, warn};

pub(crate) const AUDIO_CACHE_DIR_NAME: &str = "audio_cache";
pub(crate) const MODELS_DIR_NAME: &str = "models";

const COPY_CHUNK_SIZE: usize = 1024 * 1024;

#[derive(Debug)]
pub(crate) struct ManualTransferRequired;

impl std::fmt::Display for ManualTransferRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Source and destination both contain data; manual transfer required")
    }
}

impl std::error::Error for ManualTransferRequired {}

/// Фазы переноса, публикуемые в событии `storage-transfer-progress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransferPhase {
    Preparing,
    Copying,
    Finalizing,
}

impl TransferPhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            TransferPhase::Preparing => "preparing",
            TransferPhase::Copying => "copying",
            TransferPhase::Finalizing => "finalizing",
        }
    }
}

#[derive(Debug)]
struct FileToCopy {
    rel: PathBuf,
    size: u64,
}

#[derive(Debug)]
struct CopyUnit {
    source_dir: PathBuf,
    target_dir: PathBuf,
    files: Vec<FileToCopy>,
}

/// Результат планирования (validation + scan). `total_bytes` используется
/// командой `prepare`.
#[derive(Debug)]
pub(crate) struct TransferPlan {
    target_root: PathBuf,
    pub(crate) total_bytes: u64,
    pub(crate) models_migrated: bool,
    units: Vec<CopyUnit>,
}

/// Результат выполнения переноса.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TransferOutcome {
    /// Были ли перенесены файлы моделей (живые инстансы могут держать старые пути).
    pub(crate) models_migrated: bool,
}

/// Спланировать перенос: провалидировать пути и посчитать объём данных.
///
/// `audio_cache_source` — эффективный каталог аудио-кеша (override либо
/// `<source_root>/audio_cache`); `legacy_audio_cache` — признак того, что
/// источник кеша является отдельным legacy-override.
pub(crate) fn build_transfer_plan(
    source_root: &Path,
    target_root: &Path,
    audio_cache_source: &Path,
    legacy_audio_cache: bool,
    config_root: &Path,
) -> Result<TransferPlan> {
    require_absolute(source_root, "source data root")?;
    require_absolute(target_root, "target data root")?;
    require_absolute(audio_cache_source, "audio cache source")?;
    require_absolute(config_root, "config root")?;

    ensure_no_reparse(source_root)?;
    ensure_no_reparse(target_root)?;
    ensure_no_reparse(audio_cache_source)?;

    let source_canon = canonical_or_nearest(source_root)?;
    let target_canon = canonical_or_nearest(target_root)?;
    let config_canon = canonical_or_nearest(config_root)?;

    let same_root = paths_equal(&source_canon, &target_canon);
    reject_overlap(&target_canon, &config_canon, "config root")?;

    let mut units = Vec::new();
    let mut models_migrated = false;
    let mut total_bytes = 0u64;

    if same_root {
        // Смена корня не требуется: значим только перенос legacy-кеша в корень.
        if legacy_audio_cache {
            let ac_canon = canonical_or_nearest(audio_cache_source)?;
            let ac_target = target_root.join(AUDIO_CACHE_DIR_NAME);
            let ac_target_canon = canonical_or_nearest(&ac_target)?;
            if !paths_equal(&ac_canon, &ac_target_canon) {
                reject_nested(&ac_canon, &ac_target_canon, "audio cache")?;
                let files = collect_files(audio_cache_source, &ac_target)?;
                total_bytes += files.iter().map(|f| f.size).sum::<u64>();
                units.push(CopyUnit {
                    source_dir: audio_cache_source.to_path_buf(),
                    target_dir: ac_target,
                    files,
                });
            }
        }
    } else {
        reject_nested(&source_canon, &target_canon, "data root")?;
        reject_overlap(&target_canon, &config_canon, "config root")?;

        let models_src = source_root.join(MODELS_DIR_NAME);
        let models_tgt = target_root.join(MODELS_DIR_NAME);
        let models_files = collect_files(&models_src, &models_tgt)?;
        if !models_files.is_empty() {
            models_migrated = true;
            total_bytes += models_files.iter().map(|f| f.size).sum::<u64>();
            units.push(CopyUnit {
                source_dir: models_src,
                target_dir: models_tgt,
                files: models_files,
            });
        }

        let ac_target = target_root.join(AUDIO_CACHE_DIR_NAME);
        let ac_canon = canonical_or_nearest(audio_cache_source)?;
        let ac_target_canon = canonical_or_nearest(&ac_target)?;
        reject_nested(&ac_canon, &ac_target_canon, "audio cache")?;
        let files = collect_files(audio_cache_source, &ac_target)?;
        total_bytes += files.iter().map(|f| f.size).sum::<u64>();
        units.push(CopyUnit {
            source_dir: audio_cache_source.to_path_buf(),
            target_dir: ac_target,
            files,
        });
    }

    Ok(TransferPlan {
        target_root: target_root.to_path_buf(),
        total_bytes,
        models_migrated,
        units,
    })
}

/// Выполнить перенос по готовому плану.
///
/// Caller holds `history::cache_io_lock()` from planning through runtime
/// publication, so background cache I/O cannot race with the transfer.
/// `persist` сохраняет настройки и вызывается только после успешного
/// копирования; провал копирования или сохранения удаляет только созданные
/// данным вызовом артефакты и оставляет источник пригодным для повторной
/// попытки.
pub(crate) fn execute_transfer(
    plan: TransferPlan,
    progress: &mut dyn FnMut(TransferPhase, u64, u64),
    persist: &mut dyn FnMut() -> Result<()>,
) -> Result<TransferOutcome> {
    let total = plan.total_bytes;
    progress(TransferPhase::Preparing, 0, total);

    let mut snapshot: Vec<(PathBuf, u64, u64)> = Vec::new();
    let mut created_files: Vec<PathBuf> = Vec::new();
    let mut created_dirs: Vec<PathBuf> = Vec::new();
    let mut completed: u64 = 0;

    let copy_result = (|| -> Result<()> {
        ensure_dir_created(&plan.target_root, &mut created_dirs)?;
        for unit in &plan.units {
            ensure_dir_created(&unit.target_dir, &mut created_dirs)?;
            for file in &unit.files {
                let src = unit.source_dir.join(&file.rel);
                let dst = unit.target_dir.join(&file.rel);
                if dst.exists() {
                    bail!("Destination already exists: {}", dst.display());
                }
                if let Some(parent) = dst.parent() {
                    ensure_dir_created(parent, &mut created_dirs)?;
                }
                ensure_no_reparse(&src)?;
                ensure_no_reparse(&dst)?;
                let mut writer = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&dst)
                    .with_context(|| format!("Failed to create {}", dst.display()))?;
                created_files.push(dst.clone());
                let (copied, file_hash) = copy_file_chunked(&src, &dst, &mut writer, |delta| {
                    completed += delta;
                    progress(TransferPhase::Copying, completed, total);
                })?;
                if copied != file.size {
                    bail!("Source changed during transfer: {}", src.display());
                }
                snapshot.push((src, file.size, file_hash));
            }
        }
        Ok(())
    })();

    if let Err(e) = copy_result {
        cleanup_created(&created_files, &created_dirs);
        return Err(e);
    }

    progress(TransferPhase::Finalizing, completed, total);

    if let Err(e) = persist() {
        cleanup_created(&created_files, &created_dirs);
        return Err(e);
    }

    // Удалить исходные файлы только если они не изменились с момента снимка.
    for (src, size, hash) in &snapshot {
        if source_unchanged(src, *size, *hash) {
            let _ = std::fs::remove_file(src);
        }
    }
    for unit in &plan.units {
        remove_empty_dirs_bottom_up(&unit.source_dir);
    }

    Ok(TransferOutcome {
        models_migrated: plan.models_migrated,
    })
}

// ============================ legacy models migration ============================

/// Одноразовый перенос legacy-моделей из Roaming (`<config_root>/models`) в
/// эффективный корень данных (`<data_root>/models`).
///
/// Переносит только каталог моделей (`piper`, `ocr`, `ruaccent`) и никогда не
/// трогает cwd или audio-кеш. Возвращает `true`, только когда перенос можно
/// считать завершённым: источник отсутствует либо все его файлы перенесены и
/// каталог источника удалён. Ошибки файловой системы, reparse-точки и
/// пересечение путей дают `false` — маркер не ставится и попытка повторяется
/// на следующем старте.
pub(crate) fn migrate_legacy_models_from_config() -> bool {
    let (Ok(config_root), Ok(data_root)) = (crate::paths::config_root(), crate::paths::data_root())
    else {
        warn!("Legacy models migration skipped: cannot resolve paths");
        return false;
    };
    migrate_legacy_models(
        &config_root.join(MODELS_DIR_NAME),
        &data_root.join(MODELS_DIR_NAME),
    )
}

/// Действие для одного файла модели после preflight.
enum ModelFileAction {
    /// Скопировать в назначение.
    Copy,
    /// Назначение уже содержит побайтово совпадающий файл — копировать не нужно.
    AlreadyPresent,
}

/// Ядро переноса legacy-моделей с явными путями (для тестируемости).
///
/// Файлы назначения никогда не перезаписываются. Все конфликты проверяются до
/// любых мутаций: различающийся файл назначения оставляет источник нетронутым
/// и даёт `false`; побайтово совпадающие файлы считаются уже перенесёнными
/// (безопасный повтор после частичного переноса). Оригиналы удаляются только
/// после проверенного копирования, и перенос считается завершённым лишь при
/// удалении (или отсутствии) каталога источника.
pub(crate) fn migrate_legacy_models(source: &Path, target: &Path) -> bool {
    if require_absolute(source, "legacy models source").is_err()
        || require_absolute(target, "models target").is_err()
    {
        warn!("Legacy models migration skipped: paths must be absolute without parent traversal");
        return false;
    }
    if ensure_no_reparse(source).is_err() {
        warn!(
            dir = %crate::secret_log::safe_path_for_log(source),
            "Legacy models migration skipped: source is a symlink/junction"
        );
        return false;
    }
    if ensure_no_reparse(target).is_err() {
        warn!(
            dir = %crate::secret_log::safe_path_for_log(target),
            "Legacy models migration skipped: target is a symlink/junction"
        );
        return false;
    }
    if reject_nested(source, target, "models").is_err() {
        warn!(
            source = %crate::secret_log::safe_path_for_log(source),
            target = %crate::secret_log::safe_path_for_log(target),
            "Legacy models migration skipped: overlapping paths"
        );
        return false;
    }

    // Отсутствие источника — завершённый перенос. Родитель проверяется на
    // существование каталога: на Windows файл в пути-предке тоже даёт NotFound
    // (ERROR_PATH_NOT_FOUND) и не должен считаться отсутствием источника.
    let meta = match std::fs::symlink_metadata(source) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let absent = confirmed_missing_directory(source);
            if absent {
                info!(
                    dir = %crate::secret_log::safe_path_for_log(source),
                    "Legacy models directory absent; migration complete"
                );
            } else {
                warn!(
                    dir = %crate::secret_log::safe_path_for_log(source),
                    "Legacy models parent is not a directory; migration incomplete"
                );
            }
            return absent;
        }
        Err(e) => {
            warn!(
                dir = %crate::secret_log::safe_path_for_log(source),
                error = %e,
                "Legacy models metadata unavailable; migration incomplete"
            );
            return false;
        }
    };
    if !meta.is_dir() {
        warn!(
            dir = %crate::secret_log::safe_path_for_log(source),
            "Legacy models source is not a directory; migration incomplete"
        );
        return false;
    }

    match std::fs::symlink_metadata(target) {
        Ok(target_meta) if !target_meta.is_dir() => {
            warn!(
                dir = %crate::secret_log::safe_path_for_log(target),
                "Legacy models target is not a directory; migration incomplete"
            );
            return false;
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            warn!(
                dir = %crate::secret_log::safe_path_for_log(target),
                error = %e,
                "Legacy models target unavailable; migration incomplete"
            );
            return false;
        }
    }

    let files = match collect_model_files(source) {
        Ok(files) => files,
        Err(e) => {
            warn!(
                dir = %crate::secret_log::safe_path_for_log(source),
                error = %e,
                "Legacy models scan failed; migration incomplete"
            );
            return false;
        }
    };

    // Preflight: определить действие для каждого файла до любых мутаций.
    let mut plan: Vec<(PathBuf, PathBuf, u64, ModelFileAction)> = Vec::new();
    for file in &files {
        let src = source.join(&file.rel);
        let dst = target.join(&file.rel);
        if ensure_no_reparse(&dst).is_err() {
            warn!("Legacy models migration incomplete: destination contains a reparse point");
            return false;
        }
        match std::fs::symlink_metadata(&dst) {
            Ok(dst_meta) => {
                if is_reparse_meta(&dst_meta) || !dst_meta.is_file() {
                    warn!(
                        file = %crate::secret_log::safe_path_for_log(&dst),
                        "Legacy models migration incomplete: destination exists and is not a regular file"
                    );
                    return false;
                }
                if files_equal(&src, &dst) {
                    plan.push((src, dst, file.size, ModelFileAction::AlreadyPresent));
                } else {
                    warn!(
                        file = %crate::secret_log::safe_path_for_log(&dst),
                        "Legacy models migration incomplete: destination differs"
                    );
                    return false;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                plan.push((src, dst, file.size, ModelFileAction::Copy));
            }
            Err(e) => {
                warn!(
                    file = %crate::secret_log::safe_path_for_log(&dst),
                    error = %e,
                    "Legacy models destination unavailable; migration incomplete"
                );
                return false;
            }
        }
    }

    let mut created_dirs: Vec<PathBuf> = Vec::new();
    let mut created_files: Vec<PathBuf> = Vec::new();
    let mut snapshot: Vec<(PathBuf, PathBuf, u64, u64)> = Vec::new();

    let copy_result = (|| -> Result<()> {
        ensure_dir_created(target, &mut created_dirs)?;
        for (src, dst, size, action) in &plan {
            match action {
                ModelFileAction::AlreadyPresent => {
                    let hash = hash_file(src).context("Failed to hash existing model")?;
                    snapshot.push((src.clone(), dst.clone(), *size, hash));
                }
                ModelFileAction::Copy => {
                    if let Some(parent) = dst.parent() {
                        ensure_dir_created(parent, &mut created_dirs)?;
                    }
                    ensure_no_reparse(src)?;
                    ensure_no_reparse(dst)?;
                    let mut writer = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(dst)
                        .with_context(|| "Failed to create destination model")?;
                    created_files.push(dst.clone());
                    let (copied, hash) = copy_file_chunked(src, dst, &mut writer, |_| {})?;
                    if copied != *size {
                        bail!("Source model changed during transfer");
                    }
                    snapshot.push((src.clone(), dst.clone(), *size, hash));
                }
            }
        }
        Ok(())
    })();

    if copy_result.is_err() {
        cleanup_created(&created_files, &created_dirs);
        warn!(
            dir = %crate::secret_log::safe_path_for_log(source),
            "Legacy models migration failed; will retry on next start"
        );
        return false;
    }

    // Удалить исходные файлы только если они не изменились с момента снимка.
    for (src, dst, size, hash) in &snapshot {
        if ensure_no_reparse(src).is_ok()
            && ensure_no_reparse(dst).is_ok()
            && source_unchanged(src, *size, *hash)
            && files_equal(src, dst)
        {
            let _ = std::fs::remove_file(src);
        }
    }
    remove_empty_dirs_bottom_up(source);

    match std::fs::symlink_metadata(source) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("Migrated legacy models out of Roaming");
            true
        }
        _ => {
            warn!(
                dir = %crate::secret_log::safe_path_for_log(source),
                "Legacy models partially migrated; remaining files retry on next start"
            );
            false
        }
    }
}

/// Confirm absence through missing ancestors, without confusing inaccessible
/// ancestors or a regular file in the path with an absent directory.
fn confirmed_missing_directory(path: &Path) -> bool {
    let mut current = path;
    loop {
        match std::fs::symlink_metadata(current) {
            Ok(meta) => return meta.is_dir() && !is_reparse_meta(&meta),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let Some(parent) = current.parent() else {
                    return false;
                };
                current = parent;
            }
            Err(_) => return false,
        }
    }
}

/// Собрать файлы моделей из `source` рекурсивно, отклоняя reparse-точки.
/// Возвращает относительные пути и размеры. Ошибки — чистые `io::Error` без
/// встроенных путей (безопасны для лога).
fn collect_model_files(source: &Path) -> std::io::Result<Vec<FileToCopy>> {
    let mut files = Vec::new();
    collect_model_files_recursive(source, source, &mut files)?;
    Ok(files)
}

fn collect_model_files_recursive(
    base: &Path,
    current: &Path,
    out: &mut Vec<FileToCopy>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)?;
        if is_reparse_meta(&meta) {
            return Err(std::io::Error::other(
                "symlink/junction in legacy models tree",
            ));
        }
        if meta.is_dir() {
            collect_model_files_recursive(base, &path, out)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(base)
                .map_err(|_| std::io::Error::other("failed to relativize models path"))?
                .to_path_buf();
            out.push(FileToCopy {
                rel,
                size: meta.len(),
            });
        }
    }
    Ok(())
}

/// Побайтовое сравнение содержимого двух существующих файлов.
fn files_equal(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
        return false;
    };
    if !ma.is_file() || !mb.is_file() || ma.len() != mb.len() {
        return false;
    }
    let (Ok(mut fa), Ok(mut fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else {
        return false;
    };
    let mut ba = [0u8; 64 * 1024];
    let mut bb = [0u8; 64 * 1024];
    loop {
        let na = match fa.read(&mut ba) {
            Ok(n) => n,
            Err(_) => return false,
        };
        let nb = match fb.read(&mut bb) {
            Ok(n) => n,
            Err(_) => return false,
        };
        if na != nb {
            return false;
        }
        if na == 0 {
            return true;
        }
        if ba[..na] != bb[..nb] {
            return false;
        }
    }
}

// ============================ path validation ============================

fn require_absolute(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute() {
        bail!("{} must be an absolute path: {}", label, path.display());
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        bail!("Path must not contain parent traversal: {}", path.display());
    }
    Ok(())
}

#[cfg(windows)]
fn normalize_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
        .trim_end_matches('\\')
        .to_string()
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        normalize_key(a) == normalize_key(b)
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

fn is_ancestor_of(parent: &Path, child: &Path) -> bool {
    #[cfg(windows)]
    {
        let p = normalize_key(parent);
        let c = normalize_key(child);
        c.starts_with(&p) && c.len() > p.len() && c[p.len()..].starts_with('\\')
    }
    #[cfg(not(windows))]
    {
        child.starts_with(parent) && child != parent
    }
}

fn reject_nested(a: &Path, b: &Path, label: &str) -> Result<()> {
    if paths_equal(a, b) {
        bail!(
            "{} source and target are the same path: {}",
            label,
            a.display()
        );
    }
    if is_ancestor_of(a, b) {
        bail!("{} target is inside the source: {}", label, b.display());
    }
    if is_ancestor_of(b, a) {
        bail!("{} source is inside the target: {}", label, a.display());
    }
    Ok(())
}

fn reject_overlap(a: &Path, b: &Path, label: &str) -> Result<()> {
    if paths_equal(a, b) || is_ancestor_of(a, b) || is_ancestor_of(b, a) {
        bail!("target overlaps {}: {}", label, b.display());
    }
    Ok(())
}

/// Канонизировать существующий путь; для несуществующего — канонизировать
/// ближайший существующий предок и дописать оставшиеся компоненты.
fn canonical_or_nearest(path: &Path) -> Result<PathBuf> {
    if let Ok(canon) = std::fs::canonicalize(path) {
        return Ok(canon);
    }
    let mut ancestor = path;
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match std::fs::canonicalize(ancestor) {
            Ok(canon) => {
                let mut result = canon;
                for part in tail.iter().rev() {
                    result.push(part);
                }
                return Ok(result);
            }
            Err(_) => match (ancestor.parent(), ancestor.file_name()) {
                (Some(parent), Some(name)) => {
                    tail.push(name.to_os_string());
                    ancestor = parent;
                }
                _ => bail!("Cannot resolve any existing ancestor of {}", path.display()),
            },
        }
    }
}

#[cfg(windows)]
fn is_reparse_meta(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    (meta.file_attributes() & 0x400) != 0
}

#[cfg(not(windows))]
fn is_reparse_meta(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

fn is_reparse_point(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| is_reparse_meta(&m))
        .unwrap_or(false)
}

/// Отклонить путь, если он или любой существующий компонент-предок является
/// symlink/junction/reparse point. Несуществующие хвостовые компоненты
/// пропускаются.
fn ensure_no_reparse(path: &Path) -> Result<()> {
    let mut current = Some(path);
    while let Some(p) = current {
        if is_reparse_point(p) {
            bail!(
                "Path contains a symlink/junction/reparse point: {}",
                p.display()
            );
        }
        current = p.parent();
    }
    Ok(())
}

// ============================ scanning / copying ============================

/// Собрать файлы для копирования из `source` в `target`.
///
/// Пустой источник допускает уже заполненное назначение. Если данные есть
/// с обеих сторон, требуется ручной перенос; автоматического объединения нет.
/// Любой reparse-point внутри переносимого дерева отклоняется.
fn collect_files(source: &Path, target: &Path) -> Result<Vec<FileToCopy>> {
    ensure_no_reparse(source)?;
    ensure_no_reparse(target)?;
    if target.exists() && !target.is_dir() {
        bail!("destination is not a directory: {}", target.display());
    }
    match std::fs::metadata(source) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => bail!("Source is not a directory: {}", source.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("Cannot read {}", source.display())),
    }
    let mut files = Vec::new();
    collect_files_recursive(source, source, &mut files)?;
    if !files.is_empty() && target.is_dir() && directory_has_data(target)? {
        return Err(ManualTransferRequired.into());
    }
    Ok(files)
}

fn directory_has_data(directory: &Path) -> Result<bool> {
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("Cannot read destination {}", directory.display()))?
    {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if is_reparse_meta(&metadata) || !metadata.is_dir() || directory_has_data(&path)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn collect_files_recursive(base: &Path, current: &Path, out: &mut Vec<FileToCopy>) -> Result<()> {
    let entries = std::fs::read_dir(current)
        .with_context(|| format!("Failed to read directory {}", current.display()))?;
    for entry in entries {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)
            .with_context(|| format!("Failed to stat {}", path.display()))?;
        if is_reparse_meta(&meta) {
            bail!("Refusing to transfer symlink/junction: {}", path.display());
        }
        if meta.is_dir() {
            collect_files_recursive(base, &path, out)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(base)
                .with_context(|| format!("Failed to relativize {}", path.display()))?
                .to_path_buf();
            out.push(FileToCopy {
                rel,
                size: meta.len(),
            });
        }
    }
    Ok(())
}

fn ensure_dir_created(dir: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    if dir.exists() {
        if !dir.is_dir() {
            bail!("path is not a directory: {}", dir.display());
        }
        return Ok(());
    }
    if let Some(parent) = dir.parent() {
        if !parent.exists() {
            ensure_dir_created(parent, created)?;
        }
    }
    std::fs::create_dir(dir)
        .with_context(|| format!("Failed to create directory {}", dir.display()))?;
    created.push(dir.to_path_buf());
    Ok(())
}

/// Потоковое копирование файла кусками; возвращает (bytes_copied, hash).
fn copy_file_chunked(
    src: &Path,
    dst: &Path,
    writer: &mut std::fs::File,
    mut on_chunk: impl FnMut(u64),
) -> Result<(u64, u64)> {
    let mut reader = std::fs::File::open(src)
        .with_context(|| format!("Failed to open source {}", src.display()))?;
    let mut hasher = DefaultHasher::new();
    let mut buf = vec![0u8; COPY_CHUNK_SIZE];
    let mut copied: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .with_context(|| format!("Failed to read {}", src.display()))?;
        if n == 0 {
            break;
        }
        hasher.write(&buf[..n]);
        writer
            .write_all(&buf[..n])
            .with_context(|| format!("Failed to write {}", dst.display()))?;
        copied += n as u64;
        on_chunk(n as u64);
    }
    writer
        .sync_all()
        .with_context(|| format!("Failed to flush {}", dst.display()))?;
    Ok((copied, hasher.finish()))
}

fn hash_file(path: &Path) -> Result<u64> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = DefaultHasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.write(&buf[..n]);
    }
    Ok(hasher.finish())
}

fn source_unchanged(path: &Path, expected_size: u64, expected_hash: u64) -> bool {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() && m.len() == expected_size => {}
        _ => return false,
    }
    hash_file(path).map(|h| h == expected_hash).unwrap_or(false)
}

fn cleanup_created(files: &[PathBuf], dirs: &[PathBuf]) {
    for f in files.iter().rev() {
        let _ = std::fs::remove_file(f);
    }
    for d in dirs.iter().rev() {
        let _ = std::fs::remove_dir(d);
    }
}

/// Best-effort удаление опустевших каталогов снизу вверх (не удаляет непустые).
fn remove_empty_dirs_bottom_up(dir: &Path) {
    fn walk(d: &Path) {
        if !d.is_dir() {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(d) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path);
                }
            }
        }
        if let Ok(mut it) = std::fs::read_dir(d) {
            if it.next().is_none() {
                let _ = std::fs::remove_dir(d);
            }
        }
    }
    walk(dir);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn tmp_dir(label: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-transfer-{}-{}-{}",
            label,
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_bytes(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, bytes).unwrap();
    }

    fn noop_progress(_phase: TransferPhase, _completed: u64, _total: u64) {}

    #[test]
    fn unrelated_roots_are_not_ancestors() {
        assert!(!is_ancestor_of(
            Path::new("C:/aaa"),
            Path::new("C:/bbb/child")
        ));
        assert!(is_ancestor_of(
            Path::new("C:/aaa"),
            Path::new("C:/aaa/child")
        ));
    }

    #[test]
    fn conflict_after_plan_preserves_both_files() {
        let root = tmp_dir("late-conflict");
        let src = source_with_data(&root);
        let dst = root.join("dst");
        let plan = build_transfer_plan(
            &src,
            &dst,
            &src.join("audio_cache"),
            false,
            &root.join("cfg"),
        )
        .unwrap();
        write_bytes(&dst.join("audio_cache/a.wav"), b"other");
        assert!(
            execute_transfer(plan, &mut noop_progress, &mut || panic!("must not persist")).is_err()
        );
        assert_eq!(
            std::fs::read(dst.join("audio_cache/a.wav")).unwrap(),
            b"other"
        );
        assert!(src.join("audio_cache/a.wav").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_source_size_rolls_back_partial_copy() {
        let root = tmp_dir("changed-size");
        let src = source_with_data(&root);
        let dst = root.join("dst");
        let plan = build_transfer_plan(
            &src,
            &dst,
            &src.join("audio_cache"),
            false,
            &root.join("cfg"),
        )
        .unwrap();
        write_bytes(&src.join("audio_cache/a.wav"), b"changed-size");
        assert!(
            execute_transfer(plan, &mut noop_progress, &mut || panic!("must not persist")).is_err()
        );
        assert!(!dst.exists());
        assert_eq!(
            std::fs::read(src.join("audio_cache/a.wav")).unwrap(),
            b"changed-size"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_root_legacy_conflict_is_rejected() {
        let root = tmp_dir("legacy-conflict");
        write_bytes(&root.join("data/audio_cache/a.wav"), b"new");
        write_bytes(&root.join("legacy/a.wav"), b"old");
        assert!(build_transfer_plan(
            &root.join("data"),
            &root.join("data"),
            &root.join("legacy"),
            true,
            &root.join("cfg")
        )
        .is_err());
        assert_eq!(std::fs::read(root.join("legacy/a.wav")).unwrap(), b"old");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn traversal_target_is_rejected() {
        let root = tmp_dir("traversal");
        assert!(build_transfer_plan(
            &root.join("data"),
            &root.join("other/../data/child"),
            &root.join("data/audio_cache"),
            false,
            &root.join("cfg")
        )
        .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Build a source root with managed `audio_cache` and `models` content.
    fn source_with_data(root: &Path) -> PathBuf {
        let src = root.join("src");
        write_bytes(&src.join("audio_cache").join("a.wav"), b"audio-a");
        write_bytes(
            &src.join("audio_cache").join("sub").join("b.wav"),
            b"audio-b",
        );
        write_bytes(
            &src.join("models").join("piper").join("m.onnx"),
            b"model-data",
        );
        src
    }

    #[test]
    fn plan_collects_managed_dirs_and_total_bytes() {
        let root = tmp_dir("plan");
        let src = source_with_data(&root);
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();

        assert!(plan.models_migrated);
        // a.wav(7) + b.wav(7) + m.onnx(10) = 24
        assert_eq!(plan.total_bytes, 24);
    }

    #[test]
    fn plan_missing_source_is_zero_bytes() {
        let root = tmp_dir("missing");
        let src = root.join("does-not-exist");
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        assert_eq!(plan.total_bytes, 0);
        assert!(!plan.models_migrated);
    }

    #[test]
    fn plan_rejects_relative_target() {
        let root = tmp_dir("relative");
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let err = build_transfer_plan(
            &src,
            Path::new("relative/target"),
            &src.join("audio_cache"),
            false,
            &config,
        )
        .unwrap_err();
        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn plan_rejects_target_inside_source() {
        let root = tmp_dir("nested");
        let src = root.join("src");
        let target = src.join("inner");
        let config = root.join("cfg");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&config).unwrap();

        let err = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("inside") || msg.contains("same"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn plan_rejects_config_root_target() {
        let root = tmp_dir("config");
        let src = root.join("src");
        let config = root.join("cfg");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&config).unwrap();

        let err = build_transfer_plan(&src, &config, &src.join("audio_cache"), false, &config)
            .unwrap_err();
        assert!(err.to_string().contains("config root"));
    }

    #[cfg(windows)]
    #[test]
    fn plan_rejects_junction_reparse_target() {
        use std::process::Command;
        let root = tmp_dir("junction");
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let real = root.join("real-target");
        std::fs::create_dir_all(&real).unwrap();
        let junction = root.join("junction-target");

        let created = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                real.to_str().unwrap(),
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !created {
            let _ = std::fs::remove_dir_all(&root);
            return;
        }

        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();
        let err = build_transfer_plan(&src, &junction, &src.join("audio_cache"), false, &config)
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("reparse") || msg.contains("symlink"),
            "unexpected error: {msg}"
        );

        let _ = std::fs::remove_dir(&junction);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn execute_copies_recursively_and_cleans_up_unchanged_source() {
        let root = tmp_dir("execute");
        let src = source_with_data(&root);
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        let mut persist_called = false;
        let outcome = execute_transfer(plan, &mut noop_progress, &mut || {
            persist_called = true;
            Ok(())
        })
        .unwrap();

        assert!(persist_called);
        assert!(outcome.models_migrated);
        assert!(target.join("audio_cache").join("a.wav").exists());
        assert!(target
            .join("audio_cache")
            .join("sub")
            .join("b.wav")
            .exists());
        assert!(target.join("models").join("piper").join("m.onnx").exists());
        // Source managed files removed (unchanged), unknown source data preserved.
        assert!(!src.join("audio_cache").join("a.wav").exists());
        assert!(!src.join("models").join("piper").join("m.onnx").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn execute_preserves_unknown_source_data() {
        let root = tmp_dir("preserve");
        let src = root.join("src");
        write_bytes(&src.join("audio_cache").join("a.wav"), b"audio");
        write_bytes(&src.join("notes.txt"), b"keep-me");
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        execute_transfer(plan, &mut noop_progress, &mut || Ok(())).unwrap();

        assert!(
            src.join("notes.txt").exists(),
            "unknown source data must be preserved"
        );
        assert!(target.join("audio_cache").join("a.wav").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nonconflicting_files_in_both_roots_require_manual_transfer() {
        let root = tmp_dir("nonempty-dst");
        let src = source_with_data(&root);
        let target = root.join("dst");
        write_bytes(
            &target.join("audio_cache").join("existing.wav"),
            b"occupied",
        );
        write_bytes(
            &target.join("models").join("ocr").join("existing.onnx"),
            b"model",
        );
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let error = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap_err();
        assert!(error.is::<ManualTransferRequired>());
        assert_eq!(
            std::fs::read(target.join("audio_cache/existing.wav")).unwrap(),
            b"occupied"
        );
        assert_eq!(
            std::fs::read(target.join("models/ocr/existing.onnx")).unwrap(),
            b"model"
        );
        assert!(!target.join("audio_cache/a.wav").exists());
        assert!(!target.join("models/piper/m.onnx").exists());
        assert!(src.join("audio_cache/a.wav").is_file());
        assert!(src.join("models/piper/m.onnx").is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn empty_source_can_switch_to_existing_data_root() {
        let root = tmp_dir("empty-to-existing");
        let src = root.join("src");
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(src.join("models/piper")).unwrap();
        std::fs::create_dir_all(src.join("audio_cache")).unwrap();
        write_bytes(
            &target.join("models/piper/existing.onnx"),
            b"existing model",
        );
        write_bytes(&target.join("audio_cache/existing.wav"), b"existing cache");
        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        assert_eq!(plan.total_bytes, 0);
        let mut persisted = false;
        let outcome = execute_transfer(plan, &mut noop_progress, &mut || {
            persisted = true;
            Ok(())
        })
        .unwrap();
        assert!(persisted);
        assert!(!outcome.models_migrated);
        assert_eq!(
            std::fs::read(target.join("models/piper/existing.onnx")).unwrap(),
            b"existing model"
        );
        assert_eq!(
            std::fs::read(target.join("audio_cache/existing.wav")).unwrap(),
            b"existing cache"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn empty_destination_subdirectories_do_not_require_manual_transfer() {
        let root = tmp_dir("empty-dst-subdirs");
        let src = source_with_data(&root);
        let target = root.join("dst");
        std::fs::create_dir_all(target.join("models/piper")).unwrap();
        std::fs::create_dir_all(target.join("audio_cache/empty")).unwrap();
        let plan = build_transfer_plan(
            &src,
            &target,
            &src.join("audio_cache"),
            false,
            &root.join("cfg"),
        )
        .unwrap();
        execute_transfer(plan, &mut noop_progress, &mut || Ok(())).unwrap();
        assert!(target.join("models/piper/m.onnx").is_file());
        assert!(target.join("audio_cache/a.wav").is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn categories_are_checked_independently() {
        let root = tmp_dir("independent-categories");
        let src = root.join("src");
        let target = root.join("dst");
        write_bytes(&src.join("models/piper/new.onnx"), b"model");
        write_bytes(&target.join("audio_cache/existing.wav"), b"cache");
        let plan = build_transfer_plan(
            &src,
            &target,
            &src.join("audio_cache"),
            false,
            &root.join("cfg"),
        )
        .unwrap();
        execute_transfer(plan, &mut noop_progress, &mut || Ok(())).unwrap();
        assert_eq!(
            std::fs::read(target.join("models/piper/new.onnx")).unwrap(),
            b"model"
        );
        assert_eq!(
            std::fs::read(target.join("audio_cache/existing.wav")).unwrap(),
            b"cache"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn conflicting_destination_is_rejected_without_changing_either_root() {
        let root = tmp_dir("conflicting-dst");
        let src = source_with_data(&root);
        let target = root.join("dst");
        let config = root.join("cfg");
        write_bytes(&target.join("audio_cache/a.wav"), b"keep target");
        let source_before = std::fs::read(src.join("audio_cache/a.wav")).unwrap();
        let err = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap_err();
        assert!(err.is::<ManualTransferRequired>());
        assert_eq!(
            std::fs::read(target.join("audio_cache/a.wav")).unwrap(),
            b"keep target"
        );
        assert_eq!(
            std::fs::read(src.join("audio_cache/a.wav")).unwrap(),
            source_before
        );
        assert!(!target.join("models").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rollback_preserves_existing_destination_files() {
        let root = tmp_dir("existing-rollback");
        let src = source_with_data(&root);
        let target = root.join("dst");
        let config = root.join("cfg");
        write_bytes(&target.join("notes.txt"), b"keep notes");
        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        let result = execute_transfer(plan, &mut noop_progress, &mut || {
            anyhow::bail!("save failed")
        });
        assert!(result.is_err());
        assert_eq!(
            std::fs::read(target.join("notes.txt")).unwrap(),
            b"keep notes"
        );
        assert!(!target.join("audio_cache/a.wav").exists());
        assert!(!target.join("models/piper").exists());
        assert!(src.join("audio_cache/a.wav").is_file());
        assert!(src.join("models/piper/m.onnx").is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn execute_rolls_back_created_artifacts_on_persist_failure() {
        let root = tmp_dir("rollback");
        let src = source_with_data(&root);
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        let err = execute_transfer(plan, &mut noop_progress, &mut || {
            anyhow::bail!("simulated save failure")
        })
        .unwrap_err();
        assert!(err.to_string().contains("simulated save failure"));

        // Destination artifacts removed; source untouched.
        assert!(!target.join("audio_cache").exists());
        assert!(!target.join("models").exists());
        assert!(src.join("audio_cache").join("a.wav").exists());
        assert!(src.join("models").join("piper").join("m.onnx").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn execute_retry_works_after_rollback() {
        let root = tmp_dir("retry");
        let src = source_with_data(&root);
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        let _ = execute_transfer(plan, &mut noop_progress, &mut || anyhow::bail!("boom"));
        assert!(src.join("audio_cache").join("a.wav").exists());

        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        execute_transfer(plan, &mut noop_progress, &mut || Ok(())).unwrap();
        assert!(target.join("audio_cache").join("a.wav").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn execute_keeps_changed_source_file() {
        let root = tmp_dir("changed-source");
        let src = root.join("src");
        write_bytes(&src.join("audio_cache").join("a.wav"), b"audio-a");
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        // Simulate a concurrent change by mutating the file after the plan is
        // built but before cleanup: easiest is to change content via a persist
        // callback that rewrites the source.
        execute_transfer(plan, &mut noop_progress, &mut || {
            std::fs::write(src.join("audio_cache").join("a.wav"), b"CHANGED").unwrap();
            Ok(())
        })
        .unwrap();

        // The source file changed from its snapshot, so it must be preserved.
        assert!(src.join("audio_cache").join("a.wav").exists());
        assert_eq!(
            std::fs::read(src.join("audio_cache").join("a.wav")).unwrap(),
            b"CHANGED"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn same_root_legacy_cache_requires_manual_transfer_when_both_have_data() {
        let root = tmp_dir("same-root");
        let data_root = root.join("data");
        let legacy = root.join("legacy-cache");
        write_bytes(&legacy.join("a.wav"), b"legacy-a");
        write_bytes(&data_root.join("audio_cache").join("b.wav"), b"existing-b");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let error =
            build_transfer_plan(&data_root, &data_root, &legacy, true, &config).unwrap_err();
        assert!(error.is::<ManualTransferRequired>());
        assert!(!data_root.join("audio_cache").join("a.wav").exists());
        assert!(data_root.join("audio_cache").join("b.wav").exists());
        assert!(legacy.join("a.wav").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn execute_same_root_no_legacy_is_noop() {
        let root = tmp_dir("same-root-noop");
        let data_root = root.join("data");
        write_bytes(&data_root.join("audio_cache").join("a.wav"), b"a");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan = build_transfer_plan(
            &data_root,
            &data_root,
            &data_root.join("audio_cache"),
            false,
            &config,
        )
        .unwrap();
        assert_eq!(plan.total_bytes, 0);
        execute_transfer(plan, &mut noop_progress, &mut || Ok(())).unwrap();
        assert!(data_root.join("audio_cache").join("a.wav").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn progress_reports_real_bytes() {
        let root = tmp_dir("progress");
        let src = source_with_data(&root);
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan =
            build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config).unwrap();
        let total = plan.total_bytes;

        let mut events: Vec<(TransferPhase, u64, u64)> = Vec::new();
        execute_transfer(
            plan,
            &mut |phase, completed, t| {
                events.push((phase, completed, t));
            },
            &mut || Ok(()),
        )
        .unwrap();

        assert_eq!(events.first().unwrap().0, TransferPhase::Preparing);
        assert!(events.iter().any(|(p, _, _)| *p == TransferPhase::Copying));
        assert!(events
            .iter()
            .any(|(p, _, _)| *p == TransferPhase::Finalizing));
        let max_completed = events.iter().map(|(_, c, _)| *c).max().unwrap();
        assert_eq!(max_completed, total);

        let _ = std::fs::remove_dir_all(&root);
    }

    // ==================== legacy models migration ====================

    #[test]
    fn legacy_model_missing_source_completes() {
        let root = tmp_dir("legacy-model-missing");
        let src = root.join("does-not-exist");
        let dst = root.join("data").join("models");

        assert!(migrate_legacy_models(&src, &dst));
        assert!(!dst.exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn legacy_model_missing_config_root_completes() {
        let root = tmp_dir("legacy-model-no-config");
        let src = root.join("missing-config/models");
        let dst = root.join("data/models");
        assert!(migrate_legacy_models(&src, &dst));
        assert!(!dst.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_model_file_in_source_ancestor_is_not_absence() {
        let root = tmp_dir("legacy-model-file-ancestor");
        write_bytes(&root.join("config"), b"not a directory");
        assert!(!migrate_legacy_models(
            &root.join("config/models"),
            &root.join("data/models")
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_model_transfers_piper_ocr_ruaccent() {
        let root = tmp_dir("legacy-model-nested");
        let src = root.join("models");
        let dst = root.join("data").join("models");
        write_bytes(&src.join("piper/en.onnx"), b"piper-en");
        write_bytes(&src.join("piper/en.onnx.json"), b"{}");
        write_bytes(&src.join("ocr/rus.pack"), b"ocr-rus");
        write_bytes(&src.join("ruaccent/pack.bin"), b"ruaccent");
        write_bytes(&root.join("audio_cache/a.wav"), b"leave-cache");

        assert!(migrate_legacy_models(&src, &dst));

        assert!(dst.join("piper/en.onnx").exists());
        assert!(dst.join("piper/en.onnx.json").exists());
        assert!(dst.join("ocr/rus.pack").exists());
        assert!(dst.join("ruaccent/pack.bin").exists());
        assert!(!src.exists(), "source must be removed after full transfer");
        assert_eq!(
            std::fs::read(root.join("audio_cache/a.wav")).unwrap(),
            b"leave-cache"
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn legacy_model_custom_destination() {
        let root = tmp_dir("legacy-model-custom");
        let src = root.join("models");
        let dst = root.join("custom").join("nested").join("models");
        write_bytes(&src.join("piper/m.onnx"), b"m");

        assert!(migrate_legacy_models(&src, &dst));
        assert!(dst.join("piper/m.onnx").exists());
        assert!(!src.exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn legacy_model_differing_collision_no_mutation() {
        let root = tmp_dir("legacy-model-conflict");
        let src = root.join("models");
        let dst = root.join("data").join("models");
        write_bytes(&src.join("piper/m.onnx"), b"source-version");
        write_bytes(&dst.join("piper/m.onnx"), b"destination-version");

        assert!(!migrate_legacy_models(&src, &dst));

        assert_eq!(
            std::fs::read(src.join("piper/m.onnx")).unwrap(),
            b"source-version"
        );
        assert_eq!(
            std::fs::read(dst.join("piper/m.onnx")).unwrap(),
            b"destination-version"
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn legacy_model_identical_existing_files_allow_safe_retry() {
        let root = tmp_dir("legacy-model-retry");
        let src = root.join("models");
        let dst = root.join("data").join("models");
        write_bytes(&src.join("piper/m.onnx"), b"model-data");
        write_bytes(&src.join("ocr/o.onnx"), b"ocr-data");
        // Simulate a previous attempt that copied piper but crashed before
        // removing the source.
        write_bytes(&dst.join("piper/m.onnx"), b"model-data");

        assert!(migrate_legacy_models(&src, &dst));

        assert!(dst.join("piper/m.onnx").exists());
        assert!(dst.join("ocr/o.onnx").exists());
        assert!(
            !src.exists(),
            "identical files count as already transferred"
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn legacy_model_empty_source_completes() {
        let root = tmp_dir("legacy-model-empty");
        let src = root.join("models");
        std::fs::create_dir_all(&src).unwrap();
        let dst = root.join("data").join("models");

        assert!(migrate_legacy_models(&src, &dst));
        assert!(!src.exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn legacy_model_relative_source_rejected() {
        let root = tmp_dir("legacy-model-relative");
        let dst = root.join("data").join("models");

        assert!(!migrate_legacy_models(Path::new("relative/models"), &dst));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn legacy_model_overlapping_paths_rejected() {
        let root = tmp_dir("legacy-model-overlap");
        let src = root.join("models");
        std::fs::create_dir_all(&src).unwrap();

        assert!(
            !migrate_legacy_models(&src, &src),
            "same path must be rejected"
        );
        assert!(
            !migrate_legacy_models(&src, &src.join("nested")),
            "target inside source must be rejected"
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn legacy_model_rejects_junction_source() {
        use std::process::Command;
        let root = tmp_dir("legacy-model-junction");
        let real = root.join("real");
        std::fs::create_dir_all(&real).unwrap();
        let junction = root.join("junction");

        let created = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                real.to_str().unwrap(),
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !created {
            let _ = std::fs::remove_dir_all(&root);
            return;
        }

        let dst = root.join("data").join("models");
        assert!(!migrate_legacy_models(&junction, &dst));

        let _ = std::fs::remove_dir(&junction);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn legacy_model_rejects_nested_destination_junction_even_if_identical() {
        let root = tmp_dir("legacy-model-target-junction");
        let src = root.join("source").join("models");
        let dst = root.join("target").join("models");
        let real = root.join("external");
        write_bytes(&src.join("piper/m.onnx"), b"model");
        write_bytes(&real.join("m.onnx"), b"model");
        std::fs::create_dir_all(&dst).unwrap();
        let junction = dst.join("piper");
        let created = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                real.to_str().unwrap(),
            ])
            .status()
            .unwrap()
            .success();
        assert!(
            created,
            "junction creation is required for this regression test"
        );
        assert!(!migrate_legacy_models(&src, &dst));
        assert_eq!(std::fs::read(src.join("piper/m.onnx")).unwrap(), b"model");
        assert_eq!(std::fs::read(real.join("m.onnx")).unwrap(), b"model");
        std::fs::remove_dir(junction).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
