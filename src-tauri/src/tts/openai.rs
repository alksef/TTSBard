use reqwest::Client;
use serde::Serialize;
use crate::tts::engine::TtsEngine;
use crate::events::EventSender;
use crate::config::DEFAULT_TTS_TIMEOUT_SECS;
use async_trait::async_trait;
use std::time::{Duration, Instant};
use tracing::{debug, info, error};

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
    proxy_host: Option<String>,
    proxy_port: Option<u16>,
    timeout_secs: u64,
    event_tx: Option<EventSender>,
}

impl OpenAiTts {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            voice: "alloy".to_string(),
            proxy_host: None,
            proxy_port: None,
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

    pub fn set_proxy(&mut self, host: Option<String>, port: Option<u16>) {
        self.proxy_host = host;
        self.proxy_port = port;
    }

    #[allow(dead_code)]
    pub fn get_proxy_host(&self) -> Option<&str> {
        self.proxy_host.as_deref()
    }

    #[allow(dead_code)]
    pub fn get_proxy_port(&self) -> Option<u16> {
        self.proxy_port
    }

    fn build_client(&self) -> Result<Client, String> {
        let timeout = Duration::from_secs(self.timeout_secs);

        if let (Some(host), Some(port)) = (&self.proxy_host, self.proxy_port) {
            info!(proxy_host = host, proxy_port = port, "Using proxy");
            let proxy_url = format!("http://{}:{}", host, port);
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| {
                    error!(error = %e, "Failed to create proxy");
                    format!("Failed to create proxy: {}", e)
                })?;

            Client::builder()
                .proxy(proxy)
                .timeout(timeout)
                .build()
                .map_err(|e| {
                    error!(error = %e, "Failed to build client with proxy");
                    format!("Failed to build client with proxy: {}", e)
                })
        } else {
            info!("Direct connection (no proxy)");
            Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| {
                    error!(error = %e, "Failed to build client");
                    format!("Failed to build client: {}", e)
                })
        }
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
            api_key_prefix = &self.api_key[..7.min(self.api_key.len())],
            api_key_suffix = &self.api_key[self.api_key.len().saturating_sub(4)..],
            timeout_secs = self.timeout_secs,
            "TTS request started"
        );

        let request = TtsRequest {
            model: "tts-1".to_string(),
            input: text.to_string(),
            voice: self.voice.clone(),
        };

        match serde_json::to_string(&request) {
            Ok(body) => debug!(request_body = &body, "Request body serialized"),
            Err(_) => debug!("Request body: (unable to serialize)"),
        }

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
                    format!("OpenAI timeout ({}s). Check internet or proxy settings.", self.timeout_secs)
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

        debug!("Response headers:");
        for (name, value) in response.headers().iter() {
            if let Ok(value_str) = value.to_str() {
                debug!(header_name = %name, header_value = value_str);
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
            return Err(format!("TTS request failed ({}): {}", status.as_u16(), error_text));
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

    #[allow(dead_code)]
    fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && self.api_key.starts_with("sk-")
    }

    #[allow(dead_code)]
    fn name(&self) -> &str {
        "OpenAI"
    }
}
