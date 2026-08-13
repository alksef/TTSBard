use crate::events::AppEvent;
use crate::webview::WebViewSettings;
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::Arc;
use tauri::Emitter;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum WebViewServerStatus {
    Stopped,
    Starting,
    Running,
    Error { message: String },
}

pub struct WebViewService {
    pub settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    pub event_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>>>,
    status: Arc<Mutex<WebViewServerStatus>>,
}

impl WebViewService {
    pub fn new() -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(WebViewSettings::default())),
            event_sender: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(WebViewServerStatus::Stopped)),
        }
    }

    pub fn set_event_sender(&self, sender: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        info!("Storing WebView event sender");
        *self.event_sender.lock() = Some(sender);
    }

    pub fn send_event(&self, event: AppEvent) {
        if let Some(ref sender) = *self.event_sender.lock() {
            debug!(event = ?event, "Sending event to WebView");
            let _ = sender.send(event);
        } else {
            warn!("WebView event sender not set");
        }
    }

    pub fn status(&self) -> WebViewServerStatus {
        self.status.lock().clone()
    }

    pub fn set_status(&self, app_handle: &tauri::AppHandle, status: WebViewServerStatus) {
        if *self.status.lock() == status {
            return;
        }
        *self.status.lock() = status.clone();
        if let Err(error) = app_handle.emit("webview-server-status-changed", status) {
            warn!(%error, "Failed to emit WebView server status");
        }
    }
}
