use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

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

#[derive(Debug, thiserror::Error)]
pub enum PiperRuntimeError {
    #[error("Failed to load model: {0}")]
    Load(String),
    #[error("Phonemization error: {0}")]
    Phonemization(String),
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
    speaker_id_map: HashMap<String, i64>,
    phoneme_id_map: HashMap<String, Vec<i64>>,
}

pub struct LocalModelTts {
    model_path: std::path::PathBuf,
    config_path: std::path::PathBuf,
    session: Mutex<Option<Session>>,
    config: Mutex<Option<ModelConfig>>,
    provider_id: String,
    display_name: String,
}

impl std::fmt::Debug for LocalModelTts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalModelTts")
            .field("model_path", &self.model_path)
            .field("config_path", &self.config_path)
            .field("provider_id", &self.provider_id)
            .field("display_name", &self.display_name)
            .finish_non_exhaustive()
    }
}

impl LocalModelTts {
    pub fn new(model_path: impl AsRef<Path>, config_path: impl AsRef<Path>) -> Self {
        Self {
            model_path: model_path.as_ref().to_path_buf(),
            config_path: config_path.as_ref().to_path_buf(),
            session: Mutex::new(None),
            config: Mutex::new(None),
            provider_id: String::new(),
            display_name: String::new(),
        }
    }

    pub fn from_descriptor(descriptor: &PiperModelDescriptor) -> Self {
        Self {
            model_path: descriptor.onnx_path.clone(),
            config_path: descriptor.json_path.clone(),
            session: Mutex::new(None),
            config: Mutex::new(None),
            provider_id: descriptor.id.clone(),
            display_name: descriptor.display_name.clone(),
        }
    }

    fn ensure_loaded(&self) -> Result<(), PiperRuntimeError> {
        {
            let session = self.session.lock().unwrap();
            if session.is_some() {
                return Ok(());
            }
        }

        let config_content = std::fs::read_to_string(&self.config_path).map_err(|e| {
            PiperRuntimeError::Load(format!(
                "Failed to read config {}: {}",
                self.config_path.display(),
                e
            ))
        })?;

        let config: ModelConfig =
            serde_json::from_str(&config_content).map_err(|e| {
                PiperRuntimeError::Load(format!(
                    "Failed to parse config {}: {}",
                    self.config_path.display(),
                    e
                ))
            })?;

        let onnx_session = Session::builder()
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

        {
            let mut session = self.session.lock().unwrap();
            *session = Some(onnx_session);
        }
        {
            let mut cfg = self.config.lock().unwrap();
            *cfg = Some(config);
        }

        Ok(())
    }

    fn phonemes_to_ids(config: &ModelConfig, phonemes: &str) -> Vec<i64> {
        let map = &config.phoneme_id_map;
        let pad_id = map
            .get(&PAD.to_string())
            .and_then(|v| v.first())
            .copied()
            .unwrap_or(0);
        let bos_id = map
            .get(&BOS.to_string())
            .and_then(|v| v.first())
            .copied()
            .unwrap_or(0);
        let eos_id = map
            .get(&EOS.to_string())
            .and_then(|v| v.first())
            .copied()
            .unwrap_or(0);

        let mut ids = Vec::new();
        ids.push(bos_id);
        for token in phonemes.split_whitespace() {
            if let Some(id) = map.get(token).and_then(|v| v.first()) {
                ids.push(*id);
                ids.push(pad_id);
            }
        }
        ids.push(eos_id);
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

        let input_t = Tensor::<i64>::from_array(([1, input_len], ids.into_boxed_slice()))
            .map_err(|e| {
                PiperRuntimeError::Inference(format!("Failed to create input tensor: {}", e))
            })?;

        let lengths_t = Tensor::<i64>::from_array((
            [1],
            vec![input_len as i64].into_boxed_slice(),
        ))
        .map_err(|e| {
            PiperRuntimeError::Inference(format!("Failed to create lengths tensor: {}", e))
        })?;

        let scales_t =
            Tensor::<f32>::from_array((
                [3],
                vec![noise_scale, length_scale, noise_w].into_boxed_slice(),
            ))
            .map_err(|e| {
                PiperRuntimeError::Inference(format!("Failed to create scales tensor: {}", e))
            })?;

        let outputs = if config.num_speakers > 1 {
            let sid = speaker_id.unwrap_or(0);
            let sid_t = Tensor::<i64>::from_array(([1], vec![sid].into_boxed_slice()))
                .map_err(|e| {
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

        let (_, audio) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| PiperRuntimeError::Inference(format!("Failed to extract output: {}", e)))?;

        Ok(audio.to_vec())
    }
}

#[async_trait]
impl TtsEngine for LocalModelTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        self.ensure_loaded().map_err(|e| e.to_string())?;

        let config = {
            let guard = self.config.lock().unwrap();
            guard
                .clone()
                .ok_or_else(|| PiperRuntimeError::NotLoaded.to_string())?
        };

        let voice = &config.espeak.voice;
        let phonemes = espeak_rs::text_to_phonemes(text, voice, None)
            .map_err(|e| format!("Phonemization failed: {}", e))?
            .join(" ");

        let inf = &config.inference;
        let noise_scale = inf.noise_scale;
        let length_scale = inf.length_scale;
        let noise_w = inf.noise_w;

        let samples = {
            let mut session_guard = self.session.lock().unwrap();
            let session = session_guard
                .as_mut()
                .ok_or_else(|| PiperRuntimeError::NotLoaded.to_string())?;
            Self::run_inference(
                session,
                &config,
                &phonemes,
                noise_scale,
                length_scale,
                noise_w,
                None,
            )
            .map_err(|e| e.to_string())?
        };

        encode_wav(&samples, config.audio.sample_rate, 1)
            .map_err(|e| format!("Failed to encode WAV: {}", e))
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

        assert!(
            !audio.is_empty(),
            "synthesized audio should not be empty"
        );

        let pcm = crate::audio::effects::decode_audio(&audio)
            .expect("output should be valid WAV");

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
        assert_eq!(ids, vec![1, 4, 0, 5, 0, 6, 0, 2]);
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
        assert_eq!(ids, vec![1, 4, 0, 2]);
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
            tts.session.lock().unwrap().is_none(),
            "session must not be loaded during construction"
        );
        assert_eq!(tts.provider_id, "local-piper:test-model");
        assert_eq!(tts.display_name, "Test Model");
    }
}
