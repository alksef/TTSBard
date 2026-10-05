//! Settings-load failure diagnostics (ROADMAP-123).
//!
//! When `settings.json` cannot be read, parsed, or deserialized, the
//! application must not silently replace it with defaults. Instead the failure
//! is described by [`SettingsLoadFailure`]: the exact source path, a
//! secret-safe reason, the reported position when available, and the actual
//! outcome of the best-effort byte-exact backup copy. The ordinary startup is
//! stopped and the recovery dialog renders this information instead.

use serde::Serialize;
use std::path::{Path, PathBuf};

use super::persistence;

/// Which step of the settings load failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingsFailureStage {
    /// The file exists but could not be read (permissions, IO error).
    Read,
    /// The content is not valid JSON.
    Syntax,
    /// Valid JSON that does not match the `AppSettings` schema.
    Deserialize,
    /// The file loaded but persisting a migration failed.
    Write,
}

impl SettingsFailureStage {
    /// Stable machine-readable identifier for the frontend.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Syntax => "syntax",
            Self::Deserialize => "deserialize",
            Self::Write => "write",
        }
    }
}

/// Outcome of the attempt to back up the original settings file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum BackupOutcome {
    /// An exact copy was written at `path`; the source file stays in place.
    Created { path: PathBuf },
    /// No backup exists; `error` explains why. The source is untouched.
    Failed { error: String },
    /// No source file existed, so there was nothing to back up.
    SourceMissing,
}

impl BackupOutcome {
    /// Path of a confirmed backup copy, if one exists on disk.
    pub fn confirmed_path(&self) -> Option<&Path> {
        match self {
            Self::Created { path } => Some(path.as_path()),
            Self::Failed { .. } | Self::SourceMissing => None,
        }
    }
}

/// Structured diagnostics for a failed `settings.json` load.
///
/// `reason` must never contain password, token, or key values: deserialization
/// errors embed the offending value from the user's file, so those are
/// replaced with a fixed schema-level sentence (see
/// [`sanitized_deserialize_reason`]).
#[derive(Debug, Clone)]
pub struct SettingsLoadFailure {
    /// Absolute path to the original `settings.json` when it could be resolved.
    pub settings_path: Option<PathBuf>,
    pub stage: SettingsFailureStage,
    /// Secret-safe, user-displayable reason (English technical text).
    pub reason: String,
    /// 1-based line reported by the parser, when available.
    pub line: Option<usize>,
    /// 1-based column reported by the parser, when available.
    pub column: Option<usize>,
    pub backup: BackupOutcome,
    /// Best-effort `ui_language` read from the broken file, for dialog display only.
    pub detected_locale: Option<String>,
    /// Best-effort `theme` read from the broken file, for dialog display only.
    pub detected_theme: Option<String>,
}

impl std::fmt::Display for SettingsLoadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.settings_path, self.line, self.column) {
            (Some(path), Some(line), Some(column)) => write!(
                f,
                "settings load failed at {:?} ({}): {} (line {}, column {})",
                path,
                self.stage.as_str(),
                self.reason,
                line,
                column
            ),
            (Some(path), _, _) => write!(
                f,
                "settings load failed at {:?} ({}): {}",
                path,
                self.stage.as_str(),
                self.reason
            ),
            (None, _, _) => write!(
                f,
                "settings load failed ({}): {}",
                self.stage.as_str(),
                self.reason
            ),
        }
    }
}

impl std::error::Error for SettingsLoadFailure {}

