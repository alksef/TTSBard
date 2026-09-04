use std::sync::Arc;

use crate::commands::input_server::accept_external_text;
use crate::commands::speech_queue::SpeechQueueState;
use crate::config::{Hotkey, SettingsManager};
use crate::ocr::capture::{capture_virtual_desktop, CaptureError, VirtualScreenGeometry};
use crate::ocr::packs::scan_ocr_packs;
use crate::ocr::service::{
    decide_ocr_refresh_reconcile, decide_transition, BeginOutcome, FinishOutcome,
    OcrRefreshReconcile, OcrTransition,
};
use crate::ocr::settings::OcrSettings;
use crate::ocr::OcrStatus;
use crate::speech_queue::SubmissionSource;
use crate::state::AppState;
use image::RgbImage;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Event emitted on every real OCR runtime status transition.
pub const OCR_STATUS_CHANGED_EVENT: &str = "ocr-status-changed";

/// Event emitted when a one-shot OCR capture begins but fails before the
/// selection overlay is usable. Payload: `{ reason: string }` (camelCase).
pub const OCR_ONE_SHOT_FAILED_EVENT: &str = "ocr-one-shot-failed";

/// Event emitted when a submitted selection was rejected as too small and the
/// overlay is re-shown at its unchanged geometry. No payload.
pub const OCR_SELECTION_REJECTED_EVENT: &str = "ocr-selection-rejected";

/// Safe presentation DTO for a discovered OCR pack (no resolved paths).
#[derive(Debug, Clone, Serialize)]
pub struct OcrPackDto {
    pub id: String,
    pub display_name: String,
    pub languages: Vec<String>,
}

/// Serializable payload for [`OCR_ONE_SHOT_FAILED_EVENT`]. Deliberately only a
/// safe reason string: no panic payloads, pixels, paths or model internals.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrOneShotFailedPayload {
    pub reason: String,
}

/// Fixed safe user-facing reason for a [`CaptureError`] category. The detailed
/// error text is only ever traced, never serialized to the frontend.
fn capture_failed_reason(error: &CaptureError) -> &'static str {
    match error {
        CaptureError::NoMonitors => "noMonitors",
        CaptureError::CaptureFailed(_) | CaptureError::SelectionTooSmall => "captureFailed",
    }
}

/// True when recognized OCR text is normalized blank/whitespace-only.
///
/// Such a result carries no usable content, so no incoming item is created —
/// instead a single fixed-safe `emptyResult` failure is surfaced. The text
/// itself is never logged or exposed.
fn is_blank_ocr_text(text: &str) -> bool {
    text.trim().is_empty()
}

fn emit_ocr_status(app_handle: &AppHandle, status: &OcrStatus) {
    let _ = app_handle.emit(OCR_STATUS_CHANGED_EVENT, status.clone());
}

fn emit_ocr_one_shot_failed(app_handle: &AppHandle, reason: String) {
    let _ = app_handle.emit(
        OCR_ONE_SHOT_FAILED_EVENT,
        OcrOneShotFailedPayload { reason },
    );
}

/// Deliver recognized OCR text through the shared external-text intake seam.
///
/// Resolves the managed [`SpeechQueueState`] from the Tauri app handle, then
/// forwards the recognized text to [`accept_external_text`] tagged with the OCR
/// producer source: with `auto_play` off it lands in the Incoming inbox
/// (`input-server-incoming-changed` is emitted), with `auto_play` on it is
/// submitted directly to the speech queue as `source: "ocr"`. Returns a fixed
/// safe reason when the queue state is unavailable or the seam rejects the text
/// (blank, overlong or inbox-full); the recognized text is never logged or
/// exposed.
async fn deliver_recognized_text(
    app_handle: &AppHandle,
    state: &AppState,
    text: String,
) -> Result<(), String> {
    let Some(queue) = app_handle.try_state::<SpeechQueueState>() else {
        tracing::warn!("SpeechQueueState unavailable while delivering OCR text");
        return Err("intakeFailed".to_string());
    };

    if let Err(error) = accept_external_text(
        app_handle,
        state,
        queue.inner(),
        SubmissionSource::Ocr,
        text,
    )
    .await
    {
        // The intake error carries only safe codes/messages, never the text.
        tracing::warn!(error = ?error, "OCR intake rejected recognized text");
        return Err("intakeFailed".to_string());
    }

    Ok(())
}

