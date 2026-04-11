# План реализации: TTS провайдер Fish Audio

## Обзор

Добавление Fish Audio (https://fish.audio) как нового TTS провайдера с поддержкой SOCKS5 прокси и управлением голосовыми моделями через API.

**Ключевые решения:**
- Название провайдера: **Fish Audio** (вариант enum: `Fish`, label в UI: "Fish Audio")
- Управление голосами: Выбор из API Fish Audio через модальное окно с поиском
- Параметры API: Базовые настройки (reference_id, temperature, format, sample_rate)
- Прокси: Поддержка SOCKS5 через существующую конфигурацию `reqwest`
- Хранение моделей: Список выбранных моделей с метаданными в конфиге провайдера

## Fish Audio API

### TTS Эндпоинт
**URL:** `POST https://api.fish.audio/v1/tts`

**Заголовки:**
- `Authorization: Bearer <token>` (обязательный)
- `Content-Type: application/json` (обязательный)
- `model: s2-pro` (обязательный, альтернатива: `s1`)

**Тело запроса:**
```json
{
  "text": "Текст для синтеза",
  "reference_id": "id-голосовой-модели",
  "temperature": 0.7,
  "top_p": 0.7,
  "format": "mp3",
  "sample_rate": 44100
}
```

**Ответ:** Raw аудио данные (mp3 по умолчанию)

### Список моделей Эндпоинт
**URL:** `GET https://api.fish.audio/model`

**Заголовки:**
- `Authorization: Bearer <token>` (обязательный)

**Query параметры:**
- `page_size` (default: 10) - количество моделей на странице
- `page_number` (default: 1) - номер страницы
- `title` - фильтр по названию модели
- `language` - фильтр по языку (например, "ru", "en")
- `self` (default: false) - только модели пользователя

**Ответ:**
```json
{
  "total": 123,
  "items": [
    {
      "_id": "model-id-string",
      "type": "svc",
      "title": "Название модели",
      "description": "Описание модели",
      "cover_image": "https://...",
      "languages": ["en", "ru"],
      "author": {
        "_id": "author-id",
        "nickname": "Автор",
        "avatar": "https://..."
      },
      "like_count": 123,
      "state": "created"
    }
  ]
}
```

---

## Фаза 1: Реализация бэкенда (Rust)

### 1.1 Создание модуля Fish Audio TTS

**Файл:** `src-tauri/src/tts/fish.rs` (НОВЫЙ)

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::tts::engine::TtsEngine;
use crate::events::EventSender;
use crate::config::DEFAULT_TTS_TIMEOUT_SECS;
use async_trait::async_trait;
use std::time::{Duration, Instant};
use tracing::{debug, info, error};

/// Модель голоса из Fish Audio API
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl FishTts {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            reference_id: String::new(),
            proxy_url: None,
            timeout_secs: DEFAULT_TTS_TIMEOUT_SECS,
            event_tx: None,
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
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Failed to list models ({}): {}", response.status(), error_text));
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
        let start_time = Instant::now();
        let client = self.build_client()?;

        if self.reference_id.is_empty() {
            return Err("Fish Audio reference_id (voice model) is not set. Please add a voice model in settings.".to_string());
        }

        let request = FishTtsRequest {
            text: text.to_string(),
            reference_id: self.reference_id.clone(),
            temperature: Some(0.7),
            format: Some("mp3".to_string()),
            sample_rate: Some(44100),
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

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && !self.reference_id.is_empty()
    }

    fn name(&self) -> &str {
        "FishAudio"
    }
}
```

### 1.2 Обновление модуля TTS

**Файл:** `src-tauri/src/tts/mod.rs`

Добавить экспорт модуля и обновить enum-ы:

```rust
pub mod engine;
pub mod fish;      // НОВЫЙ
pub mod local;
pub mod openai;
pub mod silero;

// Реэкспорт VoiceModel для использования в других модулях
pub use fish::VoiceModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TtsProviderType {
    #[default]
    OpenAi,
    Silero,
    Local,
    Fish,  // НОВЫЙ
}

#[derive(Clone, Debug)]
pub enum TtsProvider {
    OpenAi(openai::OpenAiTts),
    Silero(silero::SileroTts),
    Local(local::LocalTts),
    Fish(fish::FishTts),  // НОВЫЙ
}

impl TtsProvider {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        match self {
            TtsProvider::OpenAi(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
            TtsProvider::Local(tts) => tts.synthesize(text).await,
            TtsProvider::Silero(tts) => tts.synthesize(text).await,
            TtsProvider::Fish(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
        }
    }

    #[allow(dead_code)]
    pub fn is_configured(&self) -> bool {
        match self {
            TtsProvider::OpenAi(tts) => tts.is_configured(),
            TtsProvider::Local(tts) => tts.is_configured(),
            TtsProvider::Silero(tts) => tts.is_configured(),
            TtsProvider::Fish(tts) => tts.is_configured(),
        }
    }
}
```

### 1.3 Обновление структуры настроек

**Файл:** `src-tauri/src/config/settings.rs`

Добавить структуру `FishAudioSettings` с хранением моделей:

```rust
// Используем VoiceModel из tts модуля
use crate::tts::VoiceModel;

/// Настройки TTS провайдера
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TtsSettings {
    #[serde(default)]
    pub provider: TtsProviderType,
    pub openai: OpenAiSettings,
    pub local: LocalTtsSettings,
    pub fish: FishAudioSettings,  // НОВЫЙ
    pub telegram: TelegramTtsSettings,
    #[serde(default)]
    pub network: NetworkSettings,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            provider: TtsProviderType::OpenAi,
            openai: OpenAiSettings::default(),
            local: LocalTtsSettings::default(),
            fish: FishAudioSettings::default(),  // НОВЫЙ
            telegram: TelegramTtsSettings::default(),
            network: NetworkSettings::default(),
        }
    }
}

/// Настройки Fish Audio TTS
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FishAudioSettings {
    pub api_key: Option<String>,
    /// Список сохранённых голосовых моделей с метаданными
    #[serde(default)]
    pub voices: Vec<VoiceModel>,
    /// Текущий выбранный ID голосовой модели
    #[serde(default)]
    pub reference_id: String,
    /// Формат аудио (mp3, wav, pcm, opus)
    #[serde(default = "default_fish_format")]
    pub format: String,
    /// Температура (0.0-1.0)
    #[serde(default = "default_fish_temperature")]
    pub temperature: f32,
    /// Частота дискретизации (Гц)
    #[serde(default = "default_fish_sample_rate")]
    pub sample_rate: u32,
    /// Использовать унифицированный прокси из глобальных настроек
    #[serde(default)]
    pub use_proxy: bool,
}

