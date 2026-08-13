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

/// Exhaustive production declaration: every `submit_speech` error code and its retryability.
#[derive(Debug, Clone, Serialize)]
pub struct SpeechErrorDef {
    pub code: &'static str,
    pub retryable: bool,
}

pub const SPEECH_SUBMIT_ERRORS: &[SpeechErrorDef] = &[
    SpeechErrorDef {
        code: speech::error_code::SNAPSHOT_UNAVAILABLE,
        retryable: false,
    },
    SpeechErrorDef {
        code: speech::error_code::EMPTY_TEXT,
        retryable: false,
    },
    SpeechErrorDef {
        code: speech::error_code::QUEUE_FULL,
        retryable: true,
    },
    SpeechErrorDef {
        code: speech::error_code::QUEUE_REJECTED,
        retryable: false,
    },
];

pub fn speech_error_code_to_retryable(code: &str) -> bool {
    SPEECH_SUBMIT_ERRORS
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("unknown submit_speech error code: {code}"))
        .retryable
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
#[derive(Serialize)]
struct SpeechErrorFixture {
    codes: &'static [SpeechErrorDef],
    envelope: CommandError,
}

#[cfg(test)]
mod tests {
    use super::{speech::error_code, CommandError, SpeechErrorFixture, SPEECH_SUBMIT_ERRORS};

    fn speech_error_fixture() -> SpeechErrorFixture {
        SpeechErrorFixture {
            codes: SPEECH_SUBMIT_ERRORS,
            envelope: CommandError::new(
                error_code::QUEUE_FULL,
                "queue full",
                super::speech_error_code_to_retryable(error_code::QUEUE_FULL),
            ),
        }
    }

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

    #[test]
    fn speech_contract_error_defs_are_exhaustive_and_unique() {
        let codes: Vec<&str> = SPEECH_SUBMIT_ERRORS.iter().map(|d| d.code).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(codes.len(), sorted.len(), "duplicate error codes");
        assert_eq!(
            codes.len(),
            4,
            "expected exactly 4 submit_speech error codes"
        );
    }

    #[test]
    fn speech_contract_error_fixture_is_current() {
        let fixture = speech_error_fixture();
        let current = serde_json::to_value(&fixture).expect("serialize");
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/contract-fixtures/speech/speech-errors.json"
        );
        let on_disk_raw = std::fs::read_to_string(fixture_path).unwrap_or_else(|_| {
            panic!(
                "Fixture 'speech-errors.json' missing — regenerate with \
                 `cargo test speech_contract_fixtures_regenerate -- --ignored`"
            )
        });
        let on_disk =
            serde_json::from_str::<serde_json::Value>(&on_disk_raw).unwrap_or_else(|err| {
                panic!(
                    "Fixture 'speech-errors.json' is not valid JSON: {err} — regenerate with \
                 `cargo test speech_contract_fixtures_regenerate -- --ignored`"
                )
            });
        assert_eq!(
            current, on_disk,
            "Fixture 'speech-errors.json' is stale — regenerate with \
             `cargo test speech_contract_fixtures_regenerate -- --ignored`"
        );
    }

    #[test]
    fn speech_contract_retryability_is_consistent_with_constants() {
        let def = |code: &str| -> &super::SpeechErrorDef {
            SPEECH_SUBMIT_ERRORS
                .iter()
                .find(|d| d.code == code)
                .unwrap()
        };

        assert!(!def(error_code::SNAPSHOT_UNAVAILABLE).retryable);
        assert!(!def(error_code::EMPTY_TEXT).retryable);
        assert!(def(error_code::QUEUE_FULL).retryable);
        assert!(!def(error_code::QUEUE_REJECTED).retryable);
    }

    #[test]
    fn speech_contract_lookup_finds_all_known_codes() {
        for def in SPEECH_SUBMIT_ERRORS {
            let found = super::speech_error_code_to_retryable(def.code);
            assert_eq!(found, def.retryable);
        }
    }

    #[test]
    #[should_panic(expected = "unknown submit_speech error code")]
    fn speech_contract_lookup_rejects_unknown_code() {
        super::speech_error_code_to_retryable("nonexistent.code");
    }

    /// Regenerates all speech contract fixture files. Excluded from standard test runs.
    #[test]
    #[ignore]
    fn speech_contract_fixtures_regenerate() {
        let fixture_dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/contract-fixtures/speech"
        );

        std::fs::create_dir_all(fixture_dir).expect("create fixture dir");

        let errors_fixture = speech_error_fixture();
        let errors_json =
            serde_json::to_string_pretty(&errors_fixture).expect("serialize speech errors");
        let errors_path = format!("{}/speech-errors.json", fixture_dir);
        std::fs::write(&errors_path, &errors_json).expect("write speech-errors fixture");
        eprintln!("Fixture written: {}", errors_path);
    }
}
