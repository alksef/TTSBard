# Task 100 / Round 1 / Iteration 12d

## Goal

Review and correct the current dynamic Piper TTS panel selection flow without
changing its scope.

## Allowed files

- `src/components/TtsPanel.vue`
- `src/types/settings.ts`

## Required review fixes

1. Ensure only one provider card is visually active: when a Piper provider is
   active, legacy built-in cards must not remain highlighted from the legacy
   `provider` enum.
2. Track the concrete active ID from `tts.provider_id` (with a legacy built-in
   ID mapping fallback) so a failed Piper warmup can restore the previous
   built-in or Piper provider, not only a previous Piper provider.
3. When a built-in provider is selected through the existing
   `set_tts_provider` flow, also persist/select its concrete registry ID when
   available, so choosing a built-in after Piper does not leave the Piper ID in
   settings.
4. Preserve loading cleanup and error display; do not add missing-model cards or
   an Add button. Keep changes frontend-only.

## Verification

- Run `npx vue-tsc --noEmit`.
- Do not build Windows.
