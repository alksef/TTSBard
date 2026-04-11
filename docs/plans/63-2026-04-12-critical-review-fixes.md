# Critical Review Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix all 7 critical issues found in review-016-2026-04-12: panics on log/runtime creation, unsafe string slicing, duplicate SettingsManager I/O, potential deadlocks in AppState, and event listener leak.

**Architecture:** Each task is independent and can be committed separately. Tasks are ordered from highest risk (panics) to lowest risk (style). All changes are backward-compatible.

**Tech Stack:** Rust (Tauri 2.0, tokio, parking_lot), TypeScript (Vue 3 Composition API)

---

### Task 1: Fix log file open panic (lib.rs)

**Files:**
- Modify: `src-tauri/src/lib.rs:118-166`

**Step 1: Extract log file open into a helper closure**

Replace the two `.expect("Failed to open log file")` blocks with a fallback to `std::io::sink()`. The debug branch (lines 120-125) and release branch (lines 162-166) both need the same fix.

In the **debug branch** (lines 120-125), replace:

```rust
let log_file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log_dir.join("ttsbard.log"))
    .expect("Failed to open log file");
```

With:

```rust
let log_file = match std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log_dir.join("ttsbard.log"))
{
    Ok(f) => f,
    Err(e) => {
        eprintln!("Failed to open log file: {}. Logging to stdout only.", e);
        std::io::sink()
    }
};
```

In the **release branch** (lines 162-166), replace:

```rust
let log_file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log_dir.join("ttsbard.log"))
    .expect("Failed to open log file");
```

With the same pattern as above.

**Step 2: Build to verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors.

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: prevent panic when log file cannot be opened"
```

---

### Task 2: Fix env_filter parse panic (lib.rs)

**Files:**
- Modify: `src-tauri/src/lib.rs:112`

**Step 1: Replace expect with error handling**

Replace:

```rust
env_filter = env_filter.add_directive(directive.parse().expect("Invalid log directive"));
```

With:

```rust
match directive.parse() {
    Ok(d) => env_filter = env_filter.add_directive(d),
    Err(e) => warn!(directive = %directive, error = %e, "Invalid log directive in settings, skipping"),
}
```

**Step 2: Build to verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors.

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: prevent panic on invalid log directive in settings"
```

---

### Task 3: Fix tokio runtime panics in setup.rs (4 locations)

**Files:**
- Modify: `src-tauri/src/setup.rs:350-354`, `setup.rs:367-372`, `setup.rs:394-399`, `setup.rs:412-417`

All four locations follow the same pattern inside `thread::spawn`. Replace each `.expect(...)` with error logging.

**Step 1: Fix webview server runtime (line 350-354)**

Replace:

```rust
let rt = tokio::runtime::Builder::new_multi_thread()
    .thread_stack_size(8 * 1024 * 1024)
    .enable_all()
    .build()
    .expect("Failed to create tokio runtime for webview");
```

With:

```rust
let rt = match tokio::runtime::Builder::new_multi_thread()
    .thread_stack_size(8 * 1024 * 1024)
    .enable_all()
    .build()
{
    Ok(rt) => rt,
    Err(e) => {
        tracing::error!(error = %e, "Failed to create tokio runtime for webview");
        return;
    }
};
```

**Step 2: Fix webview autostart runtime (line 367-372)**

Same pattern — replace `.expect("Failed to create WebView autostart runtime")` with `match` + `tracing::error!` + `return`.

**Step 3: Fix Twitch client runtime (line 394-399)**

Same pattern — replace `.expect("Failed to create Twitch tokio runtime")` with `match` + `tracing::error!` + `return`.

**Step 4: Fix Twitch autostart runtime (line 412-417)**

Same pattern — replace `.expect("Failed to create Twitch autostart runtime")` with `match` + `tracing::error!` + `return`.

**Step 5: Build to verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors.

**Step 6: Commit**

```bash
git add src-tauri/src/setup.rs
git commit -m "fix: prevent panic on tokio runtime creation in spawned threads"
```

---

### Task 4: Fix unsafe api_key string slicing (state.rs + openai.rs)

**Files:**
- Modify: `src-tauri/src/state.rs:258`
- Modify: `src-tauri/src/tts/openai.rs:115`

**Step 1: Fix state.rs:258**

Replace:

```rust
info!(key_prefix = %&api_key[..7], "init_openai_tts called");
```

With:

```rust
info!(key_prefix = %&api_key[..7.min(api_key.len())], "init_openai_tts called");
```

**Step 2: Verify openai.rs:115 is already safe**

Line 115 already uses `7.min(self.api_key.len())` — no change needed. Confirm by reading the line.

**Step 3: Build to verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src-tauri/src/state.rs
git commit -m "fix: prevent panic on short api_key in init_openai_tts log"
```

---

### Task 5: Eliminate duplicate SettingsManager::new() + load() in speak_text_internal

**Files:**
- Modify: `src-tauri/src/commands/mod.rs:75-212`

**Step 1: Refactor speak_text_internal to load settings once**

The function creates `SettingsManager::new()` + `.load()` at line 104-108 (for AI check) and again at line 177-181 (for audio settings). Refactor to create once at the top and reuse.

Replace the entire function body from `// === STAGE 2.5:` through the audio settings loading with a single settings load at the top of the function.

