use crate::commands::get_provider_voice_names;
use crate::speech_queue::{AcceptedJob, Snapshot, SpeechQueue, SpeechQueueStateDto};
use crate::state::AppState;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

pub struct SpeechQueueState(pub Arc<Mutex<SpeechQueue>>);

const SPEECH_QUEUE_CHANGED: &str = "speech-queue-changed";

fn emit_queue_changed(app_handle: &AppHandle, dto: SpeechQueueStateDto) {
    let _ = app_handle.emit(SPEECH_QUEUE_CHANGED, dto);
}

fn build_snapshot(state: &AppState, text: &str) -> Snapshot {
    let settings = state.settings_cache.read().clone();
    let prefix_result = crate::preprocessor::parse_prefix(text);

    let (provider, voice) = get_provider_voice_names(state, &settings);

    Snapshot {
        provider,
        voice,
        skip_twitch: prefix_result.skip_twitch,
        skip_webview: prefix_result.skip_webview,
        ai_enabled: settings.editor.ai,
        audio_effects: settings.audio_effects,
        dsp: settings.dsp,
        audio: settings.audio,
        ai: settings.ai,
    }
}

#[tauri::command]
pub fn submit_speech(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    queue: State<'_, SpeechQueueState>,
    text: String,
) -> Result<AcceptedJob, String> {
    let snapshot = build_snapshot(&state, &text);
    let mut q = queue.0.lock();
    let job_id = q.submit(&text, snapshot).map_err(|e| e.to_string())?;
    let dto = q.state();
    drop(q);
    emit_queue_changed(&app_handle, dto);
    Ok(AcceptedJob { job_id })
}

#[tauri::command]
pub fn get_speech_queue_state(queue: State<'_, SpeechQueueState>) -> SpeechQueueStateDto {
    queue.0.lock().state()
}

#[tauri::command]
pub fn retry_speech_job(
    app_handle: AppHandle,
    queue: State<'_, SpeechQueueState>,
    job_id: Uuid,
) -> Result<(), String> {
    let mut q = queue.0.lock();
    q.retry_job(job_id).map_err(|e| e.to_string())?;
    let dto = q.state();
    drop(q);
    emit_queue_changed(&app_handle, dto);
    Ok(())
}

#[tauri::command]
pub fn cancel_speech_job(
    app_handle: AppHandle,
    queue: State<'_, SpeechQueueState>,
    job_id: Uuid,
) -> Result<(), String> {
    let mut q = queue.0.lock();
    q.cancel_job(job_id).map_err(|e| e.to_string())?;
    let dto = q.state();
    drop(q);
    emit_queue_changed(&app_handle, dto);
    Ok(())
}

#[tauri::command]
pub fn skip_speech_job(
    app_handle: AppHandle,
    queue: State<'_, SpeechQueueState>,
    job_id: Uuid,
) -> Result<(), String> {
    let mut q = queue.0.lock();
    q.skip_job(job_id).map_err(|e| e.to_string())?;
    let dto = q.state();
    drop(q);
    emit_queue_changed(&app_handle, dto);
    Ok(())
}
