//! OCR service: desired settings, runtime lifecycle status, and conditional
//! global hotkey registration.
//!
//! Mirrors the Input Server feature seam: persisted `OcrSettings` express only
//! desired state, while the actual runtime status (`OcrStatus`) is tracked
//! here and published to the frontend through a pluggable emit seam.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::{RgbImage, RgbaImage};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::capture::{
    clamp_selection, crop_to_rgb, normalize_selection, validate_selection, CaptureError,
    VirtualScreenGeometry,
};
use super::packs::scan_ocr_packs;
use super::runtime::{OcrResult, OcrRuntime};
use super::settings::OcrSettings;
use crate::config::Hotkey;

/// Runtime lifecycle status of the OCR runtime.
///
/// Uses the same externally tagged camelCase wire shape as
/// [`crate::input_server::InputServerStatus`]:
/// `{"state": "disabled" | "starting" | "ready" | "error", "message"?: ...}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum OcrStatus {
    Disabled,
    /// Published before scanning/loading; replaced with `Ready` only after the
    /// runtime is built and the capture shortcut is registered.
    Starting,
    Ready,
    /// The one-shot capture session is open and the selection overlay is
    /// visible; no recognition runs until a valid selection is submitted.
    SelectingArea,
    /// The selection was submitted and recognition is running on the stored
    /// frame; the overlay is already hidden.
    Recognizing,
    Error {
        message: String,
    },
}

/// Lifecycle delta to apply after an OCR settings save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrTransition {
    Noop,
    Start,
    Stop,
    /// Model changed while already enabled: shut down and start again.
    Restart,
    /// Capture binding changed while `Ready`: re-register with rollback.
    ReregisterHotkey,
    /// Unchanged settings while the runtime is in `Error`: retry the start.
    Retry,
}

/// Decide the lifecycle delta for an OCR settings save.
///
/// Pure and free of any shortcut-plugin or ONNX dependency. `old_hotkey` is the
/// binding the service currently has registered (if any); `new_hotkey` is the
/// binding from the current hotkey settings.
pub fn decide_transition(
    old: &OcrSettings,
    new: &OcrSettings,
    old_hotkey: Option<&Hotkey>,
    new_hotkey: Option<&Hotkey>,
    status: &OcrStatus,
) -> OcrTransition {
    match (old.enabled, new.enabled) {
        (false, true) => OcrTransition::Start,
        (true, false) => OcrTransition::Stop,
        (true, true) => {
            if old.model_id != new.model_id {
                OcrTransition::Restart
            } else if matches!(status, OcrStatus::Ready) && old_hotkey != new_hotkey {
                OcrTransition::ReregisterHotkey
            } else if matches!(status, OcrStatus::Error { .. }) {
                OcrTransition::Retry
            } else {
                OcrTransition::Noop
            }
        }
        (false, false) => OcrTransition::Noop,
    }
}

/// Result of [`OcrService::begin_capture_session`].
#[derive(Debug)]
pub enum BeginOutcome {
    /// The frame is saved, `SelectingArea` is published and the show-overlay
    /// callback has already run.
    Started,
    /// The status was not `Ready`; nothing was captured or shown.
    Ignored,
    /// The injected capture failed; no overlay was shown and status stays
    /// `Ready`.
    CaptureFailed(CaptureError),
    /// The frame was saved and `SelectingArea` was published, but the
    /// show-overlay callback reported a missing/broken overlay. The session is
    /// cleared and status is back to `Ready`.
    OverlayFailed(String),
}

/// Result of [`OcrService::finish_selection`].
#[derive(Debug, Clone, PartialEq)]
pub enum FinishOutcome {
    /// The selection was empty or smaller than [`capture::MIN_SELECTION_SIDE`].
    /// The session is kept and the status stays `SelectingArea` so the user can
    /// drag again.
    TooSmall,
    /// Recognition finished with the given result; status is back to `Ready`.
    Recognized(OcrResult),
    /// Recognition failed with a safe reason; status is back to `Ready`.
    RecognizeFailed(String),
    /// No active capture session; a no-op.
    NoSession,
}

/// A single in-flight one-shot capture.
///
/// Owns the virtual-screen geometry and the full frame saved BEFORE the
/// selection overlay was shown, so cropping never contains overlay pixels.
struct CaptureSession {
    geometry: VirtualScreenGeometry,
    frame: Arc<RgbaImage>,
}

/// Map overlay-local physical corners into virtual-screen coordinates.
///
/// The selection overlay sits with its client-area origin on the stored
/// geometry origin, so a local corner `(lx, ly)` denotes the virtual-screen
/// pixel `(origin.x + lx, origin.y + ly)`. Addition is checked: a corner that
/// would overflow `i32` yields `None` so the caller can keep the retryable
/// selection instead of panicking.
fn translate_by_session_origin(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    origin: (i32, i32),
) -> Option<(i32, i32, i32, i32)> {
    let (ox, oy) = origin;
    Some((
        x1.checked_add(ox)?,
        y1.checked_add(oy)?,
        x2.checked_add(ox)?,
        y2.checked_add(oy)?,
    ))
}

/// Backend-owned domain state for the one-shot screen OCR feature.
///
/// Owns the desired settings snapshot, the runtime lifecycle status, the
/// resolved `OcrRuntime` (if `Ready`) and the currently registered capture
/// shortcut. The packs root is injectable so tests can point at a temp dir.
pub struct OcrService {
    /// Desired settings snapshot, updated by the save command.
    pub settings: Arc<tokio::sync::RwLock<OcrSettings>>,
    status: Arc<Mutex<OcrStatus>>,
    packs_root: PathBuf,
    runtime: tokio::sync::Mutex<Option<OcrRuntime>>,
    registered_hotkey: Mutex<Option<Hotkey>>,
    /// The single in-flight capture session (`SelectingArea`/`Recognizing`), if
    /// any. Async mutex so the frame is never read while another task mutates
    /// the session; guards are never held across the status/settings locks.
    session: tokio::sync::Mutex<Option<CaptureSession>>,
    /// Serialises transitions so only one start/stop runs at a time. Held across
    /// the async ONNX load; a concurrent stop waits instead of deadlocking.
    transition_lock: tokio::sync::Mutex<()>,
}

impl OcrService {
    pub fn new() -> Self {
        let packs_root = dirs::config_dir()
            .map(|dir| dir.join("ttsbard"))
            .unwrap_or_default();
        Self::with_packs_root(packs_root)
    }

