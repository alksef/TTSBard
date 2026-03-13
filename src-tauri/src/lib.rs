mod commands;
mod audio;
mod config;
mod error;
mod event_loop;
mod events;
mod floating;
mod hook;
mod hotkeys;
mod servers;
mod setup;
mod soundpanel;
mod state;
mod preprocessor;
mod telegram;
mod tts;
mod window;
mod webview;
mod twitch;
mod rate_limiter;
mod thread_manager;

use state::AppState;
use commands::telegram::TelegramState;
use config::{SettingsManager, WindowsManager};
use tauri::Manager;
use commands::{speak_text, get_tts_provider, set_tts_provider, get_local_tts_url, set_local_tts_url, get_openai_api_key, set_openai_api_key, get_openai_voice, set_openai_voice, get_openai_proxy, set_openai_proxy, get_interception, set_interception, has_api_key, get_floating_appearance, set_floating_opacity, set_floating_bg_color, set_clickthrough, is_clickthrough_enabled, is_enter_closes_disabled, toggle_interception, toggle_floating_window, show_floating_window_cmd, hide_floating_window_cmd, is_floating_window_visible, quit_app, get_hotkey_enabled, set_hotkey_enabled, get_global_exclude_from_capture, set_global_exclude_from_capture, open_file_dialog, get_output_devices, get_virtual_mic_devices, get_audio_settings, set_speaker_device, set_speaker_enabled, set_speaker_volume, set_virtual_mic_device, enable_virtual_mic, disable_virtual_mic, set_virtual_mic_volume, set_quick_editor_enabled, get_quick_editor_enabled, hide_main_window, close_floating_window, close_soundpanel_window};
use commands::telegram::{telegram_init, telegram_request_code, telegram_sign_in, telegram_sign_out, telegram_get_status, telegram_get_user, telegram_auto_restore};
use soundpanel::{sp_get_bindings, sp_add_binding, sp_remove_binding, sp_test_sound, sp_is_supported_format, sp_get_floating_appearance, sp_set_floating_opacity, sp_set_floating_bg_color, sp_set_floating_clickthrough, sp_is_floating_clickthrough_enabled};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Инициализируем состояние и менеджеры ДО setup
    let app_state = AppState::new();

    let settings_manager = SettingsManager::new()
        .expect("Failed to create settings manager");

    let windows_manager = WindowsManager::new()
        .expect("Failed to create windows manager");

    let appdata_path = std::env::var("APPDATA")
        .unwrap_or_else(|_| ".".to_string());
    let appdata_path = format!("{}\\ttsbard", appdata_path);

    std::fs::create_dir_all(&appdata_path)
        .expect("Failed to create appdata directory");

    let soundpanel_state = soundpanel::SoundPanelState::new(appdata_path);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .manage(TelegramState::new())
        .manage(settings_manager)
        .manage(windows_manager)
        .manage(soundpanel_state)
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
            // Quick editor commands
            get_quick_editor_enabled,
            set_quick_editor_enabled,
            hide_main_window,
            close_floating_window,
            close_soundpanel_window,
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
            commands::webview::open_template_folder,
            commands::webview::send_test_message,
            commands::webview::reload_templates,
            // Twitch commands
            commands::twitch::get_twitch_settings,
            commands::twitch::save_twitch_settings,
            commands::twitch::test_twitch_connection,
            commands::twitch::send_twitch_test_message,
            commands::twitch::connect_twitch,
            commands::twitch::disconnect_twitch,
            commands::twitch::get_twitch_status,
        ])
        .setup(|app| setup::init_app(app))
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
                                let app_handle = window.app_handle().clone();
                                let app_state = app_handle.state::<AppState>();
                                let app_state_inner = app_state.inner().clone();

                                eprintln!("Window focused - initializing hotkeys...");

                                // Initialize hotkeys using tauri-plugin-global-shortcut
                                match hotkeys::initialize_hotkeys(0, app_state_inner, app_handle.clone()) {
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
                                let x: i32 = pos.x;
                                let y: i32 = pos.y;
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
