use crate::audio::{
    apply_effects, decode_audio, process_boundaries, AudioEffects, AudioPcm, OutputConfig,
};
use crate::config::AppSettings;
use crate::state::AppState;
use std::fs;
use tracing::{debug, error, info, warn};

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
        Ok(client) => match client.correct(text, &settings.ai.prompt).await {
            Ok(corrected) => {
                if corrected != text {
                    info!(
                        original = text.len(),
                        corrected = corrected.len(),
                        "AI correction applied"
                    );
                }
                corrected
            }
            Err(e) => {
                warn!("AI correction failed, using original text: {}", e);
                text.to_string()
            }
        },
        Err(e) => {
            warn!("AI client not available, skipping correction: {}", e);
            text.to_string()
        }
    }
}

/// 3. Этап синтеза аудиоданных через выбранный TTS-провайдер
pub async fn synthesize_audio(state: &AppState, text: &str) -> Result<Vec<u8>, String> {
    let provider = state.get_active_provider().ok_or_else(|| {
        error!("TTS provider not initialized");
        "TTS provider не инициализирован. Выберите провайдер в настройках.".to_string()
    })?;

    let audio_data = provider.synthesize(text).await.map_err(|e| {
        error!(error = %e, "synthesize() error");
        format!("Ошибка синтеза: {}", e)
    })?;

    debug!(bytes = audio_data.len(), "Audio synthesized");
    Ok(audio_data)
}

/// 4. Этап применения аудио-эффектов (pitch, speed, volume,
///    DeepFilterNet, DSP), boundary cleanup (DC offset + fade-in/out).
///
/// Pipeline order:
///   1. Decode audio to PCM
///   2. DeepFilterNet noise suppression (if enabled)
///   3. Signalsmith Stretch (tempo + pitch + formant correction)
///   4. DSP (EQ + compressor + limiter)
///   5. Per-phrase boundary cleanup (DC offset removal + start/end fade)
///
/// Returns `AudioPcm` ready for playback.
pub fn apply_audio_effects_pipeline(
    audio_data: Vec<u8>,
    settings: &AppSettings,
) -> Result<AudioPcm, String> {
    let pcm = if settings.audio_effects.enabled {
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
        let dsp_config = settings.dsp.to_dsp_config();
        match apply_effects(&audio_data, &effects, Some(&dsp_config)) {
            Ok(pcm) => {
                debug!(
                    original = original_len,
                    frames = pcm.frame_count(),
                    "Audio effects applied"
                );
                pcm
            }
            Err(e) => {
                error!(error = %e, "Failed to apply audio effects");
                return Err(format!("Не удалось применить аудио эффекты: {}", e));
            }
        }
    } else {
        decode_audio(&audio_data).map_err(|e| format!("Audio decode failed: {}", e))?
    };

    // Step 5: Per-phrase boundary cleanup (DC offset + fade-in/out).
    // Safe optional cleanup — on error, fall back to the original PCM.
    if settings.audio_effects.boundary_cleanup_enabled {
        let cleaned = process_boundaries(&pcm);
        if !cleaned.samples.is_empty()
            && cleaned.sample_rate == pcm.sample_rate
            && cleaned.channels == pcm.channels
            && cleaned.frame_count() == pcm.frame_count()
        {
            debug!(frames = cleaned.frame_count(), "Boundary cleanup applied");
            return Ok(cleaned);
        }
        warn!("Boundary cleanup produced invalid result, falling back to original PCM");
    }

    Ok(pcm)
}

/// 5. Отправка звука в плеер
pub fn enqueue_and_record(
    state: &AppState,
    text: String,
    audio: AudioPcm,
    settings: &AppSettings,
) -> Result<(), String> {
    let audio_settings = &settings.audio;
    let effects_volume = if settings.audio_effects.enabled {
        Some(
            AudioEffects::new(
                settings.audio_effects.pitch,
                settings.audio_effects.speed,
                settings.audio_effects.volume,
            )
            .volume_factor(),
        )
    } else {
        None
    };

    let speaker_config = if audio_settings.speaker_enabled {
        let base_volume = audio_settings.speaker_volume as f32 / 100.0;
        let final_volume = effects_volume
            .map(|ev| base_volume * ev)
            .unwrap_or(base_volume);
        Some(OutputConfig {
            device_id: audio_settings.speaker_device.clone(),
            volume: final_volume,
        })
    } else {
        None
    };

    let virtual_mic_config = audio_settings.virtual_mic_device.as_ref().map(|device_id| {
        let base_volume = audio_settings.virtual_mic_volume as f32 / 100.0;
        let final_volume = effects_volume
            .map(|ev| base_volume * ev)
            .unwrap_or(base_volume);
        OutputConfig {
            device_id: Some(device_id.clone()),
            volume: final_volume,
        }
    });

    if speaker_config.is_none() && virtual_mic_config.is_none() {
        return Err(
            "Аудиовывод и виртуальный микрофон выключены. Включите хотя бы один вывод.".to_string(),
        );
    }

    if let Some(pb) = state.playback_manager.lock().as_ref() {
        pb.update_audio_config(speaker_config, virtual_mic_config);
        let phrase_id = uuid::Uuid::new_v4().to_string();
        info!(target: "playback", "Enqueueing phrase to PlaybackManager");
        let enqueued = pb.enqueue(phrase_id, text.clone(), audio);
        if !enqueued {
            warn!("Playback queue full, phrase dropped: {}", text);
            return Err("Очередь воспроизведения переполнена. Попробуйте позже.".to_string());
        }
        Ok(())
    } else {
        Err("Плеер не инициализирован".to_string())
    }
}

