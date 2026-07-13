//! DeepSeek (OpenAI-compatible) AI client for text correction
//!
//! This module implements a client for DeepSeek's OpenAI-compatible API.
//! Uses async-openai crate with fixed base URL.

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

const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/v1";

// ============================================================================
// DeepSeekClient
// ============================================================================

/// DeepSeek (OpenAI-compatible) client for text correction
pub struct DeepSeekClient {
    client: Client<OpenAIConfig>,
    model: String,
    timeout: u64,
}

impl DeepSeekClient {
    /// Create a new DeepSeek client from settings
    ///
    /// # Errors
    ///
    /// Returns `AiError::NotConfigured` if API key is not set.
    pub fn new(settings: &AiSettings, network_settings: &NetworkSettings) -> Result<Self, AiError> {
        let api_key = settings
            .deepseek
            .api_key
            .as_ref()
            .ok_or_else(|| AiError::NotConfigured("DeepSeek API key not set".to_string()))?
            .clone();

        let model = settings.deepseek.model.clone();
        let timeout = settings.timeout;

        // Create HTTP client with proxy if needed
        let http_client = if settings.deepseek.use_proxy {
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
                "DeepSeekClient created with proxy"
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
                base_url = DEEPSEEK_BASE_URL,
                "DeepSeekClient created (direct connection)"
            );

            ReqwestClient::builder()
                .timeout(Duration::from_secs(timeout))
                .build()
                .map_err(|e| {
                    error!(error = %e, "Failed to build HTTP client");
                    AiError::ClientBuild(format!("Failed to build HTTP client: {}", e))
                })?
        };

        // Build async-openai client with DeepSeek base URL
        let backoff = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(0)),
            ..Default::default()
        };
        let client = Client::build(
            http_client,
            OpenAIConfig::new()
                .with_api_key(api_key)
                .with_api_base(DEEPSEEK_BASE_URL),
            backoff,
        );

        Ok(Self {
            client,
            model,
            timeout,
        })
    }

    /// Send correction request to DeepSeek API
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
            "Sending DeepSeek correction request"
        );

        let response = self.client.chat().create(request).await.map_err(|e| {
            let error_msg = e.to_string();
            error!(
                error_type = "deepseek_api_error",
                "DeepSeek API request failed"
            );

            if error_msg.contains("timeout") || error_msg.contains("timed out") {
                error!(timeout_secs = self.timeout, "DeepSeek request timeout");
                AiError::Timeout(format!(
                    "DeepSeek timeout ({}s). Check internet or proxy settings.",
                    self.timeout
                ))
            } else if error_msg.contains("connect") || error_msg.contains("connection") {
                error!("DeepSeek connection failed");
                AiError::Connection(format!("DeepSeek connection failed: {}", e))
            } else if error_msg.contains("401") || error_msg.contains("unauthorized") {
                error!("DeepSeek authentication failed (401)");
                AiError::NotConfigured("DeepSeek API key is invalid or missing".to_string())
            } else if error_msg.contains("402") || error_msg.contains("Insufficient Balance") {
                error!("DeepSeek insufficient balance (402)");
                AiError::ApiError {
                    status: 402,
                    message: "Insufficient balance. Please top up your DeepSeek account."
                        .to_string(),
                }
            } else if error_msg.contains("429") {
                error!("DeepSeek rate limit exceeded (429)");
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
            "DeepSeek response received"
        );

        let content = ai_common::extract_response_content(&response, "DeepSeek")?;
        ai_common::log_response_preview(&content, "DeepSeek");

        Ok(content)
    }
}

#[async_trait::async_trait]
impl AiClient for DeepSeekClient {
    async fn correct(&self, text: &str, prompt: &str) -> Result<String, AiError> {
        ai_common::validate_correction_input(text, prompt)?;

        let corrected = self.send_request(text, prompt).await?;

        ai_common::validate_correction_result(&corrected, text, "DeepSeek")?;

        Ok(corrected)
    }
}
