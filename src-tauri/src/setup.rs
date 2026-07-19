// Setup module - Application initialization
//
// This module handles initialization of application components including:
// - Settings loading
// - Window initialization
// - System tray setup
// - Event system setup
// - WebView and Twitch server initialization
//
// Refactored from lib.rs run() setup callback (2026-03-11)

use std::sync::mpsc;
use std::thread;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager};
use tracing::{error, info, warn};

use crate::commands::playback::PlaybackState;
use crate::commands::telegram::TelegramState;
use crate::config::{AppSettings, SettingsManager, WindowsManager, WindowsSettings};
use crate::event_loop::EventHandler;
use crate::events::AppEvent;
use crate::secret_log;
use crate::soundpanel::SoundPanelState;
use crate::state::AppState;
use crate::tts::TtsProviderType;
use std::sync::Arc;

/// Initialize the application (called from Tauri's setup callback)
///
/// Settings are passed from lib.rs to avoid race condition from double loading.
/// Logger is initialized before this function with the same settings.
pub fn init_app(app: &App, settings: AppSettings) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Application setup started ===");

    // Get state managers
    let settings_manager = app.state::<SettingsManager>();
    let windows_manager = app.state::<WindowsManager>();
    let app_state = app.state::<AppState>();
    let telegram_state = app.state::<TelegramState>();
    let soundpanel_state = app.state::<SoundPanelState>();

    info!(tts_provider = ?settings.tts.provider, hotkey_enabled = settings.hotkey_enabled, "Settings loaded");

    let windows = windows_manager.load()?;

    // Load Twitch settings into AppState
    info!("Loading Twitch settings...");
    *app_state.inner().twitch.settings.blocking_write() = settings.twitch.clone();

    // Load VTube Studio settings into AppState
    info!("Loading VTube Studio settings...");
    *app_state.inner().vtube_studio.settings.blocking_write() = settings.vtube_studio.clone();

    // Load WebView settings into AppState
    info!("Loading WebView settings...");
    *app_state.inner().webview.settings.blocking_write() = crate::webview::WebViewSettings {
        enabled: settings.webview.enabled,
        start_on_boot: settings.webview.start_on_boot,
        port: settings.webview.port,
        bind_address: settings.webview.bind_address.clone(),
        access_token: settings.webview.access_token.clone(),
        upnp_enabled: settings.webview.upnp_enabled,
    };

    // Load hotkey_enabled setting into AppState
    info!("Loading hotkey_enabled setting...");
    *app_state.inner().hotkey_enabled.lock() = settings.hotkey_enabled;

    // Setup event system (must be before PlaybackManager)
    let app_handle = app.handle().clone();
    let app_state_for_events = app_state.inner().clone();
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();

    app_state_for_events.set_event_sender(event_tx.clone());

    // Initialize PlaybackManager
    let app_handle_pb = app.handle().clone();
    let initial_speaker = if settings.audio.speaker_enabled {
        Some(crate::audio::OutputConfig {
            device_id: settings.audio.speaker_device.clone(),
            volume: settings.audio.speaker_volume as f32 / 100.0,
        })
    } else {
        None
    };
    let initial_mic =
        settings
            .audio
            .virtual_mic_device
            .clone()
            .map(|dev_id| crate::audio::OutputConfig {
                device_id: Some(dev_id),
                volume: settings.audio.virtual_mic_volume as f32 / 100.0,
            });
    let pb_manager = Arc::new(crate::playback::PlaybackManager::new(
        app_handle_pb,
        event_tx.clone(),
        crate::playback::AudioOutputsConfig {
            speaker: initial_speaker,
            mic: initial_mic,
        },
        Some(app_state.inner().cached_devices.clone()),
    ));
    *app_state.inner().playback_manager.lock() = Some(pb_manager.clone());
    app.manage(PlaybackState(pb_manager));
    info!("Event sender configured in AppState");

    thread::spawn(move || {
        info!("Event thread started, waiting for events...");
        for event in event_rx {
            info!(event = ?std::mem::discriminant(&event), "Received from channel");
            let event_name = event.to_tauri_event();
            let _ = app_handle.emit(event_name, &event);

            let handler = EventHandler::new(app_state_for_events.clone(), app_handle.clone());
            handler.handle(event);
        }
    });

    // Setup SoundPanel event system
    use crate::playback_window::{hide_playback_window, show_playback_window};
    use crate::soundpanel::{load_appearance, load_bindings};
    use crate::soundpanel_window::{
        emit_soundpanel_no_binding, hide_soundpanel_window, show_soundpanel_window,
        update_soundpanel_appearance,
    };

    let (soundpanel_tx, soundpanel_rx) = mpsc::channel::<AppEvent>();
    soundpanel_state.inner().set_event_sender(soundpanel_tx);
    info!("[SOUNDPANEL] Event sender configured");

    let app_handle_for_soundpanel = app.handle().clone();
    let app_state_for_soundpanel = app_state.inner().clone();
    thread::spawn(move || {
        for event in soundpanel_rx {
            let event_name = event.to_tauri_event();
            let _ = app_handle_for_soundpanel.emit(event_name, &event);

            match event {
                AppEvent::ShowSoundPanelWindow => {
                    info!("[SOUNDPANEL] Show soundpanel window");
                    let _ = show_soundpanel_window(&app_handle_for_soundpanel);
                }
                AppEvent::HideSoundPanelWindow => {
                    info!("[SOUNDPANEL] Hide soundpanel window");
                    let _ = hide_soundpanel_window(
                        &app_handle_for_soundpanel,
                        &app_state_for_soundpanel,
                    );
                }
                AppEvent::SoundPanelNoBinding(key) => {
                    info!(key = ?key, "No binding for key");
                    let _ = emit_soundpanel_no_binding(&app_handle_for_soundpanel, key);
                }
                AppEvent::SoundPanelAppearanceChanged => {
                    info!("[SOUNDPANEL] === Appearance changed event received ===");
                    let _ = update_soundpanel_appearance(&app_handle_for_soundpanel);
                }
                AppEvent::TtsProviderChanged(_) => {}
                AppEvent::ShowPlaybackControlWindow => {
                    info!("[PLAYBACK] Show playback control window");
                    let _ = show_playback_window(&app_handle_for_soundpanel);
                }
                AppEvent::HidePlaybackControlWindow => {
                    info!("[PLAYBACK] Hide playback control window");
                    let _ = hide_playback_window(&app_handle_for_soundpanel);
                }
                _ => {}
            }
        }
    });

    // Load soundpanel bindings
    match load_bindings(&soundpanel_state) {
        Ok(bindings) => {
            info!(count = bindings.len(), "Loaded bindings on startup");
        }
        Err(e) => {
            error!(error = %e, "Failed to load bindings");
        }
    }

    match load_appearance(&soundpanel_state, &windows_manager) {
        Ok(appearance) => {
            info!(opacity = appearance.opacity, bg_color = %appearance.bg_color, "[SOUNDPANEL] Loaded appearance");
        }
        Err(e) => {
            error!(error = %e, "Failed to load appearance");
        }
    }

    // Initialize TTS provider
    init_tts_provider(&app_state, &telegram_state, settings.clone());

    // Register discovered Piper providers (no ONNX session created yet)
    app_state.register_piper_providers();

    // Initialize espeak-ng data path for Piper phonemization
    {
        let resource_dir = match app.path().resource_dir() {
            Ok(dir) => Some(dir),
            Err(e) => {
                warn!(error = %e, "resource_dir() failed, espeak-ng data not initialised");
                None
            }
        };
        crate::tts::piper::runtime::LocalModelTts::init_espeak_data(resource_dir);
    }

    // Restore saved concrete provider ID with safe fallback.
    // If the saved ID exists in the registry it is selected; if it was a
    // deleted Piper model the current (built-in) provider is preserved.
    // If nothing is active after fallback the first registered provider wins.
    let saved_id = settings_manager.get_tts_provider_id();
    {
        let mut registry = app_state.tts_registry.lock();
        registry.restore_saved_or_first(saved_id.as_deref());
    }

    // Initialize offline spellcheck
    init_spellcheck(app, &app_state);

    // Initialize windows
    init_windows(app, &windows, &windows_manager, &settings)?;

    // Initialize system tray
    init_tray(app)?;

    // Initialize hooks
    init_hooks(&app_state, &soundpanel_state, app.handle().clone())?;

    // Initialize WebView server
    init_webview_server(&app_state, app.handle().clone());

    // Initialize Twitch client
    init_twitch_client(&app_state, app.handle().clone());

    // Initialize VTube Studio autostart
    init_vtube_studio(&app_state, app.handle().clone());

    // Initialize window protection (Windows only)
    #[cfg(windows)]
    init_window_protection(app, &windows_manager);

    // Set backend ready flag - all initialization complete
    app_state
        .backend_ready
        .store(true, std::sync::atomic::Ordering::SeqCst);
    info!("Backend ready flag set");

    // Show main window after backend is fully initialized
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.show();
        info!("Main window shown");
    }

    // Auto-show playback control window on start if setting enabled
    if settings.show_playback_on_start {
        let _ = crate::playback_window::show_playback_window(app.handle());
    }

    info!("Setup complete - hotkeys will be registered when window gains focus");
    Ok(())
}

