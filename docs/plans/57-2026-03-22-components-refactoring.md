# Plan: Components Refactoring - Settings & Shared Components

**Plan ID:** 57-2026-03-22
**Date:** 2026-03-22

## Summary

Refactor `src/components/SettingsPanel.vue` (1,481 lines) by extracting all tab sections into separate files and creating reusable shared components. This will improve code maintainability and reduce duplication.

## Current State Analysis

### SettingsPanel.vue Structure (1,481 lines)
- **Lines 1-31:** Imports & tab state
- **Lines 36-455:** Network/Proxy state & functions (SOCKS5 + MTProxy)
- **Lines 457-561:** General/Editor/Logging functions
- **Lines 564-886:** Template with 4 tabs
- **Lines 889-1481:** Styles (all tabs combined)

### SettingsAiPanel.vue Structure (882 lines)
- Already separate component
- Contains OpenAI and Z.ai provider cards
- Similar patterns to network settings (input toggle, status messages)

### Identified Duplications
1. **Input with toggle visibility** - repeated 5+ times
2. **Provider card pattern** - repeated in SettingsAiPanel, TtsPanel
3. **Status message toast** - repeated across panels
4. **Network form rows** - repeated pattern

## Implementation Plan

### Phase 1: Create Shared Components (Priority: HIGH)

#### 1.1 Create `src/components/shared/` directory
```bash
src/components/shared/
├── InputWithToggle.vue      # Password input with show/hide eye icon
├── ProviderCard.vue         # Expandable card with radio selector
├── StatusMessage.vue        # Fixed position toast notification
└── TestResult.vue           # Connection test result display
```

#### 1.2 Implement shared components

**InputWithToggle.vue** (~80 lines)
- Props: `modelValue`, `type` (default: "password"), `placeholder`
- Emits: `update:modelValue`
- Features: Eye/EyeOff icon toggle button
- Used in: SettingsPanel (network), SettingsAiPanel

**ProviderCard.vue** (~100 lines)
- Props: `title`, `icon`, `active`, `expanded`
- Slots: `default` (card content)
- Features: Radio button, expand/collapse, active state styling
- Used in: SettingsAiPanel, TtsPanel

**StatusMessage.vue** (~60 lines)
- Props: `message`, `type` ('success' | 'error' | 'info')
- Features: Auto-hide after 3s, dismiss button, fixed positioning
- Used in: SettingsPanel, SettingsAiPanel

**TestResult.vue** (~50 lines)
- Props: `result` (TestResult | null)
- Features: Success/error styling, icon, latency display
- Used in: SettingsPanel network tests

### Phase 2: Extract SettingsPanel Sections (Priority: HIGH)

#### 2.1 Create `src/components/settings/` directory
```bash
src/components/settings/
├── SettingsGeneral.vue      # Theme, capture, logging
├── SettingsEditor.vue       # Quick editor toggle
└── SettingsNetwork.vue      # SOCKS5 + MTProxy
```

#### 2.2 Implement extracted components

**SettingsGeneral.vue** (~200 lines)
- Move from SettingsPanel.vue lines 596-678
- Theme selector (dark/light)
- Exclude from capture toggle
- Logging enable + level select
- Uses composables: `useGeneralSettings`, `useWindowsSettings`, `useLoggingSettings`

**SettingsEditor.vue** (~100 lines)
- Move from SettingsPanel.vue lines 680-698
- Quick editor toggle
- Uses composable: `useEditorSettings`

**SettingsNetwork.vue** (~450 lines)
- Move from SettingsPanel.vue lines 36-455 (state) + 700-880 (template)
- SOCKS5 configuration (host, port, username, password)
- MTProxy configuration (host, port, secret, DC ID)
- Connection testing for both
- Uses shared: `InputWithToggle`, `StatusMessage`, `TestResult`
- Backend commands: `get_proxy_settings`, `set_proxy_url`, `test_proxy`, `get_mtproxy_settings`, `set_mtproxy_settings`, `test_mtproxy`