New structure:

```rust
pub async fn speak_text_internal(state: &AppState, text: String) -> Result<(), String> {
    info!(text, "Starting TTS");

    if text.trim().is_empty() {
        return Err("Текст не может быть пустым".to_string());
    }

    // Load settings once for all stages
    let settings_manager = SettingsManager::new()
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // === STAGE 1: Parse prefixes ===
    let prefix_result = crate::preprocessor::parse_prefix(&text);
    let text = prefix_result.text;

    if prefix_result.skip_twitch || prefix_result.skip_webview {
        debug!(skip_twitch = prefix_result.skip_twitch, skip_webview = prefix_result.skip_webview, "Prefix flags");
    }

    // === STAGE 2: Replacements (existing) ===
    let text = if let Some(preprocessor) = state.get_preprocessor() {
        let processed = preprocessor.process(&text);
        if processed != text {
            debug!(text, processed, "Replacements");
        }
        processed
    } else {
        text
    };

    // === STAGE 2.5: AI Text Correction (if enabled) ===
    let text = {
        if settings.editor.ai {
            match state.get_or_create_ai_client(&settings.ai, &settings.tts.network) {
                Ok(client) => {
                    match client.correct(&text, &settings.ai.prompt).await {
                        Ok(corrected) => {
                            if corrected != text {
                                tracing::info!(
                                    original = text.len(),
                                    corrected = corrected.len(),
                                    "AI correction applied"
                                );
                            }
                            corrected
                        }
                        Err(e) => {
                            tracing::warn!("AI correction failed, using original text: {}", e);
                            text
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("AI client not available, skipping correction: {}", e);
                    text
                }
            }
        } else {
            text
        }
    };
    tracing::debug!(text, "Text after AI correction stage");

    // === STAGE 3: Numbers to text ===
    let text = crate::preprocessor::process_numbers(&text);
    debug!(text, "Final text for TTS");

    // Store flags for event handlers
    state.set_prefix_flags(prefix_result.skip_twitch, prefix_result.skip_webview);

    // Get the current TTS provider
    let provider = {
        let providers = state.tts_providers.lock();

        providers.as_ref()
            .ok_or_else(|| {
                error!("TTS provider not initialized");
                debug!(provider = ?state.get_tts_provider_type(), "Provider type");
                "TTS provider не инициализирован. Выберите провайдер в настройках.".to_string()
            })?
            .clone()
    };

    // Synthesize audio
    let audio_data = provider.synthesize(&text).await
        .map_err(|e| {
            error!(error = %e, "synthesize() error");
            e
        })?;
    debug!(bytes = audio_data.len(), "Audio synthesized");

    // Send message event immediately before playback (synchronized with audio)
    state.emit_event(AppEvent::TextSentToTts(text.clone()));

    // Use already-loaded audio settings
    let audio_settings = settings.audio;

    // Build speaker config
    let speaker_config = if audio_settings.speaker_enabled {
        Some(OutputConfig {
            device_id: audio_settings.speaker_device,
            volume: audio_settings.speaker_volume as f32 / 100.0,
        })
    } else {
        None
    };

    // Build virtual mic config
    let virtual_mic_config = audio_settings.virtual_mic_device.map(|device_id| OutputConfig {
        device_id: Some(device_id),
        volume: audio_settings.virtual_mic_volume as f32 / 100.0,
    });

    // Check at least one output is enabled
    if speaker_config.is_none() && virtual_mic_config.is_none() {
        return Err("Аудиовывод и виртуальный микрофон выключены. Включите хотя бы один вывод.".to_string());
    }

    // Play audio with dual output support (use cached devices if available)
    let mut player = AudioPlayer::new();
    let cached_devices = Some(state.cached_devices.clone());
    player.play_mp3_async_dual(audio_data, speaker_config, virtual_mic_config, cached_devices)?;

    info!("TTS completed successfully");

    Ok(())
}
```

**Step 2: Build to verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors.

**Step 3: Commit**

```bash
git add src-tauri/src/commands/mod.rs
git commit -m "fix: eliminate duplicate SettingsManager load in TTS pipeline"
```

---

### Task 6: Fix potential deadlock in AppState setter methods

**Files:**
- Modify: `src-tauri/src/state.rs:343-502`

The four methods (`set_fish_audio_reference_id`, `set_fish_audio_proxy`, `set_openai_voice`, `set_openai_proxy`) acquire `tts_config.read()` + `tts_providers.lock()` in one block, then release and re-acquire in another. The fix: acquire all needed data in a single lock scope, then do the work outside locks.

The current code already releases locks before re-acquiring them. The real risk is if two threads call these methods concurrently and interleave. Fix by extracting the common pattern into a helper that minimizes lock hold time.

**Step 1: Add a helper method to AppState**

Add this private method to `impl AppState` (before the setters):

