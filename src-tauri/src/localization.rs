//! Pure language-pack catalog for ROADMAP-096.
//!
//! Loads the embedded English/Russian dictionaries and, when supplied, external
//! language packs from a resource and a config `locales` directory. The catalog
//! is fully in-memory once built: selecting a locale never touches the
//! filesystem. No Tauri commands or persistence live here yet.

#![allow(dead_code)]

use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::Path;

const SCHEMA_VERSION: u64 = 1;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_NAME_CHARS: usize = 256;
const MAX_ENTRIES: usize = 10_000;
const MAX_LOCALE_LEN: usize = 35;

const BUILTIN_EN_JSON: &str = include_str!("../../locales/en.json");
const BUILTIN_RU_JSON: &str = include_str!("../../locales/ru.json");

/// HTML event-handler attribute names that make a message executable HTML.
const HTML_EVENT_HANDLERS: &[&str] = &[
    "onabort",
    "onblur",
    "oncanplay",
    "onchange",
    "onclick",
    "oncontextmenu",
    "oncopy",
    "oncut",
    "ondblclick",
    "ondragstart",
    "onerror",
    "onfocus",
    "oninput",
    "onkeydown",
    "onkeypress",
    "onkeyup",
    "onload",
    "onmousedown",
    "onmouseover",
    "onmouseout",
    "onmouseup",
    "onpaste",
    "onreset",
    "onscroll",
    "onselect",
    "onsubmit",
];

/// A single loaded language pack.
#[derive(Debug, Clone)]
struct Pack {
    locale: String,
    name: String,
    messages: BTreeMap<String, String>,
}

/// A language shown in the settings picker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    locale: String,
    name: String,
}

impl Language {
    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Serializable snapshot of the effective dictionary for a requested locale.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleSnapshot {
    requested_locale: String,
    locale: String,
    languages: Vec<Language>,
    messages: BTreeMap<String, String>,
    diagnostics: Vec<String>,
}

impl LocaleSnapshot {
    pub fn requested_locale(&self) -> &str {
        &self.requested_locale
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    pub fn messages(&self) -> &BTreeMap<String, String> {
        &self.messages
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// Resolve a message key against the effective dictionary.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(|s| s.as_str())
    }
}

/// In-memory language-pack catalog.
#[derive(Debug)]
pub struct LocaleCatalog {
    builtins: Vec<Pack>,
    resource: Vec<Pack>,
    config: Vec<Pack>,
    diagnostics: Vec<String>,
}

impl LocaleCatalog {
    /// Build the catalog from the embedded dictionaries and, when supplied, the
    /// external `resource/locales` and `config/locales` directories. Each
    /// directory is scanned exactly once; discovered packs are read now and
    /// never re-read later.
    pub fn load(resource_locales_dir: Option<&Path>, config_locales_dir: Option<&Path>) -> Self {
        let en_pack =
            parse_pack(BUILTIN_EN_JSON, "en", None).expect("embedded en.json must be valid");
        let en_messages = en_pack.pack.messages.clone();
        let ru_pack = parse_pack(BUILTIN_RU_JSON, "ru", Some(&en_messages))
            .expect("embedded ru.json must be valid");
        let builtins = vec![en_pack.pack, ru_pack.pack];

        let mut diagnostics = Vec::new();
        let resource = load_dir(resource_locales_dir, &en_messages, &mut diagnostics);
        let config = load_dir(config_locales_dir, &en_messages, &mut diagnostics);

        Self {
            builtins,
            resource,
            config,
            diagnostics,
        }
    }

    /// Compute the effective snapshot for a requested locale without touching
    /// the filesystem.
    pub fn snapshot(&self, requested_locale: &str) -> LocaleSnapshot {
        let requested = requested_locale.to_string();
        let effective = if self.is_locale_available(&requested) {
            requested.clone()
        } else {
            "en".to_string()
        };

        let messages = self.merged_messages(&effective);
        let languages = self.languages();

        let mut diagnostics = self.diagnostics.clone();
        if !self.is_locale_available(&requested) {
            diagnostics.push(format!(
                "requested locale {requested:?} is not available; using 'en'"
            ));
        }

        LocaleSnapshot {
            requested_locale: requested,
            locale: effective,
            languages,
            messages,
            diagnostics,
        }
    }

