# Multiple TTS Providers Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add support for multiple TTS providers (OpenAI, Silero Bot, TTSVoiceWizard) with a collapsible UI for configuration.

**Architecture:** Create a `tts/` module with a `TtsEngine` trait and `TtsProvider` enum that abstracts different TTS implementations. Each provider implements the same interface for synthesis and configuration checking.

**Tech Stack:** Rust (async/await, reqwest), Tauri commands, Vue 3 Composition API, TypeScript

---

## Task 1: Create tts module directory structure

**Files:**
- Create: `src-tauri/src/tts/mod.rs`
- Create: `src-tauri/src/tts/engine.rs`
- Create: `src-tauri/src/tts/openai.rs`
- Create: `src-tauri/src/tts/local.rs`
- Create: `src-tauri/src/tts/silero.rs`

**Step 1: Create module structure**

Create the `tts` directory and files.

**Step 2: Write tts/mod.rs**

```rust
pub mod engine;
pub mod local;
pub mod openai;
pub mod silero;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TtsProviderType {
    OpenAi,
    Silero,
    Local,
}

impl Default for TtsProviderType {
    fn default() -> Self {
        TtsProviderType::OpenAi
    }
}

pub enum TtsProvider {
    OpenAi(openai::OpenAiTts),
    Silero(silero::SileroTts),
    Local(local::LocalTts),
}

impl TtsProvider {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        match self {
            TtsProvider::OpenAi(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
            TtsProvider::Local(tts) => tts.synthesize(text).await,
            TtsProvider::Silero(tts) => tts.synthesize(text).await,
        }
    }

    pub fn is_configured(&self) -> bool {
        match self {
            TtsProvider::OpenAi(tts) => tts.is_configured(),
            TtsProvider::Local(tts) => tts.is_configured(),
            TtsProvider::Silero(tts) => tts.is_configured(),
        }
    }
}
```

**Step 3: Commit**

```bash
git add src-tauri/src/tts/
git commit -m "feat(tts): create module structure for multiple TTS providers"
```

---

## Task 2: Implement TtsEngine trait

**Files:**
- Modify: `src-tauri/src/tts/engine.rs`

**Step 1: Write the trait definition**

```rust
use async_trait::async_trait;

#[async_trait]
pub trait TtsEngine: Send + Sync {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String>;
    fn is_configured(&self) -> bool;
    fn name(&self) -> &str;
}
```

**Step 2: Commit**

```bash
git add src-tauri/src/tts/engine.rs
git commit -m "feat(tts): add TtsEngine trait for provider abstraction"
```

---

## Task 3: Refactor OpenAI TTS to use new structure

**Files:**
- Modify: `src-tauri/src/tts/openai.rs`
- Delete: `src-tauri/src/tts.rs` (old file)

**Step 1: Move and refactor OpenAI implementation**

Move content from `src-tauri/src/tts.rs` to `src-tauri/src/tts/openai.rs`:

```rust
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::tts::engine::TtsEngine;

#[derive(Debug, Serialize)]
struct TtsRequest {
    model: String,
    input: String,
    voice: String,
}

#[derive(Clone)]
pub struct OpenAiTts {
    api_key: String,
    voice: String,
}

impl OpenAiTts {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            voice: "alloy".to_string(),
        }
    }

    pub fn set_voice(&mut self, voice: String) {
        self.voice = voice;
    }

    pub fn get_api_key(&self) -> &str {
        &self.api_key
    }

    pub fn get_voice(&self) -> &str {
        &self.voice
    }
}

#[async_trait]
impl TtsEngine for OpenAiTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        let client = Client::new();
        let request = TtsRequest {
            model: "tts-1".to_string(),
            input: text.to_string(),
            voice: self.voice.clone(),
        };

        let response = client
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to send TTS request: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("TTS request failed: {}", error_text));
        }

        let audio_data = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read audio data: {}", e))?
            .to_vec();

        Ok(audio_data)
    }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && self.api_key.starts_with("sk-")
    }

    fn name(&self) -> &str {
        "OpenAI"
    }
}

// AudioPlayer remains
pub struct AudioPlayer;

impl AudioPlayer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn play(&self, audio_data: Vec<u8>) -> Result<(), String> {
        use std::io::Write;
        use std::env::temp_dir;

        let temp_path = temp_dir().join("tts_output.mp3");

        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| format!("Failed to create temp file: {}", e))?;
        file.write_all(&audio_data)
            .map_err(|e| format!("Failed to write audio data: {}", e))?;

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/c", "start", "", temp_path.to_str().unwrap()])
                .spawn()
                .map_err(|e| format!("Failed to play audio: {}", e))?;
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(temp_path)
                .spawn()
                .map_err(|e| format!("Failed to play audio: {}", e))?;
        }

        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(temp_path)
                .spawn()
                .map_err(|e| format!("Failed to play audio: {}", e))?;
        }

        Ok(())
    }
}
```

