//! Tauri commands backing the settings-load recovery dialog (ROADMAP-123).
//!
//! These commands are registered only in the minimal recovery process, where
//! normal startup stopped because `settings.json` could not be loaded. They
//! never report an unconfirmed success: the dialog text mirrors the actual
//! filesystem outcomes, and the original file is modified only by the explicit
//! defaults restore, which is gated on a confirmed backup copy.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};
use tracing::info;

use crate::config::{
    backup_json_copy, normalize_detected_locale, normalize_detected_theme, write_default_settings,
    BackupOutcome, SettingsLoadFailure,
};

/// Serializable view of the backup attempt shown in the dialog.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum RecoveryBackupDto {
    Created { path: String },
    Failed { error: String },
    SourceMissing,
}

impl From<&BackupOutcome> for RecoveryBackupDto {
    fn from(backup: &BackupOutcome) -> Self {
        match backup {
            BackupOutcome::Created { path } => Self::Created {
                path: path.to_string_lossy().into_owned(),
            },
            BackupOutcome::Failed { error } => Self::Failed {
                error: error.clone(),
            },
            BackupOutcome::SourceMissing => Self::SourceMissing,
        }
    }
}

/// Full diagnostics payload rendered by the recovery dialog.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryDiagnosticsDto {
    /// Original `settings.json` path, when it could be resolved.
    pub settings_path: Option<String>,
    /// Machine-readable failure stage (`read` | `syntax` | `deserialize` | `write`).
    pub stage: String,
    /// Secret-safe reason; deserialization errors never embed file values.
    pub reason: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub backup: RecoveryBackupDto,
    /// Dialog language detected from the broken file, falling back to `en`.
    pub requested_locale: String,
    /// Dialog theme detected from the broken file, falling back to `dark`.
    pub requested_theme: String,
}

/// Tauri-managed state of the recovery process.
#[derive(Clone)]
pub struct RecoveryState {
    failure: SettingsLoadFailure,
    /// Path of a backup copy confirmed to exist on disk, if any.
    backup_confirmed: Arc<Mutex<Option<PathBuf>>>,
}

impl RecoveryState {
    pub fn new(failure: SettingsLoadFailure) -> Self {
        let backup_confirmed = Arc::new(Mutex::new(
            failure
                .backup
                .confirmed_path()
                .map(|path| path.to_path_buf()),
        ));
        Self {
            failure,
            backup_confirmed,
        }
    }

    pub fn diagnostics(&self) -> RecoveryDiagnosticsDto {
        RecoveryDiagnosticsDto {
            settings_path: self
                .failure
                .settings_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            stage: self.failure.stage.as_str().to_string(),
            reason: self.failure.reason.clone(),
            line: self.failure.line,
            column: self.failure.column,
            backup: RecoveryBackupDto::from(&self.failure.backup),
            requested_locale: normalize_detected_locale(self.failure.detected_locale.as_deref())
                .to_string(),
            requested_theme: normalize_detected_theme(self.failure.detected_theme.as_deref())
                .to_string(),
        }
    }

    /// Path of the original `settings.json` when known.
    fn settings_path(&self) -> Option<&Path> {
        self.failure.settings_path.as_deref()
    }

    /// Return a confirmed backup path, creating a fresh copy when needed.
    ///
    /// Defaults must never replace the original file without a confirmed
    /// copy of its bytes, so restore is impossible until this succeeds.
    fn ensure_backup(&self) -> Result<PathBuf, String> {
        if let Some(path) = self.backup_confirmed.lock().unwrap().clone() {
            if path.is_file() {
                return Ok(path);
            }
        }

        let source = self
            .settings_path()
            .ok_or_else(|| "Settings file path is unavailable".to_string())?;
        if !source.is_file() {
            return Err(format!(
                "Settings file is missing or unreadable: {}",
                source.display()
            ));
        }

        let backup = backup_json_copy(source).map_err(|error| {
            format!(
                "Failed to create a backup copy of {}: {}",
                source.display(),
                error
            )
        })?;
        *self.backup_confirmed.lock().unwrap() = Some(backup.clone());
        Ok(backup)
    }

    /// Replace the broken `settings.json` with canonical defaults, atomically,
    /// only after a backup copy of the original bytes is confirmed on disk.
    ///
    /// A missing source file has nothing to back up: the write proceeds
    /// directly so the user can still reset (no loss of original data is
    /// possible when no original data exists).
    pub fn restore_defaults(&self) -> Result<(), String> {
        let source = self
            .settings_path()
            .ok_or_else(|| "Settings file path is unavailable".to_string())?;
        if source.is_file() {
            self.ensure_backup()?;
        }
        write_default_settings(source).map_err(|error| {
            format!(
                "Failed to write default settings to {}: {}",
                source.display(),
                error
            )
        })?;
        Ok(())
    }
}

