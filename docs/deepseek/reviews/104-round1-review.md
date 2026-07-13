# Review 104 Round 1: AudioDevicesTab

## Verdict

APPROVED для первого extraction.

## Проверки

- `npx vue-tsc --noEmit` — passed.
- `src/components/audio/AudioDevicesTab.vue` содержит загрузку устройств, настройки динамика, виртуального микрофона, тестирование, refresh и локальную обработку ошибок.
- В `AudioPanel.vue` удалены device refs/handlers/template; осталась только композиция вкладок и необходимые данные preview.
- Backend, composables и визуальное поведение не менялись.

## Следующий шаг

Вынести вкладку `effects_dsp` в `AudioEffectsTab.vue`. Её draft/save/cancel состояние должно переехать вместе с preview; после этого отдельно выделить `DspSettings` и его EQ/Compressor/Limiter.
