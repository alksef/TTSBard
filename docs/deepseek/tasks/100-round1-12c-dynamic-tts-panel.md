# Task 100 / Round 1 / Iteration 12c

## Goal

Update the TTS panel to render discovered providers dynamically and support the
selection/loading/error flow for Piper models.

## Allowed files

- `src/types/settings.ts`
- `src/components/TtsPanel.vue`

Do not modify backend files or existing provider card components.

## Requirements

1. Add TypeScript DTO types for `provider_id` and runtime `providers`:
   `id`, `display_name`, `kind`, `active`.
2. Keep existing OpenAI/Silero/Local HTTP/Fish cards and behavior intact.
3. Render an additional simple card for every runtime provider with `kind ===
   "piper"`; use its stable ID and display name, not a hard-coded voice list.
   Do not add an “Add local model” button or grey missing-model cards.
4. When a Piper card is selected: set a loading state for that ID, call
   `select_tts_provider_by_id`, then `prepare_tts_provider_by_id`; on success
   show ready state, on failure show the existing error status and restore the
   previous active ID if possible. Ensure concurrent clicks cannot leave stale
   loading state.
5. Highlight the active Piper card from the DTO `active` flag / saved concrete
   ID. Keep legacy built-in selection routed through the existing
   `set_tts_provider` command; do not break current settings forms.
6. Use existing panel styling conventions and concise labels. No new external
   dependency.
7. Do not build Windows or run a Windows smoke test.

## Verification

- Run the frontend type check (`npm run type-check` or the project equivalent)
  if available; do not run Windows build.
- Review no hard-coded local model paths and no Add button.
- Confirm loading is cleared in `finally` and failures are visible.
