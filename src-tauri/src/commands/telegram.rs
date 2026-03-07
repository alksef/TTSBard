use crate::telegram::{TelegramClient, UserInfo, TtsResult, SileroTtsBot, CurrentVoice, Limits, get_current_voice, get_limits};
use crate::config::SettingsManager;
use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Глобальное состояние Telegram клиента
pub struct TelegramState {
    pub client: Arc<Mutex<Option<TelegramClient>>>,
}

impl TelegramState {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for TelegramState {
    fn default() -> Self {
        Self::new()
    }
}

/// Инициализация Telegram клиента
#[tauri::command]
pub async fn telegram_init(
    state: State<'_, TelegramState>,
    api_id: u32,
    api_hash: String,
    phone: String,
) -> Result<(), String> {
    // Валидация входных данных
    if api_id == 0 {
        return Err("API ID не может быть пустым".to_string());
    }
    if api_hash.trim().is_empty() {
        return Err("API Hash не может быть пустым".to_string());
    }
    if phone.trim().is_empty() {
        return Err("Номер телефона не может быть пустым".to_string());
    }

    // Создаем новый клиент
    let client = TelegramClient::new();

    // Инициализируем - игнорируем результат операции, проверяем только на ошибки
    client.init(api_id, api_hash, phone).await
        .map_err(|e| format!("Ошибка инициализации клиента: {}", e))?;

    // Сохраняем клиент в состоянии
    let mut state_guard = state.client.lock().await;
    *state_guard = Some(client);

    Ok(())
}

/// Запрос кода подтверждения
#[tauri::command]
pub async fn telegram_request_code(
    state: State<'_, TelegramState>,
) -> Result<(), String> {
    let client_guard = state.client.lock().await;
    let client = client_guard.as_ref()
        .ok_or_else(|| "Клиент не инициализирован. Сначала вызовите telegram_init.".to_string())?;

    client.request_code().await
        .map_err(|e| format!("Ошибка запроса кода: {}", e))?;
    Ok(())
}

/// Ввод кода подтверждения
#[tauri::command]
pub async fn telegram_sign_in(
    state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
    code: String,
) -> Result<(), String> {
    if code.trim().is_empty() {
        return Err("Код не может быть пустым".to_string());
    }

    let client_guard = state.client.lock().await;
    let client = client_guard.as_ref()
        .ok_or_else(|| "Клиент не инициализирован. Сначала вызовите telegram_init.".to_string())?;

    client.sign_in(&code).await
        .map_err(|e| format!("Ошибка входа: {}", e))?;

    // После успешного входа сохраняем api_id в settings.json
    let api_id_to_save = {
        let api_id_lock = client.api_id.lock().await;
        match &*api_id_lock {
            Some(id) => *id,
            None => return Err("api_id not set".to_string()),
        }
    };
    drop(client_guard);

    // Save to settings.json (convert u32 to i64)
    settings_manager.set_telegram_api_id(Some(api_id_to_save as i64))
        .map_err(|e| format!("Failed to save api_id: {}", e))?;

    Ok(())
}

/// Выход из аккаунта
#[tauri::command]
pub async fn telegram_sign_out(
    state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    let client_guard = state.client.lock().await;
    let client = client_guard.as_ref()
        .ok_or_else(|| "Клиент не инициализирован".to_string())?;

    client.sign_out().await
        .map_err(|e| format!("Ошибка выхода: {}", e))?;

    // Удаляем клиент из состояния
    drop(client_guard);
    let mut state_guard = state.client.lock().await;
    *state_guard = None;

    // Удаляем сохранённый api_id из settings.json
    settings_manager.set_telegram_api_id(None)
        .map_err(|e| format!("Failed to delete api_id: {}", e))?;

    Ok(())
}

/// Проверка статуса авторизации
#[tauri::command]
pub async fn telegram_get_status(
    state: State<'_, TelegramState>,
) -> Result<bool, String> {
    let client_guard = state.client.lock().await;
    let client = match client_guard.as_ref() {
        Some(c) => c,
        None => return Ok(false), // Неинициализирован = не авторизован
    };

    client.is_authorized().await
}

/// Получение информации о пользователе
#[tauri::command]
pub async fn telegram_get_user(
    state: State<'_, TelegramState>,
) -> Result<Option<UserInfo>, String> {
    let client_guard = state.client.lock().await;
    let client = match client_guard.as_ref() {
        Some(c) => c,
        None => return Ok(None), // Неинициализирован = нет пользователя
    };

    // Проверяем авторизацию
    let is_authorized = client.is_authorized().await?;
    if !is_authorized {
        return Ok(None);
    }

    // Получаем информацию о пользователе
    let user_info = client.get_user_info().await?;
    Ok(Some(user_info))
}

/// Синтез речи через Silero TTS (Telegram bot)
#[tauri::command]
pub async fn speak_text_silero(
    state: State<'_, TelegramState>,
    text: String,
) -> Result<TtsResult, String> {
    println!("[SILERO COMMAND] Starting TTS for text: '{}'", text);

    // Валидация
    let text = text.trim();
    if text.is_empty() {
        return Ok(TtsResult::error("Text cannot be empty".to_string()));
    }

    // Получаем клиент
    let client_guard = state.client.lock().await;
    let client = client_guard
        .as_ref()
        .ok_or_else(|| {
            "Telegram client not initialized. Please connect to Telegram first.".to_string()
        })?;

    // Проверяем авторизацию
    let is_authorized = client.is_authorized().await?;
    if !is_authorized {
        return Ok(TtsResult::error(
            "Not authorized in Telegram. Please sign in first.".to_string(),
        ));
    }

    // Выполняем синтез
    let result = SileroTtsBot::synthesize(client, text).await?;

    Ok(result)
}

/// Получить текущий голос Silero TTS
#[tauri::command]
pub async fn telegram_get_current_voice(
    state: State<'_, TelegramState>,
) -> Result<Option<CurrentVoice>, String> {
    println!("[TELEGRAM COMMAND] Getting current voice");

    // Получаем клиент
    let client_guard = state.client.lock().await;
    let client = client_guard
        .as_ref()
        .ok_or_else(|| {
            "Telegram client not initialized. Please connect to Telegram first.".to_string()
        })?;

    // Проверяем авторизацию
    let is_authorized = client.is_authorized().await?;
    if !is_authorized {
        return Ok(None);
    }

    // Получаем текущий голос (может вернуть None при таймауте)
    let voice = get_current_voice(client).await?;

    Ok(voice)
}

/// Получить лимиты Silero TTS
#[tauri::command]
pub async fn telegram_get_limits(
    state: State<'_, TelegramState>,
) -> Result<Option<Limits>, String> {
    println!("[TELEGRAM COMMAND] Getting limits");

    // Получаем клиент
    let client_guard = state.client.lock().await;
    let client = client_guard
        .as_ref()
        .ok_or_else(|| {
            "Telegram client not initialized. Please connect to Telegram first.".to_string()
        })?;

    // Проверяем авторизацию
    let is_authorized = client.is_authorized().await?;
    if !is_authorized {
        return Ok(None);
    }

    // Получаем лимиты (может вернуть None при таймауте)
    let limits = get_limits(client).await?;

    Ok(limits)
}

/// Автоматически восстановить сессию при старте приложения
#[tauri::command]
pub async fn telegram_auto_restore(
    state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<bool, String> {
    println!("[TELEGRAM] Auto-restoring session...");

    // Загружаем сохранённый api_id из settings.json
    let api_id = match settings_manager.get_telegram_api_id() {
        Some(id) => id as u32, // Convert i64 to u32
        None => {
            println!("[TELEGRAM] No saved api_id found");
            return Ok(false);
        }
    };

    println!("[TELEGRAM] Found saved api_id: {}", api_id);

    // Создаём новый клиент
    let client = TelegramClient::new();

    // Инициализируем с пустыми данными (сессия уже есть)
    client.init_empty(api_id).await
        .map_err(|e| format!("Ошибка инициализации сессии: {}", e))?;

    // Сохраняем клиент в состоянии
    let mut state_guard = state.client.lock().await;
    *state_guard = Some(client);

    // Проверяем авторизацию
    drop(state_guard);
    let client_guard = state.client.lock().await;
    let is_authorized = if let Some(c) = client_guard.as_ref() {
        c.is_authorized().await?
    } else {
        false
    };

    if is_authorized {
        println!("[TELEGRAM] Session auto-restored successfully");
        Ok(true)
    } else {
        println!("[TELEGRAM] Session exists but not authorized");
        Ok(false)
    }
}
