// Input server module
//
// Owns the single loopback listener lifecycle for external text intake:
// rereads desired settings, binds `127.0.0.1:<port>`, and reports runtime
// status through `InputServerService::publish_status`.

use crate::commands::input_server::{accept_external_text, InputServerAccepted};
use crate::commands::speech_queue::SpeechQueueState;
use crate::input_server::server::{build_router, TextIntake};
use crate::input_server::{InputServerService, InputServerStatus};
use crate::ipc::CommandError;
use crate::speech_queue::SubmissionSource;
use crate::state::AppState;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
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

    let serve = |listener: TcpListener, router: Router| {
        tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router).await {
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

    run_input_server_core(service, wake_rx, shutdown, intake, serve, emit).await;
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
    serve: S,
    mut emit: E,
) where
    E: FnMut(&InputServerStatus),
    S: Fn(TcpListener, Router) -> JoinHandle<()>,
{
    loop {
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

        let router = build_router(intake.clone());
        let mut server_task = serve(listener, router);
        service.publish_status(InputServerStatus::Running, &mut emit);
        info!(port, "Input server running");

        tokio::select! {
            biased;
            _ = shutdown.cancelled() => {
                server_task.abort();
                let _ = server_task.await;
                service.publish_status(InputServerStatus::Stopped, &mut emit);
                info!("Input server stopped on shutdown");
                return;
            }
            _ = wake_rx.recv() => {
                server_task.abort();
                let _ = server_task.await;
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

    fn real_serve(listener: TcpListener, router: Router) -> JoinHandle<()> {
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
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
            service, wake_rx, shutdown, intake, real_serve, emit,
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
        let service = Arc::new(InputServerService::new());
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
}
