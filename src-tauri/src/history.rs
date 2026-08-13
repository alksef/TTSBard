use anyhow::{Context, Result};
use chrono::Utc;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::config::replace_file_atomically;

const PHRASE_HISTORY_SIZE: usize = 200;

static HISTORY_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn history_write_lock() -> &'static Mutex<()> {
    HISTORY_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub word: String,
    pub count: u32,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PhraseEntry {
    // #[serde(default)] на каждом поле + Default на структуре — backwards-compatibility:
    // при добавлении новых полей старые phrase_history.json продолжат десериализоваться
    // (урок playback_pause / HotkeySettings, commit 704be39).
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub last_used: i64,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub voice: String,
    #[serde(default)]
    pub cache_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NgramData {
    pub bigrams: HashMap<String, HashMap<String, u32>>,
    pub trigrams: HashMap<String, HashMap<String, u32>>,
}

fn trigram_key(w1: &str, w2: &str) -> String {
    format!("{}||{}", w1, w2)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryData {
    pub entries: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseSuggestion {
    pub text: String,
    pub count: u32,
}

#[derive(Debug)]
pub struct HistoryManager {
    path: PathBuf,
    ngram_path: PathBuf,
    phrase_path: PathBuf,
    data: RwLock<HistoryData>,
    ngrams: RwLock<NgramData>,
    phrases: RwLock<Vec<PhraseEntry>>,
}

fn clean_token(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

fn save_history_sync(
    path: &Path,
    ngram_path: &Path,
    data: &HistoryData,
    ngrams: &NgramData,
) -> Result<()> {
    let _lock = history_write_lock().lock();

    let content = serde_json::to_string_pretty(data).context("Failed to serialize history")?;
    replace_file_atomically(path, content.as_bytes())
        .with_context(|| format!("Failed to persist history to {:?}", path))?;

    let content = serde_json::to_string_pretty(ngrams).context("Failed to serialize ngrams")?;
    replace_file_atomically(ngram_path, content.as_bytes())
        .with_context(|| format!("Failed to persist ngrams to {:?}", ngram_path))?;

    Ok(())
}

fn save_phrases_sync(path: &Path, phrases: &[PhraseEntry]) -> Result<()> {
    let _lock = history_write_lock().lock();

    let content = serde_json::to_string_pretty(phrases).context("Failed to serialize phrases")?;
    replace_file_atomically(path, content.as_bytes())
        .with_context(|| format!("Failed to persist phrases to {:?}", path))?;

    Ok(())
}

impl HistoryManager {
    pub fn new(path: PathBuf, ngram_path: PathBuf, phrase_path: PathBuf) -> Self {
        let _ = fs::create_dir_all(path.parent().unwrap_or(&path));

        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str::<HistoryData>(&c).ok())
            .unwrap_or_default();

        let ngrams = fs::read_to_string(&ngram_path)
            .ok()
            .and_then(|c| serde_json::from_str::<NgramData>(&c).ok())
            .unwrap_or_default();

        let phrases = fs::read_to_string(&phrase_path)
            .ok()
            .and_then(|c| serde_json::from_str::<Vec<PhraseEntry>>(&c).ok())
            .unwrap_or_default();

        HistoryManager {
            path,
            ngram_path,
            phrase_path,
            data: RwLock::new(data),
            ngrams: RwLock::new(ngrams),
            phrases: RwLock::new(phrases),
        }
    }

    pub fn record_text(&self, text: &str) -> Result<()> {
        let tokens: Vec<String> = text
            .split_whitespace()
            .filter(|t| t.len() >= 2)
            .map(clean_token)
            .filter(|t| t.len() >= 2)
            .collect();

        if tokens.is_empty() {
            return Ok(());
        }

        let mut data = self.data.write();
        let mut ngrams = self.ngrams.write();

        let mut data_candidate = data.clone();
        let mut ngrams_candidate = ngrams.clone();

        for token in &tokens {
            if let Some(entry) = data_candidate.entries.iter_mut().find(|e| e.word == *token) {
                entry.count += 1;
                entry.last_used = Utc::now().timestamp();
            } else {
                data_candidate.entries.push(HistoryEntry {
                    word: token.clone(),
                    count: 1,
                    last_used: Utc::now().timestamp(),
                });
            }
        }

        for window in tokens.windows(2) {
            *ngrams_candidate
                .bigrams
                .entry(window[0].clone())
                .or_default()
                .entry(window[1].clone())
                .or_insert(0) += 1;
        }

        for window in tokens.windows(3) {
            let key = trigram_key(&window[0], &window[1]);
            *ngrams_candidate
                .trigrams
                .entry(key)
                .or_default()
                .entry(window[2].clone())
                .or_insert(0) += 1;
        }

        save_history_sync(
            &self.path,
            &self.ngram_path,
            &data_candidate,
            &ngrams_candidate,
        )?;

        *data = data_candidate;
        *ngrams = ngrams_candidate;
        Ok(())
    }

    pub fn suggest(&self, query: &str, limit: usize) -> Vec<HistoryEntry> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let data = self.data.read();

        let mut results: Vec<HistoryEntry> = data
            .entries
            .iter()
            .filter(|e| e.word.contains(&query_lower))
            .cloned()
            .collect();

        results.sort_by(|a, b| b.count.cmp(&a.count).then(b.last_used.cmp(&a.last_used)));
        results.truncate(limit);
        results
    }

    pub fn suggest_phrase(&self, context: &str, limit: usize) -> Vec<PhraseSuggestion> {
        let words: Vec<&str> = context
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .collect();

        if words.is_empty() {
            return Vec::new();
        }

        let ngrams = self.ngrams.read();
        let mut suggestions: Vec<PhraseSuggestion> = Vec::new();

        if words.len() >= 2 {
            let w1 = words[words.len() - 2].to_lowercase();
            let w2 = words[words.len() - 1].to_lowercase();
            let key = trigram_key(&w1, &w2);
            if let Some(next_words) = ngrams.trigrams.get(&key) {
                for (next_word, count) in next_words.iter() {
                    suggestions.push(PhraseSuggestion {
                        text: next_word.clone(),
                        count: *count,
                    });
                }
            }
        }

        if let Some(last_word) = words.last() {
            let w = last_word.to_lowercase();
            if let Some(next_words) = ngrams.bigrams.get(&w) {
                for (next_word, count) in next_words.iter() {
                    if !suggestions.iter().any(|s| s.text == *next_word) {
                        suggestions.push(PhraseSuggestion {
                            text: next_word.clone(),
                            count: *count,
                        });
                    }
                }
            }
        }

        suggestions.sort_by_key(|b| std::cmp::Reverse(b.count));
        suggestions.truncate(limit);
        suggestions
    }

    pub fn clear(&self) -> Result<()> {
        let mut data = self.data.write();
        let mut ngrams = self.ngrams.write();

        let empty_history = HistoryData::default();
        let empty_ngrams = NgramData::default();
        save_history_sync(&self.path, &self.ngram_path, &empty_history, &empty_ngrams)?;

        *data = empty_history;
        *ngrams = empty_ngrams;
        Ok(())
    }

    // Контракт нормализации фраз: храним text.trim(); дедупликация и поиск —
    // case-insensitive по подстроке (to_lowercase()). См. также get_phrases.
    // Не менять без обновления обоих методов — это сломает дедупликацию/поиск.
    pub fn record_phrase(&self, text: &str) -> Result<()> {
        self.record_phrase_with_meta(text, "", "", "")
    }

    pub fn record_phrase_with_meta(
        &self,
        text: &str,
        provider: &str,
        voice: &str,
        cache_key: &str,
    ) -> Result<()> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let trimmed_lower = trimmed.to_lowercase();
        let now = Utc::now().timestamp();

        let mut phrases = self.phrases.write();
        let mut candidate = phrases.clone();

        let found = if provider.is_empty() && voice.is_empty() {
            candidate
                .iter_mut()
                .find(|e| e.text.trim().to_lowercase() == trimmed_lower)
        } else {
            let prov_lower = provider.to_lowercase();
            let voice_lower = voice.to_lowercase();
            candidate.iter_mut().find(|e| {
                e.text.trim().to_lowercase() == trimmed_lower
                    && e.provider.to_lowercase() == prov_lower
                    && e.voice.to_lowercase() == voice_lower
                    && e.cache_key == cache_key
            })
        };

        if let Some(existing) = found {
            existing.count += 1;
            existing.last_used = now;
            if !cache_key.is_empty() {
                existing.provider = provider.to_string();
                existing.voice = voice.to_string();
                existing.cache_key = cache_key.to_string();
            }
        } else {
            candidate.push(PhraseEntry {
                id: uuid::Uuid::new_v4().to_string(),
                text: trimmed.to_string(),
                count: 1,
                last_used: now,
                provider: provider.to_string(),
                voice: voice.to_string(),
                cache_key: cache_key.to_string(),
            });
        }

        while candidate.len() > PHRASE_HISTORY_SIZE {
            if let Some(oldest_pos) = candidate
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(i, _)| i)
            {
                candidate.remove(oldest_pos);
            } else {
                break;
            }
        }

        save_phrases_sync(&self.phrase_path, &candidate)?;

        *phrases = candidate;
        Ok(())
    }

    pub fn get_phrases(&self, filter: Option<&str>, limit: usize) -> Vec<PhraseEntry> {
        let phrases = self.phrases.read();
        let mut results: Vec<PhraseEntry> = if let Some(f) = filter {
            let f_lower = f.to_lowercase();
            phrases
                .iter()
                .filter(|e| e.text.to_lowercase().contains(&f_lower))
                .cloned()
                .collect()
        } else {
            phrases.clone()
        };

        results.sort_by_key(|b| std::cmp::Reverse(b.last_used));
        results.truncate(limit);
        results
    }

    pub fn delete_phrase(&self, id: &str) -> Result<()> {
        let mut phrases = self.phrases.write();
        let mut candidate = phrases.clone();
        candidate.retain(|e| e.id != id);

        save_phrases_sync(&self.phrase_path, &candidate)?;

        *phrases = candidate;
        Ok(())
    }

    pub fn clear_phrases(&self) -> Result<()> {
        let mut phrases = self.phrases.write();

        let empty: Vec<PhraseEntry> = Vec::new();
        save_phrases_sync(&self.phrase_path, &empty)?;

        *phrases = empty;
        Ok(())
    }
}

