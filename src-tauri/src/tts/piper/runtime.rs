use std::collections::BTreeMap;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use ort::session::Session;
use ort::value::Tensor;
use serde::Deserialize;

use crate::audio::effects::encode_wav;
use crate::tts::engine::TtsEngine;
use crate::tts::piper::scanner::PiperModelDescriptor;

const BOS: char = '^';
const EOS: char = '$';
const PAD: char = '_';

static ESPEAKNG_DATA_INIT: OnceLock<()> = OnceLock::new();

#[derive(Debug, thiserror::Error)]
pub enum PiperRuntimeError {
    #[error("Failed to load model: {0}")]
    Load(String),
    #[error("Inference error: {0}")]
    Inference(String),
    #[error("Model not loaded")]
    NotLoaded,
    #[error("Piper is not ready: eSpeak NG phonemization unavailable ({0:?})")]
    NotReady(PiperReadiness),
    #[error("Piper model voice is unavailable: {0}")]
    VoiceUnavailable(String),
}

/// Readiness of eSpeak NG phonemization for Piper, fixed once per app run.
///
/// The espeak-rs dependency initializes eSpeak exactly once into its own
/// internal `OnceLock`, so a failed initialization cannot be retried without
/// restarting the application. The verdict is therefore cached process-wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiperReadiness {
    /// The real phonemization probe succeeded.
    Ready,
    /// A local `espeak-ng-data` folder is visible to the library lookup, but
    /// the real phonemization probe failed: the folder is empty, incomplete
    /// or damaged.
    DataDamaged,
    /// No local `espeak-ng-data` folder is visible and the system fallback
    /// (registry / `ESPEAK_DATA_PATH`) failed to initialize as well.
    DataNotFound,
}

impl PiperReadiness {
    pub fn is_ready(self) -> bool {
        matches!(self, PiperReadiness::Ready)
    }

    /// Locale key naming the specific known cause; `None` when ready.
    pub fn message_key(self) -> Option<&'static str> {
        match self {
            PiperReadiness::Ready => None,
            PiperReadiness::DataDamaged => Some("tts.piper.espeak_data_damaged"),
            PiperReadiness::DataNotFound => Some("tts.piper.espeak_data_not_found"),
        }
    }

    /// Model-independent process readiness; never use a model voice here.
    pub fn assess_cached() -> PiperReadiness {
        *ESPEAK_READINESS.get_or_init(Self::assess)
    }

    fn assess() -> PiperReadiness {
        let local_data_found = local_espeak_data_visible();
        let probe = espeak_rs::text_to_phonemes(ESPEAK_PROBE_TEXT, ESPEAK_PROBE_VOICE, None);
        classify_readiness(
            local_data_found,
            probe.and_then(|phonemes| {
                if phonemes.iter().all(|text| text.trim().is_empty()) {
                    Err(espeak_rs::ESpeakError(
                        "Empty phonemization probe result".into(),
                    ))
                } else {
                    Ok(())
                }
            }),
        )
    }
}

static ESPEAK_READINESS: OnceLock<PiperReadiness> = OnceLock::new();

fn validate_model_voice(
    readiness: PiperReadiness,
    voice: &str,
    probe: impl FnOnce(&str) -> espeak_rs::ESpeakResult<Vec<String>>,
) -> Result<(), PiperRuntimeError> {
    if !readiness.is_ready() {
        return Err(PiperRuntimeError::NotReady(readiness));
    }
    let phonemes = probe(voice).map_err(|error| {
        tracing::warn!(voice, error = %error, "Piper model voice check failed");
        PiperRuntimeError::VoiceUnavailable(voice.to_string())
    })?;
    if phonemes.iter().all(|text| text.trim().is_empty()) {
        return Err(PiperRuntimeError::VoiceUnavailable(voice.to_string()));
    }
    Ok(())
}

pub fn phonemization_error_key(error: &PiperRuntimeError) -> Option<&'static str> {
    match error {
        PiperRuntimeError::NotReady(_) => Some("tts.piper.espeak_not_ready"),
        PiperRuntimeError::VoiceUnavailable(_) => Some("tts.piper.voice_unavailable"),
        _ => None,
    }
}

const ESPEAK_PROBE_TEXT: &str = "eSpeak readiness probe";
const ESPEAK_PROBE_VOICE: &str = "en";

/// Generic locale key used when the specific cause key is missing from the
/// loaded locale pack.
pub const PIPER_NOT_READY_FALLBACK_KEY: &str = "tts.piper.espeak_not_ready";

/// Hardcoded last-resort message, matching the generic locale key.
pub const PIPER_NOT_READY_FALLBACK_TEXT: &str = "Piper не готов: данные eSpeak NG не найдены или повреждены. Восстановите папку espeak-ng-data или установку eSpeak NG и перезапустите приложение";

