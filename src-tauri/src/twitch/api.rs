//! Thin adapter over the published `twitch_api` and `twitch_oauth2` crates
//! (ROADMAP-132).
//!
//! These are third-party twitch-rs community libraries, **not** an official
//! Twitch SDK. The adapter owns only the HTTP transport (`TwitchHttpClient`,
//! a `reqwest` 0.13 client with redirects disabled and a finite timeout) and
//! the mapping of library errors to the stable, non-secret [`ApiError`] codes.
//! Protocol construction and response parsing are delegated to the libraries:
//! no hand-written endpoint, request or response models live here.
//!
//! Message POSTs are never retried automatically: a transport error or a 5xx
//! response leaves the send ambiguous, and the caller must not replay.

use std::time::Duration;

use twitch_api::helix::chat::send_chat_message::{
    ChatMessageDropCode, SendChatMessageBody, SendChatMessageRequest, SendChatMessageResponse,
};
#[cfg(test)]
use twitch_api::helix::users::get_users::GetUsersRequest;
use twitch_api::helix::{HelixClient, HelixRequestGetError, HelixRequestPostError};
use twitch_oauth2::tokens::errors::{AppAccessTokenError, RefreshTokenError, ValidationError};
use twitch_oauth2::{
    AppAccessToken, ClientId, ClientSecret, TwitchToken, UserToken, ValidatedToken,
};

use super::credentials::SecretString;
use super::MAX_MESSAGE_CHARS;

/// Default per-request timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Stable, non-secret API error codes. Never carries a raw response body or a
/// credential-containing URL.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error("twitch.api.unauthorized")]
    Unauthorized,
    #[error("twitch.api.forbidden")]
    Forbidden,
    #[error("twitch.api.rate_limited")]
    RateLimited { retry_after_seconds: Option<u64> },
    #[error("twitch.api.http.{status}")]
    Http { status: u16 },
    #[error("twitch.api.transport")]
    Transport,
    #[error("twitch.api.malformed")]
    Malformed,
    #[error("twitch.api.invalid_input")]
    InvalidInput,
    #[error("twitch.api.validation.{code}")]
    Validation { code: &'static str },
    #[error("twitch.api.dropped.{reason}")]
    Dropped { reason: DropReason },
}

/// Why Twitch dropped a chat message (`drop_reason.code`), mapped to stable
/// variants. Unknown codes become a static fallback; never expose the free-text
/// `message` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropReason {
    Duplicate,
    RateLimit,
    Rejected,
    TimedOut,
    Blocked,
    Suspended,
    Unknown,
}

impl DropReason {
    pub fn code(&self) -> &str {
        match self {
            DropReason::Duplicate => "msg_duplicate",
            DropReason::RateLimit => "msg_ratelimit",
            DropReason::Rejected => "msg_rejected",
            DropReason::TimedOut => "msg_timedout",
            DropReason::Blocked => "msg_channel_blocked",
            DropReason::Suspended => "msg_channel_suspended",
            DropReason::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for DropReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

/// Outcome of a send-chat-message call, projected from the library's typed
/// `SendChatMessageResponse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendOutcome {
    pub message_id: Option<String>,
    pub is_sent: bool,
    pub drop_reason: Option<DropReason>,
}

/// Narrowly scoped HTTP transport for the twitch-rs libraries. A `reqwest`
/// 0.13 client is kept separate from the application's `reqwest` 0.12 so the
/// whole app networking is not upgraded. Redirects are disabled and a finite
/// timeout is applied at construction time.
#[derive(Clone)]
pub struct TwitchHttpClient(reqwest_twitch::Client);

impl TwitchHttpClient {
    fn build(timeout: Duration) -> Result<Self, ApiError> {
        let client = reqwest_twitch::Client::builder()
            .redirect(reqwest_twitch::redirect::Policy::none())
            .retry(reqwest_twitch::retry::never())
            .timeout(timeout)
            .connect_timeout(timeout)
            .build()
            .map_err(|_| ApiError::Transport)?;
        Ok(Self(client))
    }
}

impl twitch_api::client::Client for TwitchHttpClient {
    type Error = reqwest_twitch::Error;

    fn req(
        &self,
        request: twitch_api::client::Request,
    ) -> impl std::future::Future<Output = Result<twitch_api::client::Response, Self::Error>>
           + Send
           + use<> {
        let client = self.0.clone();
        async move {
            let request = reqwest_twitch::Request::try_from(request)?;
            let mut response = client.execute(request).await?;
            let status = response.status();
            let version = response.version();
            let mut builder = http::Response::builder().status(status).version(version);
            {
                let headers = builder
                    .headers_mut()
                    .expect("a new http response builder always has headers");
                std::mem::swap(headers, response.headers_mut());
            }
            let body = response.bytes().await?;
            Ok(builder
                .body(body)
                .expect("an http response with a body should always build"))
        }
    }
}

/// Twitch Helix / OAuth client adapter backed by the twitch-rs libraries.
#[derive(Clone)]
pub struct TwitchApi {
    client: HelixClient<'static, TwitchHttpClient>,
    client_id: ClientId,
    client_secret: ClientSecret,
}

impl TwitchApi {
    pub fn new(
        client_id: String,
        client_secret: SecretString,
        timeout: Duration,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            client: HelixClient::with_client(TwitchHttpClient::build(timeout)?),
            client_id: ClientId::from(client_id),
            client_secret: ClientSecret::from(client_secret.expose().to_string()),
        })
    }

