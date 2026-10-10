//! Twitch API authorization coordinator (ROADMAP-132).
//!
//! Owns the encrypted credential document and exactly one in-flight device
//! OAuth operation. Device authorization is delegated to
//! [`super::device_oauth`] and the protocol calls to [`super::api`]; this module owns
//! the credential lifecycle, the generation/cancel guards and the safe status
//! projection returned to the frontend.
//!
//! # Invariants
//!
//! - Tokens and the client secret never leave this module: statuses and results
//!   are safe projections, and every error code is a stable, non-secret string.
//! - A single generation counter guards every credential write. An OAuth finish
//!   or a token refresh captures its generation before doing any network work
//!   and re-checks it before persisting, so a stale authorization/refresh can never
//!   overwrite newer credentials.
//! - All encrypted read-modify-write goes through coordinator serialization and
//!   runs on a blocking thread pool (DPAPI + filesystem).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use twitch_api::types::{UserId, UserName};
use twitch_oauth2::{
    AccessToken, AppAccessToken, ClientId, ClientSecret, RefreshToken, Scope, TwitchToken,
    UserToken, ValidatedToken,
};

use super::api::{ApiError, SendOutcome, TwitchApi};
use super::credentials::{
    now_unix, AccountGrant, AppToken, CredentialsError, CredentialsStore, SecretString,
    StoredCredentials,
};
use super::device_oauth::{DeviceFlow, PendingDeviceAuthorization};
use super::oauth::{OAuthError, OAuthRole};

/// Upper bound for the complete begin/open/wait/exchange operation.
const OAUTH_OPERATION_TIMEOUT: Duration = Duration::from_secs(300);
/// App access tokens are refreshed when this close to expiry.
const APP_TOKEN_EARLY_EXPIRY_SECS: i64 = 60;
/// Minimum interval between periodic `/validate` calls for active user tokens.
const USER_TOKEN_VALIDATION_INTERVAL: Duration = Duration::from_secs(3600);

/// Minimal abstraction over the encrypted credential file, so the coordinator
/// is testable with an in-memory store.
pub trait CredentialsBackend: Send + Sync {
    fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError>;
    fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError>;
    #[cfg(test)]
    fn clear(&self) -> Result<(), CredentialsError>;
}

impl CredentialsBackend for CredentialsStore {
    fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
        CredentialsStore::load(self)
    }

    fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
        CredentialsStore::save(self, creds)
    }

    #[cfg(test)]
    fn clear(&self) -> Result<(), CredentialsError> {
        CredentialsStore::remove(self)
    }
}

/// A pending account authorization session.
#[async_trait]
pub trait OAuthSession: Send + Sync {
    fn authorize_url(&self) -> String;
    async fn finish(&mut self, cancel: CancellationToken) -> Result<UserToken, OAuthError>;
}

/// Thin adapter over the twitch-rs SDK used by the coordinator. Test doubles
/// implement this trait instead of talking to Twitch.
#[async_trait]
pub trait TwitchAuthApi: Send + Sync {
    /// Begin device-code authorization for the requested role. Fakes that do not
    /// exercise the device flow fall back to a stable error.
    async fn begin_device(&self, _role: OAuthRole) -> Result<Box<dyn OAuthSession>, OAuthError> {
        Err(OAuthError::ExchangeFailed(ApiError::Malformed))
    }
    async fn validate_user(&self, token: &UserToken) -> Result<ValidatedToken, ApiError>;
    async fn refresh_user(&self, token: &mut UserToken) -> Result<(), ApiError>;
    async fn app_access(&self) -> Result<AppAccessToken, ApiError>;
    async fn validate_app(&self, token: &AppAccessToken) -> Result<ValidatedToken, ApiError> {
        Ok(ValidatedToken {
            client_id: token.client_id().clone(),
            login: None,
            user_id: None,
            scopes: Some(Vec::new()),
            expires_in: Some(token.expires_in()),
        })
    }
    async fn send_chat(
        &self,
        broadcaster_id: &str,
        sender_id: &str,
        message: &str,
        app_token: &AppAccessToken,
    ) -> Result<SendOutcome, ApiError>;
}

/// Production [`TwitchAuthApi`] backed by the task001c SDK adapter.
struct SdkTwitchAuthApi {
    api: TwitchApi,
}

impl SdkTwitchAuthApi {
    fn new(api: TwitchApi) -> Self {
        Self { api }
    }
}

#[async_trait]
impl TwitchAuthApi for SdkTwitchAuthApi {
    async fn begin_device(&self, role: OAuthRole) -> Result<Box<dyn OAuthSession>, OAuthError> {
        let flow = DeviceFlow::new(self.api.clone());
        let pending = flow.begin(role).await?;
        Ok(Box::new(TwitchDeviceOAuthSession {
            pending: Some(pending),
        }))
    }

    async fn validate_user(&self, token: &UserToken) -> Result<ValidatedToken, ApiError> {
        self.api.validate_user_token(token).await
    }

    async fn refresh_user(&self, token: &mut UserToken) -> Result<(), ApiError> {
        self.api.refresh_user_token(token).await
    }

    async fn app_access(&self) -> Result<AppAccessToken, ApiError> {
        self.api.client_credentials().await
    }

    async fn validate_app(&self, token: &AppAccessToken) -> Result<ValidatedToken, ApiError> {
        self.api.validate_app_token(token).await
    }

    async fn send_chat(
        &self,
        broadcaster_id: &str,
        sender_id: &str,
        message: &str,
        app_token: &AppAccessToken,
    ) -> Result<SendOutcome, ApiError> {
        self.api
            .send_chat_message(broadcaster_id, sender_id, message, app_token)
            .await
    }
}

struct TwitchDeviceOAuthSession {
    pending: Option<PendingDeviceAuthorization>,
}

#[async_trait]
impl OAuthSession for TwitchDeviceOAuthSession {
    fn authorize_url(&self) -> String {
        self.pending
            .as_ref()
            .map(|p| p.verification_uri().to_string())
            .unwrap_or_default()
    }

    async fn finish(&mut self, cancel: CancellationToken) -> Result<UserToken, OAuthError> {
        let pending = self.pending.take().expect("finish is called at most once");
        pending.wait_for_token(cancel).await
    }
}

pub type ApiFactory =
    Arc<dyn Fn(&StoredCredentials) -> Result<Arc<dyn TwitchAuthApi>, AuthError> + Send + Sync>;

struct CachedApi {
    client_id: String,
    secret: SecretString,
    generation: u64,
    api: Arc<dyn TwitchAuthApi>,
}

struct Session {
    id: String,
    role: OAuthRole,
    generation: u64,
    cancel: CancellationToken,
    deadline: tokio::time::Instant,
    pending: Option<Box<dyn OAuthSession>>,
}

struct Inner {
    generation: u64,
    creds: Option<StoredCredentials>,
    session: Option<Session>,
    api: Option<CachedApi>,
    last_bot_validation: Option<std::time::Instant>,
    last_broadcaster_validation: Option<std::time::Instant>,
    last_app_validation: Option<std::time::Instant>,
}

impl Inner {
    fn new() -> Self {
        Self {
            generation: 0,
            creds: None,
            session: None,
            api: None,
            last_bot_validation: None,
            last_broadcaster_validation: None,
            last_app_validation: None,
        }
    }
}

pub struct TwitchAuthCoordinator {
    backend: Arc<dyn CredentialsBackend>,
    api_factory: ApiFactory,
    inner: Arc<tokio::sync::Mutex<Inner>>,
    shutdown: CancellationToken,
    delivery_guard: tokio::sync::Mutex<()>,
    begin_guard: tokio::sync::Mutex<()>,
    operation_timeout: Duration,
    validation_interval: Duration,
    invalidation_tx: tokio::sync::watch::Sender<u64>,
}

impl TwitchAuthCoordinator {
    /// Production coordinator backed by the DPAPI credential file.
    pub fn new(store: CredentialsStore, timeout: Duration, shutdown: CancellationToken) -> Self {
        let backend: Arc<dyn CredentialsBackend> = Arc::new(store);
        let factory: ApiFactory = Arc::new(move |creds| {
            if creds.client_id.is_empty() || creds.client_secret.is_empty() {
                return Err(AuthError::NotConfigured);
            }
            let api = TwitchApi::new(
                creds.client_id.clone(),
                creds.client_secret.clone(),
                timeout,
            )
            .map_err(AuthError::Api)?;
            Ok(Arc::new(SdkTwitchAuthApi::new(api)) as Arc<dyn TwitchAuthApi>)
        });
        Self::with_components(backend, factory, shutdown)
    }

    pub fn with_components(
        backend: Arc<dyn CredentialsBackend>,
        api_factory: ApiFactory,
        shutdown: CancellationToken,
    ) -> Self {
        let (invalidation_tx, _) = tokio::sync::watch::channel(0);
        Self {
            backend,
            api_factory,
            inner: Arc::new(tokio::sync::Mutex::new(Inner::new())),
            shutdown,
            delivery_guard: tokio::sync::Mutex::new(()),
            begin_guard: tokio::sync::Mutex::new(()),
            operation_timeout: OAUTH_OPERATION_TIMEOUT,
            validation_interval: USER_TOKEN_VALIDATION_INTERVAL,
            invalidation_tx,
        }
    }

    #[cfg(test)]
    pub fn with_operation_timeout(mut self, timeout: Duration) -> Self {
        self.operation_timeout = timeout;
        self
    }

    #[cfg(test)]
    pub fn with_validation_interval(mut self, interval: Duration) -> Self {
        self.validation_interval = interval;
        self
    }

    /// Subscribe to authorization invalidations / identity changes.
    pub fn subscribe_invalidation(&self) -> tokio::sync::watch::Receiver<u64> {
        self.invalidation_tx.subscribe()
    }

    /// Current generation counter, exposed for the runtime task.
    #[cfg(test)]
    pub async fn generation(&self) -> u64 {
        self.inner.lock().await.generation
    }

    async fn snapshot_creds(&self) -> Result<(StoredCredentials, u64), AuthError> {
        let mut inner = self.inner.lock().await;
        if inner.creds.is_none() {
            let backend = self.backend.clone();
            let loaded = tokio::task::spawn_blocking(move || backend.load())
                .await
                .map_err(|_| AuthError::Store)?
                .map_err(|_| AuthError::Store)?
                .unwrap_or_default();
            inner.creds = Some(loaded);
        }
        let creds = inner.creds.clone().unwrap();
        let generation = inner.generation;
        Ok((creds, generation))
    }

    async fn api_for_creds(
        &self,
        creds: &StoredCredentials,
        generation: u64,
    ) -> Result<Arc<dyn TwitchAuthApi>, AuthError> {
        if creds.client_id.is_empty() || creds.client_secret.is_empty() {
            return Err(AuthError::NotConfigured);
        }
        let mut inner = self.inner.lock().await;
        if inner.generation != generation {
            return Err(AuthError::StaleSession);
        }
        if let Some(cached) = inner.api.as_ref() {
            if cached.client_id == creds.client_id
                && cached.secret == creds.client_secret
                && cached.generation == generation
            {
                return Ok(cached.api.clone());
            }
        }
        let api = (self.api_factory)(creds)?;
        inner.api = Some(CachedApi {
            client_id: creds.client_id.clone(),
            secret: creds.client_secret.clone(),
            generation,
            api: api.clone(),
        });
        Ok(api)
    }

    /// Safe, non-secret authorization status for the frontend.
    pub async fn status(&self) -> AuthStatusDto {
        let mut inner = self.inner.lock().await;
        if inner.creds.is_none() {
            let backend = self.backend.clone();
            let loaded = tokio::task::spawn_blocking(move || backend.load())
                .await
                .map_err(|_| AuthError::Store)
                .and_then(|r| r.map_err(|_| AuthError::Store));
            match loaded {
                Ok(c) => inner.creds = Some(c.unwrap_or_default()),
                Err(_) => {
                    return AuthStatusDto {
                        client_id: String::new(),
                        secret_configured: false,
                        bot_login: None,
                        broadcaster_login: None,
                        channels: Vec::new(),
                        selected_channel_id: None,
                        busy: false,
                        session: None,
                        store_error: Some("twitch.api_auth.store_failed".to_string()),
                    };
                }
            }
        }

        let creds = inner.creds.clone().unwrap_or_default();
        let busy = inner.session.is_some();
        let session = inner.session.as_ref().map(|s| AuthSessionDto {
            session_id: s.id.clone(),
            role: role_str(s.role).to_string(),
        });

        let secret_configured = !creds.client_secret.is_empty();
        let bot_login = creds.bot.as_ref().map(|g| g.login.clone());
        let broadcaster_login = creds.selected_channel().map(|g| g.login.clone());
        let channels = creds
            .channels
            .iter()
            .map(|g| ChannelStatusDto {
                user_id: g.user_id.clone(),
                login: g.login.clone(),
            })
            .collect();
        let selected_channel_id = creds.selected_channel_id.clone();

        AuthStatusDto {
            client_id: creds.client_id,
            secret_configured,
            bot_login,
            broadcaster_login,
            channels,
            selected_channel_id,
            busy,
            session,
            store_error: None,
        }
    }

