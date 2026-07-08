use crate::playback::{PlaybackManager, PlaybackStateDto};
use std::sync::Arc;
use tauri::State;
use tracing::debug;

pub struct PlaybackState(pub Arc<PlaybackManager>);

#[tauri::command]
pub fn playback_pause(playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    if pb.pause() {
        Ok(())
    } else {
        Err("Нечего приостановить (воспроизведение не активно)".to_string())
    }
}

#[tauri::command]
pub fn playback_resume(playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    if pb.resume() {
        Ok(())
    } else {
        Err("Нечего возобновить (воспроизведение не приостановлено)".to_string())
    }
}

#[tauri::command]
pub fn playback_stop(playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    if pb.stop() {
        Ok(())
    } else {
        Err("Нечего остановить (воспроизведение не активно)".to_string())
    }
}

#[tauri::command]
pub fn playback_repeat(playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    if pb.repeat() {
        Ok(())
    } else {
        Err("Нечего повторить (воспроизведение не активно)".to_string())
    }
}

#[tauri::command]
pub fn replay_phrase(id: String, playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    pb.replay_from_cache(&id);
    Ok(())
}

#[tauri::command]
pub fn get_playback_state(playback: State<'_, PlaybackState>) -> PlaybackStateDto {
    let pb = &playback.inner().0;
    let dto = pb.get_state();
    debug!(target: "playback", status=?dto.status, current=dto.current.is_some(), recent=dto.recent.len(), "get_playback_state");
    dto
}
