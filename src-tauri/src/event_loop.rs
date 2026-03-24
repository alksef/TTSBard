// Event loop module - Event handling
//
// This module handles application events and routes them to appropriate handlers.
// Refactored from lib.rs handle_event() function (2026-03-11)

use crate::events::{AppEvent, InputLayout, TwitchEvent};
use crate::state::AppState;
use crate::floating::{show_floating_window, hide_floating_window, update_floating_text, update_floating_title, update_soundpanel_appearance};
use tauri::{AppHandle, Manager};
use tracing::{debug, error, info};

/// Update tray icon based on interception state
fn update_tray_icon(_app_handle: &AppHandle, is_intercepting: bool) {
    debug!(is_intercepting, "[TRAY] Interception mode: tray icon update skipped (not implemented)");
    // TODO: Implement tray icon update with proper resource paths
}

/// Event handler for application events
pub struct EventHandler {
    state: AppState,
    app_handle: AppHandle,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(state: AppState, app_handle: AppHandle) -> Self {
        Self { state, app_handle }
    }

    /// Handle an application event
    pub fn handle(&self, event: AppEvent) {
        debug!(event = ?std::mem::discriminant(&event), "[HANDLE_EVENT] Received event");
        match event {
            AppEvent::InterceptionChanged(enabled) => {
                self.process_interception_changed(enabled);
            }
            AppEvent::LayoutChanged(layout) => {
                self.process_layout_changed(layout);
            }
            AppEvent::TextReady(text) => {
                self.process_text_ready(text);
            }
            AppEvent::TextSentToTts(text) => {
                self.process_text_sent_to_tts(text);
            }
            AppEvent::TtsStatusChanged(status) => {
                debug!(?status, "TTS status changed");
            }
            AppEvent::TtsError(err) => {
                error!(error = %err, "TTS error");
            }
            AppEvent::ShowFloatingWindow => {
                self.process_show_floating_window();
            }
            AppEvent::HideFloatingWindow => {
                self.process_hide_floating_window();
            }
            AppEvent::ShowMainWindow => {
                self.process_show_main_window();
            }
            AppEvent::UpdateFloatingText(text) => {
                self.process_update_floating_text(text);
            }
            AppEvent::UpdateTrayIcon(is_intercepting) => {
                self.process_update_tray_icon(is_intercepting);
            }
            AppEvent::FloatingAppearanceChanged => {
                debug!("Floating window appearance changed");
            }
            AppEvent::ClickthroughChanged(enabled) => {
                self.process_clickthrough_changed(enabled);
            }
            AppEvent::ShowSoundPanelWindow => {
                debug!("[EVENT] ShowSoundPanelWindow event received");
                // Handled by SoundPanel event system
            }
            AppEvent::HideSoundPanelWindow => {
                debug!("[EVENT] HideSoundPanelWindow event received");
                // Handled by SoundPanel event system
            }
            AppEvent::SoundPanelNoBinding(key) => {
                debug!(key = %key, "[EVENT] SoundPanelNoBinding");
                // Handled by SoundPanel event system
            }
            AppEvent::SoundPanelAppearanceChanged => {
                debug!("[EVENT MAIN] === SoundPanelAppearanceChanged event received ===");
                let _ = update_soundpanel_appearance(&self.app_handle);
            }
            AppEvent::TtsProviderChanged(provider) => {
                debug!(?provider, "[EVENT] TTS provider changed");
            }
            AppEvent::EnterClosesDisabled(disabled) => {
                debug!(disabled, "[EVENT] Enter closes disabled");
            }
            AppEvent::WebViewServerError(error) => {
                error!(error = %error, "[EVENT] WebView server error");
            }
            AppEvent::RestartWebViewServer => {
                debug!("[EVENT] Restart WebView server requested");
            }
            AppEvent::ReloadWebViewTemplates => {
                debug!("[EVENT] Reload WebView templates requested");
            }
            AppEvent::ToggleUpnp(enabled) => {
                debug!(enabled, "[EVENT] Toggle UPnP requested");
            }
            AppEvent::TwitchStatusChanged(status) => {
                debug!(?status, "[EVENT] Twitch status changed");
            }
            AppEvent::Quit => {
                info!("[EVENT] Quit event received - WebView server should handle cleanup");
            }
        }
    }

    /// Process interception changed event
    fn process_interception_changed(&self, enabled: bool) {
        info!(enabled, "Interception changed");
        if enabled {
            info!("Text interception mode enabled - type to capture text");
            info!("Press F8 to switch layout (EN/RU)");
            info!("Press Enter to send text to TTS");
            info!("Press Escape to cancel");
        }
        update_tray_icon(&self.app_handle, enabled);
    }

