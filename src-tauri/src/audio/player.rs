//! Audio Player
//!
//! Общие функции воспроизведения: [`resolve_output_device`] (резолв id→Device с
//! кешем) и [`open_sink_on_device`] (OutputStream+Sink+WAV/MP3-детект+volume).
//! Используются и `PlaybackManager` (основной путь, plan 74), и `AudioPlayer`.
//!
//! `AudioPlayer` — блокирующий плеер для тестового звука (`test_audio_device`):
//! запускает фоновые потоки dual-output со stop_flag и join. Основной путь
//! воспроизведения фраз идёт через `playback::PlaybackManager`, который хранит
//! Sink и поддерживает очередь/pause/resume/seek.

use cpal::traits::{DeviceTrait, HostTrait};
use parking_lot::RwLock;
use rodio::{Decoder, OutputStream, Sink};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// Конфигурация вывода звука
#[derive(Clone, Debug)]
pub struct OutputConfig {
    pub device_id: Option<String>, // None = устройство по умолчанию
    pub volume: f32,               // 0.0 - 1.0
}

/// Резолв device_id (строка-индекс или None=default) в cpal::Device с кешем.
/// Возвращает Result с человекочитаемой ошибкой.
pub fn resolve_output_device(
    device_id: &Option<String>,
    cached: &Option<Arc<RwLock<HashMap<String, cpal::Device>>>>,
) -> Result<cpal::Device, String> {
    match device_id {
        Some(dev_id) => {
            if let Some(cache) = cached {
                let c = cache.read();
                if let Some(device) = c.get(dev_id) {
                    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                    debug!(device_name = %device_name, "Using cached device");
                    let device_clone = device.clone();
                    drop(c);
                    return Ok(device_clone);
                }
                drop(c);
            }
            let host = cpal::default_host();
            let devices = host
                .output_devices()
                .map_err(|e| format!("Failed to get output devices: {}", e))?;
            let index: usize = dev_id
                .parse()
                .map_err(|_| format!("Invalid device ID: {}", dev_id))?;
            devices
                .into_iter()
                .nth(index)
                .ok_or_else(|| format!("Device not found: {}", dev_id))
        }
        None => {
            let host = cpal::default_host();
            host.default_output_device()
                .ok_or_else(|| "No default output device".to_string())
        }
    }
}

/// Создаёт OutputStream + Sink на устройстве, декодирует data (MP3/WAV auto),
/// выставляет volume, append source. Возвращает (OutputStream, Sink).
/// НЕ ждёт завершения, НЕ трогает потоки — чистая синхронная операция.
pub fn open_sink_on_device(
    device: &cpal::Device,
    data: &[u8],
    volume: f32,
) -> Result<(OutputStream, Sink), String> {
    let (_stream, stream_handle) = OutputStream::try_from_device(device)
        .map_err(|e| format!("Failed to create output stream: {}", e))?;

    let is_wav = data.len() > 4 && &data[0..4] == b"RIFF";
    let format_name = if is_wav { "WAV" } else { "MP3" };
    debug!(
        "Detected {} format, decoding with rodio::Decoder",
        format_name
    );

    let cursor = Cursor::new(data.to_vec());
    let source = Decoder::new(cursor).map_err(|e| format!("Failed to decode audio: {}", e))?;

    let sink =
        Sink::try_new(&stream_handle).map_err(|e| format!("Failed to create sink: {}", e))?;

    sink.set_volume(volume);
    debug!(volume = volume, "Volume set");

    sink.append(source);
    Ok((_stream, sink))
}