**Step 2: Update lib.rs imports**

Change:
```rust
use crate::tts::OpenAiTts;
```
To:
```rust
use crate::tts::openai::OpenAiTts;
use crate::tts::{TtsProvider, TtsProviderType};
```

**Step 3: Delete old tts.rs**

```bash
rm src-tauri/src/tts.rs
```

**Step 4: Commit**

```bash
git add src-tauri/src/tts/openai.rs src-tauri/src/lib.rs
git commit -m "refactor(tts): move OpenAI to new module structure"
```

---

## Task 4: Implement LocalTts (TTSVoiceWizard)

**Files:**
- Create: `src-tauri/src/tts/local.rs`

**Step 1: Write LocalTts implementation**

```rust
use async_trait::async_trait;
use reqwest::Client;
use crate::tts::engine::TtsEngine;

#[derive(Clone)]
pub struct LocalTts {
    server_url: String,
    client: Client,
}

impl LocalTts {
    pub fn new(server_url: String) -> Self {
        Self {
            server_url,
            client: Client::new(),
        }
    }

    pub fn set_url(&mut self, url: String) -> Result<(), String> {
        if url.is_empty() {
            return Err("URL не может быть пустым".into());
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("URL должен начинаться с http:// или https://".into());
        }
        self.server_url = url;
        Ok(())
    }

    pub fn get_url(&self) -> &str {
        &self.server_url
    }
}

impl Default for LocalTts {
    fn default() -> Self {
        Self::new("http://localhost:5002".to_string())
    }
}

#[async_trait]
impl TtsEngine for LocalTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        if !self.is_configured() {
            return Err("TTSVoiceWizard не настроен. Укажите URL сервера.".into());
        }

        // TTSVoiceWizard API: GET /api/tts?text={text}
        let url = format!("{}/api/tts", self.server_url.trim_end_matches('/'));

        let response = self
            .client
            .get(&url)
            .query(&[("text", text)])
            .send()
            .await
            .map_err(|e| format!("Failed to connect to TTSVoiceWizard: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("TTSVoiceWizard request failed: {}", error_text));
        }

        let audio_data = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read audio data: {}", e))?
            .to_vec();

        Ok(audio_data)
    }

    fn is_configured(&self) -> bool {
        !self.server_url.is_empty()
    }

    fn name(&self) -> &str {
        "TTSVoiceWizard"
    }
}
```

**Step 2: Commit**

```bash
git add src-tauri/src/tts/local.rs
git commit -m "feat(tts): add LocalTts provider for TTSVoiceWizard"
```

---

## Task 5: Implement SileroTts (stub)

**Files:**
- Create: `src-tauri/src/tts/silero.rs`

**Step 1: Write SileroTts stub**

```rust
use async_trait::async_trait;
use crate::tts::engine::TtsEngine;

#[derive(Clone, Default)]
pub struct SileroTts;

#[async_trait]
impl TtsEngine for SileroTts {
    async fn synthesize(&self, _text: &str) -> Result<Vec<u8>, String> {
        Err("Silero Bot TTS еще не реализован".into())
    }

    fn is_configured(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "Silero Bot"
    }
}
```

**Step 2: Commit**

```bash
git add src-tauri/src/tts/silero.rs
git commit -m "feat(tts): add SileroTts stub provider"
```

---

## Task 6: Update AppState for multiple providers

**Files:**
- Modify: `src-tauri/src/state.rs`

**Step 1: Update imports and struct**

Change imports from:
```rust
use crate::tts::OpenAiTts;
```
To:
```rust
use crate::tts::{TtsProvider, TtsProviderType, openai::OpenAiTts, local::LocalTts, silero::SileroTts};
```

