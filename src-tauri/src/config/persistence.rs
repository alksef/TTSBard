//! Shared JSON persistence primitives
//!
//! Provides a global write lock and atomic temp-file write/replace strategy
//! for all JSON config files managed by this module.

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static JSON_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn config_write_lock() -> &'static Mutex<()> {
    JSON_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

/// Write JSON content atomically to a file using temp-file + rename strategy.
///
/// Creates a temp file in the same directory as the target, writes + flushes,
/// then renames it over the target. If rename fails, removes the target and retries.
/// The caller is responsible for holding `config_write_lock()` to prevent concurrent
/// writers of the same logical config.
pub fn write_json_atomically(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().context("Path must have a parent directory")?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is before UNIX_EPOCH")?
        .as_nanos();
    let tmp_path = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config.json"),
        stamp
    ));

    {
        let mut file = fs::File::create(&tmp_path)
            .with_context(|| format!("Failed to create temp file at {:?}", tmp_path))?;
        file.write_all(content.as_bytes())
            .with_context(|| format!("Failed to write temp file at {:?}", tmp_path))?;
        file.sync_all()
            .with_context(|| format!("Failed to flush temp file at {:?}", tmp_path))?;
    }

    if let Err(rename_error) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(path);
        fs::rename(&tmp_path, path).with_context(|| {
            format!(
                "Failed to replace file {:?} with temp file {:?}: {}",
                path, tmp_path, rename_error
            )
        })?;
    }

    Ok(())
}

/// Attempt to recover from a corrupted JSON config file.
///
/// Under the shared write lock, renames the original file to a backup with
/// a nanosecond timestamp suffix, writes the supplied defaults atomically,
/// and returns a clone of the defaults.
///
/// If the backup path already exists (extremely unlikely with nanosecond
/// granularity), retries with a 1ms delay to guarantee uniqueness and avoid
/// overwriting a previous backup.
///
/// Returns a clear error if the rename fails, preserving the original file on
/// disk — the source file is never silently deleted.
pub fn recover_corrupted_json<T: Serialize + Clone>(path: &Path, defaults: &T) -> Result<T> {
    let _guard = config_write_lock().lock();

    let backup_path = loop {
        let bp = make_backup_path(path)?;
        if !bp.exists() {
            break bp;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };

    fs::rename(path, &backup_path).with_context(|| {
        format!(
            "Failed to backup corrupted config file {:?} to {:?}. Original file is preserved.",
            path, backup_path
        )
    })?;

    let content = serde_json::to_string_pretty(defaults)
        .context("Failed to serialize default config during corruption recovery")?;
    write_json_atomically(path, &content)?;

    Ok(defaults.clone())
}

fn make_backup_path(path: &Path) -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is before UNIX_EPOCH")?
        .as_nanos();
    let parent = path.parent().context("Path must have a parent directory")?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("config");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("json");
    Ok(parent.join(format!("{}.bak.{}.{}", stem, stamp, ext)))
}