impl SettingsLoadFailure {
    /// Build diagnostics for a failed load of the file at `path`.
    ///
    /// Attempts a byte-exact backup copy when the source file exists and
    /// reports the actual outcome; the source file itself is never modified.
    /// `raw_content` (the file text, when it was read) is used only to detect
    /// the user's UI language and theme for the dialog; nothing from it is
    /// copied into `reason`.
    pub fn diagnose(
        path: &Path,
        stage: SettingsFailureStage,
        reason: String,
        line: Option<usize>,
        column: Option<usize>,
        raw_content: Option<&str>,
    ) -> Self {
        let backup = if path.is_file() {
            match persistence::backup_json_copy(path) {
                Ok(backup_path) => BackupOutcome::Created { path: backup_path },
                Err(error) => BackupOutcome::Failed {
                    error: error.to_string(),
                },
            }
        } else {
            BackupOutcome::SourceMissing
        };

        let (detected_locale, detected_theme) = raw_content
            .and_then(|content| serde_json::from_str::<serde_json::Value>(content).ok())
            .map(|value| (detect_locale(&value), detect_theme(&value)))
            .unwrap_or((None, None));

        Self {
            settings_path: Some(path.to_path_buf()),
            stage,
            reason,
            line,
            column,
            backup,
            detected_locale,
            detected_theme,
        }
    }

    /// Diagnostics for a failure where the settings path could not even be
    /// resolved (for example, the config directory itself is unavailable).
    /// No backup is attempted.
    pub fn unresolved(stage: SettingsFailureStage, reason: String) -> Self {
        Self {
            settings_path: None,
            stage,
            reason,
            line: None,
            column: None,
            backup: BackupOutcome::SourceMissing,
            detected_locale: None,
            detected_theme: None,
        }
    }
}

/// Recognized built-in locales for the recovery dialog fallback.
pub fn normalize_detected_locale(raw: Option<&str>) -> &'static str {
    select_recovery_locale(raw, system_ui_language())
}

fn select_recovery_locale(saved: Option<&str>, system: Option<&str>) -> &'static str {
    match saved {
        Some("ru") => "ru",
        Some("en") => "en",
        _ => match system {
            Some("ru") => "ru",
            _ => "en",
        },
    }
}

