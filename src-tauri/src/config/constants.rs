//! Application Constants
//!
//! Централизованное хранилище констант приложения.
//! Избегает дублирования magic numbers и значений по умолчанию.

/// Порт по умолчанию для WebView сервера
#[allow(dead_code)]
pub const DEFAULT_WEBVIEW_PORT: u16 = 10100;

/// Прозрачность floating окна по умолчанию (10-100)
pub const DEFAULT_FLOATING_OPACITY: u8 = 90;

/// Цвет фона floating окна по умолчанию (hex #RRGGBB)
pub const DEFAULT_FLOATING_BG_COLOR: &str = "#2a2a2a";

/// Таймаут для HTTP запросов TTS в секундах
pub const DEFAULT_TTS_TIMEOUT_SECS: u64 = 30;

/// Минимальная прозрачность floating окна
pub const MIN_FLOATING_OPACITY: u8 = 10;

/// Максимальная прозрачность floating окна
pub const MAX_FLOATING_OPACITY: u8 = 100;

/// Скорость анимации по умолчанию (мс)
#[allow(dead_code)]
pub const DEFAULT_ANIMATION_SPEED_MS: u32 = 300;

/// Адрес привязки по умолчанию для WebView сервера
#[allow(dead_code)]
pub const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0";
