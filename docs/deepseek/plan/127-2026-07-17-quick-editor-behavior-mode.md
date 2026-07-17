# План 127: Режим поведения быстрого редактора

## Цель

Заменить checkbox быстрого редактора на radio-настройку с тремя режимами:
`disabled`, `collapse`, `return_focus`.

## Поведение

- `disabled`: Enter отправляет текст без скрытия окна; Esc не меняет visibility/focus;
- `collapse`: после Enter (после отправки в TTS) и Esc главное окно скрывается;
- `return_focus`: после Enter (после отправки в TTS) и Esc главное окно остаётся видимым,
  но вызывается `return_to_previous_window`.

## Совместимость

Старые JSON с `editor.quick: false/true` должны загружаться как `disabled/collapse`.
Новое сохранение использует строку enum; DTO и TypeScript используют union:
`'disabled' | 'collapse' | 'return_focus'`.

## UI

В `SettingsEditor.vue` сделать отдельную карточку «Быстрый редактор», под заголовком
мелким текстом: «Реакция на Enter, Esc и отправку текста в TTS». Ниже три radio-опции:
«Отключено», «Сворачивать», «Возвращать фокус».

Проверки: `cargo check`, `npx vue-tsc --noEmit`, `git diff --check` и ручная проверка
трёх режимов.
