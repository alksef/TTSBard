//! Sound Panel State
//!
//! Управление состоянием звуковой панели: привязки клавиш, флаг перехвата.

use crate::config::{
    DEFAULT_FLOATING_BG_COLOR, DEFAULT_FLOATING_OPACITY, MAX_FLOATING_OPACITY, MIN_FLOATING_OPACITY,
};
use crate::events::AppEvent;
use crate::soundpanel::intercept::InterceptSettings;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Привязка звука к клавише
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundBinding {
    /// Клавиша (A-Z)
    pub key: char,
    /// Описание звука
    pub description: String,
    /// Имя файла в папке soundpanel
    pub filename: String,
    /// Оригинальный путь к файлу (для информации)
    pub original_path: Option<String>,
}

/// Набор звуков (Set) — группа привязок A-Z
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoundSet {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub bindings: Vec<SoundBinding>,
}

/// Контейнер всех наборов + ID активного набора
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoundSets {
    #[serde(default)]
    pub active_set_id: String,
    #[serde(default)]
    pub sets: Vec<SoundSet>,
}

impl SoundSets {
    /// Найти активный набор по active_set_id, с fallback на первый
    pub fn find_active(&self) -> Option<&SoundSet> {
        if !self.active_set_id.is_empty() {
            if let Some(set) = self.sets.iter().find(|s| s.id == self.active_set_id) {
                return Some(set);
            }
        }
        self.sets.first()
    }

    /// Найти индекс активного набора, с fallback на 0
    pub fn find_active_index(&self) -> usize {
        if let Some(idx) = self.sets.iter().position(|s| s.id == self.active_set_id) {
            idx
        } else {
            0
        }
    }
}

/// Состояние звуковой панели
#[derive(Clone)]
pub struct SoundPanelState {
    /// Включен ли режим перехвата для звуковой панели
    pub interception_enabled: Arc<Mutex<bool>>,

    /// Наборы звуков (Set), каждый со своими привязками
    pub sets: Arc<Mutex<SoundSets>>,

    /// Отправитель событий для MPSC канала
    pub event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,

    /// Путь к папке %APPDATA%
    pub appdata_path: Arc<Mutex<String>>,

    /// Прозрачность floating окна (10-100)
    pub floating_opacity: Arc<Mutex<u8>>,

    /// Цвет фона floating окна (hex #RRGGBB)
    pub floating_bg_color: Arc<Mutex<String>>,

    /// Пропускает ли floating окно клики
    pub floating_clickthrough: Arc<Mutex<bool>>,

    /// Intercept-настройки (NumPad/F-keys → actions, persisted)
    pub intercept: Arc<Mutex<InterceptSettings>>,

