use crate::audio::{
    apply_effects, decode_audio, process_boundaries, AudioEffects, AudioPcm, OutputConfig,
};
use crate::config::{
    AiSettings, AppSettings, AudioEffectsSettings, AudioSettings, DspSettings, NetworkSettings,
};
use crate::speech_queue::Snapshot;
use crate::state::AppState;
use crate::tts::TtsProvider;
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
            debug!(
                original_len = text.chars().count(),
                processed_len = processed.chars().count(),
                "Replacements applied"
            );
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

// ── Blocking stage isolation ──
//
// Filesystem cache read/decode, effects/DSP/boundary processing, and WAV cache
// encode/write are CPU/IO-bound and must not run on the async worker thread.
// Each stage runs inside `spawn_blocking` with owned inputs so the speech worker
// keeps observing shutdown and other control futures while the stage executes.

/// Map a `spawn_blocking` `JoinError` into the preparation error contract.
fn join_error_to_speech_error(join_err: tokio::task::JoinError) -> String {
    if join_err.is_panic() {
        format!("Speech preparation background task panicked: {join_err}")
    } else {
        format!("Speech preparation background task was cancelled: {join_err}")
    }
}

/// Run a fallible blocking stage off the async worker thread.
///
/// The inner error is already a `String` (the preparation error contract), and
/// a `JoinError` (panic or runtime shutdown) is mapped to the same contract.
async fn spawn_blocking_prepare<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(inner) => inner,
        Err(join_err) => Err(join_error_to_speech_error(join_err)),
    }
}

async fn read_cache_blocking(cache_key: String) -> Result<AudioPcm, String> {
    spawn_blocking_prepare(move || {
        crate::history::read_audio_cache(&cache_key).map_err(|e| e.to_string())
    })
    .await
}

async fn apply_effects_blocking(
    audio_data: Vec<u8>,
    audio_effects: AudioEffectsSettings,
    dsp: DspSettings,
) -> Result<AudioPcm, String> {
    spawn_blocking_prepare(move || {
        apply_audio_effects_pipeline_with_settings(audio_data, &audio_effects, &dsp)
    })
    .await
}

async fn save_cache_blocking(cache_key: String, audio: AudioPcm) -> bool {
    spawn_blocking_prepare(move || {
        crate::history::save_audio_cache(&cache_key, &audio).map_err(|e| e.to_string())
    })
    .await
    .is_ok()
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

    match read_cache_blocking(cache_key.clone()).await {
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
            if !e.contains("CacheMiss") {
                return Err(format!(
                    "Cache read error (corrupted/unreadable): {}",
                    e
                ));
            }
        }
    }

    let audio_data = synthesize_with_provider(&snapshot.tts_provider, &text).await?;
    let audio = apply_effects_blocking(
        audio_data,
        snapshot.audio_effects.clone(),
        snapshot.dsp.clone(),
    )
    .await?;

    let cache_saved = save_cache_blocking(cache_key.clone(), audio.clone()).await;

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

/// Build the per-phrase output configuration captured by the speech worker.
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

