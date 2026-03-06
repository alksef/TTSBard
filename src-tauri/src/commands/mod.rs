use crate::state::AppState;
use crate::events::AppEvent;
use crate::settings::SettingsManager;
use crate::floating::{show_floating_window, hide_floating_window};
use crate::tts::TtsProviderType;
use crate::audio::{AudioPlayer, AudioSettingsManager, OutputConfig};
use crate::commands::telegram::TelegramState;
use tauri::{State, AppHandle, Manager, Emitter};
use std::sync::Arc;

// Preprocessor commands
pub mod preprocessor;

// Telegram commands
pub mod telegram;

// WebView commands
pub mod webview;

// Twitch commands
pub mod twitch;

/// Quit the application
#[tauri::command]
pub fn quit_app(app_handle: AppHandle) -> Result<(), String> {
    eprintln!("[APP] Quit requested - saving window states");

    // Сохраняем состояние окон перед выходом
    if let Some(settings_manager) = app_handle.try_state::<SettingsManager>() {
        // Сохраняем позицию главного окна
        if let Some(main_window) = app_handle.get_webview_window("main") {
            if let Ok(pos) = main_window.outer_position() {
                let x = pos.x as i32;
                let y = pos.y as i32;
                eprintln!("[APP] Saving main window position: {}, {}", x, y);
                let _ = settings_manager.set_main_window_position(Some(x), Some(y));
            }
        }

        // Проверяем видимость плавающего окна
        let is_visible = app_handle.get_webview_window("floating")
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false);

        eprintln!("[APP] Floating window visible: {}", is_visible);

        // Сохраняем видимость
        let _ = settings_manager.set_floating_window_visibility(is_visible);

        // Если окно видимо, сохраняем его позицию
        if is_visible {
            if let Some(window) = app_handle.get_webview_window("floating") {
                if let Ok(pos) = window.outer_position() {
                    let x = pos.x as i32;
                    let y = pos.y as i32;
                    eprintln!("[APP] Saving floating window position: {}, {}", x, y);
                    let _ = settings_manager.set_floating_window_position(Some(x), Some(y));
                }
            }
        }
    }

    // Emit cleanup event if needed
    let _ = app_handle.emit("app-exit", ());
    // Exit the application cleanly - let Tauri handle cleanup
    app_handle.exit(0);
    Ok(())
}

/// Internal function for TTS synthesis (shared between command and event handler)
/// This function handles the complete TTS pipeline using the configured provider
pub async fn speak_text_internal(state: &AppState, text: String) -> Result<(), String> {
    eprintln!("[SPEAK_INTERNAL] Starting TTS for text: '{}'", text);

    if text.trim().is_empty() {
        return Err("Текст не может быть пустым".to_string());
    }

    // Preprocess text before TTS
    let text = if let Some(preprocessor) = state.get_preprocessor() {
        let processed = preprocessor.process(&text);
        if processed != text {
            eprintln!("[SPEAK_INTERNAL] Text preprocessed: '{}' -> '{}'", text, processed);
        }
        processed
    } else {
        text
    };

    // Get the current TTS provider
    let provider = {
        let providers = state.tts_providers.lock();

        providers.as_ref()
            .ok_or_else(|| {
                eprintln!("[SPEAK_INTERNAL] ERROR: TTS provider not initialized!");
                eprintln!("[SPEAK_INTERNAL] Provider type: {:?}", state.get_tts_provider_type());
                "TTS provider не инициализирован. Выберите провайдер в настройках.".to_string()
            })?
            .clone()
    };

    // Synthesize audio
    let audio_data = provider.synthesize(&text).await
        .map_err(|e| {
            eprintln!("[SPEAK_INTERNAL] synthesize() error: {}", e);
            e
        })?;
    eprintln!("[SPEAK_INTERNAL] Audio synthesized: {} bytes", audio_data.len());

    // Load audio settings
    let audio_settings = AudioSettingsManager::new()
        .and_then(|mgr| mgr.load())
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    // Build speaker config
    let speaker_config = if audio_settings.speaker_enabled {
        Some(OutputConfig {
            device_id: audio_settings.speaker_device,
            volume: audio_settings.speaker_volume as f32 / 100.0,
        })
    } else {
        None
    };

    // Build virtual mic config
    let virtual_mic_config = audio_settings.virtual_mic_device.map(|device_id| OutputConfig {
        device_id: Some(device_id),
        volume: audio_settings.virtual_mic_volume as f32 / 100.0,
    });

    // Check at least one output is enabled
    if speaker_config.is_none() && virtual_mic_config.is_none() {
        return Err("Аудиовывод и виртуальный микрофон выключены. Включите хотя бы один вывод.".to_string());
    }

    // Play audio with dual output support
    let mut player = AudioPlayer::new();
    player.play_mp3_async_dual(audio_data, speaker_config, virtual_mic_config)?;

    eprintln!("[SPEAK_INTERNAL] TTS completed successfully");

    Ok(())
}

