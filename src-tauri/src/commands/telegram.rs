use crate::telegram::{TelegramClient, UserInfo, TtsResult, SileroTtsBot, CurrentVoice, Limits, get_current_voice, get_limits, ProxyStatus, types::VoiceCode, bot::set_speaker};
use crate::config::{SettingsManager, ProxyMode};
use tauri::{State, AppHandle};
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

    info!(proxy_mode = ?settings.tts.telegram.proxy_mode, "Initializing Telegram with proxy settings");

    // Создаем новый клиент
    let client = TelegramClient::new();

    // Инициализируем с поддержкой выбранного прокси
    match &settings.tts.telegram.proxy_mode {
        ProxyMode::None => {
            info!("Initializing without proxy");
            client.init_with_proxy(api_id, api_hash, phone, None).await
                .map_err(|e| format!("Ошибка инициализации клиента: {}", e))?;
        }
        ProxyMode::Socks5 => {
            let proxy_url = settings.tts.network.proxy.proxy_url.clone();
            info!(has_proxy = proxy_url.is_some(), "Initializing with SOCKS5 proxy");
            client.init_with_proxy(api_id, api_hash, phone, proxy_url).await
                .map_err(|e| format!("Ошибка инициализации клиента: {}", e))?;
        }
        ProxyMode::MtProxy => {
            #[cfg(feature = "mtproxy")]
            {
                info!("Initializing with MTProxy");
                client.init_with_mtproxy(api_id, api_hash, phone, &settings.tts.network.mtproxy).await
                    .map_err(|e| format!("Ошибка инициализации клиента: {}", e))?;
            }
            #[cfg(not(feature = "mtproxy"))]
            {
                return Err("MTProxy feature is not enabled".to_string());
            }
        }
    }

    // Сохраняем клиент в состоянии
    let client_clone = {
        let mut state_guard = state.client.lock().await;
        *state_guard = Some(client.clone());
        client
    };

    // Update cached proxy status
    let proxy_status = client_clone.get_proxy_status().await;
    state.update_proxy_status(proxy_status).await;

    Ok(())
}

/// Запрос кода подтверждения
#[tauri::command]
pub async fn telegram_request_code(
    state: State<'_, TelegramState>,
) -> Result<(), String> {
    let client_opt = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let client = client_opt.ok_or_else(|| "Клиент не инициализирован. Сначала вызовите telegram_init.".to_string())?;

    client.request_code().await
        .map_err(|e| format!("Ошибка запроса кода: {}", e))?;
    Ok(())
}

/// Ввод кода подтверждения
#[tauri::command]
pub async fn telegram_sign_in(
    state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
    code: String,
) -> Result<(), String> {
    if code.trim().is_empty() {
        return Err("Код не может быть пустым".to_string());
    }

    let client_opt = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let client = client_opt.ok_or_else(|| "Клиент не инициализирован. Сначала вызовите telegram_init.".to_string())?;

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

    // Save to settings.json (convert u32 to i64)
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_telegram_api_id(Some(api_id_to_save as i64))
    }).await?;

    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Выход из аккаунта
#[tauri::command]
pub async fn telegram_sign_out(
    state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let client_opt = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let client = client_opt.ok_or_else(|| "Клиент не инициализирован".to_string())?;

    client.sign_out().await
        .map_err(|e| format!("Ошибка выхода: {}", e))?;

    // Удаляем клиент из состояния
    let mut state_guard = state.client.lock().await;
    *state_guard = None;

    // Удаляем сохранённый api_id из settings.json
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_telegram_api_id(None)
    }).await?;

    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Проверка статуса авторизации
