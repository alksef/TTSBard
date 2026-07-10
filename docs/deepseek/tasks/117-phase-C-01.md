# Task 117-phase-C-01: Рефакторинг TTS Pipeline

План: `docs/plans/117-2026-07-11-appstate-decomposition-and-commands-refactoring.md` (читать обязательно).

## Описание задачи
Функция `speak_text_internal` в `src-tauri/src/commands/mod.rs` сейчас занимает более 160 строк монолитного кода бэкенда.
Наша цель — вынести этапы генерации звука из текста в структурированный конвейер (pipeline) в новом файле `src-tauri/src/commands/tts_pipeline.rs`.

---

## Шаги реализации

### 1. Создать `src-tauri/src/commands/tts_pipeline.rs`
Реализуй этапы в виде последовательных и тестируемых функций:

```rust
use crate::state::AppState;
use crate::config::{AppSettings, SettingsManager};
use crate::events::AppEvent;
use crate::audio::{OutputConfig, AudioEffects, apply_effects};
use tracing::{info, warn, error, debug};
use std::sync::Arc;

/// 1. Этап предварительной подготовки текста (препроцессор + замена чисел)
pub fn preprocess_text(state: &AppState, text: &str) -> String {
    let text = if let Some(preprocessor) = state.editor.get_preprocessor() {
        let processed = preprocessor.process(text);
        if processed != text {
            debug!(text, processed, "Replacements applied");
        }
        processed
    } else {
        text.to_string()
    };
    crate::preprocessor::process_numbers(&text)
}

/// 2. Этап AI-исправления грамматики (с безопасным fallback)
pub async fn ai_correct_text(state: &AppState, text: &str, settings: &AppSettings) -> String {
    if !settings.editor.ai {
        return text.to_string();
    }

    match state.get_or_create_ai_client(&settings.ai, &settings.tts.network) {
        Ok(client) => {
            match client.correct(text, &settings.ai.prompt).await {
                Ok(corrected) => {
                    if corrected != text {
                        info!(original = text.len(), corrected = corrected.len(), "AI correction applied");
                    }
                    corrected
                }
                Err(e) => {
                    warn!("AI correction failed, using original text: {}", e);
                    text.to_string()
                }
            }
        }
        Err(e) => {
            warn!("AI client not available, skipping correction: {}", e);
            text.to_string()
        }
    }
}

/// 3. Этап синтеза аудиоданных через выбранный TTS-провайдер
pub async fn synthesize_audio(state: &AppState, text: &str) -> Result<Vec<u8>, String> {
    let provider = {
        let providers = state.tts_providers.lock();
        providers.as_ref()
            .ok_or_else(|| {
                error!("TTS provider not initialized");
                "TTS provider не инициализирован. Выберите провайдер в настройках.".to_string()
            })?
            .clone()
    };

    let audio_data = provider.synthesize(text).await
        .map_err(|e| {
            error!(error = %e, "synthesize() error");
            format!("Ошибка синтеза: {}", e)
        })?;

    debug!(bytes = audio_data.len(), "Audio synthesized");
    Ok(audio_data)
}

/// 4. Этап применения аудио-эффектов (pitch, speed, volume)
pub fn apply_audio_effects(audio_data: Vec<u8>, settings: &AppSettings) -> Result<Vec<u8>, String> {
    if !settings.audio_effects.enabled {
        return Ok(audio_data);
    }

    let effects = AudioEffects::new(
        settings.audio_effects.pitch,
        settings.audio_effects.speed,
        settings.audio_effects.volume,
    );

    let original_len = audio_data.len();
    match apply_effects(audio_data, &effects) {
        Ok(processed) => {
            debug!(original = original_len, processed = processed.len(), "Audio effects applied");
            Ok(processed)
        }
        Err(e) => {
            error!(error = %e, "Failed to apply audio effects");
            Err(format!("Не удалось применить аудио эффекты: {}", e))
        }
    }
}

/// 5. Отправка звука в плеер и запись истории
pub fn enqueue_and_record(state: &AppState, text: String, audio_data: Vec<u8>, settings: &AppSettings) -> Result<(), String> {
    let audio_settings = &settings.audio;
    let effects_volume = if settings.audio_effects.enabled {
        Some(AudioEffects::new(
            settings.audio_effects.pitch,
            settings.audio_effects.speed,
            settings.audio_effects.volume,
        ).volume_factor())
    } else {
        None
    };

    let speaker_config = if audio_settings.speaker_enabled {
        let base_volume = audio_settings.speaker_volume as f32 / 100.0;
        let final_volume = effects_volume.map(|ev| base_volume * ev).unwrap_or(base_volume);
        Some(OutputConfig {
            device_id: audio_settings.speaker_device.clone(),
            volume: final_volume,
        })
    } else {
        None
    };

    let virtual_mic_config = audio_settings.virtual_mic_device.as_ref().map(|device_id| {
        let base_volume = audio_settings.virtual_mic_volume as f32 / 100.0;
        let final_volume = effects_volume.map(|ev| base_volume * ev).unwrap_or(base_volume);
        Some(OutputConfig {
            device_id: Some(device_id.clone()),
            volume: final_volume,
        })
    }).flatten();

    if speaker_config.is_none() && virtual_mic_config.is_none() {
        return Err("Аудиовывод и виртуальный микрофон выключены. Включите хотя бы один вывод.".to_string());
    }

    if let Some(pb) = state.playback_manager.lock().as_ref() {
        pb.update_audio_config(speaker_config, virtual_mic_config);
        let phrase_id = uuid::Uuid::new_v4().to_string();
        info!(target: "playback", "Enqueueing phrase to PlaybackManager");
        let enqueued = pb.enqueue(phrase_id, text.clone(), audio_data);
        if !enqueued {
            warn!("Playback queue full, phrase dropped: {}", text);
            return Err("Очередь воспроизведения переполнена. Попробуйте позже.".to_string());
        }
        if let Some(hm) = state.editor.history_manager.lock().as_ref() {
            hm.record_phrase(&text);
        }
        Ok(())
    } else {
        Err("Плеер не инициализирован".to_string())
    }
}
```

