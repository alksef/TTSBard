use serde::Serialize;
use std::fmt;

/// Stable IPC names for the speech submission vertical slice.
pub mod speech {
    pub const SUBMIT_COMMAND: &str = "submit_speech";
    pub const QUEUE_CHANGED_EVENT: &str = "speech-queue-changed";

    pub mod error_code {
        pub const SNAPSHOT_UNAVAILABLE: &str = "speech.snapshot_unavailable";
        pub const EMPTY_TEXT: &str = "speech.empty_text";
        pub const QUEUE_FULL: &str = "speech.queue_full";
        pub const QUEUE_REJECTED: &str = "speech.queue_rejected";
    }
}

/// Stable error envelope serialized by Tauri command rejections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

#[cfg(test)]
mod tests {
    use super::CommandError;

    #[test]
    fn command_error_has_stable_wire_shape() {
        let error = CommandError::new("speech.queue_full", "queue full", true);

        assert_eq!(
            serde_json::to_value(error).unwrap(),
            serde_json::json!({
                "code": "speech.queue_full",
                "message": "queue full",
                "retryable": true
            })
        );
    }
}
