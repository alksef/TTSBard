use std::sync::Arc;

use parking_lot::Mutex;

use crate::config::TwitchSettings;
use crate::events::{TwitchConnectionStatus, TwitchEvent, TwitchEventSender};

pub struct TwitchService {
    pub settings: Arc<tokio::sync::RwLock<TwitchSettings>>,
    pub connection_status: Arc<Mutex<TwitchConnectionStatus>>,
    pub event_tx: TwitchEventSender,
}

impl TwitchService {
    pub fn new(event_tx: TwitchEventSender) -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(TwitchSettings::default())),
            connection_status: Arc::new(Mutex::new(TwitchConnectionStatus::Disconnected)),
            event_tx,
        }
    }

    pub fn send_event(&self, event: TwitchEvent) {
        let _ = self.event_tx.send(event);
    }
}
