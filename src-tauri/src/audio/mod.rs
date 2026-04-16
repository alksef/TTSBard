//! Audio Module
//!
//! Модуль для управления аудио выводом с поддержкой двух устройств:
//! - Динамик (speaker) для обычного воспроизведения
//! - Виртуальный микрофон (virtual mic) для трансляции в другие приложения

mod device;
pub mod effects;
mod player;

pub use device::{get_output_devices, get_virtual_mic_devices, OutputDeviceInfo};
pub use effects::{AudioEffects, apply_effects};
pub use player::{AudioPlayer, OutputConfig};
