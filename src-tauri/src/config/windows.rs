//! Window settings configuration
//!
//! Manages window positions and appearance settings stored in windows.json

use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::persistence;
use super::validation::{is_valid_hex_color, validate_opacity};

/// Main window settings (position and appearance)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MainWindowSettings {
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default = "default_main_custom_background")]
    pub custom_background: bool,
    #[serde(default = "default_main_opacity")]
    pub opacity: u8,
    #[serde(default = "default_main_bg_color")]
    pub bg_color: String,
    #[serde(default)]
    pub custom_opacity: bool,
    #[serde(default)]
    pub opacity_compact_only: bool,
    #[serde(default = "default_compact_width")]
    pub compact_width: u32,
    #[serde(default = "default_compact_height")]
    pub compact_height: u32,
}

/// Sound panel window settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundPanelWindowSettings {
    pub x: Option<i32>,
    pub y: Option<i32>,
    #[serde(default = "default_soundpanel_opacity")]
    pub opacity: u8,
    #[serde(default = "default_soundpanel_bg_color")]
    pub bg_color: String,
    #[serde(default)]
    pub clickthrough: bool,
    #[serde(default)]
    pub stay_visible: bool,
    #[serde(default = "default_soundpanel_hide_on_blur")]
    pub hide_on_blur: bool,
    #[serde(default = "default_appearance_source")]
    pub appearance_source: String,
}

/// Playback control window settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackWindowSettings {
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default = "default_playback_opacity")]
    pub opacity: u8,
    #[serde(default = "default_playback_bg_color")]
    pub bg_color: String,
    #[serde(default = "default_appearance_source")]
    pub appearance_source: String,
}

/// Global settings that apply to all windows
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GlobalSettings {
    #[serde(default)]
    pub exclude_from_capture: bool,
}

/// All window settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowsSettings {
    #[serde(default)]
    pub global: GlobalSettings,
    #[serde(default)]
    pub main: MainWindowSettings,
    #[serde(default)]
    pub soundpanel: SoundPanelWindowSettings,
    #[serde(default)]
    pub playback: PlaybackWindowSettings,
}

// Default functions
fn default_soundpanel_opacity() -> u8 {
    90
}
fn default_soundpanel_bg_color() -> String {
    "#2a2a2a".to_string()
}
fn default_playback_opacity() -> u8 {
    94
}
fn default_playback_bg_color() -> String {
    "#10131a".to_string()
}
fn default_main_custom_background() -> bool {
    false
}
fn default_main_opacity() -> u8 {
    100
}
fn default_main_bg_color() -> String {
    "#10131a".to_string()
}
fn default_soundpanel_hide_on_blur() -> bool {
    true
}
fn default_appearance_source() -> String {
    "own".to_string()
}
fn default_compact_width() -> u32 {
    450
}
fn default_compact_height() -> u32 {
    400
}

impl Default for MainWindowSettings {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            custom_background: false,
            opacity: 100,
            bg_color: "#10131a".to_string(),
            custom_opacity: false,
            opacity_compact_only: false,
            compact_width: 450,
            compact_height: 400,
        }
    }
}

impl Default for SoundPanelWindowSettings {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            opacity: 90,
            bg_color: "#2a2a2a".to_string(),
            clickthrough: false,
            stay_visible: false,
            hide_on_blur: true,
            appearance_source: "own".to_string(),
        }
    }
}

impl Default for PlaybackWindowSettings {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            opacity: 94,
            bg_color: "#10131a".to_string(),
            appearance_source: "own".to_string(),
        }
    }
}

