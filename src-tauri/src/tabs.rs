use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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

fn spawn_save(path: PathBuf, data: TabsData) {
    std::thread::spawn(move || {
        if let Ok(content) = serde_json::to_string_pretty(&data) {
            let _ = fs::write(&path, content);
        }
    });
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

    pub fn save_all(&self, mut data: TabsData) {
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
        let mut guard = self.data.write();
        *guard = data.clone();
        drop(guard);
        spawn_save(self.path.clone(), data);
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
        let dir = std::env::temp_dir()
            .join(format!("ttsbard-tabs-test-{}-{}", std::process::id(), n));
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
                EditorTab { id: "id-1".into(), title: "Текст 1".into(), text: "привет".into() },
                EditorTab { id: "id-2".into(), title: "Текст 2".into(), text: "мир".into() },
            ],
        };
        mgr.save_all(data);
        // save_all spawns a writer thread; wait for it to flush before re-reading.
        std::thread::sleep(std::time::Duration::from_millis(150));

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
        mgr.save_all(TabsData { active_id: "id-0".into(), tabs });
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
            tabs: vec![EditorTab { id: "id-1".into(), title: "T".into(), text: huge }],
        });
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
                EditorTab { id: "a".into(), title: "A".into(), text: String::new() },
                EditorTab { id: "b".into(), title: "B".into(), text: String::new() },
            ],
        });
        let loaded = mgr.load_all();
        assert_eq!(loaded.active_id, "a", "invalid active_id must fall back to first tab");
        let _ = fs::remove_file(&path);
    }
}
