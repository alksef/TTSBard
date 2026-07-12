//! Window settings configuration
//!
//! Manages window positions and appearance settings stored in windows.json

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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

/// Manager for window settings
pub struct WindowsManager {
    config_dir: PathBuf,
}

impl WindowsManager {
    /// Create a new WindowsManager
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard");

        fs::create_dir_all(&config_dir).context("Failed to create config dir")?;

        Ok(Self { config_dir })
    }

    /// Get the path to windows.json
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("windows.json")
    }

    /// Load window settings from file
    pub fn load(&self) -> Result<WindowsSettings> {
        let path = self.settings_path();

        if path.exists() {
            let content =
                fs::read_to_string(&path).context("Failed to read windows settings file")?;

            let mut settings: WindowsSettings =
                serde_json::from_str(&content).context("Failed to parse windows settings")?;

            settings.validate();
            Ok(settings)
        } else {
            tracing::info!("Settings file not found, creating with defaults");
            let settings = WindowsSettings::default();
            // Save defaults to disk for next time
            self.save(&settings)?;
            Ok(settings)
        }
    }

    /// Save window settings to file
    pub fn save(&self, settings: &WindowsSettings) -> Result<()> {
        let path = self.settings_path();

        let content = serde_json::to_string_pretty(settings)
            .context("Failed to serialize windows settings")?;

        fs::write(&path, content).context("Failed to write windows settings file")?;

        tracing::info!("Settings saved");
        Ok(())
    }

    // ========== Main Window ==========

    /// Set main window position
    pub fn set_main_position(&self, x: Option<i32>, y: Option<i32>) -> Result<()> {
        let mut settings = self.load()?;
        settings.main.x = x;
        settings.main.y = y;
        self.save(&settings)
    }

    /// Get main window appearance (custom_background, effective_opacity, bg_color)
    pub fn get_main_appearance(&self) -> (bool, u8, String) {
        self.load()
            .map(|s| {
                let opacity = if s.main.custom_opacity {
                    s.main.opacity
                } else {
                    100
                };
                (s.main.custom_background, opacity, s.main.bg_color)
            })
            .unwrap_or((false, 100, "#10131a".to_string()))
    }

    /// Set whether the main window uses a custom background color
    pub fn set_main_custom_background(&self, value: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.main.custom_background = value;
        self.save(&settings)
    }

    /// Set main window opacity
    pub fn set_main_opacity(&self, opacity: u8) -> Result<()> {
        let mut settings = self.load()?;
        settings.main.opacity = validate_opacity(opacity);
        self.save(&settings)
    }

    /// Set main window background color
    pub fn set_main_bg_color(&self, color: String) -> Result<()> {
        let mut settings = self.load()?;
        if is_valid_hex_color(&color) {
            settings.main.bg_color = color;
            self.save(&settings)
        } else {
            Err(anyhow::anyhow!("Invalid hex color format"))
        }
    }

    /// Set whether the main window uses custom opacity
    pub fn set_main_custom_opacity(&self, value: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.main.custom_opacity = value;
        self.save(&settings)
    }

    /// Set whether custom opacity is applied only in compact mode
    pub fn set_main_opacity_compact_only(&self, value: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.main.opacity_compact_only = value;
        self.save(&settings)
    }

    /// Set main window compact dimensions (clamped to 300..500)
    pub fn set_main_compact_dims(&self, width: u32, height: u32) -> Result<()> {
        let mut settings = self.load()?;
        settings.main.compact_width = width.clamp(300, 500);
        settings.main.compact_height = height.clamp(300, 500);
        self.save(&settings)
    }

    /// Get main window compact dimensions
    pub fn get_main_compact_dims(&self) -> (u32, u32) {
        self.load()
            .map(|s| (s.main.compact_width, s.main.compact_height))
            .unwrap_or((450, 400))
    }

    // ========== Sound Panel Window ==========

    /// Set soundpanel window position
    pub fn set_soundpanel_position(&self, x: Option<i32>, y: Option<i32>) -> Result<()> {
        let mut settings = self.load()?;
        settings.soundpanel.x = x;
        settings.soundpanel.y = y;
        self.save(&settings)
    }

    /// Get soundpanel window position
    pub fn get_soundpanel_position(&self) -> (Option<i32>, Option<i32>) {
        self.load()
            .map(|s| (s.soundpanel.x, s.soundpanel.y))
            .unwrap_or((None, None))
    }

    /// Set soundpanel opacity
    pub fn set_soundpanel_opacity(&self, opacity: u8) -> Result<()> {
        let mut settings = self.load()?;
        settings.soundpanel.opacity = validate_opacity(opacity);
        self.save(&settings)
    }

    /// Get soundpanel opacity
    pub fn get_soundpanel_opacity(&self) -> u8 {
        self.load().map(|s| s.soundpanel.opacity).unwrap_or(90)
    }

    /// Set soundpanel background color
    pub fn set_soundpanel_bg_color(&self, color: String) -> Result<()> {
        let mut settings = self.load()?;
        if is_valid_hex_color(&color) {
            settings.soundpanel.bg_color = color;
            self.save(&settings)
        } else {
            Err(anyhow::anyhow!("Invalid hex color format"))
        }
    }

    /// Get soundpanel background color
    pub fn get_soundpanel_bg_color(&self) -> String {
        self.load()
            .map(|s| s.soundpanel.bg_color)
            .unwrap_or_else(|_| "#2a2a2a".to_string())
    }

    /// Set soundpanel clickthrough
    pub fn set_soundpanel_clickthrough(&self, clickthrough: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.soundpanel.clickthrough = clickthrough;
        self.save(&settings)
    }

    /// Get soundpanel clickthrough
    pub fn get_soundpanel_clickthrough(&self) -> bool {
        self.load()
            .map(|s| s.soundpanel.clickthrough)
            .unwrap_or(false)
    }

    /// Set soundpanel stay_visible
    pub fn set_soundpanel_stay_visible(&self, stay_visible: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.soundpanel.stay_visible = stay_visible;
        self.save(&settings)
    }

    /// Get soundpanel stay_visible
    pub fn get_soundpanel_stay_visible(&self) -> bool {
        self.load()
            .map(|s| s.soundpanel.stay_visible)
            .unwrap_or(false)
    }

    /// Set soundpanel appearance source ("own" or "main")
    pub fn set_soundpanel_appearance_source(&self, source: String) -> Result<()> {
        let mut settings = self.load()?;
        settings.soundpanel.appearance_source = source;
        self.save(&settings)
    }

    /// Get soundpanel appearance source ("own" or "main")
    pub fn get_soundpanel_appearance_source(&self) -> String {
        self.load()
            .map(|s| s.soundpanel.appearance_source)
            .unwrap_or_else(|_| "own".to_string())
    }

    // ========== Playback Control Window ==========

    /// Set playback window position
    pub fn set_playback_position(&self, x: Option<i32>, y: Option<i32>) -> Result<()> {
        let mut settings = self.load()?;
        settings.playback.x = x;
        settings.playback.y = y;
        self.save(&settings)
    }

    /// Get playback window position
    pub fn get_playback_position(&self) -> (Option<i32>, Option<i32>) {
        self.load()
            .map(|s| (s.playback.x, s.playback.y))
            .unwrap_or((None, None))
    }

    /// Set playback opacity
    pub fn set_playback_opacity(&self, opacity: u8) -> Result<()> {
        let mut settings = self.load()?;
        settings.playback.opacity = validate_opacity(opacity);
        self.save(&settings)
    }

    /// Get playback opacity
    pub fn get_playback_opacity(&self) -> u8 {
        self.load().map(|s| s.playback.opacity).unwrap_or(94)
    }

    /// Set playback background color
    pub fn set_playback_bg_color(&self, color: String) -> Result<()> {
        let mut settings = self.load()?;
        if is_valid_hex_color(&color) {
            settings.playback.bg_color = color;
            self.save(&settings)
        } else {
            Err(anyhow::anyhow!("Invalid hex color format"))
        }
    }

    /// Get playback background color
    pub fn get_playback_bg_color(&self) -> String {
        self.load()
            .map(|s| s.playback.bg_color)
            .unwrap_or_else(|_| "#10131a".to_string())
    }

    /// Set playback appearance source ("own" or "main")
    pub fn set_playback_appearance_source(&self, source: String) -> Result<()> {
        let mut settings = self.load()?;
        settings.playback.appearance_source = source;
        self.save(&settings)
    }

    /// Get playback appearance source ("own" or "main")
    pub fn get_playback_appearance_source(&self) -> String {
        self.load()
            .map(|s| s.playback.appearance_source)
            .unwrap_or_else(|_| "own".to_string())
    }

    // ========== Global Settings ==========

    /// Set global exclude from capture
    pub fn set_global_exclude_from_capture(&self, exclude: bool) -> Result<()> {
        tracing::debug!(exclude, "set_global_exclude_from_capture called");
        let mut settings = self.load()?;
        settings.global.exclude_from_capture = exclude;
        self.save(&settings)?;
        tracing::debug!(exclude, "set_global_exclude_from_capture saved");
        Ok(())
    }

    /// Get global exclude from capture
    pub fn get_global_exclude_from_capture(&self) -> bool {
        let value = self.load()
            .map(|s| {
                tracing::debug!(exclude_from_capture = s.global.exclude_from_capture, "get_global_exclude_from_capture from file");
                s.global.exclude_from_capture
            })
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "get_global_exclude_from_capture error, using default: false");
                false
            });
        tracing::debug!(value, "get_global_exclude_from_capture returning");
        value
    }
}