pub fn history_paths() -> Result<(PathBuf, PathBuf, PathBuf)> {
    let dir = dirs::config_dir()
        .context("Failed to get config dir")?
        .join("ttsbard");
    fs::create_dir_all(&dir).context("Failed to create ttsbard dir")?;
    Ok((
        dir.join("input_history.json"),
        dir.join("ngrams.json"),
        dir.join("phrase_history.json"),
    ))
}

const CACHE_NAMESPACE: uuid::Uuid = uuid::Uuid::from_bytes([
    0x63, 0x78, 0xb2, 0xe0, 0x1c, 0x4d, 0x4e, 0x8a, 0x9c, 0x3f, 0x7a, 0x2b, 0x1d, 0x5e, 0x6f, 0x0a,
]);

pub fn cache_dir_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("Failed to get config dir")?
        .join("ttsbard")
        .join("audio_cache");
    fs::create_dir_all(&dir).context("Failed to create audio_cache dir")?;
    Ok(dir)
}

pub fn build_cache_key(
    processed_text: &str,
    provider: &str,
    voice: &str,
    effects_fingerprint: u64,
) -> String {
    let combined = format!(
        "{}|{}|{}|{:x}",
        processed_text.trim().to_lowercase(),
        provider.to_lowercase(),
        voice.to_lowercase(),
        effects_fingerprint
    );
    uuid::Uuid::new_v5(&CACHE_NAMESPACE, combined.as_bytes()).to_string()
}

