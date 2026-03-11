//! Audio Player
//!
//! Плеер с поддержкой воспроизведения на два устройства одновременно
//! Uses Arc for efficient data sharing between multiple outputs

use cpal::traits::{DeviceTrait, HostTrait};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use std::collections::HashMap;

/// Конфигурация вывода звука
#[derive(Clone, Debug)]
pub struct OutputConfig {
    pub device_id: Option<String>,  // None = устройство по умолчанию
    pub volume: f32,                // 0.0 - 1.0
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
                eprintln!("[AUDIO_PLAYER] Join timeout reached, some threads may still be running");
                break;
            }
            // Примечание: join() блокирующий, но это допустимо для десктопного приложения
            // Потоки должны быстро завершаться после установки stop_flag
            if let Err(e) = handle.join() {
                eprintln!("[AUDIO_PLAYER] Thread join error: {:?}", e);
            }
        }

        // Сбрасываем флаг остановки для нового воспроизведения
        self.stop_flag.store(false, Ordering::SeqCst);

        eprintln!("[AUDIO_PLAYER] Starting dual output playback, data size: {} bytes", mp3_data.len());

        // Проверяем, что хотя бы один вывод включен
        if speaker_config.is_none() && virtual_mic_config.is_none() {
            return Err("No output devices configured".to_string());
        }

        // Получаем конфигурацию громкости для логирования
        let speaker_vol = speaker_config.as_ref().map(|c| c.volume).unwrap_or(0.0);
        let mic_vol = virtual_mic_config.as_ref().map(|c| c.volume).unwrap_or(0.0);
        eprintln!("[AUDIO_PLAYER] Speaker volume: {:.0}%, Virtual mic volume: {:.0}%",
            speaker_vol * 100.0, mic_vol * 100.0);

        // Use Arc to share audio data efficiently (Arc<[u8]> instead of Arc<Vec<u8>> for better cache efficiency)
        let shared_data: Arc<[u8]> = mp3_data.into();

        // Собираем handles новых потоков
        let mut new_handles = Vec::new();

        // Запускаем воспроизведение на динамике
        if let Some(config) = speaker_config {
            let stop_flag = self.stop_flag.clone();
            let data = Arc::clone(&shared_data);  // Cheap Arc clone, not Vec clone
            let devices_cache = cached_devices.clone();

            let handle = thread::spawn(move || {
                eprintln!("[AUDIO_PLAYER] Speaker thread started");
                if let Err(e) = Self::play_to_device(stop_flag, data, config, "Speaker", devices_cache) {
                    eprintln!("[AUDIO_PLAYER] Speaker playback error: {}", e);
                }
                eprintln!("[AUDIO_PLAYER] Speaker thread finished");
            });
            new_handles.push(handle);
        }

        // Запускаем воспроизведение на виртуальном микрофоне
        if let Some(config) = virtual_mic_config {
            let stop_flag = self.stop_flag.clone();
            let data = Arc::clone(&shared_data);  // Cheap Arc clone, not Vec clone
            let devices_cache = cached_devices.clone();

            let handle = thread::spawn(move || {
                eprintln!("[AUDIO_PLAYER] Virtual mic thread started");
                if let Err(e) = Self::play_to_device(stop_flag, data, config, "Virtual Mic", devices_cache) {
                    eprintln!("[AUDIO_PLAYER] Virtual mic playback error: {}", e);
                }
                eprintln!("[AUDIO_PLAYER] Virtual mic thread finished");
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
        mp3_data: Arc<[u8]>,  // Use Arc<[u8]> instead of Arc<Vec<u8>> for better cache efficiency
        config: OutputConfig,
        device_label: &str,
        cached_devices: Option<Arc<RwLock<HashMap<String, cpal::Device>>>>,
    ) -> Result<(), String> {
        // Получаем устройство
        let device = if let Some(device_id) = &config.device_id {
            // Try to use cached devices first
            if let Some(cache) = cached_devices {
                let cached = cache.read();
                if let Some(device) = cached.get(device_id) {
                    eprintln!("[AUDIO_PLAYER] {} using cached device: {}", device_label,
                        device.name().unwrap_or_else(|_| "Unknown".to_string()));
                    let device_clone = device.clone();
                    drop(cached);
                    device_clone
                } else {
                    drop(cached);
                    // Fallback to enumeration if device not in cache
                    let host = cpal::default_host();
                    let devices = host.output_devices()
                        .map_err(|e| format!("Failed to get output devices: {}", e))?;
                    let index: usize = device_id.parse()
                        .map_err(|_| format!("Invalid device ID: {}", device_id))?;
                    devices.into_iter()
                        .nth(index)
                        .ok_or_else(|| format!("Device not found: {}", device_id))?
                }
            } else {
                // No cache, enumerate devices
                let host = cpal::default_host();
                let devices = host.output_devices()
                    .map_err(|e| format!("Failed to get output devices: {}", e))?;
                let index: usize = device_id.parse()
                    .map_err(|_| format!("Invalid device ID: {}", device_id))?;
                devices.into_iter()
                    .nth(index)
                    .ok_or_else(|| format!("Device not found: {}", device_id))?
            }
        } else {
            // Устройство по умолчанию
            let host = cpal::default_host();
            host.default_output_device()
                .ok_or_else(|| "No default output device".to_string())?
        };

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        eprintln!("[AUDIO_PLAYER] {} playing on: {}", device_label, device_name);

        // Создаём выходной поток
        let (_stream, stream_handle) = rodio::OutputStream::try_from_device(&device)
            .map_err(|e| format!("Failed to create output stream: {}", e))?;

        // Декодируем MP3 из байтов (Note: Clone is necessary here because Cursor takes ownership
        // and rodio::Decoder requires 'static. Arc sharing between threads happens before this point.)
        let cursor = Cursor::new(mp3_data.to_vec());  // One-time clone per thread
        let source = rodio::Decoder::new(cursor)
            .map_err(|e| format!("Failed to decode audio: {}", e))?;

        // Создаём sink
        let sink = rodio::Sink::try_new(&stream_handle)
            .map_err(|e| format!("Failed to create sink: {}", e))?;

        // Применяем громкость
        sink.set_volume(config.volume);
        eprintln!("[AUDIO_PLAYER] {} volume set to: {:.2}", device_label, config.volume);

        // Воспроизводим
        sink.append(source);

        // Ждём окончания воспроизведения или остановки
        while !sink.empty() {
            if stop_flag.load(Ordering::SeqCst) {
                eprintln!("[AUDIO_PLAYER] {} playback stopped by flag", device_label);
                sink.stop();
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        if !stop_flag.load(Ordering::SeqCst) {
            eprintln!("[AUDIO_PLAYER] {} playback completed", device_label);
        }

        Ok(())
    }

    /// Остановить воспроизведение
    #[allow(dead_code)]
    pub fn stop(&mut self) {
        eprintln!("[AUDIO_PLAYER] Stopping playback");
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}
