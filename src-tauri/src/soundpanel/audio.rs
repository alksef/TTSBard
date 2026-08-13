//! Sound Panel Audio Playback
//!
//! Воспроизведение аудиофайлов для звуковой панели.
//!
//! Воспроизводит звук на тех же настроенных выходах, что и обычная TTS-озвучка:
//! динамики (когда `audio.speaker_enabled == true`) и виртуальный микрофон
//! (когда задан `audio.virtual_mic_device`). Оба выхода подготавливаются до
//! ожидания завершения любого из них, поэтому sink стартуют одновременно.
//! Ошибка на одном устройстве не отменяет другой выход.

use crate::audio::{resolve_output_device, OutputConfig};
use crate::config::AudioSettings;
use cpal::traits::DeviceTrait;
use std::fs::File;
use std::io::BufReader;
use tracing::{debug, error, info, warn};

/// Построить включённые конфигурации вывода из настроек аудио.
///
/// Возвращает `(speaker, virtual_mic)`. `None` означает, что соответствующий
/// выход выключен/не настроен. Проценты громкости переводятся в 0.0–1.0.
pub(crate) fn build_output_configs(
    audio_settings: &AudioSettings,
) -> (Option<OutputConfig>, Option<OutputConfig>) {
    let speaker_config = if audio_settings.speaker_enabled {
        Some(OutputConfig {
            device_id: audio_settings.speaker_device.clone(),
            volume: audio_settings.speaker_volume as f32 / 100.0,
        })
    } else {
        None
    };

    let virtual_mic_config = audio_settings.virtual_mic_device.as_ref().map(|device_id| {
        OutputConfig {
            device_id: Some(device_id.clone()),
            volume: audio_settings.virtual_mic_volume as f32 / 100.0,
        }
    });

    (speaker_config, virtual_mic_config)
}

/// Воспроизвести аудиофайл
///
/// Поддерживаемые форматы: MP3, WAV, OGG, FLAC (через rodio)
pub fn play_audio_file(path: &str, audio_settings: &AudioSettings) {
    info!(path, "Playing audio");

    // Проверяем существование файла
    if !std::path::Path::new(path).exists() {
        error!(path, "File not found");
        return;
    }

    let (speaker_config, virtual_mic_config) = build_output_configs(audio_settings);

    if speaker_config.is_none() && virtual_mic_config.is_none() {
        warn!("No output devices configured; skipping playback");
        return;
    }

    let path = path.to_string();
    let mut outputs: Vec<(rodio::OutputStream, rodio::Sink)> = Vec::new();

    if let Some(config) = speaker_config {
        match setup_output(&path, &config, "Speaker") {
            Ok(pair) => outputs.push(pair),
            Err(e) => error!(error = %e, "Speaker playback failed"),
        }
    }

    if let Some(config) = virtual_mic_config {
        match setup_output(&path, &config, "Virtual Mic") {
            Ok(pair) => outputs.push(pair),
            Err(e) => error!(error = %e, "Virtual mic playback failed"),
        }
    }

    // Все sink подготовлены и запущены; ждём завершения каждого. Streams
    // остаются живы, пока живёт коллекция.
    for (_stream, sink) in outputs {
        sink.sleep_until_end();
    }
}

/// Подготовить вывод на одном устройстве.
///
/// Резолвит устройство, открывает файл, декодирует отдельным
/// decoder-экземпляром, создаёт stream+sink, выставляет громкость, добавляет
/// source и возвращает пару stream/sink. Вызывающий должен удерживать stream,
/// пока sink живёт. Любая ошибка возвращается вызывающему потоку и не влияет
/// на другой выход.
fn setup_output(
    path: &str,
    config: &OutputConfig,
    label: &str,
) -> Result<(rodio::OutputStream, rodio::Sink), String> {
    let device = resolve_output_device(&config.device_id, &None)?;

    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    info!(label, device_name = %device_name, "Playing on device");

    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let source = rodio::Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Failed to decode audio: {}", e))?;

    let (stream, stream_handle) = rodio::OutputStream::try_from_device(&device)
        .map_err(|e| format!("Failed to create output stream: {}", e))?;

    let sink = rodio::Sink::try_new(&stream_handle)
        .map_err(|e| format!("Failed to create sink: {}", e))?;

    sink.set_volume(config.volume);
    debug!(label, volume = config.volume, "Volume set");

    sink.append(source);

    Ok((stream, sink))
}

/// Проверить, является ли файл поддерживаемым аудиоформатом
pub fn is_supported_audio_format(filename: &str) -> bool {
    let filename_lower = filename.to_lowercase();

    let supported_extensions = [".mp3", ".wav", ".ogg", ".flac", ".m4a", ".aac", ".wma"];

    supported_extensions
        .iter()
        .any(|ext| filename_lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_audio(speaker_enabled: bool, mic: Option<&str>) -> AudioSettings {
        AudioSettings {
            speaker_enabled,
            speaker_device: None,
            speaker_volume: 80,
            virtual_mic_device: mic.map(str::to_string),
            virtual_mic_volume: 60,
        }
    }

    #[test]
    fn test_is_supported_audio_format() {
        assert!(is_supported_audio_format("test.mp3"));
        assert!(is_supported_audio_format("test.wav"));
        assert!(is_supported_audio_format("test.OGG")); // case insensitive
        assert!(!is_supported_audio_format("test.txt"));
        assert!(!is_supported_audio_format("test.doc"));
    }

    #[test]
    fn outputs_both_enabled() {
        let audio = make_audio(true, Some("mic1"));
        let (speaker, mic) = build_output_configs(&audio);

        assert!(speaker.is_some());
        assert_eq!(speaker.as_ref().map(|c| c.volume), Some(0.8));
        assert!(mic.is_some());
        assert_eq!(
            mic.as_ref().and_then(|c| c.device_id.as_deref()),
            Some("mic1")
        );
        assert_eq!(mic.as_ref().map(|c| c.volume), Some(0.6));
    }

    #[test]
    fn outputs_speaker_disabled_leaves_mic_only() {
        let audio = make_audio(false, Some("mic1"));
        let (speaker, mic) = build_output_configs(&audio);

        assert!(speaker.is_none());
        assert!(mic.is_some());
    }

    #[test]
    fn outputs_mic_missing_leaves_speaker_only() {
        let audio = make_audio(true, None);
        let (speaker, mic) = build_output_configs(&audio);

        assert!(speaker.is_some());
        assert!(mic.is_none());
    }
}
