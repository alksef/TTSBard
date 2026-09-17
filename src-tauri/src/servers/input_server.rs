// Input server module
//
// Owns the single loopback listener lifecycle for external text intake:
// rereads desired settings, binds `127.0.0.1:<port>`, and reports runtime
// status through `InputServerService::publish_status`.

use crate::commands::input_server::{accept_external_text, InputServerAccepted};
use crate::commands::speech_queue::SpeechQueueState;
use crate::input_server::server::{build_router, OverlayLanguage, TextIntake};
use crate::input_server::{InputServerService, InputServerStatus};
use crate::ipc::CommandError;
use crate::speech_queue::SubmissionSource;
use crate::state::AppState;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Frontend event emitted on every real runtime status transition.
pub const STATUS_CHANGED_EVENT: &str = "input-server-status-changed";

/// Production [`TextIntake`] adapter backed by the application state.
///
/// Resolves `AppState` and the managed `SpeechQueueState` from the Tauri app
/// on each request and forwards to the shared application-level accept seam.
pub struct ProductionTextIntake {
    app_handle: AppHandle,
}

#[async_trait::async_trait]
impl TextIntake for ProductionTextIntake {
    async fn accept(&self, text: String) -> Result<InputServerAccepted, CommandError> {
        let app_state = self.app_handle.try_state::<AppState>().ok_or_else(|| {
            CommandError::new("input_server.unavailable", "AppState unavailable", false)
        })?;
        let queue = self
            .app_handle
            .try_state::<SpeechQueueState>()
            .ok_or_else(|| {
                CommandError::new(
                    "input_server.unavailable",
                    "SpeechQueueState unavailable",
                    false,
                )
            })?;
        accept_external_text(
            &self.app_handle,
            app_state.inner(),
            queue.inner(),
            SubmissionSource::Server,
            text,
        )
        .await
    }
}

/// Bind a loopback listener on `127.0.0.1:<port>`, returning a user-facing
/// error message on failure.
async fn bind_loopback(port: u16) -> Result<TcpListener, String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpListener::bind(addr)
        .await
        .map_err(|error| format!("Failed to bind 127.0.0.1:{port}: {error}"))
}

/// Resolve the overlay language decision from the effective UI locale.
///
/// The startup [`crate::commands::localization::LocalizationState`] holds the
/// effective locale for this run; Russian selects Russian overlay messages and
/// every other locale falls back to English. A missing state (never expected in
/// production) also falls back to English.
fn resolve_overlay_language(app_handle: &AppHandle) -> OverlayLanguage {
    let locale = app_handle
        .try_state::<crate::commands::localization::LocalizationState>()
        .map(|state| state.snapshot().locale)
        .unwrap_or_else(|| "en".to_string());
    if locale == "ru" {
        OverlayLanguage::Russian
    } else {
        OverlayLanguage::English
    }
}

/// Run the input server listener lifecycle against the service's desired
/// settings.
///
/// Installs the service wake sender before the lifecycle loop so settings
/// changes wake the supervisor even while the server is disabled. `serve`
/// spawns the axum accept task (injected so the core is testable without a
/// live Tauri app); `emit` receives every real status transition after
/// duplicate suppression.
pub async fn run_input_server(app_handle: AppHandle, shutdown: CancellationToken) {
    let Some(app_state) = app_handle.try_state::<AppState>() else {
        error!("Input server AppState unavailable");
        return;
    };
    let service = app_state.inner().input_server.clone();

    let (wake_tx, wake_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    service.install_wake_sender(wake_tx);

    let intake: Arc<dyn TextIntake> = Arc::new(ProductionTextIntake {
        app_handle: app_handle.clone(),
    });

    let overlay_language = resolve_overlay_language(&app_handle);

    let serve = |listener: TcpListener, router: Router, token: CancellationToken| {
        tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router)
                .with_graceful_shutdown(token.cancelled_owned())
                .await
            {
                warn!(error = %error, "Input server serve error");
            }
        })
    };

    let emit_app_handle = app_handle.clone();
    let emit = move |status: &InputServerStatus| {
        if let Err(error) = emit_app_handle.emit(STATUS_CHANGED_EVENT, status.clone()) {
            warn!(error = %error, "Failed to emit input server status");
        }
    };

    run_input_server_core(
        service,
        wake_rx,
        shutdown,
        intake,
        overlay_language,
        serve,
        emit,
    )
    .await;
}