    /// Save client credentials. A blank secret preserves the existing secret
    /// only for the same client id; a changed id clears both grants and the
    /// cached app token. Cancels any active session and invalidates the SDK
    /// client.
    pub async fn save_client(
        &self,
        client_id: String,
        client_secret: Option<String>,
    ) -> Result<AuthStatusDto, AuthError> {
        let client_id = client_id.trim().to_string();
        let secret_opt = client_secret.map(SecretString::from);

        {
            let mut inner = self.inner.lock().await;
            if inner.creds.is_none() {
                let backend = self.backend.clone();
                let loaded = tokio::task::spawn_blocking(move || backend.load())
                    .await
                    .map_err(|_| AuthError::Store)?
                    .map_err(|_| AuthError::Store)?
                    .unwrap_or_default();
                inner.creds = Some(loaded);
            }

            let mut creds = inner.creds.clone().unwrap_or_default();
            let secret_is_blank = secret_opt.as_ref().map(|s| s.is_empty()).unwrap_or(true);
            let id_changed = creds.client_id != client_id;
            if id_changed {
                creds.bot = None;
                creds.channels.clear();
                creds.selected_channel_id = None;
                creds.app_token = None;
            }
            creds.client_id = client_id;
            if !secret_is_blank {
                creds.client_secret = secret_opt.expect("non-blank secret is Some");
            } else if id_changed {
                creds.client_secret = SecretString::from(String::new());
            }

            let backend = self.backend.clone();
            let to_save = creds.clone();
            tokio::task::spawn_blocking(move || backend.save(&to_save))
                .await
                .map_err(|_| AuthError::Store)?
                .map_err(|_| AuthError::Store)?;

            if let Some(s) = inner.session.take() {
                s.cancel.cancel();
            }
            inner.creds = Some(creds);
            inner.generation = inner.generation.wrapping_add(1);
            self.invalidation_tx.send_modify(|v| *v = v.wrapping_add(1));
            inner.api = None;
            inner.last_bot_validation = None;
            inner.last_broadcaster_validation = None;
        }

        Ok(self.status().await)
    }

    /// Start a broadcaster device-code session.
    /// Returns an opaque session id and the Twitch activation URL.
    pub async fn begin_device(&self) -> Result<BeginAuthResult, AuthError> {
        self.begin_device_for_role(OAuthRole::Broadcaster).await
    }

    /// Start device authorization with the scopes belonging to this account role.
    pub async fn begin_device_for_role(
        &self,
        role: OAuthRole,
    ) -> Result<BeginAuthResult, AuthError> {
        self.begin_session(role).await
    }

    /// Shared begin path: resolve the API, start the flow-specific pending
    /// session under the generation guard, then register it with a timeout.
    async fn begin_session(&self, role: OAuthRole) -> Result<BeginAuthResult, AuthError> {
        if self.shutdown.is_cancelled() {
            return Err(AuthError::Cancelled);
        }

        let _begin_guard = self.begin_guard.lock().await;
        let (creds, snapshot_gen) = self.snapshot_creds().await?;
        if creds.client_id.is_empty() || creds.client_secret.is_empty() {
            return Err(AuthError::NotConfigured);
        }

        let api = self.api_for_creds(&creds, snapshot_gen).await?;

        // Overall operation deadline begins before the flow-specific begin call
        let deadline = tokio::time::Instant::now() + self.operation_timeout;
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());

        let pending = tokio::time::timeout(remaining, api.begin_device(role))
            .await
            .map_err(|_| AuthError::OAuth(OAuthError::TimedOut))??;

        let authorize_url = pending.authorize_url();
        let session_id = uuid::Uuid::new_v4().to_string();
        let cancel = self.shutdown.child_token();

        let assigned_gen = {
            let mut inner = self.inner.lock().await;
            if self.shutdown.is_cancelled() {
                drop(pending);
                return Err(AuthError::Cancelled);
            }
            if inner.generation != snapshot_gen {
                drop(pending);
                return Err(AuthError::StaleSession);
            }
            if let Some(s) = inner.session.take() {
                s.cancel.cancel();
            }
            inner.generation = inner.generation.wrapping_add(1);
            inner.session = Some(Session {
                id: session_id.clone(),
                role,
                generation: inner.generation,
                cancel: cancel.clone(),
                deadline,
                pending: Some(pending),
            });
            inner.generation
        };

        // Auto-cancel and drop pending listener on overall timeout if finish was never called
        let timeout_inner = Arc::clone(&self.inner);
        let timeout_session_id = session_id.clone();
        let timeout_cancel = cancel;
        let op_timeout = self.operation_timeout;
        tokio::spawn(async move {
            tokio::time::sleep(op_timeout).await;
            timeout_cancel.cancel();
            let mut inner = timeout_inner.lock().await;
            if let Some(s) = inner.session.as_ref() {
                if s.id == timeout_session_id && s.generation == assigned_gen {
                    inner.session = None;
                }
            }
        });