#### 2.3 Refactor SettingsPanel.vue to container (~300 lines)
```vue
<script setup lang="ts">
import { ref } from 'vue'
import { AlertTriangle, Moon, Sun, Settings, Network, Type, Sparkles } from 'lucide-vue-next'
import type { Tab } from '../types/settings'
import SettingsGeneral from './settings/SettingsGeneral.vue'
import SettingsEditor from './settings/SettingsEditor.vue'
import SettingsNetwork from './settings/SettingsNetwork.vue'
import SettingsAiPanel from './SettingsAiPanel.vue'

const activeTab = ref<Tab>('general')
</script>

<template>
  <div class="settings-panel">
    <!-- Tabs Navigation -->
    <div class="settings-tabs">...</div>

    <!-- Tab Contents -->
    <SettingsGeneral v-show="activeTab === 'general'" />
    <SettingsEditor v-show="activeTab === 'editor'" />
    <SettingsNetwork v-show="activeTab === 'network'" />
    <SettingsAiPanel v-show="activeTab === 'ai'" />
  </div>
</template>

<style scoped>
/* Only tab navigation and shared styles */
</style>
```

### Phase 3: Update Imports & Dependencies

#### 3.1 Update SettingsAiPanel.vue to use shared components
- Replace `input-with-toggle` custom implementation with `InputWithToggle`
- Replace `status-message` with `StatusMessage`
- Extract provider card pattern to use `ProviderCard` (optional)

### Phase 4: Verify & Test

#### 4.1 Build validation
```bash
npm run check       # TypeScript checks
npm run build       # Production build
```

#### 4.2 Manual testing checklist
- [ ] General tab: theme switching works
- [ ] General tab: capture exclusion toggle works
- [ ] General tab: logging enable/disable + level change works
- [ ] Editor tab: quick editor toggle works
- [ ] Network tab: SOCKS5 save/test works
- [ ] Network tab: MTProxy save/test works
- [ ] Network tab: password visibility toggles work
- [ ] AI tab: OpenAI settings work
- [ ] AI tab: Z.ai settings work
- [ ] All status messages display correctly
- [ ] Tab switching is smooth

## File Structure After Refactoring

```
src/components/
├── shared/                           # NEW - Shared components
│   ├── InputWithToggle.vue           # NEW
│   ├── ProviderCard.vue              # NEW
│   ├── StatusMessage.vue             # NEW
│   └── TestResult.vue                # NEW
├── settings/                         # NEW - Settings subfolder
│   ├── SettingsGeneral.vue           # NEW
│   ├── SettingsEditor.vue            # NEW
│   └── SettingsNetwork.vue           # NEW
├── SettingsPanel.vue                 # MODIFIED - Container only (~300 lines)
├── SettingsAiPanel.vue               # MODIFIED - Use shared components
├── TtsPanel.vue
├── AudioPanel.vue
├── ... (other components unchanged)
```

## Lines of Code Impact

| File | Before | After | Change |
|------|--------|-------|--------|
| SettingsPanel.vue | 1,481 | ~300 | -1,181 |
| SettingsAiPanel.vue | 882 | ~750 | -132 |
| SettingsGeneral.vue | 0 | ~200 | +200 |
| SettingsEditor.vue | 0 | ~100 | +100 |
| SettingsNetwork.vue | 0 | ~450 | +450 |
| InputWithToggle.vue | 0 | ~80 | +80 |
| ProviderCard.vue | 0 | ~100 | +100 |
| StatusMessage.vue | 0 | ~60 | +60 |
| TestResult.vue | 0 | ~50 | +50 |
| **Total** | **2,363** | **2,090** | **-273** |

**Note:** While total LOC decreases slightly, the main benefit is separation of concerns - each file has a single, clear responsibility.

## Critical Files to Modify

