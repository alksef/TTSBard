use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use super::messages::{
    self, HotkeyInfo, HotkeysInCurrentModelResponseData, VtsRequest, VtsResponse,
};
use crate::config::{VTubeStudioSettings, VTubeStudioTypingMode};
use crate::events::VTubeStudioConnectionStatus;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const TYPING_KEEPALIVE_MS: u64 = 500;

type WsStream = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

struct InnerState {
    ws: Option<WsStream>,
    typing_cancel: Option<CancellationToken>,
    typing_handle: Option<tokio::task::JoinHandle<()>>,
    typing_active: bool,
}

pub struct VTubeStudioService {
    pub settings: Arc<tokio::sync::RwLock<VTubeStudioSettings>>,
    inner: Arc<tokio::sync::Mutex<InnerState>>,
    is_authenticated: Arc<AtomicBool>,
    desired_running: Arc<AtomicBool>,
    connection_status: Arc<parking_lot::Mutex<VTubeStudioConnectionStatus>>,
}

impl VTubeStudioService {
    pub fn new() -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(VTubeStudioSettings::default())),
            inner: Arc::new(tokio::sync::Mutex::new(InnerState {
                ws: None,
                typing_cancel: None,
                typing_handle: None,
                typing_active: false,
            })),
            is_authenticated: Arc::new(AtomicBool::new(false)),
            desired_running: Arc::new(AtomicBool::new(false)),
            connection_status: Arc::new(parking_lot::Mutex::new(
                VTubeStudioConnectionStatus::Disconnected,
            )),
        }
    }

    pub fn set_connection_status(&self, status: VTubeStudioConnectionStatus) {
        *self.connection_status.lock() = status;
    }

    pub fn get_connection_status(&self) -> VTubeStudioConnectionStatus {
        self.connection_status.lock().clone()
    }

    pub fn set_desired_running(&self, value: bool) {
        self.desired_running.store(value, Ordering::SeqCst);
        info!(value, "VTube Studio desired_running set");
    }

    pub fn is_desired_running(&self) -> bool {
        self.desired_running.load(Ordering::SeqCst)
    }

    #[allow(dead_code)]
    pub fn mark_authenticated(&self, value: bool) {
        self.is_authenticated.store(value, Ordering::SeqCst);
    }

    fn next_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }

    #[allow(dead_code)]
    pub async fn is_connected(&self) -> bool {
        self.inner.lock().await.ws.is_some()
    }

    pub async fn test_connection(
        &self,
        port: u16,
        stored_token: Option<&str>,
    ) -> Result<Option<String>, String> {
        let mut inner = self.inner.lock().await;

        self.stop_typing_keepalive_locked(&mut inner);
        inner.typing_active = false;
        inner.ws = None;

        let mut ws = connect_ws(port).await?;
        let new_token = perform_authentication(&mut ws, self.next_id(), stored_token).await?;

        if self.is_desired_running() {
            inner.ws = Some(ws);
        }
        Ok(new_token)
    }

    pub async fn connect(
        &self,
        port: u16,
        stored_token: Option<&str>,
    ) -> Result<Option<String>, String> {
        self.set_desired_running(true);
        self.set_connection_status(VTubeStudioConnectionStatus::Connecting);

        let mut inner = self.inner.lock().await;
        self.stop_typing_keepalive_locked(&mut inner);
        inner.typing_active = false;
        inner.ws = None;

        let ws_result = connect_ws(port).await;
        let mut ws = match ws_result {
            Ok(ws) => ws,
            Err(e) => {
                self.is_authenticated.store(false, Ordering::SeqCst);
                self.set_desired_running(false);
                self.set_connection_status(VTubeStudioConnectionStatus::Error);
                return Err(e);
            }
        };

        let auth_result = perform_authentication(&mut ws, self.next_id(), stored_token).await;
        let new_token = match auth_result {
            Ok(token) => token,
            Err(e) => {
                self.is_authenticated.store(false, Ordering::SeqCst);
                self.set_desired_running(false);
                self.set_connection_status(VTubeStudioConnectionStatus::Error);
                return Err(e);
            }
        };

        inner.ws = Some(ws);
        self.is_authenticated.store(true, Ordering::SeqCst);
        self.set_connection_status(VTubeStudioConnectionStatus::Connected);
        Ok(new_token)
    }

    pub async fn set_typing(
        &self,
        typing: bool,
        port: u16,
        stored_token: &str,
    ) -> Result<(), String> {
        let typing_action = { self.settings.read().await.typing_action.clone() };

        let mut inner = self.inner.lock().await;

        if !typing {
            self.stop_typing_keepalive_locked(&mut inner);
            inner.typing_active = false;

            if typing_action.output_mode == VTubeStudioTypingMode::Event {
                if let Some(ref mut ws) = inner.ws {
                    let param_name = typing_action.parameter_name.clone();
                    if let Err(e) = inject_typing(ws, self.next_id(), &param_name, 0.0).await {
                        debug!(error = %e, "VTS inject typing=false failed, discarding broken socket");
                        inner.ws = None;
                        self.is_authenticated.store(false, Ordering::SeqCst);
                    }
                }
            } else {
                if let Some(ref mut ws) = inner.ws {
                    let stop_id = typing_action.stop_hotkey_id.clone();
                    if let Err(e) = trigger_hotkey(ws, self.next_id(), &stop_id).await {
                        debug!(error = %e, "VTS hotkey stop trigger failed, discarding broken socket");
                        inner.ws = None;
                        self.is_authenticated.store(false, Ordering::SeqCst);
                    }
                }
            }
            return Ok(());
        }

        if stored_token.is_empty() {
            return Ok(());
        }

        if !self.is_desired_running() {
            debug!("VTS: typing=true ignored — desired_running is false");
            return Ok(());
        }

        if inner.ws.is_none() {
            self.set_connection_status(VTubeStudioConnectionStatus::Connecting);

            let mut ws = match connect_ws(port).await {
                Ok(ws) => ws,
                Err(e) => {
                    debug!(error = %e, "VTS connect for typing=true failed");
                    self.set_connection_status(VTubeStudioConnectionStatus::Error);
                    return Err(e);
                }
            };

            match perform_authentication(&mut ws, self.next_id(), Some(stored_token)).await {
                Ok(_) => {}
                Err(e) => {
                    debug!(error = %e, "VTS auth for typing=true failed, discarding broken socket");
                    self.is_authenticated.store(false, Ordering::SeqCst);
                    self.set_connection_status(VTubeStudioConnectionStatus::Error);
                    return Err(e);
                }
            }

            if typing_action.output_mode == VTubeStudioTypingMode::Event {
                let param_name = typing_action.parameter_name.clone();
                if let Err(e) = create_typing_param(&mut ws, self.next_id(), &param_name).await {
                    debug!(error = %e, "VTS create param for typing=true failed, discarding broken socket");
                    self.is_authenticated.store(false, Ordering::SeqCst);
                    self.set_connection_status(VTubeStudioConnectionStatus::Error);
                    return Err(e);
                }
            }

            inner.ws = Some(ws);
            self.set_connection_status(VTubeStudioConnectionStatus::Connected);
        }

        self.stop_typing_keepalive_locked(&mut inner);
        inner.typing_active = true;

        match typing_action.output_mode {
            VTubeStudioTypingMode::Event => {
                let param_name = typing_action.parameter_name.clone();
                let ws = match inner.ws.as_mut() {
                    Some(ws) => ws,
                    None => {
                        inner.typing_active = false;
                        return Ok(());
                    }
                };

                if let Err(e) = inject_typing(ws, self.next_id(), &param_name, 1.0).await {
                    debug!(error = %e, "VTS inject typing=true failed, discarding broken socket");
                    inner.ws = None;
                    inner.typing_active = false;
                    self.is_authenticated.store(false, Ordering::SeqCst);
                    self.set_connection_status(VTubeStudioConnectionStatus::Error);
                    return Err(e);
                }

                let cancel = CancellationToken::new();
                let cancel_ct = cancel.clone();
                inner.typing_cancel = Some(cancel);

                let inner_arc = Arc::clone(&self.inner);
                let auth_flag = Arc::clone(&self.is_authenticated);
                let status_arc = Arc::clone(&self.connection_status);

                let handle = tokio::spawn(async move {
                    loop {
                        if cancel_ct.is_cancelled() {
                            break;
                        }

                        tokio::time::sleep(Duration::from_millis(TYPING_KEEPALIVE_MS)).await;

                        if cancel_ct.is_cancelled() {
                            break;
                        }

                        let mut inner_guard = inner_arc.lock().await;
                        let id = uuid::Uuid::new_v4().to_string();
                        if let Some(ref mut ws) = inner_guard.ws {
                            if let Err(e) = inject_typing(ws, id, &param_name, 1.0).await {
                                debug!(error = %e, "VTS typing keep-alive inject failed, discarding broken socket");
                                inner_guard.ws = None;
                                inner_guard.typing_active = false;
                                auth_flag.store(false, Ordering::SeqCst);
                                *status_arc.lock() = VTubeStudioConnectionStatus::Error;
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    if !cancel_ct.is_cancelled() {
                        auth_flag.store(false, Ordering::SeqCst);
                    }
                    debug!("VTS typing keep-alive stopped");
                });

                inner.typing_handle = Some(handle);
            }
            VTubeStudioTypingMode::Hotkeys => {
                let start_id = typing_action.start_hotkey_id.clone();
                let ws = match inner.ws.as_mut() {
                    Some(ws) => ws,
                    None => {
                        inner.typing_active = false;
                        return Ok(());
                    }
                };

                if let Err(e) = trigger_hotkey(ws, self.next_id(), &start_id).await {
                    debug!(error = %e, "VTS hotkey start trigger failed, discarding broken socket");
                    inner.ws = None;
                    inner.typing_active = false;
                    self.is_authenticated.store(false, Ordering::SeqCst);
                    self.set_connection_status(VTubeStudioConnectionStatus::Error);
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    pub async fn disconnect(&self) {
        let mut inner = self.inner.lock().await;
        self.set_desired_running(false);

        let typing_active = inner.typing_active;
        let typing_action = { self.settings.read().await.typing_action.clone() };

        self.stop_typing_keepalive_locked(&mut inner);
        inner.typing_active = false;

        if typing_active {
            if let Some(ref mut ws) = inner.ws {
                match typing_action.output_mode {
                    VTubeStudioTypingMode::Event => {
                        let _ =
                            inject_typing(ws, self.next_id(), &typing_action.parameter_name, 0.0)
                                .await;
                    }
                    VTubeStudioTypingMode::Hotkeys => {
                        if !typing_action.stop_hotkey_id.is_empty() {
                            let _ =
                                trigger_hotkey(ws, self.next_id(), &typing_action.stop_hotkey_id)
                                    .await;
                        }
                    }
                }
            }
        }

        inner.ws = None;
        self.is_authenticated.store(false, Ordering::SeqCst);
        self.set_connection_status(VTubeStudioConnectionStatus::Disconnected);
        info!("VTube Studio disconnected");
    }

    fn stop_typing_keepalive_locked(&self, inner: &mut InnerState) {
        if let Some(cancel) = inner.typing_cancel.take() {
            cancel.cancel();
        }
        if let Some(handle) = inner.typing_handle.take() {
            handle.abort();
        }
    }

    pub async fn get_current_model_hotkeys(&self) -> Result<Vec<HotkeyInfo>, String> {
        if !self.is_desired_running() {
            return Err("VTube Studio is not running.".to_string());
        }

        let status = self.get_connection_status();
        if status != VTubeStudioConnectionStatus::Connected {
            return Err(format!(
                "VTube Studio is not connected (status: {:?}).",
                status
            ));
        }

        if !self.is_authenticated.load(Ordering::SeqCst) {
            return Err("VTube Studio is not authenticated.".to_string());
        }

        let mut inner = self.inner.lock().await;
        let ws = inner
            .ws
            .as_mut()
            .ok_or_else(|| "VTube Studio WebSocket is not available.".to_string())?;

        let req = VtsRequest::hotkeys_in_current_model_request(&self.next_id());
        let req_id = req.request_id.clone();
        let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;

        let value = send_and_recv(ws, &json, &req_id, "HotkeysInCurrentModelResponse")
            .await
            .map_err(|e| format!("Hotkeys request failed: {}", e))?;

        let data: HotkeysInCurrentModelResponseData =
            serde_json::from_value(value).map_err(|e| format!("Parse hotkeys response: {}", e))?;

        if !data.model_loaded {
            return Err("No model is currently loaded in VTube Studio.".to_string());
        }

        Ok(data.available_hotkeys)
    }

    pub async fn test_typing_action(
        &self,
        timeout_ms: u64,
        repeat_count: u64,
    ) -> Result<String, String> {
        let typing_action = self.settings.read().await.typing_action.clone();

        if timeout_ms < 100 || timeout_ms > 5000 {
            return Err("Таймаут должен быть от 100 до 5000 мс".to_string());
        }
        if repeat_count < 1 || repeat_count > 10 {
            return Err("Повторы должны быть от 1 до 10".to_string());
        }

        if !self.is_desired_running() {
            return Err("VTube Studio is not running. Start the connection first.".to_string());
        }

        let status = self.get_connection_status();
        if status != VTubeStudioConnectionStatus::Connected {
            return Err(format!(
                "VTube Studio is not connected (status: {:?}). Must be Connected.",
                status
            ));
        }

        let mut inner = self.inner.lock().await;

        if inner.ws.is_none() {
            return Err("VTube Studio WebSocket is not available.".to_string());
        }

        self.stop_typing_keepalive_locked(&mut inner);

        let timeout_dur = Duration::from_millis(timeout_ms);

        for i in 0..repeat_count {
            let ws = inner.ws.as_mut().unwrap();

            match typing_action.output_mode {
                VTubeStudioTypingMode::Event => {
                    let param_name = typing_action.parameter_name.clone();
                    if let Err(e) = inject_typing(ws, self.next_id(), &param_name, 1.0).await {
                        inner.ws = None;
                        self.is_authenticated.store(false, Ordering::SeqCst);
                        self.set_connection_status(VTubeStudioConnectionStatus::Error);
                        return Err(format!(
                            "VTube Studio action test failed at repeat {} (start): {}",
                            i + 1,
                            e
                        ));
                    }
                }
                VTubeStudioTypingMode::Hotkeys => {
                    let start_id = typing_action.start_hotkey_id.clone();
                    if let Err(e) = trigger_hotkey(ws, self.next_id(), &start_id).await {
                        inner.ws = None;
                        self.is_authenticated.store(false, Ordering::SeqCst);
                        self.set_connection_status(VTubeStudioConnectionStatus::Error);
                        return Err(format!(
                            "VTube Studio action test failed at repeat {} (start): {}",
                            i + 1,
                            e
                        ));
                    }
                }
            }

            tokio::time::sleep(timeout_dur).await;

            let ws = inner.ws.as_mut().unwrap();

            match typing_action.output_mode {
                VTubeStudioTypingMode::Event => {
                    let param_name = typing_action.parameter_name.clone();
                    if let Err(e) = inject_typing(ws, self.next_id(), &param_name, 0.0).await {
                        inner.ws = None;
                        self.is_authenticated.store(false, Ordering::SeqCst);
                        self.set_connection_status(VTubeStudioConnectionStatus::Error);
                        return Err(format!(
                            "VTube Studio action test failed at repeat {} (stop): {}",
                            i + 1,
                            e
                        ));
                    }
                }
                VTubeStudioTypingMode::Hotkeys => {
                    let stop_id = typing_action.stop_hotkey_id.clone();
                    if let Err(e) = trigger_hotkey(ws, self.next_id(), &stop_id).await {
                        inner.ws = None;
                        self.is_authenticated.store(false, Ordering::SeqCst);
                        self.set_connection_status(VTubeStudioConnectionStatus::Error);
                        return Err(format!(
                            "VTube Studio action test failed at repeat {} (stop): {}",
                            i + 1,
                            e
                        ));
                    }
                }
            }

            if i + 1 < repeat_count {
                tokio::time::sleep(timeout_dur).await;
            }
        }

        Ok(format!(
            "Тест действия выполнен: повторов — {}, таймаут — {} мс",
            repeat_count, timeout_ms
        ))
    }
}

async fn connect_ws(port: u16) -> Result<WsStream, String> {
    let url = format!("ws://127.0.0.1:{}", port);
    info!(%url, "Connecting to VTube Studio");

    let (ws, _resp) = timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(&url))
        .await
        .map_err(|_| {
            "Connection to VTube Studio timed out. Is it running with Plugin API enabled?"
                .to_string()
        })?
        .map_err(|e| format!("WebSocket connect failed: {}", e))?;

    info!("Connected to VTube Studio");
    Ok(ws)
}

async fn perform_authentication(
    ws: &mut WsStream,
    request_id: String,
    stored_token: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(token) = stored_token {
        if !token.is_empty() {
            debug!("Trying stored authentication token");
            let req = VtsRequest::authentication_request(&request_id, token);
            let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;

            match send_and_recv(ws, &json, &request_id, "AuthenticationResponse").await {
                Ok(value) => {
                    let data: messages::AuthenticationResponseData = serde_json::from_value(value)
                        .map_err(|e| format!("Parse auth response: {}", e))?;
                    if data.authenticated {
                        info!("Authenticated with stored token");
                        return Ok(None);
                    }
                    debug!("Stored token rejected, requesting new");
                }
                Err(e)
                    if e.starts_with("VTS error ")
                        || e.starts_with("Parse error data")
                        || e.starts_with("Parse response JSON") =>
                {
                    debug!(error = %e, "Stored token rejected by VTS, requesting new");
                }
                Err(e) => {
                    return Err(format!("Stored token authentication failed: {}", e));
                }
            }
        }
    }

    let token_req_id = format!("{}-tk", request_id);
    info!("Requesting new authentication token");
    let req = VtsRequest::auth_token_request(&token_req_id);
    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    let value = send_and_recv(ws, &json, &token_req_id, "AuthenticationTokenResponse")
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;

    let token_data: messages::AuthTokenResponseData =
        serde_json::from_value(value).map_err(|e| format!("Parse token response: {}", e))?;
    let token = token_data.authentication_token;
    debug!("Received new authentication token");

    let auth_req_id = format!("{}-au", request_id);
    let req = VtsRequest::authentication_request(&auth_req_id, &token);
    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    let value = send_and_recv(ws, &json, &auth_req_id, "AuthenticationResponse")
        .await
        .map_err(|e| format!("Auth request failed: {}", e))?;

    let data: messages::AuthenticationResponseData =
        serde_json::from_value(value).map_err(|e| format!("Parse auth response: {}", e))?;

    if !data.authenticated {
        return Err(
            "VTS rejected authentication. The token was not approved in VTube Studio.".to_string(),
        );
    }

    info!("Authentication successful");
    Ok(Some(token))
}

async fn create_typing_param(
    ws: &mut WsStream,
    request_id: String,
    parameter_name: &str,
) -> Result<(), String> {
    let req = VtsRequest::parameter_creation_request(&request_id, parameter_name);
    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    let _value = send_and_recv(ws, &json, &request_id, "ParameterCreationResponse")
        .await
        .map_err(|e| format!("Create parameter failed: {}", e))?;

    debug!(parameter_name, "Parameter ensured");
    Ok(())
}

async fn inject_typing(
    ws: &mut WsStream,
    request_id: String,
    parameter_name: &str,
    value: f64,
) -> Result<(), String> {
    let req = VtsRequest::inject_parameter_request(&request_id, parameter_name, value);
    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    let _value = send_and_recv(ws, &json, &request_id, "InjectParameterDataResponse")
        .await
        .map_err(|e| format!("Inject parameter failed: {}", e))?;

    debug!(parameter_name, value, "Parameter injected");
    Ok(())
}

async fn trigger_hotkey(
    ws: &mut WsStream,
    request_id: String,
    hotkey_id: &str,
) -> Result<(), String> {
    let req = VtsRequest::hotkey_trigger_request(&request_id, hotkey_id);
    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    let _value = send_and_recv(ws, &json, &request_id, "HotkeyTriggerResponse")
        .await
        .map_err(|e| format!("Hotkey trigger failed: {}", e))?;

    debug!(hotkey_id, "Hotkey triggered");
    Ok(())
}

enum RecvResult {
    Match(serde_json::Value),
    Skip,
    Error(String),
}

fn classify_vts_response(
    resp: &VtsResponse,
    expected_id: &str,
    expected_msg_type: &str,
) -> RecvResult {
    if resp.message_type == "APIError" {
        if resp.request_id == expected_id {
            match serde_json::from_value::<messages::VtsErrorData>(resp.data.clone()) {
                Ok(err) => RecvResult::Error(format!("VTS error {}", err.error_id)),
                Err(e) => RecvResult::Error(format!("Parse error data: {}", e)),
            }
        } else {
            RecvResult::Skip
        }
    } else if resp.message_type == expected_msg_type || resp.message_type == "APIResponse" {
        if resp.request_id != expected_id {
            RecvResult::Skip
        } else {
            RecvResult::Match(resp.data.clone())
        }
    } else {
        RecvResult::Skip
    }
}

async fn send_and_recv(
    ws: &mut WsStream,
    request_json: &str,
    expected_id: &str,
    expected_msg_type: &str,
) -> Result<serde_json::Value, String> {
    use tokio_tungstenite::tungstenite::Message;

    let send_msg = Message::Text(request_json.to_string().into());
    timeout(REQUEST_TIMEOUT, ws.send(send_msg))
        .await
        .map_err(|_| "Send timed out".to_string())?
        .map_err(|e| format!("Send failed: {}", e))?;

    timeout(
        REQUEST_TIMEOUT,
        recv_until_match(ws, expected_id, expected_msg_type),
    )
    .await
    .map_err(|_| "Response timed out".to_string())?
}

async fn recv_until_match(
    ws: &mut WsStream,
    expected_id: &str,
    expected_msg_type: &str,
) -> Result<serde_json::Value, String> {
    use tokio_tungstenite::tungstenite::Message;

    loop {
        let raw_msg = ws
            .next()
            .await
            .ok_or_else(|| "VTS connection closed".to_string())?
            .map_err(|e| format!("Read error: {}", e))?;

        let text = match raw_msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => return Err("VTS closed the connection".to_string()),
            Message::Ping(_) | Message::Pong(_) => continue,
            other => return Err(format!("Unexpected WebSocket message: {:?}", other)),
        };

        let parsed: VtsResponse =
            serde_json::from_str(&text).map_err(|e| format!("Parse response JSON: {}", e))?;

        let msg_type = parsed.message_type.clone();
        let req_id = parsed.request_id.clone();

        match classify_vts_response(&parsed, expected_id, expected_msg_type) {
            RecvResult::Match(data) => return Ok(data),
            RecvResult::Skip => {
                debug!(
                    expected_id,
                    %req_id,
                    %msg_type,
                    expected_msg_type,
                    "Skipping VTS response"
                );
                continue;
            }
            RecvResult::Error(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_defaults_are_correct() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            let settings = svc.settings.read().await;
            assert!(!settings.enabled);
            assert_eq!(settings.port, 8001);
            assert!(settings.token.is_none());
            assert_eq!(
                settings.typing_action.output_mode,
                VTubeStudioTypingMode::Event
            );
            assert_eq!(settings.typing_action.parameter_name, "TTSBardTyping");
        });
        assert!(!svc.is_desired_running());
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Disconnected
        );
    }

    #[test]
    fn next_id_is_uuid() {
        let svc = VTubeStudioService::new();
        let id1 = svc.next_id();
        let id2 = svc.next_id();
        assert_ne!(id1, id2);
        assert!(
            uuid::Uuid::parse_str(&id1).is_ok(),
            "{} is not a valid UUID",
            id1
        );
        assert!(
            uuid::Uuid::parse_str(&id2).is_ok(),
            "{} is not a valid UUID",
            id2
        );
    }

    #[test]
    fn disconnect_cleans_state() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        rt.block_on(async {
            svc.disconnect().await;
            let inner = svc.inner.lock().await;
            assert!(inner.ws.is_none());
            assert!(inner.typing_cancel.is_none());
            assert!(inner.typing_handle.is_none());
            assert!(!inner.typing_active);
        });
        assert!(!svc.is_desired_running());
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Disconnected
        );
    }

    #[test]
    fn set_typing_false_when_disconnected_is_noop() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            let result = svc.set_typing(false, 8001, "").await;
            assert!(result.is_ok());
            let inner = svc.inner.lock().await;
            assert!(!inner.typing_active);
        });
    }

    #[test]
    fn set_typing_true_with_empty_token_is_noop() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        rt.block_on(async {
            let result = svc.set_typing(true, 8001, "").await;
            assert!(result.is_ok());
            let inner = svc.inner.lock().await;
            assert!(inner.ws.is_none());
        });
    }

    #[test]
    fn set_typing_true_ignored_when_not_desired_running() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        assert!(!svc.is_desired_running());
        rt.block_on(async {
            let result = svc.set_typing(true, 8001, "test-token").await;
            assert!(result.is_ok());
            let inner = svc.inner.lock().await;
            assert!(inner.ws.is_none());
        });
    }

    #[test]
    fn desired_running_flags() {
        let svc = VTubeStudioService::new();
        assert!(!svc.is_desired_running());
        svc.set_desired_running(true);
        assert!(svc.is_desired_running());
        svc.set_desired_running(false);
        assert!(!svc.is_desired_running());
    }

    #[test]
    fn connection_status_transitions() {
        let svc = VTubeStudioService::new();
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Disconnected
        );

        svc.set_connection_status(VTubeStudioConnectionStatus::Connecting);
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Connecting
        );

        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Connected
        );

        svc.set_connection_status(VTubeStudioConnectionStatus::Error);
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Error
        );
    }

    fn make_response(msg_type: &str, request_id: &str, data: serde_json::Value) -> VtsResponse {
        VtsResponse {
            api_name: "VTubeStudioPublicAPI".into(),
            api_version: "1.0".into(),
            request_id: request_id.into(),
            message_type: msg_type.into(),
            data,
        }
    }

    #[test]
    fn classify_typed_response_matches() {
        let resp = make_response(
            "AuthenticationResponse",
            "req-1",
            serde_json::json!({"authenticated": true, "reason": ""}),
        );
        match classify_vts_response(&resp, "req-1", "AuthenticationResponse") {
            RecvResult::Match(data) => {
                assert_eq!(data["authenticated"].as_bool().unwrap(), true);
            }
            RecvResult::Skip => panic!("expected Match, got Skip"),
            RecvResult::Error(e) => panic!("expected Match, got Error: {}", e),
        }
    }

    #[test]
    fn classify_api_response_fallback_matches() {
        let resp = make_response(
            "APIResponse",
            "req-2",
            serde_json::json!({"authenticated": true, "reason": ""}),
        );
        match classify_vts_response(&resp, "req-2", "AuthenticationResponse") {
            RecvResult::Match(data) => {
                assert_eq!(data["authenticated"].as_bool().unwrap(), true);
            }
            RecvResult::Skip => panic!("expected Match, got Skip"),
            RecvResult::Error(e) => panic!("expected Match, got Error: {}", e),
        }
    }

    #[test]
    fn classify_parameter_creation_response() {
        let resp = make_response(
            "ParameterCreationResponse",
            "req-3",
            serde_json::json!({"parameterName": "TTSBardTyping"}),
        );
        match classify_vts_response(&resp, "req-3", "ParameterCreationResponse") {
            RecvResult::Match(data) => {
                assert_eq!(data["parameterName"].as_str().unwrap(), "TTSBardTyping");
            }
            _ => panic!("expected Match for ParameterCreationResponse"),
        }
    }

    #[test]
    fn classify_inject_parameter_response() {
        let resp = make_response(
            "InjectParameterDataResponse",
            "req-4",
            serde_json::json!({}),
        );
        match classify_vts_response(&resp, "req-4", "InjectParameterDataResponse") {
            RecvResult::Match(_) => {}
            _ => panic!("expected Match for InjectParameterDataResponse"),
        }
    }

    #[test]
    fn classify_hotkeys_in_current_model_response() {
        let resp = make_response(
            "HotkeysInCurrentModelResponse",
            "req-hk",
            serde_json::json!({
                "modelLoaded": true,
                "modelName": "test",
                "modelID": "id",
                "availableHotkeys": []
            }),
        );
        match classify_vts_response(&resp, "req-hk", "HotkeysInCurrentModelResponse") {
            RecvResult::Match(_) => {}
            _ => panic!("expected Match for HotkeysInCurrentModelResponse"),
        }
    }

    #[test]
    fn classify_hotkey_trigger_response_matches() {
        let resp = make_response("HotkeyTriggerResponse", "req-ht", serde_json::json!({}));
        match classify_vts_response(&resp, "req-ht", "HotkeyTriggerResponse") {
            RecvResult::Match(_) => {}
            _ => panic!("expected Match for HotkeyTriggerResponse"),
        }
    }

    #[test]
    fn classify_api_error_sanitizes_to_numeric_id() {
        let resp = make_response(
            "APIError",
            "req-5",
            serde_json::json!({"errorID": 42, "message": "Token rejected: secret-token-value"}),
        );
        match classify_vts_response(&resp, "req-5", "AuthenticationResponse") {
            RecvResult::Error(e) => {
                assert!(
                    e.contains("VTS error 42"),
                    "error should contain only numeric error ID, got: {}",
                    e
                );
                assert!(
                    !e.contains("secret-token-value"),
                    "error must not contain VTS message text: {}",
                    e
                );
                assert!(
                    !e.contains("Token rejected"),
                    "error must not contain VTS message text: {}",
                    e
                );
            }
            RecvResult::Match(_) => panic!("expected Error, got Match"),
            RecvResult::Skip => panic!("expected Error, got Skip"),
        }
    }

    #[test]
    fn classify_api_error_wrong_id_skipped() {
        let resp = make_response(
            "APIError",
            "other-req",
            serde_json::json!({"errorID": 1, "message": "Not ready"}),
        );
        match classify_vts_response(&resp, "my-req", "AuthenticationResponse") {
            RecvResult::Skip => {}
            RecvResult::Error(e) => {
                panic!("APIError with wrong ID must be skipped, got Error: {}", e)
            }
            RecvResult::Match(_) => panic!("APIError must not produce Match"),
        }
    }

    #[test]
    fn classify_mismatched_id_skipped() {
        let resp = make_response(
            "AuthenticationResponse",
            "wrong-id",
            serde_json::json!({"authenticated": true, "reason": ""}),
        );
        match classify_vts_response(&resp, "my-req", "AuthenticationResponse") {
            RecvResult::Skip => {}
            _ => panic!("mismatched request_id must be skipped"),
        }
    }

    #[test]
    fn classify_mismatched_id_on_api_response_fallback() {
        let resp = make_response(
            "APIResponse",
            "wrong-id",
            serde_json::json!({"authenticated": true, "reason": ""}),
        );
        match classify_vts_response(&resp, "my-req", "AuthenticationResponse") {
            RecvResult::Skip => {}
            _ => panic!("APIResponse with mismatched id must be skipped"),
        }
    }

    #[test]
    fn classify_unknown_type_skipped() {
        let resp = make_response(
            "ModelLoadedEvent",
            "req-6",
            serde_json::json!({"modelName": "test"}),
        );
        match classify_vts_response(&resp, "req-6", "AuthenticationResponse") {
            RecvResult::Skip => {}
            _ => panic!("unknown message type must be skipped"),
        }
    }

    #[test]
    fn classify_api_error_parse_failure() {
        let resp = make_response("APIError", "req-7", serde_json::json!("garbage"));
        match classify_vts_response(&resp, "req-7", "AuthenticationResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("Parse error data"), "got: {}", e);
            }
            _ => panic!("expected parse error"),
        }
    }

    #[test]
    fn inject_error_during_test_pulse_produces_error() {
        let resp = make_response(
            "APIError",
            "pulse-id",
            serde_json::json!({"errorID": 13, "message": "fail"}),
        );
        match classify_vts_response(&resp, "pulse-id", "InjectParameterDataResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 13"), "got: {}", e);
            }
            _ => panic!("InjectParameterData APIError must produce Error for test pulse"),
        }
    }

    #[test]
    fn inject_error_for_reset_produces_error() {
        let resp = make_response(
            "APIError",
            "reset-id",
            serde_json::json!({"errorID": 7, "message": "stub"}),
        );
        match classify_vts_response(&resp, "reset-id", "InjectParameterDataResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 7"), "got: {}", e);
            }
            _ => panic!("InjectParameterData APIError on reset must produce Error"),
        }
    }

    #[test]
    fn hotkey_trigger_error_classifies() {
        let resp = make_response(
            "APIError",
            "hk-err",
            serde_json::json!({"errorID": 5, "message": "invalid hotkey"}),
        );
        match classify_vts_response(&resp, "hk-err", "HotkeyTriggerResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 5"), "got: {}", e);
            }
            _ => panic!("HotkeyTriggerResponse APIError must produce Error"),
        }
    }

    #[test]
    fn disconnect_resets_desired_running() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        assert!(svc.is_desired_running());
        rt.block_on(async {
            svc.disconnect().await;
        });
        assert!(!svc.is_desired_running());
    }

    #[test]
    fn connect_failure_clears_desired_running() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            let result = svc.connect(8001, None).await;
            assert!(result.is_err());
        });
        assert!(!svc.is_desired_running());
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Error
        );
    }

    #[test]
    fn test_typing_action_fails_when_not_desired_running() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        assert!(!svc.is_desired_running());
        rt.block_on(async {
            let result = svc.test_typing_action(800, 1).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("not running") || msg.contains("Start the connection"),
                "error should mention not running: {}",
                msg
            );
        });
        assert!(!svc.is_desired_running());
    }

    #[test]
    fn test_typing_action_fails_when_disconnected() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        assert!(svc.is_desired_running());
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Disconnected
        );
        rt.block_on(async {
            let result = svc.test_typing_action(800, 1).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("not connected"),
                "error should mention not connected: {}",
                msg
            );
        });
        assert!(
            svc.is_desired_running(),
            "desired_running must remain unchanged"
        );
    }

    #[test]
    fn test_typing_action_rejects_timeout_99() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            let result = svc.test_typing_action(99, 1).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("100") && (msg.contains("5000") || msg.contains("до")),
                "error should mention 100–5000 range, got: {}",
                msg
            );
        });
        assert!(!svc.is_desired_running());
    }

    #[test]
    fn test_typing_action_rejects_timeout_5001() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            let result = svc.test_typing_action(5001, 1).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("100") && (msg.contains("5000") || msg.contains("до")),
                "error should mention 100–5000 range, got: {}",
                msg
            );
        });
        assert!(!svc.is_desired_running());
    }

    #[test]
    fn test_typing_action_rejects_repeat_0() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            let result = svc.test_typing_action(800, 0).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("1") && (msg.contains("10") || msg.contains("до")),
                "error should mention 1–10 range, got: {}",
                msg
            );
        });
        assert!(!svc.is_desired_running());
    }

    #[test]
    fn test_typing_action_rejects_repeat_11() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            let result = svc.test_typing_action(800, 11).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("1") && (msg.contains("10") || msg.contains("до")),
                "error should mention 1–10 range, got: {}",
                msg
            );
        });
        assert!(!svc.is_desired_running());
    }

    #[test]
    fn test_typing_action_boundary_100_passes_validation() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        rt.block_on(async {
            let result = svc.test_typing_action(100, 1).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("not connected") || msg.contains("Start the connection"),
                "boundary timeout 100 should pass validation and fail on lifecycle: {}",
                msg
            );
        });
        assert!(svc.is_desired_running());
    }

    #[test]
    fn test_typing_action_boundary_5000_passes_validation() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        rt.block_on(async {
            let result = svc.test_typing_action(5000, 1).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("not connected") || msg.contains("Start the connection"),
                "boundary timeout 5000 should pass validation and fail on lifecycle: {}",
                msg
            );
        });
        assert!(svc.is_desired_running());
    }

    #[test]
    fn test_typing_action_boundary_repeat_1_passes_validation() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        rt.block_on(async {
            let result = svc.test_typing_action(800, 1).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("not connected") || msg.contains("Start the connection"),
                "boundary repeat 1 should pass validation and fail on lifecycle: {}",
                msg
            );
        });
        assert!(svc.is_desired_running());
    }

    #[test]
    fn test_typing_action_boundary_repeat_10_passes_validation() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        rt.block_on(async {
            let result = svc.test_typing_action(800, 10).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("not connected") || msg.contains("Start the connection"),
                "boundary repeat 10 should pass validation and fail on lifecycle: {}",
                msg
            );
        });
        assert!(svc.is_desired_running());
    }

    #[test]
    fn test_typing_action_keeps_desired_running_on_failure() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        rt.block_on(async {
            let result = svc.test_typing_action(800, 1).await;
            assert!(result.is_err());
        });
        assert!(svc.is_desired_running());
    }

    #[test]
    fn get_current_model_hotkeys_fails_when_not_desired_running() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            let result = svc.get_current_model_hotkeys().await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn get_current_model_hotkeys_fails_when_disconnected() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        rt.block_on(async {
            let result = svc.get_current_model_hotkeys().await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn typing_action_default_is_event_mode() {
        let svc = VTubeStudioService::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let s = svc.settings.read().await;
            assert_eq!(s.typing_action.output_mode, VTubeStudioTypingMode::Event);
            assert_eq!(s.typing_action.parameter_name, "TTSBardTyping");
            assert!(s.typing_action.start_hotkey_id.is_empty());
            assert!(s.typing_action.stop_hotkey_id.is_empty());
        });
    }

    #[test]
    fn get_current_model_hotkeys_fails_when_not_authenticated() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
        assert!(!svc.is_authenticated.load(Ordering::SeqCst));
        rt.block_on(async {
            let result = svc.get_current_model_hotkeys().await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("not authenticated"),
                "should fail with not authenticated: {}",
                msg
            );
        });
    }

    #[test]
    fn get_current_model_hotkeys_fails_when_no_live_socket() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
        svc.is_authenticated.store(true, Ordering::SeqCst);
        rt.block_on(async {
            let result = svc.get_current_model_hotkeys().await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("not available"),
                "should fail with not available: {}",
                msg
            );
        });
    }

    #[test]
    fn connect_is_transport_only_no_side_effects() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            let result = svc.connect(8001, None).await;
            assert!(result.is_err());
        });
        assert!(!svc.is_desired_running());
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Error
        );
        assert!(!svc.is_authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn disconnect_with_hotkeys_config_cleans_state() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Hotkeys;
                s.typing_action.start_hotkey_id = "hk-start".to_string();
                s.typing_action.stop_hotkey_id = "hk-stop".to_string();
            }
            svc.disconnect().await;
            let inner = svc.inner.lock().await;
            assert!(inner.ws.is_none());
            assert!(inner.typing_cancel.is_none());
            assert!(inner.typing_handle.is_none());
            assert!(!inner.typing_active);
        });
        assert!(!svc.is_desired_running());
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Disconnected
        );
    }

    #[test]
    fn test_typing_action_fails_when_ws_not_available() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
        rt.block_on(async {
            let result = svc.test_typing_action(800, 1).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(
                msg.contains("not available"),
                "should fail with not available: {}",
                msg
            );
        });
        assert!(svc.is_desired_running());
    }

    #[test]
    fn classification_roundtrip_all_response_types() {
        let cases = vec![
            ("AuthenticationTokenResponse", "APIResponse"),
            ("AuthenticationResponse", "AuthenticationResponse"),
            ("AuthenticationResponse", "APIResponse"),
            ("ParameterCreationResponse", "ParameterCreationResponse"),
            ("InjectParameterDataResponse", "InjectParameterDataResponse"),
            (
                "HotkeysInCurrentModelResponse",
                "HotkeysInCurrentModelResponse",
            ),
            ("HotkeyTriggerResponse", "HotkeyTriggerResponse"),
        ];
        let req_id = "rtt";

        for (msg_type, expected_match) in cases {
            let resp = make_response(msg_type, req_id, serde_json::json!({}));
            let expected_msg_type = if expected_match == "APIResponse" {
                if msg_type == "AuthenticationTokenResponse" {
                    "AuthenticationTokenResponse"
                } else {
                    "AuthenticationResponse"
                }
            } else {
                msg_type
            };
            match classify_vts_response(&resp, req_id, expected_msg_type) {
                RecvResult::Match(_) => {
                    assert!(
                        msg_type == expected_msg_type
                            || (expected_match == "APIResponse" && msg_type == expected_match)
                            || msg_type == expected_match,
                        "expected Match for {}/expecting {}, but got Match from msg_type={}",
                        expected_msg_type,
                        expected_match,
                        msg_type
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn hotkey_trigger_api_error_skipped_when_wrong_expected_type() {
        let resp = make_response(
            "APIError",
            "hk-err-2",
            serde_json::json!({"errorID": 99, "message": "bad"}),
        );
        match classify_vts_response(&resp, "hk-err-2", "InjectParameterDataResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 99"), "got: {}", e);
            }
            _ => panic!("expected Error"),
        }
    }
}
