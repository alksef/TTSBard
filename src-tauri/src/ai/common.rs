//! Common functionality for AI clients

use async_openai::types::chat::CreateChatCompletionResponse;
use tracing::error;
use super::AiError;

// ============================================================================
// Constants
// ============================================================================

/// Default temperature for chat completion
pub const DEFAULT_TEMPERATURE: f32 = 0.7;

/// Default max tokens for chat completion
pub const DEFAULT_MAX_TOKENS: u32 = 4096;

// ============================================================================
// Validation Functions
// ============================================================================

/// Validate input parameters for text correction
pub fn validate_correction_input(text: &str, prompt: &str) -> Result<(), AiError> {
    if text.trim().is_empty() {
        return Err(AiError::InvalidInput("Text cannot be empty".to_string()));
    }
    if prompt.trim().is_empty() {
        return Err(AiError::InvalidInput("Prompt cannot be empty".to_string()));
    }
    Ok(())
}

/// Validate correction result and log success
pub fn validate_correction_result(
    corrected: &str,
    original: &str,
    provider_name: &str,
) -> Result<(), AiError> {
    if corrected.trim().is_empty() {
        error!("{} returned empty corrected text", provider_name);
        return Err(AiError::InvalidResponse("AI returned empty corrected text".to_string()));
    }

    tracing::info!(
        original_length = original.len(),
        corrected_length = corrected.len(),
        "{} correction applied successfully",
        provider_name
    );

    Ok(())
}

// ============================================================================
// Response Extraction
// ============================================================================

/// Extract content from chat completion response
pub fn extract_response_content(
    response: &CreateChatCompletionResponse,
    provider_name: &str,
) -> Result<String, AiError> {
    response
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .ok_or_else(|| {
            error!("{} response missing choices or content", provider_name);
            AiError::InvalidResponse("Response missing choices or content".to_string())
        })
        .map(|s| s.to_string())
}

// ============================================================================
// Logging
// ============================================================================

/// Log response preview
pub fn log_response_preview(content: &str, provider_name: &str) {
    tracing::info!(
        content_length = content.len(),
        content_preview = &content[..content.len().min(200)],
        "{} correction completed",
        provider_name
    );
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_correction_input_valid() {
        assert!(validate_correction_input("test text", "test prompt").is_ok());
    }

    #[test]
    fn test_validate_correction_input_empty_text() {
        let result = validate_correction_input("", "test prompt");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Invalid input: Text cannot be empty");
    }

    #[test]
    fn test_validate_correction_input_empty_prompt() {
        let result = validate_correction_input("test text", "");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Invalid input: Prompt cannot be empty");
    }

    #[test]
    fn test_validate_correction_result_empty() {
        let result = validate_correction_result("", "original", "TestProvider");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Invalid response: AI returned empty corrected text");
    }

    #[test]
    fn test_validate_correction_result_valid() {
        assert!(validate_correction_result("corrected", "original", "TestProvider").is_ok());
    }

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_TEMPERATURE, 0.7);
        assert_eq!(DEFAULT_MAX_TOKENS, 4096);
    }
}