/// Initialize TTS provider based on settings
fn init_tts_provider(app_state: &AppState, telegram_state: &TelegramState, settings: AppSettings) {
    info!(provider = ?settings.tts.provider, "Initializing TTS provider");
    app_state.set_tts_provider_type(settings.tts.provider);

    // Always load OpenAI API key if available (for UI display)
    if let Some(ref api_key) = settings.tts.openai.api_key {
        app_state.set_openai_api_key(Some(api_key.clone()));
        info!("OpenAI API key loaded for UI");
    }

    // Always load Fish Audio API key if available (for UI display)
    if let Some(ref api_key) = settings.tts.fish.api_key {
        app_state.set_fish_audio_api_key(Some(api_key.clone()));
        app_state.set_fish_audio_format(settings.tts.fish.format.clone());
        app_state.set_fish_audio_temperature(settings.tts.fish.temperature);
        app_state.set_fish_audio_sample_rate(settings.tts.fish.sample_rate);
        info!("Fish Audio API key loaded for UI");
    }

    match settings.tts.provider {
        TtsProviderType::OpenAi => {
            if let Some(ref api_key) = settings.tts.openai.api_key {
                let api_key_str: String = api_key.clone();
                app_state.set_openai_api_key(Some(api_key_str.clone()));
                info!("OpenAI TTS initialized with API key");
                app_state.init_openai_tts(api_key_str.clone());
                app_state.set_openai_voice(settings.tts.openai.voice.clone());
                // Apply proxy settings respecting use_proxy flag
                let proxy_url = if settings.tts.openai.use_proxy {
                    settings.tts.network.proxy.proxy_url.clone()
                } else {
                    None
                };
                app_state.set_openai_proxy(proxy_url);
            } else {
                warn!("OpenAI selected but no API key found");
            }
        }
        TtsProviderType::Silero => {
            info!("Initializing Silero TTS on startup");
            let client_arc = std::sync::Arc::clone(&telegram_state.client);
            app_state.init_silero_tts(client_arc);
            info!("Silero TTS initialized");
        }
        TtsProviderType::Local => {
            let url = settings.tts.local.url.clone();
            app_state.set_local_tts_url(url.clone());
            app_state.init_local_tts(url);
            info!("Local TTS initialized");
        }
        TtsProviderType::Fish => {
            if let Some(ref api_key) = settings.tts.fish.api_key {
                let api_key_str: String = api_key.clone();
                app_state.set_fish_audio_api_key(Some(api_key_str.clone()));
                app_state.set_fish_audio_format(settings.tts.fish.format.clone());
                app_state.set_fish_audio_temperature(settings.tts.fish.temperature);
                app_state.set_fish_audio_sample_rate(settings.tts.fish.sample_rate);
                info!("Fish Audio API key loaded");
                app_state.init_fish_audio_tts(api_key_str.clone());
                app_state.set_fish_audio_reference_id(settings.tts.fish.reference_id.clone());

                if settings.tts.fish.use_proxy {
                    if let Some(ref proxy_url) = settings.tts.network.proxy.proxy_url {
                        app_state.set_fish_audio_proxy(Some(proxy_url.clone()));
                    }
                }
            } else {
                warn!("Fish Audio selected but no API key found");
            }
        }
    }
}