/// Export raw TTS audio bytes to a file — synthesis only, no effects, no playback.
pub async fn synthesize_and_export(state: &AppState, text: &str, path: &str) -> Result<(), String> {
    let settings = state.settings_cache.read().clone();

    let prefix_result = crate::preprocessor::parse_prefix(text);
    let text = prefix_result.text;
    state.set_prefix_flags(prefix_result.skip_twitch, prefix_result.skip_webview);

    let text = preprocess_text(state, &text);
    let text = ai_correct_text(state, &text, &settings).await;
    let audio_data = synthesize_audio(state, &text).await?;

    fs::write(path, &audio_data).map_err(|e| format!("Failed to write audio file: {}", e))?;

    if let Some(hm) = state.editor.history_manager.lock().as_ref() {
        hm.record_phrase(&text);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_settings(boundary_cleanup: bool) -> AppSettings {
        let mut s = AppSettings::default();
        s.audio_effects.boundary_cleanup_enabled = boundary_cleanup;
        s
    }

    fn generate_silent_wav() -> Vec<u8> {
        let sample_rate = 48000u32;
        let channels = 1usize;
        let frames = 1000usize;
        let samples: Vec<f32> = (0..frames).map(|_| 0.0f32).collect();
        crate::audio::effects::encode_wav(&samples, sample_rate, channels).expect("encode test WAV")
    }

    /// Boundary cleanup enabled: pipeline must return valid, finite PCM.
    #[test]
    fn pipeline_with_boundary_cleanup_enabled() {
        let wav = generate_silent_wav();
        let settings = make_settings(true);
        let result = apply_audio_effects_pipeline(wav, &settings)
            .expect("pipeline with boundary cleanup enabled");
        assert!(result.samples.iter().all(|s| s.is_finite()));
        assert_eq!(result.sample_rate, 48000);
        assert_eq!(result.channels, 1);
    }

    /// Boundary cleanup disabled: pipeline must return valid, finite PCM.
    #[test]
    fn pipeline_with_boundary_cleanup_disabled() {
        let wav = generate_silent_wav();
        let settings = make_settings(false);
        let result = apply_audio_effects_pipeline(wav, &settings)
            .expect("pipeline with boundary cleanup disabled");
        assert!(result.samples.iter().all(|s| s.is_finite()));
        assert_eq!(result.sample_rate, 48000);
        assert_eq!(result.channels, 1);
    }

    /// Boundary cleanup disabled must preserve original samples (no fade-in/out).
    #[test]
    fn pipeline_boundary_disabled_preserves_samples() {
        let sample_rate = 48000u32;
        let channels = 1usize;
        let frames = 2000usize;
        let samples: Vec<f32> = (0..frames)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin() * 0.5
            })
            .collect();
        let wav = crate::audio::effects::encode_wav(&samples, sample_rate, channels)
            .expect("encode test WAV");

        let settings = make_settings(false);
        let result =
            apply_audio_effects_pipeline(wav, &settings).expect("pipeline with boundary disabled");
        for (a, b) in samples.iter().zip(result.samples.iter()) {
            assert!(
                (a - b).abs() < 0.01,
                "samples changed with boundary disabled"
            );
        }
    }

    /// Boundary cleanup enabled + effects disabled: DeepFilterNet and DSP
    /// must still be inactive (only boundary cleanup runs).
    #[test]
    fn pipeline_boundary_enabled_does_not_enable_enhance_or_dsp() {
        let wav = generate_silent_wav();
        let mut settings = make_settings(true);
        settings.audio_effects.enabled = false;
        settings.audio_effects.enhance_enabled = false;
        settings.dsp.eq.enabled = false;
        settings.dsp.compressor.enabled = false;
        settings.dsp.limiter.enabled = false;

        let result = apply_audio_effects_pipeline(wav, &settings)
            .expect("pipeline with boundary enabled, effects disabled");
        // Sample rate preserved (no DeepFilterNet resampling).
        assert_eq!(result.sample_rate, 48000);
        assert!(result.samples.iter().all(|s| s.is_finite()));
    }

    /// Sample rate, channels, and frame count must be preserved after boundary cleanup.
    #[test]
    fn pipeline_preserves_metadata_with_boundary() {
        let sample_rate = 44100u32;
        let channels = 2usize;
        let frames = 500usize;
        let samples: Vec<f32> = (0..frames * channels)
            .map(|i| (i as f32 * 0.001).sin() * 0.3)
            .collect();
        let wav = crate::audio::effects::encode_wav(&samples, sample_rate, channels)
            .expect("encode stereo WAV");

        let settings = make_settings(true);
        let result = apply_audio_effects_pipeline(wav, &settings).expect("pipeline with boundary");
        assert_eq!(result.sample_rate, sample_rate);
        assert_eq!(result.channels, channels);
        assert_eq!(result.frame_count(), frames);
    }
}