1. **src/components/SettingsPanel.vue** - Refactor to container
2. **src/components/SettingsAiPanel.vue** - Use shared components
3. **src/components/shared/InputWithToggle.vue** - NEW
4. **src/components/shared/ProviderCard.vue** - NEW
5. **src/components/shared/StatusMessage.vue** - NEW
6. **src/components/shared/TestResult.vue** - NEW
7. **src/components/settings/SettingsGeneral.vue** - NEW
8. **src/components/settings/SettingsEditor.vue** - NEW
9. **src/components/settings/SettingsNetwork.vue** - NEW

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking existing functionality | HIGH | Thorough testing of each tab after refactoring |
| Styling inconsistencies | MEDIUM | Copy styles from original, test both themes |
| Props/events mismatch | MEDIUM | Use TypeScript interfaces, test all interactions |
| Import path issues | LOW | Update all imports, verify build |

## Verification Commands

```bash
# TypeScript check
npm run check

# Build check
npm run build

# Development server for manual testing
npm run dev
```

## Phase 5: TtsPanel.vue Refactoring (Priority: HIGH)

### Current State Analysis

### TtsPanel.vue Structure (1,644 lines)
- **Lines 1-100:** Imports, composables, state
- **Lines 100-250:** TTS provider state management
- **Lines 250-400:** OpenAI TTS configuration
- **Lines 400-450:** Custom status message (duplicate)
- **Lines 450-550:** Telegram connection status section
- **Lines 550-650:** Voice selection and settings
- **Lines 650-700:** Custom input-with-toggle (duplicate)
- **Lines 700-850:** Provider-specific settings sections
- **Lines 850-1050:** More status messages and connection UI
- **Lines 1050-1644:** Styles (massive, with duplicates)

### Identified Issues
1. **Custom status message** (lines 446-450, 1031-1076) - should use `StatusMessage`
2. **Custom input-with-toggle** (lines 637-653, 886-915) - should use `InputWithToggle`
3. **Provider card patterns** - only OpenAI uses `ProviderCard`, others have custom markup
4. **Telegram connection status** - ~300 lines, could be separate component
5. **Proxy settings section** - duplicated from SettingsNetwork
6. **Massive style section** - many duplicates with other files

### 5.1 Create TTS-specific components directory
```bash
src/components/tts/
├── TtsProviderCard.vue       # Generic provider card wrapper
├── TtsSileroCard.vue         # Silero provider settings
├── TtsLocalCard.vue          # Local provider settings
├── TtsOpenAICard.vue         # OpenAI provider settings
├── TtsElevenLabsCard.vue     # ElevenLabs provider settings
└── TelegramConnectionStatus.vue  # Telegram status section
```

### 5.2 Implement TTS provider cards

**TtsProviderCard.vue** (~60 lines)
- Wrapper component that uses `ProviderCard`
- Adds TTS-specific features: voice select, test button, settings toggle
- Props: `provider`, `active`, `expanded`, `voiceId`, `available`
- Emits: `select`, `toggle`, `test`, `voice-change`

**TtsSileroCard.vue** (~150 lines)
- Silero-specific: model select, speaker select, sample rate
- Uses `TtsProviderCard` as base
- Backend: `set_tts_provider`, `set_tts_silero_model`, `set_tts_silero_speaker`

**TtsLocalCard.vue** (~120 lines)
- Local-specific: URL input with validation, test button
- Uses `InputWithToggle` for URL field
- Backend: `set_tts_provider`, `set_tts_local_url`, `test_local_tts`

**TtsOpenAICard.vue** (~180 lines)
- OpenAI-specific: API key, model select, voice select, proxy toggle
- Uses `InputWithToggle` for API key
- Backend: `set_tts_provider`, `set_tts_openai_api_key`, `set_tts_openai_model`, `set_tts_openai_voice`, `set_tts_openai_use_proxy`

**TtsElevenLabsCard.vue** (~200 lines)
- ElevenLabs-specific: API key, model select, voice select, latency
- Uses `InputWithToggle` for API key
- Backend: `set_tts_provider`, `set_tts_elevenlabs_api_key`, `set_tts_elevenlabs_model`, `set_tts_elevenlabs_voice`, `set_tts_elevenlabs_latency`