/// Handler for the registered `ocr_capture` shortcut.
///
/// Guards match the existing global handlers: skip while hotkey recording is
/// active and respect the global hotkeys-enabled flag. The one-shot flow runs
/// on the app's Tokio runtime so the shortcut callback never blocks.
fn handle_ocr_capture(app_handle: &AppHandle, app_state: &AppState) {
    if app_state.is_hotkey_recording() {
        return;
    }
    if !app_state.is_hotkey_enabled() {
        return;
    }

    let app_handle = app_handle.clone();
    let app_state = app_state.clone();
    let runtime = app_state.runtime.clone();
    runtime.spawn(async move {
        let service = app_state.ocr.clone();
        let emit = |status: &OcrStatus| emit_ocr_status(&app_handle, status);
        let capture = capture_virtual_desktop;
        let show_overlay =
            |geometry: &VirtualScreenGeometry| show_ocr_selection_overlay(&app_handle, geometry);

        match service
            .begin_capture_session(emit, capture, show_overlay)
            .await
        {
            BeginOutcome::Started | BeginOutcome::Ignored => {}
            BeginOutcome::CaptureFailed(error) => {
                tracing::warn!(error = %error, "One-shot OCR capture failed");
                emit_ocr_one_shot_failed(&app_handle, capture_failed_reason(&error).to_string());
            }
            BeginOutcome::OverlayFailed(reason) => {
                tracing::warn!(error = %reason, "Failed to open ocr-selection overlay");
                emit_ocr_one_shot_failed(&app_handle, "overlayOpenFailed".to_string());
            }
        }
    });
}

/// Position, exclude from capture, show and focus the declarative
/// `ocr-selection` overlay window over the captured virtual desktop.
///
/// The window is placed in physical virtual-screen coordinates (`origin` may
/// be negative for monitors left/above the primary). Every failure is a safe
/// `Err(String)`; if the window may already be visible, it is best-effort
/// hidden again before the error is returned.
fn show_ocr_selection_overlay(
    app_handle: &AppHandle,
    geometry: &VirtualScreenGeometry,
) -> Result<(), String> {
    let window = app_handle
        .get_webview_window("ocr-selection")
        .ok_or_else(|| "ocr-selection window not found".to_string())?;

    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: geometry.origin.0,
            y: geometry.origin.1,
        }))
        .map_err(|e| format!("Failed to position ocr-selection overlay: {e}"))?;

    window
        .set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: geometry.size.0,
            height: geometry.size.1,
        }))
        .map_err(|e| format!("Failed to size ocr-selection overlay: {e}"))?;

    #[cfg(windows)]
    {
        // Capture exclusion is non-fatal: the frame was already saved before
        // the overlay is shown, so privacy is preserved by capture→show
        // ordering. A failed exclusion only degrades a later re-capture and is
        // logged, never surfaced as an overlay-open failure.
        if let Ok(hwnd) = window.hwnd() {
            apply_overlay_capture_exclusion(
                hwnd.0 as isize,
                crate::window::set_window_exclude_from_capture,
            );
        } else {
            tracing::warn!("Failed to get ocr-selection overlay HWND for capture exclusion");
        }
    }

    if let Err(error) = window.show() {
        let _ = window.hide();
        return Err(format!("Failed to show ocr-selection overlay: {error}"));
    }

    if let Err(error) = window.set_focus() {
        let _ = window.hide();
        return Err(format!("Failed to focus ocr-selection overlay: {error}"));
    }

    Ok(())
}

/// Apply capture exclusion for the overlay, treating any failure as non-fatal.
///
/// The exclusion is injectable so the non-fatal behavior is unit-testable
/// without a live window.
#[cfg(windows)]
fn apply_overlay_capture_exclusion(
    hwnd: isize,
    exclude: impl FnOnce(isize, bool) -> anyhow::Result<()>,
) {
    if let Err(error) = exclude(hwnd, true) {
        tracing::warn!(error = %error, "Failed to exclude ocr-selection overlay from capture");
    }
}

