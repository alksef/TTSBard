//! Twitch OAuth Device Code flow (for bot and channel accounts).
//!
//! This module owns bot and broadcaster device authorization: it starts the
//! device code request through `twitch_oauth2::tokens::DeviceUserTokenBuilder`
//! and polls for the resulting user token. The SDK owns the protocol, request
//! serialization and response parsing; no endpoint, form body or JSON model is
//! hand-written here.
//!
//! Unlike the loopback flow there is no redirect URI and no local listener. The
//! only value exposed to the caller (and therefore the UI) is the Twitch
//! `verification_uri`, which already embeds the one-time user code. The
//! `device_code` never leaves this module.
//!
//! # Bounds
//!
//! Polling honours the interval returned by Twitch (with a positive minimum to
//! avoid a busy loop), stops on the Twitch expiry, and aborts promptly when the
//! supplied cancellation token fires. The caller additionally bounds the whole
//! operation with the shared 300-second test timeout.

use std::time::Duration;

use tokio_util::sync::CancellationToken;
use twitch_oauth2::tokens::errors::DeviceUserTokenExchangeError;
use twitch_oauth2::tokens::DeviceUserTokenBuilder;
use twitch_oauth2::{ClientId, ClientSecret, UserToken};

use super::api::{ApiError, TwitchApi};
use super::oauth::{OAuthError, OAuthRole};

/// Minimum poll delay even if Twitch returns `interval = 0`.
const MIN_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Configures and begins device authorization for an account role.
pub struct DeviceFlow {
    api: TwitchApi,
}

impl DeviceFlow {
    pub fn new(api: TwitchApi) -> Self {
        Self { api }
    }

    /// Request a device code and return the pending authorization. The user
    /// code is embedded in the returned `verification_uri`.
    pub async fn begin(&self, role: OAuthRole) -> Result<PendingDeviceAuthorization, OAuthError> {
        let mut builder = device_builder(
            self.api.client_id().clone(),
            self.api.client_secret().clone(),
            role,
        );
        let (verification_uri, interval, expires_in) = {
            let code = builder
                .start(self.api.helix())
                .await
                .map_err(map_device_error)?;
            (
                code.verification_uri.to_string(),
                code.interval,
                code.expires_in,
            )
        };
        Ok(PendingDeviceAuthorization {
            builder,
            api: self.api.clone(),
            verification_uri,
            interval: poll_interval(interval),
            expires_in: Duration::from_secs(expires_in),
            started: std::time::Instant::now(),
        })
    }
}

/// Build the device builder with the exact role scopes and
/// the confidential client secret (so the issued token can be refreshed).
fn device_builder(
    client_id: ClientId,
    secret: ClientSecret,
    role: OAuthRole,
) -> DeviceUserTokenBuilder {
    let mut builder = DeviceUserTokenBuilder::new(client_id, role.scopes().to_vec());
    builder.set_secret(Some(secret));
    builder
}

/// A positive poll delay derived from the Twitch-provided interval.
fn poll_interval(seconds: u64) -> Duration {
    Duration::from_secs(seconds.max(MIN_POLL_INTERVAL.as_secs()))
}

/// An in-flight device authorization awaiting the user's confirmation.
pub struct PendingDeviceAuthorization {
    builder: DeviceUserTokenBuilder,
    api: TwitchApi,
    verification_uri: String,
    interval: Duration,
    expires_in: Duration,
    started: std::time::Instant,
}

impl PendingDeviceAuthorization {
    /// The Twitch activation URL (already contains the one-time user code).
    pub fn verification_uri(&self) -> &str {
        &self.verification_uri
    }