**Step 2: Update AppState struct fields**

Replace these fields:
```rust
pub openai_api_key: Arc<Mutex<Option<String>>>,
pub tts_client: Arc<Mutex<Option<OpenAiTts>>>,
pub voice: Arc<Mutex<String>>,
```

With:
```rust
pub tts_provider_type: Arc<Mutex<TtsProviderType>>,
pub tts_providers: Arc<Mutex<TtsProvider>>,
pub openai_api_key: Arc<Mutex<Option<String>>>,
pub openai_voice: Arc<Mutex<String>>,
pub local_tts_url: Arc<Mutex<String>>,
```

**Step 3: Update AppState::new()**

Replace initialization with:
```rust
pub fn new() -> Self {
    Self {
        event_sender: Arc::new(Mutex::new(None)),
        interception_enabled: Arc::new(Mutex::new(false)),
        current_text: Arc::new(Mutex::new(String::new())),
        current_layout: Arc::new(Mutex::new(InputLayout::English)),
        tts_provider_type: Arc::new(Mutex::new(TtsProviderType::OpenAi)),
        tts_providers: Arc::new(Mutex::new(TtsProvider::OpenAi(OpenAiTts::new(String::new())))),
        openai_api_key: Arc::new(Mutex::new(None)),
        openai_voice: Arc::new(Mutex::new("alloy".to_string())),
        local_tts_url: Arc::new(Mutex::new("http://localhost:5002".to_string())),
        floating_opacity: Arc::new(Mutex::new(90)),
        floating_bg_color: Arc::new(Mutex::new("#1e1e1e".to_string())),
        floating_clickthrough: Arc::new(Mutex::new(false)),
        hotkey_enabled: Arc::new(Mutex::new(true)),
    }
}
```

**Step 4: Add helper methods**

```rust
pub fn get_tts_provider_type(&self) -> TtsProviderType {
    *self.tts_provider_type.lock().unwrap()
}

pub fn set_tts_provider_type(&self, provider: TtsProviderType) {
    if let Ok(mut p) = self.tts_provider_type.lock() {
        *p = provider;
    }
    self.emit_event(AppEvent::TtsProviderChanged(provider));
}

pub fn init_openai_tts(&self, api_key: String) {
    let mut tts = OpenAiTts::new(api_key);
    tts.set_voice(self.get_openai_voice());
    if let Ok(mut providers) = self.tts_providers.lock() {
        *providers = TtsProvider::OpenAi(tts);
    }
}

pub fn init_local_tts(&self, url: String) {
    let tts = LocalTts::new(url);
    if let Ok(mut providers) = self.tts_providers.lock() {
        *providers = TtsProvider::Local(tts);
    }
}

pub fn get_openai_voice(&self) -> String {
    self.openai_voice.lock().unwrap().clone()
}

pub fn set_openai_voice(&self, voice: String) {
    if let Ok(mut v) = self.openai_voice.lock() {
        *v = voice;
    }
    // Reinitialize if OpenAI is current
    if let Ok(api_key) = self.openai_api_key.lock() {
        if let Some(key) = api_key.as_ref() {
            let mut tts = OpenAiTts::new(key.clone());
            tts.set_voice(voice);
            if let Ok(mut providers) = self.tts_providers.lock() {
                if matches!(*providers, TtsProvider::OpenAi(_)) {
                    *providers = TtsProvider::OpenAi(tts);
                }
            }
        }
    }
}

pub fn get_local_tts_url(&self) -> String {
    self.local_tts_url.lock().unwrap().clone()
}

pub fn set_local_tts_url(&self, url: String) {
    if let Ok(mut u) = self.local_tts_url.lock() {
        *u = url;
    }
}
```

**Step 5: Update events.rs**

Add new event:
```rust
pub enum AppEvent {
    // ... existing events
    TtsProviderChanged(TtsProviderType),
}
```

**Step 6: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/events.rs
git commit -m "refactor(state): update AppState for multiple TTS providers"
```

---

## Task 7: Update settings.rs for new structure

**Files:**
- Modify: `src-tauri/src/settings.rs`

**Step 1: Update AppSettings struct**

Add new fields:
```rust
pub struct AppSettings {
    // ... existing fields