fn default_fish_format() -> String { "mp3".to_string() }
fn default_fish_temperature() -> f32 { 0.7 }
fn default_fish_sample_rate() -> u32 { 44100 }

impl Default for FishAudioSettings {
    fn default() -> Self {
        Self {
            api_key: None,
            voices: Vec::new(),
            reference_id: String::new(),
            format: "mp3".to_string(),
            temperature: 0.7,
            sample_rate: 44100,
            use_proxy: false,
        }
    }
}
```

Добавить методы getter/setter в `SettingsManager`:

```rust
// ========== Настройки Fish Audio ==========

pub fn set_fish_audio_api_key(&self, api_key: Option<String>) -> Result<()> {
    self.update_field("/tts/fish/api_key", &api_key)
}

pub fn get_fish_audio_api_key(&self) -> Option<String> {
    self.cache.read().tts.fish.api_key.clone()
}

pub fn set_fish_audio_reference_id(&self, reference_id: String) -> Result<()> {
    self.update_field("/tts/fish/reference_id", &reference_id)
}

pub fn get_fish_audio_reference_id(&self) -> String {
    self.cache.read().tts.fish.reference_id.clone()
}

pub fn add_fish_audio_voice(&self, voice: VoiceModel) -> Result<()> {
    let mut settings = self.load()?;
    if !settings.tts.fish.voices.iter().any(|v| v.id == voice.id) {
        settings.tts.fish.voices.push(voice);
        self.save(&settings)?;
    }
    Ok(())
}

pub fn remove_fish_audio_voice(&self, voice_id: &str) -> Result<()> {
    let mut settings = self.load()?;
    settings.tts.fish.voices.retain(|v| v.id != voice_id);
    self.save(&settings)?;
    Ok(())
}

pub fn get_fish_audio_voices(&self) -> Vec<VoiceModel> {
    self.cache.read().tts.fish.voices.clone()
}

pub fn set_fish_audio_format(&self, format: String) -> Result<()> {
    self.update_field("/tts/fish/format", &format)
}

pub fn get_fish_audio_format(&self) -> String {
    self.cache.read().tts.fish.format.clone()
}

pub fn set_fish_audio_temperature(&self, temperature: f32) -> Result<()> {
    self.update_field("/tts/fish/temperature", &temperature)
}

pub fn get_fish_audio_temperature(&self) -> f32 {
    self.cache.read().tts.fish.temperature
}

pub fn set_fish_audio_sample_rate(&self, sample_rate: u32) -> Result<()> {
    self.update_field("/tts/fish/sample_rate", &sample_rate)
}

pub fn get_fish_audio_sample_rate(&self) -> u32 {
    self.cache.read().tts.fish.sample_rate
}

pub fn set_fish_audio_use_proxy(&self, enabled: bool) -> Result<()> {
    self.update_field("/tts/fish/use_proxy", &enabled)
}

pub fn get_fish_audio_use_proxy(&self) -> bool {
    self.cache.read().tts.fish.use_proxy
}
```

### 1.4 Обновление DTO

**Файл:** `src-tauri/src/config/dto.rs`

Добавить DTO для VoiceModel и обновить FishAudioSettingsDto:

```rust
use crate::tts::VoiceModel;

/// DTO голосовой модели Fish Audio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceModelDto {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_image: Option<String>,
    pub languages: Vec<String>,
    pub author_nickname: Option<String>,
}

impl From<VoiceModel> for VoiceModelDto {
    fn from(v: VoiceModel) -> Self {
        Self {
            id: v.id,
            title: v.title,
            description: v.description,
            cover_image: v.cover_image,
            languages: v.languages,
            author_nickname: v.author_nickname,
        }
    }
}

impl From<VoiceModelDto> for VoiceModel {
    fn from(dto: VoiceModelDto) -> Self {
        Self {
            id: dto.id,
            title: dto.title,
            description: dto.description,
            cover_image: dto.cover_image,
            languages: dto.languages,
            author_nickname: dto.author_nickname,
        }
    }
}

/// DTO настроек Fish Audio TTS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FishAudioSettingsDto {
    pub api_key: Option<String>,
    #[serde(default)]
    pub voices: Vec<VoiceModelDto>,
    #[serde(default)]
    pub reference_id: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub sample_rate: u32,
    #[serde(default)]
    pub use_proxy: bool,
}

impl From<crate::config::settings::FishAudioSettings> for FishAudioSettingsDto {
    fn from(s: crate::config::settings::FishAudioSettings) -> Self {
        Self {
            api_key: s.api_key,
            voices: s.voices.into_iter().map(|v| v.into()).collect(),
            reference_id: s.reference_id,
            format: s.format,
            temperature: s.temperature,
            sample_rate: s.sample_rate,
            use_proxy: s.use_proxy,
        }
    }
}

impl From<FishAudioSettingsDto> for crate::config::settings::FishAudioSettings {
    fn from(dto: FishAudioSettingsDto) -> Self {
        Self {
            api_key: dto.api_key,
            voices: dto.voices.into_iter().map(|v| v.into()).collect(),
            reference_id: dto.reference_id,
            format: dto.format,
            temperature: dto.temperature,
            sample_rate: dto.sample_rate,
            use_proxy: dto.use_proxy,
        }
    }
}
```

### 1.5 Обновление управления состоянием

**Файл:** `src-tauri/src/state.rs`

Обновить структуру `TtsConfig` и добавить методы для Fish Audio:

```rust
#[derive(Clone, Debug)]
pub struct TtsConfig {
    pub provider_type: TtsProviderType,
    pub openai_key: Option<String>,
    pub openai_voice: String,
    pub openai_proxy_url: Option<String>,
    pub fish_api_key: Option<String>,
    pub fish_reference_id: String,
    pub fish_proxy_url: Option<String>,
    pub local_url: String,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            provider_type: TtsProviderType::OpenAi,
            openai_key: None,
            openai_voice: "alloy".to_string(),
            openai_proxy_url: None,
            fish_api_key: None,
            fish_reference_id: String::new(),
            fish_proxy_url: None,
            local_url: "http://127.0.0.1:8124".to_string(),
        }
    }
}
```

Добавить методы Fish Audio в `AppState`:

```rust
pub fn init_fish_audio_tts(&self, api_key: String) {
    let mut tts = crate::tts::fish::FishTts::new(api_key);
    let config = self.tts_config.read();
    tts.set_reference_id(config.fish_reference_id.clone());
    if let Some(proxy_url) = &config.fish_proxy_url {
        tts.set_proxy(Some(proxy_url.clone()));
    }
    drop(config);

    if let Some(event_tx) = self.get_event_sender() {
        tts = tts.with_event_tx(event_tx);
    }

    *self.tts_providers.lock() = Some(TtsProvider::Fish(tts));
}

