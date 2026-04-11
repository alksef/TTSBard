# Code Optimizations

## Steps

### 1. Extract shared `parse_proxy_url` and `build_client` into `tts/proxy_utils.rs`

**Sources:** `src-tauri/src/tts/openai.rs`, `src-tauri/src/tts/fish.rs`

- Create `src-tauri/src/tts/proxy_utils.rs` with:
  - `parse_proxy_url(proxy_str: &str) -> Result<String, String>` — shared implementation (currently duplicated in both providers).
  - `build_client_with_proxy(proxy_url: Option<&str>, timeout: Duration) -> Result<reqwest::Client, String>` — shared client builder with proxy + timeout logic.
- Update `tts/mod.rs` to declare `pub mod proxy_utils`.
- Replace duplicated functions in `openai.rs` and `fish.rs` with calls to the shared module.

**Acceptance:** `cargo check` passes, no duplication of proxy parsing/client building code.

### 2. Add `update_*` methods to TTS providers to avoid full rebuild

**Sources:** `src-tauri/src/state.rs:345-502`

- In `OpenAiTts`, add methods: `update_voice(&mut self, voice: String)`, `update_proxy(&mut self, proxy: Option<String>)` — reconfigure client in-place instead of full replacement.
- In `FishTts`, add methods: `update_reference_id(&mut self, id: String)`, `update_proxy(&mut self, proxy: Option<String>)`.
- Refactor `AppState` methods (`set_fish_audio_reference_id`, `set_fish_audio_proxy`, `set_openai_voice`, `set_openai_proxy`) to use these `update_*` methods, reducing cloning and provider recreation.

**Acceptance:** `cargo check` passes, `set_*` methods no longer clone entire configs.

### 3. Debounce URL save in TtsLocalCard

**Source:** `src/components/tts/TtsLocalCard.vue:30-31`

- Replace immediate `emit('save', ...)` on `@input` with a debounced version (300ms).
- Add immediate save on `@blur` and `@keyup.enter`.
- Import or inline a simple debounce helper (lodash debounce or a small composable).

**Acceptance:** Typing in URL field does not trigger save on every keystroke; save fires on blur/enter or after 300ms idle.

### 4. Merge multiple watchers into one in TtsFishAudioCard

**Source:** `src/components/tts/TtsFishAudioCard.vue:56-70`

- Replace three separate `watch()` calls with a single `watch()` observing an array of sources: `[() => props.referenceId, () => props.model, () => props.voiceId]`.
- Inside the single watcher, apply the same sync logic currently split across three watchers.

**Acceptance:** Props still sync correctly to local state; one watcher instead of three.

### 5. Add image loading state to FishAudioModelPicker

**Source:** `src/components/tts/FishAudioModelPicker.vue:74-75`

- Add a reactive `imageLoadStates: Record<string, boolean>` (model reference ID -> loaded).
- In `loadImages`, set state to `false` on start, `true` on successful load.
- In template, show a placeholder/skeleton for images not yet loaded.

**Acceptance:** Users see placeholder while images load; loaded images display normally.

### 6. Remove unused settings load in `get_proxy_settings`

**Source:** `src-tauri/src/commands/proxy.rs:189`

- Remove the unused `_settings` variable and the `settings_manager.load()` call.

**Acceptance:** `cargo check` passes, no unused variable warning.
