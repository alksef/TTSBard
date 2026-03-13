use crate::state::AppState;
use crate::events::AppEvent;
use crate::config::{SettingsManager, WindowsManager, is_valid_hex_color};
use crate::floating::{show_floating_window, hide_floating_window, hide_soundpanel_window};
use crate::tts::{TtsProviderType, TtsProvider};
use crate::audio::{AudioPlayer, OutputConfig};
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
    if let Some(windows_manager) = app_handle.try_state::<WindowsManager>() {
        // Сохраняем позицию главного окна
        if let Some(main_window) = app_handle.get_webview_window("main") {
            if let Ok(pos) = main_window.outer_position() {
                let x = pos.x;
                let y = pos.y;
                eprintln!("[APP] Saving main window position: {}, {}", x, y);
                let _ = windows_manager.set_main_position(Some(x), Some(y));
            }
        }

        // Сохраняем позицию плавающего окна (если оно было показано)
        if let Some(floating_window) = app_handle.get_webview_window("floating") {
            if let Ok(true) = floating_window.is_visible() {
                if let Ok(pos) = floating_window.outer_position() {
                    let x = pos.x;
                    let y = pos.y;
                    eprintln!("[APP] Saving floating window position: {}, {}", x, y);
                    let _ = windows_manager.set_floating_position(Some(x), Some(y));
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

    // === STAGE 1: Parse prefixes ===
    let prefix_result = crate::preprocessor::parse_prefix(&text);
    let text = prefix_result.text;

    if prefix_result.skip_twitch || prefix_result.skip_webview {
        eprintln!("[PREFIX] Flags - skip_twitch: {}, skip_webview: {}",
            prefix_result.skip_twitch, prefix_result.skip_webview);
    }

    // === STAGE 2: Replacements (existing) ===
    let text = if let Some(preprocessor) = state.get_preprocessor() {
        let processed = preprocessor.process(&text);
        if processed != text {
            eprintln!("[PREPROCESSOR] Replacements: '{}' -> '{}'", text, processed);
        }
        processed
    } else {
        text
    };

    // === STAGE 3: Numbers to text ===
    let text = crate::preprocessor::process_numbers(&text);
    eprintln!("[PREPROCESSOR] Final text for TTS: '{}'", text);

    // Store flags for event handlers
    state.set_prefix_flags(prefix_result.skip_twitch, prefix_result.skip_webview);

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

    // Send message event immediately before playback (synchronized with audio)
    state.emit_event(AppEvent::TextSentToTts(text.clone()));

    // Load audio settings
    let settings_manager = SettingsManager::new()
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let audio_settings = settings_manager.load()
        .map(|s| s.audio)
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

    // Play audio with dual output support (use cached devices if available)
    let mut player = AudioPlayer::new();
    let cached_devices = Some(state.cached_devices.clone());
    player.play_mp3_async_dual(audio_data, speaker_config, virtual_mic_config, cached_devices)?;

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
pub fn get_tts_provider(settings_manager: State<'_, SettingsManager>) -> TtsProviderType {
    settings_manager.get_tts_provider()
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
            let api_key = state.get_openai_api_key();
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
            let _connected = match telegram::telegram_auto_restore(telegram_state, settings_manager.clone()).await {
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
            let url = state.get_local_tts_url();
            state.init_local_tts(url);
            eprintln!("[SET_PROVIDER] Local TTS initialized");
        }
    }

    state.set_tts_provider_type(provider);

    // Save to settings
    settings_manager.set_tts_provider(provider)
        .map_err(|e| format!("Failed to save provider: {}", e))?;

    eprintln!("[SET_PROVIDER] Provider set to {:?}", provider);
    Ok(())
}

// ============================================================================
// Local TTS commands
// ============================================================================

/// Get Local TTS URL
#[tauri::command]
pub fn get_local_tts_url(
    settings_manager: State<'_, SettingsManager>
) -> String {
    settings_manager.get_local_tts_url()
}

/// Set Local TTS URL
#[tauri::command]
pub fn set_local_tts_url(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    url: String,
) -> Result<(), String> {
    eprintln!("[LOCAL_TTS] Setting URL to: {}", url);

    // Validate URL
    if url.is_empty() {
        return Err("URL не может быть пустым".into());
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL должен начинаться с http:// или https://".into());
    }

    // Save to config first
    eprintln!("[LOCAL_TTS] Saving URL to config...");
    settings_manager.set_local_tts_url(url.clone())
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Update runtime state
    eprintln!("[LOCAL_TTS] Updating runtime state...");

    // Collect data with minimal locks (following deadlock prevention pattern)
    let current_provider = {
        let provider = state.tts_providers.lock().clone();
        provider
    };

    // Reinitialize LocalTts if it's the active provider
    if matches!(current_provider.as_ref(), Some(TtsProvider::Local(_))) {
        eprintln!("[LOCAL_TTS] Local TTS is active, reinitializing with new URL...");
        state.init_local_tts(url.clone());
        eprintln!("[LOCAL_TTS] Local TTS reinitialized with URL: {}", url);
    } else {
        eprintln!("[LOCAL_TTS] Local TTS is not active, skipping reinitialization");
    }

    // Update URL in state (always, so it's ready when LocalTts is activated)
    state.set_local_tts_url(url.clone());

    eprintln!("[LOCAL_TTS] URL set successfully: {}", url);
    Ok(())
}

// ============================================================================
// OpenAI TTS commands
// ============================================================================

/// Get OpenAI API key
#[tauri::command]
pub fn get_openai_api_key(state: State<'_, AppState>) -> Option<String> {
    state.get_openai_api_key()
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

    state.set_openai_api_key(Some(key.clone()));
    state.init_openai_tts(key.clone());

    // Save to config
    settings_manager.set_openai_api_key(Some(key))
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

/// Get OpenAI voice
#[tauri::command]
pub fn get_openai_voice(
    settings_manager: State<'_, SettingsManager>
) -> String {
    settings_manager.get_openai_voice()
}

/// Set OpenAI voice
#[tauri::command]
pub fn set_openai_voice(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    voice: String,
) -> Result<(), String> {
    eprintln!("[TTS] Setting OpenAI voice to: {}", voice);

    const VOICES: &[&str] = &["alloy", "echo", "fable", "onyx", "nova", "shimmer"];
    if !VOICES.contains(&voice.as_str()) {
        eprintln!("[TTS] Invalid voice: {}", voice);
        return Err("Неверный голос".into());
    }

    // Save to config first
    eprintln!("[TTS] Saving voice to config...");
    settings_manager.set_openai_voice(voice.clone())
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Update runtime state and reinitialize OpenAI TTS instance
    eprintln!("[TTS] Updating runtime state and reinitializing OpenAI TTS...");
    state.set_openai_voice(voice.clone());

    eprintln!("[TTS] OpenAI voice set successfully: {}", voice);
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
    _state: State<'_, AppState>,
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

    // Save to config
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
#[allow(dead_code)]
pub async fn check_api_key(key: String) -> Result<bool, String> {
    // Simple validation: OpenAI keys start with "sk-" and are longer than 20 characters
    let is_valid = key.starts_with("sk-") && key.len() > 20;
    Ok(is_valid)
}

/// Get floating window appearance settings
#[tauri::command]
pub fn get_floating_appearance(
    windows_manager: State<'_, WindowsManager>
) -> (u8, String) {
    let opacity = windows_manager.get_floating_opacity();
    let color = windows_manager.get_floating_bg_color();
    (opacity, color)
}

/// Set floating window opacity
#[tauri::command]
pub fn set_floating_opacity(
    value: u8,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>
) -> Result<(), String> {
    // Save to config
    windows_manager.set_floating_opacity(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Emit event to update window
    if let Some(state) = app_handle.try_state::<AppState>() {
        state.emit_event(AppEvent::FloatingAppearanceChanged);
    }

    Ok(())
}

/// Set floating window background color
#[tauri::command]
pub fn set_floating_bg_color(
    color: String,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>
) -> Result<(), String> {
    // Validate hex color format
    if !is_valid_hex_color(&color) {
        return Err("Invalid color format. Use #RRGGBB".to_string());
    }

    // Save to config
    windows_manager.set_floating_bg_color(color)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Emit event to update window
    if let Some(state) = app_handle.try_state::<AppState>() {
        state.emit_event(AppEvent::FloatingAppearanceChanged);
    }

    Ok(())
}

/// Toggle clickthrough mode for floating window
#[tauri::command]
pub fn set_clickthrough(
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
    enabled: bool
) -> Result<bool, String> {
    // Save to config
    windows_manager.set_floating_clickthrough(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Apply to window
    if let Some(window) = app_handle.get_webview_window("floating") {
        window.set_ignore_cursor_events(enabled)
            .map_err(|e| format!("Failed to set clickthrough: {}", e))?;
    }

    Ok(enabled)
}

/// Get current clickthrough state
#[tauri::command]
pub fn is_clickthrough_enabled(
    windows_manager: State<'_, WindowsManager>
) -> bool {
    windows_manager.get_floating_clickthrough()
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
) -> Result<bool, String> {
    let is_visible = app_handle.get_webview_window("floating")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

    if is_visible {
        // Window is visible - hide it
        hide_floating_window(&app_handle, &app_state)
            .map_err(|e| format!("Failed to hide window: {}", e))?;
        Ok(false)
    } else {
        // Window is hidden or doesn't exist - show it
        show_floating_window(&app_handle)
            .map_err(|e| format!("Failed to show window: {}", e))?;
        Ok(true)
    }
}

/// Show floating window
#[tauri::command]
pub fn show_floating_window_cmd(
    app_handle: AppHandle,
) -> Result<(), String> {
    show_floating_window(&app_handle)
        .map_err(|e| format!("Failed to show window: {}", e))?;
    Ok(())
}

/// Hide floating window
#[tauri::command]
pub fn hide_floating_window_cmd(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    hide_floating_window(&app_handle, &app_state)
        .map_err(|e| format!("Failed to hide window: {}", e))?;
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
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>
) -> Result<(), String> {
    // Save to disk
    settings_manager.set_hotkey_enabled(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Update runtime state
    state.set_hotkey_enabled(enabled);

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
pub fn get_audio_settings() -> Result<crate::config::AudioSettings, String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.load())
        .map(|s| s.audio)
        .map_err(|e| e.to_string())
}

/// Set speaker device
#[tauri::command]
pub fn set_speaker_device(device_id: Option<String>) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_device(device_id))
        .map_err(|e| e.to_string())
}

/// Set speaker enabled
#[tauri::command]
pub fn set_speaker_enabled(enabled: bool) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_enabled(enabled))
        .map_err(|e| e.to_string())
}

/// Set speaker volume
#[tauri::command]
pub fn set_speaker_volume(volume: u8) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_volume(volume))
        .map_err(|e| e.to_string())
}

/// Set virtual mic device
#[tauri::command]
pub fn set_virtual_mic_device(device_id: Option<String>) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_device(device_id))
        .map_err(|e| e.to_string())
}

