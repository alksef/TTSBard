//! Runtime localization IPC (ROADMAP-096).
//!
//! Hosts the Tauri-managed [`LocalizationState`]: the in-memory
//! [`LocaleCatalog`], an async mutex holding the current [`LocaleSnapshot`] and
//! a startup snapshot for every window. Language selection is persisted for the
//! next application start; it deliberately does not change a running UI.

use crate::config::SettingsManager;
use crate::localization::{Language, LocaleCatalog, LocaleSnapshot};
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::{AppHandle, State};

/// Serializable camelCase DTO for a single available language.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageDto {
    pub locale: String,
    pub name: String,
}

impl From<&Language> for LanguageDto {
    fn from(language: &Language) -> Self {
        Self {
            locale: language.locale().to_string(),
            name: language.name().to_string(),
        }
    }
}

/// Serializable camelCase DTO for a localization snapshot.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizationSnapshotDto {
    pub revision: u64,
    pub requested_locale: String,
    pub locale: String,
    pub languages: Vec<LanguageDto>,
    pub messages: BTreeMap<String, String>,
    pub diagnostics: Vec<String>,
}

impl LocalizationSnapshotDto {
    /// Clone the getter-only snapshot data into the owned DTO.
    fn from_snapshot(snapshot: &LocaleSnapshot, revision: u64) -> Self {
        Self {
            revision,
            requested_locale: snapshot.requested_locale().to_string(),
            locale: snapshot.locale().to_string(),
            languages: snapshot.languages().iter().map(LanguageDto::from).collect(),
            messages: snapshot.messages().clone(),
            diagnostics: snapshot.diagnostics().to_vec(),
        }
    }
}

/// Tauri-managed immutable localization state for the current application run.
pub struct LocalizationState {
    catalog: LocaleCatalog,
    current: LocaleSnapshot,
}

impl LocalizationState {
    /// Build the state from an already-loaded catalog and the requested locale.
    ///
    /// Revision starts at zero. A requested locale missing from the catalog
    /// falls back to `en` exactly like [`LocaleCatalog::snapshot`].
    pub fn new(catalog: LocaleCatalog, requested_locale: &str) -> Self {
        let current = catalog.snapshot(requested_locale);
        Self { catalog, current }
    }

    /// Whether a locale is available in the loaded catalog.
    pub fn is_locale_available(&self, locale: &str) -> bool {
        self.catalog.is_locale_available(locale)
    }

    /// Read the startup snapshot. Revision stays zero throughout this run.
    pub fn snapshot(&self) -> LocalizationSnapshotDto {
        LocalizationSnapshotDto::from_snapshot(&self.current, 0)
    }
}

/// Get the current localization snapshot.
#[tauri::command]
pub async fn get_localization(
    state: State<'_, LocalizationState>,
) -> Result<LocalizationSnapshotDto, String> {
    Ok(state.snapshot())
}

