use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::config::{config_write_lock, replace_file_atomically};

const MAX_TABS: usize = 50;
const MAX_TAB_TEXT_LEN: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EditorTab {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TabsData {
    #[serde(default)]
    pub active_id: String,
    #[serde(default)]
    pub tabs: Vec<EditorTab>,
}

pub struct TabManager {
    path: PathBuf,
    data: RwLock<TabsData>,
}

impl TabManager {
    pub fn new(path: PathBuf) -> Self {
        let _ = fs::create_dir_all(path.parent().unwrap_or(&path));
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str::<TabsData>(&c).ok())
            .unwrap_or_default();
        TabManager {
            path,
            data: RwLock::new(data),
        }
    }

    pub fn load_all(&self) -> TabsData {
        self.data.read().clone()
    }

    pub fn save_all(&self, mut data: TabsData) -> Result<()> {
        let _write_lock = config_write_lock().lock();
        if data.tabs.len() > MAX_TABS {
            data.tabs.truncate(MAX_TABS);
        }
        for t in &mut data.tabs {
            if t.text.len() > MAX_TAB_TEXT_LEN {
                t.text.truncate(MAX_TAB_TEXT_LEN);
            }
        }
        if !data.active_id.is_empty() && !data.tabs.iter().any(|t| t.id == data.active_id) {
            data.active_id = data.tabs.first().map(|t| t.id.clone()).unwrap_or_default();
        }

        let content = serde_json::to_string_pretty(&data).context("Failed to serialize tabs")?;
        replace_file_atomically(&self.path, content.as_bytes())
            .with_context(|| format!("Failed to persist tabs to {:?}", self.path))?;

        *self.data.write() = data;
        Ok(())
    }
}

pub fn tabs_path() -> std::io::Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "config dir"))?
        .join("ttsbard");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("tabs.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a TabManager backed by a unique temp file path.
    /// Uses an atomic counter — tests run in parallel within one process, so a
    /// per-process path would race (multiple tests writing the same file).
    fn manager_in_tmp() -> (TabManager, PathBuf) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("ttsbard-tabs-test-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tabs_test.json");
        let _ = fs::remove_file(&path);
        (TabManager::new(path.clone()), path)
    }

    #[test]
    fn new_manager_loads_empty_when_no_file() {
        let (mgr, _path) = manager_in_tmp();
        let data = mgr.load_all();
        assert!(data.tabs.is_empty());
        assert_eq!(data.active_id, "");
    }

    #[test]
    fn save_then_load_round_trip() {
        let (mgr, path) = manager_in_tmp();
        let data = TabsData {
            active_id: "id-2".into(),
            tabs: vec![
                EditorTab {
                    id: "id-1".into(),
                    title: "Текст 1".into(),
                    text: "привет".into(),
                },
                EditorTab {
                    id: "id-2".into(),
                    title: "Текст 2".into(),
                    text: "мир".into(),
                },
            ],
        };
        mgr.save_all(data).unwrap();

        // A fresh manager reading the same file must hydrate the saved data.
        let mgr2 = TabManager::new(path.clone());
        let loaded = mgr2.load_all();
        assert_eq!(loaded.tabs.len(), 2);
        assert_eq!(loaded.active_id, "id-2");
        assert_eq!(loaded.tabs[1].text, "мир");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_all_truncates_over_max_tabs() {
        let (mgr, path) = manager_in_tmp();
        let tabs: Vec<EditorTab> = (0..(MAX_TABS + 5))
            .map(|i| EditorTab {
                id: format!("id-{i}"),
                title: format!("T{i}"),
                text: String::new(),
            })
            .collect();
        mgr.save_all(TabsData {
            active_id: "id-0".into(),
            tabs,
        })
        .unwrap();
        let loaded = mgr.load_all();
        assert_eq!(loaded.tabs.len(), MAX_TABS);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_all_truncates_oversized_text() {
        let (mgr, path) = manager_in_tmp();
        let huge = "x".repeat(MAX_TAB_TEXT_LEN + 1000);
        mgr.save_all(TabsData {
            active_id: "id-1".into(),
            tabs: vec![EditorTab {
                id: "id-1".into(),
                title: "T".into(),
                text: huge,
            }],
        })
        .unwrap();
        let loaded = mgr.load_all();
        assert_eq!(loaded.tabs[0].text.len(), MAX_TAB_TEXT_LEN);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_all_resets_invalid_active_id_to_first() {
        let (mgr, path) = manager_in_tmp();
        mgr.save_all(TabsData {
            active_id: "does-not-exist".into(),
            tabs: vec![
                EditorTab {
                    id: "a".into(),
                    title: "A".into(),
                    text: String::new(),
                },
                EditorTab {
                    id: "b".into(),
                    title: "B".into(),
                    text: String::new(),
                },
            ],
        })
        .unwrap();
        let loaded = mgr.load_all();
        assert_eq!(
            loaded.active_id, "a",
            "invalid active_id must fall back to first tab"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_all_concurrently_maintains_consistency() {
        let (mgr, path) = manager_in_tmp();
        let mgr_arc = std::sync::Arc::new(mgr);
        let mut threads = vec![];

        for i in 0..10 {
            let mgr_clone = std::sync::Arc::clone(&mgr_arc);
            threads.push(std::thread::spawn(move || {
                let data = TabsData {
                    active_id: format!("id-{}", i),
                    tabs: vec![EditorTab {
                        id: format!("id-{}", i),
                        title: format!("Tab {}", i),
                        text: format!("Text content {}", i),
                    }],
                };
                mgr_clone.save_all(data).unwrap();
            }));
        }

        for t in threads {
            t.join().unwrap();
        }

        // Verify the file exists and is valid JSON
        let content = fs::read_to_string(&path).unwrap();
        let loaded: TabsData = serde_json::from_str(&content).unwrap();

        assert!(loaded.active_id.starts_with("id-"));
        assert_eq!(loaded.tabs.len(), 1);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_all_keeps_published_state_on_persistence_failure() {
        let (mgr, path) = manager_in_tmp();

        let first = TabsData {
            active_id: "a".into(),
            tabs: vec![EditorTab {
                id: "a".into(),
                title: "A".into(),
                text: "first".into(),
            }],
        };
        mgr.save_all(first).unwrap();

        // Break persistence: replace the parent directory with a regular file so
        // the temporary file can no longer be created.
        let parent = path.parent().unwrap().to_path_buf();
        fs::remove_dir_all(&parent).unwrap();
        fs::write(&parent, "not a directory").unwrap();

        let second = TabsData {
            active_id: "b".into(),
            tabs: vec![EditorTab {
                id: "b".into(),
                title: "B".into(),
                text: "second".into(),
            }],
        };
        let result = mgr.save_all(second);
        assert!(
            result.is_err(),
            "save_all must fail when persistence is broken"
        );

        let published = mgr.load_all();
        assert_eq!(published.active_id, "a");
        assert_eq!(published.tabs.len(), 1);
        assert_eq!(published.tabs[0].id, "a");
        assert_eq!(published.tabs[0].text, "first");

        let _ = fs::remove_file(&parent);
    }
}
