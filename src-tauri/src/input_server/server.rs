use super::overlay::overlay;
pub use super::overlay::OverlayLanguage;
use super::service::InputServerService;
use crate::commands::input_server::{error_code, InputServerAccepted};
use crate::ipc::{speech as speech_contract, CommandError};
use crate::webview::security::{is_local_network, validate_token};
use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

/// Maximum accepted HTTP request body size (64 KiB).
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Stable error code used for JSON extraction failures (bad/missing body or
/// wrong/missing Content-Type).
pub const INVALID_REQUEST_CODE: &str = "input_server.invalid_request";

/// Stable error code used when the request body exceeds [`MAX_BODY_BYTES`].
pub const BODY_TOO_LARGE_CODE: &str = "input_server.body_too_large";

/// Stable error code used when the request Host header is missing or does not
/// name this listener by a loopback name or a local-network IP literal.
pub const FORBIDDEN_HOST_CODE: &str = "input_server.forbidden_host";

/// Stable error code used when a non-loopback request does not carry the valid
/// access token.
pub const UNAUTHORIZED_CODE: &str = "input_server.unauthorized";

/// Cookie carrying the input-server access token for browser clients.
///
/// Deliberately distinct from the WebView server cookie: cookies are scoped by
/// domain, not port, and the shared `localhost` origin must not leak one
/// server's token to the other.
const AUTH_COOKIE_NAME: &str = "input_server_auth";

/// Test-injectable asynchronous intake seam for the input server.
///
/// The supervisor plugs an adapter wrapping the application-level accept
/// seam here; the router itself never touches `AppHandle`.
#[async_trait::async_trait]
pub trait TextIntake: Send + Sync {
    /// Accept one validated external text and return the accepted job/inbox result.
    async fn accept(&self, text: String) -> Result<InputServerAccepted, CommandError>;
}

#[derive(Clone)]
struct RouterState {
    intake: Arc<dyn TextIntake>,
    /// Live settings for token rotation: the auth middleware reads the current
    /// token on every request, so rotating it never restarts the listener.
    service: Arc<InputServerService>,
    overlay_language: OverlayLanguage,
}

/// Incoming request contract for `POST /v1/speech`.
#[derive(Debug, Deserialize)]
struct SpeechRequest {
    text: String,
}

/// Query contract for `GET /overlay`.
#[derive(Debug, Deserialize)]
struct OverlayQuery {
    token: Option<String>,
}

/// Build the input server router.
///
/// Routes: `GET /health`, `GET /overlay`, `POST /v1/speech`. No CORS
/// middleware; body limited to [`MAX_BODY_BYTES`]. Middleware, outside in:
///
/// 1. `host_gate` — exact-match `Host` against the listener port with a
///    loopback name or a local-network IP literal, so DNS-rebinding pages
///    rebinding an attacker domain are rejected with 403 before any handler.
/// 2. `auth_gate` — loopback connections and `/health` pass freely; every
///    other connection must present the current access token (query `?token=`,
///    cookie, or `Authorization: Bearer`) and is rejected with 401 otherwise.
///
/// The router must be served through `into_make_service_with_connect_info`
/// so `auth_gate` can classify the client address; a missing connection info
/// fails closed (treated as non-loopback).
pub fn build_router(
    intake: Arc<dyn TextIntake>,
    port: u16,
    overlay_language: OverlayLanguage,
    service: Arc<InputServerService>,
) -> Router {
    let state = RouterState {
        intake,
        service,
        overlay_language,
    };
    Router::new()
        .route("/health", get(health))
        .route("/overlay", get(overlay_handler))
        .route("/v1/speech", post(post_speech))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), auth_gate))
        .layer(middleware::from_fn_with_state(port, host_gate))
        .with_state(state)
}

async fn host_gate(State(port): State<u16>, request: Request, next: Next) -> Response {
    let allowed = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| host_matches_listener(host, port));
    if allowed {
        next.run(request).await
    } else {
        let error = CommandError::new(FORBIDDEN_HOST_CODE, "Forbidden host", false);
        (StatusCode::FORBIDDEN, Json(error)).into_response()
    }
}

