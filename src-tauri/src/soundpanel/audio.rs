//! Sound Panel Audio Playback
//!
//! Воспроизведение аудиофайлов для звуковой панели.
//!
//! Использует rodio crate для кроссплатформенного воспроизведения.

use std::fs::File;
use std::io::BufReader;

/// Воспроизвести аудиофайл
///
/// Поддерживаемые форматы: MP3, WAV, OGG, FLAC (через rodio)
pub fn play_audio_file(path: &str) {
    eprintln!("[SOUNDPANEL] Playing audio: {}", path);

    // Проверяем существование файла
    if !std::path::Path::new(path).exists() {
        eprintln!("[SOUNDPANEL] File not found: {}", path);
        return;
    }

    // Используем rodio для воспроизведения
    match play_with_rodio(path) {
        Ok(_) => {
            eprintln!("[SOUNDPANEL] Playback completed");
        }
        Err(e) => {
            eprintln!("[SOUNDPANEL] Failed to play: {}", e);

            // Fallback: попробовать системный способ
            eprintln!("[SOUNDPANEL] Trying fallback method...");
            play_with_fallback(path);
        }
    }
}

/// Воспроизведение через rodio (рекомендуемый способ)
fn play_with_rodio(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // rodio требует, чтобы OutputStream жил всё время воспроизведения
    let (_stream, stream_handle) = rodio::OutputStream::try_default()?;

    // Открыть файл
    let file = File::open(path)?;
    let source = rodio::Decoder::new(BufReader::new(file))?;

    // Создать sink и воспроизвести
    let sink = rodio::Sink::try_new(&stream_handle)?;
    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}

/// Fallback метод воспроизведения (для Windows)
/// Использует PowerShell как fallback если rodio не работает
#[cfg(target_os = "windows")]
fn play_with_fallback(path: &str) {
    use std::process::Command;

    // Экранировать путь для PowerShell
    let escaped_path = path.replace('\\', "\\\\").replace(' ', "\\ ");

    let result = Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-Command",
            &format!(
                "(New-Object Media.SoundPlayer '{}').PlaySync()",
                escaped_path
            ),
        ])
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                eprintln!("[SOUNDPANEL] Fallback playback succeeded");
            } else {
                eprintln!("[SOUNDPANEL] Fallback playback failed: {:?}", output);
            }
        }
        Err(e) => {
            eprintln!("[SOUNDPANEL] Fallback command failed: {}", e);
        }
    }
}

/// Fallback метод для non-Windows (пустой)
#[cfg(not(target_os = "windows"))]
fn play_with_fallback(_path: &str) {
    eprintln!("[SOUNDPANEL] No fallback available for this platform");
}

/// Проверить, является ли файл поддерживаемым аудиоформатом
pub fn is_supported_audio_format(filename: &str) -> bool {
    let filename_lower = filename.to_lowercase();

    let supported_extensions = [
        ".mp3",
        ".wav",
        ".ogg",
        ".flac",
        ".m4a",
        ".aac",
        ".wma",
    ];

    supported_extensions.iter().any(|ext| filename_lower.ends_with(ext))
}

/// Получить длительность аудиофайла (опционально)
/// Возвращает длительность в секундах
#[allow(dead_code)]
pub fn get_audio_duration(_path: &str) -> Option<f64> {
    // Для простоты возвращаем None
    // В реальном коде можно использовать библиотеку symphonia для точного определения
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_audio_format() {
        assert!(is_supported_audio_format("test.mp3"));
        assert!(is_supported_audio_format("test.wav"));
        assert!(is_supported_audio_format("test.OGG")); // case insensitive
        assert!(!is_supported_audio_format("test.txt"));
        assert!(!is_supported_audio_format("test.doc"));
    }
}
