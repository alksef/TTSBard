// Twitch server module
//
// This module manages the Twitch client connection.
// Refactored from lib.rs Twitch client thread (2026-03-11)

use crate::config::TwitchSettings as ConfigTwitchSettings;
use crate::events::{TwitchConnectionStatus, TwitchEvent};
use crate::state::AppState;
use crate::twitch::{SendFailure, TwitchClient, TwitchStatus, OUTGOING_QUEUE_CAPACITY};
use std::future::Future;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

const RECONNECT_BASE_DELAY_MS: u64 = 500;
const RECONNECT_MAX_DELAY_MS: u64 = 30_000;
const RECONNECT_JITTER_MIN_MS: i64 = 100;
const RECONNECT_JITTER_MAX_MS: i64 = 500;

fn reconnect_base_delay_ms(attempt: u32) -> u64 {
    let factor = 2u64.checked_pow(attempt).unwrap_or(u64::MAX);
    RECONNECT_BASE_DELAY_MS
        .saturating_mul(factor)
        .min(RECONNECT_MAX_DELAY_MS)
}

fn bound_reconnect_jitter_ms(jitter_ms: i64) -> u64 {
    jitter_ms.clamp(RECONNECT_JITTER_MIN_MS, RECONNECT_JITTER_MAX_MS) as u64
}

fn is_reconnect_retryable(status: &TwitchStatus) -> bool {
    matches!(status, TwitchStatus::TransportFailure(_))
}

/// Решение по ошибке чтения broadcast-канала управляющих событий:
/// переполнение (Lagged) теряет только пропущенные события и продолжает loop,
/// закрытый канал (Closed) окончательно завершает supervisor.
#[derive(Debug, PartialEq, Eq)]
enum RecvErrorAction {
    Continue(u64),
    Break,
}

fn classify_recv_error(err: RecvError) -> RecvErrorAction {
    match err {
        RecvError::Lagged(skipped) => RecvErrorAction::Continue(skipped),
        RecvError::Closed => RecvErrorAction::Break,
    }
}

/// Логирует typed-отказ отправки безопасным сообщением (без текста/token).
fn log_send_failure(failure: &SendFailure) {
    match failure {
        SendFailure::QueueFull => {
            warn!(
                capacity = OUTGOING_QUEUE_CAPACITY,
                "Twitch outgoing queue full; message dropped"
            );
        }
        SendFailure::NotConnected => {
            debug!("Cannot send message - not connected");
        }
        SendFailure::Send(e) => {
            error!(error = %e, "Failed to send message");
        }
    }
}

struct PendingAttempt {
    client: TwitchClient,
    handle: JoinHandle<Result<(), String>>,
    generation: u64,
    settings: ConfigTwitchSettings,
}

fn connection_settings_match(
    current: &ConfigTwitchSettings,
    attempted: &ConfigTwitchSettings,
) -> bool {
    current.enabled
        && current.is_valid().is_ok()
        && current.username == attempted.username
        && current.token == attempted.token
        && current.channel == attempted.channel
}

/// Stops the client and discards the task of an attempt that was superseded
/// (Stop, Restart, shutdown, a newer attempt or changed settings).
async fn discard_attempt(attempt: PendingAttempt) {
    attempt.client.stop().await;
    attempt.handle.abort();
}

/// Starts a new startup attempt for `client`, invalidating all older
/// generations.
fn spawn_attempt<F, Fut>(
    pending: &mut Option<PendingAttempt>,
    generation: &mut u64,
    client: TwitchClient,
    settings: ConfigTwitchSettings,
    start_attempt: &mut F,
) where
    F: FnMut(TwitchClient) -> Fut,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    *generation = generation.saturating_add(1);
    let handle = tokio::spawn(start_attempt(client.clone()));
    *pending = Some(PendingAttempt {
        client,
        handle,
        generation: *generation,
        settings,
    });
}

