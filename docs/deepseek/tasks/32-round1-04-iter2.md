# Task 32-round1-04-iter2: исправить runtime-логику DSP presets

## Контекст review

В `src/components/AudioPanel.vue` после task 32-round1-04 обнаружены два runtime-багa:

1. `draftDsp = ref(createNaturalDsp())` находится выше `const createNaturalDsp = ...`, поэтому при выполнении setup script возможна обращаемая до инициализации const (`ReferenceError`).
2. `setDspPreset()` сначала устанавливает `dspPreset` в выбранное значение, затем вызывает `markDspDirty()`, а тот всегда переключает режим в `custom`. Поэтому нажатие Natural/Clear визуально сразу становится Custom.

Дополнительно текущая подпись boundary UI содержит «плавная склейка», хотя overlap-crossfade и lookahead отложены.

## Разрешённый файл

- `src/components/AudioPanel.vue`

Не изменять backend, `src-tauri/src/playback.rs`, `src-tauri/src/audio/device.rs` и другие файлы.

## Требуемые исправления

- Сделать функции создания пресетов hoisted function declarations либо разместить их до первого вызова; исключить TDZ/runtime ReferenceError.
- При выборе Natural/Clear сохранить выбранный `dspPreset` после вызова `markDspDirty` либо изменить логику так, чтобы ручное изменение переводило в Custom, а применение пресета оставалось Natural/Clear.
- При `cancelDsp()` заново вычислять `dspPreset` по восстановленному draft.
- Заменить misleading текст boundary hint на формулировку без обещания склейки, например «Исправление резких начал и концов фраз».
- Не менять существующее поведение сохранения/отмены и DeepFilterNet.

## Проверка

```text
npx vue-tsc --noEmit
```

Проверить фактический diff вручную, особенно порядок объявлений функций и `setDspPreset`/`markDspDirty`.