/// Export raw TTS audio bytes to a file — synthesis only, no effects, no playback.
pub async fn synthesize_and_export(state: &AppState, text: &str, path: &str) -> Result<(), String> {
    let settings = state.settings_cache.read().clone();

    let prefix_result = crate::preprocessor::parse_prefix(text);
    let text = prefix_result.text;

    let text = preprocess_text(state, &text);
    let text = ai_correct_text(state, &text, &settings).await;
    let audio_data = synthesize_audio(state, &text).await?;

    tokio::fs::write(path, &audio_data)
        .await
        .map_err(|e| format!("Failed to write audio file: {}", e))?;

    if let Some(hm) = state.editor.history_manager.lock().as_ref() {
        if let Err(error) = hm.record_phrase(&text) {
            tracing::error!(error = %error, "Failed to persist exported phrase history");
        }
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

    fn apply_pipeline(audio_data: Vec<u8>, settings: &AppSettings) -> Result<AudioPcm, String> {
        apply_audio_effects_pipeline_with_settings(
            audio_data,
            &settings.audio_effects,
            &settings.dsp,
        )
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
        let result =
            apply_pipeline(wav, &settings).expect("pipeline with boundary cleanup enabled");
        assert!(result.samples.iter().all(|s| s.is_finite()));
        assert_eq!(result.sample_rate, 48000);
        assert_eq!(result.channels, 1);
    }

    /// Boundary cleanup disabled: pipeline must return valid, finite PCM.
    #[test]
    fn pipeline_with_boundary_cleanup_disabled() {
        let wav = generate_silent_wav();
        let settings = make_settings(false);
        let result =
            apply_pipeline(wav, &settings).expect("pipeline with boundary cleanup disabled");
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
        let result = apply_pipeline(wav, &settings).expect("pipeline with boundary disabled");
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

        let result = apply_pipeline(wav, &settings)
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
        let result = apply_pipeline(wav, &settings).expect("pipeline with boundary");
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

    fn make_audio_settings(speaker_enabled: bool, mic_device: Option<&str>) -> AudioSettings {
        AudioSettings {
            speaker_enabled,
            speaker_device: None,
            speaker_volume: 80,
            virtual_mic_device: mic_device.map(str::to_string),
            virtual_mic_volume: 60,
        }
    }

    #[test]
    fn output_configs_both_enabled() {
        let audio = make_audio_settings(true, Some("mic1"));
        let effects = AudioEffectsSettings::default();
        let (speaker, mic) = compute_output_configs(&audio, &effects);

        assert_eq!(speaker.as_ref().map(|config| config.volume), Some(0.8));
        assert_eq!(
            mic.as_ref().and_then(|config| config.device_id.as_deref()),
            Some("mic1")
        );
        assert_eq!(mic.as_ref().map(|config| config.volume), Some(0.6));
    }

    #[test]
    fn output_configs_speaker_only() {
        let audio = make_audio_settings(true, None);
        let (speaker, mic) = compute_output_configs(&audio, &AudioEffectsSettings::default());

        assert!(speaker.is_some());
        assert!(mic.is_none());
    }

    #[test]
    fn output_configs_mic_only() {
        let audio = make_audio_settings(false, Some("mic1"));
        let (speaker, mic) = compute_output_configs(&audio, &AudioEffectsSettings::default());

        assert!(speaker.is_none());
        assert!(mic.is_some());
    }

    #[test]
    fn output_configs_both_disabled() {
        let audio = make_audio_settings(false, None);
        let (speaker, mic) = compute_output_configs(&audio, &AudioEffectsSettings::default());

        assert!(speaker.is_none());
        assert!(mic.is_none());
    }

    #[test]
    fn output_configs_apply_effects_volume_factor() {
        let audio = make_audio_settings(true, Some("mic1"));
        let effects = AudioEffectsSettings {
            enabled: true,
            volume: 50,
            ..Default::default()
        };
        let (speaker, _) = compute_output_configs(&audio, &effects);
        let expected = 0.8 * (50.0f32 / 100.0);

        assert!((speaker.unwrap().volume - expected).abs() < 0.0001);
    }

    // ── Snapshot identity / cache key separation tests ──

    #[test]
    fn cache_key_differs_for_openai_voices_same_text() {
        let fp = crate::history::compute_effects_fingerprint(
            &crate::config::AudioEffectsSettings::default(),
            &crate::config::DspSettings::default(),
        );
        let key_alloy = crate::history::build_cache_key("hello world", "openai", "alloy", fp);
        let key_echo = crate::history::build_cache_key("hello world", "openai", "echo", fp);
        assert_ne!(key_alloy, key_echo);
    }

    #[test]
    fn cache_key_differs_for_fish_reference_ids_same_text() {
        let fp = crate::history::compute_effects_fingerprint(
            &crate::config::AudioEffectsSettings::default(),
            &crate::config::DspSettings::default(),
        );
        let key_a = crate::history::build_cache_key("hello world", "fish", "ref-aaa", fp);
        let key_b = crate::history::build_cache_key("hello world", "fish", "ref-bbb", fp);
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn snapshot_identity_reflects_provider_voice_not_registry_id() {
        use crate::config::{
            AiSettings, AudioEffectsSettings, AudioSettings, DspSettings, NetworkSettings,
        };

        let mut alloy_tts = crate::tts::openai::OpenAiTts::new("sk-test".into());
        alloy_tts.set_voice("alloy".to_string());
        let snapshot_alloy = Snapshot {
            provider: "openai".into(),
            voice: alloy_tts.voice().to_string(),
            skip_twitch: false,
            skip_webview: false,
            ai_enabled: false,
            audio_effects: AudioEffectsSettings::default(),
            dsp: DspSettings::default(),
            audio: AudioSettings::default(),
            ai: AiSettings::default(),
            tts_provider: crate::tts::TtsProvider::OpenAi(alloy_tts),
            preprocessor: None,
            network_settings: NetworkSettings::default(),
        };

        let mut echo_tts = crate::tts::openai::OpenAiTts::new("sk-test".into());
        echo_tts.set_voice("echo".to_string());
        let snapshot_echo = Snapshot {
            provider: "openai".into(),
            voice: echo_tts.voice().to_string(),
            skip_twitch: false,
            skip_webview: false,
            ai_enabled: false,
            audio_effects: AudioEffectsSettings::default(),
            dsp: DspSettings::default(),
            audio: AudioSettings::default(),
            ai: AiSettings::default(),
            tts_provider: crate::tts::TtsProvider::OpenAi(echo_tts),
            preprocessor: None,
            network_settings: NetworkSettings::default(),
        };

        assert_ne!(snapshot_alloy.voice, snapshot_echo.voice);
        assert_ne!(snapshot_alloy.provider, "openai-entry-id");
        assert_ne!(snapshot_echo.provider, "openai-entry-id");
        assert_eq!(snapshot_alloy.voice, "alloy");
        assert_eq!(snapshot_echo.voice, "echo");
    }

    #[test]
    fn snapshot_identity_distinct_cache_for_same_text_different_openai() {
        let fp = crate::history::compute_effects_fingerprint(
            &crate::config::AudioEffectsSettings::default(),
            &crate::config::DspSettings::default(),
        );
        let text = "the quick brown fox";
        let key_alloy = crate::history::build_cache_key(text, "openai", "alloy", fp);
        let key_echo = crate::history::build_cache_key(text, "openai", "echo", fp);
        let key_fable = crate::history::build_cache_key(text, "openai", "fable", fp);
        assert_ne!(key_alloy, key_echo);
        assert_ne!(key_alloy, key_fable);
        assert_ne!(key_echo, key_fable);
    }

    // ── Blocking boundary seam tests ──

    /// A control future must keep running while a deliberately delayed blocking
    /// stage is executing (the blocking stage runs off the async worker thread).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn control_future_runs_during_delayed_blocking_stage() {
        use std::time::{Duration, Instant};

        let blocking = spawn_blocking_prepare(move || -> Result<i32, String> {
            std::thread::sleep(Duration::from_millis(200));
            Ok(42)
        });

        // Control future: a short sleep that must complete while the blocking
        // task is still running (200ms). If the blocking stage stalled the async
        // worker, this sleep would take ~200ms, not ~50ms.
        let start = Instant::now();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let control_elapsed = start.elapsed();
        assert!(
            control_elapsed < Duration::from_millis(150),
            "control future was stalled by the blocking stage (took {:?})",
            control_elapsed
        );

        let value = blocking.await.expect("blocking stage result");
        assert_eq!(value, 42);
    }

    /// A panicking blocking stage surfaces as a preparation error (JoinError).
    #[tokio::test]
    async fn blocking_stage_panic_maps_to_speech_error() {
        let result: Result<i32, String> = spawn_blocking_prepare(move || -> Result<i32, String> {
            panic!("intentional panic in blocking stage");
        })
        .await;

        let err = result.expect_err("panicking blocking stage must surface an error");
        assert!(err.contains("panicked"), "unexpected error: {err}");
    }

    /// Cancellation must win over a delayed blocking stage without waiting for it,
    /// mirroring the speech worker's shutdown observation during preparation.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancellation_wins_over_delayed_blocking_stage() {
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        let cancel = tokio_util::sync::CancellationToken::new();
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();

        let blocking = tokio::spawn(async move {
            spawn_blocking_prepare(move || -> Result<(), String> {
                started_tx.send(()).ok();
                // Simulate a long blocking stage until the test releases it.
                let _ = release_rx.recv();
                Ok(())
            })
            .await
        });

        tokio::task::spawn_blocking(move || started_rx.recv())
            .await
            .expect("start observer must not panic")
            .expect("blocking stage should start");

        let cancel_task = {
            let cancel = cancel.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                cancel.cancel();
            })
        };

        let start = Instant::now();
        let outcome = tokio::select! {
            result = blocking => Some(result),
            _ = cancel.cancelled() => None,
        };

        assert!(outcome.is_none(), "cancellation should win over the blocking stage");
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "cancellation should not wait for the blocking stage (took {:?})",
            start.elapsed()
        );

        // Release the detached blocking task so it can finish cleanly.
        release_tx.send(()).ok();
        cancel_task.await.expect("cancel task");
    }

    /// Inner cache-miss errors surface through the blocking seam, not a JoinError.
    #[tokio::test]
    async fn cache_read_blocking_surfaces_cache_miss() {
        let missing = uuid::Uuid::new_v4().to_string();
        let err = read_cache_blocking(missing)
            .await
            .expect_err("missing cache key must error");
        assert!(err.contains("CacheMiss"), "unexpected error: {err}");
    }
}