/// Run Twitch client in async context
pub async fn run_twitch_client(
    app_state: AppState,
    app_handle: AppHandle,
    twitch_rx: Receiver<TwitchEvent>,
    shutdown: CancellationToken,
) {
    let report_status = {
        let app_state = app_state.clone();
        let app_handle = app_handle.clone();
        move |status: TwitchConnectionStatus| {
            *app_state.twitch.connection_status.lock() = status.clone();
            let _ = app_handle.emit("twitch-status-changed", &status);
        }
    };

    let start_attempt =
        |client: TwitchClient| async move { client.start().await.map_err(|e| e.to_string()) };

    run_twitch_client_core(app_state, twitch_rx, shutdown, report_status, start_attempt).await;
}

async fn run_twitch_client_core<F, Fut>(
    app_state: AppState,
    mut twitch_rx: Receiver<TwitchEvent>,
    shutdown: CancellationToken,
    mut update_status: impl FnMut(TwitchConnectionStatus),
    mut start_attempt: F,
) where
    F: FnMut(TwitchClient) -> Fut,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    let mut twitch_client: Option<TwitchClient> = None;
    let mut last_status = TwitchConnectionStatus::Disconnected;
    let mut status_check_interval = tokio::time::interval(Duration::from_secs(1));
    let mut retry_deadline: Option<Instant> = None;
    let mut retry_attempt = 0u32;
    let mut attempt_generation: u64 = 0;
    let mut pending: Option<PendingAttempt> = None;

    let schedule_retry = |retry_deadline: &mut Option<Instant>, retry_attempt: &mut u32| {
        let jitter = bound_reconnect_jitter_ms(100 + i64::from(rand::random::<u16>() % 401));
        let delay = Duration::from_millis(reconnect_base_delay_ms(*retry_attempt) + jitter);
        *retry_deadline = Some(Instant::now() + delay);
        *retry_attempt = retry_attempt.saturating_add(1);
        delay
    };

    {
        let settings = app_state.twitch.settings.read().await;
        if settings.start_on_boot && settings.enabled && settings.is_valid().is_ok() {
            drop(settings);
            info!("[TWITCH] Auto-start on boot");
            app_state.send_twitch_event(crate::events::TwitchEvent::Restart);
        }
    }

    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => {
                info!("[TWITCH] ⛔ Shutdown");
                if let Some(attempt) = pending.take() {
                    discard_attempt(attempt).await;
                }
                if let Some(client) = twitch_client.take() {
                    client.stop().await;
                }
                *app_state.twitch.client.write().await = None;
                return;
            }
            _ = status_check_interval.tick() => {
                if let Some(client) = &twitch_client {
                    let twitch_status = client.status().await;
                    if is_reconnect_retryable(&twitch_status) {
                        let failed_client = twitch_client
                            .take()
                            .expect("client presence checked above");
                        *app_state.twitch.client.write().await = None;
                        failed_client.stop().await;

                        let delay = schedule_retry(&mut retry_deadline, &mut retry_attempt);
                        info!(
                            ?delay,
                            attempt = retry_attempt,
                            generation = attempt_generation,
                            "Scheduling Twitch reconnect"
                        );
                        last_status = TwitchConnectionStatus::Connecting;
                        update_status(last_status.clone());
                        continue;
                    }

                    if matches!(twitch_status, TwitchStatus::Connected) {
                        retry_attempt = 0;
                    }

                    let new_status = match &twitch_status {
                        TwitchStatus::Connected => {
                            TwitchConnectionStatus::Connected
                        }
                        TwitchStatus::Connecting => {
                            TwitchConnectionStatus::Connecting
                        }
                        TwitchStatus::Disconnected => {
                            TwitchConnectionStatus::Disconnected
                        }
                        TwitchStatus::Error(e) => {
                            TwitchConnectionStatus::Error(e.clone())
                        }
                        TwitchStatus::TransportFailure(_) => {
                            TwitchConnectionStatus::Connecting
                        }
                    };

                    if last_status != new_status {
                        last_status = new_status.clone();
                        update_status(new_status.clone());
                    }
                } else if retry_deadline.is_none()
                    && last_status != TwitchConnectionStatus::Disconnected
                {
                    last_status = TwitchConnectionStatus::Disconnected;
                    update_status(last_status.clone());
                }

                if retry_deadline.is_some_and(|deadline| deadline <= Instant::now()) {
                    if pending.is_some() {
                        continue;
                    }

                    retry_deadline = None;

                    let settings = app_state.twitch.settings.read().await;
                    let is_enabled = settings.enabled;
                    let is_valid = settings.is_valid().is_ok();
                    let settings_clone = settings.clone();
                    drop(settings);

                    if !is_enabled || !is_valid {
                        retry_attempt = 0;
                        last_status = TwitchConnectionStatus::Disconnected;
                        update_status(last_status.clone());
                        continue;
                    }

                    last_status = TwitchConnectionStatus::Connecting;
                    update_status(last_status.clone());

                    let client = TwitchClient::new(settings_clone.clone().into());
                    spawn_attempt(
                        &mut pending,
                        &mut attempt_generation,
                        client,
                        settings_clone,
                        &mut start_attempt,
                    );
                }
            }
            event = twitch_rx.recv() => {
                match event {
                    Ok(event) => {
                        match event {
                            TwitchEvent::Restart => {
                                info!("Restart event received");
                                retry_deadline = None;
                                retry_attempt = 0;
                                attempt_generation = attempt_generation.saturating_add(1);

                                let settings = app_state.twitch.settings.read().await;
                                let is_enabled = settings.enabled;
                                let is_valid = settings.is_valid().is_ok();
                                let settings_clone = settings.clone();
                                drop(settings);

                                if let Some(attempt) = pending.take() {
                                    discard_attempt(attempt).await;
                                }
                                if let Some(client) = twitch_client.take() {
                                    info!("Stopping previous client...");
                                    client.stop().await;
                                }
                                *app_state.twitch.client.write().await = None;

                                last_status = TwitchConnectionStatus::Disconnected;
                                update_status(last_status.clone());

                                if is_enabled {
                                    if is_valid {
                                        info!("Settings valid, creating new client");
                                        last_status = TwitchConnectionStatus::Connecting;
                                        update_status(last_status.clone());

                                        let client = TwitchClient::new(settings_clone.clone().into());
                                        spawn_attempt(
                                            &mut pending,
                                            &mut attempt_generation,
                                            client,
                                            settings_clone,
                                            &mut start_attempt,
                                        );
                                    } else {
                                        debug!("Settings invalid, not starting client");
                                    }
                                } else {
                                    debug!("Twitch disabled, not starting client");
                                }
                            }
                            TwitchEvent::Stop => {
                                info!("Stop event received");
                                retry_deadline = None;
                                retry_attempt = 0;
                                attempt_generation = attempt_generation.saturating_add(1);
                                if let Some(attempt) = pending.take() {
                                    discard_attempt(attempt).await;
                                }
                                if let Some(client) = twitch_client.take() {
                                    client.stop().await;
                                }
                                *app_state.twitch.client.write().await = None;
                                last_status = TwitchConnectionStatus::Disconnected;
                                update_status(last_status.clone());
                            }
                            TwitchEvent::SendMessage(text) => {
                                debug!(text_len = text.chars().count(), "SendMessage event received");
                                if let Some(client) = twitch_client.clone() {
                                    // Enqueue выполняется прямо в control loop, поэтому
                                    // последовательные SendMessage попадают в outgoing queue
                                    // в порядке их чтения (FIFO). Внутри enqueue — только
                                    // короткие await mutex'ов; pacing и send completion не
                                    // блокируют supervisor.
                                    match client.enqueue(&text).await {
                                        Ok(completion) => {
                                            tokio::spawn(async move {
                                                match completion.wait().await {
                                                    Ok(()) => debug!("Message sent successfully"),
                                                    Err(failure) => log_send_failure(&failure),
                                                }
                                            });
                                        }
                                        Err(failure) => log_send_failure(&failure),
                                    }
                                } else {
                                    debug!("Cannot send message - no active client");
                                }
                            }
                        }
                    }
                    Err(e) => match classify_recv_error(e) {
                        RecvErrorAction::Continue(skipped) => {
                            warn!(skipped, "Twitch event receiver lagged; dropped events");
                        }
                        RecvErrorAction::Break => {
                            error!("Twitch event channel closed");
                            break;
                        }
                    },
                }
            }
            res = async { (&mut pending.as_mut().unwrap().handle).await }, if pending.is_some() => {
                let attempt = pending
                    .take()
                    .expect("pending attempt present in guarded branch");

                if attempt.generation != attempt_generation {
                    // Superseded by Stop, Restart, shutdown or a newer attempt:
                    // never install, publish or schedule a retry for it.
                    info!(
                        stale_generation = attempt.generation,
                        current_generation = attempt_generation,
                        "Discarding stale Twitch startup completion"
                    );
                    discard_attempt(attempt).await;
                    continue;
                }

                let result = match res {
                    Ok(result) => result,
                    Err(join_err) => {
                        error!(error = %join_err, "Twitch startup attempt task failed");
                        Err(join_err.to_string())
                    }
                };

                match result {
                    Ok(()) => {
                        // The attempt succeeded, but settings may have changed
                        // while it was pending; install only if still wanted.
                        let settings = app_state.twitch.settings.read().await;
                        let settings_still_match =
                            connection_settings_match(&settings, &attempt.settings);
                        drop(settings);

                        if settings_still_match {
                            info!("Twitch client started");
                            *app_state.twitch.client.write().await = Some(attempt.client.clone());
                            twitch_client = Some(attempt.client);
                        } else {
                            info!("Twitch startup finished but connection settings changed; discarding client");
                            discard_attempt(attempt).await;
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Twitch reconnect attempt failed");
                        let delay = schedule_retry(&mut retry_deadline, &mut retry_attempt);
                        info!(
                            ?delay,
                            attempt = retry_attempt,
                            generation = attempt_generation,
                            "Scheduling next Twitch reconnect"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bound_reconnect_jitter_ms, classify_recv_error, is_reconnect_retryable,
        reconnect_base_delay_ms, run_twitch_client_core, RecvErrorAction,
    };
    use crate::events::{TwitchConnectionStatus, TwitchEvent};
    use crate::state::AppState;
    use crate::twitch::{TwitchClient, TwitchStatus};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::{broadcast, mpsc};
    use tokio_util::sync::CancellationToken;

    struct DropGuard(Arc<AtomicBool>);

    impl Drop for DropGuard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn valid_settings() -> crate::config::TwitchSettings {
        crate::config::TwitchSettings {
            enabled: true,
            username: "user".to_string(),
            token: "token".to_string(),
            channel: "channel".to_string(),
            start_on_boot: false,
        }
    }

    async fn set_settings(state: &AppState, settings: crate::config::TwitchSettings) {
        *state.twitch.settings.write().await = settings;
    }

    async fn wait_until(mut cond: impl FnMut() -> bool) {
        for _ in 0..10_000 {
            if cond() {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("condition not satisfied");
    }

    async fn drop_state(state: AppState) {
        tokio::task::spawn_blocking(move || drop(state))
            .await
            .expect("drop AppState on a blocking thread");
    }

    #[tokio::test]
    async fn stop_cancels_pending_attempt_and_never_retries() {
        let state = AppState::new();
        set_settings(&state, valid_settings()).await;

        let (event_tx, _) = broadcast::channel::<TwitchEvent>(16);
        let event_rx = event_tx.subscribe();

        let statuses = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicBool::new(false));

        let shutdown = CancellationToken::new();

        let factory = {
            let calls = Arc::clone(&calls);
            let dropped = Arc::clone(&dropped);
            move |_client: TwitchClient| {
                calls.fetch_add(1, Ordering::SeqCst);
                let guard = DropGuard(Arc::clone(&dropped));
                async move {
                    let _guard = guard;
                    std::future::pending::<()>().await;
                    Ok(())
                }
            }
        };

        let report = {
            let statuses = Arc::clone(&statuses);
            move |status: TwitchConnectionStatus| {
                statuses.lock().unwrap().push(status);
            }
        };

        let core = tokio::spawn(run_twitch_client_core(
            state.clone(),
            event_rx,
            shutdown.clone(),
            report,
            factory,
        ));

        event_tx.send(TwitchEvent::Restart).unwrap();
        wait_until(|| calls.load(Ordering::SeqCst) == 1).await;

        event_tx.send(TwitchEvent::Stop).unwrap();
        wait_until(|| dropped.load(Ordering::SeqCst)).await;

        assert!(
            dropped.load(Ordering::SeqCst),
            "pending attempt future must be dropped"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "factory must not be called again after Stop"
        );
        assert!(state.twitch.client.read().await.is_none());

        {
            let statuses = statuses.lock().unwrap();
            assert_eq!(statuses.last(), Some(&TwitchConnectionStatus::Disconnected));
        }

        shutdown.cancel();
        let _ = core.await;

        drop_state(state).await;
    }

    #[tokio::test]
    async fn shutdown_exits_promptly_during_pending_attempt() {
        let state = AppState::new();
        set_settings(&state, valid_settings()).await;

        let (event_tx, _) = broadcast::channel::<TwitchEvent>(16);
        let event_rx = event_tx.subscribe();

        let calls = Arc::new(AtomicUsize::new(0));

        let shutdown = CancellationToken::new();

        let factory = {
            let calls = Arc::clone(&calls);
            move |_client: TwitchClient| {
                calls.fetch_add(1, Ordering::SeqCst);
                std::future::pending::<Result<(), String>>()
            }
        };

        let report = |_status: TwitchConnectionStatus| {};

        let core = tokio::spawn(run_twitch_client_core(
            state.clone(),
            event_rx,
            shutdown.clone(),
            report,
            factory,
        ));

        event_tx.send(TwitchEvent::Restart).unwrap();
        wait_until(|| calls.load(Ordering::SeqCst) == 1).await;

        shutdown.cancel();

        let result = tokio::time::timeout(Duration::from_secs(2), core).await;
        assert!(
            result.is_ok(),
            "supervisor must finish promptly during shutdown"
        );

        assert!(state.twitch.client.read().await.is_none());

        drop_state(state).await;
    }

    #[tokio::test]
    async fn fresh_success_with_disabled_settings_is_not_installed() {
        let state = AppState::new();
        set_settings(&state, valid_settings()).await;

        let (event_tx, _) = broadcast::channel::<TwitchEvent>(16);
        let event_rx = event_tx.subscribe();

        let statuses = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let (status_tx, mut status_rx) = mpsc::unbounded_channel();

        let shutdown = CancellationToken::new();

        // The attempt stays pending until the test resolves the oneshot, so
        // settings can be disabled while the startup "connect" is in flight.
        let (resolve_tx, resolve_rx) = tokio::sync::oneshot::channel::<()>();
        let factory = {
            let calls = Arc::clone(&calls);
            let mut resolve_rx = Some(resolve_rx);
            move |_client: TwitchClient| {
                calls.fetch_add(1, Ordering::SeqCst);
                let rx = resolve_rx
                    .take()
                    .expect("factory must only be called once in this test");
                async move {
                    let _ = rx.await;
                    Ok(())
                }
            }
        };

        let report = {
            let statuses = Arc::clone(&statuses);
            move |status: TwitchConnectionStatus| {
                statuses.lock().unwrap().push(status.clone());
                let _ = status_tx.send(status);
            }
        };

        let core = tokio::spawn(run_twitch_client_core(
            state.clone(),
            event_rx,
            shutdown.clone(),
            report,
            factory,
        ));

        event_tx.send(TwitchEvent::Restart).unwrap();
        wait_until(|| calls.load(Ordering::SeqCst) == 1).await;
        while status_rx.try_recv().is_ok() {}

        // Disable WITHOUT sending Stop: the successful completion must
        // invalidate itself instead of installing the client.
        let mut disabled = valid_settings();
        disabled.enabled = false;
        set_settings(&state, disabled).await;

        let _ = resolve_tx.send(());

        // Once the completion has been discarded, the next supervisor tick
        // observes no client/retry and publishes Disconnected. Await that
        // state transition instead of treating elapsed time as evidence.
        let acknowledged = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if status_rx.recv().await == Some(TwitchConnectionStatus::Disconnected) {
                    break;
                }
            }
        })
        .await;
        assert!(
            acknowledged.is_ok(),
            "supervisor must acknowledge the discarded completion"
        );

        assert!(
            state.twitch.client.read().await.is_none(),
            "client must not be installed when settings became disabled"
        );
        {
            let statuses = statuses.lock().unwrap();
            assert!(
                !statuses.contains(&TwitchConnectionStatus::Connected),
                "no Connected status may be published: {:?}",
                statuses
            );
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "no retry may be scheduled for the discarded completion"
        );

        shutdown.cancel();
        let _ = core.await;

        drop_state(state).await;
    }

    #[tokio::test]
    async fn fresh_success_with_changed_connection_settings_is_not_installed() {
        let state = AppState::new();
        set_settings(&state, valid_settings()).await;

        let (event_tx, _) = broadcast::channel::<TwitchEvent>(16);
        let event_rx = event_tx.subscribe();
        let calls = Arc::new(AtomicUsize::new(0));
        let (status_tx, mut status_rx) = mpsc::unbounded_channel();
        let shutdown = CancellationToken::new();

        let (resolve_tx, resolve_rx) = tokio::sync::oneshot::channel::<()>();
        let factory = {
            let calls = Arc::clone(&calls);
            let mut resolve_rx = Some(resolve_rx);
            move |_client: TwitchClient| {
                calls.fetch_add(1, Ordering::SeqCst);
                let rx = resolve_rx
                    .take()
                    .expect("factory must only be called once in this test");
                async move {
                    let _ = rx.await;
                    Ok(())
                }
            }
        };

        let core = tokio::spawn(run_twitch_client_core(
            state.clone(),
            event_rx,
            shutdown.clone(),
            move |status| {
                let _ = status_tx.send(status);
            },
            factory,
        ));

        event_tx.send(TwitchEvent::Restart).unwrap();
        wait_until(|| calls.load(Ordering::SeqCst) == 1).await;
        while status_rx.try_recv().is_ok() {}

        let mut changed = valid_settings();
        changed.channel = "other-channel".to_string();
        set_settings(&state, changed).await;
        let _ = resolve_tx.send(());

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if status_rx.recv().await == Some(TwitchConnectionStatus::Disconnected) {
                    break;
                }
            }
        })
        .await
        .expect("supervisor must discard a completion for changed connection settings");

        assert!(state.twitch.client.read().await.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        shutdown.cancel();
        let _ = core.await;
        drop_state(state).await;
    }

    #[tokio::test]
    async fn start_on_boot_change_does_not_invalidate_fresh_success() {
        let state = AppState::new();
        set_settings(&state, valid_settings()).await;

        let (event_tx, _) = broadcast::channel::<TwitchEvent>(16);
        let event_rx = event_tx.subscribe();
        let calls = Arc::new(AtomicUsize::new(0));
        let shutdown = CancellationToken::new();

        let (resolve_tx, resolve_rx) = tokio::sync::oneshot::channel::<()>();
        let factory = {
            let calls = Arc::clone(&calls);
            let mut resolve_rx = Some(resolve_rx);
            move |_client: TwitchClient| {
                calls.fetch_add(1, Ordering::SeqCst);
                let rx = resolve_rx
                    .take()
                    .expect("factory must only be called once in this test");
                async move {
                    let _ = rx.await;
                    Ok(())
                }
            }
        };

        let core = tokio::spawn(run_twitch_client_core(
            state.clone(),
            event_rx,
            shutdown.clone(),
            |_status| {},
            factory,
        ));

        event_tx.send(TwitchEvent::Restart).unwrap();
        wait_until(|| calls.load(Ordering::SeqCst) == 1).await;

        let mut changed = valid_settings();
        changed.start_on_boot = true;
        set_settings(&state, changed).await;
        let _ = resolve_tx.send(());

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if state.twitch.client.read().await.is_some() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("start_on_boot alone must not invalidate a successful attempt");

        shutdown.cancel();
        let _ = core.await;
        drop_state(state).await;
    }

    #[tokio::test]
    async fn queued_stop_is_processed_before_ready_completion() {
        let state = AppState::new();
        set_settings(&state, valid_settings()).await;

        let (event_tx, _) = broadcast::channel::<TwitchEvent>(16);
        let event_rx = event_tx.subscribe();
        let calls = Arc::new(AtomicUsize::new(0));
        let (status_tx, mut status_rx) = mpsc::unbounded_channel();
        let shutdown = CancellationToken::new();

        let (resolve_tx, resolve_rx) = tokio::sync::oneshot::channel::<()>();
        let factory = {
            let calls = Arc::clone(&calls);
            let mut resolve_rx = Some(resolve_rx);
            move |_client: TwitchClient| {
                calls.fetch_add(1, Ordering::SeqCst);
                let rx = resolve_rx
                    .take()
                    .expect("factory must only be called once in this test");
                async move {
                    let _ = rx.await;
                    Ok(())
                }
            }
        };

        let core = tokio::spawn(run_twitch_client_core(
            state.clone(),
            event_rx,
            shutdown.clone(),
            move |status| {
                let _ = status_tx.send(status);
            },
            factory,
        ));

        event_tx.send(TwitchEvent::Restart).unwrap();
        wait_until(|| calls.load(Ordering::SeqCst) == 1).await;
        while status_rx.try_recv().is_ok() {}

        // If the ready completion branch ran first, it would block re-reading
        // settings here and Stop could not publish Disconnected. The event-first
        // branch order lets Stop complete without taking the settings lock.
        let settings_guard = state.twitch.settings.write().await;
        event_tx.send(TwitchEvent::Stop).unwrap();
        let _ = resolve_tx.send(());

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if status_rx.recv().await == Some(TwitchConnectionStatus::Disconnected) {
                    break;
                }
            }
        })
        .await
        .expect("queued Stop must win over a simultaneously ready completion");

        assert!(state.twitch.client.read().await.is_none());
        drop(settings_guard);

        shutdown.cancel();
        let _ = core.await;
        drop_state(state).await;
    }

    #[tokio::test]
    async fn restart_supersedes_pending_attempt() {
        let state = AppState::new();
        set_settings(&state, valid_settings()).await;

        let (event_tx, _) = broadcast::channel::<TwitchEvent>(16);
        let event_rx = event_tx.subscribe();

        let calls = Arc::new(AtomicUsize::new(0));
        let first_dropped = Arc::new(AtomicBool::new(false));

        let shutdown = CancellationToken::new();

        // Every attempt stays pending forever and sets the shared flag when
        // dropped, so the test observes the Restart discarding attempt #1.
        let factory = {
            let calls = Arc::clone(&calls);
            let first_dropped = Arc::clone(&first_dropped);
            move |_client: TwitchClient| {
                calls.fetch_add(1, Ordering::SeqCst);
                let guard = DropGuard(Arc::clone(&first_dropped));
                async move {
                    let _guard = guard;
                    std::future::pending::<()>().await;
                    Ok(())
                }
            }
        };

        let report = |_status: TwitchConnectionStatus| {};

        let core = tokio::spawn(run_twitch_client_core(
            state.clone(),
            event_rx,
            shutdown.clone(),
            report,
            factory,
        ));

        event_tx.send(TwitchEvent::Restart).unwrap();
        wait_until(|| calls.load(Ordering::SeqCst) == 1).await;

        event_tx.send(TwitchEvent::Restart).unwrap();
        wait_until(|| calls.load(Ordering::SeqCst) == 2).await;

        assert!(
            first_dropped.load(Ordering::SeqCst),
            "Restart must discard the superseded attempt"
        );
        assert!(state.twitch.client.read().await.is_none());

        shutdown.cancel();
        let _ = core.await;

        drop_state(state).await;
    }

    #[test]
    fn initial_delay_is_500ms() {
        assert_eq!(reconnect_base_delay_ms(0), 500);
    }

    #[test]
    fn delay_doubles_each_attempt() {
        assert_eq!(reconnect_base_delay_ms(1), 1_000);
        assert_eq!(reconnect_base_delay_ms(2), 2_000);
        assert_eq!(reconnect_base_delay_ms(3), 4_000);
        assert_eq!(reconnect_base_delay_ms(4), 8_000);
    }

    #[test]
    fn delay_saturates_at_30_seconds() {
        assert_eq!(reconnect_base_delay_ms(6), 30_000);
        assert_eq!(reconnect_base_delay_ms(7), 30_000);
        assert_eq!(reconnect_base_delay_ms(100), 30_000);
    }

    #[test]
    fn no_overflow_for_very_large_attempt() {
        assert_eq!(reconnect_base_delay_ms(u32::MAX), 30_000);
    }

    #[test]
    fn jitter_is_bounded_lower() {
        assert_eq!(bound_reconnect_jitter_ms(0), 100);
        assert_eq!(bound_reconnect_jitter_ms(50), 100);
        assert_eq!(bound_reconnect_jitter_ms(100), 100);
    }

    #[test]
    fn jitter_is_bounded_upper() {
        assert_eq!(bound_reconnect_jitter_ms(500), 500);
        assert_eq!(bound_reconnect_jitter_ms(501), 500);
        assert_eq!(bound_reconnect_jitter_ms(i64::MAX), 500);
        assert_eq!(bound_reconnect_jitter_ms(i64::MIN), 100);
    }

    #[test]
    fn jitter_passes_through_in_range() {
        assert_eq!(bound_reconnect_jitter_ms(250), 250);
    }

    #[test]
    fn only_transport_failure_is_retryable() {
        assert!(is_reconnect_retryable(&TwitchStatus::TransportFailure(
            "connection dropped".to_string()
        )));
        assert!(!is_reconnect_retryable(&TwitchStatus::Error(
            "boom".to_string()
        )));
        assert!(!is_reconnect_retryable(&TwitchStatus::Disconnected));
        assert!(!is_reconnect_retryable(&TwitchStatus::Connecting));
        assert!(!is_reconnect_retryable(&TwitchStatus::Connected));
    }

    #[test]
    fn recv_error_closed_breaks_loop() {
        assert_eq!(
            classify_recv_error(tokio::sync::broadcast::error::RecvError::Closed),
            RecvErrorAction::Break
        );
    }

    #[test]
    fn recv_error_lagged_continues_loop_with_skipped_count() {
        assert_eq!(
            classify_recv_error(tokio::sync::broadcast::error::RecvError::Lagged(3)),
            RecvErrorAction::Continue(3)
        );
    }

    #[tokio::test]
    async fn broadcast_receiver_recovers_after_lag() {
        let (tx, mut rx) = broadcast::channel::<TwitchEvent>(1);

        // Переполняем канал малой ёмкости, пока receiver не читает: события
        // перезаписываются и следующий recv обязан сообщить о потере, а не
        // вернуть закрытие канала.
        tx.send(TwitchEvent::Stop).unwrap();
        tx.send(TwitchEvent::Stop).unwrap();
        tx.send(TwitchEvent::SendMessage("latest".to_string()))
            .unwrap();

        match rx.recv().await {
            Err(e) => match classify_recv_error(e) {
                RecvErrorAction::Continue(skipped) => {
                    assert!(skipped > 0, "lag must report skipped events");
                }
                RecvErrorAction::Break => panic!("lag must not be classified as Break"),
            },
            Ok(_) => panic!("receiver must observe lag after overflow"),
        }

        let next = rx
            .recv()
            .await
            .expect("receiver must recover and receive the next event after lag");
        match next {
            TwitchEvent::SendMessage(text) => assert_eq!(text, "latest"),
            other => panic!("next event after lag must be the latest send, got {other:?}"),
        }
    }
}