/// Build the localized user-facing message for a not-ready verdict. Falls
/// back from the specific cause key to the generic key and then to the
/// hardcoded default, mirroring window-title localization in setup.
pub fn not_ready_message(messages: &BTreeMap<String, String>, readiness: PiperReadiness) -> String {
    if readiness.is_ready() {
        return String::new();
    }
    messages
        .get(
            readiness
                .message_key()
                .unwrap_or(PIPER_NOT_READY_FALLBACK_KEY),
        )
        .or_else(|| messages.get(PIPER_NOT_READY_FALLBACK_KEY))
        .cloned()
        .unwrap_or_else(|| PIPER_NOT_READY_FALLBACK_TEXT.to_string())
}

/// Pure classification of the phonemization probe outcome. `local_data_found`
/// mirrors the library's own folder lookup and only refines the failure
/// reason; readiness itself is always decided by the real probe, so a working
/// system installation (registry / `ESPEAK_DATA_PATH`) stays valid.
fn classify_readiness(
    local_data_found: bool,
    probe: espeak_rs::ESpeakResult<()>,
) -> PiperReadiness {
    match probe {
        Ok(_) => PiperReadiness::Ready,
        Err(error) => {
            tracing::warn!(
                error = %error,
                local_data_found,
                "eSpeak NG phonemization probe failed; Piper runtime is not ready"
            );
            if local_data_found {
                PiperReadiness::DataDamaged
            } else {
                PiperReadiness::DataNotFound
            }
        }
    }
}

/// Mirror of the espeak-rs `locate_espeak_data` lookup: a `PIPER_ESPEAKNG_DATA_DIRECTORY`
/// value pointing at the directory that *contains* `espeak-ng-data`, then the
/// working directory, then the executable directory. Used only to refine the
/// failure reason — the probe itself resolves the path exactly like synthesis.
fn local_espeak_data_visible() -> bool {
    const ENV_KEY: &str = "PIPER_ESPEAKNG_DATA_DIRECTORY";
    const DATA_DIR_NAME: &str = "espeak-ng-data";

    if let Ok(dir) = env::var(ENV_KEY) {
        if PathBuf::from(&dir).join(DATA_DIR_NAME).is_dir() {
            return true;
        }
    }
    if let Ok(cwd) = env::current_dir() {
        if cwd.join(DATA_DIR_NAME).is_dir() {
            return true;
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            if exe_dir.join(DATA_DIR_NAME).is_dir() {
                return true;
            }
        }
    }
    false
}

/// Rewrite a Windows verbatim path (`\\?\C:\...` or `\\?\UNC\server\share\...`)
/// into the ordinary drive / UNC form the C runtime's `stat` accepts.
///
/// The Tauri resource resolver and `std::fs::canonicalize` return verbatim
/// paths on Windows, but the C `stat` used by eSpeak NG rejects the `\\?\`
/// prefix even when the underlying folder exists. Only the verbatim prefix is
/// rewritten; ordinary paths and every non-Windows path pass through unchanged
/// (no filesystem canonicalization is performed).
#[cfg(windows)]
fn normalize_for_clib(path: &Path) -> PathBuf {
    use std::path::{Component, Prefix};

    let mut components = path.components();
    let prefix = match components.next() {
        Some(Component::Prefix(prefix)) => prefix,
        _ => return path.to_path_buf(),
    };

    let mut result = match prefix.kind() {
        Prefix::VerbatimDisk(drive) => format!("{}:", drive as char),
        Prefix::VerbatimUNC(server, share) => {
            format!(
                "\\\\{}\\{}",
                server.to_string_lossy(),
                share.to_string_lossy()
            )
        }
        _ => return path.to_path_buf(),
    };

    let mut pending_separator = false;
    for component in components {
        match component {
            Component::RootDir => {
                result.push('\\');
                pending_separator = false;
            }
            Component::CurDir | Component::Prefix(_) => {}
            Component::ParentDir => {
                if pending_separator {
                    result.push('\\');
                }
                result.push_str("..");
                pending_separator = true;
            }
            Component::Normal(part) => {
                if pending_separator {
                    result.push('\\');
                }
                result.push_str(&part.to_string_lossy());
                pending_separator = true;
            }
        }
    }

    PathBuf::from(result)
}

#[cfg(not(windows))]
fn normalize_for_clib(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[derive(Deserialize, Clone)]
struct AudioConfig {
    sample_rate: u32,
}

#[derive(Deserialize, Clone)]
struct ESpeakConfig {
    voice: String,
}

#[derive(Deserialize, Clone)]
struct InferenceConfig {
    noise_scale: f32,
    length_scale: f32,
    noise_w: f32,
}

#[derive(Deserialize, Clone)]
struct ModelConfig {
    audio: AudioConfig,
    espeak: ESpeakConfig,
    inference: InferenceConfig,
    num_speakers: u32,
    #[serde(default)]
    #[allow(dead_code)]
    speaker_id_map: HashMap<String, i64>,
    phoneme_id_map: HashMap<String, Vec<i64>>,
}

struct ModelState {
    session: Session,
    config: ModelConfig,
}

#[derive(Clone)]
pub struct LocalModelTts {
    model_path: std::path::PathBuf,
    config_path: std::path::PathBuf,
    model: Arc<Mutex<Option<ModelState>>>,
    provider_id: String,
    display_name: String,
}

impl std::fmt::Debug for LocalModelTts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalModelTts")
            .field("model_path", &self.model_path)
            .field("config_path", &self.config_path)
            .field("model", &"<lazy>")
            .field("provider_id", &self.provider_id)
            .field("display_name", &self.display_name)
            .finish()
    }
}