    /// All available languages (builtins always present), sorted by locale code.
    pub fn languages(&self) -> Vec<Language> {
        let mut names: HashMap<&str, &str> = HashMap::new();
        let mut locales: Vec<&str> = Vec::new();

        for pack in self
            .builtins
            .iter()
            .chain(self.resource.iter())
            .chain(self.config.iter())
        {
            if !names.contains_key(pack.locale.as_str()) {
                locales.push(pack.locale.as_str());
            }
            names.insert(pack.locale.as_str(), pack.name.as_str());
        }

        locales.sort_by_key(|locale| locale.to_ascii_lowercase());
        locales
            .into_iter()
            .map(|locale| Language {
                locale: locale.to_string(),
                name: names[locale].to_string(),
            })
            .collect()
    }

    /// Diagnostics collected while loading external packs (never raw file contents).
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// Whether a locale is available from any source.
    pub fn is_locale_available(&self, locale: &str) -> bool {
        self.builtins.iter().any(|p| p.locale == locale)
            || self.resource.iter().any(|p| p.locale == locale)
            || self.config.iter().any(|p| p.locale == locale)
    }

    /// Effective messages for a locale: builtin en base overlaid by the builtin,
    /// resource and config packs of that locale (highest priority wins). Unknown
    /// keys from external packs are ignored.
    fn merged_messages(&self, locale: &str) -> BTreeMap<String, String> {
        let mut merged = self
            .builtins
            .iter()
            .find(|p| p.locale == "en")
            .map(|p| p.messages.clone())
            .expect("embedded en must exist");

        for pack in self.packs_in_order(locale) {
            for (key, value) in &pack.messages {
                if merged.contains_key(key) {
                    merged.insert(key.clone(), value.clone());
                }
            }
        }

        merged
    }

    fn packs_in_order(&self, locale: &str) -> Vec<&Pack> {
        let mut out = Vec::new();
        if let Some(pack) = self.builtins.iter().find(|p| p.locale == locale) {
            out.push(pack);
        }
        if let Some(pack) = self.resource.iter().find(|p| p.locale == locale) {
            out.push(pack);
        }
        if let Some(pack) = self.config.iter().find(|p| p.locale == locale) {
            out.push(pack);
        }
        out
    }
}

fn load_dir(
    dir: Option<&Path>,
    en_messages: &BTreeMap<String, String>,
    diagnostics: &mut Vec<String>,
) -> Vec<Pack> {
    let Some(dir) = dir else {
        return Vec::new();
    };

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // A not-yet-created resource/config `locales` directory is normal on a
        // fresh install; it simply contributes no packs and no diagnostic.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            diagnostics.push(format!(
                "cannot read locales directory {}: {e}",
                dir.display()
            ));
            return Vec::new();
        }
    };

    let mut packs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(extension) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !extension.eq_ignore_ascii_case("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_valid_locale_code(stem) {
            diagnostics.push(format!(
                "{}: file name is not a valid locale code",
                path.display()
            ));
            continue;
        }

        let raw = match read_bounded(&path) {
            Ok(raw) => raw,
            Err(e) => {
                diagnostics.push(format!("{}: {e}", path.display()));
                continue;
            }
        };

        match parse_pack(&raw, stem, Some(en_messages)) {
            Ok(load) => {
                for message in load.diagnostics {
                    diagnostics.push(format!("{}: {message}", path.display()));
                }
                packs.push(load.pack);
            }
            Err(e) => {
                diagnostics.push(format!("{}: {e}", path.display()));
            }
        }
    }

    packs.sort_by_key(|a| a.locale.to_ascii_lowercase());
    packs
}

fn read_bounded(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("cannot open: {e}"))?;
    let len = file
        .metadata()
        .map_err(|e| format!("cannot stat: {e}"))?
        .len();
    if len > MAX_FILE_BYTES {
        return Err(format!(
            "file too large ({len} bytes, limit {MAX_FILE_BYTES})"
        ));
    }
    let mut buffer = String::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_string(&mut buffer)
        .map_err(|e| format!("cannot read: {e}"))?;
    if buffer.len() as u64 > MAX_FILE_BYTES {
        return Err("file too large".to_string());
    }
    Ok(buffer)
}

struct LoadResult {
    pack: Pack,
    diagnostics: Vec<String>,
}

