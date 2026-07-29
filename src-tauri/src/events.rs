use crate::tts::TtsProviderType;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
    TextSentToTts(RoutedText),
    /// Изменение статуса TTS
    TtsStatusChanged(TtsStatus),
    /// Ошибка TTS
    TtsError(String),
    /// Показать главное окно
    ShowMainWindow,
    /// Обновить иконку в системном трее
    UpdateTrayIcon(bool),
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
    /// Показать окно управления воспроизведением
    ShowPlaybackControlWindow,
    /// Скрыть окно управления воспроизведением
    HidePlaybackControlWindow,
    /// Изменение TTS провайдера
    TtsProviderChanged(TtsProviderType),
    /// Ошибка запуска WebView сервера
    WebViewServerError(String),
    /// Перезапустить WebView сервер (изменились настройки)
    RestartWebViewServer,
    /// Перезагрузить шаблоны WebView (без перезапуска сервера)
    ReloadWebViewTemplates,
    /// Включить/выключить UPnP (без перезапуска сервера)
    ToggleUpnp(bool),
    /// Изменение статуса подключения Twitch
    TwitchStatusChanged(TwitchConnectionStatus),
    /// Воспроизведение началось
    PlaybackStarted { text_id: String, text: String },
    /// Воспроизведение фразы завершено
    PlaybackFinished { text_id: String },
    /// Ошибка воспроизведения (не удалось открыть ни одного output sink)
    PlaybackFailed { text_id: String, error: String },
    /// Воспроизведение приостановлено
    PlaybackPaused,
    /// Воспроизведение возобновлено
    PlaybackResumed,
    /// Воспроизведение остановлено
    PlaybackStopped,
    /// Очередь изменилась
    QueueChanged,
    /// WebView typing state changed (service-owned, routed to WebView server)
    WebViewTypingChanged(bool),
    /// Завершение работы приложения
    Quit,
}

/// Text routed after synthesis together with request-local delivery policy.
///
/// The routing fields are deliberately not serialized: the frontend keeps the
/// existing `{"TextSentToTts":"..."}` contract while backend consumers avoid
/// sharing mutable prefix flags between concurrent requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedText {
    pub text: String,
    pub skip_twitch: bool,
    pub skip_webview: bool,
}

impl RoutedText {
    pub fn new(text: String, skip_twitch: bool, skip_webview: bool) -> Self {
        Self {
            text,
            skip_twitch,
            skip_webview,
        }
    }

    pub fn broadcast(text: String) -> Self {
        Self::new(text, false, false)
    }
}

impl Serialize for RoutedText {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.text)
    }
}

impl<'de> Deserialize<'de> for RoutedText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::broadcast)
    }
}

/// Typed WebView SSE broadcast payload
#[derive(Debug, Clone)]
pub enum WebViewSseEvent {
    Text(String),
    Typing(bool),
}

/// События для управления Twitch клиентом
#[derive(Debug, Clone)]
pub enum TwitchEvent {
    /// Перезапустить клиент (изменены настройки)
    Restart,
    /// Остановить клиент
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

/// Статус подключения к VTube Studio
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VTubeStudioConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
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
            AppEvent::ShowMainWindow => "show-main-window",
            AppEvent::UpdateTrayIcon(_) => "update-tray-icon",
            AppEvent::ClickthroughChanged(_) => "clickthrough-changed",
            AppEvent::ShowSoundPanelWindow => "show-soundpanel-window",
            AppEvent::HideSoundPanelWindow => "hide-soundpanel-window",
            AppEvent::SoundPanelNoBinding(_) => "soundpanel-no-binding",
            AppEvent::SoundPanelAppearanceChanged => "soundpanel-appearance-changed",
            AppEvent::ShowPlaybackControlWindow => "show-playback-control-window",
            AppEvent::HidePlaybackControlWindow => "hide-playback-control-window",
            AppEvent::TtsProviderChanged(_) => "tts-provider-changed",
            AppEvent::WebViewServerError(_) => "webview-server-error",
            AppEvent::RestartWebViewServer => "restart-webview-server",
            AppEvent::ReloadWebViewTemplates => "reload-webview-templates",
            AppEvent::ToggleUpnp(_) => "toggle-upnp",
            AppEvent::WebViewTypingChanged(_) => "webview-typing-changed",
            AppEvent::TwitchStatusChanged(_) => "twitch-status-changed",
            AppEvent::PlaybackStarted { .. } => "playback-started",
            AppEvent::PlaybackFinished { .. } => "playback-finished",
            AppEvent::PlaybackFailed { .. } => "playback-failed",
            AppEvent::PlaybackPaused => "playback-paused",
            AppEvent::PlaybackResumed => "playback-resumed",
            AppEvent::PlaybackStopped => "playback-stopped",
            AppEvent::QueueChanged => "queue-changed",
            AppEvent::Quit => "app-quit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppEvent, RoutedText};

    #[test]
    fn routed_text_preserves_frontend_wire_format() {
        let event = AppEvent::TextSentToTts(RoutedText::new("hello".to_string(), true, true));

        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"{"TextSentToTts":"hello"}"#
        );
    }

    #[test]
    fn routed_text_deserializes_legacy_wire_format_with_broadcast_policy() {
        let event: AppEvent = serde_json::from_str(r#"{"TextSentToTts":"hello"}"#).unwrap();

        match event {
            AppEvent::TextSentToTts(routed) => {
                assert_eq!(routed.text, "hello");
                assert!(!routed.skip_twitch);
                assert!(!routed.skip_webview);
            }
            _ => panic!("unexpected event"),
        }
    }
}