    pub fn with_packs_root(packs_root: PathBuf) -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(OcrSettings::default())),
            status: Arc::new(Mutex::new(OcrStatus::Disabled)),
            packs_root,
            runtime: tokio::sync::Mutex::new(None),
            registered_hotkey: Mutex::new(None),
            session: tokio::sync::Mutex::new(None),
            transition_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn packs_root(&self) -> &Path {
        &self.packs_root
    }

    pub fn status(&self) -> OcrStatus {
        self.status.lock().clone()
    }

    /// Replace the runtime status, returning whether it actually changed.
    pub fn set_status(&self, status: OcrStatus) -> bool {
        let mut guard = self.status.lock();
        if *guard == status {
            return false;
        }
        *guard = status;
        true
    }

    /// Set the runtime status and invoke `emit` with the new value only when it
    /// actually changed.
    ///
    /// The emitter is a plain callback so this helper stays testable without a
    /// live Tauri app; the command layer forwards it to
    /// `app_handle.emit("ocr-status-changed", status)`.
    pub fn publish_status<E>(&self, status: OcrStatus, emit: E)
    where
        E: FnOnce(&OcrStatus),
    {
        if self.set_status(status.clone()) {
            emit(&status);
        }
    }

    /// Snapshot of the capture binding currently registered (if any).
    pub fn registered_hotkey(&self) -> Option<Hotkey> {
        self.registered_hotkey.lock().clone()
    }

    /// True while the OCR runtime is live and the capture shortcut is held
    /// (the feature is enabled), regardless of the current one-shot session
    /// phase (`Ready`, `SelectingArea` or `Recognizing`).
    ///
    /// The sync `registered_hotkey` slot is populated exactly when the async
    /// runtime slot is, so this mirrors a non-empty runtime slot without an
    /// async lock.
    pub fn is_runtime_active(&self) -> bool {
        self.registered_hotkey.lock().is_some()
    }

    /// The runtime storage slot.
    ///
    /// Crate-private so the OCR command layer can run recognition against the
    /// stored runtime without going through Tauri. The guard must never be held
    /// across the settings/status locks or another async mutex.
    pub(crate) fn runtime_slot(&self) -> &tokio::sync::Mutex<Option<OcrRuntime>> {
        &self.runtime
    }

    /// Start the OCR runtime: scan packs, resolve the selected model, build the
    /// runtime and register the capture shortcut. Any failure lands in a
    /// terminal `Error` status.
    pub async fn start<E, R>(&self, hotkey: &Hotkey, mut emit: E, register: R)
    where
        E: FnMut(&OcrStatus),
        R: FnOnce(&Hotkey) -> Result<(), String>,
    {
        let _guard = self.transition_lock.lock().await;

        // A concurrent or already-complete start is a no-op.
        if matches!(self.status(), OcrStatus::Ready | OcrStatus::Starting) {
            return;
        }

        self.publish_status(OcrStatus::Starting, &mut emit);

        let model_id = self.settings.read().await.model_id.clone();

        let Some(model_id) = model_id else {
            self.publish_status(
                OcrStatus::Error {
                    message: "No OCR model selected".to_string(),
                },
                &mut emit,
            );
            return;
        };

        let packs = scan_ocr_packs(&self.packs_root);
        let Some(descriptor) = packs.into_iter().find(|p| p.id == model_id) else {
            self.publish_status(
                OcrStatus::Error {
                    message: format!("OCR model pack not found: {model_id}"),
                },
                &mut emit,
            );
            return;
        };

        let runtime = match OcrRuntime::new(&descriptor).await {
            Ok(runtime) => runtime,
            Err(error) => {
                self.publish_status(
                    OcrStatus::Error {
                        message: error.to_string(),
                    },
                    &mut emit,
                );
                return;
            }
        };

        if let Err(message) = register(hotkey) {
            // A failed registration must not leak a live runtime.
            let _ = runtime.shutdown().await;
            self.publish_status(OcrStatus::Error { message }, &mut emit);
            return;
        }

        *self.runtime.lock().await = Some(runtime);
        *self.registered_hotkey.lock() = Some(hotkey.clone());

        self.publish_status(OcrStatus::Ready, &mut emit);
    }

    /// Stop the OCR runtime: release the shortcut, shut the runtime down, drop
    /// any in-flight capture session, then report `Disabled`. A failed shutdown
    /// still lands in `Disabled` but is logged.
    pub async fn stop<E, U>(&self, emit: E, unregister: U)
    where
        E: FnMut(&OcrStatus),
        U: Fn(&Hotkey) -> Result<(), String>,
    {
        let _guard = self.transition_lock.lock().await;
        self.stop_locked(emit, unregister).await;
    }

    /// Stop the runtime for app shutdown without blocking on an in-flight
    /// recognition.
    ///
    /// Uses `try_lock` on the transition lock: when a running session holds it
    /// (recognition in flight, up to 15 s), the stop is detached to a
    /// background task and this returns immediately, so app exit is not delayed
    /// by OCR — the process exit reclaims the runtime thread. When the lock is
    /// free, the stop runs inline.
    pub async fn stop_for_shutdown<E, U>(self: Arc<Self>, emit: E, unregister: U)
    where
        E: FnMut(&OcrStatus) + Send + 'static,
        U: Fn(&Hotkey) -> Result<(), String> + Send + 'static,
    {
        match self.transition_lock.try_lock() {
            Ok(_guard) => {
                self.stop_locked(emit, unregister).await;
            }
            Err(_) => {
                let this = Arc::clone(&self);
                tokio::spawn(async move {
                    this.stop(emit, unregister).await;
                });
            }
        }
    }

    async fn stop_locked<E, U>(&self, mut emit: E, unregister: U)
    where
        E: FnMut(&OcrStatus),
        U: Fn(&Hotkey) -> Result<(), String>,
    {
        let runtime = self.runtime.lock().await.take();
        let hotkey = self.registered_hotkey.lock().take();
        // Drop any in-flight capture session so a stale overlay submit/cancel
        // can never resurrect `Ready` from the disabled state.
        self.session.lock().await.take();

        if let Some(hotkey) = &hotkey {
            if let Err(error) = unregister(hotkey) {
                tracing::warn!(error = %error, "Failed to unregister OCR hotkey during stop");
            }
        }

        if let Some(runtime) = runtime {
            if let Err(error) = runtime.shutdown().await {
                tracing::warn!(error = %error, "OCR runtime shutdown failed");
            }
        }

        self.publish_status(OcrStatus::Disabled, &mut emit);
    }

    /// Re-register the capture shortcut for a changed binding while the OCR
    /// feature is enabled (`Ready`, `SelectingArea` or `Recognizing`).
    ///
    /// The new binding is registered first and the old one released only after
    /// that succeeds. On registration failure the old binding stays active and
    /// `Error` is surfaced, keeping the persisted setting and the runtime in
    /// agreement (roadmap «Настройки и runtime truth»).
    pub async fn reregister_hotkey<E, R, U>(
        &self,
        new_hotkey: &Hotkey,
        mut emit: E,
        register: R,
        unregister: U,
    ) where
        E: FnMut(&OcrStatus),
        R: FnOnce(&Hotkey) -> Result<(), String>,
        U: Fn(&Hotkey) -> Result<(), String>,
    {
        let _guard = self.transition_lock.lock().await;

        // A rebind is valid while the feature is enabled, including during an
        // active one-shot session (`SelectingArea`/`Recognizing`): the new
        // binding is registered and the old released without disturbing the
        // stored frame. Any other status (disabled, starting, error) is a no-op.
        match self.status() {
            OcrStatus::Ready | OcrStatus::SelectingArea | OcrStatus::Recognizing => {}
            _ => return,
        }

        let old_hotkey = self.registered_hotkey.lock().clone();
        if old_hotkey.as_ref() == Some(new_hotkey) {
            return;
        }

        if let Err(message) = register(new_hotkey) {
            self.publish_status(OcrStatus::Error { message }, &mut emit);
            return;
        }

        if let Some(old) = &old_hotkey {
            if let Err(error) = unregister(old) {
                tracing::warn!(
                    error = %error,
                    "Failed to unregister old OCR hotkey after rebind"
                );
            }
        }

        *self.registered_hotkey.lock() = Some(new_hotkey.clone());
    }

    /// Begin the one-shot capture session from `Ready`.
    ///
    /// The transition lock is the serialization boundary: exactly one session
    /// can exist, and it can only start from `Ready` (a re-entry while
    /// `SelectingArea`/`Recognizing` — or any other non-`Ready` status — is
    /// ignored). The frame is captured THROUGH the injected closure on Tokio's
    /// blocking pool — the production closure blocks on `xcap` and must never
    /// run on a Tokio worker — and stored BEFORE the show-overlay callback
    /// runs, so the overlay never appears in OCR input. A panicking capture
    /// closure surfaces as a safe `CaptureFailed`, never a panic payload. All
    /// work stays Tauri-free; the command layer supplies the capture and window
    /// plumbing.
    ///
    /// The show-overlay callback receives the captured geometry (physical
    /// origin/size in virtual-screen coordinates) and returns
    /// `Result<(), String>`. A failed show is not a runtime failure: the stored
    /// session is dropped, `Ready` is published and `OverlayFailed` is
    /// returned so the runtime can start a fresh session.
    pub async fn begin_capture_session<E, C, S>(
        &self,
        mut emit: E,
        capture: C,
        show_overlay: S,
    ) -> BeginOutcome
    where
        E: FnMut(&OcrStatus),
        C: FnOnce() -> Result<(VirtualScreenGeometry, RgbaImage), CaptureError> + Send + 'static,
        S: FnOnce(&VirtualScreenGeometry) -> Result<(), String>,
    {
        let _guard = self.transition_lock.lock().await;

        if self.status() != OcrStatus::Ready {
            return BeginOutcome::Ignored;
        }

        // Capture the full virtual desktop BEFORE any overlay UI is shown. The
        // blocking capture must not occupy a Tokio worker, so it runs on the
        // blocking pool instead.
        let captured = tokio::task::spawn_blocking(capture).await;
        let (geometry, frame) = match captured {
            Ok(Ok(captured)) => captured,
            Ok(Err(error)) => return BeginOutcome::CaptureFailed(error),
            Err(join_error) => {
                // Never surface panic payloads or JoinError text to the
                // frontend; log the detail for developers only.
                tracing::warn!(error = %join_error, "Capture worker panicked");
                return BeginOutcome::CaptureFailed(CaptureError::CaptureFailed(
                    "capture worker failed".to_string(),
                ));
            }
        };

        *self.session.lock().await = Some(CaptureSession {
            geometry: geometry.clone(),
            frame: Arc::new(frame),
        });

        self.publish_status(OcrStatus::SelectingArea, &mut emit);

        // Only now is it safe to reveal the overlay over the saved frame. A
        // missing or broken overlay is not a runtime failure: drop the session
        // and return to `Ready` so the runtime can start again.
        if let Err(reason) = show_overlay(&geometry) {
            self.session.lock().await.take();
            self.publish_status(OcrStatus::Ready, &mut emit);
            return BeginOutcome::OverlayFailed(reason);
        }

        BeginOutcome::Started
    }

    /// Submit a selection rectangle for the in-flight session.
    ///
    /// The overlay reports the corners as physical coordinates local to its own
    /// client area (its top-left sits on the stored geometry origin), so both
    /// corners are first shifted by that origin into virtual-screen
    /// coordinates. The translated rectangle is then normalized, clamped to the
    /// saved virtual-screen bounds and validated against
    /// [`capture::MIN_SELECTION_SIDE`]. A too-small or empty selection — or one
    /// whose translation would overflow `i32` — keeps the session and stays
    /// `SelectingArea` so the user can drag again. A valid selection drops the
    /// session, publishes `Recognizing`, crops the SAVED frame via
    /// [`crop_to_rgb`] and runs recognition through the injected closure (the
    /// command layer locks [`Self::runtime_slot`]); the status is published
    /// back to `Ready` either way. No pixel work happens under any lock.
    pub async fn finish_selection<E, F, Fut>(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        mut emit: E,
        recognize: F,
    ) -> FinishOutcome
    where
        E: FnMut(&OcrStatus),
        F: FnOnce(Arc<RgbImage>) -> Fut,
        Fut: Future<Output = Result<OcrResult, String>>,
    {
        let _guard = self.transition_lock.lock().await;

        let Some(session) = self.session.lock().await.take() else {
            return FinishOutcome::NoSession;
        };

        // The overlay-local corners are physical px relative to the overlay's
        // own client area; the saved frame lives in virtual-screen coordinates,
        // so shift both corners by the session geometry origin first. A
        // representational overflow is retryable, never a panic.
        let Some((x1, y1, x2, y2)) =
            translate_by_session_origin(x1, y1, x2, y2, session.geometry.origin)
        else {
            *self.session.lock().await = Some(session);
            return FinishOutcome::TooSmall;
        };

        let rect = normalize_selection(x1, y1, x2, y2);
        let rect = match clamp_selection(rect, &session.geometry) {
            None => {
                // Empty/outside selection: retryable, keep the session.
                *self.session.lock().await = Some(session);
                return FinishOutcome::TooSmall;
            }
            Some(clamped) => match validate_selection(clamped) {
                Ok(rect) => rect,
                Err(CaptureError::SelectionTooSmall) => {
                    // Retryable: keep the session, stay SelectingArea.
                    *self.session.lock().await = Some(session);
                    return FinishOutcome::TooSmall;
                }
                Err(CaptureError::NoMonitors | CaptureError::CaptureFailed(_)) => {
                    unreachable!("validate_selection only ever returns SelectionTooSmall")
                }
            },
        };

        self.publish_status(OcrStatus::Recognizing, &mut emit);

        // The RGBA→RGB crop is a pixel conversion (up to ~100 MB) and must not
        // occupy a Tokio worker: run it on the blocking pool. A panicking crop
        // worker surfaces as a safe RecognizeFailed (recognition path), never a
        // panic payload.
        let frame = session.frame.clone();
        let geometry = session.geometry.clone();
        let cropped =
            match tokio::task::spawn_blocking(move || crop_to_rgb(&frame, &geometry, rect)).await {
                Ok(cropped) => Arc::new(cropped),
                Err(join_error) => {
                    tracing::warn!(error = %join_error, "OCR crop worker panicked");
                    let outcome = FinishOutcome::RecognizeFailed("recognitionFailed".to_string());
                    if self.is_runtime_active() {
                        self.publish_status(OcrStatus::Ready, &mut emit);
                    }
                    return outcome;
                }
            };

        let outcome = match recognize(cropped).await {
            Ok(result) => FinishOutcome::Recognized(result),
            Err(reason) => FinishOutcome::RecognizeFailed(reason),
        };

        // Return to `Ready` only while the runtime is still installed. After a
        // stop the session is already gone, but this guard keeps a defensive
        // finish from ever publishing `Ready` out of a disabled state.
        if self.is_runtime_active() {
            self.publish_status(OcrStatus::Ready, &mut emit);
        }

        outcome
    }

    /// Cancel the in-flight capture session.
    ///
    /// Drops the stored frame and returns to `Ready` when a session was active;
    /// a session-less call is a no-op. Returns whether a session was dropped.
    pub async fn cancel_session<E>(&self, mut emit: E) -> bool
    where
        E: FnMut(&OcrStatus),
    {
        let _guard = self.transition_lock.lock().await;

        let had_session = self.session.lock().await.take().is_some();
        // Publish `Ready` only while the runtime is still installed: a stop
        // drops the session first, so this guard is defensive against a stale
        // cancel resurrecting `Ready` from a disabled state.
        if had_session && self.is_runtime_active() {
            self.publish_status(OcrStatus::Ready, &mut emit);
        }
        had_session
    }
}

