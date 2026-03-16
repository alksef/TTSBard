# Proxy Settings Implementation Plan

**Date:** 2026-03-15
**Issue:** Add SOCKS5 proxy support for OpenAI TTS and Telegram Silero bot

## Background

Current status:
- **grammers** has moved to Codeberg: https://codeberg.org/Lonami/grammers
- **grammers-mtsender now supports SOCKS5/SOCKS4/HTTP proxy** (via `proxy` feature, PR #10 merged)
- OpenAI TTS already has HTTP proxy support (`proxy_host`, `proxy_port` in `openai.rs`)
- Telegram Silero uses `grammers-client` which can now use proxy through `grammers-mtsender`
- MTProxy is a different protocol (mtproto://) and requires separate implementation

**Goal:** Implement SOCKS5 proxy for both TTS providers using native grammers support.

---

## Requirements

### Frontend UI

**TTS Panel → Proxy Settings** (after Audio, new section)

```
┌─────────────────────────────────────────────┐
│ 🌐 Proxy Settings                            │
├─────────────────────────────────────────────┤
│                                              │
│ SOCKS5 Proxy                                 │
│ ┌─────────────────────────────────────────┐ │
│ │ URL:   [socks5://proxy.com:1080      ] │ │
│ │        [user:pass@host:port supported]  │ │
│ │        [Test Connection (3s)           ] │ │
│ └─────────────────────────────────────────┘ │
│                                              │
│ MTProxy (Telegram protocol)                  │
│ ┌─────────────────────────────────────────┐ │
│ │ URL:   [mtproto://secret@host:port   ] │ │
│ │        [Test Connection (3s)           ] │ │
│ └─────────────────────────────────────────┘ │
│                                              │
│ Telegram DC (Advanced)                       │
│ ┌─────────────────────────────────────────┐ │
│ │ Server: [Auto ▼]                       │ │
│ │ Options: Auto, DC1, DC2, DC3, DC4      │ │
│ └─────────────────────────────────────────┘ │
└─────────────────────────────────────────────┘
```

**OpenAI Panel**

```
┌─────────────────────────────────────────────┐
│ OpenAI TTS                                  │
├─────────────────────────────────────────────┤
│ API Key:    [sk-...]                        │
│ Voice:      [alloy ▼]                       │
│ Proxy:      ☐ Use SOCKS5 proxy              │
└─────────────────────────────────────────────┘
```

**Silero (Telegram Bot) Panel**

```
┌─────────────────────────────────────────────┐
│ Silero TTS (Telegram Bot)                   │
├─────────────────────────────────────────────┤
│ API ID:     [12345                         ] │
│ Phone:      [+7999...                      ] │
│                                             │
│ Proxy:      [Нет ▼]                         │
│              Options: Нет, SOCKS5, MTProxy  │
│                                             │
│ Status:     ● Connected via SOCKS5          │
│              [Reconnect]                    │
└─────────────────────────────────────────────┘
```

---

## Data Structures

### Rust Backend

```rust
// In config/settings.rs

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProxyType {
    #[serde(rename = "socks5")]
    Socks5,
    #[serde(rename = "socks4")]
    Socks4,
    #[serde(rename = "http")]
    Http,
}

pub struct ProxySettings {
    // Unified proxy URL (type is encoded in scheme)
    pub proxy_url: Option<String>,   // socks5://, socks4://, http://user:pass@host:port
    pub proxy_type: ProxyType,        // For UI selection
    pub telegram_dc: Option<String>,  // "auto" или "dc1.telegram.org:443"
}

// In config/dto.rs

pub struct ProxySettingsDto {
    pub proxy_url: Option<String>,
    pub proxy_type: ProxyType,
    pub telegram_dc: Option<String>,
}

// In TtsSettings

pub struct TtsSettings {
    pub provider: TtsProviderType,
    pub openai: OpenAiSettings {
        pub api_key: Option<String>,
        pub voice: String,
        pub use_proxy: bool,  // ← Add
    },
    pub telegram: TelegramTtsSettings {
        pub api_id: Option<i64>,
        pub proxy_mode: ProxyMode,  // ← Add
    },
    pub proxy: ProxySettings,  // ← Add
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProxyMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "socks5")]
    Socks5,
    #[serde(rename = "mtproxy")]
    MtProxy,
}
```

---

## Implementation Plan

### Phase 1: Backend - Proxy Support for OpenAI

**File:** `src-tauri/src/tts/openai.rs`

1. Update `build_client()` to support all proxy types:
   - Parse proxy URL to determine type (SOCKS5/SOCKS4/HTTP)
   - Use `reqwest::Proxy::custom()` for SOCKS5/SOCKS4
   - Use `reqwest::Proxy::all()` for HTTP

**Dependencies:** None (reqwest already supports SOCKS5/SOCKS4/HTTP via Proxy::custom)

---

### Phase 2: Backend - Proxy Support for Silero

**File:** `src-tauri/src/telegram/client.rs`

**Update dependency in `Cargo.toml`:**
```toml
grammers-client = { version = "0.8.0", features = ["proxy"] }
grammers-mtsender = { version = "0.8.0", features = ["proxy"] }
```

**Implementation for SOCKS5 (supported):**

```rust
use grammers_client::client::client::Config;
use grammers_mtsender::proxy;

pub async fn init_with_socks5(
    &self,
    api_id: u32,
    api_hash: String,
    phone: String,
    socks5_url: String,
) -> Result<OperationResult, String> {

    // Parse SOCKS5 URL
    let proxy = self.parse_socks5_url(&socks5_url)?;

    // Create Config with proxy
    let config = Config::builder()
        .proxy(proxy)
        .build();

    // Initialize client with proxy configuration
    let client = Client::connect(config)
        .await
        .map_err(|e| e.to_string())?;

    // ... rest of initialization
}

fn parse_socks5_url(&self, url: &str) -> Result<proxy::Proxy, String> {
    // Parse socks5:// URL and create grammers SOCKS5 proxy
}
```

**Note for MTProxy (not supported yet):**
- MTProxy requires separate implementation (mtproto:// protocol)
- Blocked by: codeberg.org/Lonami/grammers/issues/331
- Currently only SOCKS5 is supported via grammers native proxy feature

---

### Phase 3: Backend - Test Connection

**New file:** `src-tauri/src/commands/proxy.rs`

```rust
use std::time::Instant;
use reqwest::Proxy;

/// Test proxy connection (all types)
#[tauri::command]
pub async fn test_proxy(url: String, proxy_type: ProxyType) -> Result<TestResultDto, String> {
    let start = Instant::now();

    // Build proxy based on type
    let proxy = match proxy_type {
        ProxyType::Socks5 => Proxy::custom(move |url| {
            let proxy_url = url.clone();
            async move {
                // Use socks5 proxy for all requests
                Ok(proxy_url)
            }
        }),
        ProxyType::Socks4 => Proxy::custom(move |url| {
            let proxy_url = url.clone();
            async move {
                // Use socks4 proxy for all requests
                Ok(proxy_url)
            }
        }),
        ProxyType::Http => Proxy::all(&url)?,
    };

    // Test connection to Telegram
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;

    let response = client
        .get("https://telegram.org")
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let latency = start.elapsed();
            Ok(TestResultDto {
                success: true,
                latency_ms: latency.as_millis() as u64,
                mode: format!("{:?}", proxy_type),
                error: None,
            })
        }
        Ok(resp) => {
            Ok(TestResultDto {
                success: false,
                latency_ms: start.elapsed().as_millis() as u64,
                mode: format!("{:?}", proxy_type),
                error: Some(format!("HTTP status: {}", resp.status())),
            })
        }
        Err(e) => {
            Ok(TestResultDto {
                success: false,
                latency_ms: start.elapsed().as_millis() as u64,
                mode: format!("{:?}", proxy_type),
                error: Some(e.to_string()),
            })
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct TestResultDto {
    success: bool,
    latency_ms: u64,
    mode: String,
    error: Option<String>,
}
```

---

### Phase 4: Backend - Settings Commands

**File:** `src-tauri/src/commands/proxy.rs` (continuation)

```rust
/// Get proxy settings
#[tauri::command]
pub fn get_proxy_settings(settings_manager: State<'_, SettingsManager>) -> Result<ProxySettingsDto, String> {
    settings_manager.load()
        .map(|s| s.proxy)
        .map_err(|e| e.to_string())
}

/// Set proxy URL and type
#[tauri::command]
pub fn set_proxy_url(url: String, proxy_type: ProxyType, settings_manager: State<'_, SettingsManager>) -> Result<(), String> {
    settings_manager.set_proxy_url(url, proxy_type)
        .map_err(|e| e.to_string())
}

/// Set Telegram DC server
#[tauri::command]
pub fn set_telegram_dc(dc: Option<String>, settings_manager: State<'_, SettingsManager>) -> Result<(), String> {
    settings_manager.set_telegram_dc(dc)
        .map_err(|e| e.to_string())
}

/// Set OpenAI use proxy
#[tauri::command]
pub fn set_openai_use_proxy(enabled: bool, settings_manager: State<'_, SettingsManager>) -> Result<(), String> {
    settings_manager.set_openai_use_proxy(enabled)
        .map_err(|e| e.to_string())
}

/// Set Telegram proxy mode
#[tauri::command]
pub fn set_telegram_proxy_mode(mode: ProxyMode, settings_manager: State<'_, SettingsManager>) -> Result<(), String> {
    settings_manager.set_telegram_proxy_mode(mode)
        .map_err(|e| e.to_string())
}

/// Reconnect Telegram with new proxy
#[tauri::command]
pub async fn reconnect_telegram(state: State<'_, AppState>) -> Result<String, String> {
    // Full disconnect + reconnect
    telegram_client.disconnect().await?;

    // Get settings
    let settings_manager = SettingsManager::new()?;
    let settings = settings_manager.load()?;

    // Reconnect based on proxy_mode
    match settings.telegram.proxy_mode {
        ProxyMode::Socks5 => {
            if let Some(url) = settings.proxy.proxy_url {
                telegram_client.init_with_socks5(...).await?;
            }
        }
        ProxyMode::MtProxy => {
            // Not implemented yet - blocked by issue #331
            return Err("MTProxy not supported yet".to_string());
        }
        ProxyMode::None => {
            telegram_client.init_empty(...).await?;
        }
    }

    Ok("Reconnected successfully".to_string())
}
```

---

### Phase 5: Backend - Settings Manager

**File:** `src-tauri/src/config/settings.rs`

Add methods:
```rust
impl SettingsManager {
    pub fn set_proxy_url(&self, url: String, proxy_type: ProxyType) -> Result<()> {
        self.update_field("/proxy/proxy_url", &url)?;
        self.update_field("/proxy/proxy_type", &proxy_type)
    }

    pub fn get_proxy_url(&self) -> Option<String> {
        self.cache.read().proxy.proxy_url.clone()
    }

    pub fn get_proxy_type(&self) -> ProxyType {
        self.cache.read().proxy.proxy_type.clone()
    }

    pub fn set_telegram_dc(&self, dc: Option<String>) -> Result<()> {
        self.update_field("/proxy/telegram_dc", &dc)
    }

    pub fn set_openai_use_proxy(&self, enabled: bool) -> Result<()> {
        self.update_field("/tts/openai/use_proxy", &enabled)
    }

    pub fn set_telegram_proxy_mode(&self, mode: ProxyMode) -> Result<()> {
        self.update_field("/tts/telegram/proxy_mode", &mode)
    }
}
```

---

### Phase 6: Frontend - Proxy Panel Component

**New file:** `src/components/ProxyPanel.vue`

Sections:
1. Proxy type selector (SOCKS5, SOCKS4, HTTP)
2. URL input field with scheme prefix (socks5://, socks4://, http://)
3. Test Connection button
4. MTProxy info section (disabled, with explanation)
5. Telegram DC selector (Auto, DC1-4)
6. Status indicators

---

### Phase 7: Frontend - TTS Panels Update

**OpenAI Panel:**
- Add checkbox "Use proxy"

**Silero Panel:**
- Add proxy selector [Нет, SOCKS5, MTProxy]
- Add Test Connection button
- Add Reconnect button
- Add status indicator

---

## URLs and Formats

### OpenAI TTS Proxy URL Formats

**SOCKS5:**
```
socks5://host:port
socks5://username:password@host:port
```

**SOCKS4:**
```
socks4://host:port
socks4://username:password@host:port
```

**HTTP CONNECT:**
```
http://host:port
http://username:password@host:port
https://host:port
https://username:password@host:port
```

**Examples:**
- `socks5://127.0.0.1:1080`
- `socks5://user:pass@proxy.com:1080`
- `socks4://10.0.0.1:1080`
- `http://proxy.example.com:8080`
- `https://user:pass@proxy.example.com:8080`

### Telegram Silero Proxy URL Formats

**SOCKS5:**
```
socks5://host:port
socks5://username:password@host:port
```

**MTProxy:**
```
mtproto://secret@host:port
```

**Examples:**
- `socks5://127.0.0.1:1080`
- `socks5://user:pass@proxy.com:1080`
- `mtproto://secret@mtproxy.com:443`

### Telegram DC Servers

```
auto    → Let grammers decide
dc1     → dc1.telegram.org:443 (or IP)
dc2     → 149.154.167.50:443
dc3     → 149.154.175.50:443
dc4     → 149.154.175.51:443
```

---

## Error Handling

### Test Connection Errors

**Proxy Tests (all types):**
- Invalid URL → "Invalid proxy URL format"
- Connection timeout → "Proxy connection timeout"
- Auth failed → "Proxy authentication failed"
- Success → "Connected via {type} (145ms)"

**Show:** Under test button as status text

---

### Retry Logic

- Max retries: 2
- Timeout: 5 seconds per test
- Exponential backoff not needed for tests

---

## Dependencies

### Backend (Cargo.toml)

```toml
[dependencies]
# Existing - no new dependencies needed
reqwest = { version = "0.12", features = ["json"] }  # Already supports SOCKS5/SOCKS4/HTTP

# Update grammers to enable proxy feature:
grammers-client = { version = "0.8.0", features = ["proxy"] }
grammers-mtsender = { version = "0.8.0", features = ["proxy"] }
```

### Frontend (package.json)

No new dependencies needed.

---

## Future Work: MTProxy

**Blocked by:** codeberg.org/Lonami/grammers/issues/331

**When issue #331 is resolved:**
1. Add MTProxy URL field (mtproto://secret@host:port)
2. Implement MTProxy handshake test
3. Add "MtProxy" option to Silero proxy selector
4. Update grammers-client dependency

---

## Testing Plan

### Unit Tests

1. `parse_proxy_url()` — validate all URL formats (SOCKS5, SOCKS4, HTTP)
2. ProxySettingsDto serialization
3. SettingsManager proxy methods
4. ProxyType enum serialization

### Integration Tests

1. OpenAI TTS with SOCKS5 proxy
2. OpenAI TTS with SOCKS4 proxy
3. OpenAI TTS with HTTP proxy
4. Telegram Silero with SOCKS5 proxy
5. Telegram Silero with SOCKS4 proxy
6. Telegram Silero with HTTP proxy
7. Proxy connection test (all types)
8. Telegram reconnect with proxy

### Manual Testing

1. Test with local SOCKS5 proxy (ssh -D 1080)
2. Test with local SOCKS4 proxy
3. Test with HTTP CONNECT proxy
4. Verify OpenAI TTS works through all proxy types
5. Verify Telegram Silero works through all proxy types

---

## Migration Path

**Existing OpenAI proxy (HTTP):**
- Current: `proxy_host`, `proxy_port` fields (HTTP only)
- New: Use `proxy_url` and `proxy_type` (SOCKS5/SOCKS4/HTTP)
- Migration: Convert old HTTP format to new unified format

**Telegram:**
- No proxy support currently
- Add native grammers proxy support (SOCKS5/SOCKS4/HTTP) via `proxy` feature

---

## Notes

- **grammers** moved from GitHub to Codeberg: https://codeberg.org/Lonami/grammers
- **grammers-mtsender** now supports SOCKS5/SOCKS4/HTTP proxy via `proxy` feature (PR #10)
- MTProxy (mtproto://) is a different protocol and not supported yet (issue #331)
- Standard proxy support (SOCKS5/SOCKS4/HTTP) is sufficient for most use cases
- Telegram DC selection is advanced feature (rarely needed)
- Passwords stored in plaintext in settings.json (acceptable for desktop app)

---

**Next Steps:**

1. Update grammers dependencies to enable `proxy` feature
2. Implement Phase 1 (Backend - OpenAI proxy for all types)
3. Implement Phase 2 (Backend - Telegram proxy via grammers)
4. Implement Phase 3 (Backend - Test connection)
5. Implement Phase 4 (Backend - Settings commands)
6. Implement Phase 5 (Backend - SettingsManager)
7. Implement Phase 6 (Frontend - Proxy Panel)
8. Implement Phase 7 (Frontend - TTS Panels)
9. Testing and refinement

**Estimated effort:** 2-3 hours for Phase 1-2, 1-2 hours for remaining phases
