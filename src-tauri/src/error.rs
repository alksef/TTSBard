//! Unified error handling for the application
//!
//! Provides a centralized error type using thiserror for consistent
//! error handling across all modules.

#![allow(dead_code)]

use thiserror::Error;

/// Unified application error type
#[derive(Debug, Error)]
pub enum AppError {
    /// IO errors from file operations
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Network-related errors
    #[error("Network error: {0}")]
    Network(String),

    /// TTS synthesis failures
    #[error("TTS synthesis failed: {0}")]
    TtsFailed(String),

    /// Audio playback errors
    #[error("Audio playback error: {0}")]
    Audio(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Serialization errors (JSON, etc.)
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Telegram client errors
    #[error("Telegram error: {0}")]
    Telegram(String),

    /// HTTP request errors
    #[error("HTTP request error: {0}")]
    Http(String),

    /// Generic errors with context
    #[error("{0}")]
    Other(String),
}

/// Type alias for Result with AppError
pub type Result<T> = std::result::Result<T, AppError>;

/// Trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| AppError::Other(format!("{}: {}", f(), e)))
    }
}

/// Extension trait for converting Option to Result
pub trait OptionExt<T> {
    /// Convert Option to Result with error context
    fn ok_or_else_msg<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_else_msg<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| AppError::Other(f()))
    }
}