Не забудь импортировать в `tts_pipeline.rs` все типы, которые используются в этих функциях. Зарегистрируй новый модуль в `commands/mod.rs` (`pub mod tts_pipeline;`).

---

### 2. `src-tauri/src/commands/mod.rs` — Обновление `speak_text_internal`
Перепиши функцию `speak_text_internal`, собрав её из шагов пайплайна:

```rust
pub async fn speak_text_internal(state: &AppState, text: String) -> Result<(), String> {
    info!(text, "Starting TTS Pipeline");

    if text.trim().is_empty() {
        return Err("Текст не может быть пустым".to_string());
    }

    let settings_manager = SettingsManager::new()
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // 1. Парсинг префикса
    let prefix_result = crate::preprocessor::parse_prefix(&text);
    let text = prefix_result.text;
    state.set_prefix_flags(prefix_result.skip_twitch, prefix_result.skip_webview);

    // 2. Препроцессинг текста
    let text = tts_pipeline::preprocess_text(state, &text);

    // 3. AI-коррекция
    let text = tts_pipeline::ai_correct_text(state, &text, &settings).await;

    // 4. Синтез аудио
    let audio_data = tts_pipeline::synthesize_audio(state, &text).await?;

    // 5. Применение аудиоэффектов
    let audio_data = tts_pipeline::apply_audio_effects(audio_data, &settings)?;

    // 6. Отправка события во фронтенд (WebView)
    state.emit_event(AppEvent::TextSentToTts(text.clone()));

    // 7. Помещение в очередь плеера и запись в историю
    tts_pipeline::enqueue_and_record(state, text, audio_data, &settings)?;

    Ok(())
}
```

## Верификация
1. `npx vue-tsc --noEmit` — 0 ошибок.
2. `cargo check` — убедись, что все импорты на месте.
3. В отчёте: покажи финальный вид функции `speak_text_internal`.
