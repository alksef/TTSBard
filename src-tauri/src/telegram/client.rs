use super::types::{AuthState, OperationResult, UserInfo};
use grammers_client::{Client, SignInError};
use grammers_client::types::LoginToken;
use grammers_mtsender::SenderPool;
use grammers_session::updates::UpdatesLike;
use grammers_session::storages::SqliteSession;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, debug};

// NOTE: api_id is now stored in settings.json (settings.tts.telegram.api_id)
// The old telegram_config.json file is no longer used

pub struct TelegramClient {
    pub(crate) pool_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub(crate) client: Arc<Mutex<Option<Client>>>,
    pub(crate) updates: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<UpdatesLike>>>>,
    login_token: Arc<Mutex<Option<LoginToken>>>,
    pub(crate) api_id: Arc<Mutex<Option<u32>>>,
    api_hash: Arc<Mutex<Option<String>>>,
    phone_number: Arc<Mutex<Option<String>>>,
    session_path: Arc<Mutex<Option<PathBuf>>>,
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

    /// Инициализация клиента
    pub async fn init(&self, api_id: u32, api_hash: String, phone: String) -> Result<OperationResult, String> {
        let session_path = Self::get_session_path()?;

        // Сохраняем для последующего использования
        *self.api_id.lock().await = Some(api_id);
        *self.api_hash.lock().await = Some(api_hash.clone());
        *self.phone_number.lock().await = Some(phone.clone());
        *self.session_path.lock().await = Some(session_path.clone());

        debug!(session_path = ?session_path, "[AUTH] Session path");
        debug!(exists = session_path.exists(), "[AUTH] Session exists");

        // Создаём сессию (откроет существующую или создаст новую)
        let session = SqliteSession::open(&session_path)
            .map_err(|e| format!("Не удалось создать сессию: {}", e))?;
        let session = Arc::new(session);

        // Создаём SenderPool
        let pool = SenderPool::new(session, api_id as i32);

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
    pub async fn init_empty(&self, api_id: u32) -> Result<OperationResult, String> {
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

        // Создаём SenderPool
        let pool = SenderPool::new(session, api_id as i32);

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
}

impl Default for TelegramClient {
    fn default() -> Self {
        Self::new()
    }
}
