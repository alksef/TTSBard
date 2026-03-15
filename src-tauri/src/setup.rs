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
use tauri::{App, AppHandle, Manager, Emitter};
use tauri::image::Image;
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::menu::{Menu, MenuItem};
use tracing::{info, warn, error};

use crate::state::AppState;
use crate::events::AppEvent;
use crate::config::{SettingsManager, WindowsManager, AppSettings, WindowsSettings};
use crate::soundpanel::SoundPanelState;
use crate::commands::telegram::TelegramState;
use crate::tts::TtsProviderType;
use crate::event_loop::EventHandler;

/// Initialize the application (called from Tauri's setup callback)
///
/// Settings are passed from lib.rs to avoid race condition from double loading.
/// Logger is initialized before this function with the same settings.
pub fn init_app(app: &App, settings: AppSettings) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Application setup started ===");

    // Get state managers
    let _settings_manager = app.state::<SettingsManager>();
    let windows_manager = app.state::<WindowsManager>();
    let app_state = app.state::<AppState>();
    let telegram_state = app.state::<TelegramState>();
    let soundpanel_state = app.state::<SoundPanelState>();

    info!(tts_provider = ?settings.tts.provider, hotkey_enabled = settings.hotkey_enabled, "Settings loaded");

    let windows = windows_manager.load()?;
    info!(floating_opacity = windows.floating.opacity, floating_clickthrough = windows.floating.clickthrough, "Windows settings");

    // Load Twitch settings into AppState
    info!("Loading Twitch settings...");
    *app_state.inner().twitch_settings.blocking_write() = settings.twitch.clone();

    // Load WebView settings into AppState
    info!("Loading WebView settings...");
    *app_state.inner().webview_settings.blocking_write() = crate::webview::WebViewSettings {
        enabled: settings.webview.enabled,
        start_on_boot: settings.webview.start_on_boot,
        port: settings.webview.port,
        bind_address: settings.webview.bind_address.clone(),
    };

    // Load hotkey_enabled setting into AppState
    info!("Loading hotkey_enabled setting...");
    *app_state.inner().hotkey_enabled.lock() = settings.hotkey_enabled;

    // Setup event system
    let app_handle = app.handle().clone();
    let app_state_for_events = app_state.inner().clone();
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();

    app_state_for_events.set_event_sender(event_tx.clone());
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
    use crate::floating::{show_soundpanel_window, hide_soundpanel_window, emit_soundpanel_no_binding, update_soundpanel_appearance};
    use crate::soundpanel::{load_bindings, load_appearance};

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
                    let _ = hide_soundpanel_window(&app_handle_for_soundpanel, &app_state_for_soundpanel);
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
    init_tts_provider(&app_state, &telegram_state, settings);

    // Initialize windows
    init_windows(app, &windows, &windows_manager)?;

    // Initialize system tray
    init_tray(app)?;

    // Initialize hooks
    init_hooks(&app_state, &soundpanel_state)?;

    // Initialize WebView server
    init_webview_server(&app_state, app.handle().clone());

    // Initialize Twitch client
    init_twitch_client(&app_state, app.handle().clone());

    // Initialize window protection (Windows only)
    #[cfg(windows)]
    init_window_protection(app, &windows_manager);

    // Set backend ready flag - all initialization complete
    app_state.backend_ready.store(true, std::sync::atomic::Ordering::SeqCst);
    info!("Backend ready flag set");

    // Show main window after backend is fully initialized
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.show();
        info!("Main window shown");
    }

    info!("Setup complete - hotkeys will be registered when window gains focus");
    Ok(())
}

