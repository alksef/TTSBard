//! Window settings configuration
//!
//! Manages window positions and appearance settings stored in windows.json

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::validation::{is_valid_hex_color, validate_opacity};

/// Main window position
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowPosition {
    pub x: Option<i32>,
    pub y: Option<i32>,
}

/// Floating window settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FloatingWindowSettings {
    pub x: Option<i32>,
    pub y: Option<i32>,
    #[serde(default = "default_floating_opacity")]
    pub opacity: u8,
    #[serde(default = "default_floating_bg_color")]
    pub bg_color: String,
    #[serde(default)]
    pub clickthrough: bool,
    #[serde(default)]
    pub exclude_from_recording: bool,
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
    pub exclude_from_recording: bool,
}

/// All window settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowsSettings {
    pub main: WindowPosition,
    pub floating: FloatingWindowSettings,
    pub soundpanel: SoundPanelWindowSettings,
}

// Default functions
fn default_floating_opacity() -> u8 { 90 }
fn default_floating_bg_color() -> String { "#1e1e1e".to_string() }
fn default_soundpanel_opacity() -> u8 { 90 }
fn default_soundpanel_bg_color() -> String { "#2a2a2a".to_string() }

impl Default for WindowPosition {
    fn default() -> Self {
        Self { x: None, y: None }
    }
}

impl Default for FloatingWindowSettings {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            opacity: 90,
            bg_color: "#1e1e1e".to_string(),
            clickthrough: false,
            exclude_from_recording: false,
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
            exclude_from_recording: false,
        }
    }
}

impl Default for WindowsSettings {
    fn default() -> Self {
        Self {
            main: WindowPosition::default(),
            floating: FloatingWindowSettings::default(),
            soundpanel: SoundPanelWindowSettings::default(),
        }
    }
}

impl WindowsSettings {
    /// Validate all settings and fix invalid values
    pub fn validate(&mut self) {
        // Validate opacity
        self.floating.opacity = validate_opacity(self.floating.opacity);
        self.soundpanel.opacity = validate_opacity(self.soundpanel.opacity);

        // Validate colors
        if !is_valid_hex_color(&self.floating.bg_color) {
            eprintln!("[WINDOWS] Invalid floating bg_color, using default");
            self.floating.bg_color = "#1e1e1e".to_string();
        }
        if !is_valid_hex_color(&self.soundpanel.bg_color) {
            eprintln!("[WINDOWS] Invalid soundpanel bg_color, using default");
            self.soundpanel.bg_color = "#2a2a2a".to_string();
        }
    }
}

/// Manager for window settings
pub struct WindowsManager {
    config_dir: PathBuf,
}

#[allow(dead_code)]
impl WindowsManager {
    /// Create a new WindowsManager
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard");

        fs::create_dir_all(&config_dir)
            .context("Failed to create config dir")?;

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
            let content = fs::read_to_string(&path)
                .context("Failed to read windows settings file")?;

            let mut settings: WindowsSettings = serde_json::from_str(&content)
                .context("Failed to parse windows settings")?;

            settings.validate();
            Ok(settings)
        } else {
            eprintln!("[WINDOWS] Settings file not found, creating with defaults");
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

        fs::write(&path, content)
            .context("Failed to write windows settings file")?;

        eprintln!("[WINDOWS] Settings saved");
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

    /// Get main window position
    pub fn get_main_position(&self) -> (Option<i32>, Option<i32>) {
        self.load()
            .map(|s| (s.main.x, s.main.y))
            .unwrap_or((None, None))
    }

    // ========== Floating Window ==========

    /// Set floating window position
    pub fn set_floating_position(&self, x: Option<i32>, y: Option<i32>) -> Result<()> {
        let mut settings = self.load()?;
        settings.floating.x = x;
        settings.floating.y = y;
        self.save(&settings)
    }

    /// Get floating window position
    pub fn get_floating_position(&self) -> (Option<i32>, Option<i32>) {
        self.load()
            .map(|s| (s.floating.x, s.floating.y))
            .unwrap_or((None, None))
    }

    /// Set floating window opacity
    pub fn set_floating_opacity(&self, opacity: u8) -> Result<()> {
        let mut settings = self.load()?;
        settings.floating.opacity = validate_opacity(opacity);
        self.save(&settings)
    }

    /// Get floating window opacity
    pub fn get_floating_opacity(&self) -> u8 {
        self.load()
            .map(|s| s.floating.opacity)
            .unwrap_or(90)
    }

    /// Set floating window background color
    pub fn set_floating_bg_color(&self, color: String) -> Result<()> {
        let mut settings = self.load()?;
        if is_valid_hex_color(&color) {
            settings.floating.bg_color = color;
            self.save(&settings)
        } else {
            Err(anyhow::anyhow!("Invalid hex color format"))
        }
    }

    /// Get floating window background color
    pub fn get_floating_bg_color(&self) -> String {
        self.load()
            .map(|s| s.floating.bg_color)
            .unwrap_or_else(|_| "#1e1e1e".to_string())
    }

    /// Set floating window clickthrough
    pub fn set_floating_clickthrough(&self, clickthrough: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.floating.clickthrough = clickthrough;
        self.save(&settings)
    }

    /// Get floating window clickthrough
    pub fn get_floating_clickthrough(&self) -> bool {
        self.load()
            .map(|s| s.floating.clickthrough)
            .unwrap_or(false)
    }

    /// Set floating window exclude from recording
    pub fn set_floating_exclude_from_recording(&self, exclude: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.floating.exclude_from_recording = exclude;
        self.save(&settings)
    }

    /// Get floating window exclude from recording
    pub fn get_floating_exclude_from_recording(&self) -> bool {
        self.load()
            .map(|s| s.floating.exclude_from_recording)
            .unwrap_or(false)
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
        self.load()
            .map(|s| s.soundpanel.opacity)
            .unwrap_or(90)
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

    /// Set soundpanel exclude from recording
    pub fn set_soundpanel_exclude_from_recording(&self, exclude: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.soundpanel.exclude_from_recording = exclude;
        self.save(&settings)
    }

    /// Get soundpanel exclude from recording
    pub fn get_soundpanel_exclude_from_recording(&self) -> bool {
        self.load()
            .map(|s| s.soundpanel.exclude_from_recording)
            .unwrap_or(false)
    }
}
