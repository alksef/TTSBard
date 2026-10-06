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
    /// Сервер не смог стартовать.
    ///
    /// `attended = true`, когда попытку запуска инициировал пользователь
    /// (иконка заголовка не красная); `false` — boot-попытка или падение в
    /// рантайме (красная навсегда).
    Error { message: String, attended: bool },
}

/// Stable, frontend-parseable prefix for an occupied-port startup failure.
///
/// The failed port follows the colon; the frontend parses this prefix and
/// shows a localized message with the captured port. The OS error text is
/// logged by the bind site and never reaches the frontend.
pub const PORT_IN_USE_PREFIX: &str = "port_in_use:";

/// Stable startup-failure code for every failure other than an occupied port.
pub const START_FAILED_CODE: &str = "server_start_failed";

/// Build the encoded occupied-port message carried by the `Error` status.
pub fn port_in_use_message(port: u16) -> String {
    format!("{PORT_IN_USE_PREFIX}{port}")
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

/// Фактический runtime-статус UPnP-проброса: mapping — факт, а не пожелание.
///
/// Не путается с `WebViewSettings.upnp_enabled` (пожелание): `Open` появляется
/// только когда router подтвердил mapping, `Failed` — когда последняя попытка
/// открытия провалилась, `Closed` — во всех остальных случаях (сервер не
/// запущен, проброс выключен или попытка ещё не завершилась).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpnpForwardStatus {
    /// Router подтвердил mapping.
    Open,
    /// Mapping отсутствует.
    Closed,
    /// Последняя попытка открыть mapping провалилась.
    Failed { code: String },
}

pub struct WebViewService {
    pub settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    pub event_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>>>,
    status: Arc<Mutex<WebViewServerStatus>>,
    /// Активный UPnP-владелец: появляется вместе с запущенным сервером и
    /// очищается на его остановке.
    upnp_manager: Arc<Mutex<Option<Arc<UpnpManager>>>>,
    /// Persistent-код отказа последней фоновой попытки UPnP. Сбрасывается на
    /// успехе, остановке сервера и явном переключении.
    upnp_failure: Arc<Mutex<Option<String>>>,
    /// Последний опубликованный статус: повторный emit без изменения не нужен.
    last_upnp_status: Arc<Mutex<UpnpForwardStatus>>,
}

