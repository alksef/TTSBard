// WebView server module
//
// This module manages the WebView server for broadcasting TTS to web clients.
// Refactored from lib.rs WebView server thread (2026-03-11)

use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, error};
use crate::events::AppEvent;
use crate::webview::WebViewServer;
use crate::webview::WebViewSettings;
use crate::setup::parse_webview_server_error;

/// Run WebView server in async context
/// This function is called from a dedicated thread with tokio runtime
pub async fn run_webview_server(
    webview_settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    app_handle: AppHandle,
    webview_rx: std::sync::mpsc::Receiver<AppEvent>,
) {
    loop {
        // Check current settings
        let settings = webview_settings.read().await;
        let mut enabled = settings.enabled;
        let start_on_boot = settings.start_on_boot;
        let bind_address = settings.bind_address.clone();
        let port = settings.port;
        drop(settings);

        // Auto-start on boot if configured
        if start_on_boot && !enabled {
            info!("[WEBVIEW] Auto-starting server on boot (start_on_boot=true)");
            let mut s = webview_settings.write().await;
            s.enabled = true;
            enabled = true;
            drop(s);
        }

        if enabled {
            info!("[WEBVIEW] ========================================");
            info!("[WEBVIEW] STARTING SERVER");
            info!("[WEBVIEW]   Address: {}:{}", bind_address, port);
            info!("[WEBVIEW] ========================================");

            let server = match WebViewServer::new(Arc::clone(&webview_settings)).await {
                Ok(s) => s,
                Err(e) => {
                    let error_msg = format!("Failed to create server: {}", e);
                    error!("[WEBVIEW] ❌ {}", error_msg);
                    let _ = app_handle.emit("webview-server-error", &error_msg);
                    // Wait a bit before retrying
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            // Spawn server task with improved error handling
            let server_clone = server.clone();
            let app_handle_clone = app_handle.clone();
            let bind_address_clone = bind_address.clone();
            let server_handle = tokio::spawn(async move {
                info!("[WEBVIEW] Server task started, waiting for connections...");

                if let Err(e) = server_clone.start().await {
                    // Extract error details for user-friendly message
                    let error_msg = format!("{}", e);
                    let (user_friendly_msg, log_context) = parse_webview_server_error(&error_msg, bind_address_clone, port);

                    // Log with full context
                    error!("[WEBVIEW] ❌ Server startup failed:");
                    error!("[WEBVIEW]   Context: {}", log_context);
                    error!("[WEBVIEW]   Error: {}", error_msg);

                    // Emit user-friendly error to frontend
                    let _ = app_handle_clone.emit("webview-server-error", &user_friendly_msg);

                    // Also emit via AppEvent system for consistency
                    if let Some(state) = app_handle_clone.try_state::<crate::state::AppState>() {
                        state.emit_event(AppEvent::WebViewServerError(user_friendly_msg));
                    }
                }
                // Server task completed
                info!("[WEBVIEW] Server task stopped");
            });

            // Handle events and broadcast text
            let mut server_running = true;
            while server_running {
                // Check if settings changed
                let current_settings = webview_settings.read().await;
                let still_enabled = current_settings.enabled;
                let same_port = current_settings.port == port && current_settings.bind_address == bind_address;
                drop(current_settings);

                if !still_enabled || !same_port {
                    info!("[WEBVIEW] ========================================");
                    info!("[WEBVIEW] STOPPING SERVER (settings changed)");
                    info!("[WEBVIEW]   Still enabled: {}", still_enabled);
                    info!("[WEBVIEW]   Same port: {}", same_port);
                    info!("[WEBVIEW] ========================================");
                    server_handle.abort();
                    server_running = false;
                } else {
                    // Process events with timeout (synchronous)
                    match webview_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                        Ok(event) => {
                            info!("[WEBVIEW] 📨 Event received: {:?}", std::mem::discriminant(&event));
                            match event {
                                AppEvent::TextSentToTts(text) => {
                                    let preview = text.chars().take(50).collect::<String>();
                                    info!("[WEBVIEW] 📤 Broadcasting to SSE clients: '{}'...", preview);
                                    server.broadcast_text(&text).await;
                                }
                                AppEvent::RestartWebViewServer => {
                                    info!("[WEBVIEW] ⚠ Restart event received, stopping server...");
                                    server_handle.abort();
                                    // Wait a bit for the server to fully shut down
                                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
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
                                _ => {
                                    info!("[WEBVIEW] ℹ️  Ignoring event: {:?}", std::mem::discriminant(&event));
                                }
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            // Timeout - continue loop to check settings
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            // Channel closed
                            info!("[WEBVIEW] Event channel disconnected");
                            return;
                        }
                    }
                }
            }
        } else {
            info!("[WEBVIEW] ========================================");
            info!("[WEBVIEW] SERVER DISABLED");
            info!("[WEBVIEW] Waiting for enable signal...");
            info!("[WEBVIEW] ========================================");
            // Wait for enable or restart event
            loop {
                match webview_rx.recv_timeout(std::time::Duration::from_secs(2)) {
                    Ok(AppEvent::RestartWebViewServer) => {
                        info!("[WEBVIEW] ⚠ Restart event received, exiting disabled state");
                        break;
                    }
                    Ok(AppEvent::TextSentToTts(text)) => {
                        // Ignore TTS events while disabled but log them
                        let preview = text.chars().take(30).collect::<String>();
                        info!("[WEBVIEW] Ignoring TTS text (server disabled): '{}'...", preview);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Timeout - check if enabled now
                        let settings = webview_settings.read().await;
                        if settings.enabled {
                            drop(settings);
                            info!("[WEBVIEW] ✓ Enabled detected via timeout!");
                            break;
                        }
                        drop(settings);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        info!("[WEBVIEW] Event channel disconnected");
                        return;
                    }
                    Ok(other) => {
                        info!("[WEBVIEW] Received unexpected event while disabled: {:?}", other);
                    }
                }
            }
        }
    }
}
