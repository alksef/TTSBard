use serde::{Deserialize, Serialize};
use crate::tts::TtsProviderType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    /// Изменение статуса перехвата клавиатуры
    InterceptionChanged(bool),
    /// Изменение раскладки (EN/RU)
    LayoutChanged(InputLayout),
    /// Текст готов для отправки в TTS
    TextReady(String),
    /// Изменение статуса TTS
    TtsStatusChanged(TtsStatus),
    /// Ошибка TTS
    TtsError(String),
    /// Показать плавающее окно
    ShowFloatingWindow,
    /// Скрыть плавающее окно
    HideFloatingWindow,
    /// Показать главное окно
    ShowMainWindow,
    /// Обновить текст в плавающем окне
    UpdateFloatingText(String),
    /// Обновить иконку в системном трее
    UpdateTrayIcon(bool),
    /// Изменение внешнего вида плавающего окна
    FloatingAppearanceChanged,
    /// Изменение clickthrough режима
    ClickthroughChanged(bool),
    /// Показать floating окно звуковой панели
    ShowSoundPanelWindow,
    /// Скрыть floating окно звуковой панели
    HideSoundPanelWindow,
    /// Нет привязки для нажатой клавиши (параметр - клавиша)
    SoundPanelNoBinding(char),
    /// Изменение внешнего вида звуковой панели
    SoundPanelAppearanceChanged,
    /// Изменение TTS провайдера
    TtsProviderChanged(TtsProviderType),
    /// Изменение режима закрытия по Enter (F6 mode)
    EnterClosesDisabled(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq)]
pub enum InputLayout {
    English,
    Russian,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TtsStatus {
    Idle,
    Speaking,
    Error(String),
}

impl AppEvent {
    pub fn to_tauri_event(&self) -> &'static str {
        match self {
            AppEvent::InterceptionChanged(_) => "interception-changed",
            AppEvent::LayoutChanged(_) => "layout-changed",
            AppEvent::TextReady(_) => "text-ready",
            AppEvent::TtsStatusChanged(_) => "tts-status-changed",
            AppEvent::TtsError(_) => "tts-error",
            AppEvent::ShowFloatingWindow => "show-floating-window",
            AppEvent::HideFloatingWindow => "hide-floating-window",
            AppEvent::ShowMainWindow => "show-main-window",
            AppEvent::UpdateFloatingText(_) => "update-floating-text",
            AppEvent::UpdateTrayIcon(_) => "update-tray-icon",
            AppEvent::FloatingAppearanceChanged => "floating-appearance-changed",
            AppEvent::ClickthroughChanged(_) => "clickthrough-changed",
            AppEvent::ShowSoundPanelWindow => "show-soundpanel-window",
            AppEvent::HideSoundPanelWindow => "hide-soundpanel-window",
            AppEvent::SoundPanelNoBinding(_) => "soundpanel-no-binding",
            AppEvent::SoundPanelAppearanceChanged => "soundpanel-appearance-changed",
            AppEvent::TtsProviderChanged(_) => "tts-provider-changed",
            AppEvent::EnterClosesDisabled(_) => "enter-closes-disabled",
        }
    }
}
