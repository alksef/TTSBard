# Task 100 / Round 1 / Iteration 12a

## Goal

Add a backend command that selects an already registered TTS provider by its
stable concrete ID and persists that ID. This is the backend contract for the
later dynamic TTS panel.

## Allowed files

- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/lib.rs`

Do not modify frontend, settings structs, provider implementations, registry
internals, startup, or inference code.

## Requirements

1. Add a Tauri command such as `select_tts_provider_by_id(id: String)` taking
   `State<AppState>` and `State<SettingsManager>`.
2. Validate/select only an entry already present in `state.tts_registry`; return
   a clear error if the ID is unknown. Do not silently select the first entry.
3. After successful selection, persist the ID through
   `SettingsManager::set_tts_provider_id(Some(id))` using the existing blocking
   persistence helper, and emit the existing settings-changed event.
4. Do not trigger ONNX inference or add a second provider collection. Selection
   itself must remain cheap; Piper loading/warmup is a later task.
5. Register the command in the Tauri invoke handler in `lib.rs`.
6. Do not build Windows or run a Windows smoke test.

## Verification

- Confirm unknown IDs do not alter active selection or settings.
- Confirm the command uses the concrete ID, not `TtsProviderType`.
- Run the narrowest available Linux check and report system-library failures.
