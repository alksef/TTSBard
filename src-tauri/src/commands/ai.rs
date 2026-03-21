//! AI text correction commands
//!
//! Provides Tauri commands for managing AI provider settings

use crate::config::SettingsManager;
use tauri::State;

/// Set AI provider
#[tauri::command]
pub fn set_ai_provider(
    settings_manager: State<'_, SettingsManager>,
    provider: String,
) -> Result<(), String> {
    let provider_enum = match provider.as_str() {
        "openai" => crate::config::AiProviderType::OpenAi,
        "zai" => crate::config::AiProviderType::ZAi,
        _ => return Err("Invalid provider".into()),
    };
    settings_manager.set_ai_provider(provider_enum)
        .map_err(|e| format!("Failed to save AI provider: {}", e))
}

/// Set AI global prompt
#[tauri::command]
pub fn set_ai_prompt(
    settings_manager: State<'_, SettingsManager>,
    prompt: String,
) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("Prompt cannot be empty".into());
    }
    settings_manager.set_ai_prompt(prompt)
        .map_err(|e| format!("Failed to save AI prompt: {}", e))
}

/// Set OpenAI API key for AI text correction
#[tauri::command]
pub fn set_ai_openai_api_key(
    settings_manager: State<'_, SettingsManager>,
    key: String,
) -> Result<(), String> {
    settings_manager.set_ai_openai_api_key(Some(key))
        .map_err(|e| format!("Failed to save API key: {}", e))
}

/// Set OpenAI use proxy for AI text correction
#[tauri::command]
pub fn set_ai_openai_use_proxy(
    settings_manager: State<'_, SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    settings_manager.set_ai_openai_use_proxy(enabled)
        .map_err(|e| format!("Failed to save proxy setting: {}", e))
}

/// Set Z.ai URL
#[tauri::command]
pub fn set_ai_zai_url(
    settings_manager: State<'_, SettingsManager>,
    url: String,
) -> Result<(), String> {
    if url.trim().is_empty() {
        return Err("URL cannot be empty".into());
    }
    settings_manager.set_ai_zai_url(Some(url))
        .map_err(|e| format!("Failed to save Z.ai URL: {}", e))
}

/// Set Z.ai token
#[tauri::command]
pub fn set_ai_zai_token(
    settings_manager: State<'_, SettingsManager>,
    token: String,
) -> Result<(), String> {
    if token.trim().is_empty() {
        return Err("Token cannot be empty".into());
    }
    settings_manager.set_ai_zai_token(Some(token))
        .map_err(|e| format!("Failed to save Z.ai token: {}", e))
}
