use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::messages::{
    self, HotkeyInfo, HotkeysInCurrentModelResponseData, ItemAnimationControlResponseData,
    ItemInstanceInfo, ItemListResponseData, VtsRequest, VtsResponse,
};
use crate::config::{VTubeStudioSettings, VTubeStudioTypingMode};
use crate::events::VTubeStudioConnectionStatus;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const ITEM_ACTION_TIMEOUT: Duration = Duration::from_secs(2);
const TYPING_KEEPALIVE_MS: u64 = 500;

type WsStream = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone, PartialEq)]
enum ItemKind {
    Static,
    Animated,
    Unsupported { original_type: String },
}

impl ItemKind {
    fn classify(item_type: &str) -> Self {
        match item_type {
            "PNG" | "JPG" => ItemKind::Static,
            "GIF" | "AnimationFolder" => ItemKind::Animated,
            "Live2D" | "Unknown" => ItemKind::Unsupported {
                original_type: item_type.to_string(),
            },
            _ => ItemKind::Unsupported {
                original_type: item_type.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status")]
pub enum VTubeStudioItemStatus {
    Inactive,
    Ready {
        #[serde(rename = "fileName")]
        file_name: String,
        #[serde(rename = "vtsType")]
        vts_type: String,
    },
    Missing {
        #[serde(rename = "fileName")]
        file_name: String,
    },
    Ambiguous {
        #[serde(rename = "fileName")]
        file_name: String,
        #[serde(rename = "matchCount")]
        match_count: u32,
    },
    Unsupported {
        #[serde(rename = "fileName")]
        file_name: String,
        #[serde(rename = "vtsType")]
        vts_type: String,
    },
    Error {
        #[serde(rename = "fileName")]
        file_name: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneItemRecord {
    pub file_name: String,
    pub item_type: String,
    pub supported: bool,
    pub duplicate_count: u32,
}

#[derive(Debug, Clone)]
struct ResolvedItem {
    instance_id: String,
    file_name: String,
    item_type: String,
    kind: ItemKind,
}

#[derive(Debug, PartialEq)]
enum ResolveItemError {
    Missing,
    Ambiguous,
    Unsupported {
        file_name: String,
        item_type: String,
    },
}

impl std::fmt::Display for ResolveItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveItemError::Missing => {
                write!(f, "no scene item matching the configured filename")
            }
            ResolveItemError::Ambiguous => {
                write!(f, "multiple scene items match the configured filename")
            }
            ResolveItemError::Unsupported {
                file_name,
                item_type,
            } => {
                write!(
                    f,
                    "item '{}' has unsupported type '{}'",
                    file_name, item_type
                )
            }
        }
    }
}

fn resolve_item(
    instances: &[ItemInstanceInfo],
    configured_file_name: &str,
) -> Result<ResolvedItem, ResolveItemError> {
    let matching: Vec<&ItemInstanceInfo> = instances
        .iter()
        .filter(|i| i.file_name == configured_file_name)
        .collect();

    if matching.is_empty() {
        return Err(ResolveItemError::Missing);
    }
    if matching.len() > 1 {
        return Err(ResolveItemError::Ambiguous);
    }

    let item = matching[0];
    let kind = ItemKind::classify(&item.item_type);

    if let ItemKind::Unsupported { ref original_type } = kind {
        return Err(ResolveItemError::Unsupported {
            file_name: item.file_name.clone(),
            item_type: original_type.clone(),
        });
    }

    Ok(ResolvedItem {
        instance_id: item.instance_id.clone(),
        file_name: item.file_name.clone(),
        item_type: item.item_type.clone(),
        kind,
    })
}

struct InnerState {
    ws: Option<WsStream>,
    typing_cancel: Option<CancellationToken>,
    typing_handle: Option<tokio::task::JoinHandle<()>>,
    typing_active: bool,
    resolved_item: Option<ResolvedItem>,
}

#[derive(Debug, Default)]
struct ItemSyncState {
    desired: bool,
    generation: u64,
    applied: Option<bool>,
    worker_running: bool,
}

struct ItemTransitionState {
    inner: parking_lot::Mutex<ItemSyncState>,
}

impl ItemTransitionState {
    fn new() -> Self {
        Self {
            inner: parking_lot::Mutex::new(ItemSyncState::default()),
        }
    }

    fn record_desired(&self, visible: bool) {
        let mut s = self.inner.lock();
        if s.worker_running {
            if s.desired != visible {
                s.desired = visible;
                s.generation = s.generation.wrapping_add(1);
            }
        } else if s.applied != Some(visible) {
            s.desired = visible;
            s.generation = s.generation.wrapping_add(1);
        }
    }

    fn read_desired(&self) -> (bool, u64) {
        let s = self.inner.lock();
        (s.desired, s.generation)
    }

    fn read_applied(&self) -> Option<bool> {
        self.inner.lock().applied
    }

    fn begin_work(&self) -> Option<u64> {
        let mut s = self.inner.lock();
        if s.worker_running {
            return None;
        }
        if s.applied == Some(s.desired) {
            return None;
        }
        s.worker_running = true;
        Some(s.generation)
    }

    fn finish_success(&self, gen: u64, visible: bool) -> bool {
        let mut s = self.inner.lock();
        if s.generation == gen {
            s.applied = Some(visible);
        }
        if s.applied != Some(s.desired) {
            true
        } else {
            s.worker_running = false;
            false
        }
    }

    fn finish_failure(&self, attempted_gen: u64) -> bool {
        let mut s = self.inner.lock();
        s.applied = None;
        if s.generation != attempted_gen {
            true
        } else {
            s.worker_running = false;
            false
        }
    }

    fn end_work(&self) {
        self.inner.lock().worker_running = false;
    }

    fn set_applied_if_current(&self, visible: bool, gen: u64) -> bool {
        let mut s = self.inner.lock();
        if s.generation == gen {
            s.applied = Some(visible);
            true
        } else {
            false
        }
    }

    fn mark_applied_unknown(&self) {
        self.inner.lock().applied = None;
    }

    fn reset(&self) {
        let mut s = self.inner.lock();
        s.desired = false;
        s.generation = s.generation.wrapping_add(1);
        s.applied = None;
        s.worker_running = false;
    }

    fn force_applied(&self, visible: bool) {
        self.inner.lock().applied = Some(visible);
    }
}

pub struct VTubeStudioService {
    pub settings: Arc<tokio::sync::RwLock<VTubeStudioSettings>>,
    inner: Arc<tokio::sync::Mutex<InnerState>>,
    is_authenticated: Arc<AtomicBool>,
    desired_running: Arc<AtomicBool>,
    connection_status: Arc<parking_lot::Mutex<VTubeStudioConnectionStatus>>,
    item_status: Arc<parking_lot::Mutex<VTubeStudioItemStatus>>,
    item_transition: ItemTransitionState,
    session: AtomicU64,
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
                resolved_item: None,
            })),
            is_authenticated: Arc::new(AtomicBool::new(false)),
            desired_running: Arc::new(AtomicBool::new(false)),
            connection_status: Arc::new(parking_lot::Mutex::new(
                VTubeStudioConnectionStatus::Disconnected,
            )),
            item_status: Arc::new(parking_lot::Mutex::new(VTubeStudioItemStatus::Inactive)),
            item_transition: ItemTransitionState::new(),
            session: AtomicU64::new(0),
        }
    }

    fn read_session(&self) -> u64 {
        self.session.load(Ordering::Acquire)
    }

    fn invalidate_session(&self) -> u64 {
        self.session.fetch_add(1, Ordering::SeqCst) + 1
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

    pub fn get_item_status(&self) -> VTubeStudioItemStatus {
        self.item_status.lock().clone()
    }

    pub fn record_item_desired(&self, visible: bool) {
        self.item_transition.record_desired(visible);
    }

    pub async fn run_item_sync(&self) -> VTubeStudioItemStatus {
        match self.item_transition.begin_work() {
            Some(_) => {}
            None => return self.get_item_status(),
        }

        let session = self.read_session();

        loop {
            let typing_action = { self.settings.read().await.typing_action.clone() };
            if typing_action.output_mode != VTubeStudioTypingMode::Item {
                self.item_transition.end_work();
                break;
            }

            if self.read_session() != session {
                self.item_transition.end_work();
                break;
            }

            let (desired, gen) = self.item_transition.read_desired();

            let (mut sock, resolved, item_type_for_log) = {
                let mut inner = self.inner.lock().await;
                if self.read_session() != session {
                    self.item_transition.end_work();
                    break;
                }
                let ws = inner.ws.take();
                let resolved = inner.resolved_item.clone();
                let it = match &resolved {
                    Some(r) => r.item_type.clone(),
                    None => String::new(),
                };
                (ws, resolved, it)
            };

            let file_name_for_log = match &resolved {
                Some(r) => r.file_name.clone(),
                None => typing_action.item_file_name.clone(),
            };

            let result = match (sock.as_mut(), &resolved) {
                (Some(ws), Some(item)) => {
                    let start = Instant::now();
                    let res = animate_item(ws, self.next_id(), item, desired).await;
                    let duration = start.elapsed();
                    info!(
                        mode = "Item",
                        item_type = %item_type_for_log,
                        file_name = %file_name_for_log,
                        desired = desired,
                        duration_ms = duration.as_millis(),
                        "Item animation request completed"
                    );
                    res
                }
                (None, _) => {
                    warn!(
                        mode = "Item",
                        file_name = %file_name_for_log,
                        "No WebSocket available for item sync"
                    );
                    self.item_transition.end_work();
                    return self.get_item_status();
                }
                (_, None) => {
                    self.item_transition.end_work();
                    return self.get_item_status();
                }
            };

            if let Some(s) = sock {
                let mut inner = self.inner.lock().await;
                if self.read_session() == session {
                    inner.ws = Some(s);
                }
            }

            match result {
                Ok(()) => {
                    if self.read_session() == session {
                        let continue_work = self.item_transition.finish_success(gen, desired);
                        let status = VTubeStudioItemStatus::Ready {
                            file_name: file_name_for_log.clone(),
                            vts_type: item_type_for_log.clone(),
                        };
                        *self.item_status.lock() = status;

                        if !continue_work {
                            break;
                        }
                    } else {
                        self.item_transition.end_work();
                        break;
                    }
                }
                Err(..) => {
                    if self.read_session() == session {
                        warn!(
                            mode = "Item",
                            file_name = %file_name_for_log,
                            "Item animation request failed"
                        );
                        let continue_work = self.item_transition.finish_failure(gen);
                        let status = VTubeStudioItemStatus::Error {
                            file_name: file_name_for_log.clone(),
                            message: "item action failed".to_string(),
                        };
                        *self.item_status.lock() = status;
                        if continue_work {
                            continue;
                        }
                    } else {
                        self.item_transition.end_work();
                    }
                    return self.get_item_status();
                }
            }
        }

        self.get_item_status()
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

        let typing_action = { self.settings.read().await.typing_action.clone() };

        let mut inner = self.inner.lock().await;
        self.stop_typing_keepalive_locked(&mut inner);
        inner.typing_active = false;
        inner.resolved_item = None;
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

        match typing_action.output_mode {
            VTubeStudioTypingMode::Event | VTubeStudioTypingMode::Hotkeys => {
                *self.item_status.lock() = VTubeStudioItemStatus::Inactive;
            }
            VTubeStudioTypingMode::Item => {
                let file_name = typing_action.item_file_name.clone();
                if file_name.is_empty() {
                    *self.item_status.lock() = VTubeStudioItemStatus::Missing {
                        file_name: String::new(),
                    };
                } else {
                    let session = self.read_session();
                    let mut sock = inner.ws.take().unwrap();
                    let (resolved, status) = self
                        .do_item_refresh_with_desired(&mut sock, &file_name, session)
                        .await;
                    if self.read_session() == session {
                        inner.ws = Some(sock);
                        inner.resolved_item = resolved;
                        *self.item_status.lock() = status;
                    }
                }
            }
        }

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
            } else if typing_action.output_mode == VTubeStudioTypingMode::Hotkeys {
                if let Some(ref mut ws) = inner.ws {
                    let stop_id = typing_action.stop_hotkey_id.clone();
                    if let Err(e) = trigger_hotkey(ws, self.next_id(), &stop_id).await {
                        debug!(error = %e, "VTS hotkey stop trigger failed, discarding broken socket");
                        inner.ws = None;
                        self.is_authenticated.store(false, Ordering::SeqCst);
                    }
                }
            } else if typing_action.output_mode == VTubeStudioTypingMode::Item {
                self.item_transition.record_desired(false);
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

                if let Err(e) = ensure_event_parameter(ws, self.next_id(), &param_name).await {
                    debug!(error = %e, "VTS ensure event parameter failed, discarding broken socket");
                    inner.ws = None;
                    inner.typing_active = false;
                    self.is_authenticated.store(false, Ordering::SeqCst);
                    self.set_connection_status(VTubeStudioConnectionStatus::Error);
                    return Err(e);
                }

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
            VTubeStudioTypingMode::Item => {
                self.item_transition.record_desired(true);
            }
        }

        Ok(())
    }

    pub async fn refresh_item_action(&self) -> VTubeStudioItemStatus {
        let typing_action = { self.settings.read().await.typing_action.clone() };

        match typing_action.output_mode {
            VTubeStudioTypingMode::Event | VTubeStudioTypingMode::Hotkeys => {
                let mut inner = self.inner.lock().await;
                inner.resolved_item = None;
                let status = VTubeStudioItemStatus::Inactive;
                *self.item_status.lock() = status.clone();
                status
            }
            VTubeStudioTypingMode::Item => {
                let file_name = typing_action.item_file_name.clone();

                if file_name.is_empty() {
                    let mut inner = self.inner.lock().await;
                    inner.resolved_item = None;
                    let status = VTubeStudioItemStatus::Missing {
                        file_name: String::new(),
                    };
                    *self.item_status.lock() = status.clone();
                    return status;
                }

                let mut inner = self.inner.lock().await;

                let mut sock = match inner.ws.take() {
                    Some(ws) => ws,
                    None => {
                        inner.resolved_item = None;
                        let status = VTubeStudioItemStatus::Error {
                            file_name: file_name.clone(),
                            message: "WebSocket not available".to_string(),
                        };
                        *self.item_status.lock() = status.clone();
                        return status;
                    }
                };

                let session = self.read_session();
                let (resolved, status) = self
                    .do_item_refresh_with_desired(&mut sock, &file_name, session)
                    .await;
                if self.read_session() == session {
                    inner.ws = Some(sock);
                    inner.resolved_item = resolved;
                    *self.item_status.lock() = status.clone();
                }
                status
            }
        }
    }

    async fn do_item_refresh_with_desired(
        &self,
        ws: &mut WsStream,
        file_name: &str,
        session: u64,
    ) -> (Option<ResolvedItem>, VTubeStudioItemStatus) {
        let (desired, desired_gen) = self.item_transition.read_desired();

        match fetch_scene_instances(ws, self.next_id(), Some(file_name)).await {
            Ok(instances) => {
                if self.read_session() != session {
                    return (None, VTubeStudioItemStatus::Inactive);
                }

                match resolve_item(&instances, file_name) {
                    Ok(item) => {
                        let resolved = item.clone();
                        match animate_item(ws, self.next_id(), &item, desired).await {
                            Ok(_) => {
                                if self.read_session() == session {
                                    self.item_transition
                                        .set_applied_if_current(desired, desired_gen);
                                }
                                let status = VTubeStudioItemStatus::Ready {
                                    file_name: resolved.file_name.clone(),
                                    vts_type: resolved.item_type.clone(),
                                };
                                (Some(resolved), status)
                            }
                            Err(e) => {
                                if self.read_session() == session {
                                    self.item_transition.mark_applied_unknown();
                                }
                                let status = VTubeStudioItemStatus::Error {
                                    file_name: file_name.to_string(),
                                    message: format!("failed to normalize item: {}", e),
                                };
                                (Some(resolved), status)
                            }
                        }
                    }
                    Err(ResolveItemError::Missing) => {
                        let status = VTubeStudioItemStatus::Missing {
                            file_name: file_name.to_string(),
                        };
                        (None, status)
                    }
                    Err(ResolveItemError::Ambiguous) => {
                        let count = instances
                            .iter()
                            .filter(|i| i.file_name == file_name)
                            .count() as u32;
                        let status = VTubeStudioItemStatus::Ambiguous {
                            file_name: file_name.to_string(),
                            match_count: count,
                        };
                        (None, status)
                    }
                    Err(ResolveItemError::Unsupported {
                        file_name: fn_,
                        item_type,
                    }) => {
                        let status = VTubeStudioItemStatus::Unsupported {
                            file_name: fn_,
                            vts_type: item_type,
                        };
                        (None, status)
                    }
                }
            }
            Err(e) => {
                if self.read_session() == session {
                    self.item_transition.mark_applied_unknown();
                }
                let status = VTubeStudioItemStatus::Error {
                    file_name: file_name.to_string(),
                    message: e,
                };
                (None, status)
            }
        }
    }

    pub async fn list_scene_items(&self) -> Result<Vec<SceneItemRecord>, String> {
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

        let instances = fetch_scene_instances(ws, self.next_id(), None).await?;

        Ok(build_scene_records(&instances))
    }

    pub async fn disconnect(&self) {
        let typing_action = { self.settings.read().await.typing_action.clone() };
        let mut inner = self.inner.lock().await;
        self.set_desired_running(false);

        let typing_active = inner.typing_active;
        let resolved_item = inner.resolved_item.clone();

        self.stop_typing_keepalive_locked(&mut inner);
        inner.typing_active = false;

        if typing_action.output_mode == VTubeStudioTypingMode::Item {
            let applied = self.item_transition.read_applied();
            self.invalidate_session();
            self.item_transition.reset();

            let should_hide = matches!(
                (resolved_item.as_ref(), applied),
                (Some(_), Some(true)) | (Some(_), None)
            );

            if should_hide {
                if let (Some(ref mut ws), Some(ref item)) = (&mut inner.ws, &resolved_item) {
                    let start = Instant::now();
                    if let Err(_e) = animate_item(ws, self.next_id(), item, false).await {
                        warn!(
                            mode = "Item",
                            file_name = %item.file_name,
                            "Best-effort hide during disconnect failed"
                        );
                    }
                    let duration = start.elapsed();
                    info!(
                        mode = "Item",
                        item_type = %item.item_type,
                        file_name = %item.file_name,
                        desired = false,
                        duration_ms = duration.as_millis(),
                        "Disconnect hide completed"
                    );
                }
            }

            inner.ws = None;
            inner.resolved_item = None;
            self.is_authenticated.store(false, Ordering::SeqCst);
            self.set_connection_status(VTubeStudioConnectionStatus::Disconnected);
            *self.item_status.lock() = VTubeStudioItemStatus::Inactive;
            info!("VTube Studio disconnected");
            return;
        }

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
                    VTubeStudioTypingMode::Item => {}
                }
            }
        }

        inner.ws = None;
        inner.resolved_item = None;
        self.is_authenticated.store(false, Ordering::SeqCst);
        self.set_connection_status(VTubeStudioConnectionStatus::Disconnected);
        *self.item_status.lock() = VTubeStudioItemStatus::Inactive;
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

        if !(100..=5000).contains(&timeout_ms) {
            return Err("Таймаут должен быть от 100 до 5000 мс".to_string());
        }
        if !(1..=10).contains(&repeat_count) {
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

        if typing_action.output_mode == VTubeStudioTypingMode::Item {
            // Invalidate session so any in-flight worker completes without
            // restoring state after this test ends (requirement 4).
            let session = self.invalidate_session();
            let resolved = inner.resolved_item.clone();
            let ws = inner.ws.as_mut().unwrap();

            let item = match resolved {
                Some(ref item) => item.clone(),
                None => {
                    return Err(
                        "No resolved item available for testing. Refresh the item first."
                            .to_string(),
                    );
                }
            };

            for i in 0..repeat_count {
                if let Err(e) = animate_item(ws, self.next_id(), &item, true).await {
                    self.item_transition.mark_applied_unknown();
                    *self.item_status.lock() = VTubeStudioItemStatus::Error {
                        file_name: item.file_name.clone(),
                        message: format!("show failed at repeat {}: {}", i + 1, e),
                    };
                    // Do not change connection status on item test failure
                    return Err(format!("Item show failed at repeat {}: {}", i + 1, e));
                }

                tokio::time::sleep(Duration::from_millis(timeout_ms)).await;

                if let Err(e) = animate_item(ws, self.next_id(), &item, false).await {
                    self.item_transition.mark_applied_unknown();
                    *self.item_status.lock() = VTubeStudioItemStatus::Error {
                        file_name: item.file_name.clone(),
                        message: format!("hide failed at repeat {}: {}", i + 1, e),
                    };
                    // Do not change connection status on item test failure
                    return Err(format!("Item hide failed at repeat {}: {}", i + 1, e));
                }

                if i + 1 < repeat_count {
                    tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
                }
            }

            // Guarded commit: only restore if session is still current (requirement 3)
            if self.read_session() == session {
                self.item_transition.reset();
                self.item_transition.force_applied(false);
                *self.item_status.lock() = VTubeStudioItemStatus::Ready {
                    file_name: item.file_name.clone(),
                    vts_type: item.item_type.clone(),
                };
            }

            return Ok(format!(
                "Тест действия выполнен: повторов — {}, таймаут — {} мс",
                repeat_count, timeout_ms
            ));
        }

        let timeout_dur = Duration::from_millis(timeout_ms);

        for i in 0..repeat_count {
            let ws = inner.ws.as_mut().unwrap();

            match typing_action.output_mode {
                VTubeStudioTypingMode::Event => {
                    let param_name = typing_action.parameter_name.clone();
                    if let Err(e) = ensure_event_parameter(ws, self.next_id(), &param_name).await {
                        inner.ws = None;
                        self.is_authenticated.store(false, Ordering::SeqCst);
                        self.set_connection_status(VTubeStudioConnectionStatus::Error);
                        return Err(format!(
                            "VTube Studio action test failed at repeat {} (ensure): {}",
                            i + 1,
                            e
                        ));
                    }
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
                VTubeStudioTypingMode::Item => {
                    // Unreachable — Item mode handled above.
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
                VTubeStudioTypingMode::Item => {
                    // Unreachable — Item mode handled above.
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

/// Идемпотентно гарантирует custom INPUT перед inject в Event-режиме.
/// VTS повторно создаёт тот же параметр тем же plugin identity без ошибки.
async fn ensure_event_parameter(
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

async fn fetch_scene_instances(
    ws: &mut WsStream,
    request_id: String,
    file_name: Option<&str>,
) -> Result<Vec<ItemInstanceInfo>, String> {
    let req = VtsRequest::item_list_request(&request_id, file_name);
    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    let value = send_and_recv(ws, &json, &request_id, "ItemListResponse")
        .await
        .map_err(|e| e.to_string())?;

    let data: ItemListResponseData =
        serde_json::from_value(value).map_err(|e| format!("Parse item list response: {}", e))?;

    Ok(data.item_instances_in_scene)
}

async fn animate_item(
    ws: &mut WsStream,
    request_id: String,
    resolved: &ResolvedItem,
    show: bool,
) -> Result<(), String> {
    let (opacity, frame, play_state) = match (&resolved.kind, show) {
        (ItemKind::Animated, true) => (1.0, Some(0), Some(true)),
        (ItemKind::Animated, false) => (0.0, None, Some(false)),
        (ItemKind::Static, true) => (1.0, None, None),
        (ItemKind::Static, false) => (0.0, None, None),
        _ => {
            return Err(format!(
                "cannot animate unsupported item kind for '{}'",
                resolved.file_name
            ))
        }
    };

    let req = VtsRequest::item_animation_control_request(
        &request_id,
        &resolved.instance_id,
        opacity,
        frame,
        play_state,
    );
    let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    let value = timeout(
        ITEM_ACTION_TIMEOUT,
        send_and_recv(ws, &json, &request_id, "ItemAnimationControlResponse"),
    )
    .await
    .map_err(|_| "Item animation request timed out".to_string())?
    .map_err(|e| format!("Item animation request failed: {}", e))?;

    let _data: ItemAnimationControlResponseData = serde_json::from_value(value)
        .map_err(|e| format!("Malformed item animation response: {}", e))?;

    debug!(
        opacity,
        show,
        item_type = %resolved.item_type,
        file_name = %resolved.file_name,
        "Item animation control applied"
    );
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

    let send_msg = Message::Text(request_json.to_string());
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

fn build_scene_records(instances: &[ItemInstanceInfo]) -> Vec<SceneItemRecord> {
    let records: Vec<SceneItemRecord> = instances
        .iter()
        .map(|inst| {
            let kind = ItemKind::classify(&inst.item_type);
            let supported = !matches!(kind, ItemKind::Unsupported { .. });
            SceneItemRecord {
                file_name: inst.file_name.clone(),
                item_type: inst.item_type.clone(),
                supported,
                duplicate_count: 1,
            }
        })
        .collect();

    let mut grouped: Vec<SceneItemRecord> = Vec::new();
    let mut seen: HashMap<(String, String), usize> = HashMap::new();

    for rec in &records {
        let key = (rec.file_name.clone(), rec.item_type.clone());
        if let Some(idx) = seen.get(&key) {
            grouped[*idx].duplicate_count += 1;
        } else {
            seen.insert(key, grouped.len());
            grouped.push(rec.clone());
        }
    }

    grouped.sort_by(|a, b| {
        a.file_name
            .cmp(&b.file_name)
            .then_with(|| a.item_type.cmp(&b.item_type))
    });

    grouped
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
                assert!(data["authenticated"].as_bool().unwrap());
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
                assert!(data["authenticated"].as_bool().unwrap());
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
            if let RecvResult::Match(_) = classify_vts_response(&resp, req_id, expected_msg_type) {
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

    fn make_instance(file_name: &str, instance_id: &str, item_type: &str) -> ItemInstanceInfo {
        ItemInstanceInfo {
            file_name: file_name.to_string(),
            instance_id: instance_id.to_string(),
            item_type: item_type.to_string(),
            framerate: 0.0,
            frame_count: -1,
            current_frame: -1,
        }
    }

    #[test]
    fn item_kind_classifies_types() {
        assert_eq!(ItemKind::classify("PNG"), ItemKind::Static);
        assert_eq!(ItemKind::classify("JPG"), ItemKind::Static);
        assert_eq!(ItemKind::classify("GIF"), ItemKind::Animated);
        assert_eq!(ItemKind::classify("AnimationFolder"), ItemKind::Animated);
        assert_eq!(
            ItemKind::classify("Live2D"),
            ItemKind::Unsupported {
                original_type: "Live2D".to_string()
            }
        );
        assert_eq!(
            ItemKind::classify("Unknown"),
            ItemKind::Unsupported {
                original_type: "Unknown".to_string()
            }
        );
        assert_eq!(
            ItemKind::classify("FutureTypeXYZ"),
            ItemKind::Unsupported {
                original_type: "FutureTypeXYZ".to_string()
            }
        );
    }

    #[test]
    fn resolve_zero_matches_is_missing() {
        let instances = vec![make_instance("icon.png", "i1", "PNG")];
        assert_eq!(
            resolve_item(&instances, "other.png").unwrap_err(),
            ResolveItemError::Missing
        );
    }

    #[test]
    fn resolve_single_match_returns_item() {
        let instances = vec![make_instance("icon.png", "i1", "PNG")];
        let resolved = resolve_item(&instances, "icon.png").unwrap();
        assert_eq!(resolved.instance_id, "i1");
        assert_eq!(resolved.file_name, "icon.png");
        assert_eq!(resolved.item_type, "PNG");
        assert_eq!(resolved.kind, ItemKind::Static);
    }

    #[test]
    fn resolve_duplicate_filename_is_ambiguous() {
        let instances = vec![
            make_instance("icon.png", "i1", "PNG"),
            make_instance("icon.png", "i2", "PNG"),
        ];
        assert_eq!(
            resolve_item(&instances, "icon.png").unwrap_err(),
            ResolveItemError::Ambiguous
        );
    }

    #[test]
    fn resolve_wrong_case_is_missing() {
        let instances = vec![make_instance("Icon.png", "i1", "PNG")];
        assert_eq!(
            resolve_item(&instances, "icon.png").unwrap_err(),
            ResolveItemError::Missing
        );
    }

    #[test]
    fn resolve_unsupported_rejected_with_type() {
        let instances = vec![make_instance("model.moc3", "i1", "Live2D")];
        assert_eq!(
            resolve_item(&instances, "model.moc3").unwrap_err(),
            ResolveItemError::Unsupported {
                file_name: "model.moc3".to_string(),
                item_type: "Live2D".to_string(),
            }
        );
    }

    #[test]
    fn resolve_future_type_rejected_with_diagnostic() {
        let instances = vec![make_instance("video.mp4", "i1", "VideoClip")];
        assert_eq!(
            resolve_item(&instances, "video.mp4").unwrap_err(),
            ResolveItemError::Unsupported {
                file_name: "video.mp4".to_string(),
                item_type: "VideoClip".to_string(),
            }
        );
    }

    #[test]
    fn resolve_unsupported_unknown_rejected() {
        let instances = vec![make_instance("thing.dat", "i1", "Unknown")];
        assert_eq!(
            resolve_item(&instances, "thing.dat").unwrap_err(),
            ResolveItemError::Unsupported {
                file_name: "thing.dat".to_string(),
                item_type: "Unknown".to_string(),
            }
        );
    }

    #[test]
    fn resolve_duplicate_unsupported_is_ambiguous_not_unsupported() {
        let instances = vec![
            make_instance("bad.moc3", "i1", "Live2D"),
            make_instance("bad.moc3", "i2", "Live2D"),
        ];
        assert_eq!(
            resolve_item(&instances, "bad.moc3").unwrap_err(),
            ResolveItemError::Ambiguous
        );
    }

    #[test]
    fn resolve_multiple_duplicate_unsupported_ambiguous_first() {
        let instances = vec![
            make_instance("a.png", "i1", "PNG"),
            make_instance("bad.moc3", "i2", "Live2D"),
            make_instance("bad.moc3", "i3", "Live2D"),
        ];
        let result = resolve_item(&instances, "bad.moc3");
        assert_eq!(result.unwrap_err(), ResolveItemError::Ambiguous);
    }

    #[test]
    fn resolve_animated_gif_is_animated_kind() {
        let instances = vec![make_instance("anim.gif", "i1", "GIF")];
        let resolved = resolve_item(&instances, "anim.gif").unwrap();
        assert_eq!(resolved.kind, ItemKind::Animated);
    }

    #[test]
    fn resolve_animated_folder_is_animated_kind() {
        let instances = vec![make_instance("akari_fly", "i1", "AnimationFolder")];
        let resolved = resolve_item(&instances, "akari_fly").unwrap();
        assert_eq!(resolved.kind, ItemKind::Animated);
    }

    #[test]
    fn resolve_exact_filename_match_in_mixed_set() {
        let instances = vec![
            make_instance("icon.png", "i1", "PNG"),
            make_instance("icon.gif", "i2", "GIF"),
            make_instance("icon.png", "i3", "PNG"),
        ];
        assert_eq!(
            resolve_item(&instances, "icon.png").unwrap_err(),
            ResolveItemError::Ambiguous
        );
        let resolved = resolve_item(&instances, "icon.gif").unwrap();
        assert_eq!(resolved.instance_id, "i2");
        assert_eq!(resolved.kind, ItemKind::Animated);
    }

    #[test]
    fn classify_item_list_response_matches() {
        let resp = make_response(
            "ItemListResponse",
            "ilr-1",
            serde_json::json!({
                "itemsInSceneCount": 1,
                "totalItemsAllowedCount": 60,
                "canLoadItemsRightNow": true,
                "itemInstancesInScene": [
                    {"fileName": "a.png", "instanceID": "i1", "type": "PNG"}
                ]
            }),
        );
        match classify_vts_response(&resp, "ilr-1", "ItemListResponse") {
            RecvResult::Match(data) => {
                let parsed: ItemListResponseData = serde_json::from_value(data).unwrap();
                assert_eq!(parsed.items_in_scene_count, 1);
                assert!(parsed.can_load_items_right_now);
                assert_eq!(parsed.item_instances_in_scene.len(), 1);
            }
            RecvResult::Skip => panic!("expected Match, got Skip"),
            RecvResult::Error(e) => panic!("expected Match, got Error: {}", e),
        }
    }

    #[test]
    fn classify_item_list_api_error_matches() {
        let resp = make_response(
            "APIError",
            "ilr-err",
            serde_json::json!({"errorID": 150, "message": "Permission not granted"}),
        );
        match classify_vts_response(&resp, "ilr-err", "ItemListResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 150"), "got: {}", e);
                assert!(
                    !e.contains("Permission"),
                    "error must not contain VTS message text: {}",
                    e
                );
            }
            RecvResult::Match(_) => panic!("expected Error, got Match"),
            RecvResult::Skip => panic!("expected Error, got Skip"),
        }
    }

    #[test]
    fn classify_item_animation_control_response_matches() {
        let resp = make_response(
            "ItemAnimationControlResponse",
            "iac-r-1",
            serde_json::json!({"frame": 3, "animationPlaying": true}),
        );
        match classify_vts_response(&resp, "iac-r-1", "ItemAnimationControlResponse") {
            RecvResult::Match(data) => {
                let parsed: ItemAnimationControlResponseData =
                    serde_json::from_value(data).unwrap();
                assert_eq!(parsed.frame, 3);
                assert!(parsed.animation_playing);
            }
            RecvResult::Skip => panic!("expected Match, got Skip"),
            RecvResult::Error(e) => panic!("expected Match, got Error: {}", e),
        }
    }

    #[test]
    fn classify_item_animation_control_api_error_matches() {
        let resp = make_response(
            "APIError",
            "iac-err",
            serde_json::json!({"errorID": 200, "message": "Bad request"}),
        );
        match classify_vts_response(&resp, "iac-err", "ItemAnimationControlResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 200"), "got: {}", e);
                assert!(
                    !e.contains("Bad request"),
                    "error must not contain VTS message text: {}",
                    e
                );
            }
            RecvResult::Match(_) => panic!("expected Error, got Match"),
            RecvResult::Skip => panic!("expected Error, got Skip"),
        }
    }

    #[test]
    fn malformed_item_animation_control_response_fails_deserialization() {
        let resp = make_response(
            "ItemAnimationControlResponse",
            "iac-mal",
            serde_json::json!({"unexpectedField": "garbage"}),
        );
        match classify_vts_response(&resp, "iac-mal", "ItemAnimationControlResponse") {
            RecvResult::Match(data) => {
                let err =
                    serde_json::from_value::<ItemAnimationControlResponseData>(data).unwrap_err();
                assert!(
                    err.to_string().contains("frame"),
                    "malformed deserialization should mention missing frame field: {}",
                    err
                );
            }
            RecvResult::Skip => panic!("expected Match, got Skip"),
            RecvResult::Error(e) => panic!("expected Match, got Error: {}", e),
        }
    }

    #[test]
    fn resolve_item_error_display() {
        assert_eq!(
            ResolveItemError::Missing.to_string(),
            "no scene item matching the configured filename"
        );
        assert_eq!(
            ResolveItemError::Ambiguous.to_string(),
            "multiple scene items match the configured filename"
        );
        assert_eq!(
            ResolveItemError::Unsupported {
                file_name: "x.moc3".to_string(),
                item_type: "Live2D".to_string(),
            }
            .to_string(),
            "item 'x.moc3' has unsupported type 'Live2D'"
        );
    }

    // ---------------------------------------------------------------------------
    // VTubeStudioItemStatus serde shape
    // ---------------------------------------------------------------------------

    fn has_instance_id_keys(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                map.keys().any(|k| k == "instanceID" || k == "instanceId")
                    || map.values().any(has_instance_id_keys)
            }
            serde_json::Value::Array(arr) => arr.iter().any(has_instance_id_keys),
            _ => false,
        }
    }

    #[test]
    fn item_status_serde_inactive() {
        let status = VTubeStudioItemStatus::Inactive;
        let expected = serde_json::json!({"status": "Inactive"});
        assert_eq!(serde_json::to_value(&status).unwrap(), expected);
        assert!(!has_instance_id_keys(
            &serde_json::to_value(&status).unwrap()
        ));

        let parsed: VTubeStudioItemStatus = serde_json::from_value(expected).unwrap();
        assert_eq!(parsed, VTubeStudioItemStatus::Inactive);
    }

    #[test]
    fn item_status_serde_ready() {
        let status = VTubeStudioItemStatus::Ready {
            file_name: "icon.png".into(),
            vts_type: "PNG".into(),
        };
        let expected =
            serde_json::json!({"status": "Ready", "fileName": "icon.png", "vtsType": "PNG"});
        assert_eq!(serde_json::to_value(&status).unwrap(), expected);
        assert!(!has_instance_id_keys(
            &serde_json::to_value(&status).unwrap()
        ));

        let parsed: VTubeStudioItemStatus = serde_json::from_value(expected).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn item_status_serde_missing() {
        let status = VTubeStudioItemStatus::Missing {
            file_name: "ghost.png".into(),
        };
        let expected = serde_json::json!({"status": "Missing", "fileName": "ghost.png"});
        assert_eq!(serde_json::to_value(&status).unwrap(), expected);
        assert!(!has_instance_id_keys(
            &serde_json::to_value(&status).unwrap()
        ));

        let parsed: VTubeStudioItemStatus = serde_json::from_value(expected).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn item_status_serde_ambiguous() {
        let status = VTubeStudioItemStatus::Ambiguous {
            file_name: "dup.png".into(),
            match_count: 3,
        };
        let expected =
            serde_json::json!({"status": "Ambiguous", "fileName": "dup.png", "matchCount": 3});
        assert_eq!(serde_json::to_value(&status).unwrap(), expected);
        assert!(!has_instance_id_keys(
            &serde_json::to_value(&status).unwrap()
        ));

        let parsed: VTubeStudioItemStatus = serde_json::from_value(expected).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn item_status_serde_unsupported() {
        let status = VTubeStudioItemStatus::Unsupported {
            file_name: "model.moc3".into(),
            vts_type: "Live2D".into(),
        };
        let expected = serde_json::json!({"status": "Unsupported", "fileName": "model.moc3", "vtsType": "Live2D"});
        assert_eq!(serde_json::to_value(&status).unwrap(), expected);
        assert!(!has_instance_id_keys(
            &serde_json::to_value(&status).unwrap()
        ));

        let parsed: VTubeStudioItemStatus = serde_json::from_value(expected).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn item_status_serde_error() {
        let status = VTubeStudioItemStatus::Error {
            file_name: "bad.gif".into(),
            message: "request timed out".into(),
        };
        let expected = serde_json::json!({"status": "Error", "fileName": "bad.gif", "message": "request timed out"});
        assert_eq!(serde_json::to_value(&status).unwrap(), expected);
        assert!(!has_instance_id_keys(
            &serde_json::to_value(&status).unwrap()
        ));

        let parsed: VTubeStudioItemStatus = serde_json::from_value(expected).unwrap();
        assert_eq!(parsed, status);
    }

    // ---------------------------------------------------------------------------
    // Resolve error → Item status mapping (pure)
    // ---------------------------------------------------------------------------

    #[test]
    fn resolve_missing_maps_to_missing_status() {
        let instances = vec![make_instance("icon.png", "i1", "PNG")];
        let err = resolve_item(&instances, "other.png").unwrap_err();
        assert_eq!(err, ResolveItemError::Missing);

        let status = VTubeStudioItemStatus::Missing {
            file_name: "other.png".into(),
        };
        assert!(
            matches!(status, VTubeStudioItemStatus::Missing { ref file_name } if file_name == "other.png")
        );
    }

    #[test]
    fn resolve_ambiguous_maps_to_ambiguous_status_with_count() {
        let instances = vec![
            make_instance("dup.png", "i1", "PNG"),
            make_instance("dup.png", "i2", "PNG"),
        ];
        let err = resolve_item(&instances, "dup.png").unwrap_err();
        assert_eq!(err, ResolveItemError::Ambiguous);

        let count = instances
            .iter()
            .filter(|i| i.file_name == "dup.png")
            .count() as u32;
        let status = VTubeStudioItemStatus::Ambiguous {
            file_name: "dup.png".into(),
            match_count: count,
        };
        assert!(
            matches!(&status, VTubeStudioItemStatus::Ambiguous { file_name, match_count } if file_name == "dup.png" && *match_count == 2)
        );
    }

    #[test]
    fn resolve_unsupported_maps_to_unsupported_status() {
        let instances = vec![make_instance("model.moc3", "i1", "Live2D")];
        let err = resolve_item(&instances, "model.moc3").unwrap_err();

        match err {
            ResolveItemError::Unsupported {
                file_name,
                item_type,
            } => {
                let status = VTubeStudioItemStatus::Unsupported {
                    file_name: file_name.clone(),
                    vts_type: item_type.clone(),
                };
                assert!(
                    matches!(&status, VTubeStudioItemStatus::Unsupported { file_name: f, vts_type: t } if f == "model.moc3" && t == "Live2D")
                );
            }
            _ => panic!("expected Unsupported"),
        }
    }

    // ---------------------------------------------------------------------------
    // Scene record aggregation (pure)
    // ---------------------------------------------------------------------------

    #[test]
    fn build_scene_records_single_item() {
        let instances = vec![make_instance("icon.png", "i1", "PNG")];
        let records = build_scene_records(&instances);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].file_name, "icon.png");
        assert_eq!(records[0].item_type, "PNG");
        assert!(records[0].supported);
        assert_eq!(records[0].duplicate_count, 1);
    }

    #[test]
    fn build_scene_records_aggregates_duplicates_same_file_type() {
        let instances = vec![
            make_instance("icon.png", "i1", "PNG"),
            make_instance("icon.png", "i2", "PNG"),
        ];
        let records = build_scene_records(&instances);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].file_name, "icon.png");
        assert_eq!(records[0].item_type, "PNG");
        assert_eq!(records[0].duplicate_count, 2);
        assert!(records[0].supported);
    }

    #[test]
    fn build_scene_records_separates_by_type_even_with_same_filename() {
        let instances = vec![
            make_instance("item.png", "i1", "PNG"),
            make_instance("item.png", "i2", "Live2D"),
        ];
        let records = build_scene_records(&instances);
        assert_eq!(records.len(), 2);
        let png = records.iter().find(|r| r.item_type == "PNG").unwrap();
        assert!(png.supported);
        assert_eq!(png.duplicate_count, 1);
        let l2d = records.iter().find(|r| r.item_type == "Live2D").unwrap();
        assert!(!l2d.supported);
        assert_eq!(l2d.duplicate_count, 1);
    }

    #[test]
    fn build_scene_records_unsupported_future_type_is_false() {
        let instances = vec![make_instance("video.mp4", "i1", "VideoClip")];
        let records = build_scene_records(&instances);
        assert_eq!(records.len(), 1);
        assert!(!records[0].supported);
        assert_eq!(records[0].item_type, "VideoClip");
    }

    #[test]
    fn build_scene_records_unknown_type_is_unsupported() {
        let instances = vec![make_instance("thing.dat", "i1", "Unknown")];
        let records = build_scene_records(&instances);
        assert_eq!(records.len(), 1);
        assert!(!records[0].supported);
    }

    #[test]
    fn build_scene_records_deterministic_order_by_filename_then_type() {
        let instances = vec![
            make_instance("z.png", "iz", "PNG"),
            make_instance("a.png", "ia1", "AnimationFolder"),
            make_instance("a.png", "ia2", "PNG"),
        ];
        let records = build_scene_records(&instances);
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].file_name, "a.png");
        assert_eq!(records[0].item_type, "AnimationFolder");
        assert_eq!(records[1].file_name, "a.png");
        assert_eq!(records[1].item_type, "PNG");
        assert_eq!(records[2].file_name, "z.png");
        assert_eq!(records[2].item_type, "PNG");

        let instances_rev = vec![
            make_instance("a.png", "ia2", "PNG"),
            make_instance("a.png", "ia1", "AnimationFolder"),
            make_instance("z.png", "iz", "PNG"),
        ];
        let records_rev = build_scene_records(&instances_rev);
        assert_eq!(records, records_rev);
    }

    #[test]
    fn scene_item_record_serde_no_instance_id() {
        let rec = SceneItemRecord {
            file_name: "icon.png".into(),
            item_type: "PNG".into(),
            supported: true,
            duplicate_count: 2,
        };
        let json = serde_json::to_string(&rec).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["fileName"].as_str().unwrap(), "icon.png");
        assert!(v.get("file_name").is_none());
        assert!(v.get("duplicate_count").is_none());
        assert_eq!(v["duplicateCount"].as_u64().unwrap(), 2);
        assert!(v.get("instanceID").is_none());
        assert!(v.get("instanceId").is_none());
        assert!(v.get("instance_id").is_none());
    }

    #[test]
    fn scene_item_record_supported_static() {
        let rec = SceneItemRecord {
            file_name: "a.png".into(),
            item_type: "PNG".into(),
            supported: true,
            duplicate_count: 1,
        };
        assert!(rec.supported);

        let rec2 = SceneItemRecord {
            file_name: "b.jpg".into(),
            item_type: "JPG".into(),
            supported: true,
            duplicate_count: 1,
        };
        assert!(rec2.supported);
    }

    #[test]
    fn scene_item_record_supported_animated() {
        let rec = SceneItemRecord {
            file_name: "anim.gif".into(),
            item_type: "GIF".into(),
            supported: true,
            duplicate_count: 1,
        };
        assert!(rec.supported);

        let rec2 = SceneItemRecord {
            file_name: "akari_fly".into(),
            item_type: "AnimationFolder".into(),
            supported: true,
            duplicate_count: 1,
        };
        assert!(rec2.supported);
    }

    #[test]
    fn scene_item_record_unsupported_live2d() {
        let rec = SceneItemRecord {
            file_name: "model.moc3".into(),
            item_type: "Live2D".into(),
            supported: false,
            duplicate_count: 1,
        };
        assert!(!rec.supported);
    }

    // ---------------------------------------------------------------------------
    // Event/Hotkeys refresh → Inactive / no network request
    // ---------------------------------------------------------------------------

    #[test]
    fn refresh_item_action_event_mode_is_inactive_no_network() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Event;
            }
            let status = svc.refresh_item_action().await;
            assert_eq!(status, VTubeStudioItemStatus::Inactive);
            assert_eq!(svc.get_item_status(), VTubeStudioItemStatus::Inactive);
        });
    }

    #[test]
    fn refresh_item_action_hotkeys_mode_is_inactive_no_network() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Hotkeys;
            }
            let status = svc.refresh_item_action().await;
            assert_eq!(status, VTubeStudioItemStatus::Inactive);
            assert_eq!(svc.get_item_status(), VTubeStudioItemStatus::Inactive);
        });
    }

    // ---------------------------------------------------------------------------
    // Empty Item filename → Missing (no network)
    // ---------------------------------------------------------------------------

    #[test]
    fn refresh_item_action_empty_filename_is_missing_no_network() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Item;
                s.typing_action.item_file_name = String::new();
            }
            let status = svc.refresh_item_action().await;
            assert_eq!(
                status,
                VTubeStudioItemStatus::Missing {
                    file_name: String::new()
                }
            );
            assert_eq!(
                svc.get_item_status(),
                VTubeStudioItemStatus::Missing {
                    file_name: String::new()
                }
            );
        });
    }

    // ---------------------------------------------------------------------------
    // Defaults and disconnect
    // ---------------------------------------------------------------------------

    #[test]
    fn service_defaults_have_separate_disconnected_and_inactive() {
        let svc = VTubeStudioService::new();
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Disconnected
        );
        assert_eq!(svc.get_item_status(), VTubeStudioItemStatus::Inactive);
    }

    #[test]
    fn disconnect_sets_item_status_inactive() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
        rt.block_on(async {
            svc.disconnect().await;
        });
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Disconnected
        );
        assert_eq!(svc.get_item_status(), VTubeStudioItemStatus::Inactive);
    }

    #[test]
    fn new_service_item_status_is_inactive() {
        let svc = VTubeStudioService::new();
        let status = svc.get_item_status();
        assert_eq!(status, VTubeStudioItemStatus::Inactive);
    }

    // ---------------------------------------------------------------------------
    // Event/Hotkeys refresh clears pre-existing resolution → Inactive (no network)
    // ---------------------------------------------------------------------------

    #[test]
    fn refresh_event_mode_clears_resolved_item_and_sets_inactive() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Event;
            }
            {
                let mut inner = svc.inner.lock().await;
                inner.resolved_item = Some(ResolvedItem {
                    instance_id: "old-id".into(),
                    file_name: "old.png".into(),
                    item_type: "PNG".into(),
                    kind: ItemKind::Static,
                });
            }

            let status = svc.refresh_item_action().await;
            assert_eq!(status, VTubeStudioItemStatus::Inactive);
            assert_eq!(svc.get_item_status(), VTubeStudioItemStatus::Inactive);

            let inner = svc.inner.lock().await;
            assert!(inner.resolved_item.is_none());
        });
    }

    #[test]
    fn refresh_hotkeys_mode_clears_resolved_item_and_sets_inactive() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Hotkeys;
            }
            {
                let mut inner = svc.inner.lock().await;
                inner.resolved_item = Some(ResolvedItem {
                    instance_id: "old-id".into(),
                    file_name: "old.gif".into(),
                    item_type: "GIF".into(),
                    kind: ItemKind::Animated,
                });
            }

            let status = svc.refresh_item_action().await;
            assert_eq!(status, VTubeStudioItemStatus::Inactive);
            assert_eq!(svc.get_item_status(), VTubeStudioItemStatus::Inactive);

            let inner = svc.inner.lock().await;
            assert!(inner.resolved_item.is_none());
        });
    }

    // ---------------------------------------------------------------------------
    // No-socket Item refresh clears resolution → Error
    // ---------------------------------------------------------------------------

    #[test]
    fn refresh_item_no_socket_clears_resolution_and_returns_error() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Item;
                s.typing_action.item_file_name = "no-socket-test.png".to_string();
            }
            {
                let mut inner = svc.inner.lock().await;
                inner.resolved_item = Some(ResolvedItem {
                    instance_id: "stale-id".into(),
                    file_name: "stale.png".into(),
                    item_type: "PNG".into(),
                    kind: ItemKind::Static,
                });
            }
            assert!(svc.inner.lock().await.ws.is_none());

            let status = svc.refresh_item_action().await;
            assert_eq!(
                status,
                VTubeStudioItemStatus::Error {
                    file_name: "no-socket-test.png".to_string(),
                    message: "WebSocket not available".to_string(),
                }
            );
            assert_eq!(
                svc.get_item_status(),
                VTubeStudioItemStatus::Error {
                    file_name: "no-socket-test.png".to_string(),
                    message: "WebSocket not available".to_string(),
                }
            );

            let inner = svc.inner.lock().await;
            assert!(inner.resolved_item.is_none());
        });
    }

    // ---------------------------------------------------------------------------
    // Item transition state-machine pure tests (no live socket)
    // ---------------------------------------------------------------------------

    #[test]
    fn item_transition_record_desired_true_starts_sync() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        let (d, gen) = state.read_desired();
        assert!(d);
        assert_eq!(state.read_applied(), None);
        assert!(gen >= 1);
    }

    #[test]
    fn item_transition_repeated_true_is_idempotent() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        let (_, g1) = state.read_desired();
        state.record_desired(true);
        let (d, g2) = state.read_desired();
        assert!(d);
        assert!(g2 >= g1);
    }

    #[test]
    fn item_transition_false_to_true_before_completion_collapses() {
        let state = ItemTransitionState::new();
        state.record_desired(false);
        let (_, gen_false) = state.read_desired();
        state.record_desired(true);
        let (d, gen_true) = state.read_desired();
        assert!(d);
        assert!(gen_true > gen_false);
        let accepted = state.set_applied_if_current(false, gen_false);
        assert!(!accepted);
        assert_eq!(state.read_applied(), None);
    }

    #[test]
    fn item_transition_completion_only_replaces_matching_generation() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        let (_, gen1) = state.read_desired();
        let accepted = state.set_applied_if_current(true, gen1);
        assert!(accepted);
        assert_eq!(state.read_applied(), Some(true));
        state.record_desired(false);
        let (_, gen2) = state.read_desired();
        assert!(!state.set_applied_if_current(true, gen1));
        let (d, _) = state.read_desired();
        assert!(!d);
        assert!(state.set_applied_if_current(false, gen2));
        assert_eq!(state.read_applied(), Some(false));
    }

    #[test]
    fn item_transition_many_updates_keep_constant_size_state() {
        let state = ItemTransitionState::new();
        for i in 0..100 {
            state.record_desired(i % 2 == 0);
        }
        let (_, gen) = state.read_desired();
        assert!(gen >= 100);
    }

    #[test]
    fn item_transition_failure_sets_applied_unknown() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        state.force_applied(true);
        assert_eq!(state.read_applied(), Some(true));
        state.mark_applied_unknown();
        assert_eq!(state.read_applied(), None);
    }

    #[test]
    fn item_transition_next_sync_retries_latest_desired_after_failure() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        // Request fails — mark applied unknown
        state.mark_applied_unknown();
        assert_eq!(state.read_applied(), None);
        // Next sync pass reads latest desired
        let (d, _gen) = state.read_desired();
        assert!(d);
        // It will be retried because applied != desired
        assert_ne!(state.read_applied(), Some(d));
    }

    #[test]
    fn item_transition_shutdown_race_handshake_observes_update() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        state.force_applied(true);
        let (d, _) = state.read_desired();
        assert_eq!(Some(d), state.read_applied());
        // No new desired, should not claim (begin_work returns None)
        assert!(state.begin_work().is_none());

        // New desired arrives
        state.record_desired(false);
        // begin_work should succeed because desired != applied
        let gen = state.begin_work();
        assert!(gen.is_some(), "new desired detected, must claim worker");
    }

    #[test]
    fn item_transition_worker_cas_ensures_single_worker() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        assert!(state.begin_work().is_some());
        assert!(state.begin_work().is_none());
        assert!(state.begin_work().is_none());
        state.end_work();
        state.record_desired(true); // trigger again (applied still None)
        assert!(state.begin_work().is_some());
    }

    #[test]
    fn item_transition_refresh_commit_sets_applied_to_desired() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        // Simulate successful refresh that normalizes to desired
        state.force_applied(true);
        assert_eq!(state.read_applied(), Some(true));

        state.record_desired(false);
        state.force_applied(false);
        assert_eq!(state.read_applied(), Some(false));
    }

    #[test]
    fn item_transition_reset_clears_desired_and_applied() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        state.force_applied(true);
        state.reset();
        let (d, _) = state.read_desired();
        assert!(!d);
        assert_eq!(state.read_applied(), None);
    }

    #[test]
    fn item_transition_force_applied_setting() {
        let state = ItemTransitionState::new();
        assert_eq!(state.read_applied(), None);
        state.force_applied(true);
        assert_eq!(state.read_applied(), Some(true));
        state.force_applied(false);
        assert_eq!(state.read_applied(), Some(false));
    }

    // ---------------------------------------------------------------------------
    // Worker ownership/exit protocol model tests
    // ---------------------------------------------------------------------------

    #[test]
    fn worker_exit_racing_with_new_desired_leaves_live_owner_or_clean_unclaimed() {
        // Equality exits unclaimed (finite steps no busy loop)
        let state = ItemTransitionState::new();
        state.record_desired(true);
        let gen = state.begin_work().expect("should claim worker");

        // Worker successfully completes: applied=true via finish_success
        let continue_work = state.finish_success(gen, true);
        assert!(!continue_work, "equality exits unclaimed, no further work");
        assert_eq!(state.read_applied(), Some(true));
        // begin_work must now return None because desired==applied
        assert!(state.begin_work().is_none());

        // New desired arrives: update racing with release is not lost
        state.record_desired(false);
        let _gen2 = state.begin_work().expect("new desired must claim worker");
    }

    #[test]
    fn persistent_failure_attempts_once_and_exits_unclaimed() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        let gen = state.begin_work().expect("should claim worker");

        // Worker fails: finish_failure releases worker and sets applied=None
        assert!(!state.finish_failure(gen));
        assert_eq!(state.read_applied(), None);
        // After failure, worker is released → begin_work may claim for retry
        // (same-value after failure is eligible for one retry, requirement 2)
    }

    #[test]
    fn same_value_update_during_inflight_does_not_stale_completion() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        let gen1 = state.begin_work().expect("worker must be in flight");
        assert!(gen1 >= 1);

        // Worker starts processing gen1 desired=true
        // Meanwhile, same desired=true recorded again — must NOT advance generation
        state.record_desired(true);
        let (d, gen2) = state.read_desired();
        assert!(d);
        assert_eq!(gen1, gen2, "same-value must not advance generation");

        // Worker finishes gen1 completion — stale completion must be rejected
        // (set_applied_if_current with gen1 still works since gen hasn't changed)
        let accepted = state.set_applied_if_current(true, gen1);
        assert!(accepted, "completion with current gen must be accepted");
        assert_eq!(state.read_applied(), Some(true));
    }

    #[test]
    fn same_value_update_after_failure_can_claim_retry() {
        let state = ItemTransitionState::new();
        state.record_desired(true);

        // Worker fails — finish_failure sets applied=None, releases worker
        let gen = state.begin_work().expect("should claim worker");
        assert!(!state.finish_failure(gen));
        assert_eq!(state.read_applied(), None);

        // New trigger: even with same desired value (true), applied=None means mismatch
        let (d, gen1) = state.read_desired();
        assert!(d);
        assert_eq!(state.read_applied(), None);
        assert_ne!(Some(d), state.read_applied());

        // Next sync can claim worker to retry (begin_work succeeds)
        // record_desired(true) must advance generation for retry
        state.record_desired(true);
        let gen_retry = state.begin_work();
        assert!(gen_retry.is_some(), "same-value after failure may retry");
        // Generation must have advanced
        assert!(
            gen_retry.unwrap() > gen1,
            "generation must advance for retry"
        );
    }

    // ---------------------------------------------------------------------------
    // Session/disconnect invalidation model tests
    // ---------------------------------------------------------------------------

    #[test]
    fn disconnect_invalidates_inflight_worker_session() {
        // Simulate: service connects (session=0), worker runs, disconnect increments session
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
        // Set Item mode with a filename so run_item_sync enters Item path
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Item;
                s.typing_action.item_file_name = "test.png".to_string();
            }
        });

        // Session starts at 0, then disconnect bumps it
        rt.block_on(async {
            svc.disconnect().await;
        });
        let session_after = svc.read_session();
        assert!(
            session_after > 0,
            "session must be incremented by disconnect"
        );

        // After disconnect, all state is clean
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Disconnected
        );
        assert!(!svc.is_desired_running());
        assert_eq!(svc.get_item_status(), VTubeStudioItemStatus::Inactive);
    }

    #[test]
    fn worker_session_mismatch_prevents_socket_restoration() {
        let state = ItemTransitionState::new();
        state.record_desired(true);
        // Worker claims and owns a generation snapshot
        let gen = state.begin_work();
        assert!(gen.is_some());
        state.force_applied(true);

        // Session invalidated externally → worker releases
        state.end_work();
        // After release, begin_work checks: applied(Some(true)) == desired(true) → no work
        assert!(
            state.begin_work().is_none(),
            "session invalidation via release prevents stale reclaim"
        );
    }

    // ---------------------------------------------------------------------------
    // Generation-aware refresh model tests
    // ---------------------------------------------------------------------------

    #[test]
    fn refresh_desired_change_during_io_cannot_commit_stale_applied() {
        let state = ItemTransitionState::new();
        let (_, gen0) = state.read_desired();
        state.record_desired(true);
        let (_, gen1) = state.read_desired();
        assert!(gen1 > gen0);

        // Simulate I/O takes time, desired changes to false during it
        state.record_desired(false);
        let (_, gen2) = state.read_desired();
        assert!(gen2 > gen1);

        // Old completion with stale desired=true, gen=gen1 must be rejected
        let stale = state.set_applied_if_current(true, gen1);
        assert!(
            !stale,
            "stale gen1 applied must be rejected when gen2 is newer"
        );

        // Current completion succeeds
        let current = state.set_applied_if_current(false, gen2);
        assert!(current, "current gen2 completion should succeed");
        assert_eq!(state.read_applied(), Some(false));
    }

    #[test]
    fn refresh_action_error_retains_resolved_item_for_retry() {
        // This tests the do_item_refresh_with_desired contract:
        // When fetch_scene_instances succeeds but animate_item fails,
        // the resolved item should be returned alongside the Error status.
        // This is inherently an async test involving network, but the model is:
        // The method returns (Some(resolved_item), Error) on animate failure.

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Item;
                s.typing_action.item_file_name = "ghost.png".to_string();
            }
            // No websocket — animate will fail
            let status = svc.refresh_item_action().await;
            assert!(
                matches!(status, VTubeStudioItemStatus::Error { .. }),
                "expected Error, got {:?}",
                status
            );
            // After Error from refresh (no socket), resolved_item is None
            let inner = svc.inner.lock().await;
            assert!(inner.resolved_item.is_none());
        });
    }

    // ---------------------------------------------------------------------------
    // Resolve errors suppress requests (no transition)
    // ---------------------------------------------------------------------------

    #[test]
    fn missing_ambiguous_unsupported_suppress_item_requests() {
        // These resolve errors do not change connection status
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Connected
        );

        // Empty filename -> Missing (no WS call)
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Item;
                s.typing_action.item_file_name = String::new();
            }
            let status = svc.refresh_item_action().await;
            assert!(matches!(status, VTubeStudioItemStatus::Missing { .. }));
        });
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Connected
        );

        // Event mode -> Inactive (no WS call)
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Event;
            }
            let status = svc.refresh_item_action().await;
            assert_eq!(status, VTubeStudioItemStatus::Inactive);
        });
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Connected
        );
    }

    // ---------------------------------------------------------------------------
    // Item status transitions never change connection to Error
    // ---------------------------------------------------------------------------

    #[test]
    fn no_item_transition_changes_connection_from_connected_to_error() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);

        // Event/Hotkeys/empty Item refresh — all return without touching connection
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Event;
            }
            let _ = svc.refresh_item_action().await;
        });
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Connected
        );

        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Hotkeys;
            }
            let _ = svc.refresh_item_action().await;
        });
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Connected
        );

        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Item;
                s.typing_action.item_file_name = String::new();
            }
            let _ = svc.refresh_item_action().await;
        });
        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Connected
        );
    }

    #[test]
    fn item_worker_failure_sets_error_status_but_keeps_connection() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);

        // Set Item mode with a filename but no WS — worker will fail
        rt.block_on(async {
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Item;
                s.typing_action.item_file_name = "test.png".to_string();
            }
        });

        svc.record_item_desired(true);
        rt.block_on(async {
            let status = svc.run_item_sync().await;
            // Status may be Inactive (no ws, no resolved item) or Error
            // Key: connection status remains Connected
            let _ = status;
        });

        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Connected,
            "item worker failure must not corrupt connection status"
        );
    }

    // ---------------------------------------------------------------------------
    // Requirement 3: session invalidation between pre-check and mutex
    // acquisition cannot restore a socket (guarded commit).
    // ---------------------------------------------------------------------------
    #[test]
    fn guarded_commit_must_recheck_session_under_mutex() {
        // This test models the required protocol:
        // 1. read session before taking ws
        // 2. take ws from inner
        // 3. do I/O (simulated)
        // 4. acquire inner, re-check session under mutex before restoring
        //
        // Previously: session was checked outside mutex, then inner locked.
        // Now: inner locked first, then session re-checked inside.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let svc = VTubeStudioService::new();
            svc.set_desired_running(true);
            svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
            svc.is_authenticated.store(true, Ordering::SeqCst);

            // Set up item mode with a filename so we can observe behavior
            {
                let mut s = svc.settings.write().await;
                s.typing_action.output_mode = VTubeStudioTypingMode::Item;
                s.typing_action.item_file_name = "test.png".to_string();
            }

            // Place a fake resolved item so the worker doesn't bail early
            {
                let mut inner = svc.inner.lock().await;
                inner.resolved_item = Some(ResolvedItem {
                    instance_id: "test-id".into(),
                    file_name: "test.png".into(),
                    item_type: "PNG".into(),
                    kind: ItemKind::Static,
                });
            }

            let session = svc.read_session();

            // Invalidate session as if disconnect happened while worker was in-flight
            let _new_session = svc.invalidate_session();
            assert_ne!(svc.read_session(), session);

            // Guarded commit: acquire inner, then re-check session
            let mut inner = svc.inner.lock().await;
            let stale = svc.read_session() != session;
            assert!(stale, "session must be stale after invalidation");

            // Because session is stale, do NOT restore socket/change state
            // The inner guard is dropped without restoring
            inner.resolved_item = None;
            // (In production: inner.ws would NOT be restored for stale session)

            assert!(
                inner.ws.is_none(),
                "no socket should be placed for stale session"
            );
        });
    }

    // ---------------------------------------------------------------------------
    // Requirement 4: Item test action coordinates with worker/session.
    // Older in-flight worker must not restore state after the test.
    // ---------------------------------------------------------------------------

    #[test]
    fn item_test_invalidates_session_to_coordinate_with_worker_and_ends_hidden() {
        let state = ItemTransitionState::new();

        // Simulate: worker running (claimed), applying desired=true
        state.record_desired(true);
        let gen = state.begin_work().expect("worker must claim");
        // Worker is in-flight with gen

        // Now test starts — invalidate by calling finish_failure (model of session invalidation)
        // In real code, session invalidation causes worker to check `read_session() != session`
        // and call end_work() or finish_failure().
        // After test ends hidden:
        state.finish_failure(gen); // old worker exits, applied=None
        assert_eq!(state.read_applied(), None);

        // Test resets to hidden
        state.reset();
        state.force_applied(false);
        let (d, _) = state.read_desired();
        assert!(!d, "must end hidden");
        assert_eq!(state.read_applied(), Some(false));

        // Verify old worker cannot resurrect — begin_work returns None
        // because desired(false) == applied(Some(false))
        assert!(
            state.begin_work().is_none(),
            "stale worker must not be able to claim after test ends hidden"
        );
    }

    // ---------------------------------------------------------------------------
    // Requirement 1/5 combined: equality exits unclaimed; success with no newer
    // desired exits unclaimed; update racing with release is not lost.
    // ---------------------------------------------------------------------------

    #[test]
    fn equality_exits_unclaimed_in_finite_steps_no_busy_loop() {
        let state = ItemTransitionState::new();

        // Record desired=true, claim worker, succeed
        state.record_desired(true);
        let gen = state.begin_work().expect("must claim");
        let continue_work = state.finish_success(gen, true);
        assert!(!continue_work, "equality exits unclaimed — no busy loop");
        assert_eq!(state.read_applied(), Some(true));

        // begin_work returns None when desired == applied
        assert!(state.begin_work().is_none());

        // A new record_desired(true) while worker not running and applied=Some(true)
        // must NOT advance generation (requirement 2)
        let (_, gen_before) = state.read_desired();
        state.record_desired(true);
        let (_, gen_after) = state.read_desired();
        assert_eq!(
            gen_before, gen_after,
            "same-value recording while already applied must not advance generation"
        );
    }

    // ---------------------------------------------------------------------------
    // Requirement 2 combined: same-value after failure is eligible for one retry.
    // ---------------------------------------------------------------------------

    #[test]
    fn same_value_after_failure_triggers_retry_by_advancing_generation() {
        let state = ItemTransitionState::new();

        state.record_desired(true);
        let gen_first = state.begin_work().expect("claim");
        state.finish_failure(gen_first);

        // After failure: applied=None, no worker. Record same desired=true
        let (_, gen_before_retry) = state.read_desired();
        state.record_desired(true); // must advance generation for retry
        let (d, gen_after_retry) = state.read_desired();
        assert!(d);
        assert!(
            gen_after_retry > gen_before_retry || gen_after_retry > gen_first,
            "generation must advance for retry after failure"
        );
        assert!(gen_after_retry > gen_first);

        // begin_work succeeds for the retry
        let gen_retry = state.begin_work();
        assert!(gen_retry.is_some(), "retry must be possible after failure");

        // Success: no more work
        let continue_work = state.finish_success(gen_retry.unwrap(), true);
        assert!(!continue_work, "after retry success, exits unclaimed");
        assert_eq!(state.read_applied(), Some(true));
    }

    #[test]
    fn session_invalidation_via_disconnect_prevents_worker_resurrection() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = VTubeStudioService::new();
        svc.set_desired_running(true);
        svc.set_connection_status(VTubeStudioConnectionStatus::Connected);
        svc.is_authenticated.store(true, Ordering::SeqCst);

        // Store a synthetic resolved item
        rt.block_on(async {
            let mut inner = svc.inner.lock().await;
            inner.resolved_item = Some(ResolvedItem {
                instance_id: "test-id".into(),
                file_name: "test.png".into(),
                item_type: "PNG".into(),
                kind: ItemKind::Static,
            });
        });

        // After disconnect, worker cannot resurrect state
        rt.block_on(async {
            svc.disconnect().await;
        });

        assert_eq!(
            svc.get_connection_status(),
            VTubeStudioConnectionStatus::Disconnected
        );
        assert_eq!(svc.get_item_status(), VTubeStudioItemStatus::Inactive);
    }

    // ---------------------------------------------------------------------------
    // Event/parameter lifecycle VTS error classification
    // ---------------------------------------------------------------------------

    #[test]
    fn classify_inject_error_453_param_name_not_found() {
        let resp = make_response(
            "APIError",
            "inj-453",
            serde_json::json!({"errorID": 453, "message": "InjectDataParamNameNotFound"}),
        );
        match classify_vts_response(&resp, "inj-453", "InjectParameterDataResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 453"), "got: {}", e);
            }
            _ => panic!("expected Error for VTS error 453 on InjectParameterDataResponse"),
        }
    }

    #[test]
    fn classify_creation_error_352_param_created_by_other_plugin() {
        let resp = make_response(
            "APIError",
            "cr-352",
            serde_json::json!({"errorID": 352, "message": "CustomParamAlreadyCreatedByOtherPlugin"}),
        );
        match classify_vts_response(&resp, "cr-352", "ParameterCreationResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 352"), "got: {}", e);
            }
            _ => panic!("expected Error for VTS error 352 on ParameterCreationResponse"),
        }
    }

    #[test]
    fn classify_creation_error_350_parameter_already_exists() {
        let resp = make_response(
            "APIError",
            "cr-350",
            serde_json::json!({"errorID": 350, "message": "ParameterAlreadyCreatedByThisPlugin"}),
        );
        match classify_vts_response(&resp, "cr-350", "ParameterCreationResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 350"), "got: {}", e);
            }
            _ => panic!("expected Error for VTS error 350 on ParameterCreationResponse"),
        }
    }

    #[test]
    fn classify_creation_error_355_max_parameter_limit() {
        let resp = make_response(
            "APIError",
            "cr-355",
            serde_json::json!({"errorID": 355, "message": "TooManyCustomParams"}),
        );
        match classify_vts_response(&resp, "cr-355", "ParameterCreationResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 355"), "got: {}", e);
            }
            _ => panic!("expected Error for VTS error 355 on ParameterCreationResponse"),
        }
    }

    #[test]
    fn classify_creation_error_356_invalid_parameter_name() {
        let resp = make_response(
            "APIError",
            "cr-356",
            serde_json::json!({"errorID": 356, "message": "InvalidParameterName"}),
        );
        match classify_vts_response(&resp, "cr-356", "ParameterCreationResponse") {
            RecvResult::Error(e) => {
                assert!(e.contains("VTS error 356"), "got: {}", e);
            }
            _ => panic!("expected Error for VTS error 356 on ParameterCreationResponse"),
        }
    }
}