impl Default for WindowsSettings {
    /// Defaults used for brand-new installations (no windows.json yet).
    ///
    /// New installs default the panels to inheriting the main window's
    /// appearance (`appearance_source = "main"`), while old files that lack the
    /// field deserialize as `"own"` to preserve their existing look.
    fn default() -> Self {
        Self {
            global: GlobalSettings::default(),
            main: MainWindowSettings {
                custom_background: false,
                opacity: 100,
                bg_color: "#10131a".to_string(),
                custom_opacity: false,
                opacity_compact_only: false,
                compact_width: 450,
                compact_height: 400,
                ..MainWindowSettings::default()
            },
            soundpanel: SoundPanelWindowSettings {
                appearance_source: "main".to_string(),
                hide_on_blur: true,
                ..SoundPanelWindowSettings::default()
            },
            playback: PlaybackWindowSettings {
                appearance_source: "main".to_string(),
                ..PlaybackWindowSettings::default()
            },
        }
    }
}

impl WindowsSettings {
    /// Validate all settings and fix invalid values
    pub fn validate(&mut self) {
        // Validate opacity (10..=100 for all windows)
        self.main.opacity = validate_opacity(self.main.opacity);
        self.soundpanel.opacity = validate_opacity(self.soundpanel.opacity);
        self.playback.opacity = validate_opacity(self.playback.opacity);

        // Clamp compact dimensions to 300..500
        self.main.compact_width = self.main.compact_width.clamp(300, 500);
        self.main.compact_height = self.main.compact_height.clamp(300, 500);

        // Validate colors
        if !is_valid_hex_color(&self.main.bg_color) {
            tracing::warn!(bg_color = ?self.main.bg_color, "Invalid main bg_color, using default");
            self.main.bg_color = "#10131a".to_string();
        }
        if !is_valid_hex_color(&self.soundpanel.bg_color) {
            tracing::warn!(bg_color = ?self.soundpanel.bg_color, "Invalid soundpanel bg_color, using default");
            self.soundpanel.bg_color = "#2a2a2a".to_string();
        }
        if !is_valid_hex_color(&self.playback.bg_color) {
            tracing::warn!(bg_color = ?self.playback.bg_color, "Invalid playback bg_color, using default");
            self.playback.bg_color = "#10131a".to_string();
        }
    }
}

/// Manager for window settings with in-memory caching
///
/// Uses RwLock for read-heavy access and a shared global write lock
/// to prevent concurrent updates from overwriting each other.
#[derive(Clone)]
pub struct WindowsManager {
    config_dir: PathBuf,
    cache: Arc<RwLock<WindowsSettings>>,
}

impl WindowsManager {
    /// Create a new WindowsManager with initialized cache
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard");

        fs::create_dir_all(&config_dir).context("Failed to create config dir")?;

        let settings = Self::load_from_disk(&config_dir)?;

