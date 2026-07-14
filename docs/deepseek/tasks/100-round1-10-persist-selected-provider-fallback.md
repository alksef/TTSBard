# Task 100 / Round 1 / Iteration 10

## Goal

Persist the concrete TTS provider ID and restore it at startup with a safe
fallback when the saved provider is unavailable (for example, a deleted Piper
model). Keep the legacy provider enum for backward compatibility.

## Allowed files

- `src-tauri/src/config/settings.rs`
- `src-tauri/src/config/dto.rs`
- `src-tauri/src/setup.rs`
- `src-tauri/src/tts/registry.rs`

Do not modify commands, frontend, scanner, runtime, or provider implementations.

## Requirements

1. Add an optional serialized field to `TtsSettings` for the concrete selected
   provider ID, with serde default so old settings files remain valid. Use a
   clear name such as `provider_id` and default it to `None`.
2. Mirror this field in `TtsSettingsDto` and both conversion implementations,
   preserving `None` for old clients/settings.
3. Add a `SettingsManager` setter/getter for the selected provider ID using the
   existing persistence/update mechanism.
4. After built-in TTS initialization and Piper discovery in `setup.rs`, restore
   the saved `provider_id` if it exists in the registry. If it is missing or
   no saved ID exists while no provider is active, select the first registered
   provider. Never select a Piper provider merely because it was discovered if
   a built-in provider is already active and no saved ID requests Piper.
5. Keep fallback in-memory for now; do not rewrite the saved invalid ID during
   startup. Do not change the legacy `provider` enum semantics.
6. Keep registry API small and deterministic. A helper accepting
   `Option<&str>` is acceptable if needed; do not duplicate provider storage.
7. Do not build Windows or run a Windows smoke test.

## Verification

- Check serde compatibility for a settings file without `provider_id`.
- Review startup ordering: init built-in -> discover/register Piper -> restore
  saved concrete ID/fallback -> backend ready.
- Run the narrowest available Linux check and report environment failures.