pub fn get_fish_audio_api_key(&self) -> Option<String> {
    self.tts_config.read().fish_api_key.clone()
}

pub fn set_fish_audio_api_key(&self, key: Option<String>) {
    self.tts_config.write().fish_api_key = key;
}

pub fn set_fish_audio_reference_id(&self, reference_id: String) {
    // Реинициализация провайдера если активен
    let (api_key, proxy_url, event_tx, current_provider) = {
        let config = self.tts_config.read();
        let event_tx = self.get_event_sender();
        let provider = self.tts_providers.lock().clone();
        (
            config.fish_api_key.clone(),
            config.fish_proxy_url.clone(),
            event_tx,
            provider
        )
    };

    if let Some(key) = api_key {
        let mut tts = crate::tts::fish::FishTts::new(key);
        tts.set_reference_id(reference_id.clone());
        if let Some(url) = proxy_url {
            tts.set_proxy(Some(url));
        }

        if let Some(tx) = event_tx {
            tts = tts.with_event_tx(tx);
        }

        if matches!(current_provider.as_ref(), Some(TtsProvider::Fish(_))) {
            *self.tts_providers.lock() = Some(TtsProvider::Fish(tts));
        }
    }

    self.tts_config.write().fish_reference_id = reference_id;
}

pub fn set_fish_audio_proxy(&self, proxy_url: Option<String>) {
    let (api_key, reference_id, event_tx, current_provider) = {
        let config = self.tts_config.read();
        let event_tx = self.get_event_sender();
        let provider = self.tts_providers.lock().clone();
        (
            config.fish_api_key.clone(),
            config.fish_reference_id.clone(),
            event_tx,
            provider
        )
    };

    if let Some(key) = api_key {
        let mut tts = crate::tts::fish::FishTts::new(key);
        tts.set_reference_id(reference_id.clone());
        if let Some(url) = &proxy_url {
            tts.set_proxy(Some(url.clone()));
        }

        if let Some(tx) = event_tx {
            tts = tts.with_event_tx(tx);
        }

        if matches!(current_provider.as_ref(), Some(TtsProvider::Fish(_))) {
            *self.tts_providers.lock() = Some(TtsProvider::Fish(tts));
        }
    }

    self.tts_config.write().fish_proxy_url = proxy_url;
}
```

### 1.6 Добавление Tauri команд

**Файл:** `src-tauri/src/commands/mod.rs`

Добавить команды Fish Audio:

```rust
use crate::tts::VoiceModel;

// ========== Команды Fish Audio TTS ==========

#[tauri::command]
pub fn get_fish_audio_api_key(state: State<'_, AppState>) -> Option<String> {
    state.get_fish_audio_api_key()
}

#[tauri::command]
pub fn set_fish_audio_api_key(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    key: String,
) -> Result<(), String> {
    if key.is_empty() {
        return Err("API Key не может быть пустым".into());
    }

    state.set_fish_audio_api_key(Some(key.clone()));
    state.init_fish_audio_tts(key.clone());

    settings_manager.set_fish_audio_api_key(Some(key))
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn get_fish_audio_reference_id(
    settings_manager: State<'_, SettingsManager>
) -> String {
    settings_manager.get_fish_audio_reference_id()
}

#[tauri::command]
pub fn set_fish_audio_reference_id(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    reference_id: String,
) -> Result<(), String> {
    if reference_id.trim().is_empty() {
        return Err("Reference ID не может быть пустым".into());
    }

    settings_manager.set_fish_audio_reference_id(reference_id.clone())
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    state.set_fish_audio_reference_id(reference_id.clone());

    Ok(())
}

#[tauri::command]
pub fn get_fish_audio_voices(
    settings_manager: State<'_, SettingsManager>
) -> Vec<VoiceModel> {
    settings_manager.get_fish_audio_voices()
}

#[tauri::command]
pub fn add_fish_audio_voice(
    settings_manager: State<'_, SettingsManager>,
    voice: VoiceModel,
) -> Result<(), String> {
    if voice.id.trim().is_empty() {
        return Err("Voice ID не может быть пустым".into());
    }

    settings_manager.add_fish_audio_voice(voice)
        .map_err(|e| format!("Failed to add voice: {}", e))
}

#[tauri::command]
pub fn remove_fish_audio_voice(
    settings_manager: State<'_, SettingsManager>,
    voice_id: String,
) -> Result<(), String> {
    settings_manager.remove_fish_audio_voice(&voice_id)
        .map_err(|e| format!("Failed to remove voice: {}", e))
}

/// Получить список моделей из Fish Audio API
#[tauri::command]
pub async fn fetch_fish_audio_models(
    settings_manager: State<'_, SettingsManager>,
    page_size: Option<u32>,
    page_number: Option<u32>,
    title: Option<String>,
    language: Option<String>,
) -> Result<(i32, Vec<VoiceModel>), String> {
    let api_key = settings_manager.get_fish_audio_api_key()
        .ok_or_else(|| "API ключ не установлен".to_string())?;

    let proxy_url = if settings_manager.get_fish_audio_use_proxy() {
        settings_manager.get_socks5_proxy_url()
    } else {
        None
    };

    let page_size = page_size.unwrap_or(10);
    let page_number = page_number.unwrap_or(1);

    crate::tts::fish::FishTts::list_models(
        &api_key,
        proxy_url.as_deref(),
        page_size,
        page_number,
        title.as_deref(),
        language.as_deref(),
    ).await
}

#[tauri::command]
pub fn set_fish_audio_format(
    settings_manager: State<'_, SettingsManager>,
    format: String,
) -> Result<(), String> {
    settings_manager.set_fish_audio_format(format)
        .map_err(|e| format!("Failed to save format: {}", e))
}

#[tauri::command]
pub fn set_fish_audio_temperature(
    settings_manager: State<'_, SettingsManager>,
    temperature: f32,
) -> Result<(), String> {
    if !(0.0..=1.0).contains(&temperature) {
        return Err("Temperature must be between 0.0 and 1.0".into());
    }

    settings_manager.set_fish_audio_temperature(temperature)
        .map_err(|e| format!("Failed to save temperature: {}", e))
}

#[tauri::command]
pub fn set_fish_audio_sample_rate(
    settings_manager: State<'_, SettingsManager>,
    sample_rate: u32,
) -> Result<(), String> {
    if sample_rate == 0 {
        return Err("Sample rate cannot be zero".into());
    }

    settings_manager.set_fish_audio_sample_rate(sample_rate)
        .map_err(|e| format!("Failed to save sample rate: {}", e))
}

#[tauri::command]
pub fn set_fish_audio_use_proxy(
    enabled: bool,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager.set_fish_audio_use_proxy(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))
}

