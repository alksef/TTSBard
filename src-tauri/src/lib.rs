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
use tracing::{info, warn, error, Level};
use tracing_subscriber::{fmt, prelude::*, Registry, EnvFilter};
use tracing_appender::non_blocking;
use std::path::PathBuf;
use anyhow::Context;
use commands::{speak_text, get_tts_provider, set_tts_provider, get_local_tts_url, set_local_tts_url, get_openai_api_key, set_openai_api_key, get_openai_voice, set_openai_voice, get_openai_proxy, set_openai_proxy, get_interception, set_interception, has_api_key, get_floating_appearance, set_floating_opacity, set_floating_bg_color, set_clickthrough, is_clickthrough_enabled, is_enter_closes_disabled, toggle_interception, toggle_floating_window, show_floating_window_cmd, hide_floating_window_cmd, is_floating_window_visible, quit_app, get_hotkey_enabled, set_hotkey_enabled, get_global_exclude_from_capture, set_global_exclude_from_capture, open_file_dialog, get_output_devices, get_virtual_mic_devices, get_audio_settings, set_speaker_device, set_speaker_enabled, set_speaker_volume, set_virtual_mic_device, enable_virtual_mic, disable_virtual_mic, set_virtual_mic_volume, set_quick_editor_enabled, get_quick_editor_enabled, hide_main_window, close_floating_window, close_soundpanel_window};
use commands::logging::{get_logging_settings, save_logging_settings};
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

    // Load settings to configure logger
    // These settings will be passed to init_app to avoid race condition
    let settings = settings_manager.load()
        .expect("Failed to load settings");

    // Validate module levels (log a warning if invalid)
    if let Err(e) = commands::logging::validate_module_levels(&settings.logging.module_levels) {
        warn!(error = %e, "Invalid module_levels in settings.json. Module logging disabled.");
    }

    // Initialize tracing subscriber
    let log_dir = PathBuf::from(std::env::var("APPDATA")
        .unwrap_or_else(|_| ".".to_string()))
        .join("ttsbard")
        .join("logs");

    // Ensure log directory exists with graceful fallback
    if let Err(e) = std::fs::create_dir_all(&log_dir)
        .with_context(|| format!("Failed to create log directory at {:?}", log_dir))
    {
        eprintln!("Failed to create log directory: {}. Logging to stdout only.", e);
        // Continue with stdout-only logging
    }

    // Build env filter with per-module directives
    let default_level = if settings.logging.enabled {
        match settings.logging.level.as_str() {
            "error" => Level::ERROR,
            "warn" => Level::WARN,
            "info" => Level::INFO,
            "debug" => Level::DEBUG,
            "trace" => Level::TRACE,
            _ => Level::INFO,
        }
    } else {
        Level::ERROR
    };

    let mut env_filter = EnvFilter::builder()
        .with_default_directive(default_level.into())
        .from_env_lossy();

    // Apply per-module filters from settings.json
    for (module, level_str) in &settings.logging.module_levels {
        let module_level = match level_str.as_str() {
            "error" => Level::ERROR,
            "warn" => Level::WARN,
            "info" => Level::INFO,
            "debug" => Level::DEBUG,
            "trace" => Level::TRACE,
            _ => Level::INFO,
        };
        let directive = format!("{}={}", module, module_level);
        env_filter = env_filter.add_directive(directive.parse().expect("Invalid log directive"));
    }

    // WorkerGuard must live for the entire program duration.
    // We use Box::leak to prevent it from being dropped, which would stop logging.
    // This is a small memory leak (a few bytes) that is acceptable for a desktop app.
    let _guard: &'static mut non_blocking::WorkerGuard = if cfg!(debug_assertions) && settings.logging.enabled {
        // Debug mode + enabled: console and file with non-blocking writer
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("ttsbard.log"))
            .expect("Failed to open log file");

        // Add session separator for readability
        use std::io::Write;
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        writeln!(
            &log_file,
            "\n====== New session: {} | Version: {} ======\n",
            timestamp,
            env!("CARGO_PKG_VERSION")
        ).ok();

        let (non_blocking_file, guard) = non_blocking(log_file);
        let leaked_guard = Box::leak(Box::new(guard));

        tracing::subscriber::set_global_default(
            Registry::default()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_writer(std::io::stdout)
                        .with_ansi(true)
                )
                .with(
                    fmt::layer()
                        .with_writer(non_blocking_file)
                        .with_ansi(false)
                )
        ).expect("Failed to set tracing subscriber");

        leaked_guard
    } else if settings.logging.enabled {
        // Release mode + enabled: file only with non-blocking writer
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("ttsbard.log"))
            .expect("Failed to open log file");

        // Add session separator for readability
        use std::io::Write;
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        writeln!(
            &log_file,
            "\n====== New session: {} | Version: {} ======\n",
            timestamp,
            env!("CARGO_PKG_VERSION")
        ).ok();

        let (non_blocking_file, guard) = non_blocking(log_file);
        let leaked_guard = Box::leak(Box::new(guard));

        tracing::subscriber::set_global_default(
            Registry::default()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_writer(non_blocking_file)
                        .with_ansi(false)
                )
        ).expect("Failed to set tracing subscriber");

        leaked_guard
    } else {
        // Logging disabled: errors only to console (no guard needed for stdout)
        tracing::subscriber::set_global_default(
            Registry::default()
                .with(EnvFilter::new("error"))
                .with(
                    fmt::layer()
                        .with_writer(std::io::stdout)
                        .with_ansi(true)
                )
        ).expect("Failed to set tracing subscriber");
        // Dummy guard to satisfy the type system - won't be used
        Box::leak(Box::new(non_blocking(std::io::sink()).1))
    };

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
            commands::twitch::restart_twitch,
            commands::twitch::get_twitch_status,
            // Logging commands
            get_logging_settings,
            save_logging_settings,
            // Unified settings commands
            commands::get_all_app_settings,
            commands::is_backend_ready,
            commands::confirm_backend_ready,
        ])
        .setup({
            let settings_clone = settings.clone();
            move |app| setup::init_app(app, settings_clone.clone())
        })
        .on_window_event(|window, event| {
            // Обрабатываем события главного окна
            if window.label() == "main" {
                // Позиция сохраняется только при закрытии (событие Destroyed)
                // Предотвращаем закрытие - скрываем окно вместо этого
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    info!("Main window close requested - hiding to tray");
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

                                info!("Window focused - initializing hotkeys...");

                                // Initialize hotkeys using tauri-plugin-global-shortcut
                                match hotkeys::initialize_hotkeys(0, app_state_inner, app_handle.clone()) {
                                    Ok(_) => {
                                        info!("Hotkeys initialized successfully");
                                    }
                                    Err(e) => {
                                        error!(error = %e, "Failed to initialize hotkeys");
                                    }
                                }
                            });
                        } else {
                            // Window lost focus - remove always-on-top
                            info!("Main window lost focus - removing always-on-top");
                            let _ = window.set_always_on_top(false);
                        }
                    }
                    tauri::WindowEvent::Destroyed => {
                        // Сохраняем позицию главного окна
                        if let Some(windows_manager) = window.app_handle().try_state::<WindowsManager>() {
                            if let Ok(pos) = window.outer_position() {
                                let x: i32 = pos.x;
                                let y: i32 = pos.y;
                                info!(x, y, "Main window destroyed - saving position");
                                let _ = windows_manager.set_main_position(Some(x), Some(y));
                            }

                            // Сохраняем позицию плавающего окна (если оно было показано)
                            if let Some(floating_window) = window.app_handle().get_webview_window("floating") {
                                if let Ok(true) = floating_window.is_visible() {
                                    if let Ok(pos) = floating_window.outer_position() {
                                        let x = pos.x;
                                        let y = pos.y;
                                        info!(x, y, "Saving floating window position");
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
