//! AI text correction module
//!
//! Provides AI client implementations for text correction using various providers.

pub mod common;
pub mod openai;
pub mod zai;

use async_trait::async_trait;
use crate::config::{AiSettings, AiProviderType, NetworkSettings};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

// ============================================================================
// AI Error Types
// ============================================================================

/// Errors that can occur during AI text correction
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// API key or credentials not configured
    #[error("AI not configured: {0}")]
    NotConfigured(String),

    /// Failed to build HTTP client
    #[error("Failed to build client: {0}")]
    ClientBuild(String),

    /// Invalid proxy configuration
    #[error("Invalid proxy: {0}")]
    InvalidProxy(String),

    /// Network connection error
    #[error("Network error: {0}")]
    Network(String),

    /// Connection failed
    #[error("Connection failed: {0}")]
    Connection(String),

    /// Request timeout
    #[error("Request timeout: {0}")]
    Timeout(String),

    /// API returned an error
    #[error("API error (status {status}): {message}")]
    ApiError {
        status: u16,
        message: String,
    },

    /// Invalid response from API
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Invalid input parameters
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

// ============================================================================
// AI Client Trait
// ============================================================================

/// Trait for AI text correction clients
///
/// Implementations of this trait provide text correction functionality
/// using different AI providers (OpenAI, Z.ai, etc.).
#[async_trait]
pub trait AiClient: Send + Sync {
    /// Correct text using AI
    ///
    /// # Arguments
    /// * `text` - The text to correct
    /// * `prompt` - The system prompt to guide correction
    ///
    /// # Returns
    /// The corrected text on success
    ///
    /// # Errors
    /// Returns `AiError` if:
    /// - API key is not configured
    /// - Network request fails
    /// - API returns an error
    /// - Response is invalid or empty
    async fn correct(&self, text: &str, prompt: &str) -> Result<String, AiError>;
}

// ============================================================================
// AI Provider Enum
// ============================================================================

/// AI provider wrapper enum
pub enum AiProvider {
    OpenAi(openai::OpenAiClient),
    ZAi(zai::ZAiClient),
}

impl AiProvider {
    /// Correct text using the configured provider
    pub async fn correct(&self, text: &str, prompt: &str) -> Result<String, AiError> {
        match self {
            AiProvider::OpenAi(client) => client.correct(text, prompt).await,
            AiProvider::ZAi(client) => client.correct(text, prompt).await,
        }
    }
}

// ============================================================================
// Factory Functions
// ============================================================================

/// Create AI client from settings
pub fn create_ai_client(
    settings: &AiSettings,
    network_settings: &NetworkSettings,
) -> Result<AiProvider, AiError> {
    match settings.provider {
        AiProviderType::OpenAi => {
            let client = openai::OpenAiClient::new(settings, network_settings)?;
            Ok(AiProvider::OpenAi(client))
        }
        AiProviderType::ZAi => {
            let client = zai::ZAiClient::new(settings, network_settings)?;
            Ok(AiProvider::ZAi(client))
        }
    }
}

// ============================================================================
// Re-exports
// ============================================================================

// OpenAiClient is used through AiProvider enum, no direct export needed

// ============================================================================
// Settings Hash Function
// ============================================================================

/// Compute a hash of AI settings for cache invalidation
///
/// This function combines all settings that affect the AI client's
/// connection configuration. Changes to these values require recreating
/// the client.
///
/// Settings that DON'T require client recreation (not hashed):
/// - `prompt`: Passed per-request
/// - `openai.model`: Passed per-request
/// - `zai.model`: Passed per-request
/// - `timeout`: Passed per-request
pub fn hash_ai_settings(settings: &AiSettings) -> u64 {
    let mut hasher = DefaultHasher::new();

    // Hash provider type (use discriminant for enum)
    std::mem::discriminant(&settings.provider).hash(&mut hasher);

    // Hash OpenAI settings
    if let Some(key) = &settings.openai.api_key {
        key.hash(&mut hasher);
    }
    settings.openai.use_proxy.hash(&mut hasher);

    // Hash Z.ai settings
    if let Some(url) = &settings.zai.url {
        url.hash(&mut hasher);
    }
    if let Some(key) = &settings.zai.api_key {
        key.hash(&mut hasher);
    }

    hasher.finish()
}