#[tauri::command]
pub async fn telegram_get_status(
    state: State<'_, TelegramState>,
) -> Result<bool, String> {
    let client_opt = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let client = match client_opt {
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
    let client_opt = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let client = match client_opt {
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
    let client_opt = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let client = client_opt.ok_or_else(|| {
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
    let result = SileroTtsBot::synthesize(&client, text).await?;

    Ok(result)
}

/// Получить текущий голос Silero TTS
#[tauri::command]
pub async fn telegram_get_current_voice(
    state: State<'_, TelegramState>,
) -> Result<Option<CurrentVoice>, String> {
    tracing::debug!("Getting current voice");

    // Получаем клиент
    let client_opt = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let client = client_opt.ok_or_else(|| {
        "Telegram client not initialized. Please connect to Telegram first.".to_string()
    })?;

    // Проверяем авторизацию
    let is_authorized = client.is_authorized().await?;
    if !is_authorized {
        return Ok(None);
    }

    // Получаем текущий голос (может вернуть None при таймауте)
    let voice = get_current_voice(&client).await?;

    Ok(voice)
}

/// Получить лимиты Silero TTS
#[tauri::command]
pub async fn telegram_get_limits(
    state: State<'_, TelegramState>,
) -> Result<Option<Limits>, String> {
    tracing::debug!("Getting limits");

    // Получаем клиент
    let client_opt = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let client = client_opt.ok_or_else(|| {
        "Telegram client not initialized. Please connect to Telegram first.".to_string()
    })?;

    // Проверяем авторизацию
    let is_authorized = client.is_authorized().await?;
    if !is_authorized {
        return Ok(None);
    }

    // Получаем лимиты (может вернуть None при таймауте)
    let limits = get_limits(&client).await?;

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

    info!(proxy_mode = ?settings.tts.telegram.proxy_mode, "Restoring session with proxy settings");

    // Создаём новый клиент
    let client = TelegramClient::new();

    // Инициализируем с пустыми данными (сессия уже есть) и поддержкой выбранного прокси
    match &settings.tts.telegram.proxy_mode {
        ProxyMode::None => {
            info!("Initializing without proxy");
            client.init_empty(api_id).await
                .map_err(|e| format!("Ошибка инициализации сессии: {}", e))?;
        }
        ProxyMode::Socks5 => {
            let proxy_url = settings.tts.network.proxy.proxy_url.clone();
            info!(has_proxy = proxy_url.is_some(), "Initializing with SOCKS5 proxy");
            client.init_empty_with_proxy(api_id, proxy_url).await
                .map_err(|e| format!("Ошибка инициализации сессии: {}", e))?;
        }
        ProxyMode::MtProxy => {
            #[cfg(feature = "mtproxy")]
            {
                info!("Initializing with MTProxy");
                client.init_empty_with_mtproxy(api_id, &settings.tts.network.mtproxy).await
                    .map_err(|e| format!("Ошибка инициализации сессии: {}", e))?;
            }
            #[cfg(not(feature = "mtproxy"))]
            {
                return Err("MTProxy feature is not enabled".to_string());
            }
        }
    }

    // Сохраняем клиент в состоянии
    let client_clone = {
        let mut state_guard = state.client.lock().await;
        *state_guard = Some(client.clone());
        client
    };

    // Update cached proxy status
    let proxy_status = client_clone.get_proxy_status().await;
    state.update_proxy_status(proxy_status).await;

    // Проверяем авторизацию
    let is_authorized = client_clone.is_authorized().await?;

    if is_authorized {
        info!("Session auto-restored successfully");
        Ok(true)
    } else {
        info!("Session exists but not authorized");
        Ok(false)
    }
}

/// Установить голос, отправив "/speaker {code}" боту
#[tauri::command]
pub async fn telegram_set_speaker(
    state: State<'_, TelegramState>,
    voice_code: String,
) -> Result<bool, String> {
    // 1. Валидация voice_code (не пустой)
    if voice_code.trim().is_empty() {
        return Err("Voice code cannot be empty".to_string());
    }

    // 2. Получить клиент из state
    let client_opt = {
        let guard = state.client.lock().await;
        guard.clone()
    };
    let client = client_opt.ok_or_else(|| "Telegram client not initialized".to_string())?;

    // 3. Вызвать bot::set_speaker()
    set_speaker(&client, &voice_code).await
}

/// Добавить голос в список сохраненных
#[tauri::command]
pub async fn telegram_add_voice_code(
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
    voice: VoiceCode,
) -> Result<(), String> {
    if voice.id.trim().is_empty() {
        return Err("Voice ID cannot be empty".to_string());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        let mut settings = mgr.load()
            .map_err(|e| anyhow::anyhow!("Failed to load settings: {}", e))?;

        if settings.tts.telegram.voices.iter().any(|v| v.id == voice.id) {
            return Err(anyhow::anyhow!("Voice with this ID already exists"));
        }

        settings.tts.telegram.voices.push(voice);
        mgr.save(&settings)
    }).await?;

    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Удалить голос из списка сохраненных
#[tauri::command]
pub async fn telegram_remove_voice_code(
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
    voice_id: String,
) -> Result<(), String> {
    let vid = voice_id.clone();
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        let mut settings = mgr.load()
            .map_err(|e| anyhow::anyhow!("Failed to load settings: {}", e))?;

        settings.tts.telegram.voices.retain(|v| v.id != vid);

        if settings.tts.telegram.current_voice_id == vid {
            settings.tts.telegram.current_voice_id.clear();
        }

        mgr.save(&settings)
    }).await?;

    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Выбрать голос из списка
#[tauri::command]
pub async fn telegram_select_voice(
    state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
    voice_id: String,
) -> Result<bool, String> {
    // 1. Отправить "/speaker {voice_id}" боту
    let client_guard = state.client.lock().await;
    let client = client_guard.as_ref()
        .ok_or_else(|| "Telegram client not initialized".to_string())?;

    let success = set_speaker(client, &voice_id).await?;
    drop(client_guard);

    // 2. Если успешно - обновить current_voice_id
    if success {
        let vid = voice_id.clone();
        super::persist_blocking(settings_manager.inner(), move |mgr| {
            let mut settings = mgr.load()
                .map_err(|e| anyhow::anyhow!("Failed to load settings: {}", e))?;

            settings.tts.telegram.current_voice_id = vid;

            mgr.save(&settings)
        }).await?;

        super::emit_settings_changed(&app_handle);
    }

    // 3. Вернуть результат
    Ok(success)
}
