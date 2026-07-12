use crate::state::AppState;
use crate::config::AppSettings;
use crate::audio::{OutputConfig, AudioEffects, apply_effects};
use tracing::{info, warn, error, debug};

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
pub fn apply_audio_effects_pipeline(audio_data: Vec<u8>, settings: &AppSettings) -> Result<Vec<u8>, String> {
    if !settings.audio_effects.enabled {
        return Ok(audio_data);
    }

    let effects = AudioEffects::new(
        settings.audio_effects.pitch,
        settings.audio_effects.speed,
        settings.audio_effects.volume,
    )
    .with_enhance(
        settings.audio_effects.enhance_enabled,
        settings.audio_effects.enhance_atten_db,
    )
    .with_formant_preserved(settings.audio_effects.formant_preserved);

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
        OutputConfig {
            device_id: Some(device_id.clone()),
            volume: final_volume,
        }
    });

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
