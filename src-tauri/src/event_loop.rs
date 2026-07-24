// Event loop module - Event handling
//
// This module handles application events and routes them to appropriate handlers.
// Refactored from lib.rs handle_event() function (2026-03-11)

use crate::commands::speech_queue::SpeechQueueState;
use crate::events::{AppEvent, InputLayout, TwitchEvent};
use crate::soundpanel_window::update_soundpanel_appearance;
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, error, info};
use uuid::Uuid;

/// Synchronous routing helper for processed TTS text.
/// Uses the snapshot's captured skip flags, not the mutable global prefix flags.
pub(crate) fn route_processed_text(
    app_state: &AppState,
    text: &str,
    skip_twitch: bool,
    skip_webview: bool,
) {
    if !skip_webview {
        app_state
            .webview
            .send_event(AppEvent::TextSentToTts(text.to_string()));
    }
    if !skip_twitch {
        let settings = app_state.twitch.settings.blocking_read();
        if settings.enabled {
            drop(settings);
            app_state.send_twitch_event(TwitchEvent::SendMessage(text.to_string()));
        }
    }
}

/// Update tray icon based on interception state
fn update_tray_icon(_app_handle: &AppHandle, is_intercepting: bool) {
    debug!(
        is_intercepting,
        "[TRAY] Interception mode: tray icon update skipped (not implemented)"
    );
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
            AppEvent::ShowMainWindow => {
                self.process_show_main_window();
            }
            AppEvent::UpdateTrayIcon(is_intercepting) => {
                self.process_update_tray_icon(is_intercepting);
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
            AppEvent::WebViewTypingChanged(typing) => {
                debug!(
                    typing,
                    "[EVENT] WebView typing changed (service-owned, forwarded to WebView loop)"
                );
            }
            AppEvent::TwitchStatusChanged(status) => {
                debug!(?status, "[EVENT] Twitch status changed");
            }
            AppEvent::PlaybackStarted {
                ref text_id,
                ref text,
            } => {
                debug!(text_id = %text_id, "[EVENT] Playback started");
                self.process_playback_started(text_id, text);
            }
            AppEvent::PlaybackFinished { ref text_id } => {
                debug!(text_id = %text_id, "[EVENT] Playback finished");
                self.process_playback_finished(text_id);
            }
            AppEvent::PlaybackFailed {
                ref text_id,
                ref error,
            } => {
                debug!(text_id = %text_id, error = %error, "[EVENT] Playback failed");
                self.process_playback_failed(text_id, error);
            }
            AppEvent::PlaybackPaused => {
                debug!("[EVENT] Playback paused");
            }
            AppEvent::PlaybackResumed => {
                debug!("[EVENT] Playback resumed");
            }
            AppEvent::PlaybackStopped => {
                debug!("[EVENT] Playback stopped");
            }
            AppEvent::ShowPlaybackControlWindow => {
                debug!("[EVENT] ShowPlaybackControlWindow event received");
            }
            AppEvent::HidePlaybackControlWindow => {
                debug!("[EVENT] HidePlaybackControlWindow event received");
            }
            AppEvent::QueueChanged => {
                debug!("[EVENT] Queue changed");
            }
            AppEvent::Quit => {
                info!("[EVENT] Quit event received - WebView server should handle cleanup");
            }
        }
    }

    /// Process PlaybackStarted — if text_id is a queue job, transition Ready→Playing.
    fn process_playback_started(&self, text_id: &str, _text: &str) {
        if let Ok(job_id) = Uuid::parse_str(text_id) {
            if let Some(sq) = self.app_handle.try_state::<SpeechQueueState>() {
                let mut q = sq.lock();
                if q.has_job(job_id) {
                    match q.mark_playing(job_id) {
                        Ok(()) => {
                            let dto = q.state();
                            drop(q);
                            let _ = self.app_handle.emit("speech-queue-changed", dto);
                            sq.notify_one();
                        }
                        Err(e) => {
                            debug!(error = %e, job_id = %job_id, "PlaybackStarted: invalid transition for queue job");
                        }
                    }
                }
            }
        }
    }

    /// Process PlaybackFinished — if queue job, transition Playing→Completed, then always advance.
    fn process_playback_finished(&self, text_id: &str) {
        if let Ok(job_id) = Uuid::parse_str(text_id) {
            if let Some(sq) = self.app_handle.try_state::<SpeechQueueState>() {
                let mut q = sq.lock();
                if q.has_job(job_id) {
                    match q.mark_completed(job_id) {
                        Ok(()) => {
                            let dto = q.state();
                            drop(q);
                            let _ = self.app_handle.emit("speech-queue-changed", dto);
                        }
                        Err(e) => {
                            debug!(error = %e, job_id = %job_id, "PlaybackFinished: invalid transition for queue job");
                        }
                    }
                }
            }
        }
        // Always advance playback (legacy behavior)
        if let Some(pb) = self.state.playback_manager.lock().as_ref() {
            pb.on_playback_finished();
        }
    }

    /// Process PlaybackFailed — if queue job, transition Ready/Playing→Failed, fail-closed.
    fn process_playback_failed(&self, text_id: &str, error_msg: &str) {
        if let Ok(job_id) = Uuid::parse_str(text_id) {
            if let Some(sq) = self.app_handle.try_state::<SpeechQueueState>() {
                let mut q = sq.lock();
                if q.has_job(job_id) {
                    match q.fail_playback(job_id, error_msg.to_string()) {
                        Ok(()) => {
                            let dto = q.state();
                            drop(q);
                            let _ = self.app_handle.emit("speech-queue-changed", dto);
                            sq.notify_one();
                        }
                        Err(e) => {
                            debug!(error = %e, job_id = %job_id, "PlaybackFailed: invalid transition for queue job");
                        }
                    }
                }
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
            self.state
                .webview
                .send_event(AppEvent::TextSentToTts(text.clone()));
        } else {
            debug!("[EVENT] WebView skipped (prefix)");
        }

        // === Twitch send (check flag) ===
        if !skip_twitch {
            let settings = self.state.twitch.settings.blocking_read();
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

    /// Process show main window event
    fn process_show_main_window(&self) {
        debug!("Show main window");
        if let Some(window) = self.app_handle.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
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
