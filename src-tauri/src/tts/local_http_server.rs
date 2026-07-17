use crate::events::EventSender;
use crate::secret_log;
use crate::tts::engine::TtsEngine;
use async_trait::async_trait;
use reqwest::Client;
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// HTTP-server-based TTS using TTSVoiceWizard Locally Hosted API
/// Compatible with TITTS.py and similar local HTTP TTS servers
#[derive(Clone, Debug)]
pub struct LocalHttpServerTts {
    server_url: String,
    event_tx: Option<EventSender>,
    timeout_secs: u64,
}

impl LocalHttpServerTts {
    pub fn new() -> Self {
        Self {
            server_url: "http://127.0.0.1:8124".to_string(),
            event_tx: None,
            timeout_secs: 30,
        }
    }

    pub fn with_event_tx(mut self, event_tx: EventSender) -> Self {
        self.event_tx = Some(event_tx);
        self
    }

    /// Set the server URL for TTS requests
    /// Example: "http://127.0.0.1:8124" or "http://localhost:8124"
    pub fn set_url(&mut self, url: String) {
        self.server_url = url;
    }

    /// Get the current server URL
    pub fn get_url(&self) -> &str {
        &self.server_url
    }

    /// Build an HTTP client with configured timeout
    fn build_client(&self) -> Result<Client, String> {
        let timeout = Duration::from_secs(self.timeout_secs);

        Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))
    }
}

impl Default for LocalHttpServerTts {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TtsEngine for LocalHttpServerTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        let start_time = Instant::now();
        let client = self.build_client()?;

        info!(
            server_url = %self.server_url,
            text_length = text.len(),
            text_preview = %text.chars().take(50).collect::<String>(),
            timeout_secs = self.timeout_secs,
            "LocalHttpServerTTS request started"
        );

        // URL encode the text for the path parameter
        let encoded_text = urlencoding::encode(text);
        let url = format!(
            "{}/synthesize/{}",
            self.server_url.trim_end_matches('/'),
            encoded_text
        );

        debug!(request_url = %secret_log::safe_url_for_log(&url), "Sending LocalHttpServerTTS request");

        let response = client.get(&url).send().await.map_err(|e| {
            let elapsed = start_time.elapsed();
            error!(
                error = %e,
                elapsed_secs = elapsed.as_secs_f64(),
                timeout_secs = self.timeout_secs,
                server_url = %self.server_url,
                "LocalHttpServerTTS request failed"
            );
            if e.is_timeout() {
                format!(
                    "Local HTTP TTS timeout ({}s). Server at {} may be slow or unavailable.",
                    self.timeout_secs, self.server_url
                )
            } else if e.is_connect() {
                format!(
                    "Local HTTP TTS connection failed to {}. Check if the TTS server is running.",
                    self.server_url
                )
            } else {
                format!(
                    "Failed to send HTTP TTS request to {}: {}",
                    self.server_url, e
                )
            }
        })?;

        // Log response status
        let status = response.status();
        let elapsed = start_time.elapsed();

        debug!(
            status_code = status.as_u16(),
            status_reason = %status.canonical_reason().unwrap_or_default(),
            response_time_secs = elapsed.as_secs_f64(),
            "LocalHttpServerTTS response received"
        );

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!(
                status_code = status.as_u16(),
                error_text = %error_text,
                "LocalHttpServerTTS request failed"
            );
            return Err(format!(
                "TTS request failed ({}): {}",
                status.as_u16(),
                error_text
            ));
        }

        // Get response as text (base64 encoded WAV data)
        let base64_data = response.text().await.map_err(|e| {
            error!(error = %e, "Failed to read LocalHttpServerTTS response text");
            format!("Failed to read response text: {}", e)
        })?;

        debug!(base64_length = base64_data.len(), "Base64 data received");

        // Decode base64 to bytes
        let audio_data =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &base64_data)
                .map_err(|e| {
                    error!(error = %e, "Base64 decode failed");
                    format!("Failed to decode base64 audio data: {}", e)
                })?;

        info!(
            audio_bytes = audio_data.len(),
            total_time_secs = elapsed.as_secs_f64(),
            "LocalHttpServerTTS synthesis completed"
        );

        Ok(audio_data)
    }
}
