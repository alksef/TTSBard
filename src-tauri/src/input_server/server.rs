use super::overlay::overlay;
pub use super::overlay::OverlayLanguage;
use crate::commands::input_server::{error_code, InputServerAccepted};
use crate::ipc::{speech as speech_contract, CommandError};
use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// Maximum accepted HTTP request body size (64 KiB).
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Stable error code used for JSON extraction failures (bad/missing body or
/// wrong/missing Content-Type).
pub const INVALID_REQUEST_CODE: &str = "input_server.invalid_request";

/// Stable error code used when the request body exceeds [`MAX_BODY_BYTES`].
pub const BODY_TOO_LARGE_CODE: &str = "input_server.body_too_large";

/// Stable error code used when the request Host header is missing or does not
/// name this loopback listener (`127.0.0.1:<port>` / `localhost:<port>`).
pub const FORBIDDEN_HOST_CODE: &str = "input_server.forbidden_host";

/// Test-injectable asynchronous intake seam for the loopback server.
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
}

/// Incoming request contract for `POST /v1/speech`.
#[derive(Debug, Deserialize)]
struct SpeechRequest {
    text: String,
}

/// Build the loopback input server router.
///
/// Routes: `GET /health`, `GET /overlay`, `POST /v1/speech`. No CORS
/// middleware; body limited to [`MAX_BODY_BYTES`]. Every request is gated on
/// an exact loopback `Host` header (`127.0.0.1:<port>` / `localhost:<port>`) so
/// DNS-rebinding pages rebinding an attacker domain to 127.0.0.1 are rejected
/// with 403 before any handler or intake runs. `overlay_language` is the single
/// language decision served to the overlay script.
pub fn build_router(
    intake: Arc<dyn TextIntake>,
    port: u16,
    overlay_language: OverlayLanguage,
) -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/overlay",
            get(move || {
                let overlay_language = overlay_language;
                async move { overlay(overlay_language).await }
            }),
        )
        .route("/v1/speech", post(post_speech))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(port, host_gate))
        .with_state(RouterState { intake })
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

fn host_matches_listener(host: &str, port: u16) -> bool {
    let Some((hostname, host_port)) = host.rsplit_once(':') else {
        return false;
    };
    if host_port.parse::<u16>() != Ok(port) {
        return false;
    }
    hostname == "127.0.0.1" || hostname.eq_ignore_ascii_case("localhost")
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
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

    fn app(result: Result<InputServerAccepted, CommandError>) -> (Router, Arc<FakeIntake>) {
        let intake = Arc::new(FakeIntake::new(result));
        (
            build_router(intake.clone(), TEST_PORT, OverlayLanguage::English),
            intake,
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
        let mut builder = Request::builder().method(method).uri(uri);
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
        let app = build_router(intake, TEST_PORT, OverlayLanguage::English);

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
    async fn ipv6_loopback_host_returns_403() {
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

        let (status, value) = response_parts(response).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(value["code"], FORBIDDEN_HOST_CODE);
        assert_eq!(intake.call_count(), 0);
    }

    #[tokio::test]
    async fn overlay_returns_html_with_utf8_content_type_and_form_landmarks() {
        let intake = Arc::new(FakeIntake::new(Ok(InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        })));
        let app = build_router(intake, TEST_PORT, OverlayLanguage::English);

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
        let app = build_router(intake, TEST_PORT, OverlayLanguage::English);

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
        let app = build_router(intake, TEST_PORT, OverlayLanguage::Russian);

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
        let app = build_router(intake, TEST_PORT, OverlayLanguage::English);

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
}