#[tauri::command]
pub fn apply_fish_audio_proxy_settings(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let proxy_url = if settings.tts.fish.use_proxy {
        settings.tts.network.proxy.proxy_url.clone()
    } else {
        None
    };

    state.set_fish_audio_proxy(proxy_url);

    Ok(())
}
```

Обновить `set_tts_provider`:

```rust
match provider {
    TtsProviderType::OpenAi => { /* существующий код */ }
    TtsProviderType::Silero => { /* существующий код */ }
    TtsProviderType::Local => { /* существующий код */ }
    TtsProviderType::Fish => {
        info!("Initializing Fish Audio TTS");
        let api_key = state.get_fish_audio_api_key();
        if let Some(key) = api_key {
            state.init_fish_audio_tts(key);
            debug!("Fish Audio TTS initialized");
        } else {
            warn!("No API key found, Fish Audio TTS not initialized");
        }
    }
}
```

### 1.7 Регистрация команд

**Файл:** `src-tauri/src/commands/mod.rs`

Добавить в `invoke_handler`:

```rust
fn generate_handler() -> impl Fn(tauri::Invoke) + Send + Sync + 'static {
    |app| {
        // ... существующие команды ...

        // Fish Audio
        invoke_handler![get_fish_audio_api_key, set_fish_audio_api_key,
                       get_fish_audio_reference_id, set_fish_audio_reference_id,
                       get_fish_audio_voices, add_fish_audio_voice, remove_fish_audio_voice,
                       fetch_fish_audio_models,
                       set_fish_audio_format, set_fish_audio_temperature,
                       set_fish_audio_sample_rate, set_fish_audio_use_proxy,
                       apply_fish_audio_proxy_settings]
    }
}
```

### 1.8 Обновление setup

**Файл:** `src-tauri/src/setup.rs`

Обновить `init_tts_provider`:

```rust
match settings.tts.provider {
    TtsProviderType::OpenAi => { /* существующий код */ }
    TtsProviderType::Silero => { /* существующий код */ }
    TtsProviderType::Local => { /* существующий код */ }
    TtsProviderType::Fish => {
        if let Some(ref api_key) = settings.tts.fish.api_key {
            let api_key_str: String = api_key.clone();
            app_state.set_fish_audio_api_key(Some(api_key_str.clone()));
            info!("Fish Audio API key loaded");
            app_state.init_fish_audio_tts(api_key_str.clone());
            app_state.set_fish_audio_reference_id(settings.tts.fish.reference_id.clone());

            if settings.tts.fish.use_proxy {
                if let Some(ref proxy_url) = settings.tts.network.proxy.proxy_url {
                    app_state.set_fish_audio_proxy(Some(proxy_url.clone()));
                }
            }
        } else {
            warn!("Fish Audio selected but no API key found");
        }
    }
}
```

---

## Фаза 2: Реализация фронтенда

### 2.1 Обновление TypeScript типов

**Файл:** `src/types/settings.ts`

```typescript
export const TtsProviderType = {
  OpenAi: 'openai',
  Silero: 'silero',
  Local: 'local',
  Fish: 'fish'  // НОВЫЙ
} as const

export type TtsProviderType = (typeof TtsProviderType)[keyof typeof TtsProviderType]

/// Голосовая модель Fish Audio
export interface VoiceModel {
  id: string
  title: string
  description?: string
  cover_image?: string
  languages: string[]
  author_nickname?: string
}

export interface FishAudioSettingsDto {
  api_key?: string
  voices: VoiceModel[]
  reference_id: string
  format: string
  temperature: number
  sample_rate: number
  use_proxy?: boolean
}

export interface TtsSettingsDto {
  provider: TtsProviderType
  openai: OpenAiSettingsDto
  local: LocalTtsSettingsDto
  fish: FishAudioSettingsDto  // НОВЫЙ
  telegram: TelegramTtsSettingsDto
  network: NetworkSettingsDto
}
```

### 2.2 Создание компонента модального окна выбора моделей

**Файл:** `src/components/tts/FishAudioModelPicker.vue` (НОВЫЙ)

```vue
<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { VoiceModel } from '../../types/settings';
import { Search, Loader2 } from 'lucide-vue-next';

interface Props {
  apiKey?: string;
  loading?: boolean;
}

