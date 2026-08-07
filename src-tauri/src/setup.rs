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
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager};
use tracing::{error, info, warn};

use crate::commands::playback::PlaybackState;
use crate::commands::speech_queue::SpeechQueueState;
use crate::commands::telegram::TelegramState;
use crate::config::{AppSettings, SettingsManager, WindowsManager, WindowsSettings};
use crate::event_loop::EventHandler;
use crate::events::AppEvent;
use crate::secret_log;
use crate::soundpanel::SoundPanelState;
use crate::speech_queue::JobStatus;
use crate::state::AppState;
use crate::tts::TtsProviderType;
use std::sync::Arc;

/// Initialize the application (called from Tauri's setup callback)
///
/// Settings are passed from lib.rs to avoid race condition from double loading.
/// Logger is initialized before this function with the same settings.
pub fn init_app(app: &App, mut settings: AppSettings) -> Result<(), Box<dyn std::error::Error>> {
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
    let pb_manager = Arc::new(crate::playback::PlaybackManager::new(
        app_handle_pb,
        event_tx.clone(),
        Some(app_state.inner().cached_devices.clone()),
    ));
    *app_state.inner().playback_manager.lock() = Some(pb_manager.clone());
    app.manage(PlaybackState(pb_manager));
    info!("Event sender configured in AppState");

    // Start speech queue worker after PlaybackManager is installed
    {
        let sq_state = app.state::<SpeechQueueState>();
        let worker_queue = sq_state.queue_arc();
        let worker_notify = sq_state.notifier();
        let worker_handle = app.handle().clone();
        let worker_shutdown = app_state.inner().shutdown.clone();
        let worker_editor = app_state.inner().editor.clone();
        let worker_playback = app_state.inner().playback_manager.clone();
        let worker_webview = app_state.inner().webview.clone();
        let worker_twitch = app_state.inner().twitch.clone();
        app_state.inner().runtime.spawn(async move {
            speech_worker(
                worker_queue,
                worker_notify,
                worker_handle,
                worker_shutdown,
                worker_editor,
                worker_playback,
                worker_webview,
                worker_twitch,
            )
            .await;
        });
        info!("Speech queue worker started");
    }

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

    // Restore saved concrete provider ID. A deleted Piper model falls back to
    // Silero, persists that choice and queues a one-shot frontend notification.
    // Other missing IDs keep the registry's generic safe fallback behavior.
    let saved_id = settings_manager.get_tts_provider_id();
    let missing_piper = {
        let registry = app_state.tts_registry.lock();
        missing_piper_model_name(saved_id.as_deref(), |id| registry.get(id).is_some())
    };

    if let Some(model_name) = missing_piper {
        app_state.init_silero_tts(Arc::clone(&telegram_state.client));
        app_state
            .tts_registry
            .lock()
            .select("silero")
            .expect("Silero is registered before Piper fallback selection");
        app_state.set_tts_provider_type(TtsProviderType::Silero);

        settings.tts.provider = TtsProviderType::Silero;
        settings.tts.provider_id = Some("silero".to_string());
        if let Err(error) = settings_manager.save(&settings) {
            warn!(error = %error, "Failed to persist Silero fallback for missing Piper model");
        }

        app_state.push_notification(missing_piper_notification(&model_name));
    } else {
        let restored_piper = {
            let mut registry = app_state.tts_registry.lock();
            registry.restore_saved_or_first(saved_id.as_deref());
            registry.active().and_then(|entry| match &entry.provider {
                crate::tts::TtsProvider::Piper(provider) => {
                    Some((entry.id.clone(), provider.clone()))
                }
                _ => None,
            })
        };

        // A persisted Piper choice is already selected from the user's point
        // of view. Prepare it before backend-ready so the first UI snapshot
        // reports Ready. Unselected discovered models remain lazy.
        if let Some((provider_id, provider)) = restored_piper {
            info!(provider_id, "Preparing restored Piper provider");
            if let Err(error) = provider.prepare() {
                warn!(provider_id, error = %error, "Failed to prepare restored Piper provider");
            }
        }
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

/// Show (restore) the main webview window: show, unminimize, and focus.
///
/// Logs failures with `warn!` and the provided `action` context, without panicking.
fn show_main_window(app_handle: &AppHandle, action: &str) {
    let window = match app_handle.get_webview_window("main") {
        Some(w) => w,
        None => {
            warn!(action, "show_main_window: main window not found");
            return;
        }
    };
    if let Err(e) = window.show() {
        warn!(action, error = %e, "show_main_window: show failed");
    }
    if let Err(e) = window.unminimize() {
        warn!(action, error = %e, "show_main_window: unminimize failed");
    }
    if let Err(e) = window.set_focus() {
        warn!(action, error = %e, "show_main_window: set_focus failed");
    }
}

/// Initialize system tray
fn init_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    // Load icon.png (512x512) for tray
    let png_bytes = include_bytes!("../icons/icon.png");
    let decoded_image = image::load_from_memory(png_bytes)
        .map_err(|e| format!("Failed to decode tray icon: {}", e))?;
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

    // Create context menu
    let show_main = MenuItem::with_id(
        &app_handle,
        "show_main",
        "Показать главное окно",
        true,
        None as Option<&str>,
    )
    .map_err(|e| format!("Failed to create 'show_main' menu item: {}", e))?;
    let sp_toggle = MenuItem::with_id(
        &app_handle,
        "toggle_soundpanel",
        "Саундпад",
        true,
        None as Option<&str>,
    )
    .map_err(|e| format!("Failed to create 'toggle_soundpanel' menu item: {}", e))?;
    let pc_toggle = MenuItem::with_id(
        &app_handle,
        "toggle_playback",
        "Управление воспроизведением",
        true,
        None as Option<&str>,
    )
    .map_err(|e| format!("Failed to create 'toggle_playback' menu item: {}", e))?;
    let separator = PredefinedMenuItem::separator(&app_handle)
        .map_err(|e| format!("Failed to create separator: {}", e))?;
    let quit_item = MenuItem::with_id(&app_handle, "quit", "Выход", true, None as Option<&str>)
        .map_err(|e| format!("Failed to create 'quit' menu item: {}", e))?;

    let menu = Menu::with_items(
        &app_handle,
        &[&show_main, &sp_toggle, &pc_toggle, &separator, &quit_item],
    )
    .map_err(|e| format!("Failed to build menu: {}", e))?;

    // Create tray icon
    info!("[TRAY] Creating tray icon...");
    TrayIconBuilder::with_id("main")
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
                    show_main_window(tray.app_handle(), "tray-left-click");
                }
            }
        })
        .on_menu_event(|tray, event| match event.id.as_ref() {
            "show_main" => {
                show_main_window(tray.app_handle(), "tray-menu-show_main");
            }
            "toggle_soundpanel" => {
                if let Err(e) =
                    crate::soundpanel_window::toggle_soundpanel_window(tray.app_handle())
                {
                    warn!(error = %e, "Tray toggle_soundpanel failed");
                }
            }
            "toggle_playback" => {
                if let Err(e) = crate::playback_window::toggle_playback_window(tray.app_handle()) {
                    warn!(error = %e, "Tray toggle_playback failed");
                }
            }
            "quit" => {
                tray.app_handle().exit(0);
            }
            _ => {}
        })
        .menu(&menu)
        .build(&app_handle)
        .map_err(|e| format!("Failed to build tray icon: {}", e))?;
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
    // Подключаем AppHandle ДО всего: connection-actor должен уметь эмитить статус
    // при transport-failure из idle независимо от того, включён ли автозапуск.
    app_state.vtube_studio.attach_app_handle(app_handle.clone());

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

