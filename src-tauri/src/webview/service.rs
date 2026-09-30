use crate::events::AppEvent;
use crate::webview::upnp::UpnpManager;
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

/// Результат переключения UPnP: настройка — пожелание, mapping — факт.
///
/// UI показывает успех только когда проброс действительно подтверждён; причина
/// отказа приходит кодом и локализуется на стороне frontend, потому что сообщения
/// `igd` пользователю не показываются.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UpnpToggleOutcome {
    /// Mapping приведён к запрошенному состоянию.
    Applied,
    /// Сервер не запущен: сохранено пожелание, проброс применится при старте.
    PreferenceOnly,
    /// Пожелание сохранено, но mapping не подтверждён.
    ForwardFailed { code: String },
}

pub struct WebViewService {
    pub settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    pub event_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>>>,
    status: Arc<Mutex<WebViewServerStatus>>,
    /// Активный UPnP-владелец: появляется вместе с запущенным сервером и
    /// очищается на его остановке.
    upnp_manager: Arc<Mutex<Option<Arc<UpnpManager>>>>,
}

impl WebViewService {
    pub fn new() -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(WebViewSettings::default())),
            event_sender: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(WebViewServerStatus::Stopped)),
            upnp_manager: Arc::new(Mutex::new(None)),
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

    /// Опубликовать/снять владельца UPnP. Сервер вызывает это при старте и
    /// остановке, поэтому `None` означает «mapping сейчас не существует».
    pub fn set_upnp_manager(&self, manager: Option<Arc<UpnpManager>>) {
        *self.upnp_manager.lock() = manager;
    }

    pub fn upnp_manager(&self) -> Option<Arc<UpnpManager>> {
        self.upnp_manager.lock().clone()
    }

    /// Применить переключение UPnP к живому владельцу.
    ///
    /// Настройка — пожелание, mapping — факт: без владельца (сервер не запущен)
    /// возвращается `PreferenceOnly`, а не «порт открыт». Ошибка router'а
    /// возвращается кодом, настройка при этом остаётся сохранённой, чтобы
    /// следующая попытка случилась при старте сервера.
    pub async fn apply_upnp_toggle(&self, enabled: bool) -> UpnpToggleOutcome {
        let Some(manager) = self.upnp_manager() else {
            return UpnpToggleOutcome::PreferenceOnly;
        };

        // Снимать нечего: mapping не подтверждён и включение не запрашивалось.
        if !enabled && !manager.is_mapping_open() && !manager.is_desired() {
            return UpnpToggleOutcome::Applied;
        }

        match manager.set_enabled(enabled).await {
            Ok(()) => UpnpToggleOutcome::Applied,
            Err(error) => {
                warn!(error = %error, enabled, "UPnP toggle failed");
                UpnpToggleOutcome::ForwardFailed {
                    code: error.failure.code().to_string(),
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webview::upnp::test_support::{FakeRouterPortMapper, RouterCall};
    use crate::webview::upnp::{RouterPortMapper, UpnpError, UpnpFailure};

    struct RejectingMapper;

    impl RouterPortMapper for RejectingMapper {
        fn open(&self, _port: u16) -> Result<(), UpnpError> {
            Err(UpnpError::new(
                UpnpFailure::RouterRejected,
                "router refused",
            ))
        }

        fn close(&self, _port: u16) -> Result<(), UpnpError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn toggle_without_a_live_owner_reports_preference_only() {
        let service = WebViewService::new();

        assert_eq!(
            service.apply_upnp_toggle(true).await,
            UpnpToggleOutcome::PreferenceOnly
        );
        assert_eq!(
            service.apply_upnp_toggle(false).await,
            UpnpToggleOutcome::PreferenceOnly
        );
    }

    #[tokio::test]
    async fn toggle_with_a_live_owner_confirms_the_mapping() {
        let mapper = Arc::new(FakeRouterPortMapper::new());
        let manager = Arc::new(UpnpManager::with_mapper(10100, mapper.clone()));
        let service = WebViewService::new();
        service.set_upnp_manager(Some(Arc::clone(&manager)));

        assert_eq!(
            service.apply_upnp_toggle(true).await,
            UpnpToggleOutcome::Applied
        );
        assert!(manager.is_mapping_open());
        assert_eq!(mapper.calls(), vec![RouterCall::Open(10100)]);

        assert_eq!(
            service.apply_upnp_toggle(false).await,
            UpnpToggleOutcome::Applied
        );
        assert!(!manager.is_mapping_open());
        assert_eq!(
            mapper.calls(),
            vec![RouterCall::Open(10100), RouterCall::Close(10100)]
        );
    }

    #[tokio::test]
    async fn disabling_without_a_mapping_does_not_touch_the_router() {
        let mapper = Arc::new(FakeRouterPortMapper::new());
        let manager = Arc::new(UpnpManager::with_mapper(10100, mapper.clone()));
        let service = WebViewService::new();
        service.set_upnp_manager(Some(manager));

        assert_eq!(
            service.apply_upnp_toggle(false).await,
            UpnpToggleOutcome::Applied
        );
        assert!(
            mapper.calls().is_empty(),
            "нет mapping — нет router-вызова: {:?}",
            mapper.calls()
        );
    }

    #[tokio::test]
    async fn router_failure_is_reported_as_a_localizable_code() {
        let manager = Arc::new(UpnpManager::with_mapper(10100, Arc::new(RejectingMapper)));
        let service = WebViewService::new();
        service.set_upnp_manager(Some(Arc::clone(&manager)));

        assert_eq!(
            service.apply_upnp_toggle(true).await,
            UpnpToggleOutcome::ForwardFailed {
                code: "webview.upnp.router_rejected".to_string()
            }
        );
        assert!(!manager.is_mapping_open());
    }

    #[test]
    fn without_a_live_owner_the_service_reports_no_manager() {
        let service = WebViewService::new();
        assert!(service.upnp_manager().is_none());

        let manager = Arc::new(UpnpManager::new(10100));
        service.set_upnp_manager(Some(Arc::clone(&manager)));
        assert!(service.upnp_manager().is_some());

        service.set_upnp_manager(None);
        assert!(service.upnp_manager().is_none());
    }
}
