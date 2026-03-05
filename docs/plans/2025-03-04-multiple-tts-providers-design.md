# Multiple TTS Providers - Design Document

**Date:** 2025-03-04
**Status:** Approved

## Overview

Add support for multiple TTS providers with a collapsible UI for configuration. Initially implement OpenAI (existing), Silero Bot (stub), and TTSVoiceWizard (HTTP API).

## Requirements Summary

| Provider | Settings | Status |
|----------|----------|--------|
| OpenAI | API Key, Voice (existing) | ✅ Implemented |
| Silero Bot | None (placeholder) | 🔜 Future |
| TTSVoiceWizard | Server URL | 🆕 New |

## UI Requirements

- 3 collapsible cards for each provider
- Radio button to select active provider
- Click card header to expand/collapse settings
- Active card highlighted (green border/background)
- Independent radio and expand behavior
- Remember last selected provider
- Show error if trying to use unconfigured provider

## Backend Architecture

### Module Structure

```
src-tauri/src/tts/
├── mod.rs           # TtsProvider enum, exports
├── engine.rs        # TtsEngine trait
├── openai.rs        # OpenAI implementation (refactored from tts.rs)
├── local.rs         # TTSVoiceWizard HTTP API
└── silero.rs        # Silero Bot stub
```

### TtsEngine Trait

```rust
#[async_trait]
pub trait TtsEngine: Send + Sync {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String>;
    fn is_configured(&self) -> bool;
    fn name(&self) -> &str;
}
```

### TtsProvider Enum

```rust
pub enum TtsProvider {
    OpenAi(OpenAiTts),
    Silero(SileroTts),
    Local(LocalTts),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TtsProviderType {
    OpenAi,
    Silero,
    Local,
}
```

### State Changes

```rust
pub struct AppState {
    // Replace: openai_api_key, tts_client, voice
    // With:
    pub tts_provider_type: Arc<Mutex<TtsProviderType>>,
    pub tts_providers: Arc<Mutex<TtsProvider>>,
}
```

## Frontend Components

### TtsPanel.vue Structure

```typescript
type TtsProviderType = 'openai' | 'silero' | 'local';

interface TtsProviderState {
  type: TtsProviderType;
  configured: boolean;
  expanded: boolean;
}

const activeProvider = ref<TtsProviderType>('openai');
const providers = ref<Record<TtsProviderType, TtsProviderState>>({
  openai: { type: 'openai', configured: false, expanded: false },
  silero: { type: 'silero', configured: false, expanded: false },
  local: { type: 'local', configured: false, expanded: false },
});
```

### New Tauri Commands

```typescript
// Provider selection
invoke('get_tts_provider')           // → 'openai' | 'silero' | 'local'
invoke('set_tts_provider', { provider })

// Local TTS
invoke('get_local_tts_url')          // → string
invoke('set_local_tts_url', { url })

// OpenAI (renamed for clarity)
invoke('set_openai_api_key', { key })
invoke('set_openai_voice', { voice })
```

## Error Handling

- Validate settings before switching provider
- Return error if trying to switch to unconfigured provider
- Show error in UI with auto-dismiss (5 seconds)
- Silero always returns "not implemented" error

## Storage

All settings in `settings.json`:

```json
{
  "tts_provider": "openai",
  "openai_api_key": "sk-...",
  "openai_voice": "alloy",
  "local_tts_url": "http://localhost:5002"
}
```

## Implementation Order

1. Create tts/ module structure
2. Implement TtsEngine trait
3. Refactor OpenAI to new structure
4. Implement LocalTts (TTSVoiceWizard)
5. Implement SileroTts (stub)
6. Update AppState and settings
7. Add Tauri commands
8. Update TtsPanel.vue UI
9. Add error handling
10. Test all providers
