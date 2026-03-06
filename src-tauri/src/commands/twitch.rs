use crate::settings::AppSettings;
use crate::state::AppState;
use crate::twitch::TwitchSettings;
use tauri::{State, Manager};

/// Получить текущие настройки Twitch
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
    eprintln!("[TWITCH] Saving settings: enabled={}, start_on_boot={}, channel={}",
        settings.enabled, settings.start_on_boot, settings.channel);

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

    // Сохранить в AppState
    let mut s = state.twitch_settings.write().await;
    *s = settings.clone();
    drop(s);

    // Сохранить в файлы
    AppSettings::save_twitch_settings(&settings)
        .map_err(|e| format!("Failed to save Twitch settings: {}", e))?;

    // Отправить событие для перезапуска клиента только если есть изменения
    if enabled_changed || credentials_changed {
        if let Some(state) = app_handle.try_state::<AppState>() {
            state.send_twitch_event(crate::events::TwitchEvent::Restart);
        }
        Ok("Настройки сохранены. Переподключение...".to_string())
    } else {
        Ok("Настройки сохранены.".to_string())
    }
}

/// Подключиться к Twitch
#[tauri::command]
pub async fn connect_twitch(
    state: State<'_, AppState>,
) -> Result<String, String> {
    eprintln!("[TWITCH] Connect command received");

    // Получаем текущие настройки
    let settings = state.twitch_settings.read().await;

    // Валидация
    if let Err(e) = settings.is_valid() {
        return Err(format!("Settings invalid: {}", e));
    }
    drop(settings);

    // Обновляем enabled и сохраняем
    let mut s = state.twitch_settings.write().await;
    s.enabled = true;
    let settings_to_save = s.clone();
    drop(s);

    // Сохраняем в файл
    AppSettings::save_twitch_settings(&settings_to_save)
        .map_err(|e| format!("Failed to save Twitch settings: {}", e))?;

    // Отправляем событие подключения
    state.send_twitch_event(crate::events::TwitchEvent::Restart);

    Ok("Подключение к Twitch...".to_string())
}

/// Отключиться от Twitch
#[tauri::command]
pub async fn disconnect_twitch(
    state: State<'_, AppState>,
) -> Result<String, String> {
    eprintln!("[TWITCH] Disconnect command received");

    // Обновляем enabled и сохраняем
    let mut s = state.twitch_settings.write().await;
    s.enabled = false;
    let settings_to_save = s.clone();
    drop(s);

    // Сохраняем в файл
    AppSettings::save_twitch_settings(&settings_to_save)
        .map_err(|e| format!("Failed to save Twitch settings: {}", e))?;

    // Отправляем событие отключения
    state.send_twitch_event(crate::events::TwitchEvent::Stop);

    Ok("Отключено от Twitch".to_string())
}

/// Получить текущий статус подключения Twitch
#[tauri::command]
pub async fn get_twitch_status(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let status = state.twitch_connection_status.lock()
        .map_err(|e| format!("Failed to get status: {}", e))?
        .clone();

    let status_str = match status {
        crate::events::TwitchConnectionStatus::Connected => "Connected",
        crate::events::TwitchConnectionStatus::Connecting => "Connecting",
        crate::events::TwitchConnectionStatus::Disconnected => "Disconnected",
        crate::events::TwitchConnectionStatus::Error(_) => "Error",
    };

    Ok(status_str.to_string())
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
