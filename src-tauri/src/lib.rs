mod commands;
mod audio;
mod config;
mod events;
mod floating;
mod hook;
mod hotkeys;
mod state;
mod tts;
mod window;
mod soundpanel;
mod preprocessor;
mod telegram;
mod webview;
mod twitch;
mod rate_limiter;
mod thread_manager;

use std::sync::mpsc;
use std::thread;
use state::AppState;
use events::{AppEvent, InputLayout, TwitchEvent};
use hotkeys::initialize_hotkeys;
use hook::initialize_text_interception_hook;
use commands::{speak_text, get_tts_provider, set_tts_provider, get_local_tts_url, set_local_tts_url, get_openai_api_key, set_openai_api_key, get_openai_voice, set_openai_voice, get_openai_proxy, set_openai_proxy, get_interception, set_interception, has_api_key, get_floating_appearance, set_floating_opacity, set_floating_bg_color, set_clickthrough, is_clickthrough_enabled, is_enter_closes_disabled, toggle_interception, toggle_floating_window, show_floating_window_cmd, hide_floating_window_cmd, is_floating_window_visible, quit_app, get_hotkey_enabled, set_hotkey_enabled, get_global_exclude_from_capture, set_global_exclude_from_capture, open_file_dialog, get_output_devices, get_virtual_mic_devices, get_audio_settings, set_speaker_device, set_speaker_enabled, set_speaker_volume, set_virtual_mic_device, enable_virtual_mic, disable_virtual_mic, set_virtual_mic_volume};
use commands::telegram::{telegram_init, telegram_request_code, telegram_sign_in, telegram_sign_out, telegram_get_status, telegram_get_user, telegram_auto_restore, TelegramState};
use soundpanel::{SoundPanelState, sp_get_bindings, sp_add_binding, sp_remove_binding, sp_test_sound, sp_is_supported_format, sp_get_floating_appearance, sp_set_floating_opacity, sp_set_floating_bg_color, sp_set_floating_clickthrough, sp_is_floating_clickthrough_enabled, load_bindings, load_appearance, initialize_soundpanel_hook};
use floating::{show_floating_window, hide_floating_window, update_floating_text, update_floating_title, show_soundpanel_window, hide_soundpanel_window, emit_soundpanel_no_binding, update_soundpanel_appearance};
use config::{SettingsManager, WindowsManager};
use webview::WebViewServer;
use twitch::TwitchClient;
use tauri::{Manager, Emitter};
use tauri::image::Image;
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// Обновление иконки трея в зависимости от состояния перехвата
fn update_tray_icon(_app_handle: &tauri::AppHandle, is_intercepting: bool) {
    eprintln!("[TRAY] Interception mode: {}, tray icon update skipped (not implemented)", is_intercepting);
    // TODO: Implement tray icon update with proper resource paths
    // For now, keep the default icon
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Инициализируем состояние (event_sender будет установлен позже в setup)
    let app_state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .manage(TelegramState::new())
        .invoke_handler(tauri::generate_handler![
            greet,
            speak_text,
            get_tts_provider,
            set_tts_provider,
            get_local_tts_url,
            set_local_tts_url,
            get_openai_api_key,
            set_openai_api_key,
            get_openai_voice,
            set_openai_voice,
            get_openai_proxy,
            set_openai_proxy,
            get_interception,
            set_interception,
            toggle_interception,
            has_api_key,
            get_floating_appearance,
            set_floating_opacity,
            set_floating_bg_color,
            set_clickthrough,
            is_clickthrough_enabled,
            is_enter_closes_disabled,
            toggle_floating_window,
            show_floating_window_cmd,
            hide_floating_window_cmd,
            is_floating_window_visible,
            quit_app,
            get_hotkey_enabled,
            set_hotkey_enabled,
            // Global settings commands
            get_global_exclude_from_capture,
            set_global_exclude_from_capture,
            sp_get_bindings,
            sp_add_binding,
            sp_remove_binding,
            sp_test_sound,
            sp_is_supported_format,
            sp_get_floating_appearance,
            sp_set_floating_opacity,
            sp_set_floating_bg_color,
            sp_set_floating_clickthrough,
            sp_is_floating_clickthrough_enabled,
            open_file_dialog,
            // Audio commands
            get_output_devices,
            get_virtual_mic_devices,
            get_audio_settings,
            set_speaker_device,
            set_speaker_enabled,
            set_speaker_volume,
            set_virtual_mic_device,
            enable_virtual_mic,
            disable_virtual_mic,
            set_virtual_mic_volume,
            // Preprocessor commands
            commands::preprocessor::get_replacements,
            commands::preprocessor::save_replacements,
            commands::preprocessor::get_usernames,
            commands::preprocessor::save_usernames,
            commands::preprocessor::preview_preprocessing,
            commands::preprocessor::load_preprocessor_data,
            // Telegram commands
            telegram_init,
            telegram_request_code,
            telegram_sign_in,
            telegram_sign_out,
            telegram_get_status,
            telegram_get_user,
            telegram_auto_restore,
            commands::telegram::speak_text_silero,
            commands::telegram::telegram_get_current_voice,
            commands::telegram::telegram_get_limits,
            // WebView commands
            commands::webview::get_webview_settings,
            commands::webview::save_webview_settings,
            commands::webview::get_local_ip,
            commands::webview::get_webview_enabled,
            commands::webview::get_webview_start_on_boot,
            commands::webview::get_webview_port,
            commands::webview::get_webview_bind_address,
            commands::webview::get_webview_animation_speed,
            // Twitch commands
            commands::twitch::get_twitch_settings,
            commands::twitch::save_twitch_settings,
            commands::twitch::test_twitch_connection,
            commands::twitch::send_twitch_test_message,
            commands::twitch::connect_twitch,
            commands::twitch::disconnect_twitch,
            commands::twitch::get_twitch_status,
            commands::twitch::get_twitch_enabled,
            commands::twitch::get_twitch_username,
            commands::twitch::get_twitch_channel,
            commands::twitch::get_twitch_start_on_boot,
        ])
        .setup(|app| {
            eprintln!("[APP] === Application setup started ===");

            // Load settings on startup
            eprintln!("[APP] Creating settings manager...");
            let settings_manager = SettingsManager::new()
                .expect("Failed to create settings manager");

            eprintln!("[APP] Creating windows manager...");
            let windows_manager = WindowsManager::new()
                .expect("Failed to create windows manager");

            eprintln!("[APP] Loading settings from disk...");
            let settings = settings_manager.load()
                .expect("Failed to load settings");

            eprintln!("[APP] Loading windows settings from disk...");
            let windows = windows_manager.load()
                .expect("Failed to load windows settings");

            eprintln!("[APP] Settings loaded: tts_provider={:?}, hotkey_enabled={}",
                settings.tts.provider, settings.hotkey_enabled);
            eprintln!("[APP] Windows settings: floating_opacity={}, floating_clickthrough={}",
                windows.floating.opacity, windows.floating.clickthrough);

            let app_state = app.state::<AppState>();
            let _telegram_state = app.state::<TelegramState>();

            // Load Twitch and WebView settings into AppState
            eprintln!("[APP] Loading Twitch settings...");
            *app_state.twitch_settings.blocking_write() = settings.twitch.clone();
            eprintln!("[APP] Twitch settings loaded: enabled={}, start_on_boot={}",
                settings.twitch.enabled, settings.twitch.start_on_boot);

            eprintln!("[APP] Loading WebView settings...");
            *app_state.webview_settings.blocking_write() = crate::webview::WebViewSettings {
                enabled: false, // runtime only
                start_on_boot: settings.webview.start_on_boot,
                port: settings.webview.port,
                bind_address: settings.webview.bind_address.clone(),
                html_template: settings.webview.html_template.clone(),
                css_style: settings.webview.css_style.clone(),
                animation_speed: settings.webview.animation_speed,
            };

            // Load hotkey_enabled setting into AppState
            eprintln!("[APP] Loading hotkey_enabled setting...");
            *app_state.hotkey_enabled.lock() = settings.hotkey_enabled;
            eprintln!("[APP] Hotkey enabled: {}", settings.hotkey_enabled);

            // IMPORTANT: Setup event forwarding BEFORE applying settings
            // so TTS providers can get valid event_sender during initialization
            let app_handle = app.handle().clone();
            let app_state_for_events = app_state.inner().clone();
            let (event_tx, event_rx) = std::sync::mpsc::channel::<AppEvent>();

            // Set event_sender in AppState FIRST
            app_state_for_events.set_event_sender(event_tx.clone());
            eprintln!("[APP] Event sender configured in AppState");

            // Initialize TTS provider based on settings
            eprintln!("[APP] Initializing TTS provider: {:?}", settings.tts.provider);
            app_state.set_tts_provider_type(settings.tts.provider);

            // Apply OpenAI settings
            if let Some(ref api_key) = settings.tts.openai.api_key {
                // Store API key in AppState for get_openai_api_key command
                *app_state.openai_api_key.lock() = Some(api_key.clone());
                eprintln!("[APP] OpenAI API key loaded and stored in AppState");
                app_state.init_openai_tts(api_key.clone());
                app_state.set_openai_voice(settings.tts.openai.voice.clone());
                app_state.set_openai_proxy(settings.tts.openai.proxy_host.clone(), settings.tts.openai.proxy_port);
            } else if settings.tts.provider == crate::tts::TtsProviderType::OpenAi {
                eprintln!("[APP] WARNING: OpenAI selected but no API key found");
            }

            // Apply local TTS settings
            if settings.tts.provider == crate::tts::TtsProviderType::Local {
                let url = settings.tts.local.url.clone();
                app_state.set_local_tts_url(url.clone());
                app_state.init_local_tts(url);
            }

            // Store settings managers for later use
            app.manage(settings_manager);
            app.manage(windows_manager);

            // Spawn event forwarding thread
            thread::spawn(move || {
                eprintln!("[EVENT_LOOP] Event thread started, waiting for events...");

                for event in event_rx {
                    eprintln!("[EVENT_LOOP] Received from channel: {:?}", std::mem::discriminant(&event));
                    // Emit event to frontend
                    let event_name = event.to_tauri_event();
                    let _ = app_handle.emit(event_name, &event);

                    // Handle internally
                    handle_event(event, &app_state_for_events, &app_handle);
                }
            });

            // Initialize SoundPanel state
            let appdata_path = std::env::var("APPDATA")
                .unwrap_or_else(|_| ".".to_string());
            let appdata_path = format!("{}\\ttsbard", appdata_path);

            // Ensure appdata directory exists
            std::fs::create_dir_all(&appdata_path)
                .expect("Failed to create appdata directory");

            let soundpanel_state = SoundPanelState::new(appdata_path.clone());
            app.manage(soundpanel_state.clone());

            // Setup event forwarding for soundpanel FIRST (before loading data that might emit events)
            let soundpanel_state_for_events = soundpanel_state;
            let (soundpanel_tx, soundpanel_rx) = mpsc::channel::<AppEvent>();
            soundpanel_state_for_events.set_event_sender(soundpanel_tx);
            eprintln!("[SOUNDPANEL] Event sender configured");

            let app_handle_for_soundpanel = app.handle().clone();
            thread::spawn(move || {
                for event in soundpanel_rx {
                    // Emit event to frontend
                    let event_name = event.to_tauri_event();
                    let _ = app_handle_for_soundpanel.emit(event_name, &event);

                    // Handle soundpanel-specific events locally
                    match event {
                        AppEvent::ShowSoundPanelWindow => {
                            eprintln!("[SOUNDPANEL] Show soundpanel window");
                            let _ = show_soundpanel_window(&app_handle_for_soundpanel);
                        }
                        AppEvent::HideSoundPanelWindow => {
                            eprintln!("[SOUNDPANEL] Hide soundpanel window");
                            // Get AppState from app_handle
                            if let Some(state) = app_handle_for_soundpanel.try_state::<AppState>() {
                                let _ = hide_soundpanel_window(&app_handle_for_soundpanel, &state);
                            }
                        }
                        AppEvent::SoundPanelNoBinding(key) => {
                            eprintln!("[SOUNDPANEL] No binding for key: {}", key);
                            let _ = emit_soundpanel_no_binding(&app_handle_for_soundpanel, key);
                        }
                        AppEvent::SoundPanelAppearanceChanged => {
                            eprintln!("[SOUNDPANEL] === Appearance changed event received ===");
                            let _ = update_soundpanel_appearance(&app_handle_for_soundpanel);
                        }
                        AppEvent::TtsProviderChanged(_) => {
                            // Ignore TTS provider changes in soundpanel
                        }
                        _ => {}
                    }
                }
            });

            // Load soundpanel bindings
            let soundpanel_state_for_load = app.state::<SoundPanelState>();
            match load_bindings(&soundpanel_state_for_load) {
                Ok(bindings) => {
                    eprintln!("[SOUNDPANEL] Loaded {} bindings on startup", bindings.len());
                }
                Err(e) => {
                    eprintln!("[SOUNDPANEL] Failed to load bindings: {}", e);
                }
            }

            // Load soundpanel appearance settings (now event_sender is configured)
            let windows_manager = app.state::<WindowsManager>();
            match load_appearance(&soundpanel_state_for_load, &windows_manager) {
                Ok(appearance) => {
                    eprintln!("[SOUNDPANEL] Loaded appearance: opacity={}%, color={}",
                        appearance.opacity, appearance.bg_color);
                }
                Err(e) => {
                    eprintln!("[SOUNDPANEL] Failed to load appearance: {}", e);
                }
            }

            eprintln!("[APP] State initialized");

            // Apply saved main window position before showing
            if let Some(main_window) = app.get_webview_window("main") {
                if let Some(x) = windows.main.x {
                    if let Some(y) = windows.main.y {
                        eprintln!("[APP] Restoring main window position: {}, {}", x, y);
                        let _ = main_window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                            x,
                            y,
                        }));
                    }
                }
                // Show window after position is applied
                let _ = main_window.show();
            } else {
                // Fallback: show main window if not found
                if let Some(main_window) = app.get_webview_window("main") {
                    let _ = main_window.show();
                }
            }

            // NOTE: Floating window is NOT restored on startup - only shown via hotkey

            // Initialize system tray
            let app_handle = app.handle().clone();

            // Load icon.png (512x512) for tray - decodes properly at any size
            let png_bytes = include_bytes!("../icons/icon.png");
            let decoded_image = image::load_from_memory(png_bytes)
                .expect("Failed to decode tray icon");
            let rgba_image = decoded_image.to_rgba8();
            let (width, height) = (rgba_image.width(), rgba_image.height());

            // Resize to 32x32 for tray (optimal for Windows system tray)
            let resized = image::imageops::resize(&rgba_image, 32, 32, image::imageops::FilterType::Lanczos3);
            let tray_icon = Image::new_owned(resized.into_raw(), 32, 32);

            eprintln!("Initializing system tray with icon (resized to 32x32 from {}x{})", width, height);

            // Создаем контекстное меню
            let show_floating_item = MenuItem::with_id(
                &app_handle,
                "show_floating",
                "Плавающее окно",
                true,
                None as Option<&str>
            ).unwrap();

            let separator = PredefinedMenuItem::separator(&app_handle).unwrap();

            let quit_item = MenuItem::with_id(
                &app_handle,
                "quit",
                "Выход",
                true,
                None as Option<&str>
            ).unwrap();

            let menu = Menu::with_items(
                &app_handle,
                &[&show_floating_item, &separator, &quit_item]
            ).unwrap();

            // Создаем tray icon с контекстным меню
            eprintln!("[TRAY] Creating tray icon...");
            let _ = TrayIconBuilder::with_id("main")
                .icon(tray_icon)
                .tooltip("TTSBard")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button, button_state, .. } = event {
                        eprintln!("[TRAY] CLICK - button: {:?}, state: {:?}", button, button_state);
                        // Левый клик - показать главное окно
                        if matches!(button, tauri::tray::MouseButton::Left) && matches!(button_state, tauri::tray::MouseButtonState::Up) {
                            eprintln!("[TRAY] LEFT CLICK UP - showing main window");
                            if let Some(window) = tray.app_handle().get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                                eprintln!("[TRAY] Main window shown");
                            } else {
                                eprintln!("[TRAY] ERROR: Main window not found!");
                            }
                        }
                    }
                })
                .on_menu_event(|tray, event| {
                    eprintln!("[TRAY] Menu event: {:?}", event.id);
                    match event.id.as_ref() {
                        "show_floating" => {
                            // Показать плавающее окно только если его нет
                            if tray.app_handle().get_webview_window("floating").is_none() {
                                match show_floating_window(tray.app_handle()) {
                                    Ok(_) => {}
                                    Err(e) => eprintln!("[TRAY] Failed to open floating window: {}", e),
                                }
                            }
                        }
                        "quit" => {
                            // Закрыть приложение корректно
                            tray.app_handle().exit(0);
                        }
                        _ => {}
                    }
                })
                .menu(&menu)
                .build(&app_handle);
            eprintln!("[TRAY] Tray icon created successfully");

            // Setup WebView server
            let app_state_for_webview = app.state::<AppState>();
            let webview_settings = app_state_for_webview.webview_settings.clone();
            let app_handle_for_webview = app.handle().clone();
            let (webview_tx, webview_rx) = std::sync::mpsc::channel::<AppEvent>();

            // Store webview_tx for sending restart events
            app_state_for_webview.set_webview_event_sender(webview_tx);

            // Start WebView server in background thread
            thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new()
                    .expect("Failed to create tokio runtime for webview");

                rt.block_on(async move {
                    loop {
                        // Check current settings
                        let settings = webview_settings.read().await;
                        let mut enabled = settings.enabled;
                        let start_on_boot = settings.start_on_boot;
                        let bind_address = settings.bind_address.clone();
                        let port = settings.port;
                        drop(settings);

                        // Auto-start on boot if configured
                        if start_on_boot && !enabled {
                            eprintln!("[WEBVIEW] Auto-starting server on boot (start_on_boot=true)");
                            let mut s = webview_settings.write().await;
                            s.enabled = true;
                            enabled = true;
                            drop(s);
                        }

                        if enabled {
                            eprintln!("[WEBVIEW] ========================================");
                            eprintln!("[WEBVIEW] STARTING SERVER");
                            eprintln!("[WEBVIEW]   Address: {}:{}", bind_address, port);
                            eprintln!("[WEBVIEW] ========================================");
                            let server = WebViewServer::new(webview_settings.clone());

                            // Spawn server task with improved error handling
                            let server_clone = server.clone();
                            let app_handle_clone = app_handle_for_webview.clone();
                            let bind_address_clone = bind_address.clone();
                            let server_handle = tokio::spawn(async move {
                                eprintln!("[WEBVIEW] Server task started, waiting for connections...");

                                if let Err(e) = server_clone.start().await {
                                    // Extract error details for user-friendly message
                                    let error_msg = format!("{}", e);
                                    let (user_friendly_msg, log_context) = parse_webview_server_error(&error_msg, bind_address_clone, port);

                                    // Log with full context
                                    eprintln!("[WEBVIEW] ❌ Server startup failed:");
                                    eprintln!("[WEBVIEW]   Context: {}", log_context);
                                    eprintln!("[WEBVIEW]   Error: {}", error_msg);

                                    // Emit user-friendly error to frontend
                                    let _ = app_handle_clone.emit("webview-server-error", &user_friendly_msg);

                                    // Also emit via AppEvent system for consistency
                                    if let Some(state) = app_handle_clone.try_state::<AppState>() {
                                        state.emit_event(AppEvent::WebViewServerError(user_friendly_msg));
                                    }
                                }
                                // Server task completed
                                eprintln!("[WEBVIEW] Server task stopped");
                            });

                            // Handle events and broadcast text
                            let mut server_running = true;
                            while server_running {
                                // Check if settings changed
                                let current_settings = webview_settings.read().await;
                                let still_enabled = current_settings.enabled;
                                let same_port = current_settings.port == port && current_settings.bind_address == bind_address;
                                drop(current_settings);

                                if !still_enabled || !same_port {
                                    eprintln!("[WEBVIEW] ========================================");
                                    eprintln!("[WEBVIEW] STOPPING SERVER (settings changed)");
                                    eprintln!("[WEBVIEW]   Still enabled: {}", still_enabled);
                                    eprintln!("[WEBVIEW]   Same port: {}", same_port);
                                    eprintln!("[WEBVIEW] ========================================");
                                    server_handle.abort();
                                    server_running = false;
                                } else {
                                    // Process events with timeout (synchronous)
                                    match webview_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                                        Ok(event) => {
                                            eprintln!("[WEBVIEW] 📨 Event received: {:?}", std::mem::discriminant(&event));
                                            match event {
                                                AppEvent::TextSentToTts(text) => {
                                                    eprintln!("[WEBVIEW] 📤 Broadcasting to WebSocket clients: '{}...'", text.chars().take(50).collect::<String>());
                                                    server.broadcast_text(text).await;
                                                }
                                                AppEvent::RestartWebViewServer => {
                                                    eprintln!("[WEBVIEW] ⚠ Restart event received, stopping server...");
                                                    server_running = false;
                                                }
                                                _ => {
                                                    eprintln!("[WEBVIEW] ℹ️  Ignoring event: {:?}", std::mem::discriminant(&event));
                                                }
                                            }
                                        }
                                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                            // Timeout - continue loop to check settings
                                        }
                                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                            // Channel closed
                                            eprintln!("[WEBVIEW] Event channel disconnected");
                                            return;
                                        }
                                    }
                                }
                            }
                        } else {
                            eprintln!("[WEBVIEW] ========================================");
                            eprintln!("[WEBVIEW] SERVER DISABLED");
                            eprintln!("[WEBVIEW] Waiting for enable signal...");
                            eprintln!("[WEBVIEW] ========================================");
                            // Wait for enable or restart event
                            loop {
                                match webview_rx.recv_timeout(std::time::Duration::from_secs(2)) {
                                    Ok(AppEvent::RestartWebViewServer) => {
                                        eprintln!("[WEBVIEW] ⚠ Restart event received, exiting disabled state");
                                        break;
                                    }
                                    Ok(AppEvent::TextSentToTts(text)) => {
                                        // Ignore TTS events while disabled but log them
                                        eprintln!("[WEBVIEW] Ignoring TTS text (server disabled): '{}...'", text.chars().take(30).collect::<String>());
                                    }
                                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                        // Timeout - check if enabled now
                                        let settings = webview_settings.read().await;
                                        if settings.enabled {
                                            drop(settings);
                                            eprintln!("[WEBVIEW] ✓ Enabled detected via timeout!");
                                            break;
                                        }
                                        drop(settings);
                                    }
                                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                        eprintln!("[WEBVIEW] Event channel disconnected");
                                        return;
                                    }
                                    Ok(other) => {
                                        eprintln!("[WEBVIEW] Received unexpected event while disabled: {:?}", other);
                                    }
                                }
                            }
                        }
                    }
                });
            });

            // Setup Twitch client event loop in dedicated thread
            let app_state_for_twitch = app.state::<AppState>();
            let app_state_for_twitch_arc = app_state_for_twitch.inner().clone();
            let app_handle_for_twitch = app.handle().clone();
            let mut twitch_rx = app_state_for_twitch.twitch_event_tx.subscribe();

            thread::spawn(move || {
                // Create tokio runtime for this thread
                let rt = tokio::runtime::Runtime::new().expect("Failed to create Twitch tokio runtime");

                rt.block_on(async move {
                    let mut twitch_client: Option<TwitchClient> = None;
                    let mut last_status = crate::events::TwitchConnectionStatus::Disconnected;
                    let mut status_check_interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

                    // Helper function to update status in AppState and emit events
                    let update_status = |status: crate::events::TwitchConnectionStatus| {
                        *app_state_for_twitch_arc.twitch_connection_status.lock() = status.clone();
                        let _ = app_handle_for_twitch.emit("twitch-status-changed", &status);
                    };

                    loop {
                        tokio::select! {
                            // Периодическая проверка статуса
                            _ = status_check_interval.tick() => {
                                if let Some(client) = &twitch_client {
                                    let twitch_status = client.status().await;
                                    let new_status = match &twitch_status {
                                        crate::twitch::TwitchStatus::Connected => {
                                            crate::events::TwitchConnectionStatus::Connected
                                        }
                                        crate::twitch::TwitchStatus::Connecting => {
                                            crate::events::TwitchConnectionStatus::Connecting
                                        }
                                        crate::twitch::TwitchStatus::Disconnected => {
                                            crate::events::TwitchConnectionStatus::Disconnected
                                        }
                                        crate::twitch::TwitchStatus::Error(e) => {
                                            crate::events::TwitchConnectionStatus::Error(e.clone())
                                        }
                                    };

                                    if last_status != new_status {
                                        last_status = new_status.clone();
                                        update_status(new_status.clone());
                                    }
                                } else if last_status != crate::events::TwitchConnectionStatus::Disconnected {
                                    last_status = crate::events::TwitchConnectionStatus::Disconnected;
                                    update_status(last_status.clone());
                                }
                            }
                            // Обработка событий Twitch
                            event = twitch_rx.recv() => {
                                match event {
                                    Ok(event) => {
                                        match event {
                                            TwitchEvent::Restart => {
                                                eprintln!("[TWITCH] Restart event received");

                                                // Получаем настройки для создания нового клиента
                                                let settings = app_state_for_twitch_arc.twitch_settings.read().await;
                                                let is_enabled = settings.enabled;
                                                let is_valid = settings.is_valid().is_ok();
                                                let settings_clone = settings.clone();
                                                drop(settings);

                                                // Остановить текущий клиент
                                                if let Some(client) = twitch_client.take() {
                                                    eprintln!("[TWITCH] Stopping previous client...");
                                                    client.stop().await;
                                                }

                                                // Сбросить статус
                                                last_status = crate::events::TwitchConnectionStatus::Disconnected;
                                                update_status(last_status.clone());

                                                // Запустить новый если включен
                                                if is_enabled {
                                                    if is_valid {
                                                        eprintln!("[TWITCH] Settings valid, creating new client...");
                                                        last_status = crate::events::TwitchConnectionStatus::Connecting;
                                                        update_status(last_status.clone());

                                                        let client = TwitchClient::new(settings_clone.into());
                                                        match client.start().await {
                                                            Ok(_) => {
                                                                eprintln!("[TWITCH] Client started, waiting for connection...");
                                                                twitch_client = Some(client);
                                                            }
                                                            Err(e) => {
                                                                eprintln!("[TWITCH] Failed to start client: {}", e);
                                                                last_status = crate::events::TwitchConnectionStatus::Error(e.to_string());
                                                                update_status(last_status.clone());
                                                            }
                                                        }
                                                    } else {
                                                        eprintln!("[TWITCH] Settings invalid, not starting client");
                                                    }
                                                } else {
                                                    eprintln!("[TWITCH] Twitch disabled, not starting client");
                                                }
                                            }
                                            TwitchEvent::Stop => {
                                                eprintln!("[TWITCH] Stop event received");
                                                if let Some(client) = twitch_client.take() {
                                                    client.stop().await;
                                                }
                                                last_status = crate::events::TwitchConnectionStatus::Disconnected;
                                                update_status(last_status.clone());
                                            }
                                            TwitchEvent::SendMessage(text) => {
                                                eprintln!("[TWITCH] SendMessage event received: '{}'", text);
                                                if let Some(client) = &twitch_client {
                                                    match client.send_message(&text).await {
                                                        Ok(_) => eprintln!("[TWITCH] Message sent successfully"),
                                                        Err(e) => {
                                                            eprintln!("[TWITCH] Failed to send message: {}", e);
                                                            last_status = crate::events::TwitchConnectionStatus::Error(e.to_string());
                                                            update_status(last_status.clone());
                                                        }
                                                    }
                                                } else {
                                                    eprintln!("[TWITCH] Cannot send message - no active client");
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[TWITCH] Event channel error: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });
            });

            // Auto-start Twitch if configured
            let app_state_for_autostart = app.state::<AppState>();
            let app_state_autostart_arc = app_state_for_autostart.inner().clone();

            thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create Twitch autostart runtime");

                rt.block_on(async move {
                    let settings = app_state_autostart_arc.twitch_settings.read().await;
                    if settings.start_on_boot && settings.enabled {
                        if let Ok(()) = settings.is_valid() {
                            eprintln!("[TWITCH] Auto-starting on boot");
                            // Отправляем событие Restart в основной event loop
                            // вместо создания клиента напрямую
                            app_state_autostart_arc.send_twitch_event(crate::events::TwitchEvent::Restart);
                            eprintln!("[TWITCH] Restart event sent for auto-start");
                        }
                    }
                });
            });

            // Hotkeys will be initialized after window is fully shown (in on_window_event)
            eprintln!("Setup complete - hotkeys will be registered when window gains focus");

            // Initialize keyboard hook for text interception
            let app_state = app.state::<AppState>();
            let app_state_inner = app_state.inner().clone();
            let _hook_handle = initialize_text_interception_hook(app_state_inner);
            eprintln!("Keyboard hook initialized");

            // Initialize keyboard hook for soundpanel
            let soundpanel_state = app.state::<SoundPanelState>();
            let soundpanel_state_inner = soundpanel_state.inner().clone();
            let _soundpanel_hook_handle = initialize_soundpanel_hook(soundpanel_state_inner);
            eprintln!("[SOUNDPANEL] Keyboard hook initialized");

            // Apply global exclude from capture to main window (before first show)
            #[cfg(windows)]
            {
                use crate::window::set_window_exclude_from_capture;

                if let Some(windows_manager) = app.try_state::<WindowsManager>() {
                    let exclude_from_capture = windows_manager.get_global_exclude_from_capture();
                    eprintln!("[APP] Applying global exclude from capture to main window: {}", exclude_from_capture);

                    if let Some(main_window) = app.get_webview_window("main") {
                        if let Ok(hwnd) = main_window.hwnd() {
                            match set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture) {
                                Ok(_) => eprintln!("[APP] Main window exclude from capture applied: {}", exclude_from_capture),
                                Err(e) => eprintln!("[APP] Failed to apply exclude from capture to main window: {}", e),
                            }
                        }
                    }

                    // Apply to floating window
                    if let Some(floating_window) = app.get_webview_window("floating") {
                        if let Ok(hwnd) = floating_window.hwnd() {
                            match set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture) {
                                Ok(_) => eprintln!("[APP] Floating window exclude from capture applied: {}", exclude_from_capture),
                                Err(e) => eprintln!("[APP] Failed to apply exclude from capture to floating window: {}", e),
                            }
                        }
                    }

                    // Apply to soundpanel window
                    if let Some(soundpanel_window) = app.get_webview_window("soundpanel") {
                        if let Ok(hwnd) = soundpanel_window.hwnd() {
                            match set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture) {
                                Ok(_) => eprintln!("[APP] SoundPanel window exclude from capture applied: {}", exclude_from_capture),
                                Err(e) => eprintln!("[APP] Failed to apply exclude from capture to soundpanel window: {}", e),
                            }
                        }
                    }
                }
            }

            // Инициализация окон
            Ok(())
        })
        .on_window_event(|window, event| {
            // Обрабатываем события главного окна
            if window.label() == "main" {
                // Позиция сохраняется только при закрытии (событие Destroyed)
                // Предотвращаем закрытие - скрываем окно вместо этого
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    eprintln!("[APP] Main window close requested - hiding to tray");
                    api.prevent_close();
                    let _ = window.hide();
                }
            }

            #[cfg(windows)]
            if window.label() == "main" {
                match event {
                    tauri::WindowEvent::Focused(focused) => {
                        if *focused {
                            // Initialize hotkeys when window gains focus (ensuring it's fully created)
                            static HOTKEY_INIT: std::sync::Once = std::sync::Once::new();

                            HOTKEY_INIT.call_once(|| {
                                let app_handle = window.app_handle();
                                let app_state = app_handle.state::<AppState>();
                                let app_state_inner = app_state.inner().clone();

                                eprintln!("Window focused - initializing hotkeys...");

                                // Initialize hotkeys using tauri-plugin-global-shortcut
                                match initialize_hotkeys(0, app_state_inner, app_handle.clone()) {
                                    Ok(_) => {
                                        eprintln!("Hotkeys initialized successfully");
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to initialize hotkeys: {}", e);
                                    }
                                }
                            });
                        } else {
                            // Window lost focus - remove always-on-top
                            eprintln!("[APP] Main window lost focus - removing always-on-top");
                            let _ = window.set_always_on_top(false);
                        }
                    }
                    tauri::WindowEvent::Destroyed => {
                        // Сохраняем позицию главного окна
                        if let Some(windows_manager) = window.app_handle().try_state::<WindowsManager>() {
                            if let Ok(pos) = window.outer_position() {
                                let x = pos.x;
                                let y = pos.y;
                                eprintln!("[APP] Main window destroyed - saving position: {}, {}", x, y);
                                let _ = windows_manager.set_main_position(Some(x), Some(y));
                            }

                            // Сохраняем позицию плавающего окна (если оно было показано)
                            if let Some(floating_window) = window.app_handle().get_webview_window("floating") {
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
                    }
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Parse WebView server startup errors and provide user-friendly messages
pub(crate) fn parse_webview_server_error(error_msg: &str, bind_address: String, port: u16) -> (String, String) {
    let log_context = format!("Failed to start WebView server on {}:{}", bind_address, port);

    // Check for common error patterns
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
        // Generic error with technical details
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

fn handle_event(event: AppEvent, state: &AppState, app_handle: &tauri::AppHandle) {
    eprintln!("[HANDLE_EVENT] Received event: {:?}", std::mem::discriminant(&event));
    match event {
        AppEvent::InterceptionChanged(enabled) => {
            eprintln!("Interception changed: {}", enabled);
            // When interception is enabled, show floating window
            if enabled {
                eprintln!("Text interception mode enabled - type to capture text");
                eprintln!("Press F8 to switch layout (EN/RU)");
                eprintln!("Press Enter to send text to TTS");
                eprintln!("Press Escape to cancel");
            }
            // Update tray icon
            update_tray_icon(app_handle, enabled);
        }
        AppEvent::LayoutChanged(layout) => {
            eprintln!("Layout changed: {:?}", layout);
            // Update floating window title with layout
            let layout_str = match layout {
                InputLayout::English => "EN",
                InputLayout::Russian => "RU",
            };
            let text = state.get_current_text();
            let _ = update_floating_title(app_handle, layout_str, &text);
            match layout {
                InputLayout::English => eprintln!("Current layout: English (EN)"),
                InputLayout::Russian => eprintln!("Current layout: Russian (RU)"),
            }
        }
        AppEvent::TextReady(text) => {
            eprintln!("[EVENT] Text ready for TTS: '{}'", text);

            // Use the unified TTS function that works with all providers
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| {
                    eprintln!("[EVENT] Failed to create runtime: {}", e);
                    state.emit_event(AppEvent::TtsError(format!("Failed to create runtime: {}", e)));
                });

            if let Ok(rt) = rt {
                rt.block_on(async {
                    match crate::commands::speak_text_internal(state, text).await {
                        Ok(_) => {
                            eprintln!("[EVENT] TTS started successfully in interception mode");
                        }
                        Err(e) => {
                            eprintln!("[EVENT] TTS failed in interception mode: {}", e);
                            state.emit_event(AppEvent::TtsError(e));
                        }
                    }
                });
            }
        }
        AppEvent::TextSentToTts(text) => {
            eprintln!("[EVENT] Text sent to TTS: '{}'", text);

            // WebView broadcast (существует)
            if let Some(ref sender) = *state.webview_event_sender.lock() {
                eprintln!("[EVENT] Forwarding TextSentToTts to WebView server");
                match sender.send(AppEvent::TextSentToTts(text.clone())) {
                    Ok(_) => eprintln!("[EVENT] TextSentToTts sent to WebView successfully"),
                    Err(e) => eprintln!("[EVENT] Failed to send to WebView: {}", e),
                }
            } else {
                eprintln!("[EVENT] WebView sender is None, not forwarding");
            }

            // Twitch send (новое)
            let settings = state.twitch_settings.blocking_read();
            if settings.enabled {
                drop(settings);
                state.send_twitch_event(TwitchEvent::SendMessage(text));
            }
        }
        AppEvent::TtsStatusChanged(status) => {
            eprintln!("TTS status changed: {:?}", status);
        }
        AppEvent::TtsError(err) => {
            eprintln!("TTS error: {}", err);
        }
        AppEvent::ShowFloatingWindow => {
            eprintln!("[EVENT] ShowFloatingWindow event received");
            match show_floating_window(app_handle) {
                Ok(_) => eprintln!("[EVENT] Floating window shown successfully"),
                Err(e) => eprintln!("[EVENT] Failed to show floating window: {}", e),
            }
            // Clear text when showing window
            state.clear_text();
            // Update UI with empty text and current layout
            let layout = match state.get_current_layout() {
                InputLayout::English => "EN",
                InputLayout::Russian => "RU",
            };
            let _ = update_floating_text(app_handle, "");
            let _ = update_floating_title(app_handle, layout, "");
        }
        AppEvent::HideFloatingWindow => {
            eprintln!("Hide floating window");
            let _ = hide_floating_window(app_handle, state);
        }
        AppEvent::ShowMainWindow => {
            eprintln!("Show main window");
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        AppEvent::UpdateFloatingText(text) => {
            eprintln!("Update floating text: '{}'", text);
            let _ = update_floating_text(app_handle, &text);
            // Also update title
            let layout = match state.get_current_layout() {
                InputLayout::English => "EN",
                InputLayout::Russian => "RU",
            };
            let _ = update_floating_title(app_handle, layout, &text);
        }
        AppEvent::UpdateTrayIcon(is_intercepting) => {
            eprintln!("Update tray icon: {}", is_intercepting);
            update_tray_icon(app_handle, is_intercepting);
        }
        AppEvent::FloatingAppearanceChanged => {
            eprintln!("Floating window appearance changed");
        }
        AppEvent::ClickthroughChanged(enabled) => {
            eprintln!("Clickthrough changed: {}", enabled);
            if let Some(window) = app_handle.get_webview_window("floating") {
                let _ = window.set_ignore_cursor_events(enabled);
            }
        }
        AppEvent::ShowSoundPanelWindow => {
            eprintln!("[EVENT] ShowSoundPanelWindow event received");
            match show_soundpanel_window(app_handle) {
                Ok(_) => eprintln!("[EVENT] Soundpanel window shown successfully"),
                Err(e) => eprintln!("[EVENT] Failed to show soundpanel window: {}", e),
            }
        }
        AppEvent::HideSoundPanelWindow => {
            eprintln!("[EVENT] HideSoundPanelWindow event received");
            let _ = hide_soundpanel_window(app_handle, state);
        }
        AppEvent::SoundPanelNoBinding(key) => {
            eprintln!("[EVENT] SoundPanelNoBinding: {}", key);
            // Emit to soundpanel window for display
            let _ = emit_soundpanel_no_binding(app_handle, key);
        }
        AppEvent::SoundPanelAppearanceChanged => {
            eprintln!("[EVENT MAIN] === SoundPanelAppearanceChanged event received ===");
            let _ = update_soundpanel_appearance(app_handle);
        }
        AppEvent::TtsProviderChanged(provider) => {
            eprintln!("[EVENT] TTS provider changed to: {:?}", provider);
            // TODO: Emit to frontend for UI update
        }
        AppEvent::EnterClosesDisabled(disabled) => {
            eprintln!("[EVENT] Enter closes disabled: {}", disabled);
        }
        AppEvent::WebViewServerError(error) => {
            eprintln!("[EVENT] WebView server error: {}", error);
            // Event is already emitted to frontend via Tauri event system
        }
        AppEvent::RestartWebViewServer => {
            eprintln!("[EVENT] Restart WebView server requested");
            // This event is handled by the WebView server thread
        }
        AppEvent::TwitchStatusChanged(status) => {
            eprintln!("[EVENT] Twitch status changed: {:?}", status);
            // Event is already emitted to frontend via Tauri event system
        }
    }
}
