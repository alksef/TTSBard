//! Sound Panel State
//!
//! Управление состоянием звуковой панели: привязки клавиш, флаг перехвата.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::events::AppEvent;

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

    /// Исключить ли окно из записи экрана
    pub exclude_from_recording: Arc<Mutex<bool>>,
}

impl SoundPanelState {
    /// Создать новое состояние звуковой панели
    pub fn new(appdata_path: String) -> Self {
        Self {
            interception_enabled: Arc::new(Mutex::new(false)),
            bindings: Arc::new(Mutex::new(HashMap::new())),
            event_sender: Arc::new(Mutex::new(None)),
            appdata_path: Arc::new(Mutex::new(appdata_path)),
            floating_opacity: Arc::new(Mutex::new(90)),
            floating_bg_color: Arc::new(Mutex::new("#2a2a2a".to_string())),
            floating_clickthrough: Arc::new(Mutex::new(false)),
            exclude_from_recording: Arc::new(Mutex::new(false)),
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
        }
    }

    /// Удалить привязку
    pub fn remove_binding(&self, key: char) {
        if let Ok(mut bindings) = self.bindings.lock() {
            bindings.remove(&key);
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

        eprintln!("[SOUNDPANEL] Playing sound: {} -> {}", binding.key, sound_path);

        // Запустить воспроизведение в отдельном потоке
        std::thread::spawn(move || {
            super::audio::play_audio_file(&sound_path);
        });
    }

    /// Установить отправитель событий
    pub fn set_event_sender(&self, sender: Sender<AppEvent>) {
        if let Ok(mut es) = self.event_sender.lock() {
            *es = Some(sender);
        }
    }

    /// Отправить событие
    pub fn emit_event(&self, event: AppEvent) {
        eprintln!("[SOUNDPANEL STATE] emit_event called with event: {:?}", std::mem::discriminant(&event));
        if let Ok(es) = self.event_sender.lock() {
            if let Some(ref sender) = *es {
                eprintln!("[SOUNDPANEL STATE] Sending event through channel");
                match sender.send(event) {
                    Ok(_) => eprintln!("[SOUNDPANEL STATE] Event sent successfully"),
                    Err(e) => eprintln!("[SOUNDPANEL STATE] Failed to send event: {}", e),
                }
            } else {
                eprintln!("[SOUNDPANEL STATE] ERROR: event_sender is None!");
            }
        } else {
            eprintln!("[SOUNDPANEL STATE] ERROR: failed to lock event_sender!");
        }
    }

    /// Получить прозрачность floating окна
    pub fn get_floating_opacity(&self) -> u8 {
        self.floating_opacity.lock().map(|v| *v).unwrap_or(90)
    }

    /// Установить прозрачность floating окна
    pub fn set_floating_opacity(&self, value: u8) {
        eprintln!("[SOUNDPANEL STATE] set_floating_opacity called with value={}", value);
        if let Ok(mut val) = self.floating_opacity.lock() {
            *val = value.clamp(10, 100);
            eprintln!("[SOUNDPANEL STATE] Opacity updated to: {}", *val);
        }
        eprintln!("[SOUNDPANEL STATE] Emitting SoundPanelAppearanceChanged event");
        self.emit_event(AppEvent::SoundPanelAppearanceChanged);
        eprintln!("[SOUNDPANEL STATE] Event emitted");
    }

    /// Получить цвет фона floating окна
    pub fn get_floating_bg_color(&self) -> String {
        self.floating_bg_color.lock().unwrap().clone()
    }

    /// Установить цвет фона floating окна
    pub fn set_floating_bg_color(&self, color: String) {
        eprintln!("[SOUNDPANEL STATE] set_floating_bg_color called with color={}", color);
        if let Ok(mut val) = self.floating_bg_color.lock() {
            *val = color.clone();
            eprintln!("[SOUNDPANEL STATE] Color updated to: {}", color);
        }
        eprintln!("[SOUNDPANEL STATE] Emitting SoundPanelAppearanceChanged event");
        self.emit_event(AppEvent::SoundPanelAppearanceChanged);
        eprintln!("[SOUNDPANEL STATE] Event emitted");
    }

    /// Проверить, включен ли clickthrough для floating окна
    pub fn is_floating_clickthrough_enabled(&self) -> bool {
        self.floating_clickthrough.lock().map(|v| *v).unwrap_or(false)
    }

    /// Установить clickthrough для floating окна
    pub fn set_floating_clickthrough(&self, enabled: bool) {
        eprintln!("[SOUNDPANEL STATE] set_floating_clickthrough called with enabled={}", enabled);
        if let Ok(mut val) = self.floating_clickthrough.lock() {
            *val = enabled;
            eprintln!("[SOUNDPANEL STATE] Clickthrough updated to: {}", enabled);
        }
    }

    /// Проверить, исключено ли окно из записи экрана
    pub fn is_exclude_from_recording(&self) -> bool {
        self.exclude_from_recording.lock().map(|v| *v).unwrap_or(false)
    }

    /// Установить исключение из записи экрана
    pub fn set_exclude_from_recording(&self, enabled: bool) {
        eprintln!("[SOUNDPANEL STATE] set_exclude_from_recording called with enabled={}", enabled);
        if let Ok(mut val) = self.exclude_from_recording.lock() {
            *val = enabled;
            eprintln!("[SOUNDPANEL STATE] Exclude from recording updated to: {}", enabled);
        }
    }
}