interface Emits {
  (e: 'select', model: VoiceModel): void;
  (e: 'close'): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const searchQuery = ref('');
const loading = ref(false);
const models = ref<VoiceModel[]>([]);
const total = ref(0);
const currentPage = ref(1);
const pageSize = 10;
const error = ref<string | null>(null);

const hasMore = computed(() => models.value.length < total.value);

async function fetchModels(page: number = 1) {
  if (!props.apiKey) {
    error.value = 'API ключ не установлен';
    return;
  }

  loading.value = true;
  error.value = null;

  try {
    const [fetchedTotal, fetchedModels] = await invoke<(number, VoiceModel[])>('fetch_fish_audio_models', {
      pageSize,
      pageNumber: page,
      title: searchQuery.value || null,
      language: null
    });

    if (page === 1) {
      models.value = fetchedModels;
    } else {
      models.value.push(...fetchedModels);
    }

    total.value = fetchedTotal;
    currentPage.value = page;
  } catch (e) {
    error.value = e as string;
    console.error('Failed to fetch models:', e);
  } finally {
    loading.value = false;
  }
}

async function loadMore() {
  if (loading.value || !hasMore.value) return;
  await fetchModels(currentPage.value + 1);
}

function selectModel(model: VoiceModel) {
  emit('select', model);
}

function handleClose() {
  emit('close');
}

// Debounced search
let searchTimeout: ReturnType<typeof setTimeout> | null = null;

watch(searchQuery, () => {
  if (searchTimeout) clearTimeout(searchTimeout);

  searchTimeout = setTimeout(() => {
    fetchModels(1);
  }, 500);
});

// Initial fetch
watch(() => props.apiKey, (key) => {
  if (key) {
    fetchModels(1);
  } else {
    models.value = [];
    total.value = 0;
  }
}, { immediate: true });
</script>

<template>
  <div class="modal-overlay" @click.self="handleClose">
    <div class="modal-content">
      <div class="modal-header">
        <h2>Выберите голосовую модель</h2>
        <button @click="handleClose" class="close-button">&times;</button>
      </div>

      <div class="modal-body">
        <!-- Search -->
        <div class="search-container">
          <Search :size="18" class="search-icon" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Поиск по названию..."
            class="search-input"
          />
        </div>

        <!-- Models list -->
        <div v-if="error" class="error-message">
          {{ error }}
        </div>

        <div v-else-if="loading && models.length === 0" class="loading-container">
          <Loader2 :size="32" class="spinner" />
          <p>Загрузка моделей...</p>
        </div>

        <div v-else-if="models.length === 0" class="empty-state">
          <p>Модели не найдены</p>
        </div>

        <div v-else class="models-list">
          <div
            v-for="model in models"
            :key="model.id"
            @click="selectModel(model)"
            class="model-item"
          >
            <div v-if="model.cover_image" class="model-cover">
              <img :src="model.cover_image" :alt="model.title" />
            </div>
            <div v-else class="model-cover model-cover-placeholder">
              {{ model.title.charAt(0) }}
            </div>

            <div class="model-info">
              <div class="model-title">{{ model.title }}</div>
              <div v-if="model.description" class="model-description">
                {{ model.description }}
              </div>
              <div class="model-meta">
                <span v-if="model.languages.length" class="model-languages">
                  {{ model.languages.join(', ') }}
                </span>
                <span v-if="model.author_nickname" class="model-author">
                  by {{ model.author_nickname }}
                </span>
              </div>
            </div>
          </div>

          <!-- Load more -->
          <div v-if="hasMore && !loading" class="load-more-container">
            <button @click.stop="loadMore" class="load-more-button">
              Загрузить ещё
            </button>
          </div>

          <div v-if="loading && models.length > 0" class="loading-more">
            <Loader2 :size="24" class="spinner" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: var(--color-bg-primary);
  border-radius: 16px;
  width: 90%;
  max-width: 600px;
  max-height: 80vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem;
  border-bottom: 1px solid var(--color-border);
}

.modal-header h2 {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--color-text-primary);
  margin: 0;
}

.close-button {
  background: none;
  border: none;
  font-size: 2rem;
  color: var(--color-text-secondary);
  cursor: pointer;
  line-height: 1;
  padding: 0;
  width: 2rem;
  height: 2rem;
}

.close-button:hover {
  color: var(--color-text-primary);
}

.modal-body {
  padding: 1.5rem;
  overflow-y: auto;
  flex: 1;
}

.search-container {
  position: relative;
  margin-bottom: 1rem;
}

.search-icon {
  position: absolute;
  left: 1rem;
  top: 50%;
  transform: translateY(-50%);
  color: var(--color-text-secondary);
}

.search-input {
  width: 100%;
  padding: 0.75rem 1rem 0.75rem 2.5rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 14px;
}

.search-input:focus {
  outline: none;
  border-color: var(--color-accent);
}

.error-message {
  padding: 1rem;
  background: rgba(239, 68, 68, 0.1);
  border: 1px solid rgba(239, 68, 68, 0.3);
  border-radius: 8px;
  color: var(--color-error);
  text-align: center;
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 2rem;
  color: var(--color-text-secondary);
}

.spinner {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.empty-state {
  text-align: center;
  padding: 2rem;
  color: var(--color-text-secondary);
}

.models-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.model-item {
  display: flex;
  gap: 1rem;
  padding: 1rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.model-item:hover {
  background: var(--color-bg-tertiary);
  border-color: var(--color-accent);
}

.model-cover {
  width: 48px;
  height: 48px;
  border-radius: 8px;
  overflow: hidden;
  flex-shrink: 0;
}

.model-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.model-cover-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--color-accent);
  color: white;
  font-size: 1.5rem;
  font-weight: 600;
}

.model-info {
  flex: 1;
  min-width: 0;
}

.model-title {
  font-size: 1rem;
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: 0.25rem;
}