fn system_ui_language() -> Option<&'static str> {
    #[cfg(windows)]
    {
        // UI language, independently of regional formats and keyboard layout.
        let language = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        match language & 0x03ff {
            0x19 => Some("ru"),
            0x09 => Some("en"),
            _ => None,
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Recognized built-in themes for the recovery dialog fallback.
pub fn normalize_detected_theme(raw: Option<&str>) -> &'static str {
    match raw {
        Some("light") => "light",
        _ => "dark",
    }
}

fn detect_locale(value: &serde_json::Value) -> Option<String> {
    let locale = value.get("ui_language")?.as_str()?;
    matches!(locale, "ru" | "en").then(|| locale.to_string())
}

fn detect_theme(value: &serde_json::Value) -> Option<String> {
    let theme = value.get("theme")?.as_str()?;
    matches!(theme, "dark" | "light").then(|| theme.to_string())
}

/// Build a secret-safe reason for a schema (de)serialization error.
///
/// `serde_json` data errors can quote the offending value from the user's file
/// (for example an invalid-type token string), so the raw message is shown
/// only when it is provably schema-derived (`missing field …`). Every other
/// variant gets a fixed sentence; the parsed position, when known, is carried
/// in the dedicated fields instead.
pub fn sanitized_deserialize_reason(error: &serde_json::Error) -> String {
    let message = error.to_string();
    if message.starts_with("missing field `") {
        message
    } else {
        "The file contains a value that does not match the expected settings structure.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::AppSettings;

    #[test]
    fn recovery_language_prefers_settings_then_system_ui_then_english() {
        assert_eq!(select_recovery_locale(Some("en"), Some("ru")), "en");
        assert_eq!(select_recovery_locale(Some("ru"), Some("en")), "ru");
        assert_eq!(select_recovery_locale(None, Some("ru")), "ru");
        assert_eq!(select_recovery_locale(None, Some("en")), "en");
        assert_eq!(select_recovery_locale(Some("xx"), Some("ru")), "ru");
        assert_eq!(select_recovery_locale(None, Some("de")), "en");
        assert_eq!(select_recovery_locale(None, None), "en");
    }

    fn write_source(dir: &Path, content: &str) -> PathBuf {
        let path = dir.join("settings.json");
        std::fs::write(&path, content).unwrap();
        path
    }

    fn unique_dir(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-recovery-{}-{}-{}",
            tag,
            std::process::id(),
            stamp
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn diagnose_creates_exact_backup_and_keeps_source() {
        let dir = unique_dir("backup");
        let source = r#"{"broken": true,}"#;
        let path = write_source(&dir, source);

        let failure = SettingsLoadFailure::diagnose(
            &path,
            SettingsFailureStage::Syntax,
            "trailing comma".to_string(),
            Some(1),
            Some(13),
            None,
        );

        let BackupOutcome::Created { path: backup } = failure.backup else {
            panic!("expected a created backup, got {:?}", failure.backup);
        };
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), source);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        assert_eq!(failure.settings_path.as_deref(), Some(path.as_path()));
        assert_eq!(failure.detected_locale, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn diagnose_reports_source_missing_without_backup() {
        let dir = unique_dir("missing");
        let path = dir.join("settings.json");

        let failure = SettingsLoadFailure::diagnose(
            &path,
            SettingsFailureStage::Read,
            "not found".to_string(),
            None,
            None,
            None,
        );

        assert!(matches!(failure.backup, BackupOutcome::SourceMissing));
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn diagnose_detects_locale_and_theme_from_broken_file() {
        let dir = unique_dir("detect");
        let content = r#"{"ui_language": "ru", "theme": "light", "audio": 42}"#;
        let path = write_source(&dir, content);

        let failure = SettingsLoadFailure::diagnose(
            &path,
            SettingsFailureStage::Deserialize,
            "schema mismatch".to_string(),
            None,
            None,
            Some(content),
        );

        assert_eq!(failure.detected_locale.as_deref(), Some("ru"));
        assert_eq!(failure.detected_theme.as_deref(), Some("light"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn diagnose_ignores_unknown_locale_and_theme() {
        let dir = unique_dir("unknown-locale");
        let content = r#"{"ui_language": "xx", "theme": "solarized"}"#;
        let path = write_source(&dir, content);

        let failure = SettingsLoadFailure::diagnose(
            &path,
            SettingsFailureStage::Deserialize,
            "schema mismatch".to_string(),
            None,
            None,
            Some(content),
        );

        assert_eq!(failure.detected_locale, None);
        assert_eq!(failure.detected_theme, None);
        assert_eq!(
            select_recovery_locale(failure.detected_locale.as_deref(), Some("ru")),
            "ru"
        );
        assert_eq!(
            normalize_detected_theme(failure.detected_theme.as_deref()),
            "dark"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitize_hides_values_but_keeps_missing_field_hint() {
        let secret = "sk-token-value-42";
        // Start from full valid defaults, then make a strict numeric field
        // carry a string: the raw serde message quotes that value.
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value["audio"]["speaker_volume"] = serde_json::json!(secret);
        let error = serde_json::from_value::<AppSettings>(value).expect_err("wrong type must fail");

        let raw = error.to_string();
        assert!(
            raw.contains(secret),
            "test setup expects the raw message to embed the value, got: {raw}"
        );
        let reason = sanitized_deserialize_reason(&error);
        assert!(
            !reason.contains(secret),
            "reason must not embed the offending value: {reason}"
        );
        assert!(reason.contains("settings structure"), "got: {reason}");

        let missing: serde_json::Value = serde_json::json!({"unrelated": true});
        let error = serde_json::from_value::<AppSettings>(missing)
            .expect_err("empty object must miss required fields");
        let reason = sanitized_deserialize_reason(&error);
        assert!(
            reason.starts_with("missing field `"),
            "missing-field messages are schema-derived and stay readable: {reason}"
        );
    }

    #[test]
    fn unresolved_failure_has_no_path_and_no_backup() {
        let failure =
            SettingsLoadFailure::unresolved(SettingsFailureStage::Read, "no config dir".into());

        assert!(failure.settings_path.is_none());
        assert!(matches!(failure.backup, BackupOutcome::SourceMissing));
        let display = failure.to_string();
        assert!(display.contains("no config dir"));
    }
}
