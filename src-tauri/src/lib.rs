mod commands;
mod events;
mod floating;
mod hook;
mod hotkeys;
mod settings;
mod state;
mod tts;
mod window;
mod soundpanel;
mod audio;
mod preprocessor;
mod telegram;

use std::sync::mpsc;
use std::thread;
use state::AppState;
use events::{AppEvent, InputLayout};
use hotkeys::initialize_hotkeys;
use hook::initialize_text_interception_hook;
use commands::{speak_text, get_tts_provider, set_tts_provider, get_local_tts_url, set_local_tts_url, get_openai_api_key, set_openai_api_key, get_openai_voice, set_openai_voice, get_openai_proxy, set_openai_proxy, get_interception, set_interception, check_api_key, get_floating_appearance, set_floating_opacity, set_floating_bg_color, set_clickthrough, is_clickthrough_enabled, is_enter_closes_disabled, toggle_interception, toggle_floating_window, show_floating_window_cmd, hide_floating_window_cmd, is_floating_window_visible, quit_app, get_hotkey_enabled, set_hotkey_enabled, open_file_dialog, get_output_devices, get_virtual_mic_devices, get_audio_settings, set_speaker_device, set_speaker_enabled, set_speaker_volume, set_virtual_mic_device, enable_virtual_mic, disable_virtual_mic, set_virtual_mic_volume};
use commands::telegram::{telegram_init, telegram_request_code, telegram_sign_in, telegram_sign_out, telegram_get_status, telegram_get_user, telegram_auto_restore, TelegramState};
use soundpanel::{SoundPanelState, sp_get_bindings, sp_add_binding, sp_remove_binding, sp_test_sound, sp_is_supported_format, sp_get_floating_appearance, sp_set_floating_opacity, sp_set_floating_bg_color, sp_set_floating_clickthrough, sp_is_floating_clickthrough_enabled, load_bindings, load_appearance, initialize_soundpanel_hook};
use floating::{show_floating_window, hide_floating_window, update_floating_text, update_floating_title, show_soundpanel_window, hide_soundpanel_window, emit_soundpanel_no_binding, update_soundpanel_appearance};
use settings::SettingsManager;
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
    // Создаем MPSC канал для событий
    let (event_tx, _event_rx) = mpsc::channel::<AppEvent>();

    // Инициализируем состояние
    let app_state = AppState::new();
    app_state.set_event_sender(event_tx);

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
            check_api_key,
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
        ])
        .setup(|app| {
            eprintln!("[APP] === Application setup started ===");

            // Load settings on startup
            eprintln!("[APP] Creating settings manager...");
            let settings_manager = SettingsManager::new()
                .expect("Failed to create settings manager");

            eprintln!("[APP] Loading settings from disk...");
            let settings = settings_manager.load()
                .expect("Failed to load settings");

            eprintln!("[APP] Settings loaded: interception={}, opacity={}, clickthrough={}, floating_visible={}",
                settings.interception_enabled,
                settings.floating_opacity,
                settings.floating_clickthrough,
                settings.floating_window_visible);

            let app_state = app.state::<AppState>();
            let telegram_state = app.state::<TelegramState>();
            settings_manager.apply_to_state(&settings, &app_state, Some(telegram_state));

            // Store settings manager for later use
            app.manage(settings_manager);

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
            match load_appearance(&soundpanel_state_for_load) {
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
                if let Some(x) = settings.main_x {
                    if let Some(y) = settings.main_y {
                        eprintln!("[APP] Restoring main window position: {}, {}", x, y);
                        let _ = main_window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                            x: x as i32,
                            y: y as i32,
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

            // Restore floating window visibility from settings
            if settings.floating_window_visible {
                eprintln!("[APP] Restoring floating window visibility");
                match show_floating_window(&app.handle()) {
                    Ok(_) => eprintln!("[APP] Floating window restored"),
                    Err(e) => eprintln!("[APP] Failed to restore floating window: {}", e),
                }
            }

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
                    match event {
                        TrayIconEvent::Click { button, button_state, .. } => {
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
                        _ => {}
                    }
                })
                .on_menu_event(|tray, event| {
                    eprintln!("[TRAY] Menu event: {:?}", event.id);
                    match event.id.as_ref() {
                        "show_floating" => {
                            // Показать плавающее окно только если его нет
                            if tray.app_handle().get_webview_window("floating").is_none() {
                                match show_floating_window(&tray.app_handle()) {
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

            // Set up event forwarding to frontend
            let app_handle = app.handle().clone();
            let app_state = app.state::<AppState>();
            let app_state_for_events = app_state.inner().clone();

            // Spawn event forwarding thread
            thread::spawn(move || {
                // Create a new channel for event forwarding
                let (event_tx, event_rx) = mpsc::channel::<AppEvent>();
                app_state_for_events.set_event_sender(event_tx);

                for event in event_rx {
                    // Emit event to frontend
                    let event_name = event.to_tauri_event();
                    let _ = app_handle.emit(event_name, &event);

                    // Handle internally
                    handle_event(event, &app_state_for_events, &app_handle);
                }
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

            // Инициализация окон
            Ok(())
        })
        .on_window_event(|window, event| {
            // Обрабатываем события главного окна
            if window.label() == "main" {
                match event {
                    // Сохраняем позицию при перемещении
                    tauri::WindowEvent::Moved(position) => {
                        if let Some(settings_manager) = window.app_handle().try_state::<SettingsManager>() {
                            let x = position.x as i32;
                            let y = position.y as i32;
                            let _ = settings_manager.set_main_window_position(Some(x), Some(y));
                        }
                    }
                    // Предотвращаем закрытие - скрываем окно вместо этого
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        eprintln!("[APP] Main window close requested - hiding to tray");
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    _ => {}
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
                        if let Some(settings_manager) = window.app_handle().try_state::<SettingsManager>() {
                            if let Ok(pos) = window.outer_position() {
                                let x = pos.x as i32;
                                let y = pos.y as i32;
                                eprintln!("[APP] Main window destroyed - saving position: {}, {}", x, y);
                                let _ = settings_manager.set_main_window_position(Some(x), Some(y));
                            }

                            // Сохраняем состояние плавающего окна при закрытии главного окна
                            let is_visible = window.app_handle().get_webview_window("floating")
                                .and_then(|w| w.is_visible().ok())
                                .unwrap_or(false);

                            eprintln!("[APP] Main window destroyed - saving floating window state, visible: {}", is_visible);

                            // Сохраняем видимость
                            let _ = settings_manager.set_floating_window_visibility(is_visible);

                            // Если окно видимо, сохраняем его позицию
                            if is_visible {
                                if let Some(floating_window) = window.app_handle().get_webview_window("floating") {
                                    if let Ok(pos) = floating_window.outer_position() {
                                        let x = pos.x as i32;
                                        let y = pos.y as i32;
                                        eprintln!("[APP] Saving floating window position: {}, {}", x, y);
                                        let _ = settings_manager.set_floating_window_position(Some(x), Some(y));
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

fn handle_event(event: AppEvent, state: &AppState, app_handle: &tauri::AppHandle) {
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
                    match crate::commands::speak_text_internal(&state, text).await {
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
    }
}
