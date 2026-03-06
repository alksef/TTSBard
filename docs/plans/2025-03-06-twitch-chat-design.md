# Twitch Chat Integration Design

**Status:** Approved
**Created:** 2025-03-06
**Author:** Claude (via brainstorming skill)

## Overview

Add ability to send TTS text to Twitch chat as a bot. Integration follows the same pattern as existing WebView Source: separate settings panel, enable/disable button, independent IRC connection from Telegram.

## Requirements

- ✅ Send to own channel as a bot
- ✅ Separate enable/disable button
- ✅ Independent IRC connection (not tied to Telegram)
- ✅ Manual OAuth token input
- ✅ Raw text (no preprocessing like num2words)

## Architecture

### Backend Structure (Rust)

```
src-tauri/src/
├── twitch/              # New module
│   ├── mod.rs           # TwitchSettings struct, exports
│   ├── client.rs        # IRC client (connection, PING/PONG, auto-reconnect)
│   └── sender.rs        # Message sending logic
├── commands/
│   └── twitch.rs        # Tauri commands for settings management
├── state.rs             # Add: twitch_settings: Arc<RwLock<TwitchSettings>>
├── events.rs            # Add: TwitchMessage(text) variant or reuse TextSentToTts
└── settings.rs          # Add: load/save_twitch_settings() methods
```

### Frontend Structure (Vue 3)

```
src/components/
└── TwitchPanel.vue      # Settings panel (mirrors WebViewPanel.vue structure)
```

## Components

### 1. TwitchSettings Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitchSettings {
    pub enabled: bool,
    pub username: String,      // Bot login name
    pub token: String,         // OAuth token (format: oauth:xxxxxxxx)
    pub channel: String,       // Target channel (usually = username)
    pub start_on_boot: bool,   // Auto-start on app launch
}
```

### 2. IRC Client (twitch/client.rs)

**Responsibilities:**
- Connect to `irc.chat.twitch.tv:6697` via TLS
- Authenticate with PASS/NICK commands
- Handle PING/PONG keepalive
- Auto-reconnect on disconnect with exponential backoff
- Run in dedicated tokio task

**Key functions:**
```rust
pub struct TwitchClient {
    settings: TwitchSettings,
    shutdown_tx: broadcast::Sender<()>,
}

impl TwitchClient {
    pub async fn start(settings: TwitchSettings) -> Result<Self>;
    pub async fn send_message(&self, text: &str) -> Result<()>;
    pub async fn stop(self);
}
```

### 3. Tauri Commands

```rust
// Get current Twitch settings
#[tauri::command]
pub async fn get_twitch_settings(
    state: State<'_, AppState>,
) -> Result<TwitchSettings, String>;

// Save settings and restart IRC if needed
#[tauri::command]
pub async fn save_twitch_settings(
    settings: TwitchSettings,
    state: State<'_, AppState>,
) -> Result<String, String>;

// Test connection with current settings
#[tauri::command]
pub async fn test_twitch_connection(
    settings: TwitchSettings,
) -> Result<String, String>;
```

### 4. Vue Component: TwitchPanel.vue

**UI Elements:**
- Start/Stop toggle buttons (controls `enabled`)
- Start on boot checkbox
- Input fields: username, token, channel
- Connection status indicator
- Test Connection button
- Help section with token generator link

**State management:**
```typescript
interface TwitchSettings {
  enabled: boolean
  username: string
  token: string
  channel: string
  start_on_boot: boolean
}

const settings = ref<TwitchSettings>({ ... })
const connectionStatus = ref<'disconnected' | 'connecting' | 'connected' | 'error'>('disconnected')
```

## Data Flow

```
User input → TTS synthesis
                ↓
    AppEvent::TextSentToTts(text)
                ↓
    ┌───────────┴───────────┐
    ↓                       ↓
WebView (existing)    Twitch (new)
broadcast via WS       send via IRC
```

**Twitch send flow:**
```rust
// In TTS modules (openai.rs, silero.rs, local.rs)
if let Some(tx) = &self.event_tx {
    let _ = tx.send(AppEvent::TextSentToTts(text.clone()));
}

// In lib.rs event loop
while let Ok(event) = rx.recv().await {
    match event {
        AppEvent::TextSentToTts(text) => {
            // WebView broadcast (existing)
            if let Some(webview) = &webview_server {
                webview.broadcast_text(text.clone()).await;
            }
            // Twitch send (new)
            if let Some(twitch) = &twitch_client {
                let _ = twitch.send_message(&text).await;
            }
        }
        _ => {}
    }
}
```

## Error Handling

| Situation | Action |
|-----------|--------|
| Invalid token format | Show error in UI, prevent save |
| Authentication failed | Show error, set enabled=false, log |
| Connection lost | Auto-reconnect with backoff (1s → 5s → 30s) |
| IRC not connected | Queue messages or drop with warning log |
| Empty username/channel | Validation error in UI |

## UI Design

```
┌─────────────────────────────────────────┐
│ Twitch Chat                             │
├─────────────────────────────────────────┤
│ Connection                              │
│ [▶ Start] [■ Stop]                      │
│ ☐ Start on boot                         │
│                                         │
│ Username: [_______________]             │
│ Token:    [_____________________]       │
│ Channel:  [_______________]             │
│                                         │
│ Status: Connected ✅                     │
│ [Test Connection]                       │
├─────────────────────────────────────────┤
│ Help                                    │
│ Get your token:                         │
│ https://twitchtokengenerator.com        │
│                                         │
│ Token format: oauth:xxxxxxxxxxxxxxx     │
│ (include "oauth:" prefix)               │
└─────────────────────────────────────────┘
```

## Dependencies (Cargo.toml)

```toml
[dependencies]
# Existing
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }

# New for Twitch IRC
tokio-native-tls = "0.3"    # TLS for IRC connection
```

## Implementation Notes

1. **IRC Protocol:**
   - Connect via TLS to `irc.chat.twitch.tv:6697`
   - Send: `PASS oauth:token\r\nNICK username\r\nJOIN #channel\r\n`
   - Respond to `PING` with `PONG :tmi.twitch.tv\r\n`
   - Send messages: `PRIVMSG #channel :text\r\n`

2. **Message formatting:**
   - Remove newlines: `text.replace('\n', ' ').replace('\r', '')`
   - Max length: 500 chars (Twitch limit)

3. **Lifecycle:**
   - App starts → load settings → if `start_on_boot` && `enabled`, connect
   - Settings save → if `enabled` changed, restart IRC client
   - App shutdown → graceful disconnect

4. **Status updates:**
   - Emit Tauri events: `twitch-status-changed` with status
   - Vue listens and updates UI accordingly

## Testing Checklist

- [ ] Manual token entry works
- [ ] Connection succeeds with valid credentials
- [ ] Connection fails gracefully with invalid token
- [ ] Messages appear in Twitch chat
- [ ] Start/Stop buttons work correctly
- [ ] Auto-reconnect on disconnect
- [ ] start_on_boot persists and works
- [ ] Test connection button reports correctly
- [ ] Long messages (>500 chars) handled
- [ ] Special characters in text work

## Next Steps

See implementation plan: `2025-03-06-twitch-chat.md`