fn parse_pack(
    raw: &str,
    expected_locale: &str,
    en_messages: Option<&BTreeMap<String, String>>,
) -> Result<LoadResult, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("invalid JSON: {e}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "pack must be a JSON object".to_string())?;

    match object.get("schemaVersion").and_then(|v| v.as_u64()) {
        Some(SCHEMA_VERSION) => {}
        Some(other) => {
            return Err(format!(
                "unsupported schemaVersion {other} (expected {SCHEMA_VERSION})"
            ))
        }
        None => return Err("missing or non-integer schemaVersion".to_string()),
    }

    let locale = object
        .get("locale")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing locale string".to_string())?;
    if !is_valid_locale_code(locale) {
        return Err(format!("invalid locale code {locale:?}"));
    }
    if locale != expected_locale {
        return Err(format!(
            "locale {locale:?} does not match file name {expected_locale:?}"
        ));
    }

    let name = object
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing name string".to_string())?;
    let name = name.trim();
    if name.is_empty() {
        return Err("empty name".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("name exceeds {MAX_NAME_CHARS} characters"));
    }
    if contains_executable_html(name) {
        return Err("name contains executable HTML".to_string());
    }

    let messages_object = object
        .get("messages")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "missing messages object".to_string())?;
    if messages_object.len() > MAX_ENTRIES {
        return Err(format!("too many messages ({})", messages_object.len()));
    }

    let mut messages = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for (key, value) in messages_object {
        let Some(text) = value.as_str() else {
            diagnostics.push(format!("message {key:?}: value is not a string"));
            continue;
        };
        if text.is_empty() {
            diagnostics.push(format!("message {key:?}: value is empty"));
            continue;
        }
        if contains_executable_html(text) {
            diagnostics.push(format!("message {key:?}: contains executable HTML"));
            continue;
        }
        if let Some(en) = en_messages {
            if let Err(why) = validate_placeholders(text, en.get(key)) {
                diagnostics.push(format!("message {key:?}: {why}"));
                continue;
            }
        }
        messages.insert(key.clone(), text.to_string());
    }

    Ok(LoadResult {
        pack: Pack {
            locale: locale.to_string(),
            name: name.to_string(),
            messages,
        },
        diagnostics,
    })
}

/// True if `code` is a locale tag that can name a catalog file (`en`, `ru`,
/// `pt-BR`, `zh-Hant`): a 2..8 ASCII-letter primary segment optionally followed
/// by non-empty ASCII-alphanumeric hyphen segments, at most 35 characters in
/// total. Shared with settings persistence so a persisted tag is always one the
/// catalog could resolve, and tags that cannot name a file (empty segments,
/// dots, separators, single-letter primaries) are rejected everywhere.
pub(crate) fn is_valid_locale_code(code: &str) -> bool {
    if code.is_empty() || code.len() > MAX_LOCALE_LEN {
        return false;
    }
    let mut segments = code.split('-');
    let first = match segments.next() {
        Some(first) => first,
        None => return false,
    };
    if first.len() < 2 || first.len() > 8 || !first.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    segments.all(|segment| {
        !segment.is_empty()
            && segment.len() <= 8
            && segment.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

fn validate_placeholders(text: &str, en: Option<&String>) -> Result<(), String> {
    let Some(en) = en else {
        // Key absent from English: it is an unknown key and will be ignored
        // during the merge, so there is nothing to validate against.
        return Ok(());
    };
    let en_set: HashSet<String> = extract_placeholders(en).into_iter().collect();
    let text_set: HashSet<String> = extract_placeholders(text).into_iter().collect();
    if text_set == en_set {
        return Ok(());
    }
    let mut missing: Vec<&str> = en_set.difference(&text_set).map(String::as_str).collect();
    missing.sort_unstable();
    let mut extra: Vec<&str> = text_set.difference(&en_set).map(String::as_str).collect();
    extra.sort_unstable();
    let mut message = "placeholder set differs from English message".to_string();
    if !missing.is_empty() {
        message.push_str(&format!("; missing {{{}}}", missing.join("}, {")));
    }
    if !extra.is_empty() {
        message.push_str(&format!("; extra {{{}}}", extra.join("}, {")));
    }
    Err(message)
}

/// Extract Vue-style named placeholders (`{name}`) without treating literal
/// braces (e.g. `{{template}}`, `{ name }`, or an unclosed brace) as parameters.
fn extract_placeholders(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut placeholders = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '{' {
            i += 1;
            continue;
        }
        // `{{` starts a Vue template expression, not a named placeholder.
        if i + 1 < chars.len() && chars[i + 1] == '{' {
            i += 2;
            continue;
        }
        let mut name = String::new();
        let mut j = i + 1;
        let mut closed = false;
        while j < chars.len() {
            let c = chars[j];
            if c == '}' {
                closed = true;
                break;
            }
            if c == '{' {
                break;
            }
            name.push(c);
            j += 1;
        }
        if closed && is_valid_placeholder_name(&name) {
            placeholders.push(name);
            i = j + 1;
        } else {
            i += 1;
        }
    }

    placeholders
}

fn is_valid_placeholder_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Structural check that every interpolation in a message is well formed:
/// `{name}` named parameters with valid identifiers, `{{...}}` literal braces
/// and `{'...'}` quoted literals kept usable. Returns the first offending span
/// so a built-in message cannot ship broken interpolation syntax. Plain text and
/// doubled braces need no escaping and are left untouched.
fn interpolation_syntax_error(text: &str) -> Option<&'static str> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '{' {
            i += 1;
            continue;
        }
        // `{{` starts a Vue-style literal, not a named placeholder.
        if i + 1 < chars.len() && chars[i + 1] == '{' {
            let mut j = i + 2;
            while j + 1 < chars.len() && !(chars[j] == '}' && chars[j + 1] == '}') {
                j += 1;
            }
            if j + 1 >= chars.len() {
                return Some("unclosed '{{' literal");
            }
            i = j + 2;
            continue;
        }
        // `{ '...' }` is a Vue-I18n quoted literal; keep it usable.
        if i + 1 < chars.len() && chars[i + 1] == '\'' {
            let mut j = i + 2;
            while j < chars.len() && chars[j] != '\'' {
                j += 1;
            }
            if j >= chars.len() {
                return Some("unclosed quoted literal");
            }
            if j + 1 < chars.len() && chars[j + 1] == '}' {
                i = j + 2;
                continue;
            }
            i += 1;
            continue;
        }
        // A single `{name}` must be closed with a valid parameter name.
        let mut j = i + 1;
        let mut name = String::new();
        while j < chars.len() && chars[j] != '}' && chars[j] != '{' {
            name.push(chars[j]);
            j += 1;
        }
        if j >= chars.len() || chars[j] != '}' {
            return Some("unclosed '{' interpolation");
        }
        if !is_valid_placeholder_name(&name) {
            return Some("malformed interpolation parameter name");
        }
        i = j + 1;
    }
    None
}

