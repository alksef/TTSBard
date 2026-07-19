use crate::config::SettingsManager;
use crate::state::AppState;
use crate::webview::WebViewSettings;
use std::fs;
use tauri::{Manager, State};

/// Get current webview settings from AppState
#[tauri::command]
pub async fn get_webview_settings(state: State<'_, AppState>) -> Result<WebViewSettings, String> {
    let settings = state.webview.settings.read().await;
    Ok(WebViewSettings {
        enabled: settings.enabled,
        start_on_boot: settings.start_on_boot,
        port: settings.port,
        bind_address: settings.bind_address.clone(),
        access_token: settings.access_token.clone(),
        upnp_enabled: settings.upnp_enabled,
    })
}

/// Get individual webview setting fields to avoid full cloning
#[tauri::command]
pub async fn get_webview_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.webview.settings.read().await.enabled)
}

#[tauri::command]
pub async fn get_webview_start_on_boot(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.webview.settings.read().await.start_on_boot)
}

#[tauri::command]
pub async fn get_webview_port(state: State<'_, AppState>) -> Result<u16, String> {
    Ok(state.webview.settings.read().await.port)
}

#[tauri::command]
pub async fn get_webview_bind_address(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.webview.settings.read().await.bind_address.clone())
}

/// Save webview settings to AppState and persist to files
#[tauri::command]
pub async fn save_webview_settings(
    settings: WebViewSettings,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    tracing::info!(
        enabled = settings.enabled,
        start_on_boot = settings.start_on_boot,
        port = settings.port,
        bind_address = %settings.bind_address,
        "Saving webview settings"
    );

    // Force disable UPnP when bind_address is 127.0.0.1
    let settings = if settings.bind_address == "127.0.0.1" && settings.upnp_enabled {
        tracing::info!("Forcing UPnP to false because bind_address is 127.0.0.1");
        WebViewSettings {
            upnp_enabled: false,
            ..settings
        }
    } else {
        settings
    };

    // Check if enabled status or port changed (start_on_boot doesn't require restart)
    let old_settings = state.webview.settings.read().await;
    let enabled_changed = old_settings.enabled != settings.enabled;
    let port_changed =
        old_settings.port != settings.port || old_settings.bind_address != settings.bind_address;
    let upnp_changed = old_settings.upnp_enabled != settings.upnp_enabled;
    drop(old_settings);

    // Get SettingsManager once and persist to config
    let settings_manager = app_handle.try_state::<SettingsManager>();
    if let Some(manager) = settings_manager {
        let bind_addr = settings.bind_address.clone();
        let start_on_boot = settings.start_on_boot;
        let port = settings.port;
        let upnp_enabled = settings.upnp_enabled;
        super::persist_blocking(manager.inner(), move |mgr| {
            mgr.set_webview_start_on_boot(start_on_boot)?;
            mgr.set_webview_port(port)?;
            mgr.set_webview_bind_address(bind_addr)?;
            mgr.set_webview_upnp_enabled(upnp_enabled)?;
            Ok(())
        })
        .await?;
        super::emit_settings_changed(&app_handle);
    }

    // Only after successful file save, update AppState (runtime state)
    let mut s = state.webview.settings.write().await;
    *s = settings.clone();
    drop(s);

    // Trigger UPnP toggle if it changed (without server restart)
    if upnp_changed {
        state
            .webview
            .send_event(crate::events::AppEvent::ToggleUpnp(settings.upnp_enabled));
    }

    // Trigger server restart if server settings changed
    // Note: start_on_boot changes don't require restart (only affects next boot)
    if enabled_changed || port_changed {
        tracing::info!("Sending RestartWebViewServer event to WebView server");
        // Send restart event directly to WebView server using the state parameter
        state
            .webview
            .send_event(crate::events::AppEvent::RestartWebViewServer);
        tracing::debug!("RestartWebViewServer event sent successfully");
        Ok("Настройки сохранены. Сервер перезапускается...".to_string())
    } else {
        Ok("Настройки сохранены.".to_string())
    }
}

/// Get local IP address using UDP socket trick
#[tauri::command]
pub fn get_local_ip() -> Result<String, String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind socket: {}", e))?;
    socket
        .connect("8.8.8.8:80")
        .map_err(|e| format!("Failed to connect: {}", e))?;
    let local_ip = socket
        .local_addr()
        .map_err(|e| format!("Failed to get local address: {}", e))?
        .ip()
        .to_string();
    Ok(local_ip)
}

