use crate::commands::speech_queue::{submit_speech_job, SpeechQueueState};
use crate::config::{validate_port, SettingsManager};
use crate::input_server::service::ConsumeError;
use crate::input_server::{
    IncomingSettings, IncomingTextItem, InputServerError, InputServerSettings, InputServerStatus,
};
use crate::ipc::CommandError;
use crate::speech_queue::SubmissionSource;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

/// Event emitted whenever the pending-review inbox changes.
pub const INCOMING_CHANGED_EVENT: &str = "input-server-incoming-changed";

/// Maximum number of Unicode scalar values accepted in one external submission.
pub const MAX_TEXT_SCALARS: usize = 20_000;

/// Stable input-server error codes for the shared intake seam.
pub mod error_code {
    pub const BLANK: &str = "input_server.blank_text";
    pub const TOO_LONG: &str = "input_server.too_long";
    pub const INBOX_FULL: &str = "input_server.inbox_full";
    pub const UNKNOWN_ITEM: &str = "input_server.unknown_item";
}

/// Stable, serializable result of the shared intake seam.
///
/// Serialized with a snake_case discriminator: `{"status": "queued", ...}` or
/// `{"status": "pending_review", ...}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InputServerAccepted {
    Queued { job_id: Uuid },
    PendingReview { incoming_id: String },
}

fn map_inbox_error(error: InputServerError) -> CommandError {
    match error {
        InputServerError::BlankText => {
            CommandError::new(error_code::BLANK, error.to_string(), false)
        }
        InputServerError::InboxFull => {
            CommandError::new(error_code::INBOX_FULL, error.to_string(), true)
        }
        InputServerError::UnknownId => {
            CommandError::new(error_code::UNKNOWN_ITEM, error.to_string(), false)
        }
    }
}

/// Trim outer whitespace and enforce the external text length/blank limits.
fn normalize_external_text(text: &str) -> Result<String, CommandError> {
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Err(CommandError::new(error_code::BLANK, "Text is blank", false));
    }
    if trimmed.chars().count() > MAX_TEXT_SCALARS {
        return Err(CommandError::new(
            error_code::TOO_LONG,
            "Text is too long",
            false,
        ));
    }
    Ok(trimmed)
}

fn emit_incoming_changed(app_handle: &AppHandle, state: &AppState) {
    let items = state.input_server.pending_items();
    let _ = app_handle.emit(INCOMING_CHANGED_EVENT, items);
}

/// Reusable application-level intake seam for external text.
///
/// Not itself a Tauri boundary. Trims outer whitespace, rejects blank/overlong
/// text, then either submits directly to the speech queue (`incoming.auto_play`)
/// or enqueues into the pending-review inbox. The normalized text (including a
/// leading `!`) is submitted/stored unchanged except for the outer trim. The
/// producer `source` is preserved end to end: auto-play submits the speech job
/// with that source, manual review stores it on the pending item, and approval
/// resubmits with the stored source. Server and OCR producers use the delivery
/// policy derived from the current `incoming.route`.
pub async fn accept_external_text(
    app_handle: &AppHandle,
    state: &AppState,
    queue: &SpeechQueueState,
    source: SubmissionSource,
    text: String,
) -> Result<InputServerAccepted, CommandError> {
    let text = normalize_external_text(&text)?;

    let incoming = state.input_server.incoming.read().await.clone();
    if incoming.auto_play {
        let job = submit_speech_job(
            app_handle,
            state,
            queue,
            text,
            source,
            incoming.route.delivery_policy(),
        )?;
        Ok(InputServerAccepted::Queued { job_id: job.job_id })
    } else {
        let item = state
            .input_server
            .enqueue(&text, source)
            .map_err(map_inbox_error)?;
        emit_incoming_changed(app_handle, state);
        Ok(InputServerAccepted::PendingReview {
            incoming_id: item.id,
        })
    }
}

#[tauri::command]
pub async fn get_input_server_settings(
    state: State<'_, AppState>,
) -> Result<InputServerSettings, String> {
    Ok(state.input_server.settings.read().await.clone())
}

#[tauri::command]
pub fn get_input_server_status(state: State<'_, AppState>) -> InputServerStatus {
    state.input_server.status()
}

#[tauri::command]
pub fn start_input_server(state: State<'_, AppState>) -> Result<(), String> {
    state.input_server.set_run_request(true);
    state.input_server.wake();
    Ok(())
}

#[tauri::command]
pub fn stop_input_server(state: State<'_, AppState>) -> Result<(), String> {
    state.input_server.set_run_request(false);
    state.input_server.wake();
    Ok(())
}

#[tauri::command]
pub async fn save_input_server_settings(
    settings: InputServerSettings,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    validate_port(settings.port).map_err(|e| e.to_string())?;

    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;
    let (start_on_boot, port) = (settings.start_on_boot, settings.port);
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_input_server_section(start_on_boot, port)
    })
    .await?;

    let port_changed = state.input_server.settings.read().await.port != port;

    let mut s = state.input_server.settings.write().await;
    s.start_on_boot = start_on_boot;
    s.port = port;
    drop(s);

    super::emit_settings_changed(&app_handle);

    // A port change rebinds an active/requested listener. Toggling the persisted
    // boot preference must not start/stop the server this session.
    if port_changed {
        state.input_server.wake();
    }

    Ok(())
}

/// Read the source-neutral Incoming policy from the runtime snapshot.
#[tauri::command]
pub async fn get_incoming_settings(state: State<'_, AppState>) -> Result<IncomingSettings, String> {
    Ok(state.input_server.incoming.read().await.clone())
}

