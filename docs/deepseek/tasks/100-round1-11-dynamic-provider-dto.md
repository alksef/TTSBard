# Task 100 / Round 1 / Iteration 11

## Goal

Expose the runtime TTS registry as serializable settings data so the frontend
can render built-in and Piper providers dynamically. Do not implement provider
selection or UI in this task.

## Allowed files

- `src-tauri/src/config/dto.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/lib.rs`

Do not modify provider implementations, registry internals, settings schema,
scanner/runtime, or frontend files.

## Requirements

1. Add a serializable `TtsProviderInfoDto` containing stable `id`,
   `display_name`, a simple provider `kind` string (`openai`, `local-http`,
   `silero`, `fish`, `piper`), and `active: bool`.
2. Add `providers: Vec<TtsProviderInfoDto>` to `TtsSettingsDto` with serde
   default. Existing settings conversion should initialize it as an empty
   vector; it is runtime data, not persisted settings.
3. In `get_all_app_settings`, populate `settings.tts.providers` from the
   current `AppState.tts_registry` after the normal settings conversion. Do
   not expose runtime provider objects or filesystem paths.
4. Register no new selection command yet. Keep this task read-only from the
   provider registry perspective.
5. Register/compile the changed command only as needed by the existing
   `get_all_app_settings` command; avoid unnecessary API changes.
6. Do not build Windows or run a Windows smoke test.

## Verification

- Review that every registry entry maps to exactly one DTO and active is based
  on the registry active ID.
- Confirm old settings JSON remains compatible because `providers` is runtime
  DTO data with a default.
- Run the narrowest available Linux check and report missing system libraries.