pub fn get_cache_file_path(cache_key: &str) -> Result<PathBuf> {
    if uuid::Uuid::parse_str(cache_key).is_err() {
        anyhow::bail!("CacheMiss");
    }
    let dir = cache_dir_path()?;
    Ok(dir.join(format!("{}.wav", cache_key)))
}

pub fn save_audio_cache(cache_key: &str, pcm: &crate::audio::AudioPcm) -> Result<()> {
    let wav_bytes = crate::audio::effects::encode_wav(&pcm.samples, pcm.sample_rate, pcm.channels)
        .map_err(|e| anyhow::anyhow!("Failed to encode cache WAV: {}", e))?;
    let path = get_cache_file_path(cache_key)?;
    replace_file_atomically(&path, &wav_bytes).with_context(|| "Failed to write audio cache")?;
    Ok(())
}

pub fn read_audio_cache(cache_key: &str) -> Result<crate::audio::AudioPcm> {
    let path = get_cache_file_path(cache_key)?;
    if !path.exists() {
        anyhow::bail!("CacheMiss");
    }
    let file_data =
        fs::read(&path).map_err(|e| anyhow::anyhow!("Failed to read cache file: {}", e))?;
    crate::audio::decode_audio(&file_data)
        .map_err(|e| anyhow::anyhow!("Failed to decode cached audio: {}", e))
}

