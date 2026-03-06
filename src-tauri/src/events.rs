use serde::{Deserialize, Serialize};
use crate::tts::TtsProviderType;
use std::sync::mpsc::Sender;
use tokio::sync::broadcast;

/// Type alias for the event sender channel
pub type EventSender = Sender<AppEvent>;

/// Type alias for Twitch event sender
pub type TwitchEventSender = broadcast::Sender<TwitchEvent>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    /// Изменение статуса перехвата клавиатуры
    InterceptionChanged(bool),
    /// Изменение раскладки (EN/RU)
    LayoutChanged(InputLayout),
    /// Текст готов для отправки в TTS
    TextReady(String),
    /// Текст отправлен в TTS (для WebView Source)
    TextSentToTts(String),
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
    /// Ошибка запуска WebView сервера
    WebViewServerError(String),
    /// Перезапустить WebView сервер (изменились настройки)
    RestartWebViewServer,
    /// Изменение статуса подключения Twitch
    TwitchStatusChanged(TwitchConnectionStatus),
}

/// События для управления Twitch клиентом
#[derive(Debug, Clone)]
pub enum TwitchEvent {
    /// Перезапустить клиент (изменены настройки)
    Restart,
    /// Остановить клиент
    #[allow(dead_code)]
    Stop,
    /// Отправить сообщение
    SendMessage(String),
}

/// Статус подключения к Twitch
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TwitchConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
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
            AppEvent::TextSentToTts(_) => "text-sent-to-tts",
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
            AppEvent::WebViewServerError(_) => "webview-server-error",
            AppEvent::RestartWebViewServer => "restart-webview-server",
            AppEvent::TwitchStatusChanged(_) => "twitch-status-changed",
        }
    }
}
