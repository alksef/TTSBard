//! AI text correction commands
//!
//! Provides Tauri commands for managing AI provider settings

use crate::config::SettingsManager;
use crate::state::AppState;
use tauri::{AppHandle, Emitter, State};

/// Set AI provider
#[tauri::command]
pub fn set_ai_provider(
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    provider: String,
) -> Result<(), String> {
    let provider_enum = match provider.as_str() {
        "openai" => crate::config::AiProviderType::OpenAi,
        "zai" => crate::config::AiProviderType::ZAi,
        _ => return Err("Invalid provider".into()),
    };
    settings_manager.set_ai_provider(provider_enum)
        .map_err(|e| format!("Failed to save AI provider: {}", e))?;

    // Invalidate AI client cache when provider changes
    state.invalidate_ai_client();

    Ok(())
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
    state: State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    settings_manager.set_ai_openai_api_key(Some(key))
        .map_err(|e| format!("Failed to save API key: {}", e))?;

    // Invalidate AI client cache when API key changes
    state.invalidate_ai_client();

    Ok(())
}

/// Set OpenAI use proxy for AI text correction
#[tauri::command]
pub fn set_ai_openai_use_proxy(
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    settings_manager.set_ai_openai_use_proxy(enabled)
        .map_err(|e| format!("Failed to save proxy setting: {}", e))?;

    // Invalidate AI client cache when proxy setting changes
    state.invalidate_ai_client();

    Ok(())
}

/// Set Z.ai URL
#[tauri::command]
pub fn set_ai_zai_url(
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    if url.trim().is_empty() {
        return Err("URL cannot be empty".into());
    }
    settings_manager.set_ai_zai_url(Some(url))
        .map_err(|e| format!("Failed to save Z.ai URL: {}", e))?;

    // Invalidate AI client cache when URL changes
    state.invalidate_ai_client();

    Ok(())
}

/// Set Z.ai API key
#[tauri::command]
pub fn set_ai_zai_api_key(
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    api_key: String,
) -> Result<(), String> {
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".into());
    }
    settings_manager.set_ai_zai_api_key(Some(api_key))
        .map_err(|e| format!("Failed to save Z.ai API key: {}", e))?;

    // Invalidate AI client cache when API key changes
    state.invalidate_ai_client();

    Ok(())
}

/// Correct text using AI
///
/// Sends the text to the configured AI provider for correction.
/// The global prompt from settings is used as the system prompt.
#[tauri::command]
pub async fn correct_text(
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    text: String,
) -> Result<String, String> {
    tracing::info!("correct_text called with {} chars", text.len());

    // Load settings
    let settings = settings_manager.load()
        .map_err(|e| {
            tracing::error!("Failed to load settings: {}", e);
            format!("Failed to load settings: {}", e)
        })?;

    // Get or create cached AI client
    let client = state.get_or_create_ai_client(&settings.ai, &settings.tts.network)
        .map_err(|e| {
            tracing::error!("Failed to get AI client: {}", e);
            e
        })?;

    // Correct text
    let corrected = client.correct(&text, &settings.ai.prompt)
        .await
        .map_err(|e| {
            tracing::error!("AI correction failed: {}", e);
            e.to_string()
        })?;

    tracing::info!("Correction successful: {} -> {} chars", text.len(), corrected.len());
    Ok(corrected)
}

/// Set AI correction in editor enabled state
#[tauri::command]
pub fn set_editor_ai(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    tracing::info!(enabled, "set_editor_ai called");
    settings_manager.set_editor_ai(enabled)
        .map_err(|e| {
            tracing::error!("Failed to set editor AI: {}", e);
            format!("Failed to save: {}", e)
        })?;

    // Emit event to notify frontend
    let _ = app_handle.emit("settings-changed", ());

    Ok(())
}

/// Get AI correction in editor enabled state
#[tauri::command]
pub fn get_editor_ai(
    settings_manager: State<'_, SettingsManager>,
) -> bool {
    settings_manager.get_editor_ai()
}

/// Set OpenAI model for AI text correction
#[tauri::command]
pub fn set_ai_openai_model(
    settings_manager: State<'_, SettingsManager>,
    model: String,
) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err("Model cannot be empty".into());
    }
    settings_manager.set_ai_openai_model(model)
        .map_err(|e| format!("Failed to save OpenAI model: {}", e))
}

/// Get OpenAI model for AI text correction
#[tauri::command]
pub fn get_ai_openai_model(
    settings_manager: State<'_, SettingsManager>,
) -> String {
    settings_manager.get_ai_openai_model()
}

/// Set Z.ai model for AI text correction
#[tauri::command]
pub fn set_ai_zai_model(
    settings_manager: State<'_, SettingsManager>,
    model: String,
) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err("Model cannot be empty".into());
    }
    settings_manager.set_ai_zai_model(model)
        .map_err(|e| format!("Failed to save Z.ai model: {}", e))
}

/// Get Z.ai model for AI text correction
#[tauri::command]
pub fn get_ai_zai_model(
    settings_manager: State<'_, SettingsManager>,
) -> String {
    settings_manager.get_ai_zai_model()
}
