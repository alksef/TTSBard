# Stage 35: Декомпозиция AudioPanel на подпанели

## Наблюдение

`src/components/AudioPanel.vue` содержит около 2100 строк и одновременно отвечает за:

1. устройства вывода и виртуальный микрофон;
2. загрузку аудиофайла и preview;
3. базовые voice-эффекты (pitch, speed, volume, DeepFilterNet);
4. DSP-пресеты и ручные EQ/compressor/limiter настройки;
5. сохранение/отмену нескольких независимых draft-состояний.

При этом слой настроек уже разделён: `useAudioSettings`, `useAudioEffectsSettings`, `useDspSettings`, а backend предоставляет отдельные DTO и команды. Значит, UI-декомпозиция соответствует существующей модели данных.

## Целевая структура

```text
AudioPanel.vue
├── AudioDevicesTab.vue
│   ├── SpeakerSection
│   └── VirtualMicSection
└── AudioEffectsTab.vue
    ├── AudioFilePreviewSection
    ├── VoiceEffectsSection
    └── DspSettings.vue
        ├── EqSection.vue
        ├── CompressorSection.vue
        └── LimiterSection.vue
```

Практическая граница первого этапа — вынести две вкладки (`AudioDevicesTab`, `AudioEffectsTab`) без изменения поведения. После этого DSP можно дробить отдельно, потому что он имеет собственные draft/save/cancel/preset состояния.

## Правила декомпозиции

- Не дублировать `invoke`-команды и не создавать второй источник истины.
- Состояние draft/save/cancel должно принадлежать соответствующему разделу.
- `AudioPanel` оставляет только вкладки, загрузку общих настроек/темы и общую оболочку.
- Preview должен получить через props/events или composable необходимые speaker device, volume, effects и DSP; нельзя связывать дочерние компоненты через скрытые глобальные refs.
- Стили общих контролов переиспользовать через существующие классы/общий style-файл, без визуального redesign.
- Удалённый `src/components/tts/AudioEffectsPanel.vue` не возвращать.

## Критерии

- `AudioPanel.vue` становится существенно меньше и отвечает за композицию вкладок.
- Поведение устройств, preview, эффектов и DSP сохраняется.
- `npx vue-tsc --noEmit` и `cargo check --manifest-path src-tauri/Cargo.toml` проходят.
- Каждый независимый extraction — отдельный проверяемый task/commit.
