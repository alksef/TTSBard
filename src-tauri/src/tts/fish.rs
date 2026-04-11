use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::tts::engine::TtsEngine;
use crate::events::EventSender;
use crate::config::DEFAULT_TTS_TIMEOUT_SECS;
use async_trait::async_trait;
use std::time::Duration;
use tracing::{info, error, debug};
use base64::Engine;

/// Модель голоса из Fish Audio API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceModel {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_image: Option<String>,
    pub languages: Vec<String>,
    pub author_nickname: Option<String>,
}

/// Ответ API со списком моделей
#[derive(Debug, Serialize, Deserialize)]
struct ListModelsResponse {
    total: i32,
    items: Vec<ModelEntity>,
}

/// Сущность модели из API Fish Audio
#[derive(Debug, Serialize, Deserialize)]
struct ModelEntity {
    #[serde(rename = "_id")]
    id: String,
    title: String,
    description: Option<String>,
    cover_image: Option<String>,
    languages: Vec<String>,
    author: Option<Author>,
    #[serde(default)]
    like_count: i32,
    #[serde(default)]
    state: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Author {
    nickname: Option<String>,
}

impl From<ModelEntity> for VoiceModel {
    fn from(entity: ModelEntity) -> Self {
        Self {
            id: entity.id,
            title: entity.title,
            description: entity.description,
            cover_image: entity.cover_image,
            languages: entity.languages,
            author_nickname: entity.author.and_then(|a| a.nickname),
        }
    }
}

#[derive(Debug, Serialize)]
struct FishTtsRequest {
    text: String,
    #[serde(rename = "reference_id")]
    reference_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_rate: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct FishTts {
    api_key: String,
    reference_id: String,
    proxy_url: Option<String>,
    timeout_secs: u64,
    event_tx: Option<EventSender>,
    format: String,
    temperature: f32,
    sample_rate: u32,
}

impl FishTts {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            reference_id: String::new(),
            proxy_url: None,
            timeout_secs: DEFAULT_TTS_TIMEOUT_SECS,
            event_tx: None,
            format: "mp3".to_string(),
            temperature: 0.7,
            sample_rate: 44100,
        }
    }

    pub fn with_event_tx(mut self, event_tx: EventSender) -> Self {
        self.event_tx = Some(event_tx);
        self
    }

    pub fn set_reference_id(&mut self, reference_id: String) {
        self.reference_id = reference_id;
    }

    pub fn set_proxy(&mut self, proxy_url: Option<String>) {
        self.proxy_url = proxy_url;
    }

    pub fn set_format(&mut self, format: String) {
        self.format = format;
    }

    pub fn set_temperature(&mut self, temperature: f32) {
        self.temperature = temperature;
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
    }

    /// Загрузить изображение через прокси (возвращает base64 data URL)
    pub async fn fetch_image(
        image_url: &str,
        proxy_url: Option<&str>,
    ) -> Result<String, String> {
        debug!(image_url, "Fetching Fish Audio image");

        let timeout = Duration::from_secs(30);

        let client = if let Some(proxy_url) = proxy_url {
            debug!(proxy_url, "Building HTTP client with proxy");
            let proxy = Self::parse_proxy_url(proxy_url)?;
            Client::builder()
                .proxy(proxy)
                .timeout(timeout)
                .build()
                .map_err(|e| {
                    error!(error = %e, "Failed to build HTTP client with proxy");
                    format!("Failed to build client with proxy '{}': {}", proxy_url, e)
                })?
        } else {
            debug!("Building HTTP client without proxy");
            Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| {
                    error!(error = %e, "Failed to build HTTP client");
                    format!("Failed to build HTTP client: {}", e)
                })?
        };