/// Initialize offline spellcheck (spellbook + Hunspell dictionary)
fn init_spellcheck(app: &App, app_state: &AppState) {
    let res_dir = match app.path().resource_dir() {
        Ok(dir) => dir,
        Err(e) => {
            warn!(error = %e, "[spellcheck] resource_dir() failed (spellcheck disabled)");
            return;
        }
    };

    let dict_dir = res_dir.join("resources").join("dict");
    let aff_path = dict_dir.join("ru.aff");
    let dic_path = dict_dir.join("ru.dic");

    if !aff_path.exists() {
        warn!(path = %secret_log::safe_path_for_log(&aff_path), "[spellcheck] ru.aff not found (spellcheck disabled)");
        return;
    }
    if !dic_path.exists() {
        warn!(path = %secret_log::safe_path_for_log(&dic_path), "[spellcheck] ru.dic not found (spellcheck disabled)");
        return;
    }

    info!(
        aff = %secret_log::safe_path_for_log(&aff_path),
        dic = %secret_log::safe_path_for_log(&dic_path),
        "[spellcheck] loading dictionary..."
    );

    let manager = Arc::new(crate::spellcheck::SpellcheckManager::new(
        aff_path, dic_path,
    ));
    let spellcheck_state = crate::commands::spellcheck::SpellcheckState(manager.clone());
    *app_state.editor.spellcheck_manager.lock() = Some(manager);
    app.manage(spellcheck_state);
    info!("[spellcheck] initialized");
}