fn q_state_dto(
    q: &Arc<parking_lot::Mutex<crate::speech_queue::SpeechQueue>>,
) -> crate::speech_queue::SpeechQueueStateDto {
    q.lock().state()
}

// ── Speech worker ──

#[allow(clippy::too_many_arguments)] // Worker dependencies are explicit ownership boundaries.
async fn speech_worker(
    queue: Arc<parking_lot::Mutex<crate::speech_queue::SpeechQueue>>,
    notify: Arc<tokio::sync::Notify>,
    app_handle: AppHandle,
    shutdown: tokio_util::sync::CancellationToken,
    editor: Arc<crate::editor::EditorService>,
    playback_manager: Arc<parking_lot::Mutex<Option<Arc<crate::playback::PlaybackManager>>>>,
    webview: Arc<crate::webview::service::WebViewService>,
    twitch: Arc<crate::twitch::TwitchService>,
) {
    use crate::commands::tts_pipeline;

    loop {
        let work_item = {
            let mut q = queue.lock();
            match q.claim_next_generation() {
                Ok(Some(item)) => Some(item),
                Ok(None) => {
                    drop(q);
                    None
                }
                Err(e) => {
                    warn!(error = %e, "claim_next_generation error, worker continuing");
                    drop(q);
                    None
                }
            }
        };

        let work_item = match work_item {
            Some(item) => item,
            None => {
                tokio::select! {
                    _ = notify.notified() => {
                        continue;
                    }
                    _ = shutdown.cancelled() => {
                        info!("Speech worker exiting on shutdown (idle wait)");
                        return;
                    }
                }
            }
        };

        {
            let q = queue.lock();
            let dto = q.state();
            drop(q);
            let _ = app_handle.emit("speech-queue-changed", dto);
        }

        let job_id = work_item.job_id;
        let snapshot = work_item.snapshot;
        let original_text = work_item.original_text;

        let preparation = tts_pipeline::prepare_speech(&snapshot, &original_text);
        let prepared = tokio::select! {
            result = preparation => result,
            _ = shutdown.cancelled() => {
                let mut q = queue.lock();
                let _ = q.fail_generation(
                    job_id,
                    "Speech preparation cancelled (shutdown)".to_string(),
                );
                let dto = q.state();
                drop(q);
                let _ = app_handle.emit("speech-queue-changed", dto);
                info!("Speech worker exiting on shutdown (during preparation)");
                return;
            }
        };

        match prepared {
            Ok(prepared) => {
                let (speaker, mic) =
                    tts_pipeline::compute_output_configs(&snapshot.audio, &snapshot.audio_effects);

                if speaker.is_none() && mic.is_none() {
                    let mut q = queue.lock();
                    let _ = q.fail_generation(
                        job_id,
                        "All outputs disabled (speaker and mic both off)".to_string(),
                    );
                    let dto = q.state();
                    drop(q);
                    let _ = app_handle.emit("speech-queue-changed", dto);
                    continue;
                }

                let processed_text = prepared.processed_text.clone();

                let handoff_guard = {
                    let mut q = queue.lock();
                    let status = q.get_status(job_id);
                    if status != Some(JobStatus::Generating) {
                        drop(q);
                        info!(job_id = %job_id, observed_status = ?status, "worker discard: not Generating after preparation");
                        let _ = app_handle.emit("speech-queue-changed", q_state_dto(&queue));
                        continue;
                    }
                    let _ = q.mark_ready(job_id, processed_text.clone());
                    let guard = q
                        .get_handoff_guard(job_id)
                        .expect("handoff_guard missing after mark_ready");
                    let dto = q.state();
                    drop(q);
                    let _ = app_handle.emit("speech-queue-changed", dto);
                    guard
                };

                let handoff_accepted = {
                    let _g = handoff_guard.lock();

                    {
                        let q = queue.lock();
                        let status = q.get_status(job_id);
                        if status != Some(JobStatus::Ready) {
                            drop(q);
                            info!(job_id = %job_id, observed_status = ?status, "worker discard: cancelled after mark_ready");
                            false
                        } else {
                            drop(q);
                            let pb_guard = playback_manager.lock();
                            if let Some(pb) = pb_guard.as_ref() {
                                pb.enqueue_with_outputs(
                                    job_id.to_string(),
                                    processed_text.clone(),
                                    prepared.audio,
                                    speaker,
                                    mic,
                                )
                            } else {
                                false
                            }
                        }
                    }
                };

                if !handoff_accepted {
                    let mut q = queue.lock();
                    if q.get_status(job_id) == Some(JobStatus::Ready) {
                        let _ = q.fail_playback(
                            job_id,
                            "Playback handoff rejected (queue full or no manager)".to_string(),
                        );
                        let dto = q.state();
                        drop(q);
                        let _ = app_handle.emit("speech-queue-changed", dto);
                    }
                    continue;
                }

                {
                    let text = processed_text.clone();
                    let webview_svc = webview.clone();
                    let twitch_svc = twitch.clone();
                    let skip_twitch = snapshot.skip_twitch;
                    let skip_webview = snapshot.skip_webview;
                    let join_handle = tokio::task::spawn_blocking(move || {
                        route_processed_text_from_handles(
                            &webview_svc,
                            &twitch_svc,
                            &text,
                            skip_twitch,
                            skip_webview,
                        );
                    });
                    if let Err(e) = join_handle.await {
                        warn!(error = %e, "route_processed_text join failed");
                    }
                }

                {
                    if let Some(hm) = editor.history_manager.lock().as_ref() {
                        if prepared.cache_saved || prepared.cache_hit {
                            hm.record_phrase_with_meta(
                                &processed_text,
                                &prepared.provider_name,
                                &prepared.voice_name,
                                &prepared.cache_key,
                            );
                        } else {
                            hm.record_phrase(&processed_text);
                        }
                    }
                }

                let history_event = crate::events::AppEvent::TextSentToTts(
                    crate::events::RoutedText::broadcast(processed_text.clone()),
                );
                let event_name = history_event.to_tauri_event();
                let _ = app_handle.emit(event_name, &history_event);

                loop {
                    let status = {
                        let q = queue.lock();
                        q.get_status(job_id)
                    };

                    match status {
                        Some(JobStatus::Playing) => break,
                        Some(JobStatus::Failed) => break,
                        Some(JobStatus::Ready) => {
                            tokio::select! {
                                _ = notify.notified() => {
                                    continue;
                                }
                                _ = shutdown.cancelled() => {
                                    info!("Speech worker exiting on shutdown (waiting for Playing)");
                                    return;
                                }
                            }
                        }
                        _ => {
                            warn!(?status, job_id = %job_id, "Unexpected status waiting for Playing");
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                let mut q = queue.lock();
                let _ = q.fail_generation(job_id, e.clone());
                let dto = q.state();
                drop(q);
                let _ = app_handle.emit("speech-queue-changed", dto);
                warn!(error = %e, job_id = %job_id, "Speech preparation failed, worker waiting for retry/skip");
            }
        }
    }
}

fn route_processed_text_from_handles(
    webview: &crate::webview::service::WebViewService,
    twitch: &crate::twitch::TwitchService,
    text: &str,
    skip_twitch: bool,
    skip_webview: bool,
) {
    if !skip_webview {
        webview.send_event(crate::events::AppEvent::TextSentToTts(
            crate::events::RoutedText::broadcast(text.to_string()),
        ));
    }
    if !skip_twitch {
        let settings = twitch.settings.blocking_read();
        if settings.enabled {
            drop(settings);
            twitch.send_event(crate::events::TwitchEvent::SendMessage(text.to_string()));
        }
    }
}

fn missing_piper_model_name<F>(saved_id: Option<&str>, is_registered: F) -> Option<String>
where
    F: FnOnce(&str) -> bool,
{
    let id = saved_id?;
    let model_name = id.strip_prefix("local-piper:")?;
    if model_name.is_empty() || is_registered(id) {
        return None;
    }
    Some(model_name.to_string())
}

fn missing_piper_notification(model_name: &str) -> String {
    format!(
        "Piper-модель «{}» не найдена. Выбран провайдер Silero",
        model_name
    )
}

#[cfg(test)]
mod tests {
    use super::{missing_piper_model_name, missing_piper_notification};

    #[test]
    fn missing_saved_piper_returns_safe_model_name() {
        assert_eq!(
            missing_piper_model_name(Some("local-piper:irina"), |_| false),
            Some("irina".to_string())
        );
    }

    #[test]
    fn registered_piper_and_builtin_ids_do_not_trigger_fallback() {
        assert_eq!(
            missing_piper_model_name(Some("local-piper:irina"), |_| true),
            None
        );
        assert_eq!(missing_piper_model_name(Some("silero"), |_| false), None);
        assert_eq!(missing_piper_model_name(None, |_| false), None);
    }

    #[test]
    fn missing_piper_notification_matches_user_facing_contract() {
        assert_eq!(
            missing_piper_notification("irina"),
            "Piper-модель «irina» не найдена. Выбран провайдер Silero"
        );
    }
}