/// Enable virtual mic
#[tauri::command]
pub fn enable_virtual_mic() -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_device(Some("".to_string())))  // Enable by setting a device
        .map_err(|e| e.to_string())
}

/// Disable virtual mic
#[tauri::command]
pub fn disable_virtual_mic() -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_device(None))
        .map_err(|e| e.to_string())
}

/// Set virtual mic volume
#[tauri::command]
pub fn set_virtual_mic_volume(volume: u8) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_volume(volume))
        .map_err(|e| e.to_string())
}

// ============================================================================
// Global settings commands
// ============================================================================

/// Set global exclude from capture for all windows
#[tauri::command]
pub fn set_global_exclude_from_capture(
    value: bool,
    _app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>
) -> Result<(), String> {
    eprintln!("[GLOBAL] ===== set_global_exclude_from_capture called with value: {} =====", value);

    // Save to config only - will be applied on app restart due to Windows API limitations
    windows_manager.set_global_exclude_from_capture(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    eprintln!("[GLOBAL] Setting saved. Will apply to all windows after application restart.");
    Ok(())
}

/// Get global exclude from capture setting
#[tauri::command]
pub fn get_global_exclude_from_capture(
    windows_manager: State<'_, WindowsManager>
) -> bool {
    let value = windows_manager.get_global_exclude_from_capture();
    eprintln!("[GLOBAL] get_global_exclude_from_capture called, returning: {}", value);
    value
}

#[tauri::command]
pub fn has_api_key(state: State<'_, AppState>) -> bool {
    state.get_openai_api_key().is_some()
}

// ============================================================================
// Quick editor commands
// ============================================================================

/// Set quick editor enabled
#[tauri::command]
pub fn set_quick_editor_enabled(
    value: bool,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_quick_editor_enabled(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Emit event to notify frontend
    let _ = app_handle.emit("settings-changed", ());

    Ok(())
}

/// Get quick editor enabled
#[tauri::command]
pub fn get_quick_editor_enabled(
    settings_manager: State<'_, SettingsManager>
) -> bool {
    settings_manager.get_quick_editor_enabled()
}

/// Hide main window
#[tauri::command]
pub fn hide_main_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide()
            .map_err(|e| format!("Failed to hide window: {}", e))?;
    }
    Ok(())
}

/// Close floating window and stop interception
#[tauri::command]
pub fn close_floating_window(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    // Останавливаем перехват
    app_state.set_interception_enabled(false);

    // Скрываем окно (сбрасывает F6 режим, сохраняет позицию)
    hide_floating_window(&app_handle, &app_state)
        .map_err(|e| format!("Failed to hide window: {}", e))?;

    Ok(())
}

/// Close soundpanel window and stop interception
#[tauri::command]
pub fn close_soundpanel_window(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
    soundpanel_state: State<'_, crate::soundpanel::SoundPanelState>,
) -> Result<(), String> {
    // Останавливаем перехват в SoundPanelState (это то, что проверяет хук звуковой панели)
    soundpanel_state.set_interception_enabled(false);
    // Также останавливаем основной перехват (для согласованности)
    app_state.set_interception_enabled(false);

    // Скрываем окно (сохраняет позицию)
    hide_soundpanel_window(&app_handle, &app_state)
        .map_err(|e| format!("Failed to hide window: {}", e))?;

    Ok(())
}