    pub fn client_id(&self) -> &ClientId {
        &self.client_id
    }

    pub fn client_secret(&self) -> &ClientSecret {
        &self.client_secret
    }

    /// The underlying [`HelixClient`], which also serves as the OAuth2 HTTP
    /// transport via its `twitch_oauth2::client::Client` implementation.
    pub fn helix(&self) -> &HelixClient<'static, TwitchHttpClient> {
        &self.client
    }

    /// Obtain an app access token via the Client Credentials flow.
    pub async fn client_credentials(&self) -> Result<AppAccessToken, ApiError> {
        AppAccessToken::get_app_access_token(
            &self.client,
            self.client_id.clone(),
            self.client_secret.clone(),
            Vec::new(),
        )
        .await
        .map_err(map_app_access_token_error)
    }

    /// Validate a user token, returning Twitch's validated identity.
    pub async fn validate_user_token(&self, token: &UserToken) -> Result<ValidatedToken, ApiError> {
        token
            .validate_token(&self.client)
            .await
            .map_err(map_validation_error)
    }

    /// Validate an app access token, returning Twitch's validated token information.
    pub async fn validate_app_token(
        &self,
        token: &AppAccessToken,
    ) -> Result<ValidatedToken, ApiError> {
        token
            .validate_token(&self.client)
            .await
            .map_err(map_validation_error)
    }

    /// Refresh a user token in place.
    pub async fn refresh_user_token(&self, token: &mut UserToken) -> Result<(), ApiError> {
        token
            .refresh_token(&self.client)
            .await
            .map_err(map_refresh_error)
    }

    /// Send a chat message as the bot on the target channel.
    ///
    /// The message must be non-empty and at most [`MAX_MESSAGE_CHARS`] unicode
    /// characters. The body is never logged and always sets `for_source_only`
    /// to `true`. Returns `Ok` only when Twitch reports `is_sent = true`; a 200
    /// response with `is_sent = false` becomes [`ApiError::Dropped`]. This call
    /// is never retried automatically.
    pub async fn send_chat_message(
        &self,
        broadcaster_id: &str,
        sender_id: &str,
        message: &str,
        app_token: &AppAccessToken,
    ) -> Result<SendOutcome, ApiError> {
        validate_message(message)?;

        let request = SendChatMessageRequest::new();
        let body =
            SendChatMessageBody::new(broadcaster_id, sender_id, message).for_source_only(true);
        let response = self
            .client
            .req_post(request, body, app_token)
            .await
            .map_err(map_client_request_error)?;
        classify_send_response(response.data)
    }
}

fn validate_message(text: &str) -> Result<(), ApiError> {
    let chars = text.chars().count();
    if chars == 0 || chars > MAX_MESSAGE_CHARS {
        return Err(ApiError::InvalidInput);
    }
    Ok(())
}

fn map_http_status(status: http::StatusCode) -> ApiError {
    match status.as_u16() {
        401 => ApiError::Unauthorized,
        403 => ApiError::Forbidden,
        429 => ApiError::RateLimited {
            retry_after_seconds: None,
        },
        other => ApiError::Http { status: other },
    }
}

/// Maps a twitch_oauth2 response parse error to a stable code. The library
/// hides raw response headers, so 429 retry-after is always `None` here.
pub(crate) fn map_oauth_parse_error(e: &twitch_oauth2::RequestParseError) -> ApiError {
    match e {
        twitch_oauth2::RequestParseError::TwitchError(err) => map_http_status(err.status),
        twitch_oauth2::RequestParseError::Other(status) => map_http_status(*status),
        _ => ApiError::Malformed,
    }
}

