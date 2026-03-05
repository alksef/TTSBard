//! Audio Module
//!
//! Модуль для управления аудио выводом с поддержкой двух устройств:
//! - Динамик (speaker) для обычного воспроизведения
//! - Виртуальный микрофон (virtual mic) для трансляции в другие приложения

mod device;
mod player;
mod settings;

pub use device::{get_output_devices, get_virtual_mic_devices, OutputDeviceInfo};
pub use player::{AudioPlayer, OutputConfig};
pub use settings::{AudioSettings, AudioSettingsManager};

use serde::{Deserialize, Serialize};

/// Информация об аудио устройстве
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
}
