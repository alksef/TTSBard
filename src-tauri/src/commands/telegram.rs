use crate::telegram::{TelegramClient, UserInfo, TtsResult, SileroTtsBot, CurrentVoice, Limits, get_current_voice, get_limits, ProxyStatus};
use crate::config::{SettingsManager, ProxyMode};
use tauri::State;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::info;

/// Глобальное состояние Telegram клиента
pub struct TelegramState {
    pub client: Arc<Mutex<Option<TelegramClient>>>,
    /// Cached proxy status (updated on connection changes)
    proxy_status: Arc<RwLock<ProxyStatus>>,
}

impl TelegramState {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            proxy_status: Arc::new(RwLock::new(ProxyStatus::default())),
        }
    }

    /// Get cached proxy status (fast, no lock on client)
    pub async fn get_proxy_status_cached(&self) -> ProxyStatus {
        self.proxy_status.read().await.clone()
    }

    /// Update cached proxy status
    pub async fn update_proxy_status(&self, status: ProxyStatus) {
        *self.proxy_status.write().await = status;
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
    settings_manager: State<'_, SettingsManager>,
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

    // Load proxy settings
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // Get proxy URL based on proxy mode
    let proxy_url = match settings.tts.telegram.proxy_mode {
        ProxyMode::None => None,
        ProxyMode::Socks5 => settings.tts.network.proxy.proxy_url.clone(),
    };

    info!(proxy_mode = ?settings.tts.telegram.proxy_mode, has_proxy = proxy_url.is_some(), "Initializing Telegram with proxy settings");

    // Создаем новый клиент
    let client = TelegramClient::new();

    // Инициализируем с поддержкой прокси
    client.init_with_proxy(api_id, api_hash, phone, proxy_url).await
        .map_err(|e| format!("Ошибка инициализации клиента: {}", e))?;

    // Сохраняем клиент в состоянии
    {
        let mut state_guard = state.client.lock().await;
        *state_guard = Some(client);
    }

    // Update cached proxy status
    let client_guard = state.client.lock().await;
    if let Some(c) = client_guard.as_ref() {
        let proxy_status = c.get_proxy_status().await;
        state.update_proxy_status(proxy_status).await;
    }

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
    tracing::debug!(text, "Starting TTS");

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
    tracing::debug!("Getting current voice");

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
    tracing::debug!("Getting limits");

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
    info!("Auto-restoring session...");

    // Загружаем сохранённый api_id из settings.json
    let api_id = match settings_manager.get_telegram_api_id() {
        Some(id) => id as u32, // Convert i64 to u32
        None => {
            info!("No saved api_id found");
            return Ok(false);
        }
    };

    info!(api_id, "Found saved api_id");

    // Load proxy settings
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // Get proxy URL based on proxy mode
    let proxy_url = match settings.tts.telegram.proxy_mode {
        ProxyMode::None => None,
        ProxyMode::Socks5 => settings.tts.network.proxy.proxy_url.clone(),
    };

    info!(proxy_mode = ?settings.tts.telegram.proxy_mode, has_proxy = proxy_url.is_some(), "Restoring session with proxy settings");

    // Создаём новый клиент
    let client = TelegramClient::new();

    // Инициализируем с пустыми данными (сессия уже есть) и поддержкой прокси
    client.init_empty_with_proxy(api_id, proxy_url).await
        .map_err(|e| format!("Ошибка инициализации сессии: {}", e))?;

    // Сохраняем клиент в состоянии
    let mut state_guard = state.client.lock().await;
    *state_guard = Some(client);

    // Update cached proxy status
    if let Some(c) = state_guard.as_ref() {
        let proxy_status = c.get_proxy_status().await;
        state.update_proxy_status(proxy_status).await;
    }

    // Проверяем авторизацию
    let is_authorized = if let Some(c) = state_guard.as_ref() {
        c.is_authorized().await?
    } else {
        false
    };

    if is_authorized {
        info!("Session auto-restored successfully");
        Ok(true)
    } else {
        info!("Session exists but not authorized");
        Ok(false)
    }
}
