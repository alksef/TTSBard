use crate::config::{SettingsManager, TwitchSettings};
use crate::events::TwitchConnectionStatus;
use crate::ipc::{self, twitch_delivery, CommandError};
use crate::state::AppState;
use crate::twitch::SendFailure;
use serde::Serialize;
use tauri::{Manager, State};

/// Получить текущие настройки Twitch (включая токен)
#[tauri::command]
pub async fn get_twitch_settings(state: State<'_, AppState>) -> Result<TwitchSettings, String> {
    let settings = state.twitch.settings.read().await;
    Ok(settings.clone())
}

/// Сохранить настройки Twitch и перезапустить клиент если нужно
#[tauri::command]
pub async fn save_twitch_settings(
    settings: TwitchSettings,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    tracing::info!(
        enabled = settings.enabled,
        start_on_boot = settings.start_on_boot,
        channel = ?settings.channel,
        "Saving Twitch settings"
    );

    // Валидация
    if let Err(e) = settings.is_valid() {
        return Err(format!("Validation failed: {}", e));
    }

    // Транзакционный подход: сначала сохраняем в файл, потом в память
    // Это предотвращает рассинхронизацию, если другой поток прочитает настройки между операциями
    // Получаем SettingsManager один раз
    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;

    // Проверка изменений по persisted-конфигу, а не по runtime-состоянию:
    // команды connect/disconnect мутируют только runtime `enabled`, и сравнение
    // с ним давало ложный Restart при сохранении без изменений.
    let old_settings = settings_manager
        .inner()
        .load()
        .map_err(|e| format!("Failed to load settings: {}", e))?
        .twitch;
    let enabled_changed = old_settings.enabled != settings.enabled;
    let credentials_changed = old_settings.username != settings.username
        || old_settings.token != settings.token
        || old_settings.channel != settings.channel;

    let persisted_settings = settings.clone();
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_twitch_settings(&persisted_settings)
    })
    .await?;

    // Только после успешного сохранения в файл обновляем AppState
    let mut s = state.twitch.settings.write().await;
    *s = settings.clone();
    drop(s);

    super::emit_settings_changed(&app_handle);

    // Отправить событие для перезапуска клиента только если есть изменения
    if enabled_changed || credentials_changed {
        state.send_twitch_event(crate::events::TwitchEvent::Restart);
        Ok("saved_reconnecting".to_string())
    } else {
        Ok("saved".to_string())
    }
}

/// Подключиться к Twitch
#[tauri::command]
pub async fn connect_twitch(state: State<'_, AppState>) -> Result<String, String> {
    tracing::info!("Connect command received");

    // Получаем текущие настройки
    let settings = state.twitch.settings.read().await;

    // Валидация
    if let Err(e) = settings.is_valid() {
        return Err(format!("Settings invalid: {}", e));
    }
    drop(settings);

    // Обновляем только runtime state (НЕ сохраняем в конфиг)
    let mut s = state.twitch.settings.write().await;
    s.enabled = true;
    drop(s);

    // Отправляем событие подключения
    state.send_twitch_event(crate::events::TwitchEvent::Restart);

    Ok("connecting".to_string())
}

/// Отключиться от Twitch
#[tauri::command]
pub async fn disconnect_twitch(state: State<'_, AppState>) -> Result<String, String> {
    tracing::info!("Disconnect command received");

    // Обновляем только runtime state (НЕ сохраняем в конфиг)
    let mut s = state.twitch.settings.write().await;
    s.enabled = false;
    drop(s);

    // Отправляем событие отключения
    state.send_twitch_event(crate::events::TwitchEvent::Stop);

    Ok("disconnected".to_string())
}

/// Получить текущий статус подключения Twitch
#[tauri::command]
pub async fn get_twitch_status(
    state: State<'_, AppState>,
) -> Result<crate::events::TwitchConnectionStatus, String> {
    let status = state.twitch.connection_status.lock().clone();
    Ok(status)
}

/// Проверить подключение к Twitch
#[tauri::command]
pub async fn test_twitch_connection(settings: TwitchSettings) -> Result<String, String> {
    // Валидация
    if let Err(e) = settings.is_valid() {
        return Err(format!("Validation failed: {}", e));
    }

    // Тестовое подключение (будет реализовано через отдельную функцию)
    // Для начала просто проверяем валидность
    Ok("Настройки валидны. Попробуйте подключиться.".to_string())
}

/// Перезапустить Twitch клиент
#[tauri::command]
pub async fn restart_twitch(state: State<'_, AppState>) -> Result<String, String> {
    tracing::info!("Restart command received");
    state.send_twitch_event(crate::events::TwitchEvent::Restart);
    Ok("restarting".to_string())
}

/// Successful Twitch-only delivery result.
#[derive(Debug, Clone, Serialize)]
pub struct DeliveredTwitchMessage {
    pub status: &'static str,
}