    // Remove/replace these:
    // pub api_key: Option<String>,
    // pub voice: String,

    // With these:
    pub tts_provider: TtsProviderType,
    pub openai_api_key: Option<String>,
    pub openai_voice: String,
    pub local_tts_url: String,
}
```

**Step 2: Update AppSettings::default()**

```rust
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // ... existing defaults
            tts_provider: TtsProviderType::OpenAi,
            openai_api_key: None,
            openai_voice: "alloy".to_string(),
            local_tts_url: "http://localhost:5002".to_string(),
        }
    }
}
```

**Step 3: Update SettingsManager::load_from_state()**

```rust
pub fn load_from_state(state: &AppState) -> AppSettings {
    AppSettings {
        // ... existing fields
        tts_provider: state.get_tts_provider_type(),
        openai_api_key: state.openai_api_key.lock().unwrap().clone(),
        openai_voice: state.get_openai_voice(),
        local_tts_url: state.get_local_tts_url(),
        // ... rest of existing fields
    }
}
```

**Step 4: Update SettingsManager::apply_to_state()**

```rust
pub fn apply_to_state(&self, state: &AppState) {
    // ... existing applies

    state.set_tts_provider_type(self.settings.tts_provider);
    state.openai_api_key.lock().unwrap().clone_from(&self.settings.openai_api_key);
    state.set_openai_voice(self.settings.openai_voice.clone());
    state.set_local_tts_url(self.settings.local_tts_url.clone());

    // Initialize providers
    if let Some(ref key) = self.settings.openai_api_key {
        state.init_openai_tts(key.clone());
    }
    state.init_local_tts(self.settings.local_tts_url.clone());
}
```

**Step 5: Commit**

```bash
git add src-tauri/src/settings.rs
git commit -m "refactor(settings): update settings for multiple TTS providers"
```

---

## Task 8: Update Tauri commands

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Add new commands**

```rust
// Provider selection
#[tauri::command]
async fn get_tts_provider(state: State<'_, AppState>) -> TtsProviderType {
    state.get_tts_provider_type()
}

#[tauri::command]
async fn set_tts_provider(
    state: State<'_, AppState>,
    provider: TtsProviderType,
) -> Result<(), String> {
    // Validate configuration
    let providers = state.tts_providers.lock().await;
    match provider {
        TtsProviderType::Silero => {
            return Err("Silero Bot еще не реализован.".into());
        }
        _ => {}
    }
    drop(providers);

    state.set_tts_provider_type(provider);
    Ok(())
}

// Local TTS
#[tauri::command]
async fn get_local_tts_url(state: State<'_, AppState>) -> String {
    state.get_local_tts_url()
}

#[tauri::command]
async fn set_local_tts_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    // Validate URL
    if url.is_empty() {
        return Err("URL не может быть пустым".into());
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL должен начинаться с http:// или https://".into());
    }

    state.set_local_tts_url(url.clone());
    state.init_local_tts(url);

    // Auto-save settings
    if let Ok(manager) = state.settings_manager.lock() {
        let _ = manager.save_from_state(state);
    }

    Ok(())
}

// Rename OpenAI commands for clarity
#[tauri::command]
async fn set_openai_api_key(
    state: State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    // Validate API key
    if !key.starts_with("sk-") || key.len() < 20 {
        return Err("Неверный формат API ключа OpenAI".into());
    }

    state.openai_api_key.lock().await.map(|mut k| *k = Some(key.clone()))?;
    state.init_openai_tts(key);

    // Auto-save settings
    if let Ok(manager) = state.settings_manager.lock() {
        let _ = manager.save_from_state(state);
    }

    Ok(())
}

#[tauri::command]
async fn set_openai_voice(
    state: State<'_, AppState>,
    voice: String,
) -> Result<(), String> {
    const VOICES: &[&str] = &["alloy", "echo", "fable", "onyx", "nova", "shimmer"];
    if !VOICES.contains(&voice.as_str()) {
        return Err("Неверный голос".into());
    }

    state.set_openai_voice(voice.clone());

    // Auto-save settings
    if let Ok(manager) = state.settings_manager.lock() {
        let _ = manager.save_from_state(state);
    }

    Ok(())
}