impl LocalModelTts {
    pub fn init_espeak_data(resource_dir: Option<PathBuf>) {
        ESPEAKNG_DATA_INIT.get_or_init(|| {
            let candidate = resource_dir.and_then(|dir| {
                let p = dir.join("espeak-ng-data");
                if p.join("voices").exists() && p.join("en_dict").exists() {
                    Some(p)
                } else {
                    None
                }
            });

            if let Some(data_dir) = candidate {
                // espeak-rs expects the variable to point at the directory
                // CONTAINING espeak-ng-data (parent form), not the data dir
                // itself; it re-validates `<value>/espeak-ng-data` before use.
                let parent = data_dir
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| data_dir.clone());
                let parent = normalize_for_clib(&parent);
                env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &parent);
                tracing::info!(
                    dir = %data_dir.display(),
                    "espeak-ng data directory set"
                );
                return;
            }

            if let Ok(cwd) = env::current_dir() {
                let p = cwd.join("espeak-ng-data");
                if p.join("voices").exists() && p.join("en_dict").exists() {
                    let cwd = normalize_for_clib(&cwd);
                    env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &cwd);
                    tracing::info!(
                        dir = %p.display(),
                        "espeak-ng data directory set (from cwd)"
                    );
                    return;
                }
            }

            if let Ok(exe) = env::current_exe() {
                if let Some(exe_dir) = exe.parent() {
                    let p = exe_dir.join("espeak-ng-data");
                    if p.join("voices").exists() && p.join("en_dict").exists() {
                        let exe_dir = normalize_for_clib(exe_dir);
                        env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", exe_dir);
                        tracing::info!(
                            dir = %p.display(),
                            "espeak-ng data directory set (next to exe)"
                        );
                        return;
                    }
                }
            }