/// Best-effort hide of the declarative `ocr-selection` overlay window.
///
/// Returns `Err` with a fixed safe code when the window is missing or its hide
/// call fails; the detailed Tauri error is only traced, never serialized to the
/// frontend. A missing window is expected when the overlay already hid itself.
fn hide_ocr_selection_overlay(app_handle: &AppHandle) -> Result<(), String> {
    let Some(window) = app_handle.get_webview_window("ocr-selection") else {
        tracing::warn!("ocr-selection window not found while cancelling selection");
        return Err("overlayHideFailed".to_string());
    };
    if let Err(error) = window.hide() {
        tracing::warn!(error = %error, "Failed to hide ocr-selection overlay");
        return Err("overlayHideFailed".to_string());
    }
    Ok(())
}

/// Handler for the `ocr_selection_cancel` command invoked by the selection
/// overlay on Escape/window blur.
///
/// The overlay is hidden best-effort first, then [`OcrService::cancel_session`]
/// always runs so an active `SelectingArea` session is dropped and status
/// returns to `Ready` — a missing window or a failed hide must never leave a
/// stale session. Such failures are traced in detail and surfaced only as the
/// fixed safe code [`hide_ocr_selection_overlay`] reports. This is an ordinary
/// user cancellation, so no `ocr-one-shot-failed` event is emitted.
#[tauri::command]
pub async fn ocr_selection_cancel(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let hide_result = hide_ocr_selection_overlay(&app_handle);

    let service = state.ocr.clone();
    let emit = |status: &OcrStatus| emit_ocr_status(&app_handle, status);
    let cancelled = service.cancel_session(emit).await;
    tracing::debug!(cancelled, "One-shot OCR selection cancelled");

    hide_result
}

/// Best-effort re-show and focus of the still-positioned `ocr-selection` overlay.
///
/// Used after a too-small selection: the window kept its virtual-screen
/// geometry from the session start, so only visibility and focus are restored.
/// A missing window or a failed show/focus returns the fixed safe code
/// `overlayOpenFailed`; the detailed Tauri error is never surfaced.
fn restore_ocr_selection_overlay(app_handle: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    let window = app_handle
        .get_webview_window("ocr-selection")
        .ok_or_else(|| "overlayOpenFailed".to_string())?;

    if let Err(error) = window.show() {
        let _ = window.hide();
        tracing::warn!(error = %error, "Failed to restore ocr-selection overlay show");
        return Err("overlayOpenFailed".to_string());
    }

    if let Err(error) = window.set_focus() {
        let _ = window.hide();
        tracing::warn!(error = %error, "Failed to restore ocr-selection overlay focus");
        return Err("overlayOpenFailed".to_string());
    }

    Ok(window)
}