/// Upper bound for draining in-flight requests before a listener stop aborts
/// the serve task as insurance.
const GRACEFUL_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Gracefully drain a running server task, aborting it as insurance only when
/// the drain exceeds [`GRACEFUL_DRAIN_TIMEOUT`].
async fn drain_server_task(server_task: &mut JoinHandle<()>) {
    if tokio::time::timeout(GRACEFUL_DRAIN_TIMEOUT, &mut *server_task)
        .await
        .is_err()
    {
        server_task.abort();
        let _ = server_task.await;
    }
}

/// Testable supervisor core: one lifecycle loop over the in-memory run request.
///
/// - not requested: no listener, status `stopped`, waits for wake/shutdown.
/// - requested: `starting`, bind, `running` only after a successful bind.
/// - bind error: `error { message }`, no retry spin, waits for wake/shutdown.
/// - wake/settings change stops the active listener and rereads the run request
///   and the desired port.
/// - shutdown stops the active listener and returns with `stopped`.
async fn run_input_server_core<E, S>(
    service: Arc<InputServerService>,
    mut wake_rx: tokio::sync::mpsc::UnboundedReceiver<()>,
    shutdown: CancellationToken,
    intake: Arc<dyn TextIntake>,
    overlay_language: OverlayLanguage,
    serve: S,
    mut emit: E,
) where
    E: FnMut(&InputServerStatus),
    S: Fn(TcpListener, Router, CancellationToken) -> JoinHandle<()>,
{
    loop {
        // Coalesce wake bursts: a rapid stop→start queues several wake messages
        // before this loop observes them. Draining here lets the loop process
        // only the latest state, so a rapid stop→start never double-binds the
        // same port.
        while wake_rx.try_recv().is_ok() {}

        let (requested, port) = {
            let requested = service.run_requested();
            let settings = service.settings.read().await;
            (requested, settings.port)
        };

        if !requested {
            service.publish_status(InputServerStatus::Stopped, &mut emit);
            info!("Input server not requested; waiting for start or shutdown");
            tokio::select! {
                _ = shutdown.cancelled() => {
                    service.publish_status(InputServerStatus::Stopped, &mut emit);
                    return;
                }
                _ = wake_rx.recv() => {
                    continue;
                }
            }
        }

        service.publish_status(InputServerStatus::Starting, &mut emit);
        info!(port, "Input server starting on 127.0.0.1:{port}");
        let listener = match bind_loopback(port).await {
            Ok(listener) => listener,
            Err(message) => {
                error!(error = %message, "Input server bind failed");
                service.publish_status(InputServerStatus::Error { message }, &mut emit);
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        service.publish_status(InputServerStatus::Stopped, &mut emit);
                        return;
                    }
                    _ = wake_rx.recv() => continue,
                }
            }
        };

        let router = build_router(intake.clone(), port, overlay_language);
        let server_token = shutdown.child_token();
        let mut server_task = serve(listener, router, server_token.clone());
        service.publish_status(InputServerStatus::Running, &mut emit);
        info!(port, "Input server running");

        tokio::select! {
            biased;
            _ = shutdown.cancelled() => {
                server_token.cancel();
                drain_server_task(&mut server_task).await;
                service.publish_status(InputServerStatus::Stopped, &mut emit);
                info!("Input server stopped on shutdown");
                return;
            }
            _ = wake_rx.recv() => {
                server_token.cancel();
                drain_server_task(&mut server_task).await;
                service.publish_status(InputServerStatus::Stopped, &mut emit);
                info!("Input server stopped on settings wake; rereading settings");
                continue;
            }
            result = &mut server_task => {
                match result {
                    Ok(()) => warn!("Input server serve task finished unexpectedly"),
                    Err(join_error) => warn!(%join_error, "Input server serve task join failed"),
                }
                service.publish_status(
                    InputServerStatus::Error {
                        message: "Input server stopped unexpectedly".into(),
                    },
                    &mut emit,
                );
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        service.publish_status(InputServerStatus::Stopped, &mut emit);
                        return;
                    }
                    _ = wake_rx.recv() => continue,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_server::InputServerSettings;
    use parking_lot::Mutex;
    use std::time::Duration;
    use uuid::Uuid;

    struct RecordingIntake;

    #[async_trait::async_trait]
    impl TextIntake for RecordingIntake {
        async fn accept(&self, _text: String) -> Result<InputServerAccepted, CommandError> {
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            })
        }
    }

    /// Intake that blocks a request in-flight until the test releases it.
    struct GatedIntake {
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }

    #[async_trait::async_trait]
    impl TextIntake for GatedIntake {
        async fn accept(&self, _text: String) -> Result<InputServerAccepted, CommandError> {
            self.entered.notify_one();
            self.release.notified().await;
            Ok(InputServerAccepted::Queued {
                job_id: Uuid::nil(),
            })
        }
    }

    fn real_serve(
        listener: TcpListener,
        router: Router,
        token: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(token.cancelled_owned())
                .await;
        })
    }

    /// Spawn the supervisor core with a recording status emitter and return its
    /// handle. The wake sender is installed on the service, so tests can wake
    /// it through [`InputServerService::wake`].
    fn spawn_core(
        service: Arc<InputServerService>,
        shutdown: CancellationToken,
        transitions: Arc<Mutex<Vec<InputServerStatus>>>,
    ) -> JoinHandle<()> {
        let (wake_tx, wake_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        service.install_wake_sender(wake_tx);
        let intake: Arc<dyn TextIntake> = Arc::new(RecordingIntake);
        let emit = move |status: &InputServerStatus| {
            transitions.lock().push(status.clone());
        };
        tokio::spawn(run_input_server_core(
            service,
            wake_rx,
            shutdown,
            intake,
            OverlayLanguage::English,
            real_serve,
            emit,
        ))
    }

    async fn wait_for_status(service: &InputServerService, expected: InputServerStatus) {
        for _ in 0..300 {
            if service.status() == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!(
            "timed out waiting for status {expected:?}, last {:?}",
            service.status()
        );
    }

    async fn wait_for_error(service: &InputServerService) {
        for _ in 0..300 {
            if matches!(service.status(), InputServerStatus::Error { .. }) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!(
            "timed out waiting for error status, last {:?}",
            service.status()
        );
    }

    /// Grab a currently-free ephemeral loopback port.
    async fn free_loopback_port() -> u16 {
        let probe = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral loopback bind");
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        port
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn disabled_server_waits_stopped_then_starts_on_wake() {
        // A free port, not the fixed default (10101): a running ttsbard.exe or
        // another test process holding the default port must not fail this
        // test with a bind error.
        let port = free_loopback_port().await;
        let service = Arc::new(InputServerService::new());
        *service.settings.write().await = InputServerSettings {
            start_on_boot: false,
            port,
        };
        let shutdown = CancellationToken::new();
        let transitions = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_core(service.clone(), shutdown.clone(), transitions.clone());

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(service.status(), InputServerStatus::Stopped);
        assert!(
            transitions.lock().is_empty(),
            "disabled state must not emit duplicate stopped"
        );

        service.set_run_request(true);
        service.wake();
        wait_for_status(&service, InputServerStatus::Running).await;

        shutdown.cancel();
        handle.await.unwrap();

        assert_eq!(service.status(), InputServerStatus::Stopped);
        let states = transitions.lock().clone();
        assert_eq!(
            states,
            vec![
                InputServerStatus::Starting,
                InputServerStatus::Running,
                InputServerStatus::Stopped,
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn free_loopback_port_reaches_running_and_serves_health() {
        let port = free_loopback_port().await;
        let service = Arc::new(InputServerService::new());
        *service.settings.write().await = InputServerSettings {
            start_on_boot: false,
            port,
        };
        service.set_run_request(true);
        let shutdown = CancellationToken::new();
        let transitions = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_core(service.clone(), shutdown.clone(), transitions.clone());

        wait_for_status(&service, InputServerStatus::Running).await;

        let response = reqwest::get(format!("http://127.0.0.1:{port}/health"))
            .await
            .expect("health probe must connect");
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(
            response.text().await.unwrap(),
            r#"{"status":"ok"}"#,
            "health endpoint must return exact JSON"
        );

        shutdown.cancel();
        handle.await.unwrap();

        assert_eq!(service.status(), InputServerStatus::Stopped);
        let states = transitions.lock().clone();
        assert_eq!(
            states,
            vec![
                InputServerStatus::Starting,
                InputServerStatus::Running,
                InputServerStatus::Stopped,
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn occupied_loopback_port_reports_stable_error_without_retry_spin() {
        let occupied = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = occupied.local_addr().unwrap().port();

        let service = Arc::new(InputServerService::new());
        *service.settings.write().await = InputServerSettings {
            start_on_boot: false,
            port,
        };
        service.set_run_request(true);
        let shutdown = CancellationToken::new();
        let transitions = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_core(service.clone(), shutdown.clone(), transitions.clone());

        wait_for_error(&service).await;

        match service.status() {
            InputServerStatus::Error { message } => {
                assert!(!message.is_empty(), "bind error must carry a message");
            }
            other => panic!("expected error status, got {other:?}"),
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            matches!(service.status(), InputServerStatus::Error { .. }),
            "bind error must not be retried into a spin"
        );

        shutdown.cancel();
        handle.await.unwrap();

        let states = transitions.lock().clone();
        assert_eq!(states.len(), 3, "no retry spin: {states:?}");
        assert_eq!(states[0], InputServerStatus::Starting);
        assert!(matches!(states[1], InputServerStatus::Error { .. }));
        assert_eq!(states[2], InputServerStatus::Stopped);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bind_error_shutdown_publishes_final_stopped() {
        let occupied = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = occupied.local_addr().unwrap().port();

        let service = Arc::new(InputServerService::new());
        *service.settings.write().await = InputServerSettings {
            start_on_boot: false,
            port,
        };
        service.set_run_request(true);
        let shutdown = CancellationToken::new();
        let transitions = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_core(service.clone(), shutdown.clone(), transitions.clone());

        wait_for_error(&service).await;
        assert!(
            matches!(service.status(), InputServerStatus::Error { .. }),
            "bind error must be reported before shutdown"
        );

        shutdown.cancel();
        handle.await.unwrap();

        assert_eq!(
            service.status(),
            InputServerStatus::Stopped,
            "shutdown after bind error must end stopped"
        );
        let states = transitions.lock().clone();
        assert_eq!(states.len(), 3, "one final stopped transition: {states:?}");
        assert_eq!(states[0], InputServerStatus::Starting);
        assert!(matches!(states[1], InputServerStatus::Error { .. }));
        assert_eq!(states[2], InputServerStatus::Stopped);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_stop_transitions_leave_start_on_boot_unchanged() {
        let port = free_loopback_port().await;
        let service = Arc::new(InputServerService::new());
        *service.settings.write().await = InputServerSettings {
            start_on_boot: false,
            port,
        };
        let shutdown = CancellationToken::new();
        let transitions = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_core(service.clone(), shutdown.clone(), transitions.clone());

        service.set_run_request(true);
        service.wake();
        wait_for_status(&service, InputServerStatus::Running).await;
        assert!(
            !service.settings.read().await.start_on_boot,
            "runtime start must not flip the persisted boot preference"
        );

        service.set_run_request(false);
        service.wake();
        wait_for_status(&service, InputServerStatus::Stopped).await;
        assert!(
            !service.settings.read().await.start_on_boot,
            "runtime stop must not flip the persisted boot preference"
        );

        shutdown.cancel();
        handle.await.unwrap();
        assert_eq!(service.status(), InputServerStatus::Stopped);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn changing_start_on_boot_alone_does_not_start_the_server() {
        let port = free_loopback_port().await;
        let service = Arc::new(InputServerService::new());
        *service.settings.write().await = InputServerSettings {
            start_on_boot: false,
            port,
        };
        let shutdown = CancellationToken::new();
        let transitions = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_core(service.clone(), shutdown.clone(), transitions.clone());

        // Flip the persisted preference without a run request: the supervisor
        // must stay stopped, and no runtime transition may be emitted.
        service.settings.write().await.start_on_boot = true;
        service.wake();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(service.status(), InputServerStatus::Stopped);
        assert!(
            transitions.lock().is_empty(),
            "start_on_boot alone must not emit any runtime transition"
        );

        shutdown.cancel();
        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn two_fast_wakes_coalesce_into_single_rebind() {
        let port = free_loopback_port().await;
        let service = Arc::new(InputServerService::new());
        *service.settings.write().await = InputServerSettings {
            start_on_boot: false,
            port,
        };
        service.set_run_request(true);
        let shutdown = CancellationToken::new();
        let transitions = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_core(service.clone(), shutdown.clone(), transitions.clone());

        wait_for_status(&service, InputServerStatus::Running).await;
        transitions.lock().clear();

        // Two rapid wakes queue before the supervisor reacts to either; the
        // coalesce drain must collapse them into a single stop→start rebound.
        service.wake();
        service.wake();

        for _ in 0..500 {
            if service.status() == InputServerStatus::Running
                && transitions.lock().contains(&InputServerStatus::Stopped)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // Allow any (incorrect) second cycle to surface before asserting.
        tokio::time::sleep(Duration::from_millis(150)).await;

        assert_eq!(
            transitions.lock().clone(),
            vec![
                InputServerStatus::Stopped,
                InputServerStatus::Starting,
                InputServerStatus::Running,
            ],
            "two rapid wakes must coalesce into one rebind cycle"
        );

        shutdown.cancel();
        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shutdown_drains_inflight_request_before_completing() {
        let port = free_loopback_port().await;
        let service = Arc::new(InputServerService::new());
        *service.settings.write().await = InputServerSettings {
            start_on_boot: false,
            port,
        };
        service.set_run_request(true);

        let shutdown = CancellationToken::new();
        let transitions = Arc::new(Mutex::new(Vec::new()));
        let (wake_tx, wake_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        service.install_wake_sender(wake_tx);

        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let intake: Arc<dyn TextIntake> = Arc::new(GatedIntake {
            entered: entered.clone(),
            release: release.clone(),
        });
        let emit = move |status: &InputServerStatus| {
            transitions.lock().push(status.clone());
        };
        let handle = tokio::spawn(run_input_server_core(
            service.clone(),
            wake_rx,
            shutdown.clone(),
            intake,
            OverlayLanguage::English,
            real_serve,
            emit,
        ));

        wait_for_status(&service, InputServerStatus::Running).await;

        let response_task = tokio::spawn(async move {
            reqwest::Client::new()
                .post(format!("http://127.0.0.1:{port}/v1/speech"))
                .header("content-type", "application/json")
                .body(serde_json::json!({ "text": "hello" }).to_string())
                .send()
                .await
        });

        // Wait until the request is inside the intake, then shut down while it
        // is still in-flight.
        entered.notified().await;
        shutdown.cancel();

        // Release the request: graceful shutdown must let it finish (no abort),
        // proving in-flight text is not dropped and re-sent on client retry.
        release.notify_one();

        let response = response_task
            .await
            .unwrap()
            .expect("in-flight request must complete during graceful shutdown");
        assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);

        handle.await.unwrap();
        assert_eq!(service.status(), InputServerStatus::Stopped);
    }
}
