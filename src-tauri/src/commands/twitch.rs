use crate::config::{SettingsManager, TwitchSettings};
use crate::events::TwitchConnectionStatus;
use crate::ipc::{self, twitch_delivery, CommandError};
use crate::state::AppState;
use crate::twitch::auth::{
    parse_role, AuthError, AuthStatusDto, BeginAuthResult, TwitchAuthCoordinator,
};
use crate::twitch::{SendFailure, TwitchClient};
use serde::Serialize;
use std::sync::Arc;
use tauri::{Manager, State};

fn connection_command_error(code: &'static str) -> CommandError {
    let stage = if code == "twitch.settings_save_failed" {
        "settings_save"
    } else {
        "settings_validation"
    };
    tracing::error!(
        error_code = code,
        stage,
        retryable = false,
        "Twitch command rejected"
    );
    CommandError::new(code, code, false)
}

fn validate_connection_settings(settings: &TwitchSettings) -> Result<(), CommandError> {
    settings.is_valid().map_err(|code| {
        connection_command_error(match code.as_str() {
            "twitch.invalid_username" => "twitch.invalid_username",
            "twitch.invalid_channel" => "twitch.invalid_channel",
            "twitch.missing_token" => "twitch.missing_token",
            _ => "twitch.settings_save_failed",
        })
    })
}

/// Получить текущие настройки Twitch (включая токен)
#[tauri::command]
pub async fn get_twitch_settings(state: State<'_, AppState>) -> Result<TwitchSettings, String> {
    let settings = state.twitch.settings.read().await;
    Ok(settings.clone())
}

/// Core logic for saving Twitch settings, separated for unit testability.
pub(crate) async fn save_twitch_settings_core(
    settings: TwitchSettings,
    settings_manager: &SettingsManager,
    state_settings: &tokio::sync::RwLock<TwitchSettings>,
) -> Result<(String, Option<crate::events::TwitchEvent>), CommandError> {
    // Проверка изменений по persisted-конфигу, а не по runtime-состоянию:
    // команды connect/disconnect мутируют только runtime `enabled`, и сравнение
    // с ним давало ложный Restart при сохранении без изменений.
    let old_settings = settings_manager
        .load()
        .map_err(|_| connection_command_error("twitch.settings_save_failed"))?
        .twitch;
    let enabled_changed = old_settings.enabled != settings.enabled;
    let mode_changed = old_settings.mode != settings.mode;
    // Выбор режима открывает его форму, даже если реквизиты ещё не заполнены.
    // Он только останавливает клиент; подключение валидируется отдельно.
    if !mode_changed {
        validate_connection_settings(&settings)?;
    }
    let credentials_changed = old_settings.username != settings.username
        || old_settings.token != settings.token
        || old_settings.channel != settings.channel;
    // Реквизиты перечитываются клиентом только при подключении: restart ради
    // переподключения имеет смысл лишь для живого клиента. Отключённому
    // сохранение даёт просто записанные реквизиты, а следующая команда
    // connect/restart поднимет одно соединение уже с ними.
    let runtime_enabled = { state_settings.read().await.enabled };

    let persisted_settings = settings.clone();
    super::persist_blocking(settings_manager, move |mgr| {
        mgr.set_twitch_settings(&persisted_settings)
    })
    .await
    .map_err(|_| connection_command_error("twitch.settings_save_failed"))?;

    // Только после успешного сохранения в файл обновляем AppState
    let mut s = state_settings.write().await;
    *s = settings.clone();
    s.enabled = if mode_changed { false } else { runtime_enabled };
    drop(s);

    if mode_changed {
        Ok(("saved".to_string(), Some(crate::events::TwitchEvent::Stop)))
    } else if enabled_changed || (credentials_changed && runtime_enabled) {
        Ok((
            "saved_reconnecting".to_string(),
            Some(crate::events::TwitchEvent::Restart),
        ))
    } else {
        Ok(("saved".to_string(), None))
    }
}

