use std::collections::HashMap;
use std::env;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

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

pub struct LocalModelTts {
    model_path: std::path::PathBuf,
    config_path: std::path::PathBuf,
    model: Mutex<Option<ModelState>>,
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
                env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &data_dir);
                tracing::info!(
                    dir = %data_dir.display(),
                    "espeak-ng data directory set"
                );
                return;
            }

            if let Ok(cwd) = env::current_dir() {
                let p = cwd.join("espeak-ng-data");
                if p.join("voices").exists() && p.join("en_dict").exists() {
                    env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &p);
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
                        env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &p);
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

    #[cfg(test)]
    pub fn new(model_path: impl AsRef<Path>, config_path: impl AsRef<Path>) -> Self {
        Self {
            model_path: model_path.as_ref().to_path_buf(),
            config_path: config_path.as_ref().to_path_buf(),
            model: Mutex::new(None),
            provider_id: String::new(),
            display_name: String::new(),
        }
    }

    pub fn from_descriptor(descriptor: &PiperModelDescriptor) -> Self {
        Self {
            model_path: descriptor.onnx_path.clone(),
            config_path: descriptor.json_path.clone(),
            model: Mutex::new(None),
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
}

#[async_trait]
impl TtsEngine for LocalModelTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MODEL_PATH: &str =
        "/home/aefimov/ProjectsMy/loca_tts/ru_RU-irina-medium-cloned.onnx";
    const TEST_CONFIG_PATH: &str =
        "/home/aefimov/ProjectsMy/loca_tts/ru_RU-irina-medium-cloned.onnx.json";

    fn model_exists() -> bool {
        Path::new(TEST_MODEL_PATH).exists() && Path::new(TEST_CONFIG_PATH).exists()
    }

    #[tokio::test]
    async fn test_local_model_tts_synthesize() {
        if !model_exists() {
            eprintln!("Skipping test: model not found at {TEST_MODEL_PATH}");
            return;
        }

        let tts = LocalModelTts::new(TEST_MODEL_PATH, TEST_CONFIG_PATH);

        let audio = tts
            .synthesize("Привет мир")
            .await
            .expect("synthesize should succeed");

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
}