/// Initialize TTS provider based on settings
fn init_tts_provider(
    app_state: &AppState,
    telegram_state: &TelegramState,
    settings: AppSettings,
) {
    info!(provider = ?settings.tts.provider, "Initializing TTS provider");
    app_state.set_tts_provider_type(settings.tts.provider);

    // Always load OpenAI API key if available (for UI display)
    if let Some(ref api_key) = settings.tts.openai.api_key {
        app_state.set_openai_api_key(Some(api_key.clone()));
        info!("OpenAI API key loaded for UI");
    }

    match settings.tts.provider {
        TtsProviderType::OpenAi => {
            if let Some(ref api_key) = settings.tts.openai.api_key {
                let api_key_str: String = api_key.clone();
                app_state.set_openai_api_key(Some(api_key_str.clone()));
                info!("OpenAI TTS initialized with API key");
                app_state.init_openai_tts(api_key_str.clone());
                app_state.set_openai_voice(settings.tts.openai.voice.clone());
                // Use legacy method to convert host/port to proxy URL
                app_state.set_openai_proxy_legacy(settings.tts.openai.proxy_host.clone(), settings.tts.openai.proxy_port);
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
    }
}

/// Initialize application windows
fn init_windows(
    app: &App,
    windows: &WindowsSettings,
    _windows_manager: &WindowsManager,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("State initialized");

    // Apply saved main window position (window will be shown after backend is ready)
    if let Some(main_window) = app.get_webview_window("main") {
        if let Some(x) = windows.main.x {
            if let Some(y) = windows.main.y {
                info!(x, y, "Restoring main window position");
                let _ = main_window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
            }
        }
    }

    Ok(())
}

/// Initialize system tray
fn init_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    // Load icon.png (512x512) for tray
    let png_bytes = include_bytes!("../icons/icon.png");
    let decoded_image = image::load_from_memory(png_bytes)
        .expect("Failed to decode tray icon");
    let rgba_image = decoded_image.to_rgba8();
    let (width, height) = (rgba_image.width(), rgba_image.height());

    // Resize to 32x32 for tray
    let resized = image::imageops::resize(&rgba_image, 32, 32, image::imageops::FilterType::Lanczos3);
    let tray_icon = Image::new_owned(resized.into_raw(), 32, 32);

    info!(width, height, "Initializing system tray with icon (resized to 32x32 from original)");

    // Create context menu (only "Quit" item)
    let quit_item = MenuItem::with_id(
        &app_handle,
        "quit",
        "Выход",
        true,
        None as Option<&str>
    ).unwrap();

    let menu = Menu::with_items(&app_handle, &[&quit_item]).unwrap();

    // Create tray icon
    info!("[TRAY] Creating tray icon...");
    let _ = TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .tooltip("TTSBard")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, button_state, .. } = event {
                if matches!(button, tauri::tray::MouseButton::Left) && matches!(button_state, tauri::tray::MouseButtonState::Up) {
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
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::hook::initialize_text_interception_hook;
    use crate::soundpanel::initialize_soundpanel_hook;

    let _hook_handle = initialize_text_interception_hook(app_state.clone());
    info!("Keyboard hook initialized");

    let _soundpanel_hook_handle = initialize_soundpanel_hook(soundpanel_state.clone());
    info!("[SOUNDPANEL] Keyboard hook initialized");

    Ok(())
}

/// Initialize WebView server
fn init_webview_server(app_state: &AppState, app_handle: AppHandle) {
    let webview_settings = app_state.webview_settings.clone();
    let (webview_tx, webview_rx) = std::sync::mpsc::channel::<AppEvent>();

    app_state.set_webview_event_sender(webview_tx);

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()
            .expect("Failed to create tokio runtime for webview");

        rt.block_on(async move {
            crate::servers::run_webview_server(
                webview_settings,
                app_handle,
                webview_rx,
            ).await;
        });
    });
}

/// Initialize Twitch client
fn init_twitch_client(app_state: &AppState, app_handle: AppHandle) {
    let app_state_clone = app_state.clone();
    let twitch_rx = app_state.twitch_event_tx.subscribe();

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Twitch tokio runtime");

        rt.block_on(async move {
            crate::servers::run_twitch_client(
                app_state_clone.clone(),
                app_handle,
                twitch_rx,
            ).await;
        });
    });

    // Auto-start Twitch if configured
    let app_state_autostart = app_state.clone();
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Twitch autostart runtime");

        rt.block_on(async move {
            let settings = app_state_autostart.twitch_settings.read().await;
            if settings.start_on_boot && settings.enabled {
                if let Ok(()) = settings.is_valid() {
                    info!("[TWITCH] Auto-starting on boot");
                    app_state_autostart.send_twitch_event(crate::events::TwitchEvent::Restart);
                    info!("[TWITCH] Restart event sent for auto-start");
                }
            }
        });
    });
}

/// Initialize window protection (Windows only)
#[cfg(windows)]
fn init_window_protection(app: &App, windows_manager: &WindowsManager) {
    use crate::window::set_window_exclude_from_capture;

    let exclude_from_capture = windows_manager.get_global_exclude_from_capture();
    info!(exclude_from_capture, "Applying global exclude from capture to main window");

    if let Some(main_window) = app.get_webview_window("main") {
        if let Ok(hwnd) = main_window.hwnd() {
            match set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture) {
                Ok(_) => info!(exclude_from_capture, "Main window exclude from capture applied"),
                Err(e) => error!(error = %e, "Failed to apply exclude from capture to main window"),
            }
        }
    }
}

/// Parse WebView server startup errors and provide user-friendly messages
pub(crate) fn parse_webview_server_error(error_msg: &str, bind_address: String, port: u16) -> (String, String) {
    let log_context = format!("Failed to start WebView server on {}:{}", bind_address, port);

    let user_friendly_msg = if error_msg.contains("addr in use") || error_msg.contains("port in use") {
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