.model-description {
  font-size: 0.875rem;
  color: var(--color-text-secondary);
  margin-bottom: 0.5rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.model-meta {
  display: flex;
  gap: 0.75rem;
  font-size: 0.75rem;
}

.model-languages {
  color: var(--color-text-secondary);
}

.model-author {
  color: var(--color-text-tertiary);
}

.load-more-container {
  display: flex;
  justify-content: center;
  margin-top: 1rem;
}

.load-more-button {
  padding: 0.75rem 1.5rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
}

.load-more-button:hover {
  background: var(--color-bg-tertiary);
  border-color: var(--color-accent);
}

.loading-more {
  display: flex;
  justify-content: center;
  padding: 1rem;
}
</style>
```

### 2.3 Создание компонента карточки Fish Audio

**Файл:** `src/components/tts/TtsFishAudioCard.vue` (НОВЫЙ)

```vue
<script setup lang="ts">
import { ref, computed } from 'vue';
import { Cloud, Plus, X, Search } from 'lucide-vue-next';
import ProviderCard from '../shared/ProviderCard.vue';
import InputWithToggle from '../shared/InputWithToggle.vue';
import FishAudioModelPicker from './FishAudioModelPicker.vue';
import type { VoiceModel } from '../../types/settings';

interface Props {
  active?: boolean;
  expanded?: boolean;
  apiKey?: string;
  referenceId?: string;
  voices?: VoiceModel[];
  format?: string;
  temperature?: number;
  sampleRate?: number;
  useProxy?: boolean;
}

interface Emits {
  (e: 'select'): void;
  (e: 'toggle'): void;
  (e: 'save-api-key', key: string): void;
  (e: 'select-voice', voiceId: string): void;
  (e: 'remove-voice', voiceId: string): void;
  (e: 'format-change', format: string): void;
  (e: 'temperature-change', temperature: number): void;
  (e: 'sample-rate-change', sampleRate: number): void;
  (e: 'toggle-proxy', enabled: boolean): void;
}

const props = withDefaults(defineProps<Props>(), {
  active: false,
  expanded: false,
  apiKey: '',
  referenceId: '',
  voices: () => [],
  format: 'mp3',
  temperature: 0.7,
  sampleRate: 44100,
  useProxy: false,
});

const emit = defineEmits<Emits>();

const showModelPicker = ref(false);

const audioFormats = [
  { value: 'mp3', label: 'MP3' },
  { value: 'wav', label: 'WAV' },
  { value: 'pcm', label: 'PCM' },
  { value: 'opus', label: 'Opus' },
];

const sampleRates = [
  { value: 8000, label: '8000 Hz' },
  { value: 16000, label: '16000 Hz' },
  { value: 24000, label: '24000 Hz' },
  { value: 32000, label: '32000 Hz' },
  { value: 44100, label: '44100 Hz' },
  { value: 48000, label: '48000 Hz' },
];

function handleSaveApiKey() {
  if (!props.apiKey.trim()) return;
  emit('save-api-key', props.apiKey);
}

function handleOpenModelPicker() {
  if (!props.apiKey) {
    emit('save-api-key', props.apiKey);
  }
  showModelPicker.value = true;
}

function handleSelectModel(model: VoiceModel) {
  emit('select-voice', model.id);
  showModelPicker.value = false;
}

function handleRemoveVoice(voiceId: string, event: Event) {
  event.stopPropagation();
  emit('remove-voice', voiceId);
}

function handleProxyToggle(event: Event) {
  const target = event.target as HTMLInputElement;
  emit('toggle-proxy', target.checked);
}

const selectedVoice = computed(() => {
  return props.voices.find(v => v.id === props.referenceId);
});
</script>

<template>
  <ProviderCard
    title="Fish Audio"
    :icon="Cloud"
    :active="active"
    :expanded="expanded"
    @select="$emit('select')"
    @toggle="$emit('toggle')"
  >
    <div class="card-content-inner">
      <!-- API Key -->
      <div class="setting-group">
        <div class="form-row">
          <label>Ключ API:</label>
          <InputWithToggle
            :model-value="apiKey"
            @update:model-value="$emit('save-api-key', $event)"
            type="password"
            placeholder="Введите API ключ"
            class="input-wide"
          />
          <button @click="handleSaveApiKey" class="save-button">Сохранить</button>
        </div>
      </div>

      <!-- Voice Management -->
      <div class="setting-group">
        <div class="voice-header">
          <label>Голосовые модели:</label>
          <button @click="handleOpenModelPicker" class="add-model-button">
            <Plus :size="16" />
            Добавить из API
          </button>
        </div>

        <div v-if="voices.length > 0" class="voice-list">
          <div
            v-for="voice in voices"
            :key="voice.id"
            :class="['voice-item', { active: referenceId === voice.id }]"
            @click="$emit('select-voice', voice.id)"
          >
            <div class="voice-cover">
              <img v-if="voice.cover_image" :src="voice.cover_image" :alt="voice.title" />
              <div v-else class="voice-cover-placeholder">{{ voice.title.charAt(0) }}</div>
            </div>

            <div class="voice-info">
              <div class="voice-title">{{ voice.title }}</div>
              <div v-if="voice.description" class="voice-description">{{ voice.description }}</div>
              <div class="voice-meta">
                <span v-if="voice.languages.length" class="voice-languages">
                  {{ voice.languages.join(', ') }}
                </span>
              </div>
            </div>

            <button
              @click="handleRemoveVoice(voice.id, $event)"
              class="remove-button"
              title="Удалить"
            >
              <X :size="14" />
            </button>
          </div>
        </div>
        <div v-else class="empty-voices">
          Нет добавленных голосовых моделей
        </div>

        <!-- Selected voice display -->
        <div v-if="selectedVoice" class="selected-voice">
          Выбрано: <strong>{{ selectedVoice.title }}</strong>
        </div>
      </div>

      <!-- Audio Settings -->
      <div class="setting-group">
        <div class="audio-settings-row">
          <div class="audio-setting">
            <label>Формат:</label>
            <select
              :value="format"
              @change="$emit('format-change', ($event.target as HTMLSelectElement).value)"
              class="setting-select"
            >
              <option v-for="f in audioFormats" :key="f.value" :value="f.value">
                {{ f.label }}
              </option>
            </select>
          </div>

          <div class="audio-setting">
            <label>Частота:</label>
            <select
              :value="sampleRate"
              @change="$emit('sample-rate-change', Number(($event.target as HTMLSelectElement).value))"
              class="setting-select"
            >
              <option v-for="sr in sampleRates" :key="sr.value" :value="sr.value">
                {{ sr.label }}
              </option>
            </select>
          </div>

          <div class="audio-setting">
            <label>Температура: {{ temperature }}</label>
            <input
              type="range"
              :value="temperature"
              @input="$emit('temperature-change', Number(($event.target as HTMLInputElement).value))"
              min="0"
              max="1"
              step="0.1"
              class="temperature-slider"
            />
          </div>
        </div>
      </div>

      <!-- Proxy -->
      <div class="setting-group">
        <div class="proxy-checkbox-container">
          <input
            id="fish-use-proxy"
            type="checkbox"
            :checked="useProxy"
            @change="handleProxyToggle"
            class="proxy-checkbox"
          />
          <label for="fish-use-proxy" class="proxy-checkbox-label">
            Использовать SOCKS5
          </label>
        </div>
      </div>
    </div>
  </ProviderCard>

  <!-- Model Picker Modal -->
  <FishAudioModelPicker
    v-if="showModelPicker"
    :api-key="apiKey"
    @select="handleSelectModel"
    @close="showModelPicker = false"
  />
</template>

<style scoped>
.card-content-inner {
  padding-top: 8px;
}

.setting-group {
  margin-top: 16px;
  margin-bottom: 12px;
}

.setting-group:last-child {
  margin-bottom: 0;
}

.setting-group > label {
  display: block;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  margin-bottom: 8px;
}

.form-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.form-row label {
  min-width: 60px;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.input-wide {
  flex: 1;
  min-width: 200px;
}

.save-button {
  padding: 0.6rem 1.2rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  border-radius: 10px;
  color: var(--color-text-white);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: filter 0.2s;
  flex-shrink: 0;
}

.save-button:hover {
  filter: brightness(1.06);
}

.voice-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.voice-header label {
  margin-bottom: 0;
}

.add-model-button {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0.5rem 1rem;
  background: var(--color-accent);
  border: none;
  border-radius: 8px;
  color: var(--color-text-white);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: filter 0.2s;
}

.add-model-button:hover {
  filter: brightness(1.1);
}

.voice-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 300px;
  overflow-y: auto;
}

.voice-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0.75rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.voice-item:hover {
  background: var(--color-bg-tertiary);
}

.voice-item.active {
  border-color: var(--color-accent);
  background: var(--color-accent-alpha);
}

.voice-cover {
  width: 40px;
  height: 40px;
  border-radius: 6px;
  overflow: hidden;
  flex-shrink: 0;
}

.voice-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.voice-cover-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--color-accent);
  color: white;
  font-size: 1.25rem;
  font-weight: 600;
}

