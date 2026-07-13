//! Audio Module
//!
//! Модуль для управления аудио выводом с поддержкой двух устройств:
//! - Динамик (speaker) для обычного воспроизведения
//! - Виртуальный микрофон (virtual mic) для трансляции в другие приложения

pub mod boundary;
mod device;
pub mod dsp;
pub mod effects;
mod player;

pub use boundary::{crossfade, process_boundaries, BoundaryConfig};
pub use device::{get_output_devices, get_virtual_mic_devices, OutputDeviceInfo};
pub use dsp::{process_dsp, CompressorConfig, DspConfig, EqBand, EqConfig, LimiterConfig};
pub use effects::{apply_effects, decode_audio, AudioEffects, AudioPcm};
pub use player::{
    open_sink_on_device, open_sink_on_device_pcm, resolve_output_device, AudioPlayer, OutputConfig,
};
