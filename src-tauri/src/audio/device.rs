//! Audio Device Discovery
//!
//! Обнаружение аудио устройств вывода с помощью cpal

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

/// Информация об устройстве вывода
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDeviceInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
}

/// Получить все устройства вывода звука
pub fn get_output_devices() -> Vec<OutputDeviceInfo> {
    eprintln!("[AUDIO] Scanning for output devices...");

    let host = cpal::default_host();
    let mut devices = Vec::new();

    if let Ok(output_devices) = host.output_devices() {
        let default_device = host.default_output_device();

        for (index, device) in output_devices.enumerate() {
            if let Ok(name) = device.name() {
                let is_default = default_device
                    .as_ref()
                    .and_then(|d| d.name().ok())
                    .as_ref()
                    == Some(&name);

                // Используем индекс как ID, так как имя может содержать специальные символы
                let device_info = OutputDeviceInfo {
                    id: index.to_string(),
                    name,
                    is_default,
                };

                eprintln!("[AUDIO] Found device: {} (default: {})", device_info.name, device_info.is_default);
                devices.push(device_info);
            }
        }
    }

    devices
}

/// Ключевые слова для определения виртуальных устройств
const VIRTUAL_KEYWORDS: &[&str] = &[
    "cable",        // VB-Cable, VoiceMeeter Cable
    "virtual",      // Virtual Speaker, Virtual Audio
    "voicemeeter",  // VoiceMeeter, VAIO
    "vb-audio",     // VB-Audio products
    "aux",          // VoiceMeeter AUX
];

/// Получить только виртуальные аудио устройства
pub fn get_virtual_mic_devices() -> Vec<OutputDeviceInfo> {
    eprintln!("[AUDIO] Scanning for virtual devices...");

    let all_devices = get_output_devices();
    let virtual_devices: Vec<OutputDeviceInfo> = all_devices
        .into_iter()
        .filter(|device| {
            let name_lower = device.name.to_lowercase();
            VIRTUAL_KEYWORDS.iter().any(|keyword| name_lower.contains(keyword))
        })
        .collect();

    eprintln!("[AUDIO] Found {} virtual devices", virtual_devices.len());
    for device in &virtual_devices {
        eprintln!("[AUDIO]   - {}", device.name);
    }

    virtual_devices
}