    /// Активные воспроизведения звука (thread handles)
    active_playbacks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

fn gen_set_id() -> String {
    Uuid::new_v4().to_string()
}

impl SoundPanelState {
    /// Создать новое состояние звуковой панели
    pub fn new(appdata_path: String) -> Self {
        let intercept = crate::soundpanel::intercept::load(&appdata_path);
        Self {
            interception_enabled: Arc::new(Mutex::new(false)),
            sets: Arc::new(Mutex::new(SoundSets::default())),
            event_sender: Arc::new(Mutex::new(None)),
            appdata_path: Arc::new(Mutex::new(appdata_path)),
            floating_opacity: Arc::new(Mutex::new(DEFAULT_FLOATING_OPACITY)),
            floating_bg_color: Arc::new(Mutex::new(DEFAULT_FLOATING_BG_COLOR.to_string())),
            floating_clickthrough: Arc::new(Mutex::new(false)),
            intercept: Arc::new(Mutex::new(intercept)),
            active_playbacks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Проверить, включен ли режим перехвата
    #[allow(dead_code)]
    pub fn is_interception_enabled(&self) -> bool {
        self.interception_enabled
            .lock()
            .map(|v| *v)
            .unwrap_or(false)
    }

    /// Установить режим перехвата
    pub fn set_interception_enabled(&self, enabled: bool) {
        if let Ok(mut val) = self.interception_enabled.lock() {
            *val = enabled;
        } else {
            error!(
                target = "soundpanel::state",
                "Failed to lock interception_enabled"
            );
        }
    }

    /// Получить привязку по клавише из активного набора
    pub fn get_binding(&self, key: char) -> Option<SoundBinding> {
        self.sets.lock().ok().and_then(|sets| {
            sets.find_active()
                .and_then(|active| active.bindings.iter().find(|b| b.key == key).cloned())
        })
    }

    /// Добавить привязку в активный набор
    pub fn add_binding(&self, binding: SoundBinding) {
        if let Ok(mut sets) = self.sets.lock() {
            let idx = sets.find_active_index();
            if let Some(active) = sets.sets.get_mut(idx) {
                active.bindings.retain(|b| b.key != binding.key);
                active.bindings.push(binding);
            }
        } else {
            error!(target = "soundpanel::state", "Failed to lock sets");
        }
    }

    /// Удалить привязку из активного набора
    pub fn remove_binding(&self, key: char) {
        if let Ok(mut sets) = self.sets.lock() {
            let idx = sets.find_active_index();
            if let Some(active) = sets.sets.get_mut(idx) {
                active.bindings.retain(|b| b.key != key);
            }
        } else {
            error!(target = "soundpanel::state", "Failed to lock sets");
        }
    }

    /// Получить все привязки активного набора (отсортированные)
    pub fn get_all_bindings(&self) -> Vec<SoundBinding> {
        self.sets
            .lock()
            .ok()
            .and_then(|sets| {
                sets.find_active().map(|active| {
                    let mut bindings: Vec<_> = active.bindings.clone();
                    bindings.sort_by(|a, b| a.key.cmp(&b.key));
                    bindings
                })
            })
            .unwrap_or_default()
    }

    /// Получить все наборы (клон)
    pub fn get_sets(&self) -> SoundSets {
        self.sets.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Получить активный набор (клон, с fallback на пустой)
    pub fn get_active_set(&self) -> SoundSet {
        self.sets
            .lock()
            .ok()
            .and_then(|sets| sets.find_active().cloned())
            .unwrap_or_default()
    }

    /// Сменить активный набор по ID
    pub fn set_active_set(&self, id: &str) {
        if let Ok(mut sets) = self.sets.lock() {
            if sets.sets.iter().any(|s| s.id == id) {
                sets.active_set_id = id.to_string();
            }
        } else {
            error!(target = "soundpanel::state", "Failed to lock sets");
        }
    }

    /// Создать новый набор и сделать его активным
    pub fn add_set(&self, name: &str) -> Result<SoundSet, String> {
        let mut sets = self.sets.lock().map_err(|e| format!("Lock error: {}", e))?;
        let id = gen_set_id();
        let set = SoundSet {
            id,
            name: name.to_string(),
            bindings: Vec::new(),
        };
        let result = set.clone();
        sets.sets.push(set);
        sets.active_set_id = result.id.clone();
        Ok(result)
    }

    /// Переименовать набор
    pub fn rename_set(&self, id: &str, name: &str) -> Result<(), String> {
        let mut sets = self.sets.lock().map_err(|e| format!("Lock error: {}", e))?;
        if let Some(set) = sets.sets.iter_mut().find(|s| s.id == id) {
            set.name = name.to_string();
        }
        Ok(())
    }

    /// Удалить набор. Если удалён активный — переключить на соседний/первый.
    pub fn remove_set(&self, id: &str) -> Result<(), String> {
        let mut sets = self.sets.lock().map_err(|e| format!("Lock error: {}", e))?;

        let target_idx = sets.sets.iter().position(|s| s.id == id);
        if let Some(idx) = target_idx {
            sets.sets.remove(idx);

            if sets.active_set_id == id {
                let new_active = if idx < sets.sets.len() {
                    sets.sets[idx].id.clone()
                } else if idx > 0 && !sets.sets.is_empty() {
                    let new_idx = idx.saturating_sub(1).min(sets.sets.len() - 1);
                    sets.sets[new_idx].id.clone()
                } else {
                    String::new()
                };
                sets.active_set_id = new_active;
            }
        }
        Ok(())
    }

    /// Целиком заменить наборы (для загрузки из хранилища)
    pub fn replace_sets(&self, new_sets: SoundSets) {
        if let Ok(mut sets) = self.sets.lock() {
            *sets = new_sets;
        } else {
            error!(target = "soundpanel::state", "Failed to lock sets");
        }
    }

    /// Воспроизвести звук по привязке
    pub fn play_sound(&self, binding: &SoundBinding) {
        let appdata_path = self.appdata_path.lock().unwrap().clone();
        let sound_path = format!("{}\\soundpanel\\{}", appdata_path, binding.filename);

        info!(target = "soundpanel", key = %binding.key, path = ?sound_path, "Playing sound");

        let handle = std::thread::spawn(move || {
            super::audio::play_audio_file(&sound_path);
        });

        if let Ok(mut playbacks) = self.active_playbacks.lock() {
            playbacks.push(handle);
            playbacks.retain(|h| !h.is_finished());
        }
    }

    /// Установить отправитель событий
    pub fn set_event_sender(&self, sender: Sender<AppEvent>) {
        if let Ok(mut es) = self.event_sender.lock() {
            *es = Some(sender);
        } else {
            error!(target = "soundpanel::state", "Failed to lock event_sender");
        }
    }

    /// Отправить событие
    pub fn emit_event(&self, event: AppEvent) {
        debug!(target = "soundpanel::state", event = ?std::mem::discriminant(&event), "emit_event called");
        if let Ok(es) = self.event_sender.lock() {
            if let Some(ref sender) = *es {
                debug!(
                    target = "soundpanel::state",
                    "Sending event through channel"
                );
                match sender.send(event) {
                    Ok(_) => debug!(target = "soundpanel::state", "Event sent successfully"),
                    Err(error) => {
                        error!(target = "soundpanel::state", error = %error, "Failed to send event")
                    }
                }
            } else {
                error!(target = "soundpanel::state", "event_sender is None");
            }
        } else {
            error!(target = "soundpanel::state", "Failed to lock event_sender");
        }
    }

    /// Получить прозрачность floating окна
    pub fn get_floating_opacity(&self) -> u8 {
        self.floating_opacity
            .lock()
            .map(|v| *v)
            .unwrap_or(DEFAULT_FLOATING_OPACITY)
    }

    /// Установить прозрачность floating окна
    pub fn set_floating_opacity(&self, value: u8) {
        debug!(
            target = "soundpanel::state",
            value, "set_floating_opacity called"
        );
        if let Ok(mut val) = self.floating_opacity.lock() {
            *val = value.clamp(MIN_FLOATING_OPACITY, MAX_FLOATING_OPACITY);
            debug!(
                target = "soundpanel::state",
                opacity = *val,
                "Opacity updated"
            );
        } else {
            error!(
                target = "soundpanel::state",
                "Failed to lock floating_opacity"
            );
            return;
        }
        debug!(
            target = "soundpanel::state",
            "Emitting SoundPanelAppearanceChanged event"
        );
        self.emit_event(AppEvent::SoundPanelAppearanceChanged);
        debug!(target = "soundpanel::state", "Event emitted");
    }

    /// Получить цвет фона floating окна
    pub fn get_floating_bg_color(&self) -> String {
        self.floating_bg_color.lock().unwrap().clone()
    }

    /// Установить цвет фона floating окна
    pub fn set_floating_bg_color(&self, color: String) {
        debug!(
            target = "soundpanel::state",
            color, "set_floating_bg_color called"
        );
        let color_clone = color.clone();
        if let Ok(mut val) = self.floating_bg_color.lock() {
            *val = color_clone.clone();
            debug!(target = "soundpanel::state", color = ?color_clone, "Color updated");
        } else {
            error!(
                target = "soundpanel::state",
                "Failed to lock floating_bg_color"
            );
            return;
        }
        debug!(
            target = "soundpanel::state",
            "Emitting SoundPanelAppearanceChanged event"
        );
        self.emit_event(AppEvent::SoundPanelAppearanceChanged);
        debug!(target = "soundpanel::state", "Event emitted");
    }

    /// Проверить, включен ли clickthrough для floating окна
    pub fn is_floating_clickthrough_enabled(&self) -> bool {
        self.floating_clickthrough
            .lock()
            .map(|v| *v)
            .unwrap_or(false)
    }

    /// Установить clickthrough для floating окна
    pub fn set_floating_clickthrough(&self, enabled: bool) {
        debug!(
            target = "soundpanel::state",
            enabled, "set_floating_clickthrough called"
        );
        if let Ok(mut val) = self.floating_clickthrough.lock() {
            *val = enabled;
            debug!(
                target = "soundpanel::state",
                enabled, "Clickthrough updated"
            );
        } else {
            error!(
                target = "soundpanel::state",
                "Failed to lock floating_clickthrough"
            );
        }
    }

    /// Получить настройки перехвата (clone)
    pub fn get_intercept(&self) -> InterceptSettings {
        self.intercept.lock().map(|v| v.clone()).unwrap_or_default()
    }

    /// Включить/выключить перехват (persist + emit)
    pub fn set_intercept_enabled(&self, enabled: bool) {
        let appdata_path = self.appdata_path.lock().unwrap().clone();
        if let Ok(mut val) = self.intercept.lock() {
            val.enabled = enabled;
            let settings = val.clone();
            drop(val);
            let _ = crate::soundpanel::intercept::save(&appdata_path, &settings);
            self.emit_event(AppEvent::InterceptionChanged(enabled));
        } else {
            error!(target = "soundpanel::state", "Failed to lock intercept");
        }
    }

    /// Установить биндинг перехвата (persist)
    pub fn set_intercept_binding(&self, key: String, action: String) {
        let appdata_path = self.appdata_path.lock().unwrap().clone();
        if let Ok(mut val) = self.intercept.lock() {
            val.bindings.retain(|b| b.key != key);
            val.bindings
                .push(crate::soundpanel::intercept::InterceptBinding {
                    key: key.clone(),
                    action: action.clone(),
                });
            let settings = val.clone();
            drop(val);
            let _ = crate::soundpanel::intercept::save(&appdata_path, &settings);
            info!(key = key, action = action, "Intercept binding set");
        } else {
            error!(target = "soundpanel::state", "Failed to lock intercept");
        }
    }

    /// Очистить биндинг перехвата (persist)
    pub fn clear_intercept_binding(&self, key: String) {
        let appdata_path = self.appdata_path.lock().unwrap().clone();
        if let Ok(mut val) = self.intercept.lock() {
            val.bindings.retain(|b| b.key != key);
            let settings = val.clone();
            drop(val);
            let _ = crate::soundpanel::intercept::save(&appdata_path, &settings);
            info!(key = key, "Intercept binding cleared");
        } else {
            error!(target = "soundpanel::state", "Failed to lock intercept");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_active_fallback() {
        let sets = SoundSets {
            active_set_id: "bogus".into(),
            sets: vec![SoundSet {
                id: "set1".into(),
                name: "First".into(),
                bindings: vec![],
            }],
        };
        let active = sets.find_active();
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, "set1");
    }

    #[test]
    fn test_find_active_empty() {
        let sets = SoundSets::default();
        assert!(sets.find_active().is_none());
    }

    #[test]
    fn test_find_active_index_not_first() {
        let sets = SoundSets {
            active_set_id: "set2".into(),
            sets: vec![
                SoundSet {
                    id: "set1".into(),
                    name: "First".into(),
                    bindings: vec![],
                },
                SoundSet {
                    id: "set2".into(),
                    name: "Second".into(),
                    bindings: vec![],
                },
            ],
        };
        assert_eq!(sets.find_active_index(), 1);
    }

    #[test]
    fn test_find_active_index_invalid_falls_back_to_zero() {
        let sets = SoundSets {
            active_set_id: "bogus".into(),
            sets: vec![SoundSet {
                id: "set1".into(),
                name: "First".into(),
                bindings: vec![],
            }],
        };
        assert_eq!(sets.find_active_index(), 0);
    }

    #[test]
    fn test_find_active_index_empty_active_set_id_returns_zero() {
        let sets = SoundSets {
            active_set_id: String::new(),
            sets: vec![
                SoundSet {
                    id: "set1".into(),
                    name: "First".into(),
                    bindings: vec![],
                },
                SoundSet {
                    id: "set2".into(),
                    name: "Second".into(),
                    bindings: vec![],
                },
            ],
        };
        assert_eq!(sets.find_active_index(), 0);
    }

    #[test]
    fn test_migration_vec_to_sets() {
        let old_json = r#"[
            {"key":"A","description":"test a","filename":"a.mp3","original_path":null},
            {"key":"B","description":"test b","filename":"b.mp3","original_path":"D:\\b.mp3"}
        ]"#;

        let old_bindings: Vec<SoundBinding> = serde_json::from_str(old_json).unwrap();
        assert_eq!(old_bindings.len(), 2);

        let id = gen_set_id();
        let sets = SoundSets {
            active_set_id: id.clone(),
            sets: vec![SoundSet {
                id,
                name: "Основной".into(),
                bindings: old_bindings,
            }],
        };

        let active = sets.find_active().unwrap();
        assert_eq!(active.name, "Основной");
        assert_eq!(active.bindings.len(), 2);
        assert_eq!(active.bindings[0].key, 'A');
    }
}