async fn auth_gate(State(state): State<RouterState>, request: Request, next: Next) -> Response {
    // Served through `into_make_service_with_connect_info`, so the extension is
    // present in production; its absence (test routers) fails closed as a
    // non-loopback client.
    let is_loopback = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|info| info.0.ip().is_loopback());
    // `/health` exposes nothing beyond liveness and stays reachable for LAN
    // monitors without a token.
    if is_loopback || request.uri().path() == "/health" {
        return next.run(request).await;
    }
    let stored = state.service.settings.read().await.access_token.clone();
    // An empty stored token never authenticates: fail closed.
    let stored = stored.filter(|token| !token.is_empty());
    let provided = extract_provided_token(&request);
    if validate_token(provided.as_deref(), stored.as_deref()) {
        next.run(request).await
    } else {
        let error = CommandError::new(UNAUTHORIZED_CODE, "Access token required", false);
        (StatusCode::UNAUTHORIZED, Json(error)).into_response()
    }
}

/// Collect the client-provided access token from the cookie, the
/// `Authorization: Bearer` header, or the `?token=` query parameter, in that
/// order of priority.
fn extract_provided_token(request: &Request) -> Option<String> {
    if let Some(value) = request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
    {
        if let Some(token) = cookie_value(value, AUTH_COOKIE_NAME) {
            return Some(token);
        }
    }
    if let Some(value) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        let bearer = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "));
        if let Some(token) = bearer.map(str::trim).filter(|token| !token.is_empty()) {
            return Some(token.to_string());
        }
    }
    let query = request.uri().query()?;
    for pair in query.split('&') {
        if let Some((name, value)) = pair.split_once('=') {
            if name == "token" {
                let decoded = urlencoding::decode(value).ok()?.into_owned();
                return (!decoded.is_empty()).then_some(decoded);
            }
        }
    }
    None
}

fn cookie_value(cookie_header: &str, name: &str) -> Option<String> {
    cookie_header.split(';').find_map(|pair| {
        let mut parts = pair.trim().splitn(2, '=');
        if parts.next()? == name {
            parts.next().map(str::to_string)
        } else {
            None
        }
    })
}

