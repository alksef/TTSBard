//! Audio Settings
//!
//! Управление настройками аудио вывода

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Настройки аудио вывода
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSettings {
    // Настройки динамика
    #[serde(default)]
    pub speaker_device: Option<String>,  // None = устройство по умолчанию
    #[serde(default = "default_speaker_enabled")]
    pub speaker_enabled: bool,
    #[serde(default = "default_speaker_volume")]
    pub speaker_volume: u8,  // 0-100

    // Настройки виртуального микрофона
    #[serde(default)]
    pub virtual_mic_device: Option<String>,
    #[serde(default = "default_virtual_mic_volume")]
    pub virtual_mic_volume: u8,  // 0-100
}

fn default_speaker_enabled() -> bool { true }
fn default_speaker_volume() -> u8 { 80 }
fn default_virtual_mic_volume() -> u8 { 100 }

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            speaker_device: None,
            speaker_enabled: true,
            speaker_volume: 80,
            virtual_mic_device: None,
            virtual_mic_volume: 100,
        }
    }
}

/// Менеджер настроек аудио
pub struct AudioSettingsManager {
    config_dir: PathBuf,
}

impl AudioSettingsManager {
    /// Создать новый менеджер настроек
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard");

        eprintln!("[AUDIO_SETTINGS] Config directory: {:?}", config_dir);

        fs::create_dir_all(&config_dir)
            .context("Failed to create config dir")?;

        Ok(Self { config_dir })
    }

    /// Путь к файлу настроек
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("audio_settings.json")
    }

    /// Загрузить настройки
    pub fn load(&self) -> Result<AudioSettings> {
        let path = self.settings_path();

        if path.exists() {
            eprintln!("[AUDIO_SETTINGS] Loading settings from: {:?}", path);
            let content = fs::read_to_string(&path)
                .context("Failed to read settings file")?;

            let settings: AudioSettings = serde_json::from_str(&content)
                .context("Failed to parse settings")?;

            eprintln!("[AUDIO_SETTINGS] Settings loaded: speaker_enabled={}, virtual_mic={:?}",
                settings.speaker_enabled, settings.virtual_mic_device);
            Ok(settings)
        } else {
            eprintln!("[AUDIO_SETTINGS] Settings file not found, using defaults");
            Ok(AudioSettings::default())
        }
    }

    /// Сохранить настройки
    pub fn save(&self, settings: &AudioSettings) -> Result<()> {
        let path = self.settings_path();

        let content = serde_json::to_string_pretty(settings)
            .context("Failed to serialize settings")?;

        fs::write(&path, content)
            .context("Failed to write settings file")?;

        eprintln!("[AUDIO_SETTINGS] Settings saved");
        Ok(())
    }

    /// Установить устройство динамика
    pub fn set_speaker_device(&self, device_id: Option<String>) -> Result<()> {
        let settings = self.load()?;
        let mut new_settings = settings.clone();
        new_settings.speaker_device = device_id;
        self.save(&new_settings)
    }

    /// Включить/выключить динамик
    pub fn set_speaker_enabled(&self, enabled: bool) -> Result<()> {
        let settings = self.load()?;
        let mut new_settings = settings.clone();
        new_settings.speaker_enabled = enabled;
        self.save(&new_settings)
    }

    /// Установить громкость динамика
    pub fn set_speaker_volume(&self, volume: u8) -> Result<()> {
        let settings = self.load()?;
        let mut new_settings = settings.clone();
        new_settings.speaker_volume = volume.clamp(0, 100);
        self.save(&new_settings)
    }

    /// Установить устройство виртуального микрофона
    pub fn set_virtual_mic_device(&self, device_id: Option<String>) -> Result<()> {
        let settings = self.load()?;
        let mut new_settings = settings.clone();
        new_settings.virtual_mic_device = device_id;
        self.save(&new_settings)
    }

    /// Установить громкость виртуального микрофона
    pub fn set_virtual_mic_volume(&self, volume: u8) -> Result<()> {
        let settings = self.load()?;
        let mut new_settings = settings.clone();
        new_settings.virtual_mic_volume = volume.clamp(0, 100);
        self.save(&new_settings)
    }

    /// Включить виртуальный микрофон
    pub fn enable_virtual_mic(&self) -> Result<()> {
        let settings = self.load()?;
        // Если устройство не выбрано, возвращаем ошибку
        if settings.virtual_mic_device.is_none() {
            return Err(anyhow::anyhow!("Virtual mic device not selected"));
        }
        // В текущей реализации virtual mic считается включенным, если устройство выбрано
        self.save(&settings)
    }

    /// Выключить виртуальный микрофон
    pub fn disable_virtual_mic(&self) -> Result<()> {
        let mut settings = self.load()?;
        settings.virtual_mic_device = None;
        self.save(&settings)
    }
}