```rust
/// Read all Fish TTS config + provider in a single lock scope.
/// Returns None if fish_api_key is not set.
fn read_fish_config(&self) -> Option<(String, String, String, f32, u32, Option<Sender<AppEvent>>, Option<TtsProvider>)> {
    let config = self.tts_config.read();
    let api_key = config.fish_api_key.clone()?;
    Some((
        api_key,
        config.fish_proxy_url.clone(),
        config.fish_format.clone(),
        config.fish_temperature,
        config.fish_sample_rate,
        self.get_event_sender(),
        self.tts_providers.lock().clone(),
    ))
}
```

**Step 2: Simplify set_fish_audio_reference_id using the helper**

Replace lines 343-380:

```rust
pub fn set_fish_audio_reference_id(&self, reference_id: String) {
    if let Some((api_key, proxy_url, format, temperature, sample_rate, event_tx, current_provider)) = self.read_fish_config() {
        if matches!(current_provider.as_ref(), Some(TtsProvider::Fish(_))) {
            let mut tts = FishTts::new(api_key);
            tts.set_reference_id(reference_id.clone());
            tts.set_format(format);
            tts.set_temperature(temperature);
            tts.set_sample_rate(sample_rate);
            if let Some(url) = proxy_url {
                tts.set_proxy(Some(url));
            }
            if let Some(tx) = event_tx {
                tts = tts.with_event_tx(tx);
            }
            *self.tts_providers.lock() = Some(TtsProvider::Fish(tts));
        }
    }
    self.tts_config.write().fish_reference_id = reference_id;
}
```

**Step 3: Simplify set_fish_audio_proxy using the helper**

Replace lines 382-418 similarly, using `self.read_fish_config()`.

**Step 4: Add similar helper for OpenAI and simplify set_openai_voice / set_openai_proxy**

```rust
/// Read all OpenAI config + provider in a single lock scope.
/// Returns None if openai_key is not set.
fn read_openai_config(&self) -> Option<(String, String, Option<String>, Option<Sender<AppEvent>>, Option<TtsProvider>)> {
    let config = self.tts_config.read();
    let api_key = config.openai_key.clone()?;
    Some((
        api_key,
        config.openai_voice.clone(),
        config.openai_proxy_url.clone(),
        self.get_event_sender(),
        self.tts_providers.lock().clone(),
    ))
}
```

Apply to `set_openai_voice` (lines 433-466) and `set_openai_proxy` (lines 469-502) using the same single-scope pattern.

**Step 5: Build to verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors.

**Step 6: Commit**

```bash
git add src-tauri/src/state.rs
git commit -m "fix: reduce deadlock risk in AppState TTS setter methods"
```

---

### Task 7: Fix HotkeysPanel showError timeout leak

**Files:**
- Modify: `src/components/HotkeysPanel.vue:192-210`

Note: The event listener cleanup in `onUnmounted` (line 217-230) is already implemented correctly. The remaining issue is the `setTimeout` in `showError` (line 206) which is never cleared on unmount.

**Step 1: Add timeout ref and clear it on unmount**

Add a ref for the timeout ID after the existing refs (after line 12):

```typescript
let messageTimeoutId: ReturnType<typeof setTimeout> | null = null
```

Replace the `showError` function:

```typescript
function showError(msg: string) {
  errorMessage.value = msg

  // Determine message type
  if (msg.includes('Ошибка') || msg.includes('ошибка') || msg.includes('Error') || msg.includes('Failed')) {
    messageState.value = 'error'
  } else if (msg.includes('сохранен') || msg.includes('сохранена') || msg.includes('Saved') || msg.includes('Сброшено')) {
    messageState.value = 'success'
  } else if (msg.includes('Перезапустите') || msg.includes('перезапустите')) {
    messageState.value = 'warning'
  } else {
    messageState.value = null
  }

  if (messageTimeoutId !== null) {
    clearTimeout(messageTimeoutId)
  }
  messageTimeoutId = setTimeout(() => {
    errorMessage.value = null
    messageState.value = null
    messageTimeoutId = null
  }, 3000)
}
```

In the `onUnmounted` hook, add timeout cleanup before the existing code:

```typescript
onUnmounted(async () => {
  if (messageTimeoutId !== null) {
    clearTimeout(messageTimeoutId)
    messageTimeoutId = null
  }

  document.removeEventListener('keydown', handleKeyDown)
  // ... rest of existing onUnmounted code
})
```

**Step 2: Build to verify**

Run: `npm run build` (or the project's TypeScript check command)
Expected: No type errors.

**Step 3: Commit**

```bash
git add src/components/HotkeysPanel.vue
git commit -m "fix: clear message timeout on HotkeysPanel unmount"
```

---

## Verification

After all tasks are complete:

1. **Rust build**: `cargo build --manifest-path src-tauri/Cargo.toml`
2. **TypeScript check**: `npx vue-tsc --noEmit`
3. **Full build**: `npm run tauri build` (or `npm run tauri dev` for smoke test)
4. Run `/code-review-changes` to verify all changes against the plan
