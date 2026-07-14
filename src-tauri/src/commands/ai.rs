//! AI text correction commands
//!
//! Provides Tauri commands for managing AI provider settings

use crate::commands::telegram::TelegramState;
use crate::config::SettingsManager;
use crate::secret_log;
use crate::state::AppState;
use crate::tts::{TtsProviderType, VoiceModel};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tracing::{debug, error, info, warn};

/// Set AI provider
#[tauri::command]
pub async fn set_ai_provider(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    provider: String,
) -> Result<(), String> {
    let provider_enum = match provider.as_str() {
        "openai" => crate::config::AiProviderType::OpenAi,
        "zai" => crate::config::AiProviderType::ZAi,
        "deepseek" => crate::config::AiProviderType::DeepSeek,
        "custom" => crate::config::AiProviderType::Custom,
        _ => return Err("Invalid provider".into()),
    };

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_provider(provider_enum)
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Set AI global prompt
#[tauri::command]
pub async fn set_ai_prompt(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    prompt: String,
) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("Prompt cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_prompt(prompt)
    })
    .await?;

    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Set OpenAI API key for AI text correction
#[tauri::command]
pub async fn set_ai_openai_api_key(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_openai_api_key(Some(key))
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Set OpenAI use proxy for AI text correction
#[tauri::command]
pub async fn set_ai_openai_use_proxy(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_openai_use_proxy(enabled)
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Set Z.ai URL
#[tauri::command]
pub async fn set_ai_zai_url(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    if url.trim().is_empty() {
        return Err("URL cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_zai_url(Some(url))
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Set Z.ai API key
#[tauri::command]
pub async fn set_ai_zai_api_key(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    api_key: String,
) -> Result<(), String> {
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_zai_api_key(Some(api_key))
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);

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
    let settings = settings_manager.load().map_err(|e| {
        tracing::error!("Failed to load settings: {}", e);
        format!("Failed to load settings: {}", e)
    })?;

    // Get or create cached AI client
    let client = state
        .get_or_create_ai_client(&settings.ai, &settings.tts.network)
        .map_err(|e| {
            tracing::error!("Failed to get AI client: {}", e);
            e
        })?;

    // Correct text
    let corrected = client
        .correct(&text, &settings.ai.prompt)
        .await
        .map_err(|e| {
            tracing::error!("AI correction failed: {}", e);
            e.to_string()
        })?;

    tracing::info!(
        "Correction successful: {} -> {} chars",
        text.len(),
        corrected.len()
    );
    Ok(corrected)
}

/// Set AI correction in editor enabled state
#[tauri::command]
pub async fn set_editor_ai(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    tracing::info!(enabled, "set_editor_ai called");

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_editor_ai(enabled)
    })
    .await?;

    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Get AI correction in editor enabled state
#[tauri::command]
pub fn get_editor_ai(settings_manager: State<'_, SettingsManager>) -> bool {
    settings_manager.get_editor_ai()
}

/// Set AI completion in editor enabled state
#[tauri::command]
pub async fn set_editor_ai_completion(
    app_handle: tauri::AppHandle,
    settings_manager: State<'_, SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_editor_ai_completion(enabled)
    })
    .await?;

    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Get AI completion in editor enabled state
#[tauri::command]
pub fn get_editor_ai_completion(settings_manager: State<'_, SettingsManager>) -> bool {
    settings_manager.get_editor_ai_completion()
}

/// Complete text using AI
///
/// Sends the context to the configured AI provider for continuation.
/// Returns a short continuation of the given text.
#[tauri::command]
pub async fn get_ai_completion(
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    context: String,
) -> Result<String, String> {
    let settings = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    if !settings.editor.ai_completion {
        return Ok(String::new());
    }

    let has_key = match settings.ai.provider {
        crate::config::AiProviderType::OpenAi => settings.ai.openai.api_key.is_some(),
        crate::config::AiProviderType::ZAi => settings.ai.zai.api_key.is_some(),
        crate::config::AiProviderType::DeepSeek => settings.ai.deepseek.api_key.is_some(),
        crate::config::AiProviderType::Custom => {
            settings.ai.custom.api_key.is_some() && settings.ai.custom.url.is_some()
        }
    };
    if !has_key {
        return Ok(String::new());
    }

    let client = state
        .get_or_create_ai_client(&settings.ai, &settings.tts.network)
        .map_err(|e| format!("AI client error: {}", e))?;

    let prompt = "Ты помощник для завершения текста. Пользователь написал часть текста. \
        Продолжи его одним-двумя словами или короткой фразой (максимум 5 слов). \
        Отвечай только продолжением, без пояснений. Пиши на том же языке, что и контекст.";

    let completion = client
        .correct(&context, prompt)
        .await
        .map_err(|e| e.to_string())?;

    let trimmed = completion.trim().to_string();
    Ok(trimmed)
}

/// Set OpenAI model for AI text correction
#[tauri::command]
pub async fn set_ai_openai_model(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    model: String,
) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err("Model cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_openai_model(model)
    })
    .await?;

    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Get OpenAI model for AI text correction
#[tauri::command]
pub fn get_ai_openai_model(settings_manager: State<'_, SettingsManager>) -> String {
    settings_manager.get_ai_openai_model()
}

/// Set Z.ai model for AI text correction
#[tauri::command]
pub async fn set_ai_zai_model(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    model: String,
) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err("Model cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_zai_model(model)
    })
    .await?;

    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Get Z.ai model for AI text correction