            tracing::warn!("espeak-ng data directory not found; Piper phonemization may fail");
        });
    }

    pub fn prepare(&self) -> Result<(), String> {
        self.ensure_loaded().map_err(|e| e.to_string())
    }

    /// Cached shared data readiness, independent of the model voice.
    pub fn espeak_readiness(&self) -> PiperReadiness {
        PiperReadiness::assess_cached()
    }

    /// Validate shared readiness and the model voice before any ONNX load.
    pub fn check_phonemization(&self) -> Result<(), PiperRuntimeError> {
        let readiness = self.espeak_readiness();
        if !readiness.is_ready() {
            return Err(PiperRuntimeError::NotReady(readiness));
        }
        let content = std::fs::read_to_string(&self.config_path)
            .map_err(|error| PiperRuntimeError::Load(error.to_string()))?;
        let config: ModelConfig = serde_json::from_str(&content)
            .map_err(|error| PiperRuntimeError::Load(error.to_string()))?;
        validate_model_voice(readiness, &config.espeak.voice, |voice| {
            espeak_rs::text_to_phonemes(ESPEAK_PROBE_TEXT, voice, None)
        })
    }

    pub fn is_loaded(&self) -> bool {
        self.model.lock().unwrap().is_some()
    }

    #[cfg(test)]
    pub fn new(model_path: impl AsRef<Path>, config_path: impl AsRef<Path>) -> Self {
        Self {
            model_path: model_path.as_ref().to_path_buf(),
            config_path: config_path.as_ref().to_path_buf(),
            model: Arc::new(Mutex::new(None)),
            provider_id: String::new(),
            display_name: String::new(),
        }
    }

    pub fn from_descriptor(descriptor: &PiperModelDescriptor) -> Self {
        Self {
            model_path: descriptor.onnx_path.clone(),
            config_path: descriptor.json_path.clone(),
            model: Arc::new(Mutex::new(None)),
            provider_id: descriptor.id.clone(),
            display_name: descriptor.display_name.clone(),
        }
    }

    fn ensure_loaded(&self) -> Result<(), PiperRuntimeError> {
        let mut guard = self.model.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }

        Self::init_espeak_data(None);

        let config_content = std::fs::read_to_string(&self.config_path).map_err(|e| {
            PiperRuntimeError::Load(format!(
                "Failed to read config {}: {}",
                self.config_path.display(),
                e
            ))
        })?;

        let config: ModelConfig = serde_json::from_str(&config_content).map_err(|e| {
            PiperRuntimeError::Load(format!(
                "Failed to parse config {}: {}",
                self.config_path.display(),
                e
            ))
        })?;

        // ROADMAP-119: never create the ONNX session when phonemization
        // cannot work; the probe verdict is cached for the whole app run.
        validate_model_voice(
            PiperReadiness::assess_cached(),
            &config.espeak.voice,
            |voice| espeak_rs::text_to_phonemes(ESPEAK_PROBE_TEXT, voice, None),
        )?;

        let session = Session::builder()
            .map_err(|e| {
                PiperRuntimeError::Load(format!("Failed to create session builder: {}", e))
            })?
            .commit_from_file(&self.model_path)
            .map_err(|e| {
                PiperRuntimeError::Load(format!(
                    "Failed to load model {}: {}",
                    self.model_path.display(),
                    e
                ))
            })?;

        *guard = Some(ModelState { session, config });
        Ok(())
    }

    fn phonemes_to_ids(config: &ModelConfig, phonemes: &str) -> Vec<i64> {
        let map = &config.phoneme_id_map;
        let pad_ids = map
            .get(&PAD.to_string())
            .map(|v| v.as_slice())
            .unwrap_or(&[0]);
        let bos_ids = map
            .get(&BOS.to_string())
            .map(|v| v.as_slice())
            .unwrap_or(&[0]);
        let eos_ids = map
            .get(&EOS.to_string())
            .map(|v| v.as_slice())
            .unwrap_or(&[0]);

        let mut ids = Vec::new();
        ids.extend_from_slice(bos_ids);
        ids.extend_from_slice(pad_ids);

        let char_indices: Vec<(usize, char)> = phonemes.char_indices().collect();
        let mut pos = 0;
        while pos < char_indices.len() {
            let byte_pos = char_indices[pos].0;
            let remaining = &phonemes[byte_pos..];

            let mut best_key: Option<&str> = None;
            let mut best_len = 0;
            for key in map.keys() {
                if remaining.starts_with(key.as_str()) && key.len() > best_len {
                    best_key = Some(key.as_str());
                    best_len = key.len();
                }
            }

            if let Some(key) = best_key {
                if let Some(token_ids) = map.get(key) {
                    ids.extend_from_slice(token_ids);
                    ids.extend_from_slice(pad_ids);
                }
                pos += key.chars().count();
            } else {
                pos += 1;
            }
        }

        ids.extend_from_slice(eos_ids);
        ids
    }

    fn run_inference(
        session: &mut Session,
        config: &ModelConfig,
        phonemes: &str,
        noise_scale: f32,
        length_scale: f32,
        noise_w: f32,
        speaker_id: Option<i64>,
    ) -> Result<Vec<f32>, PiperRuntimeError> {
        let ids = Self::phonemes_to_ids(config, phonemes);
        let input_len = ids.len();

        let input_t =
            Tensor::<i64>::from_array(([1, input_len], ids.into_boxed_slice())).map_err(|e| {
                PiperRuntimeError::Inference(format!("Failed to create input tensor: {}", e))
            })?;

        let lengths_t = Tensor::<i64>::from_array(([1], vec![input_len as i64].into_boxed_slice()))
            .map_err(|e| {
                PiperRuntimeError::Inference(format!("Failed to create lengths tensor: {}", e))
            })?;

        let scales_t = Tensor::<f32>::from_array((
            [3],
            vec![noise_scale, length_scale, noise_w].into_boxed_slice(),
        ))
        .map_err(|e| {
            PiperRuntimeError::Inference(format!("Failed to create scales tensor: {}", e))
        })?;

        let outputs = if config.num_speakers > 1 {
            let sid = speaker_id.unwrap_or(0);
            let sid_t =
                Tensor::<i64>::from_array(([1], vec![sid].into_boxed_slice())).map_err(|e| {
                    PiperRuntimeError::Inference(format!("Failed to create sid tensor: {}", e))
                })?;
            session
                .run(ort::inputs![input_t, lengths_t, scales_t, sid_t])
                .map_err(|e| PiperRuntimeError::Inference(format!("Inference failed: {}", e)))?
        } else {
            session
                .run(ort::inputs![input_t, lengths_t, scales_t])
                .map_err(|e| PiperRuntimeError::Inference(format!("Inference failed: {}", e)))?
        };

        let (_, audio) = outputs[0].try_extract_tensor::<f32>().map_err(|e| {
            PiperRuntimeError::Inference(format!("Failed to extract output: {}", e))
        })?;

        Ok(audio.to_vec())
    }

    fn synthesize_blocking(&self, text: &str) -> Result<Vec<u8>, String> {
        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (voice, noise_scale, length_scale, noise_w, sample_rate) = {
            let guard = self.model.lock().unwrap();
            let state = guard
                .as_ref()
                .ok_or_else(|| PiperRuntimeError::NotLoaded.to_string())?;
            (
                state.config.espeak.voice.clone(),
                state.config.inference.noise_scale,
                state.config.inference.length_scale,
                state.config.inference.noise_w,
                state.config.audio.sample_rate,
            )
        };

        let phonemes = espeak_rs::text_to_phonemes(text, &voice, None)
            .map_err(|e| format!("Phonemization failed: {}", e))?
            .join(" ");

        let mut samples = {
            let mut guard = self.model.lock().unwrap();
            let state = guard
                .as_mut()
                .ok_or_else(|| PiperRuntimeError::NotLoaded.to_string())?;
            Self::run_inference(
                &mut state.session,
                &state.config,
                &phonemes,
                noise_scale,
                length_scale,
                noise_w,
                None,
            )
            .map_err(|e| e.to_string())?
        };

        const POST_ROLL_MS: u32 = 250;
        let post_roll_frames = (sample_rate as usize * POST_ROLL_MS as usize) / 1000;
        samples.extend(std::iter::repeat_n(0.0f32, post_roll_frames));

        encode_wav(&samples, sample_rate, 1).map_err(|e| format!("Failed to encode WAV: {}", e))
    }
}

