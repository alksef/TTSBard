use anyhow::{Context, Result};
use chrono::Utc;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const PHRASE_HISTORY_SIZE: usize = 200;

static HISTORY_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn history_write_lock() -> &'static Mutex<()> {
    HISTORY_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

fn write_json_atomically(path: &Path, content: &str) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No parent dir"))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
        .as_nanos();
    let tmp_path = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|name| name.to_str()).unwrap_or("history.json"),
        stamp
    ));

    {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
    }

    if let Err(rename_error) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(path);
        fs::rename(&tmp_path, path).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Replace failed: {} (original: {})", e, rename_error),
            )
        })?;
    }

    Ok(())
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

fn save_history_sync(path: PathBuf, ngram_path: PathBuf, data: HistoryData, ngrams: NgramData) {
    let _lock = history_write_lock().lock();
    if let Ok(content) = serde_json::to_string_pretty(&data) {
        let _ = write_json_atomically(&path, &content);
    }
    if let Ok(content) = serde_json::to_string_pretty(&ngrams) {
        let _ = write_json_atomically(&ngram_path, &content);
    }
}

fn save_phrases_sync(path: PathBuf, phrases: Vec<PhraseEntry>) {
    let _lock = history_write_lock().lock();
    if let Ok(content) = serde_json::to_string_pretty(&phrases) {
        let _ = write_json_atomically(&path, &content);
    }
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

    pub fn record_text(&self, text: &str) {
        let tokens: Vec<String> = text
            .split_whitespace()
            .filter(|t| t.len() >= 2)
            .map(clean_token)
            .filter(|t| t.len() >= 2)
            .collect();

        if tokens.is_empty() {
            return;
        }

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

        let path = self.path.clone();
        let ngram_path = self.ngram_path.clone();
        let data_snapshot = data.clone();
        let ngrams_snapshot = ngrams.clone();
        drop(data);
        drop(ngrams);
        save_history_sync(path, ngram_path, data_snapshot, ngrams_snapshot);
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

        suggestions.sort_by(|a, b| b.count.cmp(&a.count));
        suggestions.truncate(limit);
        suggestions
    }

    pub fn clear(&self) {
        let mut data = self.data.write();
        let mut ngrams = self.ngrams.write();
        data.entries.clear();
        *ngrams = NgramData::default();

        let path = self.path.clone();
        let ngram_path = self.ngram_path.clone();
        let data_snapshot = data.clone();
        let ngrams_snapshot = ngrams.clone();
        drop(data);
        drop(ngrams);
        save_history_sync(path, ngram_path, data_snapshot, ngrams_snapshot);
    }

    // Контракт нормализации фраз: храним text.trim(); дедупликация и поиск —
    // case-insensitive по подстроке (to_lowercase()). См. также get_phrases.
    // Не менять без обновления обоих методов — это сломает дедупликацию/поиск.
    pub fn record_phrase(&self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let trimmed_lower = trimmed.to_lowercase();
        let now = Utc::now().timestamp();

        let mut phrases = self.phrases.write();

        if let Some(existing) = phrases
            .iter_mut()
            .find(|e| e.text.trim().to_lowercase() == trimmed_lower)
        {
            existing.count += 1;
            existing.last_used = now;
        } else {
            phrases.push(PhraseEntry {
                id: uuid::Uuid::new_v4().to_string(),
                text: trimmed.to_string(),
                count: 1,
                last_used: now,
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

        let path = self.phrase_path.clone();
        let snapshot = phrases.clone();
        drop(phrases);
        save_phrases_sync(path, snapshot);
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

        results.sort_by(|a, b| b.last_used.cmp(&a.last_used));
        results.truncate(limit);
        results
    }

    pub fn delete_phrase(&self, id: &str) {
        let mut phrases = self.phrases.write();
        phrases.retain(|e| e.id != id);

        let path = self.phrase_path.clone();
        let snapshot = phrases.clone();
        drop(phrases);
        save_phrases_sync(path, snapshot);
    }

    pub fn clear_phrases(&self) {
        let mut phrases = self.phrases.write();
        phrases.clear();

        let path = self.phrase_path.clone();
        let snapshot = phrases.clone();
        drop(phrases);
        save_phrases_sync(path, snapshot);
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
