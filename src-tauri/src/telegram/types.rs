use serde::{Deserialize, Serialize};

/// Состояние клиента авторизации
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthState {
    Idle,
    CodeRequired,
    Connected,
    Error(String),
}

/// Информация о пользователе
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: String,
}

/// Результат операции
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
}

impl OperationResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }

}

/// Результат TTS операции
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsResult {
    pub success: bool,
    pub audio_path: Option<String>,
    pub duration: Option<f32>,
    pub error: Option<String>,
}

impl TtsResult {
    pub fn success(audio_path: String) -> Self {
        Self {
            success: true,
            audio_path: Some(audio_path),
            duration: None,
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            audio_path: None,
            duration: None,
            error: Some(error),
        }
    }
}

/// Информация о текущем голосе Silero TTS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentVoice {
    pub name: String,
    pub id: String,
}

/// Информация о лимитах Silero TTS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub voices: String,
    pub gifs: String,
}

/// Сохраненный код голоса для Telegram TTS
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceCode {
    pub id: String,  // e.g., "rene", "hamster_clerk"
    pub description: Option<String>,  // e.g., "Rene", "Хомяки"
}
