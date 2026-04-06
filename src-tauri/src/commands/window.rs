use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn resize_main_window(
    app_handle: AppHandle,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }))
            .map_err(|e| format!("Failed to resize: {}", e))?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}
