use crate::commands::provider_kind;
use crate::speech_queue::{AcceptedJob, Snapshot, SpeechQueue, SpeechQueueStateDto};
use crate::state::AppState;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Notify;
use uuid::Uuid;

pub struct SpeechQueueState {
    queue: Arc<Mutex<SpeechQueue>>,
    notify: Arc<Notify>,
}

impl SpeechQueueState {
    pub fn new(queue: SpeechQueue) -> Self {
        Self {
            queue: Arc::new(Mutex::new(queue)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub(crate) fn lock(&self) -> parking_lot::MutexGuard<'_, SpeechQueue> {
        self.queue.lock()
    }

    pub(crate) fn notify_one(&self) {
        self.notify.notify_one();
    }

    pub(crate) fn notifier(&self) -> Arc<Notify> {
        self.notify.clone()
    }

    pub(crate) fn queue_arc(&self) -> Arc<Mutex<SpeechQueue>> {
        self.queue.clone()
    }
}

const SPEECH_QUEUE_CHANGED: &str = "speech-queue-changed";

fn emit_queue_changed(app_handle: &AppHandle, dto: SpeechQueueStateDto) {
    let _ = app_handle.emit(SPEECH_QUEUE_CHANGED, dto);
}

pub(crate) fn build_snapshot(state: &AppState, text: &str) -> Result<Snapshot, String> {
    let settings = state.settings_cache.read().clone();
    let prefix_result = crate::preprocessor::parse_prefix(text);

    let registry = state.tts_registry.lock();
    let entry = registry
        .active()
        .ok_or_else(|| "No active TTS provider configured".to_string())?;
    let provider = provider_kind(&entry.provider).to_string();
    let voice = entry.id.clone();
    let tts_provider = entry.provider.clone();
    drop(registry);

    let preprocessor = state.editor.get_preprocessor();
    let network_settings = settings.tts.network.clone();

    Ok(Snapshot {
        provider,
        voice,
        skip_twitch: prefix_result.skip_twitch,
        skip_webview: prefix_result.skip_webview,
        ai_enabled: settings.editor.ai,
        audio_effects: settings.audio_effects,
        dsp: settings.dsp,
        audio: settings.audio,
        ai: settings.ai,
        tts_provider,
        preprocessor,
        network_settings,
    })
}

#[tauri::command]
pub fn submit_speech(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    queue: State<'_, SpeechQueueState>,
    text: String,
) -> Result<AcceptedJob, String> {
    let snapshot = build_snapshot(&state, &text).map_err(|e| format!("Snapshot error: {}", e))?;
    let mut q = queue.lock();
    let job_id = q.submit(&text, snapshot).map_err(|e| e.to_string())?;
    let dto = q.state();
    drop(q);
    emit_queue_changed(&app_handle, dto);
    queue.notify_one();
    Ok(AcceptedJob { job_id })
}

#[tauri::command]
pub fn get_speech_queue_state(queue: State<'_, SpeechQueueState>) -> SpeechQueueStateDto {
    queue.lock().state()
}

#[tauri::command]
pub fn retry_speech_job(
    app_handle: AppHandle,
    queue: State<'_, SpeechQueueState>,
    job_id: Uuid,
) -> Result<(), String> {
    let mut q = queue.lock();
    q.retry_job(job_id).map_err(|e| e.to_string())?;
    let dto = q.state();
    drop(q);
    emit_queue_changed(&app_handle, dto);
    queue.notify_one();
    Ok(())
}

#[tauri::command]
pub fn cancel_speech_job(
    app_handle: AppHandle,
    queue: State<'_, SpeechQueueState>,
    job_id: Uuid,
) -> Result<(), String> {
    let mut q = queue.lock();
    q.cancel_job(job_id).map_err(|e| e.to_string())?;
    let dto = q.state();
    drop(q);
    emit_queue_changed(&app_handle, dto);
    queue.notify_one();
    Ok(())
}

#[tauri::command]
pub fn skip_speech_job(
    app_handle: AppHandle,
    queue: State<'_, SpeechQueueState>,
    job_id: Uuid,
) -> Result<(), String> {
    let mut q = queue.lock();
    q.skip_job(job_id).map_err(|e| e.to_string())?;
    let dto = q.state();
    drop(q);
    emit_queue_changed(&app_handle, dto);
    queue.notify_one();
    Ok(())
}
