//! Audio Module
//!
//! Модуль для управления аудио выводом с поддержкой двух устройств:
//! - Динамик (speaker) для обычного воспроизведения
//! - Виртуальный микрофон (virtual mic) для трансляции в другие приложения

mod device;
mod player;

pub use device::{get_output_devices, get_virtual_mic_devices, OutputDeviceInfo};
pub use player::{AudioPlayer, OutputConfig};
