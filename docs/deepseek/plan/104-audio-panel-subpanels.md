# Plan 104: Декомпозиция AudioPanel на подпанели

Основан на `docs/stage/35-audio-panel-subpanels.md`.

## Порядок реализации

1. Вынести вкладку устройств в `AudioDevicesTab.vue` и проверить её отдельно.
2. Вынести вкладку эффектов/preview в `AudioEffectsTab.vue`, сохранив draft и save/cancel поведение.
3. Вынести DSP-контейнер в `DspSettings.vue`.
4. При необходимости вынести EQ, Compressor и Limiter в отдельные компоненты после проверки границ props/events.

На первом раунде реализуется только пункт 1. Последующие раунды создаются по результатам независимого review.

## Ограничения первого раунда

- `src/components/AudioPanel.vue`
- новый `src/components/audio/AudioDevicesTab.vue` или аналогичный путь внутри `src/components`

Не менять backend, composables, DTO, визуальную тему и effects/DSP-логику.