### 5.3 Extract Telegram connection status

**TelegramConnectionStatus.vue** (~300 lines)
- Props: `connected`, `statusMessage`, `userPhone`, `reconnecting`
- Emits: `connect`, `disconnect`, `refresh-status`
- Features: Connection button, status indicator, phone display
- Backend: `get_telegram_status`, `telegram_connect`, `telegram_disconnect`

### 5.4 Create shared TTS components

**VoiceSelector.vue** (~80 lines)
- Props: `voices`, `selectedVoiceId`, `loading`
- Emits: `voice-change`, `refresh`
- Features: Select dropdown, refresh button, loading state

**ModelSelector.vue** (~60 lines)
- Props: `models`, `selectedModel`, `loading`
- Emits: `model-change`
- Features: Select dropdown, loading state

### 5.5 Refactor TtsPanel.vue to container (~400 lines)

```vue
<script setup lang="ts">
import { ref, computed } from 'vue'
import { useTtsSettings } from '../composables/useAppSettings'
import TtsSileroCard from './tts/TtsSileroCard.vue'
import TtsLocalCard from './tts/TtsLocalCard.vue'
import TtsOpenAICard from './tts/TtsOpenAICard.vue'
import TtsElevenLabsCard from './tts/TtsElevenLabsCard.vue'
import TelegramConnectionStatus from './tts/TelegramConnectionStatus.vue'
import StatusMessage from './shared/StatusMessage.vue'

const ttsSettings = useTtsSettings()
const activeProvider = ref<TtsProvider>('silero')
const statusMessage = ref('')
const statusType = ref<'success' | 'error'>('error')

// ... state management
</script>

<template>
  <div class="tts-panel">
    <StatusMessage :message="statusMessage" :type="statusType" @dismiss="statusMessage = ''" />

    <TelegramConnectionStatus
      :connected="telegramConnected"
      :statusMessage="telegramStatus"
      @connect="connectTelegram"
      @disconnect="disconnectTelegram"
    />

    <div class="provider-cards">
      <TtsSileroCard
        :active="activeProvider === 'silero'"
        :expanded="providers.silero.expanded"
        :voiceId="sileroVoiceId"
        :available="providers.silero.available"
        @select="setProvider('silero')"
        @toggle="toggleProvider('silero')"
        @test="testTts('silero')"
        @voice-change="setSileroVoice"
      />

      <TtsLocalCard
        :active="activeProvider === 'local'"
        :expanded="providers.local.expanded"
        :url="localUrl"
        @select="setProvider('local')"
        @toggle="toggleProvider('local')"
        @test="testTts('local')"
        @url-change="setLocalUrl"
      />

      <TtsOpenAICard
        :active="activeProvider === 'openai'"
        :expanded="providers.openai.expanded"
        :apiKey="openaiApiKey"
        :model="openaiModel"
        :voice="openaiVoice"
        :useProxy="openaiUseProxy"
        @select="setProvider('openai')"
        @toggle="toggleProvider('openai')"
        @test="testTts('openai')"
        @api-key-change="setOpenAIKey"
      />

      <TtsElevenLabsCard
        :active="activeProvider === 'elevenlabs'"
        :expanded="providers.elevenlabs.expanded"
        :apiKey="elevenlabsApiKey"
        :model="elevenlabsModel"
        :voice="elevenlabsVoice"
        @select="setProvider('elevenlabs')"
        @toggle="toggleProvider('elevenlabs')"
        @test="testTts('elevenlabs')"
        @api-key-change="setElevenLabsKey"
      />
    </div>
  </div>
</template>

<style scoped>
/* Only shared styles, card-specific styles moved to components */
</style>
```

### 5.6 Replace duplicates with shared components
- Replace custom status message (lines 446-450, 1031-1076) → `StatusMessage`
- Replace custom input-with-toggle (lines 637-653, 886-915) → `InputWithToggle`

### 5.7 Extract common styles to shared CSS

