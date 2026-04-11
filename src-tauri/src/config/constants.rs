//! Application Constants
//!
//! Централизованное хранилище констант приложения.
//! Избегает дублирования magic numbers и значений по умолчанию.


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


