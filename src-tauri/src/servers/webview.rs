// WebView server module
//
// This module manages the WebView server for broadcasting TTS to web clients.
// Refactored from lib.rs WebView server thread (2026-03-11)

use crate::events::AppEvent;
use crate::webview::service::START_FAILED_CODE;
use crate::webview::WebViewServer;
use crate::webview::WebViewServerStatus;
use crate::webview::WebViewSettings;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Период повтора `typing: true`, пока набор остаётся активным.
///
/// Набор передаётся один раз на burst: редактор шлёт начало сразу, а завершение —
/// по idle-таймауту. Поэтому SSE-клиент не может отличить потерянное
/// `typing: false` (переполнение broadcast-буфера) от продолжающегося набора.
/// Пока состояние активно, сервер повторяет `typing: true`; клиент сбрасывает
/// индикатор, если повторов нет дольше собственного таймаута (в bundled-шаблоне
/// это `TYPING_RESET_MS`, заведомо больше периода повтора).
const TYPING_HEARTBEAT: Duration = Duration::from_secs(5);

/// Состояние heartbeat'а набора: активен ли набор и когда слать повтор.
#[derive(Debug)]
struct TypingHeartbeat {
    active: bool,
    next_at: Instant,
}

impl TypingHeartbeat {
    fn new(now: Instant) -> Self {
        Self {
            active: false,
            next_at: now,
        }
    }

    /// Применить событие набора: состояние передаётся клиенту вызывающей
    /// стороной, повтор планируется от этого момента.
    fn set(&mut self, typing: bool, now: Instant) {
        self.active = typing;
        self.next_at = now + TYPING_HEARTBEAT;
    }

    /// Пора ли повторить `typing: true`. Планирует следующий повтор.
    fn due(&mut self, now: Instant) -> bool {
        if !self.active || now < self.next_at {
            return false;
        }
        self.next_at = now + TYPING_HEARTBEAT;
        true
    }
}

/// Wait for an explicit restart or shutdown after a startup failure.
///
/// A failed start never retries on a timer and never wakes on unrelated events
/// (TTS text, typing, template reloads, etc.). Only `RestartWebViewServer` —
/// an explicit user start/restart, or a settings change that re-applies the
/// server — or shutdown resumes the supervisor. A manual stop also arrives as
/// `RestartWebViewServer` (the `enabled` flip is persisted first), after which
/// the supervisor re-reads `enabled == false` and goes to `Stopped`.
///
/// Returns `true` to continue the supervision loop, `false` to exit.
async fn wait_for_explicit_restart(
    shutdown: &CancellationToken,
    webview_rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> bool {
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => return false,
            event = webview_rx.recv() => {
                match event {
                    Some(AppEvent::RestartWebViewServer) => return true,
                    Some(AppEvent::Quit) => return false,
                    // Unrelated events must not wake a failed server.
                    Some(_) => continue,
                    None => return false,
                }
            }
        }
    }
}