impl WebViewService {
    pub fn new() -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(WebViewSettings::default())),
            event_sender: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(WebViewServerStatus::Stopped)),
            upnp_manager: Arc::new(Mutex::new(None)),
            upnp_failure: Arc::new(Mutex::new(None)),
            last_upnp_status: Arc::new(Mutex::new(UpnpForwardStatus::Closed)),
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

    /// Фактический runtime-статус проброса, выведенный из владельца и
    /// persistent-отказа. Чистая операция: ничего не публикует.
    pub fn upnp_forward_status(&self) -> UpnpForwardStatus {
        match self.upnp_manager() {
            Some(manager) if manager.is_mapping_open() => UpnpForwardStatus::Open,
            Some(_) => match self.upnp_failure.lock().clone() {
                Some(code) => UpnpForwardStatus::Failed { code },
                None => UpnpForwardStatus::Closed,
            },
            None => UpnpForwardStatus::Closed,
        }
    }

    /// Установить persistent-код отказа последней фоновой попытки. `None`
    /// снимает отказ. Не публикует событие: публикацию выполняет caller.
    pub fn set_upnp_failure(&self, code: Option<String>) {
        *self.upnp_failure.lock() = code;
    }

    /// Опубликовать фактический статус проброса, если он изменился с прошлой
    /// публикации. Возвращает текущий статус.
    pub fn publish_upnp_forward_status(&self, app_handle: &tauri::AppHandle) -> UpnpForwardStatus {
        let status = self.upnp_forward_status();
        {
            let mut last = self.last_upnp_status.lock();
            if *last == status {
                return status;
            }
            *last = status.clone();
        }
        if let Err(error) = app_handle.emit("webview-upnp-status-changed", status.clone()) {
            warn!(%error, "Failed to emit WebView UPnP status");
        }
        status
    }

    /// Публикует `Open`, если mapping только что подтвердился.
    ///
    /// Успешная фоновая попытка старта не имеет отдельного сигнала от сервера,
    /// поэтому сверка периодически публикует подтверждённый mapping. Намеренно
    /// не публикует `Failed`: отказ явного переключения не должен давать
    /// глобальное уведомление (его показывает панель по результату команды).
    pub fn publish_upnp_open_if_confirmed(&self, app_handle: &tauri::AppHandle) {
        if self.upnp_forward_status() != UpnpForwardStatus::Open {
            return;
        }
        self.publish_upnp_forward_status(app_handle);
    }

    /// Применить результат явного переключения к persistent-отказу.
    ///
    /// Только `ForwardFailed` оставляет persistent-код отказа; `Applied` и
    /// `PreferenceOnly` снимают его, потому что mapping либо подтверждён, либо
    /// отсутствует вместе с сервером. Событие не публикуется: результат команды
    /// возвращается caller'у, а панель обновляет свой runtime-статус локально.
    pub fn record_upnp_outcome(&self, outcome: &UpnpToggleOutcome) {
        self.set_upnp_failure(match outcome {
            UpnpToggleOutcome::ForwardFailed { code } => Some(code.clone()),
            _ => None,
        });
    }

    /// Снять владельца UPnP и persistent-отказ, затем опубликовать `Closed`.
    ///
    /// Используется при остановке сервера: mapping больше не существует, и
    /// отказ, относившийся к предыдущему запуску, не должен пережить его.
    pub fn clear_upnp_runtime(&self, app_handle: &tauri::AppHandle) {
        self.set_upnp_manager(None);
        self.set_upnp_failure(None);
        self.publish_upnp_forward_status(app_handle);
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

    #[test]
    fn upnp_forward_status_defaults_to_closed() {
        let service = WebViewService::new();
        assert_eq!(service.upnp_forward_status(), UpnpForwardStatus::Closed);
    }

    #[test]
    fn no_live_owner_means_closed_regardless_of_a_stale_failure() {
        let service = WebViewService::new();
        service.set_upnp_failure(Some("webview.upnp.timeout".to_string()));
        assert_eq!(service.upnp_forward_status(), UpnpForwardStatus::Closed);
    }

    #[test]
    fn a_persistent_failure_is_reported_until_cleared() {
        let manager = Arc::new(UpnpManager::new(10100));
        let service = WebViewService::new();
        service.set_upnp_manager(Some(manager));

        assert_eq!(service.upnp_forward_status(), UpnpForwardStatus::Closed);

        service.set_upnp_failure(Some("webview.upnp.router_rejected".to_string()));
        assert_eq!(
            service.upnp_forward_status(),
            UpnpForwardStatus::Failed {
                code: "webview.upnp.router_rejected".to_string()
            }
        );

        service.set_upnp_failure(None);
        assert_eq!(service.upnp_forward_status(), UpnpForwardStatus::Closed);
    }

    #[tokio::test]
    async fn a_confirmed_mapping_reports_open() {
        let mapper = Arc::new(FakeRouterPortMapper::new());
        let manager = Arc::new(UpnpManager::with_mapper(10100, mapper));
        let service = WebViewService::new();
        service.set_upnp_manager(Some(Arc::clone(&manager)));

        service.apply_upnp_toggle(true).await;
        assert_eq!(service.upnp_forward_status(), UpnpForwardStatus::Open);
    }

    #[tokio::test]
    async fn a_confirmed_mapping_wins_over_a_stale_failure() {
        let mapper = Arc::new(FakeRouterPortMapper::new());
        let manager = Arc::new(UpnpManager::with_mapper(10100, mapper));
        let service = WebViewService::new();
        service.set_upnp_manager(Some(Arc::clone(&manager)));

        service.apply_upnp_toggle(true).await;
        service.set_upnp_failure(Some("webview.upnp.router_rejected".to_string()));

        assert_eq!(service.upnp_forward_status(), UpnpForwardStatus::Open);
    }

    #[tokio::test]
    async fn a_failed_toggle_is_reported_as_a_persistent_failure() {
        let manager = Arc::new(UpnpManager::with_mapper(10100, Arc::new(RejectingMapper)));
        let service = WebViewService::new();
        service.set_upnp_manager(Some(Arc::clone(&manager)));

        let outcome = service.apply_upnp_toggle(true).await;
        assert!(matches!(
            outcome,
            UpnpToggleOutcome::ForwardFailed { ref code } if code == "webview.upnp.router_rejected"
        ));

        // Сопоставление outcome с runtime-статусом — чистый вывод, проверяется
        // без AppHandle: reject даёт persistent failure.
        let code = match outcome {
            UpnpToggleOutcome::ForwardFailed { code } => code,
            _ => panic!("expected forward failure"),
        };
        service.set_upnp_failure(Some(code));
        assert_eq!(
            service.upnp_forward_status(),
            UpnpForwardStatus::Failed {
                code: "webview.upnp.router_rejected".to_string()
            }
        );
    }

    #[test]
    fn port_in_use_message_encodes_the_captured_port() {
        assert_eq!(port_in_use_message(10100), "port_in_use:10100");
        assert_eq!(port_in_use_message(65535), "port_in_use:65535");
    }

    #[test]
    fn start_failed_code_is_stable_and_free_of_technical_text() {
        assert_eq!(START_FAILED_CODE, "server_start_failed");
    }
}
