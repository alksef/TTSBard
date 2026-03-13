// Twitch server module
//
// This module manages the Twitch client connection.
// Refactored from lib.rs Twitch client thread (2026-03-11)

use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info};
use crate::events::{TwitchEvent, TwitchConnectionStatus};
use crate::state::AppState;
use crate::twitch::TwitchClient;
use tokio::sync::broadcast::Receiver;

/// Run Twitch client in async context
pub async fn run_twitch_client(
    app_state: AppState,
    app_handle: AppHandle,
    mut twitch_rx: Receiver<TwitchEvent>,
) {
    let mut twitch_client: Option<TwitchClient> = None;
    let mut last_status = TwitchConnectionStatus::Disconnected;
    let mut status_check_interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

    let update_status = |status: TwitchConnectionStatus| {
        *app_state.twitch_connection_status.lock() = status.clone();
        let _ = app_handle.emit("twitch-status-changed", &status);
    };

    loop {
        tokio::select! {
            _ = status_check_interval.tick() => {
                if let Some(client) = &twitch_client {
                    let twitch_status = client.status().await;
                    let new_status = match &twitch_status {
                        crate::twitch::TwitchStatus::Connected => {
                            TwitchConnectionStatus::Connected
                        }
                        crate::twitch::TwitchStatus::Connecting => {
                            TwitchConnectionStatus::Connecting
                        }
                        crate::twitch::TwitchStatus::Disconnected => {
                            TwitchConnectionStatus::Disconnected
                        }
                        crate::twitch::TwitchStatus::Error(e) => {
                            TwitchConnectionStatus::Error(e.clone())
                        }
                    };

                    if last_status != new_status {
                        last_status = new_status.clone();
                        update_status(new_status.clone());
                    }
                } else if last_status != TwitchConnectionStatus::Disconnected {
                    last_status = TwitchConnectionStatus::Disconnected;
                    update_status(last_status.clone());
                }
            }
            event = twitch_rx.recv() => {
                match event {
                    Ok(event) => {
                        match event {
                            TwitchEvent::Restart => {
                                info!("Restart event received");

                                let settings = app_state.twitch_settings.read().await;
                                let is_enabled = settings.enabled;
                                let is_valid = settings.is_valid().is_ok();
                                let settings_clone = settings.clone();
                                drop(settings);

                                if let Some(client) = twitch_client.take() {
                                    info!("Stopping previous client...");
                                    client.stop().await;
                                }

                                last_status = TwitchConnectionStatus::Disconnected;
                                update_status(last_status.clone());

                                if is_enabled {
                                    if is_valid {
                                        info!("Settings valid, creating new client");
                                        last_status = TwitchConnectionStatus::Connecting;
                                        update_status(last_status.clone());

                                        let client = TwitchClient::new(settings_clone.into());
                                        match client.start().await {
                                            Ok(_) => {
                                                info!("Client started, waiting for connection");
                                                twitch_client = Some(client);
                                            }
                                            Err(e) => {
                                                error!(error = %e, "Failed to start client");
                                                last_status = TwitchConnectionStatus::Error(e.to_string());
                                                update_status(last_status.clone());
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
                                if let Some(client) = twitch_client.take() {
                                    client.stop().await;
                                }
                                last_status = TwitchConnectionStatus::Disconnected;
                                update_status(last_status.clone());
                            }
                            TwitchEvent::SendMessage(text) => {
                                debug!(message = %text, "SendMessage event received");
                                if let Some(client) = &twitch_client {
                                    match client.send_message(&text).await {
                                        Ok(_) => debug!("Message sent successfully"),
                                        Err(e) => {
                                            error!(error = %e, "Failed to send message");
                                            last_status = TwitchConnectionStatus::Error(e.to_string());
                                            update_status(last_status.clone());
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
