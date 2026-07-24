use crate::audio::{
    apply_effects, decode_audio, process_boundaries, AudioEffects, AudioPcm, OutputConfig,
};
use crate::config::{
    AiSettings, AppSettings, AudioEffectsSettings, AudioSettings, DspSettings, NetworkSettings,
};
use crate::speech_queue::Snapshot;
use crate::state::AppState;
use crate::tts::TtsProvider;
use std::fs;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct PreparedSpeech {
    pub processed_text: String,
    pub audio: AudioPcm,
    pub provider_name: String,
    pub voice_name: String,
    pub cache_key: String,
    pub cache_hit: bool,
    pub cache_saved: bool,
}

// ── Snapshot-friendly inner helpers ──

pub(crate) fn preprocess_text_with_preprocessor(
    text: &str,
    preprocessor: Option<&crate::preprocessor::TextPreprocessor>,
) -> String {
    let text = if let Some(p) = preprocessor {
        let processed = p.process(text);
        if processed != text {
            debug!(text, processed, "Replacements applied");
        }
        processed
    } else {
        text.to_string()
    };
    crate::preprocessor::process_numbers(&text)
}

pub(crate) async fn ai_correct_text_with_settings(
    text: &str,
    ai_enabled: bool,
    ai_settings: &AiSettings,
    network_settings: &NetworkSettings,
) -> Result<String, String> {
    if !ai_enabled {
        return Ok(text.to_string());
    }
    let client = crate::ai::create_ai_client(ai_settings, network_settings)
        .map_err(|e| format!("Failed to create AI client: {}", e))?;
    match client.correct(text, &ai_settings.prompt).await {
        Ok(corrected) => {
            if corrected != text {
                info!(
                    original = text.len(),
                    corrected = corrected.len(),
                    "AI correction applied"
                );
            }
            Ok(corrected)
        }
        Err(e) => {
            warn!("AI correction failed, using original text: {}", e);
            Ok(text.to_string())
        }
    }
}

pub(crate) async fn synthesize_with_provider(
    provider: &TtsProvider,
    text: &str,
) -> Result<Vec<u8>, String> {
    let audio_data = provider.synthesize(text).await.map_err(|e| {
        error!(error = %e, "synthesize() error");
        format!("Ошибка синтеза: {}", e)
    })?;
    debug!(bytes = audio_data.len(), "Audio synthesized");
    Ok(audio_data)
}

