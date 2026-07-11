use crate::config::SettingsManager;
use crate::audio::OutputDeviceInfo;
use crate::playback::{PlaybackManager, PlaybackStateDto};
use std::sync::Arc;
use tauri::State;
use tracing::{info, debug};

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

/// Get all output devices
#[tauri::command]
pub fn get_output_devices() -> Vec<OutputDeviceInfo> {
    crate::audio::get_output_devices()
}

/// Get virtual mic devices only
#[tauri::command]
pub fn get_virtual_mic_devices() -> Vec<OutputDeviceInfo> {
    crate::audio::get_virtual_mic_devices()
}

/// Get current audio settings
#[tauri::command]
pub fn get_audio_settings(
    settings_manager: State<'_, SettingsManager>,
) -> Result<crate::config::AudioSettings, String> {
    settings_manager
        .load()
        .map(|s| s.audio)
        .map_err(|e| e.to_string())
}

/// Set speaker device
#[tauri::command]
pub fn set_speaker_device(
    device_id: Option<String>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager
        .set_speaker_device(device_id)
        .map_err(|e| e.to_string())
}

/// Set speaker enabled
#[tauri::command]
pub fn set_speaker_enabled(
    enabled: bool,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager
        .set_speaker_enabled(enabled)
        .map_err(|e| e.to_string())
}

/// Set speaker volume
#[tauri::command]
pub fn set_speaker_volume(
    volume: u8,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager
        .set_speaker_volume(volume)
        .map_err(|e| e.to_string())
}

/// Set virtual mic device
#[tauri::command]
pub fn set_virtual_mic_device(
    device_id: Option<String>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager
        .set_virtual_mic_device(device_id)
        .map_err(|e| e.to_string())
}

/// Enable virtual mic
#[tauri::command]
pub fn enable_virtual_mic(
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager
        .set_virtual_mic_device(Some("".to_string())) // Enable by setting a device
        .map_err(|e| e.to_string())
}

/// Disable virtual mic
#[tauri::command]
pub fn disable_virtual_mic(
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager
        .set_virtual_mic_device(None)
        .map_err(|e| e.to_string())
}

/// Set virtual mic volume
#[tauri::command]
pub fn set_virtual_mic_volume(
    volume: u8,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager
        .set_virtual_mic_volume(volume)
        .map_err(|e| e.to_string())
}

/// Test playback on a specific audio device
/// Plays a short test sound on the specified device with the given volume
#[tauri::command]
pub fn test_audio_device(device_id: Option<String>, volume: u8) -> Result<(), String> {
    info!(?device_id, volume, "Testing audio device");

    let mp3_data = crate::assets::TEST_SOUND_MP3.to_vec();

    let config = crate::audio::OutputConfig {
        device_id,
        volume: volume as f32 / 100.0,
    };

    let mut player = crate::audio::AudioPlayer::new();
    player.play_test_sound_blocking(mp3_data, config)?;

    info!("Test sound playback completed");
    Ok(())
}

/// Get audio effects settings
#[tauri::command]
pub fn get_audio_effects(
    settings_manager: State<'_, SettingsManager>
) -> crate::config::AudioEffectsSettings {
    settings_manager.get_audio_effects()
}

/// Set audio effects enabled
#[tauri::command]
pub fn set_audio_effects_enabled(
    enabled: bool,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_enabled(enabled)
        .map_err(|e| e.to_string())
}

/// Set audio effects pitch
#[tauri::command]
pub fn set_audio_effects_pitch(
    pitch: i16,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_pitch(pitch)
        .map_err(|e| e.to_string())
}

/// Set audio effects speed
#[tauri::command]
pub fn set_audio_effects_speed(
    speed: i16,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_speed(speed)
        .map_err(|e| e.to_string())
}

/// Set audio effects volume
#[tauri::command]
pub fn set_audio_effects_volume(
    volume: i16,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_volume(volume)
        .map_err(|e| e.to_string())
}

/// Set audio effects enhance (DeepFilterNet noise suppression) enabled
#[tauri::command]
pub fn set_audio_effects_enhance_enabled(
    enabled: bool,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_enhance_enabled(enabled)
        .map_err(|e| e.to_string())
}

/// Set audio effects enhance attenuation limit (dB, 5..30)
#[tauri::command]
pub fn set_audio_effects_enhance_atten_db(
    atten_db: f32,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_enhance_atten_db(atten_db)
        .map_err(|e| e.to_string())
}