/// Handler for the `ocr_selection_submit` command invoked by the selection
/// overlay with the physical corners of the dragged rectangle in the overlay's
/// own client-area coordinates.
///
/// The overlay is hidden first so its pixels can never enter OCR input, then
/// [`OcrService::finish_selection`] shifts the corners into virtual-screen
/// coordinates (by the saved session geometry origin) and crops the saved
/// frame for recognition through the stored runtime. The recognized text is
/// forwarded to the shared incoming intake seam; failures surface only as the
/// fixed safe reasons of [`OCR_ONE_SHOT_FAILED_EVENT`].
#[tauri::command]
pub async fn ocr_selection_submit(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
) -> Result<(), String> {
    if let Err(reason) = hide_ocr_selection_overlay(&app_handle) {
        // A failed hide must never leave a stale session: always cancel it,
        // then surface only the fixed safe code.
        let service = state.ocr.clone();
        let emit = |status: &OcrStatus| emit_ocr_status(&app_handle, status);
        service.cancel_session(emit).await;
        emit_ocr_one_shot_failed(&app_handle, "overlayHideFailed".to_string());
        return Err(reason);
    }

    let service = state.ocr.clone();
    let emit = |status: &OcrStatus| emit_ocr_status(&app_handle, status);
    let recognize = {
        let service = service.clone();
        move |image: Arc<RgbImage>| {
            let service = service.clone();
            async move {
                let runtime = service.runtime_slot().lock().await;
                let Some(runtime) = runtime.as_ref() else {
                    tracing::warn!("One-shot OCR runtime unavailable");
                    return Err("runtimeUnavailable".to_string());
                };
                match runtime.recognize(image).await {
                    Ok(result) => Ok(result),
                    Err(error) => {
                        // Only app-owned safe categories are traced; the caller
                        // always sees the fixed safe reason.
                        tracing::warn!(error = ?error, "One-shot OCR recognition failed");
                        Err("recognitionFailed".to_string())
                    }
                }
            }
        }
    };

    let outcome = service
        .finish_selection(x1, y1, x2, y2, emit, recognize)
        .await;

    match outcome {
        FinishOutcome::TooSmall => {
            // The session stays active: restore the overlay at its unchanged
            // geometry and tell it to re-arm for another drag.
            match restore_ocr_selection_overlay(&app_handle) {
                Ok(window) => {
                    let _ = window.emit(OCR_SELECTION_REJECTED_EVENT, ());
                    Ok(())
                }
                Err(reason) => {
                    // No usable overlay: drop the retained session and surface
                    // only the fixed safe code.
                    let emit = |status: &OcrStatus| emit_ocr_status(&app_handle, status);
                    service.cancel_session(emit).await;
                    emit_ocr_one_shot_failed(&app_handle, "overlayOpenFailed".to_string());
                    Err(reason)
                }
            }
        }
        FinishOutcome::Recognized(result) => {
            // A normalized blank/whitespace-only result is not a real
            // recognition: surface exactly one fixed-safe `emptyResult`
            // failure, create no incoming item/job and return success to the
            // overlay. The text is never logged or exposed.
            if is_blank_ocr_text(&result.text) {
                emit_ocr_one_shot_failed(&app_handle, "emptyResult".to_string());
                return Ok(());
            }

            // Await the shared intake seam before returning. A delivery
            // failure (queue state unavailable, blank/overlong text or inbox
            // full) surfaces only as the fixed safe `intakeFailed` reason.
            match deliver_recognized_text(&app_handle, state.inner(), result.text).await {
                Ok(()) => Ok(()),
                Err(reason) => {
                    emit_ocr_one_shot_failed(&app_handle, reason);
                    Ok(())
                }
            }
        }
        FinishOutcome::RecognizeFailed(reason) => {
            // Only the allowlisted `runtimeUnavailable` is preserved; every
            // other closure reason maps to the fixed safe `recognitionFailed`
            // so arbitrary strings are never emitted to the frontend.
            let reason = if reason == "runtimeUnavailable" {
                "runtimeUnavailable"
            } else {
                "recognitionFailed"
            };
            emit_ocr_one_shot_failed(&app_handle, reason.to_string());
            Ok(())
        }
        FinishOutcome::NoSession => Ok(()),
    }
}

fn register_ocr_capture(
    app_handle: &AppHandle,
    app_state: &AppState,
    hotkey: &Hotkey,
) -> Result<(), String> {
    let shortcut = hotkey
        .to_shortcut()
        .map_err(|e| format!("Invalid OCR hotkey: {e}"))?;
    let global_shortcut = app_handle.global_shortcut();

    let handle_clone = app_handle.clone();
    let state_clone = app_state.clone();
    global_shortcut
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            handle_ocr_capture(&handle_clone, &state_clone);
        })
        .map_err(|e| format!("Failed to register OCR hotkey: {e}"))
}

fn unregister_ocr_capture(app_handle: &AppHandle, hotkey: &Hotkey) -> Result<(), String> {
    let shortcut = hotkey
        .to_shortcut()
        .map_err(|e| format!("Invalid OCR hotkey: {e}"))?;
    let global_shortcut = app_handle.global_shortcut();
    if global_shortcut.is_registered(shortcut) {
        global_shortcut
            .unregister(shortcut)
            .map_err(|e| format!("Failed to unregister OCR hotkey: {e}"))?;
    }
    Ok(())
}

/// Whether a failed OCR start requires an untick persist, and the model id to
/// keep. Returns `Some(model_id)` when the feature is still enabled (the
/// checkbox must be cleared), `None` when it is already disabled.
fn failed_start_untick(enabled: bool, model_id: Option<String>) -> Option<Option<String>> {
    enabled.then_some(model_id)
}

