use anyhow::{Context, Result};
use chrono::Utc;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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

/// Russian vowel characters (the same set used by the stress adapters).
fn is_russian_vowel(ch: char) -> bool {
    matches!(
        ch,
        'А' | 'а'
            | 'Е'
            | 'е'
            | 'Ё'
            | 'ё'
            | 'И'
            | 'и'
            | 'О'
            | 'о'
            | 'У'
            | 'у'
            | 'Ы'
            | 'ы'
            | 'Э'
            | 'э'
            | 'Ю'
            | 'ю'
            | 'Я'
            | 'я'
    )
}

/// Remove Piper stress markers (U+0301 combining acute after each stressed
/// vowel) from provider text.
fn strip_piper_markers(text: &str) -> String {
    text.chars().filter(|c| *c != '\u{0301}').collect()
}

/// Remove Silero stress markers (`+` immediately before a Russian vowel).
/// A `+` that is not immediately followed by a Russian vowel stays literal.
fn strip_silero_markers(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '+' && i + 1 < chars.len() && is_russian_vowel(chars[i + 1]) {
            i += 1;
            continue;
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

/// Derive the clean insert text from provider text by stripping the provider's
/// stress markers. Piper uses U+0301 after the stressed vowel; Silero inserts
/// `+` before the stressed Russian vowel. Unknown/plain providers return the
/// input unchanged.
pub fn strip_provider_markers(text: &str, provider: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "piper" => strip_piper_markers(text),
        "silero" => strip_silero_markers(text),
        _ => text.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PhraseEntry {
    // provider_text — provider-specific текст (с маркерами ударений), который
    // отправляется в TTS, участвует в cache key и отображается в истории, чтобы
    // пользователь видел фактические ударения.
    pub id: String,
    pub provider_text: String,
    pub insert_text: String,
    pub count: u32,
    pub last_used: i64,
    pub provider: String,
    pub voice: String,
    pub cache_key: String,
}

// Backwards-compatible deserialization: старый phrase_history.json хранил одно
// поле `text`. Новый формат хранит явные `provider_text` и `insert_text`.
// При чтении legacy-записи `provider_text` сохраняет старое значение, а
// `insert_text` выводится из него снятием provider-specific markers.
impl<'de> Deserialize<'de> for PhraseEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct PhraseEntryRaw {
            #[serde(default)]
            id: String,
            #[serde(default)]
            text: String,
            #[serde(default)]
            provider_text: String,
            #[serde(default)]
            insert_text: String,
            #[serde(default)]
            count: u32,
            #[serde(default)]
            last_used: i64,
            #[serde(default)]
            provider: String,
            #[serde(default)]
            voice: String,
            #[serde(default)]
            cache_key: String,
        }

        let raw = PhraseEntryRaw::deserialize(deserializer)?;

        let provider_text = if !raw.provider_text.is_empty() {
            raw.provider_text
        } else {
            raw.text
        };
        let insert_text = if !raw.insert_text.is_empty() {
            raw.insert_text
        } else {
            strip_provider_markers(&provider_text, &raw.provider)
        };

        Ok(PhraseEntry {
            id: raw.id,
            provider_text,
            insert_text,
            count: raw.count,
            last_used: raw.last_used,
            provider: raw.provider,
            voice: raw.voice,
            cache_key: raw.cache_key,
        })
    }
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
    /// Monotonic counters bumped on each in-place mutation. Used to make
    /// rollback after a failed persist conditional: the snapshot is restored
    /// only when no concurrent writer has built on top of our mutation.
    history_version: AtomicU64,
    phrase_version: AtomicU64,
}

fn clean_token(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

/// Serialize and persist the *current* history/ngram state while holding only
/// the global history write lock (never the manager's RwLock guards), so
/// readers are not blocked by fsync.
///
/// Serialization reads the freshest published state while the global lock is
/// held, which keeps the final file contents equal to the final in-memory
/// state even when several writers race: a later writer always writes a
/// superset of what an earlier writer saw.
fn persist_history_current(
    path: &Path,
    ngram_path: &Path,
    data: &RwLock<HistoryData>,
    ngrams: &RwLock<NgramData>,
) -> Result<()> {
    let _lock = history_write_lock().lock();

    let content =
        serde_json::to_string_pretty(&*data.read()).context("Failed to serialize history")?;
    replace_file_atomically(path, content.as_bytes())
        .with_context(|| format!("Failed to persist history to {:?}", path))?;

    let content =
        serde_json::to_string_pretty(&*ngrams.read()).context("Failed to serialize ngrams")?;
    replace_file_atomically(ngram_path, content.as_bytes())
        .with_context(|| format!("Failed to persist ngrams to {:?}", ngram_path))?;

    Ok(())
}

/// Serialize and persist the *current* phrases state under the global history
/// write lock only (see [`persist_history_current`] for the rationale).
fn persist_phrases_current(path: &Path, phrases: &RwLock<Vec<PhraseEntry>>) -> Result<()> {
    let _lock = history_write_lock().lock();

    let content =
        serde_json::to_string_pretty(&*phrases.read()).context("Failed to serialize phrases")?;
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
            history_version: AtomicU64::new(0),
            phrase_version: AtomicU64::new(0),
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

        // Mutate in place under the write locks — no full dataset clones — and
        // release the locks BEFORE the file writes (see persist_history_current).
        let mut data = self.data.write();
        let mut ngrams = self.ngrams.write();

        for token in &tokens {
            if let Some(entry) = data.entries.iter_mut().find(|e| e.word == *token) {
                entry.count += 1;
                entry.last_used = Utc::now().timestamp();
            } else {
                data.entries.push(HistoryEntry {
                    word: token.clone(),
                    count: 1,
                    last_used: Utc::now().timestamp(),
                });
            }
        }

        for window in tokens.windows(2) {
            *ngrams
                .bigrams
                .entry(window[0].clone())
                .or_default()
                .entry(window[1].clone())
                .or_insert(0) += 1;
        }

        for window in tokens.windows(3) {
            let key = trigram_key(&window[0], &window[1]);
            *ngrams
                .trigrams
                .entry(key)
                .or_default()
                .entry(window[2].clone())
                .or_insert(0) += 1;
        }

        drop(ngrams);
        drop(data);

        // Publish-immediately semantics: the mutation is already visible to
        // readers. A write failure still returns Err but leaves the in-memory
        // state internally consistent (it simply stays ahead of the file).
        // This deliberately avoids cloning the full data/ngrams datasets just
        // to roll back; no existing test requires persist-before-publish here.
        persist_history_current(&self.path, &self.ngram_path, &self.data, &self.ngrams)
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

        // Rare user-triggered operation: snapshot for rollback is acceptable
        // here so a failed write leaves the in-memory history untouched.
        let data_snapshot = data.clone();
        let ngrams_snapshot = ngrams.clone();

        *data = HistoryData::default();
        *ngrams = NgramData::default();

        let my_version = self.history_version.fetch_add(1, Ordering::SeqCst) + 1;

        drop(ngrams);
        drop(data);

        if let Err(e) =
            persist_history_current(&self.path, &self.ngram_path, &self.data, &self.ngrams)
        {
            let mut data = self.data.write();
            let mut ngrams = self.ngrams.write();
            if self.history_version.load(Ordering::SeqCst) == my_version {
                *data = data_snapshot;
                *ngrams = ngrams_snapshot;
            }
            return Err(e);
        }
        Ok(())
    }

    // Контракт нормализации фраз: храним provider_text.trim(); дедупликация и
    // поиск — case-insensitive по подстроке (to_lowercase()). См. также
    // get_phrases. Не менять без обновления обоих методов — это сломает
    // дедупликацию/поиск.
    pub fn record_phrase(&self, text: &str) -> Result<()> {
        self.record_phrase_with_meta(text, text, "", "", "")
    }

    pub fn record_phrase_with_meta(
        &self,
        provider_text: &str,
        insert_text: &str,
        provider: &str,
        voice: &str,
        cache_key: &str,
    ) -> Result<()> {
        let trimmed = provider_text.trim();
        let insert_trimmed = insert_text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let trimmed_lower = trimmed.to_lowercase();
        let now = Utc::now().timestamp();

        let mut phrases = self.phrases.write();
        // Phrase history is bounded (PHRASE_HISTORY_SIZE), so a snapshot clone
        // for rollback is cheap and keeps persist-before-publish semantics: a
        // failed write must not expose the unpersisted entry.
        let snapshot = phrases.clone();

        let found = if provider.is_empty() && voice.is_empty() {
            phrases
                .iter_mut()
                .find(|e| e.provider_text.trim().to_lowercase() == trimmed_lower)
        } else {
            let prov_lower = provider.to_lowercase();
            let voice_lower = voice.to_lowercase();
            phrases.iter_mut().find(|e| {
                e.provider_text.trim().to_lowercase() == trimmed_lower
                    && e.provider.to_lowercase() == prov_lower
                    && e.voice.to_lowercase() == voice_lower
                    && e.cache_key == cache_key
            })
        };

        if let Some(existing) = found {
            existing.count += 1;
            existing.last_used = now;
            existing.provider_text = trimmed.to_string();
            existing.insert_text = insert_trimmed.to_string();
            if !cache_key.is_empty() {
                existing.provider = provider.to_string();
                existing.voice = voice.to_string();
                existing.cache_key = cache_key.to_string();
            }
        } else {
            phrases.push(PhraseEntry {
                id: uuid::Uuid::new_v4().to_string(),
                provider_text: trimmed.to_string(),
                insert_text: insert_trimmed.to_string(),
                count: 1,
                last_used: now,
                provider: provider.to_string(),
                voice: voice.to_string(),
                cache_key: cache_key.to_string(),
            });
        }

        while phrases.len() > PHRASE_HISTORY_SIZE {
            if let Some(oldest_pos) = phrases
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(i, _)| i)
            {
                phrases.remove(oldest_pos);
            } else {
                break;
            }
        }

        let my_version = self.phrase_version.fetch_add(1, Ordering::SeqCst) + 1;

        drop(phrases);

        if let Err(e) = persist_phrases_current(&self.phrase_path, &self.phrases) {
            // Roll back the in-memory change so a failed write never exposes
            // the unpersisted entry. Only restore when no concurrent writer has
            // built on top of our mutation (detected via the version counter);
            // otherwise the newer state is left intact and stays consistent.
            let mut phrases = self.phrases.write();
            if self.phrase_version.load(Ordering::SeqCst) == my_version {
                *phrases = snapshot;
            }
            return Err(e);
        }
        Ok(())
    }

    pub fn get_phrases(&self, filter: Option<&str>, limit: usize) -> Vec<PhraseEntry> {
        let phrases = self.phrases.read();
        let mut results: Vec<PhraseEntry> = if let Some(f) = filter {
            let f_lower = f.to_lowercase();
            phrases
                .iter()
                .filter(|e| e.provider_text.to_lowercase().contains(&f_lower))
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
        let snapshot = phrases.clone();
        phrases.retain(|e| e.id != id);

        let my_version = self.phrase_version.fetch_add(1, Ordering::SeqCst) + 1;
        drop(phrases);

        if let Err(e) = persist_phrases_current(&self.phrase_path, &self.phrases) {
            let mut phrases = self.phrases.write();
            if self.phrase_version.load(Ordering::SeqCst) == my_version {
                *phrases = snapshot;
            }
            return Err(e);
        }
        Ok(())
    }

    pub fn clear_phrases(&self) -> Result<()> {
        let mut phrases = self.phrases.write();
        let snapshot = phrases.clone();
        phrases.clear();

        let my_version = self.phrase_version.fetch_add(1, Ordering::SeqCst) + 1;
        drop(phrases);

        if let Err(e) = persist_phrases_current(&self.phrase_path, &self.phrases) {
            let mut phrases = self.phrases.write();
            if self.phrase_version.load(Ordering::SeqCst) == my_version {
                *phrases = snapshot;
            }
            return Err(e);
        }
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
    provider_text: &str,
    provider: &str,
    voice: &str,
    effects_fingerprint: u64,
) -> String {
    let combined = format!(
        "{}|{}|{}|{:x}",
        provider_text.trim().to_lowercase(),
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
                mgr_clone
                    .record_phrase(&format!("test phrase {}", i))
                    .unwrap();
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
        assert_eq!(entries[0].provider_text, "hello world");
        assert_eq!(entries[0].insert_text, "hello world");
        assert_eq!(entries[0].provider, "");
        assert_eq!(entries[0].voice, "");
        assert_eq!(entries[0].cache_key, "");
        assert_eq!(entries[1].id, "def-456");
    }

    #[test]
    fn legacy_piper_text_strips_combining_acute() {
        let old_json = r#"[
            {"id": "piper-1", "text": "краси\u0301вая де\u0301вушка", "provider": "piper",
             "voice": "irina", "cache_key": "key", "count": 1, "last_used": 1}
        ]"#;
        let entries: Vec<PhraseEntry> = serde_json::from_str(old_json).unwrap();
        assert_eq!(entries[0].provider_text, "краси\u{0301}вая де\u{0301}вушка");
        assert_eq!(entries[0].insert_text, "красивая девушка");
    }

    #[test]
    fn legacy_silero_text_strips_stress_plus() {
        let old_json = r#"[
            {"id": "silero-1", "text": "зам+ок и м+ука C++", "provider": "silero",
             "voice": "", "cache_key": "key", "count": 1, "last_used": 1}
        ]"#;
        let entries: Vec<PhraseEntry> = serde_json::from_str(old_json).unwrap();
        assert_eq!(entries[0].provider_text, "зам+ок и м+ука C++");
        assert_eq!(entries[0].insert_text, "замок и мука C++");
    }

    #[test]
    fn legacy_plain_provider_text_unchanged() {
        let old_json = r#"[
            {"id": "openai-1", "text": "hello world", "provider": "openai",
             "voice": "alloy", "cache_key": "key", "count": 1, "last_used": 1}
        ]"#;
        let entries: Vec<PhraseEntry> = serde_json::from_str(old_json).unwrap();
        assert_eq!(entries[0].provider_text, "hello world");
        assert_eq!(entries[0].insert_text, "hello world");
    }

    #[test]
    fn legacy_unknown_provider_keeps_literal_plus() {
        let old_json = r#"[
            {"id": "custom-1", "text": "зам+ок и C++", "provider": "custom",
             "voice": "", "cache_key": "key", "count": 1, "last_used": 1}
        ]"#;
        let entries: Vec<PhraseEntry> = serde_json::from_str(old_json).unwrap();
        assert_eq!(entries[0].provider_text, "зам+ок и C++");
        assert_eq!(entries[0].insert_text, "зам+ок и C++");
    }

    #[test]
    fn new_json_keeps_explicit_representations() {
        let new_json = r#"[
            {"id": "piper-2", "provider_text": "краси\u0301вая", "insert_text": "красивая",
             "provider": "piper", "voice": "irina", "cache_key": "key",
             "count": 2, "last_used": 2}
        ]"#;
        let entries: Vec<PhraseEntry> = serde_json::from_str(new_json).unwrap();
        assert_eq!(entries[0].provider_text, "краси\u{0301}вая");
        assert_eq!(entries[0].insert_text, "красивая");
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

        mgr.record_phrase_with_meta("hello world", "hello world", "openai", "alloy", "key-1")
            .unwrap();
        mgr.record_phrase_with_meta("hello world", "hello world", "silero", "voice-2", "key-2")
            .unwrap();
        mgr.record_phrase_with_meta("hello world", "hello world", "openai", "alloy", "key-1")
            .unwrap();

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

        mgr.record_phrase_with_meta("hello", "hello", "openai", "alloy", "k1")
            .unwrap();
        mgr.record_phrase_with_meta("hello", "hello", "openai", "alloy", "k1")
            .unwrap();
        mgr.record_phrase_with_meta("hello", "hello", "openai", "alloy", "k1")
            .unwrap();

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
    fn record_phrase_with_meta_stores_both_representations() {
        let (mgr, p1, p2, p3) = manager_in_tmp();

        mgr.record_phrase_with_meta(
            "зам+ок и м+ука",
            "замок и мука",
            "silero",
            "voice-1",
            "key-x",
        )
        .unwrap();

        let phrases = mgr.get_phrases(None, 100);
        assert_eq!(phrases.len(), 1);
        assert_eq!(phrases[0].provider_text, "зам+ок и м+ука");
        assert_eq!(phrases[0].insert_text, "замок и мука");
        assert_eq!(phrases[0].provider, "silero");

        // Persisted JSON carries both explicit fields (no legacy `text`).
        let content = fs::read_to_string(&p3).unwrap();
        assert!(content.contains("provider_text"));
        assert!(content.contains("insert_text"));
        assert!(!content.contains("\"text\""));

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
        mgr.record_phrase_with_meta("hello world", "hello world", "openai", "alloy", "cache-x")
            .unwrap();

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
        assert_eq!(phrases[0].provider_text, "kept phrase");

        let _ = fs::remove_file(&parent);
    }

    #[test]
    fn record_text_returns_err_and_keeps_memory_consistent_on_persistence_failure() {
        let (mgr, p1, _p2, _p3) = manager_in_tmp();

        mgr.record_text("kept token").unwrap();

        // Break persistence: replace the parent directory with a regular file so
        // the history temporary file can no longer be created.
        let parent = p1.parent().unwrap().to_path_buf();
        fs::remove_dir_all(&parent).unwrap();
        fs::write(&parent, "not a directory").unwrap();

        let result = mgr.record_text("another token");
        assert!(
            result.is_err(),
            "record_text must fail when persistence is broken"
        );

        // Publish-immediately semantics (documented in record_text): the write
        // failure returns Err but the in-memory state stays internally
        // consistent — queries keep working and retain the recorded tokens.
        let suggestions = mgr.suggest("another", 10);
        assert!(
            suggestions.iter().any(|e| e.word == "another"),
            "in-memory history must remain queryable after a failed write"
        );
        let suggestions = mgr.suggest("kept", 10);
        assert!(suggestions.iter().any(|e| e.word == "kept"));

        let _ = fs::remove_file(&parent);
    }
}
