//! Audio Device Discovery
//!
//! Обнаружение аудио устройств вывода с помощью cpal

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

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
    debug!("Scanning for output devices");

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

                debug!(device_name = device_info.name, is_default = device_info.is_default,
                    "Found device");
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
    debug!("Scanning for virtual devices");

    let all_devices = get_output_devices();
    let virtual_devices: Vec<OutputDeviceInfo> = all_devices
        .into_iter()
        .filter(|device| {
            let name_lower = device.name.to_lowercase();
            VIRTUAL_KEYWORDS.iter().any(|keyword| name_lower.contains(keyword))
        })
        .collect();

    info!(count = virtual_devices.len(),
        "Found virtual devices");
    for device in &virtual_devices {
        debug!(device_name = device.name, "Virtual device");
    }

    virtual_devices
}