/// Diagnostics for the recovery dialog.
#[tauri::command]
pub fn settings_recovery_get_diagnostics(
    state: State<'_, RecoveryState>,
) -> RecoveryDiagnosticsDto {
    state.diagnostics()
}

/// Open the original `settings.json` with the system default `.json` handler.
///
/// The dialog stays open; the user edits and saves in the external
/// application. Failures return the error together with the path instead of a
/// fake success.
#[tauri::command]
pub async fn settings_recovery_open_settings_file(
    app_handle: AppHandle,
    state: State<'_, RecoveryState>,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;

    let path = state
        .settings_path()
        .ok_or_else(|| "Settings file path is unavailable".to_string())?
        .to_path_buf();
    let display = path.to_string_lossy().into_owned();
    app_handle
        .opener()
        .open_path(display.clone(), None::<&str>)
        .map_err(|error| format!("Failed to open the settings file {}: {}", display, error))
}

/// Write canonical defaults over the broken file and close the application.
///
/// On success the process exits, so the next start performs a normal load.
/// On failure the dialog stays open and the returned error explains what
/// happened; no success is reported without a confirmed write.
#[tauri::command]
pub async fn settings_recovery_restore_defaults(
    app_handle: AppHandle,
    state: State<'_, RecoveryState>,
) -> Result<(), String> {
    let state = state.inner().clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || state.restore_defaults())
        .await
        .map_err(|error| format!("Recovery task failed: {}", error))?;

    if outcome.is_ok() {
        info!("Default settings restored; closing the recovery process");
        app_handle.exit(0);
    }
    outcome
}

/// Close the application without touching the original settings file.
#[tauri::command]
pub fn settings_recovery_quit(app_handle: AppHandle) -> Result<(), String> {
    info!("Recovery dialog requested application close");
    app_handle.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BackupOutcome, SettingsFailureStage};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-recovery-cmd-{}-{}-{}",
            tag,
            std::process::id(),
            stamp
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn state_for(path: &Path) -> RecoveryState {
        RecoveryState::new(SettingsLoadFailure {
            settings_path: Some(path.to_path_buf()),
            stage: SettingsFailureStage::Syntax,
            reason: "test failure".to_string(),
            line: Some(1),
            column: Some(3),
            backup: BackupOutcome::SourceMissing,
            detected_locale: None,
            detected_theme: None,
        })
    }

    fn backup_files(dir: &Path) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".bak.") && name.ends_with(".json"))
            })
            .collect();
        files.sort();
        files
    }

    /// Restore on an existing broken file must secure a byte-exact backup
    /// first, then atomically replace the file with tokenized defaults.
    #[test]
    fn restore_backs_up_source_then_writes_defaults() {
        let dir = temp_dir("restore-with-source");
        let path = dir.join("settings.json");
        let corrupt = r#"{"audio": {"speaker_volume": "loud"},}"#;
        std::fs::write(&path, corrupt).unwrap();

        let state = state_for(&path);
        state
            .restore_defaults()
            .expect("restore must succeed after securing a backup");

        let backups = backup_files(&dir);
        assert_eq!(backups.len(), 1, "restore must create exactly one backup");
        assert_eq!(
            std::fs::read_to_string(&backups[0]).unwrap(),
            corrupt,
            "backup must preserve the original bytes"
        );

        let restored: crate::config::AppSettings =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
                .expect("restored settings must parse");
        assert!(restored
            .input_server
            .access_token
            .as_deref()
            .is_some_and(|token| !token.is_empty()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// With no source file there is nothing to back up: the reset writes
    /// defaults directly instead of refusing.
    #[test]
    fn restore_with_missing_source_writes_defaults_directly() {
        let dir = temp_dir("restore-missing-source");
        let path = dir.join("settings.json");

        let state = state_for(&path);
        state
            .restore_defaults()
            .expect("restore without a source file must still write defaults");

        let restored: crate::config::AppSettings =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
                .expect("restored settings must parse");
        assert!(restored.input_server.access_token.is_some());
        assert!(backup_files(&dir).is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Diagnostics carry the fallback locale/theme and the failure details.
    #[test]
    fn diagnostics_report_fallback_locale_and_stage() {
        let dir = temp_dir("diagnostics");
        let path = dir.join("settings.json");
        let state = state_for(&path);

        let dto = state.diagnostics();
        assert_eq!(dto.settings_path.as_deref(), Some(path.to_str().unwrap()));
        assert_eq!(dto.stage, "syntax");
        assert_eq!(dto.line, Some(1));
        assert_eq!(dto.column, Some(3));
        assert_eq!(dto.requested_locale, normalize_detected_locale(None));
        assert_eq!(dto.requested_theme, "dark");
        assert!(matches!(dto.backup, RecoveryBackupDto::SourceMissing));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