/// Manually trigger TTS for given text
#[tauri::command]
pub async fn speak_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    speak_text_internal(&state, text).await
}

// ============================================================================
// Provider selection commands
// ============================================================================

/// Get current TTS provider type
#[tauri::command]
pub fn get_tts_provider(state: State<'_, AppState>) -> TtsProviderType {
    state.get_tts_provider_type()
}

/// Set TTS provider type
#[tauri::command]
pub async fn set_tts_provider(
    state: State<'_, AppState>,
    telegram_state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
    provider: TtsProviderType,
) -> Result<(), String> {
    eprintln!("[SET_PROVIDER] Switching to provider: {:?}", provider);

    // Initialize provider based on type
    match provider {
        TtsProviderType::OpenAi => {
            eprintln!("[SET_PROVIDER] Initializing OpenAI TTS");
            // Get saved API key and initialize if available
            let api_key = state.openai_api_key.lock().clone();
            if let Some(key) = api_key {
                state.init_openai_tts(key);
                eprintln!("[SET_PROVIDER] OpenAI TTS initialized");
            } else {
                eprintln!("[SET_PROVIDER] WARNING: No API key found, OpenAI TTS not initialized");
            }
        }
        TtsProviderType::Silero => {
            eprintln!("[SET_PROVIDER] Initializing Silero TTS");

            // Клонируем Arc заранее, чтобы использовать после telegram_auto_restore
            let client_arc = Arc::clone(&telegram_state.client);

            // Восстанавливаем сессию Telegram (если есть сохранённая)
            eprintln!("[SET_PROVIDER] Checking Telegram session...");
            let _connected = match telegram::telegram_auto_restore(telegram_state).await {
                Ok(connected) => {
                    if connected {
                        eprintln!("[SET_PROVIDER] Telegram session restored");
                    } else {
                        eprintln!("[SET_PROVIDER] No saved Telegram session");
                    }
                    connected
                }
                Err(e) => {
                    eprintln!("[SET_PROVIDER] Telegram check failed: {}", e);
                    false
                }
            };

            // Инициализируем Silero с клиентом (даже если None - пользователь подключится позже)
            state.init_silero_tts(client_arc);
            eprintln!("[SET_PROVIDER] Silero TTS initialized");
        }
        TtsProviderType::Local => {
            eprintln!("[SET_PROVIDER] Initializing Local TTS");
            state.init_local_tts();
            eprintln!("[SET_PROVIDER] Local TTS initialized");
        }
    }

    state.set_tts_provider_type(provider);

    // Auto-save settings
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    eprintln!("[SET_PROVIDER] Provider set to {:?}", provider);
    Ok(())
}

// ============================================================================
// Local TTS commands
// ============================================================================

/// Get Local TTS URL
#[tauri::command]
pub fn get_local_tts_url(state: State<'_, AppState>) -> String {
    state.get_local_tts_url()
}

