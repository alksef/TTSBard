use super::types::{AuthState, OperationResult, UserInfo};
use grammers_client::{Client, SignInError};
use grammers_client::types::LoginToken;
use grammers_mtsender::{SenderPool, ConnectionParams};
use grammers_session::updates::UpdatesLike;
use grammers_session::storages::SqliteSession;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, debug, warn};
use crate::config::ProxyMode;

// NOTE: api_id is now stored in settings.json (settings.tts.telegram.api_id)
// The old telegram_config.json file is no longer used

/// Current proxy status for Telegram connection
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ProxyStatus {
    /// Current proxy mode
    pub mode: ProxyMode,
    /// Proxy URL being used (if any)
    pub proxy_url: Option<String>,
}

impl fmt::Display for ProxyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.mode {
            ProxyMode::None => write!(f, "No proxy"),
            ProxyMode::Socks5 => write!(f, "SOCKS5 proxy: {}", self.proxy_url.as_deref().unwrap_or("not configured")),
        }
    }
}

pub struct TelegramClient {
    pub(crate) pool_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub(crate) client: Arc<Mutex<Option<Client>>>,
    pub(crate) updates: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<UpdatesLike>>>>,
    login_token: Arc<Mutex<Option<LoginToken>>>,
    pub(crate) api_id: Arc<Mutex<Option<u32>>>,
    api_hash: Arc<Mutex<Option<String>>>,
    phone_number: Arc<Mutex<Option<String>>>,
    session_path: Arc<Mutex<Option<PathBuf>>>,
    /// Current proxy status
    proxy_status: Arc<Mutex<ProxyStatus>>,
}

impl fmt::Debug for TelegramClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelegramClient")
            .field("session_path", &self.session_path)
            .finish()
    }
}

impl TelegramClient {
    pub fn new() -> Self {
        Self {
            pool_task: Arc::new(Mutex::new(None)),
            client: Arc::new(Mutex::new(None)),
            updates: Arc::new(Mutex::new(None)),
            login_token: Arc::new(Mutex::new(None)),
            api_id: Arc::new(Mutex::new(None)),
            api_hash: Arc::new(Mutex::new(None)),
            phone_number: Arc::new(Mutex::new(None)),
            session_path: Arc::new(Mutex::new(None)),
            proxy_status: Arc::new(Mutex::new(ProxyStatus::default())),
        }
    }

    /// Get session path in %APPDATA%\ttsbard\telegram.session
    fn get_session_path() -> Result<PathBuf, String> {
        let appdata = std::env::var("APPDATA")
            .map_err(|e| format!("Failed to get APPDATA: {}", e))?;

        let app_dir = std::path::Path::new(&appdata).join("ttsbard");

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&app_dir)
            .map_err(|e| format!("Failed to create app directory: {}", e))?;