/// Run WebView server in async context
/// This function is called from a dedicated thread with tokio runtime
pub async fn run_webview_server(
    webview_settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    app_handle: AppHandle,
    mut webview_rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    shutdown: CancellationToken,
) {
    {
        let settings = webview_settings.read().await;
        if settings.start_on_boot && !settings.enabled {
            drop(settings);
            webview_settings.write().await.enabled = true;
            info!("[WEBVIEW] Auto-start on boot: enabled");
        }
    }

    // Первый проход цикла — boot-попытка, пользователь её не инициировал.
    let mut attempt_attended = false;

    loop {
        // Check current settings
        let settings = webview_settings.read().await;
        let enabled = settings.enabled;
        let port = settings.port;
        drop(settings);

        // Note: start_on_boot only applies to initial app startup via setup.rs
        // We don't auto-start here to avoid conflicts with manual stop/start

        if enabled {
            let Some(state) = app_handle.try_state::<crate::state::AppState>() else {
                error!("WebView AppState unavailable");
                return;
            };
            state
                .webview
                .set_status(&app_handle, WebViewServerStatus::Starting);
            info!("[WEBVIEW] ========================================");
            info!("[WEBVIEW] STARTING SERVER");
            info!("[WEBVIEW]   Address: {}:{}", crate::webview::WEBVIEW_BIND_ADDRESS, port);
            info!("[WEBVIEW] ========================================");

            let server = match WebViewServer::new(Arc::clone(&webview_settings)).await {
                Ok(s) => s,
                Err(e) => {
                    // Technical detail stays in logs; the frontend receives the
                    // stable generic failure code only.
                    error!(error = %e, "[WEBVIEW] Failed to create server");
                    state.webview.set_status(
                        &app_handle,
                        WebViewServerStatus::Error {
                            message: START_FAILED_CODE.to_string(),
                            attended: attempt_attended,
                        },
                    );
                    if !wait_for_explicit_restart(&shutdown, &mut webview_rx).await {
                        return;
                    }
                    attempt_attended = true;
                    continue;
                }
            };

            // Spawn server task with improved error handling
            // Владелец UPnP публикуется в state: команда переключения применяет
            // mapping напрямую и сообщает результат, а не подтверждает тумблер
            // заранее. На остановке сервера владелец снимается.
            state.webview.set_upnp_manager(server.upnp_manager.clone());
            let server_clone = server.clone();
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let (upnp_error_tx, mut upnp_error_rx) =
                tokio::sync::mpsc::unbounded_channel::<String>();
            let mut server_handle = tokio::spawn(async move {
                info!("[WEBVIEW] Server task started, waiting for connections...");

                if let Err(e) = server_clone
                    .start(Some(ready_tx), Some(upnp_error_tx))
                    .await
                {
                    // `e` is the stable encoded code for the frontend; the
                    // technical detail was already logged at the bind site.
                    warn!(error = %e, "[WEBVIEW] Server startup failed");
                }
                // Server task completed
                info!("[WEBVIEW] Server task stopped");
            });

            match ready_rx.await {
                Ok(Ok(())) => {
                    state
                        .webview
                        .set_status(&app_handle, WebViewServerStatus::Running);
                }
                Ok(Err(message)) => {
                    state
                        .webview
                        .set_status(&app_handle, WebViewServerStatus::Error {
                            message,
                            attended: attempt_attended,
                        });
                    let _ = server_handle.await;
                    server.stop().await;
                    state.webview.clear_upnp_runtime(&app_handle);
                    if !wait_for_explicit_restart(&shutdown, &mut webview_rx).await {
                        return;
                    }
                    attempt_attended = true;
                    continue;
                }
                Err(_) => {
                    let _ = server_handle.await;
                    server.stop().await;
                    state.webview.clear_upnp_runtime(&app_handle);
                    state.webview.set_status(
                        &app_handle,
                        WebViewServerStatus::Error {
                            message: START_FAILED_CODE.to_string(),
                            attended: attempt_attended,
                        },
                    );
                    if !wait_for_explicit_restart(&shutdown, &mut webview_rx).await {
                        return;
                    }
                    attempt_attended = true;
                    continue;
                }
            }

            // Handle events and broadcast text
            let mut server_running = true;
            let mut typing_heartbeat = TypingHeartbeat::new(Instant::now());
            while server_running {
                // Check if settings changed
                let current_settings = webview_settings.read().await;
                let still_enabled = current_settings.enabled;
                let same_port = current_settings.port == port;
                drop(current_settings);

                if !still_enabled || !same_port {
                    info!("[WEBVIEW] ========================================");
                    info!("[WEBVIEW] STOPPING SERVER (settings changed)");
                    info!("[WEBVIEW]   Still enabled: {}", still_enabled);
                    info!("[WEBVIEW]   Same port: {}", same_port);
                    info!("[WEBVIEW] ========================================");

                    // Stop server and clean up UPnP
                    server.stop().await;
                    state.webview.clear_upnp_runtime(&app_handle);

                    server_handle.abort();
                    state
                        .webview
                        .set_status(&app_handle, WebViewServerStatus::Stopped);
                    // Следующая попытка идёт от сохранения настроек.
                    attempt_attended = true;
                    server_running = false;
                } else {
                    // Пока набор активен, повторяем `typing: true`: клиент
                    // отличает потерянное `typing: false` от продолжающегося
                    // набора по отсутствию повторов.
                    if typing_heartbeat.due(Instant::now()) {
                        server.broadcast_typing(true).await;
                    }

                    tokio::select! {
                        biased;
                        result = &mut server_handle => {
                            if let Err(join_error) = result {
                                error!(%join_error, "WebView server task join failed");
                            }
                            // Сервер больше не обслуживает порт: владельца UPnP
                            // в state быть не должно. Незапланированный выход —
                            // не повод автоматически перезапускаться: ждём явного
                            // перезапуска или shutdown.
                            server.stop().await;
                            state.webview.clear_upnp_runtime(&app_handle);
                            // Рантайм-падение после успешной работы — не
                            // пользовательская попытка: всегда красный навсегда.
                            state.webview.set_status(&app_handle, WebViewServerStatus::Error {
                                message: START_FAILED_CODE.to_string(),
                                attended: false,
                            });
                            if !wait_for_explicit_restart(&shutdown, &mut webview_rx).await {
                                return;
                            }
                            attempt_attended = true;
                            server_running = false;
                        }
                        _ = shutdown.cancelled() => {
                            info!("[WEBVIEW] ⛔ Shutdown signal");
                            server.stop().await;
                            state.webview.clear_upnp_runtime(&app_handle);
                            server_handle.abort();
                            state.webview.set_status(&app_handle, WebViewServerStatus::Stopped);
                            return;
                        }
                        upnp_code = upnp_error_rx.recv() => {
                            // Провалившийся фоновый UPnP-проброс: наружу уходит
                            // только код отказа, сырой текст router'а не
                            // покидает backend. Код дополнительно хранится в
                            // state, чтобы отказ остался видимым, даже если
                            // событие пришло до регистрации listener.
                            if let Some(code) = upnp_code {
                                state.webview.set_upnp_failure(Some(code));
                                state.webview.publish_upnp_forward_status(&app_handle);
                            }
                        }
                        result = tokio::time::timeout(
                            tokio::time::Duration::from_secs(1),
                            webview_rx.recv(),
                        ) => {
                            match result {
                                Ok(Some(event)) => {
                                    info!("[WEBVIEW] 📨 Event received: {:?}", std::mem::discriminant(&event));
                                    match event {
                                        AppEvent::Quit => {
                                            info!("[WEBVIEW] ⚠ Quit event received, shutting down server...");

                                            // Stop server and clean up UPnP
                                            server.stop().await;
                                            state.webview.clear_upnp_runtime(&app_handle);

                                            server_handle.abort();
                                            state.webview.set_status(&app_handle, WebViewServerStatus::Stopped);
                                            info!("[WEBVIEW] Server shut down for quit");
                                            return;
                                        }
                                        AppEvent::TextSentToTts(routed) => {
                                            info!(text_len = routed.text.chars().count(), "[WEBVIEW] Broadcasting to SSE clients");
                                            server.broadcast_text(&routed.text).await;
                                        }
                                        AppEvent::RestartWebViewServer => {
                                            info!("[WEBVIEW] ⚠ Restart event received, stopping server...");

                                            // Stop server and clean up UPnP
                                            server.stop().await;
                                            state.webview.clear_upnp_runtime(&app_handle);

                                            server_handle.abort();
                                            state.webview.set_status(&app_handle, WebViewServerStatus::Stopped);
                                            // Wait a bit for the server to fully shut down
                                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                            // Явный рестарт пользователя: следующая попытка attended.
                                            attempt_attended = true;
                                            server_running = false;
                                        }
                                        AppEvent::ReloadWebViewTemplates => {
                                            info!("[WEBVIEW] 🔄 Reloading templates...");
                                            match server.templates.reload().await {
                                                Ok(()) => {
                                                    info!("[WEBVIEW] ✅ Templates reloaded successfully");
                                                }
                                                Err(e) => {
                                                    error!("[WEBVIEW] ❌ Failed to reload templates: {}", e);
                                                }
                                            }
                                        }
                                        AppEvent::WebViewTypingChanged(typing) => {
                                            debug!("[WEBVIEW] ⌨️ Typing changed: {}", typing);
                                            typing_heartbeat.set(typing, Instant::now());
                                            server.broadcast_typing(typing).await;
                                        }
                                        _ => {
                                            info!("[WEBVIEW] ℹ️  Ignoring event: {:?}", std::mem::discriminant(&event));
                                        }
                                    }
                                }
                                Err(_) => {
                                    // Timeout - continue loop to check settings.
                                    // Успешная фоновая попытка UPnP не имеет
                                    // отдельного события: публикуем `Open`, как
                                    // только mapping подтвердится. Отказы тут не
                                    // публикуются (их публикует явный путь).
                                    state.webview.publish_upnp_open_if_confirmed(&app_handle);
                                }
                                Ok(None) => {
                                    // Channel closed
                                    info!("[WEBVIEW] Event channel disconnected");
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            if let Some(state) = app_handle.try_state::<crate::state::AppState>() {
                // Сервер не запущен: mapping не существует, владельца быть не должно.
                state.webview.clear_upnp_runtime(&app_handle);
                state
                    .webview
                    .set_status(&app_handle, WebViewServerStatus::Stopped);
            }
            info!("[WEBVIEW] ========================================");
            info!("[WEBVIEW] SERVER DISABLED");
            info!("[WEBVIEW] Waiting for enable signal...");
            info!("[WEBVIEW] ========================================");
            // Wait for enable or restart event
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown.cancelled() => {
                        return;
                    }
                    result = tokio::time::timeout(
                        tokio::time::Duration::from_secs(2),
                        webview_rx.recv(),
                    ) => {
                        match result {
                            Ok(Some(AppEvent::Quit)) => {
                                info!("[WEBVIEW] ⚠ Quit event received (server disabled)");
                                return;
                            }
                            Ok(Some(AppEvent::RestartWebViewServer)) => {
                                info!("[WEBVIEW] ⚠ Restart event received, exiting disabled state");
                                attempt_attended = true;
                                break;
                            }
                            Ok(Some(AppEvent::TextSentToTts(routed))) => {
                                // Ignore TTS events while disabled but log them
                                info!(text_len = routed.text.chars().count(), "[WEBVIEW] Ignoring TTS text (server disabled)");
                            }
                            Ok(Some(AppEvent::WebViewTypingChanged(typing))) => {
                                debug!("[WEBVIEW] Ignoring typing change (server disabled): {}", typing);
                            }
                            Err(_) => {
                                // Timeout - check if enabled now
                                let settings = webview_settings.read().await;
                                if settings.enabled {
                                    drop(settings);
                                    info!("[WEBVIEW] ✓ Enabled detected via timeout!");
                                    attempt_attended = true;
                                    break;
                                }
                                drop(settings);
                            }
                            Ok(None) => {
                                info!("[WEBVIEW] Event channel disconnected");
                                return;
                            }
                            Ok(Some(other)) => {
                                info!("[WEBVIEW] Received unexpected event while disabled: {:?}", other);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::RoutedText;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn wait_for_explicit_restart_ignores_unrelated_events_and_returns_on_restart() {
        let shutdown = CancellationToken::new();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        let mut rx = rx;
        let mut task = tokio::spawn(async move { wait_for_explicit_restart(&shutdown, &mut rx).await });

        // Unrelated events must not wake a failed server.
        tx.send(AppEvent::TextSentToTts(RoutedText::broadcast("hi".into())))
            .unwrap();
        tx.send(AppEvent::WebViewTypingChanged(true)).unwrap();
        tx.send(AppEvent::ReloadWebViewTemplates).unwrap();

        let still_waiting = tokio::time::timeout(Duration::from_millis(100), &mut task).await;
        assert!(
            still_waiting.is_err(),
            "unrelated events must not resume the supervisor"
        );

        tx.send(AppEvent::RestartWebViewServer).unwrap();
        assert!(task.await.unwrap(), "an explicit restart must resume the supervisor");
    }

    #[tokio::test]
    async fn wait_for_explicit_restart_exits_on_shutdown() {
        let shutdown = CancellationToken::new();
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        let mut rx = rx;
        let task_shutdown = shutdown.clone();
        let task = tokio::spawn(async move { wait_for_explicit_restart(&task_shutdown, &mut rx).await });
        shutdown.cancel();
        assert!(!task.await.unwrap(), "shutdown must exit the supervisor");
    }

    #[tokio::test]
    async fn wait_for_explicit_restart_exits_on_quit_event() {
        let shutdown = CancellationToken::new();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        let mut rx = rx;
        let task = tokio::spawn(async move { wait_for_explicit_restart(&shutdown, &mut rx).await });
        tx.send(AppEvent::Quit).unwrap();
        assert!(!task.await.unwrap(), "quit must exit the supervisor");
    }

    #[test]
    fn typing_heartbeat_stays_silent_while_typing_is_inactive() {
        let now = Instant::now();
        let mut heartbeat = TypingHeartbeat::new(now);

        assert!(!heartbeat.due(now));
        assert!(
            !heartbeat.due(now + Duration::from_secs(3600)),
            "inactive typing must never produce heartbeats"
        );
    }

    #[test]
    fn typing_heartbeat_repeats_while_typing_is_active() {
        let now = Instant::now();
        let mut heartbeat = TypingHeartbeat::new(now);
        heartbeat.set(true, now);

        assert!(
            !heartbeat.due(now + TYPING_HEARTBEAT - Duration::from_millis(1)),
            "heartbeat must not fire before its period"
        );
        assert!(
            heartbeat.due(now + TYPING_HEARTBEAT),
            "active typing must be refreshed once the period elapsed"
        );
        assert!(
            !heartbeat.due(now + TYPING_HEARTBEAT + Duration::from_secs(1)),
            "the next refresh is scheduled from the previous one"
        );
        assert!(heartbeat.due(now + TYPING_HEARTBEAT * 2));
    }

    #[test]
    fn typing_heartbeat_stops_after_the_end_of_typing() {
        let now = Instant::now();
        let mut heartbeat = TypingHeartbeat::new(now);
        heartbeat.set(true, now);
        assert!(heartbeat.due(now + TYPING_HEARTBEAT));

        let stopped_at = now + TYPING_HEARTBEAT + Duration::from_secs(1);
        heartbeat.set(false, stopped_at);

        assert!(
            !heartbeat.due(stopped_at + Duration::from_secs(3600)),
            "a completed burst must stop the heartbeat"
        );
    }
}
