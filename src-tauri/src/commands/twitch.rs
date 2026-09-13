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
    /// Число сообщений, переданных клиенту Twitch: длинный текст доставляется
    /// несколькими частями по границам слов (ROADMAP-106).
    pub parts: usize,
}

/// Планирует доставку текста в Twitch: очистка, отказ от пустого текста и
/// разбиение по границам слов (ROADMAP-106). Слово, не помещающееся целиком в
/// лимит одного сообщения, — typed-отказ ДО отправки; сам длинный текст
/// разбивается на несколько сообщений без обрезания хвоста.
fn plan_delivery(text: &str, channel: &str) -> Result<Vec<String>, CommandError> {
    let clean = crate::twitch::clean_irc_text(text);
    if clean.is_empty() {
        return Err(CommandError::new(
            twitch_delivery::error_code::EMPTY_TEXT,
            "Twitch message must not be empty".to_string(),
            ipc::twitch_delivery_error_code_to_retryable(twitch_delivery::error_code::EMPTY_TEXT),
        ));
    }
    let too_long = |message: String| {
        CommandError::new(
            twitch_delivery::error_code::TOO_LONG,
            message,
            ipc::twitch_delivery_error_code_to_retryable(twitch_delivery::error_code::TOO_LONG),
        )
    };
    match crate::twitch::plan_message_parts(&clean, channel) {
        Ok(parts) => Ok(parts),
        Err(crate::twitch::PlanError::WordExceedsCharLimit) => Err(too_long(format!(
            "Twitch message contains a word longer than {} characters",
            crate::twitch::MAX_MESSAGE_CHARS
        ))),
        Err(crate::twitch::PlanError::WordExceedsWireBudget) => Err(too_long(
            "Twitch message contains a word that exceeds the IRC wire byte budget".to_string(),
        )),
    }
}

/// Маппинг отказа отправки одной части в typed-ошибку команды.
fn map_send_failure(failure: SendFailure) -> CommandError {
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
    CommandError::new(
        code,
        message,
        ipc::twitch_delivery_error_code_to_retryable(code),
    )
}

/// Провал одной из частей доставки. Провал ПЕРВОЙ части — обычная ошибка
/// отправки (ничего доставлено не было, семантика одиночного сообщения
/// сохранена). Провал последующей части — partial delivery: часть сообщений
/// уже ушла в Twitch, повтор всего текста дублировал бы их, поэтому ошибка не
/// retryable и не изображает полный успех.
fn map_part_failure(failure: SendFailure, failed_index: usize, total: usize) -> CommandError {
    if failed_index == 0 {
        return map_send_failure(failure);
    }
    CommandError::new(
        twitch_delivery::error_code::PARTIAL_DELIVERY,
        format!(
            "Twitch delivery stopped at message {} of {}: {}",
            failed_index + 1,
            total,
            failure
        ),
        ipc::twitch_delivery_error_code_to_retryable(twitch_delivery::error_code::PARTIAL_DELIVERY),
    )
}