.voice-info {
  flex: 1;
  min-width: 0;
}

.voice-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: 2px;
}

.voice-description {
  font-size: 12px;
  color: var(--color-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-bottom: 4px;
}

.voice-meta {
  font-size: 11px;
  color: var(--color-text-tertiary);
}

.voice-languages {
  text-transform: uppercase;
}

.remove-button {
  padding: 4px;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s;
  flex-shrink: 0;
}

.remove-button:hover {
  color: var(--color-error);
}

.empty-voices {
  padding: 1rem;
  text-align: center;
  color: var(--color-text-secondary);
  font-size: 13px;
  background: var(--color-bg-secondary);
  border-radius: 8px;
}

.selected-voice {
  padding: 0.75rem;
  background: var(--color-accent-alpha);
  border: 1px solid var(--color-accent);
  border-radius: 8px;
  font-size: 13px;
  color: var(--color-text-primary);
}

.audio-settings-row {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.audio-setting {
  display: flex;
  align-items: center;
  gap: 12px;
}

.audio-setting label {
  min-width: 90px;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.setting-select {
  flex: 1;
  padding: 0.5rem 0.8rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 13px;
  cursor: pointer;
}

.setting-select:focus {
  outline: none;
  border-color: var(--color-accent);
}

.temperature-slider {
  flex: 1;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.proxy-checkbox-container {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.proxy-checkbox {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.proxy-checkbox-label {
  cursor: pointer;
  user-select: none;
  font-size: 14px;
  color: var(--color-text-primary);
}
</style>
```

### 2.4 Обновление компонента TtsPanel

**Файл:** `src/components/TtsPanel.vue`

Добавить импорт и состояние для Fish Audio:

```typescript
import TtsFishAudioCard from './tts/TtsFishAudioCard.vue';
import type { VoiceModel } from '../types/settings';

const providers = ref<Record<TtsProviderType, TtsProviderState>>({
  openai: { type: 'openai', configured: false, expanded: false },
  silero: { type: 'silero', configured: false, expanded: false },
  local: { type: 'local', configured: false, expanded: false },
  fish: { type: 'fish', configured: false, expanded: false },  // NEW
});

const fishAudioApiKey = ref('');
const fishAudioReferenceId = ref('');
const fishAudioVoices = ref<VoiceModel[]>([]);
const fishAudioFormat = ref('mp3');
const fishAudioTemperature = ref(0.7);
const fishAudioSampleRate = ref(44100);
const fishAudioUseProxy = ref(false);
```

Добавить методы:

```typescript
async function saveFishAudioApiKey(key: string) {
  if (!key.trim()) {
    showError('API Key не может быть пустым');
    return;
  }

  try {
    await invoke('set_fish_audio_api_key', { key });
    providers.value.fish.configured = true;
    showSuccess('API Key сохранён');
  } catch (error) {
    showError(error as string);
  }
}

async function saveFishAudioReferenceId(referenceId: string) {
  try {
    await invoke('set_fish_audio_reference_id', { referenceId });
    showSuccess(`Модель сохранена`);
  } catch (error) {
    showError(error as string);
  }
}

async function addFishAudioVoice(voiceId: string) {
  try {
    await invoke('add_fish_audio_voice', { voiceId });
    showSuccess('Голосовая модель добавлена');
  } catch (error) {
    showError(error as string);
  }
}

async function removeFishAudioVoice(voiceId: string) {
  try {
    await invoke('remove_fish_audio_voice', { voiceId });
    showSuccess('Голосовая модель удалена');
  } catch (error) {
    showError(error as string);
  }
}

async function selectFishAudioVoice(voiceId: string) {
  await saveFishAudioReferenceId(voiceId);
}

async function changeFishAudioFormat(format: string) {
  try {
    await invoke('set_fish_audio_format', { format });
  } catch (error) {
    showError(error as string);
  }
}

async function changeFishAudioTemperature(temperature: number) {
  try {
    await invoke('set_fish_audio_temperature', { temperature });
  } catch (error) {
    showError(error as string);
  }
}

async function changeFishAudioSampleRate(sampleRate: number) {
  try {
    await invoke('set_fish_audio_sample_rate', { sampleRate });
  } catch (error) {
    showError(error as string);
  }
}

async function toggleFishAudioUseProxy(enabled: boolean) {
  try {
    await invoke('set_fish_audio_use_proxy', { enabled });

    if (activeProvider.value === 'fish') {
      await invoke('apply_fish_audio_proxy_settings');
    }

    showSuccess(enabled ? 'Прокси включён' : 'Прокси выключен');
  } catch (error) {
    showError(error as string);
    throw error;
  }
}
```

Обновить watch:

```typescript
watch(ttsSettings, (newSettings) => {
  if (!newSettings) return;

  // ... existing code ...

  if (newSettings.fish) {
    if (newSettings.fish.api_key) {
      fishAudioApiKey.value = newSettings.fish.api_key;
      providers.value.fish.configured = true;
    }
    if (newSettings.fish.reference_id) {
      fishAudioReferenceId.value = newSettings.fish.reference_id;
    }
    if (newSettings.fish.voices) {
      fishAudioVoices.value = newSettings.fish.voices;
    }
    if (newSettings.fish.format) {
      fishAudioFormat.value = newSettings.fish.format;
    }
    if (newSettings.fish.temperature !== undefined) {
      fishAudioTemperature.value = newSettings.fish.temperature;
    }
    if (newSettings.fish.sample_rate) {
      fishAudioSampleRate.value = newSettings.fish.sample_rate;
    }
    if (newSettings.fish.use_proxy !== undefined) {
      fishAudioUseProxy.value = newSettings.fish.use_proxy;
    }
  }
}, { immediate: true, deep: true });
```

Добавить в template:

```vue
<TtsFishAudioCard
  :active="activeProvider === 'fish'"
  :expanded="providers.fish.expanded"
  :api-key="fishAudioApiKey"
  :reference-id="fishAudioReferenceId"
  :voices="fishAudioVoices"
  :format="fishAudioFormat"
  :temperature="fishAudioTemperature"
  :sample-rate="fishAudioSampleRate"
  :use-proxy="fishAudioUseProxy"
  @select="setActiveProvider('fish')"
  @toggle="toggleProvider('fish')"
  @save-api-key="saveFishAudioApiKey"
  @select-voice="selectFishAudioVoice"
  @remove-voice="removeFishAudioVoice"
  @format-change="changeFishAudioFormat"
  @temperature-change="changeFishAudioTemperature"
  @sample-rate-change="changeFishAudioSampleRate"
  @toggle-proxy="toggleFishAudioUseProxy"
/>
```

---

## Фаза 3: Проверка

### Сборка и компиляция

```bash
# Бэкенд
cd src-tauri
cargo check

# Фронтенд
npm run type-check
npm run build
```

### Чеклист ручного тестирования

1. **Настройки API:**
   - [ ] API ключ сохраняется и загружается корректно
   - [ ] Ошибка при пустом API ключе

2. **Управление моделями:**
   - [ ] Кнопка "Добавить из API" открывает модальное окно
   - [ ] Список моделей загружается корректно
   - [ ] Поиск по названию работает
   - [ ] Подгрузка следующих страниц работает
   - [ ] При выборе модель добавляется в список
   - [ ] Модель удаляется из списка
   - [ ] Выбранная модель подсвечивается
   - [ ] Обложки моделей отображаются корректно

3. **Настройки аудио:**
   - [ ] Формат аудио сохраняется
   - [ ] Частота дискретизации сохраняется
   - [ ] Температура сохраняется

4. **TTS функциональность:**
   - [ ] Синтез работает с валидным API ключом и выбранной моделью
   - [ ] Ошибка когда модель не выбрана
   - [ ] Обработка ошибок API

5. **Прокси:**
   - [ ] SOCKS5 прокси работает для TTS
   - [ ] SOCKS5 прокси работает для запроса списка моделей

6. **UI:**
   - [ ] Поддержка темы (светлая/тёмная)
   - [ ] Модальное окно закрывается по клику вне
   - [ ] Скролл списка моделей работает
   - [ ] Сообщения о статусе отображаются корректно

### Энд-ту-энд тест

1. Добавить API ключ Fish Audio
2. Нажать "Добавить из API"
3. Ввести поисковый запрос (например, "ru" для русского)
4. Выбрать модель из списка
5. Проверить что модель добавилась и отобразилась
6. Включить SOCKS5 прокси (если настроен)
7. Переключиться на провайдер Fish Audio
8. Протестировать синтез TTS
9. Проверить воспроизведение аудио

---

## Порядок реализации

1. **Бэкенд (порядок важен!):**
   - Создать модуль `fish.rs` с VoiceModel и list_models
   - Обновить `mod.rs` (реэкспорт VoiceModel)
   - Обновить `settings.rs` (использовать VoiceModel вместо Vec<String>)
   - Обновить `dto.rs` (добавить VoiceModelDto)
   - Обновить `state.rs` (добавить методы Fish Audio)
   - Добавить команды в `commands/mod.rs`
   - Обновить `setup.rs`
   - Протестировать `cargo check`

2. **Фронтенд:**
   - Обновить TypeScript типы (добавить VoiceModel)
   - Создать `FishAudioModelPicker.vue`
   - Создать `TtsFishAudioCard.vue`
   - Обновить `TtsPanel.vue`
   - Протестировать `npm run type-check`

3. **Интеграционное тестирование:**
   - Полная сборка
   - Ручное тестирование с реальным API ключом
   - Тестирование прокси

---

## Сводка критических файлов

| Файл | Действие |
|------|---------|
| `src-tauri/src/tts/fish.rs` | СОЗДАТЬ - Реализация Fish Audio TTS + VoiceModel + list_models |
| `src-tauri/src/tts/mod.rs` | ИЗМЕНИТЬ - Добавить Fish + реэкспорт VoiceModel |
| `src-tauri/src/config/settings.rs` | ИЗМЕНИТЬ - FishAudioSettings с Vec<VoiceModel> |
| `src-tauri/src/config/dto.rs` | ИЗМЕНИТЬ - Добавить VoiceModelDto |
| `src-tauri/src/state.rs` | ИЗМЕНИТЬ - Добавить методы Fish Audio |
| `src-tauri/src/commands/mod.rs` | ИЗМЕНИТЬ - Добавить команды Fish Audio + fetch_fish_audio_models |
| `src-tauri/src/setup.rs` | ИЗМЕНИТЬ - Добавить инициализацию Fish Audio |
| `src/types/settings.ts` | ИЗМЕНИТЬ - Добавить VoiceModel и FishAudioSettingsDto |
| `src/components/tts/FishAudioModelPicker.vue` | СОЗДАТЬ - Модальное окно выбора моделей |
| `src/components/tts/TtsFishAudioCard.vue` | СОЗДАТЬ - UI карточка Fish Audio |
| `src/components/TtsPanel.vue` | ИЗМЕНИТЬ - Интегрировать Fish Audio |