        Ok(Self {
            config_dir,
            cache: Arc::new(RwLock::new(settings)),
        })
    }

    /// Get the path to windows.json
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("windows.json")
    }

    /// Load settings from disk (internal, called once at construction)
    fn load_from_disk(config_dir: &Path) -> Result<WindowsSettings> {
        let path = config_dir.join("windows.json");

        if path.exists() {
            let content =
                fs::read_to_string(&path).context("Failed to read windows settings file")?;

            let mut settings = match serde_json::from_str::<WindowsSettings>(&content) {
                Ok(parsed) => parsed,
                Err(e) => {
                    tracing::warn!(error = %e, "windows.json is corrupted, recovering from backup");
                    return persistence::recover_corrupted_json(&path, &WindowsSettings::default());
                }
            };

            settings.validate();
            Ok(settings)
        } else {
            tracing::info!("Windows settings file not found, creating with defaults");
            let settings = WindowsSettings::default();
            let content = serde_json::to_string_pretty(&settings)
                .context("Failed to serialize windows settings")?;
            let _guard = persistence::config_write_lock().lock();
            persistence::write_json_atomically(&path, &content)
                .context("Failed to write windows settings file")?;
            Ok(settings)
        }
    }

    /// Load window settings from cache (fast, no disk I/O)
    ///
    /// This method reads from the in-memory cache protected by RwLock.
    /// Multiple readers can access this concurrently without blocking.
    #[inline]
    pub fn load(&self) -> Result<WindowsSettings> {
        Ok(self.cache.read().clone())
    }

    /// Save window settings to both disk and cache
    ///
    /// Writes atomically to disk and updates the in-memory cache.
    /// Caller must hold `config_write_lock()` to prevent lost updates.
    fn save_locked(
        path: &Path,
        cache: &RwLock<WindowsSettings>,
        settings: &WindowsSettings,
    ) -> Result<()> {
        let content = serde_json::to_string_pretty(settings)
            .context("Failed to serialize windows settings")?;

        persistence::write_json_atomically(path, &content)
            .context("Failed to write windows settings file")?;

        *cache.write() = settings.clone();

        tracing::info!("Windows settings saved and cache updated");
        Ok(())
    }

    /// Atomically update settings under the global write lock.
    ///
    /// Reads the current on-disk snapshot (under the lock to prevent races with
    /// other managers sharing the same file), applies the update, writes
    /// atomically to disk, and updates the cache.
    fn update<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut WindowsSettings),
    {
        let path = self.settings_path();
        let _guard = persistence::config_write_lock().lock();

        let mut settings: WindowsSettings = {
            let content = if path.exists() {
                fs::read_to_string(&path).context("Failed to read windows settings")?
            } else {
                let s = WindowsSettings::default();
                serde_json::to_string_pretty(&s)?
            };
            let mut s: WindowsSettings =
                serde_json::from_str(&content).context("Failed to parse windows settings")?;
            s.validate();
            s
        };

        updater(&mut settings);

        Self::save_locked(&path, &self.cache, &settings)
    }

    // ========== Main Window ==========

    /// Set main window position
    pub fn set_main_position(&self, x: Option<i32>, y: Option<i32>) -> Result<()> {
        self.update(|s| {
            s.main.x = x;
            s.main.y = y;
        })
    }

    /// Get main window appearance (custom_background, effective_opacity, bg_color)
    pub fn get_main_appearance(&self) -> (bool, u8, String) {
        let s = self.cache.read();
        let opacity = if s.main.custom_opacity {
            s.main.opacity
        } else {
            100
        };
        (s.main.custom_background, opacity, s.main.bg_color.clone())
    }

    /// Set whether the main window uses a custom background color
    pub fn set_main_custom_background(&self, value: bool) -> Result<()> {
        self.update(|s| {
            s.main.custom_background = value;
        })
    }

    /// Set main window opacity
    pub fn set_main_opacity(&self, opacity: u8) -> Result<()> {
        self.update(|s| {
            s.main.opacity = validate_opacity(opacity);
        })
    }

    /// Set main window background color
    pub fn set_main_bg_color(&self, color: String) -> Result<()> {
        if !is_valid_hex_color(&color) {
            return Err(anyhow::anyhow!("Invalid hex color format"));
        }
        self.update(|s| {
            s.main.bg_color = color;
        })
    }

    /// Set whether the main window uses custom opacity
    pub fn set_main_custom_opacity(&self, value: bool) -> Result<()> {
        self.update(|s| {
            s.main.custom_opacity = value;
        })
    }

    /// Set whether custom opacity is applied only in compact mode
    pub fn set_main_opacity_compact_only(&self, value: bool) -> Result<()> {
        self.update(|s| {
            s.main.opacity_compact_only = value;
        })
    }

    /// Set main window compact dimensions (clamped to 300..500)
    pub fn set_main_compact_dims(&self, width: u32, height: u32) -> Result<()> {
        self.update(|s| {
            s.main.compact_width = width.clamp(300, 500);
            s.main.compact_height = height.clamp(300, 500);
        })
    }

    /// Get main window compact dimensions
    pub fn get_main_compact_dims(&self) -> (u32, u32) {
        let s = self.cache.read();
        (s.main.compact_width, s.main.compact_height)
    }

    // ========== Sound Panel Window ==========

    /// Set soundpanel window position
    pub fn set_soundpanel_position(&self, x: Option<i32>, y: Option<i32>) -> Result<()> {
        self.update(|s| {
            s.soundpanel.x = x;
            s.soundpanel.y = y;
        })
    }

    /// Get soundpanel window position
    pub fn get_soundpanel_position(&self) -> (Option<i32>, Option<i32>) {
        let s = self.cache.read();
        (s.soundpanel.x, s.soundpanel.y)
    }

    /// Set soundpanel opacity
    pub fn set_soundpanel_opacity(&self, opacity: u8) -> Result<()> {
        self.update(|s| {
            s.soundpanel.opacity = validate_opacity(opacity);
        })
    }

    /// Get soundpanel opacity
    pub fn get_soundpanel_opacity(&self) -> u8 {
        self.cache.read().soundpanel.opacity
    }

    /// Set soundpanel background color
    pub fn set_soundpanel_bg_color(&self, color: String) -> Result<()> {
        if !is_valid_hex_color(&color) {
            return Err(anyhow::anyhow!("Invalid hex color format"));
        }
        self.update(|s| {
            s.soundpanel.bg_color = color;
        })
    }

    /// Get soundpanel background color
    pub fn get_soundpanel_bg_color(&self) -> String {
        self.cache.read().soundpanel.bg_color.clone()
    }

    /// Set soundpanel clickthrough
    pub fn set_soundpanel_clickthrough(&self, clickthrough: bool) -> Result<()> {
        self.update(|s| {
            s.soundpanel.clickthrough = clickthrough;
        })
    }

    /// Get soundpanel clickthrough
    pub fn get_soundpanel_clickthrough(&self) -> bool {
        self.cache.read().soundpanel.clickthrough
    }

    /// Set soundpanel stay_visible
    pub fn set_soundpanel_stay_visible(&self, stay_visible: bool) -> Result<()> {
        self.update(|s| {
            s.soundpanel.stay_visible = stay_visible;
        })
    }

    /// Get soundpanel stay_visible
    pub fn get_soundpanel_stay_visible(&self) -> bool {
        self.cache.read().soundpanel.stay_visible
    }

    /// Set soundpanel hide_on_blur
    pub fn set_soundpanel_hide_on_blur(&self, hide_on_blur: bool) -> Result<()> {
        self.update(|s| {
            s.soundpanel.hide_on_blur = hide_on_blur;
        })
    }

    /// Get soundpanel hide_on_blur
    pub fn get_soundpanel_hide_on_blur(&self) -> bool {
        self.cache.read().soundpanel.hide_on_blur
    }

    /// Set soundpanel appearance source ("own" or "main")
    pub fn set_soundpanel_appearance_source(&self, source: String) -> Result<()> {
        self.update(|s| {
            s.soundpanel.appearance_source = source;
        })
    }

    /// Get soundpanel appearance source ("own" or "main")
    pub fn get_soundpanel_appearance_source(&self) -> String {
        self.cache.read().soundpanel.appearance_source.clone()
    }

    // ========== Playback Control Window ==========

    /// Set playback window position
    pub fn set_playback_position(&self, x: Option<i32>, y: Option<i32>) -> Result<()> {
        self.update(|s| {
            s.playback.x = x;
            s.playback.y = y;
        })
    }

    /// Get playback window position
    pub fn get_playback_position(&self) -> (Option<i32>, Option<i32>) {
        let s = self.cache.read();
        (s.playback.x, s.playback.y)
    }

    /// Set playback opacity
    pub fn set_playback_opacity(&self, opacity: u8) -> Result<()> {
        self.update(|s| {
            s.playback.opacity = validate_opacity(opacity);
        })
    }

    /// Get playback opacity
    pub fn get_playback_opacity(&self) -> u8 {
        self.cache.read().playback.opacity
    }

    /// Set playback background color
    pub fn set_playback_bg_color(&self, color: String) -> Result<()> {
        if !is_valid_hex_color(&color) {
            return Err(anyhow::anyhow!("Invalid hex color format"));
        }
        self.update(|s| {
            s.playback.bg_color = color;
        })
    }

    /// Get playback background color
    pub fn get_playback_bg_color(&self) -> String {
        self.cache.read().playback.bg_color.clone()
    }

    /// Set playback appearance source ("own" or "main")
    pub fn set_playback_appearance_source(&self, source: String) -> Result<()> {
        self.update(|s| {
            s.playback.appearance_source = source;
        })
    }

    /// Get playback appearance source ("own" or "main")
    pub fn get_playback_appearance_source(&self) -> String {
        self.cache.read().playback.appearance_source.clone()
    }

    // ========== Global Settings ==========

    /// Set global exclude from capture
    pub fn set_global_exclude_from_capture(&self, exclude: bool) -> Result<()> {
        tracing::debug!(exclude, "set_global_exclude_from_capture called");
        self.update(|s| {
            s.global.exclude_from_capture = exclude;
        })?;
        tracing::debug!(exclude, "set_global_exclude_from_capture saved");
        Ok(())
    }

    /// Get global exclude from capture
    pub fn get_global_exclude_from_capture(&self) -> bool {
        let value = self.cache.read().global.exclude_from_capture;
        tracing::debug!(value, "get_global_exclude_from_capture from cache");
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: two concurrent setter calls on different fields must both
    /// survive without either update being lost.
    #[test]
    fn concurrent_windows_updates_preserve_both_fields() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-windows-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let windows_path = config_dir.join("windows.json");
        let default_settings = WindowsSettings::default();
        std::fs::write(
            &windows_path,
            serde_json::to_string_pretty(&default_settings).unwrap(),
        )
        .unwrap();

        let manager_a = WindowsManager {
            config_dir: config_dir.clone(),
            cache: Arc::new(RwLock::new(default_settings.clone())),
        };
        let manager_b = WindowsManager {
            config_dir: config_dir.clone(),
            cache: Arc::new(RwLock::new(default_settings)),
        };

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let barrier_a = barrier.clone();
        let barrier_b = barrier.clone();

        let handle_a = std::thread::spawn(move || {
            barrier_a.wait();
            manager_a.set_main_opacity(42).unwrap();
        });
        let handle_b = std::thread::spawn(move || {
            barrier_b.wait();
            manager_b
                .set_soundpanel_position(Some(100), Some(200))
                .unwrap();
        });

        barrier.wait();
        handle_a.join().unwrap();
        handle_b.join().unwrap();

        let content = std::fs::read_to_string(&windows_path).unwrap();
        let settings: WindowsSettings = serde_json::from_str(&content).unwrap();

        assert_eq!(settings.main.opacity, 42);
        assert_eq!(settings.soundpanel.x, Some(100));
        assert_eq!(settings.soundpanel.y, Some(200));

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    fn malformed_windows_json_recovery() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-corrupt-windows-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let windows_path = config_dir.join("windows.json");
        std::fs::write(&windows_path, "{{{not valid json at all").unwrap();

        let settings = WindowsManager::load_from_disk(&config_dir).unwrap();

        assert_eq!(
            settings.main.opacity,
            WindowsSettings::default().main.opacity,
            "recovered settings should use defaults"
        );

        let backup_count = std::fs::read_dir(&config_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |n| n.contains(".bak.") && n.ends_with(".json"))
            })
            .count();
        assert_eq!(backup_count, 1, "a single backup file should exist");

        let new_content = std::fs::read_to_string(&windows_path).unwrap();
        let parsed: WindowsSettings =
            serde_json::from_str(&new_content).expect("recovered windows.json must be valid JSON");
        assert_eq!(parsed, WindowsSettings::default());

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    fn empty_windows_json_recovery() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-empty-windows-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let windows_path = config_dir.join("windows.json");
        std::fs::write(&windows_path, "").unwrap();

        let settings = WindowsManager::load_from_disk(&config_dir).unwrap();

        assert_eq!(settings, WindowsSettings::default());

        let backup_count = std::fs::read_dir(&config_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |n| n.contains(".bak.") && n.ends_with(".json"))
            })
            .count();
        assert_eq!(backup_count, 1);

        let _ = std::fs::remove_dir_all(&config_dir);
    }
}