#[tauri::command]
async fn get_openai_voice(state: State<'_, AppState>) -> String {
    state.get_openai_voice()
}

#[tauri::command]
async fn get_openai_api_key(state: State<'_, AppState>) -> Option<String> {
    state.openai_api_key.lock().await.ok().and_then(|k| k.clone())
}
```

**Step 2: Update lib.rs command registration**

Add new commands to `invoke_handler`:
```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    get_tts_provider,
    set_tts_provider,
    get_local_tts_url,
    set_local_tts_url,
    get_openai_api_key,
    set_openai_api_key,
    get_openai_voice,
    set_openai_voice,
    // ... rest of commands
])
```

**Step 3: Update speak_text command**

```rust
#[tauri::command]
async fn speak_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Текст не может быть пустым".into());
    }

    let providers = state.tts_providers.lock().await;
    let audio_data = providers.synthesize(&text).await?;
    drop(providers);

    let player = crate::tts::openai::AudioPlayer::new();
    player.play(audio_data)?;

    Ok(())
}
```

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(commands): add commands for multiple TTS providers"
```

---

## Task 9: Add async-trait dependency

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add async-trait dependency**

Add to `[dependencies]`:
```toml
async-trait = "0.1"
```

**Step 2: Run cargo check**

```bash
cd src-tauri && cargo check
```

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "deps: add async-trait for TtsEngine trait"
```

---

## Task 10: Update TtsPanel.vue - add provider state

**Files:**
- Modify: `src/components/TtsPanel.vue`

**Step 1: Update script setup**

Replace the existing script with:

```typescript
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke, listen } from '@tauri-apps/api/core';

type TtsProviderType = 'openai' | 'silero' | 'local';

interface TtsProviderState {
  type: TtsProviderType;
  configured: boolean;
  expanded: boolean;
}

// State
const activeProvider = ref<TtsProviderType>('openai');
const providers = ref<Record<TtsProviderType, TtsProviderState>>({
  openai: { type: 'openai', configured: false, expanded: false },
  silero: { type: 'silero', configured: false, expanded: false },
  local: { type: 'local', configured: false, expanded: false },
});

// OpenAI settings
const openaiApiKey = ref('');
const openaiVoice = ref('alloy');
const openaiVoices = ['alloy', 'echo', 'fable', 'onyx', 'nova', 'shimmer'];

// Local TTS settings
const localTtsUrl = ref('http://localhost:5002');

// Error state
const errorMessage = ref('');
let errorTimeout: number | null = null;

// Methods
function showError(message: string) {
  errorMessage.value = message;
  if (errorTimeout) clearTimeout(errorTimeout);
  errorTimeout = setTimeout(() => {
    errorMessage.value = '';
  }, 5000) as unknown as number;
}

function toggleProvider(provider: TtsProviderType) {
  providers.value[provider].expanded = !providers.value[provider].expanded;
}

async function saveOpenAiKey() {
  if (!openaiApiKey.value.trim()) {
    showError('API Key не может быть пустым');
    return;
  }
  try {
    await invoke('set_openai_api_key', { key: openaiApiKey.value });
    providers.value.openai.configured = true;
  } catch (error) {
    showError(error as string);
  }
}

async function saveOpenAiVoice() {
  try {
    await invoke('set_openai_voice', { voice: openaiVoice.value });
  } catch (error) {
    showError(error as string);
  }
}

async function saveLocalTtsUrl() {
  try {
    await invoke('set_local_tts_url', { url: localTtsUrl.value });
    providers.value.local.configured = true;
  } catch (error) {
    showError(error as string);
  }
}

async function setActiveProvider(provider: TtsProviderType) {
  try {
    await invoke('set_tts_provider', { provider });
    activeProvider.value = provider;
  } catch (error) {
    showError(error as string);
  }
}

// Load on mount
onMounted(async () => {
  try {
    const provider = await invoke<TtsProviderType>('get_tts_provider');
    activeProvider.value = provider;

    const apiKey = await invoke<string>('get_openai_api_key');
    if (apiKey) {
      openaiApiKey.value = apiKey;
      providers.value.openai.configured = true;
    }

    const voice = await invoke<string>('get_openai_voice');
    openaiVoice.value = voice;

    const localUrl = await invoke<string>('get_local_tts_url');
    localTtsUrl.value = localUrl;
    providers.value.local.configured = localUrl.length > 0;

    // Listen to TTS errors
    const unlisten = await listen('tts-error', (event) => {
      showError(event.payload as string);
    });
  } catch (error) {
    console.error('Failed to load TTS settings:', error);
  }
});

