use crate::config::SettingsManager;
use crate::state::AppState;
use crate::webview::WebViewSettings;
use tauri::{Manager, State};

/// Get current webview settings from AppState
#[tauri::command]
pub async fn get_webview_settings(
    state: State<'_, AppState>,
) -> Result<WebViewSettings, String> {
    let settings = state.webview_settings.read().await;
    // Клонируем только необходимые поля, html_template и css_style могут быть большими
    Ok(WebViewSettings {
        enabled: settings.enabled,
        start_on_boot: settings.start_on_boot,
        port: settings.port,
        bind_address: settings.bind_address.clone(),
        html_template: settings.html_template.clone(),
        css_style: settings.css_style.clone(),
        animation_speed: settings.animation_speed,
    })
}

/// Get individual webview setting fields to avoid full cloning
#[tauri::command]
pub async fn get_webview_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.webview_settings.read().await.enabled)
}

#[tauri::command]
pub async fn get_webview_start_on_boot(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.webview_settings.read().await.start_on_boot)
}

#[tauri::command]
pub async fn get_webview_port(state: State<'_, AppState>) -> Result<u16, String> {
    Ok(state.webview_settings.read().await.port)
}

#[tauri::command]
pub async fn get_webview_bind_address(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.webview_settings.read().await.bind_address.clone())
}

#[tauri::command]
pub async fn get_webview_animation_speed(state: State<'_, AppState>) -> Result<u32, String> {
    Ok(state.webview_settings.read().await.animation_speed)
}

/// Save webview settings to AppState and persist to files
#[tauri::command]
pub async fn save_webview_settings(
    settings: WebViewSettings,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    eprintln!("[WEBVIEW] Saving settings: enabled={}, start_on_boot={}, port={}",
        settings.enabled, settings.start_on_boot, settings.port);

    // Check if enabled status or port changed (start_on_boot doesn't require restart)
    let old_settings = state.webview_settings.read().await;
    let enabled_changed = old_settings.enabled != settings.enabled;
    let port_changed = old_settings.port != settings.port || old_settings.bind_address != settings.bind_address;
    let start_on_boot_changed = old_settings.start_on_boot != settings.start_on_boot;
    drop(old_settings);

    // Get SettingsManager once and persist to config
    let settings_manager = app_handle.try_state::<SettingsManager>();
    if let Some(manager) = settings_manager {
        manager.set_webview_start_on_boot(settings.start_on_boot)
            .map_err(|e| format!("Failed to save webview start_on_boot: {}", e))?;
        manager.set_webview_port(settings.port)
            .map_err(|e| format!("Failed to save webview port: {}", e))?;
        manager.set_webview_bind_address(settings.bind_address.clone())
            .map_err(|e| format!("Failed to save webview bind_address: {}", e))?;
        manager.set_webview_animation_speed(settings.animation_speed)
            .map_err(|e| format!("Failed to save webview animation_speed: {}", e))?;
        manager.set_webview_html_template(settings.html_template.clone())
            .map_err(|e| format!("Failed to save webview html_template: {}", e))?;
        manager.set_webview_css_style(settings.css_style.clone())
            .map_err(|e| format!("Failed to save webview css_style: {}", e))?;
    }

    // Only after successful file save, update AppState (runtime state)
    let mut s = state.webview_settings.write().await;
    *s = settings.clone();
    drop(s);

    // Trigger server restart if server settings changed
    // Note: start_on_boot changes don't require restart (only affects next boot)
    if enabled_changed || port_changed {
        if start_on_boot_changed {
            // Only start_on_boot changed, no restart needed
            eprintln!("[WEBVIEW] Only start_on_boot changed, no restart needed");
        }
        eprintln!("[WEBVIEW] Sending RestartWebViewServer event to WebView server...");
        // Send restart event directly to WebView server using the state parameter
        state.send_webview_event(crate::events::AppEvent::RestartWebViewServer);
        eprintln!("[WEBVIEW] RestartWebViewServer event sent successfully!");

        if start_on_boot_changed && !(enabled_changed || port_changed) {
            Ok("Настройки сохранены. Автозапуск обновлён (применится при следующем старте).".to_string())
        } else {
            Ok("Настройки сохранены. Сервер перезапускается...".to_string())
        }
    } else {
        Ok("Настройки сохранены. Изменения HTML/CSS применены немедленно.".to_string())
    }
}

/// Get local IP address using UDP socket trick
#[tauri::command]
pub fn get_local_ip() -> Result<String, String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind socket: {}", e))?;
    socket.connect("8.8.8.8:80")
        .map_err(|e| format!("Failed to connect: {}", e))?;
    let local_ip = socket.local_addr()
        .map_err(|e| format!("Failed to get local address: {}", e))?
        .ip()
        .to_string();
    Ok(local_ip)
}