        Ok(BeginAuthResult {
            session_id,
            authorize_url,
        })
    }

    /// Await device confirmation, validate the token, validate the role and persist the
    /// grant only if the session is still current.
    pub async fn finish(&self, session_id: &str) -> Result<AuthStatusDto, AuthError> {
        let (generation, role, cancel, deadline, mut pending) = {
            let mut inner = self.inner.lock().await;
            let Some(s) = inner.session.as_mut() else {
                return Err(AuthError::UnknownSession);
            };
            if s.id != session_id {
                return Err(AuthError::UnknownSession);
            }
            let Some(pending) = s.pending.take() else {
                return Err(AuthError::Busy);
            };
            (s.generation, s.role, s.cancel.clone(), s.deadline, pending)
        };

        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let finish_res = tokio::time::timeout(remaining, pending.finish(cancel)).await;

        {
            let mut inner = self.inner.lock().await;
            if let Some(s) = inner.session.as_ref() {
                if s.id == session_id && s.generation == generation {
                    inner.session = None;
                }
            }
        }

        let token = match finish_res {
            Ok(Ok(token)) => token,
            Ok(Err(e)) => return Err(AuthError::OAuth(e)),
            Err(_) => return Err(AuthError::OAuth(OAuthError::TimedOut)),
        };

        let user_id = token
            .user_id()
            .map(|r| r.as_str().to_string())
            .unwrap_or_default();
        let login = token
            .login()
            .map(|r| r.as_str().to_string())
            .unwrap_or_default();
        if user_id.is_empty() || login.is_empty() {
            return Err(AuthError::MissingIdentity);
        }
        if !required_scopes_met(role, token.scopes()) {
            return Err(AuthError::ScopesMismatch);
        }
        let grant = grant_from_token(&token);

        let mut inner = self.inner.lock().await;
        if inner.generation != generation {
            return Err(AuthError::StaleSession);
        }

        if inner.creds.is_none() {
            let backend = self.backend.clone();
            let loaded = tokio::task::spawn_blocking(move || backend.load())
                .await
                .map_err(|_| AuthError::Store)?
                .map_err(|_| AuthError::Store)?
                .unwrap_or_default();
            inner.creds = Some(loaded);
        }

        let mut creds = inner.creds.clone().unwrap_or_default();
        match role {
            OAuthRole::Bot => {
                // The bot must differ from every stored channel, not only the
                // currently selected one.
                let conflicts = creds
                    .channels
                    .iter()
                    .any(|c| c.user_id == user_id || c.login.eq_ignore_ascii_case(&login));
                if conflicts {
                    return Err(AuthError::SameAccount);
                }
                creds.bot = Some(grant);
            }
            OAuthRole::Broadcaster => {
                if is_same_account(&creds.bot, &user_id, &login) {
                    return Err(AuthError::SameAccount);
                }
                // Device broadcaster authorization adds or updates
                // one channel; it never replaces the whole collection.
                creds.upsert_channel(grant);
            }
        }

        let backend = self.backend.clone();
        let to_save = creds.clone();
        tokio::task::spawn_blocking(move || backend.save(&to_save))
            .await
            .map_err(|_| AuthError::Store)?
            .map_err(|_| AuthError::Store)?;

        // A broadcaster grant may only refresh the broadcaster validation cache
        // when it belongs to the currently selected delivery channel. Any other
        // channel authorization must invalidate the cache instead, so a revoked
        // selected grant can never be served from a stale cache window opened by
        // an unrelated account.
        let broadcaster_matches_selection =
            creds.selected_channel_id.as_deref() == Some(user_id.as_str());

        inner.creds = Some(creds);
        inner.generation = inner.generation.wrapping_add(1);
        self.invalidation_tx.send_modify(|v| *v = v.wrapping_add(1));
        match role {
            OAuthRole::Bot => inner.last_bot_validation = Some(std::time::Instant::now()),
            OAuthRole::Broadcaster => {
                inner.last_broadcaster_validation = if broadcaster_matches_selection {
                    Some(std::time::Instant::now())
                } else {
                    None
                };
            }
        }

        drop(inner);
        Ok(self.status().await)
    }

    /// Cancel the active session. With `session_id` provided, only a matching
    /// session is cancelled; a stale id never cancels a newer session.
    pub async fn cancel(&self, session_id: Option<&str>) -> bool {
        let mut inner = self.inner.lock().await;
        let should_cancel = match (&inner.session, session_id) {
            (Some(s), Some(id)) => s.id == id,
            (Some(_), None) => true,
            _ => false,
        };
        if !should_cancel {
            return false;
        }
        let s = inner
            .session
            .take()
            .expect("session present when should_cancel");
        s.cancel.cancel();
        inner.generation = inner.generation.wrapping_add(1);
        true
    }

    /// Cancel any active session (used on shutdown).
    pub async fn cancel_active(&self) {
        let mut inner = self.inner.lock().await;
        if let Some(s) = inner.session.take() {
            s.cancel.cancel();
            inner.generation = inner.generation.wrapping_add(1);
        }
    }

    /// Clear all stored credentials and cancel any active session.
    #[cfg(test)]
    pub async fn clear(&self) -> Result<AuthStatusDto, AuthError> {
        {
            let mut inner = self.inner.lock().await;
            let backend = self.backend.clone();
            tokio::task::spawn_blocking(move || backend.clear())
                .await
                .map_err(|_| AuthError::Store)?
                .map_err(|_| AuthError::Store)?;

            if let Some(s) = inner.session.take() {
                s.cancel.cancel();
            }
            inner.creds = Some(StoredCredentials::default());
            inner.generation = inner.generation.wrapping_add(1);
            self.invalidation_tx.send_modify(|v| *v = v.wrapping_add(1));
            inner.api = None;
            inner.last_bot_validation = None;
            inner.last_broadcaster_validation = None;
            inner.last_app_validation = None;
        }
        Ok(self.status().await)
    }

    /// Forget the bot authorization, retaining channels and application credentials.
    pub async fn reset_authorization(&self) -> Result<AuthStatusDto, AuthError> {
        {
            let _delivery_lock = self.delivery_guard.lock().await;
            let mut inner = self.inner.lock().await;
            self.ensure_loaded(&mut inner).await?;
            let mut creds = inner.creds.clone().unwrap_or_default();
            creds.bot = None;
            creds.app_token = None;
            self.commit_selection(&mut inner, creds).await?;
            inner.api = None;
            inner.last_bot_validation = None;
            inner.last_app_validation = None;
        }
        Ok(self.status().await)
    }

    /// Ensure `inner.creds` reflects the stored document (loading it once).
    async fn ensure_loaded(&self, inner: &mut Inner) -> Result<(), AuthError> {
        if inner.creds.is_none() {
            let backend = self.backend.clone();
            let loaded = tokio::task::spawn_blocking(move || backend.load())
                .await
                .map_err(|_| AuthError::Store)?
                .map_err(|_| AuthError::Store)?
                .unwrap_or_default();
            inner.creds = Some(loaded);
        }
        Ok(())
    }

    /// Persist a mutated credential document and install it as current,
    /// bumping the generation, clearing caches, cancelling any stale auth
    /// session and publishing the invalidation watch. On persistence failure
    /// the in-memory state is left untouched.
    async fn commit_selection(
        &self,
        inner: &mut Inner,
        creds: StoredCredentials,
    ) -> Result<(), AuthError> {
        let backend = self.backend.clone();
        let to_save = creds.clone();
        tokio::task::spawn_blocking(move || backend.save(&to_save))
            .await
            .map_err(|_| AuthError::Store)?
            .map_err(|_| AuthError::Store)?;
        inner.creds = Some(creds);
        if let Some(s) = inner.session.take() {
            s.cancel.cancel();
        }
        inner.generation = inner.generation.wrapping_add(1);
        self.invalidation_tx.send_modify(|v| *v = v.wrapping_add(1));
        inner.last_broadcaster_validation = None;
        Ok(())
    }

    /// Select one stored channel as the single delivery recipient. Selecting
    /// the already-selected channel is an idempotent no-op. An unknown id is
    /// rejected with [`AuthError::UnknownChannel`].
    pub async fn select_channel(&self, user_id: &str) -> Result<AuthStatusDto, AuthError> {
        let user_id = user_id.trim();
        {
            let _delivery_lock = self.delivery_guard.lock().await;
            let mut inner = self.inner.lock().await;
            self.ensure_loaded(&mut inner).await?;
            let mut creds = inner.creds.clone().unwrap_or_default();
            if !creds.select_channel(user_id) {
                return Err(AuthError::UnknownChannel);
            }
            let unchanged = inner
                .creds
                .as_ref()
                .and_then(|c| c.selected_channel_id.as_deref())
                == Some(user_id);
            if !unchanged {
                self.commit_selection(&mut inner, creds).await?;
            }
        }
        Ok(self.status().await)
    }

    /// Forget one channel grant locally: the stored grant is removed, and if it
    /// was the selected channel the selection is cleared with no fallback to
    /// another channel. No Twitch revoke request is made.
    pub async fn forget_channel(&self, user_id: &str) -> Result<AuthStatusDto, AuthError> {
        let user_id = user_id.trim();
        {
            let _delivery_lock = self.delivery_guard.lock().await;
            let mut inner = self.inner.lock().await;
            self.ensure_loaded(&mut inner).await?;
            let mut creds = inner.creds.clone().unwrap_or_default();
            if !creds.remove_channel(user_id) {
                return Err(AuthError::UnknownChannel);
            }
            self.commit_selection(&mut inner, creds).await?;
        }
        Ok(self.status().await)
    }

    /// Check whether all required credentials and distinct authorized accounts are configured
    /// for API delivery without making any network requests.
    pub async fn is_delivery_configured(&self) -> bool {
        let Ok((creds, _)) = self.snapshot_creds().await else {
            return false;
        };
        if creds.client_id.trim().is_empty() || creds.client_secret.is_empty() {
            return false;
        }
        let (Some(bot), Some(broadcaster)) = (creds.bot.as_ref(), creds.selected_channel()) else {
            return false;
        };
        if bot.user_id.is_empty()
            || bot.login.is_empty()
            || broadcaster.user_id.is_empty()
            || broadcaster.login.is_empty()
        {
            return false;
        }
        if bot.user_id == broadcaster.user_id || bot.login.eq_ignore_ascii_case(&broadcaster.login)
        {
            return false;
        }
        true
    }

    /// Validate both grants (refreshing as needed), ensure a live app token and
    /// return everything the runtime needs to send a message.
    pub async fn prepare_delivery(&self) -> Result<DeliveryReady, AuthError> {
        let _delivery_lock = self.delivery_guard.lock().await;
        let (creds, generation) = self.snapshot_creds().await?;

        if creds.client_id.is_empty() || creds.client_secret.is_empty() {
            return Err(AuthError::NotConfigured);
        }
        let bot = creds.bot.as_ref().ok_or(AuthError::NotReady)?;
        let broadcaster = creds.selected_channel().ok_or(AuthError::NotReady)?;
        if bot.user_id.is_empty()
            || bot.login.is_empty()
            || broadcaster.user_id.is_empty()
            || broadcaster.login.is_empty()
        {
            return Err(AuthError::MissingIdentity);
        }
        if bot.user_id == broadcaster.user_id || bot.login.eq_ignore_ascii_case(&broadcaster.login)
        {
            return Err(AuthError::SameAccount);
        }

        let api = self.api_for_creds(&creds, generation).await?;

        let bot = self
            .ensure_grant_valid(&api, &creds, bot, OAuthRole::Bot, generation, false)
            .await?;
        let broadcaster = self
            .ensure_grant_valid(
                &api,
                &creds,
                broadcaster,
                OAuthRole::Broadcaster,
                generation,
                false,
            )
            .await?;

        let app_token = self
            .ensure_app_token(&api, &creds, generation, false)
            .await?;

        Ok(DeliveryReady {
            broadcaster_id: broadcaster.user_id,
            sender_id: bot.user_id,
            generation,
            api,
            app_token,
        })
    }

    /// Validate both grants (refreshing if expired/revoked) and the app token
    /// without sending any messages, safe for periodic idle execution.
    ///
    /// Unlike [`prepare_delivery`], this always performs the actual `/validate`
    /// network calls and ignores the shared validation cache, so the periodic
    /// tick verifies the active user/app tokens even when the cache was just
    /// refreshed by a send.
    pub async fn validate_active_session(&self) -> Result<(), AuthError> {
        let _delivery_lock = self.delivery_guard.lock().await;
        let (creds, generation) = self.snapshot_creds().await?;

        if creds.client_id.is_empty() || creds.client_secret.is_empty() {
            return Err(AuthError::NotConfigured);
        }
        let bot = creds.bot.as_ref().ok_or(AuthError::NotReady)?;
        let broadcaster = creds.selected_channel().ok_or(AuthError::NotReady)?;
        if bot.user_id.is_empty()
            || bot.login.is_empty()
            || broadcaster.user_id.is_empty()
            || broadcaster.login.is_empty()
        {
            return Err(AuthError::MissingIdentity);
        }
        if bot.user_id == broadcaster.user_id || bot.login.eq_ignore_ascii_case(&broadcaster.login)
        {
            return Err(AuthError::SameAccount);
        }

        let api = self.api_for_creds(&creds, generation).await?;

        self.ensure_grant_valid(&api, &creds, bot, OAuthRole::Bot, generation, true)
            .await?;
        self.ensure_grant_valid(
            &api,
            &creds,
            broadcaster,
            OAuthRole::Broadcaster,
            generation,
            true,
        )
        .await?;
        self.ensure_app_token(&api, &creds, generation, true)
            .await?;

        Ok(())
    }

    /// Send a message using a previously prepared delivery. A 401 invalidates
    /// the cached app token and never resends the message POST.
    pub async fn send(
        &self,
        ready: &DeliveryReady,
        message: &str,
    ) -> Result<SendOutcome, ApiError> {
        let current_gen = self.inner.lock().await.generation;
        if current_gen != ready.generation {
            return Err(ApiError::Unauthorized);
        }

        match ready
            .api
            .send_chat(
                &ready.broadcaster_id,
                &ready.sender_id,
                message,
                &ready.app_token,
            )
            .await
        {
            Err(ApiError::Unauthorized) => {
                self.invalidate_app_token(ready.generation).await;
                Err(ApiError::Unauthorized)
            }
            other => other,
        }
    }

    async fn ensure_grant_valid(
        &self,
        api: &Arc<dyn TwitchAuthApi>,
        creds: &StoredCredentials,
        grant: &AccountGrant,
        role: OAuthRole,
        generation: u64,
        force: bool,
    ) -> Result<AccountGrant, AuthError> {
        let needs_validation = force
            || {
                let inner = self.inner.lock().await;
                let last = match role {
                    OAuthRole::Bot => inner.last_bot_validation,
                    OAuthRole::Broadcaster => inner.last_broadcaster_validation,
                };
                match last {
                    None => true,
                    Some(instant) => instant.elapsed() >= self.validation_interval,
                }
            }
            || (grant.expires_at - now_unix() <= 60);

        if !needs_validation {
            return Ok(grant.clone());
        }

        let mut token =
            user_token_from_grant(&creds.client_id, &creds.client_secret, grant, now_unix());
        match api.validate_user(&token).await {
            Ok(validated) => {
                if validated.client_id.as_str() != creds.client_id {
                    return Err(AuthError::Revoked);
                }
                if validated.user_id.as_deref().map(|id| id.as_str())
                    != Some(grant.user_id.as_str())
                {
                    return Err(AuthError::Revoked);
                }
                let login_matches = validated
                    .login
                    .as_deref()
                    .map(|l| l.as_str().eq_ignore_ascii_case(&grant.login))
                    .unwrap_or(false);
                if !login_matches {
                    return Err(AuthError::Revoked);
                }
                let scopes = validated.scopes.unwrap_or_default();
                if !required_scopes_met(role, &scopes) {
                    return Err(AuthError::ScopesMismatch);
                }

                let mut inner = self.inner.lock().await;
                if inner.generation != generation {
                    return Err(AuthError::StaleSession);
                }
                match role {
                    OAuthRole::Bot => inner.last_bot_validation = Some(std::time::Instant::now()),
                    OAuthRole::Broadcaster => {
                        inner.last_broadcaster_validation = Some(std::time::Instant::now())
                    }
                }
                Ok(grant.clone())
            }
            Err(ApiError::Unauthorized) => {
                api.refresh_user(&mut token).await.map_err(|e| match e {
                    ApiError::Unauthorized
                    | ApiError::Validation { .. }
                    | ApiError::Http { status: 400 } => AuthError::Revoked,
                    other => AuthError::Api(other),
                })?;
                let updated = grant_from_token(&token);
                // Atomically persist rotated refresh token before fallible validation
                self.persist_grant(role, updated.clone(), generation)
                    .await?;

                let validated = api.validate_user(&token).await.map_err(|e| match e {
                    ApiError::Unauthorized | ApiError::Validation { .. } => AuthError::Revoked,
                    other => AuthError::Api(other),
                })?;
                if validated.client_id.as_str() != creds.client_id {
                    return Err(AuthError::Revoked);
                }
                if validated.user_id.as_deref().map(|id| id.as_str())
                    != Some(grant.user_id.as_str())
                {
                    return Err(AuthError::Revoked);
                }
                let login_matches = validated
                    .login
                    .as_deref()
                    .map(|l| l.as_str().eq_ignore_ascii_case(&grant.login))
                    .unwrap_or(false);
                if !login_matches {
                    return Err(AuthError::Revoked);
                }
                let scopes = validated.scopes.unwrap_or_default();
                if !required_scopes_met(role, &scopes) {
                    return Err(AuthError::ScopesMismatch);
                }

                let mut inner = self.inner.lock().await;
                if inner.generation != generation {
                    return Err(AuthError::StaleSession);
                }
                match role {
                    OAuthRole::Bot => inner.last_bot_validation = Some(std::time::Instant::now()),
                    OAuthRole::Broadcaster => {
                        inner.last_broadcaster_validation = Some(std::time::Instant::now())
                    }
                }
                Ok(updated)
            }
            Err(other) => Err(AuthError::Api(other)),
        }
    }

    async fn persist_grant(
        &self,
        role: OAuthRole,
        grant: AccountGrant,
        generation: u64,
    ) -> Result<(), AuthError> {
        let backend = self.backend.clone();
        let mut inner = self.inner.lock().await;
        let mut creds = inner.creds.clone().unwrap_or_default();
        let mut updated = false;
        match role {
            OAuthRole::Bot => {
                if let Some(bot) = creds.bot.as_mut() {
                    if bot.user_id == grant.user_id {
                        *bot = grant.clone();
                        updated = true;
                    }
                } else if inner.generation == generation {
                    creds.bot = Some(grant.clone());
                    updated = true;
                }
            }
            // Refresh updates the grant for the matching channel id only; it
            // must never move the selection to another channel, and a late
            // refresh for a removed channel is a no-op.
            OAuthRole::Broadcaster => {
                if creds.has_channel(&grant.user_id) {
                    creds.replace_channel_grant(grant.clone());
                    updated = true;
                }
            }
        }
        if updated {
            inner.creds = Some(creds.clone());
            let to_save = creds;
            let save_res = tokio::task::spawn_blocking(move || backend.save(&to_save))
                .await
                .map_err(|_| AuthError::Store)?
                .map_err(|_| AuthError::Store);
            if inner.generation == generation {
                save_res?;
            }
        }
        if inner.generation != generation {
            return Err(AuthError::StaleSession);
        }
        Ok(())
    }

    async fn ensure_app_token(
        &self,
        api: &Arc<dyn TwitchAuthApi>,
        creds: &StoredCredentials,
        generation: u64,
        force: bool,
    ) -> Result<AppAccessToken, AuthError> {
        let now = now_unix();
        let needs_validation = force || {
            let inner = self.inner.lock().await;
            match inner.last_app_validation {
                None => true,
                Some(instant) => instant.elapsed() >= self.validation_interval,
            }
        };

        if let Some(app) = &creds.app_token {
            if app.expires_at - now > APP_TOKEN_EARLY_EXPIRY_SECS {
                let sdk_tok = app_token_to_sdk(app, &creds.client_id, &creds.client_secret, now);
                if !needs_validation {
                    return Ok(sdk_tok);
                }
                match api.validate_app(&sdk_tok).await {
                    Ok(_) => {
                        let mut inner = self.inner.lock().await;
                        if inner.generation != generation {
                            return Err(AuthError::StaleSession);
                        }
                        inner.last_app_validation = Some(std::time::Instant::now());
                        return Ok(sdk_tok);
                    }
                    Err(ApiError::Unauthorized) | Err(ApiError::Validation { .. }) => {
                        // Cached app token revoked or invalid -> reacquire fresh token below
                    }
                    Err(other) => return Err(AuthError::Api(other)),
                }
            }
        }

        let token = api.app_access().await.map_err(AuthError::Api)?;
        let app = app_token_from_sdk(&token);

        let backend = self.backend.clone();
        {
            let mut inner = self.inner.lock().await;
            if inner.generation != generation {
                return Err(AuthError::StaleSession);
            }
            let mut c = inner.creds.clone().unwrap_or_default();
            c.app_token = Some(app.clone());
            let to_save = c.clone();
            tokio::task::spawn_blocking(move || backend.save(&to_save))
                .await
                .map_err(|_| AuthError::Store)?
                .map_err(|_| AuthError::Store)?;
            inner.creds = Some(c);
            inner.last_app_validation = Some(std::time::Instant::now());
        }

        Ok(token)
    }

    async fn invalidate_app_token(&self, generation: u64) {
        let mut inner = self.inner.lock().await;
        if inner.generation != generation {
            return;
        }
        if let Some(c) = inner.creds.as_mut() {
            c.app_token = None;
        }
        let backend = self.backend.clone();
        let creds_to_save = inner.creds.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Some(c) = creds_to_save {
                let _ = backend.save(&c);
            }
        })
        .await;
    }
}

