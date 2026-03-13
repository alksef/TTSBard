use crate::config::{SettingsManager, TwitchSettings};
use crate::state::AppState;
use tauri::{State, Manager};

/// Получить текущие настройки Twitch (включая токен)
#[tauri::command]
pub async fn get_twitch_settings(
    state: State<'_, AppState>,
) -> Result<TwitchSettings, String> {
    let settings = state.twitch_settings.read().await;
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
    let old_settings = state.twitch_settings.read().await;
    let enabled_changed = old_settings.enabled != settings.enabled;
    let credentials_changed = old_settings.username != settings.username
        || old_settings.token != settings.token
        || old_settings.channel != settings.channel;
    drop(old_settings);

    // Транзакционный подход: сначала сохраняем в файл, потом в память
    // Это предотвращает рассинхронизацию, если другой поток прочитает настройки между операциями
    // Получаем SettingsManager один раз
    let settings_manager = app_handle.try_state::<SettingsManager>();
    if let Some(manager) = settings_manager {
        manager.set_twitch_settings(&settings)
            .map_err(|e| format!("Failed to save Twitch settings: {}", e))?;
    }

    // Только после успешного сохранения в файл обновляем AppState
    let mut s = state.twitch_settings.write().await;
    *s = settings.clone();
    drop(s);

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
pub async fn connect_twitch(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    tracing::info!("Connect command received");

    // Получаем текущие настройки
    let settings = state.twitch_settings.read().await;

    // Валидация
    if let Err(e) = settings.is_valid() {
        return Err(format!("Settings invalid: {}", e));
    }
    drop(settings);

    // Обновляем enabled и клонируем настройки для сохранения
    let mut s = state.twitch_settings.write().await;
    s.enabled = true;
    let settings_to_save = s.clone();
    drop(s);

    // Получаем SettingsManager один раз и сохраняем в файл
    let settings_manager = app_handle.try_state::<SettingsManager>();
    if let Some(manager) = settings_manager {
        manager.set_twitch_settings(&settings_to_save)
            .map_err(|e| format!("Failed to save Twitch settings: {}", e))?;
    }

    // AppState уже обновлен (enabled=true), файл синхронизирован

    // Отправляем событие подключения
    state.send_twitch_event(crate::events::TwitchEvent::Restart);

    Ok("Подключение к Twitch...".to_string())
}

/// Отключиться от Twitch
#[tauri::command]
pub async fn disconnect_twitch(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    tracing::info!("Disconnect command received");

    // Обновляем enabled и клонируем настройки для сохранения
    let mut s = state.twitch_settings.write().await;
    s.enabled = false;
    let settings_to_save = s.clone();
    drop(s);

    // Получаем SettingsManager один раз и сохраняем в файл
    let settings_manager = app_handle.try_state::<SettingsManager>();
    if let Some(manager) = settings_manager {
        manager.set_twitch_settings(&settings_to_save)
            .map_err(|e| format!("Failed to save Twitch settings: {}", e))?;
    }

    // AppState уже обновлен (enabled=false), файл синхронизирован

    // Отправляем событие отключения
    state.send_twitch_event(crate::events::TwitchEvent::Stop);

    Ok("Отключено от Twitch".to_string())
}

/// Получить текущий статус подключения Twitch
#[tauri::command]
pub async fn get_twitch_status(
    state: State<'_, AppState>,
) -> Result<crate::events::TwitchConnectionStatus, String> {
    let status = state.twitch_connection_status.lock().clone();
    Ok(status)
}

/// Проверить подключение к Twitch
#[tauri::command]
pub async fn test_twitch_connection(
    settings: TwitchSettings,
) -> Result<String, String> {
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
pub async fn send_twitch_test_message(
    state: State<'_, AppState>,
) -> Result<String, String> {
    state.send_twitch_event(crate::events::TwitchEvent::SendMessage("test message".to_string()));
    Ok("Тестовое сообщение отправлено".to_string())
}