        let response = client
            .get(image_url)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    format!("Image request timed out after {}s", timeout.as_secs())
                } else if e.is_connect() {
                    format!("Failed to connect to image server: {}", e)
                } else {
                    format!("Failed to fetch image: {}", e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Failed to fetch image ({}): {}", status.as_u16(), error_text));
        }

        // Determine content type from URL or response
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/jpeg").to_string();

        let image_bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read image data: {}", e))?
            .to_vec();

        debug!(size = image_bytes.len(), content_type, "Image fetched successfully");

        // Encode to base64
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&image_bytes);
        Ok(format!("data:{};base64,{}", content_type, base64_data))
    }

    /// Получить список моделей из Fish Audio API
    pub async fn list_models(
        api_key: &str,
        proxy_url: Option<&str>,
        page_size: u32,
        page_number: u32,
        title: Option<&str>,
        language: Option<&str>,
    ) -> Result<(i32, Vec<VoiceModel>), String> {
        let timeout = Duration::from_secs(30);

        let client = if let Some(proxy_url) = proxy_url {
            let proxy = Self::parse_proxy_url(proxy_url)?;
            Client::builder()
                .proxy(proxy)
                .timeout(timeout)
                .build()
                .map_err(|e| format!("Failed to build client with proxy: {}", e))?
        } else {
            Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| format!("Failed to build client: {}", e))?
        };

        let mut request = client
            .get("https://api.fish.audio/model")
            .header("Authorization", format!("Bearer {}", api_key))
            .query(&[("page_size", page_size), ("page_number", page_number)]);

        if let Some(title) = title {
            request = request.query(&[("title", title)]);
        }

        if let Some(language) = language {
            request = request.query(&[("language", language)]);
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("Failed to fetch models: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Failed to list models ({}): {}", status, error_text));
        }

        let models_response: ListModelsResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let models: Vec<VoiceModel> = models_response.items.into_iter().map(|m| m.into()).collect();

        Ok((models_response.total, models))
    }

    fn parse_proxy_url(url: &str) -> Result<reqwest::Proxy, String> {
        let (scheme, _rest) = url.split_once("://")
            .ok_or_else(|| "Invalid proxy URL: missing scheme".to_string())?;

        let scheme_lower = scheme.to_lowercase();
        if !matches!(scheme_lower.as_str(), "socks5" | "socks5h" | "socks4" | "socks4a" | "http" | "https") {
            return Err(format!("Unsupported proxy URL scheme: {}", scheme));
        }

        reqwest::Proxy::all(url)
            .map_err(|e| format!("Failed to create {} proxy: {}", scheme, e))
    }

    fn build_client(&self) -> Result<Client, String> {
        let timeout = Duration::from_secs(self.timeout_secs);

        if let Some(proxy_url) = &self.proxy_url {
            let proxy = Self::parse_proxy_url(proxy_url)?;
            info!(proxy_url = %proxy_url, "Using proxy");
            Client::builder()
                .proxy(proxy)
                .timeout(timeout)
                .build()
                .map_err(|e| format!("Failed to build client with proxy: {}", e))
        } else {
            info!("Direct connection (no proxy)");
            Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| format!("Failed to build client: {}", e))
        }
    }
}

#[async_trait]
impl TtsEngine for FishTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        let client = self.build_client()?;

        if self.reference_id.is_empty() {
            return Err("Fish Audio reference_id (voice model) is not set. Please add a voice model in settings.".to_string());
        }

        let request = FishTtsRequest {
            text: text.to_string(),
            reference_id: self.reference_id.clone(),
            temperature: Some(self.temperature),
            format: Some(self.format.clone()),
            sample_rate: Some(self.sample_rate),
        };

        let response = client
            .post("https://api.fish.audio/v1/tts")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("model", "s2-pro")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    format!("Fish Audio timeout ({}s)", self.timeout_secs)
                } else if e.is_connect() {
                    format!("Fish Audio connection failed: {}", e)
                } else {
                    format!("Failed to send TTS request: {}", e)
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("TTS request failed ({}): {}", status.as_u16(), error_text));
        }

        let audio_data = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read audio data: {}", e))?
            .to_vec();

        Ok(audio_data)
    }
}