/// Safe authorization status DTO (no tokens, no secret).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusDto {
    pub client_id: String,
    pub secret_configured: bool,
    pub bot_login: Option<String>,
    /// Projection of the currently selected channel, kept for compatibility.
    pub broadcaster_login: Option<String>,
    /// Canonical authorized channel collection (no tokens, no secret).
    pub channels: Vec<ChannelStatusDto>,
    /// Id of the single selected delivery recipient, or `None`.
    pub selected_channel_id: Option<String>,
    pub busy: bool,
    pub session: Option<AuthSessionDto>,
    pub store_error: Option<String>,
}

/// Nonsecret identity of one authorized channel (no tokens, no secret).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelStatusDto {
    pub user_id: String,
    pub login: String,
}

/// Metadata of the in-flight OAuth session, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionDto {
    pub session_id: String,
    pub role: String,
}

/// Result of beginning an OAuth session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginAuthResult {
    pub session_id: String,
    pub authorize_url: String,
}

/// Everything the runtime needs to send one chat message.
pub struct DeliveryReady {
    pub broadcaster_id: String,
    pub sender_id: String,
    pub generation: u64,
    api: Arc<dyn TwitchAuthApi>,
    app_token: AppAccessToken,
}

impl std::fmt::Debug for DeliveryReady {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeliveryReady")
            .field("broadcaster_id", &self.broadcaster_id)
            .field("sender_id", &self.sender_id)
            .field("generation", &self.generation)
            .finish()
    }
}

/// Stable, non-secret coordinator error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("Twitch API client is not configured")]
    NotConfigured,
    #[error("Twitch API is not ready for delivery")]
    NotReady,
    #[error("An authorization session is already active")]
    Busy,
    #[error("Unknown authorization session")]
    UnknownSession,
    #[error("Unknown Twitch channel")]
    UnknownChannel,
    #[error("Authorization session is no longer current")]
    StaleSession,
    #[error("The bot and the broadcaster must be different accounts")]
    SameAccount,
    #[error("Twitch returned no user identity for the grant")]
    MissingIdentity,
    #[error("The grant does not include the required scopes")]
    ScopesMismatch,
    #[error("Twitch API credentials could not be loaded or saved")]
    Store,
    #[error("Authorization was cancelled")]
    Cancelled,
    #[error("The Twitch grant was revoked")]
    Revoked,
    #[error(transparent)]
    OAuth(#[from] OAuthError),
    #[error(transparent)]
    Api(#[from] ApiError),
}

impl AuthError {
    pub fn code(&self) -> &'static str {
        match self {
            AuthError::NotConfigured => "twitch.api_auth.not_configured",
            AuthError::NotReady => "twitch.api_auth.not_ready",
            AuthError::Busy => "twitch.api_auth.busy",
            AuthError::UnknownSession => "twitch.api_auth.unknown_session",
            AuthError::UnknownChannel => "twitch.api_auth.unknown_channel",
            AuthError::StaleSession => "twitch.api_auth.stale_session",
            AuthError::SameAccount => "twitch.api_auth.same_account",
            AuthError::MissingIdentity => "twitch.api_auth.missing_identity",
            AuthError::ScopesMismatch => "twitch.api_auth.scopes_mismatch",
            AuthError::Store => "twitch.api_auth.store_failed",
            AuthError::Cancelled => "twitch.api_auth.cancelled",
            AuthError::Revoked => "twitch.api_auth.revoked",
            AuthError::OAuth(e) => oauth_error_code(e),
            AuthError::Api(e) => api_error_code(e),
        }
    }

    pub fn retryable(&self) -> bool {
        match self {
            AuthError::Store => true,
            AuthError::OAuth(e) => oauth_error_retryable(e),
            AuthError::Api(e) => api_error_retryable(e),
            _ => false,
        }
    }

    pub fn message(&self) -> String {
        match self {
            AuthError::OAuth(e) => oauth_error_message(e),
            AuthError::Api(e) => api_error_message(e),
            other => other.to_string(),
        }
    }
}

fn oauth_error_code(e: &OAuthError) -> &'static str {
    match e {
        OAuthError::Cancelled => "twitch.api_auth.cancelled",
        OAuthError::TimedOut => "twitch.api_auth.oauth.timed_out",
        OAuthError::Denied => "twitch.api_auth.oauth.denied",
        OAuthError::Malformed => "twitch.api_auth.oauth.malformed",
        OAuthError::ExchangeFailed(_) => "twitch.api_auth.oauth.exchange_failed",
    }
}

fn oauth_error_retryable(e: &OAuthError) -> bool {
    match e {
        OAuthError::TimedOut => true,
        OAuthError::Cancelled
        | OAuthError::Denied
        | OAuthError::Malformed
        | OAuthError::ExchangeFailed(_) => false,
    }
}

fn oauth_error_message(e: &OAuthError) -> String {
    match e {
        OAuthError::Cancelled => "Authorization was cancelled".to_string(),
        OAuthError::TimedOut => "Authorization timed out".to_string(),
        OAuthError::Denied => "Authorization was denied".to_string(),
        OAuthError::Malformed => "Authorization response was malformed".to_string(),
        OAuthError::ExchangeFailed(_) => {
            "The device authorization could not be completed".to_string()
        }
    }
}

fn api_error_code(e: &ApiError) -> &'static str {
    match e {
        ApiError::Unauthorized => "twitch.api_auth.api.unauthorized",
        ApiError::Forbidden => "twitch.api_auth.api.forbidden",
        ApiError::RateLimited { .. } => "twitch.api_auth.api.rate_limited",
        ApiError::Http { .. } => "twitch.api_auth.api.http",
        ApiError::Transport => "twitch.api_auth.api.transport",
        ApiError::Malformed => "twitch.api_auth.api.malformed",
        ApiError::InvalidInput => "twitch.api_auth.api.invalid_input",
        ApiError::Validation { .. } => "twitch.api_auth.api.validation",
        ApiError::Dropped { .. } => "twitch.api_auth.api.dropped",
    }
}

fn api_error_retryable(e: &ApiError) -> bool {
    matches!(e, ApiError::RateLimited { .. } | ApiError::Transport)
        || matches!(e, ApiError::Http { status } if *status >= 500)
}

fn api_error_message(e: &ApiError) -> String {
    match e {
        ApiError::Unauthorized => "Twitch rejected the request as unauthorized".to_string(),
        ApiError::Forbidden => "Twitch rejected the request".to_string(),
        ApiError::RateLimited { .. } => "Twitch rate limited the request".to_string(),
        ApiError::Http { .. } => "Twitch returned an HTTP error".to_string(),
        ApiError::Transport => "The Twitch request could not be completed".to_string(),
        ApiError::Malformed => "Twitch returned an unexpected response".to_string(),
        ApiError::InvalidInput => "The message is not valid for Twitch".to_string(),
        ApiError::Validation { .. } => "The Twitch token is not valid".to_string(),
        ApiError::Dropped { .. } => "Twitch dropped the message".to_string(),
    }
}

fn role_str(role: OAuthRole) -> &'static str {
    match role {
        OAuthRole::Bot => "bot",
        OAuthRole::Broadcaster => "broadcaster",
    }
}

/// Parse the backend role enum from the frontend string.
pub fn parse_role(role: &str) -> Result<OAuthRole, ()> {
    match role.trim().to_ascii_lowercase().as_str() {
        "bot" => Ok(OAuthRole::Bot),
        "broadcaster" => Ok(OAuthRole::Broadcaster),
        _ => Err(()),
    }
}

fn required_scopes_met(role: OAuthRole, scopes: &[Scope]) -> bool {
    role.scopes()
        .iter()
        .all(|required| scopes.contains(required))
}

fn is_same_account(other: &Option<AccountGrant>, user_id: &str, login: &str) -> bool {
    match other {
        Some(g) => {
            !user_id.is_empty() && (g.user_id == user_id || g.login.eq_ignore_ascii_case(login))
        }
        None => false,
    }
}

fn user_token_from_grant(
    client_id: &str,
    secret: &SecretString,
    grant: &AccountGrant,
    now: i64,
) -> UserToken {
    let expires_in = Duration::from_secs((grant.expires_at - now).max(0) as u64);
    UserToken::from_existing_unchecked(
        AccessToken::from(grant.access_token.expose().to_string()),
        RefreshToken::from(grant.refresh_token.expose().to_string()),
        ClientId::from(client_id.to_string()),
        ClientSecret::from(secret.expose().to_string()),
        UserName::from(grant.login.clone()),
        UserId::from(grant.user_id.clone()),
        Some(grant.scopes.iter().cloned().map(Scope::from).collect()),
        Some(expires_in),
    )
}

fn grant_from_token(token: &UserToken) -> AccountGrant {
    AccountGrant {
        user_id: token
            .user_id()
            .map(|r| r.as_str().to_string())
            .unwrap_or_default(),
        login: token
            .login()
            .map(|r| r.as_str().to_string())
            .unwrap_or_default(),
        access_token: SecretString::from(token.token().as_str().to_string()),
        refresh_token: SecretString::from(
            token
                .refresh_token
                .as_ref()
                .map(|r| r.as_str().to_string())
                .unwrap_or_default(),
        ),
        expires_at: now_unix() + token.expires_in().as_secs() as i64,
        scopes: token
            .scopes()
            .iter()
            .map(|s| s.as_str().to_string())
            .collect(),
    }
}

fn app_token_from_sdk(token: &AppAccessToken) -> AppToken {
    AppToken {
        access_token: SecretString::from(token.token().as_str().to_string()),
        expires_at: now_unix() + token.expires_in().as_secs() as i64,
    }
}