impl Default for OcrService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Hotkey;
    use crate::ocr::capture::{MonitorRect, MIN_SELECTION_SIDE};
    use crate::ocr::settings::OcrSettings;
    use image::Rgba;
    use std::time::Duration;

    fn unique_test_root(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ttsbard-ocr-service-test-{}-{}-{}",
            std::process::id(),
            unique,
            name
        ))
    }

    fn write_valid_pack(root: &Path, id: &str) {
        let dir = root.join("models").join("ocr").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            serde_json::json!({
                "id": id,
                "display_name": "Dummy Pack",
                "languages": ["ru", "en"],
                "family": "pp_ocr_v5",
                "det_file": "det.onnx",
                "rec_file": "rec.onnx",
                "dict_file": "dict.txt"
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(dir.join("det.onnx"), b"dummy det").unwrap();
        std::fs::write(dir.join("rec.onnx"), b"dummy rec").unwrap();
        std::fs::write(dir.join("dict.txt"), b"dummy dict").unwrap();
    }

    fn dummy_hotkey() -> Hotkey {
        Hotkey::default_ocr_capture()
    }

    fn noop_register() -> impl FnOnce(&Hotkey) -> Result<(), String> {
        |_| Ok(())
    }

    fn noop_unregister() -> impl Fn(&Hotkey) -> Result<(), String> {
        |_| Ok(())
    }

    #[test]
    fn status_wire_shape_matches_input_server_convention() {
        assert_eq!(
            serde_json::to_value(OcrStatus::Disabled).unwrap(),
            serde_json::json!({ "state": "disabled" })
        );
        assert_eq!(
            serde_json::to_value(OcrStatus::Starting).unwrap(),
            serde_json::json!({ "state": "starting" })
        );
        assert_eq!(
            serde_json::to_value(OcrStatus::Ready).unwrap(),
            serde_json::json!({ "state": "ready" })
        );
        assert_eq!(
            serde_json::to_value(OcrStatus::SelectingArea).unwrap(),
            serde_json::json!({ "state": "selectingArea" })
        );
        assert_eq!(
            serde_json::to_value(OcrStatus::Recognizing).unwrap(),
            serde_json::json!({ "state": "recognizing" })
        );
        assert_eq!(
            serde_json::to_value(OcrStatus::Error {
                message: "boom".to_string()
            })
            .unwrap(),
            serde_json::json!({ "state": "error", "message": "boom" })
        );
    }

    #[test]
    fn publish_status_emits_only_on_change() {
        let service = OcrService::with_packs_root(PathBuf::new());
        let mut emitted: Vec<OcrStatus> = Vec::new();

        service.publish_status(OcrStatus::Disabled, |status| {
            emitted.push(status.clone());
        });
        assert!(emitted.is_empty(), "no emit when status did not change");

        service.publish_status(OcrStatus::Starting, |status| {
            emitted.push(status.clone());
        });
        service.publish_status(OcrStatus::Starting, |status| {
            emitted.push(status.clone());
        });
        service.publish_status(
            OcrStatus::Error {
                message: "boom".into(),
            },
            |status| {
                emitted.push(status.clone());
            },
        );

        assert_eq!(
            emitted,
            vec![
                OcrStatus::Starting,
                OcrStatus::Error {
                    message: "boom".into()
                }
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_without_model_reports_error_never_ready() {
        let service = OcrService::with_packs_root(PathBuf::new());
        service.settings.write().await.model_id = None;

        let mut transitions: Vec<OcrStatus> = Vec::new();
        service
            .start(
                &dummy_hotkey(),
                |s| transitions.push(s.clone()),
                noop_register(),
            )
            .await;

        assert!(
            matches!(service.status(), OcrStatus::Error { .. }),
            "expected Error, got {:?}",
            service.status()
        );
        assert!(
            !transitions.contains(&OcrStatus::Ready),
            "must never reach Ready without a model"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_with_unknown_model_names_missing_pack_without_substitution() {
        let root = unique_test_root("unknown-model");
        write_valid_pack(&root, "good-pack");

        let service = OcrService::with_packs_root(root.clone());
        service.settings.write().await.model_id = Some("missing-pack".to_string());

        service
            .start(&dummy_hotkey(), |_| {}, noop_register())
            .await;

        match service.status() {
            OcrStatus::Error { message } => {
                assert!(message.contains("missing-pack"), "message: {message}");
                assert!(
                    !message.contains("good-pack"),
                    "must not substitute: {message}"
                );
            }
            other => panic!("expected Error, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_with_dummy_pack_maps_init_failure_to_error() {
        let root = unique_test_root("dummy-init");
        write_valid_pack(&root, "dummy-pack");

        let service = OcrService::with_packs_root(root.clone());
        service.settings.write().await.model_id = Some("dummy-pack".to_string());

        service
            .start(&dummy_hotkey(), |_| {}, noop_register())
            .await;

        match service.status() {
            OcrStatus::Error { message } => assert!(!message.is_empty()),
            other => panic!("expected Error from dummy ONNX, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stop_without_runtime_reports_disabled() {
        let service = OcrService::with_packs_root(PathBuf::new());
        service.stop(|_| {}, noop_unregister()).await;
        assert_eq!(service.status(), OcrStatus::Disabled);
    }

    #[test]
    fn decide_enable_flip_starts_and_stops() {
        let old = OcrSettings::default();
        let on = OcrSettings {
            enabled: true,
            ..Default::default()
        };

        assert_eq!(
            decide_transition(&old, &on, None, None, &OcrStatus::Disabled),
            OcrTransition::Start
        );
        assert_eq!(
            decide_transition(&on, &old, None, None, &OcrStatus::Ready),
            OcrTransition::Stop
        );
    }

    #[test]
    fn decide_model_change_while_enabled_restarts() {
        let mut old = OcrSettings {
            enabled: true,
            model_id: Some("a".to_string()),
        };
        let mut new = OcrSettings {
            enabled: true,
            model_id: Some("b".to_string()),
        };

        assert_eq!(
            decide_transition(&old, &new, None, None, &OcrStatus::Ready),
            OcrTransition::Restart
        );

        old.model_id = None;
        new.model_id = None;
        assert_eq!(
            decide_transition(&old, &new, None, None, &OcrStatus::Ready),
            OcrTransition::Noop
        );
    }

    #[test]
    fn decide_hotkey_change_while_ready_reregisters() {
        let settings = OcrSettings {
            enabled: true,
            model_id: Some("a".to_string()),
        };
        let old_hotkey = Hotkey::default_ocr_capture();
        let mut new_hotkey = old_hotkey.clone();
        new_hotkey.key = "O".to_string();

        assert_eq!(
            decide_transition(
                &settings,
                &settings,
                Some(&old_hotkey),
                Some(&new_hotkey),
                &OcrStatus::Ready
            ),
            OcrTransition::ReregisterHotkey
        );

        // Same binding while Ready is a no-op.
        assert_eq!(
            decide_transition(
                &settings,
                &settings,
                Some(&old_hotkey),
                Some(&old_hotkey),
                &OcrStatus::Ready
            ),
            OcrTransition::Noop
        );
    }

    #[test]
    fn decide_error_retries_unchanged_settings() {
        let settings = OcrSettings {
            enabled: true,
            model_id: Some("a".to_string()),
        };
        let hotkey = Hotkey::default_ocr_capture();
        let error = OcrStatus::Error {
            message: "boom".to_string(),
        };

        // Unchanged settings (and unchanged binding) while Error retry.
        assert_eq!(
            decide_transition(&settings, &settings, Some(&hotkey), Some(&hotkey), &error),
            OcrTransition::Retry
        );

        // Retry also covers a not-yet-registered binding (the common Error case).
        assert_eq!(
            decide_transition(&settings, &settings, None, Some(&hotkey), &error),
            OcrTransition::Retry
        );

        // A model change while Error still restarts (higher priority).
        let mut new_model = settings.clone();
        new_model.model_id = Some("b".to_string());
        assert_eq!(
            decide_transition(&settings, &new_model, Some(&hotkey), Some(&hotkey), &error),
            OcrTransition::Restart
        );
    }

    #[test]
    fn decide_disabled_to_disabled_is_noop() {
        let settings = OcrSettings::default();
        assert_eq!(
            decide_transition(&settings, &settings, None, None, &OcrStatus::Disabled),
            OcrTransition::Noop
        );
    }

    // --- One-shot capture session ---

    fn test_geometry() -> VirtualScreenGeometry {
        VirtualScreenGeometry {
            origin: (0, 0),
            size: (100, 100),
            monitors: vec![MonitorRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }],
        }
    }

    fn test_frame() -> RgbaImage {
        RgbaImage::from_pixel(100, 100, Rgba([255, 0, 0, 255]))
    }

    fn fake_ocr_result(text: &str) -> OcrResult {
        OcrResult {
            text: text.to_string(),
            lines: Vec::new(),
            total_ms: 12.5,
        }
    }

    /// A service pre-flipped to `Ready` with a live runtime and registered
    /// capture shortcut, the only state a session can begin from.
    fn ready_service() -> OcrService {
        let service = OcrService::with_packs_root(PathBuf::new());
        service.set_status(OcrStatus::Ready);
        *service.registered_hotkey.lock() = Some(dummy_hotkey());
        service
    }

    #[tokio::test]
    async fn begin_when_not_ready_is_noop() {
        let service = OcrService::with_packs_root(PathBuf::new());
        assert_eq!(service.status(), OcrStatus::Disabled);

        let captured = Arc::new(std::sync::Mutex::new(false));
        let capture_flag = Arc::clone(&captured);
        let mut shown = false;
        let outcome = service
            .begin_capture_session(
                |_| {},
                move || {
                    *capture_flag.lock().unwrap() = true;
                    Ok((test_geometry(), test_frame()))
                },
                |_geometry| {
                    shown = true;
                    Ok(())
                },
            )
            .await;

        assert!(matches!(outcome, BeginOutcome::Ignored));
        assert!(
            !*captured.lock().unwrap(),
            "capture must not run outside Ready"
        );
        assert!(!shown, "overlay must not be shown outside Ready");
        assert_eq!(service.status(), OcrStatus::Disabled);
    }

    #[tokio::test]
    async fn begin_from_ready_captures_before_show_and_publishes_selecting_area() {
        let service = ready_service();
        let order = Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
        let capture_order = Arc::clone(&order);
        let show_order = Arc::clone(&order);
        let mut published = Vec::new();

        let outcome = service
            .begin_capture_session(
                |status| published.push(status.clone()),
                move || {
                    capture_order.lock().unwrap().push("capture");
                    Ok((test_geometry(), test_frame()))
                },
                move |_geometry| {
                    show_order.lock().unwrap().push("show");
                    Ok(())
                },
            )
            .await;

        assert!(matches!(outcome, BeginOutcome::Started));
        assert_eq!(service.status(), OcrStatus::SelectingArea);
        assert_eq!(
            *order.lock().unwrap(),
            vec!["capture", "show"],
            "the frame must be captured before the overlay is revealed"
        );
        assert_eq!(published, vec![OcrStatus::SelectingArea]);
    }

    #[tokio::test]
    async fn begin_capture_failure_keeps_ready_and_shows_no_overlay() {
        let service = ready_service();
        let mut shown = false;

        let outcome = service
            .begin_capture_session(
                |_| {},
                || Err(CaptureError::NoMonitors),
                |_geometry| {
                    shown = true;
                    Ok(())
                },
            )
            .await;

        assert!(matches!(
            outcome,
            BeginOutcome::CaptureFailed(CaptureError::NoMonitors)
        ));
        assert_eq!(service.status(), OcrStatus::Ready);
        assert!(!shown, "overlay must not be shown after a capture failure");
    }

    #[tokio::test]
    async fn begin_while_selecting_area_is_ignored() {
        let service = ready_service();
        let first = service
            .begin_capture_session(|_| {}, || Ok((test_geometry(), test_frame())), |_| Ok(()))
            .await;
        assert!(matches!(first, BeginOutcome::Started));

        let second_capture = Arc::new(std::sync::Mutex::new(false));
        let capture_flag = Arc::clone(&second_capture);
        let outcome = service
            .begin_capture_session(
                |_| {},
                move || {
                    *capture_flag.lock().unwrap() = true;
                    Ok((test_geometry(), test_frame()))
                },
                |_| Ok(()),
            )
            .await;

        assert!(matches!(outcome, BeginOutcome::Ignored));
        assert!(
            !*second_capture.lock().unwrap(),
            "re-entry must not capture again"
        );
        assert_eq!(service.status(), OcrStatus::SelectingArea);
    }

    #[tokio::test]
    async fn begin_panicking_capture_returns_safe_capture_failed_and_no_overlay() {
        let service = ready_service();

        let mut shown = false;
        let outcome = service
            .begin_capture_session(
                |_| {},
                || panic!("xcap exploded"),
                |_geometry| {
                    shown = true;
                    Ok(())
                },
            )
            .await;

        match outcome {
            BeginOutcome::CaptureFailed(CaptureError::CaptureFailed(reason)) => {
                assert!(
                    !reason.to_lowercase().contains("exploded"),
                    "panic payload must not leak into the failure reason: {reason}"
                );
            }
            other => panic!("expected CaptureFailed, got {other:?}"),
        }
        assert_eq!(service.status(), OcrStatus::Ready);
        assert!(!shown, "overlay must not be shown after a capture panic");
        assert!(
            !service.cancel_session(|_| {}).await,
            "no session may exist after a capture failure"
        );
    }

    #[tokio::test]
    async fn begin_show_failure_clears_session_publishes_ready_and_allows_fresh_start() {
        let service = ready_service();
        let mut published = Vec::new();

        let outcome = service
            .begin_capture_session(
                |status| published.push(status.clone()),
                || Ok((test_geometry(), test_frame())),
                |_geometry| Err("ocr-selection overlay broken".to_string()),
            )
            .await;

        match outcome {
            BeginOutcome::OverlayFailed(reason) => {
                assert_eq!(reason, "ocr-selection overlay broken");
            }
            other => panic!("expected OverlayFailed, got {other:?}"),
        }
        assert_eq!(
            published,
            vec![OcrStatus::SelectingArea, OcrStatus::Ready],
            "show failure must publish SelectingArea then Ready"
        );
        assert_eq!(service.status(), OcrStatus::Ready);
        assert!(
            !service.cancel_session(|_| {}).await,
            "no session may survive a failed overlay show"
        );

        // A fresh session can start afterwards.
        let outcome = service
            .begin_capture_session(|_| {}, || Ok((test_geometry(), test_frame())), |_| Ok(()))
            .await;
        assert!(matches!(outcome, BeginOutcome::Started));
        assert_eq!(service.status(), OcrStatus::SelectingArea);
    }

    #[tokio::test]
    async fn finish_too_small_keeps_session_and_stays_selecting_area() {
        let service = ready_service();
        service
            .begin_capture_session(|_| {}, || Ok((test_geometry(), test_frame())), |_| Ok(()))
            .await;
        assert_eq!(service.status(), OcrStatus::SelectingArea);

        let mut recognized = false;
        let outcome = service
            .finish_selection(
                0,
                0,
                MIN_SELECTION_SIDE as i32 - 1,
                MIN_SELECTION_SIDE as i32,
                |_| {},
                |_image| {
                    recognized = true;
                    async { Ok(fake_ocr_result("nope")) }
                },
            )
            .await;

        assert_eq!(outcome, FinishOutcome::TooSmall);
        assert!(!recognized, "too-small selection must not run recognition");
        assert_eq!(service.status(), OcrStatus::SelectingArea);

        // The session survived: a valid selection now succeeds on the same
        // saved frame.
        let outcome = service
            .finish_selection(
                0,
                0,
                40,
                40,
                |_| {},
                |_image| async { Ok(fake_ocr_result("ok")) },
            )
            .await;
        assert!(matches!(outcome, FinishOutcome::Recognized(_)));
        assert_eq!(service.status(), OcrStatus::Ready);
    }

    #[tokio::test]
    async fn finish_valid_rect_crops_saved_frame_and_returns_ready() {
        let service = ready_service();
        service
            .begin_capture_session(|_| {}, || Ok((test_geometry(), test_frame())), |_| Ok(()))
            .await;

        let mut published = Vec::new();
        let seen = Arc::new(std::sync::Mutex::new(None));
        let seen_clone = seen.clone();
        let recognize = move |image: Arc<RgbImage>| {
            *seen_clone.lock().unwrap() = Some(image.dimensions());
            async { Ok(fake_ocr_result("hello")) }
        };

        let outcome = service
            .finish_selection(
                10,
                20,
                50,
                60,
                |status| published.push(status.clone()),
                recognize,
            )
            .await;

        assert_eq!(outcome, FinishOutcome::Recognized(fake_ocr_result("hello")));
        assert_eq!(
            *seen.lock().unwrap(),
            Some((40, 40)),
            "recognition must run on the crop of the saved frame"
        );
        assert_eq!(published, vec![OcrStatus::Recognizing, OcrStatus::Ready]);
        assert_eq!(service.status(), OcrStatus::Ready);

        // Session was dropped: a cancel afterwards is a no-op.
        assert!(!service.cancel_session(|_| {}).await);
    }

    #[tokio::test]
    async fn finish_translates_overlay_local_rect_by_negative_origin_before_cropping() {
        let service = ready_service();

        // Overlay window sits with its client-area origin on the geometry
        // origin (-100, -50), so an overlay-local corner (lx, ly) denotes the
        // saved-frame pixel at the same (lx, ly) offset once translated.
        let geometry = VirtualScreenGeometry {
            origin: (-100, -50),
            size: (200, 200),
            monitors: vec![MonitorRect {
                x: -100,
                y: -50,
                width: 200,
                height: 200,
            }],
        };
        let mut frame = RgbaImage::from_pixel(200, 200, Rgba([0, 0, 0, 255]));
        frame.put_pixel(12, 22, Rgba([255, 0, 0, 255]));
        frame.put_pixel(38, 58, Rgba([0, 0, 255, 255]));

        service
            .begin_capture_session(|_| {}, move || Ok((geometry, frame)), |_| Ok(()))
            .await;
        assert_eq!(service.status(), OcrStatus::SelectingArea);

        // Overlay-local rectangle (10, 20)-(40, 60). Untranslated it would be
        // clamped against the negative virtual bounds and crop black pixels far
        // from the markers; translated it must crop the exact frame region.
        let seen = Arc::new(std::sync::Mutex::new(None));
        let seen_clone = Arc::clone(&seen);
        let recognize = move |image: Arc<RgbImage>| {
            *seen_clone.lock().unwrap() = Some(image);
            async { Ok(fake_ocr_result("translated")) }
        };

        let outcome = service
            .finish_selection(10, 20, 40, 60, |_| {}, recognize)
            .await;

        assert!(matches!(outcome, FinishOutcome::Recognized(_)));
        let cropped = seen.lock().unwrap().take().expect("recognition ran");
        assert_eq!(cropped.dimensions(), (30, 40));
        assert_eq!(cropped.get_pixel(0, 0), &image::Rgb([0, 0, 0]));
        assert_eq!(cropped.get_pixel(2, 2), &image::Rgb([255, 0, 0]));
        assert_eq!(cropped.get_pixel(28, 38), &image::Rgb([0, 0, 255]));
        assert_eq!(service.status(), OcrStatus::Ready);
    }

    #[tokio::test]
    async fn finish_translation_overflow_is_retryable_and_never_recognizes() {
        let service = ready_service();

        // Origin so large that overlay-local corners above x = 1000 overflow
        // i32 on translation. Coordinates the overlay can really report (bounded
        // by the window size) stay far below that, so the retained session still
        // accepts a valid retry.
        let geometry = VirtualScreenGeometry {
            origin: (i32::MAX - 1000, 0),
            size: (200, 200),
            monitors: vec![MonitorRect {
                x: i32::MAX - 1000,
                y: 0,
                width: 200,
                height: 200,
            }],
        };
        let frame = RgbaImage::from_pixel(200, 200, Rgba([255, 0, 0, 255]));

        service
            .begin_capture_session(|_| {}, move || Ok((geometry, frame)), |_| Ok(()))
            .await;
        assert_eq!(service.status(), OcrStatus::SelectingArea);

        let mut published = Vec::new();
        let mut recognized = false;
        let outcome = service
            .finish_selection(
                2000,
                0,
                2100,
                100,
                |status| published.push(status.clone()),
                |_image| {
                    recognized = true;
                    async { Ok(fake_ocr_result("must not run")) }
                },
            )
            .await;

        assert_eq!(outcome, FinishOutcome::TooSmall);
        assert!(
            !recognized,
            "overflowing corners must never reach recognition"
        );
        assert!(published.is_empty(), "no status change on overflow");
        assert_eq!(service.status(), OcrStatus::SelectingArea);

        // The session survived the overflow: a valid selection is recognized.
        let outcome = service
            .finish_selection(
                0,
                0,
                40,
                40,
                |_| {},
                |_image| async { Ok(fake_ocr_result("retried")) },
            )
            .await;
        assert!(matches!(outcome, FinishOutcome::Recognized(_)));
        assert_eq!(service.status(), OcrStatus::Ready);
    }

    #[tokio::test]
    async fn finish_recognize_failure_returns_ready_and_failure_outcome() {
        let service = ready_service();
        service
            .begin_capture_session(|_| {}, || Ok((test_geometry(), test_frame())), |_| Ok(()))
            .await;

        let outcome = service
            .finish_selection(
                0,
                0,
                40,
                40,
                |_| {},
                |_image| async { Err("recognizer down".to_string()) },
            )
            .await;

        assert_eq!(
            outcome,
            FinishOutcome::RecognizeFailed("recognizer down".to_string())
        );
        assert_eq!(service.status(), OcrStatus::Ready);
    }

    #[tokio::test]
    async fn finish_without_session_is_noop() {
        let service = ready_service();
        let outcome = service
            .finish_selection(
                0,
                0,
                40,
                40,
                |_| {},
                |_image| async { Ok(fake_ocr_result("nope")) },
            )
            .await;

        assert_eq!(outcome, FinishOutcome::NoSession);
        assert_eq!(service.status(), OcrStatus::Ready);
    }

    #[tokio::test]
    async fn cancel_drops_session_and_returns_ready() {
        let service = ready_service();
        service
            .begin_capture_session(|_| {}, || Ok((test_geometry(), test_frame())), |_| Ok(()))
            .await;
        assert_eq!(service.status(), OcrStatus::SelectingArea);

        let mut published = Vec::new();
        let cancelled = service
            .cancel_session(|status| published.push(status.clone()))
            .await;

        assert!(cancelled);
        assert_eq!(service.status(), OcrStatus::Ready);
        assert_eq!(published, vec![OcrStatus::Ready]);
    }

    #[tokio::test]
    async fn cancel_without_session_is_noop() {
        let service = ready_service();
        let cancelled = service.cancel_session(|_| {}).await;

        assert!(!cancelled);
        assert_eq!(service.status(), OcrStatus::Ready);
    }

    // --- Stop during an active session ---

    #[tokio::test]
    async fn stop_during_selecting_area_drops_session_and_never_publishes_ready() {
        let service = ready_service();
        service
            .begin_capture_session(|_| {}, || Ok((test_geometry(), test_frame())), |_| Ok(()))
            .await;
        assert_eq!(service.status(), OcrStatus::SelectingArea);

        let mut published = Vec::new();
        service
            .stop(|status| published.push(status.clone()), noop_unregister())
            .await;

        assert_eq!(service.status(), OcrStatus::Disabled);
        assert_eq!(published, vec![OcrStatus::Disabled]);

        // A stale overlay cancel after stop is a no-op: no `Ready` is published.
        let mut published = Vec::new();
        let cancelled = service
            .cancel_session(|status| published.push(status.clone()))
            .await;
        assert!(!cancelled);
        assert!(published.is_empty(), "cancel after stop must not publish");
        assert_eq!(service.status(), OcrStatus::Disabled);

        // A stale overlay submit after stop is a no-op: no `Ready` is published.
        let mut published = Vec::new();
        let outcome = service
            .finish_selection(
                0,
                0,
                40,
                40,
                |status| published.push(status.clone()),
                |_image| async { Ok(fake_ocr_result("stale")) },
            )
            .await;
        assert_eq!(outcome, FinishOutcome::NoSession);
        assert!(published.is_empty(), "finish after stop must not publish");
        assert_eq!(service.status(), OcrStatus::Disabled);
    }

    #[tokio::test]
    async fn stop_resets_session_so_cancel_after_stop_is_noop() {
        let service = ready_service();
        service
            .begin_capture_session(|_| {}, || Ok((test_geometry(), test_frame())), |_| Ok(()))
            .await;
        assert_eq!(service.status(), OcrStatus::SelectingArea);

        service.stop(|_| {}, noop_unregister()).await;
        assert_eq!(service.status(), OcrStatus::Disabled);

        // The session was dropped by stop: cancel is a no-op and the status
        // stays `Disabled` rather than returning to `Ready`.
        let mut published = Vec::new();
        assert!(!service.cancel_session(|s| published.push(s.clone())).await);
        assert!(published.is_empty());
        assert_eq!(service.status(), OcrStatus::Disabled);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stop_for_shutdown_returns_quickly_while_transition_lock_is_held() {
        let service = Arc::new(ready_service());

        // Simulate in-flight recognition holding the transition lock with a
        // recognize future that never resolves.
        let finish = {
            let service = Arc::clone(&service);
            tokio::spawn(async move {
                service
                    .begin_capture_session(
                        |_| {},
                        || Ok((test_geometry(), test_frame())),
                        |_| Ok(()),
                    )
                    .await;
                service
                    .finish_selection(
                        0,
                        0,
                        40,
                        40,
                        |_| {},
                        |_image| std::future::pending::<Result<OcrResult, String>>(),
                    )
                    .await;
            })
        };

        // Let finish_selection acquire the transition lock and block on
        // recognition.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let started = std::time::Instant::now();
        service
            .clone()
            .stop_for_shutdown(|_| {}, noop_unregister())
            .await;
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "shutdown stop must not block on in-flight recognition"
        );

        finish.abort();
        let _ = finish.await;
    }

    // --- Hotkey rebind during an active session ---

    #[tokio::test]
    async fn reregister_from_selecting_area_swaps_binding_and_keeps_session() {
        let service = ready_service();
        service
            .begin_capture_session(|_| {}, || Ok((test_geometry(), test_frame())), |_| Ok(()))
            .await;
        assert_eq!(service.status(), OcrStatus::SelectingArea);

        let old_key = dummy_hotkey().key;
        let mut new_hotkey = dummy_hotkey();
        new_hotkey.key = "O".to_string();

        let registered = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let unregistered = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let registered_clone = Arc::clone(&registered);
        let unregistered_clone = Arc::clone(&unregistered);

        service
            .reregister_hotkey(
                &new_hotkey,
                |_| {},
                move |hk| {
                    registered_clone.lock().unwrap().push(hk.key.clone());
                    Ok(())
                },
                move |hk| {
                    unregistered_clone.lock().unwrap().push(hk.key.clone());
                    Ok(())
                },
            )
            .await;

        assert_eq!(*registered.lock().unwrap(), vec!["O".to_string()]);
        assert_eq!(*unregistered.lock().unwrap(), vec![old_key]);
        assert_eq!(service.registered_hotkey(), Some(new_hotkey.clone()));
        // The rebind must not disturb the in-flight selection.
        assert_eq!(service.status(), OcrStatus::SelectingArea);
    }

    #[tokio::test]
    async fn reregister_from_recognizing_swaps_binding() {
        let service = ready_service();
        // Simulate the recognition phase directly: the runtime is live and the
        // shortcut is registered, with status `Recognizing`.
        service.set_status(OcrStatus::Recognizing);
        *service.registered_hotkey.lock() = Some(dummy_hotkey());

        let old_key = dummy_hotkey().key;
        let mut new_hotkey = dummy_hotkey();
        new_hotkey.key = "O".to_string();

        let registered = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let unregistered = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let registered_clone = Arc::clone(&registered);
        let unregistered_clone = Arc::clone(&unregistered);

        service
            .reregister_hotkey(
                &new_hotkey,
                |_| {},
                move |hk| {
                    registered_clone.lock().unwrap().push(hk.key.clone());
                    Ok(())
                },
                move |hk| {
                    unregistered_clone.lock().unwrap().push(hk.key.clone());
                    Ok(())
                },
            )
            .await;

        assert_eq!(*registered.lock().unwrap(), vec!["O".to_string()]);
        assert_eq!(*unregistered.lock().unwrap(), vec![old_key]);
        assert_eq!(service.registered_hotkey(), Some(new_hotkey.clone()));
        assert_eq!(service.status(), OcrStatus::Recognizing);
    }

    #[tokio::test]
    async fn reregister_with_empty_runtime_slot_is_noop() {
        let service = OcrService::with_packs_root(PathBuf::new());
        assert_eq!(service.status(), OcrStatus::Disabled);
        assert!(!service.is_runtime_active());

        let mut new_hotkey = dummy_hotkey();
        new_hotkey.key = "O".to_string();

        let mut registered = false;
        service
            .reregister_hotkey(
                &new_hotkey,
                |_| {},
                |_| {
                    registered = true;
                    Ok(())
                },
                noop_unregister(),
            )
            .await;

        assert!(!registered, "no register call with an empty runtime slot");
        assert_eq!(service.status(), OcrStatus::Disabled);
        assert_eq!(service.registered_hotkey(), None);
    }
}