onUnmounted(() => {
  if (errorTimeout) clearTimeout(errorTimeout);
});
</script>
```

**Step 2: Commit**

```bash
git add src/components/TtsPanel.vue
git commit -m "feat(ui): add provider state to TtsPanel"
```

---

## Task 11: Update TtsPanel.vue - add template

**Files:**
- Modify: `src/components/TtsPanel.vue`

**Step 1: Replace template**

```vue
<template>
  <div class="tts-panel">
    <h2>TTS Настройки</h2>

    <!-- Error box -->
    <div class="error-box" v-if="errorMessage">
      {{ errorMessage }}
    </div>

    <!-- Provider cards -->
    <div class="provider-cards">

      <!-- OpenAI Card -->
      <div
        class="provider-card"
        :class="{
          'active': activeProvider === 'openai',
          'expanded': providers.openai.expanded
        }"
      >
        <div class="card-header" @click="toggleProvider('openai')">
          <input
            type="radio"
            name="tts-provider"
            value="openai"
            v-model="activeProvider"
            @click.stop="setActiveProvider('openai')"
          >
          <span class="card-title">OpenAI</span>
          <span class="expand-icon">{{ providers.openai.expanded ? '▼' : '▶' }}</span>
        </div>

        <div class="card-content" v-show="providers.openai.expanded">
          <div class="setting-group">
            <label>API Key</label>
            <input type="password" v-model="openaiApiKey" placeholder="sk-..." />
            <button @click="saveOpenAiKey">Сохранить</button>
          </div>

          <div class="setting-group">
            <label>Голос</label>
            <select v-model="openaiVoice" @change="saveOpenAiVoice">
              <option v-for="voice in openaiVoices" :key="voice">{{ voice }}</option>
            </select>
            <small>Доступные голоса: {{ openaiVoices.join(', ') }}</small>
          </div>
        </div>
      </div>

      <!-- Silero Bot Card -->
      <div
        class="provider-card"
        :class="{
          'active': activeProvider === 'silero',
          'expanded': providers.silero.expanded
        }"
      >
        <div class="card-header" @click="toggleProvider('silero')">
          <input
            type="radio"
            name="tts-provider"
            value="silero"
            v-model="activeProvider"
            @click.stop="setActiveProvider('silero')"
          >
          <span class="card-title">Silero Bot</span>
          <span class="expand-icon">{{ providers.silero.expanded ? '▼' : '▶' }}</span>
        </div>

        <div class="card-content" v-show="providers.silero.expanded">
          <div class="placeholder">
            Настройки будут добавлены позже
          </div>
        </div>
      </div>

      <!-- TTSVoiceWizard Card -->
      <div
        class="provider-card"
        :class="{
          'active': activeProvider === 'local',
          'expanded': providers.local.expanded
        }"
      >
        <div class="card-header" @click="toggleProvider('local')">
          <input
            type="radio"
            name="tts-provider"
            value="local"
            v-model="activeProvider"
            @click.stop="setActiveProvider('local')"
          >
          <span class="card-title">TTSVoiceWizard</span>
          <span class="expand-icon">{{ providers.local.expanded ? '▼' : '▶' }}</span>
        </div>

        <div class="card-content" v-show="providers.local.expanded">
          <div class="setting-group">
            <label>URL сервера</label>
            <input
              type="text"
              v-model="localTtsUrl"
              placeholder="http://localhost:5002"
            />
            <button @click="saveLocalTtsUrl">Сохранить</button>
          </div>
        </div>
      </div>

    </div>
  </div>