/// Untick the OCR checkbox after a failed start and persist the setting.
///
/// The runtime start already published `Error`; this helper makes the persisted
/// `enabled` flag agree with that truth. `model_id` is kept as-is (a dangling id
/// is allowed per spec) so returning the files restores the feature on re-enable.
/// A persist failure is only warned about: the settings stay truthful (enabled,
/// error visible via status) until the next successful save.
async fn disable_ocr_after_failed_start(app_handle: &AppHandle, state: &AppState) {
    let service = &state.ocr;
    let (enabled, model_id) = {
        let settings = service.settings.read().await;
        (settings.enabled, settings.model_id.clone())
    };

    let Some(model_id) = failed_start_untick(enabled, model_id) else {
        return;
    };

    let Some(settings_manager) = app_handle.try_state::<SettingsManager>() else {
        tracing::warn!("SettingsManager unavailable while disabling OCR after failed start");
        return;
    };

    match super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ocr_section(false, model_id)
    })
    .await
    {
        Ok(()) => {
            service.settings.write().await.enabled = false;
            super::emit_settings_changed(app_handle);
        }
        Err(error) => {
            tracing::warn!(error = %error, "Failed to persist OCR disable after failed start");
        }
    }
}

/// Start the OCR runtime against the current hotkey binding (shared seam used
/// by both the save command and startup boot-apply).
pub(crate) async fn start_ocr_runtime(app_handle: &AppHandle, state: &AppState, hotkey: &Hotkey) {
    let service = &state.ocr;
    let emit = |status: &OcrStatus| emit_ocr_status(app_handle, status);
    let register = |hotkey: &Hotkey| register_ocr_capture(app_handle, state, hotkey);
    service.start(hotkey, emit, register).await;

    // A failed start must clear the persisted enable flag so the checkbox stays
    // truthful ("enabled" only while the feature actually works).
    if matches!(service.status(), OcrStatus::Error { .. }) {
        disable_ocr_after_failed_start(app_handle, state).await;
    }
}

/// Stop the OCR runtime, releasing the shortcut (shared seam used by both the
/// save command and app shutdown).
pub(crate) async fn stop_ocr_runtime(app_handle: &AppHandle, state: &AppState) {
    let service = &state.ocr;
    // Best-effort hide of the selection overlay: a stop during an active
    // `SelectingArea`/`Recognizing` session must not leave the overlay behind.
    let _ = hide_ocr_selection_overlay(app_handle);
    let emit = |status: &OcrStatus| emit_ocr_status(app_handle, status);
    let unregister = |hotkey: &Hotkey| unregister_ocr_capture(app_handle, hotkey);
    service.stop(emit, unregister).await;
}

/// Stop the OCR runtime for app shutdown without blocking on an in-flight
/// recognition.
///
/// Clones the service into `stop_for_shutdown`, which defers the real stop to a
/// background task when a recognition holds the transition lock; app exit is
/// never delayed by OCR, and the process exit reclaims the runtime thread.
pub(crate) async fn stop_ocr_runtime_for_shutdown(app_handle: &AppHandle, state: &AppState) {
    let service = state.ocr.clone();
    let _ = hide_ocr_selection_overlay(app_handle);

    let emit_handle = app_handle.clone();
    let emit = move |status: &OcrStatus| emit_ocr_status(&emit_handle, status);
    let unregister_handle = app_handle.clone();
    let unregister = move |hotkey: &Hotkey| unregister_ocr_capture(&unregister_handle, hotkey);

    service.stop_for_shutdown(emit, unregister).await;
}

/// Re-register the OCR capture shortcut for a changed binding while `Ready`.
///
/// Shared rebind seam used by the hotkey save/reset commands. The OCR service
/// stays the single owner of the shortcut (register-new-first, release-old-
/// after, rollback on failure).
pub(crate) async fn rebind_ocr_capture_hotkey(
    app_handle: &AppHandle,
    state: &AppState,
    hotkey: &Hotkey,
) {
    let service = &state.ocr;
    let emit = |status: &OcrStatus| emit_ocr_status(app_handle, status);
    let register = |hotkey: &Hotkey| register_ocr_capture(app_handle, state, hotkey);
    let unregister = |hotkey: &Hotkey| unregister_ocr_capture(app_handle, hotkey);
    service
        .reregister_hotkey(hotkey, emit, register, unregister)
        .await;
}