/// Set the UI language.
///
/// Persists the locale for the next application start. The active snapshot,
/// native tray and open windows remain unchanged until restart.
#[tauri::command]
pub async fn set_ui_language(
    locale: String,
    app_handle: AppHandle,
    state: State<'_, LocalizationState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    if !state.is_locale_available(&locale) {
        return Err(format!("Language is not available: {locale}"));
    }

    let locale_for_persist = locale.clone();
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ui_language(locale_for_persist)
    })
    .await?;

    super::emit_settings_changed(&app_handle);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn builtin_state(requested_locale: &str) -> LocalizationState {
        LocalizationState::new(LocaleCatalog::load(None, None), requested_locale)
    }

    #[test]
    fn initial_revision_is_zero_and_locale_matches_requested() {
        let state = builtin_state("en");
        let snap = state.snapshot();

        assert_eq!(snap.revision, 0);
        assert_eq!(snap.requested_locale, "en");
        assert_eq!(snap.locale, "en");
        assert_eq!(
            snap.messages.get("common.save").map(String::as_str),
            Some("Save")
        );
    }

    #[test]
    fn startup_catalog_keeps_both_builtin_languages() {
        let state = builtin_state("en");
        assert!(state.is_locale_available("en"));
        assert!(state.is_locale_available("ru"));
        assert_eq!(state.snapshot().revision, 0);
    }

    #[test]
    fn unavailable_locale_is_not_available_in_startup_catalog() {
        let state = builtin_state("en");
        assert!(!state.is_locale_available("xx"));
        let snap = state.snapshot();
        assert_eq!(snap.revision, 0);
        assert_eq!(snap.requested_locale, "en");
        assert_eq!(snap.locale, "en");
        assert_eq!(
            snap.messages.get("common.save").map(String::as_str),
            Some("Save")
        );
    }

    #[test]
    fn dto_serializes_camel_case_with_revision() {
        let state = builtin_state("ru");
        let dto = state.snapshot();
        let value = serde_json::to_value(&dto).unwrap();
        let object = value.as_object().unwrap();

        assert_eq!(object["revision"], serde_json::json!(0));
        assert!(object.contains_key("requestedLocale"));
        assert!(object.contains_key("locale"));
        assert!(object.contains_key("languages"));
        assert!(object.contains_key("messages"));
        assert!(object.contains_key("diagnostics"));
        assert!(!object.contains_key("requested_locale"));

        let languages = object["languages"].as_array().unwrap();
        let first = languages[0].as_object().unwrap();
        assert!(first.contains_key("locale"));
        assert!(first.contains_key("name"));
    }

    fn temp_config_locales(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-acceptance-{tag}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_pack(dir: &Path, content: &str) {
        std::fs::write(dir.join("de.json"), content).unwrap();
    }

    #[test]
    fn third_language_acceptance_windows_tray_fallback_and_restart_only() {
        use crate::localization::LocaleCatalog;

        let dir = temp_config_locales("de");
        // A realistic third-language pack: translated startup labels (tray and
        // every window title), a couple of UI keys, and intentionally missing
        // keys that must fall back to English.
        write_pack(
            &dir,
            r#"{"schemaVersion":1,"locale":"de","name":"Deutsch","messages":{
              "tray.show_main":"Hauptfenster anzeigen",
              "tray.soundpanel":"Soundpanel",
              "tray.playback":"Wiedergabe",
              "tray.quit":"Beenden",
              "window.soundpanel":"Soundpanel",
              "window.playback":"Wiedergabe",
              "window.ocr_selection":"OCR-Auswahl",
              "settings.title":"Einstellungen",
              "settings.language":"Sprache",
              "common.save":"Speichern"
            }}"#,
        );

        let catalog = LocaleCatalog::load(None, Some(dir.as_path()));
        assert!(
            catalog.diagnostics().is_empty(),
            "valid third-language pack must not warn, got {:?}",
            catalog.diagnostics()
        );

        // Discovery + persisted selection applied at startup.
        let state = LocalizationState::new(catalog, "de");
        assert!(state.is_locale_available("de"));
        assert!(!state.is_locale_available("fr"));
        let snap = state.snapshot();
        assert_eq!(snap.requested_locale, "de");
        assert_eq!(snap.locale, "de");
        assert_eq!(snap.revision, 0);
        assert!(snap
            .languages
            .iter()
            .any(|l| l.locale == "de" && l.name == "Deutsch"));

        // All startup tray labels and window titles resolve for the pack.
        assert_eq!(
            snap.messages.get("tray.show_main").map(String::as_str),
            Some("Hauptfenster anzeigen")
        );
        assert_eq!(
            snap.messages.get("tray.soundpanel").map(String::as_str),
            Some("Soundpanel")
        );
        assert_eq!(
            snap.messages.get("tray.playback").map(String::as_str),
            Some("Wiedergabe")
        );
        assert_eq!(
            snap.messages.get("tray.quit").map(String::as_str),
            Some("Beenden")
        );
        assert_eq!(
            snap.messages.get("window.soundpanel").map(String::as_str),
            Some("Soundpanel")
        );
        assert_eq!(
            snap.messages.get("window.playback").map(String::as_str),
            Some("Wiedergabe")
        );
        assert_eq!(
            snap.messages
                .get("window.ocr_selection")
                .map(String::as_str),
            Some("OCR-Auswahl")
        );

        // Missing keys independently fall back to embedded English.
        assert_eq!(
            snap.messages.get("common.close").map(String::as_str),
            Some("Close")
        );
        assert_eq!(
            snap.messages.get("common.cancel").map(String::as_str),
            Some("Cancel")
        );

        // Restart-only: file edits never change the running snapshot.
        write_pack(
            &dir,
            r#"{"schemaVersion":1,"locale":"de","name":"Deutsch","messages":{
              "tray.quit":"Beenden geändert"
            }}"#,
        );
        let after = state.snapshot();
        assert_eq!(after.revision, 0);
        assert_eq!(
            after.messages.get("tray.quit").map(String::as_str),
            Some("Beenden"),
            "running snapshot must ignore pack edits until restart"
        );

        // An unavailable persisted preference keeps English for the run.
        let en_state = LocalizationState::new(LocaleCatalog::load(None, Some(dir.as_path())), "fr");
        let en_snap = en_state.snapshot();
        assert_eq!(en_snap.requested_locale, "fr");
        assert_eq!(en_snap.locale, "en");
        assert_eq!(
            en_snap.messages.get("tray.quit").map(String::as_str),
            Some("Quit")
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
