use crate::config::{SettingsManager, TwitchSettings};
use crate::events::TwitchConnectionStatus;
use crate::ipc::{self, twitch_delivery, CommandError};
use crate::state::AppState;
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

    // Проверка изменений
    let old_settings = state.twitch.settings.read().await;
    let enabled_changed = old_settings.enabled != settings.enabled;
    let credentials_changed = old_settings.username != settings.username
        || old_settings.token != settings.token
        || old_settings.channel != settings.channel;
    drop(old_settings);

    // Транзакционный подход: сначала сохраняем в файл, потом в память
    // Это предотвращает рассинхронизацию, если другой поток прочитает настройки между операциями
    // Получаем SettingsManager один раз
    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;
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
        Ok("Настройки сохранены. Переподключение...".to_string())
    } else {
        Ok("Настройки сохранены.".to_string())
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

    Ok("Подключение к Twitch...".to_string())
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

    Ok("Отключено от Twitch".to_string())
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

/// Отправить тестовое сообщение в Twitch чат
#[tauri::command]
pub async fn send_twitch_test_message(state: State<'_, AppState>) -> Result<String, String> {
    state.send_twitch_event(crate::events::TwitchEvent::SendMessage(
        "test message".to_string(),
    ));
    Ok("Тестовое сообщение отправлено".to_string())
}

/// Перезапустить Twitch клиент
#[tauri::command]
pub async fn restart_twitch(state: State<'_, AppState>) -> Result<String, String> {
    tracing::info!("Restart command received");
    state.send_twitch_event(crate::events::TwitchEvent::Restart);
    Ok("Перезапуск Twitch...".to_string())
}

/// Successful Twitch-only delivery result.
#[derive(Debug, Clone, Serialize)]
pub struct DeliveredTwitchMessage {
    pub status: &'static str,
}

/// Deliver a pre-processed message directly to the connected Twitch client.
///
/// This is the tracked Twitch-only route: it does not create a speech job,
/// does not write to phrase history and does not trigger WebView.
#[tauri::command]
pub async fn deliver_twitch_message(
    state: State<'_, AppState>,
    text: String,
) -> Result<DeliveredTwitchMessage, CommandError> {
    if text.trim().is_empty() {
        return Err(CommandError::new(
            twitch_delivery::error_code::EMPTY_TEXT,
            "Twitch message must not be empty".to_string(),
            ipc::twitch_delivery_error_code_to_retryable(twitch_delivery::error_code::EMPTY_TEXT),
        ));
    }

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
    match client.send_message(&text).await {
        Ok(()) => Ok(DeliveredTwitchMessage { status: "delivered" }),
        Err(e) => Err(CommandError::new(
            twitch_delivery::error_code::SEND_FAILED,
            e.to_string(),
            ipc::twitch_delivery_error_code_to_retryable(
                twitch_delivery::error_code::SEND_FAILED,
            ),
        )),
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
}
