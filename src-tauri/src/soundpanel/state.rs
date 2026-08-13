//! Sound Panel State
//!
//! Управление состоянием звуковой панели: привязки клавиш, флаг перехвата.

use crate::config::{
    AudioSettings, DEFAULT_FLOATING_BG_COLOR, DEFAULT_FLOATING_OPACITY, MAX_FLOATING_OPACITY,
    MIN_FLOATING_OPACITY,
};
use crate::events::AppEvent;
use crate::soundpanel::intercept::InterceptSettings;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
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
        self.sets
            .iter()
            .position(|s| s.id == self.active_set_id)
            .unwrap_or_default()
    }

    /// Добавить/заменить привязку в активном наборе (мутация клона)
    pub fn add_active_binding(&mut self, binding: SoundBinding) {
        let idx = self.find_active_index();
        if let Some(active) = self.sets.get_mut(idx) {
            active.bindings.retain(|b| b.key != binding.key);
            active.bindings.push(binding);
        }
    }

    /// Удалить привязку из активного набора (мутация клона)
    pub fn remove_active_binding(&mut self, key: char) {
        let idx = self.find_active_index();
        if let Some(active) = self.sets.get_mut(idx) {
            active.bindings.retain(|b| b.key != key);
        }
    }

    /// Ссылается ли хотя бы один набор на указанный аудиофайл
    pub fn references_filename(&self, filename: &str) -> bool {
        self.sets
            .iter()
            .any(|s| s.bindings.iter().any(|b| b.filename == filename))
    }
}

/// Максимальное число элементов в очереди воспроизведения SoundPanel.
const SOUND_QUEUE_CAPACITY: usize = 16;

/// Интервал ожидания worker-а между проверками `CancellationToken`.
const SOUND_QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Элемент очереди SoundPanel: путь к аудиофайлу и неизменяемый снимок настроек.
#[derive(Debug, Clone)]
struct QueueItem {
    path: String,
    audio_settings: AudioSettings,
}

/// Bounded FIFO-очередь воспроизведения SoundPanel (sender-сторона).
///
/// Очередь runtime-only: без persistence, UI списка и ручного управления.
/// Переполнение или отсутствие worker-а отклоняют новый элемент с ошибкой.
#[derive(Clone)]
struct SoundQueue {
    tx: SyncSender<QueueItem>,
}

impl SoundQueue {
    fn new() -> (Self, Receiver<QueueItem>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(SOUND_QUEUE_CAPACITY);
        (Self { tx }, rx)
    }

    fn try_enqueue(&self, item: QueueItem) -> Result<(), String> {
        match self.tx.try_send(item) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err("SoundPanel queue is full".to_string()),
            Err(TrySendError::Disconnected(_)) => {
                Err("SoundPanel worker is not running".to_string())
            }
        }
    }
}

/// Worker очереди SoundPanel: FIFO-воспроизведение через `play_audio_file`.
///
/// Синхронно воспроизводит один элемент и только затем берёт следующий.
/// Завершается при отмене `shutdown`, не начиная новый элемент после отмены.
fn run_queue_worker(receiver: Receiver<QueueItem>, shutdown: CancellationToken) {
    run_queue_worker_with(receiver, shutdown, |item| {
        super::audio::play_audio_file(&item.path, &item.audio_settings);
    });
}

/// Testable-ядро worker-а: FIFO-потребление с injectable-функцией воспроизведения.
fn run_queue_worker_with<F>(receiver: Receiver<QueueItem>, shutdown: CancellationToken, mut play: F)
where
    F: FnMut(QueueItem),
{
    loop {
        if shutdown.is_cancelled() {
            info!(target = "soundpanel::queue", "Queue worker exiting on shutdown");
            return;
        }

        match receiver.recv_timeout(SOUND_QUEUE_POLL_INTERVAL) {
            Ok(item) => {
                if shutdown.is_cancelled() {
                    info!(target = "soundpanel::queue", "Queue worker exiting before next item");
                    return;
                }
                play(item);
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                info!(target = "soundpanel::queue", "Queue channel disconnected; worker exiting");
                return;
            }
        }
    }
}

/// Состояние звуковой панели
#[derive(Clone)]
pub struct SoundPanelState {
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

    /// When enabled, hide_on_blur is bypassed so the panel can persist and be
    /// dragged by its title bar.
    pub stay_visible: Arc<Mutex<bool>>,

    /// Transient runtime flag: while config mode is active, blur-based hiding
    /// is suppressed (e.g. opening the native file picker must not hide the
    /// panel). Never persisted.
    pub config_mode: Arc<Mutex<bool>>,