/// Set Local TTS URL
#[tauri::command]
pub fn set_local_tts_url(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    url: String,
) -> Result<(), String> {
    // Validate URL
    if url.is_empty() {
        return Err("URL не может быть пустым".into());
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL должен начинаться с http:// или https://".into());
    }

    state.set_local_tts_url(url.clone());
    state.init_local_tts();

    // Auto-save settings
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

// ============================================================================
// OpenAI TTS commands
// ============================================================================

/// Get OpenAI API key
#[tauri::command]
pub fn get_openai_api_key(state: State<'_, AppState>) -> Option<String> {
    state.openai_api_key.lock().clone()
}

/// Set OpenAI API key
#[tauri::command]
pub fn set_openai_api_key(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    key: String,
) -> Result<(), String> {
    // Validate API key
    if !key.starts_with("sk-") || key.len() < 20 {
        return Err("Неверный формат API ключа OpenAI".into());
    }

    *state.openai_api_key.lock() = Some(key.clone());
    state.init_openai_tts(key);

    // Auto-save settings
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

/// Get OpenAI voice
#[tauri::command]
pub fn get_openai_voice(state: State<'_, AppState>) -> String {
    state.get_openai_voice()
}

/// Set OpenAI voice
#[tauri::command]
pub fn set_openai_voice(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    voice: String,
) -> Result<(), String> {
    const VOICES: &[&str] = &["alloy", "echo", "fable", "onyx", "nova", "shimmer"];
    if !VOICES.contains(&voice.as_str()) {
        return Err("Неверный голос".into());
    }

    state.set_openai_voice(voice.clone());

    // Auto-save settings
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

/// Get OpenAI proxy settings
#[tauri::command]
pub fn get_openai_proxy(
    settings_manager: State<'_, SettingsManager>,
) -> (Option<String>, Option<u16>) {
    settings_manager.get_openai_proxy()
}

/// Set OpenAI proxy settings
#[tauri::command]
pub fn set_openai_proxy(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    host: Option<String>,
    port: Option<u16>,
) -> Result<(), String> {
    // Validate: both or neither must be set
    match (&host, port) {
        (Some(h), Some(_)) => {
            if h.trim().is_empty() {
                return Err("Хост прокси не может быть пустым".into());
            }
        }
        (None, None) => {}
        _ => return Err("Укажите оба параметра: хост и порт".into()),
    }

    state.set_openai_proxy(host.clone(), port);

    // Auto-save settings
    settings_manager.set_openai_proxy(host, port)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

/// Get interception state (enabled/disabled)
#[tauri::command]
pub fn get_interception(state: State<'_, AppState>) -> bool {
    state.is_interception_enabled()
}

/// Toggle interception mode
#[tauri::command]
pub fn set_interception(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.set_interception_enabled(enabled);
    // Interception больше не автоматически показывает/скрывает окно
    Ok(())
}

/// Toggle interception mode (returns new state)
#[tauri::command]
pub fn toggle_interception(state: State<'_, AppState>) -> Result<bool, String> {
    let current = state.is_interception_enabled();
    let new_value = !current;
    state.set_interception_enabled(new_value);
    Ok(new_value)
}

/// Validate API key format
#[tauri::command]
pub async fn check_api_key(key: String) -> Result<bool, String> {
    // Simple validation: OpenAI keys start with "sk-" and are longer than 20 characters
    let is_valid = key.starts_with("sk-") && key.len() > 20;
    Ok(is_valid)
}

/// Get floating window appearance settings
#[tauri::command]
pub fn get_floating_appearance(state: State<'_, AppState>) -> (u8, String) {
    let opacity = state.get_floating_opacity();
    let color = state.get_floating_bg_color();
    (opacity, color)
}

/// Set floating window opacity
#[tauri::command]
pub fn set_floating_opacity(
    value: u8,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    state.set_floating_opacity(value);

    // Auto-save
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

/// Set floating window background color
#[tauri::command]
pub fn set_floating_bg_color(
    color: String,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    // Validate hex color format
    if !color.starts_with('#') || color.len() != 7 {
        return Err("Invalid color format. Use #RRGGBB".to_string());
    }

    state.set_floating_bg_color(color);

    // Auto-save
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

/// Toggle clickthrough mode for floating window
#[tauri::command]
pub fn set_clickthrough(state: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    // Update state
    state.set_clickthrough(enabled);

    // Emit event to apply to window
    state.emit_event(AppEvent::ClickthroughChanged(enabled));

    Ok(enabled)
}

/// Get current clickthrough state
#[tauri::command]
pub fn is_clickthrough_enabled(state: State<'_, AppState>) -> bool {
    state.is_clickthrough_enabled()
}

/// Get current Enter closes disabled state (F6 mode)
#[tauri::command]
pub fn is_enter_closes_disabled(state: State<'_, AppState>) -> bool {
    state.is_enter_closes_disabled()
}

/// Toggle floating window visibility (show if hidden, hide if visible)
#[tauri::command]
pub fn toggle_floating_window(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<bool, String> {
    let is_visible = app_handle.get_webview_window("floating")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

    if is_visible {
        // Window is visible - hide it
        hide_floating_window(&app_handle, &app_state)
            .map_err(|e| format!("Failed to hide window: {}", e))?;
        // Save state
        settings_manager.set_floating_window_visibility(false)
            .map_err(|e| format!("Failed to save settings: {}", e))?;
        Ok(false)
    } else {
        // Window is hidden or doesn't exist - show it
        show_floating_window(&app_handle)
            .map_err(|e| format!("Failed to show window: {}", e))?;
        // Save state
        settings_manager.set_floating_window_visibility(true)
            .map_err(|e| format!("Failed to save settings: {}", e))?;
        Ok(true)
    }
}

/// Show floating window
#[tauri::command]
pub fn show_floating_window_cmd(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    show_floating_window(&app_handle)
        .map_err(|e| format!("Failed to show window: {}", e))?;
    // Save state
    settings_manager.set_floating_window_visibility(true)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    Ok(())
}

/// Hide floating window
#[tauri::command]
pub fn hide_floating_window_cmd(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    hide_floating_window(&app_handle, &app_state)
        .map_err(|e| format!("Failed to hide window: {}", e))?;
    // Save state
    settings_manager.set_floating_window_visibility(false)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    Ok(())
}

/// Check if floating window is currently visible
#[tauri::command]
pub fn is_floating_window_visible(app_handle: AppHandle) -> bool {
    app_handle.get_webview_window("floating")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// Get hotkey enabled setting
#[tauri::command]
pub fn get_hotkey_enabled(
    settings_manager: State<'_, SettingsManager>
) -> bool {
    settings_manager.get_hotkey_enabled()
}

/// Set hotkey enabled setting
#[tauri::command]
pub fn set_hotkey_enabled(
    enabled: bool,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    // Update state
    state.set_hotkey_enabled(enabled);

    // Save to disk
    settings_manager.set_hotkey_enabled(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

/// Open file dialog for selecting audio files
/// NOTE: This command is not used - dialog is called from frontend using @tauri-apps/plugin-dialog
#[tauri::command]
pub fn open_file_dialog() -> Result<String, String> {
    Err("Use the frontend dialog API instead: import { open } from '@tauri-apps/plugin-dialog'".to_string())
}

// ============================================================================
// Audio commands
// ============================================================================

use crate::audio::OutputDeviceInfo;

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
pub fn get_audio_settings() -> Result<crate::audio::AudioSettings, String> {
    AudioSettingsManager::new()
        .and_then(|mgr| mgr.load())
        .map_err(|e| e.to_string())
}

/// Set speaker device
#[tauri::command]
pub fn set_speaker_device(device_id: Option<String>) -> Result<(), String> {
    AudioSettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_device(device_id))
        .map_err(|e| e.to_string())
}

/// Set speaker enabled
#[tauri::command]
pub fn set_speaker_enabled(enabled: bool) -> Result<(), String> {
    AudioSettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_enabled(enabled))
        .map_err(|e| e.to_string())
}

/// Set speaker volume
#[tauri::command]
pub fn set_speaker_volume(volume: u8) -> Result<(), String> {
    AudioSettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_volume(volume))
        .map_err(|e| e.to_string())
}

/// Set virtual mic device
#[tauri::command]
pub fn set_virtual_mic_device(device_id: Option<String>) -> Result<(), String> {
    AudioSettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_device(device_id))
        .map_err(|e| e.to_string())
}

/// Enable virtual mic
#[tauri::command]
pub fn enable_virtual_mic() -> Result<(), String> {
    AudioSettingsManager::new()
        .and_then(|mgr| mgr.enable_virtual_mic())
        .map_err(|e| e.to_string())
}

/// Disable virtual mic
#[tauri::command]
pub fn disable_virtual_mic() -> Result<(), String> {
    AudioSettingsManager::new()
        .and_then(|mgr| mgr.disable_virtual_mic())
        .map_err(|e| e.to_string())
}

/// Set virtual mic volume
#[tauri::command]
pub fn set_virtual_mic_volume(volume: u8) -> Result<(), String> {
    AudioSettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_volume(volume))
        .map_err(|e| e.to_string())
}

// ============================================================================
// Exclude from recording commands
// ============================================================================

/// Set floating window exclude from recording
#[tauri::command]
pub fn set_floating_exclude_from_recording(
    value: bool,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    state.set_floating_exclude_from_recording(value);

    // Auto-save
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get floating window exclude from recording setting
#[tauri::command]
pub fn get_floating_exclude_from_recording(
    state: State<'_, AppState>
) -> bool {
    state.is_floating_exclude_from_recording()
}

/// Apply exclude from recording to existing floating window
#[tauri::command]
pub fn apply_floating_exclude_recording(
    app_handle: AppHandle,
    state: State<'_, AppState>
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("floating") {
        #[cfg(windows)]
        {
            use crate::window::set_window_exclude_from_capture;

            if let Ok(hwnd) = window.hwnd() {
                let exclude = state.is_floating_exclude_from_recording();
                set_window_exclude_from_capture(hwnd.0 as isize, exclude)
                    .map_err(|e| e.to_string())?;

                eprintln!("[FLOATING] Applied exclude from recording: {}", exclude);
                return Ok(());
            }
        }
        #[cfg(not(windows))]
        {
            // No-op on other platforms
            return Ok(());
        }
    }
    Err("Window not available".to_string())
}