    /// Poll Twitch until the user confirms, Twitch expires the device code or
    /// `cancel` fires.
    pub async fn wait_for_token(&self, cancel: CancellationToken) -> Result<UserToken, OAuthError> {
        let deadline = self.started + self.expires_in;
        let mut interval = self.interval;
        loop {
            // The expiry is checked before every exchange request: once the
            // deadline has passed no further HTTP request is issued.
            if std::time::Instant::now() >= deadline {
                return Err(OAuthError::TimedOut);
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let attempt = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(OAuthError::Cancelled),
                _ = tokio::time::sleep(remaining) => return Err(OAuthError::TimedOut),
                result = self.builder.try_finish(self.api.helix()) => result,
            };
            match attempt {
                Ok(token) => return Ok(token),
                Err(ref error) if error.is_pending() => {
                    let now = std::time::Instant::now();
                    if now >= deadline {
                        return Err(OAuthError::TimedOut);
                    }
                    let wait = (deadline - now).min(interval);
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => return Err(OAuthError::Cancelled),
                        _ = tokio::time::sleep(wait) => {}
                    }
                }
                Err(ref error) if is_slow_down(error) => {
                    // RFC 8628 §3.5: MUST increase polling interval by 5 seconds
                    interval += Duration::from_secs(5);
                    let now = std::time::Instant::now();
                    if now >= deadline {
                        return Err(OAuthError::TimedOut);
                    }
                    let wait = (deadline - now).min(interval);
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => return Err(OAuthError::Cancelled),
                        _ = tokio::time::sleep(wait) => {}
                    }
                }
                Err(error) => return Err(map_device_error(error)),
            }
        }
    }
}

/// Check whether the exchange error indicates RFC 8628 §3.5 `slow_down`.
fn is_slow_down<RE: std::error::Error + Send + Sync + 'static>(
    error: &DeviceUserTokenExchangeError<RE>,
) -> bool {
    use DeviceUserTokenExchangeError as E;
    match error {
        E::DeviceExchangeParseError(pe) | E::TokenParseError(pe) => is_parse_slow_down(pe),
        _ => false,
    }
}

fn is_parse_slow_down(error: &twitch_oauth2::RequestParseError) -> bool {
    if let twitch_oauth2::RequestParseError::TwitchError(e) = error {
        for code in [e.error.as_deref(), Some(e.message.as_str())]
            .into_iter()
            .flatten()
        {
            if code == "slow_down" {
                return true;
            }
        }
    }
    false
}

/// Map the SDK device error to a stable, non-secret [`OAuthError`]. Raw SDK
/// responses are never surfaced.
fn map_device_error<RE: std::error::Error + Send + Sync + 'static>(
    error: DeviceUserTokenExchangeError<RE>,
) -> OAuthError {
    use DeviceUserTokenExchangeError as E;
    match error {
        E::Expired => OAuthError::TimedOut,
        E::NoDeviceCode => OAuthError::Malformed,
        E::ValidationError(v) => OAuthError::ExchangeFailed(super::api::map_validation_error(v)),
        E::DeviceExchangeRequestError(_) | E::TokenRequestError(_) => {
            OAuthError::ExchangeFailed(ApiError::Transport)
        }
        E::DeviceExchangeParseError(pe) | E::TokenParseError(pe) => map_device_parse_error(&pe),
        _ => OAuthError::ExchangeFailed(ApiError::Malformed),
    }
}