#[tauri::command]
pub fn get_ocr_settings(
    settings_manager: State<'_, SettingsManager>,
) -> Result<OcrSettings, String> {
    settings_manager
        .load()
        .map(|settings| settings.ocr)
        .map_err(|e| format!("Failed to load settings: {e}"))
}

#[tauri::command]
pub async fn save_ocr_settings(
    settings: OcrSettings,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;

    let (enabled, model_id) = (settings.enabled, settings.model_id.clone());
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ocr_section(enabled, model_id)
    })
    .await?;

    let service = state.ocr.clone();
    let old_settings = service.settings.read().await.clone();
    let old_hotkey = service.registered_hotkey();

    // Read the current capture binding fresh from disk: the shared
    // `settings_cache` is only refreshed by unrelated save paths and would
    // serve a stale (default) binding here.
    let new_hotkey = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {e}"))?
        .hotkeys
        .ocr_capture;

    {
        let mut snapshot = service.settings.write().await;
        snapshot.enabled = settings.enabled;
        snapshot.model_id = settings.model_id.clone();
    }

    let decision = decide_transition(
        &old_settings,
        &settings,
        old_hotkey.as_ref(),
        Some(&new_hotkey),
        &service.status(),
    );

    match decision {
        OcrTransition::Noop => {}
        OcrTransition::Start => {
            start_ocr_runtime(&app_handle, state.inner(), &new_hotkey).await;
        }
        OcrTransition::Stop => {
            stop_ocr_runtime(&app_handle, state.inner()).await;
        }
        OcrTransition::Restart => {
            stop_ocr_runtime(&app_handle, state.inner()).await;
            start_ocr_runtime(&app_handle, state.inner(), &new_hotkey).await;
        }
        OcrTransition::Retry => {
            start_ocr_runtime(&app_handle, state.inner(), &new_hotkey).await;
        }
        OcrTransition::ReregisterHotkey => {
            let service = &state.ocr;
            let emit = |status: &OcrStatus| emit_ocr_status(&app_handle, status);
            let register =
                |hotkey: &Hotkey| register_ocr_capture(&app_handle, state.inner(), hotkey);
            let unregister = |hotkey: &Hotkey| unregister_ocr_capture(&app_handle, hotkey);
            service
                .reregister_hotkey(&new_hotkey, emit, register, unregister)
                .await;
        }
    }

    super::emit_settings_changed(&app_handle);

    Ok(())
}

#[tauri::command]
pub fn get_ocr_status(state: State<'_, AppState>) -> OcrStatus {
    state.ocr.status()
}

#[tauri::command]
pub fn list_ocr_packs(state: State<'_, AppState>) -> Vec<OcrPackDto> {
    scan_ocr_packs(state.ocr.packs_root())
        .into_iter()
        .map(|pack| OcrPackDto {
            id: pack.id,
            display_name: pack.display_name,
            languages: pack.languages,
        })
        .collect()
}

/// Re-scan the OCR packs directory and reconcile the persisted selection.
///
/// The reconcile follows the shared checkbox rule: when the selected model is
/// gone and not held in memory the feature is disabled with a save (auto-
/// selecting the single remaining model); a dangling selection with exactly one
/// pack auto-selects it without enabling; a model still held in memory is left
/// untouched. Returns the fresh pack list.
#[tauri::command]
pub async fn refresh_ocr_packs(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
) -> Result<Vec<OcrPackDto>, String> {
    let packs = scan_ocr_packs(state.ocr.packs_root());
    let persisted = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {e}"))?
        .ocr;

    let pack_ids: Vec<String> = packs.iter().map(|p| p.id.clone()).collect();

    let holds_in_memory = {
        let snapshot = state.ocr.settings.read().await;
        state.ocr.is_runtime_active() && persisted.model_id.is_some() && *snapshot == persisted
    };

    let decision = decide_ocr_refresh_reconcile(
        persisted.enabled,
        persisted.model_id.as_deref(),
        &pack_ids,
        holds_in_memory,
    );

    match decision {
        OcrRefreshReconcile::None => {}
        OcrRefreshReconcile::Disable { model_id } => {
            let model_id_for_persist = model_id.clone();
            super::persist_blocking(settings_manager.inner(), move |mgr| {
                mgr.set_ocr_section(false, model_id_for_persist)
            })
            .await?;
            {
                let mut snapshot = state.ocr.settings.write().await;
                snapshot.enabled = false;
                snapshot.model_id = model_id;
            }
            // Stop unconditionally: `stop_ocr_runtime` serializes on the
            // service transition lock, so a stop issued while a concurrent
            // startup is still `Starting` waits for that start and tears down
            // whatever it installed. Gating on `is_runtime_active()` would
            // skip the stop while a startup is mid-flight (no shortcut
            // registered yet) and let that start publish `Ready` after
            // `enabled` was already persisted false.
            stop_ocr_runtime(&app_handle, state.inner()).await;
            super::emit_settings_changed(&app_handle);
        }
        OcrRefreshReconcile::AutoSelect(id) => {
            let id_for_persist = id.clone();
            super::persist_blocking(settings_manager.inner(), move |mgr| {
                mgr.set_ocr_section(false, Some(id_for_persist))
            })
            .await?;
            {
                let mut snapshot = state.ocr.settings.write().await;
                snapshot.enabled = false;
                snapshot.model_id = Some(id);
            }
            super::emit_settings_changed(&app_handle);
        }
    }

    Ok(packs
        .into_iter()
        .map(|pack| OcrPackDto {
            id: pack.id,
            display_name: pack.display_name,
            languages: pack.languages,
        })
        .collect())
}