/// Open template folder in file explorer
#[tauri::command]
pub async fn open_template_folder() -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or("Failed to get config dir")?
        .join("ttsbard")
        .join("webview");

    // Create directory first, before canonicalize
    fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;

    let config_dir = config_dir
        .canonicalize()
        .map_err(|e| format!("Invalid config dir: {}", e))?;

    let path = config_dir.to_str().ok_or("Invalid path")?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args([path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args([path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .args([path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Send test message to SSE (without TTS)
#[tauri::command]
pub async fn send_test_message(text: String, state: State<'_, AppState>) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Text cannot be empty".to_string());
    }

    // Send ONLY to WebView channel, not to TTS
    // This allows testing WebView display without triggering voice synthesis
    state
        .webview
        .send_event(crate::events::AppEvent::TextSentToTts(text));
    Ok(())
}

/// Reload templates from disk (hot reload without server restart)
#[tauri::command]
pub async fn reload_templates(state: State<'_, AppState>) -> Result<String, String> {
    // Send event to reload templates without restarting the server
    state
        .webview
        .send_event(crate::events::AppEvent::ReloadWebViewTemplates);
    Ok("Шаблоны обновлены!".to_string())
}

// ==================== Security Commands ====================

/// Generate a new access token for external WebView access
#[tauri::command]
pub async fn generate_webview_token(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let token = uuid::Uuid::new_v4().to_string();

    // Update both runtime state and persistent settings
    let mut settings = state.webview.settings.write().await;
    settings.access_token = Some(token.clone());
    drop(settings);

    // Persist to settings
    let settings_manager = app_handle.try_state::<SettingsManager>();
    if let Some(manager) = settings_manager {
        let t = token.clone();
        super::persist_blocking(manager.inner(), move |mgr| {
            mgr.set_webview_access_token(Some(t))
        })
        .await?;
        super::emit_settings_changed(&app_handle);
    }

    Ok(token)
}

/// Get the masked access token (first 8 chars only)
#[tauri::command]
pub async fn get_webview_token(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let settings = state.webview.settings.read().await;
    Ok(settings.access_token.as_ref().map(|t| {
        if t.len() > 8 {
            format!("{}***", &t[..8])
        } else {
            t.clone()
        }
    }))
}

/// Copy the access token to clipboard
#[tauri::command]
pub async fn copy_webview_token(state: State<'_, AppState>) -> Result<String, String> {
    let settings = state.webview.settings.read().await;
    let token = settings
        .access_token
        .clone()
        .ok_or("Токен не сгенерирован")?;

    drop(settings);
    Ok(token)
}

/// Regenerate the access token
#[tauri::command]
pub async fn regenerate_webview_token(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let token = uuid::Uuid::new_v4().to_string();

    // Update both runtime state and persistent settings
    let mut settings = state.webview.settings.write().await;
    settings.access_token = Some(token.clone());
    drop(settings);

    // Persist to settings
    let settings_manager = app_handle.try_state::<SettingsManager>();
    if let Some(manager) = settings_manager {
        let t = token.clone();
        super::persist_blocking(manager.inner(), move |mgr| {
            mgr.set_webview_access_token(Some(t))
        })
        .await?;
        super::emit_settings_changed(&app_handle);
    }

    // Restart server to apply new token
    state
        .webview
        .send_event(crate::events::AppEvent::RestartWebViewServer);

    Ok("Токен перегенерирован. Старый токен больше не действителен.".to_string())
}

/// Set UPnP enabled status
#[tauri::command]
pub async fn set_webview_upnp_enabled(
    enabled: bool,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // Update runtime state
    let mut settings = state.webview.settings.write().await;
    settings.upnp_enabled = enabled;
    drop(settings);

    // Persist to settings
    let settings_manager = app_handle.try_state::<SettingsManager>();
    if let Some(manager) = settings_manager {
        super::persist_blocking(manager.inner(), move |mgr| {
            mgr.set_webview_upnp_enabled(enabled)
        })
        .await?;
        super::emit_settings_changed(&app_handle);
    }

    // Toggle UPnP without server restart
    state
        .webview
        .send_event(crate::events::AppEvent::ToggleUpnp(enabled));

    if enabled {
        Ok("UPnP включён".to_string())
    } else {
        Ok("UPnP выключен".to_string())
    }
}

/// Get UPnP enabled status
#[tauri::command]
pub async fn get_webview_upnp_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.webview.settings.read().await.upnp_enabled)
}

/// Forward typing state to WebView SSE (consumer adapter for the editor typing burst)
#[tauri::command]
pub async fn set_webview_typing(typing: bool, state: State<'_, AppState>) -> Result<(), String> {
    state
        .webview
        .send_event(crate::events::AppEvent::WebViewTypingChanged(typing));
    Ok(())
}

/// Get external/public IP address with fallback
#[tauri::command]
pub async fn get_external_ip() -> Result<String, String> {
    let sources = vec![
        "https://api.ipify.org?format=text",
        "https://icanhazip.com",
        "https://ifconfig.me",
    ];

    let client = reqwest::Client::new();
    for url in sources {
        match client.get(url).send().await {
            Ok(resp) => {
                if let Ok(ip) = resp.text().await {
                    let ip = ip.trim().to_string();
                    if !ip.is_empty() {
                        return Ok(ip);
                    }
                }
            }
            Err(_) => continue,
        }
    }

    Err("Не удалось получить внешний IP. Проверьте подключение к интернету.".to_string())
}