/// Сохранить настройки Twitch и перезапустить клиент если нужно
#[tauri::command]
pub async fn save_twitch_settings(
    settings: TwitchSettings,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, CommandError> {
    tracing::info!(
        enabled = settings.enabled,
        start_on_boot = settings.start_on_boot,
        "Saving Twitch settings"
    );

    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| connection_command_error("twitch.settings_save_failed"))?;

    let (result, event) =
        save_twitch_settings_core(settings, settings_manager.inner(), &state.twitch.settings)
            .await?;

    super::emit_settings_changed(&app_handle);

    if let Some(event) = event {
        state.send_twitch_event(event);
    }

    Ok(result)
}

/// Подключиться к Twitch
#[tauri::command]
pub async fn connect_twitch(state: State<'_, AppState>) -> Result<String, CommandError> {
    tracing::info!("Connect command received");

    // Получаем текущие настройки
    let settings = state.twitch.settings.read().await;

    // Валидация
    validate_connection_settings(&settings)?;
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

/// Планирует части сообщения для Twitch API (500 символов Unicode, без IRC wire-budget).
fn plan_api_delivery(text: &str) -> Result<Vec<String>, CommandError> {
    let too_long = |message: String| {
        CommandError::new(
            twitch_delivery::error_code::TOO_LONG,
            message,
            ipc::twitch_delivery_error_code_to_retryable(twitch_delivery::error_code::TOO_LONG),
        )
    };
    let clean = crate::twitch::clean_irc_text(text);
    if clean.is_empty() {
        return Err(CommandError::new(
            twitch_delivery::error_code::EMPTY_TEXT,
            "Twitch message must not be empty",
            false,
        ));
    }
    crate::twitch::plan_api_message_parts(&clean).map_err(|_| {
        too_long(format!(
            "Twitch message contains a word longer than {} characters",
            crate::twitch::MAX_MESSAGE_CHARS
        ))
    })
}

/// Планирует части по режиму и параметрам живого клиента.
fn plan_client_delivery(client: &TwitchClient, text: &str) -> Result<Vec<String>, CommandError> {
    match client.mode() {
        crate::config::TwitchMode::Irc => plan_delivery(text, client.channel_name()),
        crate::config::TwitchMode::Api => plan_api_delivery(text),
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
        SendFailure::Rejected(e) => (twitch_delivery::error_code::SEND_FAILED, e.clone()),
        SendFailure::Ambiguous(_) => (
            twitch_delivery::error_code::DELIVERY_UNKNOWN,
            "Twitch delivery outcome is unknown".to_string(),
        ),
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
    // Одно чтение settings: enabled определяет доступность runtime.
    let settings_enabled = {
        let settings = state.twitch.settings.read().await;
        settings.enabled
    };
    let is_connected = matches!(
        state.twitch.connection_status.lock().clone(),
        TwitchConnectionStatus::Connected
    );
    // Клиент клонируется до планирования: его канал — источник истины для
    // wire-budget, а блокировка не удерживается во время планирования/отправки.
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
    let parts = plan_client_delivery(&client, &text)?;
    let total = parts.len();
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

// ── Twitch API authorization commands (ROADMAP-132) ──

fn auth_command_error(error: AuthError) -> CommandError {
    tracing::warn!(
        error_code = error.code(),
        stage = "api_auth",
        retryable = error.retryable(),
        "Twitch API auth command rejected"
    );
    CommandError::new(error.code(), error.message(), error.retryable())
}

async fn auth_coordinator(
    state: &State<'_, AppState>,
) -> Result<Arc<TwitchAuthCoordinator>, CommandError> {
    state.twitch.auth.read().await.clone().ok_or_else(|| {
        CommandError::new(
            "twitch.api_auth.unavailable",
            "Twitch API authorization is not initialized".to_string(),
            false,
        )
    })
}

/// Safe authorization status (no tokens, no secret).
#[tauri::command]
pub async fn get_twitch_api_auth_status(
    state: State<'_, AppState>,
) -> Result<AuthStatusDto, CommandError> {
    let coord = auth_coordinator(&state).await?;
    Ok(coord.status().await)
}

/// Save the personal app client id and optional secret.
#[tauri::command]
pub async fn save_twitch_api_client(
    client_id: String,
    client_secret: Option<String>,
    state: State<'_, AppState>,
) -> Result<AuthStatusDto, CommandError> {
    let coord = auth_coordinator(&state).await?;
    let prev_client_id = coord.status().await.client_id;
    let trimmed_id = client_id.trim().to_string();
    let status = coord
        .save_client(client_id, client_secret)
        .await
        .map_err(auth_command_error)?;
    if state.twitch.settings.read().await.mode == crate::config::TwitchMode::Api
        && (prev_client_id != trimmed_id || trimmed_id.is_empty())
    {
        state.send_twitch_event(crate::events::TwitchEvent::Stop);
    }
    Ok(status)
}

/// Start device authorization for an account role; the UI opens the activation link.
#[tauri::command]
pub async fn begin_twitch_api_auth(
    role: String,
    state: State<'_, AppState>,
) -> Result<BeginAuthResult, CommandError> {
    let role = parse_role(&role).map_err(|_| {
        CommandError::new(
            "twitch.api_auth.invalid_role",
            "Invalid authorization role".to_string(),
            false,
        )
    })?;
    let coord = auth_coordinator(&state).await?;
    coord
        .begin_device_for_role(role)
        .await
        .map_err(auth_command_error)
}

/// Start a broadcaster-only device-code authorization and return an opaque
/// session id plus the Twitch activation URL. The device code itself never
/// leaves the backend. The UI offers explicit copy/open actions for the link.
#[tauri::command]
pub async fn begin_twitch_api_channel_device_auth(
    state: State<'_, AppState>,
) -> Result<BeginAuthResult, CommandError> {
    let coord = auth_coordinator(&state).await?;
    coord.begin_device().await.map_err(auth_command_error)
}

/// Await device confirmation and persist the grant if the session is still current.
#[tauri::command]
pub async fn finish_twitch_api_auth(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<AuthStatusDto, CommandError> {
    let coord = auth_coordinator(&state).await?;
    coord.finish(&session_id).await.map_err(auth_command_error)
}

/// Cancel the active session (optionally only a matching session id).
#[tauri::command]
pub async fn cancel_twitch_api_auth(
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<AuthStatusDto, CommandError> {
    let coord = auth_coordinator(&state).await?;
    let _ = coord.cancel(session_id.as_deref()).await;
    Ok(coord.status().await)
}

/// Forget the bot authorization while retaining channels and client credentials.
#[tauri::command]
pub async fn clear_twitch_api_auth(
    state: State<'_, AppState>,
) -> Result<AuthStatusDto, CommandError> {
    let coord = auth_coordinator(&state).await?;
    let status = coord
        .reset_authorization()
        .await
        .map_err(auth_command_error)?;
    if state.twitch.settings.read().await.mode == crate::config::TwitchMode::Api {
        state.send_twitch_event(crate::events::TwitchEvent::Stop);
    }
    Ok(status)
}

/// Revalidate an enabled API connection after selection, or stop on removal.
/// A manually disabled connection and IRC are never restarted here.
async fn update_api_transport_if_selection_changed(
    service: &crate::twitch::TwitchService,
    previous_selection: Option<String>,
    current_selection: Option<&str>,
    reconnect: bool,
) {
    if previous_selection.as_deref() == current_selection {
        return;
    }
    let settings = service.settings.read().await;
    if settings.mode == crate::config::TwitchMode::Api {
        let event = if reconnect && current_selection.is_some() && settings.enabled {
            crate::events::TwitchEvent::Restart
        } else {
            crate::events::TwitchEvent::Stop
        };
        service.send_event(event);
    }
}

/// Select one authorized channel as the single API delivery recipient.
#[tauri::command]
pub async fn select_twitch_api_channel(
    user_id: String,
    state: State<'_, AppState>,
) -> Result<AuthStatusDto, CommandError> {
    let coord = auth_coordinator(&state).await?;
    let previous = coord.status().await.selected_channel_id;
    let status = coord
        .select_channel(&user_id)
        .await
        .map_err(auth_command_error)?;
    update_api_transport_if_selection_changed(
        &state.twitch,
        previous,
        status.selected_channel_id.as_deref(),
        true,
    )
    .await;
    Ok(status)
}

/// Forget one channel grant locally. If it was the selected channel, the
/// selection is cleared and the API transport is stopped; no revoke request is
/// sent to Twitch.
#[tauri::command]
pub async fn forget_twitch_api_channel(
    user_id: String,
    state: State<'_, AppState>,
) -> Result<AuthStatusDto, CommandError> {
    let coord = auth_coordinator(&state).await?;
    let previous = coord.status().await.selected_channel_id;
    let status = coord
        .forget_channel(&user_id)
        .await
        .map_err(auth_command_error)?;
    update_api_transport_if_selection_changed(
        &state.twitch,
        previous,
        status.selected_channel_id.as_deref(),
        false,
    )
    .await;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn channel_switch_restarts_only_enabled_api_and_removal_stops() {
        use crate::events::TwitchEvent;
        let (tx, mut rx) = tokio::sync::broadcast::channel(16);
        let service = crate::twitch::TwitchService::new(tx);
        {
            let mut settings = service.settings.write().await;
            settings.mode = crate::config::TwitchMode::Api;
            settings.enabled = true;
        }
        update_api_transport_if_selection_changed(&service, Some("old".into()), Some("new"), true)
            .await;
        assert!(matches!(rx.try_recv(), Ok(TwitchEvent::Restart)));
        update_api_transport_if_selection_changed(&service, Some("new".into()), Some("new"), true)
            .await;
        assert!(rx.try_recv().is_err());
        update_api_transport_if_selection_changed(&service, Some("new".into()), None, false).await;
        assert!(matches!(rx.try_recv(), Ok(TwitchEvent::Stop)));
        service.settings.write().await.enabled = false;
        update_api_transport_if_selection_changed(&service, None, Some("new"), true).await;
        assert!(matches!(rx.try_recv(), Ok(TwitchEvent::Stop)));
        service.settings.write().await.mode = crate::config::TwitchMode::Irc;
        update_api_transport_if_selection_changed(&service, Some("old".into()), Some("new"), true)
            .await;
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn connection_validation_exposes_field_code_and_is_not_retryable() {
        let settings = TwitchSettings {
            username: "user".into(),
            token: "token".into(),
            channel: "https://twitch.tv/channel".into(),
            ..TwitchSettings::default()
        };
        let error = validate_connection_settings(&settings).unwrap_err();
        assert_eq!(error.code, "twitch.invalid_channel");
        assert!(!error.retryable);
        let json = serde_json::to_value(error).unwrap();
        assert_eq!(json["code"], "twitch.invalid_channel");
    }

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
    fn api_delivery_rejects_empty_cleaned_text() {
        for text in ["", "   ", "\t\r\n", "\x01\x02 "] {
            let error = plan_api_delivery(text).unwrap_err();
            assert_eq!(error.code, "twitch.empty_text");
            assert!(!error.retryable);
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
        let err = map_part_failure(SendFailure::Ambiguous("write failed".to_string()), 2, 4);
        assert_eq!(err.code, "twitch.partial_delivery");
        assert_eq!(
            err.message,
            "Twitch delivery stopped at message 3 of 4: write failed"
        );
    }

    #[test]
    fn ambiguous_first_part_maps_to_unknown_nonretryable() {
        let err = map_send_failure(SendFailure::Ambiguous("transport".to_string()));
        assert_eq!(err.code, "twitch.delivery_unknown");
        assert!(!err.retryable);
        assert_eq!(err.message, "Twitch delivery outcome is unknown");
    }

    #[test]
    fn rejected_first_part_maps_to_send_failed_retryable() {
        let err = map_send_failure(SendFailure::Rejected("rejected".to_string()));
        assert_eq!(err.code, "twitch.send_failed");
        assert!(err.retryable);
        assert_eq!(err.message, "rejected");
    }

    #[test]
    fn ambiguous_first_part_via_part_failure_stays_unknown_nonretryable() {
        let err = map_part_failure(SendFailure::Ambiguous("transport".to_string()), 0, 3);
        assert_eq!(err.code, "twitch.delivery_unknown");
        assert!(!err.retryable);
    }

    fn client_with_channel(channel: &str) -> TwitchClient {
        TwitchClient::new(crate::twitch::TwitchSettings {
            channel: channel.to_string(),
            ..crate::twitch::TwitchSettings::default()
        })
    }

    #[test]
    fn delivery_plan_uses_live_client_channel_budget() {
        // 490 символов: помещаются в wire-budget короткого канала, но не
        // длинного. Планирование обязано опираться на канал живого клиента,
        // а не на persisted settings.
        let text = "a".repeat(490);

        let short = client_with_channel("x");
        assert!(plan_client_delivery(&short, &text).is_ok());

        let long_channel = "a".repeat(50);
        let long = client_with_channel(&long_channel);
        let err = plan_client_delivery(&long, &text).unwrap_err();
        assert_eq!(err.code, "twitch.too_long");
        assert!(!err.retryable);
    }

    #[test]
    fn delivery_plan_selects_500_char_budget_in_api_mode() {
        let api_client = TwitchClient::new(crate::twitch::TwitchSettings {
            mode: crate::config::TwitchMode::Api,
            ..crate::twitch::TwitchSettings::default()
        });
        // Multi-byte text within 500 characters
        let words = (0..20).map(|_| "тест").collect::<Vec<_>>().join(" ");
        let parts = plan_client_delivery(&api_client, &words).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], words);
    }

    fn temp_settings_manager() -> (SettingsManager, std::path::PathBuf) {
        static NEXT_DIR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-twitch-test-{}-{}-{}",
            std::process::id(),
            unique,
            sequence
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = SettingsManager::with_config_dir(dir.clone()).unwrap();
        (mgr, dir)
    }

    #[tokio::test]
    async fn mode_switch_from_irc_to_api_while_connected_emits_stop_and_sets_runtime_disabled() {
        let (mgr, dir) = temp_settings_manager();
        let initial_irc = TwitchSettings {
            mode: crate::config::TwitchMode::Irc,
            enabled: true,
            username: "bot_user".into(),
            token: "oauth:abc123xyz".into(),
            channel: "mychannel".into(),
            ..TwitchSettings::default()
        };
        mgr.set_twitch_settings(&initial_irc).unwrap();

        let state_settings = tokio::sync::RwLock::new(initial_irc.clone());

        let target_api = TwitchSettings {
            mode: crate::config::TwitchMode::Api,
            enabled: true,
            username: "bot_user".into(),
            token: "oauth:abc123xyz".into(),
            channel: "mychannel".into(),
            ..TwitchSettings::default()
        };

        let (result, event) = save_twitch_settings_core(target_api, &mgr, &state_settings)
            .await
            .unwrap();

        assert_eq!(result, "saved");
        assert!(matches!(event, Some(crate::events::TwitchEvent::Stop)));

        let runtime = state_settings.read().await;
        assert_eq!(runtime.mode, crate::config::TwitchMode::Api);
        assert!(
            !runtime.enabled,
            "Runtime must be disabled upon mode switch"
        );

        let persisted = mgr.load().unwrap().twitch;
        assert_eq!(persisted.mode, crate::config::TwitchMode::Api);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn mode_switch_from_api_to_irc_while_disconnected_emits_stop_and_preserves_disabled() {
        let (mgr, dir) = temp_settings_manager();
        let initial_api = TwitchSettings {
            mode: crate::config::TwitchMode::Api,
            enabled: false,
            ..TwitchSettings::default()
        };
        mgr.set_twitch_settings(&initial_api).unwrap();

        let state_settings = tokio::sync::RwLock::new(initial_api.clone());

        let target_irc = TwitchSettings {
            mode: crate::config::TwitchMode::Irc,
            enabled: false,
            username: "bot_user".into(),
            token: "oauth:abc123xyz".into(),
            channel: "mychannel".into(),
            ..TwitchSettings::default()
        };

        let (result, event) = save_twitch_settings_core(target_irc, &mgr, &state_settings)
            .await
            .unwrap();

        assert_eq!(result, "saved");
        assert!(matches!(event, Some(crate::events::TwitchEvent::Stop)));

        let runtime = state_settings.read().await;
        assert_eq!(runtime.mode, crate::config::TwitchMode::Irc);
        assert!(!runtime.enabled);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn credentials_change_without_mode_change_while_connected_emits_restart() {
        let (mgr, dir) = temp_settings_manager();
        let initial_irc = TwitchSettings {
            mode: crate::config::TwitchMode::Irc,
            enabled: true,
            username: "bot_user".into(),
            token: "oauth:abc123xyz".into(),
            channel: "channel1".into(),
            ..TwitchSettings::default()
        };
        mgr.set_twitch_settings(&initial_irc).unwrap();

        let state_settings = tokio::sync::RwLock::new(initial_irc.clone());

        let mut updated = initial_irc.clone();
        updated.channel = "channel2".into();

        let (result, event) = save_twitch_settings_core(updated, &mgr, &state_settings)
            .await
            .unwrap();

        assert_eq!(result, "saved_reconnecting");
        assert!(matches!(event, Some(crate::events::TwitchEvent::Restart)));

        let runtime = state_settings.read().await;
        assert!(runtime.enabled);
        assert_eq!(runtime.channel, "channel2");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn credentials_change_without_mode_change_while_disconnected_does_not_emit_restart() {
        let (mgr, dir) = temp_settings_manager();
        let initial_irc = TwitchSettings {
            mode: crate::config::TwitchMode::Irc,
            enabled: false,
            username: "bot_user".into(),
            token: "oauth:abc123xyz".into(),
            channel: "channel1".into(),
            ..TwitchSettings::default()
        };
        mgr.set_twitch_settings(&initial_irc).unwrap();

        let state_settings = tokio::sync::RwLock::new(initial_irc.clone());

        let mut updated = initial_irc.clone();
        updated.channel = "channel2".into();

        let (result, event) = save_twitch_settings_core(updated, &mgr, &state_settings)
            .await
            .unwrap();

        assert_eq!(result, "saved");
        assert!(event.is_none());

        let runtime = state_settings.read().await;
        assert!(!runtime.enabled);
        assert_eq!(runtime.channel, "channel2");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn unchanged_settings_save_emits_no_event() {
        let (mgr, dir) = temp_settings_manager();
        let initial_irc = TwitchSettings {
            mode: crate::config::TwitchMode::Irc,
            enabled: true,
            username: "bot_user".into(),
            token: "oauth:abc123xyz".into(),
            channel: "channel1".into(),
            ..TwitchSettings::default()
        };
        mgr.set_twitch_settings(&initial_irc).unwrap();

        let state_settings = tokio::sync::RwLock::new(initial_irc.clone());

        let (result, event) = save_twitch_settings_core(initial_irc.clone(), &mgr, &state_settings)
            .await
            .unwrap();

        assert_eq!(result, "saved");
        assert!(event.is_none());

        let runtime = state_settings.read().await;
        assert!(runtime.enabled);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn failed_mode_switch_persistence_keeps_running_mode() {
        let (mgr, dir) = temp_settings_manager();
        let initial = TwitchSettings {
            mode: crate::config::TwitchMode::Api,
            enabled: true,
            ..TwitchSettings::default()
        };
        mgr.set_twitch_settings(&initial).unwrap();
        let runtime = tokio::sync::RwLock::new(initial.clone());
        // A directory in place of the file deterministically prevents persistence.
        let path = dir.join("settings.json");
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let mut target = initial;
        target.mode = crate::config::TwitchMode::Irc;
        let err = save_twitch_settings_core(target, &mgr, &runtime)
            .await
            .unwrap_err();
        assert_eq!(err.code, "twitch.settings_save_failed");
        let current = runtime.read().await;
        assert_eq!(current.mode, crate::config::TwitchMode::Api);
        assert!(current.enabled);
        assert_eq!(
            mgr.load().unwrap().twitch.mode,
            crate::config::TwitchMode::Api
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn mode_switch_to_unconfigured_irc_stops_and_connect_validation_still_rejects_it() {
        let (mgr, dir) = temp_settings_manager();
        let initial_api = TwitchSettings {
            mode: crate::config::TwitchMode::Api,
            enabled: true,
            ..TwitchSettings::default()
        };
        mgr.set_twitch_settings(&initial_api).unwrap();

        let state_settings = tokio::sync::RwLock::new(initial_api.clone());

        // Selecting an unconfigured mode is allowed; connecting is not.
        let invalid_irc = TwitchSettings {
            mode: crate::config::TwitchMode::Irc,
            username: "".into(),
            token: "".into(),
            channel: "".into(),
            ..TwitchSettings::default()
        };

        let (result, event) = save_twitch_settings_core(invalid_irc.clone(), &mgr, &state_settings)
            .await
            .unwrap();
        assert_eq!(result, "saved");
        assert!(matches!(event, Some(crate::events::TwitchEvent::Stop)));
        assert_eq!(
            mgr.load().unwrap().twitch.mode,
            crate::config::TwitchMode::Irc
        );
        assert_eq!(
            validate_connection_settings(&invalid_irc).unwrap_err().code,
            "twitch.invalid_username"
        );
        assert!(
            save_twitch_settings_core(invalid_irc, &mgr, &state_settings)
                .await
                .is_err()
        );

        let runtime = state_settings.read().await;
        assert_eq!(runtime.mode, crate::config::TwitchMode::Irc);
        assert!(!runtime.enabled);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn save_settings_with_false_in_payload_preserves_runtime_enabled_when_mode_unchanged() {
        let (mgr, dir) = temp_settings_manager();
        let initial_irc = TwitchSettings {
            mode: crate::config::TwitchMode::Irc,
            enabled: false,
            username: "bot_user".into(),
            token: "oauth:abc123xyz".into(),
            channel: "channel1".into(),
            ..TwitchSettings::default()
        };
        mgr.set_twitch_settings(&initial_irc).unwrap();

        let mut runtime_irc = initial_irc.clone();
        runtime_irc.enabled = true;
        let state_settings = tokio::sync::RwLock::new(runtime_irc);

        let incoming_save = TwitchSettings {
            mode: crate::config::TwitchMode::Irc,
            enabled: false,
            username: "bot_user".into(),
            token: "oauth:abc123xyz".into(),
            channel: "channel1".into(),
            start_on_boot: true,
            ..TwitchSettings::default()
        };

        let (result, event) = save_twitch_settings_core(incoming_save, &mgr, &state_settings)
            .await
            .unwrap();

        assert_eq!(result, "saved");
        assert!(event.is_none());

        let runtime = state_settings.read().await;
        assert!(runtime.enabled, "runtime enabled must be preserved");
        assert!(runtime.start_on_boot);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
