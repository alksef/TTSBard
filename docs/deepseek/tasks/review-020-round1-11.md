# Review 020 — typed unified settings reads in UI

Исправь только замечание про раздробленные IPC-read в UI. Security DTO/API keys не менять.

## Цель

Использовать уже существующий typed `AppSettingsDto` из `useAppSettings` как единый read-model для HotkeysPanel и AudioPanel. Отдельные IPC-команды записи оставить без изменений.

## Файлы

- `src/components/HotkeysPanel.vue`
- `src/components/AudioPanel.vue`

## Требования

1. `HotkeysPanel.vue` должен получать hotkeys через `useAppSettings()`/typed section (`settings.value?.hotkeys` или существующий typed helper), а не через `invoke('get_hotkey_settings')`.
2. После успешной записи/reset hotkey не мутировать отдельную копию как второй source of truth: либо обновить unified context через `reload()`, либо использовать безопасный единый механизм. Не добавлять новый IPC read.
3. `AudioPanel.vue` должен получать audio effects из `useAudioEffectsSettings()`, удалить `invoke('get_audio_effects')` и не поддерживать отдельный initial read для тех же данных.
4. Сохранить текущую UX-логику draft/dirty: внешнее обновление не затирает dirty draft, а после успешного сохранения локальный saved state синхронизируется.
5. Не менять backend, DTO, security-related fields и команды записи.
6. Не оставлять `invoke<any>`.

## Проверки

- `npx vue-tsc --noEmit`
- убедиться поиском, что в этих двух компонентах нет `get_hotkey_settings`/`get_audio_effects`.

Выполни изменения сейчас, не ограничивайся анализом или описанием. После правок повторно перечитай diff и исправь ошибки типов.