pub(crate) fn map_validation_error<RE: std::error::Error + Send + Sync + 'static>(
    e: ValidationError<RE>,
) -> ApiError {
    match e {
        ValidationError::NotAuthorized => ApiError::Unauthorized,
        ValidationError::RequestParseError(pe) => map_oauth_parse_error(&pe),
        ValidationError::InvalidToken(_) => ApiError::Validation {
            code: "invalid_token",
        },
        ValidationError::Request(_) => ApiError::Transport,
        _ => ApiError::Malformed,
    }
}

fn map_app_access_token_error<RE: std::error::Error + Send + Sync + 'static>(
    e: AppAccessTokenError<RE>,
) -> ApiError {
    match e {
        AppAccessTokenError::Request(_) => ApiError::Transport,
        AppAccessTokenError::RequestParseError(pe) => map_oauth_parse_error(&pe),
        _ => ApiError::Malformed,
    }
}

fn map_refresh_error<RE: std::error::Error + Send + Sync + 'static>(
    e: RefreshTokenError<RE>,
) -> ApiError {
    match e {
        RefreshTokenError::RequestError(_) => ApiError::Transport,
        RefreshTokenError::RequestParseError(pe) => map_oauth_parse_error(&pe),
        RefreshTokenError::NoClientSecretFound
        | RefreshTokenError::NoRefreshToken
        | RefreshTokenError::NoExpiration => ApiError::Validation {
            code: "refresh_unavailable",
        },
        _ => ApiError::Malformed,
    }
}

fn map_client_request_error<RE: std::error::Error + Send + Sync + 'static>(
    e: twitch_api::helix::ClientRequestError<RE>,
) -> ApiError {
    match e {
        twitch_api::helix::ClientRequestError::RequestError(_)
        | twitch_api::helix::ClientRequestError::HyperError(_) => ApiError::Transport,
        twitch_api::helix::ClientRequestError::HelixRequestGetError(e) => map_helix_get_error(e),
        twitch_api::helix::ClientRequestError::HelixRequestPostError(e) => map_helix_post_error(e),
        _ => ApiError::Malformed,
    }
}

fn map_helix_get_error(e: HelixRequestGetError) -> ApiError {
    match e {
        HelixRequestGetError::Error { status, .. } => map_http_status(status),
        _ => ApiError::Malformed,
    }
}

fn map_helix_post_error(e: HelixRequestPostError) -> ApiError {
    match e {
        HelixRequestPostError::Error { status, .. } => map_http_status(status),
        _ => ApiError::Malformed,
    }
}

fn classify_send_response(resp: SendChatMessageResponse) -> Result<SendOutcome, ApiError> {
    let message_id = resp.message_id.map(|id| id.as_str().to_string());
    let drop_reason = resp.drop_reason.map(|d| map_drop_code(&d.code));
    let outcome = SendOutcome {
        message_id,
        is_sent: resp.is_sent,
        drop_reason,
    };
    if outcome.is_sent {
        Ok(outcome)
    } else {
        Err(ApiError::Dropped {
            reason: outcome.drop_reason.unwrap_or(DropReason::Unknown),
        })
    }
}