</template>
```

**Step 2: Commit**

```bash
git add src/components/TtsPanel.vue
git commit -m "feat(ui): add provider cards template to TtsPanel"
```

---

## Task 12: Update TtsPanel.vue - add styles

**Files:**
- Modify: `src/components/TtsPanel.vue`

**Step 1: Add styles**

Add to `<style scoped>`:

```css
.provider-cards {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.provider-card {
  border: 2px solid #444;
  border-radius: 8px;
  background: #2a2a2a;
  transition: all 0.2s ease;
}

.provider-card.active {
  border-color: #4CAF50;
  background: #2a3a2a;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  cursor: pointer;
  user-select: none;
}

.card-header:hover {
  background: #333;
}

.card-header input[type="radio"] {
  cursor: pointer;
}

.card-title {
  flex: 1;
  font-weight: 600;
  font-size: 16px;
}

.expand-icon {
  color: #888;
  font-size: 12px;
  transition: transform 0.2s ease;
}

.card-content {
  padding: 0 16px 16px;
  border-top: 1px solid #333;
  animation: slideDown 0.2s ease;
}

@keyframes slideDown {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.placeholder {
  padding: 24px;
  text-align: center;
  color: #666;
  font-style: italic;
}

.setting-group {
  margin-top: 16px;
}

.setting-group label {
  display: block;
  margin-bottom: 8px;
  color: #aaa;
  font-size: 14px;
}

.setting-group input[type="text"],
.setting-group input[type="password"],
.setting-group select {
  width: 100%;
  padding: 10px;
  background: #1a1a1a;
  border: 1px solid #444;
  border-radius: 4px;
  color: #fff;
  font-size: 14px;
  margin-bottom: 8px;
  box-sizing: border-box;
}

.setting-group input:focus,
.setting-group select:focus {
  outline: none;
  border-color: #4CAF50;
}

.setting-group button {
  padding: 8px 16px;
  background: #4CAF50;
  border: none;
  border-radius: 4px;
  color: #fff;
  cursor: pointer;
  font-size: 14px;
  transition: background 0.2s ease;
}

.setting-group button:hover {
  background: #45a049;
}

.setting-group small {
  display: block;
  margin-top: 4px;
  color: #666;
  font-size: 12px;
}

.error-box {
  background: #5a1a1a;
  border: 1px solid #a33;
  border-radius: 4px;
  padding: 12px;
  margin-bottom: 16px;
  color: #ff9999;
  animation: slideDown 0.2s ease;
}
```

**Step 2: Commit**

```bash
git add src/components/TtsPanel.vue
git commit -m "feat(ui): add styles for provider cards"
```

---

## Task 13: Test the implementation

**Files:**
- Test: Manual testing

**Step 1: Build and run**

```bash
npm run tauri dev
```

**Step 2: Test OpenAI provider**

1. Navigate to TTS Settings panel
2. Click OpenAI card to expand
3. Enter API key
4. Click "Сохранить"
5. Select voice from dropdown
6. Select OpenAI via radio button
7. Test with InputPanel or floating window

**Step 3: Test TTSVoiceWizard provider**

1. Click TTSVoiceWizard card to expand
2. Enter URL (e.g., http://localhost:5002)
3. Click "Сохранить"
4. Select via radio button
5. Should show error if server not running

**Step 4: Test Silero Bot**

1. Click Silero Bot card to expand
2. Should show placeholder
3. Selecting via radio should show error "не реализован"

**Step 5: Test provider switching**

1. Configure OpenAI
2. Try to switch to Silero - should error
3. Configure Local URL
4. Switch between OpenAI and Local - should work
5. Restart app - selection should persist

**Step 6: Verify settings persistence**

Check `settings.json` contains:
```json
{
  "tts_provider": "openai",
  "openai_api_key": "sk-...",
  "openai_voice": "alloy",
  "local_tts_url": "http://localhost:5002"
}
```

**Step 7: Commit if everything works**

```bash
git add .
git commit -m "test: verify multiple TTS providers implementation"
```

---

## Summary

This plan implements support for multiple TTS providers with:

1. **Backend:**
   - `tts/` module with `TtsEngine` trait
   - `TtsProvider` enum for abstraction
   - OpenAI (refactored), Local (TTSVoiceWizard), Silero (stub)
   - Updated AppState, settings, and commands

2. **Frontend:**
   - Collapsible provider cards
   - Radio button selection
   - Active card highlighting
   - Error handling and validation

3. **Storage:**
   - All settings persist to `settings.json`
   - Provider selection remembered across sessions

**Total estimated time:** 2-3 hours
**Total commits:** ~13