pub(crate) fn apply_audio_effects_pipeline_with_settings(
    audio_data: Vec<u8>,
    audio_effects: &AudioEffectsSettings,
    dsp: &DspSettings,
) -> Result<AudioPcm, String> {
    let pcm = if audio_effects.enabled {
        let effects = AudioEffects::new(
            audio_effects.pitch,
            audio_effects.speed,
            audio_effects.volume,
        )
        .with_enhance(
            audio_effects.enhance_enabled,
            audio_effects.enhance_atten_db,
        )
        .with_formant_preserved(audio_effects.formant_preserved);

        let original_len = audio_data.len();
        let dsp_config = dsp.to_dsp_config();
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

    if audio_effects.boundary_cleanup_enabled {
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

// ── Snapshot-driven preparation (pure, no side effects) ──

pub async fn prepare_speech(
    snapshot: &Snapshot,
    original_text: &str,
) -> Result<PreparedSpeech, String> {
    let prefix_result = crate::preprocessor::parse_prefix(original_text);
    if prefix_result.skip_twitch != snapshot.skip_twitch
        || prefix_result.skip_webview != snapshot.skip_webview
    {
        return Err(
            "Internal error: prefix flags mismatch between snapshot and parsed text".to_string(),
        );
    }
    let text = prefix_result.text;

    let text = preprocess_text_with_preprocessor(&text, snapshot.preprocessor.as_ref());

    let text = match ai_correct_text_with_settings(
        &text,
        snapshot.ai_enabled,
        &snapshot.ai,
        &snapshot.network_settings,
    )
    .await
    {
        Ok(corrected) => corrected,
        Err(e) => {
            warn!(
                "AI client construction failed, using uncorrected text: {}",
                e
            );
            text
        }
    };

    let effects_fp =
        crate::history::compute_effects_fingerprint(&snapshot.audio_effects, &snapshot.dsp);
    let cache_key =
        crate::history::build_cache_key(&text, &snapshot.provider, &snapshot.voice, effects_fp);

    match crate::history::read_audio_cache(&cache_key) {
        Ok(pcm) => {
            return Ok(PreparedSpeech {
                processed_text: text,
                audio: pcm,
                provider_name: snapshot.provider.clone(),
                voice_name: snapshot.voice.clone(),
                cache_key,
                cache_hit: true,
                cache_saved: false,
            });
        }
        Err(e) => {
            let err_str = e.to_string();
            if !err_str.contains("CacheMiss") {
                return Err(format!(
                    "Cache read error (corrupted/unreadable): {}",
                    err_str
                ));
            }
        }
    }

    let audio_data = synthesize_with_provider(&snapshot.tts_provider, &text).await?;
    let audio = apply_audio_effects_pipeline_with_settings(
        audio_data,
        &snapshot.audio_effects,
        &snapshot.dsp,
    )?;

    let cache_saved = crate::history::save_audio_cache(&cache_key, &audio).is_ok();

    Ok(PreparedSpeech {
        processed_text: text,
        audio,
        provider_name: snapshot.provider.clone(),
        voice_name: snapshot.voice.clone(),
        cache_key,
        cache_hit: false,
        cache_saved,
    })
}

// ── Public helpers (original API wrappers for backward compatibility) ──

/// 1. Этап предварительной подготовки текста (препроцессор + замена чисел)
pub fn preprocess_text(state: &AppState, text: &str) -> String {
    preprocess_text_with_preprocessor(text, state.editor.get_preprocessor().as_ref())
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

    synthesize_with_provider(&provider, text).await
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
    apply_audio_effects_pipeline_with_settings(audio_data, &settings.audio_effects, &settings.dsp)
}

// ── OutputConfig helper (pure, reused by worker and legacy enqueue_and_record) ──

pub(crate) fn compute_output_configs(
    audio_settings: &AudioSettings,
    effects_settings: &AudioEffectsSettings,
) -> (Option<OutputConfig>, Option<OutputConfig>) {
    let effects_volume = if effects_settings.enabled {
        Some(
            AudioEffects::new(
                effects_settings.pitch,
                effects_settings.speed,
                effects_settings.volume,
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

    (speaker_config, virtual_mic_config)
}

/// 5. Отправка звука в плеер (legacy path, uses global audio_config snapshots)
pub fn enqueue_and_record(
    state: &AppState,
    text: String,
    audio: AudioPcm,
    settings: &AppSettings,
) -> Result<(), String> {
    let (speaker_config, virtual_mic_config) =
        compute_output_configs(&settings.audio, &settings.audio_effects);

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

    // ── Focused snapshot/preparation tests ──

    fn make_snapshot(skip_twitch: bool, skip_webview: bool, ai_enabled: bool) -> Snapshot {
        use crate::config::{
            AiSettings, AudioEffectsSettings, AudioSettings, DspSettings, NetworkSettings,
        };
        Snapshot {
            provider: "test-provider".into(),
            voice: "test-voice".into(),
            skip_twitch,
            skip_webview,
            ai_enabled,
            audio_effects: AudioEffectsSettings::default(),
            dsp: DspSettings::default(),
            audio: AudioSettings::default(),
            ai: AiSettings::default(),
            tts_provider: crate::tts::TtsProvider::Local(
                crate::tts::local_http_server::LocalHttpServerTts::new(),
            ),
            preprocessor: None,
            network_settings: NetworkSettings::default(),
        }
    }

    /// Prefix flag mismatch between snapshot and text fails before synthesis.
    #[tokio::test]
    async fn prefix_flag_mismatch_fails_before_synthesis() {
        let snapshot = make_snapshot(true, false, false);
        let err = prepare_speech(&snapshot, "hello world").await.unwrap_err();
        assert!(
            err.contains("prefix flags mismatch"),
            "expected prefix mismatch error, got: {err}"
        );
    }

    /// ai_correct_text_with_settings with ai_enabled=false returns unchanged text.
    #[tokio::test]
    async fn ai_disabled_helper_returns_unchanged_text() {
        let result = ai_correct_text_with_settings(
            "hello world",
            false,
            &crate::config::AiSettings::default(),
            &crate::config::NetworkSettings::default(),
        )
        .await
        .expect("AI-disabled helper should never fail");
        assert_eq!(result, "hello world");
    }

    // ── compute_output_configs tests ──

    fn make_audio_settings(speaker_enabled: bool, mic_device: Option<&str>) -> AudioSettings {
        use crate::config::AudioSettings;
        AudioSettings {
            speaker_enabled,
            speaker_device: None,
            speaker_volume: 80,
            virtual_mic_device: mic_device.map(|s| s.to_string()),
            virtual_mic_volume: 60,
            ..Default::default()
        }
    }

    #[test]
    fn output_configs_both_enabled() {
        let audio = make_audio_settings(true, Some("mic1"));
        let effects = AudioEffectsSettings::default();
        let (spk, mic) = compute_output_configs(&audio, &effects);
        assert!(spk.is_some());
        assert_eq!(spk.as_ref().unwrap().volume, 0.8);
        assert!(mic.is_some());
        assert_eq!(mic.as_ref().unwrap().device_id.as_deref(), Some("mic1"));
        assert_eq!(mic.as_ref().unwrap().volume, 0.6);
    }

    #[test]
    fn output_configs_speaker_only() {
        let audio = make_audio_settings(true, None);
        let effects = AudioEffectsSettings::default();
        let (spk, mic) = compute_output_configs(&audio, &effects);
        assert!(spk.is_some());
        assert!(mic.is_none());
    }

    #[test]
    fn output_configs_mic_only() {
        let audio = make_audio_settings(false, Some("mic1"));
        let effects = AudioEffectsSettings::default();
        let (spk, mic) = compute_output_configs(&audio, &effects);
        assert!(spk.is_none());
        assert!(mic.is_some());
    }

    #[test]
    fn output_configs_both_disabled() {
        let audio = make_audio_settings(false, None);
        let effects = AudioEffectsSettings::default();
        let (spk, mic) = compute_output_configs(&audio, &effects);
        assert!(spk.is_none());
        assert!(mic.is_none());
    }

    #[test]
    fn output_configs_effects_volume_factor_applied() {
        let audio = make_audio_settings(true, Some("mic1"));
        let mut effects = AudioEffectsSettings::default();
        effects.enabled = true;
        effects.volume = 50;
        let (spk, _mic) = compute_output_configs(&audio, &effects);
        let expected = 0.8 * (50.0f32 / 100.0);
        assert!((spk.unwrap().volume - expected).abs() < 0.0001);
    }

    #[test]
    fn output_configs_parity_with_legacy() {
        let audio = make_audio_settings(true, Some("dev-test"));
        let effects = AudioEffectsSettings::default();
        let (spk, mic) = compute_output_configs(&audio, &effects);

        let legacy_effects_volume: Option<f32> = if effects.enabled {
            Some(AudioEffects::new(effects.pitch, effects.speed, effects.volume).volume_factor())
        } else {
            None
        };
        let legacy_spk_vol = 0.8 * legacy_effects_volume.unwrap_or(1.0);
        assert_eq!(spk.unwrap().volume, legacy_spk_vol);

        let legacy_mic_vol = 0.6 * legacy_effects_volume.unwrap_or(1.0);
        assert_eq!(mic.unwrap().volume, legacy_mic_vol);
    }
}
