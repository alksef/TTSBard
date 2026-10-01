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

pub(crate) const AUDIO_CACHE_DIR_NAME: &str = "audio_cache";
pub(crate) const MODELS_DIR_NAME: &str = "models";

const COPY_CHUNK_SIZE: usize = 1024 * 1024;

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
                let files = collect_files(audio_cache_source, &ac_target, false)?;
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
        let models_files = collect_files(&models_src, &models_tgt, true)?;
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
        let files = collect_files(audio_cache_source, &ac_target, true)?;
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
                let mut writer = std::fs::OpenOptions::new().write(true).create_new(true)
                    .open(&dst).with_context(|| format!("Failed to create {}", dst.display()))?;
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

// ============================ path validation ============================

fn require_absolute(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute() {
        bail!("{} must be an absolute path: {}", label, path.display());
    }
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
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
        bail!("{} source and target are the same path: {}", label, a.display());
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

fn dir_nonempty(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut it) => it.next().is_some(),
        Err(_) => false,
    }
}

/// Собрать файлы для копирования из `source` в `target`.
///
/// `reject_nonempty_target` отклоняет непустой каталог-назначение (защита от
/// молчаливой перезаписи) — для свежего корня. Файлы с уже существующим
/// назначением пропускаются (никогда не перезаписываются). Любой reparse-point
/// внутри дерева отклоняется.
fn collect_files(source: &Path, target: &Path, reject_nonempty_target: bool) -> Result<Vec<FileToCopy>> {
    ensure_no_reparse(source)?;
    ensure_no_reparse(target)?;
    if reject_nonempty_target {
        if target.exists() && !target.is_dir() {
            bail!("destination is not a directory: {}", target.display());
        }
        if target.is_dir() && dir_nonempty(target) {
            bail!("destination is not empty: {}", target.display());
        }
    }
    match std::fs::metadata(source) {
        Ok(m) if m.is_dir() => {},
        Ok(_) => bail!("Source is not a directory: {}", source.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("Cannot read {}", source.display())),
    }
    let mut files = Vec::new();
    collect_files_recursive(source, target, source, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    base: &Path,
    target_base: &Path,
    current: &Path,
    out: &mut Vec<FileToCopy>,
) -> Result<()> {
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
            collect_files_recursive(base, target_base, &path, out)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(base)
                .with_context(|| format!("Failed to relativize {}", path.display()))?
                .to_path_buf();
            let dest = target_base.join(&rel);
            if dest.exists() {
                bail!("Destination already exists: {}", dest.display());
            }
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
    hash_file(path)
        .map(|h| h == expected_hash)
        .unwrap_or(false)
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
        assert!(!is_ancestor_of(Path::new("C:/aaa"), Path::new("C:/bbb/child")));
        assert!(is_ancestor_of(Path::new("C:/aaa"), Path::new("C:/aaa/child")));
    }

    #[test]
    fn conflict_after_plan_preserves_both_files() {
        let root = tmp_dir("late-conflict");
        let src = source_with_data(&root);
        let dst = root.join("dst");
        let plan = build_transfer_plan(&src, &dst, &src.join("audio_cache"), false, &root.join("cfg")).unwrap();
        write_bytes(&dst.join("audio_cache/a.wav"), b"other");
        assert!(execute_transfer(plan, &mut noop_progress, &mut || panic!("must not persist")).is_err());
        assert_eq!(std::fs::read(dst.join("audio_cache/a.wav")).unwrap(), b"other");
        assert!(src.join("audio_cache/a.wav").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_source_size_rolls_back_partial_copy() {
        let root = tmp_dir("changed-size");
        let src = source_with_data(&root);
        let dst = root.join("dst");
        let plan = build_transfer_plan(&src, &dst, &src.join("audio_cache"), false, &root.join("cfg")).unwrap();
        write_bytes(&src.join("audio_cache/a.wav"), b"changed-size");
        assert!(execute_transfer(plan, &mut noop_progress, &mut || panic!("must not persist")).is_err());
        assert!(!dst.exists());
        assert_eq!(std::fs::read(src.join("audio_cache/a.wav")).unwrap(), b"changed-size");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_root_legacy_conflict_is_rejected() {
        let root = tmp_dir("legacy-conflict");
        write_bytes(&root.join("data/audio_cache/a.wav"), b"new");
        write_bytes(&root.join("legacy/a.wav"), b"old");
        assert!(build_transfer_plan(&root.join("data"), &root.join("data"), &root.join("legacy"), true, &root.join("cfg")).is_err());
        assert_eq!(std::fs::read(root.join("legacy/a.wav")).unwrap(), b"old");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn traversal_target_is_rejected() {
        let root = tmp_dir("traversal");
        assert!(build_transfer_plan(&root.join("data"), &root.join("other/../data/child"), &root.join("data/audio_cache"), false, &root.join("cfg")).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Build a source root with managed `audio_cache` and `models` content.
    fn source_with_data(root: &Path) -> PathBuf {
        let src = root.join("src");
        write_bytes(&src.join("audio_cache").join("a.wav"), b"audio-a");
        write_bytes(&src.join("audio_cache").join("sub").join("b.wav"), b"audio-b");
        write_bytes(&src.join("models").join("piper").join("m.onnx"), b"model-data");
        src
    }

    #[test]
    fn plan_collects_managed_dirs_and_total_bytes() {
        let root = tmp_dir("plan");
        let src = source_with_data(&root);
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan = build_transfer_plan(
            &src,
            &target,
            &src.join("audio_cache"),
            false,
            &config,
        )
        .unwrap();

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

        let plan = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap();
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

        let err = build_transfer_plan(&src, Path::new("relative/target"), &src.join("audio_cache"), false, &config)
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
        assert!(msg.contains("inside") || msg.contains("same"), "unexpected: {msg}");
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
        let err =
            build_transfer_plan(&src, &junction, &src.join("audio_cache"), false, &config)
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

        let plan = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap();
        let mut persist_called = false;
        let outcome = execute_transfer(plan, &mut noop_progress, &mut || {
            persist_called = true;
            Ok(())
        })
        .unwrap();

        assert!(persist_called);
        assert!(outcome.models_migrated);
        assert!(target.join("audio_cache").join("a.wav").exists());
        assert!(target.join("audio_cache").join("sub").join("b.wav").exists());
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

        let plan = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap();
        execute_transfer(plan, &mut noop_progress, &mut || Ok(())).unwrap();

        assert!(src.join("notes.txt").exists(), "unknown source data must be preserved");
        assert!(target.join("audio_cache").join("a.wav").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn execute_rejects_nonempty_destination() {
        let root = tmp_dir("nonempty-dst");
        let src = source_with_data(&root);
        let target = root.join("dst");
        write_bytes(&target.join("audio_cache").join("existing.wav"), b"occupied");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let err = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap_err();
        assert!(err.to_string().contains("not empty"));
    }

    #[test]
    fn execute_rolls_back_created_artifacts_on_persist_failure() {
        let root = tmp_dir("rollback");
        let src = source_with_data(&root);
        let target = root.join("dst");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap();
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

        let plan = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap();
        let _ = execute_transfer(plan, &mut noop_progress, &mut || {
            anyhow::bail!("boom")
        });
        assert!(src.join("audio_cache").join("a.wav").exists());

        let plan = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap();
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

        let plan = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap();
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
    fn execute_same_root_legacy_cache_merges() {
        let root = tmp_dir("same-root");
        let data_root = root.join("data");
        let legacy = root.join("legacy-cache");
        write_bytes(&legacy.join("a.wav"), b"legacy-a");
        write_bytes(&data_root.join("audio_cache").join("b.wav"), b"existing-b");
        let config = root.join("cfg");
        std::fs::create_dir_all(&config).unwrap();

        let plan = build_transfer_plan(&data_root, &data_root, &legacy, true, &config).unwrap();
        assert_eq!(plan.total_bytes, 8);

        execute_transfer(plan, &mut noop_progress, &mut || Ok(())).unwrap();

        // Legacy file migrated into the default cache; existing file preserved.
        assert!(data_root.join("audio_cache").join("a.wav").exists());
        assert!(data_root.join("audio_cache").join("b.wav").exists());
        assert!(!legacy.join("a.wav").exists(), "migrated legacy file removed");

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

        let plan = build_transfer_plan(&src, &target, &src.join("audio_cache"), false, &config)
            .unwrap();
        let total = plan.total_bytes;

        let mut events: Vec<(TransferPhase, u64, u64)> = Vec::new();
        execute_transfer(plan, &mut |phase, completed, t| {
            events.push((phase, completed, t));
        }, &mut || Ok(()))
        .unwrap();

        assert_eq!(events.first().unwrap().0, TransferPhase::Preparing);
        assert!(events.iter().any(|(p, _, _)| *p == TransferPhase::Copying));
        assert!(events.iter().any(|(p, _, _)| *p == TransferPhase::Finalizing));
        let max_completed = events
            .iter()
            .map(|(_, c, _)| *c)
            .max()
            .unwrap();
        assert_eq!(max_completed, total);

        let _ = std::fs::remove_dir_all(&root);
    }
}
