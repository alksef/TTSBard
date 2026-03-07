use crate::preprocessor::{TextPreprocessor, replacements_file, usernames_file};
use crate::state::AppState;
use tauri::State;
use std::fs;

/// Get the current replacements list content
#[tauri::command]
pub fn get_replacements() -> Result<String, String> {
    let path = replacements_file()
        .map_err(|e| format!("Failed to get replacements file path: {}", e))?;

    if path.exists() {
        fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read replacements file: {}", e))
    } else {
        Ok(String::new())
    }
}

/// Save the replacements list content
#[tauri::command]
pub fn save_replacements(content: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = replacements_file()
        .map_err(|e| format!("Failed to get replacements file path: {}", e))?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    fs::write(&path, content)
        .map_err(|e| format!("Failed to write replacements file: {}", e))?;

    // Reload preprocessor in state
    state.reload_preprocessor();

    Ok(())
}

/// Get the current usernames list content
#[tauri::command]
pub fn get_usernames() -> Result<String, String> {
    let path = usernames_file()
        .map_err(|e| format!("Failed to get usernames file path: {}", e))?;

    if path.exists() {
        fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read usernames file: {}", e))
    } else {
        Ok(String::new())
    }
}

/// Save the usernames list content
#[tauri::command]
pub fn save_usernames(content: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = usernames_file()
        .map_err(|e| format!("Failed to get usernames file path: {}", e))?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    fs::write(&path, content)
        .map_err(|e| format!("Failed to write usernames file: {}", e))?;

    // Reload preprocessor in state
    state.reload_preprocessor();

    Ok(())
}

/// Preview preprocessed text (for UI testing)
#[tauri::command]
pub fn preview_preprocessing(text: String) -> Result<String, String> {
    let preprocessor = TextPreprocessor::load_from_files()
        .map_err(|e| format!("Failed to load preprocessor: {}", e))?;

    Ok(preprocessor.process(&text))
}

/// Load preprocessor data for live replacement in UI
/// Returns a struct with replacements and usernames as HashMaps
#[tauri::command]
pub fn load_preprocessor_data() -> Result<PreprocessorData, String> {
    let preprocessor = TextPreprocessor::load_from_files()
        .map_err(|e| format!("Failed to load preprocessor: {}", e))?;

    Ok(PreprocessorData {
        replacements: preprocessor.get_replacements_map().clone(),
        usernames: preprocessor.get_usernames_map().clone(),
    })
}

/// Struct to hold preprocessor data for UI
#[derive(serde::Serialize)]
pub struct PreprocessorData {
    pub replacements: std::collections::HashMap<String, String>,
    pub usernames: std::collections::HashMap<String, String>,
}