fn map_drop_code(code: &ChatMessageDropCode) -> DropReason {
    match code {
        ChatMessageDropCode::MsgDuplicate => DropReason::Duplicate,
        ChatMessageDropCode::MsgRatelimit => DropReason::RateLimit,
        ChatMessageDropCode::MsgRejected | ChatMessageDropCode::MsgRejectedMandatory => {
            DropReason::Rejected
        }
        ChatMessageDropCode::MsgTimedout => DropReason::TimedOut,
        ChatMessageDropCode::MsgChannelBlocked => DropReason::Blocked,
        ChatMessageDropCode::MsgSuspended | ChatMessageDropCode::MsgChannelSuspended => {
            DropReason::Suspended
        }
        _ => DropReason::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use twitch_api::helix::Request;

    // ---------- library request serialization ----------

    #[test]
    fn send_chat_message_body_serializes_for_source_only_true() {
        let body = SendChatMessageBody::new("111", "222", "hello").for_source_only(true);
        let value = serde_json::to_value(&body).unwrap();
        assert_eq!(value["broadcaster_id"], json!("111"));
        assert_eq!(value["sender_id"], json!("222"));
        assert_eq!(value["message"], json!("hello"));
        assert_eq!(value["for_source_only"], json!(true));
    }

    #[test]
    fn get_users_request_uri_contains_login() {
        let request = GetUsersRequest::logins(&["justintv"]);
        let uri = request.get_uri().unwrap().to_string();
        assert!(uri.starts_with("https://api.twitch.tv/helix/users?"));
        assert!(uri.contains("login=justintv"));
    }

    // ---------- thin adapter classification ----------

    #[test]
    fn classify_send_response_rejects_is_sent_false() {
        let resp: SendChatMessageResponse = serde_json::from_value(json!({
            "message_id": "",
            "is_sent": false,
            "drop_reason": {"code": "msg_duplicate", "message": "dup"}
        }))
        .unwrap();
        assert_eq!(
            classify_send_response(resp),
            Err(ApiError::Dropped {
                reason: DropReason::Duplicate
            })
        );
    }

    #[test]
    fn classify_send_response_accepts_is_sent_true() {
        let resp: SendChatMessageResponse =
            serde_json::from_value(json!({"message_id": "abc-123", "is_sent": true})).unwrap();
        let outcome = classify_send_response(resp).unwrap();
        assert!(outcome.is_sent);
        assert_eq!(outcome.message_id.as_deref(), Some("abc-123"));
        assert!(outcome.drop_reason.is_none());
    }

    #[test]
    fn classify_send_response_unknown_drop_code_is_safe() {
        let resp: SendChatMessageResponse = serde_json::from_value(json!({
            "message_id": "",
            "is_sent": false,
            "drop_reason": {"code": "something_new", "message": "mystery"}
        }))
        .unwrap();
        assert_eq!(
            classify_send_response(resp),
            Err(ApiError::Dropped {
                reason: DropReason::Unknown
            })
        );
    }

    #[test]
    fn drop_code_maps_known_and_unknown() {
        let code: ChatMessageDropCode = serde_json::from_str("\"msg_duplicate\"").unwrap();
        assert_eq!(map_drop_code(&code), DropReason::Duplicate);

        let code: ChatMessageDropCode = serde_json::from_str("\"msg_rejected_mandatory\"").unwrap();
        assert_eq!(map_drop_code(&code), DropReason::Rejected);

        let code: ChatMessageDropCode = serde_json::from_str("\"msg_timedout\"").unwrap();
        assert_eq!(map_drop_code(&code), DropReason::TimedOut);

        let code: ChatMessageDropCode = serde_json::from_str("\"msg_channel_blocked\"").unwrap();
        assert_eq!(map_drop_code(&code), DropReason::Blocked);

        let code: ChatMessageDropCode = serde_json::from_str("\"msg_channel_suspended\"").unwrap();
        assert_eq!(map_drop_code(&code), DropReason::Suspended);

        let code: ChatMessageDropCode = serde_json::from_str("\"something_new\"").unwrap();
        assert_eq!(map_drop_code(&code), DropReason::Unknown);
    }

    #[test]
    fn http_status_maps_to_stable_codes() {
        assert_eq!(
            map_http_status(http::StatusCode::UNAUTHORIZED),
            ApiError::Unauthorized
        );
        assert_eq!(
            map_http_status(http::StatusCode::FORBIDDEN),
            ApiError::Forbidden
        );
        assert_eq!(
            map_http_status(http::StatusCode::TOO_MANY_REQUESTS),
            ApiError::RateLimited {
                retry_after_seconds: None
            }
        );
        assert_eq!(
            map_http_status(http::StatusCode::INTERNAL_SERVER_ERROR),
            ApiError::Http { status: 500 }
        );
    }

    #[test]
    fn message_length_validates_unicode_boundary() {
        assert!(validate_message(&"a".repeat(500)).is_ok());
        assert!(validate_message(&"я".repeat(500)).is_ok());
        assert_eq!(validate_message(""), Err(ApiError::InvalidInput));
        assert_eq!(
            validate_message(&"я".repeat(501)),
            Err(ApiError::InvalidInput)
        );
    }

    #[test]
    fn drop_reason_stable_codes() {
        assert_eq!(DropReason::Duplicate.code(), "msg_duplicate");
        assert_eq!(DropReason::RateLimit.code(), "msg_ratelimit");
        assert_eq!(DropReason::Rejected.code(), "msg_rejected");
    }
}