/// Open the configured OCR packs root in the OS file manager.
///
/// The target is resolved only from the managed state's `packs_root()`, so the
/// frontend can never supply an arbitrary path. The root directory is created
/// when missing, canonicalized to a real platform path, then revealed with the
/// platform file manager (Explorer on Windows), mirroring the existing
/// `open_template_folder` command. Model-pack contents are never scanned,
/// downloaded, deleted or modified. Every failure is a safe `Err(String)`.
#[tauri::command]
pub async fn open_ocr_packs_folder(state: State<'_, AppState>) -> Result<(), String> {
    let root = state.ocr.packs_root().to_path_buf();

    // Create the root directory first, before canonicalize.
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;

    let root = root
        .canonicalize()
        .map_err(|e| format!("Invalid OCR packs root: {e}"))?;

    let path = root.to_str().ok_or("Invalid path")?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args([path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args([path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .args([path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_blank_ocr_text;

    #[test]
    fn blank_and_whitespace_only_text_is_detected() {
        assert!(is_blank_ocr_text(""));
        assert!(is_blank_ocr_text(" "));
        assert!(is_blank_ocr_text("\t\n\u{00A0}"));
    }

    #[test]
    fn non_blank_text_is_not_detected() {
        assert!(!is_blank_ocr_text("привет"));
        assert!(!is_blank_ocr_text("  hello world  "));
        assert!(!is_blank_ocr_text("\n\nпервая строка\n\n"));
    }

    #[cfg(windows)]
    #[test]
    fn capture_exclusion_failure_is_non_fatal() {
        let mut called = false;
        super::apply_overlay_capture_exclusion(123, |hwnd, exclude| {
            called = true;
            assert_eq!(hwnd, 123);
            assert!(exclude);
            Err(anyhow::anyhow!("affinity denied"))
        });
        assert!(called, "exclude closure must be invoked");
    }

    #[test]
    fn failed_start_untick_only_when_enabled_and_keeps_model_id() {
        assert_eq!(
            super::failed_start_untick(true, Some("a".to_string())),
            Some(Some("a".to_string()))
        );
        assert_eq!(super::failed_start_untick(true, None), Some(None));
        assert_eq!(
            super::failed_start_untick(false, Some("a".to_string())),
            None
        );
    }

    #[test]
    fn failed_start_disable_persists_enabled_false_with_model_id_kept() {
        use super::SettingsManager;

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-ocr-untick-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let manager = SettingsManager::with_config_dir(dir.clone()).unwrap();

        manager
            .set_ocr_section(true, Some("com.example.ocr".to_string()))
            .unwrap();

        // The failed-start untick persists enabled=false and keeps the model id.
        manager
            .set_ocr_section(false, Some("com.example.ocr".to_string()))
            .unwrap();

        let after = manager.load().unwrap();
        assert!(!after.ocr.enabled);
        assert_eq!(after.ocr.model_id.as_deref(), Some("com.example.ocr"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
