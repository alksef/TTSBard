//! OpenAI chat completions client for AI text correction
//!
//! Implements the AiClient trait using async-openai library with proxy support.

use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        CreateChatCompletionRequestArgs,
        ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage,
    },
};
use backoff::ExponentialBackoff;
use reqwest::Client as ReqwestClient;
use std::time::Duration;
use tracing::{error, info};

use crate::config::{AiSettings, NetworkSettings};
use super::{AiClient, AiError};
use super::common as ai_common;

// ============================================================================
// OpenAI Client
// ============================================================================

/// OpenAI chat completions client using async-openai
pub struct OpenAiClient {
    client: Client<OpenAIConfig>,
    model: String,
    timeout: u64,
}

impl OpenAiClient {
    /// Create a new OpenAI client from settings
    pub fn new(settings: &AiSettings, network_settings: &NetworkSettings) -> Result<Self, AiError> {
        let api_key = settings.openai.api_key
            .as_ref()
            .ok_or_else(|| AiError::NotConfigured("OpenAI API key not set".to_string()))?
            .clone();

        let model = settings.openai.model.clone();
        let timeout = settings.timeout;

        // Create HTTP client with proxy if needed
        let http_client = if settings.openai.use_proxy {
            let proxy_url = network_settings.proxy.proxy_url
                .as_ref()
                .ok_or_else(|| AiError::InvalidProxy("Proxy enabled but URL not set".to_string()))?
                .clone();

            info!(
                model = &model,
                proxy_url = %proxy_url,
                "OpenAiClient created with proxy"
            );

            // Parse proxy URL to determine type
            let (scheme, _rest) = proxy_url.split_once("://")
                .ok_or_else(|| AiError::InvalidProxy("Invalid proxy URL: missing scheme".to_string()))?;

            let scheme_lower = scheme.to_lowercase();
            if !matches!(scheme_lower.as_str(), "socks5" | "socks5h" | "socks4" | "socks4a" | "http" | "https") {
                return Err(AiError::InvalidProxy(format!("Unsupported proxy URL scheme: {}", scheme)));
            }

            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| {
                    error!(error = %e, proxy_url = %proxy_url, "Failed to create proxy");
                    AiError::InvalidProxy(format!("Failed to create {} proxy: {}", scheme, e))
                })?;

            ReqwestClient::builder()
                .proxy(proxy)
                .timeout(Duration::from_secs(settings.timeout))
                .build()
                .map_err(|e| {
                    error!(error = %e, "Failed to build client with proxy");
                    AiError::InvalidProxy(format!("Failed to build client with proxy: {}", e))
                })?
        } else {
            info!(
                model = &model,
                "OpenAiClient created (direct connection)"
            );

            ReqwestClient::builder()
                .timeout(Duration::from_secs(settings.timeout))
                .build()
                .map_err(|e| {
                    error!(error = %e, "Failed to build client");
                    AiError::ClientBuild(format!("Failed to build client: {}", e))
                })?
        };

        // Build async-openai client with custom HTTP client
        // Disable internal retries (single attempt only)
        let backoff = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(0)),
            ..Default::default()
        };
        let client = Client::build(
            http_client,
            OpenAIConfig::new().with_api_key(api_key),
            backoff,
        );

        Ok(Self { client, model, timeout })
    }

    /// Send chat completion request to OpenAI API
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
            "Sending OpenAI correction request"
        );

        let response = self.client
            .chat()
            .create(request)
            .await
            .map_err(|e| {
                let error_msg = e.to_string();
                error!(
                    error = %error_msg,
                    error_type = "openai_api_error",
                    "OpenAI API request failed"
                );

                // Parse error message for better error handling
                if error_msg.contains("timeout") || error_msg.contains("timed out") {
                    error!(timeout_secs = self.timeout, "OpenAI request timeout");
                    AiError::Timeout(format!("OpenAI timeout ({}s). Check internet or proxy settings.", self.timeout))
                } else if error_msg.contains("connect") || error_msg.contains("connection") {
                    error!("OpenAI connection failed");
                    AiError::Connection(format!("OpenAI connection failed: {}", e))
                } else if error_msg.contains("401") || error_msg.contains("unauthorized") {
                    error!("OpenAI authentication failed (401)");
                    AiError::NotConfigured("OpenAI API key is invalid or missing".to_string())
                } else if error_msg.contains("429") {
                    error!("OpenAI rate limit or quota exceeded (429)");
                    AiError::ApiError {
                        status: 429,
                        message: "Rate limit exceeded or quota exceeded. Please check your OpenAI account.".to_string(),
                    }
                } else {
                    AiError::Network(format!("Failed to send request: {}", e))
                }
            })?;

        info!(
            choices_count = response.choices.len(),
            "OpenAI response received"
        );

        let content = ai_common::extract_response_content(&response, "OpenAI")?;
        ai_common::log_response_preview(&content, "OpenAI");

        Ok(content)
    }
}

#[async_trait::async_trait]
impl AiClient for OpenAiClient {
    async fn correct(&self, text: &str, prompt: &str) -> Result<String, AiError> {
        ai_common::validate_correction_input(text, prompt)?;

        let corrected = self.send_request(text, prompt).await?;

        ai_common::validate_correction_result(&corrected, text, "OpenAI")?;

        Ok(corrected)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_client_has_model_field() {
        // This test verifies that OpenAiClient has a model field
        // The actual value is set at runtime from settings
        assert_eq!(std::mem::size_of::<OpenAiClient>(), std::mem::size_of::<Client<OpenAIConfig>>() + std::mem::size_of::<String>());
    }
}