/// Initialize application windows
fn init_windows(
    app: &App,
    windows: &WindowsSettings,
    _windows_manager: &WindowsManager,
    settings: &AppSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("State initialized");

    // Apply saved main window position (window will be shown after backend is ready)
    if let Some(main_window) = app.get_webview_window("main") {
        if let Some(x) = windows.main.x {
            if let Some(y) = windows.main.y {
                info!(x, y, "Restoring main window position");
                let _ = main_window
                    .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
            }
        }

        // Apply theme to the Tauri window itself to ensure titlebar and OS frames match
        let tauri_theme = match settings.theme {
            crate::config::Theme::Light => tauri::Theme::Light,
            crate::config::Theme::Dark => tauri::Theme::Dark,
        };
        let _ = main_window.set_theme(Some(tauri_theme));
        info!(?tauri_theme, "Applied initial window theme");
    }

    if let Some(pb_window) = app.get_webview_window("playback-control") {
        let tauri_theme = match settings.theme {
            crate::config::Theme::Light => tauri::Theme::Light,
            crate::config::Theme::Dark => tauri::Theme::Dark,
        };
        let _ = pb_window.set_theme(Some(tauri_theme));
    }

    Ok(())
}

/// Initialize system tray
fn init_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    // Load icon.png (512x512) for tray
    let png_bytes = include_bytes!("../icons/icon.png");
    let decoded_image = image::load_from_memory(png_bytes).expect("Failed to decode tray icon");
    let rgba_image = decoded_image.to_rgba8();
    let (width, height) = (rgba_image.width(), rgba_image.height());

    // Resize to 32x32 for tray
    let resized =
        image::imageops::resize(&rgba_image, 32, 32, image::imageops::FilterType::Lanczos3);
    let tray_icon = Image::new_owned(resized.into_raw(), 32, 32);

    info!(
        width,
        height, "Initializing system tray with icon (resized to 32x32 from original)"
    );

    // Create context menu (only "Quit" item)
    let quit_item =
        MenuItem::with_id(&app_handle, "quit", "Выход", true, None as Option<&str>).unwrap();

    let menu = Menu::with_items(&app_handle, &[&quit_item]).unwrap();

    // Create tray icon
    info!("[TRAY] Creating tray icon...");
    let _ = TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .tooltip("TTSBard")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if matches!(button, tauri::tray::MouseButton::Left)
                    && matches!(button_state, tauri::tray::MouseButtonState::Up)
                {
                    if let Some(window) = tray.app_handle().get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .on_menu_event(|tray, event| {
            if event.id.as_ref() == "quit" {
                tray.app_handle().exit(0);
            }
        })
        .menu(&menu)
        .build(&app_handle);
    info!("[TRAY] Tray icon created successfully");

    Ok(())
}

/// Initialize keyboard hooks
fn init_hooks(
    app_state: &AppState,
    soundpanel_state: &SoundPanelState,
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::soundpanel::initialize_soundpanel_hook;

    let hook_manager = initialize_soundpanel_hook(soundpanel_state.clone(), app_handle);
    *app_state.soundpanel_hook.lock() = Some(hook_manager);
    info!("[SOUNDPANEL] Keyboard hook initialized");

    Ok(())
}

