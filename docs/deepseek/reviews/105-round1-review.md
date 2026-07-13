# Review 105 Round 1: AudioEffectsTab

## Verdict

APPROVED.

## Проверки

- `npx vue-tsc --noEmit` — passed independently.
- `git diff --check` — passed, кроме стандартных Windows LF/CRLF warnings.
- `AudioPanel.vue` оставлен оболочкой вкладок.
- `AudioEffectsTab.vue` содержит preview, voice effects, DeepFilterNet, DSP draft/save/cancel и прежние invoke-команды.
- `AudioDevicesTab.vue` не изменён.

## Следующий шаг

Вынести DSP-состояние и DSP-разметку в `DspSettings.vue`, передав в него только необходимые speaker settings для preview не нужно: preview остаётся в `AudioEffectsTab` и получает DSP settings через composable/props.