/// Deliver a pre-processed message directly to the connected Twitch client.
///
/// This is the tracked Twitch-only route: it does not create a speech job,
/// does not write to phrase history and does not trigger WebView.
///
/// A `sent` result means the message was handed to the local IRC connection,
/// NOT that it is confirmed visible in the chat.
/// Long texts are split into ordered word-boundary messages (ROADMAP-106);
/// `parts` reports how many were handed to the client.
#[tauri::command]
pub async fn deliver_twitch_message(
    state: State<'_, AppState>,
    text: String,
) -> Result<DeliveredTwitchMessage, CommandError> {
    // Канал нужен планировщику частей: IRC wire budget зависит от длины
    // `PRIVMSG #<channel> :…` (ROADMAP-106).
    let channel = {
        let settings = state.twitch.settings.read().await;
        settings.channel.clone()
    };
    let parts = plan_delivery(&text, &channel)?;
    let total = parts.len();

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
    // Части отправляются строго последовательно (await каждой до постановки
    // следующей): порядок FIFO гарантирован, исходящая очередь не
    // переполняется, pacing worker'а соблюдает rate limit Twitch.
    for (index, part) in parts.into_iter().enumerate() {
        if let Err(failure) = client.send_message(&part).await {
            return Err(map_part_failure(failure, index, total));
        }
    }
    Ok(DeliveredTwitchMessage {
        status: "sent",
        parts: total,
    })
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
            let err = plan_delivery(text, "test").unwrap_err();
            assert_eq!(err.code, "twitch.empty_text");
            assert!(!err.retryable);
        }
    }

    #[test]
    fn delivery_validation_preserves_user_unicode_text() {
        let clean = plan_delivery("  привет 🌑 世界  ", "test").unwrap();
        assert_eq!(clean, vec!["привет 🌑 世界".to_string()]);
    }

    #[test]
    fn delivery_validation_measures_after_crlf_cleanup() {
        // \r удаляется, \n становится пробелом: замер по очищенному тексту.
        let clean = plan_delivery("a\r\nb", "test").unwrap();
        assert_eq!(clean, vec!["a b".to_string()]);
    }

    /// Слова по 4 символа с одним пробелом: ровно `total_chars` символов.
    fn words_text(total_chars: usize) -> String {
        let n = total_chars.div_ceil(5);
        let words_total = total_chars - (n - 1);
        let base = words_total / n;
        let extra = words_total % n;
        (0..n)
            .map(|i| "a".repeat(base + if i < extra { 1 } else { 0 }))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn delivery_plan_splits_long_text_into_wire_fitting_parts() {
        let text = words_text(501);
        let parts = plan_delivery(&text, "test").unwrap();
        assert!(parts.len() >= 2, "501 chars must split");
        for part in &parts {
            assert!(part.chars().count() <= crate::twitch::MAX_MESSAGE_CHARS);
            assert!(
                crate::twitch::wire_frame_len("test", part.len()) <= crate::twitch::MAX_WIRE_BYTES
            );
        }
        let planned: Vec<&str> = parts.iter().flat_map(|p| p.split_whitespace()).collect();
        let original: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(planned, original, "порядок слов и хвост сохранены");
    }

    #[test]
    fn delivery_plan_rejects_word_over_char_limit_before_any_send() {
        let text = "a".repeat(crate::twitch::MAX_MESSAGE_CHARS + 1);
        let err = plan_delivery(&text, "test").unwrap_err();
        assert_eq!(err.code, "twitch.too_long");
        assert!(!err.retryable);
        assert_eq!(
            err.message,
            "Twitch message contains a word longer than 500 characters"
        );
    }

    #[test]
    fn delivery_plan_rejects_word_over_wire_budget_before_any_send() {
        // 497 символов <= 500, но 17 + 497 = 514 > 512 байт wire-строки.
        let text = "a".repeat(497);
        let err = plan_delivery(&text, "test").unwrap_err();
        assert_eq!(err.code, "twitch.too_long");
        assert!(!err.retryable);
        assert_eq!(
            err.message,
            "Twitch message contains a word that exceeds the IRC wire byte budget"
        );
    }

    #[test]
    fn delivery_plan_returns_single_part_for_short_text() {
        let parts = plan_delivery("привет мир", "test").unwrap();
        assert_eq!(parts, vec!["привет мир".to_string()]);
    }

    #[test]
    fn first_part_failure_maps_to_single_send_error() {
        let err = map_part_failure(SendFailure::NotConnected, 0, 3);
        assert_eq!(err.code, "twitch.unavailable");
        assert!(err.retryable);
        assert_eq!(err.message, "Twitch is not connected");
    }

    #[test]
    fn later_part_failure_maps_to_typed_partial_delivery() {
        let err = map_part_failure(SendFailure::NotConnected, 1, 3);
        assert_eq!(err.code, "twitch.partial_delivery");
        assert!(!err.retryable);
        assert_eq!(
            err.message,
            "Twitch delivery stopped at message 2 of 3: Twitch not connected"
        );
    }

    #[test]
    fn partial_message_includes_underlying_failure_text() {
        let err = map_part_failure(SendFailure::Send("write failed".to_string()), 2, 4);
        assert_eq!(err.code, "twitch.partial_delivery");
        assert_eq!(
            err.message,
            "Twitch delivery stopped at message 3 of 4: write failed"
        );
    }
}
