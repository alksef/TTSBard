//! Custom OpenAI-compatible AI client for text correction
//!
//! This module implements a client for any OpenAI-compatible API.
//! Uses async-openai crate with a user-provided base URL.

use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use backoff::ExponentialBackoff;
use reqwest::Client as ReqwestClient;
use std::time::Duration;
use tracing::{error, info};

use super::common as ai_common;
use super::{AiClient, AiError};
use crate::config::{AiSettings, NetworkSettings};
use crate::secret_log;

// ============================================================================
// CustomClient
// ============================================================================

/// Custom (OpenAI-compatible) client for text correction
pub struct CustomClient {
    client: Client<OpenAIConfig>,
    model: String,
    timeout: u64,
}

impl CustomClient {
    /// Create a new Custom client from settings
    ///
    /// # Errors
    ///
    /// Returns `AiError::NotConfigured` if API URL or API key is not set.
    pub fn new(settings: &AiSettings, network_settings: &NetworkSettings) -> Result<Self, AiError> {
        let base_url = settings
            .custom
            .url
            .as_ref()
            .filter(|url| !url.trim().is_empty())
            .ok_or_else(|| AiError::NotConfigured("Custom API URL not set".to_string()))?
            .clone();

        let api_key = settings
            .custom
            .api_key
            .as_ref()
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| AiError::NotConfigured("Custom API key not set".to_string()))?
            .clone();

        let model = settings.custom.model.clone();
        let timeout = settings.timeout;

        // Create HTTP client with proxy if needed
        let http_client = if settings.custom.use_proxy {
            let proxy_url = network_settings
                .proxy
                .proxy_url
                .as_ref()
                .ok_or_else(|| AiError::InvalidProxy("Proxy enabled but URL not set".to_string()))?
                .clone();

            info!(
                model = &model,
                has_proxy = true,
                safe_url = %secret_log::safe_url_for_log(&proxy_url),
                "CustomClient created with proxy"
            );

            let (scheme, _rest) = proxy_url.split_once("://").ok_or_else(|| {
                AiError::InvalidProxy("Invalid proxy URL: missing scheme".to_string())
            })?;

            let scheme_lower = scheme.to_lowercase();
            if !matches!(
                scheme_lower.as_str(),
                "socks5" | "socks5h" | "socks4" | "socks4a" | "http" | "https"
            ) {
                return Err(AiError::InvalidProxy(format!(
                    "Unsupported proxy URL scheme: {}",
                    scheme
                )));
            }

            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| {
                    error!(error = %e, safe_url = %secret_log::safe_url_for_log(&proxy_url), "Failed to create proxy");
                    AiError::InvalidProxy(format!("Failed to create {} proxy: {}", scheme, e))
                })?;

            ReqwestClient::builder()
                .proxy(proxy)
                .timeout(Duration::from_secs(timeout))
                .build()
                .map_err(|e| {
                    error!(error = %e, "Failed to build client with proxy");
                    AiError::InvalidProxy(format!("Failed to build client with proxy: {}", e))
                })?
        } else {
            info!(
                model = &model,
                safe_url = %secret_log::safe_url_for_log(&base_url),
                "CustomClient created (direct connection)"
            );

            ReqwestClient::builder()
                .timeout(Duration::from_secs(timeout))
                .build()
                .map_err(|e| {
                    error!(error = %e, "Failed to build HTTP client");
                    AiError::ClientBuild(format!("Failed to build HTTP client: {}", e))
                })?
        };

        // Build async-openai client with custom base URL
        let backoff = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(0)),
            ..Default::default()
        };
        let client = Client::build(
            http_client,
            OpenAIConfig::new()
                .with_api_key(api_key)
                .with_api_base(base_url),
            backoff,
        );

        Ok(Self {
            client,
            model,
            timeout,
        })
    }

    /// Send correction request to the custom API
    async fn send_request(&self, text: &str, prompt: &str) -> Result<String, AiError> {
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages([
                ChatCompletionRequestSystemMessage::from(prompt).into(),
                ChatCompletionRequestUserMessage::from(text).into(),
            ])
            .temperature(ai_common::DEFAULT_TEMPERATURE)
            .max_tokens(ai_common::DEFAULT_MAX_TOKENS)
            .build()
            .map_err(|e| AiError::InvalidInput(format!("Failed to build request: {}", e)))?;

        info!(
            model = &self.model,
            text_length = text.len(),
            prompt_length = prompt.len(),
            timeout_secs = self.timeout,
            "Sending Custom correction request"
        );

        let response = self.client.chat().create(request).await.map_err(|e| {
            let error_msg = e.to_string();
            error!(
                error_type = "custom_api_error",
                "Custom API request failed"
            );

            if error_msg.contains("timeout") || error_msg.contains("timed out") {
                error!(timeout_secs = self.timeout, "Custom request timeout");
                AiError::Timeout(format!(
                    "Custom timeout ({}s). Check internet or proxy settings.",
                    self.timeout
                ))
            } else if error_msg.contains("connect") || error_msg.contains("connection") {
                error!("Custom connection failed");
                AiError::Connection(format!("Custom connection failed: {}", e))
            } else if error_msg.contains("401") || error_msg.contains("unauthorized") {
                error!("Custom authentication failed (401)");
                AiError::NotConfigured("Custom API key is invalid or missing".to_string())
            } else if error_msg.contains("429") {
                error!("Custom rate limit exceeded (429)");
                AiError::ApiError {
                    status: 429,
                    message: "Rate limit exceeded. Please try again later.".to_string(),
                }
            } else {
                AiError::Network(format!("Failed to send request: {}", e))
            }
        })?;

        info!(
            choices_count = response.choices.len(),
            model = ?response.model,
            "Custom response received"
        );

        let content = ai_common::extract_response_content(&response, "Custom")?;
        ai_common::log_response_preview(&content, "Custom");

        Ok(content)
    }
}

#[async_trait::async_trait]
impl AiClient for CustomClient {
    async fn correct(&self, text: &str, prompt: &str) -> Result<String, AiError> {
        ai_common::validate_correction_input(text, prompt)?;

        let corrected = self.send_request(text, prompt).await?;

        ai_common::validate_correction_result(&corrected, text, "Custom")?;

        Ok(corrected)
    }
}