fn app_token_to_sdk(
    app: &AppToken,
    client_id: &str,
    secret: &SecretString,
    now: i64,
) -> AppAccessToken {
    let expires_in = Duration::from_secs((app.expires_at - now).max(0) as u64);
    AppAccessToken::from_existing_unchecked(
        AccessToken::from(app.access_token.expose().to_string()),
        None::<RefreshToken>,
        ClientId::from(client_id.to_string()),
        ClientSecret::from(secret.expose().to_string()),
        None,
        Some(expires_in),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};
    use std::sync::Mutex as StdMutex;

    struct MemoryBackend {
        creds: StdMutex<Option<StoredCredentials>>,
    }

    impl MemoryBackend {
        fn new(creds: Option<StoredCredentials>) -> Self {
            Self {
                creds: StdMutex::new(creds),
            }
        }

        fn snapshot(&self) -> Option<StoredCredentials> {
            self.creds.lock().unwrap().clone()
        }
    }

    impl CredentialsBackend for MemoryBackend {
        fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
            Ok(self.creds.lock().unwrap().clone())
        }

        fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
            *self.creds.lock().unwrap() = Some(creds.clone());
            Ok(())
        }

        #[cfg(test)]
        fn clear(&self) -> Result<(), CredentialsError> {
            *self.creds.lock().unwrap() = None;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeState {
        oauth_result: Option<Result<UserToken, OAuthError>>,
        oauth_waiter_signal: Option<tokio::sync::oneshot::Sender<()>>,
        oauth_release_signal: Option<tokio::sync::oneshot::Receiver<()>>,
        begin_device_barrier: Option<(
            tokio::sync::oneshot::Sender<()>,
            tokio::sync::oneshot::Receiver<()>,
        )>,
        validate_user_barrier: Option<(
            tokio::sync::oneshot::Sender<()>,
            tokio::sync::oneshot::Receiver<()>,
        )>,
        send_401_once: bool,
        unauthorized_logins: HashSet<String>,
        expired_logins: HashSet<String>,
        refresh_fails_for: HashSet<String>,
        refresh_400_for: HashSet<String>,
        validate_transient: VecDeque<ApiError>,
        validate_user_calls: usize,
        new_refresh_token: Option<String>,
        last_sent_chat: Option<(String, String, String)>,
    }

    #[derive(Clone)]
    struct FakeApi {
        state: Arc<StdMutex<FakeState>>,
    }

    impl FakeApi {
        fn new() -> Self {
            Self {
                state: Arc::new(StdMutex::new(FakeState::default())),
            }
        }
    }

    #[async_trait]
    impl TwitchAuthApi for FakeApi {
        async fn begin_device(&self, role: OAuthRole) -> Result<Box<dyn OAuthSession>, OAuthError> {
            let barrier = self.state.lock().unwrap().begin_device_barrier.take();
            if let Some((entered_tx, release_rx)) = barrier {
                let _ = entered_tx.send(());
                let _ = release_rx.await;
            }
            Ok(Box::new(FakeSession {
                role,
                state: self.state.clone(),
            }))
        }

        async fn validate_user(&self, token: &UserToken) -> Result<ValidatedToken, ApiError> {
            let barrier = self.state.lock().unwrap().validate_user_barrier.take();
            if let Some((entered_tx, release_rx)) = barrier {
                let _ = entered_tx.send(());
                let _ = release_rx.await;
            }
            let login = token
                .login()
                .map(|r| r.as_str().to_string())
                .unwrap_or_default();
            let mut state = self.state.lock().unwrap();
            state.validate_user_calls += 1;
            if state.unauthorized_logins.contains(&login) || state.expired_logins.contains(&login) {
                return Err(ApiError::Unauthorized);
            }
            if let Some(err) = state.validate_transient.pop_front() {
                return Err(err);
            }
            Ok(default_validated(token))
        }

        async fn refresh_user(&self, token: &mut UserToken) -> Result<(), ApiError> {
            let login = token
                .login()
                .map(|r| r.as_str().to_string())
                .unwrap_or_default();
            let mut state = self.state.lock().unwrap();
            if state.refresh_400_for.contains(&login) {
                return Err(ApiError::Http { status: 400 });
            }
            if state.refresh_fails_for.contains(&login) {
                return Err(ApiError::Unauthorized);
            }
            state.expired_logins.remove(&login);
            if let Some(rotated) = &state.new_refresh_token {
                token.refresh_token = Some(RefreshToken::from(rotated.clone()));
                token.access_token = AccessToken::from("rotated-access".to_string());
            }
            Ok(())
        }

        async fn app_access(&self) -> Result<AppAccessToken, ApiError> {
            Ok(AppAccessToken::from_existing_unchecked(
                AccessToken::from("app-access".to_string()),
                None::<RefreshToken>,
                ClientId::from("client".to_string()),
                ClientSecret::from("super-secret-client-secret".to_string()),
                None,
                Some(Duration::from_secs(3600)),
            ))
        }

        async fn send_chat(
            &self,
            broadcaster_id: &str,
            sender_id: &str,
            message: &str,
            _app_token: &AppAccessToken,
        ) -> Result<SendOutcome, ApiError> {
            if self.state.lock().unwrap().send_401_once {
                self.state.lock().unwrap().send_401_once = false;
                return Err(ApiError::Unauthorized);
            }
            self.state.lock().unwrap().last_sent_chat = Some((
                broadcaster_id.to_string(),
                sender_id.to_string(),
                message.to_string(),
            ));
            Ok(SendOutcome {
                message_id: Some("msg-1".to_string()),
                is_sent: true,
                drop_reason: None,
            })
        }
    }

    #[derive(Clone)]
    struct FakeSession {
        role: OAuthRole,
        state: Arc<StdMutex<FakeState>>,
    }

    #[async_trait]
    impl OAuthSession for FakeSession {
        fn authorize_url(&self) -> String {
            "https://www.twitch.tv/activate?device-code=TEST".to_string()
        }

        async fn finish(&mut self, cancel: CancellationToken) -> Result<UserToken, OAuthError> {
            let (tx, rx) = {
                let mut state = self.state.lock().unwrap();
                (
                    state.oauth_waiter_signal.take(),
                    state.oauth_release_signal.take(),
                )
            };
            if let Some(tx) = tx {
                let _ = tx.send(());
            }
            if let Some(rx) = rx {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(OAuthError::Cancelled),
                    _ = rx => {}
                }
            }
            if cancel.is_cancelled() {
                return Err(OAuthError::Cancelled);
            }
            let result = self.state.lock().unwrap().oauth_result.take();
            result.unwrap_or_else(|| {
                let (user_id, login) = match self.role {
                    OAuthRole::Bot => ("111", "bot"),
                    OAuthRole::Broadcaster => ("222", "streamer"),
                };
                Ok(fake_user_token(self.role, user_id, login))
            })
        }
    }

    fn fake_user_token(role: OAuthRole, user_id: &str, login: &str) -> UserToken {
        UserToken::from_existing_unchecked(
            AccessToken::from("access".to_string()),
            RefreshToken::from("refresh".to_string()),
            ClientId::from("client".to_string()),
            ClientSecret::from("super-secret-client-secret".to_string()),
            UserName::from(login.to_string()),
            UserId::from(user_id.to_string()),
            Some(role.scopes().to_vec()),
            Some(Duration::from_secs(3600)),
        )
    }

    fn default_validated(token: &UserToken) -> ValidatedToken {
        ValidatedToken {
            client_id: ClientId::from("client".to_string()),
            login: token
                .login()
                .map(|r| UserName::from(r.as_str().to_string())),
            user_id: token
                .user_id()
                .map(|r| UserId::from(r.as_str().to_string())),
            scopes: Some(token.scopes().to_vec()),
            expires_in: Some(Duration::from_secs(3600)),
        }
    }

    fn configured_creds() -> StoredCredentials {
        StoredCredentials {
            version: super::super::credentials::FORMAT_VERSION,
            client_id: "client".to_string(),
            client_secret: SecretString::from("super-secret-client-secret"),
            bot: None,
            channels: Vec::new(),
            selected_channel_id: None,
            app_token: None,
        }
    }

    fn test_coordinator(
        backend: Arc<dyn CredentialsBackend>,
        api: Arc<FakeApi>,
    ) -> TwitchAuthCoordinator {
        let factory: ApiFactory = Arc::new(move |_creds| Ok(api.clone() as Arc<dyn TwitchAuthApi>));
        TwitchAuthCoordinator::with_components(backend, factory, CancellationToken::new())
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn status_never_exposes_tokens_or_secret() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("super-secret-access"),
            refresh_token: SecretString::from("super-secret-refresh"),
            expires_at: 0,
            scopes: vec![],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let coord = test_coordinator(backend, Arc::new(FakeApi::new()));

        let status = rt.block_on(coord.status());
        let json = serde_json::to_string(&status).unwrap();
        // Specifically check that actual sensitive token and secret strings never appear:
        assert!(!json.contains("super-secret-access"));
        assert!(!json.contains("super-secret-refresh"));
        assert!(!json.contains("super-secret-client-secret"));
        // secret_configured is a boolean flag indicating presence, which is safe:
        assert!(status.secret_configured);
        assert_eq!(status.bot_login.as_deref(), Some("bot"));
    }

    #[test]
    fn save_client_blank_secret_preserves_same_id() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));

        rt.block_on(coord.save_client("client".to_string(), None))
            .unwrap();

        let creds = backend.snapshot().unwrap();
        assert_eq!(creds.client_id, "client");
        assert_eq!(creds.client_secret.expose(), "super-secret-client-secret");
    }

    #[test]
    fn save_client_changed_id_invalidates_grants_and_secret() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "1".into(),
            login: "bot".into(),
            access_token: SecretString::from("a"),
            refresh_token: SecretString::from("r"),
            expires_at: 0,
            scopes: vec![],
        });
        creds.app_token = Some(AppToken {
            access_token: SecretString::from("t"),
            expires_at: 0,
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));

        rt.block_on(coord.save_client("other".to_string(), None))
            .unwrap();

        let creds = backend.snapshot().unwrap();
        assert_eq!(creds.client_id, "other");
        assert!(creds.client_secret.is_empty());
        assert!(creds.bot.is_none());
        assert!(creds.channels.is_empty());
        assert!(creds.selected_channel_id.is_none());
        assert!(creds.app_token.is_none());
    }

    #[test]
    fn same_account_rejected_both_authorization_orders() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());

        // Order 1: Bot authorized first, then Broadcaster with the SAME user id
        let coord1 = test_coordinator(backend.clone(), api.clone());
        let bot_begin = rt
            .block_on(coord1.begin_device_for_role(OAuthRole::Bot))
            .unwrap();
        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Bot, "999", "dual_account")));
        rt.block_on(coord1.finish(&bot_begin.session_id)).unwrap();

        let bc_begin = rt
            .block_on(coord1.begin_device_for_role(OAuthRole::Broadcaster))
            .unwrap();
        api.state.lock().unwrap().oauth_result = Some(Ok(fake_user_token(
            OAuthRole::Broadcaster,
            "999",
            "dual_account",
        )));
        let err1 = rt
            .block_on(coord1.finish(&bc_begin.session_id))
            .unwrap_err();
        assert_eq!(err1, AuthError::SameAccount);

        // Order 2: Broadcaster authorized first, then Bot with the SAME user id
        let backend2 = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let coord2 = test_coordinator(backend2.clone(), api.clone());
        let bc_begin2 = rt
            .block_on(coord2.begin_device_for_role(OAuthRole::Broadcaster))
            .unwrap();
        api.state.lock().unwrap().oauth_result = Some(Ok(fake_user_token(
            OAuthRole::Broadcaster,
            "888",
            "dual_account2",
        )));
        rt.block_on(coord2.finish(&bc_begin2.session_id)).unwrap();

        let bot_begin2 = rt
            .block_on(coord2.begin_device_for_role(OAuthRole::Bot))
            .unwrap();
        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Bot, "888", "dual_account2")));
        let err2 = rt
            .block_on(coord2.finish(&bot_begin2.session_id))
            .unwrap_err();
        assert_eq!(err2, AuthError::SameAccount);
    }

    #[test]
    fn begin_finish_persists_grant_and_generation_cancel() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Bot, "111", "bot")));
        let coord = test_coordinator(backend.clone(), api.clone());

        let begin = rt
            .block_on(coord.begin_device_for_role(OAuthRole::Bot))
            .unwrap();
        assert!(!begin.session_id.is_empty());
        assert_eq!(
            begin.authorize_url,
            "https://www.twitch.tv/activate?device-code=TEST"
        );

        let status = rt.block_on(coord.status());
        assert!(status.busy);
        assert_eq!(status.session.as_ref().unwrap().role, "bot");

        let status = rt.block_on(coord.finish(&begin.session_id)).unwrap();
        assert!(!status.busy);
        assert_eq!(status.bot_login.as_deref(), Some("bot"));

        let creds = backend.snapshot().unwrap();
        assert_eq!(creds.bot.as_ref().unwrap().login, "bot");
        assert_eq!(creds.bot.as_ref().unwrap().user_id, "111");
    }

    #[test]
    fn finish_race_with_cancel_cancels_and_does_not_persist() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        let (waiter_tx, waiter_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();

        api.state.lock().unwrap().oauth_waiter_signal = Some(waiter_tx);
        api.state.lock().unwrap().oauth_release_signal = Some(release_rx);

        let coord = Arc::new(test_coordinator(backend.clone(), api));
        let begin = rt
            .block_on(coord.begin_device_for_role(OAuthRole::Bot))
            .unwrap();

        let coord_clone = coord.clone();
        let session_id = begin.session_id.clone();
        let finish_handle = rt.spawn(async move { coord_clone.finish(&session_id).await });

        // Wait until finish is actively awaiting device confirmation in FakeSession
        rt.block_on(async {
            waiter_rx.await.unwrap();
            // During await, status must still report busy!
            let status = coord.status().await;
            assert!(status.busy);
            // Cancel the active session while finish is awaiting
            let cancelled = coord.cancel(Some(&begin.session_id)).await;
            assert!(cancelled);
            // Release the fake session
            let _ = release_tx.send(());
        });

        let finish_res = rt.block_on(finish_handle).unwrap();
        assert!(matches!(
            finish_res,
            Err(AuthError::OAuth(OAuthError::Cancelled)) | Err(AuthError::StaleSession)
        ));
        // Credentials must not be persisted
        assert!(backend.snapshot().unwrap().bot.is_none());
    }

    #[test]
    fn begin_race_with_save_invalidates_stale_begin() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt
            .block_on(coord.begin_device_for_role(OAuthRole::Bot))
            .unwrap();
        // Save client credentials while session is pending
        rt.block_on(coord.save_client("new_client".to_string(), Some("new_secret".to_string())))
            .unwrap();

        // Finish on the old session must be rejected as unknown/stale
        let res = rt.block_on(coord.finish(&begin.session_id));
        assert!(res.is_err());
        assert!(backend.snapshot().unwrap().bot.is_none());
    }

    #[test]
    fn begin_race_with_clear_invalidates_stale_begin() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt
            .block_on(coord.begin_device_for_role(OAuthRole::Bot))
            .unwrap();
        // Clear all credentials
        rt.block_on(coord.clear()).unwrap();

        // Finish on the old session must fail
        let res = rt.block_on(coord.finish(&begin.session_id));
        assert!(res.is_err());
        assert!(backend.snapshot().is_none());
    }

    #[test]
    fn new_session_supersedes_old_session() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let coord = test_coordinator(backend, Arc::new(FakeApi::new()));

        let first = rt
            .block_on(coord.begin_device_for_role(OAuthRole::Bot))
            .unwrap();
        let second = rt
            .block_on(coord.begin_device_for_role(OAuthRole::Broadcaster))
            .unwrap();

        assert!(!rt.block_on(coord.cancel(Some(&first.session_id))));
        let status = rt.block_on(coord.status());
        assert!(status.busy);
        assert_eq!(
            status.session.as_ref().unwrap().session_id,
            second.session_id
        );

        let err = rt.block_on(coord.finish(&first.session_id)).unwrap_err();
        assert_eq!(err.code(), "twitch.api_auth.unknown_session");
    }

    #[test]
    fn refresh_rotation_persists_before_subsequent_checks() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("old-access"),
            refresh_token: SecretString::from("old-refresh"),
            expires_at: now_unix() - 10,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().new_refresh_token = Some("rotated-refresh".to_string());
        api.state
            .lock()
            .unwrap()
            .expired_logins
            .insert("bot".to_string());
        let coord = test_coordinator(backend.clone(), api);

        let ready = rt.block_on(coord.prepare_delivery()).unwrap();
        assert_eq!(ready.sender_id, "111");
        assert_eq!(ready.broadcaster_id, "222");

        let creds = backend.snapshot().unwrap();
        assert_eq!(
            creds.bot.as_ref().unwrap().refresh_token.expose(),
            "rotated-refresh"
        );
        assert_eq!(
            creds.bot.as_ref().unwrap().access_token.expose(),
            "rotated-access"
        );
        assert!(creds.app_token.is_some());
    }

    #[test]
    fn send_checks_generation_and_rejects_stale_target() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api.clone());

        let ready = rt.block_on(coord.prepare_delivery()).unwrap();

        // Now credentials are cleared or modified before send!
        rt.block_on(coord.clear()).unwrap();

        // Send must be rejected as unauthorized because generation changed
        let res = rt.block_on(coord.send(&ready, "hello"));
        assert_eq!(res.unwrap_err(), ApiError::Unauthorized);
        assert!(api.state.lock().unwrap().last_sent_chat.is_none());
    }

    #[test]
    fn concurrent_begin_and_save_barrier_rejects_stale_begin() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();

        api.state.lock().unwrap().begin_device_barrier = Some((entered_tx, release_rx));

        let coord = Arc::new(test_coordinator(backend.clone(), api));
        let coord_clone = coord.clone();

        let begin_task =
            rt.spawn(async move { coord_clone.begin_device_for_role(OAuthRole::Bot).await });

        rt.block_on(async {
            entered_rx.await.unwrap();
            // In between snapshot_creds and begin installing the session, save_client runs:
            coord
                .save_client("new_client".to_string(), Some("new_secret".to_string()))
                .await
                .unwrap();
            let _ = release_tx.send(());
        });

        let res = rt.block_on(begin_task).unwrap();
        assert_eq!(res.unwrap_err(), AuthError::StaleSession);
        // Busy must be false since stale session was rejected and dropped
        assert!(!rt.block_on(coord.status()).busy);
    }

    #[test]
    fn concurrent_prepare_delivery_and_clear_barrier_rejects_stale_validation() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();

        api.state.lock().unwrap().validate_user_barrier = Some((entered_tx, release_rx));

        let coord = Arc::new(test_coordinator(backend.clone(), api));
        let coord_clone = coord.clone();

        let prep_task = rt.spawn(async move { coord_clone.prepare_delivery().await });

        rt.block_on(async {
            entered_rx.await.unwrap();
            // During validation, user clears credentials:
            coord.clear().await.unwrap();
            let _ = release_tx.send(());
        });

        let res = rt.block_on(prep_task).unwrap();
        assert_eq!(res.unwrap_err(), AuthError::StaleSession);
        // Stale validation must not have repopulated credentials into cleared backend
        assert!(backend.snapshot().is_none());
    }

    #[test]
    fn begin_timeout_without_finish_drops_session_and_frees_busy() {
        let rt = runtime();
        rt.block_on(async {
            let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
            let coord = test_coordinator(backend, Arc::new(FakeApi::new()))
                .with_operation_timeout(Duration::from_millis(50));

            let begin = coord.begin_device_for_role(OAuthRole::Bot).await.unwrap();
            assert!(!begin.session_id.is_empty());
            assert!(coord.status().await.busy);

            // Wait past the operation timeout (50ms)
            tokio::time::sleep(Duration::from_millis(80)).await;

            // Timeout task must have dropped the pending session and cleared busy
            let status = coord.status().await;
            assert!(!status.busy);
            assert!(status.session.is_none());
        });
    }

    #[test]
    fn send_401_invalidates_app_token_without_generation_bump_and_next_prepare_acquires_new_token()
    {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api.clone());

        let ready = rt.block_on(coord.prepare_delivery()).unwrap();
        let gen_before = rt.block_on(coord.generation());

        // First send receives 401 Unauthorized
        api.state.lock().unwrap().send_401_once = true;
        let res = rt.block_on(coord.send(&ready, "msg1"));
        assert_eq!(res.unwrap_err(), ApiError::Unauthorized);

        // App token invalidation must NOT bump generation
        let gen_after = rt.block_on(coord.generation());
        assert_eq!(gen_before, gen_after);

        // Cached app token on disk was invalidated
        assert!(backend.snapshot().unwrap().app_token.is_none());

        // Next independent prepare_delivery succeeds and acquires fresh app token
        let ready2 = rt.block_on(coord.prepare_delivery()).unwrap();
        assert!(backend.snapshot().unwrap().app_token.is_some());

        // Next independent send succeeds
        let res2 = rt.block_on(coord.send(&ready2, "msg2"));
        assert!(res2.is_ok());
    }

    #[test]
    fn validate_active_session_succeeds_without_sending() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api.clone())
            .with_validation_interval(Duration::from_millis(10));

        let res = rt.block_on(coord.validate_active_session());
        assert!(res.is_ok());
        assert!(api.state.lock().unwrap().last_sent_chat.is_none());
    }

    #[test]
    fn validate_active_session_detects_revocation_on_400_refresh() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: 0, // force validation
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state
            .lock()
            .unwrap()
            .unauthorized_logins
            .insert("bot".to_string());
        api.state
            .lock()
            .unwrap()
            .refresh_400_for
            .insert("bot".to_string());
        let coord = test_coordinator(backend.clone(), api.clone())
            .with_validation_interval(Duration::from_millis(10));

        let res = rt.block_on(coord.validate_active_session());
        assert_eq!(res.unwrap_err(), AuthError::Revoked);
    }

    #[test]
    fn validate_active_session_detects_revocation_on_401_refresh() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: 0, // force validation
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state
            .lock()
            .unwrap()
            .unauthorized_logins
            .insert("bot".to_string());
        api.state
            .lock()
            .unwrap()
            .refresh_fails_for
            .insert("bot".to_string());
        let coord = test_coordinator(backend.clone(), api.clone())
            .with_validation_interval(Duration::from_millis(10));

        let res = rt.block_on(coord.validate_active_session());
        assert_eq!(res.unwrap_err(), AuthError::Revoked);
    }

    #[test]
    fn validate_active_session_forces_validation_every_call() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        // A very long interval makes the shared validation cache effectively
        // always fresh, which is exactly the skip condition this test guards.
        let coord = test_coordinator(backend.clone(), api.clone())
            .with_validation_interval(Duration::from_secs(3600));

        rt.block_on(coord.validate_active_session()).unwrap();
        assert_eq!(api.state.lock().unwrap().validate_user_calls, 2);

        // The cache is fresh, but the periodic validate must still perform the
        // real `/validate` calls rather than skipping until the next hour.
        rt.block_on(coord.validate_active_session()).unwrap();
        assert_eq!(api.state.lock().unwrap().validate_user_calls, 4);
    }

    #[test]
    fn refresh_success_then_transient_validate_is_retryable_and_preserves_rotated_token() {
        let transient_errors = vec![
            ApiError::Transport,
            ApiError::RateLimited {
                retry_after_seconds: Some(1),
            },
            ApiError::Http { status: 500 },
        ];
        for transient in transient_errors {
            let rt = runtime();
            let mut creds = configured_creds();
            creds.bot = Some(AccountGrant {
                user_id: "111".into(),
                login: "bot".into(),
                access_token: SecretString::from("old-access"),
                refresh_token: SecretString::from("old-refresh"),
                expires_at: now_unix() - 10,
                scopes: vec!["user:write:chat".into(), "user:bot".into()],
            });
            creds.upsert_channel(AccountGrant {
                user_id: "222".into(),
                login: "owner".into(),
                access_token: SecretString::from("bc-access"),
                refresh_token: SecretString::from("bc-refresh"),
                expires_at: now_unix() + 3600,
                scopes: vec!["channel:bot".into()],
            });
            let backend = Arc::new(MemoryBackend::new(Some(creds)));
            let api = Arc::new(FakeApi::new());
            api.state.lock().unwrap().new_refresh_token = Some("rotated-refresh".to_string());
            // Expired grant -> validate returns Unauthorized -> refresh rotates.
            api.state
                .lock()
                .unwrap()
                .expired_logins
                .insert("bot".to_string());
            // The post-refresh validate hits a transient failure.
            api.state
                .lock()
                .unwrap()
                .validate_transient
                .push_back(transient.clone());
            let coord = test_coordinator(backend.clone(), api.clone())
                .with_validation_interval(Duration::from_millis(10));

            // A transport/429/5xx after a successful refresh must NOT be
            // surfaced as Revoked; it stays a retryable Api error.
            let res = rt.block_on(coord.validate_active_session());
            assert_eq!(res.unwrap_err(), AuthError::Api(transient.clone()));

            // The rotated refresh token was persisted before the fallible validate.
            let snap = backend.snapshot().unwrap();
            assert_eq!(
                snap.bot.as_ref().unwrap().refresh_token.expose(),
                "rotated-refresh"
            );
            assert_eq!(api.state.lock().unwrap().last_sent_chat, None);

            // Recovery: once the transient clears, validation succeeds.
            let res2 = rt.block_on(coord.validate_active_session());
            assert!(res2.is_ok());
        }
    }

    #[test]
    fn validate_active_session_after_clear_returns_not_configured_without_restoring() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));

        rt.block_on(coord.clear()).unwrap();

        let res = rt.block_on(coord.validate_active_session());
        assert_eq!(res.unwrap_err(), AuthError::NotConfigured);
        // A stale validation must never repopulate the cleared store.
        assert!(backend.snapshot().is_none());
    }

    #[test]
    fn validate_active_session_transient_error_preserves_grants() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: 0,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(AccountGrant {
            user_id: "222".into(),
            login: "owner".into(),
            access_token: SecretString::from("bc-access"),
            refresh_token: SecretString::from("bc-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));

        struct TransientTestApi;
        #[async_trait::async_trait]
        impl TwitchAuthApi for TransientTestApi {
            async fn validate_user(&self, _token: &UserToken) -> Result<ValidatedToken, ApiError> {
                Err(ApiError::Transport)
            }
            async fn refresh_user(&self, _token: &mut UserToken) -> Result<(), ApiError> {
                unimplemented!()
            }
            async fn app_access(&self) -> Result<AppAccessToken, ApiError> {
                Ok(AppAccessToken::from_existing_unchecked(
                    AccessToken::from("app-token".to_string()),
                    None,
                    ClientId::from("client".to_string()),
                    ClientSecret::from("secret".to_string()),
                    None,
                    Some(Duration::from_secs(3600)),
                ))
            }
            async fn send_chat(
                &self,
                _b: &str,
                _s: &str,
                _m: &str,
                _t: &AppAccessToken,
            ) -> Result<SendOutcome, ApiError> {
                unimplemented!()
            }
        }

        let factory: ApiFactory =
            Arc::new(move |_| Ok(Arc::new(TransientTestApi) as Arc<dyn TwitchAuthApi>));
        let coord = TwitchAuthCoordinator::with_components(
            backend.clone(),
            factory,
            CancellationToken::new(),
        )
        .with_validation_interval(Duration::from_millis(10));

        let res = rt.block_on(coord.validate_active_session());
        assert_eq!(res.unwrap_err(), AuthError::Api(ApiError::Transport));
        // Grants are still present in store
        let snap = backend.snapshot().unwrap();
        assert!(snap.bot.is_some());
        assert!(!snap.channels.is_empty());
    }

    fn existing_channel_grant() -> AccountGrant {
        AccountGrant {
            user_id: "222".into(),
            login: "oldchannel".into(),
            access_token: SecretString::from("old-access"),
            refresh_token: SecretString::from("old-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        }
    }

    #[test]
    fn bot_device_persists_bot_and_preserves_channels() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.channels.push(existing_channel_grant());
        creds.selected_channel_id = Some("222".into());
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Bot, "111", "bot")));
        let coord = test_coordinator(backend.clone(), api);
        let begin = rt
            .block_on(coord.begin_device_for_role(OAuthRole::Bot))
            .unwrap();
        assert_eq!(rt.block_on(coord.status()).session.unwrap().role, "bot");
        let status = rt.block_on(coord.finish(&begin.session_id)).unwrap();
        assert_eq!(status.bot_login.as_deref(), Some("bot"));
        assert_eq!(status.selected_channel_id.as_deref(), Some("222"));
        assert_eq!(backend.snapshot().unwrap().channels.len(), 1);
    }

    #[test]
    fn reset_authorization_preserves_client_and_invalidates_pending_finish() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(grant_from_token(&fake_user_token(
            OAuthRole::Bot,
            "111",
            "bot",
        )));
        creds.channels.push(existing_channel_grant());
        creds.selected_channel_id = Some("222".into());
        creds.app_token = Some(AppToken {
            access_token: SecretString::from("app-access"),
            expires_at: now_unix() + 3600,
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds.clone())));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api);
        let begin = rt
            .block_on(coord.begin_device_for_role(OAuthRole::Bot))
            .unwrap();
        let status = rt.block_on(coord.reset_authorization()).unwrap();
        assert_eq!(status.client_id, creds.client_id);
        assert!(status.secret_configured);
        assert!(status.bot_login.is_none());
        assert_eq!(status.channels.len(), 1);
        assert_eq!(status.selected_channel_id.as_deref(), Some("222"));
        assert!(!status.busy);
        let stored = backend.snapshot().unwrap();
        assert_eq!(stored.client_secret, creds.client_secret);
        assert_eq!(stored.channels.len(), creds.channels.len());
        assert_eq!(stored.selected_channel_id, creds.selected_channel_id);
        assert!(stored.app_token.is_none());
        assert!(rt.block_on(coord.finish(&begin.session_id)).is_err());
    }

    #[test]
    fn reset_store_failure_preserves_grants() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(grant_from_token(&fake_user_token(
            OAuthRole::Bot,
            "111",
            "bot",
        )));
        creds.channels.push(existing_channel_grant());
        creds.selected_channel_id = Some("222".into());
        let backend = Arc::new(FailingBackend::new(creds));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));
        let before = rt.block_on(coord.status());
        backend
            .fail
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            rt.block_on(coord.reset_authorization()).unwrap_err(),
            AuthError::Store
        );
        let after = rt.block_on(coord.status());
        assert_eq!(after.selected_channel_id, before.selected_channel_id);
        assert_eq!(after.channels.len(), before.channels.len());
        assert!(after.secret_configured);
        assert_eq!(after.bot_login, before.bot_login);
    }

    #[test]
    fn begin_device_persists_only_broadcaster_grant() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().oauth_result = Some(Ok(fake_user_token(
            OAuthRole::Broadcaster,
            "222",
            "streamer",
        )));
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt.block_on(coord.begin_device()).unwrap();
        assert!(!begin.session_id.is_empty());

        let status = rt.block_on(coord.status());
        assert!(status.busy);
        assert_eq!(status.session.as_ref().unwrap().role, "broadcaster");

        let status = rt.block_on(coord.finish(&begin.session_id)).unwrap();
        assert!(!status.busy);
        assert_eq!(status.broadcaster_login.as_deref(), Some("streamer"));

        let creds = backend.snapshot().unwrap();
        assert!(creds.bot.is_none());
        assert_eq!(creds.channels.len(), 1);
        assert_eq!(creds.selected_channel().unwrap().login, "streamer");
        assert_eq!(creds.selected_channel_id.as_deref(), Some("222"));
        assert_eq!(status.selected_channel_id.as_deref(), Some("222"));
        assert_eq!(status.channels.len(), 1);
    }

    #[test]
    fn begin_device_rejects_same_account_as_bot() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("a"),
            refresh_token: SecretString::from("r"),
            expires_at: 0,
            scopes: vec![],
        });
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Broadcaster, "111", "bot")));
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt.block_on(coord.begin_device()).unwrap();
        let err = rt.block_on(coord.finish(&begin.session_id)).unwrap_err();
        assert_eq!(err, AuthError::SameAccount);
        let creds = backend.snapshot().unwrap();
        assert!(creds.channels.is_empty());
        assert!(creds.bot.is_some());
    }

    #[test]
    fn begin_device_denial_keeps_existing_channel_grant() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.upsert_channel(existing_channel_grant());
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().oauth_result = Some(Err(OAuthError::Denied));
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt.block_on(coord.begin_device()).unwrap();
        let err = rt.block_on(coord.finish(&begin.session_id)).unwrap_err();
        assert_eq!(err, AuthError::OAuth(OAuthError::Denied));
        // A denial never erases the previously authorized channel.
        let creds = backend.snapshot().unwrap();
        assert_eq!(creds.selected_channel().unwrap().login, "oldchannel");
    }

    #[test]
    fn begin_device_expiry_maps_to_timed_out_and_keeps_grant() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.upsert_channel(existing_channel_grant());
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().oauth_result = Some(Err(OAuthError::TimedOut));
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt.block_on(coord.begin_device()).unwrap();
        let err = rt.block_on(coord.finish(&begin.session_id)).unwrap_err();
        assert_eq!(err.code(), "twitch.api_auth.oauth.timed_out");
        let creds = backend.snapshot().unwrap();
        assert_eq!(creds.selected_channel().unwrap().login, "oldchannel");
    }

    #[test]
    fn begin_device_network_failure_maps_to_exchange_failed() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().oauth_result =
            Some(Err(OAuthError::ExchangeFailed(ApiError::Transport)));
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt.block_on(coord.begin_device()).unwrap();
        let err = rt.block_on(coord.finish(&begin.session_id)).unwrap_err();
        assert_eq!(err.code(), "twitch.api_auth.oauth.exchange_failed");
        assert!(backend.snapshot().unwrap().channels.is_empty());
    }

    #[test]
    fn device_finish_race_with_cancel_does_not_persist() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        let (waiter_tx, waiter_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();

        api.state.lock().unwrap().oauth_waiter_signal = Some(waiter_tx);
        api.state.lock().unwrap().oauth_release_signal = Some(release_rx);

        let coord = Arc::new(test_coordinator(backend.clone(), api));
        let begin = rt.block_on(coord.begin_device()).unwrap();

        let coord_clone = coord.clone();
        let session_id = begin.session_id.clone();
        let finish_handle = rt.spawn(async move { coord_clone.finish(&session_id).await });

        rt.block_on(async {
            waiter_rx.await.unwrap();
            assert!(coord.status().await.busy);
            assert!(coord.cancel(Some(&begin.session_id)).await);
            let _ = release_tx.send(());
        });

        let finish_res = rt.block_on(finish_handle).unwrap();
        assert!(matches!(
            finish_res,
            Err(AuthError::OAuth(OAuthError::Cancelled)) | Err(AuthError::StaleSession)
        ));
        assert!(backend.snapshot().unwrap().channels.is_empty());
    }

    #[test]
    fn device_begin_stale_after_save_client_cancels_pending() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt.block_on(coord.begin_device()).unwrap();
        rt.block_on(coord.save_client("new_client".to_string(), Some("new_secret".to_string())))
            .unwrap();

        let res = rt.block_on(coord.finish(&begin.session_id));
        assert!(res.is_err());
        assert!(!rt.block_on(coord.status()).busy);
        assert!(backend.snapshot().unwrap().channels.is_empty());
    }

    #[test]
    fn device_begin_stale_after_clear_cancels_pending() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt.block_on(coord.begin_device()).unwrap();
        rt.block_on(coord.clear()).unwrap();

        let res = rt.block_on(coord.finish(&begin.session_id));
        assert!(res.is_err());
        assert!(backend.snapshot().is_none());
    }

    // ── Multiple channel grants (canonical channels + selection) ──

    fn channel_grant(user_id: &str, login: &str, access: &str) -> AccountGrant {
        AccountGrant {
            user_id: user_id.into(),
            login: login.into(),
            access_token: SecretString::from(access),
            refresh_token: SecretString::from("refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["channel:bot".into()],
        }
    }

    fn two_channel_creds() -> StoredCredentials {
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(channel_grant("222", "owner", "bc-access"));
        creds.upsert_channel(channel_grant("333", "second", "bc2-access"));
        creds
    }

    struct FailingBackend {
        creds: StdMutex<Option<StoredCredentials>>,
        fail: std::sync::atomic::AtomicBool,
    }

    impl FailingBackend {
        fn new(creds: StoredCredentials) -> Self {
            Self {
                creds: StdMutex::new(Some(creds)),
                fail: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    impl CredentialsBackend for FailingBackend {
        fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
            Ok(self.creds.lock().unwrap().clone())
        }

        fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
            if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(CredentialsError::Io("persist failed".to_string()));
            }
            *self.creds.lock().unwrap() = Some(creds.clone());
            Ok(())
        }

        #[cfg(test)]
        fn clear(&self) -> Result<(), CredentialsError> {
            *self.creds.lock().unwrap() = None;
            Ok(())
        }
    }

    #[test]
    fn empty_channel_list_is_not_ready() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let coord = test_coordinator(backend, Arc::new(FakeApi::new()));
        rt.block_on(coord.forget_channel("222")).unwrap();
        rt.block_on(coord.forget_channel("333")).unwrap();
        let err = rt.block_on(coord.prepare_delivery()).unwrap_err();
        assert_eq!(err, AuthError::NotReady);
    }

    #[test]
    fn is_delivery_configured_validates_presence_of_all_prerequisites() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let coord = test_coordinator(backend, Arc::new(FakeApi::new()));
        assert!(rt.block_on(coord.is_delivery_configured()));

        rt.block_on(coord.forget_channel("222")).unwrap();
        rt.block_on(coord.forget_channel("333")).unwrap();
        assert!(!rt.block_on(coord.is_delivery_configured()));

        let backend_empty = Arc::new(MemoryBackend::new(None));
        let coord_empty = test_coordinator(backend_empty, Arc::new(FakeApi::new()));
        assert!(!rt.block_on(coord_empty.is_delivery_configured()));
    }

    #[test]
    fn select_channel_affects_prepare_delivery_broadcaster_id() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));

        let ready = rt.block_on(coord.prepare_delivery()).unwrap();
        assert_eq!(ready.broadcaster_id, "222");

        let status = rt.block_on(coord.select_channel("333")).unwrap();
        assert_eq!(status.selected_channel_id.as_deref(), Some("333"));
        assert_eq!(status.broadcaster_login.as_deref(), Some("second"));

        let ready = rt.block_on(coord.prepare_delivery()).unwrap();
        assert_eq!(ready.broadcaster_id, "333");
    }

    #[test]
    fn select_channel_unknown_id_is_rejected_and_selection_unchanged() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));

        let err = rt.block_on(coord.select_channel("999")).unwrap_err();
        assert_eq!(err, AuthError::UnknownChannel);
        assert_eq!(err.code(), "twitch.api_auth.unknown_channel");
        assert!(!err.retryable());
        assert_eq!(
            backend.snapshot().unwrap().selected_channel_id.as_deref(),
            Some("222")
        );
    }

    #[test]
    fn select_channel_is_idempotent_without_generation_bump() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let coord = test_coordinator(backend, Arc::new(FakeApi::new()));

        let before = rt.block_on(coord.generation());
        rt.block_on(coord.select_channel("222")).unwrap();
        assert_eq!(rt.block_on(coord.generation()), before);
    }

    #[test]
    fn forgetting_selected_channel_clears_selection_without_fallback() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));

        let status = rt.block_on(coord.forget_channel("222")).unwrap();
        assert!(status.selected_channel_id.is_none());
        assert!(status.broadcaster_login.is_none());
        assert_eq!(status.channels.len(), 1);
        assert_eq!(status.channels[0].user_id, "333");

        // The remaining channel is never auto-selected: delivery is not ready.
        let err = rt.block_on(coord.prepare_delivery()).unwrap_err();
        assert_eq!(err, AuthError::NotReady);

        // An explicit selection restores delivery.
        let status = rt.block_on(coord.select_channel("333")).unwrap();
        assert_eq!(status.selected_channel_id.as_deref(), Some("333"));
        assert!(rt.block_on(coord.prepare_delivery()).is_ok());
    }

    #[test]
    fn forgetting_nonselected_channel_preserves_selection() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));

        let status = rt.block_on(coord.forget_channel("333")).unwrap();
        assert_eq!(status.selected_channel_id.as_deref(), Some("222"));
        assert_eq!(status.channels.len(), 1);
        assert_eq!(status.channels[0].user_id, "222");
    }

    #[test]
    fn forgetting_unknown_channel_is_rejected() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let coord = test_coordinator(backend, Arc::new(FakeApi::new()));
        let err = rt.block_on(coord.forget_channel("999")).unwrap_err();
        assert_eq!(err, AuthError::UnknownChannel);
    }

    #[test]
    fn first_device_channel_selected_later_adds_preserve_selection() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(configured_creds())));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api.clone());

        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Broadcaster, "222", "owner")));
        let begin = rt.block_on(coord.begin_device()).unwrap();
        rt.block_on(coord.finish(&begin.session_id)).unwrap();

        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Broadcaster, "333", "second")));
        let begin = rt.block_on(coord.begin_device()).unwrap();
        rt.block_on(coord.finish(&begin.session_id)).unwrap();

        let creds = backend.snapshot().unwrap();
        assert_eq!(creds.channels.len(), 2);
        assert_eq!(creds.selected_channel_id.as_deref(), Some("222"));
    }

    #[test]
    fn reauth_same_user_updates_grant_without_duplicate() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api.clone());

        api.state.lock().unwrap().oauth_result = Some(Ok(fake_user_token(
            OAuthRole::Broadcaster,
            "333",
            "second_renamed",
        )));
        let begin = rt.block_on(coord.begin_device()).unwrap();
        rt.block_on(coord.finish(&begin.session_id)).unwrap();

        let creds = backend.snapshot().unwrap();
        assert_eq!(creds.channels.len(), 2);
        let updated = creds.channels.iter().find(|c| c.user_id == "333").unwrap();
        assert_eq!(updated.login, "second_renamed");
        // Re-auth of a nonselected channel does not change selection.
        assert_eq!(creds.selected_channel_id.as_deref(), Some("222"));
    }

    #[test]
    fn bot_cannot_equal_a_nonselected_channel() {
        let rt = runtime();
        let mut creds = configured_creds();
        creds.upsert_channel(channel_grant("333", "second", "bc2"));
        creds.upsert_channel(channel_grant("222", "owner", "bc"));
        creds.select_channel("222");
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Bot, "333", "second")));
        let coord = test_coordinator(backend.clone(), api);

        let begin = rt
            .block_on(coord.begin_device_for_role(OAuthRole::Bot))
            .unwrap();
        let err = rt.block_on(coord.finish(&begin.session_id)).unwrap_err();
        assert_eq!(err, AuthError::SameAccount);
        assert!(backend.snapshot().unwrap().bot.is_none());
    }

    #[test]
    fn new_client_id_clears_all_channels_and_selection() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));

        let status = rt
            .block_on(coord.save_client("other".to_string(), Some("secret".to_string())))
            .unwrap();
        assert!(status.channels.is_empty());
        assert!(status.selected_channel_id.is_none());
        assert!(status.broadcaster_login.is_none());

        let creds = backend.snapshot().unwrap();
        assert!(creds.channels.is_empty());
        assert!(creds.selected_channel_id.is_none());
    }

    #[test]
    fn persist_failure_leaves_selection_and_channels_unchanged() {
        let rt = runtime();
        let backend = Arc::new(FailingBackend::new(two_channel_creds()));
        let coord = test_coordinator(backend.clone(), Arc::new(FakeApi::new()));

        backend
            .fail
            .store(true, std::sync::atomic::Ordering::SeqCst);

        assert_eq!(
            rt.block_on(coord.select_channel("333")).unwrap_err(),
            AuthError::Store
        );
        assert_eq!(
            rt.block_on(coord.forget_channel("222")).unwrap_err(),
            AuthError::Store
        );

        // In-memory state is untouched.
        let status = rt.block_on(coord.status());
        assert_eq!(status.selected_channel_id.as_deref(), Some("222"));
        assert_eq!(status.channels.len(), 2);
        // Persisted state is untouched too.
        let creds = backend.creds.lock().unwrap().clone().unwrap();
        assert_eq!(creds.selected_channel_id.as_deref(), Some("222"));
        assert_eq!(creds.channels.len(), 2);
    }

    #[test]
    fn stale_device_finish_after_select_cannot_restore_grants() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api.clone());

        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Broadcaster, "444", "fourth")));
        let begin = rt.block_on(coord.begin_device()).unwrap();
        rt.block_on(coord.select_channel("333")).unwrap();

        let res = rt.block_on(coord.finish(&begin.session_id));
        assert!(res.is_err());
        let creds = backend.snapshot().unwrap();
        assert!(!creds.has_channel("444"));
        assert_eq!(creds.selected_channel_id.as_deref(), Some("333"));
    }

    #[test]
    fn stale_device_finish_after_forget_cannot_restore_channel() {
        let rt = runtime();
        let backend = Arc::new(MemoryBackend::new(Some(two_channel_creds())));
        let api = Arc::new(FakeApi::new());
        let coord = test_coordinator(backend.clone(), api.clone());

        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Broadcaster, "222", "owner")));
        let begin = rt.block_on(coord.begin_device()).unwrap();
        rt.block_on(coord.forget_channel("222")).unwrap();

        let res = rt.block_on(coord.finish(&begin.session_id));
        assert!(res.is_err());
        let creds = backend.snapshot().unwrap();
        assert!(!creds.has_channel("222"));
        assert!(creds.selected_channel_id.is_none());
    }

    #[test]
    fn refresh_updates_selected_channel_grant_not_other_channels() {
        let rt = runtime();
        let mut creds = two_channel_creds();
        let selected = creds
            .channels
            .iter_mut()
            .find(|c| c.user_id == "222")
            .unwrap();
        selected.expires_at = now_unix() - 10;
        creds.select_channel("222");
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state.lock().unwrap().new_refresh_token = Some("rotated-refresh".to_string());
        api.state
            .lock()
            .unwrap()
            .expired_logins
            .insert("owner".to_string());
        let coord = test_coordinator(backend.clone(), api.clone());

        let ready = rt.block_on(coord.prepare_delivery()).unwrap();
        assert_eq!(ready.broadcaster_id, "222");

        let creds = backend.snapshot().unwrap();
        let updated = creds.channels.iter().find(|c| c.user_id == "222").unwrap();
        assert_eq!(updated.refresh_token.expose(), "rotated-refresh");
        assert_eq!(updated.access_token.expose(), "rotated-access");
        // The other (nonselected) channel is not touched.
        let other = creds.channels.iter().find(|c| c.user_id == "333").unwrap();
        assert_eq!(other.refresh_token.expose(), "refresh");
        assert_eq!(other.access_token.expose(), "bc2-access");
        assert_eq!(creds.selected_channel_id.as_deref(), Some("222"));
    }

    #[test]
    fn unrelated_broadcaster_grant_does_not_mask_selected_channel_revocation() {
        let rt = runtime();
        // Bot plus a single selected channel whose token is revoked and whose
        // refresh token cannot recover it.
        let mut creds = configured_creds();
        creds.bot = Some(AccountGrant {
            user_id: "111".into(),
            login: "bot".into(),
            access_token: SecretString::from("bot-access"),
            refresh_token: SecretString::from("bot-refresh"),
            expires_at: now_unix() + 3600,
            scopes: vec!["user:write:chat".into(), "user:bot".into()],
        });
        creds.upsert_channel(channel_grant("222", "owner", "bc-access"));
        creds.select_channel("222");
        let backend = Arc::new(MemoryBackend::new(Some(creds)));
        let api = Arc::new(FakeApi::new());
        api.state
            .lock()
            .unwrap()
            .unauthorized_logins
            .insert("owner".to_string());
        api.state
            .lock()
            .unwrap()
            .refresh_fails_for
            .insert("owner".to_string());
        let coord = test_coordinator(backend.clone(), api.clone());

        // A different broadcaster authorizes successfully; the selection stays
        // on the revoked "222" channel and no duplicate grant is created.
        api.state.lock().unwrap().oauth_result =
            Some(Ok(fake_user_token(OAuthRole::Broadcaster, "333", "second")));
        let begin = rt.block_on(coord.begin_device()).unwrap();
        let status = rt.block_on(coord.finish(&begin.session_id)).unwrap();
        assert_eq!(status.selected_channel_id.as_deref(), Some("222"));
        assert_eq!(status.channels.len(), 2);

        // The unrelated authorization must not open a fresh validation-cache
        // window for the selected channel: prepare_delivery validates "222" and
        // surfaces the revocation instead of skipping the check.
        let err = rt.block_on(coord.prepare_delivery()).unwrap_err();
        assert_eq!(err, AuthError::Revoked);

        // The selected channel is retained and "333" was added exactly once.
        let creds = backend.snapshot().unwrap();
        assert_eq!(creds.selected_channel_id.as_deref(), Some("222"));
        assert_eq!(creds.channels.len(), 2);
        assert!(creds.has_channel("222"));
        assert_eq!(
            creds.channels.iter().filter(|c| c.user_id == "333").count(),
            1
        );
    }
}
