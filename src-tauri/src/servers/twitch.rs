// Twitch server module
//
// This module manages the Twitch client connection.
// Refactored from lib.rs Twitch client thread (2026-03-11)

use crate::events::{TwitchConnectionStatus, TwitchEvent};
use crate::state::AppState;
use crate::twitch::{TwitchClient, TwitchStatus};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

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

/// Run Twitch client in async context
pub async fn run_twitch_client(
    app_state: AppState,
    app_handle: AppHandle,
    mut twitch_rx: Receiver<TwitchEvent>,
    shutdown: CancellationToken,
) {
    let mut twitch_client: Option<TwitchClient> = None;
    let mut last_status = TwitchConnectionStatus::Disconnected;
    let mut status_check_interval = tokio::time::interval(Duration::from_secs(1));
    let mut retry_deadline: Option<Instant> = None;
    let mut retry_attempt = 0u32;

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

    let update_status = |status: TwitchConnectionStatus| {
        *app_state.twitch.connection_status.lock() = status.clone();
        let _ = app_handle.emit("twitch-status-changed", &status);
    };

    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => {
                info!("[TWITCH] ⛔ Shutdown");
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
                        info!(?delay, attempt = retry_attempt, "Scheduling Twitch reconnect");
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

                    let client = TwitchClient::new(settings_clone.into());
                    match client.start().await.map_err(|e| e.to_string()) {
                        Ok(()) => {
                            info!("Twitch reconnect attempt started");
                            *app_state.twitch.client.write().await = Some(client.clone());
                            twitch_client = Some(client);
                        }
                        Err(e) => {
                            error!(error = %e, "Twitch reconnect attempt failed");
                            let delay = schedule_retry(&mut retry_deadline, &mut retry_attempt);
                            info!(?delay, attempt = retry_attempt, "Scheduling next Twitch reconnect");
                        }
                    }
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

                                let settings = app_state.twitch.settings.read().await;
                                let is_enabled = settings.enabled;
                                let is_valid = settings.is_valid().is_ok();
                                let settings_clone = settings.clone();
                                drop(settings);

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

                                        let client = TwitchClient::new(settings_clone.into());
                                        // Convert the boxed error up front: `Box<dyn StdError>`
                                        // is not `Send`, and the match temporary would otherwise
                                        // live across the `client` publish await below.
                                        let start_result =
                                            client.start().await.map_err(|e| e.to_string());
                                        match start_result {
                                            Ok(()) => {
                                                info!("Client started, waiting for connection");
                                                *app_state.twitch.client.write().await = Some(client.clone());
                                                twitch_client = Some(client);
                                            }
                                            Err(e) => {
                                                error!(error = %e, "Failed to start client");
                                                let delay = schedule_retry(&mut retry_deadline, &mut retry_attempt);
                                                info!(?delay, attempt = retry_attempt, "Scheduling Twitch reconnect after initial failure");
                                            }
                                        }
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
                                if let Some(client) = twitch_client.take() {
                                    client.stop().await;
                                }
                                *app_state.twitch.client.write().await = None;
                                last_status = TwitchConnectionStatus::Disconnected;
                                update_status(last_status.clone());
                            }
                            TwitchEvent::SendMessage(text) => {
                                debug!(text_len = text.chars().count(), "SendMessage event received");
                                if let Some(client) = &twitch_client {
                                    let send_result = client
                                        .send_message(&text)
                                        .await
                                        .map_err(|e| e.to_string());
                                    match send_result {
                                        Ok(_) => debug!("Message sent successfully"),
                                        Err(error_message) => {
                                            error!(error = %error_message, "Failed to send message");
                                            if !is_reconnect_retryable(&client.status().await) {
                                                last_status = TwitchConnectionStatus::Error(error_message);
                                                update_status(last_status.clone());
                                            }
                                        }
                                    }
                                } else {
                                    debug!("Cannot send message - no active client");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Event channel error");
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bound_reconnect_jitter_ms, is_reconnect_retryable, reconnect_base_delay_ms};
    use crate::twitch::TwitchStatus;

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
}