async fn run_on_blocking_pool<F>(job: F) -> Result<Vec<u8>, String>
where
    F: FnOnce() -> Result<Vec<u8>, String> + Send + 'static,
{
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|e| format!("Piper worker failed: {}", e))?
}

#[async_trait]
impl TtsEngine for LocalModelTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        let worker = self.clone();
        let text = text.to_string();
        run_on_blocking_pool(move || worker.synthesize_blocking(&text)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "set TTSBARD_PIPER_TEST_MODEL and TTSBARD_PIPER_TEST_CONFIG to run real inference"]
    async fn test_local_model_tts_synthesize_with_fixture() {
        let model_path = env::var("TTSBARD_PIPER_TEST_MODEL")
            .expect("TTSBARD_PIPER_TEST_MODEL must point to a Piper ONNX fixture");
        let config_path = env::var("TTSBARD_PIPER_TEST_CONFIG")
            .expect("TTSBARD_PIPER_TEST_CONFIG must point to its JSON config");
        assert!(
            PathBuf::from(&model_path).is_file(),
            "model fixture is missing"
        );
        assert!(
            PathBuf::from(&config_path).is_file(),
            "config fixture is missing"
        );

        let tts = LocalModelTts::new(model_path, config_path);
        assert!(!tts.is_loaded());

        let audio = tts
            .synthesize("Привет мир")
            .await
            .expect("synthesize should succeed");

        assert!(tts.is_loaded());
        assert!(!audio.is_empty(), "synthesized audio should not be empty");

        let pcm = crate::audio::effects::decode_audio(&audio).expect("output should be valid WAV");

        assert!(pcm.sample_rate > 0);
        assert!(pcm.channels == 1);
        assert!(pcm.frame_count() > 0);

        eprintln!(
            "Synthesized: {} frames, {} Hz, {:.2}s",
            pcm.frame_count(),
            pcm.sample_rate,
            pcm.duration_secs()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn blocking_worker_keeps_async_executor_responsive() {
        let mut worker = tokio::spawn(run_on_blocking_pool(|| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            Ok(vec![1])
        }));
        let timer = tokio::time::sleep(std::time::Duration::from_millis(10));
        tokio::pin!(timer);

        tokio::select! {
            _ = &mut timer => assert!(!worker.is_finished()),
            result = &mut worker => panic!("blocking job completed before timer: {result:?}"),
        }

        assert_eq!(worker.await.unwrap().unwrap(), vec![1]);
    }

    #[test]
    fn new_model_is_discovered_but_not_loaded() {
        let tts = LocalModelTts::new("/dummy/model.onnx", "/dummy/model.onnx.json");
        assert!(!tts.is_loaded());
    }

    #[test]
    fn test_phonemes_to_ids() {
        let config = ModelConfig {
            audio: AudioConfig { sample_rate: 22050 },
            espeak: ESpeakConfig {
                voice: "ru".to_string(),
            },
            inference: InferenceConfig {
                noise_scale: 0.667,
                length_scale: 1.0,
                noise_w: 0.8,
            },
            num_speakers: 1,
            speaker_id_map: HashMap::new(),
            phoneme_id_map: {
                let mut map = HashMap::new();
                map.insert("_".to_string(), vec![0]);
                map.insert("^".to_string(), vec![1]);
                map.insert("$".to_string(), vec![2]);
                map.insert("a".to_string(), vec![4]);
                map.insert("b".to_string(), vec![5]);
                map.insert("c".to_string(), vec![6]);
                map
            },
        };

        let ids = LocalModelTts::phonemes_to_ids(&config, "a b c");
        assert_eq!(ids, vec![1, 0, 4, 0, 5, 0, 6, 0, 2]);
    }

    #[test]
    fn test_phonemes_to_ids_unknown_skipped() {
        let config = ModelConfig {
            audio: AudioConfig { sample_rate: 22050 },
            espeak: ESpeakConfig {
                voice: "ru".to_string(),
            },
            inference: InferenceConfig {
                noise_scale: 0.667,
                length_scale: 1.0,
                noise_w: 0.8,
            },
            num_speakers: 1,
            speaker_id_map: HashMap::new(),
            phoneme_id_map: {
                let mut map = HashMap::new();
                map.insert("_".to_string(), vec![0]);
                map.insert("^".to_string(), vec![1]);
                map.insert("$".to_string(), vec![2]);
                map.insert("a".to_string(), vec![4]);
                map
            },
        };

        let ids = LocalModelTts::phonemes_to_ids(&config, "a x z");
        assert_eq!(ids, vec![1, 0, 4, 0, 2]);
    }

    #[test]
    fn test_phonemes_to_ids_single_char_ipa() {
        let config = ModelConfig {
            audio: AudioConfig { sample_rate: 22050 },
            espeak: ESpeakConfig {
                voice: "en".to_string(),
            },
            inference: InferenceConfig {
                noise_scale: 0.667,
                length_scale: 1.0,
                noise_w: 0.8,
            },
            num_speakers: 1,
            speaker_id_map: HashMap::new(),
            phoneme_id_map: {
                let mut map = HashMap::new();
                map.insert("_".to_string(), vec![0]);
                map.insert("^".to_string(), vec![1]);
                map.insert("$".to_string(), vec![2]);
                map.insert("b".to_string(), vec![5]);
                map
            },
        };

        let ids = LocalModelTts::phonemes_to_ids(&config, "b");
        assert_eq!(ids, vec![1, 0, 5, 0, 2]);
    }

    #[test]
    fn test_phonemes_to_ids_two_char_ipa_greedy_match() {
        let config = ModelConfig {
            audio: AudioConfig { sample_rate: 22050 },
            espeak: ESpeakConfig {
                voice: "en".to_string(),
            },
            inference: InferenceConfig {
                noise_scale: 0.667,
                length_scale: 1.0,
                noise_w: 0.8,
            },
            num_speakers: 1,
            speaker_id_map: HashMap::new(),
            phoneme_id_map: {
                let mut map = HashMap::new();
                map.insert("_".to_string(), vec![0]);
                map.insert("^".to_string(), vec![1]);
                map.insert("$".to_string(), vec![2]);
                map.insert("tʃ".to_string(), vec![7]);
                map.insert("t".to_string(), vec![8]);
                map.insert("ʃ".to_string(), vec![9]);
                map
            },
        };

        let ids = LocalModelTts::phonemes_to_ids(&config, "tʃ");
        assert_eq!(
            ids,
            vec![1, 0, 7, 0, 2],
            "two-char IPA key should match as one token, not t + ʃ"
        );
    }

    #[test]
    fn test_phonemes_to_ids_whitespace_between_phonemes() {
        let config = ModelConfig {
            audio: AudioConfig { sample_rate: 22050 },
            espeak: ESpeakConfig {
                voice: "en".to_string(),
            },
            inference: InferenceConfig {
                noise_scale: 0.667,
                length_scale: 1.0,
                noise_w: 0.8,
            },
            num_speakers: 1,
            speaker_id_map: HashMap::new(),
            phoneme_id_map: {
                let mut map = HashMap::new();
                map.insert("_".to_string(), vec![0]);
                map.insert("^".to_string(), vec![1]);
                map.insert("$".to_string(), vec![2]);
                map.insert("a".to_string(), vec![4]);
                map.insert("ɪ".to_string(), vec![10]);
                map
            },
        };

        let ids = LocalModelTts::phonemes_to_ids(&config, "a   ɪ");
        assert_eq!(ids, vec![1, 0, 4, 0, 10, 0, 2]);
    }

    #[test]
    fn test_phonemes_to_ids_unknown_char_skipped() {
        let config = ModelConfig {
            audio: AudioConfig { sample_rate: 22050 },
            espeak: ESpeakConfig {
                voice: "en".to_string(),
            },
            inference: InferenceConfig {
                noise_scale: 0.667,
                length_scale: 1.0,
                noise_w: 0.8,
            },
            num_speakers: 1,
            speaker_id_map: HashMap::new(),
            phoneme_id_map: {
                let mut map = HashMap::new();
                map.insert("_".to_string(), vec![0]);
                map.insert("^".to_string(), vec![1]);
                map.insert("$".to_string(), vec![2]);
                map.insert("a".to_string(), vec![4]);
                map
            },
        };

        let ids = LocalModelTts::phonemes_to_ids(&config, "q");
        assert_eq!(
            ids,
            vec![1, 0, 2],
            "unknown char should be skipped, only BOS/PAD/EOS remain"
        );
    }

    #[test]
    fn test_phonemes_to_ids_multi_id_mapping() {
        let config = ModelConfig {
            audio: AudioConfig { sample_rate: 22050 },
            espeak: ESpeakConfig {
                voice: "en".to_string(),
            },
            inference: InferenceConfig {
                noise_scale: 0.667,
                length_scale: 1.0,
                noise_w: 0.8,
            },
            num_speakers: 1,
            speaker_id_map: HashMap::new(),
            phoneme_id_map: {
                let mut map = HashMap::new();
                map.insert("_".to_string(), vec![0]);
                map.insert("^".to_string(), vec![1]);
                map.insert("$".to_string(), vec![2]);
                map.insert("a".to_string(), vec![4, 14]);
                map
            },
        };

        let ids = LocalModelTts::phonemes_to_ids(&config, "a");
        assert_eq!(ids, vec![1, 0, 4, 14, 0, 2]);
    }

    #[test]
    fn test_phonemes_to_ids_empty_string() {
        let config = ModelConfig {
            audio: AudioConfig { sample_rate: 22050 },
            espeak: ESpeakConfig {
                voice: "en".to_string(),
            },
            inference: InferenceConfig {
                noise_scale: 0.667,
                length_scale: 1.0,
                noise_w: 0.8,
            },
            num_speakers: 1,
            speaker_id_map: HashMap::new(),
            phoneme_id_map: {
                let mut map = HashMap::new();
                map.insert("_".to_string(), vec![0]);
                map.insert("^".to_string(), vec![1]);
                map.insert("$".to_string(), vec![2]);
                map
            },
        };

        let ids = LocalModelTts::phonemes_to_ids(&config, "");
        assert_eq!(ids, vec![1, 0, 2]);
    }

    #[test]
    fn from_descriptor_lazy_loading() {
        use std::path::PathBuf;

        let desc = PiperModelDescriptor {
            id: "local-piper:test-model".into(),
            display_name: "Test Model".into(),
            onnx_path: PathBuf::from("/nonexistent/model.onnx"),
            json_path: PathBuf::from("/nonexistent/model.onnx.json"),
            sample_rate: 22050,
            phoneme_id_map: serde_json::json!({}),
        };

        let tts = LocalModelTts::from_descriptor(&desc);
        assert!(
            tts.model.lock().unwrap().is_none(),
            "model must not be loaded during construction"
        );
        assert_eq!(tts.provider_id, "local-piper:test-model");
        assert_eq!(tts.display_name, "Test Model");
    }

    #[test]
    fn readiness_model_voice_failures_do_not_poison_other_models() {
        for voices in [["invalid", "en"], ["en", "invalid"]] {
            for voice in voices {
                let result = validate_model_voice(PiperReadiness::Ready, voice, |name| {
                    if name == "invalid" {
                        Err(espeak_rs::ESpeakError("Unknown voice".into()))
                    } else {
                        Ok(vec!["phonemes".into()])
                    }
                });
                assert_eq!(result.is_ok(), voice == "en");
                if let Err(error) = result {
                    assert!(matches!(error, PiperRuntimeError::VoiceUnavailable(_)));
                }
            }
        }
    }

    #[test]
    fn readiness_unavailable_data_blocks_voice_probe() {
        let result = validate_model_voice(PiperReadiness::DataNotFound, "en", |_| {
            panic!("Voice probe must not run after shared initialization failed")
        });
        assert!(matches!(result, Err(PiperRuntimeError::NotReady(_))));
    }

    #[test]
    fn readiness_empty_voice_result_is_not_ready() {
        for phonemes in [vec![], vec![" ".into()]] {
            assert!(matches!(
                validate_model_voice(PiperReadiness::Ready, "en", |_| Ok(phonemes)),
                Err(PiperRuntimeError::VoiceUnavailable(_))
            ));
        }
    }

    #[test]
    fn readiness_classifies_probe_outcomes() {
        let ok: espeak_rs::ESpeakResult<()> = Ok(());
        let failed: espeak_rs::ESpeakResult<()> = Err(espeak_rs::ESpeakError(
            "Failed to initialize eSpeak-ng".into(),
        ));

        assert_eq!(classify_readiness(true, ok), PiperReadiness::Ready);
        assert_eq!(classify_readiness(false, Ok(())), PiperReadiness::Ready);
        assert_eq!(
            classify_readiness(true, failed.clone()),
            PiperReadiness::DataDamaged,
            "a visible local data folder that fails the probe is damaged"
        );
        assert_eq!(
            classify_readiness(false, failed),
            PiperReadiness::DataNotFound,
            "no visible local folder and a failed probe means data not found"
        );
    }

    #[test]
    fn readiness_message_uses_specific_cause_key() {
        let mut messages = BTreeMap::new();
        messages.insert(
            "tts.piper.espeak_data_damaged".to_string(),
            "damaged-specific".to_string(),
        );
        messages.insert(
            "tts.piper.espeak_data_not_found".to_string(),
            "not-found-specific".to_string(),
        );
        messages.insert(
            PIPER_NOT_READY_FALLBACK_KEY.to_string(),
            "generic".to_string(),
        );

        assert_eq!(
            not_ready_message(&messages, PiperReadiness::DataDamaged),
            "damaged-specific"
        );
        assert_eq!(
            not_ready_message(&messages, PiperReadiness::DataNotFound),
            "not-found-specific"
        );
    }

    #[test]
    fn readiness_message_falls_back_to_generic_key_then_default_text() {
        let mut messages = BTreeMap::new();
        messages.insert(
            PIPER_NOT_READY_FALLBACK_KEY.to_string(),
            "generic".to_string(),
        );
        assert_eq!(
            not_ready_message(&messages, PiperReadiness::DataDamaged),
            "generic"
        );

        let empty = BTreeMap::new();
        assert_eq!(
            not_ready_message(&empty, PiperReadiness::DataNotFound),
            PIPER_NOT_READY_FALLBACK_TEXT
        );
        assert!(not_ready_message(&empty, PiperReadiness::Ready).is_empty());
    }

    #[test]
    fn not_ready_error_keeps_verdict_for_logs() {
        let error = PiperRuntimeError::NotReady(PiperReadiness::DataDamaged);
        assert!(error.to_string().contains("not ready"));
        assert!(!PiperReadiness::DataDamaged.is_ready());
        assert!(PiperReadiness::Ready.is_ready());
    }

    #[cfg(windows)]
    #[test]
    fn normalize_verbatim_drive_path_for_clib() {
        let verbatim = PathBuf::from(r"\\?\E:\cargo-target\app tts v2\espeak-ng-data");
        assert_eq!(
            normalize_for_clib(&verbatim),
            PathBuf::from(r"E:\cargo-target\app tts v2\espeak-ng-data")
        );

        let root = PathBuf::from(r"\\?\E:\");
        assert_eq!(normalize_for_clib(&root), PathBuf::from(r"E:\"));
    }

    #[cfg(windows)]
    #[test]
    fn normalize_verbatim_unc_path_for_clib() {
        let verbatim = PathBuf::from(r"\\?\UNC\server\share\folder with space");
        assert_eq!(
            normalize_for_clib(&verbatim),
            PathBuf::from(r"\\server\share\folder with space")
        );
    }

    #[cfg(windows)]
    #[test]
    fn normalize_ordinary_paths_unchanged_for_clib() {
        let drive = PathBuf::from(r"E:\cargo-target\app-tts-v2");
        assert_eq!(normalize_for_clib(&drive), drive);

        let unc = PathBuf::from(r"\\server\share\folder with space");
        assert_eq!(normalize_for_clib(&unc), unc);

        let spaced = PathBuf::from(r"C:\Program Files\App TTS");
        assert_eq!(normalize_for_clib(&spaced), spaced);

        let relative = PathBuf::from(r"relative\path");
        assert_eq!(normalize_for_clib(&relative), relative);
    }

    #[test]
    #[ignore = "set TTSBARD_ESPEAK_TEST_DATA_PARENT to a parent dir containing espeak-ng-data"]
    fn espeak_real_phonemization_with_verbatim_resource_dir() {
        let parent = env::var("TTSBARD_ESPEAK_TEST_DATA_PARENT").expect(
            "TTSBARD_ESPEAK_TEST_DATA_PARENT must point to a directory containing espeak-ng-data",
        );
        let parent = PathBuf::from(&parent);

        let data_dir = parent.join("espeak-ng-data");
        assert!(
            data_dir.join("voices").is_dir() && data_dir.join("en_dict").is_file(),
            "espeak-ng-data (voices dir + en_dict file) must exist under the supplied parent"
        );

        let canonical =
            std::fs::canonicalize(&parent).expect("supplied parent must be canonicalizable");

        LocalModelTts::init_espeak_data(Some(canonical.clone()));

        let exported = env::var("PIPER_ESPEAKNG_DATA_DIRECTORY")
            .expect("PIPER_ESPEAKNG_DATA_DIRECTORY must be exported after init");

        #[cfg(windows)]
        {
            assert!(
                !exported.starts_with(r"\\?\"),
                "exported data directory must not be a verbatim path: {exported}"
            );
            assert_eq!(
                exported,
                normalize_for_clib(&canonical)
                    .to_string_lossy()
                    .into_owned()
            );
        }

        let readiness = PiperReadiness::assess_cached();
        assert!(
            readiness.is_ready(),
            "real eSpeak phonemization readiness must succeed, got {readiness:?}"
        );

        let phonemes = espeak_rs::text_to_phonemes("hello world", "en", None)
            .expect("neutral English phonemization must succeed");
        assert!(
            phonemes.iter().any(|text| !text.trim().is_empty()),
            "neutral English phonemization must return non-empty text"
        );
    }
}
