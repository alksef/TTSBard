use crate::settings::AppSettings;
use crate::state::AppState;
use crate::webview::WebViewSettings;
use tauri::State;

/// Get current webview settings from AppState
#[tauri::command]
pub async fn get_webview_settings(
    state: State<'_, AppState>,
) -> Result<WebViewSettings, String> {
    let settings = state.webview_settings.read().await;
    Ok(settings.clone())
}

/// Save webview settings to AppState and persist to files
#[tauri::command]
pub async fn save_webview_settings(
    settings: WebViewSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Save to AppState (runtime state)
    let mut s = state.webview_settings.write().await;
    *s = settings.clone();
    drop(s);

    // Persist to files
    AppSettings::save_webview_settings(&settings)
        .map_err(|e| format!("Failed to save webview settings to files: {}", e))?;

    Ok(())
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