/// Валидирует текст доставки Twitch: возвращает очищенный текст либо
/// typed-отказ. Длина измеряется в байтах после очистки — это фактический
/// лимит доставки (см. `sanitize_irc_text` в twitch::client); политика
/// символов и разбиения длинного текста — ROADMAP-106, здесь только отказ
/// вместо молчаливой обрезки.
fn validate_delivery_text(text: &str) -> Result<String, CommandError> {
    let clean = crate::twitch::clean_irc_text(text);
    let code = if clean.is_empty() {
        twitch_delivery::error_code::EMPTY_TEXT
    } else if clean.len() > crate::twitch::MAX_MESSAGE_BYTES {
        twitch_delivery::error_code::TOO_LONG
    } else {
        return Ok(clean);
    };
    let message = if code == twitch_delivery::error_code::EMPTY_TEXT {
        "Twitch message must not be empty".to_string()
    } else {
        format!(
            "Twitch message exceeds {} bytes",
            crate::twitch::MAX_MESSAGE_BYTES
        )
    };
    Err(CommandError::new(
        code,
        message,
        ipc::twitch_delivery_error_code_to_retryable(code),
    ))
}

/// Deliver a pre-processed message directly to the connected Twitch client.
///
/// This is the tracked Twitch-only route: it does not create a speech job,
/// does not write to phrase history and does not trigger WebView.
///
/// A `sent` result means the message was handed to the local IRC connection,
/// NOT that it is confirmed visible in the chat.
#[tauri::command]
pub async fn deliver_twitch_message(
    state: State<'_, AppState>,
    text: String,
) -> Result<DeliveredTwitchMessage, CommandError> {
    let clean_text = validate_delivery_text(&text)?;

    let settings_enabled = {
        let settings = state.twitch.settings.read().await;
        settings.enabled
    };
    let is_connected = matches!(
        state.twitch.connection_status.lock().clone(),
        TwitchConnectionStatus::Connected
    );
    let client = {
        let guard = state.twitch.client.read().await;
        guard.clone()
    };

    if !settings_enabled || !is_connected || client.is_none() {
        return Err(CommandError::new(
            twitch_delivery::error_code::UNAVAILABLE,
            "Twitch is not connected".to_string(),
            ipc::twitch_delivery_error_code_to_retryable(twitch_delivery::error_code::UNAVAILABLE),
        ));
    }

    let client = client.expect("client presence checked above");
    match client.send_message(&clean_text).await {
        Ok(()) => Ok(DeliveredTwitchMessage { status: "sent" }),
        Err(failure) => {
            let (code, message) = match &failure {
                SendFailure::NotConnected => (
                    twitch_delivery::error_code::UNAVAILABLE,
                    "Twitch is not connected".to_string(),
                ),
                SendFailure::QueueFull => (
                    twitch_delivery::error_code::QUEUE_FULL,
                    "Twitch outgoing queue is full".to_string(),
                ),
                SendFailure::Send(e) => (twitch_delivery::error_code::SEND_FAILED, e.clone()),
            };
            Err(CommandError::new(
                code,
                message,
                ipc::twitch_delivery_error_code_to_retryable(code),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deliver_command_name_matches_registered_function() {
        assert_eq!(
            twitch_delivery::DELIVER_COMMAND,
            stringify!(deliver_twitch_message)
        );
    }

    #[test]
    fn delivery_validation_rejects_empty_and_whitespace_text() {
        for text in ["", "   ", "\t\r\n", "\x01\x02 "] {
            let err = validate_delivery_text(text).unwrap_err();
            assert_eq!(err.code, "twitch.empty_text");
            assert!(!err.retryable);
        }
    }

    #[test]
    fn delivery_validation_preserves_user_unicode_text() {
        let clean = validate_delivery_text("  привет 🌑 世界  ").unwrap();
        assert_eq!(clean, "привет 🌑 世界");
    }

    #[test]
    fn delivery_validation_accepts_exactly_max_bytes() {
        // 250 кириллических символов = ровно 500 байт.
        let text = "я".repeat(250);
        let clean = validate_delivery_text(&text).unwrap();
        assert_eq!(clean.len(), crate::twitch::MAX_MESSAGE_BYTES);
    }

    #[test]
    fn delivery_validation_rejects_over_limit_with_typed_code() {
        // 251 кириллический символ = 502 байта: отказ вместо молчаливой обрезки.
        let text = "я".repeat(251);
        let err = validate_delivery_text(&text).unwrap_err();
        assert_eq!(err.code, "twitch.too_long");
        assert!(!err.retryable);
        assert_eq!(err.message, "Twitch message exceeds 500 bytes");
    }

    #[test]
    fn delivery_validation_measures_after_crlf_cleanup() {
        // \r удаляется, \n становится пробелом: замер по очищенному тексту.
        let clean = validate_delivery_text("a\r\nb").unwrap();
        assert_eq!(clean, "a b");
    }
}