/// Аудио плеер с поддержкой dual output
pub struct AudioPlayer {
    stop_flag: Arc<AtomicBool>,
    /// Хранит handle предыдущих потоков для корректного завершения
    active_threads: Vec<JoinHandle<()>>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
            active_threads: Vec::new(),
        }
    }

    /// Воспроизвести MP3 данные асинхронно на одно или два устройства
    /// Uses Arc to share audio data efficiently between multiple outputs
    /// cached_devices: Optional cache of audio devices to avoid enumeration
    pub fn play_mp3_async_dual(
        &mut self,
        mp3_data: Vec<u8>,
        speaker_config: Option<OutputConfig>,
        virtual_mic_config: Option<OutputConfig>,
        cached_devices: Option<Arc<RwLock<HashMap<String, cpal::Device>>>>,
    ) -> Result<(), String> {
        // Останавливаем предыдущее воспроизведение
        self.stop_flag.store(true, Ordering::SeqCst);

        // Ждём завершения старых потоков с реальным таймаутом
        const JOIN_TIMEOUT: Duration = Duration::from_secs(1);
        let deadline = Instant::now() + JOIN_TIMEOUT;
        let mut still_active = Vec::new();

        for handle in self.active_threads.drain(..) {
            if !handle.is_finished() {
                still_active.push(handle);
            }
        }

        // Присоединяем потоки с таймаутом
        for handle in still_active {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                error!("Join timeout reached, some threads may still be running");
                break;
            }
            // Примечание: join() блокирующий, но это допустимо для десктопного приложения
            // Потоки должны быстро завершаться после установки stop_flag
            if let Err(e) = handle.join() {
                error!(error = ?e, "Thread join error");
            }
        }

        // Сбрасываем флаг остановки для нового воспроизведения
        self.stop_flag.store(false, Ordering::SeqCst);

        info!(data_size = mp3_data.len(), "Starting dual output playback");

        // Проверяем, что хотя бы один вывод включен
        if speaker_config.is_none() && virtual_mic_config.is_none() {
            return Err("No output devices configured".to_string());
        }

        // Получаем конфигурацию громкости для логирования
        let speaker_vol = speaker_config.as_ref().map(|c| c.volume).unwrap_or(0.0);
        let mic_vol = virtual_mic_config.as_ref().map(|c| c.volume).unwrap_or(0.0);
        debug!(
            speaker_volume = speaker_vol * 100.0,
            virtual_mic_volume = mic_vol * 100.0,
            "Volume configuration"
        );

        // Use Arc to share audio data efficiently (Arc<[u8]> instead of Arc<Vec<u8>> for better cache efficiency)
        let shared_data: Arc<[u8]> = mp3_data.into();

        // Собираем handles новых потоков
        let mut new_handles = Vec::new();

        // Запускаем воспроизведение на динамике
        if let Some(config) = speaker_config {
            let stop_flag = self.stop_flag.clone();
            let data = Arc::clone(&shared_data); // Cheap Arc clone, not Vec clone
            let devices_cache = cached_devices.clone();

            let handle = thread::spawn(move || {
                debug!("Speaker thread started");
                if let Err(e) =
                    Self::play_to_device(stop_flag, data, config, "Speaker", devices_cache)
                {
                    error!(error = %e, "Speaker playback error");
                }
                debug!("Speaker thread finished");
            });
            new_handles.push(handle);
        }

        // Запускаем воспроизведение на виртуальном микрофоне
        if let Some(config) = virtual_mic_config {
            let stop_flag = self.stop_flag.clone();
            let data = Arc::clone(&shared_data); // Cheap Arc clone, not Vec clone
            let devices_cache = cached_devices.clone();

            let handle = thread::spawn(move || {
                debug!("Virtual mic thread started");
                if let Err(e) =
                    Self::play_to_device(stop_flag, data, config, "Virtual Mic", devices_cache)
                {
                    error!(error = %e, "Virtual mic playback error");
                }
                debug!("Virtual mic thread finished");
            });
            new_handles.push(handle);
        }

        // Сохраняем handles для последующего управления
        self.active_threads = new_handles;

        Ok(())
    }

    /// Воспроизвести на конкретном устройстве (в отдельном потоке)
    fn play_to_device(
        stop_flag: Arc<AtomicBool>,
        mp3_data: Arc<[u8]>,
        config: OutputConfig,
        device_label: &str,
        cached_devices: Option<Arc<RwLock<HashMap<String, cpal::Device>>>>,
    ) -> Result<(), String> {
        let device = resolve_output_device(&config.device_id, &cached_devices)?;

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        info!(device_label = %device_label, device_name = %device_name,
            "Playing on device");

        let (_stream, sink) = open_sink_on_device(&device, &mp3_data, config.volume)?;

        while !sink.empty() {
            if stop_flag.load(Ordering::SeqCst) {
                debug!(device_label = %device_label,
                    "Playback stopped by flag");
                sink.stop();
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        if !stop_flag.load(Ordering::SeqCst) {
            debug!(device_label = %device_label, "Playback completed");
        }

        Ok(())
    }

    /// Воспроизвести тестовый звук на одном устройстве (блокирующе)
    /// Используется для тестирования аудиоустройств
    pub fn play_test_sound_blocking(
        &mut self,
        mp3_data: Vec<u8>,
        config: OutputConfig,
    ) -> Result<(), String> {
        info!("Playing test sound (blocking)");

        let stop_flag = Arc::new(AtomicBool::new(false));
        let data: Arc<[u8]> = mp3_data.into();

        // Use existing play_to_device logic but block until completion
        let result = Self::play_to_device(stop_flag.clone(), data, config, "Test Device", None);

        // Clean up the stop flag
        drop(stop_flag);

        result
    }
    /// Play preview audio with external stop flag (for preview commands).
    /// This is a STATIC method that uses a shared AtomicBool for cancellation.
    pub fn play_preview_with_stop_flag(
        stop_flag: Arc<AtomicBool>,
        audio_data: Vec<u8>,
        config: OutputConfig,
    ) -> Result<(), String> {
        let device = resolve_output_device(&config.device_id, &None)?;
        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        debug!(device_name = %device_name, "Playing preview audio");

        let (_stream, sink) = open_sink_on_device(&device, &audio_data, config.volume)?;

        while !sink.empty() {
            if stop_flag.load(Ordering::SeqCst) {
                debug!("Preview playback stopped by flag");
                sink.stop();
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        debug!("Preview playback finished");
        Ok(())
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}
