//! Shared JSON persistence primitives
//!
//! Provides a global write lock and atomic temp-file write/replace strategy
//! for all JSON config files managed by this module.

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static JSON_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn config_write_lock() -> &'static Mutex<()> {
    JSON_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

fn make_temp_path(parent: &Path, path: &Path) -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is before UNIX_EPOCH")?
        .as_nanos();
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let base = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    Ok(parent.join(format!(".{}.{}.{}.tmp", base, stamp, seq)))
}

/// Atomically replace the file at `path` with `bytes`.
///
/// Writes the bytes to a same-directory temporary file, flushes it, then renames
/// the temporary file over the destination in a single step. The existing
/// destination is never deleted before replacement; if the rename fails, the
/// temporary file is removed and the destination is left untouched.
pub fn replace_file_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Path must have a parent directory")?;
    let tmp_path = make_temp_path(parent, path)?;

    let write_result = (|| -> Result<()> {
        {
            let mut file = fs::File::create(&tmp_path)
                .with_context(|| format!("Failed to create temp file at {:?}", tmp_path))?;
            file.write_all(bytes)
                .with_context(|| format!("Failed to write temp file at {:?}", tmp_path))?;
            file.sync_all()
                .with_context(|| format!("Failed to flush temp file at {:?}", tmp_path))?;
        }

        replace_temp_file(&tmp_path, path).with_context(|| {
            format!(
                "Failed to replace file {:?} with temp file {:?}",
                path, tmp_path
            )
        })?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }

    write_result
}

#[cfg(windows)]
fn replace_temp_file(tmp_path: &Path, path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return fs::rename(tmp_path, path);
    }

    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Storage::FileSystem::{ReplaceFileW, REPLACE_FILE_FLAGS};

    let target = HSTRING::from(path.as_os_str().to_string_lossy().as_ref());
    let replacement = HSTRING::from(tmp_path.as_os_str().to_string_lossy().as_ref());
    unsafe {
        ReplaceFileW(
            &target,
            &replacement,
            PCWSTR::null(),
            REPLACE_FILE_FLAGS(0),
            None,
            None,
        )
    }
    .map_err(|error| std::io::Error::other(error.to_string()))
}

#[cfg(not(windows))]
fn replace_temp_file(tmp_path: &Path, path: &Path) -> std::io::Result<()> {
    fs::rename(tmp_path, path)
}

