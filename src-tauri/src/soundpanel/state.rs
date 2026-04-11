//! Sound Panel State
//!
//! Управление состоянием звуковой панели: привязки клавиш, флаг перехвата.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread::JoinHandle;
use serde::{Deserialize, Serialize};
use tracing::{error, info, debug};
use crate::events::AppEvent;
use crate::config::{DEFAULT_FLOATING_OPACITY, DEFAULT_FLOATING_BG_COLOR, MIN_FLOATING_OPACITY, MAX_FLOATING_OPACITY};

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

/// Состояние звуковой панели
#[derive(Clone)]
pub struct SoundPanelState {
    /// Включен ли режим перехвата для звуковой панели
    pub interception_enabled: Arc<Mutex<bool>>,

    /// Привязки клавиш к звукам (key -> binding)
    pub bindings: Arc<Mutex<HashMap<char, SoundBinding>>>,

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

    /// Активные воспроизведения звука (thread handles)
    active_playbacks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl SoundPanelState {
    /// Создать новое состояние звуковой панели
    pub fn new(appdata_path: String) -> Self {
        Self {
            interception_enabled: Arc::new(Mutex::new(false)),
            bindings: Arc::new(Mutex::new(HashMap::new())),
            event_sender: Arc::new(Mutex::new(None)),
            appdata_path: Arc::new(Mutex::new(appdata_path)),
            floating_opacity: Arc::new(Mutex::new(DEFAULT_FLOATING_OPACITY)),
            floating_bg_color: Arc::new(Mutex::new(DEFAULT_FLOATING_BG_COLOR.to_string())),
            floating_clickthrough: Arc::new(Mutex::new(false)),
            active_playbacks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Проверить, включен ли режим перехвата
    pub fn is_interception_enabled(&self) -> bool {
        self.interception_enabled.lock().map(|v| *v).unwrap_or(false)
    }

    /// Установить режим перехвата
    pub fn set_interception_enabled(&self, enabled: bool) {
        if let Ok(mut val) = self.interception_enabled.lock() {
            *val = enabled;
        } else {
            error!(target = "soundpanel::state", "Failed to lock interception_enabled");
        }
    }

    /// Получить привязку по клавише
    pub fn get_binding(&self, key: char) -> Option<SoundBinding> {
        self.bindings.lock()
            .ok()
            .and_then(|b| b.get(&key).cloned())
    }

    /// Добавить привязку
    pub fn add_binding(&self, binding: SoundBinding) {
        if let Ok(mut bindings) = self.bindings.lock() {
            bindings.insert(binding.key, binding.clone());
        } else {
            error!(target = "soundpanel::state", "Failed to lock bindings");
        }
    }

    /// Удалить привязку
    pub fn remove_binding(&self, key: char) {
        if let Ok(mut bindings) = self.bindings.lock() {
            bindings.remove(&key);
        } else {
            error!(target = "soundpanel::state", "Failed to lock bindings");
        }
    }

    /// Получить все привязки
    pub fn get_all_bindings(&self) -> Vec<SoundBinding> {
        self.bindings.lock()
            .ok()
            .map(|b| {
                let mut bindings: Vec<_> = b.values().cloned().collect();
                bindings.sort_by(|a, b| a.key.cmp(&b.key));
                bindings
            })
            .unwrap_or_default()
    }

    /// Воспроизвести звук по привязке
    pub fn play_sound(&self, binding: &SoundBinding) {
        let appdata_path = self.appdata_path.lock().unwrap().clone();
        let sound_path = format!("{}\\soundpanel\\{}", appdata_path, binding.filename);

        info!(target = "soundpanel", key = %binding.key, path = ?sound_path, "Playing sound");

        // Запустить воспроизведение в отдельном потоке и отслеживать handle
        let handle = std::thread::spawn(move || {
            super::audio::play_audio_file(&sound_path);
        });

        // Сохранить handle и очистить завершённые потоки
        if let Ok(mut playbacks) = self.active_playbacks.lock() {
            playbacks.push(handle);
            // Удаляем завершённые потоки для предотвращения утечки памяти
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
                debug!(target = "soundpanel::state", "Sending event through channel");
                match sender.send(event) {
                    Ok(_) => debug!(target = "soundpanel::state", "Event sent successfully"),
                    Err(error) => error!(target = "soundpanel::state", error = %error, "Failed to send event"),
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
        self.floating_opacity.lock().map(|v| *v).unwrap_or(DEFAULT_FLOATING_OPACITY)
    }

    /// Установить прозрачность floating окна
    pub fn set_floating_opacity(&self, value: u8) {
        debug!(target = "soundpanel::state", value, "set_floating_opacity called");
        if let Ok(mut val) = self.floating_opacity.lock() {
            *val = value.clamp(MIN_FLOATING_OPACITY, MAX_FLOATING_OPACITY);
            debug!(target = "soundpanel::state", opacity = *val, "Opacity updated");
        } else {
            error!(target = "soundpanel::state", "Failed to lock floating_opacity");
            return;
        }
        // Emit event AFTER releasing the mutex to prevent potential deadlock
        debug!(target = "soundpanel::state", "Emitting SoundPanelAppearanceChanged event");
        self.emit_event(AppEvent::SoundPanelAppearanceChanged);
        debug!(target = "soundpanel::state", "Event emitted");
    }

    /// Получить цвет фона floating окна
    pub fn get_floating_bg_color(&self) -> String {
        self.floating_bg_color.lock().unwrap().clone()
    }

    /// Установить цвет фона floating окна
    pub fn set_floating_bg_color(&self, color: String) {
        debug!(target = "soundpanel::state", color, "set_floating_bg_color called");
        let color_clone = color.clone();
        if let Ok(mut val) = self.floating_bg_color.lock() {
            *val = color_clone.clone();
            debug!(target = "soundpanel::state", color = ?color_clone, "Color updated");
        } else {
            error!(target = "soundpanel::state", "Failed to lock floating_bg_color");
            return;
        }
        // Emit event AFTER releasing the mutex to prevent potential deadlock
        debug!(target = "soundpanel::state", "Emitting SoundPanelAppearanceChanged event");
        self.emit_event(AppEvent::SoundPanelAppearanceChanged);
        debug!(target = "soundpanel::state", "Event emitted");
    }

    /// Проверить, включен ли clickthrough для floating окна
    pub fn is_floating_clickthrough_enabled(&self) -> bool {
        self.floating_clickthrough.lock().map(|v| *v).unwrap_or(false)
    }

    /// Установить clickthrough для floating окна
    pub fn set_floating_clickthrough(&self, enabled: bool) {
        debug!(target = "soundpanel::state", enabled, "set_floating_clickthrough called");
        if let Ok(mut val) = self.floating_clickthrough.lock() {
            *val = enabled;
            debug!(target = "soundpanel::state", enabled, "Clickthrough updated");
        } else {
            error!(target = "soundpanel::state", "Failed to lock floating_clickthrough");
        }
    }
}