pub fn compute_effects_fingerprint(
    effects: &crate::config::AudioEffectsSettings,
    dsp: &crate::config::DspSettings,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    effects.enabled.hash(&mut hasher);
    effects.pitch.hash(&mut hasher);
    effects.speed.hash(&mut hasher);
    effects.volume.hash(&mut hasher);
    effects.enhance_enabled.hash(&mut hasher);
    effects.enhance_atten_db.to_bits().hash(&mut hasher);
    effects.formant_preserved.hash(&mut hasher);
    effects.boundary_cleanup_enabled.hash(&mut hasher);
    dsp.eq.enabled.hash(&mut hasher);
    dsp.compressor.enabled.hash(&mut hasher);
    dsp.limiter.enabled.hash(&mut hasher);
    dsp.eq.bands.iter().for_each(|b| {
        b.enabled.hash(&mut hasher);
        b.frequency_hz.to_bits().hash(&mut hasher);
        b.gain_db.to_bits().hash(&mut hasher);
        b.q.to_bits().hash(&mut hasher);
    });
    dsp.compressor.threshold_db.to_bits().hash(&mut hasher);
    dsp.compressor.ratio.to_bits().hash(&mut hasher);
    dsp.compressor.attack_ms.to_bits().hash(&mut hasher);
    dsp.compressor.release_ms.to_bits().hash(&mut hasher);
    dsp.compressor.knee_db.to_bits().hash(&mut hasher);
    dsp.compressor.makeup_db.to_bits().hash(&mut hasher);
    dsp.limiter.ceiling_db.to_bits().hash(&mut hasher);
    dsp.limiter.release_ms.to_bits().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn manager_in_tmp() -> (HistoryManager, PathBuf, PathBuf, PathBuf) {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("ttsbard-history-test-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        let p1 = dir.join("input_history.json");
        let p2 = dir.join("ngrams.json");
        let p3 = dir.join("phrase_history.json");
        (
            HistoryManager::new(p1.clone(), p2.clone(), p3.clone()),
            p1,
            p2,
            p3,
        )
    }

    #[test]
    fn test_concurrent_phrase_recording() {
        let (mgr, p1, p2, p3) = manager_in_tmp();
        let mgr_arc = std::sync::Arc::new(mgr);
        let mut threads = vec![];

        for i in 0..20 {
            let mgr_clone = std::sync::Arc::clone(&mgr_arc);
            threads.push(std::thread::spawn(move || {
                mgr_clone.record_phrase(&format!("test phrase {}", i)).unwrap();
            }));
        }

        for t in threads {
            t.join().unwrap();
        }

        let content = fs::read_to_string(&p3).unwrap();
        let loaded: Vec<PhraseEntry> = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded.len(), 20);

        let _ = fs::remove_file(&p1);
        let _ = fs::remove_file(&p2);
        let _ = fs::remove_file(&p3);
    }

    #[test]
    fn test_old_json_deserialization() {
        let old_json = r#"[
            {"id": "abc-123", "text": "hello world", "count": 5, "last_used": 1700000000},
            {"id": "def-456", "text": "test phrase", "count": 2, "last_used": 1700000001}
        ]"#;
        let entries: Vec<PhraseEntry> = serde_json::from_str(old_json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "abc-123");
        assert_eq!(entries[0].text, "hello world");
        assert_eq!(entries[0].provider, "");
        assert_eq!(entries[0].voice, "");
        assert_eq!(entries[0].cache_key, "");
        assert_eq!(entries[1].id, "def-456");
    }

    #[test]
    fn test_cache_key_separation() {
        let key1 = build_cache_key("hello", "openai", "alloy", 0);
        let key2 = build_cache_key("hello", "silero", "", 0);
        let key3 = build_cache_key("hello", "openai", "alloy", 1);
        let key4 = build_cache_key("world", "openai", "alloy", 0);

        assert_ne!(key1, key2);
        assert_ne!(key1, key3);
        assert_ne!(key1, key4);
    }

    #[test]
    fn test_cache_key_deterministic() {
        let key_a = build_cache_key("test text", "openai", "alloy", 42);
        let key_b = build_cache_key("test text", "openai", "alloy", 42);
        assert_eq!(key_a, key_b);
    }

    #[test]
    fn test_cache_write_read_roundtrip() {
        let pcm = crate::audio::AudioPcm::new(vec![0.0f32; 4800], 48000, 1).unwrap();

        let key = build_cache_key("roundtrip test", "openai", "alloy", 0);
        save_audio_cache(&key, &pcm).unwrap();

        let decoded = read_audio_cache(&key).unwrap();
        assert_eq!(decoded.sample_rate, 48000);
        assert_eq!(decoded.channels, 1);
        assert_eq!(decoded.samples.len(), 4800);

        let path = get_cache_file_path(&key).unwrap();
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_missing_cache_error() {
        let result = read_audio_cache("nonexistent-key-00000000-0000-0000-0000-000000000000");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("CacheMiss") || err.contains("Failed to read cache file"),
            "Expected CacheMiss error, got: {}",
            err
        );
    }

    #[test]
    fn test_record_phrase_with_meta_dedup_different_providers() {
        let (mgr, p1, p2, p3) = manager_in_tmp();

        mgr.record_phrase_with_meta("hello world", "openai", "alloy", "key-1").unwrap();
        mgr.record_phrase_with_meta("hello world", "silero", "voice-2", "key-2").unwrap();
        mgr.record_phrase_with_meta("hello world", "openai", "alloy", "key-1").unwrap();

        let phrases = mgr.get_phrases(None, 100);
        assert_eq!(
            phrases.len(),
            2,
            "Different providers should create separate entries, same provider+voice should dedup"
        );

        let openai_entry = phrases.iter().find(|e| e.provider == "openai").unwrap();
        assert_eq!(openai_entry.voice, "alloy");
        assert_eq!(openai_entry.count, 2);

        let silero_entry = phrases.iter().find(|e| e.provider == "silero").unwrap();
        assert_eq!(silero_entry.voice, "voice-2");
        assert_eq!(silero_entry.count, 1);

        let _ = fs::remove_file(&p1);
        let _ = fs::remove_file(&p2);
        let _ = fs::remove_file(&p3);
    }

    #[test]
    fn test_record_phrase_backward_compat_dedup() {
        let (mgr, p1, p2, p3) = manager_in_tmp();

        mgr.record_phrase("test phrase").unwrap();
        mgr.record_phrase("test phrase").unwrap();

        let phrases = mgr.get_phrases(None, 100);
        assert_eq!(phrases.len(), 1);
        assert_eq!(phrases[0].count, 2);
        assert_eq!(phrases[0].provider, "");
        assert_eq!(phrases[0].voice, "");
        assert_eq!(phrases[0].cache_key, "");

        let _ = fs::remove_file(&p1);
        let _ = fs::remove_file(&p2);
        let _ = fs::remove_file(&p3);
    }

    #[test]
    fn test_record_phrase_with_meta_same_provider_voice_dedup() {
        let (mgr, p1, p2, p3) = manager_in_tmp();

        mgr.record_phrase_with_meta("hello", "openai", "alloy", "k1").unwrap();
        mgr.record_phrase_with_meta("hello", "openai", "alloy", "k1").unwrap();
        mgr.record_phrase_with_meta("hello", "openai", "alloy", "k1").unwrap();

        let phrases = mgr.get_phrases(None, 100);
        assert_eq!(phrases.len(), 1);
        assert_eq!(phrases[0].count, 3);
        assert_eq!(phrases[0].provider, "openai");
        assert_eq!(phrases[0].voice, "alloy");

        let _ = fs::remove_file(&p1);
        let _ = fs::remove_file(&p2);
        let _ = fs::remove_file(&p3);
    }

    #[test]
    fn test_effects_fingerprint_different() {
        let defaults = crate::config::AudioEffectsSettings::default();
        let dsp_defaults = crate::config::DspSettings::default();

        let mut alt = defaults.clone();
        alt.pitch = 5;

        let fp1 = compute_effects_fingerprint(&defaults, &dsp_defaults);
        let fp2 = compute_effects_fingerprint(&alt, &dsp_defaults);

        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_effects_fingerprint_same() {
        let a = crate::config::AudioEffectsSettings::default();
        let b = crate::config::AudioEffectsSettings::default();
        let dsp = crate::config::DspSettings::default();

        assert_eq!(
            compute_effects_fingerprint(&a, &dsp),
            compute_effects_fingerprint(&b, &dsp)
        );
    }

    #[test]
    fn test_cache_key_filename_safe() {
        let key = build_cache_key("hello world! @#$%", "notebook", "", 12345);
        assert!(!key.contains('/'));
        assert!(!key.contains('\\'));
        assert!(!key.contains(':'));
        assert!(!key.contains(' '));

        let path = get_cache_file_path(&key).unwrap();
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with(".wav"));
    }

    #[test]
    fn test_record_phrase_meta_updates_existing_no_meta_entry() {
        let (mgr, p1, p2, p3) = manager_in_tmp();

        mgr.record_phrase("hello world").unwrap();
        mgr.record_phrase_with_meta("hello world", "openai", "alloy", "cache-x").unwrap();

        let phrases = mgr.get_phrases(None, 100);
        assert_eq!(phrases.len(), 2);
        let metadata_entry = phrases
            .iter()
            .find(|entry| entry.provider == "openai")
            .unwrap();
        assert_eq!(metadata_entry.voice, "alloy");
        assert_eq!(metadata_entry.cache_key, "cache-x");
        assert_eq!(metadata_entry.count, 1);

        let _ = fs::remove_file(&p1);
        let _ = fs::remove_file(&p2);
        let _ = fs::remove_file(&p3);
    }

    #[test]
    fn record_phrase_preserves_published_state_on_persistence_failure() {
        let (mgr, _p1, _p2, p3) = manager_in_tmp();

        mgr.record_phrase("kept phrase").unwrap();
        assert_eq!(mgr.get_phrases(None, 100).len(), 1);

        // Break persistence: replace the parent directory with a regular file so
        // the phrases temporary file can no longer be created.
        let parent = p3.parent().unwrap().to_path_buf();
        fs::remove_dir_all(&parent).unwrap();
        fs::write(&parent, "not a directory").unwrap();

        let result = mgr.record_phrase("lost phrase");
        assert!(
            result.is_err(),
            "record_phrase must fail when persistence is broken"
        );

        let phrases = mgr.get_phrases(None, 100);
        assert_eq!(phrases.len(), 1);
        assert_eq!(phrases[0].text, "kept phrase");

        let _ = fs::remove_file(&parent);
    }
}