/// Recognize the known device-flow denial/expiry codes before falling back to
/// the generic status mapping. Only fixed protocol codes are compared; the
/// free-text portion of the Twitch error is never surfaced.
fn map_device_parse_error(error: &twitch_oauth2::RequestParseError) -> OAuthError {
    if let twitch_oauth2::RequestParseError::TwitchError(e) = error {
        for code in [e.error.as_deref(), Some(e.message.as_str())]
            .into_iter()
            .flatten()
        {
            match code {
                "access_denied" => return OAuthError::Denied,
                "expired_token" => return OAuthError::TimedOut,
                _ => {}
            }
        }
    }
    OAuthError::ExchangeFailed(super::api::map_oauth_parse_error(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use twitch_oauth2::Scope;

    fn test_api() -> TwitchApi {
        TwitchApi::new(
            "client".to_string(),
            super::super::credentials::SecretString::from("super-secret-client-secret"),
            super::super::api::DEFAULT_TIMEOUT,
        )
        .unwrap()
    }

    /// Parse the request target with the SDK's own `url` crate and expose its
    /// query pairs. The SDK places all device-flow parameters in the URL query
    /// and leaves the body empty, so assertions must read the query.
    fn query_pairs(request: &http::Request<Vec<u8>>) -> Vec<(String, String)> {
        let url = twitch_oauth2::url::Url::parse(&request.uri().to_string())
            .expect("device request URI is an absolute URL");
        url.query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect()
    }

    fn query_value(request: &http::Request<Vec<u8>>, key: &str) -> Option<String> {
        query_pairs(request)
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, value)| value)
    }

    #[test]
    fn device_code_request_targets_device_endpoint_without_redirect() {
        let api = test_api();
        let builder = device_builder(
            api.client_id().clone(),
            api.client_secret().clone(),
            OAuthRole::Broadcaster,
        );
        let request = builder.get_exchange_device_code_request();

        assert_eq!(request.method(), http::Method::POST);
        assert_eq!(request.uri().host(), Some("id.twitch.tv"));
        assert_eq!(request.uri().path(), "/oauth2/device");
        assert!(request.body().is_empty(), "device request has no body");

        assert_eq!(
            query_value(&request, "client_id").as_deref(),
            Some("client")
        );
        assert_eq!(
            query_value(&request, "scopes").as_deref(),
            Some("channel:bot")
        );
        assert!(query_value(&request, "redirect_uri").is_none());
        assert!(query_value(&request, "client_secret").is_none());
    }

    #[test]
    fn broadcaster_builder_only_requests_channel_bot_scope() {
        let api = test_api();
        let builder = device_builder(
            api.client_id().clone(),
            api.client_secret().clone(),
            OAuthRole::Broadcaster,
        );
        let request = builder.get_exchange_device_code_request();
        let scopes = query_value(&request, "scopes").expect("scopes parameter present");
        assert_eq!(scopes, "channel:bot");
        assert!(!scopes.contains("user:write:chat"));
        assert!(!scopes.contains("user:bot"));
    }

    #[test]
    fn bot_device_requests_only_bot_scopes_without_redirect() {
        let api = test_api();
        let builder = device_builder(
            api.client_id().clone(),
            api.client_secret().clone(),
            OAuthRole::Bot,
        );
        let request = builder.get_exchange_device_code_request();
        let scopes = query_value(&request, "scopes").unwrap();
        let scopes: Vec<_> = scopes.split_whitespace().collect();
        assert_eq!(scopes.len(), 2);
        assert!(scopes.contains(&"user:write:chat"));
        assert!(scopes.contains(&"user:bot"));
        assert!(query_value(&request, "redirect_uri").is_none());
    }

    #[test]
    fn token_exchange_request_uses_device_code_without_redirect() {
        let api = test_api();
        let mut builder = device_builder(
            api.client_id().clone(),
            api.client_secret().clone(),
            OAuthRole::Broadcaster,
        );
        let body = br#"{
            "device_code": "device-abc",
            "expires_in": 1800,
            "interval": 5,
            "user_code": "ABCDEFGH",
            "verification_uri": "https://www.twitch.tv/activate?public=true&device-code=ABCDEFGH"
        }"#;
        let response = http::Response::builder()
            .status(200)
            .body(body.to_vec())
            .unwrap();
        let parsed = builder
            .parse_exchange_device_code_response(response)
            .unwrap();
        assert_eq!(
            parsed.verification_uri.to_string(),
            "https://www.twitch.tv/activate?public=true&device-code=ABCDEFGH"
        );
        assert_eq!(parsed.interval, 5);
        assert_eq!(parsed.expires_in, 1800);

        let request = builder
            .get_user_token_request()
            .expect("device code parsed above");
        assert_eq!(request.method(), http::Method::POST);
        assert_eq!(request.uri().host(), Some("id.twitch.tv"));
        assert_eq!(request.uri().path(), "/oauth2/token");
        assert!(request.body().is_empty(), "token request has no body");

        assert_eq!(
            query_value(&request, "device_code").as_deref(),
            Some("device-abc")
        );
        assert_eq!(
            query_value(&request, "grant_type").as_deref(),
            Some("urn:ietf:params:oauth:grant-type:device_code")
        );
        assert!(query_value(&request, "redirect_uri").is_none());
        assert!(query_value(&request, "client_secret").is_none());
    }

    fn pending_authorization(
        api: TwitchApi,
        expires_in: Duration,
        started: std::time::Instant,
    ) -> PendingDeviceAuthorization {
        let builder = device_builder(
            api.client_id().clone(),
            api.client_secret().clone(),
            OAuthRole::Broadcaster,
        );
        PendingDeviceAuthorization {
            builder,
            api,
            verification_uri: "https://www.twitch.tv/activate?public=true&device-code=TEST"
                .to_string(),
            interval: MIN_POLL_INTERVAL,
            expires_in,
            started,
        }
    }

    #[tokio::test]
    async fn expired_pending_returns_timed_out_without_request() {
        // `started` is already past the deadline: the exchange must not run.
        let pending = pending_authorization(
            test_api(),
            Duration::from_secs(1),
            std::time::Instant::now() - Duration::from_secs(5),
        );
        let result = pending.wait_for_token(CancellationToken::new()).await;
        assert!(matches!(result, Err(OAuthError::TimedOut)));
    }

    #[tokio::test]
    async fn pre_cancelled_wait_returns_cancelled_without_request() {
        // A builder without a device code would map to `Malformed` if the
        // exchange ran; the biased cancellation must win over the request.
        let pending = pending_authorization(
            test_api(),
            Duration::from_secs(1800),
            std::time::Instant::now(),
        );
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = pending.wait_for_token(cancel).await;
        assert!(matches!(result, Err(OAuthError::Cancelled)));
    }

    fn synthetic_device_error(
        message: &str,
    ) -> DeviceUserTokenExchangeError<std::convert::Infallible> {
        DeviceUserTokenExchangeError::TokenParseError(
            twitch_oauth2::RequestParseError::TwitchError(
                twitch_oauth2::id::TwitchTokenErrorResponse {
                    status: http::StatusCode::BAD_REQUEST,
                    message: message.to_string(),
                    error: None,
                },
            ),
        )
    }

    #[test]
    fn device_access_denied_maps_to_denied() {
        assert_eq!(
            map_device_error(synthetic_device_error("access_denied")),
            OAuthError::Denied
        );
    }

    #[test]
    fn device_expired_token_maps_to_timed_out() {
        assert_eq!(
            map_device_error(synthetic_device_error("expired_token")),
            OAuthError::TimedOut
        );
    }

    #[test]
    fn device_unknown_twitch_error_never_exposes_raw_message() {
        let error = map_device_error(synthetic_device_error("some raw server text"));
        assert!(!error.to_string().contains("some raw server text"));
    }

    #[test]
    fn poll_interval_enforces_positive_minimum() {
        assert_eq!(poll_interval(0), MIN_POLL_INTERVAL);
        assert_eq!(poll_interval(5), Duration::from_secs(5));
    }

    #[test]
    fn broadcaster_role_scopes_are_channel_bot_only() {
        assert_eq!(OAuthRole::Broadcaster.scopes(), &[Scope::ChannelBot]);
    }

    #[test]
    fn device_slow_down_detected_from_error() {
        let err = synthetic_device_error("slow_down");
        assert!(is_slow_down(&err));
        let other = synthetic_device_error("access_denied");
        assert!(!is_slow_down(&other));
    }
}
