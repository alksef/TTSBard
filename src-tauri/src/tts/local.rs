use crate::tts::engine::TtsEngine;
use crate::events::{AppEvent, EventSender};
use async_trait::async_trait;
use reqwest::Client;
use std::time::{Duration, Instant};

/// Local TTS implementation using TTSVoiceWizard Locally Hosted API
/// Compatible with TITTS.py and similar local TTS servers
#[derive(Clone, Debug)]
pub struct LocalTts {
    server_url: String,
    configured: bool,
    event_tx: Option<EventSender>,
    timeout_secs: u64,
}

impl LocalTts {
    pub fn new() -> Self {
        Self {
            server_url: "http://127.0.0.1:8124".to_string(),
            configured: true, // Local TTS is always available
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
    #[allow(dead_code)]
    pub fn get_url(&self) -> &str {
        &self.server_url
    }

    /// Set the timeout for HTTP requests in seconds
    #[allow(dead_code)]
    pub fn set_timeout(&mut self, timeout_secs: u64) {
        self.timeout_secs = timeout_secs;
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

impl Default for LocalTts {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TtsEngine for LocalTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        // Send event before synthesizing
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(AppEvent::TextSentToTts(text.to_string()));
        }

        let start_time = Instant::now();
        let client = self.build_client()?;

        eprintln!("[LocalTTS] ========================================");
        eprintln!("[LocalTTS] TTS Request Started");
        eprintln!("[LocalTTS] Server: {}", self.server_url);
        eprintln!("[LocalTTS] Text length: {} chars", text.len());
        eprintln!("[LocalTTS] Text preview: \"{}\"", text.chars().take(50).collect::<String>());
        eprintln!("[LocalTTS] Timeout: {}s", self.timeout_secs);

        // URL encode the text for the path parameter
        let encoded_text = urlencoding::encode(text);
        let url = format!("{}/synthesize/{}", self.server_url.trim_end_matches('/'), encoded_text);

        eprintln!("[LocalTTS] Request URL: {}", url);

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                let elapsed = start_time.elapsed();
                eprintln!("[LocalTTS] Request failed after {:.2}s", elapsed.as_secs_f64());
                if e.is_timeout() {
                    format!("Local TTS timeout ({}s). Server at {} may be slow or unavailable.", self.timeout_secs, self.server_url)
                } else if e.is_connect() {
                    format!("Local TTS connection failed to {}. Check if the TTS server is running.", self.server_url)
                } else {
                    format!("Failed to send TTS request to {}: {}", self.server_url, e)
                }
            })?;

        // Log response status
        let status = response.status();
        eprintln!("[LocalTTS] Response Status: {} {}", status.as_u16(), status.canonical_reason().unwrap_or_default());

        let elapsed = start_time.elapsed();
        eprintln!("[LocalTTS] Response time: {:.2}s", elapsed.as_secs_f64());

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            eprintln!("[LocalTTS] Error Response: {}", error_text);
            return Err(format!("TTS request failed ({}): {}", status.as_u16(), error_text));
        }

        // Get response as text (base64 encoded WAV data)
        let base64_data = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response text: {}", e))?;

        eprintln!("[LocalTTS] Base64 data received: {} chars", base64_data.len());

        // Decode base64 to bytes
        let audio_data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &base64_data)
            .map_err(|e| {
                eprintln!("[LocalTTS] Base64 decode failed: {}", e);
                format!("Failed to decode base64 audio data: {}", e)
            })?;

        eprintln!("[LocalTTS] Audio data decoded: {} bytes", audio_data.len());
        eprintln!("[LocalTTS] Total time: {:.2}s", elapsed.as_secs_f64());
        eprintln!("[LocalTTS] ========================================");

        Ok(audio_data)
    }

    fn is_configured(&self) -> bool {
        self.configured
    }

    fn name(&self) -> &str {
        "Local"
    }
}
