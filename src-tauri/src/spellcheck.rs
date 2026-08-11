use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SpellResult {
    pub word: String,
    pub correct: bool,
    pub suggestions: Vec<String>,
}

pub struct SpellcheckManager {
    dict: RwLock<Option<spellbook::Dictionary>>,
    cache: RwLock<HashMap<String, SpellResult>>,
}

impl SpellcheckManager {
    pub fn new(aff_path: PathBuf, dic_path: PathBuf) -> Self {
        let dict = (|| -> Result<spellbook::Dictionary> {
            let aff = std::fs::read_to_string(&aff_path).context("read ru.aff")?;
            let dic = std::fs::read_to_string(&dic_path).context("read ru.dic")?;
            spellbook::Dictionary::new(&aff, &dic)
                .map_err(|e| anyhow::anyhow!("parse hunspell dict: {e:?}"))
        })();
        if let Err(e) = &dict {
            eprintln!("[spellcheck] dictionary load failed: {e:?} (spellcheck disabled)");
        }
        Self {
            dict: RwLock::new(dict.ok()),
            cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn is_available(&self) -> bool {
        self.dict.read().is_some()
    }

    pub fn check_words(&self, words: &[String]) -> Vec<SpellResult> {
        let mut results = Vec::with_capacity(words.len());
        let mut to_check: Vec<usize> = Vec::new();

        {
            let cache = self.cache.read();
            for (i, w) in words.iter().enumerate() {
                if let Some(r) = cache.get(w) {
                    results.push(r.clone());
                } else {
                    to_check.push(i);
                }
            }
        }

        if to_check.is_empty() {
            return results;
        }

        let mut new_results: Vec<(usize, SpellResult)> = Vec::with_capacity(to_check.len());
        {
            let dict_guard = self.dict.read();
            if let Some(dict) = dict_guard.as_ref() {
                for &idx in &to_check {
                    let w = &words[idx];
                    let correct = dict.check(w);
                    let mut suggestions = Vec::new();
                    if !correct {
                        dict.suggest(w, &mut suggestions);
                    }
                    new_results.push((
                        idx,
                        SpellResult {
                            word: w.clone(),
                            correct,
                            suggestions,
                        },
                    ));
                }
            } else {
                return Vec::new();
            }
        }

        // Phase 3: write new results to cache (dict read-lock released)
        {
            let mut cache = self.cache.write();
            for (idx, r) in &new_results {
                let w = &words[*idx];
                cache.entry(w.clone()).or_insert_with(|| r.clone());
            }
        }

        // Phase 4: merge cached (Phase 1) + new (Phase 2) in original word order
        let mut final_results: Vec<SpellResult> = Vec::with_capacity(words.len());
        let mut ni = 0;
        let mut ri = 0;
        for i in 0..words.len() {
            if ni < to_check.len() && to_check[ni] == i {
                final_results.push(new_results[ni].1.clone());
                ni += 1;
            } else {
                final_results.push(results[ri].clone());
                ri += 1;
            }
        }

        final_results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict_paths() -> (PathBuf, PathBuf) {
        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        (base.join("resources/dict/ru.aff"), base.join("resources/dict/ru.dic"))
    }

    fn nonexistent_paths() -> (PathBuf, PathBuf) {
        (PathBuf::from("/nonexistent/ru.aff"), PathBuf::from("/nonexistent/ru.dic"))
    }

    #[test]
    fn is_available_false_when_dict_none() {
        let (aff, dic) = nonexistent_paths();
        let mgr = SpellcheckManager::new(aff, dic);
        assert!(!mgr.is_available());
    }

    #[test]
    fn is_available_true_when_dict_loaded() {
        let (aff, dic) = dict_paths();
        let mgr = SpellcheckManager::new(aff, dic);
        assert!(mgr.is_available());
    }

    #[test]
    fn check_words_returns_empty_when_dict_unavailable() {
        let (aff, dic) = nonexistent_paths();
        let mgr = SpellcheckManager::new(aff, dic);
        let results = mgr.check_words(&["тест".into(), "слово".into()]);
        assert!(results.is_empty());
    }

    #[test]
    fn check_words_no_false_correct_when_dict_unavailable() {
        let (aff, dic) = nonexistent_paths();
        let mgr = SpellcheckManager::new(aff, dic);
        let words: Vec<String> = vec!["любой".into(), "текст".into()];
        let results = mgr.check_words(&words);
        assert!(results.is_empty(), "must not return correct=true for all words");
    }

    #[test]
    fn check_words_with_real_dict_marks_misspelling() {
        let (aff, dic) = dict_paths();
        let mgr = SpellcheckManager::new(aff, dic);
        assert!(mgr.is_available());
        let words: Vec<String> = vec!["здрвствуйте".into()];
        let results = mgr.check_words(&words);
        assert_eq!(results.len(), 1);
        assert!(!results[0].correct);
    }

    #[test]
    fn check_words_with_real_dict_accepts_correct_word() {
        let (aff, dic) = dict_paths();
        let mgr = SpellcheckManager::new(aff, dic);
        assert!(mgr.is_available());
        let words: Vec<String> = vec!["здравствуйте".into()];
        let results = mgr.check_words(&words);
        assert_eq!(results.len(), 1);
        assert!(results[0].correct);
    }

    #[test]
    fn check_words_caches_results() {
        let (aff, dic) = dict_paths();
        let mgr = SpellcheckManager::new(aff, dic);
        assert!(mgr.is_available());
        let words: Vec<String> = vec!["здравствуйте".into()];
        let _ = mgr.check_words(&words);
        let second = mgr.check_words(&words);
        assert_eq!(second.len(), 1);
        assert!(second[0].correct);
    }

    #[test]
    fn check_words_no_cache_when_dict_unavailable() {
        let (aff, dic) = nonexistent_paths();
        let mgr = SpellcheckManager::new(aff, dic);
        assert!(!mgr.is_available());
        let words: Vec<String> = vec!["any".into(), "text".into()];
        let _ = mgr.check_words(&words);
        let second = mgr.check_words(&words);
        assert!(second.is_empty());
    }
}