/// Initialize WebView server
fn init_webview_server(app_state: &AppState, app_handle: AppHandle) {
    let webview_settings = app_state.webview.settings.clone();
    let (webview_tx, webview_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let shutdown = app_state.shutdown.clone();

    app_state.webview.set_event_sender(webview_tx);

    app_state.runtime.spawn(async move {
        crate::servers::run_webview_server(webview_settings, app_handle, webview_rx, shutdown)
            .await;
    });
}

/// Initialize Twitch client
fn init_twitch_client(app_state: &AppState, app_handle: AppHandle) {
    let app_state_clone = app_state.clone();
    let twitch_rx = app_state.twitch.event_tx.subscribe();
    let shutdown = app_state.shutdown.clone();

    app_state.runtime.spawn(async move {
        crate::servers::run_twitch_client(app_state_clone, app_handle, twitch_rx, shutdown).await;
    });
}

/// Initialize window protection (Windows only)
#[cfg(windows)]
fn init_window_protection(app: &App, windows_manager: &WindowsManager) {
    use crate::window::set_window_exclude_from_capture;

    let exclude_from_capture = windows_manager.get_global_exclude_from_capture();
    info!(
        exclude_from_capture,
        "Applying global exclude from capture to main window"
    );

    if let Some(main_window) = app.get_webview_window("main") {
        if let Ok(hwnd) = main_window.hwnd() {
            match set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture) {
                Ok(_) => info!(
                    exclude_from_capture,
                    "Main window exclude from capture applied"
                ),
                Err(e) => error!(error = %e, "Failed to apply exclude from capture to main window"),
            }
        }
    }
}

fn init_vtube_studio(app_state: &AppState, app_handle: AppHandle) {
    let start_on_boot = {
        let settings = app_state.vtube_studio.settings.blocking_read();
        settings.start_on_boot
    };

    if !start_on_boot {
        info!("VTube Studio autostart disabled (start_on_boot=false)");
        return;
    }

    info!("VTube Studio: auto-start on boot");

    let app_handle_clone = app_handle.clone();
    let app_state_clone = app_state.clone();

    app_state.runtime.spawn(async move {
        let (port, stored_token) = {
            let settings = app_state_clone.vtube_studio.settings.read().await;
            (settings.port, settings.token.clone())
        };

        info!(
            port,
            has_token = stored_token.is_some(),
            "VTube Studio: attempting autostart connection"
        );

        let result = app_state_clone
            .vtube_studio
            .connect(port, stored_token.as_deref())
            .await;

        let status = app_state_clone.vtube_studio.get_connection_status();
        let _ = app_handle_clone.emit("vtube-studio-status-changed", &status);

        match result {
            Ok(new_token) => {
                info!("VTube Studio: autostart connected successfully");
                if let Some(ref tok) = new_token {
                    let mut s = app_state_clone.vtube_studio.settings.write().await;
                    s.token = Some(tok.clone());
                    drop(s);

                    let mgr_opt = app_handle_clone.try_state::<SettingsManager>();
                    if let Some(mgr) = mgr_opt {
                        let inner = mgr.inner().clone();
                        let tok_clone = tok.clone();
                        let _ = crate::commands::persist_blocking(&inner, move |m| {
                            let mut vts = m.get_vtube_studio_settings();
                            vts.token = Some(tok_clone);
                            m.set_vtube_studio_settings(&vts)
                        })
                        .await;
                    }
                }
            }
            Err(e) => {
                info!(error = %e, "VTube Studio: autostart connection failed (non-fatal)");
            }
        }
    });
}

/// Parse WebView server startup errors and provide user-friendly messages
pub(crate) fn parse_webview_server_error(
    error_msg: &str,
    bind_address: String,
    port: u16,
) -> (String, String) {
    let log_context = format!(
        "Failed to start WebView server on {}:{}",
        bind_address, port
    );

    let user_friendly_msg =
        if error_msg.contains("addr in use") || error_msg.contains("port in use") {
            format!(
                "Порт {} уже занят. Пожалуйста, выберите другой порт в настройках WebView.",
                port
            )
        } else if error_msg.contains("permission denied") {
            format!(
                "Нет прав для запуска сервера на порту {}. Попробуйте использовать порт выше 1024.",
                port
            )
        } else if error_msg.contains("invalid input") || error_msg.contains("invalid address") {
            format!(
                "Некорректный адрес {}:{}. Пожалуйста, проверьте настройки WebView.",
                bind_address, port
            )
        } else if error_msg.contains("access denied") {
            "Доступ запрещен. Возможно, брандмауэр блокирует соединение.".to_string()
        } else {
            format!(
                "Не удалось запустить WebView сервер: {}",
                if error_msg.len() > 100 {
                    format!("{}...", &error_msg[..97])
                } else {
                    error_msg.to_string()
                }
            )
        };

    (user_friendly_msg, log_context)
}
