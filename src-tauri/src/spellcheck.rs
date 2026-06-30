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

    pub fn check_words(&self, words: &[String]) -> Vec<SpellResult> {
        // Phase 1: collect cached results and identify words needing lookup
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

        // Phase 2: check missing words under dict read-lock
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
                for &idx in &to_check {
                    let w = &words[idx];
                    new_results.push((
                        idx,
                        SpellResult {
                            word: w.clone(),
                            correct: true,
                            suggestions: vec![],
                        },
                    ));
                }
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