        Ok(app_dir.join("telegram.session"))
    }

    /// Validate and normalize proxy URL
    ///
    /// Supports:
    /// - SOCKS5: socks5://host:port or socks5://user:pass@host:port
    fn validate_proxy_url(url: &str, mode: &ProxyMode) -> Result<String, String> {
        match mode {
            ProxyMode::None => Err("Proxy mode is None but URL was provided".to_string()),
            ProxyMode::Socks5 => {
                // Validate SOCKS5 URL format: socks5://[user:pass@]host:port
                if !url.starts_with("socks5://") {
                    return Err(format!("Invalid SOCKS5 URL: must start with socks5://"));
                }

                let url_without_scheme = &url[9..]; // Remove "socks5://"

                // Split into auth part and address part
                let (_auth, address) = match url_without_scheme.find('@') {
                    Some(at_idx) => {
                        let (user_pass, addr) = url_without_scheme.split_at(at_idx);
                        (Some(user_pass), &addr[1..]) // Skip '@'
                    }
                    None => (None, url_without_scheme),
                };

                // Split address into host and port
                let (host, port_str) = address.rsplit_once(':')
                    .ok_or_else(|| format!("Invalid SOCKS5 URL: missing port"))?;

                let _port: u16 = port_str.parse()
                    .map_err(|_| format!("Invalid SOCKS5 URL: invalid port"))?;

                info!(host, _port, has_auth = _auth.is_some(), "Validated SOCKS5 proxy");

                Ok(url.to_string())
            }
        }
    }

    /// Инициализация клиента
    #[allow(dead_code)]
    pub async fn init(&self, api_id: u32, api_hash: String, phone: String) -> Result<OperationResult, String> {
        self.init_with_proxy(api_id, api_hash, phone, None).await
    }

    /// Инициализация клиента с поддержкой прокси
    pub async fn init_with_proxy(&self, api_id: u32, api_hash: String, phone: String, proxy_url: Option<String>) -> Result<OperationResult, String> {
        // Determine proxy mode from URL
        let (proxy_mode, proxy_url_str) = if let Some(ref url) = &proxy_url {
            let mode = ProxyMode::from_url(url);
            if mode == ProxyMode::None && !url.is_empty() {
                warn!(url = %url, "[AUTH] Unknown proxy URL format, using direct connection");
                (ProxyMode::None, None)
            } else {
                (mode, proxy_url.clone())
            }
        } else {
            (ProxyMode::None, None)
        };

        // Validate and normalize proxy URL
        let normalized_proxy_url = if let Some(url) = &proxy_url_str {
            match Self::validate_proxy_url(url, &proxy_mode) {
                Ok(url) => Some(url),
                Err(e) => {
                    error!(error = %e, "[AUTH] Failed to validate proxy URL");
                    return Err(e);
                }
            }
        } else {
            None
        };

        // Update proxy status
        *self.proxy_status.lock().await = ProxyStatus {
            mode: proxy_mode.clone(),
            proxy_url: proxy_url_str.clone(),
        };

        if let Some(ref url) = proxy_url_str {
            info!(proxy_url = %url, mode = ?proxy_mode, "[AUTH] Initializing with proxy");
        } else {
            info!("[AUTH] Initializing without proxy");
        }

        let session_path = Self::get_session_path()?;

        // Сохраняем для последующего использования
        *self.api_id.lock().await = Some(api_id);
        *self.api_hash.lock().await = Some(api_hash.clone());
        *self.phone_number.lock().await = Some(phone.clone());
        *self.session_path.lock().await = Some(session_path.clone());

        debug!(session_path = ?session_path, "[AUTH] Session path");
        debug!(exists = session_path.exists(), "[AUTH] Session exists");

        // Создаём сессию (откроем существующую или создадим новую)
        let session = SqliteSession::open(&session_path)
            .map_err(|e| format!("Не удалось создать сессию: {}", e))?;
        let session = Arc::new(session);

        // Создаём SenderPool with proxy if configured
        let pool = if let Some(proxy) = normalized_proxy_url {
            info!("[AUTH] Creating SenderPool with proxy configuration");
            let conn_params = ConnectionParams {
                proxy_url: Some(proxy),
                ..Default::default()
            };
            SenderPool::with_configuration(session, api_id as i32, conn_params)
        } else {
            info!("[AUTH] Creating SenderPool without proxy");
            SenderPool::new(session, api_id as i32)
        };

        // Создаём клиент
        let client = Client::new(&pool);

        // Запускаем runner и сохраняем updates канал
        let SenderPool { runner, handle: _, updates } = pool;
        let pool_task = tokio::spawn(async move {
            runner.run().await;
        });

        *self.pool_task.lock().await = Some(pool_task);
        *self.client.lock().await = Some(client);
        *self.updates.lock().await = Some(updates);

        // Проверяем, авторизован ли уже пользователь (восстановление сессии)
        let is_authorized = {
            let client_guard = self.client.lock().await;
            if let Some(c) = client_guard.as_ref() {
                match c.is_authorized().await {
                    Ok(authorized) => {
                        debug!(authorized, "[AUTH] Session restored");
                        authorized
                    }
                    Err(e) => {
                        error!(error = %e, "[AUTH] Error checking authorization");
                        false
                    }
                }
            } else {
                false
            }
        };

        if is_authorized {
            // Сессия валидна и пользователь авторизован
            info!("[AUTH] User already authorized, session restored");
            return Ok(OperationResult::success("Сессия восстановлена"));
        }

        // Нужно авторизоваться заново
        info!("[AUTH] Session not authorized, need to sign in");
        Ok(OperationResult::success("Клиент инициализирован, требуется авторизация"))
    }

    /// Проверка авторизации
    pub async fn is_authorized(&self) -> Result<bool, String> {
        let client = self.client.lock().await;
        let client = client.as_ref()
            .ok_or_else(|| "Клиент не инициализирован".to_string())?;

        client.is_authorized().await
            .map_err(|e| format!("Ошибка проверки авторизации: {}", e))
    }

    /// Запрос кода подтверждения
    pub async fn request_code(&self) -> Result<AuthState, String> {
        let client = self.client.lock().await;
        let client = client.as_ref()
            .ok_or_else(|| "Клиент не инициализирован".to_string())?;

        let api_hash = self.api_hash.lock().await;
        let api_hash = api_hash.as_ref()
            .ok_or_else(|| "API hash не задан".to_string())?;

        let phone_number = self.phone_number.lock().await;
        let phone_number = phone_number.as_ref()
            .ok_or_else(|| "Номер телефона не задан".to_string())?;

        let token = client.request_login_code(phone_number, api_hash).await
            .map_err(|e| format!("Ошибка при запросе кода: {}", e))?;

        *self.login_token.lock().await = Some(token);

        Ok(AuthState::CodeRequired)
    }

    /// Ввод кода подтверждения
    /// Uses timing normalization and jitter to prevent timing attacks
    pub async fn sign_in(&self, code: &str) -> Result<AuthState, String> {
        // Security: Don't log the actual code - prevents timing analysis via logs
        info!("[AUTH] Starting sign_in (code not logged for security)");

        let client = self.client.lock().await;
        let client = client.as_ref()
            .ok_or_else(|| "Клиент не инициализирован".to_string())?;
        info!("[AUTH] Client acquired");

        // Забираем токен из Option (take() освобождает мьютекс, возвращая значение)
        let token = self.login_token.lock().await.take()
            .ok_or_else(|| "Токен авторизации не найден. Сначала запросите код.".to_string())?;
        info!("[AUTH] Token acquired and removed from mutex");

        let start_time = std::time::Instant::now();

        // Добавляем таймаут на авторизацию (30 секунд)
        let sign_in_result = tokio::time::timeout(
            tokio::time::Duration::from_secs(30),
            client.sign_in(&token, code)
        ).await;

        let elapsed = start_time.elapsed();

        let signed_in = match sign_in_result {
            Ok(result) => result,
            Err(_) => {
                error!("[AUTH] Sign in timed out after 30 seconds");
                // Add jitter to obscure timing even on timeout
                let jitter = rand::random::<u64>() % 500;
                tokio::time::sleep(tokio::time::Duration::from_millis(500 + jitter)).await;
                return Err("Превышено время ожидания. Проверьте подключение к интернету.".to_string());
            }
        };

        info!("[AUTH] Sign in result: {:?}", signed_in.is_ok());
        info!("[AUTH] About to match on signed_in...");

        match signed_in {
            Ok(_) => {
                info!("[AUTH] Sign in successful (token already cleared)");
                // Normalize timing to ~1 second to prevent timing attacks
                let target_duration = std::time::Duration::from_millis(1000);
                let remaining = target_duration.saturating_sub(elapsed);
                tokio::time::sleep(remaining).await;
                Ok(AuthState::Connected)
            }
            Err(SignInError::PasswordRequired(_)) => {
                // Add jitter to obscure timing before returning token
                let jitter = rand::random::<u64>() % 500;
                tokio::time::sleep(tokio::time::Duration::from_millis(500 + jitter)).await;
                // Возвращаем токен обратно если нужна 2FA
                *self.login_token.lock().await = Some(token);
                Err("Требуется двухфакторная аутентификация. Эта функция пока не реализована.".to_string())
            }
            Err(e) => {
                error!("[AUTH] Sign in error: {:?}", e);
                // Add jitter to obscure timing on error
                let jitter = rand::random::<u64>() % 500;
                tokio::time::sleep(tokio::time::Duration::from_millis(1000 + jitter)).await;
                Err(format!("Ошибка авторизации: {}", e))
            }
        }
    }

    /// Получение информации о пользователе
    pub async fn get_user_info(&self) -> Result<UserInfo, String> {
        debug!("[AUTH] Getting user info");

        let client = self.client.lock().await;
        let client = client.as_ref()
            .ok_or_else(|| "Клиент не инициализирован".to_string())?;

        // Добавляем таймаут на получение информации (10 секунд)
        let me = tokio::time::timeout(
            tokio::time::Duration::from_secs(10),
            client.get_me()
        ).await
        .map_err(|_| "Превышено время ожидания при получении информации о пользователе".to_string())?
        .map_err(|e| format!("Ошибка при получении информации: {}", e))?;

        debug!(username = ?me.username(), "[AUTH] User info received");

        let phone_number = self.phone_number.lock().await;
        let phone_number = phone_number.as_ref()
            .unwrap_or(&"unknown".to_string())
            .clone();

        Ok(UserInfo {
            id: me.raw.id(),
            first_name: me.first_name().map(|s| s.to_string()),
            last_name: me.last_name().map(|s| s.to_string()),
            username: me.username().map(|s| s.to_string()),
            phone: phone_number,
        })
    }

    /// Отключение клиента
    pub async fn disconnect(&self) -> Result<OperationResult, String> {
        // Завершаем задачу пула
        let mut pool_task = self.pool_task.lock().await;
        if let Some(task) = pool_task.take() {
            task.abort();
        }

        // Сбрасываем состояние
        *self.client.lock().await = None;
        *self.updates.lock().await = None;
        *self.login_token.lock().await = None;
        *self.api_id.lock().await = None;
        *self.api_hash.lock().await = None;
        *self.phone_number.lock().await = None;

        Ok(OperationResult::success("Клиент отключен"))
    }

    /// Удаление сессии (sign out)
    pub async fn sign_out(&self) -> Result<OperationResult, String> {
        // Сначала отключаем клиент
        self.disconnect().await?;

        // Удаляем файл сессии
        if let Some(session_path) = self.session_path.lock().await.as_ref() {
            if session_path.exists() {
                std::fs::remove_file(session_path)
                    .map_err(|e| format!("Не удалось удалить файл сессии: {}", e))?;
            }
        }

        Ok(OperationResult::success("Выход выполнен успешно"))
    }

    /// Инициализация с существующей сессией (без phone/api_hash)
    #[allow(dead_code)]
    pub async fn init_empty(&self, api_id: u32) -> Result<OperationResult, String> {
        self.init_empty_with_proxy(api_id, None).await
    }

    /// Инициализация с существующей сессией с поддержкой прокси
    pub async fn init_empty_with_proxy(&self, api_id: u32, proxy_url: Option<String>) -> Result<OperationResult, String> {
        // Determine proxy mode from URL
        let (proxy_mode, proxy_url_str) = if let Some(ref url) = &proxy_url {
            let mode = ProxyMode::from_url(url);
            if mode == ProxyMode::None && !url.is_empty() {
                warn!(url = %url, "[AUTH] Unknown proxy URL format, using direct connection");
                (ProxyMode::None, None)
            } else {
                (mode, proxy_url.clone())
            }
        } else {
            (ProxyMode::None, None)
        };

        // Validate and normalize proxy URL
        let normalized_proxy_url = if let Some(url) = &proxy_url_str {
            match Self::validate_proxy_url(url, &proxy_mode) {
                Ok(url) => Some(url),
                Err(e) => {
                    error!(error = %e, "[AUTH] Failed to validate proxy URL");
                    return Err(e);
                }
            }
        } else {
            None
        };

        // Update proxy status
        *self.proxy_status.lock().await = ProxyStatus {
            mode: proxy_mode.clone(),
            proxy_url: proxy_url_str.clone(),
        };

        if let Some(ref url) = proxy_url_str {
            info!(proxy_url = %url, mode = ?proxy_mode, "[AUTH] Restoring session with proxy");
        } else {
            info!("[AUTH] Restoring session without proxy");
        }

        let session_path = Self::get_session_path()?;

        // Сохраняем api_id
        *self.api_id.lock().await = Some(api_id);
        *self.session_path.lock().await = Some(session_path.clone());

        debug!(session_path = ?session_path, "[AUTH] Session path");
        debug!(exists = session_path.exists(), "[AUTH] Session exists");

        // Открываем существующую сессию
        let session = SqliteSession::open(&session_path)
            .map_err(|e| format!("Не удалось создать сессию: {}", e))?;
        let session = Arc::new(session);

        // Создаём SenderPool with proxy if configured
        let pool = if let Some(proxy) = normalized_proxy_url {
            info!("[AUTH] Creating SenderPool with proxy configuration for session restore");
            let conn_params = ConnectionParams {
                proxy_url: Some(proxy),
                ..Default::default()
            };
            SenderPool::with_configuration(session, api_id as i32, conn_params)
        } else {
            info!("[AUTH] Creating SenderPool without proxy for session restore");
            SenderPool::new(session, api_id as i32)
        };

        // Создаём клиент
        let client = Client::new(&pool);

        // Запускаем runner
        let SenderPool { runner, handle: _, updates } = pool;
        let pool_task = tokio::spawn(async move {
            runner.run().await;
        });

        *self.pool_task.lock().await = Some(pool_task);
        *self.client.lock().await = Some(client);
        *self.updates.lock().await = Some(updates);

        Ok(OperationResult::success("Клиент инициализирован с существующей сессией"))
    }

    /// Get current proxy status
    pub async fn get_proxy_status(&self) -> ProxyStatus {
        self.proxy_status.lock().await.clone()
    }
}

impl Default for TelegramClient {
    fn default() -> Self {
        Self::new()
    }
}
