# Implement Audio tabs, effect cards, draft/save workflow, and file preview

## Read first

- `docs/stage/22-audio-effects-navigation-and-preview.md`
- `docs/deepseek/WORKFLOW.md`
- relevant current Vue/Rust audio settings, playback, effects, and file-dialog code

## User-approved UX

Move audio effects out of TTS and into the main Audio section. The Audio section must have internal tabs **Устройства** and **Эффекты**.

The Effects tab contains:

1. Preview card for selecting a local audio file and playing **Оригинал** or **С эффектами**.
2. Separate **Преобразование голоса** card with pitch, speed, volume and an explicit enabled/apply toggle. The toggle is required so users can see whether these effects apply. Disabled controls remain visible and retain values.
3. Separate **Очистка шума — DeepFilterNet** card with its own toggle and attenuation/depth control. State that the model is embedded and requires no download.
4. A distinct **Сохранить** button and dirty/saved/error feedback.

## Required state semantics

- Controls edit a frontend draft; changing controls must not immediately persist global settings.
- Preview with effects immediately uses the current unsaved draft, including both enable toggles and all values.
- Real TTS continues using last saved backend settings until Save is clicked.
- Save persists all effect settings atomically if practical; avoid a sequence that can leave partially saved state. Add a dedicated backend command/settings-manager operation if needed.
- Original preview bypasses all effects.
- Do not persist the selected test-file path.

## Backend and playback requirements

- Preview must use the same decoding and effect-processing implementation/order as real TTS, not a frontend approximation.
- Support at least WAV and MP3 using existing project capabilities.
- Do not mutate the source file.
- Only one preview may play/process at once; a new request must stop or supersede the previous one.
- Provide Stop. Pause is optional for the first version if the existing player architecture makes it disproportionate.
- Play through the configured speaker output. Do not route preview to virtual microphone in this iteration.
- Use a specialized audio-file picker/filter rather than broadening unrelated command semantics where reasonable.
- Keep error messages user-facing and in Russian.

## UI/structure requirements

- Remove `AudioEffectsPanel` rendering and effect state handlers from `TtsPanel`; remove the component if no longer used.
- Preserve the established application visual language and theme variables. Do not introduce a visually unrelated design system.
- Tabs must be accessible buttons with an obvious active state.
- The Effects view must fit the current panel width and remain usable at narrow supported widths.
- Avoid a waveform in this iteration.

## Compatibility and safety

- Preserve existing saved settings and serde defaults.
- Preserve unrelated working-tree changes. Do not reset, clean, commit, or reformat unrelated files.
- The recent DeepFilterNet compatibility fixes in `Cargo.toml` and `audio/effects.rs` must remain intact.
- Do not trust task checklist completion; actually run validation.

## Validation and acceptance

- `npx vue-tsc --noEmit` passes with zero warnings/errors.
- `cargo check --manifest-path src-tauri/Cargo.toml` passes with zero warnings/errors.
- `scripts/build-debug.bat` exits 0 with no project warnings/errors.
- Existing device configuration/testing still works under the Devices tab.
- Effects no longer render in TTS.
- Draft changes affect preview immediately but do not alter backend settings until Save.
- Save updates settings used by subsequent real TTS.
- Original/effected preview and Stop work without concurrent playback leaks.

At completion report changed files, architectural decisions, and exact validation results.
