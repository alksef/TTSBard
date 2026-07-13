use crate::config::DEFAULT_TTS_TIMEOUT_SECS;
use crate::events::EventSender;
use crate::tts::engine::TtsEngine;
use crate::tts::proxy_utils;
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use std::time::{Duration, Instant};
use tracing::{debug, error};

#[derive(Debug, Serialize)]
struct TtsRequest {
    model: String,
    input: String,
    voice: String,
}

#[derive(Clone, Debug)]
pub struct OpenAiTts {
    api_key: String,
    voice: String,
    /// Unified proxy URL (socks5://, socks4://, http://user:pass@host:port)
    proxy_url: Option<String>,
    timeout_secs: u64,
    event_tx: Option<EventSender>,
}

impl OpenAiTts {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            voice: "alloy".to_string(),
            proxy_url: None,
            timeout_secs: DEFAULT_TTS_TIMEOUT_SECS,
            event_tx: None,
        }
    }

    pub fn with_event_tx(mut self, event_tx: EventSender) -> Self {
        self.event_tx = Some(event_tx);
        self
    }

    pub fn set_voice(&mut self, voice: String) {
        self.voice = voice;
    }

    /// Set unified proxy URL (socks5://, socks4://, http://user:pass@host:port)
    pub fn set_proxy(&mut self, proxy_url: Option<String>) {
        self.proxy_url = proxy_url;
    }

    pub fn get_proxy_url(&self) -> Option<&str> {
        self.proxy_url.as_deref()
    }

    fn build_client(&self) -> Result<Client, String> {
        let timeout = Duration::from_secs(self.timeout_secs);
        proxy_utils::build_client_with_proxy(self.proxy_url.as_deref(), timeout)
    }
}

#[async_trait]
impl TtsEngine for OpenAiTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        let start_time = Instant::now();
        let client = self.build_client()?;

        debug!(
            model = "tts-1",
            voice = &self.voice,
            text_length = text.len(),
            text_preview = &text.chars().take(50).collect::<String>(),
            has_api_key = true,
            timeout_secs = self.timeout_secs,
            "TTS request started"
        );

        let request = TtsRequest {
            model: "tts-1".to_string(),
            input: text.to_string(),
            voice: self.voice.clone(),
        };

        debug!(
            request_json_len = request.input.len(),
            "Request body prepared"
        );

        let response = client
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                let elapsed = start_time.elapsed();
                if e.is_timeout() {
                    error!(
                        error = %e,
                        elapsed_secs = elapsed.as_secs_f64(),
                        timeout_secs = self.timeout_secs,
                        "Request timeout"
                    );
                    format!(
                        "OpenAI timeout ({}s). Check internet or proxy settings.",
                        self.timeout_secs
                    )
                } else if e.is_connect() {
                    error!(
                        error = %e,
                        elapsed_secs = elapsed.as_secs_f64(),
                        "Connection failed"
                    );
                    format!("OpenAI connection failed: {}", e)
                } else {
                    error!(
                        error = %e,
                        elapsed_secs = elapsed.as_secs_f64(),
                        "Failed to send TTS request"
                    );
                    format!("Failed to send TTS request: {}", e)
                }
            })?;

        let status = response.status();
        debug!(
            status_code = status.as_u16(),
            status_reason = status.canonical_reason().unwrap_or_default(),
            "Response status received"
        );

        const ALLOWLISTED: &[&str] = &["content-type", "content-length", "server"];
        const SENSITIVE: &[&str] = &[
            "authorization",
            "proxy-authorization",
            "proxy-authenticate",
            "www-authenticate",
            "cookie",
            "set-cookie",
        ];

        debug!("Response headers:");
        for (name, value) in response.headers().iter() {
            let lower = name.as_str().to_lowercase();
            if SENSITIVE.iter().any(|h| lower.eq_ignore_ascii_case(h)) {
                continue;
            }
            if ALLOWLISTED.iter().any(|h| lower.eq_ignore_ascii_case(h)) {
                if let Ok(s) = value.to_str() {
                    debug!(header_name = %name, header_value = s);
                } else {
                    debug!(header_name = %name, header_value = "[non-utf8]");
                }
            } else {
                debug!(header_name = %name, header_value = "[omitted]");
            }
        }

        let elapsed = start_time.elapsed();
        debug!(response_time_secs = elapsed.as_secs_f64(), "Response time");

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!(
                status_code = status.as_u16(),
                error_text = &error_text,
                "TTS request failed"
            );
            return Err(format!(
                "TTS request failed ({}): {}",
                status.as_u16(),
                error_text
            ));
        }

        let audio_data = response
            .bytes()
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to read audio data");
                format!("Failed to read audio data: {}", e)
            })?
            .to_vec();

        debug!(
            audio_data_bytes = audio_data.len(),
            total_time_secs = elapsed.as_secs_f64(),
            "Audio data received successfully"
        );

        Ok(audio_data)
    }
}
