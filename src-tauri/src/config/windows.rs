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

/// Compact view mode style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompactView {
    #[default]
    Compact,
    Mono,
}

pub fn deserialize_compact_view<'de, D>(deserializer: D) -> Result<CompactView, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match val.as_str() {
        Some("mono") => Ok(CompactView::Mono),
        _ => Ok(CompactView::Compact),
    }
}

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
    #[serde(
        default = "default_compact_view",
        deserialize_with = "deserialize_compact_view"
    )]
    pub compact_view: CompactView,
    #[serde(default)]
    pub hide_extra_window_buttons: bool,
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
fn default_compact_view() -> CompactView {
    CompactView::Compact
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
            compact_view: CompactView::Compact,
            hide_extra_window_buttons: false,
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

/// Physical inner dimensions contract for compact mode.
///
/// `compact_width` / `compact_height` in `windows.json` are stored as PHYSICAL
/// pixels and describe the INNER size of the main window (Tauri `set_size` /
/// `inner_size`), not the outer frame. The minimum is a physical floor of
/// 300×300 on each axis. The maximum is expressed in LOGICAL pixels
/// (800×630, matching the ordinary window size) and is converted to physical
/// pixels at runtime using the main window's actual `scale_factor`, because
/// config loading has no monitor/DPI context.
pub const COMPACT_MIN_WIDTH_PX: u32 = 300;
pub const COMPACT_MIN_HEIGHT_PX: u32 = 300;
pub const COMPACT_MAX_LOGICAL_WIDTH: u32 = 800;
pub const COMPACT_MAX_LOGICAL_HEIGHT: u32 = 630;

/// Resolve a scale factor to a safe multiplier.
///
/// Non-finite or non-positive values fall back to `1.0` so the logical maximum
/// maps 1:1 to physical pixels instead of producing garbage or panicking.
fn valid_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

/// Convert a logical dimension to physical pixels at `scale_factor`.
///
/// Uses the same rounding as the rest of the app (`round` to nearest integer);
/// invalid scales fall back to `1.0`.
pub fn logical_to_physical(logical: u32, scale_factor: f64) -> u32 {
    let factor = valid_scale_factor(scale_factor);
    (f64::from(logical) * factor).round() as u32
}

/// Physical `(min_width, min_height, max_width, max_height)` compact bounds.
///
/// The maximum is the logical 800×630 scaled to physical pixels; it is never
/// allowed to fall below the physical minimum, so the returned range is always
/// a valid `clamp` target.
pub fn compact_physical_bounds(scale_factor: f64) -> (u32, u32, u32, u32) {
    let max_w =
        logical_to_physical(COMPACT_MAX_LOGICAL_WIDTH, scale_factor).max(COMPACT_MIN_WIDTH_PX);
    let max_h =
        logical_to_physical(COMPACT_MAX_LOGICAL_HEIGHT, scale_factor).max(COMPACT_MIN_HEIGHT_PX);
    (COMPACT_MIN_WIDTH_PX, COMPACT_MIN_HEIGHT_PX, max_w, max_h)
}

/// Clamp a physical inner `(width, height)` to the compact bounds at scale.
pub fn clamp_compact_size(width: u32, height: u32, scale_factor: f64) -> (u32, u32) {
    let (min_w, min_h, max_w, max_h) = compact_physical_bounds(scale_factor);
    (width.clamp(min_w, max_w), height.clamp(min_h, max_h))
}

impl WindowsSettings {
    /// Validate all settings and fix invalid values
    pub fn validate(&mut self) {
        // Validate opacity (10..=100 for all windows)
        self.main.opacity = validate_opacity(self.main.opacity);
        self.soundpanel.opacity = validate_opacity(self.soundpanel.opacity);
        self.playback.opacity = validate_opacity(self.playback.opacity);

        // Compact dimensions are stored as physical inner pixels. Config
        // loading has no monitor/scale context, so only the physical minimum
        // is enforced here; the scale-aware maximum is applied at the runtime
        // command/startup boundaries. Enlarged stored values must survive
        // validation and unrelated settings updates untouched.
        self.main.compact_width = self.main.compact_width.max(COMPACT_MIN_WIDTH_PX);
        self.main.compact_height = self.main.compact_height.max(COMPACT_MIN_HEIGHT_PX);

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
        let config_dir = crate::paths::config_root().context("Failed to get config dir")?;

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

    /// Set main window opacity and enable custom opacity.
    pub fn set_main_opacity(&self, opacity: u8) -> Result<()> {
        self.update(|s| {
            s.main.opacity = validate_opacity(opacity);
            s.main.custom_opacity = true;
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

    /// Set main window compact dimensions (physical inner pixels).
    ///
    /// Only the physical minimum is enforced here; the scale-aware maximum is
    /// applied by the command layer (`commands::window::set_main_compact_dims`)
    /// against the actual main-window scale before this setter is called.
    pub fn set_main_compact_dims(&self, width: u32, height: u32) -> Result<()> {
        self.update(|s| {
            s.main.compact_width = width.max(COMPACT_MIN_WIDTH_PX);
            s.main.compact_height = height.max(COMPACT_MIN_HEIGHT_PX);
        })
    }

    /// Get main window compact dimensions
    pub fn get_main_compact_dims(&self) -> (u32, u32) {
        let s = self.cache.read();
        (s.main.compact_width, s.main.compact_height)
    }

    /// Set main window compact view ('compact' or 'mono')
    pub fn set_main_compact_view(&self, view: CompactView) -> Result<()> {
        self.update(|s| {
            s.main.compact_view = view;
        })
    }

    /// Get main window compact view
    pub fn get_main_compact_view(&self) -> CompactView {
        self.cache.read().main.compact_view
    }

    /// Set whether the extra floating-window buttons are hidden in the title bar
    pub fn set_hide_extra_window_buttons(&self, value: bool) -> Result<()> {
        self.update(|s| {
            s.main.hide_extra_window_buttons = value;
        })
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
            s.soundpanel.hide_on_blur = !stay_visible;
        })
    }

    /// Get soundpanel stay_visible
    pub fn get_soundpanel_stay_visible(&self) -> bool {
        self.cache.read().soundpanel.stay_visible
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

    /// 100% / 150% / 200% scales convert the logical 800×630 maximum to the
    /// expected physical pixels with integer rounding.
    #[test]
    fn compact_max_physical_conversion_at_common_scales() {
        assert_eq!(logical_to_physical(800, 1.0), 800);
        assert_eq!(logical_to_physical(630, 1.0), 630);

        assert_eq!(logical_to_physical(800, 1.5), 1200);
        assert_eq!(logical_to_physical(630, 1.5), 945);

        assert_eq!(logical_to_physical(800, 2.0), 1600);
        assert_eq!(logical_to_physical(630, 2.0), 1260);

        assert_eq!(compact_physical_bounds(1.0), (300, 300, 800, 630));
        assert_eq!(compact_physical_bounds(1.5), (300, 300, 1200, 945));
        assert_eq!(compact_physical_bounds(2.0), (300, 300, 1600, 1260));
    }

    /// Invalid (non-finite / non-positive) scales fall back to 1.0.
    #[test]
    fn invalid_scale_factor_falls_back_to_1_0() {
        for scale in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.5] {
            assert_eq!(logical_to_physical(800, scale), 800, "scale {scale}");
            assert_eq!(logical_to_physical(630, scale), 630, "scale {scale}");
        }
        assert_eq!(compact_physical_bounds(f64::NAN), (300, 300, 800, 630));
    }

    /// Compact clamping pulls too-small sizes up to the physical minimum and
    /// too-large sizes down to the scale-aware maximum.
    #[test]
    fn clamp_compact_size_small_and_large() {
        // Small sizes clamp to the physical minimum.
        assert_eq!(clamp_compact_size(100, 100, 1.0), (300, 300));
        // Large sizes clamp to the logical maximum scaled to physical.
        assert_eq!(clamp_compact_size(5000, 5000, 1.0), (800, 630));
        assert_eq!(clamp_compact_size(5000, 5000, 1.5), (1200, 945));
        // In-range sizes pass through unchanged.
        assert_eq!(clamp_compact_size(450, 400, 1.0), (450, 400));
    }

    /// A too-low maximum (scale < min/logical ratio) never dips below the
    /// physical minimum, keeping the returned range clamp-safe.
    #[test]
    fn compact_bounds_max_never_below_min() {
        let (min_w, min_h, max_w, max_h) = compact_physical_bounds(0.1);
        assert!(max_w >= min_w);
        assert!(max_h >= min_h);
    }

    /// Legacy 450×400 survives `validate` unchanged.
    #[test]
    fn legacy_compact_dims_survive_validate() {
        let mut settings = WindowsSettings::default();
        settings.main.compact_width = 450;
        settings.main.compact_height = 400;
        settings.validate();
        assert_eq!(settings.main.compact_width, 450);
        assert_eq!(settings.main.compact_height, 400);
    }

    /// `validate` enforces only the physical minimum, bumping tiny values up
    /// but never capping enlarged values.
    #[test]
    fn validate_enforces_physical_minimum_only() {
        let mut settings = WindowsSettings::default();
        settings.main.compact_width = 100;
        settings.main.compact_height = 200;
        settings.validate();
        assert_eq!(settings.main.compact_width, 300);
        assert_eq!(settings.main.compact_height, 300);
    }

    /// Enlarged 1200×945 survives validation, an unrelated settings update,
    /// and a serialize/reload round-trip without being rewritten back to 500.
    #[test]
    fn enlarged_compact_dims_survive_validate_update_and_reload() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-compact-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let mut settings = WindowsSettings::default();
        settings.main.compact_width = 1200;
        settings.main.compact_height = 945;
        settings.validate();
        assert_eq!(settings.main.compact_width, 1200);
        assert_eq!(settings.main.compact_height, 945);

        let windows_path = config_dir.join("windows.json");
        std::fs::write(
            &windows_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let manager = WindowsManager {
            config_dir: config_dir.clone(),
            cache: Arc::new(RwLock::new(settings)),
        };

        // Unrelated update must not rewrite the enlarged compact dimensions.
        manager.set_main_opacity(42).unwrap();

        let content = std::fs::read_to_string(&windows_path).unwrap();
        let reloaded: WindowsSettings = serde_json::from_str(&content).unwrap();
        assert_eq!(reloaded.main.compact_width, 1200);
        assert_eq!(reloaded.main.compact_height, 945);
        assert_eq!(reloaded.main.opacity, 42);
        assert!(reloaded.main.custom_opacity);

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    fn soundpanel_pin_updates_visibility_flags_together() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-windows-pin-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let defaults = WindowsSettings::default();
        std::fs::write(
            config_dir.join("windows.json"),
            serde_json::to_string_pretty(&defaults).unwrap(),
        )
        .unwrap();
        let manager = WindowsManager {
            config_dir: config_dir.clone(),
            cache: Arc::new(RwLock::new(defaults)),
        };

        manager.set_soundpanel_stay_visible(true).unwrap();
        assert!(manager.get_soundpanel_stay_visible());
        assert!(!manager.get_soundpanel_hide_on_blur());

        manager.set_soundpanel_stay_visible(false).unwrap();
        assert!(!manager.get_soundpanel_stay_visible());
        assert!(manager.get_soundpanel_hide_on_blur());

        let _ = std::fs::remove_dir_all(&config_dir);
    }

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
        assert!(settings.main.custom_opacity);
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
                    .is_some_and(|n| n.contains(".bak.") && n.ends_with(".json"))
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
                    .is_some_and(|n| n.contains(".bak.") && n.ends_with(".json"))
            })
            .count();
        assert_eq!(backup_count, 1);

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    /// Legacy windows.json without `hide_extra_window_buttons` deserializes with
    /// the field defaulting to `false` while unrelated settings stay intact.
    #[test]
    fn legacy_windows_json_missing_hide_extra_window_buttons_defaults_false() {
        let legacy = serde_json::json!({
            "main": {
                "x": 10,
                "y": 20,
                "custom_background": true,
                "opacity": 80,
                "bg_color": "#123456",
                "custom_opacity": true,
                "opacity_compact_only": false,
                "compact_width": 640,
                "compact_height": 520
            }
        });

        let settings: WindowsSettings = serde_json::from_value(legacy).unwrap();

        assert!(!settings.main.hide_extra_window_buttons);
        assert_eq!(settings.main.x, Some(10));
        assert_eq!(settings.main.opacity, 80);
        assert_eq!(settings.main.compact_width, 640);
        assert_eq!(settings.main.compact_height, 520);
    }

    /// `set_hide_extra_window_buttons` persists true and false round-trips
    /// without disturbing unrelated main-window settings.
    #[test]
    fn hide_extra_window_buttons_round_trips_true_and_false() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-windows-buttons-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let mut settings = WindowsSettings::default();
        settings.main.custom_background = true;
        settings.main.compact_width = 640;
        settings.main.compact_height = 520;
        let windows_path = config_dir.join("windows.json");
        std::fs::write(
            &windows_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let manager = WindowsManager {
            config_dir: config_dir.clone(),
            cache: Arc::new(RwLock::new(settings)),
        };

        manager.set_hide_extra_window_buttons(true).unwrap();
        assert!(manager.load().unwrap().main.hide_extra_window_buttons);

        let content = std::fs::read_to_string(&windows_path).unwrap();
        let persisted: WindowsSettings = serde_json::from_str(&content).unwrap();
        assert!(persisted.main.hide_extra_window_buttons);
        // Unrelated settings survive the update.
        assert!(persisted.main.custom_background);
        assert_eq!(persisted.main.compact_width, 640);
        assert_eq!(persisted.main.compact_height, 520);

        manager.set_hide_extra_window_buttons(false).unwrap();
        assert!(!manager.load().unwrap().main.hide_extra_window_buttons);

        let content = std::fs::read_to_string(&windows_path).unwrap();
        let persisted: WindowsSettings = serde_json::from_str(&content).unwrap();
        assert!(!persisted.main.hide_extra_window_buttons);
        assert!(persisted.main.custom_background);
        assert_eq!(persisted.main.compact_width, 640);
        assert_eq!(persisted.main.compact_height, 520);

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    /// CompactView round-trips correctly for both compact and mono values.
    #[test]
    fn compact_view_round_trips_compact_and_mono() {
        for (view, expected_json) in [
            (CompactView::Compact, "\"compact\""),
            (CompactView::Mono, "\"mono\""),
        ] {
            let serialized = serde_json::to_string(&view).unwrap();
            assert_eq!(serialized, expected_json);
            let deserialized: CompactView = serde_json::from_str(&serialized).unwrap();
            assert_eq!(deserialized, view);
        }
    }

    /// Legacy windows.json without `compact_view` deserializes with default Compact.
    #[test]
    fn legacy_windows_json_missing_compact_view_defaults_to_compact() {
        let json = r#"{
            "main": {
                "compact_width": 640,
                "compact_height": 520
            }
        }"#;
        let settings: WindowsSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.main.compact_view, CompactView::Compact);
        assert_eq!(settings.main.compact_width, 640);
        assert_eq!(settings.main.compact_height, 520);
    }

    /// Unknown or corrupted `compact_view` values normalize safely to Compact.
    #[test]
    fn windows_json_unknown_compact_view_normalizes_to_compact() {
        for raw in [
            "\"unknown\"",
            "\"ultra\"",
            "\"\"",
            "123",
            "null",
            "true",
            "{\"invalid\": true}",
        ] {
            let json = format!(
                r#"{{
                    "main": {{
                        "compact_view": {},
                        "compact_width": 500,
                        "compact_height": 450
                    }}
                }}"#,
                raw
            );
            let settings: WindowsSettings = serde_json::from_str(&json).unwrap();
            assert_eq!(
                settings.main.compact_view,
                CompactView::Compact,
                "Failed for raw value: {}",
                raw
            );
        }

        // Explicit "mono" deserializes to Mono
        let mono_json = r#"{
            "main": {
                "compact_view": "mono"
            }
        }"#;
        let settings: WindowsSettings = serde_json::from_str(mono_json).unwrap();
        assert_eq!(settings.main.compact_view, CompactView::Mono);
    }

    /// Unrelated setter calls preserve compact_view and shared compact dimensions.
    #[test]
    fn unrelated_windows_update_preserves_compact_view_and_enlarged_dims() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-windows-mono-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let mut settings = WindowsSettings::default();
        settings.main.compact_width = 1200;
        settings.main.compact_height = 945;
        settings.main.compact_view = CompactView::Mono;
        let windows_path = config_dir.join("windows.json");
        std::fs::write(
            &windows_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let manager = WindowsManager {
            config_dir: config_dir.clone(),
            cache: Arc::new(RwLock::new(settings)),
        };

        // Unrelated update (opacity)
        manager.set_main_opacity(75).unwrap();

        let content = std::fs::read_to_string(&windows_path).unwrap();
        let persisted: WindowsSettings = serde_json::from_str(&content).unwrap();
        assert_eq!(persisted.main.compact_view, CompactView::Mono);
        assert_eq!(persisted.main.compact_width, 1200);
        assert_eq!(persisted.main.compact_height, 945);
        assert_eq!(persisted.main.opacity, 75);

        // Toggle compact_view through manager
        manager.set_main_compact_view(CompactView::Compact).unwrap();
        assert_eq!(manager.get_main_compact_view(), CompactView::Compact);

        let content = std::fs::read_to_string(&windows_path).unwrap();
        let persisted: WindowsSettings = serde_json::from_str(&content).unwrap();
        assert_eq!(persisted.main.compact_view, CompactView::Compact);
        assert_eq!(persisted.main.compact_width, 1200);
        assert_eq!(persisted.main.compact_height, 945);

        // Set back to Mono
        manager.set_main_compact_view(CompactView::Mono).unwrap();
        assert_eq!(manager.get_main_compact_view(), CompactView::Mono);

        let content = std::fs::read_to_string(&windows_path).unwrap();
        let persisted: WindowsSettings = serde_json::from_str(&content).unwrap();
        assert_eq!(persisted.main.compact_view, CompactView::Mono);

        let _ = std::fs::remove_dir_all(&config_dir);
    }
}