**src/components/shared/styles/tts.css** (~200 lines)
```css
/* TTS-specific shared styles */
.tts-panel { ... }
.provider-cards { ... }
.voice-selector { ... }
.model-selector { ... }
```

## File Structure After TtsPanel Refactoring

```
src/components/
├── shared/
│   ├── InputWithToggle.vue       # ✓ Already created
│   ├── ProviderCard.vue          # ✓ Already created
│   ├── StatusMessage.vue         # ✓ Already created
│   ├── TestResult.vue            # ✓ Already created
│   └── styles/
│       └── tts.css               # NEW - TTS shared styles
├── tts/                          # NEW - TTS-specific components
│   ├── TtsProviderCard.vue       # NEW
│   ├── TtsSileroCard.vue         # NEW
│   ├── TtsLocalCard.vue          # NEW
│   ├── TtsOpenAICard.vue         # NEW
│   ├── TtsElevenLabsCard.vue     # NEW
│   ├── TelegramConnectionStatus.vue  # NEW
│   ├── VoiceSelector.vue         # NEW
│   └── ModelSelector.vue         # NEW
├── settings/
│   ├── SettingsGeneral.vue       # ✓ Already created
│   ├── SettingsEditor.vue        # ✓ Already created
│   └── SettingsNetwork.vue       # ✓ Already created
├── SettingsPanel.vue             # ✓ Already refactored
├── SettingsAiPanel.vue           # ✓ Already refactored
├── TtsPanel.vue                  # MODIFIED - Container only (~400 lines)
└── ... (other components)
```

## Lines of Code Impact (TtsPanel Only)

| File | Before | After | Change |
|------|--------|-------|--------|
| TtsPanel.vue | 1,644 | ~400 | -1,244 |
| TtsProviderCard.vue | 0 | ~60 | +60 |
| TtsSileroCard.vue | 0 | ~150 | +150 |
| TtsLocalCard.vue | 0 | ~120 | +120 |
| TtsOpenAICard.vue | 0 | ~180 | +180 |
| TtsElevenLabsCard.vue | 0 | ~200 | +200 |
| TelegramConnectionStatus.vue | 0 | ~300 | +300 |
| VoiceSelector.vue | 0 | ~80 | +80 |
| ModelSelector.vue | 0 | ~60 | +60 |
| tts.css (shared styles) | 0 | ~200 | +200 |
| **Total** | **1,644** | **1,750** | **+106** |

**Note:** Total LOC increases slightly, but each file is now focused and maintainable. Main benefit: separation of concerns, easier testing, reusable components.

## Testing Checklist (TtsPanel)

- [ ] Silero provider: model/speaker selection works
- [ ] Silero provider: test TTS works
- [ ] Local provider: URL input/save works
- [ ] Local provider: test TTS works
- [ ] OpenAI provider: API key save works
- [ ] OpenAI provider: model/voice selection works
- [ ] OpenAI provider: test TTS works
- [ ] OpenAI provider: proxy toggle works
- [ ] ElevenLabs provider: API key save works
- [ ] ElevenLabs provider: model/voice selection works
- [ ] ElevenLabs provider: test TTS works
- [ ] Telegram connection: connect button works
- [ ] Telegram connection: status updates correctly
- [ ] Telegram connection: disconnect button works
- [ ] Provider switching: switches correctly
- [ ] Status messages: display correctly for all actions
- [ ] All cards: expand/collapse works
- [ ] All themes: dark/light mode works

## Next Steps (Future Work)

After completing TtsPanel refactoring:
1. Extract SoundPanelTab.vue bindings/dialog to separate components
2. Apply similar refactoring to AudioPanel.vue (~700 lines)
3. Apply similar refactoring to TwitchPanel.vue (~667 lines)
4. Apply similar refactoring to WebViewPanel.vue (~744 lines)
5. Consider extracting form validation logic to composable
6. Create unified settings management system

---

**Dependencies:** Phase 1-4 (shared components must exist first)
**Blocking:** None
**Estimated complexity:** High (many interconnected components, careful state management required)
