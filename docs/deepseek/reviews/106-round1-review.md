# Review 106 Round 1: DspSettings

## Verdict

APPROVED.

## Проверки

- `npx vue-tsc --noEmit` — passed independently.
- `git diff --check` — passed, кроме стандартного Windows LF/CRLF warning.
- `AudioEffectsTab.vue` сохранил `draftDsp`, watcher, preview payload и save/cancel invoke-команды.
- `DspSettings.vue` отвечает только за DSP UI и использует props/events.

## Следующий шаг

Разделить DSP UI-контейнер на EQ, Compressor и Limiter с props/events. Общие DSP-стили не дублировать по дочерним компонентам.