    /// Process layout changed event
    fn process_layout_changed(&self, layout: InputLayout) {
        debug!(?layout, "Layout changed");
        let layout_str = match layout {
            InputLayout::English => "EN",
            InputLayout::Russian => "RU",
        };
        let text = self.state.get_current_text();
        let _ = update_floating_title(&self.app_handle, layout_str, &text);
        match layout {
            InputLayout::English => debug!("Current layout: English (EN)"),
            InputLayout::Russian => debug!("Current layout: Russian (RU)"),
        }
    }

    /// Process text ready for TTS event
    fn process_text_ready(&self, text: String) {
        debug!(text = %text, "[EVENT] Text ready for TTS");

        // Используем общий runtime вместо создания нового
        let state = self.state.clone();
        self.state.runtime.spawn(async move {
            match crate::commands::speak_text_internal(&state, text).await {
                Ok(_) => {
                    debug!("[EVENT] TTS started successfully in interception mode");
                }
                Err(e) => {
                    error!(error = %e, "[EVENT] TTS failed in interception mode");
                    state.emit_event(AppEvent::TtsError(e));
                }
            }
        });
    }

    /// Process text sent to TTS event
    fn process_text_sent_to_tts(&self, text: String) {
        debug!(text = %text, "[EVENT] Text sent to TTS");

        let (skip_twitch, skip_webview) = self.state.get_prefix_flags();

        // === WebView broadcast (check flag) ===
        if !skip_webview {
            if let Some(ref sender) = *self.state.webview_event_sender.lock() {
                debug!("[EVENT] Sending to WebView");
                match sender.send(AppEvent::TextSentToTts(text.clone())) {
                    Ok(_) => debug!("[EVENT] TextSentToTts sent to WebView successfully"),
                    Err(e) => error!(error = %e, "[EVENT] Failed to send to WebView"),
                }
            } else {
                debug!("[EVENT] WebView sender is None, not forwarding");
            }
        } else {
            debug!("[EVENT] WebView skipped (prefix)");
        }

        // === Twitch send (check flag) ===
        if !skip_twitch {
            let settings = self.state.twitch_settings.blocking_read();
            if settings.enabled {
                drop(settings);
                self.state.send_twitch_event(TwitchEvent::SendMessage(text));
            }
        } else {
            debug!("[EVENT] Twitch skipped (prefix)");
        }

        // Clear flags after use
        self.state.clear_prefix_flags();
    }

    /// Process show floating window event
    fn process_show_floating_window(&self) {
        debug!("[EVENT] ShowFloatingWindow event received");
        match show_floating_window(&self.app_handle) {
            Ok(_) => debug!("[EVENT] Floating window shown successfully"),
            Err(e) => error!(error = %e, "[EVENT] Failed to show floating window"),
        }
        // Clear text when showing window
        self.state.clear_text();
        // Update UI with empty text and current layout
        let layout = match self.state.get_current_layout() {
            InputLayout::English => "EN",
            InputLayout::Russian => "RU",
        };
        let _ = update_floating_text(&self.app_handle, "");
        let _ = update_floating_title(&self.app_handle, layout, "");
    }

    /// Process hide floating window event
    fn process_hide_floating_window(&self) {
        debug!("Hide floating window");
        let _ = hide_floating_window(&self.app_handle, &self.state);
    }

    /// Process show main window event
    fn process_show_main_window(&self) {
        debug!("Show main window");
        if let Some(window) = self.app_handle.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    /// Process update floating text event
    fn process_update_floating_text(&self, text: String) {
        debug!(text = %text, "Update floating text");
        let _ = update_floating_text(&self.app_handle, &text);
        // Also update title
        let layout = match self.state.get_current_layout() {
            InputLayout::English => "EN",
            InputLayout::Russian => "RU",
        };
        let _ = update_floating_title(&self.app_handle, layout, &text);
    }

    /// Process update tray icon event
    fn process_update_tray_icon(&self, is_intercepting: bool) {
        debug!(is_intercepting, "Update tray icon");
        update_tray_icon(&self.app_handle, is_intercepting);
    }

    /// Process clickthrough changed event
    fn process_clickthrough_changed(&self, enabled: bool) {
        debug!(enabled, "Clickthrough changed");
        if let Some(window) = self.app_handle.get_webview_window("floating") {
            let _ = window.set_ignore_cursor_events(enabled);
        }
    }
}