fn host_matches_listener(host: &str, port: u16) -> bool {
    let Some((hostname, host_port)) = host.rsplit_once(':') else {
        return false;
    };
    if host_port.parse::<u16>() != Ok(port) {
        return false;
    }
    let hostname = hostname.trim_start_matches('[').trim_end_matches(']');
    if hostname == "127.0.0.1" || hostname.eq_ignore_ascii_case("localhost") {
        return true;
    }
    // A literal local-network IP is the normal Host shape when a phone or OBS
    // opens the LAN URL. Remote literals and names stay 403 so DNS-rebinding
    // pages never reach a handler; the token gate remains the primary
    // non-loopback defense.
    match hostname.parse::<IpAddr>() {
        Ok(ip) => is_local_network(ip),
        Err(_) => false,
    }
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

async fn overlay_handler(
    State(state): State<RouterState>,
    Query(query): Query<OverlayQuery>,
) -> Response {
    let mut response = overlay(state.overlay_language).await;
    // A valid `?token=` promotes the browser client to the cookie so the
    // same-origin form POST works without a token in every request. The
    // overlay page itself is unchanged.
    if let Some(provided) = query.token.as_deref() {
        let stored = state.service.settings.read().await.access_token.clone();
        let stored = stored.filter(|token| !token.is_empty());
        if validate_token(Some(provided), stored.as_deref()) {
            let cookie = format!("{AUTH_COOKIE_NAME}={provided}; HttpOnly; Path=/; SameSite=Lax");
            if let Ok(header_value) = cookie.parse() {
                response
                    .headers_mut()
                    .insert(header::SET_COOKIE, header_value);
            }
        }
    }
    response
}

async fn post_speech(
    State(state): State<RouterState>,
    request: Result<Json<SpeechRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match request {
        Ok(ok) => ok,
        Err(rejection) => return extraction_rejection_response(rejection),
    };

    match state.intake.accept(request.text).await {
        Ok(accepted) => (StatusCode::ACCEPTED, Json(accepted)).into_response(),
        Err(error) => {
            let status = status_for_command_error(error.code);
            (status, Json(error)).into_response()
        }
    }
}

fn status_for_command_error(code: &str) -> StatusCode {
    match code {
        speech_contract::error_code::QUEUE_FULL | error_code::INBOX_FULL => {
            StatusCode::TOO_MANY_REQUESTS
        }
        speech_contract::error_code::SNAPSHOT_UNAVAILABLE => StatusCode::SERVICE_UNAVAILABLE,
        error_code::BLANK | error_code::TOO_LONG | error_code::UNKNOWN_ITEM => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        _ => StatusCode::CONFLICT,
    }
}

fn extraction_rejection_response(rejection: JsonRejection) -> Response {
    let status = rejection.status();
    let code = if status == StatusCode::PAYLOAD_TOO_LARGE {
        BODY_TOO_LARGE_CODE
    } else {
        INVALID_REQUEST_CODE
    };
    let error = CommandError::new(code, rejection.body_text(), false);
    (status, Json(error)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_server::{InputServerService, InputServerSettings};
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Method, Request, StatusCode};
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;
    use uuid::Uuid;

    struct FakeIntake {
        result: Mutex<Result<InputServerAccepted, CommandError>>,
        calls: AtomicUsize,
    }

    impl FakeIntake {
        fn new(result: Result<InputServerAccepted, CommandError>) -> Self {
            Self {
                result: Mutex::new(result),
                calls: AtomicUsize::new(0),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl TextIntake for FakeIntake {
        async fn accept(&self, _text: String) -> Result<InputServerAccepted, CommandError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.result.lock().clone()
        }
    }

    const TEST_PORT: u16 = 10101;
    /// A private-network address standing in for a phone or OBS client on the
    /// LAN; used by the auth tests as the `ConnectInfo` client address.
    const LAN_CLIENT_ADDR: SocketAddr = SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 55)),
        51_000,
    );

    /// A test service whose settings carry the given access token.
    fn test_service(token: Option<&str>) -> Arc<InputServerService> {
        let service = Arc::new(InputServerService::new());
        // try_write instead of an await: the helper runs on the sync fixture
        // path of async tests, and the lock is uncontended by construction.
        *service
            .settings
            .try_write()
            .expect("uncontended settings lock") = InputServerSettings {
            start_on_boot: false,
            port: TEST_PORT,
            bind_address: "127.0.0.1".to_string(),
            access_token: token.map(str::to_string),
        };
        service
    }

    fn app(result: Result<InputServerAccepted, CommandError>) -> (Router, Arc<FakeIntake>) {
        let (router, intake, _service) = app_with_token(result, None);
        (router, intake)
    }

    fn app_with_token(
        result: Result<InputServerAccepted, CommandError>,
        token: Option<&str>,
    ) -> (Router, Arc<FakeIntake>, Arc<InputServerService>) {
        let intake = Arc::new(FakeIntake::new(result));
        let service = test_service(token);
        (
            build_router(
                intake.clone(),
                TEST_PORT,
                OverlayLanguage::English,
                service.clone(),
            ),
            intake,
            service,
        )
    }

    fn request(method: Method, uri: &str, content_type: Option<&str>, body: Body) -> Request<Body> {
        request_with_host(
            method,
            uri,
            content_type,
            body,
            Some(&format!("127.0.0.1:{TEST_PORT}")),
        )
    }

    fn request_with_host(
        method: Method,
        uri: &str,
        content_type: Option<&str>,
        body: Body,
        host: Option<&str>,
    ) -> Request<Body> {
        request_full(
            method,
            uri,
            content_type,
            body,
            host,
            loopback_client_addr(),
        )
    }

    /// A request as it would arrive from a LAN client: private-network
    /// connection address and a matching literal IP in the Host header.
    fn lan_request(
        method: Method,
        uri: &str,
        content_type: Option<&str>,
        body: Body,
    ) -> Request<Body> {
        request_full(
            method,
            uri,
            content_type,
            body,
            Some(&format!("192.168.1.55:{TEST_PORT}")),
            LAN_CLIENT_ADDR,
        )
    }

    fn loopback_client_addr() -> SocketAddr {
        SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            50_000,
        )
    }

    fn request_full(
        method: Method,
        uri: &str,
        content_type: Option<&str>,
        body: Body,
        host: Option<&str>,
        client_addr: SocketAddr,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .extension(ConnectInfo(client_addr));
        if let Some(content_type) = content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        if let Some(host) = host {
            builder = builder.header(header::HOST, host);
        }
        builder.body(body).unwrap()
    }

    fn json_body(text: &str) -> Body {
        Body::from(serde_json::json!({ "text": text }).to_string())
    }

    async fn response_parts(response: Response<Body>) -> (StatusCode, serde_json::Value) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value = serde_json::from_slice(&body).unwrap();
        (status, value)
    }

    async fn overlay_html(app: Router) -> String {
        let response = app
            .oneshot(request(Method::GET, "/overlay", None, Body::empty()))
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn health_returns_exact_json_with_200() {
        let intake = Arc::new(FakeIntake::new(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        })));
        let app = build_router(
            intake,
            TEST_PORT,
            OverlayLanguage::English,
            test_service(None),
        );

        let response = app
            .oneshot(request(Method::GET, "/health", None, Body::empty()))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], br#"{"status":"ok"}"#);
    }

    #[tokio::test]
    async fn queued_success_returns_202_with_accepted_json() {
        let job_id = Uuid::new_v4();
        let (app, intake) = app(Ok(InputServerAccepted::Queued { job_id }));

        let response = app
            .oneshot(request(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(
            value,
            serde_json::json!({ "status": "queued", "job_id": job_id.to_string() })
        );
        assert_eq!(intake.call_count(), 1);
    }

    #[tokio::test]
    async fn pending_success_returns_202_with_accepted_json() {
        let (app, intake) = app(Ok(InputServerAccepted::PendingReview {
            incoming_id: "abc".to_string(),
        }));

        let response = app
            .oneshot(request(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(
            value,
            serde_json::json!({ "status": "pending_review", "incoming_id": "abc" })
        );
        assert_eq!(intake.call_count(), 1);
    }

    async fn assert_command_error_maps(error: CommandError, expected_status: StatusCode) {
        let (app, intake) = app(Err(error.clone()));

        let response = app
            .oneshot(request(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, expected_status);
        assert_eq!(value["code"], error.code);
        assert_eq!(value["message"], error.message);
        assert_eq!(value["retryable"], error.retryable);
        assert_eq!(intake.call_count(), 1);
    }

    #[tokio::test]
    async fn queue_full_maps_to_429() {
        let error = CommandError::new(speech_contract::error_code::QUEUE_FULL, "queue full", true);
        assert_command_error_maps(error, StatusCode::TOO_MANY_REQUESTS).await;
    }

    #[tokio::test]
    async fn inbox_full_maps_to_429() {
        let error = CommandError::new(error_code::INBOX_FULL, "inbox full", true);
        assert_command_error_maps(error, StatusCode::TOO_MANY_REQUESTS).await;
    }

    #[tokio::test]
    async fn snapshot_unavailable_maps_to_503() {
        let error = CommandError::new(
            speech_contract::error_code::SNAPSHOT_UNAVAILABLE,
            "no snapshot",
            false,
        );
        assert_command_error_maps(error, StatusCode::SERVICE_UNAVAILABLE).await;
    }

    #[tokio::test]
    async fn blank_text_maps_to_422() {
        let error = CommandError::new(error_code::BLANK, "blank", false);
        assert_command_error_maps(error, StatusCode::UNPROCESSABLE_ENTITY).await;
    }

    #[tokio::test]
    async fn too_long_text_maps_to_422() {
        let error = CommandError::new(error_code::TOO_LONG, "too long", false);
        assert_command_error_maps(error, StatusCode::UNPROCESSABLE_ENTITY).await;
    }

    #[tokio::test]
    async fn unknown_item_maps_to_422() {
        let error = CommandError::new(error_code::UNKNOWN_ITEM, "unknown id", false);
        assert_command_error_maps(error, StatusCode::UNPROCESSABLE_ENTITY).await;
    }

    #[tokio::test]
    async fn other_structured_error_maps_to_409() {
        let error = CommandError::new("speech.twitch_only_route", "twitch only", false);
        assert_command_error_maps(error, StatusCode::CONFLICT).await;
    }

    #[tokio::test]
    async fn malformed_json_returns_400_and_does_not_call_adapter() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                Body::from("not json"),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(value["code"], INVALID_REQUEST_CODE);
        assert!(!value["retryable"].as_bool().unwrap());
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn missing_content_type_returns_415_and_does_not_call_adapter() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request(
                Method::POST,
                "/v1/speech",
                None,
                json_body("hello world"),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(value["code"], INVALID_REQUEST_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn wrong_content_type_returns_415_and_does_not_call_adapter() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request(
                Method::POST,
                "/v1/speech",
                Some("text/plain"),
                json_body("hello world"),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(value["code"], INVALID_REQUEST_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn body_too_large_returns_413_and_does_not_call_adapter() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let oversized = format!(
            "{}",
            serde_json::json!({ "text": "a".repeat(MAX_BODY_BYTES) })
        );
        assert!(oversized.len() > MAX_BODY_BYTES);

        let response = app
            .oneshot(request(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                Body::from(oversized),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(value["code"], BODY_TOO_LARGE_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn response_has_no_cors_allow_origin_header() {
        let (app, _intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();

        assert!(response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
    }

    #[tokio::test]
    async fn adapter_is_called_exactly_once_per_valid_request() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(intake.call_count(), 1);
    }

    #[tokio::test]
    async fn foreign_host_returns_403_and_does_not_call_adapter() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request_with_host(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
                Some(&format!("evil.example:{TEST_PORT}")),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(value["code"], FORBIDDEN_HOST_CODE);
        assert_eq!(value["message"], "Forbidden host");
        assert!(!value["retryable"].as_bool().unwrap());
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn missing_host_returns_403_and_does_not_call_adapter() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request_with_host(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
                None,
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(value["code"], FORBIDDEN_HOST_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn health_with_foreign_host_returns_403() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request_with_host(
                Method::GET,
                "/health",
                None,
                Body::empty(),
                Some(&format!("rebind.example:{TEST_PORT}")),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(value["code"], FORBIDDEN_HOST_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn loopback_ip_and_localhost_hosts_are_accepted() {
        for host in [
            format!("127.0.0.1:{TEST_PORT}"),
            format!("localhost:{TEST_PORT}"),
            format!("LOCALHOST:{TEST_PORT}"),
        ] {
            let (app, intake) = app(Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }));

            let response = app
                .oneshot(request_with_host(
                    Method::POST,
                    "/v1/speech",
                    Some("application/json"),
                    json_body("hello world"),
                    Some(&host),
                ))
                .await
                .unwrap();

            assert_eq!(
                response.status(),
                StatusCode::ACCEPTED,
                "host {host} must be accepted"
            );
            assert_eq!(intake.call_count(), 1);
        }
    }

    #[tokio::test]
    async fn loopback_hostname_with_wrong_port_returns_403() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request_with_host(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
                Some(&format!("127.0.0.1:{}", TEST_PORT + 1)),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(value["code"], FORBIDDEN_HOST_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn ipv6_loopback_host_is_accepted() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request_with_host(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
                Some(&format!("[::1]:{TEST_PORT}")),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(intake.call_count(), 1);
    }

    #[tokio::test]
    async fn overlay_returns_html_with_utf8_content_type_and_form_landmarks() {
        let intake = Arc::new(FakeIntake::new(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        })));
        let app = build_router(
            intake,
            TEST_PORT,
            OverlayLanguage::English,
            test_service(None),
        );

        let response = app
            .oneshot(request(Method::GET, "/overlay", None, Body::empty()))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(
            response.headers()[header::CONTENT_SECURITY_POLICY],
            "frame-ancestors 'none'"
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        for landmark in [
            "<form",
            "<textarea",
            "<script",
            "aria-live",
            "autofocus",
            "/v1/speech",
            "prefers-color-scheme",
            "isComposing",
            "const language = 'en'",
            "language === 'ru'",
            "Sent",
            "Отправлено",
            "10000",
        ] {
            assert!(
                html.contains(landmark),
                "overlay page must contain {landmark}"
            );
        }
        for absent in ["<h1", "<button"] {
            assert!(
                !html.contains(absent),
                "overlay page must stay minimal without {absent}"
            );
        }
    }

    #[tokio::test]
    async fn overlay_english_serves_english_marker_without_browser_detection() {
        let intake = Arc::new(FakeIntake::new(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        })));
        let app = build_router(
            intake,
            TEST_PORT,
            OverlayLanguage::English,
            test_service(None),
        );

        let html = overlay_html(app).await;

        assert!(
            html.contains("const language = 'en'"),
            "English router must serve the English language marker"
        );
        assert!(
            !html.contains("navigator.language"),
            "overlay must not detect the browser language"
        );
    }

    #[tokio::test]
    async fn overlay_russian_serves_russian_marker_without_browser_detection() {
        let intake = Arc::new(FakeIntake::new(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        })));
        let app = build_router(
            intake,
            TEST_PORT,
            OverlayLanguage::Russian,
            test_service(None),
        );

        let html = overlay_html(app).await;

        assert!(
            html.contains("const language = 'ru'"),
            "Russian router must serve the Russian language marker"
        );
        assert!(
            !html.contains("navigator.language"),
            "overlay must not detect the browser language"
        );
    }

    #[tokio::test]
    async fn overlay_has_no_cors_allow_origin_header() {
        let intake = Arc::new(FakeIntake::new(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        })));
        let app = build_router(
            intake,
            TEST_PORT,
            OverlayLanguage::English,
            test_service(None),
        );

        let response = app
            .oneshot(request(Method::GET, "/overlay", None, Body::empty()))
            .await
            .unwrap();

        assert!(response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
    }

    #[tokio::test]
    async fn overlay_with_foreign_host_returns_403() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request_with_host(
                Method::GET,
                "/overlay",
                None,
                Body::empty(),
                Some(&format!("evil.example:{TEST_PORT}")),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(value["code"], FORBIDDEN_HOST_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn overlay_with_missing_host_returns_403() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request_with_host(
                Method::GET,
                "/overlay",
                None,
                Body::empty(),
                None,
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(value["code"], FORBIDDEN_HOST_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn private_ip_host_is_accepted() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request_with_host(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
                Some(&format!("192.168.1.55:{TEST_PORT}")),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(intake.call_count(), 1);
    }

    #[tokio::test]
    async fn public_ip_host_returns_403() {
        let (app, intake) = app(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        }));

        let response = app
            .oneshot(request_with_host(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
                Some(&format!("8.8.8.8:{TEST_PORT}")),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(value["code"], FORBIDDEN_HOST_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn lan_speech_without_token_returns_401_and_does_not_call_adapter() {
        let (app, intake, _service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("secret-token"),
        );

        let response = app
            .oneshot(lan_request(
                Method::POST,
                "/v1/speech",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(value["code"], UNAUTHORIZED_CODE);
        assert!(!value["retryable"].as_bool().unwrap());
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn lan_speech_with_valid_query_token_returns_202() {
        let (app, intake, _service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("secret-token"),
        );

        let response = app
            .oneshot(lan_request(
                Method::POST,
                "/v1/speech?token=secret-token",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(intake.call_count(), 1);
    }

    #[tokio::test]
    async fn lan_speech_with_bearer_token_returns_202() {
        let (app, intake, _service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("secret-token"),
        );

        let request =
            lan_speech_request(Some((header::AUTHORIZATION, "Bearer secret-token".into())));

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(intake.call_count(), 1);
    }

    #[tokio::test]
    async fn lan_speech_with_cookie_token_returns_202() {
        let (app, intake, _service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("secret-token"),
        );

        let request = lan_speech_request(Some((
            header::COOKIE,
            format!("{AUTH_COOKIE_NAME}=secret-token"),
        )));

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(intake.call_count(), 1);
    }

    #[tokio::test]
    async fn lan_speech_with_wrong_token_returns_401() {
        let (app, intake, _service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("secret-token"),
        );

        let response = app
            .oneshot(lan_request(
                Method::POST,
                "/v1/speech?token=not-the-token",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(value["code"], UNAUTHORIZED_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn lan_health_without_token_returns_200() {
        let (app, _intake, _service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("secret-token"),
        );

        let response = app
            .oneshot(lan_request(Method::GET, "/health", None, Body::empty()))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn overlay_with_valid_query_token_sets_cookie() {
        let (app, _intake, _service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("secret-token"),
        );

        let response = app
            .oneshot(lan_request(
                Method::GET,
                "/overlay?token=secret-token",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .expect("valid token must set the auth cookie");
        assert!(
            cookie.starts_with(&format!("{AUTH_COOKIE_NAME}=secret-token")),
            "cookie: {cookie}"
        );
        assert!(cookie.contains("HttpOnly"), "cookie: {cookie}");
        assert!(cookie.contains("SameSite=Lax"), "cookie: {cookie}");
    }

    #[tokio::test]
    async fn overlay_with_invalid_query_token_does_not_set_cookie() {
        let (app, _intake, _service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("secret-token"),
        );

        let response = app
            .oneshot(lan_request(
                Method::GET,
                "/overlay?token=wrong-token",
                None,
                Body::empty(),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
    }

    #[tokio::test]
    async fn lan_post_with_overlay_cookie_passes_auth() {
        let (app, intake, _service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("secret-token"),
        );

        let request = lan_speech_request(Some((
            header::COOKIE,
            format!("other=1; {AUTH_COOKIE_NAME}=secret-token; another=2"),
        )));

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(intake.call_count(), 1);
    }

    #[tokio::test]
    async fn token_rotation_rejects_old_and_accepts_new_without_rebuild() {
        let (app, _intake, service) = app_with_token(
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            }),
            Some("old-token"),
        );

        let old_accepted = app
            .clone()
            .oneshot(lan_request(
                Method::POST,
                "/v1/speech?token=old-token",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();
        assert_eq!(old_accepted.status(), StatusCode::ACCEPTED);

        // Rotate the token in the live settings: the same router must pick it
        // up on the next request without a listener restart.
        service.settings.write().await.access_token = Some("new-token".to_string());

        let old_rejected = app
            .clone()
            .oneshot(lan_request(
                Method::POST,
                "/v1/speech?token=old-token",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();
        assert_eq!(old_rejected.status(), StatusCode::UNAUTHORIZED);

        let new_accepted = app
            .oneshot(lan_request(
                Method::POST,
                "/v1/speech?token=new-token",
                Some("application/json"),
                json_body("hello world"),
            ))
            .await
            .unwrap();
        assert_eq!(new_accepted.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn missing_and_empty_stored_tokens_fail_closed() {
        for stored in [None, Some("")] {
            let (app, intake, _service) = app_with_token(
                Ok(InputServerAccepted::Queued {
                    job_id: Uuid::nil(),
                }),
                stored,
            );

            let response = app
                .oneshot(lan_request(
                    Method::POST,
                    "/v1/speech?token=anything",
                    Some("application/json"),
                    json_body("hello world"),
                ))
                .await
                .unwrap();

            let (status, value) = response_parts(response).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "stored: {stored:?}");
            assert_eq!(value["code"], UNAUTHORIZED_CODE);
            assert_eq!(intake.call_count(), 0);
        }
    }

    /// A LAN POST to `/v1/speech` with one extra header (cookie/bearer variants).
    fn lan_speech_request(extra: Option<(header::HeaderName, String)>) -> Request<Body> {
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri("/v1/speech")
            .extension(ConnectInfo(LAN_CLIENT_ADDR))
            .header(header::HOST, format!("192.168.1.55:{TEST_PORT}"))
            .header(header::CONTENT_TYPE, "application/json");
        if let Some((name, value)) = extra {
            builder = builder.header(name, value);
        }
        builder.body(json_body("hello world")).unwrap()
    }
}
