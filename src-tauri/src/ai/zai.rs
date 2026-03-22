//! Z.ai (OpenAI-compatible) AI client for text correction
//!
//! This module implements a client for Z.ai's OpenAI-compatible API.
//! Uses async-openai crate with custom base URL.

use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        CreateChatCompletionRequestArgs,
        ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage,
    },
};
use reqwest::Client as ReqwestClient;
use std::time::Duration;
use tracing::{error, info};

use crate::config::{AiSettings, NetworkSettings};
use super::{AiClient, AiError};
use super::common as ai_common;

// ============================================================================
// ZAiClient
// ============================================================================

/// Z.ai (OpenAI-compatible) client for text correction
pub struct ZAiClient {
    client: Client<OpenAIConfig>,
    model: String,
    timeout: u64,
}

impl ZAiClient {
    /// Create a new Z.ai client from settings
    ///
    /// # Errors
    ///
    /// Returns `AiError::NotConfigured` if URL or API key is not set.
    pub fn new(settings: &AiSettings, _network_settings: &NetworkSettings) -> Result<Self, AiError> {
        let api_key = settings.zai.api_key
            .as_ref()
            .ok_or_else(|| AiError::NotConfigured("Z.ai API key not set".to_string()))?
            .clone();

        let url = settings.zai.url
            .as_ref()
            .ok_or_else(|| AiError::NotConfigured("Z.ai URL not set".to_string()))?
            .clone();

        let model = settings.zai.model.clone();
        let timeout = settings.timeout;

        // Log the endpoint that will be used
        let expected_endpoint = if url.ends_with('/') {
            format!("{}chat/completions", url)
        } else {
            format!("{}/chat/completions", url)
        };

        info!(
            model = &model,
            base_url = &url,
            expected_endpoint = &expected_endpoint,
            api_key_prefix = &api_key[..api_key.len().min(7)],
            timeout_secs = timeout,
            "ZAiClient created with OpenAI-compatible API"
        );

        // Create HTTP client with timeout
        let http_client = ReqwestClient::builder()
            .timeout(Duration::from_secs(timeout))
            .build()
            .map_err(|e| {
                error!(error = %e, "Failed to build HTTP client");
                AiError::ClientBuild(format!("Failed to build HTTP client: {}", e))
            })?;

        // Configure async-openai client with custom base URL for Z.ai
        let client = Client::with_config(
            OpenAIConfig::new()
                .with_api_key(api_key)
                .with_api_base(&url)
        )
        .with_http_client(http_client);

        Ok(Self {
            client,
            model,
            timeout,
        })
    }

    /// Send correction request to Z.ai API
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
            "Sending Z.ai correction request"
        );

        let response = self.client
            .chat()
            .create(request)
            .await
            .map_err(|e| {
                let error_msg = e.to_string();
                error!(
                    error = %error_msg,
                    error_type = "async_openai_error",
                    "Z.ai API request failed"
                );

                // Provide helpful hints based on error content
                if error_msg.contains("missing field `id`") {
                    error!(
                        "This error usually means the endpoint returned a non-OpenAI format response. \
                         Check that your Z.ai URL is correct: https://api.z.ai/api/paas/v4 \
                         (without /chat/completions at the end)"
                    );
                    AiError::InvalidResponse(format!(
                        "Z.ai returned non-OpenAI format response. Check your Z.ai URL setting. \
                         Expected base URL: https://api.z.ai/api/paas/v4. Error: {}", e
                    ))
                } else if error_msg.contains("timeout") || error_msg.contains("timed out") {
                    error!(timeout_secs = self.timeout, "Z.ai request timeout");
                    AiError::Timeout(format!("Z.ai timeout ({}s). Check internet or proxy settings.", self.timeout))
                } else if error_msg.contains("connect") || error_msg.contains("connection") {
                    error!("Z.ai connection failed");
                    AiError::Connection(format!("Z.ai connection failed: {}", e))
                } else if error_msg.contains("404") || error_msg.contains("NOT_FOUND") {
                    error!("Z.ai endpoint not found (404)");
                    AiError::InvalidResponse(format!(
                        "Z.ai endpoint not found (404). Check your Z.ai URL setting. \
                         Expected base URL: https://api.z.ai/api/paas/v4. Error: {}", e
                    ))
                } else if error_msg.contains("401") || error_msg.contains("unauthorized") {
                    error!("Z.ai authentication failed (401)");
                    AiError::NotConfigured("Z.ai API key is invalid or missing".to_string())
                } else if error_msg.contains("429") {
                    error!("Z.ai rate limit or balance issue (429)");
                    AiError::ApiError {
                        status: 429,
                        message: "Insufficient balance, no resource package, or rate limit exceeded. Please recharge your Z.ai account.".to_string(),
                    }
                } else {
                    AiError::Network(format!("Failed to send request: {}", e))
                }
            })?;

        // Log response details
        info!(
            choices_count = response.choices.len(),
            model = ?response.model,
            "Z.ai response received"
        );

        let content = ai_common::extract_response_content(&response, "Z.ai")?;
        ai_common::log_response_preview(&content, "Z.ai");

        Ok(content)
    }
}

#[async_trait::async_trait]
impl AiClient for ZAiClient {
    async fn correct(&self, text: &str, prompt: &str) -> Result<String, AiError> {
        ai_common::validate_correction_input(text, prompt)?;

        let corrected = self.send_request(text, prompt).await?;

        ai_common::validate_correction_result(&corrected, text, "Z.ai")?;

        Ok(corrected)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_expected_endpoint_formatting() {
        // Test that base URL formatting works correctly
        let url_with_slash = "https://api.z.ai/api/paas/v4/";
        let expected_with_slash = if url_with_slash.ends_with('/') {
            format!("{}chat/completions", url_with_slash)
        } else {
            format!("{}/chat/completions", url_with_slash)
        };
        assert_eq!(expected_with_slash, "https://api.z.ai/api/paas/v4/chat/completions");

        let url_without_slash = "https://api.z.ai/api/paas/v4";
        let expected_without_slash = if url_without_slash.ends_with('/') {
            format!("{}chat/completions", url_without_slash)
        } else {
            format!("{}/chat/completions", url_without_slash)
        };
        assert_eq!(expected_without_slash, "https://api.z.ai/api/paas/v4/chat/completions");
    }
}