#[tauri::command]
pub fn get_ai_zai_model(settings_manager: State<'_, SettingsManager>) -> String {
    settings_manager.get_ai_zai_model()
}

/// Set DeepSeek API key for AI text correction
#[tauri::command]
pub async fn set_ai_deepseek_api_key(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("API key cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_deepseek_api_key(Some(key))
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Set DeepSeek model for AI text correction
#[tauri::command]
pub async fn set_ai_deepseek_model(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    model: String,
) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err("Model cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_deepseek_model(model)
    })
    .await?;

    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Set DeepSeek use proxy for AI text correction
#[tauri::command]
pub async fn set_ai_deepseek_use_proxy(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_deepseek_use_proxy(enabled)
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Get DeepSeek model for AI text correction
#[tauri::command]
pub fn get_ai_deepseek_model(settings_manager: State<'_, SettingsManager>) -> String {
    settings_manager.get_ai_deepseek_model()
}

/// Set Custom API URL for AI text correction
#[tauri::command]
pub async fn set_ai_custom_url(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    if url.trim().is_empty() {
        return Err("API URL cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_custom_url(Some(url))
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Set Custom API key for AI text correction
#[tauri::command]
pub async fn set_ai_custom_api_key(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("API key cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_custom_api_key(Some(key))
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Set Custom model for AI text correction
#[tauri::command]
pub async fn set_ai_custom_model(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    model: String,
) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err("Model cannot be empty".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_custom_model(model)
    })
    .await?;

    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Set Custom use proxy for AI text correction
#[tauri::command]
pub async fn set_ai_custom_use_proxy(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_ai_custom_use_proxy(enabled)
    })
    .await?;

    state.invalidate_ai_client();
    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Get Custom model for AI text correction
#[tauri::command]
pub fn get_ai_custom_model(settings_manager: State<'_, SettingsManager>) -> String {
    settings_manager.get_ai_custom_model()
}

const MAX_GRAMMAR_TEXT_LEN: usize = 10_000;

const GRAMMAR_PROMPT: &str = "Проверь орфографию и грамматику русского текста. \
    Исправь ошибки, сохранив смысл, стиль и регистр. Верни только исправленный текст \
    без пояснений. Если ошибок нет — верни текст как есть.";

/// Check grammar using AI provider
///
/// Sends the given text (selection or full text) to the configured AI provider
/// for grammar and spelling correction. Uses a dedicated prompt focused on
/// grammar only (vs. the TTS-focused default prompt used by correct_text).
#[tauri::command]
pub async fn ai_check_grammar(
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
    text: String,
) -> Result<String, String> {
    if text.len() > MAX_GRAMMAR_TEXT_LEN {
        return Err(format!(
            "Text too long (max {} chars)",
            MAX_GRAMMAR_TEXT_LEN
        ));
    }
    let settings = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;
    let client = state
        .get_or_create_ai_client(&settings.ai, &settings.tts.network)
        .map_err(|e| format!("AI client error: {}", e))?;
    let result = client
        .correct(&text, GRAMMAR_PROMPT)
        .await
        .map_err(|e| e.to_string())?;
    Ok(result)
}

/// Get current TTS provider type
#[tauri::command]
pub fn get_tts_provider(settings_manager: State<'_, SettingsManager>) -> TtsProviderType {
    settings_manager.get_tts_provider()
}

/// Set TTS provider type
#[tauri::command]
pub async fn set_tts_provider(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    telegram_state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
    provider: TtsProviderType,
) -> Result<(), String> {
    info!(?provider, "Switching to provider");

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_tts_provider(provider)
    })
    .await?;

    match provider {
        TtsProviderType::OpenAi => {
            info!("Initializing OpenAI TTS");
            let api_key = settings_manager.get_openai_api_key();
            if let Some(key) = api_key {
                state.init_openai_tts(key);
                debug!("OpenAI TTS initialized");
            } else {
                warn!("No API key found, OpenAI TTS not initialized");
            }
        }
        TtsProviderType::Silero => {
            info!("Initializing Silero TTS");

            let client_arc = Arc::clone(&telegram_state.client);

            debug!("Checking Telegram session");
            let _connected = match super::telegram::telegram_auto_restore(
                telegram_state,
                settings_manager.clone(),
            )
            .await
            {
                Ok(connected) => {
                    if connected {
                        info!("Telegram session restored");
                    } else {
                        debug!("No saved Telegram session");
                    }
                    connected
                }
                Err(e) => {
                    warn!(error = %e, "Telegram check failed");
                    false
                }
            };

            state.init_silero_tts(client_arc);
            info!("Silero TTS initialized");
        }
        TtsProviderType::Local => {
            info!("Initializing Local TTS");
            let url = settings_manager.get_local_tts_url();
            state.init_local_tts(url);
            debug!("Local TTS initialized");
        }
        TtsProviderType::Fish => {
            info!("Initializing Fish Audio TTS");
            let api_key = settings_manager.get_fish_audio_api_key();
            if let Some(key) = api_key {
                state.init_fish_audio_tts(key);
                debug!("Fish Audio TTS initialized");
            } else {
                warn!("No API key found, Fish Audio TTS not initialized");
            }
        }
    }

    state.set_tts_provider_type(provider);

    super::emit_settings_changed(&app_handle);

    info!(?provider, "Provider set successfully");
    Ok(())
}

/// Get Local TTS URL
#[tauri::command]
pub fn get_local_tts_url(settings_manager: State<'_, SettingsManager>) -> String {
    settings_manager.get_local_tts_url()
}

/// Set Local TTS URL
#[tauri::command]
pub async fn set_local_tts_url(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    url: String,
) -> Result<(), String> {
    info!(safe_url = %secret_log::safe_url_for_log(&url), "Setting Local TTS URL");

    if url.is_empty() {
        return Err("URL не может быть пустым".into());
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL должен начинаться с http:// или https://".into());
    }

    debug!("Saving URL to config...");

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_local_tts_url(url)
    })
    .await?;

    debug!("Updating runtime state");

    let is_local_active = {
        let registry = state.tts_registry.lock();
        registry.active_id() == Some("local-http")
    };

    let url_val = settings_manager.get_local_tts_url();
    if is_local_active {
        info!("Local TTS is active, reinitializing with new URL");
        state.init_local_tts(url_val.clone());
        debug!(safe_url = %secret_log::safe_url_for_log(&url_val), "Local TTS reinitialized");
    } else {
        debug!("Local TTS is not active, skipping reinitialization");
    }

    state.set_local_tts_url(url_val.clone());
    super::emit_settings_changed(&app_handle);

    info!(safe_url = %secret_log::safe_url_for_log(&url_val), "Local TTS URL set successfully");
    Ok(())
}

/// Get OpenAI API key
#[tauri::command]
pub fn get_openai_api_key(settings_manager: State<'_, SettingsManager>) -> Option<String> {
    settings_manager.get_openai_api_key()
}

/// Set OpenAI API key
#[tauri::command]
pub async fn set_openai_api_key(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    key: String,
) -> Result<(), String> {
    if !key.starts_with("sk-") || key.len() < 20 {
        return Err("Неверный формат API ключа OpenAI".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_openai_api_key(Some(key))
    })
    .await?;

    let saved_key = settings_manager.get_openai_api_key();
    if let Some(ref k) = saved_key {
        state.set_openai_api_key(Some(k.clone()));
        state.init_openai_tts(k.clone());
    }
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Get OpenAI voice
#[tauri::command]
pub fn get_openai_voice(settings_manager: State<'_, SettingsManager>) -> String {
    settings_manager.get_openai_voice()
}

/// Set OpenAI voice
#[tauri::command]
pub async fn set_openai_voice(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    voice: String,
) -> Result<(), String> {
    info!(voice, "Setting OpenAI voice");

    const VOICES: &[&str] = &["alloy", "echo", "fable", "onyx", "nova", "shimmer"];
    if !VOICES.contains(&voice.as_str()) {
        warn!(voice, "Invalid voice");
        return Err("Неверный голос".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_openai_voice(voice)
    })
    .await?;

    let saved_voice = settings_manager.get_openai_voice();
    debug!("Updating runtime state and reinitializing OpenAI TTS");
    state.set_openai_voice(saved_voice.clone());
    super::emit_settings_changed(&app_handle);

    info!(voice = %saved_voice, "OpenAI voice set successfully");
    Ok(())
}

/// Apply OpenAI proxy settings from unified config to active provider
#[tauri::command]
pub fn apply_openai_proxy_settings(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    let settings = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let proxy_url = if settings.tts.openai.use_proxy {
        settings.tts.network.proxy.proxy_url.clone()
    } else {
        if let (Some(host), Some(port)) = (
            &settings.tts.openai.proxy_host,
            settings.tts.openai.proxy_port,
        ) {
            if !host.trim().is_empty() {
                Some(format!("http://{}:{}", host.trim(), port))
            } else {
                None
            }
        } else {
            None
        }
    };

    tracing::info!(
        use_proxy = settings.tts.openai.use_proxy,
        has_proxy_url = proxy_url.is_some(),
        "Applying OpenAI proxy settings"
    );

    state.set_openai_proxy(proxy_url);

    Ok(())
}

/// Get Fish Audio API key
#[tauri::command]
pub fn get_fish_audio_api_key(settings_manager: State<'_, SettingsManager>) -> Option<String> {
    settings_manager.get_fish_audio_api_key()
}

/// Set Fish Audio API key
#[tauri::command]
pub async fn set_fish_audio_api_key(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    key: String,
) -> Result<(), String> {
    if key.is_empty() {
        return Err("API Key не может быть пустым".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_fish_audio_api_key(Some(key))
    })
    .await?;

    let saved_key = settings_manager.get_fish_audio_api_key();
    if let Some(ref k) = saved_key {
        state.set_fish_audio_api_key(Some(k.clone()));
        state.init_fish_audio_tts(k.clone());
    }
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Get Fish Audio reference ID (voice model ID)
#[tauri::command]
pub fn get_fish_audio_reference_id(settings_manager: State<'_, SettingsManager>) -> String {
    settings_manager.get_fish_audio_reference_id()
}

/// Set Fish Audio reference ID (voice model ID)
#[tauri::command]
pub async fn set_fish_audio_reference_id(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    reference_id: String,
) -> Result<(), String> {
    if reference_id.trim().is_empty() {
        return Err("Reference ID не может быть пустым".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_fish_audio_reference_id(reference_id)
    })
    .await?;

    let saved_id = settings_manager.get_fish_audio_reference_id();
    state.set_fish_audio_reference_id(saved_id.clone());
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Get Fish Audio saved voice models
#[tauri::command]
pub fn get_fish_audio_voices(settings_manager: State<'_, SettingsManager>) -> Vec<VoiceModel> {
    settings_manager.get_fish_audio_voices()
}

/// Add Fish Audio voice model to saved list
#[tauri::command]
pub async fn add_fish_audio_voice(
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    voice: VoiceModel,
) -> Result<(), String> {
    info!(voice_id = %voice.id, voice_title = %voice.title, "Adding Fish Audio voice model");

    if voice.id.trim().is_empty() {
        error!("Voice ID is empty");
        return Err("Voice ID не может быть пустым".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.add_fish_audio_voice(voice)
    })
    .await?;

    info!("Fish Audio voice added successfully");
    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Remove Fish Audio voice model from saved list
#[tauri::command]
pub async fn remove_fish_audio_voice(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    voice_id: String,
) -> Result<(), String> {
    let reference_id = settings_manager.get_fish_audio_reference_id();
    let was_selected = reference_id == voice_id;

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.remove_fish_audio_voice(&voice_id)
    })
    .await?;

    if was_selected {
        state.set_fish_audio_reference_id(String::new());
    }

    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Fetch Fish Audio models from API
#[tauri::command]
pub async fn fetch_fish_audio_models(
    settings_manager: State<'_, SettingsManager>,
    page_size: Option<u32>,
    page_number: Option<u32>,
    title: Option<String>,
    language: Option<String>,
) -> Result<(i32, Vec<VoiceModel>), String> {
    let api_key = settings_manager
        .get_fish_audio_api_key()
        .ok_or_else(|| "API ключ не установлен".to_string())?;

    let proxy_url = if settings_manager.get_fish_audio_use_proxy() {
        settings_manager
            .get_socks5_proxy_url()
            .filter(|url| !url.is_empty())
    } else {
        None
    };

    let page_size = page_size.unwrap_or(10);
    let page_number = page_number.unwrap_or(1);

    crate::tts::fish::FishTts::list_models(
        &api_key,
        proxy_url.as_deref(),
        page_size,
        page_number,
        title.as_deref(),
        language.as_deref(),
    )
    .await
}

/// Fetch Fish Audio cover image through proxy
#[tauri::command]
pub async fn fetch_fish_audio_image(
    settings_manager: State<'_, SettingsManager>,
    image_url: String,
) -> Result<String, String> {
    let proxy_url = if settings_manager.get_fish_audio_use_proxy() {
        settings_manager
            .get_socks5_proxy_url()
            .filter(|url| !url.is_empty())
    } else {
        None
    };

    crate::tts::fish::FishTts::fetch_image(&image_url, proxy_url.as_deref()).await
}

/// Set Fish Audio format
#[tauri::command]
pub async fn set_fish_audio_format(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    format: String,
) -> Result<(), String> {
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_fish_audio_format(format)
    })
    .await?;

    let saved_format = settings_manager
        .load()
        .map(|s| s.tts.fish.format.clone())
        .unwrap_or_default();
    state.set_fish_audio_format(saved_format);
    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Set Fish Audio temperature
#[tauri::command]
pub async fn set_fish_audio_temperature(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    temperature: f32,
) -> Result<(), String> {
    if !(0.0..=1.0).contains(&temperature) {
        return Err("Temperature must be between 0.0 and 1.0".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_fish_audio_temperature(temperature)
    })
    .await?;

    state.set_fish_audio_temperature(temperature);
    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Set Fish Audio sample rate
#[tauri::command]
pub async fn set_fish_audio_sample_rate(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    sample_rate: u32,
) -> Result<(), String> {
    if sample_rate == 0 {
        return Err("Sample rate cannot be zero".into());
    }

    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_fish_audio_sample_rate(sample_rate)
    })
    .await?;

    state.set_fish_audio_sample_rate(sample_rate);
    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Set Fish Audio use proxy flag
#[tauri::command]
pub async fn set_fish_audio_use_proxy(
    app_handle: AppHandle,
    enabled: bool,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_fish_audio_use_proxy(enabled)
    })
    .await?;

    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Apply Fish Audio proxy settings from unified config to active provider
#[tauri::command]
pub fn apply_fish_audio_proxy_settings(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    let settings = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let proxy_url = if settings.tts.fish.use_proxy {
        settings.tts.network.proxy.proxy_url.clone()
    } else {
        None
    };

    state.set_fish_audio_proxy(proxy_url);
    state.set_fish_audio_format(settings.tts.fish.format);
    state.set_fish_audio_temperature(settings.tts.fish.temperature);
    state.set_fish_audio_sample_rate(settings.tts.fish.sample_rate);

    Ok(())
}

/// Check if API key is set
#[tauri::command]
pub fn has_api_key(settings_manager: State<'_, SettingsManager>) -> bool {
    settings_manager.get_openai_api_key().is_some()
}
