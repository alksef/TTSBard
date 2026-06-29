use crate::playback::{PlaybackManager, PlaybackStateDto};
use std::sync::Arc;
use tauri::State;

pub struct PlaybackState(pub Arc<PlaybackManager>);

#[tauri::command]
pub fn playback_pause(playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    pb.pause();
    Ok(())
}

#[tauri::command]
pub fn playback_resume(playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    pb.resume();
    Ok(())
}

#[tauri::command]
pub fn playback_stop(playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    pb.stop();
    Ok(())
}

#[tauri::command]
pub fn playback_repeat(playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    pb.repeat();
    Ok(())
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
    pb.get_state()
}