    /// Transient focus flag used by the keyboard hook to let the active
    /// SoundPanel handle F1-F12 itself.
    window_focused: Arc<AtomicBool>,

    /// Очередь воспроизведения SoundPanel (sender-сторона).
    queue: SoundQueue,

    /// Приёмник очереди, изымается ровно один раз при запуске worker-а.
    queue_receiver: Arc<Mutex<Option<Receiver<QueueItem>>>>,
}

fn gen_set_id() -> String {
    Uuid::new_v4().to_string()
}

impl SoundPanelState {
    /// Создать новое состояние звуковой панели
    pub fn new(appdata_path: String) -> Self {
        let intercept = crate::soundpanel::intercept::load(&appdata_path);
        let (queue, queue_receiver) = SoundQueue::new();
        Self {
            sets: Arc::new(Mutex::new(SoundSets::default())),
            event_sender: Arc::new(Mutex::new(None)),
            appdata_path: Arc::new(Mutex::new(appdata_path)),
            floating_opacity: Arc::new(Mutex::new(DEFAULT_FLOATING_OPACITY)),
            floating_bg_color: Arc::new(Mutex::new(DEFAULT_FLOATING_BG_COLOR.to_string())),
            floating_clickthrough: Arc::new(Mutex::new(false)),
            intercept: Arc::new(Mutex::new(intercept)),
            stay_visible: Arc::new(Mutex::new(false)),
            config_mode: Arc::new(Mutex::new(false)),
            window_focused: Arc::new(AtomicBool::new(false)),
            queue,
            queue_receiver: Arc::new(Mutex::new(Some(queue_receiver))),
        }
    }

    /// Получить привязку по клавише из активного набора
    pub fn get_binding(&self, key: char) -> Option<SoundBinding> {
        self.sets.lock().ok().and_then(|sets| {
            sets.find_active()
                .and_then(|active| active.bindings.iter().find(|b| b.key == key).cloned())
        })
    }