/// Write JSON content atomically to a file via [`replace_file_atomically`].
///
/// The caller is responsible for holding `config_write_lock()` to prevent
/// concurrent writers of the same logical config.
pub fn write_json_atomically(path: &Path, content: &str) -> Result<()> {
    replace_file_atomically(path, content.as_bytes())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    struct TestConfig {
        name: String,
        count: u32,
    }

    fn tmp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-persist-{}-{}-{}",
            label,
            std::process::id(),
            stamp
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    // ---- write_json_atomically ----

    #[test]
    fn write_atomic_creates_new_file_with_exact_content() {
        let dir = tmp_dir("new-file");
        let path = dir.join("config.json");
        let content = r#"{"name":"test","count":42}"#;

        write_json_atomically(&path, content).unwrap();

        let actual = fs::read_to_string(&path).unwrap();
        assert_eq!(actual, content);
        cleanup(&dir);
    }

    #[test]
    fn write_atomic_replaces_existing_content() {
        let dir = tmp_dir("replace");
        let path = dir.join("config.json");
        fs::write(&path, "old data").unwrap();

        let new_content = r#"{"name":"replaced","count":99}"#;
        write_json_atomically(&path, new_content).unwrap();

        let actual = fs::read_to_string(&path).unwrap();
        assert_eq!(actual, new_content);
        cleanup(&dir);
    }

    #[test]
    fn write_atomic_leaves_no_tmp_file() {
        let dir = tmp_dir("no-tmp");
        let path = dir.join("config.json");

        write_json_atomically(&path, "test content").unwrap();

        let tmp_files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".tmp")))
            .collect();
        assert!(
            tmp_files.is_empty(),
            "expected no .tmp files after atomic write, found {:?}",
            tmp_files
        );
        cleanup(&dir);
    }

    #[test]
    fn write_atomic_errors_when_parent_missing() {
        let dir = tmp_dir("bad-parent");
        let missing_parent = dir.join("does-not-exist").join("config.json");

        let result = write_json_atomically(&missing_parent, "content");

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Failed to create temp file"),
            "expected error about temp file creation, got: {}",
            err_msg
        );
        cleanup(&dir);
    }

    // ---- replace_file_atomically ----

    #[test]
    fn replace_writes_supplied_bytes() {
        let dir = tmp_dir("replace-bytes");
        let path = dir.join("raw.bin");

        replace_file_atomically(&path, b"payload bytes").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"payload bytes");
        cleanup(&dir);
    }

    #[test]
    fn replace_existing_file_preserves_new_content() {
        let dir = tmp_dir("replace-existing");
        let path = dir.join("raw.bin");
        fs::write(&path, b"old bytes").unwrap();

        replace_file_atomically(&path, b"new bytes").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new bytes");
        cleanup(&dir);
    }

    #[test]
    fn replace_failure_leaves_existing_target_unchanged_and_no_tmp_file() {
        let dir = tmp_dir("replace-fail");
        // A directory cannot be replaced by a file, so the rename must fail while
        // the destination (and everything else in `dir`) stays intact.
        let target = dir.join("target");
        fs::create_dir_all(&target).unwrap();

        let result = replace_file_atomically(&target, b"new bytes");

        assert!(result.is_err(), "replacement over a directory must fail");
        assert!(target.is_dir(), "target directory must remain unchanged");

        let tmp_files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".tmp")))
            .collect();
        assert!(
            tmp_files.is_empty(),
            "expected no .tmp files after failed replacement, found {:?}",
            tmp_files
        );
        cleanup(&dir);
    }

    // ---- recover_corrupted_json ----

    #[test]
    fn recover_renames_corrupted_to_backup_and_writes_defaults() {
        let dir = tmp_dir("recover-basic");
        let path = dir.join("config.json");
        let corrupt = "{{{broken!!!";

        fs::write(&path, corrupt).unwrap();

        let defaults = TestConfig {
            name: "default".into(),
            count: 10,
        };
        let returned = recover_corrupted_json(&path, &defaults).unwrap();

        assert_eq!(returned, defaults);

        let actual_bytes = fs::read_to_string(&path).unwrap();
        let parsed: TestConfig =
            serde_json::from_str(&actual_bytes).expect("recovered config should be valid JSON");
        assert_eq!(parsed, defaults);

        let backup_files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.contains(".bak.") && n.ends_with(".json"))
            })
            .collect();
        assert_eq!(backup_files.len(), 1, "expected exactly one backup file");

        let backup_content = fs::read_to_string(backup_files[0].path()).unwrap();
        assert_eq!(
            backup_content, corrupt,
            "backup must preserve corrupted bytes"
        );

        cleanup(&dir);
    }

    #[test]
    fn recover_pretty_prints_defaults() {
        let dir = tmp_dir("recover-pretty");
        let path = dir.join("config.json");
        fs::write(&path, "garbage").unwrap();

        let defaults = TestConfig {
            name: "pretty".into(),
            count: 7,
        };
        recover_corrupted_json(&path, &defaults).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(
            raw.contains('\n') || raw.contains("  "),
            "expected pretty-printed (indented) JSON, got: {}",
            raw
        );
        cleanup(&dir);
    }

    #[test]
    fn recover_missing_source_returns_error_no_replacement() {
        let dir = tmp_dir("recover-missing");
        let path = dir.join("nonexistent.json");

        let defaults = TestConfig {
            name: "ghost".into(),
            count: 0,
        };
        let result = recover_corrupted_json(&path, &defaults);

        assert!(result.is_err());
        assert!(
            !path.exists(),
            "replacement file must not be created when source is missing"
        );

        cleanup(&dir);
    }

    // ---- concurrency ----

    #[test]
    fn concurrent_recover_on_different_files_is_deterministic() {
        let dir = tmp_dir("concurrent");
        let path_a = dir.join("a.json");
        let path_b = dir.join("b.json");
        fs::write(&path_a, "corrupt-a").unwrap();
        fs::write(&path_b, "corrupt-b").unwrap();

        let defaults_a = TestConfig {
            name: "alpha".into(),
            count: 1,
        };
        let defaults_b = TestConfig {
            name: "beta".into(),
            count: 2,
        };

        let da = defaults_a.clone();
        let db = defaults_b.clone();

        let pa = path_a.clone();
        let pb = path_b.clone();

        let handle_a = std::thread::spawn(move || recover_corrupted_json(&pa, &da).unwrap());
        let handle_b = std::thread::spawn(move || recover_corrupted_json(&pb, &db).unwrap());

        let result_a = handle_a.join().unwrap();
        let result_b = handle_b.join().unwrap();

        assert_eq!(result_a, defaults_a);
        assert_eq!(result_b, defaults_b);

        let raw_a = fs::read_to_string(&path_a).unwrap();
        let parsed_a: TestConfig = serde_json::from_str(&raw_a).expect("a.json must be valid JSON");
        assert_eq!(parsed_a, defaults_a);

        let raw_b = fs::read_to_string(&path_b).unwrap();
        let parsed_b: TestConfig = serde_json::from_str(&raw_b).expect("b.json must be valid JSON");
        assert_eq!(parsed_b, defaults_b);

        let backup_count = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.contains(".bak.") && n.ends_with(".json"))
            })
            .count();
        assert_eq!(backup_count, 2, "each recovery must produce one backup");

        cleanup(&dir);
    }
}
