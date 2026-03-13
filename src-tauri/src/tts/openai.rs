use reqwest::Client;
use serde::Serialize;
use crate::tts::engine::TtsEngine;
use crate::events::EventSender;
use crate::config::DEFAULT_TTS_TIMEOUT_SECS;
use async_trait::async_trait;
use std::time::{Duration, Instant};

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
            eprintln!("[OpenAI] Using proxy: {}:{}", host, port);
            let proxy_url = format!("http://{}:{}", host, port);
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| format!("Failed to create proxy: {}", e))?;

            Client::builder()
                .proxy(proxy)
                .timeout(timeout)
                .build()
                .map_err(|e| format!("Failed to build client with proxy: {}", e))
        } else {
            eprintln!("[OpenAI] Direct connection (no proxy)");
            Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| format!("Failed to build client: {}", e))
        }
    }
}

#[async_trait]
impl TtsEngine for OpenAiTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        let start_time = Instant::now();
        let client = self.build_client()?;

        // Логирование деталей запроса
        eprintln!("[OpenAI] ========================================");
        eprintln!("[OpenAI] TTS Request Started");
        eprintln!("[OpenAI] Model: tts-1");
        eprintln!("[OpenAI] Voice: {}", self.voice);
        eprintln!("[OpenAI] Text length: {} chars", text.len());
        eprintln!("[OpenAI] Text preview: \"{}\"", text.chars().take(50).collect::<String>());
        eprintln!("[OpenAI] API Key: {}...{}", &self.api_key[..7.min(self.api_key.len())], &self.api_key[self.api_key.len().saturating_sub(4)..]);
        eprintln!("[OpenAI] Timeout: {}s", self.timeout_secs);

        let request = TtsRequest {
            model: "tts-1".to_string(),
            input: text.to_string(),
            voice: self.voice.clone(),
        };

        // Логируем тело запроса (без аллокации при успехе)
        match serde_json::to_string(&request) {
            Ok(body) => eprintln!("[OpenAI] Request body: {}", body),
            Err(_) => eprintln!("[OpenAI] Request body: (unable to serialize)"),
        }

        let response = client
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                let elapsed = start_time.elapsed();
                eprintln!("[OpenAI] Request failed after {:.2}s", elapsed.as_secs_f64());
                if e.is_timeout() {
                    format!("OpenAI timeout ({}s). Check internet or proxy settings.", self.timeout_secs)
                } else if e.is_connect() {
                    format!("OpenAI connection failed: {}", e)
                } else {
                    format!("Failed to send TTS request: {}", e)
                }
            })?;

        // Логирование ответа
        let status = response.status();
        eprintln!("[OpenAI] Response Status: {} {}", status.as_u16(), status.canonical_reason().unwrap_or_default());

        // Логирование заголовков ответа
        eprintln!("[OpenAI] Response Headers:");
        for (name, value) in response.headers().iter() {
            if let Ok(value_str) = value.to_str() {
                eprintln!("[OpenAI]   {}: {}", name, value_str);
            }
        }

        let elapsed = start_time.elapsed();
        eprintln!("[OpenAI] Response time: {:.2}s", elapsed.as_secs_f64());

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            eprintln!("[OpenAI] Error Response: {}", error_text);
            return Err(format!("TTS request failed ({}): {}", status.as_u16(), error_text));
        }

        let audio_data = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read audio data: {}", e))?
            .to_vec();

        eprintln!("[OpenAI] Audio data received: {} bytes", audio_data.len());
        eprintln!("[OpenAI] Total time: {:.2}s", elapsed.as_secs_f64());
        eprintln!("[OpenAI] ========================================");

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
