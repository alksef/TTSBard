use std::sync::Arc;
use parking_lot::Mutex;
use crate::webview::WebViewSettings;
use crate::events::AppEvent;
use tracing::{info, debug, warn};

pub struct WebViewService {
    pub settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    pub event_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>>>,
}

impl WebViewService {
    pub fn new() -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(WebViewSettings::default())),
            event_sender: Arc::new(Mutex::new(None)),
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
}