/// Persist the source-neutral Incoming policy, update the runtime snapshot and
/// notify the frontend through the global settings-changed event.
#[tauri::command]
pub async fn save_incoming_settings(
    settings: IncomingSettings,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let settings_manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "SettingsManager not available".to_string())?;
    let auto_play = settings.auto_play;
    let route = settings.route;
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_incoming_section(auto_play, route)
    })
    .await?;

    // Persist succeeded: publish the full snapshot (auto_play + route) as one
    // update. On a failed save the `?` above returns early, so runtime state is
    // never mutated.
    *state.input_server.incoming.write().await = settings;

    super::emit_settings_changed(&app_handle);

    Ok(())
}

#[tauri::command]
pub async fn submit_input_server_test(
    text: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
    queue: State<'_, SpeechQueueState>,
) -> Result<InputServerAccepted, CommandError> {
    accept_external_text(
        &app_handle,
        state.inner(),
        queue.inner(),
        SubmissionSource::Server,
        text,
    )
    .await
}

#[tauri::command]
pub fn list_incoming_texts(state: State<'_, AppState>) -> Vec<IncomingTextItem> {
    state.input_server.pending_items()
}

#[tauri::command]
pub async fn approve_incoming_text(
    incoming_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
    queue: State<'_, SpeechQueueState>,
) -> Result<InputServerAccepted, CommandError> {
    // Read the current route at approval time, before the atomic consume. The
    // route is a snapshot; a later change must not affect this job.
    let route = state.input_server.incoming.read().await.route;

    // Submit and remove atomically under the inbox lock so a concurrent
    // approval of the same item can never enqueue a second speech job. The job
    // inherits the producer source stored on the pending item.
    let job = state
        .input_server
        .consume(&incoming_id, |item| {
            submit_speech_job(
                &app_handle,
                state.inner(),
                queue.inner(),
                item.text.clone(),
                item.source,
                route.delivery_policy(),
            )
        })
        .map_err(|error| match error {
            ConsumeError::UnknownId => CommandError::new(
                error_code::UNKNOWN_ITEM,
                "Unknown incoming text item id",
                false,
            ),
            ConsumeError::Rejected(error) => error,
        })?;

    emit_incoming_changed(&app_handle, state.inner());

    Ok(InputServerAccepted::Queued { job_id: job.job_id })
}

#[tauri::command]
pub async fn take_incoming_text_for_edit(
    incoming_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<IncomingTextItem, CommandError> {
    let item = state
        .input_server
        .take(&incoming_id)
        .map_err(map_inbox_error)?;
    emit_incoming_changed(&app_handle, state.inner());
    Ok(item)
}

#[tauri::command]
pub async fn discard_incoming_text(
    incoming_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state
        .input_server
        .discard(&incoming_id)
        .map_err(map_inbox_error)?;
    emit_incoming_changed(&app_handle, state.inner());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_non_empty_text_and_trims_outer_whitespace() {
        assert_eq!(
            normalize_external_text("  hello world  ").unwrap(),
            "hello world"
        );
    }

    #[test]
    fn normalize_preserves_leading_bang() {
        assert_eq!(
            normalize_external_text("  !speak this  ").unwrap(),
            "!speak this"
        );
    }

    #[test]
    fn normalize_rejects_blank_and_whitespace_only() {
        assert!(normalize_external_text("").is_err());
        assert!(normalize_external_text("   ").is_err());
        assert!(normalize_external_text("\t\n").is_err());
    }

    #[test]
    fn normalize_rejects_text_over_limit() {
        let long: String = "a".repeat(MAX_TEXT_SCALARS + 1);
        assert!(normalize_external_text(&long).is_err());
        assert!(normalize_external_text(&"a".repeat(MAX_TEXT_SCALARS)).is_ok());
    }

    #[test]
    fn blank_and_too_long_errors_are_not_retryable() {
        let blank = normalize_external_text("").unwrap_err();
        assert_eq!(blank.code, error_code::BLANK);
        assert!(!blank.retryable);

        let long = normalize_external_text(&"a".repeat(MAX_TEXT_SCALARS + 1)).unwrap_err();
        assert_eq!(long.code, error_code::TOO_LONG);
        assert!(!long.retryable);
    }

    #[test]
    fn inbox_full_error_is_retryable_others_are_not() {
        let full = map_inbox_error(InputServerError::InboxFull);
        assert_eq!(full.code, error_code::INBOX_FULL);
        assert!(full.retryable);

        let blank = map_inbox_error(InputServerError::BlankText);
        assert_eq!(blank.code, error_code::BLANK);
        assert!(!blank.retryable);

        let unknown = map_inbox_error(InputServerError::UnknownId);
        assert_eq!(unknown.code, error_code::UNKNOWN_ITEM);
        assert!(!unknown.retryable);
    }

    #[test]
    fn accepted_result_wire_shape_is_stable() {
        let queued = InputServerAccepted::Queued {
            job_id: Uuid::nil(),
        };
        assert_eq!(
            serde_json::to_value(queued).unwrap(),
            serde_json::json!({
                "status": "queued",
                "job_id": Uuid::nil().to_string()
            })
        );

        let pending = InputServerAccepted::PendingReview {
            incoming_id: "abc".to_string(),
        };
        assert_eq!(
            serde_json::to_value(pending).unwrap(),
            serde_json::json!({
                "status": "pending_review",
                "incoming_id": "abc"
            })
        );
    }
}
