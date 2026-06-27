use anyhow::{Context, Result};
use chrono::Utc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub word: String,
    pub count: u32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    data: RwLock<HistoryData>,
    ngrams: RwLock<NgramData>,
}

fn clean_token(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

fn spawn_save(path: PathBuf, ngram_path: PathBuf, data: HistoryData, ngrams: NgramData) {
    std::thread::spawn(move || {
        if let Ok(content) = serde_json::to_string_pretty(&data) {
            let _ = fs::write(&path, content);
        }
        if let Ok(content) = serde_json::to_string_pretty(&ngrams) {
            let _ = fs::write(&ngram_path, content);
        }
    });
}

impl HistoryManager {
    pub fn new(path: PathBuf, ngram_path: PathBuf) -> Self {
        let _ = fs::create_dir_all(path.parent().unwrap_or(&path));

        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str::<HistoryData>(&c).ok())
            .unwrap_or_default();

        let ngrams = fs::read_to_string(&ngram_path)
            .ok()
            .and_then(|c| serde_json::from_str::<NgramData>(&c).ok())
            .unwrap_or_default();

        HistoryManager {
            path,
            ngram_path,
            data: RwLock::new(data),
            ngrams: RwLock::new(ngrams),
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
        spawn_save(path, ngram_path, data_snapshot, ngrams_snapshot);
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
        spawn_save(path, ngram_path, data_snapshot, ngrams_snapshot);
    }
}

impl Default for HistoryData {
    fn default() -> Self {
        HistoryData {
            entries: Vec::new(),
        }
    }
}

pub fn history_paths() -> Result<(PathBuf, PathBuf)> {
    let dir = dirs::config_dir()
        .context("Failed to get config dir")?
        .join("ttsbard");
    fs::create_dir_all(&dir).context("Failed to create ttsbard dir")?;
    Ok((dir.join("input_history.json"), dir.join("ngrams.json")))
}