fn contains_executable_html(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.contains("<script") || lower.contains("javascript:") {
        return true;
    }
    HTML_EVENT_HANDLERS.iter().any(|handler| {
        let mut search_from = 0;
        while let Some(relative) = lower[search_from..].find(handler) {
            let mut after = search_from + relative + handler.len();
            while after < lower.len() && lower.as_bytes()[after] == b' ' {
                after += 1;
            }
            if after < lower.len() && lower.as_bytes()[after] == b'=' {
                return true;
            }
            search_from += relative + handler.len();
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-locales-{tag}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_locale(dir: &Path, file_name: &str, content: &str) {
        std::fs::write(dir.join(file_name), content).unwrap();
    }

    fn pack_json(locale: &str, name: &str, messages: &[(&str, &str)]) -> String {
        let mut map = serde_json::Map::new();
        for (key, value) in messages {
            map.insert(
                (*key).to_string(),
                serde_json::Value::String((*value).to_string()),
            );
        }
        serde_json::json!({
            "schemaVersion": 1,
            "locale": locale,
            "name": name,
            "messages": serde_json::Value::Object(map),
        })
        .to_string()
    }

    #[test]
    fn builtin_keys_available() {
        let catalog = LocaleCatalog::load(None, None);

        let en = catalog.snapshot("en");
        assert_eq!(en.locale(), "en");
        assert_eq!(en.get("common.save"), Some("Save"));
        assert_eq!(en.get("common.cancel"), Some("Cancel"));
        assert_eq!(en.get("common.close"), Some("Close"));
        assert_eq!(en.get("settings.language"), Some("Language"));
        assert_eq!(en.get("settings.title"), Some("Settings"));
        assert_eq!(en.get("tray.show_main"), Some("Show main window"));
        assert_eq!(en.get("tray.soundpanel"), Some("Sound panel"));
        assert_eq!(en.get("tray.playback"), Some("Playback"));
        assert_eq!(en.get("tray.quit"), Some("Quit"));
        assert_eq!(en.get("window.playback"), Some("Playback"));

        let ru = catalog.snapshot("ru");
        assert_eq!(ru.locale(), "ru");
        assert_eq!(ru.get("common.save"), Some("Сохранить"));
        assert_eq!(ru.get("common.cancel"), Some("Отмена"));
        assert_eq!(ru.get("common.close"), Some("Закрыть"));
        assert_eq!(ru.get("settings.language"), Some("Язык"));
        assert_eq!(ru.get("settings.title"), Some("Настройки"));
        assert_eq!(ru.get("tray.show_main"), Some("Показать главное окно"));
        assert_eq!(ru.get("tray.soundpanel"), Some("Саундпад"));
        assert_eq!(ru.get("tray.playback"), Some("Управление воспроизведением"));
        assert_eq!(ru.get("tray.quit"), Some("Выход"));
        assert_eq!(ru.get("window.playback"), Some("Воспроизведение"));
    }

    #[test]
    fn third_language_covers_tray_and_ui() {
        let dir = temp_dir("third");
        write_locale(
            &dir,
            "de.json",
            &pack_json(
                "de",
                "Deutsch",
                &[
                    ("tray.show_main", "Hauptfenster anzeigen"),
                    ("tray.soundpanel", "Soundpanel"),
                    ("tray.playback", "Wiedergabe"),
                    ("tray.quit", "Beenden"),
                    ("common.save", "Speichern"),
                    ("common.cancel", "Abbrechen"),
                    ("common.close", "Schließen"),
                    ("settings.language", "Sprache"),
                    ("settings.title", "Einstellungen"),
                    ("window.playback", "Wiedergabe"),
                ],
            ),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        let de = catalog.snapshot("de");
        assert_eq!(de.locale(), "de");
        assert_eq!(de.get("tray.show_main"), Some("Hauptfenster anzeigen"));
        assert_eq!(de.get("tray.soundpanel"), Some("Soundpanel"));
        assert_eq!(de.get("tray.playback"), Some("Wiedergabe"));
        assert_eq!(de.get("tray.quit"), Some("Beenden"));
        assert_eq!(de.get("common.save"), Some("Speichern"));
        assert_eq!(de.get("settings.language"), Some("Sprache"));
        assert_eq!(de.get("window.playback"), Some("Wiedergabe"));

        assert!(de
            .languages()
            .iter()
            .any(|l| l.locale() == "de" && l.name() == "Deutsch"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn partial_fallback() {
        let dir = temp_dir("partial");
        write_locale(
            &dir,
            "de.json",
            &pack_json(
                "de",
                "Deutsch",
                &[("common.save", "Speichern"), ("tray.quit", "Beenden")],
            ),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        let de = catalog.snapshot("de");
        assert_eq!(de.get("common.save"), Some("Speichern"));
        assert_eq!(de.get("tray.quit"), Some("Beenden"));
        assert_eq!(de.get("common.cancel"), Some("Cancel"));
        assert_eq!(de.get("tray.show_main"), Some("Show main window"));
        assert_eq!(de.get("window.playback"), Some("Playback"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn precedence_including_en_override() {
        let resource = temp_dir("resource");
        let config = temp_dir("config");
        write_locale(
            &resource,
            "ru.json",
            &pack_json("ru", "Русский", &[("common.save", "Сохранить из resource")]),
        );
        write_locale(
            &config,
            "ru.json",
            &pack_json("ru", "Русский", &[("common.save", "Сохранить из config")]),
        );
        let catalog = LocaleCatalog::load(Some(resource.as_path()), Some(config.as_path()));

        let ru = catalog.snapshot("ru");
        assert_eq!(ru.get("common.save"), Some("Сохранить из config"));
        assert_eq!(ru.get("common.cancel"), Some("Отмена"));

        let dir = temp_dir("en_override");
        write_locale(
            &dir,
            "en.json",
            &pack_json("en", "English (custom)", &[("common.save", "Save custom")]),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);
        let en = catalog.snapshot("en");
        assert_eq!(en.get("common.save"), Some("Save custom"));
        assert_eq!(en.get("common.cancel"), Some("Cancel"));

        std::fs::remove_dir_all(&resource).ok();
        std::fs::remove_dir_all(&config).ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_key_ignored() {
        let dir = temp_dir("unknown");
        write_locale(
            &dir,
            "de.json",
            &pack_json(
                "de",
                "Deutsch",
                &[("common.save", "Speichern"), ("some.legacy.key", "Alt")],
            ),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        let de = catalog.snapshot("de");
        assert_eq!(de.get("common.save"), Some("Speichern"));
        assert_eq!(de.get("some.legacy.key"), None);
        assert!(!de.messages().contains_key("some.legacy.key"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn malformed_pack_rejected() {
        let dir = temp_dir("malformed");
        write_locale(&dir, "de.json", "{ not valid json");
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        assert!(!catalog.is_locale_available("de"));
        let de = catalog.snapshot("de");
        assert_eq!(de.requested_locale(), "de");
        assert_eq!(de.locale(), "en");
        assert!(catalog
            .diagnostics()
            .iter()
            .any(|d| d.contains("invalid JSON")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn version_mismatch_rejected() {
        let dir = temp_dir("version");
        write_locale(
            &dir,
            "de.json",
            r#"{"schemaVersion":2,"locale":"de","name":"Deutsch","messages":{"common.save":"Speichern"}}"#,
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        assert!(!catalog.is_locale_available("de"));
        assert!(catalog
            .diagnostics()
            .iter()
            .any(|d| d.contains("schemaVersion")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn locale_filename_mismatch_rejected() {
        let dir = temp_dir("mismatch");
        write_locale(
            &dir,
            "de.json",
            &pack_json("fr", "Deutsch", &[("common.save", "Speichern")]),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        assert!(!catalog.is_locale_available("de"));
        assert!(!catalog.is_locale_available("fr"));
        assert!(catalog
            .diagnostics()
            .iter()
            .any(|d| d.contains("does not match")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_placeholder_falls_back_to_english() {
        let dir = temp_dir("placeholder");
        write_locale(
            &dir,
            "de.json",
            &pack_json(
                "de",
                "Deutsch",
                &[
                    ("common.save", "Speichern"),
                    ("common.close", "Schließen {count}"),
                ],
            ),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        let de = catalog.snapshot("de");
        assert_eq!(de.get("common.save"), Some("Speichern"));
        assert_eq!(de.get("common.close"), Some("Close"));
        assert!(catalog
            .diagnostics()
            .iter()
            .any(|d| d.contains("placeholder")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn oversized_file_rejected() {
        let dir = temp_dir("oversized");
        let big = "a".repeat((MAX_FILE_BYTES + 1) as usize);
        write_locale(&dir, "de.json", &big);
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        assert!(!catalog.is_locale_available("de"));
        assert!(catalog
            .diagnostics()
            .iter()
            .any(|d| d.contains("too large")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_requested_language_retains_preference() {
        let catalog = LocaleCatalog::load(None, None);
        let snap = catalog.snapshot("xx");

        assert_eq!(snap.requested_locale(), "xx");
        assert_eq!(snap.locale(), "en");
        assert_eq!(snap.get("common.save"), Some("Save"));
    }

    #[test]
    fn changes_after_catalog_load_ignored() {
        let dir = temp_dir("reload");
        write_locale(
            &dir,
            "de.json",
            &pack_json("de", "Deutsch", &[("common.save", "Speichern")]),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);
        assert_eq!(catalog.snapshot("de").get("common.save"), Some("Speichern"));

        write_locale(
            &dir,
            "de.json",
            &pack_json("de", "Deutsch", &[("common.save", "Neu")]),
        );
        assert_eq!(catalog.snapshot("de").get("common.save"), Some("Speichern"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn discovered_locales_sorted_builtins_available() {
        let dir = temp_dir("sorted");
        write_locale(
            &dir,
            "de.json",
            &pack_json("de", "Deutsch", &[("common.save", "Speichern")]),
        );
        write_locale(
            &dir,
            "fr.json",
            &pack_json("fr", "Français", &[("common.save", "Enregistrer")]),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        let languages = catalog.languages();
        let codes: Vec<&str> = languages.iter().map(|l| l.locale()).collect();
        assert_eq!(codes, vec!["de", "en", "fr", "ru"]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nested_object_message_rejected() {
        let dir = temp_dir("nested");
        write_locale(
            &dir,
            "de.json",
            r#"{"schemaVersion":1,"locale":"de","name":"Deutsch","messages":{"common.save":{"nested":"Speichern"}}}"#,
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        let de = catalog.snapshot("de");
        assert_eq!(de.get("common.save"), Some("Save"));
        assert!(catalog
            .diagnostics()
            .iter()
            .any(|d| d.contains("not a string")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn executable_html_rejected() {
        let dir = temp_dir("html");
        write_locale(
            &dir,
            "de.json",
            &pack_json(
                "de",
                "Deutsch",
                &[
                    ("common.save", "<script>alert(1)</script>"),
                    ("common.cancel", "Abbrechen"),
                ],
            ),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        let de = catalog.snapshot("de");
        assert_eq!(de.get("common.save"), Some("Save"));
        assert_eq!(de.get("common.cancel"), Some("Abbrechen"));
        assert!(catalog.diagnostics().iter().any(|d| d.contains("HTML")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_external_message_falls_back_to_english() {
        let dir = temp_dir("empty-message");
        write_locale(
            &dir,
            "de.json",
            &pack_json(
                "de",
                "Deutsch",
                &[("common.save", ""), ("common.cancel", "Abbrechen")],
            ),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);

        let de = catalog.snapshot("de");
        assert_eq!(de.get("common.save"), Some("Save"));
        assert_eq!(de.get("common.cancel"), Some("Abbrechen"));
        assert!(catalog.diagnostics().iter().any(|d| d.contains("empty")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn placeholder_extraction_ignores_literal_braces() {
        assert_eq!(
            extract_placeholders("You have {count} items"),
            vec!["count"]
        );
        assert_eq!(extract_placeholders("{{template}} {name}"), vec!["name"]);
        assert_eq!(
            extract_placeholders("no placeholders { count }"),
            Vec::<String>::new()
        );
        assert_eq!(
            extract_placeholders("unclosed {brace"),
            Vec::<String>::new()
        );
        assert_eq!(extract_placeholders("100% and $5 {total}"), vec!["total"]);
    }

    #[test]
    fn placeholder_validation_against_english() {
        let en = "You have {count} items".to_string();
        assert!(validate_placeholders("У вас {count} предметов", Some(&en)).is_ok());
        assert!(validate_placeholders("У вас {total} предметов", Some(&en)).is_err());
        assert!(validate_placeholders("У вас предметы", Some(&en)).is_err());
        assert!(validate_placeholders("Нет", None).is_ok());
    }

    #[test]
    fn external_message_requires_exact_placeholder_set() {
        let dir = temp_dir("exact_ok");
        write_locale(
            &dir,
            "de.json",
            &pack_json(
                "de",
                "Deutsch",
                &[
                    ("input_server.error.save", "Fehler beim Speichern: {detail}"),
                    ("tray.quit", "Beenden"),
                ],
            ),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);
        let de = catalog.snapshot("de");
        assert_eq!(
            de.get("input_server.error.save"),
            Some("Fehler beim Speichern: {detail}")
        );
        assert_eq!(de.get("tray.quit"), Some("Beenden"));
        assert!(catalog.diagnostics().is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn external_message_with_mismatched_placeholders_falls_back() {
        let dir = temp_dir("exact_mismatch");
        write_locale(
            &dir,
            "de.json",
            &pack_json(
                "de",
                "Deutsch",
                &[
                    // Missing the `{detail}` parameter used by English.
                    ("input_server.error.save", "Fehler beim Speichern"),
                    // Extra parameter that English does not use.
                    ("ocr.message.save_error", "Fehler: {detail} und {count}"),
                    // Unknown keys are still ignored, never merged.
                    ("some.future.key", "Zukunft"),
                    // A conforming message must still load.
                    ("tray.quit", "Beenden"),
                ],
            ),
        );
        let catalog = LocaleCatalog::load(Some(dir.as_path()), None);
        let de = catalog.snapshot("de");
        assert_eq!(
            de.get("input_server.error.save"),
            Some("Could not save settings: {detail}")
        );
        assert_eq!(
            de.get("ocr.message.save_error"),
            Some("Could not save the settings: {detail}")
        );
        assert!(!de.messages().contains_key("some.future.key"));
        assert_eq!(de.get("tray.quit"), Some("Beenden"));
        assert!(
            catalog
                .diagnostics()
                .iter()
                .any(|d| d.contains("missing {detail}")),
            "missing placeholder must be reported"
        );
        assert!(
            catalog
                .diagnostics()
                .iter()
                .any(|d| d.contains("extra {count}")),
            "extra placeholder must be reported"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn literal_vue_i18n_escaping_remains_usable() {
        let en_messages: BTreeMap<String, String> = [(
            "doc.open".to_string(),
            "Open the {{file}} and see {'{code}'} below".to_string(),
        )]
        .into_iter()
        .collect();
        let raw = r#"{"schemaVersion":1,"locale":"de","name":"Deutsch","messages":{"doc.open":"Öffnen Sie {{file}} und siehe {'{code}'} unten"}}"#;
        let load = parse_pack(raw, "de", Some(&en_messages)).expect("literal escaping must load");
        assert_eq!(
            load.pack.messages["doc.open"],
            "Öffnen Sie {{file}} und siehe {'{code}'} unten"
        );
        assert!(load.diagnostics.is_empty());
    }

    #[test]
    fn missing_locales_directory_is_not_a_diagnostic() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let missing = std::env::temp_dir().join(format!(
            "ttsbard-missing-locales-{}-{unique}",
            std::process::id()
        ));
        assert!(!missing.exists());

        let catalog = LocaleCatalog::load(Some(missing.as_path()), Some(missing.as_path()));
        assert!(
            catalog.diagnostics().is_empty(),
            "a missing locales directory must be silent, got {:?}",
            catalog.diagnostics()
        );
        assert_eq!(catalog.snapshot("ru").locale(), "ru");
        assert!(catalog.is_locale_available("en"));
        assert!(catalog.is_locale_available("ru"));
    }

    #[test]
    fn existing_unreadable_locales_path_keeps_diagnostic() {
        let dir = temp_dir("file-as-dir");
        let file = dir.join("de.json");
        write_locale(
            &dir,
            "de.json",
            &pack_json("de", "Deutsch", &[("tray.quit", "Beenden")]),
        );

        // Point the scan at an existing regular file: read_dir fails with a
        // non-NotFound error, so a diagnostic must be retained.
        let catalog = LocaleCatalog::load(Some(file.as_path()), None);
        assert!(
            catalog
                .diagnostics()
                .iter()
                .any(|d| d.contains("cannot read locales directory")),
            "an existing but unreadable path must keep a diagnostic, got {:?}",
            catalog.diagnostics()
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn builtin_messages_are_valid_safe_and_consistent() {
        fn load(raw: &str) -> BTreeMap<String, String> {
            let value: serde_json::Value =
                serde_json::from_str(raw).expect("builtin pack must be valid JSON");
            let messages = value
                .get("messages")
                .and_then(serde_json::Value::as_object)
                .expect("builtin messages must be an object");
            messages
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        value
                            .as_str()
                            .expect("builtin message value must be a string")
                            .to_string(),
                    )
                })
                .collect()
        }

        let en = load(BUILTIN_EN_JSON);
        let ru = load(BUILTIN_RU_JSON);
        assert!(!en.is_empty(), "embedded English catalog must not be empty");

        for (key, value) in &en {
            assert!(!value.is_empty(), "message {key:?} must not be empty");
            assert!(
                !contains_executable_html(value),
                "English message {key:?} contains executable HTML"
            );
            assert_eq!(
                interpolation_syntax_error(value),
                None,
                "English message {key:?} has invalid interpolation syntax"
            );
        }

        assert_eq!(
            en.len(),
            ru.len(),
            "builtin ru must cover every English key"
        );
        for (key, value) in &ru {
            let en_value = en
                .get(key)
                .unwrap_or_else(|| panic!("builtin ru key {key:?} is missing from English"));
            assert!(!value.is_empty(), "message {key:?} must not be empty");
            assert!(
                !contains_executable_html(value),
                "Russian message {key:?} contains executable HTML"
            );
            assert_eq!(
                interpolation_syntax_error(value),
                None,
                "Russian message {key:?} has invalid interpolation syntax"
            );
            let ru_set: HashSet<String> = extract_placeholders(value).into_iter().collect();
            let en_set: HashSet<String> = extract_placeholders(en_value).into_iter().collect();
            assert_eq!(
                ru_set, en_set,
                "Russian message {key:?} placeholder set must match English"
            );
        }
    }

    #[test]
    fn interpolation_syntax_error_detects_malformed_messages() {
        assert_eq!(interpolation_syntax_error("plain text"), None);
        assert_eq!(interpolation_syntax_error("Hello {name}"), None);
        assert_eq!(interpolation_syntax_error("Use {{docs}} for help"), None);
        assert_eq!(interpolation_syntax_error("Literal {'{brace}'} kept"), None);
        assert!(interpolation_syntax_error("unclosed {name").is_some());
        assert!(interpolation_syntax_error("unclosed {{literal").is_some());
        assert!(interpolation_syntax_error("{bad name}").is_some());
    }

    #[test]
    fn locale_code_rejects_empty_segments_and_uncatalogable_tags() {
        for valid in ["en", "ru", "de", "pt-BR", "zh-Hant", "sr-Latn-RS"] {
            assert!(is_valid_locale_code(valid), "{valid} must be accepted");
        }
        for invalid in [
            "",
            "-",
            "en-",
            "-en",
            "en--BR",
            "a",
            "en_US",
            "e",
            "1en",
            "en/a",
            "en\\a",
            ".en",
            "en.",
            "abcdefghi",
            "ab-abcdefghi",
        ] {
            assert!(
                !is_valid_locale_code(invalid),
                "{invalid:?} must be rejected"
            );
        }
    }

    #[test]
    fn snapshot_serializes_camel_case() {
        let catalog = LocaleCatalog::load(None, None);
        let snap = catalog.snapshot("ru");
        let value = serde_json::to_value(&snap).unwrap();
        let object = value.as_object().unwrap();

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

    #[test]
    fn resource_locale_template_matches_embedded_english() {
        let template = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("locales")
            .join("en.json");
        let raw = std::fs::read_to_string(&template)
            .unwrap_or_else(|error| panic!("release locales template must exist: {error}"));
        assert_eq!(
            raw, BUILTIN_EN_JSON,
            "src-tauri/locales/en.json must mirror the English catalog that ships embedded"
        );
    }
}