    /// Получить все привязки активного набора (отсортированные)
    pub fn get_all_bindings(&self) -> Vec<SoundBinding> {
        self.sets
            .lock()
            .ok()
            .and_then(|sets| {
                sets.find_active().map(|active| {
                    let mut bindings: Vec<_> = active.bindings.clone();
                    bindings.sort_by_key(|a| a.key);
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

    /// Построить кандидатный снимок наборов с добавленной/заменённой привязкой
    /// в активном наборе. Не изменяет runtime-состояние.
    pub fn candidate_add_binding(&self, binding: &SoundBinding) -> SoundSets {
        let mut sets = self.get_sets();
        sets.add_active_binding(binding.clone());
        sets
    }

    /// Построить кандидатный снимок наборов без привязки в активном наборе.
    /// Не изменяет runtime-состояние.
    pub fn candidate_remove_binding(&self, key: char) -> SoundSets {
        let mut sets = self.get_sets();
        sets.remove_active_binding(key);
        sets
    }

    /// Опубликовать кандидатный снимок в runtime-состояние. Вызывается только
    /// после успешного долговечного сохранения.
    pub fn publish_sets(&self, new_sets: SoundSets) {
        if let Ok(mut sets) = self.sets.lock() {
            *sets = new_sets;
        } else {
            error!(target = "soundpanel::state", "Failed to lock sets");
        }
    }

    /// Воспроизвести звук по привязке: строит путь и ставит элемент в очередь.
    ///
    /// Не создаёт поток на каждый звук и не блокирует вызов. Возвращает ошибку
    /// при переполнении очереди или недоступном worker-е.
    pub fn play_sound(&self, binding: &SoundBinding, audio_settings: AudioSettings) -> Result<(), String> {
        let appdata_path = self
            .appdata_path
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .clone();
        let sound_path = format!("{}\\soundpanel\\{}", appdata_path, binding.filename);

        info!(target = "soundpanel", key = %binding.key, path = ?sound_path, "Enqueueing sound");

        self.enqueue_sound(sound_path, audio_settings)
    }

    /// Поставить аудиофайл в очередь воспроизведения SoundPanel.
    pub fn enqueue_sound(&self, path: String, audio_settings: AudioSettings) -> Result<(), String> {
        self.queue.try_enqueue(QueueItem {
            path,
            audio_settings,
        })
    }

    /// Запустить единственного worker-а очереди воспроизведения SoundPanel.
    ///
    /// Worker забирает элементы FIFO и синхронно воспроизводит каждый через
    /// `soundpanel::audio::play_audio_file`, не создавая поток на каждый звук.
    /// Завершается при отмене переданного `CancellationToken`. Вызывается ровно
    /// один раз из `setup::init_app`.
    pub fn start_queue_worker(&self, shutdown: CancellationToken) -> Result<(), String> {
        let receiver = self
            .queue_receiver
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .take()
            .ok_or_else(|| "SoundPanel queue worker already started".to_string())?;

        std::thread::spawn(move || {
            run_queue_worker(receiver, shutdown);
        });

        info!(target = "soundpanel::queue", "SoundPanel queue worker started");
        Ok(())
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

    /// Проверить, включен ли stay_visible
    pub fn get_stay_visible(&self) -> bool {
        self.stay_visible.lock().map(|v| *v).unwrap_or(false)
    }

    /// Установить stay_visible
    pub fn set_stay_visible(&self, enabled: bool) {
        if let Ok(mut val) = self.stay_visible.lock() {
            *val = enabled;
        } else {
            error!(target = "soundpanel::state", "Failed to lock stay_visible");
        }
    }

    /// Проверить, активен ли config-режим
    pub fn get_config_mode(&self) -> bool {
        self.config_mode.lock().map(|v| *v).unwrap_or(false)
    }

    /// Установить config-режим (транзитный флаг, не сохраняется на диск)
    pub fn set_config_mode(&self, enabled: bool) {
        if let Ok(mut val) = self.config_mode.lock() {
            *val = enabled;
        } else {
            error!(target = "soundpanel::state", "Failed to lock config_mode");
        }
    }

    pub fn is_window_focused(&self) -> bool {
        self.window_focused.load(Ordering::Acquire)
    }

    pub fn set_window_focused(&self, focused: bool) {
        self.window_focused.store(focused, Ordering::Release);
    }

    /// Получить настройки перехвата (clone)
    pub fn get_intercept(&self) -> InterceptSettings {
        self.intercept.lock().map(|v| v.clone()).unwrap_or_default()
    }

    /// Включить/выключить перехват (persist + emit)
    pub fn set_intercept_enabled(&self, enabled: bool) -> Result<(), String> {
        let appdata_path = self
            .appdata_path
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .clone();
        let mut val = self
            .intercept
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        let mut new_settings = val.clone();
        new_settings.enabled = enabled;
        crate::soundpanel::intercept::save(&appdata_path, &new_settings)?;
        *val = new_settings;
        let changed = val.enabled;
        drop(val);
        self.emit_event(AppEvent::InterceptionChanged(changed));
        Ok(())
    }

    /// Установить биндинг перехвата (persist)
    pub fn set_intercept_binding(&self, key: String, action: String) -> Result<(), String> {
        let appdata_path = self
            .appdata_path
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .clone();
        let mut val = self
            .intercept
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        let mut new_settings = val.clone();
        new_settings.bindings.retain(|b| b.key != key);
        new_settings
            .bindings
            .push(crate::soundpanel::intercept::InterceptBinding {
                key: key.clone(),
                action: action.clone(),
            });
        crate::soundpanel::intercept::save(&appdata_path, &new_settings)?;
        *val = new_settings;
        drop(val);
        info!(key = key, action = action, "Intercept binding set");
        Ok(())
    }

    /// Очистить биндинг перехвата (persist)
    pub fn clear_intercept_binding(&self, key: String) -> Result<(), String> {
        let appdata_path = self
            .appdata_path
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .clone();
        let mut val = self
            .intercept
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        let mut new_settings = val.clone();
        new_settings.bindings.retain(|b| b.key != key);
        crate::soundpanel::intercept::save(&appdata_path, &new_settings)?;
        *val = new_settings;
        drop(val);
        info!(key = key, "Intercept binding cleared");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn bad_path(label: &str) -> String {
        std::env::temp_dir()
            .join(format!(
                "ttsbard_state_test_{}_{}",
                std::process::id(),
                label,
            ))
            .join("nonexistent")
            .to_string_lossy()
            .to_string()
    }

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
    fn references_filename_protects_file_used_in_other_set() {
        let mut sets = SoundSets {
            active_set_id: "set1".into(),
            sets: vec![
                SoundSet {
                    id: "set1".into(),
                    name: "First".into(),
                    bindings: vec![SoundBinding {
                        key: 'A',
                        description: "removed".into(),
                        filename: "shared.mp3".into(),
                        original_path: None,
                    }],
                },
                SoundSet {
                    id: "set2".into(),
                    name: "Second".into(),
                    bindings: vec![SoundBinding {
                        key: 'B',
                        description: "other".into(),
                        filename: "shared.mp3".into(),
                        original_path: None,
                    }],
                },
            ],
        };

        sets.remove_active_binding('A');

        assert!(
            sets.references_filename("shared.mp3"),
            "filename still referenced by another set must be protected"
        );
        assert!(!sets.references_filename("unused.mp3"));
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

    // ── Intercept persistence failure safety ────────────────────────────

    #[test]
    fn set_intercept_enabled_persist_failure_leaves_state_unchanged() {
        let path = bad_path("enabled_fail");
        let state = SoundPanelState::new(path);
        assert!(!state.get_intercept().enabled);

        let result = state.set_intercept_enabled(true);
        assert!(result.is_err());
        assert!(!state.get_intercept().enabled);
    }

    #[test]
    fn set_intercept_enabled_persist_failure_does_not_emit_event() {
        let path = bad_path("enabled_noevent");
        let state = SoundPanelState::new(path);
        let (tx, rx) = mpsc::channel();
        state.set_event_sender(tx);

        let result = state.set_intercept_enabled(true);
        assert!(result.is_err());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn set_intercept_binding_persist_failure_leaves_state_unchanged() {
        let path = bad_path("binding_fail");
        let state = SoundPanelState::new(path);
        assert!(state.get_intercept().bindings.is_empty());

        let result = state.set_intercept_binding("NUMPAD1".into(), "play_sound".into());
        assert!(result.is_err());
        assert!(state.get_intercept().bindings.is_empty());
    }

    #[test]
    fn clear_intercept_binding_persist_failure_leaves_state_unchanged() {
        let path = bad_path("clear_fail");
        let state = SoundPanelState::new(path);
        // Pre-populate runtime (without persist) so we can test rollback
        {
            let mut val = state.intercept.lock().unwrap();
            val.bindings
                .push(crate::soundpanel::intercept::InterceptBinding {
                    key: "NUMPAD1".into(),
                    action: "play_sound".into(),
                });
        }
        assert_eq!(state.get_intercept().bindings.len(), 1);

        let result = state.clear_intercept_binding("NUMPAD1".into());
        assert!(result.is_err());
        assert_eq!(state.get_intercept().bindings.len(), 1);
    }

    // ── SoundPanel playback queue ────────────────────────────────────────

    fn queue_item(path: &str) -> QueueItem {
        QueueItem {
            path: path.to_string(),
            audio_settings: AudioSettings::default(),
        }
    }

    #[test]
    fn queue_worker_processes_items_fifo() {
        let (queue, rx) = SoundQueue::new();
        let token = CancellationToken::new();
        let played = Arc::new(Mutex::new(Vec::new()));
        let played_clone = Arc::clone(&played);

        let handle = std::thread::spawn(move || {
            run_queue_worker_with(rx, token, |item| {
                played_clone.lock().unwrap().push(item.path);
            });
        });

        queue.try_enqueue(queue_item("a")).unwrap();
        queue.try_enqueue(queue_item("b")).unwrap();
        queue.try_enqueue(queue_item("c")).unwrap();
        drop(queue);

        handle.join().unwrap();

        assert_eq!(*played.lock().unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn bounded_queue_rejects_when_full() {
        let (queue, rx) = SoundQueue::new();

        for i in 0..SOUND_QUEUE_CAPACITY {
            queue.try_enqueue(queue_item(&format!("{i}"))).unwrap();
        }

        assert!(queue.try_enqueue(queue_item("overflow")).is_err());

        drop(rx);
        assert!(queue.try_enqueue(queue_item("disconnected")).is_err());
    }

    #[test]
    fn queue_worker_stops_at_shutdown_before_next_item() {
        let (queue, rx) = SoundQueue::new();
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let played = Arc::new(Mutex::new(Vec::new()));
        let played_clone = Arc::clone(&played);

        let (started_tx, started_rx) = mpsc::channel::<()>();

        let handle = std::thread::spawn(move || {
            run_queue_worker_with(rx, worker_token, move |item| {
                played_clone.lock().unwrap().push(item.path);
                let _ = started_tx.send(());
                std::thread::sleep(Duration::from_millis(50));
            });
        });

        queue.try_enqueue(queue_item("a")).unwrap();
        queue.try_enqueue(queue_item("b")).unwrap();

        started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("first item must start playing");

        token.cancel();

        handle.join().unwrap();

        assert_eq!(*played.lock().unwrap(), vec!["a"]);
    }
}
